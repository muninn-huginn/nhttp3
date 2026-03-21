use wasm_bindgen::prelude::*;

/// QUIC/HTTP3 configuration for WASM consumers.
#[wasm_bindgen]
pub struct Config {
    max_idle_timeout_ms: u32,
    initial_max_data: u64,
    initial_max_streams_bidi: u64,
    initial_max_streams_uni: u64,
}

#[wasm_bindgen]
impl Config {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            max_idle_timeout_ms: 30_000,
            initial_max_data: 10_000_000,
            initial_max_streams_bidi: 100,
            initial_max_streams_uni: 100,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn max_idle_timeout_ms(&self) -> u32 {
        self.max_idle_timeout_ms
    }

    #[wasm_bindgen(setter)]
    pub fn set_max_idle_timeout_ms(&mut self, ms: u32) {
        self.max_idle_timeout_ms = ms;
    }

    #[wasm_bindgen(getter)]
    pub fn initial_max_data(&self) -> u64 {
        self.initial_max_data
    }

    #[wasm_bindgen(setter)]
    pub fn set_initial_max_data(&mut self, val: u64) {
        self.initial_max_data = val;
    }

    #[wasm_bindgen(getter)]
    pub fn initial_max_streams_bidi(&self) -> u64 {
        self.initial_max_streams_bidi
    }

    #[wasm_bindgen(setter)]
    pub fn set_initial_max_streams_bidi(&mut self, val: u64) {
        self.initial_max_streams_bidi = val;
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
