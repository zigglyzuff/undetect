use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::oneshot;

use crate::voice::setup::app_data_root;

pub const MAX_SESSION_SCREENSHOTS: usize = 25;

#[derive(Clone, Serialize)]
pub struct ScreenshotMeta {
    pub id: String,
    pub timestamp: String,
}

#[derive(Clone)]
pub struct ScreenshotEntry {
    pub id: String,
    pub timestamp: String,
    pub jpeg_data: Vec<u8>,
    pub thumb_data: Vec<u8>,
}

#[derive(Clone, Serialize)]
pub struct ServerConfig {
    pub port: u16,
    pub password: String,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PersistedServerConfig {
    pub port: u16,
    pub password: String,
}

impl Default for PersistedServerConfig {
    fn default() -> Self {
        Self {
            port: 9090,
            password: "1234".to_string(),
        }
    }
}

pub fn server_config_path() -> PathBuf {
    app_data_root().join("server_config.json")
}

pub fn load_server_config_from(path: &Path) -> PersistedServerConfig {
    let Ok(raw) = fs::read_to_string(path) else {
        return PersistedServerConfig::default();
    };
    let mut parsed = serde_json::from_str::<PersistedServerConfig>(&raw).unwrap_or_default();
    if parsed.port == 0 {
        parsed.port = 9090;
    }
    parsed
}

pub fn save_server_config_to(path: &Path, port: u16, password: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create server config dir: {e}"))?;
    }
    let payload = PersistedServerConfig {
        port: if port == 0 { 9090 } else { port },
        password: password.to_string(),
    };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Failed to serialize server config: {e}"))?;
    fs::write(path, json).map_err(|e| format!("Failed to write server config: {e}"))
}

pub struct AppState {
    pub screenshots: RwLock<VecDeque<ScreenshotEntry>>,
    pub config: RwLock<ServerConfig>,
    pub shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
    pub skip_taskbar: AtomicBool,
    pub content_protected: AtomicBool,
    pub always_on_top: AtomicBool,
    pub click_through: AtomicBool,
    pub window_opacity: AtomicU8,
    pub llm: Arc<crate::llm::LlmService>,
    pub voice: Arc<crate::voice::VoiceService>,
    pub telemetry: Arc<crate::telemetry::TelemetryService>,
    pub active_ui_patch: RwLock<Option<crate::ui_patch::ActiveUiPatch>>,
}

impl Default for AppState {
    fn default() -> Self {
        let saved_server = load_server_config_from(&server_config_path());
        Self {
            screenshots: RwLock::new(VecDeque::with_capacity(MAX_SESSION_SCREENSHOTS)),
            config: RwLock::new(ServerConfig {
                port: saved_server.port,
                password: saved_server.password,
                running: false,
            }),
            shutdown_tx: Mutex::new(None),
            skip_taskbar: AtomicBool::new(false),
            content_protected: AtomicBool::new(true),
            always_on_top: AtomicBool::new(true),
            click_through: AtomicBool::new(false),
            window_opacity: AtomicU8::new(100),
            llm: Arc::new(crate::llm::LlmService::new()),
            voice: Arc::new(crate::voice::VoiceService::new()),
            telemetry: Arc::new(crate::telemetry::TelemetryService::new()),
            active_ui_patch: RwLock::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_and_reloads_server_port_and_pin() {
        let mut temp_path = std::env::temp_dir();
        temp_path.push(format!("undetect_server_cfg_test_{}.json", std::process::id()));
        let _ = fs::remove_file(&temp_path);

        assert_eq!(load_server_config_from(&temp_path), PersistedServerConfig::default());

        save_server_config_to(&temp_path, 8787, "9988").unwrap();
        let loaded = load_server_config_from(&temp_path);
        assert_eq!(loaded.port, 8787);
        assert_eq!(loaded.password, "9988");

        let _ = fs::remove_file(&temp_path);
    }
}
