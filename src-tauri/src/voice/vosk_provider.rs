use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use super::audio::VOSK_SAMPLE_RATE;
use super::ffi;
use super::provider::{SpeechProvider, TranscriptEvent, TranscriptSink};
use super::setup::{resolve_model_dir, resolve_runtime_dll, VoiceConfig};

struct CachedVoskModel {
    runtime_path: PathBuf,
    model_path: PathBuf,
    model: Arc<ffi::VoskModelHandle>,
}

/// Offline speech-to-text provider backed by `libvosk` and local acoustic models.
pub struct VoskSpeechProvider {
    cached_model: Mutex<Option<CachedVoskModel>>,
}

impl VoskSpeechProvider {
    pub fn new() -> Self {
        Self {
            cached_model: Mutex::new(None),
        }
    }

    pub fn clear_cache(&self) {
        if let Ok(mut guard) = self.cached_model.lock() {
            *guard = None;
        }
    }

    fn load_or_reuse_model(
        &self,
        runtime_path: &Path,
        model_path: &Path,
    ) -> Result<Arc<ffi::VoskModelHandle>, String> {
        let mut cache = self.cached_model.lock().unwrap();
        if let Some(existing) = cache.as_ref()
            && existing.runtime_path == runtime_path && existing.model_path == model_path
        {
            return Ok(Arc::clone(&existing.model));
        }

        let api = ffi::VoskApi::load(runtime_path)?;
        let model = Arc::new(ffi::VoskModelHandle::new(api, model_path)?);
        *cache = Some(CachedVoskModel {
            runtime_path: runtime_path.to_path_buf(),
            model_path: model_path.to_path_buf(),
            model: Arc::clone(&model),
        });
        Ok(model)
    }
}

impl Default for VoskSpeechProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeechProvider for VoskSpeechProvider {
    fn id(&self) -> &'static str {
        "vosk"
    }

    fn display_name(&self) -> &'static str {
        "Vosk (Offline)"
    }

    fn validate_ready(&self, cfg: &VoiceConfig) -> Result<(), String> {
        resolve_runtime_dll(cfg.custom_runtime_dir.as_deref()).ok_or_else(|| {
            "Vosk runtime is not installed yet. Download offline assets or switch to Deepgram in Settings."
                .to_string()
        })?;
        resolve_model_dir(cfg.custom_model_dir.as_deref()).ok_or_else(|| {
            "Vosk speech model is not installed yet. Download offline assets or switch to Deepgram in Settings."
                .to_string()
        })?;
        Ok(())
    }

    fn stream_session(
        &self,
        cfg: &VoiceConfig,
        pcm16k_rx: mpsc::Receiver<Vec<i16>>,
        stop_flag: Arc<AtomicBool>,
        ready_tx: mpsc::Sender<Result<(), String>>,
        sink: Arc<dyn TranscriptSink>,
    ) -> Result<(), String> {
        let runtime_path = match resolve_runtime_dll(cfg.custom_runtime_dir.as_deref()) {
            Some(p) => p,
            None => {
                let err = "Vosk runtime is missing.".to_string();
                let _ = ready_tx.send(Err(err.clone()));
                return Err(err);
            }
        };
        let model_path = match resolve_model_dir(cfg.custom_model_dir.as_deref()) {
            Some(p) => p,
            None => {
                let err = "Vosk model directory is missing.".to_string();
                let _ = ready_tx.send(Err(err.clone()));
                return Err(err);
            }
        };

        let model = match self.load_or_reuse_model(&runtime_path, &model_path) {
            Ok(m) => m,
            Err(e) => {
                let _ = ready_tx.send(Err(e.clone()));
                return Err(e);
            }
        };

        let mut recognizer = match ffi::VoskRecognizerHandle::new(model, VOSK_SAMPLE_RATE as f32) {
            Ok(r) => r,
            Err(e) => {
                let _ = ready_tx.send(Err(e.clone()));
                return Err(e);
            }
        };

        let _ = ready_tx.send(Ok(()));

        let mut last_partial = String::new();
        while !stop_flag.load(Ordering::Relaxed) {
            match pcm16k_rx.recv_timeout(Duration::from_millis(80)) {
                Ok(samples) => {
                    if recognizer.accept_waveform(&samples) {
                        let text = recognizer.get_result_text();
                        if !text.is_empty() {
                            last_partial.clear();
                            sink.emit_transcript(TranscriptEvent {
                                kind: "final".to_string(),
                                text,
                                speech_final: true,
                            });
                        }
                    } else {
                        let partial = recognizer.get_partial_text();
                        if partial != last_partial {
                            last_partial = partial.clone();
                            sink.emit_transcript(TranscriptEvent {
                                kind: "partial".to_string(),
                                text: partial,
                                speech_final: false,
                            });
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        let trailing = recognizer.get_final_text();
        if !trailing.is_empty() {
            sink.emit_transcript(TranscriptEvent {
                kind: "final".to_string(),
                text: trailing,
                speech_final: true,
            });
        }

        Ok(())
    }
}
