use std::sync::Arc;

use super::deepgram_provider::DeepgramSpeechProvider;
use super::provider::SpeechProvider;
use super::vosk_provider::VoskSpeechProvider;

/// Factory encapsulating provider selection so `VoiceService` stays decoupled
/// from concrete speech-to-text implementations.
pub struct SpeechProviderFactory {
    vosk: Arc<VoskSpeechProvider>,
    deepgram: Arc<DeepgramSpeechProvider>,
}

impl SpeechProviderFactory {
    pub fn new() -> Self {
        Self {
            vosk: Arc::new(VoskSpeechProvider::new()),
            deepgram: Arc::new(DeepgramSpeechProvider::new()),
        }
    }

    pub fn resolve(&self, provider_id: &str) -> Arc<dyn SpeechProvider> {
        if provider_id.trim().eq_ignore_ascii_case("vosk") {
            Arc::clone(&self.vosk) as Arc<dyn SpeechProvider>
        } else {
            Arc::clone(&self.deepgram) as Arc<dyn SpeechProvider>
        }
    }

    pub fn invalidate_local_cache(&self) {
        self.vosk.clear_cache();
    }
}

impl Default for SpeechProviderFactory {
    fn default() -> Self {
        Self::new()
    }
}
