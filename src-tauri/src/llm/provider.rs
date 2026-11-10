use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;

use crate::llm::types::ChatMessageDto;

/// Data packet supplied to a provider implementation.
/// Encapsulates the conversation turns, optional system prompt, optional raw screenshot JPEG,
/// and optional Windows OCR extracted text.
pub struct ProviderRequest<'a> {
    pub messages: &'a [ChatMessageDto],
    pub system_prompt: &'a str,
    pub jpeg_bytes: Option<&'a [u8]>,
    pub ocr_text: Option<&'a str>,
}

/// Abstract contract for an LLM provider (Object-Oriented Abstraction Principle).
/// Both Gemini and Groq implement this interface.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Identifier for the provider (e.g. "gemini", "groq")
    fn provider_id(&self) -> &'static str;

    /// Human-friendly display label (e.g. "Google Gemini", "Groq Cloud")
    fn display_name(&self) -> &'static str;

    /// Currently active model identifier
    fn model(&self) -> &str;

    /// Whether this provider and selected model support direct image inputs
    fn supports_vision(&self) -> bool;

    /// Execute completion call with the given request payload
    async fn complete(&self, req: &ProviderRequest<'_>, client: &reqwest::Client) -> Result<String, String>;

    /// Execute streaming completion call, emitting incremental text deltas over `delta_tx`
    async fn complete_stream(
        &self,
        req: &ProviderRequest<'_>,
        client: &reqwest::Client,
        delta_tx: &UnboundedSender<String>,
    ) -> Result<String, String>;
}

/// Parses an error response body cleanly across OpenAI-compatible and Gemini JSON errors.
pub async fn extract_http_error(res: reqwest::Response, provider_label: &str) -> String {
    let status = res.status();
    let body_text = res.text().await.unwrap_or_default();
    let message = serde_json::from_str::<serde_json::Value>(&body_text)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| {
            if body_text.trim().is_empty() {
                format!("Unknown {provider_label} API error")
            } else {
                body_text
            }
        });
    format!("{provider_label} API Error ({status}): {message}")
}

/// Consumes an HTTP `text/event-stream` response line by line and forwards each `data:` payload.
/// Keeps line buffering safe across partial UTF-8/TCP chunks without duplicating SSE loop code.
pub async fn consume_sse_stream<F>(
    res: reqwest::Response,
    provider_label: &str,
    mut on_data: F,
) -> Result<(), String>
where
    F: FnMut(&str) -> Result<bool, String>,
{
    let mut stream = res.bytes_stream();
    let mut raw_buf: Vec<u8> = Vec::new();

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| format!("{provider_label} stream read error: {e}"))?;
        raw_buf.extend_from_slice(&chunk);

        while let Some(nl_pos) = raw_buf.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = raw_buf.drain(..=nl_pos).collect();
            let line = String::from_utf8_lossy(&line_bytes);
            let trimmed = line.trim();
            if let Some(payload) = trimmed.strip_prefix("data:") {
                let data = payload.trim();
                if !data.is_empty() && !on_data(data)? {
                    return Ok(());
                }
            }
        }
    }

    if !raw_buf.is_empty() {
        let trailing = String::from_utf8_lossy(&raw_buf);
        let trimmed = trailing.trim();
        if let Some(payload) = trimmed.strip_prefix("data:") {
            let data = payload.trim();
            if !data.is_empty() {
                let _ = on_data(data)?;
            }
        }
    }

    Ok(())
}
