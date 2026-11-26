//! Live model catalog fetching. OpenRouter exposes a public index —
//! no auth required — so the UI can offer current models instead of
//! a hardcoded list that ages within weeks.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub context_length: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

/// GET https://openrouter.ai/api/v1/models — public endpoint, no auth.
pub async fn fetch_openrouter_models(client: &reqwest::Client) -> Result<Vec<ModelInfo>, crate::ProviderError> {
    let resp = client
        .get("https://openrouter.ai/api/v1/models")
        .send()
        .await
        .map_err(|e| crate::ProviderError::Network(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(crate::ProviderError::Api {
            status: resp.status().as_u16(),
            body: resp.text().await.unwrap_or_default(),
        });
    }
    let parsed: ModelsResponse = resp
        .json()
        .await
        .map_err(|e| crate::ProviderError::Network(format!("parse: {e}")))?;
    Ok(parsed.data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_models_response_shape() {
        let json = r#"{"data":[{"id":"anthropic/claude-sonnet-4.5","name":"Claude Sonnet 4.5","context_length":200000},{"id":"openai/gpt-5.1","name":"GPT-5.1"}]}"#;
        let parsed: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.data.len(), 2);
        assert_eq!(parsed.data[0].id, "anthropic/claude-sonnet-4.5");
        assert_eq!(parsed.data[0].context_length, Some(200000));
        assert_eq!(parsed.data[1].context_length, None);
    }

    #[test]
    fn parses_anthropic_models_shape() {
        let json = r#"{"data":[{"id":"claude-sonnet-4-5","display_name":"Claude Sonnet 4.5","type":"model"}],"has_more":false}"#;
        let parsed: AnthropicModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.data[0].id, "claude-sonnet-4-5");
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
    #[serde(default)]
    display_name: String,
}

/// Fetch the model catalog for a provider using its own API. Requires a
/// retrievable key for all providers except openrouter (public index).
/// Returns Err on auth/network failure — callers fall back to static lists.
pub async fn fetch_models(
    provider_id: &str,
    client: &reqwest::Client,
) -> Result<Vec<ModelInfo>, crate::ProviderError> {
    if provider_id == "openrouter" {
        return fetch_openrouter_models(client).await;
    }
    let key = crate::credentials::retrieve(provider_id)
        .map_err(|e| crate::ProviderError::Auth(e.to_string()))?;
    match provider_id {
        "anthropic" => {
            let resp = client
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", &key)
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
                .map_err(|e| crate::ProviderError::Network(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(crate::ProviderError::Api {
                    status: resp.status().as_u16(),
                    body: resp.text().await.unwrap_or_default(),
                });
            }
            let parsed: AnthropicModelsResponse = resp
                .json()
                .await
                .map_err(|e| crate::ProviderError::Network(format!("parse: {e}")))?;
            Ok(parsed
                .data
                .into_iter()
                .map(|m| ModelInfo {
                    id: m.id,
                    name: m.display_name,
                    context_length: None,
                })
                .collect())
        }
        "openai" | "qwen" => {
            let base = if provider_id == "qwen" {
                std::env::var("DASHSCOPE_BASE_URL")
                    .unwrap_or_else(|_| "https://dashscope-intl.aliyuncs.com/compatible-mode/v1".into())
            } else {
                "https://api.openai.com/v1".into()
            };
            let resp = client
                .get(format!("{base}/models"))
                .bearer_auth(&key)
                .send()
                .await
                .map_err(|e| crate::ProviderError::Network(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(crate::ProviderError::Api {
                    status: resp.status().as_u16(),
                    body: resp.text().await.unwrap_or_default(),
                });
            }
            let parsed: ModelsResponse = resp
                .json()
                .await
                .map_err(|e| crate::ProviderError::Network(format!("parse: {e}")))?;
            let mut models: Vec<ModelInfo> = parsed
                .data
                .into_iter()
                .filter(|m| {
                    // keep chat-capable families; drop embeddings/audio/image/legacy
                    let id = m.id.as_str();
                    id.starts_with("gpt-")
                        || id.starts_with("o1")
                        || id.starts_with("o3")
                        || id.starts_with("o4")
                        || id.starts_with("chatgpt-")
                        || id.starts_with("qwen")
                })
                .collect();
            models.sort_by(|a, b| b.id.cmp(&a.id));
            Ok(models)
        }
        other => Err(crate::ProviderError::Api {
            status: 400,
            body: format!("unknown provider: {other}"),
        }),
    }
}
