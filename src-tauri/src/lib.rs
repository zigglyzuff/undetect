mod capture;
mod server;
mod state;
mod win_util;

use base64::Engine;
use qrcode::QrCode;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::runtime::Runtime;

use crate::capture::capture_primary_screen;
use crate::state::{AppState, ScreenshotMeta};

pub fn tokio_rt() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to initialize Tokio runtime")
    })
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub running: bool,
    pub port: u16,
    pub password: String,
    pub local_urls: Vec<String>,
    pub screenshot_count: usize,
    pub skip_taskbar: bool,
    pub content_protected: bool,
    pub app_version: String,
}

fn get_local_urls(port: u16, pass: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let pass_param = if !pass.is_empty() {
        format!("?pass={}", pass)
    } else {
        "".to_string()
    };

    if let Ok(ip) = local_ip_address::local_ip() {
        let u = format!("http://{}:{}/{}", ip, port, pass_param);
        urls.push(u);
    }

    if let Ok(interfaces) = local_ip_address::list_afinet_netifas() {
        for (_name, ip) in interfaces {
            if ip.is_ipv4() && !ip.is_loopback() {
                let u = format!("http://{}:{}/{}", ip, port, pass_param);
                if !urls.contains(&u) {
                    urls.push(u);
                }
            }
        }
    }

    if urls.is_empty() {
        urls.push(format!("http://127.0.0.1:{}/{}", port, pass_param));
    }
    urls
}

fn build_status(state: &AppState, app_version: &str) -> StatusResponse {
    let cfg = state.config.read().unwrap().clone();
    let urls = if cfg.running {
        get_local_urls(cfg.port, &cfg.password)
    } else {
        Vec::new()
    };
    let count = state.screenshots.read().map(|l| l.len()).unwrap_or(0);

    StatusResponse {
        running: cfg.running,
        port: cfg.port,
        password: cfg.password,
        local_urls: urls,
        screenshot_count: count,
        skip_taskbar: state.skip_taskbar.load(Ordering::Relaxed),
        content_protected: state.content_protected.load(Ordering::Relaxed),
        app_version: app_version.to_string(),
    }
}

#[tauri::command]
async fn get_status(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<StatusResponse, String> {
    let ver = app.package_info().version.to_string();
    Ok(build_status(&state, &ver))
}

#[tauri::command]
async fn start_server(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    port: u16,
    password: String,
) -> Result<StatusResponse, String> {
    // Stop any existing server first
    {
        let mut tx_lock = state.shutdown_tx.lock().unwrap();
        if let Some(tx) = tx_lock.take() {
            let _ = tx.send(());
        }
    }

    {
        let mut cfg = state.config.write().unwrap();
        cfg.port = port;
        cfg.password = password;
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    {
        let mut tx_lock = state.shutdown_tx.lock().unwrap();
        *tx_lock = Some(shutdown_tx);
    }

    let state_clone = Arc::clone(&state);
    tokio_rt().spawn(async move {
        server::start_http_server(state_clone, port, shutdown_rx).await;
    });

    // Small delay for listener to bind
    std::thread::sleep(std::time::Duration::from_millis(150));

    let ver = app.package_info().version.to_string();
    Ok(build_status(&state, &ver))
}

#[tauri::command]
async fn stop_server(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<StatusResponse, String> {
    {
        let mut tx_lock = state.shutdown_tx.lock().unwrap();
        if let Some(tx) = tx_lock.take() {
            let _ = tx.send(());
        }
    }

    if let Ok(mut cfg) = state.config.write() {
        cfg.running = false;
    }

    let ver = app.package_info().version.to_string();
    Ok(build_status(&state, &ver))
}

#[tauri::command]
async fn capture_now(state: tauri::State<'_, Arc<AppState>>) -> Result<ScreenshotMeta, String> {
    capture_primary_screen(&state)
}

#[tauri::command]
async fn clear_session(state: tauri::State<'_, Arc<AppState>>) -> Result<bool, String> {
    let mut list = state.screenshots.write().map_err(|e| e.to_string())?;
    list.clear();
    Ok(true)
}

#[tauri::command]
async fn toggle_taskbar(
    window: WebviewWindow,
    state: tauri::State<'_, Arc<AppState>>,
    hide: bool,
) -> Result<bool, String> {
    window
        .set_skip_taskbar(hide)
        .map_err(|e| format!("Failed to set skip taskbar: {}", e))?;
    state.skip_taskbar.store(hide, Ordering::Relaxed);
    Ok(hide)
}

#[tauri::command]
async fn toggle_content_protection(
    window: WebviewWindow,
    state: tauri::State<'_, Arc<AppState>>,
    protect: bool,
) -> Result<bool, String> {
    win_util::set_content_protection(&window, protect)?;
    state.content_protected.store(protect, Ordering::Relaxed);
    Ok(protect)
}

#[tauri::command]
fn generate_qr(text: String) -> Result<String, String> {
    let code = QrCode::new(text.as_bytes()).map_err(|e| format!("QR Error: {}", e))?;
    let svg_string = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(180, 180)
        .build();
    let b64 = base64::engine::general_purpose::STANDARD.encode(svg_string.as_bytes());
    Ok(format!("data:image/svg+xml;base64,{}", b64))
}

#[tauri::command]
fn minimize_window(window: WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
fn close_window(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn drag_window(window: WebviewWindow) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
        use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, HTCAPTION, WM_NCLBUTTONDOWN};

        if let Ok(hwnd_raw) = window.hwnd() {
            let hwnd = HWND(hwnd_raw.0 as _);
            unsafe {
                let _ = ReleaseCapture();
                SendMessageW(hwnd, WM_NCLBUTTONDOWN, WPARAM(HTCAPTION as usize), LPARAM(0));
            }
            return Ok(());
        }
    }

    window.start_dragging().map_err(|e| e.to_string())
}

pub fn run() {
    let app_state = Arc::new(AppState::default());
    let state_for_setup = Arc::clone(&app_state);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler({
                    let state_for_shortcut = Arc::clone(&app_state);
                    move |_app, shortcut, event| {
                        if event.state() == ShortcutState::Pressed {
                            let key = shortcut.to_string().to_lowercase();
                            if key.contains("ctrl") && key.contains("alt") && key.contains("s") {
                                println!("[Undetect] Global shortcut triggered capture");
                                let _ = capture_primary_screen(&state_for_shortcut);
                            }
                        }
                    }
                })
                .build(),
        )
        .manage(app_state)
        .setup(move |app| {
            // Apply initial content protection & register shortcut
            if let Some(main_window) = app.get_webview_window("main") {
                let _ = win_util::set_content_protection(&main_window, true);
            }

            // Register global shortcut
            let shortcut: Result<Shortcut, _> = "ctrl+alt+s".parse();
            if let Ok(sc) = shortcut {
                let _ = app.global_shortcut().register(sc);
            }

            // Start default server on port 9090 with PIN "1234"
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
            {
                let mut tx_lock = state_for_setup.shutdown_tx.lock().unwrap();
                *tx_lock = Some(shutdown_tx);
            }
            let server_state = Arc::clone(&state_for_setup);
            tokio_rt().spawn(async move {
                server::start_http_server(server_state, 9090, shutdown_rx).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            start_server,
            stop_server,
            capture_now,
            clear_session,
            toggle_taskbar,
            toggle_content_protection,
            generate_qr,
            minimize_window,
            close_window,
            drag_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running undetect application");
}
