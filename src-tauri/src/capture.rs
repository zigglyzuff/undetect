use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};
use xcap::Monitor;

use crate::state::{AppState, ScreenshotEntry, ScreenshotMeta, MAX_SESSION_SCREENSHOTS};

#[cfg(windows)]
fn capture_screen_gdi() -> Result<image::RgbaImage, String> {
    use windows::core::w;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    unsafe {
        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);
        if width <= 0 || height <= 0 {
            return Err("Invalid screen metrics".to_string());
        }

        // Create DC for the primary display
        let hdc_screen = CreateDCW(w!("DISPLAY"), None, None, None);
        let hdc_screen = if !hdc_screen.is_invalid() {
            hdc_screen
        } else {
            GetDC(HWND::default())
        };

        if hdc_screen.is_invalid() {
            return Err("Failed to get screen DC".to_string());
        }

        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        let old_obj = SelectObject(hdc_mem, hbitmap);

        let res = BitBlt(
            hdc_mem,
            0,
            0,
            width,
            height,
            hdc_screen,
            0,
            0,
            SRCCOPY,
        );

        SelectObject(hdc_mem, old_obj);
        let _ = DeleteDC(hdc_screen);

        if let Err(e) = res {
            let _ = DeleteDC(hdc_mem);
            let _ = DeleteObject(hbitmap);
            return Err(format!("BitBlt failed: {:?}", e));
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut raw_bytes = vec![0u8; (width * height * 4) as usize];
        let lines = GetDIBits(
            hdc_mem,
            hbitmap,
            0,
            height as u32,
            Some(raw_bytes.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        let _ = DeleteDC(hdc_mem);
        let _ = DeleteObject(hbitmap);

        if lines == 0 {
            return Err("GetDIBits failed".to_string());
        }

        // Windows GDI gives BGRA; swap to RGBA
        for chunk in raw_bytes.chunks_exact_mut(4) {
            let b = chunk[0];
            let r = chunk[2];
            chunk[0] = r;
            chunk[2] = b;
            chunk[3] = 255;
        }

        image::RgbaImage::from_raw(width as u32, height as u32, raw_bytes)
            .ok_or_else(|| "Failed to construct RgbaImage from GDI buffer".to_string())
    }
}

pub fn capture_primary_screen(state: &AppState) -> Result<ScreenshotMeta, String> {
    // Attempt capture using xcap first
    let mut captured_img = None;

    if let Ok(monitors) = Monitor::all() {
        let target_mon = monitors
            .iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .or_else(|| monitors.first());

        if let Some(monitor) = target_mon {
            match monitor.capture_image() {
                Ok(img) => captured_img = Some(img),
                Err(e) => eprintln!("[UndetectCapture] xcap error: {}", e),
            }
        }
    }

    // If xcap was unavailable or failed (e.g. DXGI Desktop Duplication handle invalid), use GDI
    #[cfg(windows)]
    if captured_img.is_none() {
        match capture_screen_gdi() {
            Ok(img) => captured_img = Some(img),
            Err(e) => eprintln!("[UndetectCapture] GDI capture fallback error: {}", e),
        }
    }

    let rgba_img = captured_img.ok_or_else(|| "Failed to capture display image".to_string())?;

    let (w, h) = (rgba_img.width(), rgba_img.height());
    let max_w: u32 = 1920;

    let target_img = if w > max_w {
        let target_h = ((h as f64) * (max_w as f64 / w as f64)).round() as u32;
        image::imageops::resize(&rgba_img, max_w, target_h, image::imageops::FilterType::Triangle)
    } else {
        rgba_img
    };

    let mut jpeg_buf = Cursor::new(Vec::new());
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, 85);
    encoder
        .encode_image(&target_img)
        .map_err(|e| format!("JPEG encoding failed: {}", e))?;

    let jpeg_data = jpeg_buf.into_inner();

    // Generate lightweight thumbnail (width: 160px) for filmstrip reel (~3-5 KB instead of ~500 KB)
    let thumb_w: u32 = 160;
    let thumb_h = ((target_img.height() as f64) * (thumb_w as f64 / target_img.width() as f64)).round().max(1.0) as u32;
    let thumb_img = image::imageops::resize(&target_img, thumb_w, thumb_h, image::imageops::FilterType::Triangle);
    let mut thumb_buf = Cursor::new(Vec::new());
    let mut thumb_encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut thumb_buf, 65);
    let thumb_data = match thumb_encoder.encode_image(&thumb_img) {
        Ok(_) => thumb_buf.into_inner(),
        Err(_) => Vec::new(),
    };

    let now = SystemTime::now();
    let epoch_duration = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let id = epoch_duration.as_millis().to_string();

    let total_secs = epoch_duration.as_secs();
    let s = total_secs % 60;
    let m = (total_secs / 60) % 60;
    let h = (total_secs / 3600) % 24;
    let timestamp = format!("{:02}:{:02}:{:02}", h, m, s);

    let entry = ScreenshotEntry {
        id: id.clone(),
        timestamp: timestamp.clone(),
        jpeg_data,
        thumb_data,
    };

    let meta = ScreenshotMeta {
        id: id.clone(),
        timestamp: timestamp.clone(),
    };

    {
        let mut list = state.screenshots.write().map_err(|e| e.to_string())?;
        list.push_back(entry);
        if list.len() > MAX_SESSION_SCREENSHOTS {
            list.pop_front();
        }
    }

    println!("[UndetectCapture] Captured screen successfully ({})", timestamp);
    Ok(meta)
}
