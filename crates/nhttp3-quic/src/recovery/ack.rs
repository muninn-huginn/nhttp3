use std::collections::BTreeSet;
use std::time::Instant;

/// Tracks received packet numbers for ACK generation.
#[derive(Debug)]
pub struct AckTracker {
    received: BTreeSet<u64>,
    largest_received: Option<u64>,
    largest_received_time: Option<Instant>,
    ack_eliciting_received: bool,
}

impl AckTracker {
    pub fn new() -> Self {
        Self {
            received: BTreeSet::new(),
            largest_received: None,
            largest_received_time: None,
            ack_eliciting_received: false,
        }
    }

    pub fn on_packet_received(&mut self, pn: u64, ack_eliciting: bool, now: Instant) {
        self.received.insert(pn);
        if self.largest_received.map_or(true, |l| pn > l) {
            self.largest_received = Some(pn);
            self.largest_received_time = Some(now);
        }
        if ack_eliciting {
            self.ack_eliciting_received = true;
        }
    }

    pub fn should_send_ack(&self) -> bool {
        self.ack_eliciting_received
    }

    pub fn on_ack_sent(&mut self) {
        self.ack_eliciting_received = false;
    }

    pub fn largest_received(&self) -> Option<u64> {
        self.largest_received
    }
}

impl Default for AckTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tracker() {
        let tracker = AckTracker::new();
        assert!(!tracker.should_send_ack());
        assert!(tracker.largest_received().is_none());
    }

    #[test]
    fn single_packet() {
        let mut tracker = AckTracker::new();
        tracker.on_packet_received(0, true, Instant::now());
        assert!(tracker.should_send_ack());
        assert_eq!(tracker.largest_received(), Some(0));
    }

    #[test]
    fn non_ack_eliciting_does_not_trigger() {
        let mut tracker = AckTracker::new();
        tracker.on_packet_received(0, false, Instant::now());
        assert!(!tracker.should_send_ack());
    }

    #[test]
    fn ack_sent_clears_flag() {
        let mut tracker = AckTracker::new();
        tracker.on_packet_received(0, true, Instant::now());
        assert!(tracker.should_send_ack());
        tracker.on_ack_sent();
        assert!(!tracker.should_send_ack());
    }

    #[test]
    fn largest_tracks_correctly() {
        let mut tracker = AckTracker::new();
        let now = Instant::now();
        tracker.on_packet_received(5, true, now);
        tracker.on_packet_received(3, true, now);
        tracker.on_packet_received(10, true, now);
        assert_eq!(tracker.largest_received(), Some(10));
    }
}
