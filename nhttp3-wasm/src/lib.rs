//! WASM bindings for nhttp3.
//!
//! Browser: WebTransport as transport, nhttp3 for HTTP/3 framing.
//! Node.js: Import as ESM, use for QPACK/frame encoding.
//! Cloudflare Workers / Deno: Full stack where UDP sockets are available.

use wasm_bindgen::prelude::*;

mod config;
mod frame;
mod headers;

/// Initialize the WASM module.
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
