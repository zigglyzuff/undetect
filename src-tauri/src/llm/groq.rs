use async_trait::async_trait;
use base64::Engine;
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;

use crate::llm::provider::{consume_sse_stream, extract_http_error, LlmProvider, ProviderRequest};

/// Concrete implementation of `LlmProvider` for Groq Cloud.
/// Encapsulates authentication, OpenAI-compatible JSON framing, LPU inference,
/// and hybrid vision / on-device Windows OCR fallback integration.
pub struct GroqProvider {
    api_key: String,
    model: String,
}

impl GroqProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let m = model.into();
        let final_model = if m.trim().is_empty() {
            "openai/gpt-oss-120b".to_string()
        } else {
            m
        };
        Self {
            api_key: api_key.into(),
            model: final_model,
        }
    }

    /// Check if the selected Groq model supports direct multimodal image input
    pub fn model_supports_vision(model_id: &str) -> bool {
        let lower = model_id.to_lowercase();
        lower.contains("vision")
            || lower.contains("vl")
            || lower.contains("llama-4")
            || lower.contains("qwen3.8")
            || lower.contains("muse-spark")
            || lower.contains("llava")
            || lower.contains("pixtral")
    }

    fn build_payload(&self, req: &ProviderRequest<'_>, stream: bool) -> Result<serde_json::Value, String> {
        let is_vision = self.supports_vision();
        let mut messages = Vec::new();

        if !req.system_prompt.trim().is_empty() {
            messages.push(json!({
                "role": "system",
                "content": req.system_prompt
            }));
        }

        let total_msgs = req.messages.len();
        for (idx, msg) in req.messages.iter().enumerate() {
            let is_last = idx + 1 == total_msgs;
            let role = match msg.role.as_str() {
                "assistant" | "model" => "assistant",
                "system" => "system",
                _ => "user",
            };

            if is_last && role == "user" {
                let ocr_prefix = req
                    .ocr_text
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|ocr| {
                        if is_vision {
                            format!("[Windows On-Device Screen OCR Extracted Text]:\n{}\n\n", ocr)
                        } else {
                            format!("[Screen Capture Text via Windows On-Device OCR (text-only model; non-text visuals not visible)]:\n{}\n\n", ocr)
                        }
                    })
                    .unwrap_or_default();

                if is_vision && let Some(jpeg) = req.jpeg_bytes {
                    // Vision model path: format content as an array with text and image_url
                    let b64 = base64::engine::general_purpose::STANDARD.encode(jpeg);
                    let data_url = format!("data:image/jpeg;base64,{}", b64);
                    let text_content = format!("{}{}", ocr_prefix, msg.content);

                    messages.push(json!({
                        "role": "user",
                        "content": [
                            {
                                "type": "text",
                                "text": text_content
                            },
                            {
                                "type": "image_url",
                                "image_url": {
                                    "url": data_url
                                }
                            }
                        ]
                    }));
                } else {
                    // Text-only model path: relies on extracted Windows OCR text
                    if req.jpeg_bytes.is_some() && ocr_prefix.is_empty() {
                        return Err(format!(
                            "Model '{}' is text-only and Windows OCR found no readable text on screen. Switch to Google Gemini or a Groq vision model (e.g. qwen/qwen3.8-27b) under Settings -> Model Provider.",
                            self.model
                        ));
                    }

                    let text_content = format!("{}{}", ocr_prefix, msg.content);
                    messages.push(json!({
                        "role": "user",
                        "content": text_content
                    }));
                }
            } else {
                messages.push(json!({
                    "role": role,
                    "content": msg.content
                }));
            }
        }

        Ok(json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
            "max_completion_tokens": 2048,
            "stream": stream
        }))
    }

    fn extract_sse_delta(data: &str) -> Result<Option<String>, bool> {
        if data == "[DONE]" {
            return Err(true);
        }
        let Ok(val) = serde_json::from_str::<serde_json::Value>(data) else {
            return Ok(None);
        };
        Ok(val["choices"][0]["delta"]["content"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_string))
    }
}

#[async_trait]
impl LlmProvider for GroqProvider {
    fn provider_id(&self) -> &'static str {
        "groq"
    }

    fn display_name(&self) -> &'static str {
        "Groq Cloud"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn supports_vision(&self) -> bool {
        Self::model_supports_vision(&self.model)
    }

    async fn complete(&self, req: &ProviderRequest<'_>, client: &reqwest::Client) -> Result<String, String> {
        if self.api_key.trim().is_empty() {
            return Err("Groq API Key is missing. Please set your Groq API key in Settings.".to_string());
        }

        let url = "https://api.groq.com/openai/v1/chat/completions";
        let payload = self.build_payload(req, false)?;

        let res = client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key.trim()))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Failed to send request to Groq API: {e}"))?;

        if !res.status().is_success() {
            return Err(extract_http_error(res, "Groq").await);
        }

        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse Groq API JSON response: {e}"))?;

        let text = body["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| "Empty response received from Groq API".to_string())?;

        Ok(text.to_string())
    }

    async fn complete_stream(
        &self,
        req: &ProviderRequest<'_>,
        client: &reqwest::Client,
        delta_tx: &UnboundedSender<String>,
    ) -> Result<String, String> {
        if self.api_key.trim().is_empty() {
            return Err("Groq API Key is missing. Please set your Groq API key in Settings.".to_string());
        }

        let url = "https://api.groq.com/openai/v1/chat/completions";
        let payload = self.build_payload(req, true)?;

        let res = client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key.trim()))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Failed to send request to Groq API: {e}"))?;

        if !res.status().is_success() {
            return Err(extract_http_error(res, "Groq").await);
        }

        let mut full = String::new();
        consume_sse_stream(res, "Groq", |data| match Self::extract_sse_delta(data) {
            Err(true) => Ok(false),
            Err(false) => Ok(true),
            Ok(Some(delta)) => {
                full.push_str(&delta);
                let _ = delta_tx.send(delta);
                Ok(true)
            }
            Ok(None) => Ok(true),
        })
        .await?;

        if full.is_empty() {
            return Err("Empty response received from Groq API".to_string());
        }
        Ok(full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::ChatMessageDto;

    #[test]
    fn vision_detection_includes_current_groq_vision_models() {
        assert!(GroqProvider::model_supports_vision("qwen/qwen3.8-27b"));
        assert!(GroqProvider::model_supports_vision("meta-models/muse-spark-open"));
        assert!(GroqProvider::model_supports_vision("meta-llama/llama-4-scout-17b-16e-instruct"));
        assert!(GroqProvider::model_supports_vision("meta-llama/llama-4-maverick-17b-128e-instruct"));
        assert!(GroqProvider::model_supports_vision("llama-3.2-11b-vision-preview"));
        assert!(!GroqProvider::model_supports_vision("openai/gpt-oss-120b"));
        assert!(!GroqProvider::model_supports_vision("allam-2-7b"));
    }

    #[test]
    fn qwen38_payload_attaches_image_url() {
        let provider = GroqProvider::new("gsk_test", "qwen/qwen3.8-27b");
        let msgs = vec![ChatMessageDto {
            role: "user".to_string(),
            content: "What is on my screen?".to_string(),
            ocr_text: None,
        }];
        let jpeg = [0xFFu8, 0xD8, 0xFF, 0xD9];
        let req = ProviderRequest {
            messages: &msgs,
            system_prompt: "Helpful assistant",
            jpeg_bytes: Some(&jpeg),
            ocr_text: None,
        };

        let payload = provider.build_payload(&req, false).expect("payload should build");
        let content = &payload["messages"][1]["content"];
        assert!(content.is_array());
        assert_eq!(content[1]["type"], "image_url");
        assert!(content[1]["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        assert_eq!(payload["max_completion_tokens"], 2048);
        assert_eq!(payload["stream"], false);
    }

    #[test]
    fn text_only_model_rejects_blank_ocr_with_clear_error() {
        let provider = GroqProvider::new("gsk_test", "llama-3.3-70b-versatile");
        let msgs = vec![ChatMessageDto {
            role: "user".to_string(),
            content: "What is on my screen?".to_string(),
            ocr_text: None,
        }];
        let jpeg = [0xFFu8, 0xD8, 0xFF, 0xD9];
        let req = ProviderRequest {
            messages: &msgs,
            system_prompt: "",
            jpeg_bytes: Some(&jpeg),
            ocr_text: Some("   "),
        };

        let err = provider.build_payload(&req, false).unwrap_err();
        assert!(err.contains("text-only"));
    }

    #[test]
    fn extracts_groq_sse_deltas_cleanly() {
        assert_eq!(
            GroqProvider::extract_sse_delta(r#"{"choices":[{"delta":{"content":"Hello"}}]}"#),
            Ok(Some("Hello".to_string()))
        );
        assert_eq!(GroqProvider::extract_sse_delta("[DONE]"), Err(true));
    }
}
