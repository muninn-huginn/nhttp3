pub mod parse;

use nhttp3_core::VarInt;

/// HTTP/3 frame types (RFC 9114 §7.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum H3Frame {
    Data {
        data: Vec<u8>,
    },
    Headers {
        block: Vec<u8>,
    },
    CancelPush {
        push_id: VarInt,
    },
    Settings {
        settings: Vec<(VarInt, VarInt)>,
    },
    PushPromise {
        push_id: VarInt,
        block: Vec<u8>,
    },
    GoAway {
        id: VarInt,
    },
    MaxPushId {
        push_id: VarInt,
    },
    /// Unknown/reserved frame type — skip per spec.
    Unknown {
        frame_type: VarInt,
        data: Vec<u8>,
    },
}

// Frame type constants (RFC 9114 §7.2)
pub const DATA: u64 = 0x00;
pub const HEADERS: u64 = 0x01;
pub const CANCEL_PUSH: u64 = 0x03;
pub const SETTINGS: u64 = 0x04;
pub const PUSH_PROMISE: u64 = 0x05;
pub const GOAWAY: u64 = 0x07;
pub const MAX_PUSH_ID: u64 = 0x0d;
