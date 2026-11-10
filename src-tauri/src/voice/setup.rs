use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

pub const VOSK_WIN64_ZIP_URL: &str =
    "https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-win64-0.3.45.zip";
pub const VOSK_MODEL_SMALL_EN_URL: &str =
    "https://alphacephei.com/kaldi/models/vosk-model-small-en-us-0.15.zip";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceConfig {
    /// Speech recognition backend: "vosk" (local offline) or "deepgram" (cloud streaming API).
    pub provider: String,
    /// Audio source: "mic" (microphone) or "system" (Windows WASAPI loopback output capture).
    pub source: String,
    pub device_name: Option<String>,
    pub custom_runtime_dir: Option<String>,
    pub custom_model_dir: Option<String>,
    pub deepgram_api_key: String,
    pub deepgram_model: String,
    pub deepgram_language: String,
    #[serde(default)]
    pub auto_send: bool,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            provider: "deepgram".to_string(),
            source: "system".to_string(),
            device_name: None,
            custom_runtime_dir: None,
            custom_model_dir: None,
            deepgram_api_key: String::new(),
            deepgram_model: "nova-3".to_string(),
            deepgram_language: "en-US".to_string(),
            auto_send: false,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct VoiceStatus {
    pub provider: String,
    pub runtime_installed: bool,
    pub model_installed: bool,
    pub has_deepgram_key: bool,
    pub deepgram_key_masked: String,
    pub deepgram_model: String,
    pub deepgram_language: String,
    pub auto_send: bool,
    pub ready: bool,
    pub listening: bool,
    pub installing: bool,
    pub source: String,
    pub device_name: Option<String>,
    pub runtime_path: Option<String>,
    pub model_path: Option<String>,
    pub custom_runtime_dir: Option<String>,
    pub custom_model_dir: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct SetupProgressEvent {
    pub stage: String,
    pub percent: u8,
    pub message: String,
}

pub fn app_data_root() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local).join("Undetect")
    } else if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        PathBuf::from(home).join(".undetect")
    } else {
        std::env::temp_dir().join("Undetect")
    }
}

pub fn voice_config_path() -> PathBuf {
    app_data_root().join("voice_config.json")
}

pub fn default_runtime_dir() -> PathBuf {
    app_data_root().join("vosk").join("bin")
}

pub fn default_models_dir() -> PathBuf {
    app_data_root().join("vosk").join("models")
}

pub fn load_voice_config() -> VoiceConfig {
    let path = voice_config_path();
    if let Ok(raw) = fs::read_to_string(&path)
        && let Ok(mut cfg) = serde_json::from_str::<VoiceConfig>(&raw)
    {
        cfg.provider = normalize_provider(&cfg.provider);
        cfg.source = normalize_source(&cfg.source);
        if cfg.deepgram_model.trim().is_empty() {
            cfg.deepgram_model = "nova-3".to_string();
        }
        if cfg.deepgram_language.trim().is_empty() {
            cfg.deepgram_language = "en-US".to_string();
        }
        return cfg;
    }
    VoiceConfig::default()
}

pub fn save_voice_config(cfg: &VoiceConfig) -> Result<(), String> {
    let path = voice_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| format!("Failed to serialize voice config: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write voice config: {}", e))
}

pub fn normalize_provider(raw: &str) -> String {
    if raw.trim().eq_ignore_ascii_case("vosk") {
        "vosk".to_string()
    } else {
        "deepgram".to_string()
    }
}

pub fn mask_api_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 8 {
        return "••••••••".to_string();
    }
    let prefix: String = chars[..3].iter().collect();
    let suffix: String = chars[chars.len() - 4..].iter().collect();
    format!("{}••••{}", prefix, suffix)
}

pub fn normalize_source(raw: &str) -> String {
    if raw.trim().eq_ignore_ascii_case("mic") {
        "mic".to_string()
    } else {
        "system".to_string()
    }
}

pub fn resolve_runtime_dll(custom_dir: Option<&str>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    let lib_filename = if cfg!(windows) {
        "libvosk.dll"
    } else if cfg!(target_os = "macos") {
        "libvosk.dylib"
    } else {
        "libvosk.so"
    };

    if let Some(custom) = custom_dir.map(str::trim).filter(|s| !s.is_empty()) {
        let p = PathBuf::from(custom);
        candidates.push(p.clone());
        candidates.push(p.join(lib_filename));
    }

    candidates.push(default_runtime_dir().join(lib_filename));

    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join(lib_filename));
        candidates.push(dir.join("vosk").join(lib_filename));
    }

    candidates.into_iter().find(|p| p.is_file())
}

pub fn is_valid_vosk_model_dir(dir: &Path) -> bool {
    dir.is_dir()
        && (dir.join("am").join("final.mdl").is_file()
            || dir.join("conf").join("model.conf").is_file()
            || dir.join("final.mdl").is_file())
}

pub fn resolve_model_dir(custom_dir: Option<&str>) -> Option<PathBuf> {
    if let Some(custom) = custom_dir.map(str::trim).filter(|s| !s.is_empty()) {
        let p = PathBuf::from(custom);
        if is_valid_vosk_model_dir(&p) {
            return Some(p);
        }
    }

    let preferred = default_models_dir().join("vosk-model-small-en-us-0.15");
    if is_valid_vosk_model_dir(&preferred) {
        return Some(preferred);
    }

    if let Ok(entries) = fs::read_dir(default_models_dir()) {
        for entry in entries.flatten() {
            let p = entry.path();
            if is_valid_vosk_model_dir(&p) {
                return Some(p);
            }
        }
    }

    None
}

fn strip_archive_root(rel: &Path, strip_top: bool) -> Option<PathBuf> {
    if !strip_top {
        return Some(rel.to_path_buf());
    }
    let mut comps = rel.components();
    let _first = comps.next()?;
    let remainder: PathBuf = comps.collect();
    if remainder.as_os_str().is_empty() {
        None
    } else {
        Some(remainder)
    }
}

fn emit_progress(app: &AppHandle, stage: &str, percent: u8, message: &str) {
    let _ = app.emit(
        "vosk://setup-progress",
        SetupProgressEvent {
            stage: stage.to_string(),
            percent: percent.min(100),
            message: message.to_string(),
        },
    );
}

struct ZipDownloadSpec<'a> {
    url: &'a str,
    stage: &'a str,
    label: &'a str,
    base_pct: u8,
    span_pct: u8,
    strip_top_folder: bool,
}

async fn download_and_extract_zip(
    app: &AppHandle,
    dest_dir: &Path,
    spec: ZipDownloadSpec<'_>,
) -> Result<(), String> {
    let url = spec.url;
    let stage = spec.stage;
    let label = spec.label;
    let base_pct = spec.base_pct;
    let span_pct = spec.span_pct;
    let strip_top_folder = spec.strip_top_folder;

    emit_progress(app, stage, base_pct, &format!("Downloading {}...", label));

    let client = reqwest::Client::builder()
        .user_agent("Undetect-App")
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to download {}: {}", label, e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Failed to download {} (HTTP {})",
            label,
            resp.status()
        ));
    }

    let total_len = resp.content_length().unwrap_or(0);
    let tmp_path = std::env::temp_dir().join(format!("undetect_vosk_{}.zip", stage));
    let mut tmp_file =
        File::create(&tmp_path).map_err(|e| format!("Failed to create temp zip: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut last_reported_pct = base_pct;

    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("Stream error while downloading {}: {}", label, e))?
    {
        tmp_file
            .write_all(&chunk)
            .map_err(|e| format!("Disk write error: {}", e))?;
        downloaded += chunk.len() as u64;

        if total_len > 0 {
            let ratio = (downloaded as f64 / total_len as f64).min(1.0);
            let pct = base_pct + ((ratio * (span_pct as f64) * 0.85) as u8);
            if pct >= last_reported_pct + 2 {
                last_reported_pct = pct;
                let mb = downloaded as f64 / (1024.0 * 1024.0);
                let total_mb = total_len as f64 / (1024.0 * 1024.0);
                emit_progress(
                    app,
                    stage,
                    pct,
                    &format!("Downloading {} ({:.1} / {:.1} MB)...", label, mb, total_mb),
                );
            }
        }
    }
    tmp_file
        .flush()
        .map_err(|e| format!("Failed to flush archive: {}", e))?;
    drop(tmp_file);

    emit_progress(
        app,
        stage,
        base_pct + ((span_pct as f64 * 0.9) as u8),
        &format!("Extracting {}...", label),
    );

    fs::create_dir_all(dest_dir)
        .map_err(|e| format!("Failed to create destination {:?}: {}", dest_dir, e))?;

    let archive_file =
        File::open(&tmp_path).map_err(|e| format!("Failed to open downloaded archive: {}", e))?;
    let mut archive = zip::ZipArchive::new(archive_file)
        .map_err(|e| format!("Corrupt zip archive for {}: {}", label, e))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Zip read error: {}", e))?;
        let Some(rel_path) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        let Some(target_rel) = strip_archive_root(&rel_path, strip_top_folder) else {
            continue;
        };
        let out_path = dest_dir.join(target_rel);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("Failed to create dir {:?}: {}", out_path, e))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent {:?}: {}", parent, e))?;
            }
            let mut out_file = File::create(&out_path)
                .map_err(|e| format!("Failed to write extracted file {:?}: {}", out_path, e))?;
            std::io::copy(&mut entry, &mut out_file)
                .map_err(|e| format!("Failed to copy extracted bytes: {}", e))?;
        }
    }

    let _ = fs::remove_file(&tmp_path);
    Ok(())
}

pub async fn install_default_vosk(app: &AppHandle, cfg: &VoiceConfig) -> Result<(), String> {
    if resolve_runtime_dll(cfg.custom_runtime_dir.as_deref()).is_none() {
        #[cfg(not(windows))]
        {
            return Err("Automatic runtime download is only bundled for Windows. Place libvosk in custom runtime path.".to_string());
        }
        #[cfg(windows)]
        {
            let bin_dir = default_runtime_dir();
            download_and_extract_zip(
                app,
                &bin_dir,
                ZipDownloadSpec {
                    url: VOSK_WIN64_ZIP_URL,
                    stage: "runtime",
                    label: "Vosk Windows runtime (~18 MB)",
                    base_pct: 5,
                    span_pct: 35,
                    strip_top_folder: true,
                },
            )
            .await?;
        }
    }

    if resolve_model_dir(cfg.custom_model_dir.as_deref()).is_none() {
        let models_dir = default_models_dir();
        download_and_extract_zip(
            app,
            &models_dir,
            ZipDownloadSpec {
                url: VOSK_MODEL_SMALL_EN_URL,
                stage: "model",
                label: "English speech model (~40 MB)",
                base_pct: 45,
                span_pct: 52,
                strip_top_folder: false,
            },
        )
        .await?;
    }

    emit_progress(app, "done", 100, "Offline voice engine ready.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_archive_root_removes_top_directory() {
        assert_eq!(
            strip_archive_root(Path::new("vosk-win64-0.3.45/libvosk.dll"), true),
            Some(PathBuf::from("libvosk.dll"))
        );
        assert_eq!(strip_archive_root(Path::new("vosk-win64-0.3.45"), true), None);
        assert_eq!(
            strip_archive_root(Path::new("vosk-model-small-en-us-0.15/am/final.mdl"), false),
            Some(PathBuf::from("vosk-model-small-en-us-0.15/am/final.mdl"))
        );
    }

    #[test]
    fn normalizes_audio_source_safely() {
        assert_eq!(normalize_source("system"), "system");
        assert_eq!(normalize_source("SYSTEM"), "system");
        assert_eq!(normalize_source("mic"), "mic");
        assert_eq!(normalize_source("MIC"), "mic");
        assert_eq!(normalize_source("anything_else"), "system");
    }

    #[test]
    fn default_voice_provider_is_deepgram() {
        let cfg = VoiceConfig::default();
        assert_eq!(cfg.provider, "deepgram");
        assert_eq!(cfg.source, "system");
        assert_eq!(normalize_provider(""), "deepgram");
        assert_eq!(normalize_provider("vosk"), "vosk");
        assert_eq!(normalize_provider("VOSK"), "vosk");
        assert_eq!(normalize_provider("deepgram"), "deepgram");
    }
}
