# Undetect (`v2.2.0`) — Technical Reference (`llms.md`)

> **LLM / Agent Reader Protocol (Progressive Disclosure)**
> Do **not** read this entire file at once.
> 1. Read **lines 1–40** (`Snapshot` + `Section Index`).
> 2. Pick only the `[SEC-XX]` section(s) relevant to your task.
> 3. Fetch that exact line range (`StartLine..EndLine`) or grep `## [SEC-XX]`.

## 30-Second Snapshot

- **What it is**: Windows-first stealth desktop AI interview assistant + local Wi-Fi remote screen viewer (`com.undetect.desktop`, `v2.2.0`).
- **Stack**: **Rust / Tauri v2 / Axum / Tokio** (`src-tauri/`) + **Svelte 5 / Vite 6 / Tailwind CSS v4 / Flowbite Svelte** (`src/`).
- **Primary Capabilities**:
  1. Invisible-to-capture overlay (`WDA_EXCLUDEFROMCAPTURE`, Ghost Mode click-through, Neutral Cursor, opacity, taskbar hiding).
  2. Streaming AI Chat (`Groq Cloud` default `openai/gpt-oss-120b` or `Google Gemini` `gemini-3.6-flash`) with screen capture + on-device Windows OCR fallback.
  3. Interview context injection (max 2-page Resume upload + chat-scoped Job Description).
  4. Real-time voice transcription (`Deepgram Cloud` WebSocket default `nova-3` or offline `Vosk` FFI), defaulting to WASAPI `system` loopback audio, with utterance auto-stop & optional auto-send.
  5. Embedded LAN Remote Viewer served over Axum HTTP (`undetect` title + favicon, PIN-protected screenshot viewer).
  6. Anonymous PostHog telemetry (US Cloud, hashed `MachineGuid`, dev/prod split, opt-out toggle) + hybrid updates (signed `tauri-plugin-updater` passive binary releases + signed `Context::set_assets` UI-only hot-patches).

---

## Section Index

| ID | Topic | Lines | Primary Source Files |
| :--- | :--- | :--- | :--- |
| `[SEC-01]` | Architecture, Entrypoints & Build Layout | `L42-L64` | `src-tauri/src/main.rs`, `lib.rs`, `tauri.conf.json`, `vite.config.js`, `.cargo/config.toml` |
| `[SEC-02]` | Window Stealth, Capture & Win32 Internals | `L68-L88` | `src-tauri/src/win_util.rs`, `capture.rs`, `ocr.rs` |
| `[SEC-03]` | AI Chat & Streaming LLM Pipeline | `L92-L119` | `src-tauri/src/llm/*`, `src/chat/*`, `ModelProviderFields.svelte` |
| `[SEC-04]` | Resume & Job Description Interview Context | `L123-L133` | `src-tauri/src/llm/resume.rs`, `service.rs`, `ChatView.svelte` |
| `[SEC-05]` | Live Voice Transcription Engine | `L137-L157` | `src-tauri/src/voice/*`, `VoiceSettingsCard.svelte`, `ChatView.svelte` |
| `[SEC-06]` | LAN Remote Viewer & Axum HTTP Server | `L161-L183` | `src-tauri/src/server.rs`, `viewer.html`, `src/viewer/*`, `RemoteServerView.svelte` |
| `[SEC-07]` | Keyboard Shortcuts & Tauri Event Bus | `L187-L215` | `src-tauri/src/lib.rs`, `src/lib/shortcuts.js`, `src/App.svelte` |
| `[SEC-08]` | Complete IPC Command & HTTP API Reference | `L219-L226` | `src-tauri/src/lib.rs`, `src-tauri/src/server.rs` |
| `[SEC-09]` | Persistence, AppData Paths & Fresh-Slate Reset | `L230-L245` | `state.rs`, `llm/service.rs`, `voice/setup.rs`, `telemetry.rs`, `reset-undetect.bat` |
| `[SEC-10]` | PostHog Telemetry & In-App Updater | `L249-L262` | `src-tauri/src/telemetry.rs`, `lib.rs`, `src/App.svelte`, `SettingsView.svelte` |
| `[SEC-11]` | Frontend UI, Tailwind v4 Caveats & Dead Files | `L266-L280` | `src/app.css`, `src/components/*`, `src/lib/*` |
| `[SEC-12]` | Build, Test, CI & Release Engineering | `L284-L296` | `src-tauri/Cargo.toml`, `.github/workflows/release.yml`, `capabilities/default.json` |

---

## [SEC-01] Architecture, Entrypoints & Build Layout

### Boot Sequence
1. **Binary entry**: `src-tauri/src/main.rs` calls `undetect_lib::run()` (`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`).
2. **State bootstrap**: `AppState::default()` (`src-tauri/src/state.rs`) initializes:
   - `screenshots`: bounded `RwLock<VecDeque<ScreenshotEntry>>` (cap `25`).
   - `config`: `ServerConfig` loaded from `%LOCALAPPDATA%\Undetect\server_config.json` (default `port: 9090`, `password: "1234"`, `running: false`).
   - `llm`: `Arc<LlmService>` loaded from `undetect_llm_config.json`.
   - `voice`: `Arc<VoiceService>` loaded from `voice_config.json`.
   - `telemetry`: `Arc<TelemetryService>` loaded from `telemetry_config.json`.
   - Atomic window flags: `skip_taskbar = false`, `content_protected = true`, `always_on_top = true`, `click_through = false`, `window_opacity = 100`.
3. **Tauri `.setup(...)` (`src-tauri/src/lib.rs`)**:
   - Configures `"main"` window (`460x710`, min `360x480`, frameless `decorations: false`, `always_on_top: true`, `content_protected: true`, resizable, app icon).
   - Registers all `GLOBAL_SHORTCUTS` via `tauri-plugin-global-shortcut`.
   - Spawns Axum HTTP remote server on `tokio_rt()` bound to `0.0.0.0:<persisted_port>`.
   - Starts `TelemetryService::start_heartbeat_loop`.

### Multi-Entry Vite + RustEmbed Packaging
- `vite.config.js` builds **two** HTML inputs into `dist/`:
  - `main`: `index.html` -> `src/main.js` -> `src/App.svelte` (desktop Tauri shell).
  - `viewer`: `viewer.html` -> `src/viewer/main.js` -> `src/viewer/ViewerApp.svelte` (LAN web viewer, favicon `/src/assets/logo.png`, `<title>undetect</title>`).
- `src-tauri/src/server.rs` embeds `dist/` at compile time using `#[derive(RustEmbed)] #[folder = "../dist"] struct EmbeddedDist;` + `include_str!("../../dist/viewer.html")`, prioritizing dev disk candidates in debug builds and `%LOCALAPPDATA%\Undetect\ui_patches\` when a verified UI hot-patch is active.
- Desktop assets resolve through `HotSwapAssets` (`Context::set_assets` in `src-tauri/src/lib.rs`, `src-tauri/src/ui_patch.rs`), preserving `http://tauri.localhost` origin while overlaying verified UI patches onto compiled `EmbeddedAssets`.
- `.cargo/config.toml` (gitignored) sets `target-dir = "E:/cargo_targets/undetect"` locally; CI runners use default `src-tauri/target/`.

---

## [SEC-02] Window Stealth, Capture & Win32 Internals

### Stealth & Window Controls (`src-tauri/src/win_util.rs`)
- **Capture exclusion (`set_content_protection`)**: Calls Win32 `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` (`0x11`) when `true` or `WDA_NONE` (`0x0`) when `false`. Hides window pixels from `BitBlt`, `DXGI Desktop Duplication`, `Windows.Graphics.Capture`, Zoom/Meet/Teams/OBS.
- **Ghost Mode (`toggle_click_through`)**: Calls `window.set_ignore_cursor_events(bool)`. Mouse hover, clicks, and wheel pass directly through Undetect to underlying windows without changing OS cursor shape or stealing focus. Reversible via global hotkey `Ctrl + Alt + G`.
- **Neutral Cursor (`stealthCursor` in `src/App.svelte`, `src/app.css`)**: Defaults to `true` (persisted in `localStorage` as `undetect_stealth_cursor`). Applies `.stealth-cursor, .stealth-cursor * { cursor: default !important; }` so moving over resize grips/buttons doesn't flash telltale `↔`/`↕`/`grab`/`pointer` cursors over invisible capture bounds. Note: Windows OS hardware cursor itself cannot be selectively hidden from screen capture without hiding it locally too (`ShowCursor`).
- **Taskbar & Always-on-Top**: `window.set_skip_taskbar(bool)` and `window.set_always_on_top(bool)`.
- **Window Opacity (`set_window_opacity`)**: Clamped `40..=100`. Uses `WS_EX_LAYERED` + `SetLayeredWindowAttributes(..., LWA_ALPHA)`. Clears `WS_EX_LAYERED` at `100%` for native rendering.
- **Show/Hide & Focus (`toggle_window_visibility`, `focus_window`)**: Hiding uses `window.hide()`; showing calls `ShowWindow(hwnd, SW_SHOWNOACTIVATE)` + `SetWindowPos(HWND_TOPMOST)` so toggling visibility via `Ctrl + Alt + H` never steals focus from an active browser/IDE. `focus_window` (`Ctrl + Alt + K`) activates focus intentionally.
- **Sizing & Edges (`set_window_size`, `step_resize_window`, `reset_window_size`, `start_resize_drag`)**:
  - Default `460x710`, minimum `360x480`, clamped cleanly inside current monitor's logical `work_area`. Presets: Compact (`380x560`), Default (`460x710`), Wide (`640x760`).
  - Borderless resize handles initiate native `WM_NCLBUTTONDOWN` with `HTTOP`/`HTBOTTOM`/`HTLEFT`/`HTRIGHT`/corners after `ReleaseCapture()`.

### Screen Capture & On-Device OCR (`capture.rs`, `ocr.rs`)
- **Capture pipeline (`capture.rs`)**:
  - Tries `xcap::Monitor::all()` primary monitor first; on Windows handle/DXGI failures falls back automatically to Win32 GDI `CreateDCW("DISPLAY")` + `BitBlt(SRCCOPY)` + `GetDIBits`.
  - Downscales width > `1920px` proportionally (`FilterType::Triangle`). Encodes JPEG at quality `85`, dropping to quality `65` if > `2,800,000` bytes.
  - `capture_primary_screen(state)` stores JPEG + `160px`-wide thumbnail (~3–5 KB, q65) in `state.screenshots` (`MAX_SESSION_SCREENSHOTS = 25`), assigning epoch-ms `id` and `HH:MM:SS` timestamp.
  - `capture_primary_screen_jpeg()` captures ephemeral JPEG for chat/OCR without mutating remote viewer session history.
- **Windows OCR (`ocr.rs`)**:
  - `extract_text_from_jpeg(&[u8])` feeds JPEG bytes into `InMemoryRandomAccessStream` -> `BitmapDecoder` -> `SoftwareBitmap` -> `OcrEngine::TryCreateFromUserProfileLanguages()`. No external Tesseract/binaries required.

---

## [SEC-03] AI Chat & Streaming LLM Pipeline

### OOP Backend Design (`src-tauri/src/llm/`)
- **Contract (`provider.rs`)**: `#[async_trait] pub trait LlmProvider: Send + Sync`:
  - `provider_id()`, `display_name()`, `model()`, `supports_vision()`
  - `complete(&self, req, client)` (non-streaming JSON completion, used by HTTP API & tests)
  - `complete_stream(&self, req, client, &UnboundedSender<String>)` (SSE streaming)
  - Shared helpers: `extract_http_error(res, label)` and `consume_sse_stream(res, label, on_data)` (line-buffered byte stream parser across partial UTF-8/TCP chunks).
- **Factory (`factory.rs`)**: `LlmFactory::create_provider(provider_name, config, model_override)` instantiates `GeminiProvider` or `GroqProvider`.
- **Groq (`groq.rs`)**:
  - Default active provider (`"groq"`) and default model (`"openai/gpt-oss-120b"`).
  - Endpoint: `https://api.groq.com/openai/v1/chat/completions` (`temperature: 0.7`, `max_completion_tokens: 2048`, `stream: bool`).
  - Dynamic model discovery (`LlmService::fetch_groq_models`): `GET https://api.groq.com/openai/v1/models`, excludes `whisper*`, `tts*`, `*orpheus*`, `*prompt-guard*`, `*safeguard*`, ranks `gpt-oss-120b` -> vision models (`vision`, `vl`, `llama-4`, `qwen3.8`, `muse-spark`, `llava`, `pixtral`) -> `gpt-oss` -> `llama-3.3` -> `deepseek` -> alphabetical.
  - Vision vs Text-only: Vision models receive `[text, image_url(data:image/jpeg;base64,...)]`; text-only models inject Windows OCR text and return an explicit error if screen attach is requested on blank OCR.
- **Gemini (`gemini.rs`)**:
  - Default model: `"gemini-3.6-flash"`. Active free-tier list in `ChatService.js`: `gemini-3.6-flash`, `gemini-3.8-flash`, `gemini-3.5-flash-lite`, `gemini-3.1-flash-lite`, `gemini-2.5-flash-lite`. Obsolete IDs (`1.5`, `2.0`, `2.5-flash`/`pro`, `3-pro-preview`, `3.1-flash-lite-preview`) auto-migrate in `normalize_gemini_model`.
  - Endpoints: `generateContent` / `streamGenerateContent?alt=sse` with `x-goog-api-key` header, inline JPEG (`inlineData`), and `systemInstruction`.
  - Filters out `"thought": true` reasoning parts cleanly in both JSON and SSE responses.

### Chat Service & Streaming UX (`service.rs`, `ChatService.js`, `ChatView.svelte`)
- `LlmService::prepare_chat` captures/resolves JPEG (`"capture"` / `"latest"` / ID), triggers `spawn_blocking` OCR when `use_ocr || auto_ocr || !provider.supports_vision()`, and compiles effective interview system prompt (`resume.rs`).
- `execute_chat_stream` emits `llm://stream` (`{ stream_id, delta }`) to Tauri frontend via `mpsc` forwarder task.
- Frontend `ChatView.svelte`:
  - `ChatService.sendChatStream` invokes `send_llm_chat_stream` in Tauri (or falls back to HTTP `POST /api/llm/chat` outside Tauri).
  - Anchors scroll cleanly to the **top** of the new assistant message (`scrollMessageTopIntoView`) once on the first streamed chunk, rather than auto-scrolling down while the user reads.
  - Custom markdown renderer (`src/chat/utils/markdown.js`) escapes HTML, isolates code fences/inline code from inline rules, and renders `.copy-btn` blocks.
  - `sessionStorage` caches `undetect_chat_history` per session.
  - Missing API key triggers centered setup notice card + `LLM API Key Required` modal on send.

---

## [SEC-04] Resume & Job Description Interview Context

### Multi-Format Resume Extractor (`src-tauri/src/llm/resume.rs`)
- OOP trait `ResumeParser` selected by extension + magic bytes (`%PDF-`, `PK\x03\x04` OOXML/ODF):
  - **PDF (`PdfResumeParser`)**: Page count checked strictly via `lopdf::Document::load_mem`; text extracted via `pdf_extract::extract_text_from_mem`. Rejects scanned image-only PDFs with zero selectable text.
  - **Word OOXML (`DocxResumeParser`)**: Reads `docProps/app.xml` `<Pages>`, counts explicit `<w:br w:type="page"/>` / `<w:lastRenderedPageBreak/>`, parses `word/header*.xml` + `word/document.xml`.
  - **OpenDocument (`OdtResumeParser`)**: Parses `content.xml` + `meta.xml`.
  - **RTF / HTML-Word / Plain Text (`TextAndRtfResumeParser`)**: Strips RTF control groups or HTML `.doc` tags; rejects legacy OLE2 binary `.doc` files cleanly.
- **Hard Constraints**: Max **2 pages**, max **10 MB** upload (`MAX_RESUME_BYTES`), truncated cleanly at **14,000 chars** (`MAX_RESUME_CHARS`), non-metadata word cap `1,800` / line cap `160`.
- **Job Description (`set_job_description`)**: Saved independently from chat (`+ JD` / `JD Active` pill or `Ctrl + J` modal, 14,000 char cap) so users swap roles per interview without touching Settings.
- **Prompt Assembly (`build_interview_system_prompt`)**: Combines `DEFAULT_INTERVIEW_SYSTEM_PROMPT` (concise first-person `"I"` interviewee persona) + `[INTERVIEWEE RESUME CONTEXT — ...]` + `[TARGET JOB DESCRIPTION CONTEXT]`. Preserved across Settings saves unless explicitly cleared via `clear_resume` or JD Clear.

---

## [SEC-05] Live Voice Transcription Engine

### OOP Audio & Speech Architecture (`src-tauri/src/voice/`)
- **Capture & Resampling (`audio.rs`)**:
  - `cpal` opens either WASAPI loopback output capture (`"system"`, **default**) or microphone input (`"mic"`), with optional `device_name` match.
  - `Resampler16k` downmixes channels to mono and resamples continuously to **16,000 Hz `i16` PCM** (`VOSK_SAMPLE_RATE = 16_000`) with light box low-pass filtering when `step > 1.5`.
  - Transient `xrun` / stream errors are treated safely without tearing down unnecessarily.
- **Abstraction (`provider.rs`, `factory.rs`, `mod.rs`)**:
  - `SpeechProvider` trait implemented by `DeepgramSpeechProvider` (**default**) and `VoskSpeechProvider`.
  - `SpeechProviderFactory` resolves provider (`"vosk"` -> Vosk, otherwise Deepgram).
  - `TauriTranscriptSink` emits `vosk://transcript` (`{ kind: "partial"|"final", text, speech_final }`) and automatically sets `stop_flag = true` after any non-empty `final` + `speech_final` utterance (so listening stops cleanly after one spoken question until re-triggered).
- **Deepgram Cloud (`deepgram_provider.rs`)**:
  - Connects via `tokio_tungstenite` WebSocket to `wss://api.deepgram.com/v1/listen?model=<model>&language=<lang>&encoding=linear16&sample_rate=16000&channels=1&interim_results=true&punctuate=true&smart_format=true&endpointing=800&utterance_end_ms=1100` (`Authorization: Token <key>`).
  - Default model `nova-3` (options: `nova-3`, `nova-2`, `nova-3-general`, `nova-2-meeting`), default language `en-US` (`en-US`, `en`, `multi`).
- **Vosk Local (`ffi.rs`, `vosk_provider.rs`, `setup.rs`)**:
  - Dynamically loads `libvosk.dll` at runtime via `libloading` (`VoskApi`, `VoskModelHandle`, `VoskRecognizerHandle`) and caches loaded acoustic model in `Mutex`.
  - `setup_vosk` downloads official `vosk-win64-0.3.45.zip` + `vosk-model-small-en-us-0.15.zip` into `%LOCALAPPDATA%\Undetect\vosk\`, emitting `vosk://setup-progress`. Supports custom model/runtime directories.
- **Chat UX (`ChatView.svelte`)**:
  - Transcript appears in a dedicated `#voice-transcript-bubble` above the composer (final white text + italic partial), never clobbering manual textarea drafts.
  - Quick `System` / `Mic` toggle pill (`set_voice_source`) switches loopback vs mic seamlessly (restarts active capture if listening).
  - `auto_send` option (`VoiceSettingsCard.svelte` + `VoiceConfig.auto_send`) auto-sends transcripts on natural `speech_final` pause while suppressing auto-send if the user clicked manual Stop (`manualVoiceStop`).

---

## [SEC-06] LAN Remote Viewer & Axum HTTP Server

### Axum Server (`src-tauri/src/server.rs`)
- **Bind**: `0.0.0.0:<port>` (persisted `port` / `password` in `server_config.json`, defaults `9090` / `"1234"`). Starts automatically on desktop launch; controllable via **Remote Server** tab.
- **Auth (`check_auth`)**: Required on all `/api/*` endpoints when `password` is non-empty (`?pass=` or `?pin=`; returns `403` JSON `{"error":"Unauthorized"}`).
- **CORS**: Permissive (`Any` origin/methods/headers).
- **Routes**:
  - `GET /`, `/index.html`, `/viewer.html` -> serves `viewer.html`.
  - `GET /assets/{*path}` -> serves static JS/CSS/PNG/SVG/ICO (`max-age=31536000, immutable`).
  - `GET /api/session` -> `{ screenshots: [{ id, timestamp }] }`.
  - `GET /api/screenshot[?id=...&thumb=1]` -> `image/jpeg` (`no-store`).
  - `POST /api/capture` -> captures primary display into session queue.
  - `POST /api/session/clear` -> empties `state.screenshots`.
  - `GET|POST /api/llm/config`, `POST /api/llm/chat`, `POST /api/llm/ocr`, `GET /api/llm/groq-models`.
  - `GET /api/dev-reload?since=<ms>` -> long-poll viewer file mtime check.

### Mobile Web Viewer (`viewer.html`, `src/viewer/*`)
- **Screenshot-only UI**: Focused strictly on viewing/capturing/downloading screenshots (chat UI removed from remote viewer).
- **Branding**: Site `<title>undetect</title>` + favicon/apple-touch-icon (`logo.png`), top bar (`ViewerNav.svelte`) with `Live`/`Connecting`/`Locked` pill.
- **Controls (`ViewerApp.svelte`, `ViewerPreview.svelte`, `ViewerFilmstrip.svelte`)**:
  - Polls `/api/session` every 3s, auto-following latest screenshot if user was on the newest item.
  - Pinch-to-zoom, mouse wheel zoom (up to 6x), double-click/tap 2.5x toggle, 1:1 touch/mouse pan, floating zoom reset pill.
  - Bottom `160px`-thumb filmstrip, camera capture trigger, single download, staggered batch download (`220ms` interval), session clear confirmation modal, PIN modal persisted in `sessionStorage` (`undetect_pass`).

---

## [SEC-07] Keyboard Shortcuts & Tauri Event Bus

### Global Shortcuts (`GLOBAL_SHORTCUTS` in `src-tauri/src/lib.rs`, `src/lib/shortcuts.js`)
Work system-wide even when another window is focused:
- `Ctrl + Alt + H`: Show / hide Undetect window without stealing focus (`shortcut://visibility`).
- `Ctrl + Alt + K`: Show, restore, and focus chat input (`shortcut://focus-chat`).
- `Ctrl + Alt + Enter`: Answer current screen directly without moving mouse (`shortcut://answer-screen`).
- `Ctrl + Alt + V`: Start or stop live voice listening (`shortcut://voice-unconfigured` / `vosk://state`).
- `Ctrl + Alt + A`: Toggle screenshot attachment on chat messages (`shortcut://toggle-screen`).
- `Ctrl + Alt + G`: Toggle Click-Through Ghost Mode (`shortcut://click-through`).
- `Ctrl + Alt + P`: Toggle `WDA_EXCLUDEFROMCAPTURE` protection (`shortcut://protection`).
- `Ctrl + Alt + Shift + ↑ / ↓`: Scroll active chat/settings feed remotely (`shortcut://scroll`).
- `Ctrl + Alt + Shift + ← / →`: Resize window width `-/+ 60px` (`shortcut://resized`).
- `Ctrl + Alt + 0`: Reset window to default `460x710` centered (`shortcut://resized`).

### In-App Focused Shortcuts (`src/App.svelte`, `src/chat/ChatView.svelte`)
- `Enter` / `Shift + Enter`: Send message (or voice transcript if input empty) / newline.
- `Alt + Enter` / `Alt + Backspace`: Send or clear live voice transcript bubble.
- `Ctrl + J`: Open / edit Job Description modal.
- `Ctrl + ,`: Toggle Settings view.
- `Ctrl + K`: Focus Chat input.
- `Ctrl + ↑ / ↓`, `Alt + ↑ / ↓`, `PageUp / PageDown`: Scroll active feed.
- `Ctrl + [ / ]`, `Ctrl + 0`: Narrow/widen (`-/+ 60px`) or reset window bounds.
- `Esc`: Close What's New -> close Settings -> blur focused input.

### Emitted Tauri Events
- `llm://stream` (`{ stream_id, delta }`)
- `vosk://transcript` (`{ kind, text, speech_final }`), `vosk://state` (`bool`), `vosk://setup-progress` (`{ stage, percent, message }`), `vosk://error` (`string`)
- `shortcut://visibility`, `shortcut://focus-chat`, `shortcut://answer-screen`, `shortcut://scroll`, `shortcut://toggle-screen`, `shortcut://voice-unconfigured`, `shortcut://protection`, `shortcut://click-through`, `shortcut://resized`

---

## [SEC-08] Complete IPC Command & HTTP API Reference

### All 43 Registered Tauri Commands (`src-tauri/src/lib.rs`)
- **Server & Capture**: `get_status`, `start_server(port, password)`, `stop_server`, `capture_now`, `clear_session`, `generate_qr(text)`.
- **Window & Stealth**: `toggle_taskbar(hide)`, `toggle_content_protection(protect)`, `toggle_always_on_top(alwaysOnTop)`, `toggle_click_through(clickThrough)`, `set_window_opacity(opacity)`, `minimize_window`, `close_window`, `drag_window`, `get_window_size`, `set_window_size(width, height)`, `step_resize_window(dw, dh)`, `reset_window_size`, `nudge_window(dx, dy)`, `toggle_window_visibility`, `start_resize_drag(direction)`.
- **Telemetry & Updater**: `toggle_telemetry(enabled)`, `confirm_ui_boot`, `check_and_stage_ui_patch` (binary updates handled by `tauri-plugin-updater` + `tauri-plugin-process`).
- **LLM, Resume & OCR**: `send_llm_chat(request)`, `send_llm_chat_stream(request, streamId)`, `get_llm_config`, `set_llm_config(config)`, `clear_llm_api_key(provider)`, `upload_resume(filename, bytes)`, `clear_resume`, `set_job_description(jobDescription)`, `run_ocr(screenshotId)`, `get_groq_models(apiKey)`.
- **Voice**: `get_voice_status`, `set_voice_config(config)`, `set_voice_source(source)`, `clear_voice_api_key`, `list_voice_devices`, `setup_vosk`, `start_voice_input`, `stop_voice_input`, `toggle_voice_input`.

---

## [SEC-09] Persistence, AppData Paths & Fresh-Slate Reset

### Disk Storage (`%LOCALAPPDATA%\Undetect`)
- `undetect_llm_config.json`: LLM `active_provider`, masked-on-read API keys (`gemini_api_key`, `groq_api_key`), model IDs, `auto_ocr`, `system_prompt`, `resume_filename`/`pages`/`text`, `job_description`.
- `voice_config.json`: Voice `provider`, `source`, `device_name`, custom Vosk dirs, `deepgram_api_key`, `deepgram_model`, `deepgram_language`, `auto_send`.
- `server_config.json`: Persisted remote server `port` and `password`.
- `telemetry_config.json`: `{ "enabled": true|false }`; `telemetry_id` fallback file if Registry `MachineGuid` unavailable.
- `vosk/bin/` & `vosk/models/`: Downloaded `libvosk.dll` + MinGW DLLs and `vosk-model-small-en-us-0.15`.
- `ui_patches/`: Signed UI hot-patch state (`state.json`), staged downloads (`staging/`), and extracted Vite bundles (`bundles/<ui_version>/`).

### Browser Storage (`%LOCALAPPDATA%\com.undetect.desktop\EBWebView`)
- **`localStorage`**: `undetect_seen_version` (`major.minor` key gating What's New modal), `undetect_window_size`, `undetect_always_on_top`, `undetect_window_opacity`, `undetect_stealth_cursor`.
- **`sessionStorage`**: `undetect_chat_history`, `undetect_pass` (mobile viewer PIN).

### Reset Caveat & `reset-undetect.bat`
- NSIS `installMode: "currentUser"` also places installed binaries (`undetect.exe`, `uninstall.exe`) inside `%LOCALAPPDATA%\Undetect`. Never delete the entire directory indiscriminately.
- `reset-undetect.bat` (gitignored + `.gitingestignore`d; pass `/y` to run non-interactively) kills `undetect.exe`, deletes only the JSON config files, `telemetry_id`, `vosk/`, `ui_patches/`, `%TEMP%\undetect_vosk_*.zip`, and `EBWebView`, leaving binaries intact.

---

## [SEC-10] PostHog Telemetry & In-App Updater

### Telemetry (`src-tauri/src/telemetry.rs`)
- **Target**: PostHog US Cloud capture API `https://us.i.posthog.com/i/v0/e/` (5s HTTP timeout, async fire-and-forget via `tokio_rt()`, build-time override via `UNDETECT_POSTHOG_API_KEY`).
- **Identity**: Salted SHA-256 of Windows `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`:
  - Production (`!cfg!(debug_assertions)`): `ud_<32-hex>`, `env = "prod"`.
  - Development (`cfg!(debug_assertions)`): `ud_dev_<32-hex>`, `env = "dev"`.
- **Events**: `app_opened` (+2s after launch), `app_heartbeat` (every 10m), `chat_message_sent` (`provider`, `model`, `with_screen`), `voice_listening_started` (`provider`, `source`), `remote_server_started`.
- **Person Props (`$set`)**: `app_version`, `ui_version` (when hot-patched), `os`, `arch`, `last_env`, `llm_provider`, `voice_provider`, `has_resume`, `has_jd`, `remote_server_running`. Zero PII, keys, prompts, resumes, JDs, or screenshots ever leave the machine.
- **Opt-Out**: `Usage Stats` toggle in Settings -> Window persists `telemetry_config.json`.

### Hybrid Auto-Updater (`src/App.svelte`, `src-tauri/src/ui_patch.rs`, `src-tauri/src/lib.rs`)
- **Binary Track (`tauri-plugin-updater` + `tauri-plugin-process`)**: Queries `https://github.com/zigglyzuff/undetect/releases/latest/download/latest.json` (Ed25519 minisign verified via `pubkey` in `tauri.conf.json`), installs passive NSIS updates on click, and relaunches automatically.
- **UI-Only Track (`ui_patch.rs` + `scripts/build-ui-patch.mjs`)**: Silently checks `https://github.com/zigglyzuff/undetect/releases/download/ui-latest/ui-manifest.json` (rolling `prerelease: true` tag so `/releases/latest` stays pinned to binary releases), verifies Ed25519 signature + SHA-256 checksum, stages `dist.zip` into `%LOCALAPPDATA%\Undetect\ui_patches\`, applies on next app launch via `Context::set_assets` (`HotSwapAssets`) + `server.rs` disk override, and rolls back automatically after unconfirmed boots or binary upgrades.

---

## [SEC-11] Frontend UI, Tailwind v4 Caveats & Dead Files

### Active UI Structure (`src/`)
- **`App.svelte`**: Frameless root shell, 8 edge/corner resize grips, `TitleBar` (centered logo/version + `Ghost` badge + Settings & Exit buttons), `Tabs` switching `Chat` (`ChatView`) and `Remote Server` (`RemoteServerView`), `SettingsView` overlay, `WhatsNewModal`, `Toast`.
- **`SettingsView.svelte`**: 5 sections (`AI Model`, `Voice`, `Window`, `Server`, `Shortcuts`), inline update header button/banner, sticky bottom **Save All Settings** bar.
- **Shared Tailwind Utilities**:
  - `src/lib/dropdownClasses.js`: `DROPDOWN_CLASS`, `DROPDOWN_GROUP_CLASS`, `getDropdownItemClass`, `syncDropdownWidth` (matches dropdown menu width to trigger button, monochrome zinc-950/zinc-700 styling).
  - `src/lib/tooltip.js`: `TOOLTIP_CLASS`.

### Critical Tailwind v4 + Flowbite CSS Rule (`src/app.css`)
- **Do NOT place unlayered `[popover]` or `dialog::backdrop` rules in `app.css`**: In Tailwind v4, unlayered rules override `@layer utilities`, stripping `bg-*`, `px-*`, `border-*`, and `backdrop:*` off Flowbite `<Tooltip>` and `<Modal>`. Keep resets strictly inside `@layer base { ... }`.

### Legacy Unused Files (Safe to Ignore / Delete Later)
These v2.0.0 files remain in `src/components/` and are **not imported anywhere** in `v2.1.0`:
- `ActionButtons.svelte`, `LanAccessCard.svelte`, `ProtectionCard.svelte`, `QrModal.svelte`, `ServerConfigCard.svelte`.

---

## [SEC-12] Build, Test, CI & Release Engineering

### Commands & Tooling
- **Dev**: `npm run tauri dev` (Vite HMR on `:5173`, Rust debug build).
- **Frontend build**: `npm run build` (`vite build` -> `dist/`).
- **Rust lint**: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- **Unit tests**: `cargo test --manifest-path src-tauri/Cargo.toml` (`41` tests across `llm`, `voice`, `ocr`, `state`, `telemetry`, `ui_patch`, `win_util`, `shortcut_tests`).
- **Release bundle**: `CARGO_BUILD_JOBS=4` + `npm run tauri build` -> NSIS `currentUser` installer + MSI + updater `.sig` artifacts (`createUpdaterArtifacts: true`).

### Release Profile & CI Caveats
- `src-tauri/Cargo.toml` sets `[profile.release] strip = true`. Avoid enabling `lto = "thin"` + `codegen-units = 1` simultaneously on default thread counts on Windows (triggers MSVC/rustc virtual memory exhaustion `os error 1453`).
- `.github/workflows/release.yml` builds on `windows-latest` using `cargo test` + `tauri-apps/tauri-action@v0` on `v*` tags (`includeUpdaterJson: true`, `TAURI_SIGNING_PRIVATE_KEY` secret).
- `.github/workflows/ui-patch.yml` builds `dist/` on `ubuntu-latest` for `ui-v*` tags / manual dispatch, signs `ui-manifest.json` with `UI_PATCH_SIGNING_PRIVATE_KEY`, and updates the rolling `ui-latest` GitHub pre-release.
- **Code signing**: `tauri.conf.json` has no Authenticode `signCommand` / certificate configured yet (installers show Windows "Unverified Publisher" until signed via SignPath Foundation / Azure Artifact Signing / commercial cert).
