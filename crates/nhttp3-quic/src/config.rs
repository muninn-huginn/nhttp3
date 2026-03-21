use std::time::Duration;

/// QUIC endpoint configuration.
#[derive(Clone, Debug)]
pub struct Config {
    pub max_idle_timeout: Duration,
    pub initial_max_data: u64,
    pub initial_max_stream_data_bidi_local: u64,
    pub initial_max_stream_data_bidi_remote: u64,
    pub initial_max_stream_data_uni: u64,
    pub initial_max_streams_bidi: u64,
    pub initial_max_streams_uni: u64,
    pub active_connection_id_limit: u64,
    /// Enable 0-RTT early data (requires session resumption).
    pub enable_0rtt: bool,
    /// Congestion control algorithm.
    pub congestion_algorithm: CongestionAlgorithm,
}

/// Available congestion control algorithms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CongestionAlgorithm {
    NewReno,
    Cubic,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_idle_timeout: Duration::from_secs(30),
            initial_max_data: 10_000_000,
            initial_max_stream_data_bidi_local: 1_000_000,
            initial_max_stream_data_bidi_remote: 1_000_000,
            initial_max_stream_data_uni: 1_000_000,
            initial_max_streams_bidi: 100,
            initial_max_streams_uni: 100,
            active_connection_id_limit: 8,
            enable_0rtt: false,
            congestion_algorithm: CongestionAlgorithm::NewReno,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = Config::default();
        assert_eq!(config.max_idle_timeout, Duration::from_secs(30));
        assert_eq!(config.initial_max_streams_bidi, 100);
    }
}
