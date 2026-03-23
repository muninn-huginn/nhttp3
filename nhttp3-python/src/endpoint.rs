use pyo3::prelude::*;
use pyo3::Py;

use crate::async_bridge;
use crate::config::Config;
use crate::connection::Connection;

#[pyclass]
pub struct Endpoint {
    local_addr: String,
    port: u16,
    config: Config,
    _active: bool,
}

#[pymethods]
impl Endpoint {
    #[staticmethod]
    #[pyo3(signature = (host, port, config=None))]
    fn bind(
        py: Python<'_>,
        host: String,
        port: u16,
        config: Option<Config>,
    ) -> PyResult<Py<PyAny>> {
        let config = config.unwrap_or_else(Config::default_config);
        async_bridge::spawn_and_resolve(py, async move {
            Ok(Endpoint {
                local_addr: host,
                port,
                config,
                _active: true,
            })
        })
    }

    fn accept(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let addr = format!("{}:{}", self.local_addr, self.port);
        async_bridge::spawn_and_resolve(py, async move { Ok(Connection::new_server(addr)) })
    }

    #[pyo3(signature = (host, port, server_name=None))]
    fn connect(
        &self,
        py: Python<'_>,
        host: String,
        port: u16,
        server_name: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let sni = server_name.unwrap_or_else(|| host.clone());
        let addr = format!("{host}:{port}");
        async_bridge::spawn_and_resolve(py, async move { Ok(Connection::new_client(addr, sni)) })
    }

    fn local_addr(&self) -> String {
        format!("{}:{}", self.local_addr, self.port)
    }

    fn close(&mut self) {
        self._active = false;
    }

    fn __repr__(&self) -> String {
        format!("Endpoint({}:{})", self.local_addr, self.port)
    }
}
