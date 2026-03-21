use crate::Error;

const MAX_CID_LEN: usize = 20;

/// QUIC Connection ID — 0 to 20 bytes (RFC 9000 §17.2).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ConnectionId {
    len: u8,
    bytes: [u8; MAX_CID_LEN],
}

impl ConnectionId {
    /// Creates an empty (zero-length) connection ID.
    pub fn empty() -> Self {
        Self {
            len: 0,
            bytes: [0; MAX_CID_LEN],
        }
    }

    /// Creates a connection ID from a byte slice.
    /// Returns an error if the slice is longer than 20 bytes.
    pub fn from_slice(src: &[u8]) -> Result<Self, Error> {
        if src.len() > MAX_CID_LEN {
            return Err(Error::InvalidConnectionId(src.len()));
        }
        let mut bytes = [0u8; MAX_CID_LEN];
        bytes[..src.len()].copy_from_slice(src);
        Ok(Self {
            len: src.len() as u8,
            bytes,
        })
    }

    /// Returns the length of the connection ID.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns true if the connection ID is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the connection ID bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

impl std::fmt::Debug for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConnectionId(")?;
        for byte in self.as_bytes() {
            write!(f, "{byte:02x}")?;
        }
        write!(f, ")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_connection_id() {
        let cid = ConnectionId::empty();
        assert_eq!(cid.len(), 0);
        assert_eq!(cid.as_bytes(), &[]);
    }

    #[test]
    fn from_slice_valid() {
        let cid = ConnectionId::from_slice(&[1, 2, 3, 4]).unwrap();
        assert_eq!(cid.len(), 4);
        assert_eq!(cid.as_bytes(), &[1, 2, 3, 4]);
    }

    #[test]
    fn from_slice_max_length() {
        let data = [0xffu8; 20];
        let cid = ConnectionId::from_slice(&data).unwrap();
        assert_eq!(cid.len(), 20);
    }

    #[test]
    fn from_slice_too_long() {
        let data = [0u8; 21];
        assert!(ConnectionId::from_slice(&data).is_err());
    }

    #[test]
    fn debug_format() {
        let cid = ConnectionId::from_slice(&[0xab, 0xcd]).unwrap();
        let dbg = format!("{:?}", cid);
        assert!(dbg.contains("abcd"), "debug should show hex: {dbg}");
    }

    #[test]
    fn equality() {
        let a = ConnectionId::from_slice(&[1, 2]).unwrap();
        let b = ConnectionId::from_slice(&[1, 2]).unwrap();
        let c = ConnectionId::from_slice(&[1, 3]).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
