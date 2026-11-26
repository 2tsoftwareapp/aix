//! providers: multi-provider LLM abstraction.
//! v1: Anthropic, OpenAI, OpenRouter — API key auth; OAuth subscription modules later.

pub mod anthropic;
pub mod credentials;
pub mod http;
pub mod live;
pub mod models;
pub mod openai_compat;
pub mod sse;

use async_trait::async_trait;
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("rate limited; retry after {retry_after_ms:?}ms")]
    RateLimited { retry_after_ms: Option<u64> },
    #[error("network error: {0}")]
    Network(String),
    #[error("provider error {status}: {body}")]
    Api { status: u16, body: String },
    #[error("stream interrupted: {0}")]
    StreamInterrupted(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// Tool calls the assistant made in this turn (assistant messages only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// Tool results being returned to the model (user/tool messages only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_results: Vec<ToolResult>,
}

/// Result of executing one tool call, fed back to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Tool definition handed to the model (file/shell executor tools).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// One tool invocation requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    TextDelta { text: String },
    ToolUse { call: ToolCall },
    Usage { input_tokens: u64, output_tokens: u64 },
    Done,
    Error { message: String },
}

pub type EventStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub max_tokens: u32,
}

/// Auth material for a provider. OAuth variants arrive with the subscription milestone.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Credential {
    ApiKey { key_ref: String },
    OAuthToken { token_ref: String },
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    async fn complete(&self, req: CompletionRequest) -> Result<EventStream, ProviderError>;
    async fn list_models(&self) -> Result<Vec<String>, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider;

    #[async_trait]
    impl Provider for MockProvider {
        fn id(&self) -> &'static str {
            "mock"
        }
        fn display_name(&self) -> &'static str {
            "Mock"
        }
        async fn complete(&self, _req: CompletionRequest) -> Result<EventStream, ProviderError> {
            Ok(Box::pin(futures_util::stream::empty()))
        }
        async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
            Ok(vec!["mock-1".into()])
        }
    }

    #[tokio::test]
    async fn trait_is_object_safe_and_usable() {
        let p: Box<dyn Provider> = Box::new(MockProvider);
        assert_eq!(p.id(), "mock");
        let models = p.list_models().await.unwrap();
        assert_eq!(models.len(), 1);
    }

    #[test]
    fn stream_event_roundtrips_json() {
        let ev = StreamEvent::TextDelta { text: "hi".into() };
        let json = serde_json::to_string(&ev).unwrap();
        let back: StreamEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, StreamEvent::TextDelta { .. }));
    }

    #[test]
    fn credential_serializes_without_key_material_leak_in_kind() {
        let c = Credential::ApiKey { key_ref: "keychain:anthropic".into() };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("api_key"));
        assert!(!json.contains("sk-"));
    }
}
