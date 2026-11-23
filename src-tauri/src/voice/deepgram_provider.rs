use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::Message;

use super::provider::{SpeechProvider, TranscriptEvent, TranscriptSink};
use super::setup::VoiceConfig;

/// Cloud real-time streaming speech-to-text provider backed by Deepgram `/v1/listen`.
pub struct DeepgramSpeechProvider;

impl DeepgramSpeechProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeepgramSpeechProvider {
    fn default() -> Self {
        Self::new()
    }
}

pub fn build_deepgram_ws_url(model: &str, language: &str) -> String {
    let clean_model = model.trim();
    let effective_model = if clean_model.is_empty() {
        "nova-3"
    } else {
        clean_model
    };
    let clean_lang = language.trim();
    let effective_lang = if clean_lang.is_empty() {
        "en-US"
    } else {
        clean_lang
    };

    format!(
        "wss://api.deepgram.com/v1/listen?model={}&language={}&encoding=linear16&sample_rate=16000&channels=1&interim_results=true&punctuate=true&smart_format=true&endpointing=800&utterance_end_ms=1100",
        effective_model, effective_lang
    )
}

pub fn is_deepgram_utterance_end(raw_json: &str) -> bool {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(raw_json) else {
        return false;
    };
    let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if msg_type == "UtteranceEnd" {
        return true;
    }
    msg_type == "Results" && val.get("speech_final").and_then(|v| v.as_bool()).unwrap_or(false)
}

pub fn samples_to_le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

pub fn parse_deepgram_message(raw_json: &str) -> Result<Option<TranscriptEvent>, String> {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(raw_json) else {
        return Ok(None);
    };

    let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if msg_type == "Error" {
        let err_msg = val
            .get("description")
            .or_else(|| val.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("Deepgram streaming error");
        return Err(err_msg.to_string());
    }

    if msg_type != "Results" {
        return Ok(None);
    }

    let transcript = val
        .get("channel")
        .and_then(|c| c.get("alternatives"))
        .and_then(|a| a.as_array())
        .and_then(|arr| arr.first())
        .and_then(|alt| alt.get("transcript"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim();

    if transcript.is_empty() {
        return Ok(None);
    }

    let is_final = val
        .get("is_final")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let speech_final = val
        .get("speech_final")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(Some(TranscriptEvent {
        kind: if is_final {
            "final".to_string()
        } else {
            "partial".to_string()
        },
        text: transcript.to_string(),
        speech_final,
    }))
}

impl SpeechProvider for DeepgramSpeechProvider {
    fn id(&self) -> &'static str {
        "deepgram"
    }

    fn display_name(&self) -> &'static str {
        "Deepgram Streaming"
    }

    fn validate_ready(&self, cfg: &VoiceConfig) -> Result<(), String> {
        if cfg.deepgram_api_key.trim().is_empty() {
            return Err(
                "Deepgram API key is missing. Add your key under Settings -> Voice Input."
                    .to_string(),
            );
        }
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
        let api_key = cfg.deepgram_api_key.trim().to_string();
        if api_key.is_empty() {
            let err = "Deepgram API key is missing.".to_string();
            let _ = ready_tx.send(Err(err.clone()));
            return Err(err);
        }

        let url = build_deepgram_ws_url(&cfg.deepgram_model, &cfg.deepgram_language);
        let mut request = match url.into_client_request() {
            Ok(req) => req,
            Err(e) => {
                let err = format!("Invalid Deepgram URL: {}", e);
                let _ = ready_tx.send(Err(err.clone()));
                return Err(err);
            }
        };

        let auth_header = match HeaderValue::from_str(&format!("Token {}", api_key)) {
            Ok(h) => h,
            Err(e) => {
                let err = format!("Invalid Deepgram API key characters: {}", e);
                let _ = ready_tx.send(Err(err.clone()));
                return Err(err);
            }
        };
        request.headers_mut().insert("Authorization", auth_header);

        crate::tokio_rt().block_on(async move {
            let (ws_stream, _) = match connect_async(request).await {
                Ok(conn) => conn,
                Err(e) => {
                    let msg = format!("Deepgram connection failed: {}", e);
                    let _ = ready_tx.send(Err(msg.clone()));
                    return Err(msg);
                }
            };

            let _ = ready_tx.send(Ok(()));

            let (mut ws_write, mut ws_read) = ws_stream.split();
            let (async_tx, mut async_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(256);
            let pump_stop = Arc::clone(&stop_flag);

            // Bridge blocking `std::sync::mpsc` audio frames onto Tokio async channel,
            // batching samples into ~100ms (1600 samples = 3200 bytes) optimal chunks
            // to avoid flooding the WebSocket with hundreds of tiny packets per second.
            let bridge_handle = tokio::task::spawn_blocking(move || {
                let mut chunk_buf = Vec::with_capacity(3200);
                while !pump_stop.load(Ordering::Relaxed) {
                    match pcm16k_rx.recv_timeout(Duration::from_millis(60)) {
                        Ok(samples) => {
                            if !samples.is_empty() {
                                chunk_buf.extend_from_slice(&samples);
                                if chunk_buf.len() >= 1600 {
                                    let bytes = samples_to_le_bytes(&chunk_buf);
                                    chunk_buf.clear();
                                    if async_tx.blocking_send(bytes).is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if !chunk_buf.is_empty() {
                                let bytes = samples_to_le_bytes(&chunk_buf);
                                chunk_buf.clear();
                                if async_tx.blocking_send(bytes).is_err() {
                                    break;
                                }
                            }
                            continue;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            if !chunk_buf.is_empty() {
                                let bytes = samples_to_le_bytes(&chunk_buf);
                                let _ = async_tx.blocking_send(bytes);
                            }
                            break;
                        }
                    }
                }
            });

            let sink_for_recv = Arc::clone(&sink);
            let recv_stop = Arc::clone(&stop_flag);
            let stop_notify = Arc::new(tokio::sync::Notify::new());
            let stop_notify_recv = Arc::clone(&stop_notify);
            let auto_stop = cfg.auto_send && cfg.source != "system";
            let recv_task = tokio::spawn(async move {
                let mut had_spoken_text = false;
                while let Some(item) = ws_read.next().await {
                    match item {
                        Ok(Message::Text(txt)) => {
                            let raw = txt.as_str();
                            match parse_deepgram_message(raw) {
                                Ok(Some(ev)) => {
                                    had_spoken_text = true;
                                    sink_for_recv.emit_transcript(ev);
                                }
                                Ok(None) => {
                                    if auto_stop && had_spoken_text && is_deepgram_utterance_end(raw) {
                                        recv_stop.store(true, Ordering::SeqCst);
                                    }
                                }
                                Err(err) => {
                                    sink_for_recv.emit_error(err);
                                    recv_stop.store(true, Ordering::SeqCst);
                                    stop_notify_recv.notify_one();
                                    break;
                                }
                            }
                            if recv_stop.load(Ordering::Relaxed) {
                                stop_notify_recv.notify_one();
                                break;
                            }
                        }
                        Ok(Message::Close(maybe_frame)) => {
                            if let Some(frame) = maybe_frame {
                                let code_num = u16::from(frame.code);
                                eprintln!(
                                    "[Undetect Voice] Deepgram closed connection: code={}, reason={}",
                                    code_num, frame.reason
                                );
                                if code_num != 1000 {
                                    sink_for_recv.emit_error(format!(
                                        "Deepgram closed: {} ({})",
                                        frame.reason, code_num
                                    ));
                                }
                            }
                            recv_stop.store(true, Ordering::SeqCst);
                            stop_notify_recv.notify_one();
                            break;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            if !recv_stop.load(Ordering::Relaxed) {
                                sink_for_recv.emit_error(format!("Deepgram stream closed: {}", e));
                                recv_stop.store(true, Ordering::SeqCst);
                            }
                            stop_notify_recv.notify_one();
                            break;
                        }
                    }
                }
            });

            let mut last_audio_sent = Instant::now();
            let mut keepalive_tick = tokio::time::interval(Duration::from_millis(120));

            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                tokio::select! {
                    _ = stop_notify.notified() => {
                        if stop_flag.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                    maybe_bytes = async_rx.recv() => {
                        match maybe_bytes {
                            Some(bytes) => {
                                if !bytes.is_empty() {
                                    if let Err(e) = ws_write.send(Message::Binary(bytes.into())).await {
                                        sink.emit_error(format!("Deepgram send error: {}", e));
                                        stop_flag.store(true, Ordering::Relaxed);
                                        break;
                                    }
                                    last_audio_sent = Instant::now();
                                }
                            }
                            None => break,
                        }
                    }
                    _ = keepalive_tick.tick() => {
                        if last_audio_sent.elapsed() >= Duration::from_secs(4) {
                            let _ = ws_write
                                .send(Message::Text(r#"{"type":"KeepAlive"}"#.into()))
                                .await;
                            last_audio_sent = Instant::now();
                        }
                    }
                }
            }

            // Drop async_rx and ensure stop_flag is set so bridge_handle never blocks or deadlocks
            drop(async_rx);
            stop_flag.store(true, Ordering::Relaxed);

            // Signal Deepgram to flush buffered transcripts cleanly before shutting down
            let _ = tokio::time::timeout(
                Duration::from_millis(400),
                ws_write.send(Message::Text(r#"{"type":"CloseStream"}"#.into())),
            )
            .await;
            let _ = tokio::time::timeout(Duration::from_millis(200), ws_write.flush()).await;
            let _ = tokio::time::timeout(Duration::from_millis(400), recv_task).await;
            let _ = tokio::time::timeout(Duration::from_millis(400), bridge_handle).await;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_valid_deepgram_listen_url() {
        let url = build_deepgram_ws_url("nova-3", "en-US");
        assert!(url.starts_with("wss://api.deepgram.com/v1/listen?"));
        assert!(url.contains("model=nova-3"));
        assert!(url.contains("language=en-US"));
        assert!(url.contains("encoding=linear16"));
        assert!(url.contains("sample_rate=16000"));
        assert!(url.contains("interim_results=true"));
    }

    #[test]
    fn parses_deepgram_interim_and_final_results() {
        let interim = r#"{"type":"Results","is_final":false,"speech_final":false,"channel":{"alternatives":[{"transcript":"what is bfs"}]}}"#;
        let final_msg = r#"{"type":"Results","is_final":true,"speech_final":true,"channel":{"alternatives":[{"transcript":"What is BFS?"}]}}"#;

        assert_eq!(
            parse_deepgram_message(interim).unwrap(),
            Some(TranscriptEvent {
                kind: "partial".to_string(),
                text: "what is bfs".to_string(),
                speech_final: false,
            })
        );
        assert_eq!(
            parse_deepgram_message(final_msg).unwrap(),
            Some(TranscriptEvent {
                kind: "final".to_string(),
                text: "What is BFS?".to_string(),
                speech_final: true,
            })
        );
    }

    #[test]
    fn ignores_deepgram_metadata_and_empty_transcripts() {
        let meta = r#"{"type":"Metadata","request_id":"abc"}"#;
        let empty = r#"{"type":"Results","is_final":true,"channel":{"alternatives":[{"transcript":"   "}]}}"#;
        let err = r#"{"type":"Error","description":"Invalid credentials"}"#;

        assert_eq!(parse_deepgram_message(meta).unwrap(), None);
        assert_eq!(parse_deepgram_message(empty).unwrap(), None);
        assert_eq!(
            parse_deepgram_message(err).unwrap_err(),
            "Invalid credentials"
        );
    }

    #[test]
    fn encodes_i16_pcm_samples_little_endian() {
        assert_eq!(samples_to_le_bytes(&[0x0102, -1]), vec![0x02, 0x01, 0xFF, 0xFF]);
    }

    #[test]
    fn detects_deepgram_utterance_end_signals() {
        assert!(is_deepgram_utterance_end(r#"{"type":"UtteranceEnd","channel":[0,1],"last_word_end":1.42}"#));
        assert!(is_deepgram_utterance_end(r#"{"type":"Results","speech_final":true,"is_final":true}"#));
        assert!(!is_deepgram_utterance_end(r#"{"type":"Results","speech_final":false,"is_final":true}"#));
        assert!(!is_deepgram_utterance_end(r#"{"type":"Metadata"}"#));
    }
}
