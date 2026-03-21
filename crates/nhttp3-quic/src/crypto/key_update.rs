use rustls::quic::Secrets;

use super::SpaceKeys;

/// Manages 1-RTT key updates (RFC 9001 §6).
///
/// After the handshake, endpoints can update their 1-RTT keys by
/// deriving new keys from the current traffic secrets.
#[derive(Debug)]
pub struct KeyUpdateState {
    /// Current key phase (alternates between 0 and 1).
    key_phase: bool,
    /// Number of key updates performed.
    update_count: u64,
}

impl KeyUpdateState {
    pub fn new() -> Self {
        Self {
            key_phase: false,
            update_count: 0,
        }
    }

    /// Returns the current key phase bit.
    pub fn key_phase(&self) -> bool {
        self.key_phase
    }

    /// Records that a key update was performed.
    pub fn on_key_update(&mut self) {
        self.key_phase = !self.key_phase;
        self.update_count += 1;
    }

    /// Returns the number of key updates performed.
    pub fn update_count(&self) -> u64 {
        self.update_count
    }
}

impl Default for KeyUpdateState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_key_phase() {
        let ks = KeyUpdateState::new();
        assert!(!ks.key_phase());
        assert_eq!(ks.update_count(), 0);
    }

    #[test]
    fn key_update_toggles_phase() {
        let mut ks = KeyUpdateState::new();
        ks.on_key_update();
        assert!(ks.key_phase());
        assert_eq!(ks.update_count(), 1);

        ks.on_key_update();
        assert!(!ks.key_phase());
        assert_eq!(ks.update_count(), 2);
    }
}
