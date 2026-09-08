//! Tiny console helpers for the browser (WASM-only). These never panic and
//! never throw; they are the only place we talk to the devtools console.

use wasm_bindgen::JsValue;

/// Log a warning to the browser console.
pub fn warn(message: &str) {
    web_sys::console::warn_1(&JsValue::from_str(message));
}

/// Log an error to the browser console.
pub fn error(message: &str) {
    web_sys::console::error_1(&JsValue::from_str(message));
}
