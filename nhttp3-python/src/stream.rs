use pyo3::prelude::*;
use pyo3::Py;

use crate::async_bridge;

#[pyclass]
#[derive(Clone)]
pub struct SendStream {
    stream_id: u64,
    buffer: Vec<u8>,
    finished: bool,
}

impl SendStream {
    pub fn new(stream_id: u64) -> Self {
        Self {
            stream_id,
            buffer: Vec::new(),
            finished: false,
        }
    }
}

#[pymethods]
impl SendStream {
    fn write(&mut self, py: Python<'_>, data: &[u8]) -> PyResult<Py<PyAny>> {
        self.buffer.extend_from_slice(data);
        let n = data.len();
        async_bridge::spawn_and_resolve(py, async move { Ok(n) })
    }

    fn finish(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.finished = true;
        async_bridge::spawn_and_resolve(py, async move { Ok(true) })
    }

    fn reset(&mut self, _error_code: u64) {
        self.finished = true;
    }

    #[getter]
    fn stream_id(&self) -> u64 {
        self.stream_id
    }

    fn __repr__(&self) -> String {
        format!("SendStream(id={})", self.stream_id)
    }
}

#[pyclass]
#[derive(Clone)]
pub struct RecvStream {
    stream_id: u64,
    buffer: Vec<u8>,
    fin: bool,
}

impl RecvStream {
    pub fn new(stream_id: u64) -> Self {
        Self {
            stream_id,
            buffer: Vec::new(),
            fin: false,
        }
    }
}

#[pymethods]
impl RecvStream {
    #[pyo3(signature = (max_bytes=65536))]
    fn read(&mut self, py: Python<'_>, max_bytes: usize) -> PyResult<Py<PyAny>> {
        let n = std::cmp::min(max_bytes, self.buffer.len());
        let data: Vec<u8> = self.buffer.drain(..n).collect();
        async_bridge::spawn_and_resolve(py, async move { Ok(data) })
    }

    fn stop(&mut self, _error_code: u64) {}

    #[getter]
    fn stream_id(&self) -> u64 {
        self.stream_id
    }

    fn __repr__(&self) -> String {
        format!("RecvStream(id={})", self.stream_id)
    }
}
