use async_trait::async_trait;
use base64::Engine;
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;

use crate::llm::provider::{consume_sse_stream, extract_http_error, LlmProvider, ProviderRequest};

/// Concrete implementation of `LlmProvider` for Google Gemini.
/// Encapsulates authentication, Gemini REST payload framing, and multimodal support.
pub struct GeminiProvider {
    api_key: String,
    model: String,
}

impl GeminiProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let m = model.into();
        let final_model = if m.trim().is_empty() {
            "gemini-3.6-flash".to_string()
        } else {
            m
        };
        Self {
            api_key: api_key.into(),
            model: final_model,
        }
    }

    fn build_payload(&self, req: &ProviderRequest<'_>) -> serde_json::Value {
        let mut contents = Vec::new();
        let total_msgs = req.messages.len();

        for (idx, msg) in req.messages.iter().enumerate() {
            let is_last = idx + 1 == total_msgs;
            let role = match msg.role.as_str() {
                "assistant" | "model" => "model",
                _ => "user",
            };

            let mut parts = Vec::new();

            if is_last && role == "user" {
                if let Some(jpeg) = req.jpeg_bytes {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(jpeg);
                    parts.push(json!({
                        "inlineData": {
                            "mimeType": "image/jpeg",
                            "data": b64
                        }
                    }));
                }

                let mut text_content = String::new();
                if let Some(ocr) = req.ocr_text
                    && !ocr.trim().is_empty()
                {
                    text_content.push_str(&format!("[Windows On-Device Screen OCR Extracted Text]:\n{}\n\n", ocr.trim()));
                }
                text_content.push_str(&msg.content);
                parts.push(json!({ "text": text_content }));
            } else {
                parts.push(json!({ "text": msg.content }));
            }

            contents.push(json!({
                "role": role,
                "parts": parts
            }));
        }

        let mut payload = json!({
            "contents": contents
        });

        if !req.system_prompt.trim().is_empty() {
            payload["systemInstruction"] = json!({
                "parts": [{ "text": req.system_prompt }]
            });
        }

        payload
    }

    fn extract_parts_text(body: &serde_json::Value) -> Option<String> {
        let parts = body["candidates"][0]["content"]["parts"].as_array()?;
        let collected: Vec<&str> = parts
            .iter()
            .filter(|p| !p["thought"].as_bool().unwrap_or(false))
            .filter_map(|p| p["text"].as_str())
            .collect();
        if collected.is_empty() {
            None
        } else {
            Some(collected.join(""))
        }
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    fn provider_id(&self) -> &'static str {
        "gemini"
    }

    fn display_name(&self) -> &'static str {
        "Google Gemini"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn supports_vision(&self) -> bool {
        true
    }

    async fn complete(&self, req: &ProviderRequest<'_>, client: &reqwest::Client) -> Result<String, String> {
        if self.api_key.trim().is_empty() {
            return Err("Gemini API Key is missing. Please set your Gemini API key in Settings.".to_string());
        }

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model
        );
        let payload = self.build_payload(req);

        let res = client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Failed to send request to Gemini API: {e}"))?;

        if !res.status().is_success() {
            return Err(extract_http_error(res, "Gemini").await);
        }

        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse Gemini API JSON response: {e}"))?;

        Self::extract_parts_text(&body).ok_or_else(|| {
            if let Some(block_reason) = body["candidates"][0]["finishReason"].as_str() {
                format!("Gemini finished with reason: {}", block_reason)
            } else {
                "Empty or malformed response received from Gemini API".to_string()
            }
        })
    }

    async fn complete_stream(
        &self,
        req: &ProviderRequest<'_>,
        client: &reqwest::Client,
        delta_tx: &UnboundedSender<String>,
    ) -> Result<String, String> {
        if self.api_key.trim().is_empty() {
            return Err("Gemini API Key is missing. Please set your Gemini API key in Settings.".to_string());
        }

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
            self.model
        );
        let payload = self.build_payload(req);

        let res = client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Failed to send request to Gemini API: {e}"))?;

        if !res.status().is_success() {
            return Err(extract_http_error(res, "Gemini").await);
        }

        let mut full = String::new();
        let mut finish_reason: Option<String> = None;

        consume_sse_stream(res, "Gemini", |data| {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(reason) = val["candidates"][0]["finishReason"].as_str() {
                    finish_reason = Some(reason.to_string());
                }
                if let Some(text) = Self::extract_parts_text(&val)
                    && !text.is_empty()
                {
                    full.push_str(&text);
                    let _ = delta_tx.send(text);
                }
            }
            Ok(true)
        })
        .await?;

        if full.is_empty() {
            return Err(match finish_reason {
                Some(reason) => format!("Gemini finished with reason: {}", reason),
                None => "Empty or malformed response received from Gemini API".to_string(),
            });
        }
        Ok(full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::ChatMessageDto;

    #[test]
    fn gemini_payload_attaches_inline_jpeg() {
        let provider = GeminiProvider::new("AIza_test", "gemini-3.8-flash");
        let msgs = vec![ChatMessageDto {
            role: "user".to_string(),
            content: "Describe screen".to_string(),
            ocr_text: None,
        }];
        let jpeg = [0xFFu8, 0xD8, 0xFF, 0xD9];
        let req = ProviderRequest {
            messages: &msgs,
            system_prompt: "",
            jpeg_bytes: Some(&jpeg),
            ocr_text: None,
        };

        let payload = provider.build_payload(&req);
        let parts = &payload["contents"][0]["parts"];
        assert_eq!(parts[0]["inlineData"]["mimeType"], "image/jpeg");
        assert!(!parts[0]["inlineData"]["data"].as_str().unwrap().is_empty());
        assert_eq!(parts[1]["text"], "Describe screen");
    }

    #[test]
    fn skips_thought_parts_in_gemini_sse_chunks() {
        let chunk = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        { "text": "internal thought", "thought": true },
                        { "text": "Answer " },
                        { "text": "here." }
                    ]
                }
            }]
        });
        assert_eq!(
            GeminiProvider::extract_parts_text(&chunk),
            Some("Answer here.".to_string())
        );
    }
}
