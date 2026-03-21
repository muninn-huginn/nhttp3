use std::time::{Duration, Instant};

/// Congestion controller trait — pluggable algorithm.
pub trait CongestionController: Send + Sync {
    fn window(&self) -> u64;
    fn on_ack(&mut self, bytes_acked: u64, rtt: Duration, now: Instant);
    fn on_loss(&mut self, bytes_lost: u64, now: Instant);
    fn ssthresh(&self) -> u64;
}
