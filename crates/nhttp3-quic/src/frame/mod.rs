pub mod parse;
pub mod write;

use nhttp3_core::VarInt;

/// QUIC frame types (RFC 9000 §12.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Padding,
    Ping,
    Ack {
        largest_ack: VarInt,
        ack_delay: VarInt,
        first_ack_range: VarInt,
        ack_ranges: Vec<AckRange>,
        ecn: Option<EcnCounts>,
    },
    ResetStream {
        stream_id: VarInt,
        error_code: VarInt,
        final_size: VarInt,
    },
    StopSending {
        stream_id: VarInt,
        error_code: VarInt,
    },
    Crypto {
        offset: VarInt,
        data: Vec<u8>,
    },
    NewToken {
        token: Vec<u8>,
    },
    Stream {
        stream_id: VarInt,
        offset: Option<VarInt>,
        data: Vec<u8>,
        fin: bool,
    },
    MaxData {
        max_data: VarInt,
    },
    MaxStreamData {
        stream_id: VarInt,
        max_data: VarInt,
    },
    MaxStreams {
        bidi: bool,
        max_streams: VarInt,
    },
    DataBlocked {
        max_data: VarInt,
    },
    StreamDataBlocked {
        stream_id: VarInt,
        max_data: VarInt,
    },
    StreamsBlocked {
        bidi: bool,
        max_streams: VarInt,
    },
    NewConnectionId {
        sequence: VarInt,
        retire_prior_to: VarInt,
        connection_id: nhttp3_core::ConnectionId,
        stateless_reset_token: [u8; 16],
    },
    RetireConnectionId {
        sequence: VarInt,
    },
    PathChallenge {
        data: [u8; 8],
    },
    PathResponse {
        data: [u8; 8],
    },
    ConnectionClose {
        error_code: VarInt,
        frame_type: Option<VarInt>,
        reason: Vec<u8>,
    },
    HandshakeDone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckRange {
    pub gap: VarInt,
    pub range: VarInt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcnCounts {
    pub ect0: VarInt,
    pub ect1: VarInt,
    pub ecn_ce: VarInt,
}
