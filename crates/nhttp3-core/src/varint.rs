use bytes::{Buf, BufMut};

use crate::Error;

/// Maximum value representable as a QUIC variable-length integer: 2^62 - 1.
pub const MAX: u64 = (1 << 62) - 1;

/// QUIC variable-length integer (RFC 9000 §16).
///
/// Values range from 0 to 2^62 - 1 (4,611,686,018,427,387,903).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VarInt(u64);

impl VarInt {
    /// Creates a new `VarInt` from a `u64`, returning an error if the value
    /// exceeds the maximum (2^62 - 1).
    pub fn new(val: u64) -> Result<Self, Error> {
        if val > MAX {
            return Err(Error::InvalidVarInt);
        }
        Ok(Self(val))
    }

    /// Creates a `VarInt` from a `u32`. Always succeeds since u32::MAX < 2^62.
    pub fn from_u32(val: u32) -> Self {
        Self(val as u64)
    }

    /// Returns the value as a `u64`.
    pub fn value(self) -> u64 {
        self.0
    }

    /// Returns the number of bytes needed to encode this value.
    pub fn encoded_size(self) -> usize {
        if self.0 < 64 {
            1
        } else if self.0 < 16384 {
            2
        } else if self.0 < 1_073_741_824 {
            4
        } else {
            8
        }
    }

    /// Encodes this varint into the buffer.
    pub fn encode<B: BufMut>(&self, buf: &mut B) {
        match self.encoded_size() {
            1 => buf.put_u8(self.0 as u8),
            2 => buf.put_u16((0x4000 | self.0) as u16),
            4 => buf.put_u32((0x8000_0000 | self.0) as u32),
            8 => buf.put_u64(0xc000_0000_0000_0000 | self.0),
            _ => unreachable!(),
        }
    }

    /// Decodes a varint from the buffer, advancing the cursor.
    pub fn decode<B: Buf>(buf: &mut B) -> Result<Self, Error> {
        if !buf.has_remaining() {
            return Err(Error::BufferTooShort);
        }

        let first = buf.chunk()[0];
        let len = 1 << (first >> 6);

        if buf.remaining() < len {
            return Err(Error::BufferTooShort);
        }

        let val = match len {
            1 => {
                buf.advance(1);
                u64::from(first)
            }
            2 => {
                let raw = buf.get_u16();
                u64::from(raw & 0x3fff)
            }
            4 => {
                let raw = buf.get_u32();
                u64::from(raw & 0x3fff_ffff)
            }
            8 => {
                let raw = buf.get_u64();
                raw & 0x3fff_ffff_ffff_ffff
            }
            _ => unreachable!(),
        };

        Ok(Self(val))
    }
}

impl TryFrom<u64> for VarInt {
    type Error = Error;

    fn try_from(val: u64) -> Result<Self, Self::Error> {
        Self::new(val)
    }
}

impl From<VarInt> for u64 {
    fn from(v: VarInt) -> u64 {
        v.0
    }
}

impl From<u32> for VarInt {
    fn from(val: u32) -> Self {
        Self::from_u32(val)
    }
}

impl std::fmt::Display for VarInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{Bytes, BytesMut};

    #[test]
    fn from_u64_valid() {
        assert!(VarInt::try_from(0u64).is_ok());
        assert!(VarInt::try_from(63u64).is_ok());
        assert!(VarInt::try_from(16383u64).is_ok());
        assert!(VarInt::try_from(1_073_741_823u64).is_ok());
        assert!(VarInt::try_from(4_611_686_018_427_387_903u64).is_ok());
    }

    #[test]
    fn from_u64_overflow() {
        assert!(VarInt::try_from(4_611_686_018_427_387_904u64).is_err());
        assert!(VarInt::try_from(u64::MAX).is_err());
    }

    #[test]
    fn encode_1byte() {
        let v = VarInt::try_from(37u64).unwrap();
        let mut buf = BytesMut::new();
        v.encode(&mut buf);
        assert_eq!(&buf[..], &[0x25]);
    }

    #[test]
    fn encode_2byte() {
        let v = VarInt::try_from(15293u64).unwrap();
        let mut buf = BytesMut::new();
        v.encode(&mut buf);
        assert_eq!(&buf[..], &[0x7b, 0xbd]);
    }

    #[test]
    fn encode_4byte() {
        let v = VarInt::try_from(494_878_333u64).unwrap();
        let mut buf = BytesMut::new();
        v.encode(&mut buf);
        assert_eq!(&buf[..], &[0x9d, 0x7f, 0x3e, 0x7d]);
    }

    #[test]
    fn encode_8byte() {
        let v = VarInt::try_from(151_288_809_941_952_652u64).unwrap();
        let mut buf = BytesMut::new();
        v.encode(&mut buf);
        assert_eq!(&buf[..], &[0xc2, 0x19, 0x7c, 0x5e, 0xff, 0x14, 0xe8, 0x8c]);
    }

    #[test]
    fn decode_roundtrip() {
        for val in [
            0u64,
            1,
            63,
            64,
            16383,
            16384,
            1_073_741_823,
            1_073_741_824,
            4_611_686_018_427_387_903,
        ] {
            let v = VarInt::try_from(val).unwrap();
            let mut buf = BytesMut::new();
            v.encode(&mut buf);
            let decoded = VarInt::decode(&mut buf.freeze()).unwrap();
            assert_eq!(v, decoded, "roundtrip failed for {val}");
        }
    }

    #[test]
    fn decode_empty_buffer() {
        let mut buf = Bytes::new();
        assert!(VarInt::decode(&mut buf).is_err());
    }

    #[test]
    fn decode_truncated_buffer() {
        // 2-byte encoding but only 1 byte available
        let mut buf = Bytes::from_static(&[0x40]);
        assert!(VarInt::decode(&mut buf).is_err());
    }

    #[test]
    fn encoded_size() {
        assert_eq!(VarInt::try_from(0u64).unwrap().encoded_size(), 1);
        assert_eq!(VarInt::try_from(63u64).unwrap().encoded_size(), 1);
        assert_eq!(VarInt::try_from(64u64).unwrap().encoded_size(), 2);
        assert_eq!(VarInt::try_from(16383u64).unwrap().encoded_size(), 2);
        assert_eq!(VarInt::try_from(16384u64).unwrap().encoded_size(), 4);
        assert_eq!(
            VarInt::try_from(1_073_741_823u64).unwrap().encoded_size(),
            4
        );
        assert_eq!(
            VarInt::try_from(1_073_741_824u64).unwrap().encoded_size(),
            8
        );
    }

    #[test]
    fn into_u64() {
        let v = VarInt::try_from(42u64).unwrap();
        assert_eq!(u64::from(v), 42);
    }
}
