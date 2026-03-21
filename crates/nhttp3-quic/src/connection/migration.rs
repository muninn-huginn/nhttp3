use std::net::SocketAddr;
use std::time::Instant;

/// State for path validation during connection migration (RFC 9000 §9).
#[derive(Debug)]
pub struct PathValidator {
    /// The new path being validated.
    new_path: SocketAddr,
    /// Challenge data we sent.
    challenge_data: [u8; 8],
    /// When the challenge was sent.
    sent_at: Instant,
    /// Whether validation is complete.
    validated: bool,
}

impl PathValidator {
    /// Creates a new path validator for the given remote address.
    pub fn new(new_path: SocketAddr, now: Instant) -> Self {
        let mut challenge_data = [0u8; 8];
        // Simple deterministic challenge for now — production would use random
        let addr_bytes = match new_path {
            SocketAddr::V4(a) => {
                let ip = a.ip().octets();
                let port = a.port().to_be_bytes();
                [ip[0], ip[1], ip[2], ip[3], port[0], port[1], 0, 0]
            }
            SocketAddr::V6(a) => {
                let ip = a.ip().octets();
                [ip[0], ip[1], ip[2], ip[3], ip[4], ip[5], ip[6], ip[7]]
            }
        };
        challenge_data = addr_bytes;

        Self {
            new_path,
            challenge_data,
            sent_at: now,
            validated: false,
        }
    }

    /// Returns the challenge data to send in a PATH_CHALLENGE frame.
    pub fn challenge_data(&self) -> &[u8; 8] {
        &self.challenge_data
    }

    /// Processes a PATH_RESPONSE. Returns true if validation succeeds.
    pub fn on_response(&mut self, data: &[u8; 8]) -> bool {
        if data == &self.challenge_data {
            self.validated = true;
            true
        } else {
            false
        }
    }

    pub fn is_validated(&self) -> bool {
        self.validated
    }

    pub fn new_path(&self) -> SocketAddr {
        self.new_path
    }

    /// Returns true if the validation has timed out (3x PTO default ~3s).
    pub fn is_timed_out(&self, now: Instant) -> bool {
        now.duration_since(self.sent_at) > std::time::Duration::from_secs(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    fn test_addr() -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 4433))
    }

    #[test]
    fn create_and_validate() {
        let now = Instant::now();
        let mut pv = PathValidator::new(test_addr(), now);
        assert!(!pv.is_validated());

        let challenge = *pv.challenge_data();
        assert!(pv.on_response(&challenge));
        assert!(pv.is_validated());
    }

    #[test]
    fn wrong_response() {
        let now = Instant::now();
        let mut pv = PathValidator::new(test_addr(), now);
        assert!(!pv.on_response(&[0xff; 8]));
        assert!(!pv.is_validated());
    }

    #[test]
    fn timeout() {
        let now = Instant::now();
        let pv = PathValidator::new(test_addr(), now);
        assert!(!pv.is_timed_out(now));
        let later = now + std::time::Duration::from_secs(5);
        assert!(pv.is_timed_out(later));
    }
}
