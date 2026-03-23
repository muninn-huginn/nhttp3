use pyo3::prelude::*;
use pyo3::Py;

use crate::async_bridge;
use crate::stream::{RecvStream, SendStream};

#[pyclass]
#[derive(Clone)]
pub struct Connection {
    remote_addr: String,
    server_name: Option<String>,
    is_client: bool,
    established: bool,
    next_stream_id: u64,
}

impl Connection {
    pub fn new_client(remote_addr: String, server_name: String) -> Self {
        Self {
            remote_addr,
            server_name: Some(server_name),
            is_client: true,
            established: true,
            next_stream_id: 0,
        }
    }

    pub fn new_server(remote_addr: String) -> Self {
        Self {
            remote_addr,
            server_name: None,
            is_client: false,
            established: true,
            next_stream_id: 1,
        }
    }
}

#[pymethods]
impl Connection {
    fn open_bidi_stream(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let sid = self.next_stream_id;
        self.next_stream_id += 4;
        async_bridge::spawn_and_resolve(py, async move {
            Ok((SendStream::new(sid), RecvStream::new(sid)))
        })
    }

    fn open_uni_stream(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let sid = self.next_stream_id + 2;
        self.next_stream_id += 4;
        async_bridge::spawn_and_resolve(py, async move { Ok(SendStream::new(sid)) })
    }

    fn remote_addr(&self) -> &str {
        &self.remote_addr
    }
    fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }
    fn is_established(&self) -> bool {
        self.established
    }

    #[pyo3(signature = (error_code=0, reason=None))]
    fn close(&mut self, error_code: u64, reason: Option<String>) {
        self.established = false;
    }

    fn __repr__(&self) -> String {
        format!(
            "Connection({}, {})",
            self.remote_addr,
            if self.is_client { "client" } else { "server" }
        )
    }
}
