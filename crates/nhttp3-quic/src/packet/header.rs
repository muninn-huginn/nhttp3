use bytes::{Buf, Bytes};
use nhttp3_core::{ConnectionId, Error as CoreError, VarInt};

/// QUIC version 1.
pub const QUIC_VERSION_1: u32 = 0x00000001;
/// QUIC version 2.
pub const QUIC_VERSION_2: u32 = 0x6b3343cf;

/// Errors specific to packet parsing.
#[derive(Debug, thiserror::Error)]
pub enum PacketError {
    #[error(transparent)]
    Core(#[from] CoreError),

    #[error("invalid packet: {0}")]
    Invalid(String),
}

/// QUIC packet type for long headers (RFC 9000 §17.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongPacketType {
    Initial,
    ZeroRtt,
    Handshake,
    Retry,
}

/// Parsed QUIC packet header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Header {
    Long(LongHeader),
    Short(ShortHeader),
}

/// Long header (RFC 9000 §17.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongHeader {
    pub first_byte: u8,
    pub packet_type: LongPacketType,
    pub version: u32,
    pub dcid: ConnectionId,
    pub scid: ConnectionId,
    pub token: Vec<u8>,
    pub payload_length: u64,
    pub pn_offset: usize,
}

/// Short header / 1-RTT (RFC 9000 §17.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortHeader {
    pub first_byte: u8,
    pub dcid: ConnectionId,
    pub pn_offset: usize,
}

impl Header {
    /// Returns true if the first byte indicates a long header.
    pub fn is_long_header(first_byte: u8) -> bool {
        first_byte & 0x80 != 0
    }

    /// Parses a QUIC packet header from the buffer.
    pub fn parse(buf: &mut Bytes, local_cid_len: usize) -> Result<Self, PacketError> {
        let original_len = buf.remaining();

        if !buf.has_remaining() {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }

        let first_byte = buf.chunk()[0];

        if Self::is_long_header(first_byte) {
            Self::parse_long(buf, original_len)
        } else {
            Self::parse_short(buf, local_cid_len)
        }
    }

    fn parse_long(buf: &mut Bytes, original_len: usize) -> Result<Self, PacketError> {
        if buf.remaining() < 6 {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }

        let first_byte = buf.get_u8();
        let version = buf.get_u32();

        let dcid_len = buf.get_u8() as usize;
        if buf.remaining() < dcid_len {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }
        let dcid = ConnectionId::from_slice(&buf.chunk()[..dcid_len])?;
        buf.advance(dcid_len);

        if !buf.has_remaining() {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }
        let scid_len = buf.get_u8() as usize;
        if buf.remaining() < scid_len {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }
        let scid = ConnectionId::from_slice(&buf.chunk()[..scid_len])?;
        buf.advance(scid_len);

        let packet_type = match (first_byte & 0x30) >> 4 {
            0x00 => LongPacketType::Initial,
            0x01 => LongPacketType::ZeroRtt,
            0x02 => LongPacketType::Handshake,
            0x03 => LongPacketType::Retry,
            _ => unreachable!(),
        };

        let token = if packet_type == LongPacketType::Initial {
            let token_len = VarInt::decode(buf)?.value() as usize;
            if buf.remaining() < token_len {
                return Err(PacketError::Core(CoreError::BufferTooShort));
            }
            let t = buf.chunk()[..token_len].to_vec();
            buf.advance(token_len);
            t
        } else {
            Vec::new()
        };

        let (payload_length, pn_offset) = if packet_type != LongPacketType::Retry {
            let payload_length = VarInt::decode(buf)?.value();
            let pn_offset = original_len - buf.remaining();
            (payload_length, pn_offset)
        } else {
            (0, 0)
        };

        Ok(Header::Long(LongHeader {
            first_byte,
            packet_type,
            version,
            dcid,
            scid,
            token,
            payload_length,
            pn_offset,
        }))
    }

    fn parse_short(buf: &mut Bytes, local_cid_len: usize) -> Result<Self, PacketError> {
        if buf.remaining() < 1 + local_cid_len {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }

        let first_byte = buf.get_u8();
        let dcid = ConnectionId::from_slice(&buf.chunk()[..local_cid_len])?;
        buf.advance(local_cid_len);

        Ok(Header::Short(ShortHeader {
            first_byte,
            dcid,
            pn_offset: 1 + local_cid_len,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_initial_header() {
        let data = vec![
            0xc0, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0x04,
        ];
        let mut buf = Bytes::from(data);
        let header = Header::parse(&mut buf, 0).unwrap();

        match header {
            Header::Long(h) => {
                assert_eq!(h.packet_type, LongPacketType::Initial);
                assert_eq!(h.version, QUIC_VERSION_1);
                assert_eq!(h.dcid.as_bytes(), &[1, 2, 3, 4]);
                assert_eq!(h.scid.len(), 0);
                assert!(h.token.is_empty());
                assert_eq!(h.payload_length, 4);
            }
            _ => panic!("expected long header"),
        }
    }

    #[test]
    fn parse_handshake_header() {
        let data = vec![
            0xe0, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x02, 0x03, 0x04, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x04,
        ];
        let mut buf = Bytes::from(data);
        let header = Header::parse(&mut buf, 0).unwrap();

        match header {
            Header::Long(h) => {
                assert_eq!(h.packet_type, LongPacketType::Handshake);
                assert_eq!(h.dcid.as_bytes(), &[1, 2, 3, 4]);
                assert_eq!(h.scid.as_bytes(), &[5, 6, 7, 8]);
            }
            _ => panic!("expected long header"),
        }
    }

    #[test]
    fn parse_short_header() {
        let data = vec![0x40, 0x01, 0x02, 0x03, 0x04];
        let mut buf = Bytes::from(data);
        let header = Header::parse(&mut buf, 4).unwrap();

        match header {
            Header::Short(h) => {
                assert_eq!(h.dcid.as_bytes(), &[1, 2, 3, 4]);
            }
            _ => panic!("expected short header"),
        }
    }

    #[test]
    fn detect_long_vs_short() {
        assert!(Header::is_long_header(0xc0));
        assert!(Header::is_long_header(0xff));
        assert!(!Header::is_long_header(0x40));
        assert!(!Header::is_long_header(0x00));
    }
}
