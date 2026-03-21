/// FFI error codes.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nhttp3Error {
    Ok = 0,
    InvalidArgument = -1,
    BufferTooSmall = -2,
    ConnectionClosed = -3,
    StreamBlocked = -4,
    TlsError = -5,
    InternalError = -6,
    Timeout = -7,
}

impl Nhttp3Error {
    pub fn is_ok(self) -> bool {
        self == Self::Ok
    }
}

impl std::fmt::Display for Nhttp3Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::InvalidArgument => write!(f, "invalid argument"),
            Self::BufferTooSmall => write!(f, "buffer too small"),
            Self::ConnectionClosed => write!(f, "connection closed"),
            Self::StreamBlocked => write!(f, "stream blocked"),
            Self::TlsError => write!(f, "TLS error"),
            Self::InternalError => write!(f, "internal error"),
            Self::Timeout => write!(f, "timeout"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes() {
        assert!(Nhttp3Error::Ok.is_ok());
        assert!(!Nhttp3Error::InternalError.is_ok());
        assert_eq!(Nhttp3Error::Ok as i32, 0);
        assert_eq!(Nhttp3Error::InternalError as i32, -6);
    }

    #[test]
    fn error_display() {
        assert_eq!(Nhttp3Error::Ok.to_string(), "ok");
        assert_eq!(Nhttp3Error::TlsError.to_string(), "TLS error");
    }
}
