//! C-compatible FFI layer for nhttp3.
//!
//! Provides opaque handle types and C-ABI functions for consuming nhttp3
//! from other languages (primarily Python via PyO3).

pub mod error;
pub mod runtime;
pub mod types;

pub use error::Nhttp3Error;
pub use runtime::Runtime;
