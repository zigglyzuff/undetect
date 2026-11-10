use crate::llm::gemini::GeminiProvider;
use crate::llm::groq::GroqProvider;
use crate::llm::provider::LlmProvider;
use crate::llm::types::LlmConfig;

/// Factory for instantiating polymorphic `LlmProvider` implementations (Factory Pattern).
pub struct LlmFactory;

impl LlmFactory {
    /// Create a boxed `LlmProvider` based on active configuration or explicit overrides.
    pub fn create_provider(
        provider_name: &str,
        config: &LlmConfig,
        model_override: Option<&str>,
    ) -> Result<Box<dyn LlmProvider>, String> {
        match provider_name.to_lowercase().as_str() {
            "gemini" => {
                let model = model_override
                    .unwrap_or(&config.gemini_model);
                Ok(Box::new(GeminiProvider::new(&config.gemini_api_key, model)))
            }
            "groq" => {
                let model = model_override
                    .unwrap_or(&config.groq_model);
                Ok(Box::new(GroqProvider::new(&config.groq_api_key, model)))
            }
            other => Err(format!(
                "Unsupported LLM provider '{}'. Supported providers are 'gemini' and 'groq'.",
                other
            )),
        }
    }
}
