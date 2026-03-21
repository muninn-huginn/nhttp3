/// QUIC connection state (RFC 9000 §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Initial,
    Handshake,
    Established,
    Closing,
    Draining,
    Closed,
}

impl ConnectionState {
    pub fn can_send_app_data(&self) -> bool {
        matches!(self, Self::Established)
    }

    pub fn can_open_streams(&self) -> bool {
        matches!(self, Self::Established)
    }

    pub fn is_closed(&self) -> bool {
        matches!(self, Self::Closed)
    }

    pub fn is_closing(&self) -> bool {
        matches!(self, Self::Closing | Self::Draining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_cannot_send_data() {
        assert!(!ConnectionState::Initial.can_send_app_data());
        assert!(!ConnectionState::Initial.can_open_streams());
    }

    #[test]
    fn established_state_can_send_data() {
        assert!(ConnectionState::Established.can_send_app_data());
        assert!(ConnectionState::Established.can_open_streams());
    }

    #[test]
    fn closing_states() {
        assert!(ConnectionState::Closing.is_closing());
        assert!(ConnectionState::Draining.is_closing());
        assert!(!ConnectionState::Established.is_closing());
    }

    #[test]
    fn closed_state() {
        assert!(ConnectionState::Closed.is_closed());
        assert!(!ConnectionState::Established.is_closed());
    }
}
