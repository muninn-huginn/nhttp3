use thiserror::Error;

/// Errors from nhttp3-core operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("buffer too short")]
    BufferTooShort,

    #[error("invalid variable-length integer")]
    InvalidVarInt,

    #[error("invalid connection ID length: {0}")]
    InvalidConnectionId(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        assert_eq!(Error::BufferTooShort.to_string(), "buffer too short");
        assert_eq!(
            Error::InvalidVarInt.to_string(),
            "invalid variable-length integer"
        );
        assert_eq!(
            Error::InvalidConnectionId(21).to_string(),
            "invalid connection ID length: 21"
        );
    }

    #[test]
    fn error_is_clone_and_eq() {
        let e = Error::BufferTooShort;
        assert_eq!(e.clone(), e);
    }
}
