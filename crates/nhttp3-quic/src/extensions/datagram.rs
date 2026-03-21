use nhttp3_core::VarInt;

/// QUIC DATAGRAM frame (RFC 9221).
///
/// Unreliable datagrams sent over a QUIC connection. Unlike streams,
/// datagrams are not retransmitted on loss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Datagram {
    pub data: Vec<u8>,
}

/// DATAGRAM frame type values (RFC 9221 §4).
/// 0x30 = DATAGRAM without Length field
/// 0x31 = DATAGRAM with Length field
pub const DATAGRAM_NO_LEN: u64 = 0x30;
pub const DATAGRAM_WITH_LEN: u64 = 0x31;

/// Transport parameter for max_datagram_frame_size (RFC 9221 §3).
pub const MAX_DATAGRAM_FRAME_SIZE_PARAM: u64 = 0x20;

/// Configuration for QUIC datagrams.
#[derive(Debug, Clone)]
pub struct DatagramConfig {
    /// Maximum size of a datagram frame payload. 0 = disabled.
    pub max_datagram_frame_size: u64,
}

impl Default for DatagramConfig {
    fn default() -> Self {
        Self {
            max_datagram_frame_size: 0, // disabled by default
        }
    }
}

impl DatagramConfig {
    pub fn enabled(max_size: u64) -> Self {
        Self {
            max_datagram_frame_size: max_size,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.max_datagram_frame_size > 0
    }
}

impl Datagram {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Encodes as a DATAGRAM frame with length field.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = bytes::BytesMut::new();
        use bytes::BufMut;
        VarInt::try_from(DATAGRAM_WITH_LEN).unwrap().encode(&mut buf);
        VarInt::try_from(self.data.len() as u64).unwrap().encode(&mut buf);
        buf.put_slice(&self.data);
        buf.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datagram_config_default_disabled() {
        let config = DatagramConfig::default();
        assert!(!config.is_enabled());
    }

    #[test]
    fn datagram_config_enabled() {
        let config = DatagramConfig::enabled(65535);
        assert!(config.is_enabled());
        assert_eq!(config.max_datagram_frame_size, 65535);
    }

    #[test]
    fn datagram_encode() {
        let dg = Datagram::new(b"hello".to_vec());
        let encoded = dg.encode();
        assert!(!encoded.is_empty());
        // First byte should be varint for 0x31
        assert_eq!(encoded[0], 0x31);
    }

    #[test]
    fn datagram_new() {
        let dg = Datagram::new(vec![1, 2, 3]);
        assert_eq!(dg.data, vec![1, 2, 3]);
    }
}
