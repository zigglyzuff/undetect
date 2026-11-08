use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, RwLock};
use tokio::sync::oneshot;

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

pub struct AppState {
    pub screenshots: RwLock<VecDeque<ScreenshotEntry>>,
    pub config: RwLock<ServerConfig>,
    pub shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
    pub skip_taskbar: AtomicBool,
    pub content_protected: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            screenshots: RwLock::new(VecDeque::with_capacity(MAX_SESSION_SCREENSHOTS)),
            config: RwLock::new(ServerConfig {
                port: 9090,
                password: "1234".to_string(),
                running: false,
            }),
            shutdown_tx: Mutex::new(None),
            skip_taskbar: AtomicBool::new(false),
            content_protected: AtomicBool::new(true),
        }
    }
}
