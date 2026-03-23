use super::congestion::CongestionController;
use std::time::{Duration, Instant};

const MAX_DATAGRAM_SIZE: u64 = 1200;

/// NewReno congestion controller (RFC 9002 §7).
#[derive(Debug)]
pub struct NewReno {
    congestion_window: u64,
    ssthresh: u64,
    bytes_in_flight: u64,
    max_datagram_size: u64,
}

impl NewReno {
    pub fn new() -> Self {
        let max_datagram_size = MAX_DATAGRAM_SIZE;
        // RFC 9002 §7.2: min(10 * max_datagram_size, max(14720, 2 * max_datagram_size))
        let initial_window = std::cmp::min(
            10 * max_datagram_size,
            std::cmp::max(14720, 2 * max_datagram_size),
        );
        Self {
            congestion_window: initial_window,
            ssthresh: u64::MAX,
            bytes_in_flight: 0,
            max_datagram_size,
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

    pub fn available(&self) -> u64 {
        self.congestion_window.saturating_sub(self.bytes_in_flight)
    }
}

impl Default for NewReno {
    fn default() -> Self {
        Self::new()
    }
}

impl CongestionController for NewReno {
    fn window(&self) -> u64 {
        self.congestion_window
    }

    fn on_ack(&mut self, bytes_acked: u64, _rtt: Duration, _now: Instant) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes_acked);

        if self.congestion_window < self.ssthresh {
            // Slow start
            self.congestion_window += bytes_acked;
        } else {
            // Congestion avoidance
            self.congestion_window +=
                (self.max_datagram_size * bytes_acked) / self.congestion_window;
        }
    }

    fn on_loss(&mut self, bytes_lost: u64, _now: Instant) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes_lost);
        self.ssthresh = (self.congestion_window / 2).max(2 * self.max_datagram_size);
        self.congestion_window = self.ssthresh;
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
        let cc = NewReno::new();
        // RFC 9002 §7.2: min(10 * 1200, max(14720, 2 * 1200)) = min(12000, 14720) = 12000
        assert_eq!(cc.window(), 12000);
        assert!(cc.can_send());
    }

    #[test]
    fn slow_start_growth() {
        let mut cc = NewReno::new();
        let initial = cc.window();
        cc.on_packet_sent(1200);
        cc.on_ack(1200, Duration::from_millis(50), Instant::now());
        assert!(cc.window() > initial);
    }

    #[test]
    fn loss_reduces_window() {
        let mut cc = NewReno::new();
        let initial = cc.window();
        cc.on_packet_sent(1200);
        cc.on_loss(1200, Instant::now());
        assert!(cc.window() < initial);
        assert!(cc.ssthresh() < u64::MAX);
    }

    #[test]
    fn congestion_avoidance_growth() {
        let mut cc = NewReno::new();
        cc.on_packet_sent(1200);
        cc.on_loss(1200, Instant::now());
        let ca_window = cc.window();
        cc.on_packet_sent(1200);
        cc.on_ack(1200, Duration::from_millis(50), Instant::now());
        let growth = cc.window() - ca_window;
        assert!(growth < 1200);
    }

    #[test]
    fn minimum_window() {
        let mut cc = NewReno::new();
        for _ in 0..20 {
            cc.on_packet_sent(1200);
            cc.on_loss(1200, Instant::now());
        }
        // RFC 9002 §7.2: minimum window is 2 * max_datagram_size
        assert!(cc.window() >= 2 * 1200);
    }

    #[test]
    fn bytes_in_flight_tracking() {
        let mut cc = NewReno::new();
        assert_eq!(cc.bytes_in_flight(), 0);
        cc.on_packet_sent(1200);
        assert_eq!(cc.bytes_in_flight(), 1200);
        cc.on_ack(1200, Duration::from_millis(50), Instant::now());
        assert_eq!(cc.bytes_in_flight(), 0);
    }
}
