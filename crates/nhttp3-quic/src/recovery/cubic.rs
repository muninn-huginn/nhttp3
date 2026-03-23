use super::congestion::CongestionController;
use std::time::{Duration, Instant};

const MAX_DATAGRAM_SIZE: u64 = 1200;
const BETA_CUBIC: f64 = 0.7;
const C: f64 = 0.4;

/// CUBIC congestion controller.
///
/// Implements the CUBIC algorithm for better performance on high-bandwidth,
/// high-latency networks compared to NewReno.
#[derive(Debug)]
pub struct Cubic {
    congestion_window: u64,
    ssthresh: u64,
    bytes_in_flight: u64,
    max_datagram_size: u64,
    /// Window size just before the last congestion event.
    w_max: f64,
    /// Time of the last congestion event.
    epoch_start: Option<Instant>,
    /// Estimated RTT for calculations.
    min_rtt: Duration,
}

impl Cubic {
    pub fn new() -> Self {
        let max_datagram_size = MAX_DATAGRAM_SIZE;
        let initial_window = std::cmp::min(
            10 * max_datagram_size,
            std::cmp::max(14720, 2 * max_datagram_size),
        );
        Self {
            congestion_window: initial_window,
            ssthresh: u64::MAX,
            bytes_in_flight: 0,
            max_datagram_size,
            w_max: 0.0,
            epoch_start: None,
            min_rtt: Duration::from_millis(100),
        }
    }

    pub fn bytes_in_flight(&self) -> u64 {
        self.bytes_in_flight
    }

    pub fn on_packet_sent(&mut self, bytes: u64) {
        self.bytes_in_flight += bytes;
    }

    pub fn can_send(&self) -> bool {
        self.bytes_in_flight < self.congestion_window
    }

    fn cubic_window(&self, t: f64) -> f64 {
        let k = (self.w_max * (1.0 - BETA_CUBIC) / C).cbrt();
        C * (t - k).powi(3) + self.w_max
    }
}

impl Default for Cubic {
    fn default() -> Self {
        Self::new()
    }
}

impl CongestionController for Cubic {
    fn window(&self) -> u64 {
        self.congestion_window
    }

    fn on_ack(&mut self, bytes_acked: u64, rtt: Duration, now: Instant) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes_acked);

        if !rtt.is_zero() && rtt < self.min_rtt {
            self.min_rtt = rtt;
        }

        if self.congestion_window < self.ssthresh {
            // Slow start
            self.congestion_window += bytes_acked;
            return;
        }

        // Congestion avoidance — CUBIC
        let epoch = self.epoch_start.get_or_insert(now);
        let t = now.duration_since(*epoch).as_secs_f64();

        let w_cubic = self.cubic_window(t) * self.max_datagram_size as f64;
        let target = w_cubic.max(self.congestion_window as f64) as u64;

        // Increase towards target
        if target > self.congestion_window {
            let increase = ((target - self.congestion_window) as f64
                * self.max_datagram_size as f64
                / self.congestion_window as f64) as u64;
            self.congestion_window += increase.max(1);
        } else {
            // TCP-friendly region: linear increase
            self.congestion_window +=
                (self.max_datagram_size * bytes_acked) / self.congestion_window;
        }
    }

    fn on_loss(&mut self, bytes_lost: u64, now: Instant) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes_lost);
        self.w_max = self.congestion_window as f64 / self.max_datagram_size as f64;
        self.ssthresh =
            ((self.congestion_window as f64 * BETA_CUBIC) as u64).max(2 * self.max_datagram_size);
        self.congestion_window = self.ssthresh;
        self.epoch_start = Some(now);
    }

    fn ssthresh(&self) -> u64 {
        self.ssthresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_window() {
        let cc = Cubic::new();
        assert_eq!(cc.window(), 12000);
        assert!(cc.can_send());
    }

    #[test]
    fn slow_start_growth() {
        let mut cc = Cubic::new();
        let initial = cc.window();
        cc.on_packet_sent(1200);
        cc.on_ack(1200, Duration::from_millis(50), Instant::now());
        assert!(cc.window() > initial);
    }

    #[test]
    fn loss_reduces_window() {
        let mut cc = Cubic::new();
        let initial = cc.window();
        cc.on_packet_sent(1200);
        cc.on_loss(1200, Instant::now());
        assert!(cc.window() < initial);
        // CUBIC uses beta=0.7, so window should be ~70% of original
        assert!(cc.window() >= (initial as f64 * 0.6) as u64);
    }

    #[test]
    fn cubic_growth_after_loss() {
        let mut cc = Cubic::new();
        let now = Instant::now();

        // Trigger loss to enter congestion avoidance
        cc.on_packet_sent(1200);
        cc.on_loss(1200, now);
        let ca_window = cc.window();

        // ACK in congestion avoidance
        cc.on_packet_sent(1200);
        cc.on_ack(1200, Duration::from_millis(50), now);
        assert!(cc.window() >= ca_window);
    }

    #[test]
    fn minimum_window() {
        let mut cc = Cubic::new();
        for _ in 0..30 {
            cc.on_packet_sent(1200);
            cc.on_loss(1200, Instant::now());
        }
        assert!(cc.window() >= 2 * 1200);
    }
}
