use serde::{Deserialize, Serialize};
use crate::llm::resume::DEFAULT_INTERVIEW_SYSTEM_PROMPT;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageDto {
    pub role: String, // "user", "assistant", or "system"
    pub content: String,
    #[serde(default)]
    pub ocr_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessageDto>,
    #[serde(default)]
    pub screenshot_id: Option<String>,
    #[serde(default)]
    pub use_ocr: bool,
    #[serde(default)]
    pub provider: Option<String>, // override "gemini" or "groq"
    #[serde(default)]
    pub model: Option<String>,    // override model
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub success: bool,
    pub reply: String,
    #[serde(default)]
    pub ocr_text: Option<String>,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub active_provider: String, // "gemini" or "groq"
    pub gemini_api_key: String,
    pub gemini_model: String,
    pub groq_api_key: String,
    pub groq_model: String,
    pub auto_ocr: bool,
    pub system_prompt: String,
    #[serde(default)]
    pub resume_filename: String,
    #[serde(default)]
    pub resume_pages: u32,
    #[serde(default)]
    pub resume_text: String,
    #[serde(default)]
    pub job_description: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            active_provider: "groq".to_string(),
            gemini_api_key: "".to_string(),
            gemini_model: "gemini-3.6-flash".to_string(),
            groq_api_key: "".to_string(),
            groq_model: "openai/gpt-oss-120b".to_string(),
            auto_ocr: true,
            system_prompt: DEFAULT_INTERVIEW_SYSTEM_PROMPT.to_string(),
            resume_filename: String::new(),
            resume_pages: 0,
            resume_text: String::new(),
            job_description: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicLlmConfig {
    pub active_provider: String,
    pub has_gemini_key: bool,
    pub gemini_key_masked: String,
    pub gemini_model: String,
    pub has_groq_key: bool,
    pub groq_key_masked: String,
    pub groq_model: String,
    pub auto_ocr: bool,
    pub system_prompt: String,
    pub has_resume: bool,
    pub resume_filename: String,
    pub resume_pages: u32,
    pub resume_chars: usize,
    pub resume_preview: String,
    pub has_jd: bool,
    pub job_description: String,
}

impl PublicLlmConfig {
    pub fn from_config(cfg: &LlmConfig) -> Self {
        let mask_key = |k: &str| -> String {
            if k.len() <= 6 {
                if k.is_empty() { "".to_string() } else { "••••••".to_string() }
            } else {
                format!("{}••••{}", &k[..3], &k[k.len() - 3..])
            }
        };

        let trimmed_resume = cfg.resume_text.trim();
        let trimmed_jd = cfg.job_description.trim();
        Self {
            active_provider: cfg.active_provider.clone(),
            has_gemini_key: !cfg.gemini_api_key.trim().is_empty(),
            gemini_key_masked: mask_key(&cfg.gemini_api_key),
            gemini_model: cfg.gemini_model.clone(),
            has_groq_key: !cfg.groq_api_key.trim().is_empty(),
            groq_key_masked: mask_key(&cfg.groq_api_key),
            groq_model: cfg.groq_model.clone(),
            auto_ocr: cfg.auto_ocr,
            system_prompt: cfg.system_prompt.clone(),
            has_resume: !trimmed_resume.is_empty(),
            resume_filename: cfg.resume_filename.clone(),
            resume_pages: cfg.resume_pages,
            resume_chars: trimmed_resume.chars().count(),
            resume_preview: trimmed_resume.to_string(),
            has_jd: !trimmed_jd.is_empty(),
            job_description: trimmed_jd.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRequest {
    #[serde(default)]
    pub screenshot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResponse {
    pub success: bool,
    pub text: String,
    #[serde(default)]
    pub error: Option<String>,
}
