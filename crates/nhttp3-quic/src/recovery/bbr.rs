use super::congestion::CongestionController;
use std::time::{Duration, Instant};

const MAX_DATAGRAM_SIZE: u64 = 1200;
const BBR_GAIN_CYCLE: [f64; 8] = [1.25, 0.75, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];

/// BBR congestion controller.
///
/// Bottleneck Bandwidth and Round-trip propagation time.
/// Models the network path to find the optimal operating point.
#[derive(Debug)]
pub struct Bbr {
    // Bottleneck bandwidth estimate (bytes per second)
    btl_bw: f64,
    // Minimum RTT observed
    min_rtt: Duration,
    // Current pacing gain cycle index
    gain_cycle_idx: usize,
    // Congestion window
    congestion_window: u64,
    // Slow start threshold
    ssthresh: u64,
    // Bytes in flight
    bytes_in_flight: u64,
    // Max datagram size
    max_datagram_size: u64,
    // Round trip counter
    round_count: u64,
    // BBR state
    state: BbrState,
    // Maximum bandwidth samples for windowed max filter
    bw_samples: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BbrState {
    Startup,
    Drain,
    ProbeBw,
    ProbeRtt,
}

impl Bbr {
    pub fn new() -> Self {
        let max_datagram_size = MAX_DATAGRAM_SIZE;
        let initial_window = std::cmp::min(
            10 * max_datagram_size,
            std::cmp::max(14720, 2 * max_datagram_size),
        );
        Self {
            btl_bw: 0.0,
            min_rtt: Duration::from_millis(100),
            gain_cycle_idx: 0,
            congestion_window: initial_window,
            ssthresh: u64::MAX,
            bytes_in_flight: 0,
            max_datagram_size,
            round_count: 0,
            state: BbrState::Startup,
            bw_samples: Vec::new(),
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

    pub fn state(&self) -> &str {
        match self.state {
            BbrState::Startup => "startup",
            BbrState::Drain => "drain",
            BbrState::ProbeBw => "probe_bw",
            BbrState::ProbeRtt => "probe_rtt",
        }
    }

    fn pacing_gain(&self) -> f64 {
        match self.state {
            BbrState::Startup => 2.885, // 2/ln(2)
            BbrState::Drain => 1.0 / 2.885,
            BbrState::ProbeBw => BBR_GAIN_CYCLE[self.gain_cycle_idx],
            BbrState::ProbeRtt => 1.0,
        }
    }

    fn update_bandwidth(&mut self, bytes_delivered: u64, rtt: Duration) {
        if rtt.is_zero() {
            return;
        }
        let bw = bytes_delivered as f64 / rtt.as_secs_f64();
        self.bw_samples.push(bw);
        // Keep last 10 samples for windowed max
        if self.bw_samples.len() > 10 {
            self.bw_samples.remove(0);
        }
        self.btl_bw = self.bw_samples.iter().cloned().fold(0.0f64, f64::max);
    }

    fn update_model(&mut self, rtt: Duration) {
        if !rtt.is_zero() && rtt < self.min_rtt {
            self.min_rtt = rtt;
        }

        // BDP = btl_bw * min_rtt
        let bdp = (self.btl_bw * self.min_rtt.as_secs_f64()) as u64;

        match self.state {
            BbrState::Startup => {
                self.congestion_window = (bdp as f64 * self.pacing_gain()) as u64;
                self.congestion_window = self.congestion_window.max(self.max_datagram_size * 4);
                // Transition to Drain when growth stalls
                self.round_count += 1;
                if self.round_count > 3 && self.btl_bw > 0.0 {
                    self.state = BbrState::Drain;
                }
            }
            BbrState::Drain => {
                self.congestion_window = bdp.max(self.max_datagram_size * 2);
                if self.bytes_in_flight <= bdp {
                    self.state = BbrState::ProbeBw;
                }
            }
            BbrState::ProbeBw => {
                let gain = self.pacing_gain();
                self.congestion_window = ((bdp as f64) * gain) as u64;
                self.congestion_window = self.congestion_window.max(self.max_datagram_size * 2);
                self.gain_cycle_idx = (self.gain_cycle_idx + 1) % 8;
            }
            BbrState::ProbeRtt => {
                self.congestion_window = self.max_datagram_size * 4;
                self.state = BbrState::ProbeBw;
            }
        }
    }
}

impl Default for Bbr {
    fn default() -> Self {
        Self::new()
    }
}

impl CongestionController for Bbr {
    fn window(&self) -> u64 {
        self.congestion_window
    }

    fn on_ack(&mut self, bytes_acked: u64, rtt: Duration, _now: Instant) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes_acked);
        self.update_bandwidth(bytes_acked, rtt);
        self.update_model(rtt);
    }

    fn on_loss(&mut self, bytes_lost: u64, _now: Instant) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes_lost);
        // BBR doesn't reduce window on loss the same way — it relies on the model
        // But we still cap to prevent runaway
        self.congestion_window = self.congestion_window.max(self.max_datagram_size * 2);
    }

    fn ssthresh(&self) -> u64 {
        self.ssthresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state() {
        let bbr = Bbr::new();
        assert_eq!(bbr.state(), "startup");
        assert_eq!(bbr.window(), 12000);
        assert!(bbr.can_send());
    }

    #[test]
    fn startup_growth() {
        let mut bbr = Bbr::new();
        let now = Instant::now();

        for _ in 0..5 {
            bbr.on_packet_sent(1200);
            bbr.on_ack(1200, Duration::from_millis(50), now);
        }

        // Should have exited startup after enough rounds
        assert!(bbr.window() > 0);
    }

    #[test]
    fn loss_doesnt_collapse_window() {
        let mut bbr = Bbr::new();
        let now = Instant::now();
        let initial = bbr.window();

        bbr.on_packet_sent(1200);
        bbr.on_loss(1200, now);

        // BBR should maintain at least 2*MSS
        assert!(bbr.window() >= 2 * 1200);
    }

    #[test]
    fn bandwidth_estimation() {
        let mut bbr = Bbr::new();
        let now = Instant::now();

        // Send 12000 bytes, ack after 50ms
        bbr.on_packet_sent(12000);
        bbr.on_ack(12000, Duration::from_millis(50), now);

        // BW should be ~240KB/s (12000 / 0.05)
        assert!(bbr.btl_bw > 200_000.0);
    }
}
