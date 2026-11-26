//! OpenAI-compatible streaming — covers OpenAI and OpenRouter
//! (same chat.completions schema; different base_url and auth header).

use crate::sse::SseEvent;
use crate::{ProviderError, StreamEvent, ToolCall};
use serde_json::Value;
use std::collections::BTreeMap;

/// Stateful mapper for OpenAI chat.completion.chunk streams.
/// Tool calls stream as fragments indexed by (choice_index implicit 0, tool index).
#[derive(Default)]
pub struct OpenAiMapper {
    tool_calls: BTreeMap<u64, ToolAcc>,
    usage_emitted: bool,
}

struct ToolAcc {
    id: String,
    name: String,
    args: String,
}

impl OpenAiMapper {
    pub fn feed(&mut self, ev: &SseEvent) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        if ev.data.trim() == "[DONE]" {
            out.push(StreamEvent::Done);
            return out;
        }
        let Ok(v) = serde_json::from_str::<Value>(&ev.data) else {
            return out;
        };
        if let Some(err) = v.get("error") {
            let msg = err.get("message").and_then(Value::as_str).unwrap_or("openai error");
            out.push(StreamEvent::Error { message: msg.into() });
            return out;
        }
        if let Some(choice) = v.pointer("/choices/0") {
            if let Some(delta) = choice.get("delta") {
                if let Some(text) = delta.get("content").and_then(Value::as_str) {
                    if !text.is_empty() {
                        out.push(StreamEvent::TextDelta { text: text.into() });
                    }
                }
                if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
                    for call in calls {
                        let idx = call.get("index").and_then(Value::as_u64).unwrap_or(0);
                        let acc = self.tool_calls.entry(idx).or_insert_with(|| ToolAcc {
                            id: String::new(),
                            name: String::new(),
                            args: String::new(),
                        });
                        if let Some(id) = call.get("id").and_then(Value::as_str) {
                            acc.id = id.into();
                        }
                        if let Some(f) = call.get("function") {
                            if let Some(name) = f.get("name").and_then(Value::as_str) {
                                acc.name = name.into();
                            }
                            if let Some(args) = f.get("arguments").and_then(Value::as_str) {
                                acc.args.push_str(args);
                            }
                        }
                    }
                }
            }
            if choice.get("finish_reason").and_then(Value::as_str).is_some() {
                // flush tool calls at finish
                for (_, acc) in std::mem::take(&mut self.tool_calls) {
                    let input = serde_json::from_str(&acc.args)
                        .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
                    out.push(StreamEvent::ToolUse {
                        call: ToolCall {
                            id: acc.id,
                            name: acc.name,
                            input,
                        },
                    });
                }
            }
        }
        if let Some(usage) = v.get("usage") {
            if !self.usage_emitted {
                self.usage_emitted = true;
                out.push(StreamEvent::Usage {
                    input_tokens: usage.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
                    output_tokens: usage.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0),
                });
            }
        }
        out
    }
}

pub fn build_request_body(req: &crate::CompletionRequest) -> Value {
    let mut messages = Vec::new();
    if let Some(sys) = &req.system {
        messages.push(serde_json::json!({"role": "system", "content": sys}));
    }
    for m in &req.messages {
        if m.role == crate::Role::System {
            messages.push(serde_json::json!({"role": "system", "content": m.content}));
        } else {
            messages.push(serde_json::json!({
                "role": match m.role {
                    crate::Role::User => "user",
                    crate::Role::Assistant => "assistant",
                    crate::Role::System => unreachable!(),
                },
                "content": m.content,
            }));
        }
    }
    let mut body = serde_json::json!({
        "model": req.model,
        "stream": true,
        "stream_options": {"include_usage": true},
        "messages": messages,
    });
    if !req.tools.is_empty() {
        body["tools"] = serde_json::json!(req.tools.iter().map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                }
            })
        }).collect::<Vec<_>>());
    }
    body
}

pub fn map_http_error(status: u16, body: &str) -> ProviderError {
    match status {
        401 | 403 => ProviderError::Auth(body.to_string()),
        429 => ProviderError::RateLimited { retry_after_ms: None },
        _ => ProviderError::Api {
            status,
            body: body.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(data: &str) -> SseEvent {
        SseEvent {
            event: None,
            data: data.into(),
        }
    }

    #[test]
    fn text_deltas_mapped() {
        let mut m = OpenAiMapper::default();
        let out = m.feed(&chunk(r#"{"choices":[{"delta":{"content":"Hello"},"finish_reason":null}]}"#));
        assert!(matches!(&out[0], StreamEvent::TextDelta { text } if text == "Hello"));
    }

    #[test]
    fn done_sentinel_maps_to_done() {
        let mut m = OpenAiMapper::default();
        let out = m.feed(&chunk("[DONE]"));
        assert!(matches!(out[0], StreamEvent::Done));
    }

    #[test]
    fn tool_call_fragments_assemble_at_finish() {
        let mut m = OpenAiMapper::default();
        m.feed(&chunk(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"write_file","arguments":""}}]},"finish_reason":null}]}"#));
        m.feed(&chunk(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":"}}]},"finish_reason":null}]}"#));
        m.feed(&chunk(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"a.md\"}"}}]},"finish_reason":null}]}"#));
        let out = m.feed(&chunk(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#));
        assert_eq!(out.len(), 1);
        match &out[0] {
            StreamEvent::ToolUse { call } => {
                assert_eq!(call.id, "call_1");
                assert_eq!(call.name, "write_file");
                assert_eq!(call.input["path"], "a.md");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn usage_mapped_once() {
        let mut m = OpenAiMapper::default();
        let out = m.feed(&chunk(r#"{"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":4}}"#));
        assert!(out.iter().any(|e| matches!(e, StreamEvent::Usage { input_tokens: 10, output_tokens: 4 })));
        let out2 = m.feed(&chunk(r#"{"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":4}}"#));
        assert!(out2.is_empty(), "usage must only emit once");
    }

    #[test]
    fn system_prepended_in_body() {
        let req = crate::CompletionRequest {
            model: "gpt-5".into(),
            system: Some("sys".into()),
            messages: vec![crate::Message { role: crate::Role::User, content: "hi".into() }],
            tools: vec![],
            max_tokens: 100,
        };
        let body = build_request_body(&req);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[1]["role"], "user");
    }
}
