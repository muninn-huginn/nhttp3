use nhttp3_core::VarInt;

/// QUIC transport error codes (RFC 9000 §20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportErrorCode {
    NoError,
    InternalError,
    ConnectionRefused,
    FlowControlError,
    StreamLimitError,
    StreamStateError,
    FinalSizeError,
    FrameEncodingError,
    TransportParameterError,
    ConnectionIdLimitError,
    ProtocolViolation,
    InvalidToken,
    ApplicationError,
    CryptoBufferExceeded,
    KeyUpdateError,
    AeadLimitReached,
    NoViablePath,
    CryptoError(u8),
    Application(u64),
}

impl TransportErrorCode {
    pub fn to_varint(self) -> VarInt {
        let val = match self {
            Self::NoError => 0x00,
            Self::InternalError => 0x01,
            Self::ConnectionRefused => 0x02,
            Self::FlowControlError => 0x03,
            Self::StreamLimitError => 0x04,
            Self::StreamStateError => 0x05,
            Self::FinalSizeError => 0x06,
            Self::FrameEncodingError => 0x07,
            Self::TransportParameterError => 0x08,
            Self::ConnectionIdLimitError => 0x09,
            Self::ProtocolViolation => 0x0a,
            Self::InvalidToken => 0x0b,
            Self::ApplicationError => 0x0c,
            Self::CryptoBufferExceeded => 0x0d,
            Self::KeyUpdateError => 0x0e,
            Self::AeadLimitReached => 0x0f,
            Self::NoViablePath => 0x10,
            Self::CryptoError(code) => 0x0100 + code as u64,
            Self::Application(code) => code,
        };
        VarInt::try_from(val).unwrap()
    }

    pub fn from_u64(val: u64) -> Self {
        match val {
            0x00 => Self::NoError,
            0x01 => Self::InternalError,
            0x02 => Self::ConnectionRefused,
            0x03 => Self::FlowControlError,
            0x04 => Self::StreamLimitError,
            0x05 => Self::StreamStateError,
            0x06 => Self::FinalSizeError,
            0x07 => Self::FrameEncodingError,
            0x08 => Self::TransportParameterError,
            0x09 => Self::ConnectionIdLimitError,
            0x0a => Self::ProtocolViolation,
            0x0b => Self::InvalidToken,
            0x0c => Self::ApplicationError,
            0x0d => Self::CryptoBufferExceeded,
            0x0e => Self::KeyUpdateError,
            0x0f => Self::AeadLimitReached,
            0x10 => Self::NoViablePath,
            0x0100..=0x01ff => Self::CryptoError((val - 0x0100) as u8),
            other => Self::Application(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_roundtrip() {
        let codes = [
            TransportErrorCode::NoError,
            TransportErrorCode::InternalError,
            TransportErrorCode::FlowControlError,
            TransportErrorCode::ProtocolViolation,
            TransportErrorCode::CryptoError(0x2a),
        ];
        for code in codes {
            let val = code.to_varint().value();
            let decoded = TransportErrorCode::from_u64(val);
            assert_eq!(code, decoded);
        }
    }
}
