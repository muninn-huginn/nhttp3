//! WASM bindings for nhttp3.
//!
//! Browser target: Uses WebTransport API as the underlying transport,
//! with nhttp3 handling HTTP/3 framing on top.
//!
//! Non-browser WASM: Full stack runs where UDP socket access is available
//! (e.g., Cloudflare Workers, Deno).

use wasm_bindgen::prelude::*;

mod config;
mod frame;

/// Initialize the WASM module. Call once at startup.
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Returns the library version.
#[wasm_bindgen]
pub fn version() -> String {
    "0.1.0".to_string()
}
