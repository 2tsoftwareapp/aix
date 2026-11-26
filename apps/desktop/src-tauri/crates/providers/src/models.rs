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
}
