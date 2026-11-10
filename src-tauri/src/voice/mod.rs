pub mod audio;
pub mod deepgram_provider;
pub mod factory;
pub mod ffi;
pub mod provider;
pub mod setup;
pub mod vosk_provider;

use cpal::traits::StreamTrait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub use audio::{list_audio_devices, AudioDeviceDto, VOSK_SAMPLE_RATE};
pub use factory::SpeechProviderFactory;
pub use provider::{SpeechProvider, TauriTranscriptSink, TranscriptEvent, TranscriptSink};
pub use setup::{VoiceConfig, VoiceStatus};

struct VoiceSession {
    stop_flag: Arc<AtomicBool>,
    handles: Vec<JoinHandle<()>>,
}

pub struct VoiceService {
    config: RwLock<VoiceConfig>,
    factory: SpeechProviderFactory,
    active_session: Mutex<Option<VoiceSession>>,
    listening: Arc<AtomicBool>,
    installing: AtomicBool,
    last_error: RwLock<Option<String>>,
}

impl Default for VoiceService {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceService {
    pub fn new() -> Self {
        Self {
            config: RwLock::new(setup::load_voice_config()),
            factory: SpeechProviderFactory::new(),
            active_session: Mutex::new(None),
            listening: Arc::new(AtomicBool::new(false)),
            installing: AtomicBool::new(false),
            last_error: RwLock::new(None),
        }
    }

    pub fn get_status(&self) -> VoiceStatus {
        let cfg = self.config.read().unwrap().clone();
        let runtime_path = setup::resolve_runtime_dll(cfg.custom_runtime_dir.as_deref());
        let model_path = setup::resolve_model_dir(cfg.custom_model_dir.as_deref());
        let provider = self.factory.resolve(&cfg.provider);
        let ready = provider.validate_ready(&cfg).is_ok();
        let listening = self.listening.load(Ordering::Relaxed);
        let has_deepgram_key = !cfg.deepgram_api_key.trim().is_empty();

        VoiceStatus {
            provider: provider.id().to_string(),
            runtime_installed: runtime_path.is_some(),
            model_installed: model_path.is_some(),
            has_deepgram_key,
            deepgram_key_masked: setup::mask_api_key(&cfg.deepgram_api_key),
            deepgram_model: cfg.deepgram_model,
            deepgram_language: cfg.deepgram_language,
            auto_send: cfg.auto_send,
            ready,
            listening,
            installing: self.installing.load(Ordering::Relaxed),
            source: cfg.source,
            device_name: cfg.device_name,
            runtime_path: runtime_path.map(|p| p.to_string_lossy().to_string()),
            model_path: model_path.map(|p| p.to_string_lossy().to_string()),
            custom_runtime_dir: cfg.custom_runtime_dir,
            custom_model_dir: cfg.custom_model_dir,
            error: self.last_error.read().unwrap().clone(),
        }
    }

    pub fn update_config(&self, mut next: VoiceConfig) -> Result<VoiceStatus, String> {
        let current = self.config.read().unwrap().clone();

        next.provider = setup::normalize_provider(&next.provider);
        next.source = setup::normalize_source(&next.source);
        next.device_name = next.device_name.filter(|s| !s.trim().is_empty());
        next.custom_runtime_dir = next.custom_runtime_dir.filter(|s| !s.trim().is_empty());
        next.custom_model_dir = next.custom_model_dir.filter(|s| !s.trim().is_empty());

        if next.deepgram_api_key.trim().is_empty() || next.deepgram_api_key.contains('•') {
            next.deepgram_api_key = current.deepgram_api_key;
        } else {
            next.deepgram_api_key = next.deepgram_api_key.trim().to_string();
        }
        if next.deepgram_model.trim().is_empty() {
            next.deepgram_model = "nova-3".to_string();
        }
        if next.deepgram_language.trim().is_empty() {
            next.deepgram_language = "en-US".to_string();
        }

        setup::save_voice_config(&next)?;
        *self.config.write().unwrap() = next;
        self.factory.invalidate_local_cache();
        *self.last_error.write().unwrap() = None;
        Ok(self.get_status())
    }

    pub fn clear_deepgram_key(&self) -> Result<VoiceStatus, String> {
        let mut next = self.config.read().unwrap().clone();
        next.deepgram_api_key.clear();
        setup::save_voice_config(&next)?;
        *self.config.write().unwrap() = next;
        self.factory.invalidate_local_cache();
        *self.last_error.write().unwrap() = None;
        Ok(self.get_status())
    }

    /// Switches audio source (`"mic"` or `"system"`) and clears stale device selection when changed.
    pub fn set_source(&self, app: &AppHandle, source: &str) -> Result<VoiceStatus, String> {
        let was_listening = self.listening.load(Ordering::Relaxed);
        if was_listening {
            self.stop_listening_internal(app);
        }

        let normalized = setup::normalize_source(source);
        let mut next = self.config.read().unwrap().clone();
        if next.source != normalized {
            next.source = normalized;
            next.device_name = None;
            setup::save_voice_config(&next)?;
            *self.config.write().unwrap() = next;
            *self.last_error.write().unwrap() = None;
        }

        if was_listening {
            return self.start_listening(app);
        }
        Ok(self.get_status())
    }

    pub async fn setup_vosk(&self, app: &AppHandle) -> Result<VoiceStatus, String> {
        if self.installing.swap(true, Ordering::SeqCst) {
            return Err("Vosk download is already in progress".to_string());
        }

        *self.last_error.write().unwrap() = None;
        let cfg = self.config.read().unwrap().clone();
        let res = setup::install_default_vosk(app, &cfg).await;
        self.installing.store(false, Ordering::SeqCst);

        if let Err(err) = res {
            *self.last_error.write().unwrap() = Some(err.clone());
            return Err(err);
        }

        self.factory.invalidate_local_cache();
        Ok(self.get_status())
    }

    pub fn start_listening(&self, app: &AppHandle) -> Result<VoiceStatus, String> {
        self.stop_listening_internal(app);

        let cfg = self.config.read().unwrap().clone();
        let provider = self.factory.resolve(&cfg.provider);
        provider.validate_ready(&cfg)?;

        let stop_flag = Arc::new(AtomicBool::new(false));
        let (pcm16k_tx, pcm16k_rx) = mpsc::channel::<Vec<i16>>();

        let (audio_ready_tx, audio_ready_rx) = mpsc::channel::<Result<(), String>>();
        let (provider_ready_tx, provider_ready_rx) = mpsc::channel::<Result<(), String>>();

        let audio_stop = Arc::clone(&stop_flag);
        let audio_source = cfg.source.clone();
        let audio_device = cfg.device_name.clone();
        let audio_app = app.clone();

        let audio_handle = std::thread::Builder::new()
            .name("undetect-audio-capture".to_string())
            .spawn(move || {
                let (raw_tx, raw_rx) = mpsc::channel::<Vec<f32>>();
                let (err_tx, err_rx) = mpsc::channel::<String>();

                let (stream, channels, src_rate) = match audio::open_raw_input_stream(
                    &audio_source,
                    audio_device.as_deref(),
                    raw_tx,
                    err_tx,
                ) {
                    Ok(res) => res,
                    Err(e) => {
                        let _ = audio_ready_tx.send(Err(e));
                        return;
                    }
                };

                if let Err(e) = stream.play() {
                    let _ = audio_ready_tx.send(Err(format!("Failed to start audio capture: {}", e)));
                    return;
                }

                let _ = audio_ready_tx.send(Ok(()));
                let mut resampler = audio::Resampler16k::new(src_rate);

                while !audio_stop.load(Ordering::Relaxed) {
                    if let Ok(stream_err) = err_rx.try_recv() {
                        eprintln!("[Undetect Voice] {}", stream_err);
                        let _ = audio_app.emit("vosk://error", stream_err);
                        audio_stop.store(true, Ordering::Relaxed);
                        break;
                    }

                    match raw_rx.recv_timeout(Duration::from_millis(60)) {
                        Ok(raw_samples) => {
                            let samples = resampler.push_interleaved(&raw_samples, channels);
                            if !samples.is_empty() && pcm16k_tx.send(samples).is_err() {
                                break;
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }

                drop(stream);
            })
            .map_err(|e| format!("Failed to spawn audio thread: {}", e))?;

        let provider_stop = Arc::clone(&stop_flag);
        let provider_listening = Arc::clone(&self.listening);
        let provider_app = app.clone();
        let provider_cfg = cfg.clone();
        let sink: Arc<dyn TranscriptSink> =
            Arc::new(TauriTranscriptSink::new(app.clone(), Arc::clone(&stop_flag)));

        let provider_handle = std::thread::Builder::new()
            .name("undetect-speech-provider".to_string())
            .spawn(move || {
                let res = provider.stream_session(
                    &provider_cfg,
                    pcm16k_rx,
                    Arc::clone(&provider_stop),
                    provider_ready_tx,
                    sink,
                );
                if let Err(e) = res {
                    eprintln!("[Undetect Voice] Provider error: {}", e);
                    let _ = provider_app.emit("vosk://error", e);
                }
                provider_stop.store(true, Ordering::Relaxed);
                provider_listening.store(false, Ordering::SeqCst);
                let _ = provider_app.emit("vosk://state", false);
            })
            .map_err(|e| format!("Failed to spawn speech provider thread: {}", e))?;

        let audio_ok = audio_ready_rx.recv_timeout(Duration::from_secs(4));
        if !matches!(audio_ok, Ok(Ok(()))) {
            stop_flag.store(true, Ordering::Relaxed);
            self.listening.store(false, Ordering::SeqCst);
            let _ = audio_handle.join();
            let _ = provider_handle.join();
            let msg = match audio_ok {
                Ok(Err(e)) => e,
                _ => "Timed out initializing audio capture device.".to_string(),
            };
            *self.last_error.write().unwrap() = Some(msg.clone());
            return Err(msg);
        }

        match provider_ready_rx.recv_timeout(Duration::from_secs(7)) {
            Ok(Ok(())) => {
                self.listening.store(true, Ordering::SeqCst);
                *self.last_error.write().unwrap() = None;
                *self.active_session.lock().unwrap() = Some(VoiceSession {
                    stop_flag,
                    handles: vec![audio_handle, provider_handle],
                });
                let _ = app.emit("vosk://state", true);
                Ok(self.get_status())
            }
            Ok(Err(e)) => {
                stop_flag.store(true, Ordering::Relaxed);
                self.listening.store(false, Ordering::SeqCst);
                let _ = audio_handle.join();
                let _ = provider_handle.join();
                *self.last_error.write().unwrap() = Some(e.clone());
                Err(e)
            }
            Err(_) => {
                stop_flag.store(true, Ordering::Relaxed);
                self.listening.store(false, Ordering::SeqCst);
                let _ = audio_handle.join();
                let _ = provider_handle.join();
                let msg = "Timed out connecting speech recognition provider.".to_string();
                *self.last_error.write().unwrap() = Some(msg.clone());
                Err(msg)
            }
        }
    }

    pub fn stop_listening(&self, app: &AppHandle) -> VoiceStatus {
        self.stop_listening_internal(app);
        self.get_status()
    }

    fn stop_listening_internal(&self, app: &AppHandle) {
        let was_listening = self.listening.load(Ordering::SeqCst);
        if self.stop_active_worker() && was_listening {
            let _ = app.emit("vosk://state", false);
        }
    }

    fn stop_active_worker(&self) -> bool {
        let mut session = self.active_session.lock().unwrap().take();
        if let Some(ref mut s) = session {
            s.stop_flag.store(true, Ordering::Relaxed);
            for jh in s.handles.drain(..) {
                let _ = jh.join();
            }
            self.listening.store(false, Ordering::SeqCst);
            return true;
        }
        false
    }

    pub fn toggle_listening(&self, app: &AppHandle) -> Result<VoiceStatus, String> {
        if self.listening.load(Ordering::Relaxed) {
            Ok(self.stop_listening(app))
        } else {
            self.start_listening(app)
        }
    }
}

impl Drop for VoiceService {
    fn drop(&mut self) {
        let _ = self.stop_active_worker();
    }
}
