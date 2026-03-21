/// Stateless Reset support (RFC 9000 §10.3).
///
/// A stateless reset allows an endpoint that has lost state to signal
/// that it cannot process a packet. The reset token is a 16-byte value
/// derived from the connection ID using HMAC.

/// Minimum packet size for a stateless reset (RFC 9000 §10.3.3).
/// Must be at least 21 bytes (to be indistinguishable from a short header packet).
pub const MIN_STATELESS_RESET_SIZE: usize = 21;

/// Generates a stateless reset token from a connection ID and a static key.
///
/// Uses a simple HMAC-like construction. In production, this should use
/// HMAC-SHA256 with a server-secret key.
pub fn generate_reset_token(cid: &[u8], key: &[u8; 32]) -> [u8; 16] {
    // Simple token derivation: XOR-fold the key with the CID
    // Production: HMAC-SHA256(key, cid)[..16]
    let mut token = [0u8; 16];
    for (i, &b) in cid.iter().enumerate() {
        token[i % 16] ^= b;
    }
    for (i, &b) in key.iter().enumerate() {
        token[i % 16] ^= b;
    }
    token
}

/// Validates whether the last 16 bytes of a packet match a stateless reset token.
pub fn is_stateless_reset(packet: &[u8], expected_token: &[u8; 16]) -> bool {
    if packet.len() < MIN_STATELESS_RESET_SIZE {
        return false;
    }
    let token_start = packet.len() - 16;
    let received_token = &packet[token_start..];

    // Constant-time comparison to prevent timing oracle (aioquic #555)
    let mut diff = 0u8;
    for i in 0..16 {
        diff |= received_token[i] ^ expected_token[i];
    }
    diff == 0
}

/// Constructs a stateless reset packet.
///
/// The packet contains random bytes followed by the reset token.
/// Must be at least 21 bytes and unpredictable to prevent oracle attacks.
pub fn build_stateless_reset(token: &[u8; 16]) -> Vec<u8> {
    let random_len = 5; // Minimum: 21 - 16 = 5 random bytes
    let mut packet = Vec::with_capacity(random_len + 16);

    // First byte must have form bit = 0 (looks like short header)
    // Use fixed pattern with some entropy
    let entropy = token[0] ^ token[1];
    packet.push(0x40 | (entropy & 0x1f)); // Short header form

    // Random-ish padding (production: use OS random)
    for i in 1..random_len {
        packet.push(token[i] ^ (i as u8).wrapping_mul(37));
    }

    // Append the stateless reset token
    packet.extend_from_slice(token);
    packet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_token_deterministic() {
        let key = [0xaau8; 32];
        let cid = [1, 2, 3, 4];
        let t1 = generate_reset_token(&cid, &key);
        let t2 = generate_reset_token(&cid, &key);
        assert_eq!(t1, t2);
    }

    #[test]
    fn different_cids_different_tokens() {
        let key = [0xaau8; 32];
        let t1 = generate_reset_token(&[1, 2, 3, 4], &key);
        let t2 = generate_reset_token(&[5, 6, 7, 8], &key);
        assert_ne!(t1, t2);
    }

    #[test]
    fn validate_stateless_reset() {
        let key = [0xbb; 32];
        let cid = [1, 2, 3, 4, 5, 6, 7, 8];
        let token = generate_reset_token(&cid, &key);
        let packet = build_stateless_reset(&token);

        assert!(is_stateless_reset(&packet, &token));
        assert!(packet.len() >= MIN_STATELESS_RESET_SIZE);
    }

    #[test]
    fn wrong_token_not_validated() {
        let token = [0xaa; 16];
        let wrong = [0xbb; 16];
        let packet = build_stateless_reset(&token);
        assert!(!is_stateless_reset(&packet, &wrong));
    }

    #[test]
    fn too_short_packet_rejected() {
        let token = [0xaa; 16];
        let short_packet = [0x40; 10]; // < 21 bytes
        assert!(!is_stateless_reset(&short_packet, &token));
    }

    #[test]
    fn constant_time_comparison() {
        // This test just verifies the function works — actual timing
        // analysis would need specialized tooling
        let token = [0xcc; 16];
        let packet = build_stateless_reset(&token);
        for _ in 0..1000 {
            assert!(is_stateless_reset(&packet, &token));
        }
    }
}
