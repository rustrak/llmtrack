//! OpenAI chat completions in, Anthropic Messages out, and back.
//!
//! The mapping is what most clients already expect from a gateway in front
//! of Claude:
//!
//! | OpenAI | Anthropic |
//! |---|---|
//! | `system` / `developer` messages | `system` (blocks when any carries `cache_control`) |
//! | `tool` messages | `tool_result` blocks in a user turn |
//! | `image_url`, `file` (PDF) parts | `image`, `document` blocks |
//! | `cache_control` on parts, messages and tools | kept where Anthropic accepts it |
//! | `reasoning_effort` | `thinking` with fixed budgets |
//! | `response_format` (JSON) | a forced `json_tool_call` tool, unwrapped on the way back |
//! | `parallel_tool_calls: false` | `tool_choice.disable_parallel_tool_use` |
//! | `stop`, `top_k`, `user` | `stop_sequences`, `top_k`, `metadata.user_id` |
//!
//! Thinking comes back as `reasoning_content` plus `thinking_blocks`, and
//! `thinking_blocks` on an assistant turn are sent back as thinking blocks,
//! which Anthropic requires when tools and thinking are used together.

use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::Usage;
use crate::error::{AppError, AppResult};

pub const API_VERSION: &str = "2023-06-01";
/// When neither the request nor the catalog says how long a reply may be.
pub const DEFAULT_MAX_TOKENS: u64 = 4096;
/// The tool a JSON `response_format` is turned into.
pub const JSON_TOOL: &str = "json_tool_call";

/// What the translation needs to know about the model, from the catalog.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModelFacts {
    pub max_output_tokens: Option<i64>,
}

/// Thinking budgets per `reasoning_effort`.
fn thinking_budget(effort: &str) -> Option<u64> {
    match effort {
        "minimal" | "low" => Some(1024),
        "medium" => Some(2048),
        "high" => Some(4096),
        "xhigh" => Some(8192),
        "max" => Some(16384),
        _ => None,
    }
}

/// Builds the Messages API body for an OpenAI chat completion request.
/// Returns the body and whether the reply must be unwrapped from the JSON tool.
pub fn to_anthropic_request(
    req: &Map<String, Value>,
    upstream_model: &str,
    facts: ModelFacts,
) -> AppResult<(Value, bool)> {
    let messages = req
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Validation("messages must be an array".into()))?;

    let mut system: Vec<Value> = Vec::new();
    let mut turns: Vec<Value> = Vec::new();
    for message in messages {
        let role = message["role"].as_str().unwrap_or_default();
        match role {
            "system" | "developer" => system.extend(text_blocks(message)),
            "user" => push_turn(&mut turns, "user", user_content(message)),
            "assistant" => push_turn(&mut turns, "assistant", assistant_content(message)),
            "tool" => {
                let mut result = json!({
                    "type": "tool_result",
                    "tool_use_id": message["tool_call_id"],
                    "content": text_of(&message["content"]),
                });
                copy_cache_control(message, &mut result);
                push_turn(&mut turns, "user", vec![result]);
            }
            other => {
                return Err(AppError::Validation(format!(
                    "unsupported message role '{other}'"
                )))
            }
        }
    }

    let asked_max_tokens = req
        .get("max_completion_tokens")
        .or_else(|| req.get("max_tokens"))
        .and_then(Value::as_u64);
    let mut max_tokens = asked_max_tokens
        .or_else(|| facts.max_output_tokens.and_then(|n| u64::try_from(n).ok()))
        .unwrap_or(DEFAULT_MAX_TOKENS);

    let mut body = json!({ "model": upstream_model, "messages": turns });
    let out = body.as_object_mut().expect("built as an object");
    if !system.is_empty() {
        let cached = system.iter().any(|b| b.get("cache_control").is_some());
        out.insert(
            "system".into(),
            if cached {
                Value::Array(system)
            } else {
                Value::String(
                    system
                        .iter()
                        .filter_map(|b| b["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                )
            },
        );
    }
    for field in ["temperature", "top_p", "top_k", "stream"] {
        if let Some(value) = req.get(field) {
            out.insert(field.into(), value.clone());
        }
    }
    match req.get("stop") {
        Some(Value::String(stop)) => {
            out.insert("stop_sequences".into(), json!([stop]));
        }
        Some(stop @ Value::Array(_)) => {
            out.insert("stop_sequences".into(), stop.clone());
        }
        _ => {}
    }
    if let Some(user) = req.get("user").and_then(Value::as_str) {
        out.insert("metadata".into(), json!({ "user_id": user }));
    }

    // Thinking: passed through as given, or from `reasoning_effort`.
    let thinking = match (req.get("thinking"), req.get("reasoning_effort").and_then(Value::as_str)) {
        (Some(thinking), _) => Some(thinking.clone()),
        (None, Some(effort)) => match thinking_budget(effort) {
            Some(_) if asked_max_tokens.is_some_and(|m| m <= 1024) => None,
            // A limit the client set caps the budget; our
            // own default makes room for it instead.
            Some(budget) => Some(json!({
                "type": "enabled",
                "budget_tokens": match asked_max_tokens {
                    Some(asked) => budget.min(asked - 1),
                    None => budget,
                },
            })),
            None if effort == "none" => None,
            None => {
                return Err(AppError::Validation(format!(
                    "reasoning_effort must be minimal, low, medium, high, xhigh, max or none, not '{effort}'"
                )))
            }
        },
        (None, None) => None,
    };
    let thinking_on = thinking.as_ref().is_some_and(|t| t["type"] != "disabled");
    if let Some(thinking) = thinking {
        if let Some(budget) = thinking["budget_tokens"].as_u64() {
            max_tokens = max_tokens.max(budget + 1);
        }
        out.insert("thinking".into(), thinking);
    }
    if thinking_on {
        // Anthropic refuses sampling settings while thinking.
        for field in ["temperature", "top_p", "top_k"] {
            out.remove(field);
        }
    }
    out.insert("max_tokens".into(), json!(max_tokens));

    let mut tools: Vec<Value> = req
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|tool| {
            let function = &tool["function"];
            let mut translated = json!({
                "name": function["name"],
                "input_schema": function
                    .get("parameters")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object", "properties": {}})),
            });
            if let Some(description) = function.get("description") {
                translated["description"] = description.clone();
            }
            if let Some(cache) = tool
                .get("cache_control")
                .or_else(|| function.get("cache_control"))
            {
                translated["cache_control"] = cache.clone();
            }
            translated
        })
        .collect();

    let mut tool_choice = req.get("tool_choice").map(|choice| match choice {
        Value::String(s) if s == "required" => json!({"type": "any"}),
        Value::String(s) if s == "none" => json!({"type": "none"}),
        Value::Object(o) => json!({"type": "tool", "name": o["function"]["name"]}),
        _ => json!({"type": "auto"}),
    });

    // JSON output through a forced tool, which every Claude model honours.
    // Anthropic does not allow forcing a tool while thinking, so then the
    // schema is only offered.
    let json_schema = match req.get("response_format") {
        Some(format) if format["type"] == "json_schema" => Some(
            format["json_schema"]["schema"]
                .clone()
                .as_object()
                .map(|_| format["json_schema"]["schema"].clone())
                .unwrap_or_else(|| json!({"type": "object"})),
        ),
        Some(format) if format["type"] == "json_object" => Some(json!({"type": "object"})),
        _ => None,
    };
    let json_mode = json_schema.is_some();
    if let Some(schema) = json_schema {
        tools.push(json!({
            "name": JSON_TOOL,
            "description": "Respond with the requested JSON.",
            "input_schema": schema,
        }));
        if !thinking_on {
            tool_choice = Some(json!({"type": "tool", "name": JSON_TOOL}));
        }
    }

    if req.get("parallel_tool_calls") == Some(&Value::Bool(false)) {
        let choice = tool_choice.get_or_insert_with(|| json!({"type": "auto"}));
        if choice["type"] != "none" {
            choice["disable_parallel_tool_use"] = Value::Bool(true);
        }
    }
    if !tools.is_empty() {
        out.insert("tools".into(), Value::Array(tools));
    }
    if let Some(choice) = tool_choice {
        out.insert("tool_choice".into(), choice);
    }
    Ok((body, json_mode))
}

/// Anthropic rejects two turns in a row from the same role, which OpenAI
/// allows (several tool results, say), so they are merged.
fn push_turn(turns: &mut Vec<Value>, role: &str, content: Vec<Value>) {
    if let Some(last) = turns.last_mut() {
        if last["role"] == role {
            if let Some(blocks) = last["content"].as_array_mut() {
                blocks.extend(content);
                return;
            }
        }
    }
    turns.push(json!({ "role": role, "content": content }));
}

/// The text of a content that is a string or an array of parts.
fn text_of(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// A message-level `cache_control` lands on the last block it produced.
fn copy_cache_control(from: &Value, to: &mut Value) {
    if let Some(cache) = from.get("cache_control") {
        to["cache_control"] = cache.clone();
    }
}

fn text_blocks(message: &Value) -> Vec<Value> {
    let mut blocks: Vec<Value> = match &message["content"] {
        Value::Array(parts) => parts
            .iter()
            .filter(|p| p["type"] == "text")
            .map(|p| {
                let mut block = json!({"type": "text", "text": p["text"]});
                copy_cache_control(p, &mut block);
                block
            })
            .collect(),
        other => vec![json!({"type": "text", "text": text_of(other)})],
    };
    if let Some(last) = blocks.last_mut() {
        copy_cache_control(message, last);
    }
    blocks
}

fn user_content(message: &Value) -> Vec<Value> {
    let mut blocks: Vec<Value> = match &message["content"] {
        Value::Array(parts) => parts.iter().filter_map(user_part).collect(),
        other => vec![json!({"type": "text", "text": text_of(other)})],
    };
    if let Some(last) = blocks.last_mut() {
        copy_cache_control(message, last);
    }
    blocks
}

/// `data:<media type>;base64,<data>` → (media type, data).
fn data_url(url: &str) -> Option<(&str, &str)> {
    url.strip_prefix("data:")?.split_once(";base64,")
}

fn user_part(part: &Value) -> Option<Value> {
    let mut block = match part["type"].as_str()? {
        "text" => json!({"type": "text", "text": part["text"]}),
        "image_url" => {
            let url = part["image_url"]["url"].as_str()?;
            let source = match data_url(url) {
                Some((media_type, data)) => {
                    json!({"type": "base64", "media_type": media_type, "data": data})
                }
                None => json!({"type": "url", "url": url}),
            };
            json!({"type": "image", "source": source})
        }
        "file" => {
            let data = part["file"]["file_data"].as_str()?;
            let (media_type, data) = data_url(data)?;
            json!({"type": "document", "source": {"type": "base64", "media_type": media_type, "data": data}})
        }
        _ => return None,
    };
    copy_cache_control(part, &mut block);
    Some(block)
}

fn assistant_content(message: &Value) -> Vec<Value> {
    // Thinking first: Anthropic wants the reasoning before what it led to.
    let mut blocks: Vec<Value> = message["thinking_blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .cloned()
        .collect();
    let text = text_of(&message["content"]);
    if !text.is_empty() {
        blocks.push(json!({"type": "text", "text": text}));
    }
    for call in message["tool_calls"].as_array().into_iter().flatten() {
        let arguments = call["function"]["arguments"].as_str().unwrap_or("{}");
        blocks.push(json!({
            "type": "tool_use",
            "id": call["id"],
            "name": call["function"]["name"],
            "input": serde_json::from_str::<Value>(arguments).unwrap_or_else(|_| json!({})),
        }));
    }
    if let Some(last) = blocks.last_mut() {
        copy_cache_control(message, last);
    }
    blocks
}

fn finish_reason(stop_reason: &Value, json_mode: bool) -> Value {
    match stop_reason.as_str() {
        Some("max_tokens") => json!("length"),
        Some("tool_use") if json_mode => json!("stop"),
        Some("tool_use") => json!("tool_calls"),
        Some(_) => json!("stop"),
        None => Value::Null,
    }
}

/// Turns a Messages API response into a chat completion.
pub fn to_openai_response(message: &Value, model: &str, json_mode: bool) -> (Value, Usage) {
    let usage = Usage::from_anthropic(&message["usage"]);
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut thinking_blocks = Vec::new();
    let mut tool_calls = Vec::new();
    for block in message["content"].as_array().into_iter().flatten() {
        match block["type"].as_str() {
            Some("text") => text.push_str(block["text"].as_str().unwrap_or_default()),
            Some("thinking") => {
                reasoning.push_str(block["thinking"].as_str().unwrap_or_default());
                thinking_blocks.push(block.clone());
            }
            Some("redacted_thinking") => thinking_blocks.push(block.clone()),
            Some("tool_use") if json_mode && block["name"] == JSON_TOOL => {
                text.push_str(&block["input"].to_string());
            }
            Some("tool_use") => tool_calls.push(json!({
                "id": block["id"],
                "type": "function",
                "function": {"name": block["name"], "arguments": block["input"].to_string()},
            })),
            _ => {}
        }
    }
    let mut reply = json!({
        "role": "assistant",
        "content": if text.is_empty() { Value::Null } else { Value::String(text) },
    });
    if !tool_calls.is_empty() {
        reply["tool_calls"] = Value::Array(tool_calls);
    }
    if !thinking_blocks.is_empty() {
        reply["reasoning_content"] = Value::String(reasoning);
        reply["thinking_blocks"] = Value::Array(thinking_blocks);
    }
    let body = json!({
        "id": message["id"],
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [{"index": 0, "message": reply, "finish_reason": finish_reason(&message["stop_reason"], json_mode)}],
        "usage": usage.to_json(),
    });
    (body, usage)
}

/// Anthropic's error body in OpenAI's shape.
pub fn to_openai_error(body: &Value) -> Value {
    json!({"error": {
        "message": body["error"]["message"].as_str().unwrap_or("provider error"),
        "type": body["error"]["type"].as_str().unwrap_or("provider_error"),
    }})
}

/// What an Anthropic content block streams as.
enum Block {
    Text,
    Thinking,
    Tool(usize),
    /// The JSON-mode tool: its input streams as content.
    JsonTool,
    Other,
}

/// Rewrites a Messages stream into chat completion chunks, one event at a
/// time.
pub struct StreamTranslator {
    id: String,
    model: String,
    created: i64,
    include_usage: bool,
    json_mode: bool,
    blocks: HashMap<u64, Block>,
    tool_calls: usize,
    pub usage: Usage,
}

impl StreamTranslator {
    pub fn new(model: &str, include_usage: bool, json_mode: bool) -> Self {
        Self {
            id: String::new(),
            model: model.to_string(),
            created: chrono::Utc::now().timestamp(),
            include_usage,
            json_mode,
            blocks: HashMap::new(),
            tool_calls: 0,
            usage: Usage::default(),
        }
    }

    fn chunk(&self, delta: Value, finish: Value) -> String {
        json!({
            "id": self.id,
            "object": "chat.completion.chunk",
            "created": self.created,
            "model": self.model,
            "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
        })
        .to_string()
    }

    /// The `data:` payloads to send for one Anthropic event's data.
    pub fn on_event(&mut self, data: &str) -> Vec<String> {
        let Ok(event) = serde_json::from_str::<Value>(data) else {
            return Vec::new();
        };
        let index = event["index"].as_u64().unwrap_or(0);
        match event["type"].as_str().unwrap_or_default() {
            "message_start" => {
                let message = &event["message"];
                self.id = message["id"].as_str().unwrap_or_default().to_string();
                self.usage = Usage::from_anthropic(&message["usage"]);
                vec![self.chunk(json!({"role": "assistant", "content": ""}), Value::Null)]
            }
            "content_block_start" => {
                let block = &event["content_block"];
                let kind = match block["type"].as_str() {
                    Some("text") => Block::Text,
                    Some("thinking") => Block::Thinking,
                    Some("tool_use") if self.json_mode && block["name"] == JSON_TOOL => {
                        Block::JsonTool
                    }
                    Some("tool_use") => {
                        self.tool_calls += 1;
                        Block::Tool(self.tool_calls - 1)
                    }
                    _ => Block::Other,
                };
                let out = match kind {
                    Block::Tool(n) => vec![self.chunk(
                        json!({"tool_calls": [{"index": n, "id": block["id"], "type": "function",
                                               "function": {"name": block["name"], "arguments": ""}}]}),
                        Value::Null,
                    )],
                    _ => Vec::new(),
                };
                self.blocks.insert(index, kind);
                out
            }
            "content_block_delta" => {
                let delta = &event["delta"];
                let kind = self.blocks.get(&index).unwrap_or(&Block::Other);
                let out = match (delta["type"].as_str(), kind) {
                    (Some("text_delta"), _) => json!({"content": delta["text"]}),
                    (Some("thinking_delta"), _) => json!({
                        "reasoning_content": delta["thinking"],
                        "thinking_blocks": [{"type": "thinking", "thinking": delta["thinking"]}],
                    }),
                    (Some("signature_delta"), _) => json!({
                        "reasoning_content": "",
                        "thinking_blocks": [{"type": "thinking", "thinking": "", "signature": delta["signature"]}],
                    }),
                    (Some("input_json_delta"), Block::JsonTool) => {
                        json!({"content": delta["partial_json"]})
                    }
                    (Some("input_json_delta"), Block::Tool(n)) => json!({
                        "tool_calls": [{"index": n, "function": {"arguments": delta["partial_json"]}}]
                    }),
                    _ => return Vec::new(),
                };
                vec![self.chunk(out, Value::Null)]
            }
            "message_delta" => {
                // Newer API versions repeat the input counts here; take
                // whatever is present.
                let usage = &event["usage"];
                if let Some(output) = usage["output_tokens"].as_i64() {
                    self.usage.completion_tokens = output;
                }
                if usage.get("input_tokens").is_some() {
                    let output = self.usage.completion_tokens;
                    self.usage = Usage {
                        completion_tokens: output,
                        ..Usage::from_anthropic(usage)
                    };
                }
                vec![self.chunk(
                    json!({}),
                    finish_reason(&event["delta"]["stop_reason"], self.json_mode),
                )]
            }
            "message_stop" => {
                let mut out = Vec::new();
                if self.include_usage {
                    out.push(
                        json!({
                            "id": self.id, "object": "chat.completion.chunk", "created": self.created,
                            "model": self.model, "choices": [], "usage": self.usage.to_json(),
                        })
                        .to_string(),
                    );
                }
                out.push("[DONE]".into());
                out
            }
            "error" => vec![to_openai_error(&event).to_string(), "[DONE]".into()],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(body: Value) -> Value {
        to_anthropic_request(body.as_object().unwrap(), "claude-x", ModelFacts::default())
            .unwrap()
            .0
    }

    #[test]
    fn system_messages_move_to_the_system_field() {
        let out = request(json!({"messages": [
            {"role": "system", "content": "A"},
            {"role": "developer", "content": [{"type": "text", "text": "B"}]},
            {"role": "user", "content": "hi"}
        ]}));
        assert_eq!(out["system"], "A\n\nB");
        assert_eq!(out["model"], "claude-x");
        assert_eq!(out["max_tokens"], DEFAULT_MAX_TOKENS);
        assert_eq!(
            out["messages"],
            json!([{"role": "user", "content": [{"type": "text", "text": "hi"}]}])
        );
    }

    #[test]
    fn the_catalog_sets_the_default_reply_length() {
        let body = json!({"messages": [{"role": "user", "content": "hi"}]});
        let facts = ModelFacts {
            max_output_tokens: Some(64_000),
        };
        let (out, _) = to_anthropic_request(body.as_object().unwrap(), "m", facts).unwrap();
        assert_eq!(out["max_tokens"], 64_000);
    }

    #[test]
    fn sampling_stop_and_limits_are_mapped() {
        let out = request(json!({"messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 50, "temperature": 0.2, "top_k": 5, "stop": "END", "user": "u1", "stream": true}));
        assert_eq!(out["max_tokens"], 50);
        assert_eq!(out["temperature"], 0.2);
        assert_eq!(out["top_k"], 5);
        assert_eq!(out["stop_sequences"], json!(["END"]));
        assert_eq!(out["metadata"]["user_id"], "u1");
        assert_eq!(out["stream"], true);
    }

    #[test]
    fn images_and_pdfs_become_blocks() {
        let out = request(json!({"messages": [{"role": "user", "content": [
            {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}},
            {"type": "image_url", "image_url": {"url": "https://x.test/cat.jpg"}},
            {"type": "file", "file": {"file_data": "data:application/pdf;base64,JVBE"}}
        ]}]}));
        let blocks = &out["messages"][0]["content"];
        assert_eq!(
            blocks[0]["source"],
            json!({"type": "base64", "media_type": "image/png", "data": "AAAA"})
        );
        assert_eq!(
            blocks[1]["source"],
            json!({"type": "url", "url": "https://x.test/cat.jpg"})
        );
        assert_eq!(blocks[2]["type"], "document");
        assert_eq!(blocks[2]["source"]["media_type"], "application/pdf");
    }

    #[test]
    fn cache_control_is_kept_on_parts_messages_system_and_tools() {
        let out = request(json!({
            "messages": [
                {"role": "system", "content": [{"type": "text", "text": "long rules", "cache_control": {"type": "ephemeral"}}]},
                {"role": "user", "content": "doc", "cache_control": {"type": "ephemeral"}}
            ],
            "tools": [{"type": "function", "function": {"name": "f", "parameters": {"type": "object"}}, "cache_control": {"type": "ephemeral"}}]
        }));
        assert_eq!(out["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(
            out["messages"][0]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );
        assert_eq!(out["tools"][0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn reasoning_effort_becomes_a_thinking_budget_and_drops_sampling() {
        let out = request(json!({"messages": [{"role": "user", "content": "hi"}],
            "reasoning_effort": "high", "temperature": 0.5}));
        assert_eq!(
            out["thinking"],
            json!({"type": "enabled", "budget_tokens": 4096})
        );
        assert!(out.get("temperature").is_none());

        let tiny = request(json!({"messages": [{"role": "user", "content": "hi"}],
            "reasoning_effort": "high", "max_tokens": 1000}));
        assert!(tiny.get("thinking").is_none(), "no room to think");

        let none = request(
            json!({"messages": [{"role": "user", "content": "hi"}], "reasoning_effort": "none"}),
        );
        assert!(none.get("thinking").is_none());

        let body =
            json!({"messages": [{"role": "user", "content": "hi"}], "reasoning_effort": "ultra"});
        assert!(
            to_anthropic_request(body.as_object().unwrap(), "m", ModelFacts::default()).is_err()
        );
    }

    #[test]
    fn an_explicit_thinking_param_passes_through() {
        let out = request(json!({"messages": [{"role": "user", "content": "hi"}],
            "thinking": {"type": "enabled", "budget_tokens": 8000}, "max_tokens": 4000}));
        assert_eq!(out["thinking"]["budget_tokens"], 8000);
        assert_eq!(out["max_tokens"], 8001, "max_tokens must exceed the budget");
    }

    #[test]
    fn json_response_format_forces_the_json_tool() {
        let schema = json!({"type": "object", "properties": {"a": {"type": "string"}}});
        let (out, json_mode) = to_anthropic_request(
            json!({"messages": [{"role": "user", "content": "hi"}],
                   "response_format": {"type": "json_schema", "json_schema": {"name": "x", "schema": schema}}})
            .as_object()
            .unwrap(),
            "m",
            ModelFacts::default(),
        )
        .unwrap();
        assert!(json_mode);
        assert_eq!(out["tools"][0]["name"], JSON_TOOL);
        assert_eq!(out["tools"][0]["input_schema"], schema);
        assert_eq!(
            out["tool_choice"],
            json!({"type": "tool", "name": JSON_TOOL})
        );

        let (reply, _) = to_openai_response(
            &json!({"id": "m", "stop_reason": "tool_use", "usage": {},
                    "content": [{"type": "tool_use", "name": JSON_TOOL, "input": {"a": "b"}}]}),
            "m",
            true,
        );
        assert_eq!(reply["choices"][0]["message"]["content"], "{\"a\":\"b\"}");
        assert_eq!(reply["choices"][0]["finish_reason"], "stop");
        assert!(reply["choices"][0]["message"].get("tool_calls").is_none());
    }

    #[test]
    fn parallel_tool_calls_false_disables_parallel_use() {
        let out = request(json!({"messages": [{"role": "user", "content": "hi"}],
            "tools": [{"type": "function", "function": {"name": "f"}}], "parallel_tool_calls": false}));
        assert_eq!(
            out["tool_choice"],
            json!({"type": "auto", "disable_parallel_tool_use": true})
        );
    }

    #[test]
    fn tool_calls_results_and_thinking_round_trip() {
        let out = request(json!({
            "messages": [
                {"role": "user", "content": "weather?"},
                {"role": "assistant", "content": null,
                 "thinking_blocks": [{"type": "thinking", "thinking": "need tool", "signature": "sig"}],
                 "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "get_weather", "arguments": "{\"city\":\"Paris\"}"}}]},
                {"role": "tool", "tool_call_id": "call_1", "content": "sunny"},
                {"role": "user", "content": "thanks"}
            ],
            "tools": [{"type": "function", "function": {"name": "get_weather", "parameters": {"type": "object"}}}],
            "tool_choice": "required"
        }));
        let turns = out["messages"].as_array().unwrap();
        assert_eq!(
            turns.len(),
            3,
            "the tool result and the next user turn merge"
        );
        assert_eq!(turns[1]["content"][0]["type"], "thinking");
        assert_eq!(turns[1]["content"][1]["input"], json!({"city": "Paris"}));
        assert_eq!(turns[2]["content"][0]["type"], "tool_result");
        assert_eq!(turns[2]["content"][1]["text"], "thanks");
        assert_eq!(out["tool_choice"], json!({"type": "any"}));
    }

    #[test]
    fn a_response_with_thinking_and_cache_becomes_a_chat_completion() {
        let (body, usage) = to_openai_response(
            &json!({"id": "msg_1", "content": [
                {"type": "thinking", "thinking": "hmm", "signature": "s"},
                {"type": "text", "text": "Checking."},
                {"type": "tool_use", "id": "toolu_1", "name": "get_weather", "input": {"city": "Paris"}}
            ], "stop_reason": "tool_use",
               "usage": {"input_tokens": 5, "cache_read_input_tokens": 100, "cache_creation_input_tokens": 20, "output_tokens": 7}}),
            "claude",
            false,
        );
        let message = &body["choices"][0]["message"];
        assert_eq!(body["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(message["content"], "Checking.");
        assert_eq!(message["reasoning_content"], "hmm");
        assert_eq!(message["thinking_blocks"][0]["signature"], "s");
        assert_eq!(
            message["tool_calls"][0]["function"]["arguments"],
            "{\"city\":\"Paris\"}"
        );
        assert_eq!(usage.prompt_tokens, 125);
        assert_eq!(usage.cached_tokens, 100);
        assert_eq!(usage.cache_write_tokens, 20);
        assert_eq!(body["usage"]["prompt_tokens_details"]["cached_tokens"], 100);
    }

    #[test]
    fn a_stream_with_thinking_and_a_tool_call_translates_event_by_event() {
        let mut t = StreamTranslator::new("claude", true, false);
        let mut out = Vec::new();
        for event in [
            json!({"type": "message_start", "message": {"id": "msg_1", "usage": {"input_tokens": 3, "cache_read_input_tokens": 10}}}),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking"}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "plan"}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "sig"}}),
            json!({"type": "content_block_start", "index": 1, "content_block": {"type": "tool_use", "id": "t1", "name": "f"}}),
            json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"a\":"}}),
            json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 9}}),
            json!({"type": "message_stop"}),
        ] {
            out.extend(t.on_event(&event.to_string()));
        }
        let chunks: Vec<Value> = out[..out.len() - 1]
            .iter()
            .map(|c| serde_json::from_str(c).unwrap())
            .collect();
        assert_eq!(
            chunks[1]["choices"][0]["delta"]["reasoning_content"],
            "plan"
        );
        assert_eq!(
            chunks[2]["choices"][0]["delta"]["thinking_blocks"][0]["signature"],
            "sig"
        );
        assert_eq!(
            chunks[3]["choices"][0]["delta"]["tool_calls"][0]["id"],
            "t1"
        );
        assert_eq!(
            chunks[4]["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"],
            "{\"a\":"
        );
        assert_eq!(chunks[5]["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(chunks[6]["usage"]["completion_tokens"], 9);
        assert_eq!(out.last().unwrap(), "[DONE]");
        assert_eq!(t.usage.prompt_tokens, 13);
        assert_eq!(t.usage.cached_tokens, 10);
    }

    #[test]
    fn a_json_mode_stream_streams_the_tool_input_as_content() {
        let mut t = StreamTranslator::new("claude", false, true);
        let mut content = String::new();
        for event in [
            json!({"type": "message_start", "message": {"id": "m", "usage": {}}}),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "t", "name": JSON_TOOL}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{\"a\":1}"}}),
        ] {
            for chunk in t.on_event(&event.to_string()) {
                let chunk: Value = serde_json::from_str(&chunk).unwrap();
                content.push_str(
                    chunk["choices"][0]["delta"]["content"]
                        .as_str()
                        .unwrap_or_default(),
                );
            }
        }
        assert_eq!(content, "{\"a\":1}");
    }
}
