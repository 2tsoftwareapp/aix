//! Anthropic Messages API — streaming implementation.
//! HTTP layer is thin; all event mapping is pure and fixture-tested.

use crate::sse::SseEvent;
use crate::{ProviderError, StreamEvent, ToolCall};
use serde_json::Value;
use std::collections::BTreeMap;

/// Stateful mapper: Anthropic SSE events → provider StreamEvents.
/// Tool-use input arrives as partial_json fragments accumulated per block index.
#[derive(Default)]
pub struct AnthropicMapper {
    tool_blocks: BTreeMap<u64, ToolBlockAcc>,
    input_tokens: u64,
    output_tokens: u64,
    usage_emitted: bool,
}

struct ToolBlockAcc {
    id: String,
    name: String,
    json: String,
}

impl AnthropicMapper {
    pub fn feed(&mut self, ev: &SseEvent) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        let kind = ev.event.as_deref().unwrap_or("");
        let Ok(v) = serde_json::from_str::<Value>(&ev.data) else {
            return out;
        };
        match kind {
            "message_start" => {
                if let Some(n) = v.pointer("/message/usage/input_tokens").and_then(Value::as_u64) {
                    self.input_tokens = n;
                }
            }
            "content_block_start" => {
                let index = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                if let Some(block) = v.get("content_block") {
                    if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                        self.tool_blocks.insert(
                            index,
                            ToolBlockAcc {
                                id: block.get("id").and_then(Value::as_str).unwrap_or("").into(),
                                name: block.get("name").and_then(Value::as_str).unwrap_or("").into(),
                                json: String::new(),
                            },
                        );
                    }
                }
            }
            "content_block_delta" => {
                let index = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                if let Some(delta) = v.get("delta") {
                    match delta.get("type").and_then(Value::as_str) {
                        Some("text_delta") => {
                            if let Some(text) = delta.get("text").and_then(Value::as_str) {
                                out.push(StreamEvent::TextDelta { text: text.into() });
                            }
                        }
                        Some("input_json_delta") => {
                            if let Some(frag) = delta.get("partial_json").and_then(Value::as_str) {
                                if let Some(acc) = self.tool_blocks.get_mut(&index) {
                                    acc.json.push_str(frag);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "content_block_stop" => {
                let index = v.get("index").and_then(Value::as_u64).unwrap_or(0);
                if let Some(acc) = self.tool_blocks.remove(&index) {
                    let input = serde_json::from_str(&acc.json)
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
            "message_delta" => {
                if let Some(n) = v.pointer("/usage/output_tokens").and_then(Value::as_u64) {
                    self.output_tokens = n;
                }
            }
            "message_stop" => {
                if !self.usage_emitted {
                    self.usage_emitted = true;
                    out.push(StreamEvent::Usage {
                        input_tokens: self.input_tokens,
                        output_tokens: self.output_tokens,
                    });
                }
                out.push(StreamEvent::Done);
            }
            "error" => {
                let msg = v
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown anthropic error");
                out.push(StreamEvent::Error { message: msg.into() });
            }
            _ => {}
        }
        out
    }
}

/// Build the Anthropic request body from a CompletionRequest.
pub fn build_request_body(req: &crate::CompletionRequest) -> Value {
    let mut body = serde_json::json!({
        "model": req.model,
        "max_tokens": req.max_tokens,
        "stream": true,
        "messages": req.messages.iter().filter(|m| m.role != crate::Role::System).map(|m| {
            serde_json::json!({
                "role": match m.role {
                    crate::Role::User => "user",
                    crate::Role::Assistant => "assistant",
                    crate::Role::System => unreachable!(),
                },
                "content": m.content,
            })
        }).collect::<Vec<_>>(),
    });
    if let Some(sys) = &req.system {
        body["system"] = Value::String(sys.clone());
    }
    if !req.tools.is_empty() {
        body["tools"] = serde_json::json!(req.tools.iter().map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
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

    fn ev(event: &str, data: &str) -> SseEvent {
        SseEvent {
            event: Some(event.into()),
            data: data.into(),
        }
    }

    #[test]
    fn text_stream_maps_to_deltas() {
        let mut m = AnthropicMapper::default();
        assert!(m.feed(&ev("message_start", r#"{"message":{"usage":{"input_tokens":25}}}"#)).is_empty());
        let out = m.feed(&ev("content_block_delta", r#"{"index":0,"delta":{"type":"text_delta","text":"Hello"}}"#));
        assert!(matches!(&out[0], StreamEvent::TextDelta { text } if text == "Hello"));
        let out = m.feed(&ev("message_delta", r#"{"usage":{"output_tokens":7}}"#));
        assert!(out.is_empty());
        let out = m.feed(&ev("message_stop", "{}"));
        assert!(out.iter().any(|e| matches!(e, StreamEvent::Usage { input_tokens: 25, output_tokens: 7 })));
        assert!(out.iter().any(|e| matches!(e, StreamEvent::Done)));
    }

    #[test]
    fn tool_use_accumulates_partial_json() {
        let mut m = AnthropicMapper::default();
        m.feed(&ev("content_block_start", r#"{"index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"write_file"}}"#));
        m.feed(&ev("content_block_delta", r#"{"index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":"}}"#));
        m.feed(&ev("content_block_delta", r#"{"index":1,"delta":{"type":"input_json_delta","partial_json":"\"spec.md\"}"}}"#));
        let out = m.feed(&ev("content_block_stop", r#"{"index":1}"#));
        assert_eq!(out.len(), 1);
        match &out[0] {
            StreamEvent::ToolUse { call } => {
                assert_eq!(call.id, "toolu_1");
                assert_eq!(call.name, "write_file");
                assert_eq!(call.input["path"], "spec.md");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn system_messages_extracted_not_inlined() {
        let req = crate::CompletionRequest {
            model: "claude-sonnet-4-5".into(),
            system: Some("be helpful".into()),
            messages: vec![
                crate::Message { role: crate::Role::System, content: "ignored-here".into() },
                crate::Message { role: crate::Role::User, content: "hi".into() },
            ],
            tools: vec![],
            max_tokens: 1024,
        };
        let body = build_request_body(&req);
        assert_eq!(body["system"], "be helpful");
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1, "system role messages must not be inlined");
        assert_eq!(msgs[0]["role"], "user");
    }

    #[test]
    fn error_event_mapped() {
        let mut m = AnthropicMapper::default();
        let out = m.feed(&ev("error", r#"{"error":{"type":"overloaded_error","message":"Overloaded"}}"#));
        assert!(matches!(&out[0], StreamEvent::Error { message } if message == "Overloaded"));
    }
}
