use nhttp3_core::VarInt;

/// Stream ID encodes the initiator and directionality (RFC 9000 §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StreamId(u64);

impl StreamId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn value(self) -> u64 {
        self.0
    }

    pub fn is_client_initiated(self) -> bool {
        self.0 & 0x01 == 0
    }

    pub fn is_server_initiated(self) -> bool {
        self.0 & 0x01 == 1
    }

    pub fn is_bidi(self) -> bool {
        self.0 & 0x02 == 0
    }

    pub fn is_uni(self) -> bool {
        self.0 & 0x02 != 0
    }

    pub fn to_varint(self) -> VarInt {
        VarInt::try_from(self.0).unwrap()
    }
}

/// Send-side stream state (RFC 9000 §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendState {
    Ready,
    Send,
    DataSent,
    DataRecvd,
    ResetSent,
    ResetRecvd,
}

/// Receive-side stream state (RFC 9000 §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvState {
    Recv,
    SizeKnown,
    DataRecvd,
    DataRead,
    ResetRecvd,
    ResetRead,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_id_client_bidi() {
        let id = StreamId::new(0);
        assert!(id.is_client_initiated());
        assert!(id.is_bidi());
    }

    #[test]
    fn stream_id_server_uni() {
        let id = StreamId::new(3);
        assert!(id.is_server_initiated());
        assert!(id.is_uni());
    }

    #[test]
    fn stream_id_client_uni() {
        let id = StreamId::new(2);
        assert!(id.is_client_initiated());
        assert!(id.is_uni());
    }

    #[test]
    fn stream_id_server_bidi() {
        let id = StreamId::new(1);
        assert!(id.is_server_initiated());
        assert!(id.is_bidi());
    }

    #[test]
    fn stream_id_ordering() {
        for i in [0u64, 4, 8, 12] {
            let id = StreamId::new(i);
            assert!(id.is_client_initiated());
            assert!(id.is_bidi());
        }
    }
}
