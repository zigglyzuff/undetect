use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::state::AppState;
use crate::voice::setup::app_data_root;

pub const POSTHOG_CAPTURE_URL: &str = "https://us.i.posthog.com/i/v0/e/";
const DEFAULT_POSTHOG_API_KEY: &str = "phc_BCmsDSFYvWf8KKjCEXTHgG3VegvvHiKcvkHScD5g87Eb";
const HEARTBEAT_INTERVAL_SECS: u64 = 10 * 60;
const MAX_CLIENT_ERRORS_PER_SESSION: usize = 5;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemDiagnostics {
    pub os_name: String,
    pub arch: String,
    pub os_version: String,
    pub total_memory_gb: Option<u32>,
    pub memory_bucket: String,
    pub cpu_cores: usize,
    pub monitor_count: u32,
    pub primary_resolution: String,
}

impl Default for SystemDiagnostics {
    fn default() -> Self {
        Self {
            os_name: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            os_version: "unknown".to_string(),
            total_memory_gb: None,
            memory_bucket: "unknown".to_string(),
            cpu_cores: 1,
            monitor_count: 1,
            primary_resolution: "unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TelemetrySnapshot {
    pub app_version: String,
    pub ui_version: Option<String>,
    pub os: &'static str,
    pub arch: &'static str,
    pub env: &'static str,
    pub llm_provider: String,
    pub voice_provider: String,
    pub has_resume: bool,
    pub has_jd: bool,
    pub remote_server_running: bool,
    pub system: SystemDiagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    pub timestamp: String,
    pub crash_type: String, // "rust_panic", "client_error", "ui_patch_rollback"
    pub location: Option<String>,
    pub message: String,
    pub backtrace: Option<String>,
    pub app_version: String,
    pub ui_version: Option<String>,
    pub os: String,
    pub arch: String,
    pub os_version: String,
    pub memory_bucket: String,
    pub cpu_cores: usize,
    #[serde(default)]
    pub sent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReportSummary {
    pub timestamp: String,
    pub crash_type: String,
    pub location: Option<String>,
    pub message: String,
    pub sent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientErrorPayload {
    pub error_type: String,
    pub message: String,
    pub source: Option<String>,
    pub lineno: Option<u32>,
    pub colno: Option<u32>,
    pub stack: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryDiagnosticsDto {
    pub enabled: bool,
    pub distinct_id: String,
    pub endpoint: String,
    pub system: SystemDiagnostics,
    pub last_crash: Option<CrashReportSummary>,
}

pub struct PrivacySanitizer;

impl PrivacySanitizer {
    /// Sanitizes any string payload before writing to crash logs or dispatching via telemetry.
    /// Redacts usernames, Windows user profile paths, common secret keys, bearer tokens,
    /// external IP addresses, and email addresses.
    pub fn sanitize_text(input: &str) -> String {
        let mut text = input.to_string();

        if let Ok(user) = std::env::var("USERNAME") {
            let trimmed = user.trim();
            if !trimmed.is_empty() && trimmed.len() > 1 {
                text = text.replace(trimmed, "<user>");
            }
        }
        if let Ok(user) = std::env::var("USER") {
            let trimmed = user.trim();
            if !trimmed.is_empty() && trimmed.len() > 1 {
                text = text.replace(trimmed, "<user>");
            }
        }

        text = sanitize_user_path_segments(&text);
        text = mask_secret_tokens(&text);
        text = mask_ip_addresses(&text);
        text = mask_emails(&text);

        if text.len() > 2000 {
            let mut truncated: String = text.chars().take(2000).collect();
            truncated.push_str("... [truncated]");
            return truncated;
        }

        text
    }
}

fn sanitize_user_path_segments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let remaining: String = chars[i..len.min(i + 12)].iter().collect();
        let rem_lower = remaining.to_ascii_lowercase();
        if (rem_lower.len() >= 9
            && rem_lower.chars().nth(1) == Some(':')
            && &rem_lower[2..9] == "\\users\\")
            || rem_lower.starts_with("/users/")
            || rem_lower.starts_with("/home/")
        {
            let (prefix_len, sep) = if rem_lower.starts_with("/users/") {
                (7, '/')
            } else if rem_lower.starts_with("/home/") {
                (6, '/')
            } else {
                (9, '\\')
            };

            let prefix: String = chars[i..i + prefix_len].iter().collect();
            out.push_str(&prefix);
            out.push_str("<user>");
            i += prefix_len;

            while i < len
                && chars[i] != sep
                && chars[i] != ' '
                && chars[i] != '\n'
                && chars[i] != '\r'
                && chars[i] != '"'
                && chars[i] != '\''
            {
                i += 1;
            }
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }
    out
}

fn mask_secret_tokens(text: &str) -> String {
    let prefixes = [
        ("gsk_", 4),
        ("sk-", 3),
        ("phc_", 4),
        ("Bearer ", 7),
        ("token=", 6),
        ("key=", 4),
        ("api_key=", 8),
    ];

    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let mut matched_prefix: Option<(&'static str, usize)> = None;
        for &(pfx, plen) in &prefixes {
            if i + plen <= len {
                let candidate: String = chars[i..i + plen].iter().collect();
                if candidate.eq_ignore_ascii_case(pfx) {
                    matched_prefix = Some((pfx, plen));
                    break;
                }
            }
        }

        if let Some((pfx, plen)) = matched_prefix {
            out.push_str(pfx);
            out.push_str("<redacted>");
            i += plen;
            while i < len
                && (chars[i].is_ascii_alphanumeric()
                    || chars[i] == '_'
                    || chars[i] == '-'
                    || chars[i] == '.'
                    || chars[i] == '=')
            {
                i += 1;
            }
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }
    out
}

fn mask_ip_addresses(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let words: Vec<&str> = text
        .split_inclusive(|c: char| !c.is_ascii_alphanumeric() && c != '.')
        .collect();

    for token in words {
        let trimmed = token.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
        let parts: Vec<&str> = trimmed.split('.').collect();
        if parts.len() == 4
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.len() <= 3 && p.chars().all(|c| c.is_ascii_digit()))
        {
            let nums: Vec<Option<u16>> = parts.iter().map(|p| p.parse::<u16>().ok()).collect();
            if nums.iter().all(|n| matches!(n, Some(0..=255))) {
                if trimmed == "127.0.0.1" || trimmed == "0.0.0.0" {
                    out.push_str(token);
                } else {
                    let replaced = token.replace(trimmed, "<ip>");
                    out.push_str(&replaced);
                }
                continue;
            }
        }
        out.push_str(token);
    }
    out
}

fn mask_emails(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let words: Vec<&str> = text
        .split_inclusive(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '<' || c == '>')
        .collect();

    for token in words {
        let trimmed = token.trim_matches(|c: char| {
            c.is_whitespace() || c == '"' || c == '\'' || c == '<' || c == '>'
        });
        if let Some(at_idx) = trimmed.find('@') {
            if at_idx > 0 && at_idx < trimmed.len() - 1 {
                let domain = &trimmed[at_idx + 1..];
                if domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') {
                    let replaced = token.replace(trimmed, "<email>");
                    out.push_str(&replaced);
                    continue;
                }
            }
        }
        out.push_str(token);
    }
    out
}

pub fn classify_llm_error(err: &str) -> &'static str {
    let lower = err.to_ascii_lowercase();
    if lower.contains("401")
        || lower.contains("unauthorized")
        || lower.contains("invalid_api_key")
        || lower.contains("api key")
    {
        "auth_401"
    } else if lower.contains("429") || lower.contains("rate limit") || lower.contains("quota") {
        "rate_limit_429"
    } else if lower.contains("context_length")
        || lower.contains("maximum context")
        || lower.contains("too large")
    {
        "context_length_exceeded"
    } else if lower.contains("timeout") || lower.contains("timed out") {
        "timeout"
    } else if lower.contains("connection") || lower.contains("dns") || lower.contains("network") {
        "network_error"
    } else {
        "api_error"
    }
}

pub fn pending_crash_path() -> PathBuf {
    app_data_root().join("pending_crash.json")
}

pub fn last_crash_path() -> PathBuf {
    app_data_root().join("last_crash.json")
}

pub fn now_iso() -> String {
    let now = std::time::SystemTime::now();
    let epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = epoch.as_secs();

    let days = secs / 86400;
    let rem_secs = secs % 86400;
    let hours = rem_secs / 3600;
    let mins = (rem_secs % 3600) / 60;
    let s = rem_secs % 60;

    let z = days as i64 + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{hours:02}:{mins:02}:{s:02}Z")
}

/// Asynchronous, privacy-preserving PostHog telemetry and crash diagnostics service.
/// Never sends prompts, API keys, resumes, job descriptions, OCR text, screenshots, or raw machine IDs.
pub struct TelemetryService {
    distinct_id: String,
    api_key: String,
    endpoint: String,
    enabled: AtomicBool,
    storage_path: PathBuf,
    http_client: reqwest::Client,
    client_error_count: AtomicUsize,
    voice_start_time: Mutex<Option<Instant>>,
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
            client_error_count: AtomicUsize::new(0),
            voice_start_time: Mutex::new(None),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<bool, String> {
        let cfg = TelemetryConfig { enabled };
        Self::save_config(&self.storage_path, &cfg)?;
        self.enabled.store(enabled, Ordering::Relaxed);
        if !enabled {
            let pending = pending_crash_path();
            if pending.is_file() {
                let _ = fs::remove_file(pending);
            }
        }
        Ok(enabled)
    }

    pub fn record_voice_started(&self) {
        if let Ok(mut lock) = self.voice_start_time.lock() {
            *lock = Some(Instant::now());
        }
    }

    pub fn record_voice_stopped(&self) -> u64 {
        if let Ok(mut lock) = self.voice_start_time.lock() {
            if let Some(start) = lock.take() {
                return start.elapsed().as_secs();
            }
        }
        0
    }

    pub fn record_client_error(&self, state: &Arc<AppState>, payload: ClientErrorPayload) -> bool {
        let count = self.client_error_count.fetch_add(1, Ordering::Relaxed);
        if count >= MAX_CLIENT_ERRORS_PER_SESSION {
            return false;
        }

        let sanitized_msg = PrivacySanitizer::sanitize_text(&payload.message);
        let sanitized_source = payload.source.map(|s| PrivacySanitizer::sanitize_text(&s));
        let sanitized_stack = payload.stack.map(|s| PrivacySanitizer::sanitize_text(&s));
        let location = match (&sanitized_source, payload.lineno, payload.colno) {
            (Some(src), Some(line), Some(col)) => Some(format!("{src}:{line}:{col}")),
            (Some(src), Some(line), None) => Some(format!("{src}:{line}")),
            (Some(src), None, None) => Some(src.clone()),
            _ => None,
        };

        let sys = gather_system_diagnostics();
        let report = CrashReport {
            timestamp: now_iso(),
            crash_type: "client_error".to_string(),
            location: location.clone(),
            message: sanitized_msg.clone(),
            backtrace: sanitized_stack.clone(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            ui_version: None,
            os: sys.os_name.clone(),
            arch: sys.arch.clone(),
            os_version: sys.os_version.clone(),
            memory_bucket: sys.memory_bucket.clone(),
            cpu_cores: sys.cpu_cores,
            sent: self.is_enabled(),
        };

        let _ = fs::write(
            last_crash_path(),
            serde_json::to_string_pretty(&report).unwrap_or_default(),
        );

        if self.is_enabled() {
            let state_clone = Arc::clone(state);
            let error_type = payload.error_type;
            crate::tokio_rt().spawn(async move {
                let props = serde_json::json!({
                    "crash_type": "client_error",
                    "error_type": error_type,
                    "message": sanitized_msg,
                    "location": location,
                    "backtrace": sanitized_stack,
                    "client_side": true,
                    "$exception_type": "client_error",
                    "$exception_message": sanitized_msg,
                });
                state_clone
                    .telemetry
                    .capture_from_state(&state_clone, "app_crashed", Some(props))
                    .await;
            });
            true
        } else {
            false
        }
    }

    pub fn get_diagnostics(&self, _state: &AppState) -> TelemetryDiagnosticsDto {
        let system = gather_system_diagnostics();
        let last_crash = fs::read_to_string(last_crash_path())
            .ok()
            .and_then(|raw| serde_json::from_str::<CrashReport>(&raw).ok())
            .map(|r| CrashReportSummary {
                timestamp: r.timestamp,
                crash_type: r.crash_type,
                location: r.location,
                message: r.message,
                sent: r.sent,
            });

        TelemetryDiagnosticsDto {
            enabled: self.is_enabled(),
            distinct_id: self.distinct_id.clone(),
            endpoint: self.endpoint.clone(),
            system,
            last_crash,
        }
    }

    /// Checks for crash reports written during an abrupt panic or rollback in a previous run.
    pub async fn flush_pending_crashes(state: &Arc<AppState>) {
        let pending_path = pending_crash_path();
        if !pending_path.is_file() {
            return;
        }

        let raw = match fs::read_to_string(&pending_path) {
            Ok(s) => s,
            Err(_) => {
                let _ = fs::remove_file(&pending_path);
                return;
            }
        };

        let report = match serde_json::from_str::<CrashReport>(&raw) {
            Ok(r) => r,
            Err(_) => {
                let _ = fs::remove_file(&pending_path);
                return;
            }
        };

        if state.telemetry.is_enabled() {
            let props = serde_json::json!({
                "crash_type": report.crash_type,
                "message": report.message,
                "location": report.location,
                "backtrace": report.backtrace,
                "crash_timestamp": report.timestamp,
                "recovered_on_boot": true,
                "os_version": report.os_version,
                "memory_bucket": report.memory_bucket,
                "cpu_cores": report.cpu_cores,
                "$exception_type": report.crash_type,
                "$exception_message": report.message,
            });

            state
                .telemetry
                .capture_from_state(state, "app_crashed", Some(props))
                .await;

            let mut updated_report = report;
            updated_report.sent = true;
            let _ = fs::write(
                last_crash_path(),
                serde_json::to_string_pretty(&updated_report).unwrap_or_default(),
            );
        }

        let _ = fs::remove_file(&pending_path);
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

    /// Starts initial crash flush, launch event and periodic 10-minute presence heartbeats.
    pub fn start_heartbeat_loop(state: Arc<AppState>) {
        crate::tokio_rt().spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;

            Self::flush_pending_crashes(&state).await;

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
        let ui_version = state
            .active_ui_patch
            .read()
            .ok()
            .and_then(|g| g.as_ref().map(|p| p.ui_version.clone()));

        TelemetrySnapshot {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            ui_version,
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            env: if cfg!(debug_assertions) { "dev" } else { "prod" },
            llm_provider: llm.active_provider,
            voice_provider: voice.provider,
            has_resume: llm.has_resume,
            has_jd: llm.has_jd,
            remote_server_running,
            system: gather_system_diagnostics(),
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
        if let Some(ui_ver) = &snapshot.ui_version {
            person_set.insert("ui_version".to_string(), serde_json::json!(ui_ver));
        }
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
        person_set.insert("os_version".to_string(), serde_json::json!(snapshot.system.os_version));
        person_set.insert("memory_bucket".to_string(), serde_json::json!(snapshot.system.memory_bucket));
        person_set.insert("cpu_cores".to_string(), serde_json::json!(snapshot.system.cpu_cores));
        person_set.insert("monitor_count".to_string(), serde_json::json!(snapshot.system.monitor_count));
        person_set.insert("primary_resolution".to_string(), serde_json::json!(snapshot.system.primary_resolution));

        let mut properties = serde_json::Map::new();
        properties.insert("app_version".to_string(), serde_json::json!(snapshot.app_version));
        if let Some(ui_ver) = &snapshot.ui_version {
            properties.insert("ui_version".to_string(), serde_json::json!(ui_ver));
        }
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
        properties.insert("os_version".to_string(), serde_json::json!(snapshot.system.os_version));
        properties.insert("memory_bucket".to_string(), serde_json::json!(snapshot.system.memory_bucket));
        properties.insert("cpu_cores".to_string(), serde_json::json!(snapshot.system.cpu_cores));
        properties.insert("monitor_count".to_string(), serde_json::json!(snapshot.system.monitor_count));
        properties.insert("primary_resolution".to_string(), serde_json::json!(snapshot.system.primary_resolution));
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

pub fn record_ui_patch_rollback(failed_ui_version: &str, attempts: u8, app_version: &str) {
    let sys = gather_system_diagnostics();
    let report = CrashReport {
        timestamp: now_iso(),
        crash_type: "ui_patch_rollback".to_string(),
        location: Some(format!("ui_patch:{}", failed_ui_version)),
        message: format!(
            "UI patch {} rolled back after {} failed boot attempt(s)",
            failed_ui_version, attempts
        ),
        backtrace: None,
        app_version: app_version.to_string(),
        ui_version: Some(failed_ui_version.to_string()),
        os: sys.os_name.clone(),
        arch: sys.arch.clone(),
        os_version: sys.os_version.clone(),
        memory_bucket: sys.memory_bucket.clone(),
        cpu_cores: sys.cpu_cores,
        sent: false,
    };

    if let Ok(json) = serde_json::to_string_pretty(&report) {
        let _ = fs::write(pending_crash_path(), &json);
        let _ = fs::write(last_crash_path(), &json);
    }
}

pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()));
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };

        let backtrace = std::backtrace::Backtrace::capture();
        let bt_str = format!("{:#?}", backtrace);

        let sanitized_msg = PrivacySanitizer::sanitize_text(&message);
        let sanitized_loc = location.map(|l| PrivacySanitizer::sanitize_text(&l));
        let sanitized_bt = PrivacySanitizer::sanitize_text(&bt_str);

        let sys = gather_system_diagnostics();
        let report = CrashReport {
            timestamp: now_iso(),
            crash_type: "rust_panic".to_string(),
            location: sanitized_loc,
            message: sanitized_msg,
            backtrace: Some(sanitized_bt),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            ui_version: None,
            os: sys.os_name.clone(),
            arch: sys.arch.clone(),
            os_version: sys.os_version.clone(),
            memory_bucket: sys.memory_bucket.clone(),
            cpu_cores: sys.cpu_cores,
            sent: false,
        };

        if let Ok(json) = serde_json::to_string_pretty(&report) {
            let _ = fs::write(pending_crash_path(), &json);
            let _ = fs::write(last_crash_path(), &json);
        }

        default_hook(info);
    }));
}

#[cfg(windows)]
pub fn gather_system_diagnostics() -> SystemDiagnostics {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CMONITORS, SM_CXSCREEN, SM_CYSCREEN,
    };

    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let (total_memory_gb, memory_bucket) = {
        let mut mem = MEMORYSTATUSEX::default();
        mem.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if unsafe { GlobalMemoryStatusEx(&mut mem).is_ok() } {
            let gb = (mem.ullTotalPhys as f64 / (1024.0 * 1024.0 * 1024.0)).round() as u32;
            let bucket = match gb {
                0..=3 => "< 4 GB",
                4..=7 => "4 GB - 7 GB",
                8..=15 => "8 GB - 15 GB",
                16..=31 => "16 GB - 31 GB",
                32..=63 => "32 GB - 63 GB",
                _ => "64 GB+",
            };
            (Some(gb), bucket.to_string())
        } else {
            (None, "unknown".to_string())
        }
    };

    let monitor_count = unsafe { GetSystemMetrics(SM_CMONITORS) };
    let monitor_count = if monitor_count > 0 {
        monitor_count as u32
    } else {
        1
    };

    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let primary_resolution = if screen_w > 0 && screen_h > 0 {
        format!("{screen_w}x{screen_h}")
    } else {
        "unknown".to_string()
    };

    let os_version =
        read_windows_os_version().unwrap_or_else(|| "Windows (unknown build)".to_string());

    SystemDiagnostics {
        os_name: "windows".to_string(),
        arch: std::env::consts::ARCH.to_string(),
        os_version,
        total_memory_gb,
        memory_bucket,
        cpu_cores,
        monitor_count,
        primary_resolution,
    }
}

#[cfg(windows)]
fn read_windows_os_version() -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};

    let subkey: Vec<u16> = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\0"
        .encode_utf16()
        .collect();

    let read_string_val = |val_name: &str| -> Option<String> {
        let val_w: Vec<u16> = format!("{val_name}\0").encode_utf16().collect();
        let mut buf = [0u16; 128];
        let mut byte_len = std::mem::size_of_val(&buf) as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(subkey.as_ptr()),
                PCWSTR(val_w.as_ptr()),
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
            let s = String::from_utf16_lossy(&slice[..nul_pos]).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
        None
    };

    let display_version = read_string_val("DisplayVersion");
    let build_number = read_string_val("CurrentBuildNumber");

    match (display_version, build_number) {
        (Some(ver), Some(build)) => Some(format!("Windows Build {build} ({ver})")),
        (None, Some(build)) => Some(format!("Windows Build {build}")),
        (Some(ver), None) => Some(format!("Windows ({ver})")),
        (None, None) => None,
    }
}

#[cfg(not(windows))]
pub fn gather_system_diagnostics() -> SystemDiagnostics {
    SystemDiagnostics {
        os_name: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        os_version: std::env::consts::OS.to_string(),
        total_memory_gb: None,
        memory_bucket: "unknown".to_string(),
        cpu_cores: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        monitor_count: 1,
        primary_resolution: "unknown".to_string(),
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
            app_version: "2.2.0".to_string(),
            ui_version: Some("2.2.0-ui.1".to_string()),
            os: "windows",
            arch: "x86_64",
            env: "prod",
            llm_provider: "groq".to_string(),
            voice_provider: "deepgram".to_string(),
            has_resume: true,
            has_jd: false,
            remote_server_running: true,
            system: SystemDiagnostics {
                os_name: "windows".to_string(),
                arch: "x86_64".to_string(),
                os_version: "Windows Build 22631".to_string(),
                total_memory_gb: Some(16),
                memory_bucket: "16 GB - 31 GB".to_string(),
                cpu_cores: 12,
                monitor_count: 2,
                primary_resolution: "2560x1440".to_string(),
            },
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
        assert_eq!(payload["properties"]["os_version"], "Windows Build 22631");
        assert_eq!(payload["properties"]["memory_bucket"], "16 GB - 31 GB");
        assert_eq!(payload["properties"]["$set"]["app_version"], "2.2.0");
        assert_eq!(payload["properties"]["$set"]["ui_version"], "2.2.0-ui.1");
        assert_eq!(payload["properties"]["$set"]["has_resume"], true);
        assert_eq!(payload["properties"]["$set"]["primary_resolution"], "2560x1440");
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

    #[test]
    fn sanitizes_user_paths_and_secret_keys() {
        let raw = "Error in C:\\Users\\AliceSmith\\AppData\\Roaming\\undetect: invalid key gsk_abc1234567890xyz and sk-9988776655443322";
        let sanitized = PrivacySanitizer::sanitize_text(raw);

        assert!(!sanitized.contains("AliceSmith"));
        assert!(sanitized.contains("C:\\Users\\<user>\\AppData\\Roaming\\undetect"));
        assert!(!sanitized.contains("gsk_abc1234567890xyz"));
        assert!(sanitized.contains("gsk_<redacted>"));
        assert!(!sanitized.contains("sk-9988776655443322"));
        assert!(sanitized.contains("sk-<redacted>"));
    }

    #[test]
    fn sanitizes_bearer_tokens_and_ips_and_emails() {
        let raw = "Auth header Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9 from 192.168.1.50 for user@example.com (local 127.0.0.1)";
        let sanitized = PrivacySanitizer::sanitize_text(raw);

        assert!(!sanitized.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
        assert!(sanitized.contains("Bearer <redacted>"));
        assert!(!sanitized.contains("192.168.1.50"));
        assert!(sanitized.contains("<ip>"));
        assert!(!sanitized.contains("user@example.com"));
        assert!(sanitized.contains("<email>"));
        assert!(sanitized.contains("127.0.0.1"));
    }

    #[test]
    fn classifies_llm_errors_accurately() {
        assert_eq!(classify_llm_error("HTTP 401 Unauthorized invalid_api_key"), "auth_401");
        assert_eq!(classify_llm_error("HTTP 429 Rate limit reached, try later"), "rate_limit_429");
        assert_eq!(classify_llm_error("request context_length exceeds 128000"), "context_length_exceeded");
        assert_eq!(classify_llm_error("operation timed out after 60s"), "timeout");
        assert_eq!(classify_llm_error("dns lookup failed for api.groq.com"), "network_error");
        assert_eq!(classify_llm_error("unknown json parsing failure"), "api_error");
    }

    #[test]
    fn gathers_system_diagnostics_without_panicking() {
        let sys = gather_system_diagnostics();
        assert!(!sys.os_name.is_empty());
        assert!(!sys.arch.is_empty());
        assert!(sys.cpu_cores >= 1);
        assert!(sys.monitor_count >= 1);
    }
}
