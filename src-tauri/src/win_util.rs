#[cfg(windows)]
pub fn set_content_protection(window: &tauri::WebviewWindow, protect: bool) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WINDOW_DISPLAY_AFFINITY};

    let hwnd_raw = window.hwnd().map_err(|e| e.to_string())?;
    let hwnd = HWND(hwnd_raw.0 as _);

    // WDA_NONE = 0x00000000, WDA_EXCLUDEFROMCAPTURE = 0x00000011 (Windows 10 2004+)
    let affinity = if protect {
        WINDOW_DISPLAY_AFFINITY(0x00000011)
    } else {
        WINDOW_DISPLAY_AFFINITY(0)
    };

    unsafe {
        SetWindowDisplayAffinity(hwnd, affinity)
            .map_err(|e| format!("SetWindowDisplayAffinity failed: {:?}", e))?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_content_protection(_window: &tauri::WebviewWindow, _protect: bool) -> Result<(), String> {
    Ok(())
}
