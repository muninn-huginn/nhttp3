/// HTTP/3 error codes (RFC 9114 §8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H3Error {
    NoError,
    GeneralProtocolError,
    InternalError,
    StreamCreationError,
    ClosedCriticalStream,
    FrameUnexpected,
    FrameError,
    ExcessiveLoad,
    IdError,
    SettingsError,
    MissingSettings,
    RequestRejected,
    RequestCancelled,
    RequestIncomplete,
    MessageError,
    ConnectError,
    VersionFallback,
    QpackDecompressionFailed,
    QpackEncoderStreamError,
    QpackDecoderStreamError,
}

impl H3Error {
    pub fn code(&self) -> u64 {
        match self {
            Self::NoError => 0x0100,
            Self::GeneralProtocolError => 0x0101,
            Self::InternalError => 0x0102,
            Self::StreamCreationError => 0x0103,
            Self::ClosedCriticalStream => 0x0104,
            Self::FrameUnexpected => 0x0105,
            Self::FrameError => 0x0106,
            Self::ExcessiveLoad => 0x0107,
            Self::IdError => 0x0108,
            Self::SettingsError => 0x0109,
            Self::MissingSettings => 0x010a,
            Self::RequestRejected => 0x010b,
            Self::RequestCancelled => 0x010c,
            Self::RequestIncomplete => 0x010d,
            Self::MessageError => 0x010e,
            Self::ConnectError => 0x010f,
            Self::VersionFallback => 0x0110,
            Self::QpackDecompressionFailed => 0x0200,
            Self::QpackEncoderStreamError => 0x0201,
            Self::QpackDecoderStreamError => 0x0202,
        }
    }

    pub fn from_code(code: u64) -> Self {
        match code {
            0x0100 => Self::NoError,
            0x0101 => Self::GeneralProtocolError,
            0x0102 => Self::InternalError,
            0x0103 => Self::StreamCreationError,
            0x0104 => Self::ClosedCriticalStream,
            0x0105 => Self::FrameUnexpected,
            0x0106 => Self::FrameError,
            0x0107 => Self::ExcessiveLoad,
            0x0108 => Self::IdError,
            0x0109 => Self::SettingsError,
            0x010a => Self::MissingSettings,
            0x010b => Self::RequestRejected,
            0x010c => Self::RequestCancelled,
            0x010d => Self::RequestIncomplete,
            0x010e => Self::MessageError,
            0x010f => Self::ConnectError,
            0x0110 => Self::VersionFallback,
            0x0200 => Self::QpackDecompressionFailed,
            0x0201 => Self::QpackEncoderStreamError,
            0x0202 => Self::QpackDecoderStreamError,
            _ => Self::GeneralProtocolError,
        }
    }
}

/// Errors from HTTP/3 operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("QUIC error: {0}")]
    Quic(#[from] nhttp3_quic::packet::PacketError),

    #[error("QPACK error: {0}")]
    Qpack(#[from] nhttp3_qpack::DecoderError),

    #[error("HTTP/3 error: {0:?}")]
    H3(H3Error),

    #[error("frame error: {0}")]
    FrameError(String),

    #[error("malformed headers")]
    MalformedHeaders,

    #[error("settings error")]
    SettingsError,

    #[error("closed critical stream")]
    ClosedCriticalStream,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_roundtrip() {
        let errors = [
            H3Error::NoError,
            H3Error::FrameError,
            H3Error::SettingsError,
            H3Error::QpackDecompressionFailed,
        ];
        for err in errors {
            assert_eq!(H3Error::from_code(err.code()), err);
        }
    }
}
