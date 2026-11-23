use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::sync::mpsc;

pub const VOSK_SAMPLE_RATE: u32 = 16_000;

#[derive(Clone, Serialize)]
pub struct AudioDeviceDto {
    pub name: String,
    pub is_default: bool,
    pub kind: String,
}

fn device_name(device: &cpal::Device) -> Option<String> {
    device.description().ok().map(|d| d.name().to_string())
}

pub fn list_audio_devices() -> Result<Vec<AudioDeviceDto>, String> {
    let host = cpal::default_host();
    let mut list = Vec::new();

    let default_in_name = host
        .default_input_device()
        .and_then(|d| device_name(&d));
    if let Ok(inputs) = host.input_devices() {
        for dev in inputs {
            if let Some(name) = device_name(&dev) {
                list.push(AudioDeviceDto {
                    is_default: default_in_name.as_deref() == Some(name.as_str()),
                    name,
                    kind: "mic".to_string(),
                });
            }
        }
    }

    let default_out_name = host
        .default_output_device()
        .and_then(|d| device_name(&d));
    if let Ok(outputs) = host.output_devices() {
        for dev in outputs {
            if let Some(name) = device_name(&dev) {
                list.push(AudioDeviceDto {
                    is_default: default_out_name.as_deref() == Some(name.as_str()),
                    name,
                    kind: "system".to_string(),
                });
            }
        }
    }

    Ok(list)
}

/// Stateful mono `f32 -> 16 kHz i16` resampler with fractional phase continuity across chunks.
pub struct Resampler16k {
    src_rate: u32,
    step: f64,
    pos: f64,
    prev_tail: Vec<f32>,
}

impl Resampler16k {
    pub fn new(src_rate: u32) -> Self {
        let rate = src_rate.max(8_000);
        Self {
            src_rate: rate,
            step: (rate as f64) / (VOSK_SAMPLE_RATE as f64),
            pos: 0.0,
            prev_tail: Vec::new(),
        }
    }

    /// Downmixes interleaved multi-channel audio (`f32`) to mono and resamples to 16 kHz `i16`.
    pub fn push_interleaved(&mut self, interleaved_mono_or_multi: &[f32], channels: usize) -> Vec<i16> {
        let ch = channels.max(1);
        let frame_count = interleaved_mono_or_multi.len() / ch;
        if frame_count == 0 {
            return Vec::new();
        }

        let mut mono = Vec::with_capacity(self.prev_tail.len() + frame_count);
        mono.extend_from_slice(&self.prev_tail);

        let scale = 1.0 / (ch as f32);
        for frame in interleaved_mono_or_multi.chunks_exact(ch) {
            let sum: f32 = frame.iter().copied().sum();
            mono.push(sum * scale);
        }

        if self.src_rate == VOSK_SAMPLE_RATE {
            self.prev_tail.clear();
            return mono
                .into_iter()
                .map(|s| (s.clamp(-1.0, 1.0) * 32767.0).round() as i16)
                .collect();
        }

        let mut out = Vec::with_capacity(((frame_count as f64) / self.step).ceil() as usize + 2);
        let last_usable = mono.len().saturating_sub(1);
        let half_window = ((self.step * 0.5).floor() as usize).max(1);

        while (self.pos as usize) < last_usable {
            let idx = self.pos as usize;
            let frac = (self.pos - idx as f64) as f32;

            let sample = if self.step > 1.5 {
                // Light box low-pass around sample position before interpolation to reduce aliasing
                let lo = idx.saturating_sub(half_window / 2);
                let hi = (idx + half_window / 2 + 1).min(mono.len());
                let avg0: f32 = mono[lo..hi].iter().sum::<f32>() / ((hi - lo) as f32);

                let idx1 = (idx + 1).min(last_usable);
                let lo1 = idx1.saturating_sub(half_window / 2);
                let hi1 = (idx1 + half_window / 2 + 1).min(mono.len());
                let avg1: f32 = mono[lo1..hi1].iter().sum::<f32>() / ((hi1 - lo1) as f32);

                avg0 + (avg1 - avg0) * frac
            } else {
                let s0 = mono[idx];
                let s1 = mono[idx + 1];
                s0 + (s1 - s0) * frac
            };

            out.push((sample.clamp(-1.0, 1.0) * 32767.0).round() as i16);
            self.pos += self.step;
        }

        let consumed = (self.pos.floor() as usize).min(last_usable);
        self.pos -= consumed as f64;
        self.prev_tail.clear();
        self.prev_tail.extend_from_slice(&mono[consumed..]);

        out
    }
}

fn find_device(source: &str, preferred_name: Option<&str>) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    let host = cpal::default_host();

    if source == "system" {
        let device = if let Some(target) = preferred_name.filter(|s| !s.is_empty()) {
            host.output_devices()
                .map_err(|e| format!("Failed to list output devices: {}", e))?
                .find(|d| device_name(d).as_deref() == Some(target))
                .or_else(|| host.default_output_device())
        } else {
            host.default_output_device()
        }
        .ok_or_else(|| "No system audio output device available for loopback".to_string())?;

        let cfg = device
            .default_output_config()
            .map_err(|e| format!("Failed to query output device config: {}", e))?;
        Ok((device, cfg))
    } else {
        let device = if let Some(target) = preferred_name.filter(|s| !s.is_empty()) {
            host.input_devices()
                .map_err(|e| format!("Failed to list input devices: {}", e))?
                .find(|d| device_name(d).as_deref() == Some(target))
                .or_else(|| host.default_input_device())
        } else {
            host.default_input_device()
        }
        .ok_or_else(|| "No microphone input device found".to_string())?;

        let cfg = device
            .default_input_config()
            .map_err(|e| format!("Failed to query microphone config: {}", e))?;
        Ok((device, cfg))
    }
}

pub fn is_fatal_stream_error(kind: cpal::ErrorKind) -> bool {
    !matches!(
        kind,
        cpal::ErrorKind::Xrun | cpal::ErrorKind::DeviceChanged | cpal::ErrorKind::RealtimeDenied
    )
}

fn build_typed_stream<S, F>(
    device: &cpal::Device,
    stream_config: cpal::StreamConfig,
    tx: mpsc::Sender<Vec<f32>>,
    err_tx: mpsc::Sender<String>,
    to_f32: F,
) -> Result<cpal::Stream, String>
where
    S: cpal::SizedSample,
    F: Fn(S) -> f32 + Send + 'static,
{
    device
        .build_input_stream(
            stream_config,
            move |data: &[S], _: &cpal::InputCallbackInfo| {
                if !data.is_empty() {
                    let samples: Vec<f32> = data.iter().copied().map(&to_f32).collect();
                    let _ = tx.send(samples);
                }
            },
            move |err: cpal::Error| {
                if is_fatal_stream_error(err.kind()) {
                    let _ = err_tx.send(format!("Audio stream error: {}", err));
                }
            },
            None,
        )
        .map_err(|e| format!("Failed to start audio stream: {}", e))
}

fn build_silent_keepalive_stream(
    device: &cpal::Device,
    stream_cfg: cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
) -> Option<cpal::Stream> {
    let res = match sample_format {
        cpal::SampleFormat::F32 => device.build_output_stream(
            stream_cfg,
            |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                data.fill(0.0);
            },
            |err| {
                eprintln!("[Undetect Voice] Keepalive stream warning: {}", err);
            },
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            stream_cfg,
            |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                data.fill(0);
            },
            |err| {
                eprintln!("[Undetect Voice] Keepalive stream warning: {}", err);
            },
            None,
        ),
        cpal::SampleFormat::U16 => device.build_output_stream(
            stream_cfg,
            |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                data.fill(32768);
            },
            |err| {
                eprintln!("[Undetect Voice] Keepalive stream warning: {}", err);
            },
            None,
        ),
        _ => return None,
    };

    match res {
        Ok(stream) => {
            if let Err(e) = stream.play() {
                eprintln!("[Undetect Voice] Failed to start keepalive stream: {}", e);
                None
            } else {
                Some(stream)
            }
        }
        Err(e) => {
            eprintln!("[Undetect Voice] Could not build keepalive stream: {}", e);
            None
        }
    }
}

pub struct RawAudioStream {
    pub stream: cpal::Stream,
    pub keepalive: Option<cpal::Stream>,
    pub channels: usize,
    pub sample_rate: u32,
}

pub fn open_raw_input_stream(
    source: &str,
    device_name: Option<&str>,
    tx: mpsc::Sender<Vec<f32>>,
    err_tx: mpsc::Sender<String>,
) -> Result<RawAudioStream, String> {
    let (device, supported_cfg) = find_device(source, device_name)?;
    let sample_format = supported_cfg.sample_format();
    let src_rate = supported_cfg.sample_rate();
    let channels = supported_cfg.channels() as usize;
    let stream_cfg: cpal::StreamConfig = supported_cfg.config();

    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            build_typed_stream::<f32, _>(&device, stream_cfg.clone(), tx, err_tx, |s| s)?
        }
        cpal::SampleFormat::I16 => build_typed_stream::<i16, _>(
            &device,
            stream_cfg.clone(),
            tx,
            err_tx,
            |s| (s as f32) / 32768.0,
        )?,
        cpal::SampleFormat::U16 => build_typed_stream::<u16, _>(
            &device,
            stream_cfg.clone(),
            tx,
            err_tx,
            |s| ((s as f32) - 32768.0) / 32768.0,
        )?,
        other => return Err(format!("Unsupported audio sample format: {:?}", other)),
    };

    let keepalive = if source == "system" {
        build_silent_keepalive_stream(&device, stream_cfg, sample_format)
    } else {
        None
    };

    Ok(RawAudioStream {
        stream,
        keepalive,
        channels,
        sample_rate: src_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treats_xrun_and_device_changed_as_transient() {
        assert!(!is_fatal_stream_error(cpal::ErrorKind::Xrun));
        assert!(!is_fatal_stream_error(cpal::ErrorKind::DeviceChanged));
        assert!(!is_fatal_stream_error(cpal::ErrorKind::RealtimeDenied));
        assert!(is_fatal_stream_error(cpal::ErrorKind::DeviceNotAvailable));
        assert!(is_fatal_stream_error(cpal::ErrorKind::StreamInvalidated));
    }

    #[test]
    fn resamples_48khz_stereo_to_16khz_mono() {
        let mut resampler = Resampler16k::new(48_000);
        // 4800 stereo frames (9600 f32 values) = 100 ms -> ~1600 samples at 16 kHz
        let stereo_48k = vec![0.5_f32; 4800 * 2];
        let out = resampler.push_interleaved(&stereo_48k, 2);
        assert!((1598..=1601).contains(&out.len()), "got len {}", out.len());
        assert!(out.iter().all(|&s| (s - 16384).abs() <= 4));
    }

    #[test]
    fn passes_through_16khz_mono_cleanly() {
        let mut resampler = Resampler16k::new(16_000);
        let input = vec![0.25_f32, -0.5_f32, 0.0_f32, 1.0_f32];
        let out = resampler.push_interleaved(&input, 1);
        assert_eq!(out, vec![8192, -16384, 0, 32767]);
    }
}
