use std::collections::HashSet;
use nhttp3_core::ConnectionId;

/// Manages connection ID retirement (RFC 9000 §5.1.2).
///
/// Tracks which CID sequence numbers have been retired to detect
/// and handle duplicate RETIRE_CONNECTION_ID frames gracefully.
///
/// Bug reference: quiche #1833 — Duplicate RETIRE_CONNECTION_ID leads
/// to connection close with PROTOCOL_VIOLATION.
#[derive(Debug)]
pub struct CidRetirementTracker {
    /// Sequence numbers that have been retired.
    retired: HashSet<u64>,
    /// Maximum sequence number we've issued.
    max_issued: u64,
}

impl CidRetirementTracker {
    pub fn new() -> Self {
        Self {
            retired: HashSet::new(),
            max_issued: 0,
        }
    }

    /// Records that we issued a new CID with this sequence number.
    pub fn on_cid_issued(&mut self, sequence: u64) {
        if sequence > self.max_issued {
            self.max_issued = sequence;
        }
    }

    /// Processes a RETIRE_CONNECTION_ID frame.
    /// Returns Ok(true) if this is a new retirement, Ok(false) if duplicate,
    /// Err if the sequence number is invalid.
    pub fn on_retire(&mut self, sequence: u64) -> Result<bool, RetireError> {
        if sequence > self.max_issued {
            return Err(RetireError::SequenceNotIssued(sequence));
        }
        if self.retired.contains(&sequence) {
            return Ok(false); // Duplicate — ignore per quiche #1833 fix
        }
        self.retired.insert(sequence);
        Ok(true)
    }

    pub fn is_retired(&self, sequence: u64) -> bool {
        self.retired.contains(&sequence)
    }

    pub fn retired_count(&self) -> usize {
        self.retired.len()
    }
}

impl Default for CidRetirementTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RetireError {
    #[error("sequence {0} was never issued")]
    SequenceNotIssued(u64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retire_valid_sequence() {
        let mut tracker = CidRetirementTracker::new();
        tracker.on_cid_issued(0);
        tracker.on_cid_issued(1);
        assert_eq!(tracker.on_retire(0).unwrap(), true);
        assert!(tracker.is_retired(0));
    }

    #[test]
    fn duplicate_retire_returns_false() {
        let mut tracker = CidRetirementTracker::new();
        tracker.on_cid_issued(0);
        assert_eq!(tracker.on_retire(0).unwrap(), true);
        assert_eq!(tracker.on_retire(0).unwrap(), false); // duplicate — no error
    }

    #[test]
    fn retire_unissued_sequence_errors() {
        let mut tracker = CidRetirementTracker::new();
        tracker.on_cid_issued(0);
        assert!(tracker.on_retire(5).is_err());
    }
}
