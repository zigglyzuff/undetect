use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use tauri::{AppHandle, Emitter};

use super::setup::VoiceConfig;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct TranscriptEvent {
    pub kind: String,
    pub text: String,
    pub speech_final: bool,
}

/// Sink abstraction decoupling speech providers from Tauri window delivery.
pub trait TranscriptSink: Send + Sync {
    fn emit_transcript(&self, event: TranscriptEvent);
    fn emit_error(&self, message: String);
}

pub struct TauriTranscriptSink {
    app: AppHandle,
    stop_flag: Arc<AtomicBool>,
    auto_stop: bool,
}

impl TauriTranscriptSink {
    pub fn new(app: AppHandle, stop_flag: Arc<AtomicBool>, auto_stop: bool) -> Self {
        Self {
            app,
            stop_flag,
            auto_stop,
        }
    }
}

impl TranscriptSink for TauriTranscriptSink {
    fn emit_transcript(&self, event: TranscriptEvent) {
        let should_stop = self.auto_stop
            && event.kind == "final"
            && event.speech_final
            && !event.text.trim().is_empty();
        let _ = self.app.emit("vosk://transcript", event);
        if should_stop {
            self.stop_flag.store(true, Ordering::SeqCst);
        }
    }

    fn emit_error(&self, message: String) {
        let _ = self.app.emit("vosk://error", message);
    }
}

/// Core polymorphic contract implemented by all live speech-to-text providers.
pub trait SpeechProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    /// Checks synchronously whether the provider has valid credentials/assets configured.
    fn validate_ready(&self, cfg: &VoiceConfig) -> Result<(), String>;

    /// Streams 16 kHz mono `i16` PCM audio chunks from `pcm16k_rx` until `stop_flag` triggers
    /// or the audio channel closes. Must send `Ok(())` or `Err(...)` once ready on `ready_tx`.
    fn stream_session(
        &self,
        cfg: &VoiceConfig,
        pcm16k_rx: mpsc::Receiver<Vec<i16>>,
        stop_flag: Arc<AtomicBool>,
        ready_tx: mpsc::Sender<Result<(), String>>,
        sink: Arc<dyn TranscriptSink>,
    ) -> Result<(), String>;
}
