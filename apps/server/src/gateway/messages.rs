//! Anthropic Messages in, OpenAI chat completions out, and back.
//!
//! The reverse of [`super::anthropic`]: it lets clients that speak Anthropic's
//! API (its SDKs, Claude Code) use any model behind the gateway.

use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::Usage;

/// Builds an OpenAI chat completion body from a Messages API body.
pub fn to_openai_request(req: &Map<String, Value>, upstream_model: &str) -> Value {
    let mut messages = Vec::new();
    match req.get("system") {
        Some(Value::String(system)) => messages.push(json!({"role": "system", "content": system})),
        Some(Value::Array(blocks)) => {
            messages.push(json!({"role": "system", "content": joined_text(blocks)}))
        }
        _ => {}
    }
    for turn in req
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let blocks = match &turn["content"] {
            Value::String(text) => vec![json!({"type": "text", "text": text})],
            Value::Array(blocks) => blocks.clone(),
            _ => Vec::new(),
        };
        if turn["role"] == "assistant" {
            messages.push(assistant_message(&blocks));
        } else {
            user_messages(&blocks, &mut messages);
        }
    }

    let mut body = json!({ "model": upstream_model, "messages": messages });
    let out = body.as_object_mut().expect("built as an object");
    for field in ["max_tokens", "temperature", "top_p", "stream"] {
        if let Some(value) = req.get(field) {
            out.insert(field.into(), value.clone());
        }
    }
    if let Some(stop) = req.get("stop_sequences") {
        out.insert("stop".into(), stop.clone());
    }
    if let Some(user) = req.get("metadata").and_then(|m| m["user_id"].as_str()) {
        out.insert("user".into(), json!(user));
    }
    if let Some(budget) = req
        .get("thinking")
        .and_then(|t| t["budget_tokens"].as_u64())
    {
        let effort = match budget {
            0..=1024 => "low",
            1025..=2048 => "medium",
            _ => "high",
        };
        out.insert("reasoning_effort".into(), json!(effort));
    }
    if let Some(tools) = req.get("tools").and_then(Value::as_array) {
        let tools: Vec<Value> = tools
            .iter()
            .map(|tool| {
                json!({"type": "function", "function": {
                    "name": tool["name"],
                    "description": tool.get("description").cloned().unwrap_or(Value::Null),
                    "parameters": tool.get("input_schema").cloned().unwrap_or_else(|| json!({"type": "object"})),
                }})
            })
            .collect();
        out.insert("tools".into(), Value::Array(tools));
    }
    if let Some(choice) = req.get("tool_choice") {
        let mapped = match choice["type"].as_str() {
            Some("any") => json!("required"),
            Some("none") => json!("none"),
            Some("tool") => json!({"type": "function", "function": {"name": choice["name"]}}),
            _ => json!("auto"),
        };
        out.insert("tool_choice".into(), mapped);
        if choice["disable_parallel_tool_use"] == true {
            out.insert("parallel_tool_calls".into(), json!(false));
        }
    }
    body
}

fn joined_text(blocks: &[Value]) -> String {
    blocks
        .iter()
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn assistant_message(blocks: &[Value]) -> Value {
    let text: String = blocks
        .iter()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect();
    let calls: Vec<Value> = blocks
        .iter()
        .filter(|b| b["type"] == "tool_use")
        .map(|b| {
            json!({"id": b["id"], "type": "function",
                   "function": {"name": b["name"], "arguments": b["input"].to_string()}})
        })
        .collect();
    let mut message = json!({
        "role": "assistant",
        "content": if text.is_empty() { Value::Null } else { Value::String(text) },
    });
    if !calls.is_empty() {
        message["tool_calls"] = Value::Array(calls);
    }
    message
}

/// A user turn: its tool results become `tool` messages (OpenAI wants them
/// first, right after the assistant turn that called), the rest one user message.
fn user_messages(blocks: &[Value], messages: &mut Vec<Value>) {
    let mut parts = Vec::new();
    for block in blocks {
        match block["type"].as_str() {
            Some("tool_result") => {
                let content = match &block["content"] {
                    Value::String(text) => text.clone(),
                    Value::Array(inner) => joined_text(inner),
                    _ => String::new(),
                };
                messages.push(json!({"role": "tool", "tool_call_id": block["tool_use_id"], "content": content}));
            }
            Some("text") => parts.push(json!({"type": "text", "text": block["text"]})),
            Some("image") => {
                let source = &block["source"];
                let url = match source["type"].as_str() {
                    Some("base64") => format!(
                        "data:{};base64,{}",
                        source["media_type"].as_str().unwrap_or("image/png"),
                        source["data"].as_str().unwrap_or_default()
                    ),
                    _ => source["url"].as_str().unwrap_or_default().to_string(),
                };
                parts.push(json!({"type": "image_url", "image_url": {"url": url}}));
            }
            Some("document") => {
                let source = &block["source"];
                parts.push(json!({"type": "file", "file": {"file_data": format!(
                    "data:{};base64,{}",
                    source["media_type"].as_str().unwrap_or("application/pdf"),
                    source["data"].as_str().unwrap_or_default()
                )}}));
            }
            _ => {}
        }
    }
    if !parts.is_empty() {
        messages.push(json!({"role": "user", "content": parts}));
    }
}

fn stop_reason(finish: &Value) -> Value {
    match finish.as_str() {
        Some("length") => json!("max_tokens"),
        Some("tool_calls") | Some("function_call") => json!("tool_use"),
        Some(_) => json!("end_turn"),
        None => Value::Null,
    }
}

/// Anthropic counts input without the cache.
fn anthropic_usage(usage: Usage) -> Value {
    json!({
        "input_tokens": (usage.prompt_tokens - usage.cached_tokens).max(0),
        "output_tokens": usage.completion_tokens,
        "cache_read_input_tokens": usage.cached_tokens,
    })
}

/// Turns a chat completion into a Messages API response.
pub fn to_anthropic_response(completion: &Value, model: &str) -> (Value, Usage) {
    let usage = Usage::from_openai(&completion["usage"]).unwrap_or_default();
    let choice = &completion["choices"][0];
    let message = &choice["message"];
    let mut content = Vec::new();
    if let Some(text) = message["content"].as_str().filter(|t| !t.is_empty()) {
        content.push(json!({"type": "text", "text": text}));
    }
    for call in message["tool_calls"].as_array().into_iter().flatten() {
        let arguments = call["function"]["arguments"].as_str().unwrap_or("{}");
        content.push(json!({
            "type": "tool_use",
            "id": call["id"],
            "name": call["function"]["name"],
            "input": serde_json::from_str::<Value>(arguments).unwrap_or_else(|_| json!({})),
        }));
    }
    let body = json!({
        "id": completion["id"],
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content,
        "stop_reason": stop_reason(&choice["finish_reason"]),
        "stop_sequence": null,
        "usage": anthropic_usage(usage),
    });
    (body, usage)
}

/// An OpenAI-shaped error as Anthropic's.
pub fn to_anthropic_error(body: &Value) -> Value {
    json!({"type": "error", "error": {
        "type": body["error"]["type"].as_str().unwrap_or("api_error"),
        "message": body["error"]["message"].as_str().unwrap_or("provider error"),
    }})
}

#[derive(Clone, Copy, PartialEq)]
enum Open {
    Text(usize),
    Tool(usize),
}

/// Rewrites a chat completion stream into Messages API events.
pub struct StreamTranslator {
    model: String,
    started: bool,
    open: Option<Open>,
    next_index: usize,
    /// OpenAI tool call index to Anthropic block index.
    tools: HashMap<u64, usize>,
    stop: Value,
    pub usage: Usage,
}

fn event(kind: &str, data: Value) -> String {
    format!("event: {kind}\ndata: {data}\n\n")
}

impl StreamTranslator {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            started: false,
            open: None,
            next_index: 0,
            tools: HashMap::new(),
            stop: Value::Null,
            usage: Usage::default(),
        }
    }

    fn close(&mut self, out: &mut String) {
        if let Some(Open::Text(i) | Open::Tool(i)) = self.open.take() {
            out.push_str(&event(
                "content_block_stop",
                json!({"type": "content_block_stop", "index": i}),
            ));
        }
    }

    /// The events for one chunk's `data:` payload.
    pub fn on_chunk(&mut self, data: &str) -> String {
        let mut out = String::new();
        if data == "[DONE]" {
            return self.finish();
        }
        let Ok(chunk) = serde_json::from_str::<Value>(data) else {
            return out;
        };
        if !self.started {
            self.started = true;
            out.push_str(&event(
                "message_start",
                json!({"type": "message_start", "message": {
                    "id": chunk["id"], "type": "message", "role": "assistant", "model": self.model,
                    "content": [], "stop_reason": null, "stop_sequence": null,
                    "usage": {"input_tokens": 0, "output_tokens": 0},
                }}),
            ));
        }
        if let Some(usage) = Usage::from_openai(&chunk["usage"]) {
            self.usage = usage;
        }
        let choice = &chunk["choices"][0];
        let delta = &choice["delta"];
        if let Some(text) = delta["content"].as_str().filter(|t| !t.is_empty()) {
            let index = match self.open {
                Some(Open::Text(i)) => i,
                _ => {
                    self.close(&mut out);
                    let i = self.next_index;
                    self.next_index += 1;
                    self.open = Some(Open::Text(i));
                    out.push_str(&event(
                        "content_block_start",
                        json!({"type": "content_block_start", "index": i,
                        "content_block": {"type": "text", "text": ""}}),
                    ));
                    i
                }
            };
            out.push_str(&event(
                "content_block_delta",
                json!({"type": "content_block_delta", "index": index,
                "delta": {"type": "text_delta", "text": text}}),
            ));
        }
        for call in delta["tool_calls"].as_array().into_iter().flatten() {
            let openai_index = call["index"].as_u64().unwrap_or(0);
            let index = match self.tools.get(&openai_index) {
                Some(i) => *i,
                None => {
                    self.close(&mut out);
                    let i = self.next_index;
                    self.next_index += 1;
                    self.tools.insert(openai_index, i);
                    self.open = Some(Open::Tool(i));
                    out.push_str(&event("content_block_start", json!({"type": "content_block_start", "index": i,
                        "content_block": {"type": "tool_use", "id": call["id"], "name": call["function"]["name"], "input": {}}})));
                    i
                }
            };
            if let Some(arguments) = call["function"]["arguments"]
                .as_str()
                .filter(|a| !a.is_empty())
            {
                out.push_str(&event(
                    "content_block_delta",
                    json!({"type": "content_block_delta", "index": index,
                    "delta": {"type": "input_json_delta", "partial_json": arguments}}),
                ));
            }
        }
        if !choice["finish_reason"].is_null() {
            self.stop = stop_reason(&choice["finish_reason"]);
        }
        out
    }

    /// Closes the message; also called when the stream ends without `[DONE]`.
    pub fn finish(&mut self) -> String {
        let mut out = String::new();
        if !self.started {
            return out;
        }
        self.close(&mut out);
        let stop = if self.stop.is_null() {
            json!("end_turn")
        } else {
            self.stop.clone()
        };
        out.push_str(&event(
            "message_delta",
            json!({"type": "message_delta",
            "delta": {"stop_reason": stop, "stop_sequence": null},
            "usage": anthropic_usage(self.usage)}),
        ));
        out.push_str(&event("message_stop", json!({"type": "message_stop"})));
        self.started = false;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_messages_request_becomes_a_chat_completion_request() {
        let req = json!({
            "model": "claude-ish",
            "system": [{"type": "text", "text": "Be terse."}],
            "max_tokens": 100,
            "stop_sequences": ["END"],
            "thinking": {"type": "enabled", "budget_tokens": 4000},
            "tools": [{"name": "get_weather", "description": "w", "input_schema": {"type": "object"}}],
            "tool_choice": {"type": "any", "disable_parallel_tool_use": true},
            "messages": [
                {"role": "user", "content": "weather?"},
                {"role": "assistant", "content": [
                    {"type": "text", "text": "Checking."},
                    {"type": "tool_use", "id": "t1", "name": "get_weather", "input": {"city": "Paris"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "t1", "content": "sunny"},
                    {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AAAA"}}
                ]}
            ]
        });
        let out = to_openai_request(req.as_object().unwrap(), "gpt-4o");
        let messages = out["messages"].as_array().unwrap();
        assert_eq!(
            messages[0],
            json!({"role": "system", "content": "Be terse."})
        );
        assert_eq!(messages[1]["content"][0]["text"], "weather?");
        assert_eq!(
            messages[2]["tool_calls"][0]["function"]["arguments"],
            "{\"city\":\"Paris\"}"
        );
        assert_eq!(
            messages[3],
            json!({"role": "tool", "tool_call_id": "t1", "content": "sunny"})
        );
        assert_eq!(
            messages[4]["content"][0]["image_url"]["url"],
            "data:image/png;base64,AAAA"
        );
        assert_eq!(out["model"], "gpt-4o");
        assert_eq!(out["stop"], json!(["END"]));
        assert_eq!(out["reasoning_effort"], "high");
        assert_eq!(
            out["tools"][0]["function"]["parameters"],
            json!({"type": "object"})
        );
        assert_eq!(out["tool_choice"], "required");
        assert_eq!(out["parallel_tool_calls"], false);
    }

    #[test]
    fn a_chat_completion_becomes_a_message() {
        let (message, usage) = to_anthropic_response(
            &json!({"id": "c1", "choices": [{"finish_reason": "tool_calls", "message": {
                "content": "Calling.", "tool_calls": [{"id": "t", "function": {"name": "f", "arguments": "{\"a\":1}"}}]}}],
                "usage": {"prompt_tokens": 50, "completion_tokens": 5, "prompt_tokens_details": {"cached_tokens": 40}}}),
            "m",
        );
        assert_eq!(message["type"], "message");
        assert_eq!(message["content"][0]["text"], "Calling.");
        assert_eq!(message["content"][1]["input"], json!({"a": 1}));
        assert_eq!(message["stop_reason"], "tool_use");
        assert_eq!(
            message["usage"],
            json!({"input_tokens": 10, "output_tokens": 5, "cache_read_input_tokens": 40})
        );
        assert_eq!(usage.prompt_tokens, 50);
    }

    #[test]
    fn a_chat_stream_becomes_message_events() {
        let mut t = StreamTranslator::new("m");
        let mut out = String::new();
        for chunk in [
            json!({"id": "c", "choices": [{"delta": {"role": "assistant", "content": "Hi"}}]}),
            json!({"id": "c", "choices": [{"delta": {"content": " there"}}]}),
            json!({"id": "c", "choices": [{"delta": {"tool_calls": [{"index": 0, "id": "t", "function": {"name": "f", "arguments": ""}}]}}]}),
            json!({"id": "c", "choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"arguments": "{}"}}]}, "finish_reason": "tool_calls"}]}),
            json!({"id": "c", "choices": [], "usage": {"prompt_tokens": 7, "completion_tokens": 3}}),
        ] {
            out.push_str(&t.on_chunk(&chunk.to_string()));
        }
        out.push_str(&t.on_chunk("[DONE]"));
        let kinds: Vec<&str> = out
            .lines()
            .filter_map(|l| l.strip_prefix("event: "))
            .collect();
        assert_eq!(
            kinds,
            [
                "message_start",
                "content_block_start",
                "content_block_delta",
                "content_block_delta",
                "content_block_stop",
                "content_block_start",
                "content_block_delta",
                "content_block_stop",
                "message_delta",
                "message_stop",
            ]
        );
        assert!(out.contains("\"stop_reason\":\"tool_use\""));
        assert!(out.contains("\"output_tokens\":3"));
        assert_eq!(t.usage.prompt_tokens, 7);
    }
}
