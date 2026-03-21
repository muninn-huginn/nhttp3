//! Python bindings for nhttp3.
//!
//! Uses PyO3 with a custom async bridge (no pyo3-asyncio dependency).
//! A background thread runs the tokio runtime, and completions are
//! posted to the Python event loop via `loop.call_soon_threadsafe()`.

use pyo3::prelude::*;

mod config;

/// The nhttp3 Python module.
#[pymodule]
fn _nhttp3(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<config::Config>()?;
    m.add("__version__", "0.1.0")?;
    Ok(())
}
