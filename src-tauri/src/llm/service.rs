use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::capture;
use crate::llm::factory::LlmFactory;
use crate::llm::groq::GroqProvider;
use crate::llm::provider::{LlmProvider, ProviderRequest};
use crate::llm::resume::{self, DEFAULT_INTERVIEW_SYSTEM_PROMPT};
use crate::llm::types::{ChatRequest, ChatResponse, LlmConfig, PublicLlmConfig};
use crate::ocr;
use crate::state::AppState;

/// Coordinator Service managing LLM provider lifecycle, configuration persistence,
/// screen capture resolution, Windows OCR extraction, and multi-turn chat sessions.
pub struct LlmService {
    pub config: RwLock<LlmConfig>,
    pub http_client: reqwest::Client,
    storage_path: PathBuf,
}

impl LlmService {
    pub fn new() -> Self {
        let storage_path = Self::get_config_path();
        let loaded_config = Self::load_config_from_disk(&storage_path).unwrap_or_default();

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap_or_default();

        Self {
            config: RwLock::new(loaded_config),
            http_client,
            storage_path,
        }
    }

    fn get_config_path() -> PathBuf {
        if let Some(mut dir) = dirs_fallback() {
            dir.push("undetect_llm_config.json");
            return dir;
        }
        PathBuf::from("undetect_llm_config.json")
    }

    fn normalize_provider(provider: &str) -> String {
        if provider.trim().eq_ignore_ascii_case("gemini") {
            "gemini".to_string()
        } else {
            "groq".to_string()
        }
    }

    fn normalize_gemini_model(model: &str) -> String {
        match model.trim() {
            "" | "gemini-1.5-flash" | "gemini-1.5-pro" | "gemini-2.0-flash"
            | "gemini-2.5-flash" | "gemini-2.5-pro" | "gemini-3-pro-preview" => {
                "gemini-3.6-flash".to_string()
            }
            "gemini-2.0-flash-lite" | "gemini-3.1-flash-lite-preview" => {
                "gemini-3.5-flash-lite".to_string()
            }
            other => other.to_string(),
        }
    }

    fn load_config_from_disk(path: &PathBuf) -> Option<LlmConfig> {
        let data = std::fs::read_to_string(path).ok()?;
        let mut cfg: LlmConfig = serde_json::from_str(&data).ok()?;
        cfg.active_provider = Self::normalize_provider(&cfg.active_provider);
        cfg.gemini_model = Self::normalize_gemini_model(&cfg.gemini_model);
        if cfg.system_prompt.trim().is_empty()
            || cfg.system_prompt.starts_with("You are an intelligent desktop assistant for Undetect")
        {
            cfg.system_prompt = DEFAULT_INTERVIEW_SYSTEM_PROMPT.to_string();
        }
        Some(cfg)
    }

    pub fn save_config(&self, new_cfg: LlmConfig) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&new_cfg)
            .map_err(|e| format!("Failed to serialize LLM config: {e}"))?;

        if let Some(parent) = self.storage_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        std::fs::write(&self.storage_path, json)
            .map_err(|e| format!("Failed to write LLM config to file: {e}"))?;

        if let Ok(mut lock) = self.config.write() {
            *lock = new_cfg;
        }
        Ok(())
    }

    pub fn get_public_config(&self) -> PublicLlmConfig {
        let cfg = self.config.read().unwrap().clone();
        PublicLlmConfig::from_config(&cfg)
    }

    pub fn get_internal_config(&self) -> LlmConfig {
        self.config.read().unwrap().clone()
    }

    pub fn apply_config_update(&self, mut next: LlmConfig) -> Result<PublicLlmConfig, String> {
        let current = self.get_internal_config();
        if next.gemini_api_key.trim().is_empty() || next.gemini_api_key.contains('•') {
            next.gemini_api_key = current.gemini_api_key;
        }
        if next.groq_api_key.trim().is_empty() || next.groq_api_key.contains('•') {
            next.groq_api_key = current.groq_api_key;
        }
        if next.resume_text.trim().is_empty() && !current.resume_text.is_empty() {
            next.resume_filename = current.resume_filename;
            next.resume_pages = current.resume_pages;
            next.resume_text = current.resume_text;
        }
        if next.job_description.trim().is_empty() && !current.job_description.trim().is_empty() {
            next.job_description = current.job_description;
        }
        next.active_provider = Self::normalize_provider(&next.active_provider);
        next.gemini_model = Self::normalize_gemini_model(&next.gemini_model);
        self.save_config(next)?;
        Ok(self.get_public_config())
    }

    pub fn set_job_description(&self, jd: &str) -> Result<PublicLlmConfig, String> {
        let mut cfg = self.get_internal_config();
        let trimmed = jd.trim();
        cfg.job_description = if trimmed.is_empty() {
            String::new()
        } else {
            resume::normalize_resume_text(trimmed)
        };
        self.save_config(cfg)?;
        Ok(self.get_public_config())
    }

    pub fn clear_api_key(&self, provider: &str) -> Result<PublicLlmConfig, String> {
        let mut cfg = self.get_internal_config();
        match provider.trim().to_ascii_lowercase().as_str() {
            "gemini" => cfg.gemini_api_key.clear(),
            "groq" => cfg.groq_api_key.clear(),
            other => return Err(format!("Unsupported LLM provider '{other}'")),
        }
        self.save_config(cfg)?;
        Ok(self.get_public_config())
    }

    pub fn upload_resume(&self, filename: &str, bytes: &[u8]) -> Result<PublicLlmConfig, String> {
        let extracted = resume::extract_resume_from_bytes(filename, bytes)?;
        let mut cfg = self.get_internal_config();
        cfg.resume_filename = extracted.filename;
        cfg.resume_pages = extracted.page_count;
        cfg.resume_text = extracted.text;
        self.save_config(cfg)?;
        Ok(self.get_public_config())
    }

    pub fn clear_resume(&self) -> Result<PublicLlmConfig, String> {
        let mut cfg = self.get_internal_config();
        cfg.resume_filename.clear();
        cfg.resume_pages = 0;
        cfg.resume_text.clear();
        self.save_config(cfg)?;
        Ok(self.get_public_config())
    }

    /// Fetch available Groq models dynamically from https://api.groq.com/openai/v1/models
    pub async fn fetch_groq_models(&self, api_key_override: Option<&str>) -> Result<Vec<String>, String> {
        let config = self.get_internal_config();
        let key = api_key_override
            .filter(|k| !k.trim().is_empty())
            .unwrap_or(&config.groq_api_key)
            .trim();

        if key.is_empty() {
            return Ok(vec![
                "openai/gpt-oss-120b".to_string(),
                "qwen/qwen3.8-27b".to_string(),
                "openai/gpt-oss-20b".to_string(),
                "allam-2-7b".to_string(),
            ]);
        }

        let res = self
            .http_client
            .get("https://api.groq.com/openai/v1/models")
            .header("Authorization", format!("Bearer {}", key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| format!("Failed to request Groq models: {e}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body: serde_json::Value = res.json().await.unwrap_or_default();
            let msg = err_body["error"]["message"]
                .as_str()
                .unwrap_or("Failed to fetch Groq models");
            return Err(format!("Groq API ({}): {}", status, msg));
        }

        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse Groq models response: {e}"))?;

        let mut models = Vec::new();
        if let Some(data) = body["data"].as_array() {
            for item in data {
                if let Some(id) = item["id"].as_str() {
                    let active = item["active"].as_bool().unwrap_or(true);
                    if active && Self::is_supported_groq_chat_model(id) {
                        models.push(id.to_string());
                    }
                }
            }
        }

        Self::sort_groq_models(&mut models);

        if models.is_empty() {
            models.push("openai/gpt-oss-120b".to_string());
        }

        Ok(models)
    }

    fn is_supported_groq_chat_model(id: &str) -> bool {
        let lower = id.to_lowercase();
        !lower.starts_with("whisper")
            && !lower.starts_with("tts")
            && !lower.contains("orpheus")
            && !lower.contains("prompt-guard")
            && !lower.contains("safeguard")
    }

    fn sort_groq_models(models: &mut [String]) {
        models.sort_by(|a, b| {
            let score = |s: &str| -> i32 {
                if s.contains("gpt-oss-120b") { 0 }
                else if GroqProvider::model_supports_vision(s) { 1 }
                else if s.contains("gpt-oss") { 2 }
                else if s.contains("llama-3.3") { 3 }
                else if s.contains("deepseek") { 4 }
                else { 5 }
            };
            score(a).cmp(&score(b)).then_with(|| a.cmp(b))
        });
    }

    /// Resolve screenshot JPEG bytes synchronously for `"capture"`, `"latest"`, or an explicit ID.
    fn resolve_screenshot_bytes(target_id: &str, state: &Arc<AppState>) -> Result<Vec<u8>, String> {
        if target_id == "capture" {
            return capture::capture_primary_screen_jpeg();
        }

        if target_id == "latest" {
            if let Ok(list) = state.screenshots.read()
                && let Some(entry) = list.back()
            {
                return Ok(entry.jpeg_data.clone());
            }
            // If no capture exists yet in session history, capture one automatically
            capture::capture_primary_screen(state)?;
            let list = state.screenshots.read().map_err(|e| e.to_string())?;
            return list
                .back()
                .map(|s| s.jpeg_data.clone())
                .ok_or_else(|| "Failed to capture initial screenshot".to_string());
        }

        let list = state.screenshots.read().map_err(|e| e.to_string())?;
        list.iter()
            .find(|s| s.id == target_id)
            .map(|s| s.jpeg_data.clone())
            .ok_or_else(|| "Selected screenshot is no longer available. Please capture a new screenshot.".to_string())
    }

    /// Extract text from a screenshot in AppState (or capture live if empty) using Windows on-device OCR
    pub fn extract_ocr_from_state(
        &self,
        screenshot_id: Option<&str>,
        state: &AppState,
    ) -> Result<String, String> {
        let jpeg_data = {
            let list = state.screenshots.read().map_err(|e| e.to_string())?;
            let entry = match screenshot_id {
                Some("latest") | Some("capture") | None => list.back(),
                Some(target_id) => list.iter().find(|s| s.id == target_id),
            };
            entry.map(|s| s.jpeg_data.clone())
        };

        let bytes = match jpeg_data {
            Some(b) => b,
            None => match screenshot_id {
                Some("latest") | Some("capture") | None => capture::capture_primary_screen_jpeg()?,
                Some(_) => return Err("No screenshot available for OCR recognition".to_string()),
            },
        };

        ocr::extract_text_from_jpeg(&bytes)
    }

    async fn prepare_chat(
        &self,
        req: &ChatRequest,
        state: &Arc<AppState>,
    ) -> Result<(Box<dyn LlmProvider>, Option<Vec<u8>>, Option<String>, String), String> {
        let config = self.get_internal_config();

        let provider_name = req
            .provider
            .as_deref()
            .unwrap_or(&config.active_provider);

        let provider = LlmFactory::create_provider(
            provider_name,
            &config,
            req.model.as_deref(),
        )?;

        // Retrieve or capture screenshot bytes if screen inclusion is enabled
        let jpeg_bytes: Option<Vec<u8>> = if let Some(target_id) = req.screenshot_id.clone() {
            let state_clone = Arc::clone(state);
            let bytes = tokio::task::spawn_blocking(move || {
                Self::resolve_screenshot_bytes(&target_id, &state_clone)
            })
            .await
            .map_err(|e| format!("Screenshot capture task failed: {e}"))??;
            Some(bytes)
        } else {
            None
        };

        // Run Windows OCR on blocking thread when requested, auto_ocr enabled, or required for text-only model
        let needs_ocr = (req.use_ocr || config.auto_ocr || !provider.supports_vision()) && jpeg_bytes.is_some();
        let ocr_text: Option<String> = if needs_ocr {
            if let Some(ref bytes) = jpeg_bytes {
                let bytes_for_ocr = bytes.clone();
                match tokio::task::spawn_blocking(move || ocr::extract_text_from_jpeg(&bytes_for_ocr)).await {
                    Ok(Ok(text)) => {
                        if !text.trim().is_empty() {
                            Some(text)
                        } else {
                            None
                        }
                    }
                    Ok(Err(e)) => {
                        eprintln!("[Undetect OCR] Warning: Windows OCR error: {e}");
                        None
                    }
                    Err(e) => {
                        eprintln!("[Undetect OCR] Warning: Windows OCR task failed: {e}");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        let effective_system_prompt = resume::build_interview_system_prompt(
            &config.system_prompt,
            &config.resume_filename,
            config.resume_pages,
            &config.resume_text,
            &config.job_description,
        );

        Ok((provider, jpeg_bytes, ocr_text, effective_system_prompt))
    }

    /// Execute a non-streaming chat completion (used by HTTP API & fallback callers)
    pub async fn execute_chat(
        &self,
        req: ChatRequest,
        state: Arc<AppState>,
    ) -> Result<ChatResponse, String> {
        let (provider, jpeg_bytes, ocr_text, effective_system_prompt) =
            self.prepare_chat(&req, &state).await?;

        let provider_req = ProviderRequest {
            messages: &req.messages,
            system_prompt: &effective_system_prompt,
            jpeg_bytes: jpeg_bytes.as_deref(),
            ocr_text: ocr_text.as_deref(),
        };

        let reply = provider.complete(&provider_req, &self.http_client).await?;

        Ok(ChatResponse {
            success: true,
            reply,
            ocr_text,
            provider: provider.provider_id().to_string(),
            model: provider.model().to_string(),
        })
    }

    /// Execute a streaming chat completion, emitting `llm://stream` deltas over Tauri events
    pub async fn execute_chat_stream(
        &self,
        req: ChatRequest,
        state: Arc<AppState>,
        stream_id: String,
        app: AppHandle,
    ) -> Result<ChatResponse, String> {
        #[derive(Clone, Serialize)]
        struct LlmStreamChunkEvent {
            stream_id: String,
            delta: String,
        }

        let (provider, jpeg_bytes, ocr_text, effective_system_prompt) =
            self.prepare_chat(&req, &state).await?;

        let provider_req = ProviderRequest {
            messages: &req.messages,
            system_prompt: &effective_system_prompt,
            jpeg_bytes: jpeg_bytes.as_deref(),
            ocr_text: ocr_text.as_deref(),
        };

        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let forwarder = tokio::spawn(async move {
            while let Some(delta) = delta_rx.recv().await {
                let _ = app.emit(
                    "llm://stream",
                    LlmStreamChunkEvent {
                        stream_id: stream_id.clone(),
                        delta,
                    },
                );
            }
        });

        let stream_res = provider
            .complete_stream(&provider_req, &self.http_client, &delta_tx)
            .await;
        drop(delta_tx);
        let _ = forwarder.await;

        let reply = stream_res?;
        Ok(ChatResponse {
            success: true,
            reply,
            ocr_text,
            provider: provider.provider_id().to_string(),
            model: provider.model().to_string(),
        })
    }
}

impl Default for LlmService {
    fn default() -> Self {
        Self::new()
    }
}

fn dirs_fallback() -> Option<PathBuf> {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let mut p = PathBuf::from(local_app_data);
        p.push("Undetect");
        return Some(p);
    }
    if let Ok(temp) = std::env::var("TEMP") {
        let mut p = PathBuf::from(temp);
        p.push("Undetect");
        return Some(p);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groq_filter_and_sort_puts_default_120b_and_vision_first_and_excludes_tts() {
        let raw = [
            "allam-2-7b",
            "canopylabs/orpheus-arabic-saudi",
            "canopylabs/orpheus-v1-english",
            "meta-llama/llama-prompt-guard-2-22m",
            "meta-llama/llama-prompt-guard-2-86m",
            "openai/gpt-oss-120b",
            "openai/gpt-oss-20b",
            "openai/gpt-oss-safeguard-20b",
            "qwen/qwen3.8-27b",
        ];

        let mut filtered: Vec<String> = raw
            .iter()
            .filter(|id| LlmService::is_supported_groq_chat_model(id))
            .map(|id| (*id).to_string())
            .collect();
        LlmService::sort_groq_models(&mut filtered);

        assert_eq!(
            filtered,
            vec![
                "openai/gpt-oss-120b",
                "qwen/qwen3.8-27b",
                "openai/gpt-oss-20b",
                "allam-2-7b",
            ]
        );
    }

    #[test]
    fn clear_api_key_removes_only_requested_provider_and_persists() {
        let mut temp_path = std::env::temp_dir();
        temp_path.push(format!("undetect_llm_clear_test_{}.json", std::process::id()));
        let _ = std::fs::remove_file(&temp_path);

        let service = LlmService {
            config: RwLock::new(LlmConfig::default()),
            http_client: reqwest::Client::new(),
            storage_path: temp_path.clone(),
        };

        let cfg = LlmConfig {
            gemini_api_key: "gemini-secret-123".to_string(),
            groq_api_key: "groq-secret-456".to_string(),
            ..Default::default()
        };
        service.save_config(cfg).unwrap();

        let after_gemini_clear = service.clear_api_key("gemini").unwrap();
        assert!(!after_gemini_clear.has_gemini_key);
        assert!(after_gemini_clear.has_groq_key);

        let persisted = LlmService::load_config_from_disk(&temp_path).unwrap();
        assert!(persisted.gemini_api_key.is_empty());
        assert_eq!(persisted.groq_api_key, "groq-secret-456");

        let after_groq_clear = service.clear_api_key("groq").unwrap();
        assert!(!after_groq_clear.has_groq_key);

        assert!(service.clear_api_key("unknown").is_err());
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn migrates_obsolete_gemini_models_to_current_free_tier_defaults() {
        assert_eq!(LlmService::normalize_gemini_model("gemini-1.5-flash"), "gemini-3.6-flash");
        assert_eq!(LlmService::normalize_gemini_model("gemini-1.5-pro"), "gemini-3.6-flash");
        assert_eq!(LlmService::normalize_gemini_model("gemini-2.0-flash"), "gemini-3.6-flash");
        assert_eq!(LlmService::normalize_gemini_model("gemini-2.5-flash"), "gemini-3.6-flash");
        assert_eq!(LlmService::normalize_gemini_model("gemini-2.0-flash-lite"), "gemini-3.5-flash-lite");
        assert_eq!(LlmService::normalize_gemini_model("gemini-3.6-flash"), "gemini-3.6-flash");
        assert_eq!(LlmService::normalize_gemini_model("gemini-3.8-flash"), "gemini-3.8-flash");
        assert_eq!(LlmService::normalize_gemini_model("gemini-3.5-flash-lite"), "gemini-3.5-flash-lite");
    }

    #[test]
    fn default_llm_provider_is_groq() {
        let cfg = LlmConfig::default();
        assert_eq!(cfg.active_provider, "groq");
        assert_eq!(cfg.groq_model, "openai/gpt-oss-120b");
        assert_eq!(LlmService::normalize_provider(""), "groq");
        assert_eq!(LlmService::normalize_provider("GEMINI"), "gemini");
    }
}
