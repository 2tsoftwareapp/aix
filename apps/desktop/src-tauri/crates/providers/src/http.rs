//! Thin HTTP layer: POST with SSE streaming, shared by all providers.

use crate::sse::SseDecoder;
use crate::{EventStream, ProviderError, StreamEvent};
use futures_util::StreamExt;

/// POST `body` to `url` with headers, stream SSE bytes through `map` per event.
/// `map` is a stateful closure factory — the mapper is created fresh per stream.
pub async fn post_sse<M, F>(
    client: &reqwest::Client,
    url: &str,
    headers: Vec<(String, String)>,
    body: serde_json::Value,
    make_mapper: F,
    mut drain: impl FnMut(&mut M, &crate::sse::SseEvent) -> Vec<StreamEvent> + Send + 'static,
) -> Result<EventStream, ProviderError>
where
    M: Send + 'static,
    F: FnOnce() -> M + Send,
{
    let mut req = client.post(url).json(&body);
    for (k, v) in headers {
        req = req.header(k, v);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        let body = resp.text().await.unwrap_or_default();
        return Err(match status {
            401 | 403 => ProviderError::Auth(body),
            429 => ProviderError::RateLimited { retry_after_ms: None },
            _ => ProviderError::Api { status, body },
        });
    }
    let mut mapper = make_mapper();
    let stream = resp.bytes_stream();
    let events = stream.scan(
        (SseDecoder::new(), Vec::<StreamEvent>::new()),
        move |(decoder, _), chunk| {
            let mut batch = Vec::new();
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    for sse in decoder.feed(&text) {
                        batch.extend(drain(&mut mapper, &sse));
                    }
                }
                Err(e) => batch.push(StreamEvent::Error {
                    message: format!("stream error: {e}"),
                }),
            }
            futures_util::future::ready(Some(batch))
        },
    );
    // flatten batches into individual events, empty batches dropped
    let flat = events.flat_map(|batch| futures_util::stream::iter(batch.into_iter().map(Ok)));
    Ok(Box::pin(flat))
}
