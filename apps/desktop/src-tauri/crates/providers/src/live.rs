//! Live provider implementations over the shared HTTP/SSE layer.

use crate::{anthropic, http, openai_compat, CompletionRequest, EventStream, Provider, ProviderError};
use async_trait::async_trait;

pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    pub base_url: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: "https://api.anthropic.com".into(),
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn id(&self) -> &'static str {
        "anthropic"
    }
    fn display_name(&self) -> &'static str {
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

pub struct OpenAiCompatProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    provider_id: &'static str,
    display: &'static str,
    extra_headers: Vec<(String, String)>,
}

impl OpenAiCompatProvider {
    pub fn openai(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: "https://api.openai.com".into(),
            provider_id: "openai",
            display: "OpenAI",
            extra_headers: vec![],
        }
    }

    pub fn openrouter(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: "https://openrouter.ai/api".into(),
            provider_id: "openrouter",
            display: "OpenRouter",
            extra_headers: vec![
                ("HTTP-Referer".into(), "https://github.com/truongnat/aix".into()),
                ("X-Title".into(), "aix".into()),
            ],
        }
    }
}

#[async_trait]
impl Provider for OpenAiCompatProvider {
    fn id(&self) -> &'static str {
        self.provider_id
    }
    fn display_name(&self) -> &'static str {
        self.display
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
            &format!("{}/v1/chat/completions", self.base_url),
            headers,
            body,
            openai_compat::OpenAiMapper::default,
            |mapper, sse| mapper.feed(sse),
        )
        .await
    }

    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        match self.provider_id {
            "openai" => Ok(vec!["gpt-5".into(), "gpt-5-mini".into()]),
            _ => Ok(vec![]), // OpenRouter catalog is large; queried live in a later task
        }
    }
}
