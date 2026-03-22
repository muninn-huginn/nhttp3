//! Builds properly framed QUIC packets from handshake/stream data.
//!
//! This is the critical missing piece: wrapping TLS handshake data in
//! CRYPTO frames inside Initial/Handshake packets with proper headers.

use bytes::{BufMut, BytesMut};
use nhttp3_core::{ConnectionId, VarInt};

use super::header::{QUIC_VERSION_1, LongPacketType};
use super::number::encode_packet_number;
use crate::frame::Frame;

/// Maximum UDP payload size for Initial packets.
const MAX_INITIAL_PAYLOAD: usize = 1200;

/// Builds a QUIC Initial packet containing CRYPTO frame(s).
///
/// RFC 9000 §17.2.2: Initial packets carry the first CRYPTO frames
/// for the TLS handshake.
pub fn build_initial_packet(
    dcid: &ConnectionId,
    scid: &ConnectionId,
    token: &[u8],
    crypto_data: &[u8],
    packet_number: u64,
) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(MAX_INITIAL_PAYLOAD);

    // We'll write the header, then come back and fill in the length field.
    // For now, build the payload first to know its length.

    // Build payload: CRYPTO frame + padding
    let mut payload = BytesMut::new();

    // CRYPTO frame
    if !crypto_data.is_empty() {
        let frame = Frame::Crypto {
            offset: VarInt::from_u32(0),
            data: crypto_data.to_vec(),
        };
        frame.encode(&mut payload);
    }

    // Packet number (1 byte for simplicity — PN = 0 for Initial)
    let pn_len: usize = 1;
    let pn_byte = (packet_number & 0xff) as u8;

    // Payload length = pn_len + payload + padding (to reach 1200 min)
    let payload_len_before_pad = pn_len + payload.len();

    // First byte: Long header (1), Fixed bit (1), Type 00 (Initial), Reserved 00, PN len 00
    // PN len field = pn_len - 1 = 0 (for 1-byte PN)
    let first_byte: u8 = 0xc0 | ((pn_len as u8 - 1) & 0x03);

    // Build header
    buf.put_u8(first_byte);
    buf.put_u32(QUIC_VERSION_1);
    buf.put_u8(dcid.len() as u8);
    buf.put_slice(dcid.as_bytes());
    buf.put_u8(scid.len() as u8);
    buf.put_slice(scid.as_bytes());

    // Token (varint length + data)
    VarInt::try_from(token.len() as u64).unwrap().encode(&mut buf);
    if !token.is_empty() {
        buf.put_slice(token);
    }

    // Calculate total packet size and add padding to reach 1200
    let header_so_far = buf.len();
    let min_payload = if MAX_INITIAL_PAYLOAD > header_so_far + 2 {
        // +2 for the Length varint (at least 2 bytes for the length field)
        MAX_INITIAL_PAYLOAD - header_so_far - 2
    } else {
        payload_len_before_pad
    };

    let total_payload = std::cmp::max(payload_len_before_pad, min_payload);
    let padding_needed = total_payload - payload_len_before_pad;

    // Length field (covers PN + payload + padding)
    VarInt::try_from(total_payload as u64).unwrap().encode(&mut buf);

    // Packet number
    buf.put_u8(pn_byte);

    // Payload (CRYPTO frame)
    buf.put_slice(&payload);

    // Padding (PADDING frames = 0x00 bytes)
    for _ in 0..padding_needed {
        buf.put_u8(0x00);
    }

    buf.to_vec()
}

/// Builds a QUIC Handshake packet containing CRYPTO frame(s).
pub fn build_handshake_packet(
    dcid: &ConnectionId,
    scid: &ConnectionId,
    crypto_data: &[u8],
    packet_number: u64,
) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(1400);

    // Build payload
    let mut payload = BytesMut::new();
    if !crypto_data.is_empty() {
        let frame = Frame::Crypto {
            offset: VarInt::from_u32(0),
            data: crypto_data.to_vec(),
        };
        frame.encode(&mut payload);
    }

    let pn_len: usize = 1;
    let pn_byte = (packet_number & 0xff) as u8;

    // First byte: Long header, type 10 (Handshake)
    let first_byte: u8 = 0xe0 | ((pn_len as u8 - 1) & 0x03);

    buf.put_u8(first_byte);
    buf.put_u32(QUIC_VERSION_1);
    buf.put_u8(dcid.len() as u8);
    buf.put_slice(dcid.as_bytes());
    buf.put_u8(scid.len() as u8);
    buf.put_slice(scid.as_bytes());

    // Length
    let total_payload = pn_len + payload.len();
    VarInt::try_from(total_payload as u64).unwrap().encode(&mut buf);

    // PN
    buf.put_u8(pn_byte);

    // Payload
    buf.put_slice(&payload);

    buf.to_vec()
}

/// Builds a QUIC 1-RTT (short header) packet containing STREAM frames.
pub fn build_short_packet(
    dcid: &ConnectionId,
    frames: &[Frame],
    packet_number: u64,
) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(1400);

    let pn_len: usize = 2;

    // First byte: Short header (0), Fixed bit (1), Spin=0, Reserved=00, Key Phase=0, PN len
    let first_byte: u8 = 0x40 | ((pn_len as u8 - 1) & 0x03);

    buf.put_u8(first_byte);
    buf.put_slice(dcid.as_bytes());

    // PN (2 bytes)
    encode_packet_number(&mut buf, packet_number, pn_len);

    // Frames
    for frame in frames {
        frame.encode(&mut buf);
    }

    buf.to_vec()
}

/// Extracts CRYPTO frame data from a received Initial/Handshake packet.
/// Returns the crypto data if found.
pub fn extract_crypto_data(packet: &[u8]) -> Option<Vec<u8>> {
    use bytes::Bytes;

    if packet.is_empty() {
        return None;
    }

    let first_byte = packet[0];
    let is_long = first_byte & 0x80 != 0;

    if !is_long {
        return None; // Short headers don't carry CRYPTO in handshake
    }

    // Skip header to find the payload
    let mut pos: usize = 1; // past first byte
    if packet.len() < 6 {
        return None;
    }

    pos += 4; // version

    let dcid_len = packet[pos] as usize;
    pos += 1 + dcid_len;
    if pos >= packet.len() {
        return None;
    }

    let scid_len = packet[pos] as usize;
    pos += 1 + scid_len;
    if pos >= packet.len() {
        return None;
    }

    let packet_type = (first_byte & 0x30) >> 4;

    // Token (Initial only)
    if packet_type == 0x00 {
        let mut token_buf = Bytes::copy_from_slice(&packet[pos..]);
        if let Ok(token_len) = VarInt::decode(&mut token_buf) {
            let varint_bytes = packet.len() - pos - token_buf.remaining();
            pos += varint_bytes + token_len.value() as usize;
        } else {
            return None;
        }
    }

    // Length field
    if pos >= packet.len() {
        return None;
    }
    let mut len_buf = Bytes::copy_from_slice(&packet[pos..]);
    let payload_len = VarInt::decode(&mut len_buf).ok()?.value() as usize;
    let varint_bytes = packet.len() - pos - len_buf.remaining();
    pos += varint_bytes;

    // PN (length from first byte low 2 bits + 1)
    let pn_len = (first_byte & 0x03) as usize + 1;
    pos += pn_len;

    // Now we're at the payload — try to parse frames
    if pos >= packet.len() {
        return None;
    }

    let mut payload = Bytes::copy_from_slice(&packet[pos..std::cmp::min(pos + payload_len - pn_len, packet.len())]);
    let mut crypto_data = Vec::new();

    while payload.has_remaining() {
        use bytes::Buf;
        match Frame::parse(&mut payload) {
            Ok(Frame::Crypto { data, .. }) => {
                crypto_data.extend_from_slice(&data);
            }
            Ok(Frame::Padding) => continue,
            Ok(_) => continue,
            Err(_) => break,
        }
    }

    if crypto_data.is_empty() {
        None
    } else {
        Some(crypto_data)
    }
}

use bytes::Buf;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_initial_packet_min_size() {
        let dcid = ConnectionId::from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        let scid = ConnectionId::from_slice(&[9, 10, 11, 12]).unwrap();
        let crypto = b"client hello data here";

        let pkt = build_initial_packet(&dcid, &scid, &[], crypto, 0);
        assert!(pkt.len() >= 1200, "Initial must be >= 1200, got {}", pkt.len());
        assert_eq!(pkt[0] & 0x80, 0x80); // long header
        assert_eq!((pkt[0] & 0x30) >> 4, 0x00); // Initial type
    }

    #[test]
    fn build_handshake_packet_valid() {
        let dcid = ConnectionId::from_slice(&[1, 2, 3, 4]).unwrap();
        let scid = ConnectionId::from_slice(&[5, 6, 7, 8]).unwrap();

        let pkt = build_handshake_packet(&dcid, &scid, b"handshake data", 0);
        assert_eq!(pkt[0] & 0x80, 0x80); // long header
        assert_eq!((pkt[0] & 0x30) >> 4, 0x02); // Handshake type
    }

    #[test]
    fn build_short_packet_valid() {
        let dcid = ConnectionId::from_slice(&[1, 2, 3, 4]).unwrap();
        let frames = vec![Frame::Stream {
            stream_id: VarInt::from_u32(0),
            offset: Some(VarInt::from_u32(0)),
            data: b"hello".to_vec(),
            fin: false,
        }];

        let pkt = build_short_packet(&dcid, &frames, 1);
        assert_eq!(pkt[0] & 0x80, 0x00); // short header
        assert_eq!(pkt[0] & 0x40, 0x40); // fixed bit
    }

    #[test]
    fn initial_packet_roundtrip_crypto() {
        let dcid = ConnectionId::from_slice(&[1, 2, 3, 4]).unwrap();
        let scid = ConnectionId::from_slice(&[5, 6]).unwrap();
        let crypto = b"test crypto payload for roundtrip";

        let pkt = build_initial_packet(&dcid, &scid, &[], crypto, 0);
        let extracted = extract_crypto_data(&pkt).expect("should extract crypto data");
        assert_eq!(extracted, crypto);
    }

    #[test]
    fn handshake_packet_roundtrip_crypto() {
        let dcid = ConnectionId::from_slice(&[1, 2]).unwrap();
        let scid = ConnectionId::from_slice(&[3, 4]).unwrap();
        let crypto = b"handshake crypto roundtrip";

        let pkt = build_handshake_packet(&dcid, &scid, crypto, 0);
        let extracted = extract_crypto_data(&pkt).expect("should extract crypto data");
        assert_eq!(extracted, crypto);
    }

    #[test]
    fn empty_packet_returns_none() {
        assert!(extract_crypto_data(&[]).is_none());
    }

    #[test]
    fn short_packet_returns_none() {
        let pkt = vec![0x40, 0x01, 0x02]; // short header
        assert!(extract_crypto_data(&pkt).is_none());
    }
}
