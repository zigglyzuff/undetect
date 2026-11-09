# Undetect v2.0.0 — The Rust & Svelte 5 Rewrite

Undetect v2.0.0 is a complete, ground-up rewrite that replaces the legacy Electron runtime with a high-performance **Rust (Tauri v2 + Axum)** architecture and a **Svelte 5** frontend. The result is a dramatically smaller binary (~10 MB vs. ~150 MB), near-zero idle RAM usage, instant startup, and a refined monochrome user experience.

---

## What's New

### ⚡ Rust & Tauri v2 Architecture
- **Rewritten from scratch**: Electron and Node.js backend have been completely replaced with a native Rust core powered by Tauri v2 and Axum.
- **Micro footprint**: RAM usage dropped from ~200 MB to under 25 MB; binary size reduced by over 90%.
- **Native Win32 Integration**: Implemented hardware-accelerated screen capture with automatic GDI fallback, seamless custom title bar window dragging, and native taskbar visibility controls.

### 🛡️ Enhanced Stealth & Cloaking
- **Screenshot Exclusion**: Automatically cloaks the application from desktop screen captures, screen recorders, and video conference screen sharing (`WDA_EXCLUDEFROMCAPTURE`).
- **Taskbar Cloaking**: Toggle to completely remove Undetect from the Windows taskbar while keeping background hotkeys active.
- **Silent Global Hotkey**: Press <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>S</kbd> anytime to capture the primary display silently without opening or focusing the app.

### 📱 Re-engineered Remote Viewer
- **Vite Multi-Page Architecture**: Extracted the remote viewer into an independent, modular Svelte 5 web app served directly over your local Wi-Fi.
- **In-Place Gesture Zoom & Pan**:
  - Multi-touch pinch-to-zoom on mobile devices.
  - Smooth mouse wheel zoom on desktop browsers.
  - Double-tap / double-click to toggle 2.5× zoom.
  - Direct 1:1 panning while zoomed in.
  - Subtle floating reset badge to return to fit screen in one tap.
- **Ergonomic Camera Dock**:
  - Prominent camera-style shutter button for instant captures.
  - Thumb-reach single download and batch download buttons.
  - Session clear action with confirmation modal.
- **100× Faster Filmstrip Reel**:
  - Dedicated lightweight thumbnail pipeline generating ~3 KB previews (JPEG quality 65) instead of loading multi-megabyte full captures for the reel.
  - Smooth horizontal scrolling with soft edge fade gradients.
  - Non-cliché, minimalist live sync indicator in the header.

### 🎨 Unified Monochrome Design
- Consistent, modern dark glassmorphism design language across both the desktop application and remote mobile viewer.
- Custom-sized modals for QR code scanning, host PIN authentication, and What's New highlights that fit the desktop window gracefully with zero right-side spacing issues.

---

## Downloads & Artifacts

| Asset | Description | Size | Location |
| :--- | :--- | :--- | :--- |
| **`undetect_2.0.0_x64-setup.exe`** *(Recommended)* | Windows NSIS Setup Installer (Installs app, adds Start Menu/Desktop shortcuts, supports auto-updates from older versions) | **2.8 MB** | `E:\cargo_targets\undetect\release\bundle\nsis\undetect_2.0.0_x64-setup.exe` |
| **`undetect.exe`** | Standalone Portable Windows Executable (No installation required) | **11.2 MB** | `E:\cargo_targets\undetect\release\undetect.exe` |
| **`undetect_2.0.0_x64_en-US.msi`** | Windows Installer Package (MSI) | **4.0 MB** | `E:\cargo_targets\undetect\release\bundle\msi\undetect_2.0.0_x64_en-US.msi` |

---

## Older Version Auto-Update Compatibility
If you are upgrading from an older v1.x installation, clicking **Update** inside the older app will automatically fetch `undetect_2.0.0_x64-setup.exe` from this release, launch the setup wizard, and seamlessly upgrade your installation.
