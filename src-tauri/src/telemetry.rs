use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::state::AppState;
use crate::voice::setup::app_data_root;

pub const POSTHOG_CAPTURE_URL: &str = "https://us.i.posthog.com/i/v0/e/";
const DEFAULT_POSTHOG_API_KEY: &str = "phc_BCmsDSFYvWf8KKjCEXTHgG3VegvvHiKcvkHScD5g87Eb";
const HEARTBEAT_INTERVAL_SECS: u64 = 10 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TelemetryConfig {
    pub enabled: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone)]
pub struct TelemetrySnapshot {
    pub app_version: String,
    pub os: &'static str,
    pub arch: &'static str,
    pub env: &'static str,
    pub llm_provider: String,
    pub voice_provider: String,
    pub has_resume: bool,
    pub has_jd: bool,
    pub remote_server_running: bool,
}

/// Asynchronous, privacy-preserving PostHog telemetry service.
/// Never sends prompts, API keys, resumes, job descriptions, OCR text, screenshots, or raw machine IDs.
pub struct TelemetryService {
    distinct_id: String,
    api_key: String,
    endpoint: String,
    enabled: AtomicBool,
    storage_path: PathBuf,
    http_client: reqwest::Client,
}

impl TelemetryService {
    pub fn new() -> Self {
        let storage_path = app_data_root().join("telemetry_config.json");
        let cfg = Self::load_config(&storage_path);
        let api_key = option_env!("UNDETECT_POSTHOG_API_KEY")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(DEFAULT_POSTHOG_API_KEY)
            .to_string();

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        Self {
            distinct_id: Self::resolve_distinct_id(cfg!(debug_assertions)),
            api_key,
            endpoint: POSTHOG_CAPTURE_URL.to_string(),
            enabled: AtomicBool::new(cfg.enabled),
            storage_path,
            http_client,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<bool, String> {
        let cfg = TelemetryConfig { enabled };
        Self::save_config(&self.storage_path, &cfg)?;
        self.enabled.store(enabled, Ordering::Relaxed);
        Ok(enabled)
    }

    /// Fire-and-forget event dispatch on the shared Tokio runtime without blocking UI or IPC.
    pub fn spawn_capture(
        state: &Arc<AppState>,
        event: &'static str,
        extra_props: Option<serde_json::Value>,
    ) {
        if !state.telemetry.is_enabled() {
            return;
        }
        let state_clone = Arc::clone(state);
        crate::tokio_rt().spawn(async move {
            state_clone
                .telemetry
                .capture_from_state(&state_clone, event, extra_props)
                .await;
        });
    }

    /// Starts initial launch event and periodic 10-minute presence heartbeats.
    pub fn start_heartbeat_loop(state: Arc<AppState>) {
        crate::tokio_rt().spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            state
                .telemetry
                .capture_from_state(&state, "app_opened", None)
                .await;

            let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
            interval.tick().await; // skip immediate tick right after app_opened
            loop {
                interval.tick().await;
                if state.telemetry.is_enabled() {
                    state
                        .telemetry
                        .capture_from_state(&state, "app_heartbeat", None)
                        .await;
                }
            }
        });
    }

    pub async fn capture_from_state(
        &self,
        state: &AppState,
        event: &str,
        extra_props: Option<serde_json::Value>,
    ) {
        if !self.is_enabled() || self.api_key.trim().is_empty() {
            return;
        }
        let snapshot = Self::snapshot_from_state(state);
        let payload = Self::build_event_payload(
            &self.api_key,
            &self.distinct_id,
            event,
            &snapshot,
            extra_props.as_ref(),
        );
        let _ = self
            .http_client
            .post(&self.endpoint)
            .json(&payload)
            .send()
            .await;
    }

    fn snapshot_from_state(state: &AppState) -> TelemetrySnapshot {
        let llm = state.llm.get_public_config();
        let voice = state.voice.get_status();
        let remote_server_running = state.config.read().map(|c| c.running).unwrap_or(false);

        TelemetrySnapshot {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            env: if cfg!(debug_assertions) { "dev" } else { "prod" },
            llm_provider: llm.active_provider,
            voice_provider: voice.provider,
            has_resume: llm.has_resume,
            has_jd: llm.has_jd,
            remote_server_running,
        }
    }

    pub fn build_event_payload(
        api_key: &str,
        distinct_id: &str,
        event: &str,
        snapshot: &TelemetrySnapshot,
        extra_props: Option<&serde_json::Value>,
    ) -> serde_json::Value {
        let mut person_set = serde_json::Map::new();
        person_set.insert("app_version".to_string(), serde_json::json!(snapshot.app_version));
        person_set.insert("os".to_string(), serde_json::json!(snapshot.os));
        person_set.insert("arch".to_string(), serde_json::json!(snapshot.arch));
        person_set.insert("last_env".to_string(), serde_json::json!(snapshot.env));
        person_set.insert("llm_provider".to_string(), serde_json::json!(snapshot.llm_provider));
        person_set.insert("voice_provider".to_string(), serde_json::json!(snapshot.voice_provider));
        person_set.insert("has_resume".to_string(), serde_json::json!(snapshot.has_resume));
        person_set.insert("has_jd".to_string(), serde_json::json!(snapshot.has_jd));
        person_set.insert(
            "remote_server_running".to_string(),
            serde_json::json!(snapshot.remote_server_running),
        );

        let mut properties = serde_json::Map::new();
        properties.insert("app_version".to_string(), serde_json::json!(snapshot.app_version));
        properties.insert("os".to_string(), serde_json::json!(snapshot.os));
        properties.insert("arch".to_string(), serde_json::json!(snapshot.arch));
        properties.insert("env".to_string(), serde_json::json!(snapshot.env));
        properties.insert("llm_provider".to_string(), serde_json::json!(snapshot.llm_provider));
        properties.insert("voice_provider".to_string(), serde_json::json!(snapshot.voice_provider));
        properties.insert("has_resume".to_string(), serde_json::json!(snapshot.has_resume));
        properties.insert("has_jd".to_string(), serde_json::json!(snapshot.has_jd));
        properties.insert(
            "remote_server_running".to_string(),
            serde_json::json!(snapshot.remote_server_running),
        );
        properties.insert("$lib".to_string(), serde_json::json!("undetect-rust"));
        properties.insert("$lib_version".to_string(), serde_json::json!(snapshot.app_version));
        properties.insert("$set".to_string(), serde_json::Value::Object(person_set));

        if let Some(serde_json::Value::Object(extra)) = extra_props {
            for (k, v) in extra {
                if k != "$set" && k != "$set_once" {
                    properties.insert(k.clone(), v.clone());
                }
            }
        }

        serde_json::json!({
            "api_key": api_key,
            "event": event,
            "distinct_id": distinct_id,
            "properties": serde_json::Value::Object(properties),
        })
    }

    pub fn hash_machine_guid(seed: &str, is_dev: bool) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"undetect:v1:posthog:");
        hasher.update(seed.trim().as_bytes());
        let digest = hasher.finalize();
        let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
        if is_dev {
            format!("ud_dev_{hex}")
        } else {
            format!("ud_{hex}")
        }
    }

    fn resolve_distinct_id(is_dev: bool) -> String {
        if let Some(guid) = read_windows_machine_guid() {
            return Self::hash_machine_guid(&guid, is_dev);
        }
        let fallback_path = app_data_root().join("telemetry_id");
        if let Ok(raw) = fs::read_to_string(&fallback_path) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return Self::hash_machine_guid(trimmed, is_dev);
            }
        }
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let generated = format!("fallback-{}-{}", std::process::id(), now_nanos);
        if let Some(parent) = fallback_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&fallback_path, &generated);
        Self::hash_machine_guid(&generated, is_dev)
    }

    fn load_config(path: &Path) -> TelemetryConfig {
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<TelemetryConfig>(&raw).ok())
            .unwrap_or_default()
    }

    fn save_config(path: &Path, cfg: &TelemetryConfig) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create telemetry config directory: {e}"))?;
        }
        let json = serde_json::to_string_pretty(cfg)
            .map_err(|e| format!("Failed to serialize telemetry config: {e}"))?;
        fs::write(path, json).map_err(|e| format!("Failed to save telemetry config: {e}"))
    }
}

impl Default for TelemetryService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
fn read_windows_machine_guid() -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};

    let subkey: Vec<u16> = "SOFTWARE\\Microsoft\\Cryptography\0".encode_utf16().collect();
    let value: Vec<u16> = "MachineGuid\0".encode_utf16().collect();
    let mut buf = [0u16; 128];
    let mut byte_len = std::mem::size_of_val(&buf) as u32;

    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut byte_len),
        )
    };

    if status.is_ok() && byte_len >= 2 {
        let wchar_len = (byte_len as usize / 2).min(buf.len());
        let slice = &buf[..wchar_len];
        let nul_pos = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
        let guid = String::from_utf16_lossy(&slice[..nul_pos]).trim().to_string();
        if !guid.is_empty() {
            return Some(guid);
        }
    }
    None
}

#[cfg(not(windows))]
fn read_windows_machine_guid() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_machine_guid_deterministically_and_separates_dev_prod() {
        let prod_a = TelemetryService::hash_machine_guid("abc-123", false);
        let prod_b = TelemetryService::hash_machine_guid(" abc-123 ", false);
        let dev_a = TelemetryService::hash_machine_guid("abc-123", true);

        assert_eq!(prod_a, prod_b);
        assert!(prod_a.starts_with("ud_"));
        assert!(!prod_a.contains("abc-123"));
        assert!(dev_a.starts_with("ud_dev_"));
        assert_ne!(prod_a, dev_a);
    }

    #[test]
    fn builds_posthog_payload_without_sensitive_fields_and_with_person_set() {
        let snapshot = TelemetrySnapshot {
            app_version: "2.1.0".to_string(),
            os: "windows",
            arch: "x86_64",
            env: "prod",
            llm_provider: "groq".to_string(),
            voice_provider: "deepgram".to_string(),
            has_resume: true,
            has_jd: false,
            remote_server_running: true,
        };
        let extra = serde_json::json!({
            "model": "openai/gpt-oss-120b",
            "with_screen": true,
        });

        let payload = TelemetryService::build_event_payload(
            "phc_test_token",
            "ud_1234",
            "chat_message_sent",
            &snapshot,
            Some(&extra),
        );

        assert_eq!(payload["api_key"], "phc_test_token");
        assert_eq!(payload["event"], "chat_message_sent");
        assert_eq!(payload["distinct_id"], "ud_1234");
        assert_eq!(payload["properties"]["llm_provider"], "groq");
        assert_eq!(payload["properties"]["voice_provider"], "deepgram");
        assert_eq!(payload["properties"]["model"], "openai/gpt-oss-120b");
        assert_eq!(payload["properties"]["with_screen"], true);
        assert_eq!(payload["properties"]["$set"]["app_version"], "2.1.0");
        assert_eq!(payload["properties"]["$set"]["has_resume"], true);
    }

    #[test]
    fn persists_telemetry_opt_out_flag() {
        let mut temp_path = std::env::temp_dir();
        temp_path.push(format!("undetect_telemetry_test_{}.json", std::process::id()));
        let _ = fs::remove_file(&temp_path);

        let default_cfg = TelemetryService::load_config(&temp_path);
        assert!(default_cfg.enabled);

        TelemetryService::save_config(&temp_path, &TelemetryConfig { enabled: false }).unwrap();
        let reloaded = TelemetryService::load_config(&temp_path);
        assert!(!reloaded.enabled);

        let _ = fs::remove_file(&temp_path);
    }
}
