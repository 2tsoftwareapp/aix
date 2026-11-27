//! OpenAI-compatible embeddings client and vendor resolution.

use crate::ProviderError;
use serde_json::Value;
use std::collections::BTreeMap;

/// Runtime configuration for one embedding-capable vendor. The key is used
/// only for the Authorization header and redacted from Debug output.
#[derive(Clone)]
pub struct EmbeddingConfig {
    pub provider_id: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub extra_headers: BTreeMap<String, String>,
}

impl std::fmt::Debug for EmbeddingConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingConfig")
            .field("provider_id", &self.provider_id)
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .field("extra_headers", &self.extra_headers)
            .finish()
    }
}

pub fn default_embedding_model(provider_id: &str) -> &'static str {
    match provider_id {
        "openrouter" => "openai/text-embedding-3-small",
        "google" => "gemini-embedding-001",
        _ => "text-embedding-3-small",
    }
}

pub fn embedding_request_body(model: &str, inputs: &[String]) -> Value {
    serde_json::json!({
        "model": model,
        "input": inputs,
    })
}

pub fn embedding_capable(vendor: &crate::vendor::VendorConfig) -> bool {
    vendor.kind == "openai_compat"
}

pub fn config_for_vendor(
    vendor: &crate::vendor::VendorConfig,
    api_key: String,
    model_override: Option<String>,
) -> EmbeddingConfig {
    EmbeddingConfig {
        provider_id: vendor.id.clone(),
        model: model_override.unwrap_or_else(|| default_embedding_model(&vendor.id).to_string()),
        base_url: vendor.resolved_base_url(),
        api_key,
        extra_headers: vendor.extra_headers.clone(),
    }
}

/// Resolve the embedding vendor without embedding credential lookup in the
/// decision logic itself — this keeps the selection policy testable without
/// mutating process-wide environment state.
pub fn resolve_embedding_config_with<F>(
    preferred_provider: Option<&str>,
    provider_override: Option<&str>,
    model_override: Option<&str>,
    retrieve_key: F,
) -> Option<EmbeddingConfig>
where
    F: Fn(&str) -> Option<String>,
{
    let model = model_override.map(str::to_string);
    let config_for = |id: &str| {
        crate::vendor::vendor(id)
            .filter(|v| embedding_capable(v))
            .and_then(|v| retrieve_key(id).map(|key| config_for_vendor(v, key, model.clone())))
    };

    if let Some(explicit) = provider_override {
        return config_for(explicit);
    }
    if let Some(preferred) = preferred_provider {
        if let Some(cfg) = config_for(preferred) {
            return Some(cfg);
        }
    }
    crate::vendor::vendors()
        .iter()
        .filter(|v| embedding_capable(v))
        .find_map(|v| retrieve_key(&v.id).map(|key| config_for_vendor(v, key, model.clone())))
}

pub fn resolve_embedding_config(
    preferred_provider: Option<&str>,
    provider_override: Option<&str>,
    model_override: Option<&str>,
) -> Option<EmbeddingConfig> {
    resolve_embedding_config_with(
        preferred_provider,
        provider_override,
        model_override,
        |id| crate::credentials::retrieve(id).ok(),
    )
}

pub fn parse_embedding_response(
    body: Value,
    expected: usize,
) -> Result<Vec<Vec<f32>>, ProviderError> {
    let data = body
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::Api {
            status: 200,
            body: "embedding response missing data array".into(),
        })?;
    let mut vectors: Vec<Option<Vec<f32>>> = vec![None; expected];
    for item in data {
        let index = item
            .get("index")
            .and_then(Value::as_u64)
            .unwrap_or(usize::MAX as u64) as usize;
        if index >= expected {
            continue;
        }
        let values = item
            .get("embedding")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::Api {
                status: 200,
                body: "embedding item missing embedding array".into(),
            })?;
        let mut vector = Vec::with_capacity(values.len());
        for value in values {
            let Some(number) = value.as_f64() else {
                return Err(ProviderError::Api {
                    status: 200,
                    body: "embedding vector contains a non-numeric value".into(),
                });
            };
            vector.push(number as f32);
        }
        if vector.is_empty() {
            return Err(ProviderError::Api {
                status: 200,
                body: "embedding vector must not be empty".into(),
            });
        }
        vectors[index] = Some(vector);
    }
    let mut out = Vec::with_capacity(expected);
    for vector in vectors {
        out.push(vector.ok_or_else(|| ProviderError::Api {
            status: 200,
            body: format!(
                "embedding response returned {} vectors, expected {expected}",
                data.len()
            ),
        })?);
    }
    Ok(out)
}

const EMBEDDING_BATCH_SIZE: usize = 64;

/// Embed texts through the OpenAI-compatible `/embeddings` endpoint. Inputs
/// are batched so first-time project indexing does not create one request per
/// chunk; callers persist the returned vectors by content hash.
pub async fn embed_texts(
    config: &EmbeddingConfig,
    inputs: &[String],
) -> Result<Vec<Vec<f32>>, ProviderError> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let client = reqwest::Client::new();
    let url = format!("{}/embeddings", config.base_url.trim_end_matches('/'));
    let mut out = Vec::with_capacity(inputs.len());
    for batch in inputs.chunks(EMBEDDING_BATCH_SIZE) {
        let body = embedding_request_body(&config.model, batch);
        let mut req = client.post(&url).bearer_auth(&config.api_key).json(&body);
        for (key, value) in &config.extra_headers {
            req = req.header(key, value);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            let body = resp.text().await.unwrap_or_default();
            return Err(crate::openai_compat::map_http_error(status, &body));
        }
        let body = resp
            .json::<Value>()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        out.extend(parse_embedding_response(body, batch.len())?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_models_are_small_text_embedding_models_per_vendor() {
        assert_eq!(default_embedding_model("openai"), "text-embedding-3-small");
        assert_eq!(
            default_embedding_model("openrouter"),
            "openai/text-embedding-3-small"
        );
        assert_eq!(default_embedding_model("google"), "gemini-embedding-001");
    }

    #[test]
    fn request_body_uses_openai_embeddings_shape() {
        let body = embedding_request_body("embed-model", &["alpha".into(), "beta".into()]);
        assert_eq!(body["model"], "embed-model");
        assert_eq!(body["input"], serde_json::json!(["alpha", "beta"]));
    }

    #[test]
    fn response_vectors_are_reordered_by_index() {
        let response = serde_json::json!({
            "data": [
                {"index": 1, "embedding": [2.0, 3.0]},
                {"index": 0, "embedding": [0.5, 1.5]}
            ]
        });
        let vectors = parse_embedding_response(response, 2).unwrap();
        assert_eq!(vectors, vec![vec![0.5, 1.5], vec![2.0, 3.0]]);
    }

    #[test]
    fn response_with_missing_vector_is_an_error() {
        let response = serde_json::json!({
            "data": [{"index": 0, "embedding": [1.0]}]
        });
        assert!(parse_embedding_response(response, 2).is_err());
    }

    #[test]
    fn response_with_non_numeric_embedding_value_is_an_error() {
        let response = serde_json::json!({
            "data": [{"index": 0, "embedding": [1.0, "bad"]}]
        });
        assert!(parse_embedding_response(response, 1).is_err());
    }

    #[test]
    fn config_for_vendor_applies_model_override_and_extra_headers() {
        let vendor = crate::vendor::vendor("openrouter").unwrap();
        let cfg = config_for_vendor(vendor, "key".into(), Some("custom/embed".into()));
        assert_eq!(cfg.provider_id, "openrouter");
        assert_eq!(cfg.model, "custom/embed");
        assert_eq!(cfg.base_url, vendor.resolved_base_url());
        assert!(cfg.extra_headers.contains_key("HTTP-Referer"));
    }

    #[test]
    fn anthropic_is_not_embedding_capable() {
        let anthropic = crate::vendor::vendor("anthropic").unwrap();
        assert!(!embedding_capable(anthropic));
        let openai = crate::vendor::vendor("openai").unwrap();
        assert!(embedding_capable(openai));
    }

    #[test]
    fn resolver_never_selects_anthropic_even_with_a_key() {
        let cfg = resolve_embedding_config_with(Some("anthropic"), None, None, |id| {
            (id == "anthropic").then(|| "key".into())
        });
        assert!(cfg.is_none());
    }

    #[test]
    fn resolver_prefers_explicit_capable_provider() {
        let cfg = resolve_embedding_config_with(
            Some("anthropic"),
            Some("openai"),
            Some("custom/embed".into()),
            |id| (id == "openai").then(|| "key".into()),
        )
        .unwrap();
        assert_eq!(cfg.provider_id, "openai");
        assert_eq!(cfg.model, "custom/embed");
    }

    #[test]
    fn resolver_scans_configured_openai_compatible_vendors_as_fallback() {
        let cfg = resolve_embedding_config_with(Some("anthropic"), None, None, |id| {
            (id == "openrouter").then(|| "key".into())
        })
        .unwrap();
        assert_eq!(cfg.provider_id, "openrouter");
        assert_eq!(cfg.model, "openai/text-embedding-3-small");
    }

    #[tokio::test]
    async fn embed_texts_batches_large_input_sets() {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = requests.clone();
        tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let seen = seen.clone();
                tokio::spawn(async move {
                    seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let (read_half, mut write_half) = socket.into_split();
                    let mut reader = tokio::io::BufReader::new(read_half);
                    let mut content_len = 0usize;
                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).await.unwrap();
                        if line == "\r\n" || line.is_empty() {
                            break;
                        }
                        if line.to_ascii_lowercase().starts_with("content-length:") {
                            content_len = line.split(':').nth(1).unwrap().trim().parse().unwrap();
                        }
                    }
                    let mut body = vec![0u8; content_len];
                    reader.read_exact(&mut body).await.unwrap();
                    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
                    let inputs = json["input"].as_array().unwrap().len();
                    let data = (0..inputs)
                        .map(|i| {
                            serde_json::json!({
                                "index": i,
                                "embedding": [i as f32, inputs as f32]
                            })
                        })
                        .collect::<Vec<_>>();
                    let response = serde_json::json!({"data": data}).to_string();
                    write_half.write_all(format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                        response.len(), response
                    ).as_bytes()).await.unwrap();
                });
            }
        });

        let cfg = EmbeddingConfig {
            provider_id: "test".into(),
            model: "embed".into(),
            base_url: format!("http://{addr}"),
            api_key: "key".into(),
            extra_headers: Default::default(),
        };
        let inputs = (0..65).map(|i| format!("chunk {i}")).collect::<Vec<_>>();
        let vectors = embed_texts(&cfg, &inputs).await.unwrap();
        assert_eq!(vectors.len(), 65);
        assert_eq!(vectors[0], vec![0.0, 64.0]);
        assert_eq!(vectors[64], vec![0.0, 1.0]);
        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn config_debug_does_not_leak_api_key() {
        let cfg = EmbeddingConfig {
            provider_id: "openai".into(),
            model: "text-embedding-3-small".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: "sk-secret".into(),
            extra_headers: Default::default(),
        };
        let rendered = format!("{cfg:?}");
        assert!(!rendered.contains("sk-secret"));
    }
}
