use serde::Serialize;

pub const DEFAULT_WINDOW_WIDTH: f64 = 460.0;
pub const DEFAULT_WINDOW_HEIGHT: f64 = 710.0;
pub const MIN_WINDOW_WIDTH: f64 = 360.0;
pub const MIN_WINDOW_HEIGHT: f64 = 480.0;

#[derive(Clone, Serialize)]
pub struct WindowSizeDto {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy)]
struct LogicalWorkArea {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

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

#[cfg(windows)]
pub fn set_window_opacity(window: &tauri::WebviewWindow, opacity_percent: u8) -> Result<u8, String> {
    use windows::Win32::Foundation::{COLORREF, HWND};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, GWL_EXSTYLE, LWA_ALPHA,
        WS_EX_LAYERED,
    };

    // Keep opacity inside a reasonable, readable 40% - 100% range
    let clamped = opacity_percent.clamp(40, 100);
    let alpha = ((clamped as u16 * 255) / 100) as u8;

    let hwnd_raw = window.hwnd().map_err(|e| e.to_string())?;
    let hwnd = HWND(hwnd_raw.0 as _);

    unsafe {
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let layered_flag = WS_EX_LAYERED.0 as i32;

        if clamped >= 100 {
            if (ex_style & layered_flag) != 0 {
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
                SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style & !layered_flag);
            }
        } else {
            if (ex_style & layered_flag) == 0 {
                SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | layered_flag);
            }
            SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA)
                .map_err(|e| format!("SetLayeredWindowAttributes failed: {:?}", e))?;
        }
    }

    Ok(clamped)
}

#[cfg(not(windows))]
pub fn set_window_opacity(_window: &tauri::WebviewWindow, opacity_percent: u8) -> Result<u8, String> {
    Ok(opacity_percent.clamp(40, 100))
}

fn current_work_area(window: &tauri::WebviewWindow, scale: f64) -> LogicalWorkArea {
    if let Ok(Some(monitor)) = window.current_monitor() {
        let area = monitor.work_area();
        return LogicalWorkArea {
            x: (area.position.x as f64) / scale,
            y: (area.position.y as f64) / scale,
            width: ((area.size.width as f64) / scale).max(MIN_WINDOW_WIDTH),
            height: ((area.size.height as f64) / scale).max(MIN_WINDOW_HEIGHT),
        };
    }
    LogicalWorkArea {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    }
}

pub fn compute_clamped_logical_size(
    target_w: f64,
    target_h: f64,
    max_w: f64,
    max_h: f64,
) -> (u32, u32) {
    let clamped_w = target_w
        .clamp(MIN_WINDOW_WIDTH, max_w.max(MIN_WINDOW_WIDTH))
        .round() as u32;
    let clamped_h = target_h
        .clamp(MIN_WINDOW_HEIGHT, max_h.max(MIN_WINDOW_HEIGHT))
        .round() as u32;
    (clamped_w, clamped_h)
}

fn compute_clamped_position(
    target_x: f64,
    target_y: f64,
    win_w: f64,
    win_h: f64,
    area: LogicalWorkArea,
) -> (f64, f64) {
    let max_x = (area.x + area.width - win_w).max(area.x);
    let max_y = (area.y + area.height - win_h).max(area.y);
    (
        target_x.clamp(area.x, max_x).round(),
        target_y.clamp(area.y, max_y).round(),
    )
}

/// Returns current logical window dimensions adjusted for display DPI scale.
pub fn get_window_logical_size(window: &tauri::WebviewWindow) -> Result<WindowSizeDto, String> {
    let scale = window.scale_factor().unwrap_or(1.0);
    let phys = window.inner_size().map_err(|e| e.to_string())?;
    let logical = phys.to_logical::<f64>(scale);
    Ok(WindowSizeDto {
        width: logical.width.round() as u32,
        height: logical.height.round() as u32,
    })
}

/// Resizes the window in logical pixels, clamped to minimum bounds and monitor work area.
pub fn set_window_logical_size(
    window: &tauri::WebviewWindow,
    target_width: f64,
    target_height: f64,
) -> Result<WindowSizeDto, String> {
    let scale = window.scale_factor().unwrap_or(1.0);
    let area = current_work_area(window, scale);
    let (w, h) = compute_clamped_logical_size(target_width, target_height, area.width, area.height);

    window
        .set_size(tauri::Size::Logical(tauri::LogicalSize::new(w as f64, h as f64)))
        .map_err(|e| format!("Failed to resize window: {}", e))?;

    // Pull window back inside the work area if growing expanded past right/bottom edges
    if let Ok(phys_pos) = window.outer_position() {
        let log_pos = phys_pos.to_logical::<f64>(scale);
        let (nx, ny) = compute_clamped_position(log_pos.x, log_pos.y, w as f64, h as f64, area);
        if (nx - log_pos.x).abs() >= 1.0 || (ny - log_pos.y).abs() >= 1.0 {
            let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(nx, ny)));
        }
    }

    Ok(WindowSizeDto { width: w, height: h })
}

/// Increments or decrements current logical window size by `(dw, dh)`.
pub fn step_resize_window(
    window: &tauri::WebviewWindow,
    dw: f64,
    dh: f64,
) -> Result<WindowSizeDto, String> {
    let current = get_window_logical_size(window)?;
    set_window_logical_size(window, current.width as f64 + dw, current.height as f64 + dh)
}

/// Restores default window dimensions (`460x710`) and re-centers on the current monitor.
pub fn reset_window_bounds(window: &tauri::WebviewWindow) -> Result<WindowSizeDto, String> {
    let size = set_window_logical_size(window, DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)?;
    let _ = window.center();
    Ok(size)
}

/// Moves the window by `(dx, dy)` logical pixels within the current monitor work area.
pub fn nudge_window_position(
    window: &tauri::WebviewWindow,
    dx: f64,
    dy: f64,
) -> Result<(), String> {
    let scale = window.scale_factor().unwrap_or(1.0);
    let area = current_work_area(window, scale);
    let current_size = get_window_logical_size(window)?;
    let phys_pos = window.outer_position().map_err(|e| e.to_string())?;
    let log_pos = phys_pos.to_logical::<f64>(scale);

    let (nx, ny) = compute_clamped_position(
        log_pos.x + dx,
        log_pos.y + dy,
        current_size.width as f64,
        current_size.height as f64,
        area,
    );

    window
        .set_position(tauri::Position::Logical(tauri::LogicalPosition::new(nx, ny)))
        .map_err(|e| format!("Failed to move window: {}", e))
}

/// Toggles window visibility without altering foreground focus of other open applications.
pub fn toggle_window_visibility(
    window: &tauri::WebviewWindow,
    always_on_top: bool,
) -> Result<bool, String> {
    let visible = window.is_visible().unwrap_or(true);
    let minimized = window.is_minimized().unwrap_or(false);

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, ShowWindow, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE,
        };

        let hwnd_raw = window.hwnd().map_err(|e| e.to_string())?;
        let hwnd = HWND(hwnd_raw.0 as _);

        if visible && !minimized {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            return Ok(false);
        }

        let insert_after = if always_on_top {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(
                hwnd,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
            .map_err(|e| format!("SetWindowPos failed: {:?}", e))?;
        }
        Ok(true)
    }

    #[cfg(not(windows))]
    {
        if visible && !minimized {
            window
                .hide()
                .map_err(|e| format!("Failed to hide window: {}", e))?;
            Ok(false)
        } else {
            window
                .show()
                .map_err(|e| format!("Failed to show window: {}", e))?;
            Ok(true)
        }
    }
}

/// Shows and activates the window so keyboard input goes directly into Undetect.
pub fn focus_window(window: &tauri::WebviewWindow, always_on_top: bool) -> Result<(), String> {
    let _ = window.unminimize();
    window
        .show()
        .map_err(|e| format!("Failed to show window: {}", e))?;
    let _ = window.set_always_on_top(always_on_top);
    window
        .set_focus()
        .map_err(|e| format!("Failed to focus window: {}", e))
}

/// Initiates native borderless window resize dragging from an edge or corner handle.
pub fn start_resize_drag(window: &tauri::WebviewWindow, direction: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
        use windows::Win32::UI::WindowsAndMessaging::{
            SendMessageW, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTLEFT, HTRIGHT, HTTOP,
            HTTOPLEFT, HTTOPRIGHT, WM_NCLBUTTONDOWN,
        };

        let hit_test = match direction {
            "n" => HTTOP,
            "s" => HTBOTTOM,
            "w" => HTLEFT,
            "e" => HTRIGHT,
            "nw" => HTTOPLEFT,
            "ne" => HTTOPRIGHT,
            "sw" => HTBOTTOMLEFT,
            "se" => HTBOTTOMRIGHT,
            _ => return Err(format!("Unsupported resize direction: {}", direction)),
        };

        if let Ok(hwnd_raw) = window.hwnd() {
            let hwnd = HWND(hwnd_raw.0 as _);
            unsafe {
                let _ = ReleaseCapture();
                SendMessageW(hwnd, WM_NCLBUTTONDOWN, WPARAM(hit_test as usize), LPARAM(0));
            }
            return Ok(());
        }
        Err("Window handle unavailable".to_string())
    }

    #[cfg(not(windows))]
    {
        let _ = (window, direction);
        Err("Native resize drag fallback unavailable on this platform".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamped_size_respects_min_and_monitor_bounds() {
        assert_eq!(compute_clamped_logical_size(120.0, 200.0, 1920.0, 1080.0), (360, 480));
        assert_eq!(compute_clamped_logical_size(5000.0, 3000.0, 1440.0, 900.0), (1440, 900));
        assert_eq!(compute_clamped_logical_size(540.4, 770.6, 1920.0, 1080.0), (540, 771));
    }

    #[test]
    fn clamped_position_keeps_window_inside_work_area() {
        let area = LogicalWorkArea {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1040.0,
        };
        assert_eq!(compute_clamped_position(-100.0, 20.0, 460.0, 710.0, area), (0.0, 20.0));
        assert_eq!(compute_clamped_position(1800.0, 500.0, 460.0, 710.0, area), (1460.0, 330.0));
    }
}
