//! Python bindings for nhttp3.
//!
//! Custom async bridge: background tokio thread + call_soon_threadsafe completions.
//! No pyo3-asyncio dependency.

use pyo3::prelude::*;

mod asgi;
mod async_bridge;
mod config;
mod connection;
mod endpoint;
mod stream;

#[pymodule]
fn _nhttp3(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<config::Config>()?;
    m.add_class::<endpoint::Endpoint>()?;
    m.add_class::<connection::Connection>()?;
    m.add_class::<stream::SendStream>()?;
    m.add_class::<stream::RecvStream>()?;
    m.add_class::<asgi::H3Server>()?;
    m.add("__version__", "0.1.0")?;
    Ok(())
}
