# Undetect

A lightweight, stealthy screen capture utility and local Wi-Fi remote viewer for Windows.

Capture your desktop silently with a hotkey, exclude the app window from screenshots and screen shares, and view high-resolution captures in real time on your phone or any browser across your local network.

---

## Features

- **Window Cloaking**: Automatically excludes itself from screenshots, screen recorders, and video calls (`WDA_EXCLUDEFROMCAPTURE`). Can also be hidden from the Windows taskbar.
- **Silent Global Hotkey**: Press <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>S</kbd> to capture your primary display silently in the background at any time.
- **Mobile Remote Viewer**: Open the viewer on your phone by scanning a QR code or navigating to your LAN IP. Protected with an optional PIN passcode.
- **In-Place Gesture Zoom**: Pinch-to-zoom, mouse wheel, double-tap to 2.5× zoom, and smooth panning directly inside the viewer.
- **Ergonomic Camera Dock**: Camera-style bottom shutter button, quick download, and batch download positioned right at thumb reach.
- **Optimized Thumbnail Engine**: Generates lightweight ~3 KB previews for the filmstrip reel, saving 99% bandwidth and memory on mobile.
- **Built with Rust & Tauri v2**: Replaced Electron with a lean, modern stack (Tauri v2, Axum, Svelte 5, Tailwind CSS). Instant startup with minimal RAM usage.

---

## Quick Start

1. Download `undetect.exe` from the [Latest Release](https://github.com/zigglyzuff/undetect/releases/latest).
2. Launch the app.
3. Configure an access PIN (optional) and port, then click **Update & Start**.
4. Click **QR** under Local Wi-Fi Access URLs and scan the code with your phone camera.
5. Take captures using the desktop button or press <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>S</kbd>.

---

## Development & Building

### Prerequisites
- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (v1.80+)

### Setup
```bash
# Clone repository
git clone https://github.com/zigglyzuff/undetect.git
cd undetect

# Install dependencies
npm install

# Run frontend + backend in development mode
npm run tauri dev

# Build production executable
npm run tauri build
```

The compiled binary will be located at `src-tauri/target/release/undetect.exe`.

---

## License

MIT License. See [LICENSE](LICENSE) for details.
