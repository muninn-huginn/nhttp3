use crate::packet::header::Header;

/// Minimum UDP payload size for Initial packets (RFC 9000 §14.1).
pub const MIN_INITIAL_PACKET_SIZE: usize = 1200;

/// Maximum CRYPTO buffer size per connection to prevent DoS (aioquic #501).
pub const MAX_CRYPTO_BUFFER_SIZE: usize = 128 * 1024; // 128 KB

/// Maximum number of pending path challenges to prevent DoS (aioquic #544).
pub const MAX_PATH_CHALLENGES: usize = 8;

/// Validates that an incoming Initial packet meets minimum size requirements.
pub fn validate_initial_packet_size(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    // Only enforce for Initial packets (long header, type 00)
    if !Header::is_long_header(data[0]) {
        return true; // Not a long header, no size requirement
    }
    let packet_type = (data[0] & 0x30) >> 4;
    if packet_type != 0x00 {
        return true; // Not an Initial packet
    }
    data.len() >= MIN_INITIAL_PACKET_SIZE
}

/// Validates that a CRYPTO buffer hasn't exceeded the maximum allowed size.
pub fn validate_crypto_buffer_size(current_size: usize, incoming: usize) -> bool {
    current_size + incoming <= MAX_CRYPTO_BUFFER_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_packet_too_small() {
        // Initial packet header (type 0x00) but only 100 bytes
        let mut data = vec![0xc0; 100]; // long header, Initial type
        assert!(!validate_initial_packet_size(&data));
    }

    #[test]
    fn initial_packet_minimum_size() {
        let data = vec![0xc0; 1200];
        assert!(validate_initial_packet_size(&data));
    }

    #[test]
    fn handshake_packet_no_size_requirement() {
        // Handshake type = 0x02, so first byte has bits 0010 in type position
        let data = vec![0xe0; 100]; // long header, Handshake type
        assert!(validate_initial_packet_size(&data));
    }

    #[test]
    fn short_header_no_size_requirement() {
        let data = vec![0x40; 50];
        assert!(validate_initial_packet_size(&data));
    }

    #[test]
    fn crypto_buffer_within_limit() {
        assert!(validate_crypto_buffer_size(0, 1000));
        assert!(validate_crypto_buffer_size(100_000, 28_000));
    }

    #[test]
    fn crypto_buffer_exceeds_limit() {
        // 128 * 1024 = 131072, so 131000 + 100 = 131100 > 131072
        assert!(!validate_crypto_buffer_size(131_000, 100));
    }

    #[test]
    fn empty_packet_rejected() {
        assert!(!validate_initial_packet_size(&[]));
    }
}
