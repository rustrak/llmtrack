//! OpenAI's Responses API on top of chat completions.
//!
//! OpenAI and Azure serve `/responses` themselves. Every other provider is
//! reached through translation: the request becomes a chat completion,
//! and the answer (or stream) is rebuilt as a response. Stateless only: a
//! `previous_response_id` needs OpenAI's own storage, so it is refused.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

use super::Usage;
use crate::error::{AppError, AppResult};

/// Builds a chat completion body from a Responses API body.
pub fn to_chat_request(req: &Map<String, Value>, model: &str) -> AppResult<Value> {
    if req
        .get("previous_response_id")
        .is_some_and(|v| !v.is_null())
    {
        return Err(AppError::Validation(
            "previous_response_id needs OpenAI's stored responses; send the whole conversation as input instead".into(),
        ));
    }
    let mut messages = Vec::new();
    if let Some(instructions) = req.get("instructions").and_then(Value::as_str) {
        messages.push(json!({"role": "system", "content": instructions}));
    }
    match req.get("input") {
        Some(Value::String(text)) => messages.push(json!({"role": "user", "content": text})),
        Some(Value::Array(items)) => {
            for item in items {
                messages.extend(input_item(item));
            }
        }
        _ => return Err(AppError::Validation("input is required".into())),
    }

    let mut body = json!({"model": model, "messages": messages});
    let out = body.as_object_mut().expect("built as an object");
    for (from, to) in [
        ("max_output_tokens", "max_tokens"),
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("stream", "stream"),
        ("user", "user"),
        ("parallel_tool_calls", "parallel_tool_calls"),
    ] {
        if let Some(value) = req.get(from) {
            out.insert(to.into(), value.clone());
        }
    }
    if let Some(effort) = req.get("reasoning").and_then(|r| r.get("effort")) {
        out.insert("reasoning_effort".into(), effort.clone());
    }
    if let Some(format) = req.get("text").and_then(|t| t.get("format")) {
        let response_format = match format["type"].as_str() {
            Some("json_schema") => json!({"type": "json_schema", "json_schema": {
                "name": format["name"], "schema": format["schema"], "strict": format["strict"],
            }}),
            Some("json_object") => json!({"type": "json_object"}),
            _ => Value::Null,
        };
        if !response_format.is_null() {
            out.insert("response_format".into(), response_format);
        }
    }
    if let Some(tools) = req.get("tools").and_then(Value::as_array) {
        let tools: Vec<Value> = tools
            .iter()
            .filter(|t| t["type"] == "function")
            .map(|t| {
                json!({"type": "function", "function": {
                    "name": t["name"], "description": t["description"], "parameters": t["parameters"],
                }})
            })
            .collect();
        if !tools.is_empty() {
            out.insert("tools".into(), Value::Array(tools));
        }
    }
    if let Some(choice) = req.get("tool_choice") {
        let mapped = match choice {
            Value::Object(o) if o.get("type") == Some(&json!("function")) => {
                json!({"type": "function", "function": {"name": o["name"]}})
            }
            other => other.clone(),
        };
        out.insert("tool_choice".into(), mapped);
    }
    Ok(body)
}

/// One `input` item as chat messages.
fn input_item(item: &Value) -> Vec<Value> {
    match item["type"].as_str() {
        Some("function_call") => vec![json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [{"id": item["call_id"], "type": "function",
                            "function": {"name": item["name"], "arguments": item["arguments"]}}],
        })],
        Some("function_call_output") => vec![json!({
            "role": "tool",
            "tool_call_id": item["call_id"],
            "content": match &item["output"] { Value::String(s) => Value::String(s.clone()), other => Value::String(other.to_string()) },
        })],
        _ => {
            let role = match item["role"].as_str() {
                Some("developer") | Some("system") => "system",
                Some("assistant") => "assistant",
                _ => "user",
            };
            let content = match &item["content"] {
                Value::String(text) => Value::String(text.clone()),
                Value::Array(parts) => {
                    Value::Array(parts.iter().filter_map(content_part).collect())
                }
                _ => Value::String(String::new()),
            };
            vec![json!({"role": role, "content": content})]
        }
    }
}

fn content_part(part: &Value) -> Option<Value> {
    match part["type"].as_str()? {
        "input_text" | "output_text" | "text" => {
            Some(json!({"type": "text", "text": part["text"]}))
        }
        "input_image" => {
            let url = part["image_url"].as_str()?;
            Some(json!({"type": "image_url", "image_url": {"url": url}}))
        }
        "input_file" => {
            let data = part["file_data"].as_str()?;
            Some(json!({"type": "file", "file": {"file_data": data}}))
        }
        _ => None,
    }
}

fn usage_json(usage: Usage) -> Value {
    json!({
        "input_tokens": usage.prompt_tokens,
        "output_tokens": usage.completion_tokens,
        "total_tokens": usage.prompt_tokens + usage.completion_tokens,
        "input_tokens_details": {"cached_tokens": usage.cached_tokens},
        "output_tokens_details": {"reasoning_tokens": usage.reasoning_tokens},
    })
}

fn response_object(
    id: &str,
    model: &str,
    status: &str,
    output: Vec<Value>,
    usage: Option<Usage>,
) -> Value {
    json!({
        "id": id,
        "object": "response",
        "created_at": chrono::Utc::now().timestamp(),
        "status": status,
        "model": model,
        "output": output,
        "usage": usage.map(usage_json),
    })
}

fn message_item(id: &str, text: &str) -> Value {
    json!({"type": "message", "id": id, "role": "assistant", "status": "completed",
           "content": [{"type": "output_text", "text": text, "annotations": []}]})
}

fn function_item(call_id: &Value, name: &Value, arguments: &str) -> Value {
    json!({"type": "function_call", "id": format!("fc_{}", call_id.as_str().unwrap_or("")),
           "call_id": call_id, "name": name, "arguments": arguments, "status": "completed"})
}

/// Turns a chat completion into a response.
pub fn to_response(completion: &Value, model: &str) -> (Value, Usage) {
    let usage = Usage::from_openai(&completion["usage"]).unwrap_or_default();
    let message = &completion["choices"][0]["message"];
    let id = format!("resp_{}", completion["id"].as_str().unwrap_or("gateway"));
    let mut output = Vec::new();
    if let Some(text) = message["content"].as_str().filter(|t| !t.is_empty()) {
        output.push(message_item(&format!("msg_{id}"), text));
    }
    for call in message["tool_calls"].as_array().into_iter().flatten() {
        let arguments = call["function"]["arguments"].as_str().unwrap_or("{}");
        output.push(function_item(
            &call["id"],
            &call["function"]["name"],
            arguments,
        ));
    }
    (
        response_object(&id, model, "completed", output, Some(usage)),
        usage,
    )
}

/// Rewrites a chat completion stream as Responses API events.
pub struct StreamTranslator {
    id: String,
    model: String,
    started: bool,
    finished: bool,
    sequence: u64,
    text: String,
    /// Chat tool call index → (call id, name, arguments so far).
    calls: BTreeMap<u64, (Value, Value, String)>,
    pub usage: Usage,
}

impl StreamTranslator {
    pub fn new(model: &str) -> Self {
        Self {
            id: format!("resp_{}", uuid::Uuid::new_v4().simple()),
            model: model.to_string(),
            started: false,
            finished: false,
            sequence: 0,
            text: String::new(),
            calls: BTreeMap::new(),
            usage: Usage::default(),
        }
    }

    fn event(&mut self, mut data: Value) -> String {
        let kind = data["type"].as_str().unwrap_or_default().to_string();
        data["sequence_number"] = json!(self.sequence);
        self.sequence += 1;
        format!("event: {kind}\ndata: {data}\n\n")
    }

    fn message_id(&self) -> String {
        format!("msg_{}", self.id)
    }

    /// The events for one chat chunk's `data:` payload.
    pub fn on_chunk(&mut self, data: &str) -> String {
        if data == "[DONE]" {
            return self.finish();
        }
        let Ok(chunk) = serde_json::from_str::<Value>(data) else {
            return String::new();
        };
        let mut out = String::new();
        if !self.started {
            self.started = true;
            let created = response_object(&self.id, &self.model, "in_progress", vec![], None);
            out.push_str(&self.event(json!({"type": "response.created", "response": created})));
        }
        if let Some(usage) = Usage::from_openai(&chunk["usage"]) {
            self.usage = usage;
        }
        let delta = &chunk["choices"][0]["delta"];
        if let Some(text) = delta["content"].as_str().filter(|t| !t.is_empty()) {
            if self.text.is_empty() {
                let item = json!({"type": "message", "id": self.message_id(), "role": "assistant",
                                  "status": "in_progress", "content": []});
                out.push_str(&self.event(
                    json!({"type": "response.output_item.added", "output_index": 0, "item": item}),
                ));
            }
            self.text.push_str(text);
            let item_id = self.message_id();
            out.push_str(&self.event(
                json!({"type": "response.output_text.delta", "item_id": item_id,
                "output_index": 0, "content_index": 0, "delta": text}),
            ));
        }
        for call in delta["tool_calls"].as_array().into_iter().flatten() {
            let index = call["index"].as_u64().unwrap_or(0);
            let entry = self.calls.entry(index).or_insert_with(|| {
                (
                    call["id"].clone(),
                    call["function"]["name"].clone(),
                    String::new(),
                )
            });
            if let Some(arguments) = call["function"]["arguments"].as_str() {
                entry.2.push_str(arguments);
            }
        }
        out
    }

    /// Closes the response; also called when the stream ends without `[DONE]`.
    pub fn finish(&mut self) -> String {
        if !self.started || self.finished {
            return String::new();
        }
        self.finished = true;
        let mut output = Vec::new();
        let mut out = String::new();
        if !self.text.is_empty() {
            let item = message_item(&self.message_id(), &self.text);
            let item_id = self.message_id();
            let text = self.text.clone();
            out.push_str(&self.event(
                json!({"type": "response.output_text.done", "item_id": item_id,
                "output_index": 0, "content_index": 0, "text": text}),
            ));
            out.push_str(&self.event(
                json!({"type": "response.output_item.done", "output_index": 0, "item": item}),
            ));
            output.push(item);
        }
        let calls: Vec<Value> = self
            .calls
            .values()
            .map(|(call_id, name, arguments)| function_item(call_id, name, arguments))
            .collect();
        for item in calls {
            let index = output.len();
            out.push_str(&self.event(
                json!({"type": "response.output_item.done", "output_index": index, "item": item}),
            ));
            output.push(item);
        }
        let done = response_object(&self.id, &self.model, "completed", output, Some(self.usage));
        out.push_str(&self.event(json!({"type": "response.completed", "response": done})));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_responses_request_becomes_a_chat_request() {
        let req = json!({
            "model": "claude",
            "instructions": "Be terse.",
            "input": [
                {"role": "user", "content": [{"type": "input_text", "text": "weather?"},
                                              {"type": "input_image", "image_url": "data:image/png;base64,AA"}]},
                {"type": "function_call", "call_id": "c1", "name": "get_weather", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "c1", "output": "sunny"}
            ],
            "max_output_tokens": 100,
            "reasoning": {"effort": "high"},
            "text": {"format": {"type": "json_schema", "name": "w", "schema": {"type": "object"}}},
            "tools": [{"type": "function", "name": "get_weather", "parameters": {"type": "object"}},
                      {"type": "web_search"}],
            "stream": true
        });
        let chat = to_chat_request(req.as_object().unwrap(), "upstream").unwrap();
        let messages = chat["messages"].as_array().unwrap();
        assert_eq!(
            messages[0],
            json!({"role": "system", "content": "Be terse."})
        );
        assert_eq!(
            messages[1]["content"][1]["image_url"]["url"],
            "data:image/png;base64,AA"
        );
        assert_eq!(messages[2]["tool_calls"][0]["id"], "c1");
        assert_eq!(
            messages[3],
            json!({"role": "tool", "tool_call_id": "c1", "content": "sunny"})
        );
        assert_eq!(chat["model"], "upstream");
        assert_eq!(chat["max_tokens"], 100);
        assert_eq!(chat["reasoning_effort"], "high");
        assert_eq!(chat["response_format"]["json_schema"]["name"], "w");
        assert_eq!(
            chat["tools"].as_array().unwrap().len(),
            1,
            "hosted tools are left out"
        );
        assert_eq!(chat["stream"], true);
    }

    #[test]
    fn a_plain_string_input_and_stored_state() {
        let chat = to_chat_request(json!({"input": "hi"}).as_object().unwrap(), "m").unwrap();
        assert_eq!(
            chat["messages"][0],
            json!({"role": "user", "content": "hi"})
        );
        let stateful = json!({"input": "hi", "previous_response_id": "resp_1"});
        assert!(to_chat_request(stateful.as_object().unwrap(), "m").is_err());
    }

    #[test]
    fn a_chat_completion_becomes_a_response() {
        let (response, usage) = to_response(
            &json!({"id": "c1", "choices": [{"message": {"content": "Hi", "tool_calls": [
                {"id": "t1", "function": {"name": "f", "arguments": "{\"a\":1}"}}]}}],
                "usage": {"prompt_tokens": 9, "completion_tokens": 3, "prompt_tokens_details": {"cached_tokens": 4}}}),
            "m",
        );
        assert_eq!(response["object"], "response");
        assert_eq!(response["status"], "completed");
        assert_eq!(response["output"][0]["content"][0]["text"], "Hi");
        assert_eq!(response["output"][1]["type"], "function_call");
        assert_eq!(response["output"][1]["arguments"], "{\"a\":1}");
        assert_eq!(
            response["usage"]["input_tokens_details"]["cached_tokens"],
            4
        );
        assert_eq!(usage.prompt_tokens, 9);
    }

    #[test]
    fn a_chat_stream_becomes_response_events() {
        let mut t = StreamTranslator::new("m");
        let mut out = String::new();
        for chunk in [
            json!({"choices": [{"delta": {"content": "Hel"}}]}),
            json!({"choices": [{"delta": {"content": "lo"}}]}),
            json!({"choices": [], "usage": {"prompt_tokens": 5, "completion_tokens": 2}}),
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
                "response.created",
                "response.output_item.added",
                "response.output_text.delta",
                "response.output_text.delta",
                "response.output_text.done",
                "response.output_item.done",
                "response.completed",
            ]
        );
        assert!(out.contains("\"text\":\"Hello\""));
        assert!(out.contains("\"input_tokens\":5"));
        assert_eq!(t.finish(), "", "finishing twice sends nothing");
    }
}
