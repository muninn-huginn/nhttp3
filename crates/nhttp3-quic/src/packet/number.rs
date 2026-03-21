use bytes::BufMut;

/// Encodes a packet number into `len` bytes (1-4).
pub fn encode_packet_number<B: BufMut>(buf: &mut B, pn: u64, len: usize) {
    let pn_bytes = pn.to_be_bytes();
    buf.put_slice(&pn_bytes[8 - len..]);
}

/// Decodes a truncated packet number given the largest acknowledged packet number.
/// RFC 9000, Appendix A.
pub fn decode_packet_number(largest_pn: u64, truncated_pn: u64, pn_nbits: u32) -> u64 {
    let expected_pn = largest_pn.wrapping_add(1);
    let pn_win = 1u64 << pn_nbits;
    let pn_hwin = pn_win / 2;
    let pn_mask = pn_win - 1;

    let candidate_pn = (expected_pn & !pn_mask) | truncated_pn;

    if candidate_pn.wrapping_add(pn_hwin) <= expected_pn
        && candidate_pn < (1u64 << 62) - pn_win
    {
        candidate_pn.wrapping_add(pn_win)
    } else if candidate_pn > expected_pn.wrapping_add(pn_hwin) && candidate_pn >= pn_win {
        candidate_pn.wrapping_sub(pn_win)
    } else {
        candidate_pn
    }
}

/// Returns the number of bytes needed to encode `pn` given the `largest_acked` PN.
/// RFC 9000 §17.1.
pub fn packet_number_length(pn: u64, largest_acked: u64) -> usize {
    let range = pn.saturating_sub(largest_acked);
    if range < (1 << 7) {
        1
    } else if range < (1 << 15) {
        2
    } else if range < (1 << 23) {
        3
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_1byte() {
        let mut buf = Vec::new();
        encode_packet_number(&mut buf, 0, 1);
        assert_eq!(buf, vec![0x00]);
    }

    #[test]
    fn encode_2byte() {
        let mut buf = Vec::new();
        encode_packet_number(&mut buf, 0x1234, 2);
        assert_eq!(buf, vec![0x12, 0x34]);
    }

    #[test]
    fn encode_4byte() {
        let mut buf = Vec::new();
        encode_packet_number(&mut buf, 0x12345678, 4);
        assert_eq!(buf, vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn decode_rfc_example_1() {
        let decoded = decode_packet_number(0xa82f30ea, 0x9b32, 16);
        assert_eq!(decoded, 0xa82f9b32);
    }

    #[test]
    fn decode_rfc_example_2() {
        let decoded = decode_packet_number(0, 0, 8);
        assert_eq!(decoded, 0);
    }

    #[test]
    fn encoded_size_selection() {
        assert_eq!(packet_number_length(0, 0), 1);
        assert_eq!(packet_number_length(256, 0), 2);
        assert_eq!(packet_number_length(0x1_0000, 0), 3);
        assert_eq!(packet_number_length(0x1_000_000, 0), 4);
    }
}
