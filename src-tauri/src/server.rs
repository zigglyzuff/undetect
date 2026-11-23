use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, Method, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::capture::capture_primary_screen;
use crate::state::{AppState, ScreenshotMeta};

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../dist"]
struct EmbeddedDist;

const EMBEDDED_VIEWER_HTML: &str = include_str!("../../dist/viewer.html");

#[derive(Deserialize)]
pub struct AuthQuery {
    pub pass: Option<String>,
    pub pin: Option<String>,
    pub id: Option<String>,
    pub thumb: Option<String>,
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub screenshots: Vec<ScreenshotMeta>,
}

#[derive(Serialize)]
pub struct ApiResponse {
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct DevReloadQuery {
    pub since: Option<u64>,
}

#[derive(Serialize)]
pub struct DevReloadResponse {
    pub reload: bool,
    pub timestamp: u64,
}

pub async fn start_http_server(
    state: Arc<AppState>,
    port: u16,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(handle_index))
        .route("/index.html", get(handle_index))
        .route("/viewer.html", get(handle_index))
        .route("/assets/{*path}", get(handle_asset))
        .route("/api/session", get(handle_session))
        .route("/api/screenshot", get(handle_screenshot))
        .route("/api/capture", post(handle_capture))
        .route("/api/session/clear", post(handle_clear))
        .route("/api/llm/chat", post(handle_llm_chat))
        .route("/api/llm/config", get(handle_get_llm_config).post(handle_set_llm_config))
        .route("/api/llm/ocr", post(handle_llm_ocr))
        .route("/api/llm/groq-models", get(handle_get_groq_models))
        .route("/api/dev-reload", get(handle_dev_reload))
        .layer(cors)
        .with_state(state.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[UndetectServer] Failed to bind on {}: {}", addr, e);
            if let Ok(mut cfg) = state.config.write() {
                cfg.running = false;
            }
            return;
        }
    };

    if let Ok(mut cfg) = state.config.write() {
        cfg.running = true;
    }
    println!("[UndetectServer] Remote Server running on http://0.0.0.0:{}", port);

    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = shutdown_rx.await;
    });

    if let Err(err) = server.await {
        eprintln!("[UndetectServer] Server error: {}", err);
    }

    if let Ok(mut cfg) = state.config.write() {
        cfg.running = false;
    }
    println!("[UndetectServer] Remote Server stopped.");
}

fn check_auth(state: &AppState, query: &AuthQuery) -> bool {
    let cfg = match state.config.read() {
        Ok(c) => c,
        Err(_) => return false,
    };

    if cfg.password.is_empty() {
        return true;
    }

    let req_pass = query
        .pass
        .as_deref()
        .or(query.pin.as_deref())
        .unwrap_or("");
    req_pass == cfg.password
}

fn active_ui_bundle_dir(state: &AppState) -> Option<std::path::PathBuf> {
    state
        .active_ui_patch
        .read()
        .ok()
        .and_then(|g| g.as_ref().map(|p| p.bundle_dir.clone()))
}

fn mime_for_asset(path: &str) -> &'static str {
    if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}

fn build_asset_response(bytes: Vec<u8>, mime: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn get_viewer_html(state: &AppState) -> String {
    if cfg!(debug_assertions) {
        let candidates = [
            "dist/viewer.html",
            "../dist/viewer.html",
            "../../dist/viewer.html",
        ];
        for path in candidates {
            if let Ok(content) = std::fs::read_to_string(path) {
                return content;
            }
        }
    }
    if let Some(bundle_dir) = active_ui_bundle_dir(state)
        && let Ok(content) = std::fs::read_to_string(bundle_dir.join("viewer.html"))
    {
        return content;
    }
    if let Some(file) = EmbeddedDist::get("viewer.html") {
        return String::from_utf8_lossy(&file.data).to_string();
    }
    EMBEDDED_VIEWER_HTML.to_string()
}

fn get_viewer_mtime() -> u64 {
    let candidates = [
        "dist/viewer.html",
        "../dist/viewer.html",
        "../../dist/viewer.html",
        "src/viewer/ViewerApp.svelte",
        "../src/viewer/ViewerApp.svelte",
    ];
    for path in candidates {
        if let Ok(meta) = std::fs::metadata(path)
            && let Ok(mtime) = meta.modified()
            && let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH)
        {
            return dur.as_millis() as u64;
        }
    }
    0
}

async fn handle_index(State(state): State<Arc<AppState>>) -> Response {
    Html(get_viewer_html(&state)).into_response()
}

async fn handle_asset(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Response {
    let Some(rel_path) = crate::ui_patch::sanitize_asset_rel_path(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let clean_path = rel_path.to_string_lossy().replace('\\', "/");
    let mime = mime_for_asset(&clean_path);

    if cfg!(debug_assertions) {
        let candidates = [
            format!("dist/assets/{clean_path}"),
            format!("../dist/assets/{clean_path}"),
            format!("../../dist/assets/{clean_path}"),
        ];
        for c in candidates {
            if let Ok(bytes) = std::fs::read(&c) {
                return build_asset_response(bytes, mime);
            }
        }
    }

    if let Some(bundle_dir) = active_ui_bundle_dir(&state)
        && let Ok(bytes) = std::fs::read(bundle_dir.join("assets").join(&rel_path))
    {
        return build_asset_response(bytes, mime);
    }

    // Embedded assets fallback for standalone binary (outside of dev environment)
    let embedded_path = format!("assets/{clean_path}");
    if let Some(file) = EmbeddedDist::get(&embedded_path) {
        return build_asset_response(file.data.into_owned(), mime);
    }

    StatusCode::NOT_FOUND.into_response()
}

async fn handle_dev_reload(Query(query): Query<DevReloadQuery>) -> Response {
    let client_since = query.since.unwrap_or(0);
    let mut checks = 0;

    loop {
        let current_mtime = get_viewer_mtime();
        if client_since > 0 && current_mtime > client_since {
            return Json(DevReloadResponse {
                reload: true,
                timestamp: current_mtime,
            })
            .into_response();
        }

        checks += 1;
        if checks >= 20 {
            return Json(DevReloadResponse {
                reload: false,
                timestamp: current_mtime,
            })
            .into_response();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

async fn handle_session(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthQuery>,
) -> Response {
    if !check_auth(&state, &query) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let screenshots: Vec<ScreenshotMeta> = state
        .screenshots
        .read()
        .map(|list| {
            list.iter()
                .map(|s| ScreenshotMeta {
                    id: s.id.clone(),
                    timestamp: s.timestamp.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    Json(SessionResponse { screenshots }).into_response()
}

async fn handle_screenshot(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthQuery>,
) -> Response {
    if !check_auth(&state, &query) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let is_thumb = query.thumb.as_deref().map(|t| t == "1" || t == "true").unwrap_or(false);

    let target_jpeg: Option<Vec<u8>> = {
        let list = match state.screenshots.read() {
            Ok(l) => l,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Lock error").into_response(),
        };

        let target_entry = if let Some(target_id) = &query.id {
            list.iter().find(|s| &s.id == target_id)
        } else {
            list.back()
        };

        target_entry.map(|s| {
            if is_thumb && !s.thumb_data.is_empty() {
                s.thumb_data.clone()
            } else {
                s.jpeg_data.clone()
            }
        })
    };

    match target_jpeg {
        Some(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/jpeg")
            .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
            .body(Body::from(bytes))
            .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR).into_response()),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "No screenshot found" })),
        )
            .into_response(),
    }
}

async fn handle_capture(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthQuery>,
) -> Response {
    if !check_auth(&state, &query) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    match capture_primary_screen(&state) {
        Ok(_) => Json(ApiResponse {
            success: true,
            message: Some("Capture triggered successfully".to_string()),
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: Some(e),
            }),
        )
            .into_response(),
    }
}

async fn handle_clear(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthQuery>,
) -> Response {
    if !check_auth(&state, &query) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    if let Ok(mut list) = state.screenshots.write() {
        list.clear();
    }

    Json(ApiResponse {
        success: true,
        message: Some("Session cleared".to_string()),
    })
    .into_response()
}


async fn handle_llm_chat(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthQuery>,
    Json(payload): Json<crate::llm::ChatRequest>,
) -> Response {
    if !check_auth(&state, &query) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    match state.llm.execute_chat(payload, state.clone()).await {
        Ok(res) => Json(res).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        )
            .into_response(),
    }
}

async fn handle_get_llm_config(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthQuery>,
) -> Response {
    if !check_auth(&state, &query) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let public_cfg = state.llm.get_public_config();
    Json(public_cfg).into_response()
}

async fn handle_set_llm_config(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthQuery>,
    Json(payload): Json<crate::llm::LlmConfig>,
) -> Response {
    if !check_auth(&state, &query) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    // Preserve keys and resume seamlessly via shared LlmService config updater
    match state.llm.apply_config_update(payload) {
        Ok(public_cfg) => {
            Json(serde_json::json!({ "success": true, "config": public_cfg })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        )
            .into_response(),
    }
}

async fn handle_llm_ocr(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthQuery>,
    Json(payload): Json<crate::llm::OcrRequest>,
) -> Response {
    if !check_auth(&state, &query) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    match state.llm.extract_ocr_from_state(payload.screenshot_id.as_deref(), &state) {
        Ok(text) => Json(crate::llm::OcrResponse {
            success: true,
            text,
            error: None,
        })
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(crate::llm::OcrResponse {
                success: false,
                text: "".to_string(),
                error: Some(e),
            }),
        )
            .into_response(),
    }
}


async fn handle_get_groq_models(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthQuery>,
) -> Response {
    if !check_auth(&state, &query) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    match state.llm.fetch_groq_models(None).await {
        Ok(models) => Json(serde_json::json!({ "success": true, "models": models })).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "success": false, "error": e })),
        )
            .into_response(),
    }
}
