mod capture;
pub mod llm;
mod ocr;
mod server;
mod state;
pub mod telemetry;
pub mod ui_patch;
pub mod voice;
mod win_util;

use base64::Engine;
use qrcode::QrCode;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::runtime::Runtime;

use crate::capture::capture_primary_screen;
use crate::state::{AppState, ScreenshotMeta};
use crate::telemetry::{classify_llm_error, TelemetryService};

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
    pub always_on_top: bool,
    pub click_through: bool,
    pub window_opacity: u8,
    pub telemetry_enabled: bool,
    pub app_version: String,
    pub ui_version: Option<String>,
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
            if let std::net::IpAddr::V4(v4) = ip
                && !v4.is_loopback() && !v4.is_link_local()
            {
                let u = format!("http://{}:{}/{}", v4, port, pass_param);
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
    let ui_version = state
        .active_ui_patch
        .read()
        .ok()
        .and_then(|g| g.as_ref().map(|p| p.ui_version.clone()));

    StatusResponse {
        running: cfg.running,
        port: cfg.port,
        password: cfg.password,
        local_urls: urls,
        screenshot_count: count,
        skip_taskbar: state.skip_taskbar.load(Ordering::Relaxed),
        content_protected: state.content_protected.load(Ordering::Relaxed),
        always_on_top: state.always_on_top.load(Ordering::Relaxed),
        click_through: state.click_through.load(Ordering::Relaxed),
        window_opacity: state.window_opacity.load(Ordering::Relaxed),
        telemetry_enabled: state.telemetry.is_enabled(),
        app_version: app_version.to_string(),
        ui_version,
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

    let normalized_port = if port == 0 { 9090 } else { port };
    state::save_server_config_to(&state::server_config_path(), normalized_port, &password)?;

    {
        let mut cfg = state.config.write().unwrap();
        cfg.port = normalized_port;
        cfg.password = password;
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    {
        let mut tx_lock = state.shutdown_tx.lock().unwrap();
        *tx_lock = Some(shutdown_tx);
    }

    let state_clone = Arc::clone(&state);
    tokio_rt().spawn(async move {
        server::start_http_server(state_clone, normalized_port, shutdown_rx).await;
    });

    // Small delay for listener to bind
    std::thread::sleep(std::time::Duration::from_millis(150));

    let ver = app.package_info().version.to_string();
    let status = build_status(&state, &ver);
    if status.running {
        TelemetryService::spawn_capture(&state, "remote_server_started", None);
    }
    Ok(status)
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
    TelemetryService::spawn_capture(
        &state,
        "stealth_mode_toggled",
        Some(serde_json::json!({
            "setting": "skip_taskbar",
            "value": hide,
        })),
    );
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
    TelemetryService::spawn_capture(
        &state,
        "stealth_mode_toggled",
        Some(serde_json::json!({
            "setting": "content_protection",
            "value": protect,
        })),
    );
    Ok(protect)
}

#[tauri::command]
async fn toggle_always_on_top(
    window: WebviewWindow,
    state: tauri::State<'_, Arc<AppState>>,
    always_on_top: bool,
) -> Result<bool, String> {
    window
        .set_always_on_top(always_on_top)
        .map_err(|e| format!("Failed to set always on top: {}", e))?;
    state.always_on_top.store(always_on_top, Ordering::Relaxed);
    TelemetryService::spawn_capture(
        &state,
        "stealth_mode_toggled",
        Some(serde_json::json!({
            "setting": "always_on_top",
            "value": always_on_top,
        })),
    );
    Ok(always_on_top)
}

#[tauri::command]
async fn toggle_telemetry(
    state: tauri::State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<bool, String> {
    let applied = state.telemetry.set_enabled(enabled)?;
    if applied {
        TelemetryService::spawn_capture(&state, "app_heartbeat", None);
    }
    Ok(applied)
}

#[tauri::command]
async fn report_client_error(
    state: tauri::State<'_, Arc<AppState>>,
    error_type: String,
    message: String,
    source: Option<String>,
    lineno: Option<u32>,
    colno: Option<u32>,
    stack: Option<String>,
) -> Result<bool, String> {
    let payload = crate::telemetry::ClientErrorPayload {
        error_type,
        message,
        source,
        lineno,
        colno,
        stack,
    };
    Ok(state.telemetry.record_client_error(&state, payload))
}

#[tauri::command]
fn get_telemetry_diagnostics(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::telemetry::TelemetryDiagnosticsDto, String> {
    Ok(state.telemetry.get_diagnostics(&state))
}

#[tauri::command]
async fn toggle_click_through(
    window: WebviewWindow,
    state: tauri::State<'_, Arc<AppState>>,
    click_through: bool,
) -> Result<bool, String> {
    window
        .set_ignore_cursor_events(click_through)
        .map_err(|e| format!("Failed to update click-through mode: {e}"))?;
    state.click_through.store(click_through, Ordering::Relaxed);
    TelemetryService::spawn_capture(
        &state,
        "stealth_mode_toggled",
        Some(serde_json::json!({
            "setting": "click_through",
            "value": click_through,
        })),
    );
    Ok(click_through)
}

#[tauri::command]
async fn set_window_opacity(
    window: WebviewWindow,
    state: tauri::State<'_, Arc<AppState>>,
    opacity: u8,
) -> Result<u8, String> {
    let clamped = win_util::set_window_opacity(&window, opacity)?;
    state.window_opacity.store(clamped, Ordering::Relaxed);
    TelemetryService::spawn_capture(
        &state,
        "stealth_mode_toggled",
        Some(serde_json::json!({
            "setting": "window_opacity",
            "value": clamped,
        })),
    );
    Ok(clamped)
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
fn confirm_ui_boot() -> Result<(), String> {
    ui_patch::confirm_ui_boot()
}

#[tauri::command]
async fn check_and_stage_ui_patch(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<ui_patch::UiPatchSyncResult, String> {
    let ver = app.package_info().version.to_string();
    let res = ui_patch::sync_ui_patch(&ver).await?;
    TelemetryService::spawn_capture(
        &state,
        "ui_patch_checked",
        Some(serde_json::json!({
            "staged": res.staged,
            "active_ui_version": res.active_ui_version,
            "pending_ui_version": res.pending_ui_version,
        })),
    );
    Ok(res)
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

#[tauri::command]
fn get_window_size(window: WebviewWindow) -> Result<win_util::WindowSizeDto, String> {
    win_util::get_window_logical_size(&window)
}

#[tauri::command]
fn set_window_size(
    window: WebviewWindow,
    width: f64,
    height: f64,
) -> Result<win_util::WindowSizeDto, String> {
    win_util::set_window_logical_size(&window, width, height)
}

#[tauri::command]
fn step_resize_window(
    window: WebviewWindow,
    dw: f64,
    dh: f64,
) -> Result<win_util::WindowSizeDto, String> {
    win_util::step_resize_window(&window, dw, dh)
}

#[tauri::command]
fn reset_window_size(window: WebviewWindow) -> Result<win_util::WindowSizeDto, String> {
    win_util::reset_window_bounds(&window)
}

#[tauri::command]
fn nudge_window(window: WebviewWindow, dx: f64, dy: f64) -> Result<(), String> {
    win_util::nudge_window_position(&window, dx, dy)
}

#[tauri::command]
fn toggle_window_visibility(
    window: WebviewWindow,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    win_util::toggle_window_visibility(&window, state.always_on_top.load(Ordering::Relaxed))
}

#[tauri::command]
fn start_resize_drag(window: WebviewWindow, direction: String) -> Result<(), String> {
    win_util::start_resize_drag(&window, &direction)
}

fn track_chat_sent(
    state: &Arc<AppState>,
    res: &crate::llm::ChatResponse,
    had_screenshot: bool,
    duration_ms: u64,
) {
    let words = res.reply.split_whitespace().count();
    let word_bucket = match words {
        0..=20 => "0-20",
        21..=100 => "21-100",
        101..=500 => "101-500",
        _ => "500+",
    };
    TelemetryService::spawn_capture(
        state,
        "chat_message_sent",
        Some(serde_json::json!({
            "provider": res.provider,
            "model": res.model,
            "with_screen": had_screenshot,
            "duration_ms": duration_ms,
            "response_words_bucket": word_bucket,
        })),
    );
}

fn track_chat_error(state: &Arc<AppState>, err_msg: &str, had_screenshot: bool) {
    let category = classify_llm_error(err_msg);
    let provider = state.llm.get_public_config().active_provider;
    TelemetryService::spawn_capture(
        state,
        "chat_error",
        Some(serde_json::json!({
            "provider": provider,
            "error_category": category,
            "with_screen": had_screenshot,
        })),
    );
}

#[tauri::command]
async fn send_llm_chat(
    state: tauri::State<'_, Arc<AppState>>,
    request: crate::llm::ChatRequest,
) -> Result<crate::llm::ChatResponse, String> {
    let start = std::time::Instant::now();
    let had_screenshot = request.screenshot_id.is_some();
    match state.llm.execute_chat(request, (*state).clone()).await {
        Ok(res) => {
            track_chat_sent(&state, &res, had_screenshot, start.elapsed().as_millis() as u64);
            Ok(res)
        }
        Err(err) => {
            track_chat_error(&state, &err, had_screenshot);
            Err(err)
        }
    }
}

#[tauri::command]
async fn send_llm_chat_stream(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    request: crate::llm::ChatRequest,
    stream_id: String,
) -> Result<crate::llm::ChatResponse, String> {
    let start = std::time::Instant::now();
    let had_screenshot = request.screenshot_id.is_some();
    match state
        .llm
        .execute_chat_stream(request, (*state).clone(), stream_id, app)
        .await
    {
        Ok(res) => {
            track_chat_sent(&state, &res, had_screenshot, start.elapsed().as_millis() as u64);
            Ok(res)
        }
        Err(err) => {
            track_chat_error(&state, &err, had_screenshot);
            Err(err)
        }
    }
}

#[tauri::command]
fn get_llm_config(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::llm::PublicLlmConfig, String> {
    Ok(state.llm.get_public_config())
}

#[tauri::command]
fn set_llm_config(
    state: tauri::State<'_, Arc<AppState>>,
    config: crate::llm::LlmConfig,
) -> Result<crate::llm::PublicLlmConfig, String> {
    state.llm.apply_config_update(config)
}

#[tauri::command]
fn clear_llm_api_key(
    state: tauri::State<'_, Arc<AppState>>,
    provider: String,
) -> Result<crate::llm::PublicLlmConfig, String> {
    state.llm.clear_api_key(&provider)
}

#[tauri::command]
fn upload_resume(
    state: tauri::State<'_, Arc<AppState>>,
    filename: String,
    bytes: Vec<u8>,
) -> Result<crate::llm::PublicLlmConfig, String> {
    state.llm.upload_resume(&filename, &bytes)
}

#[tauri::command]
fn clear_resume(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::llm::PublicLlmConfig, String> {
    state.llm.clear_resume()
}

#[tauri::command]
fn set_job_description(
    state: tauri::State<'_, Arc<AppState>>,
    job_description: String,
) -> Result<crate::llm::PublicLlmConfig, String> {
    state.llm.set_job_description(&job_description)
}

#[tauri::command]
fn run_ocr(
    state: tauri::State<'_, Arc<AppState>>,
    screenshot_id: Option<String>,
) -> Result<crate::llm::OcrResponse, String> {
    let start = std::time::Instant::now();
    match state.llm.extract_ocr_from_state(screenshot_id.as_deref(), &state) {
        Ok(text) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let words = text.split_whitespace().count();
            let bucket = match words {
                0 => "0",
                1..=50 => "1-50",
                51..=200 => "51-200",
                _ => "200+",
            };
            TelemetryService::spawn_capture(
                &state,
                "ocr_executed",
                Some(serde_json::json!({
                    "success": true,
                    "duration_ms": duration_ms,
                    "word_count_bucket": bucket,
                    "ocr_engine": "windows_media_ocr",
                })),
            );
            Ok(crate::llm::OcrResponse {
                success: true,
                text,
                error: None,
            })
        }
        Err(e) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            TelemetryService::spawn_capture(
                &state,
                "ocr_executed",
                Some(serde_json::json!({
                    "success": false,
                    "duration_ms": duration_ms,
                    "word_count_bucket": "0",
                    "ocr_engine": "windows_media_ocr",
                })),
            );
            Ok(crate::llm::OcrResponse {
                success: false,
                text: "".to_string(),
                error: Some(e),
            })
        }
    }
}

#[tauri::command]
async fn get_groq_models(
    state: tauri::State<'_, Arc<AppState>>,
    api_key: Option<String>,
) -> Result<Vec<String>, String> {
    state.llm.fetch_groq_models(api_key.as_deref()).await
}

#[tauri::command]
fn get_voice_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::voice::VoiceStatus, String> {
    Ok(state.voice.get_status())
}

#[tauri::command]
fn set_voice_config(
    state: tauri::State<'_, Arc<AppState>>,
    config: crate::voice::VoiceConfig,
) -> Result<crate::voice::VoiceStatus, String> {
    state.voice.update_config(config)
}

#[tauri::command]
fn set_voice_source(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    source: String,
) -> Result<crate::voice::VoiceStatus, String> {
    state.voice.set_source(&app, &source)
}

#[tauri::command]
fn clear_voice_api_key(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::voice::VoiceStatus, String> {
    state.voice.stop_listening(&app);
    state.voice.clear_deepgram_key()
}

#[tauri::command]
fn list_voice_devices() -> Result<Vec<crate::voice::AudioDeviceDto>, String> {
    crate::voice::list_audio_devices()
}

#[tauri::command]
async fn setup_vosk(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::voice::VoiceStatus, String> {
    state.voice.setup_vosk(&app).await
}

fn track_voice_start(state: &Arc<AppState>, status: &crate::voice::VoiceStatus) {
    if status.listening {
        state.telemetry.record_voice_started();
        TelemetryService::spawn_capture(
            state,
            "voice_listening_started",
            Some(serde_json::json!({
                "provider": status.provider,
                "source": status.source,
            })),
        );
    }
}

fn track_voice_stop(state: &Arc<AppState>, status: &crate::voice::VoiceStatus) {
    let duration_secs = state.telemetry.record_voice_stopped();
    TelemetryService::spawn_capture(
        state,
        "voice_listening_stopped",
        Some(serde_json::json!({
            "provider": status.provider,
            "source": status.source,
            "duration_secs": duration_secs,
        })),
    );
}

#[tauri::command]
fn start_voice_input(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::voice::VoiceStatus, String> {
    let status = state.voice.start_listening(&app)?;
    track_voice_start(&state, &status);
    Ok(status)
}

#[tauri::command]
fn stop_voice_input(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::voice::VoiceStatus, String> {
    let status = state.voice.stop_listening(&app);
    track_voice_stop(&state, &status);
    Ok(status)
}

#[tauri::command]
fn toggle_voice_input(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::voice::VoiceStatus, String> {
    let status = state.voice.toggle_listening(&app)?;
    if status.listening {
        track_voice_start(&state, &status);
    } else {
        track_voice_stop(&state, &status);
    }
    Ok(status)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GlobalAction {
    ToggleVisibility,
    FocusChatInput,
    AnswerScreen,
    ScrollChatUp,
    ScrollChatDown,
    WidthShrink,
    WidthGrow,
    ToggleScreenAttachment,
    ToggleVoiceInput,
    ToggleProtection,
    ToggleClickThrough,
    ResizeReset,
}

// Chosen to avoid Windows/Intel display-rotation (`Ctrl+Alt+Arrow`) and OS shell defaults.
const GLOBAL_SHORTCUTS: &[(&str, GlobalAction)] = &[
    ("ctrl+alt+h", GlobalAction::ToggleVisibility),
    ("ctrl+alt+k", GlobalAction::FocusChatInput),
    ("ctrl+alt+enter", GlobalAction::AnswerScreen),
    ("ctrl+alt+shift+up", GlobalAction::ScrollChatUp),
    ("ctrl+alt+shift+down", GlobalAction::ScrollChatDown),
    ("ctrl+alt+shift+left", GlobalAction::WidthShrink),
    ("ctrl+alt+shift+right", GlobalAction::WidthGrow),
    ("ctrl+alt+a", GlobalAction::ToggleScreenAttachment),
    ("ctrl+alt+v", GlobalAction::ToggleVoiceInput),
    ("ctrl+alt+p", GlobalAction::ToggleProtection),
    ("ctrl+alt+g", GlobalAction::ToggleClickThrough),
    ("ctrl+alt+0", GlobalAction::ResizeReset),
];

fn parse_global_shortcuts() -> Vec<(Shortcut, GlobalAction, &'static str)> {
    GLOBAL_SHORTCUTS
        .iter()
        .filter_map(|&(spec, action)| match spec.parse::<Shortcut>() {
            Ok(sc) => Some((sc, action, spec)),
            Err(err) => {
                eprintln!("[Undetect] Failed to parse shortcut {}: {:?}", spec, err);
                None
            }
        })
        .collect()
}

fn dispatch_global_action(app: &AppHandle, state: &Arc<AppState>, action: GlobalAction) {
    let Some(main_window) = app.get_webview_window("main") else {
        return;
    };

    match action {
        GlobalAction::ToggleVisibility => {
            let on_top = state.always_on_top.load(Ordering::Relaxed);
            if let Ok(visible) = win_util::toggle_window_visibility(&main_window, on_top) {
                let _ = app.emit("shortcut://visibility", visible);
            }
        }
        GlobalAction::FocusChatInput => {
            let on_top = state.always_on_top.load(Ordering::Relaxed);
            if win_util::focus_window(&main_window, on_top).is_ok() {
                let _ = app.emit("shortcut://focus-chat", true);
            }
        }
        GlobalAction::AnswerScreen => {
            let _ = app.emit("shortcut://answer-screen", true);
        }
        GlobalAction::ScrollChatUp => {
            let _ = app.emit("shortcut://scroll", -160i32);
        }
        GlobalAction::ScrollChatDown => {
            let _ = app.emit("shortcut://scroll", 160i32);
        }
        GlobalAction::WidthShrink => {
            if let Ok(size) = win_util::step_resize_window(&main_window, -60.0, 0.0) {
                let _ = app.emit("shortcut://resized", size);
            }
        }
        GlobalAction::WidthGrow => {
            if let Ok(size) = win_util::step_resize_window(&main_window, 60.0, 0.0) {
                let _ = app.emit("shortcut://resized", size);
            }
        }
        GlobalAction::ToggleScreenAttachment => {
            let _ = app.emit("shortcut://toggle-screen", true);
        }
        GlobalAction::ToggleVoiceInput => {
            let status = state.voice.get_status();
            if !status.listening && !status.ready {
                let _ = app.emit("shortcut://voice-unconfigured", status);
            } else {
                match state.voice.toggle_listening(app) {
                    Ok(next) => track_voice_start(state, &next),
                    Err(err) => {
                        let _ = app.emit("vosk://error", err);
                    }
                }
            }
        }
        GlobalAction::ToggleProtection => {
            let next = !state.content_protected.load(Ordering::Relaxed);
            if win_util::set_content_protection(&main_window, next).is_ok() {
                state.content_protected.store(next, Ordering::Relaxed);
                let _ = app.emit("shortcut://protection", next);
            }
        }
        GlobalAction::ToggleClickThrough => {
            let next = !state.click_through.load(Ordering::Relaxed);
            if main_window.set_ignore_cursor_events(next).is_ok() {
                state.click_through.store(next, Ordering::Relaxed);
                let _ = app.emit("shortcut://click-through", next);
            }
        }
        GlobalAction::ResizeReset => {
            if let Ok(size) = win_util::reset_window_bounds(&main_window) {
                let _ = app.emit("shortcut://resized", size);
            }
        }
    }
}

pub fn run() {
    telemetry::install_panic_hook();
    let mut ctx = tauri::generate_context!();
    let app_version = ctx.package_info().version.to_string();
    let active_patch = ui_patch::resolve_startup_patch(&app_version);

    if let Some(patch_dir) = active_patch.as_ref().map(|p| p.bundle_dir.clone()) {
        struct EmptyAssets;
        impl<R: tauri::Runtime> tauri::Assets<R> for EmptyAssets {
            fn get(&self, _: &tauri::utils::assets::AssetKey) -> Option<std::borrow::Cow<'_, [u8]>> {
                None
            }
            fn iter(&self) -> Box<tauri::utils::assets::AssetsIter<'_>> {
                Box::new(std::iter::empty())
            }
            fn csp_hashes(
                &self,
                _: &tauri::utils::assets::AssetKey,
            ) -> Box<dyn Iterator<Item = tauri::utils::assets::CspHash<'_>> + '_> {
                Box::new(std::iter::empty())
            }
        }
        let fallback = ctx.set_assets(Box::new(EmptyAssets));
        ctx.set_assets(Box::new(ui_patch::HotSwapAssets::new(patch_dir, fallback)));
    }

    let app_state = Arc::new(AppState::default());
    if let Ok(mut guard) = app_state.active_ui_patch.write() {
        *guard = active_patch;
    }

    let state_for_setup = Arc::clone(&app_state);
    let parsed_shortcuts = parse_global_shortcuts();
    let handler_shortcuts = parsed_shortcuts.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler({
                    let state_for_shortcut = Arc::clone(&app_state);
                    move |app, shortcut, event| {
                        if event.state() == ShortcutState::Pressed
                            && let Some((_, action, _)) = handler_shortcuts
                                .iter()
                                .find(|(registered, _, _)| registered == shortcut)
                        {
                            dispatch_global_action(app, &state_for_shortcut, *action);
                        }
                    }
                })
                .build(),
        )
        .manage(app_state)
        .setup(move |app| {
            #[cfg(desktop)]
            {
                app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
                app.handle().plugin(tauri_plugin_process::init())?;
            }

            // Apply initial content protection, always-on-top & register shortcuts
            if let Some(main_window) = app.get_webview_window("main") {
                let _ = win_util::set_content_protection(&main_window, true);
                let _ = main_window.set_always_on_top(true);
                let _ = main_window.set_resizable(true);
                if let Some(icon) = app.default_window_icon() {
                    let _ = main_window.set_icon(icon.clone());
                }
            }

            for (sc, _, spec) in &parsed_shortcuts {
                if let Err(err) = app.global_shortcut().register(*sc) {
                    eprintln!("[Undetect] Global shortcut {} unavailable: {:?}", spec, err);
                }
            }

            // Start HTTP remote viewer using persisted port & PIN
            let initial_port = state_for_setup.config.read().unwrap().port;
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
            {
                let mut tx_lock = state_for_setup.shutdown_tx.lock().unwrap();
                *tx_lock = Some(shutdown_tx);
            }
            let server_state = Arc::clone(&state_for_setup);
            tokio_rt().spawn(async move {
                server::start_http_server(server_state, initial_port, shutdown_rx).await;
            });

            TelemetryService::start_heartbeat_loop(Arc::clone(&state_for_setup));

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
            toggle_always_on_top,
            toggle_telemetry,
            report_client_error,
            get_telemetry_diagnostics,
            toggle_click_through,
            set_window_opacity,
            generate_qr,
            minimize_window,
            close_window,
            drag_window,
            get_window_size,
            set_window_size,
            step_resize_window,
            reset_window_size,
            nudge_window,
            toggle_window_visibility,
            start_resize_drag,
            confirm_ui_boot,
            check_and_stage_ui_patch,
            send_llm_chat,
            send_llm_chat_stream,
            get_llm_config,
            set_llm_config,
            clear_llm_api_key,
            upload_resume,
            clear_resume,
            set_job_description,
            run_ocr,
            get_groq_models,
            get_voice_status,
            set_voice_config,
            set_voice_source,
            clear_voice_api_key,
            list_voice_devices,
            setup_vosk,
            start_voice_input,
            stop_voice_input,
            toggle_voice_input,
        ])
        .run(ctx)
        .expect("error while running undetect application");
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;

    #[test]
    fn all_global_shortcuts_parse_cleanly() {
        for &(spec, _) in GLOBAL_SHORTCUTS {
            assert!(
                spec.parse::<Shortcut>().is_ok(),
                "Global shortcut '{}' failed to parse",
                spec
            );
        }
    }
}
