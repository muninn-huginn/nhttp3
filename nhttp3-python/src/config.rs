use pyo3::prelude::*;
use std::time::Duration;

/// QUIC/HTTP3 configuration.
///
/// Example:
///     config = nhttp3.Config()
///     config.max_idle_timeout = 30.0
///     config.initial_max_streams_bidi = 100
#[pyclass]
#[derive(Debug, Clone)]
pub struct Config {
    #[pyo3(get, set)]
    pub max_idle_timeout: f64,
    #[pyo3(get, set)]
    pub initial_max_data: u64,
    #[pyo3(get, set)]
    pub initial_max_stream_data_bidi_local: u64,
    #[pyo3(get, set)]
    pub initial_max_stream_data_bidi_remote: u64,
    #[pyo3(get, set)]
    pub initial_max_stream_data_uni: u64,
    #[pyo3(get, set)]
    pub initial_max_streams_bidi: u64,
    #[pyo3(get, set)]
    pub initial_max_streams_uni: u64,
    #[pyo3(get, set)]
    pub enable_0rtt: bool,
}

impl Config {
    pub fn default_config() -> Self {
        Self {
            max_idle_timeout: 30.0,
            initial_max_data: 10_000_000,
            initial_max_stream_data_bidi_local: 1_000_000,
            initial_max_stream_data_bidi_remote: 1_000_000,
            initial_max_stream_data_uni: 1_000_000,
            initial_max_streams_bidi: 100,
            initial_max_streams_uni: 100,
            enable_0rtt: false,
        }
    }
}

#[pymethods]
impl Config {
    #[new]
    fn new() -> Self {
        Self {
            max_idle_timeout: 30.0,
            initial_max_data: 10_000_000,
            initial_max_stream_data_bidi_local: 1_000_000,
            initial_max_stream_data_bidi_remote: 1_000_000,
            initial_max_stream_data_uni: 1_000_000,
            initial_max_streams_bidi: 100,
            initial_max_streams_uni: 100,
            enable_0rtt: false,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Config(max_idle_timeout={}, max_data={}, max_streams_bidi={})",
            self.max_idle_timeout, self.initial_max_data, self.initial_max_streams_bidi
        )
    }
}

impl Config {
    /// Converts to the Rust QUIC config.
    pub fn to_quic_config(&self) -> nhttp3_quic::config::Config {
        nhttp3_quic::config::Config {
            max_idle_timeout: Duration::from_secs_f64(self.max_idle_timeout),
            initial_max_data: self.initial_max_data,
            initial_max_stream_data_bidi_local: self.initial_max_stream_data_bidi_local,
            initial_max_stream_data_bidi_remote: self.initial_max_stream_data_bidi_remote,
            initial_max_stream_data_uni: self.initial_max_stream_data_uni,
            initial_max_streams_bidi: self.initial_max_streams_bidi,
            initial_max_streams_uni: self.initial_max_streams_uni,
            active_connection_id_limit: 8,
            enable_0rtt: self.enable_0rtt,
            congestion_algorithm: nhttp3_quic::config::CongestionAlgorithm::NewReno,
        }
    }
}
