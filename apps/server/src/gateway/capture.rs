//! What a key that logs bodies keeps: the body the client sent and the reply
//! it got. A stream is folded into the one reply it adds up to, in the
//! dialect the client speaks, so a log reads the same streamed or not.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

use super::forward::Endpoint;
use super::sse::SseParser;

/// Strings at least this long that hold a file (an image, audio, a PDF) are
/// replaced by their size: they would dwarf the text around them.
const INLINE_FILE: usize = 1024;

/// Replaces inline files (base64 or `data:` URLs) with a note of their size.
pub fn trim(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                match item {
                    Value::String(s)
                        if s.len() >= INLINE_FILE
                            && (matches!(key.as_str(), "data" | "b64_json" | "file_data")
                                || s.starts_with("data:")) =>
                    {
                        *item = Value::String(format!("[{} bytes omitted]", s.len()));
                    }
                    _ => trim(item),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(trim),
        Value::String(s) if s.len() >= INLINE_FILE && s.starts_with("data:") => {
            *value = Value::String(format!("[{} bytes omitted]", s.len()));
        }
        _ => {}
    }
}

/// Fields a delta replaces rather than appends to.
const REPLACED: [&str; 9] = [
    "id",
    "type",
    "role",
    "name",
    "model",
    "object",
    "index",
    "finish_reason",
    "stop_reason",
];

/// Folds `delta` into `into`: text appends, lists of indexed items (tool
/// calls) merge by `index`, anything else replaces.
fn merge(into: &mut Value, delta: &Value) {
    let (Value::Object(into), Value::Object(delta)) = (into, delta) else {
        return;
    };
    for (key, value) in delta {
        if value.is_null() {
            continue;
        }
        let Some(slot) = into.get_mut(key).filter(|slot| !slot.is_null()) else {
            into.insert(key.clone(), value.clone());
            continue;
        };
        match (slot, value) {
            (Value::String(s), Value::String(more)) if !REPLACED.contains(&key.as_str()) => {
                s.push_str(more);
            }
            (slot @ Value::Object(_), Value::Object(_)) => merge(slot, value),
            (Value::Array(items), Value::Array(more)) => {
                for item in more {
                    match items.iter_mut().find(|i| {
                        item.get("index").is_some() && i.get("index") == item.get("index")
                    }) {
                        Some(existing) => merge(existing, item),
                        None => items.push(item.clone()),
                    }
                }
            }
            (slot, value) => *slot = value.clone(),
        }
    }
}

/// A stream's events, folded as they go by.
pub struct Transcript {
    parser: SseParser,
    fold: Fold,
}

enum Fold {
    /// Chat or text completion chunks.
    Chat {
        head: Option<Value>,
        choices: BTreeMap<u64, Value>,
        usage: Value,
    },
    /// Anthropic's Messages events.
    Messages(Option<Value>),
    /// The Responses API's last event carries the whole response.
    Responses(Option<Value>),
}

impl Transcript {
    pub fn new(endpoint: Endpoint) -> Self {
        let fold = match endpoint {
            Endpoint::Messages => Fold::Messages(None),
            Endpoint::Responses => Fold::Responses(None),
            _ => Fold::Chat {
                head: None,
                choices: BTreeMap::new(),
                usage: Value::Null,
            },
        };
        Self {
            parser: SseParser::default(),
            fold,
        }
    }

    /// Feeds the bytes the client was sent.
    pub fn push(&mut self, bytes: &[u8]) {
        for event in self.parser.push(bytes) {
            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                self.on_event(&data);
            }
        }
    }

    fn on_event(&mut self, event: &Value) {
        match &mut self.fold {
            Fold::Chat {
                head,
                choices,
                usage,
            } => {
                if head.is_none() {
                    *head = Some(event.clone());
                }
                if event["usage"].is_object() {
                    *usage = event["usage"].clone();
                }
                for choice in event["choices"].as_array().into_iter().flatten() {
                    let index = choice["index"].as_u64().unwrap_or(0);
                    let folded = choices.entry(index).or_insert_with(|| json!({}));
                    match choice.get("delta") {
                        // Chat: the delta builds the message.
                        Some(delta) => {
                            if folded.get("message").is_none() {
                                folded["message"] = json!({});
                            }
                            merge(&mut folded["message"], delta);
                            let mut rest = choice.clone();
                            rest.as_object_mut().map(|c| c.remove("delta"));
                            merge(folded, &rest);
                        }
                        // Text completion: the choice itself is the delta.
                        None => merge(folded, choice),
                    }
                }
            }
            Fold::Messages(message) => match event["type"].as_str() {
                Some("message_start") => *message = Some(event["message"].clone()),
                Some(kind) => {
                    let Some(message) = message else { return };
                    on_message_event(message, kind, event);
                }
                None => {}
            },
            Fold::Responses(response) => {
                if matches!(
                    event["type"].as_str(),
                    Some("response.completed" | "response.incomplete" | "response.failed")
                ) {
                    *response = Some(event["response"].clone());
                }
            }
        }
    }

    /// The reply the stream added up to, `None` if nothing came.
    pub fn finish(mut self) -> Option<Value> {
        if let Some(event) = self.parser.finish() {
            if let Ok(data) = serde_json::from_str::<Value>(&event.data) {
                self.on_event(&data);
            }
        }
        match self.fold {
            Fold::Chat {
                head,
                choices,
                usage,
            } => {
                let head = head?;
                let text = head["object"] == "text_completion";
                let choices: Vec<Value> = choices
                    .into_iter()
                    .map(|(index, mut choice)| {
                        choice["index"] = json!(index);
                        if let Some(calls) = choice
                            .get_mut("message")
                            .and_then(|m| m.get_mut("tool_calls"))
                            .and_then(Value::as_array_mut)
                        {
                            for call in calls {
                                call.as_object_mut().map(|c| c.remove("index"));
                            }
                        }
                        if choice.get("finish_reason").is_none() {
                            choice["finish_reason"] = Value::Null;
                        }
                        choice
                    })
                    .collect();
                let mut reply = json!({
                    "id": head["id"],
                    "object": if text { "text_completion" } else { "chat.completion" },
                    "created": head["created"],
                    "model": head["model"],
                    "choices": choices,
                });
                if !usage.is_null() {
                    reply["usage"] = usage;
                }
                Some(reply)
            }
            Fold::Messages(message) => message,
            Fold::Responses(response) => response,
        }
    }
}

fn on_message_event(message: &mut Value, kind: &str, event: &Value) {
    let index = event["index"].as_u64().unwrap_or(0) as usize;
    match kind {
        "content_block_start" => {
            if let Some(content) = message["content"].as_array_mut() {
                content.push(event["content_block"].clone());
            }
        }
        "content_block_delta" => {
            let Some(block) = message["content"].get_mut(index) else {
                return;
            };
            let mut delta = event["delta"].clone();
            let Some(fields) = delta.as_object_mut() else {
                return;
            };
            fields.remove("type");
            if let Some(citation) = fields.remove("citation") {
                match block["citations"].as_array_mut() {
                    Some(citations) => citations.push(citation),
                    None => block["citations"] = json!([citation]),
                }
            }
            merge(block, &delta);
        }
        "content_block_stop" => {
            let Some(block) = message["content"].get_mut(index) else {
                return;
            };
            // A tool's input arrives as JSON text in pieces.
            if let Some(Value::String(partial)) =
                block.as_object_mut().and_then(|b| b.remove("partial_json"))
            {
                if let Ok(input) = serde_json::from_str(&partial) {
                    block["input"] = input;
                }
            }
        }
        "message_delta" => {
            merge(message, &event["delta"]);
            if message.get("usage").is_none() {
                message["usage"] = json!({});
            }
            merge(&mut message["usage"], &event["usage"]);
        }
        _ => {}
    }
}

/// The request of a multipart upload: its fields, files as their size.
pub fn multipart_request(parts: &[super::forward::Part]) -> Value {
    let mut request = Map::new();
    for part in parts {
        let value = match &part.filename {
            Some(name) => json!(format!("[file {name}, {} bytes]", part.data.len())),
            None => json!(String::from_utf8_lossy(&part.data)),
        };
        request.insert(part.name.clone(), value);
    }
    Value::Object(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sse(events: &[Value]) -> Vec<u8> {
        events
            .iter()
            .map(|e| format!("data: {e}\n\n"))
            .collect::<String>()
            .into_bytes()
    }

    #[test]
    fn chat_chunks_fold_into_one_completion_tool_calls_included() {
        let mut t = Transcript::new(Endpoint::ChatCompletions);
        let bytes = sse(&[
            json!({"id": "c1", "object": "chat.completion.chunk", "created": 7, "model": "m",
                   "choices": [{"index": 0, "delta": {"role": "assistant", "content": ""}}]}),
            json!({"choices": [{"index": 0, "delta": {"content": "Let me"}}]}),
            json!({"choices": [{"index": 0, "delta": {"content": " look", "tool_calls": [
                {"index": 0, "id": "t1", "type": "function", "function": {"name": "find", "arguments": ""}}]}}]}),
            json!({"choices": [{"index": 0, "delta": {"tool_calls": [
                {"index": 0, "function": {"arguments": "{\"q\":"}}]}}]}),
            json!({"choices": [{"index": 0, "delta": {"tool_calls": [
                {"index": 0, "function": {"arguments": "1}"}}]}, "finish_reason": "tool_calls"}]}),
            json!({"choices": [], "usage": {"prompt_tokens": 3, "completion_tokens": 4}}),
        ]);
        // Cut anywhere: events span network chunks.
        for piece in bytes.chunks(7) {
            t.push(piece);
        }
        t.push(b"data: [DONE]\n\n");
        assert_eq!(
            t.finish().unwrap(),
            json!({"id": "c1", "object": "chat.completion", "created": 7, "model": "m",
                "choices": [{"index": 0, "finish_reason": "tool_calls", "message": {
                    "role": "assistant", "content": "Let me look",
                    "tool_calls": [{"id": "t1", "type": "function",
                                    "function": {"name": "find", "arguments": "{\"q\":1}"}}]}}],
                "usage": {"prompt_tokens": 3, "completion_tokens": 4}})
        );
    }

    #[test]
    fn text_completion_chunks_fold_too() {
        let mut t = Transcript::new(Endpoint::Completions);
        t.push(&sse(&[
            json!({"id": "x", "object": "text_completion", "choices": [{"index": 0, "text": "Hel"}]}),
            json!({"id": "x", "object": "text_completion", "choices": [{"index": 0, "text": "lo", "finish_reason": "stop"}]}),
        ]));
        let reply = t.finish().unwrap();
        assert_eq!(reply["object"], "text_completion");
        assert_eq!(
            reply["choices"][0],
            json!({"index": 0, "text": "Hello", "finish_reason": "stop"})
        );
    }

    #[test]
    fn anthropic_events_fold_into_one_message() {
        let mut t = Transcript::new(Endpoint::Messages);
        t.push(&sse(&[
            json!({"type": "message_start", "message": {"id": "msg", "type": "message", "role": "assistant",
                   "content": [], "stop_reason": null, "usage": {"input_tokens": 9, "output_tokens": 1}}}),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": ""}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "hm"}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "sig"}}),
            json!({"type": "content_block_stop", "index": 0}),
            json!({"type": "content_block_start", "index": 1, "content_block": {"type": "tool_use", "id": "t", "name": "f", "input": {}}}),
            json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"a\""}}),
            json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": ":2}"}}),
            json!({"type": "content_block_stop", "index": 1}),
            json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 30}}),
            json!({"type": "message_stop"}),
        ]));
        assert_eq!(
            t.finish().unwrap(),
            json!({"id": "msg", "type": "message", "role": "assistant", "stop_reason": "tool_use",
                "content": [{"type": "thinking", "thinking": "hm", "signature": "sig"},
                            {"type": "tool_use", "id": "t", "name": "f", "input": {"a": 2}}],
                "usage": {"input_tokens": 9, "output_tokens": 30}})
        );
    }

    #[test]
    fn a_stream_that_never_started_is_nothing() {
        assert_eq!(Transcript::new(Endpoint::ChatCompletions).finish(), None);
        assert_eq!(Transcript::new(Endpoint::Messages).finish(), None);
    }

    #[test]
    fn inline_files_are_left_out_text_is_not() {
        let image = format!("data:image/png;base64,{}", "A".repeat(2000));
        let mut body = json!({"messages": [{"role": "user", "content": [
            {"type": "text", "text": "x".repeat(5000)},
            {"type": "image_url", "image_url": {"url": image}},
            {"type": "image", "source": {"type": "base64", "data": "B".repeat(3000)}},
        ]}], "data": [{"b64_json": "C".repeat(1500)}]});
        trim(&mut body);
        let content = &body["messages"][0]["content"];
        assert_eq!(content[0]["text"].as_str().unwrap().len(), 5000);
        assert_eq!(content[1]["image_url"]["url"], "[2022 bytes omitted]");
        assert_eq!(content[2]["source"]["data"], "[3000 bytes omitted]");
        assert_eq!(body["data"][0]["b64_json"], "[1500 bytes omitted]");
    }
}
