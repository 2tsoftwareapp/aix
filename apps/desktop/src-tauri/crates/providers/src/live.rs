//! Live provider implementations over the shared HTTP/SSE layer.

use crate::vendor::VendorConfig;
use crate::{anthropic, http, openai_compat, CompletionRequest, EventStream, Provider, ProviderError};
use async_trait::async_trait;

pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    pub base_url: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        let base_url = crate::vendor::vendor("anthropic")
            .map(|v| v.resolved_base_url())
            .unwrap_or_else(|| "https://api.anthropic.com".into());
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url,
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn id(&self) -> &str {
        "anthropic"
    }
    fn display_name(&self) -> &str {
        "Anthropic"
    }

    async fn complete(&self, req: CompletionRequest) -> Result<EventStream, ProviderError> {
        let body = anthropic::build_request_body(&req);
        http::post_sse(
            &self.client,
            &format!("{}/v1/messages", self.base_url),
            vec![
                ("x-api-key".into(), self.api_key.clone()),
                ("anthropic-version".into(), "2023-06-01".into()),
                ("accept".into(), "text/event-stream".into()),
            ],
            body,
            anthropic::AnthropicMapper::default,
            |mapper, sse| mapper.feed(sse),
        )
        .await
    }

    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        Ok(vec![
            "claude-opus-4-5".into(),
            "claude-sonnet-4-5".into(),
            "claude-haiku-4-5".into(),
        ])
    }
}

/// Generic OpenAI-compatible provider driven entirely by vendor config —
/// adding a vendor to vendors.json makes it executable with no code change.
pub struct OpenAiCompatProvider {
    client: reqwest::Client,
    api_key: String,
    /// Includes the version segment; `/chat/completions` hangs off it.
    base_url: String,
    provider_id: String,
    display: String,
    extra_headers: Vec<(String, String)>,
}

impl OpenAiCompatProvider {
    pub fn from_vendor(api_key: String, cfg: &VendorConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: cfg.resolved_base_url(),
            provider_id: cfg.id.clone(),
            display: cfg.display.clone(),
            extra_headers: cfg
                .extra_headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }
}

#[async_trait]
impl Provider for OpenAiCompatProvider {
    fn id(&self) -> &str {
        &self.provider_id
    }
    fn display_name(&self) -> &str {
        &self.display
    }

    async fn complete(&self, req: CompletionRequest) -> Result<EventStream, ProviderError> {
        let body = openai_compat::build_request_body(&req);
        let mut headers = vec![
            ("authorization".into(), format!("Bearer {}", self.api_key)),
            ("accept".into(), "text/event-stream".into()),
        ];
        headers.extend(self.extra_headers.clone());
        http::post_sse(
            &self.client,
            &format!("{}/chat/completions", self.base_url),
            headers,
            body,
            openai_compat::OpenAiMapper::default,
            |mapper, sse| mapper.feed(sse),
        )
        .await
    }

    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        // Static trait fallback — the live catalog path is commands::list_models.
        Ok(vec![])
    }
}
