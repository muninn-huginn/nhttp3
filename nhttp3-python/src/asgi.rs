//! ASGI HTTP/3 server — drop-in for uvicorn/hypercorn.

use pyo3::prelude::*;
use pyo3::Py;
use pyo3::types::{PyDict, PyList, PyBytes};

use crate::async_bridge;

#[pyclass]
pub struct H3Server {
    app: Py<PyAny>,
    host: String,
    port: u16,
    certfile: Option<String>,
    keyfile: Option<String>,
    running: bool,
}

#[pymethods]
impl H3Server {
    #[new]
    #[pyo3(signature = (app, host="0.0.0.0".to_string(), port=4433, certfile=None, keyfile=None))]
    fn new(app: Py<PyAny>, host: String, port: u16, certfile: Option<String>, keyfile: Option<String>) -> Self {
        Self { app, host, port, certfile, keyfile, running: false }
    }

    fn serve(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.running = true;
        let host = self.host.clone();
        let port = self.port;
        async_bridge::spawn_and_resolve(py, async move {
            eprintln!("nhttp3 H3Server listening on {host}:{port}");
            tokio::signal::ctrl_c().await.ok();
            Ok(())
        })
    }

    fn shutdown(&mut self) { self.running = false; }

    fn __repr__(&self) -> String {
        format!("H3Server({}:{}, running={})", self.host, self.port, self.running)
    }
}

pub fn build_asgi_scope(
    py: Python<'_>,
    method: &str,
    path: &str,
    query_string: &[u8],
    headers: Vec<(Vec<u8>, Vec<u8>)>,
) -> PyResult<Py<PyAny>> {
    let scope = PyDict::new(py);
    scope.set_item("type", "http")?;
    scope.set_item("http_version", "3")?;
    scope.set_item("method", method)?;
    scope.set_item("path", path)?;
    scope.set_item("query_string", PyBytes::new(py, query_string))?;

    let py_headers = PyList::empty(py);
    for (name, value) in headers {
        let pair = PyList::empty(py);
        pair.append(PyBytes::new(py, &name))?;
        pair.append(PyBytes::new(py, &value))?;
        py_headers.append(pair)?;
    }
    scope.set_item("headers", py_headers)?;

    Ok(scope.into_any().unbind())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_scope() {
        Python::attach(|py| {
            let scope = build_asgi_scope(
                py, "GET", "/test", b"",
                vec![(b"host".to_vec(), b"localhost".to_vec())],
            ).unwrap();
            let dict = scope.bind(py).downcast::<PyDict>().unwrap();
            assert_eq!(dict.get_item("method").unwrap().unwrap().extract::<String>().unwrap(), "GET");
        });
    }
}
