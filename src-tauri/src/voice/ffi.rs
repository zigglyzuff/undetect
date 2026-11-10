use std::ffi::{c_char, c_float, c_int, c_void, CStr, CString};
use std::path::Path;
use std::sync::Arc;

pub struct VoskApi {
    _lib: libloading::Library,
    vosk_model_new: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    vosk_model_free: unsafe extern "C" fn(*mut c_void),
    vosk_recognizer_new: unsafe extern "C" fn(*mut c_void, c_float) -> *mut c_void,
    vosk_recognizer_free: unsafe extern "C" fn(*mut c_void),
    vosk_recognizer_accept_waveform: unsafe extern "C" fn(*mut c_void, *const u8, c_int) -> c_int,
    vosk_recognizer_result: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    vosk_recognizer_partial_result: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    vosk_recognizer_final_result: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    vosk_recognizer_reset: unsafe extern "C" fn(*mut c_void),
}

impl VoskApi {
    pub fn load(dll_path: &Path) -> Result<Arc<Self>, String> {
        #[cfg(windows)]
        {
            if let Some(parent) = dll_path.parent() {
                use windows::core::HSTRING;
                use windows::Win32::System::LibraryLoader::SetDllDirectoryW;
                let hstr = HSTRING::from(parent.as_os_str());
                let _ = unsafe { SetDllDirectoryW(&hstr) };
            }
        }

        let lib = unsafe { libloading::Library::new(dll_path) }
            .map_err(|e| format!("Failed to load libvosk library from {:?}: {}", dll_path, e))?;

        unsafe {
            let vosk_set_log_level: unsafe extern "C" fn(c_int) = *lib
                .get(b"vosk_set_log_level\0")
                .map_err(|e| format!("Missing vosk_set_log_level: {}", e))?;
            let vosk_model_new = *lib
                .get(b"vosk_model_new\0")
                .map_err(|e| format!("Missing vosk_model_new: {}", e))?;
            let vosk_model_free = *lib
                .get(b"vosk_model_free\0")
                .map_err(|e| format!("Missing vosk_model_free: {}", e))?;
            let vosk_recognizer_new = *lib
                .get(b"vosk_recognizer_new\0")
                .map_err(|e| format!("Missing vosk_recognizer_new: {}", e))?;
            let vosk_recognizer_free = *lib
                .get(b"vosk_recognizer_free\0")
                .map_err(|e| format!("Missing vosk_recognizer_free: {}", e))?;
            let vosk_recognizer_accept_waveform = *lib
                .get(b"vosk_recognizer_accept_waveform\0")
                .map_err(|e| format!("Missing vosk_recognizer_accept_waveform: {}", e))?;
            let vosk_recognizer_result = *lib
                .get(b"vosk_recognizer_result\0")
                .map_err(|e| format!("Missing vosk_recognizer_result: {}", e))?;
            let vosk_recognizer_partial_result = *lib
                .get(b"vosk_recognizer_partial_result\0")
                .map_err(|e| format!("Missing vosk_recognizer_partial_result: {}", e))?;
            let vosk_recognizer_final_result = *lib
                .get(b"vosk_recognizer_final_result\0")
                .map_err(|e| format!("Missing vosk_recognizer_final_result: {}", e))?;
            let vosk_recognizer_reset = *lib
                .get(b"vosk_recognizer_reset\0")
                .map_err(|e| format!("Missing vosk_recognizer_reset: {}", e))?;

            // Silence verbose internal Kaldi stderr spam
            vosk_set_log_level(-1);

            Ok(Arc::new(Self {
                _lib: lib,
                vosk_model_new,
                vosk_model_free,
                vosk_recognizer_new,
                vosk_recognizer_free,
                vosk_recognizer_accept_waveform,
                vosk_recognizer_result,
                vosk_recognizer_partial_result,
                vosk_recognizer_final_result,
                vosk_recognizer_reset,
            }))
        }
    }
}

pub struct VoskModelHandle {
    api: Arc<VoskApi>,
    raw: *mut c_void,
}

unsafe impl Send for VoskModelHandle {}
unsafe impl Sync for VoskModelHandle {}

impl VoskModelHandle {
    pub fn new(api: Arc<VoskApi>, model_path: &Path) -> Result<Self, String> {
        let path_str = model_path
            .to_str()
            .ok_or_else(|| "Model path contains invalid characters".to_string())?;
        let c_path = CString::new(path_str)
            .map_err(|e| format!("Invalid CString path: {}", e))?;

        let raw = unsafe { (api.vosk_model_new)(c_path.as_ptr()) };
        if raw.is_null() {
            return Err(format!(
                "Failed to initialize Vosk model from {:?}. Ensure folder contains valid Vosk model files.",
                model_path
            ));
        }

        Ok(Self { api, raw })
    }
}

impl Drop for VoskModelHandle {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                (self.api.vosk_model_free)(self.raw);
            }
            self.raw = std::ptr::null_mut();
        }
    }
}

pub struct VoskRecognizerHandle {
    _model: Arc<VoskModelHandle>,
    api: Arc<VoskApi>,
    raw: *mut c_void,
}

unsafe impl Send for VoskRecognizerHandle {}

impl VoskRecognizerHandle {
    pub fn new(model: Arc<VoskModelHandle>, sample_rate: f32) -> Result<Self, String> {
        let api = Arc::clone(&model.api);
        let raw = unsafe { (api.vosk_recognizer_new)(model.raw, sample_rate as c_float) };
        if raw.is_null() {
            return Err("Failed to instantiate Vosk recognizer".to_string());
        }

        Ok(Self {
            _model: model,
            api,
            raw,
        })
    }

    /// Feeds PCM 16-bit mono LE samples into Vosk.
    /// Returns `true` if a full phrase boundary was detected (final result available).
    pub fn accept_waveform(&mut self, samples: &[i16]) -> bool {
        if samples.is_empty() {
            return false;
        }
        let byte_slice: &[u8] = unsafe {
            std::slice::from_raw_parts(
                samples.as_ptr() as *const u8,
                std::mem::size_of_val(samples),
            )
        };
        let res = unsafe {
            (self.api.vosk_recognizer_accept_waveform)(
                self.raw,
                byte_slice.as_ptr(),
                byte_slice.len() as c_int,
            )
        };
        res != 0
    }

    pub fn get_result_text(&self) -> String {
        let ptr = unsafe { (self.api.vosk_recognizer_result)(self.raw) };
        parse_vosk_json(ptr, "text")
    }

    pub fn get_partial_text(&self) -> String {
        let ptr = unsafe { (self.api.vosk_recognizer_partial_result)(self.raw) };
        parse_vosk_json(ptr, "partial")
    }

    pub fn get_final_text(&self) -> String {
        let ptr = unsafe { (self.api.vosk_recognizer_final_result)(self.raw) };
        parse_vosk_json(ptr, "text")
    }

    pub fn reset(&mut self) {
        unsafe {
            (self.api.vosk_recognizer_reset)(self.raw);
        }
    }
}

impl Drop for VoskRecognizerHandle {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                (self.api.vosk_recognizer_free)(self.raw);
            }
            self.raw = std::ptr::null_mut();
        }
    }
}

fn parse_vosk_json(c_buf: *const c_char, field_name: &str) -> String {
    if c_buf.is_null() {
        return String::new();
    }
    let c_str = unsafe { CStr::from_ptr(c_buf) };
    let Ok(slice) = c_str.to_str() else {
        return String::new();
    };
    extract_vosk_field(slice, field_name)
}

pub fn extract_vosk_field(json_str: &str, field_name: &str) -> String {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str)
        && let Some(txt) = val.get(field_name).and_then(|v| v.as_str())
    {
        return txt.trim().to_string();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_partial_and_final_from_vosk_json() {
        assert_eq!(
            extract_vosk_field("{\"partial\" : \"hello world\"}", "partial"),
            "hello world"
        );
        assert_eq!(
            extract_vosk_field("{\"text\" : \"breadth first search in python\"}", "text"),
            "breadth first search in python"
        );
        assert_eq!(extract_vosk_field("{\"partial\" : \"\"}", "partial"), "");
        assert_eq!(extract_vosk_field("{}", "text"), "");
    }
}
