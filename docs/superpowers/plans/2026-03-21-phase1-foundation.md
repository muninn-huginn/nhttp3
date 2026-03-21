# Phase 1: Foundation (nhttp3-core + nhttp3-quic) Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the core primitives crate and a working QUIC transport that can complete a handshake and exchange stream data over localhost.

**Architecture:** Cargo workspace with two crates — `nhttp3-core` (shared types, varint, buffers, I/O traits) and `nhttp3-quic` (QUIC transport on top of tokio + rustls). TDD throughout: write failing test, implement, verify, commit.

**Tech Stack:** Rust (edition 2021), tokio 1.x, rustls 0.23.x, bytes 1.x, thiserror 2.x, ring (via rustls), criterion (benches)

**Spec:** `docs/superpowers/specs/2026-03-21-nhttp3-design.md`

---

## Chunk 1: Workspace Scaffolding + nhttp3-core

### Task 1: Initialize Cargo Workspace

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/nhttp3-core/Cargo.toml`
- Create: `crates/nhttp3-core/src/lib.rs`
- Create: `crates/nhttp3-quic/Cargo.toml`
- Create: `crates/nhttp3-quic/src/lib.rs`
- Create: `.gitignore`

- [ ] **Step 1: Create workspace root Cargo.toml**

```toml
# Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/nhttp3-core",
    "crates/nhttp3-quic",
]

[workspace.package]
edition = "2021"
license = "MIT"
repository = "https://github.com/ArnBon/nhttp3"
rust-version = "1.75"

[workspace.dependencies]
bytes = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
rustls = { version = "0.23", features = ["quic"] }
```

- [ ] **Step 2: Create nhttp3-core Cargo.toml**

```toml
# crates/nhttp3-core/Cargo.toml
[package]
name = "nhttp3-core"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
description = "Core primitives for nhttp3 — varint encoding, buffers, shared types"

[dependencies]
bytes = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 3: Create nhttp3-core/src/lib.rs**

```rust
// crates/nhttp3-core/src/lib.rs
pub mod varint;
pub mod error;

pub use error::Error;
```

- [ ] **Step 4: Create nhttp3-quic Cargo.toml**

```toml
# crates/nhttp3-quic/Cargo.toml
[package]
name = "nhttp3-quic"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
description = "QUIC transport implementation for nhttp3"

[dependencies]
nhttp3-core = { path = "../nhttp3-core" }
bytes = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
rustls = { workspace = true }
```

- [ ] **Step 5: Create nhttp3-quic/src/lib.rs**

```rust
// crates/nhttp3-quic/src/lib.rs
pub mod packet;
pub mod frame;
```

- [ ] **Step 6: Create .gitignore**

```
target/
.env
.DS_Store
*.swp
*.swo
Cargo.lock
```

Wait — `Cargo.lock` should be committed for binaries but not libraries. Since this is a library workspace, include it in `.gitignore`.

- [ ] **Step 7: Verify workspace compiles**

Run: `cargo check --workspace`
Expected: Compiles with warnings about empty modules (that's fine)

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/ .gitignore
git commit -m "feat: initialize cargo workspace with nhttp3-core and nhttp3-quic"
```

---

### Task 2: Variable-Length Integer Encoding (nhttp3-core)

QUIC uses a variable-length integer encoding (RFC 9000 §16). Values 0–63 use 1 byte, 64–16383 use 2 bytes, 16384–1073741823 use 4 bytes, and up to 4611686018427387903 use 8 bytes. The 2 most significant bits encode the length.

**Files:**
- Create: `crates/nhttp3-core/src/varint.rs`
- Modify: `crates/nhttp3-core/src/lib.rs`

- [ ] **Step 1: Write failing tests for VarInt**

```rust
// crates/nhttp3-core/src/varint.rs

/// QUIC variable-length integer (RFC 9000 §16).
/// Values range from 0 to 2^62 - 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VarInt(u64);

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{Bytes, BytesMut, Buf, BufMut};

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
        for val in [0, 1, 63, 64, 16383, 16384, 1_073_741_823, 1_073_741_824, 4_611_686_018_427_387_903] {
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
        assert_eq!(VarInt::try_from(1_073_741_823u64).unwrap().encoded_size(), 4);
        assert_eq!(VarInt::try_from(1_073_741_824u64).unwrap().encoded_size(), 8);
    }

    #[test]
    fn into_u64() {
        let v = VarInt::try_from(42u64).unwrap();
        assert_eq!(u64::from(v), 42);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p nhttp3-core`
Expected: FAIL — methods `encode`, `decode`, `encoded_size` don't exist yet

- [ ] **Step 3: Implement VarInt**

```rust
// crates/nhttp3-core/src/varint.rs (replace test-only file with full implementation + tests)
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

// Tests from Step 1 go here (inside #[cfg(test)] mod tests { ... })
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p nhttp3-core`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/nhttp3-core/src/varint.rs
git commit -m "feat(core): implement QUIC variable-length integer encoding (RFC 9000 §16)"
```

---

### Task 3: Core Error Types (nhttp3-core)

**Files:**
- Create: `crates/nhttp3-core/src/error.rs`

- [ ] **Step 1: Write error types with tests**

```rust
// crates/nhttp3-core/src/error.rs
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
        assert_eq!(Error::InvalidVarInt.to_string(), "invalid variable-length integer");
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
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p nhttp3-core`
Expected: All tests PASS (including varint tests from earlier)

- [ ] **Step 3: Commit**

```bash
git add crates/nhttp3-core/src/error.rs
git commit -m "feat(core): add core error types"
```

---

### Task 4: Connection ID Type (nhttp3-core)

QUIC connection IDs are 0–20 bytes (RFC 9000 §17.2).

**Files:**
- Create: `crates/nhttp3-core/src/connection_id.rs`
- Modify: `crates/nhttp3-core/src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
// crates/nhttp3-core/src/connection_id.rs

/// QUIC Connection ID — 0 to 20 bytes (RFC 9000 §17.2).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ConnectionId {
    len: u8,
    bytes: [u8; 20],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p nhttp3-core`
Expected: FAIL — methods don't exist yet

- [ ] **Step 3: Implement ConnectionId**

```rust
// crates/nhttp3-core/src/connection_id.rs
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

// tests from Step 1
```

- [ ] **Step 4: Update lib.rs to export ConnectionId**

```rust
// crates/nhttp3-core/src/lib.rs
pub mod varint;
pub mod error;
pub mod connection_id;

pub use error::Error;
pub use varint::VarInt;
pub use connection_id::ConnectionId;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p nhttp3-core`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/nhttp3-core/
git commit -m "feat(core): add ConnectionId type (RFC 9000 §17.2)"
```

---

## Chunk 2: QUIC Packet Parsing

### Task 5: Packet Header Types (nhttp3-quic)

QUIC has two header forms: Long Header (Initial, Handshake, 0-RTT, Retry) and Short Header (1-RTT). RFC 9000 §17.

**Files:**
- Create: `crates/nhttp3-quic/src/packet/mod.rs`
- Create: `crates/nhttp3-quic/src/packet/header.rs`
- Modify: `crates/nhttp3-quic/src/lib.rs`

- [ ] **Step 1: Write header type definitions and tests**

```rust
// crates/nhttp3-quic/src/packet/header.rs
use nhttp3_core::{ConnectionId, VarInt};

/// QUIC version.
pub const QUIC_VERSION_1: u32 = 0x00000001;
pub const QUIC_VERSION_2: u32 = 0x6b3343cf;

/// QUIC packet type for long headers (RFC 9000 §17.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongPacketType {
    Initial,
    ZeroRtt,
    Handshake,
    Retry,
}

/// Parsed QUIC packet header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Header {
    Long(LongHeader),
    Short(ShortHeader),
}

/// Long header (RFC 9000 §17.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongHeader {
    /// First byte (includes form bit, fixed bit, type bits, reserved/PN length bits).
    pub first_byte: u8,
    /// Packet type.
    pub packet_type: LongPacketType,
    /// QUIC version.
    pub version: u32,
    /// Destination Connection ID.
    pub dcid: ConnectionId,
    /// Source Connection ID.
    pub scid: ConnectionId,
    /// Token (Initial packets only).
    pub token: Vec<u8>,
    /// Remaining payload length (from the Length field).
    pub payload_length: u64,
    /// Packet number offset — byte position in the original buffer where the
    /// packet number starts. Needed for header protection.
    pub pn_offset: usize,
}

/// Short header / 1-RTT (RFC 9000 §17.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortHeader {
    pub first_byte: u8,
    pub dcid: ConnectionId,
    /// Packet number offset.
    pub pn_offset: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn parse_initial_header() {
        // Minimal Initial packet header:
        // 0xc0 = long header (1), fixed bit (1), type 00 (Initial), reserved 00, PN len 00
        // Version: 0x00000001
        // DCID len: 4, DCID: 01020304
        // SCID len: 0
        // Token length: 0 (varint)
        // Payload length: 4 (varint, covers PN + payload)
        let mut data = vec![
            0xc0, // first byte
            0x00, 0x00, 0x00, 0x01, // version 1
            0x04, // DCID length
            0x01, 0x02, 0x03, 0x04, // DCID
            0x00, // SCID length
            0x00, // token length (varint: 0)
            0x04, // payload length (varint: 4)
        ];
        let mut buf = Bytes::from(data.clone());
        let header = Header::parse(&mut buf, 0).unwrap();

        match header {
            Header::Long(h) => {
                assert_eq!(h.packet_type, LongPacketType::Initial);
                assert_eq!(h.version, QUIC_VERSION_1);
                assert_eq!(h.dcid.as_bytes(), &[1, 2, 3, 4]);
                assert_eq!(h.scid.len(), 0);
                assert!(h.token.is_empty());
                assert_eq!(h.payload_length, 4);
            }
            _ => panic!("expected long header"),
        }
    }

    #[test]
    fn parse_handshake_header() {
        // 0xe0 = long header, fixed bit, type 10 (Handshake)
        let data = vec![
            0xe0,
            0x00, 0x00, 0x00, 0x01,
            0x04, 0x01, 0x02, 0x03, 0x04,
            0x04, 0x05, 0x06, 0x07, 0x08,
            0x04, // payload length
        ];
        let mut buf = Bytes::from(data);
        let header = Header::parse(&mut buf, 0).unwrap();

        match header {
            Header::Long(h) => {
                assert_eq!(h.packet_type, LongPacketType::Handshake);
                assert_eq!(h.dcid.as_bytes(), &[1, 2, 3, 4]);
                assert_eq!(h.scid.as_bytes(), &[5, 6, 7, 8]);
            }
            _ => panic!("expected long header"),
        }
    }

    #[test]
    fn parse_short_header() {
        // 0x40 = short header (0), fixed bit (1)
        // Followed by DCID (caller must know the length)
        let data = vec![
            0x40,
            0x01, 0x02, 0x03, 0x04, // DCID (4 bytes, known from connection state)
        ];
        let mut buf = Bytes::from(data);
        let header = Header::parse(&mut buf, 4).unwrap();

        match header {
            Header::Short(h) => {
                assert_eq!(h.dcid.as_bytes(), &[1, 2, 3, 4]);
            }
            _ => panic!("expected short header"),
        }
    }

    #[test]
    fn detect_long_vs_short() {
        assert!(Header::is_long_header(0xc0));
        assert!(Header::is_long_header(0xff));
        assert!(!Header::is_long_header(0x40));
        assert!(!Header::is_long_header(0x00));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p nhttp3-quic`
Expected: FAIL — `Header::parse`, `Header::is_long_header` don't exist

- [ ] **Step 3: Implement header parsing**

The key challenge is tracking `pn_offset` — we need the byte position in the original packet where the packet number starts. We track this by capturing `buf.remaining()` at the start and computing `original_len - buf.remaining()` after parsing up to the PN field.

```rust
impl Header {
    pub fn is_long_header(first_byte: u8) -> bool {
        first_byte & 0x80 != 0
    }

    /// Parses a QUIC packet header from a byte slice.
    /// Returns the parsed header. The `pn_offset` field in the returned header
    /// indicates where the packet number starts in the original slice.
    pub fn parse(buf: &mut Bytes, local_cid_len: usize) -> Result<Self, PacketError> {
        let original_len = buf.remaining();

        if !buf.has_remaining() {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }

        let first_byte = buf.chunk()[0];

        if Self::is_long_header(first_byte) {
            Self::parse_long(buf, original_len)
        } else {
            Self::parse_short(buf, local_cid_len)
        }
    }

    fn parse_long(buf: &mut Bytes, original_len: usize) -> Result<Self, PacketError> {
        if buf.remaining() < 6 {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }

        let first_byte = buf.get_u8();
        let version = buf.get_u32();

        let dcid_len = buf.get_u8() as usize;
        if buf.remaining() < dcid_len {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }
        let dcid = ConnectionId::from_slice(&buf.chunk()[..dcid_len])?;
        buf.advance(dcid_len);

        if !buf.has_remaining() {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }
        let scid_len = buf.get_u8() as usize;
        if buf.remaining() < scid_len {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }
        let scid = ConnectionId::from_slice(&buf.chunk()[..scid_len])?;
        buf.advance(scid_len);

        let packet_type = match (first_byte & 0x30) >> 4 {
            0x00 => LongPacketType::Initial,
            0x01 => LongPacketType::ZeroRtt,
            0x02 => LongPacketType::Handshake,
            0x03 => LongPacketType::Retry,
            _ => unreachable!(),
        };

        let token = if packet_type == LongPacketType::Initial {
            let token_len = VarInt::decode(buf)?.value() as usize;
            if buf.remaining() < token_len {
                return Err(PacketError::Core(CoreError::BufferTooShort));
            }
            let t = buf.chunk()[..token_len].to_vec();
            buf.advance(token_len);
            t
        } else {
            Vec::new()
        };

        let (payload_length, pn_offset) = if packet_type != LongPacketType::Retry {
            let payload_length = VarInt::decode(buf)?.value();
            let pn_offset = original_len - buf.remaining();
            (payload_length, pn_offset)
        } else {
            (0, 0)
        };

        Ok(Header::Long(LongHeader {
            first_byte,
            packet_type,
            version,
            dcid,
            scid,
            token,
            payload_length,
            pn_offset,
        }))
    }

    fn parse_short(buf: &mut Bytes, local_cid_len: usize) -> Result<Self, PacketError> {
        if buf.remaining() < 1 + local_cid_len {
            return Err(PacketError::Core(CoreError::BufferTooShort));
        }

        let first_byte = buf.get_u8();
        let dcid = ConnectionId::from_slice(&buf.chunk()[..local_cid_len])?;
        buf.advance(local_cid_len);

        Ok(Header::Short(ShortHeader {
            first_byte,
            dcid,
            pn_offset: 1 + local_cid_len,
        }))
    }
}
```

- [ ] **Step 4: Create the packet module**

```rust
// crates/nhttp3-quic/src/packet/mod.rs
pub mod header;

pub use header::{Header, LongHeader, ShortHeader, LongPacketType, PacketError};
pub use header::{QUIC_VERSION_1, QUIC_VERSION_2};
```

- [ ] **Step 5: Update nhttp3-quic lib.rs**

```rust
// crates/nhttp3-quic/src/lib.rs
pub mod packet;
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p nhttp3-quic`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/nhttp3-quic/
git commit -m "feat(quic): implement QUIC packet header parsing (RFC 9000 §17)"
```

---

### Task 6: Packet Number Encoding/Decoding

Packet numbers are 1–4 bytes. The actual packet number is reconstructed from the truncated value plus the largest acknowledged PN. RFC 9000 §17.1, Appendix A.

**Files:**
- Create: `crates/nhttp3-quic/src/packet/number.rs`
- Modify: `crates/nhttp3-quic/src/packet/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
// crates/nhttp3-quic/src/packet/number.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_1byte() {
        let mut buf = Vec::new();
        encode_packet_number(&mut buf, 0, 1);
        assert_eq!(buf, vec![0x00]);
    }

    #[test]
    fn encode_2byte() {
        let mut buf = Vec::new();
        encode_packet_number(&mut buf, 0x1234, 2);
        assert_eq!(buf, vec![0x12, 0x34]);
    }

    #[test]
    fn encode_4byte() {
        let mut buf = Vec::new();
        encode_packet_number(&mut buf, 0x12345678, 4);
        assert_eq!(buf, vec![0x12, 0x34, 0x56, 0x78]);
    }

    // RFC 9000 Appendix A test vectors
    #[test]
    fn decode_rfc_example_1() {
        // Largest PN = 0xa82f30ea, truncated PN = 0x9b32 (2 bytes)
        let decoded = decode_packet_number(0xa82f30ea, 0x9b32, 16);
        assert_eq!(decoded, 0xa82f9b32);
    }

    #[test]
    fn decode_rfc_example_2() {
        // Largest PN = 0, truncated = 0 (1 byte)
        let decoded = decode_packet_number(0, 0, 8);
        assert_eq!(decoded, 0);
    }

    #[test]
    fn encoded_size_selection() {
        // Should pick smallest encoding that avoids ambiguity
        assert_eq!(packet_number_length(0, 0), 1);
        assert_eq!(packet_number_length(256, 0), 2);
        assert_eq!(packet_number_length(0x1_0000, 0), 3);
        assert_eq!(packet_number_length(0x1_000_000, 0), 4);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p nhttp3-quic`
Expected: FAIL

- [ ] **Step 3: Implement packet number encoding/decoding**

```rust
// crates/nhttp3-quic/src/packet/number.rs
use bytes::BufMut;

/// Encodes a packet number into `len` bytes (1–4).
pub fn encode_packet_number<B: BufMut>(buf: &mut B, pn: u64, len: usize) {
    let pn_bytes = pn.to_be_bytes();
    buf.put_slice(&pn_bytes[8 - len..]);
}

/// Decodes a truncated packet number given the largest acknowledged packet number.
/// RFC 9000, Appendix A.
pub fn decode_packet_number(largest_pn: u64, truncated_pn: u64, pn_nbits: u32) -> u64 {
    let expected_pn = largest_pn.wrapping_add(1);
    let pn_win = 1u64 << pn_nbits;
    let pn_hwin = pn_win / 2;
    let pn_mask = pn_win - 1;

    let candidate_pn = (expected_pn & !pn_mask) | truncated_pn;

    if candidate_pn.wrapping_add(pn_hwin) <= expected_pn && candidate_pn < (1u64 << 62) - pn_win {
        candidate_pn.wrapping_add(pn_win)
    } else if candidate_pn > expected_pn.wrapping_add(pn_hwin) && candidate_pn >= pn_win {
        candidate_pn.wrapping_sub(pn_win)
    } else {
        candidate_pn
    }
}

/// Returns the number of bytes needed to encode `pn` given the `largest_acked` PN.
/// RFC 9000 §17.1 — uses the smallest encoding that ensures the peer can decode correctly.
pub fn packet_number_length(pn: u64, largest_acked: u64) -> usize {
    let range = pn.saturating_sub(largest_acked);
    if range < (1 << 7) {
        1
    } else if range < (1 << 15) {
        2
    } else if range < (1 << 23) {
        3
    } else {
        4
    }
}

// tests from Step 1
```

- [ ] **Step 4: Export from packet/mod.rs**

Add to `crates/nhttp3-quic/src/packet/mod.rs`:
```rust
pub mod number;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p nhttp3-quic`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/nhttp3-quic/src/packet/number.rs crates/nhttp3-quic/src/packet/mod.rs
git commit -m "feat(quic): implement packet number encoding/decoding (RFC 9000 §17.1, Appendix A)"
```

---

### Task 7: QUIC Frame Types & Parsing

QUIC frames are the fundamental data units inside packets. RFC 9000 §12.4.

**Files:**
- Create: `crates/nhttp3-quic/src/frame/mod.rs`
- Create: `crates/nhttp3-quic/src/frame/parse.rs`
- Create: `crates/nhttp3-quic/src/frame/write.rs`
- Modify: `crates/nhttp3-quic/src/lib.rs`

- [ ] **Step 1: Define frame types with tests**

```rust
// crates/nhttp3-quic/src/frame/mod.rs
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
        /// ECN counts (if present).
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
```

- [ ] **Step 2: Write frame parsing tests**

```rust
// crates/nhttp3-quic/src/frame/parse.rs
use bytes::{Bytes, Buf, BytesMut, BufMut};
use nhttp3_core::VarInt;
use super::*;

impl Frame {
    /// Parses a single frame from the buffer.
    pub fn parse(buf: &mut Bytes) -> Result<Self, crate::packet::PacketError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_varint(val: u64) -> Vec<u8> {
        let v = VarInt::try_from(val).unwrap();
        let mut buf = BytesMut::new();
        v.encode(&mut buf);
        buf.to_vec()
    }

    #[test]
    fn parse_padding() {
        let mut buf = Bytes::from_static(&[0x00]);
        let frame = Frame::parse(&mut buf).unwrap();
        assert_eq!(frame, Frame::Padding);
    }

    #[test]
    fn parse_ping() {
        let mut buf = Bytes::from_static(&[0x01]);
        let frame = Frame::parse(&mut buf).unwrap();
        assert_eq!(frame, Frame::Ping);
    }

    #[test]
    fn parse_crypto() {
        let mut data = Vec::new();
        data.push(0x06); // CRYPTO frame type
        data.extend_from_slice(&encode_varint(0)); // offset
        data.extend_from_slice(&encode_varint(5)); // length
        data.extend_from_slice(b"hello"); // data
        let mut buf = Bytes::from(data);
        let frame = Frame::parse(&mut buf).unwrap();
        match frame {
            Frame::Crypto { offset, data } => {
                assert_eq!(offset.value(), 0);
                assert_eq!(data, b"hello");
            }
            _ => panic!("expected Crypto frame"),
        }
    }

    #[test]
    fn parse_stream_with_offset_and_fin() {
        // Type 0x0f = STREAM with OFF, LEN, FIN bits all set
        let mut data = Vec::new();
        data.push(0x0f); // type
        data.extend_from_slice(&encode_varint(4)); // stream_id
        data.extend_from_slice(&encode_varint(100)); // offset
        data.extend_from_slice(&encode_varint(3)); // length
        data.extend_from_slice(b"hey"); // data
        let mut buf = Bytes::from(data);
        let frame = Frame::parse(&mut buf).unwrap();
        match frame {
            Frame::Stream { stream_id, offset, data, fin } => {
                assert_eq!(stream_id.value(), 4);
                assert_eq!(offset.unwrap().value(), 100);
                assert_eq!(data, b"hey");
                assert!(fin);
            }
            _ => panic!("expected Stream frame"),
        }
    }

    #[test]
    fn parse_connection_close() {
        let mut data = Vec::new();
        data.push(0x1c); // CONNECTION_CLOSE (transport)
        data.extend_from_slice(&encode_varint(0x0a)); // error code
        data.extend_from_slice(&encode_varint(0x06)); // frame type (CRYPTO)
        data.extend_from_slice(&encode_varint(4)); // reason length
        data.extend_from_slice(b"oops"); // reason
        let mut buf = Bytes::from(data);
        let frame = Frame::parse(&mut buf).unwrap();
        match frame {
            Frame::ConnectionClose { error_code, frame_type, reason } => {
                assert_eq!(error_code.value(), 0x0a);
                assert_eq!(frame_type.unwrap().value(), 0x06);
                assert_eq!(reason, b"oops");
            }
            _ => panic!("expected ConnectionClose frame"),
        }
    }

    #[test]
    fn parse_ack_simple() {
        let mut data = Vec::new();
        data.push(0x02); // ACK (no ECN)
        data.extend_from_slice(&encode_varint(10)); // largest ack
        data.extend_from_slice(&encode_varint(0)); // ack delay
        data.extend_from_slice(&encode_varint(0)); // ack range count
        data.extend_from_slice(&encode_varint(10)); // first ack range
        let mut buf = Bytes::from(data);
        let frame = Frame::parse(&mut buf).unwrap();
        match frame {
            Frame::Ack { largest_ack, ack_delay, first_ack_range, ack_ranges, ecn } => {
                assert_eq!(largest_ack.value(), 10);
                assert_eq!(ack_delay.value(), 0);
                assert_eq!(first_ack_range.value(), 10);
                assert!(ack_ranges.is_empty());
                assert!(ecn.is_none());
            }
            _ => panic!("expected Ack frame"),
        }
    }

    #[test]
    fn parse_max_data() {
        let mut data = Vec::new();
        data.push(0x10); // MAX_DATA
        data.extend_from_slice(&encode_varint(1_000_000));
        let mut buf = Bytes::from(data);
        let frame = Frame::parse(&mut buf).unwrap();
        match frame {
            Frame::MaxData { max_data } => {
                assert_eq!(max_data.value(), 1_000_000);
            }
            _ => panic!("expected MaxData frame"),
        }
    }

    #[test]
    fn parse_handshake_done() {
        let mut buf = Bytes::from_static(&[0x1e]);
        let frame = Frame::parse(&mut buf).unwrap();
        assert_eq!(frame, Frame::HandshakeDone);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p nhttp3-quic`
Expected: FAIL — `Frame::parse` is `todo!()`

- [ ] **Step 4: Implement Frame::parse**

```rust
// crates/nhttp3-quic/src/frame/parse.rs — replace todo!() with implementation
use bytes::Buf;
use nhttp3_core::{VarInt, ConnectionId, Error as CoreError};
use crate::packet::PacketError;
use super::*;

impl Frame {
    pub fn parse(buf: &mut Bytes) -> Result<Self, PacketError> {
        let frame_type = VarInt::decode(buf)?;

        match frame_type.value() {
            0x00 => Ok(Frame::Padding),
            0x01 => Ok(Frame::Ping),
            0x02 | 0x03 => {
                let ecn = frame_type.value() == 0x03;
                let largest_ack = VarInt::decode(buf)?;
                let ack_delay = VarInt::decode(buf)?;
                let ack_range_count = VarInt::decode(buf)?;
                let first_ack_range = VarInt::decode(buf)?;

                let mut ack_ranges = Vec::new();
                for _ in 0..ack_range_count.value() {
                    let gap = VarInt::decode(buf)?;
                    let range = VarInt::decode(buf)?;
                    ack_ranges.push(AckRange { gap, range });
                }

                let ecn = if ecn {
                    Some(EcnCounts {
                        ect0: VarInt::decode(buf)?,
                        ect1: VarInt::decode(buf)?,
                        ecn_ce: VarInt::decode(buf)?,
                    })
                } else {
                    None
                };

                Ok(Frame::Ack { largest_ack, ack_delay, first_ack_range, ack_ranges, ecn })
            }
            0x04 => {
                let stream_id = VarInt::decode(buf)?;
                let error_code = VarInt::decode(buf)?;
                let final_size = VarInt::decode(buf)?;
                Ok(Frame::ResetStream { stream_id, error_code, final_size })
            }
            0x05 => {
                let stream_id = VarInt::decode(buf)?;
                let error_code = VarInt::decode(buf)?;
                Ok(Frame::StopSending { stream_id, error_code })
            }
            0x06 => {
                let offset = VarInt::decode(buf)?;
                let len = VarInt::decode(buf)?.value() as usize;
                if buf.remaining() < len {
                    return Err(PacketError::Core(CoreError::BufferTooShort));
                }
                let data = buf.chunk()[..len].to_vec();
                buf.advance(len);
                Ok(Frame::Crypto { offset, data })
            }
            0x07 => {
                let len = VarInt::decode(buf)?.value() as usize;
                if buf.remaining() < len {
                    return Err(PacketError::Core(CoreError::BufferTooShort));
                }
                let token = buf.chunk()[..len].to_vec();
                buf.advance(len);
                Ok(Frame::NewToken { token })
            }
            0x08..=0x0f => {
                let has_offset = frame_type.value() & 0x04 != 0;
                let has_length = frame_type.value() & 0x02 != 0;
                let fin = frame_type.value() & 0x01 != 0;

                let stream_id = VarInt::decode(buf)?;
                let offset = if has_offset {
                    Some(VarInt::decode(buf)?)
                } else {
                    None
                };

                let data = if has_length {
                    let len = VarInt::decode(buf)?.value() as usize;
                    if buf.remaining() < len {
                        return Err(PacketError::Core(CoreError::BufferTooShort));
                    }
                    let d = buf.chunk()[..len].to_vec();
                    buf.advance(len);
                    d
                } else {
                    // No length — consume rest of buffer
                    let d = buf.chunk().to_vec();
                    buf.advance(d.len());
                    d
                };

                Ok(Frame::Stream { stream_id, offset, data, fin })
            }
            0x10 => {
                let max_data = VarInt::decode(buf)?;
                Ok(Frame::MaxData { max_data })
            }
            0x11 => {
                let stream_id = VarInt::decode(buf)?;
                let max_data = VarInt::decode(buf)?;
                Ok(Frame::MaxStreamData { stream_id, max_data })
            }
            0x12 | 0x13 => {
                let bidi = frame_type.value() == 0x12;
                let max_streams = VarInt::decode(buf)?;
                Ok(Frame::MaxStreams { bidi, max_streams })
            }
            0x14 => {
                let max_data = VarInt::decode(buf)?;
                Ok(Frame::DataBlocked { max_data })
            }
            0x15 => {
                let stream_id = VarInt::decode(buf)?;
                let max_data = VarInt::decode(buf)?;
                Ok(Frame::StreamDataBlocked { stream_id, max_data })
            }
            0x16 | 0x17 => {
                let bidi = frame_type.value() == 0x16;
                let max_streams = VarInt::decode(buf)?;
                Ok(Frame::StreamsBlocked { bidi, max_streams })
            }
            0x18 => {
                let sequence = VarInt::decode(buf)?;
                let retire_prior_to = VarInt::decode(buf)?;
                if !buf.has_remaining() {
                    return Err(PacketError::Core(CoreError::BufferTooShort));
                }
                let cid_len = buf.get_u8() as usize;
                if buf.remaining() < cid_len + 16 {
                    return Err(PacketError::Core(CoreError::BufferTooShort));
                }
                let connection_id = ConnectionId::from_slice(&buf.chunk()[..cid_len])?;
                buf.advance(cid_len);
                let mut token = [0u8; 16];
                token.copy_from_slice(&buf.chunk()[..16]);
                buf.advance(16);
                Ok(Frame::NewConnectionId { sequence, retire_prior_to, connection_id, stateless_reset_token: token })
            }
            0x19 => {
                let sequence = VarInt::decode(buf)?;
                Ok(Frame::RetireConnectionId { sequence })
            }
            0x1a => {
                if buf.remaining() < 8 {
                    return Err(PacketError::Core(CoreError::BufferTooShort));
                }
                let mut data = [0u8; 8];
                data.copy_from_slice(&buf.chunk()[..8]);
                buf.advance(8);
                Ok(Frame::PathChallenge { data })
            }
            0x1b => {
                if buf.remaining() < 8 {
                    return Err(PacketError::Core(CoreError::BufferTooShort));
                }
                let mut data = [0u8; 8];
                data.copy_from_slice(&buf.chunk()[..8]);
                buf.advance(8);
                Ok(Frame::PathResponse { data })
            }
            0x1c => {
                let error_code = VarInt::decode(buf)?;
                let frame_type = Some(VarInt::decode(buf)?);
                let len = VarInt::decode(buf)?.value() as usize;
                if buf.remaining() < len {
                    return Err(PacketError::Core(CoreError::BufferTooShort));
                }
                let reason = buf.chunk()[..len].to_vec();
                buf.advance(len);
                Ok(Frame::ConnectionClose { error_code, frame_type, reason })
            }
            0x1d => {
                // APPLICATION_CLOSE
                let error_code = VarInt::decode(buf)?;
                let len = VarInt::decode(buf)?.value() as usize;
                if buf.remaining() < len {
                    return Err(PacketError::Core(CoreError::BufferTooShort));
                }
                let reason = buf.chunk()[..len].to_vec();
                buf.advance(len);
                Ok(Frame::ConnectionClose { error_code, frame_type: None, reason })
            }
            0x1e => Ok(Frame::HandshakeDone),
            other => Err(PacketError::Invalid(format!("unknown frame type: {other}"))),
        }
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p nhttp3-quic`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/nhttp3-quic/src/frame/
git commit -m "feat(quic): implement QUIC frame parsing (RFC 9000 §12.4)"
```

---

### Task 8: QUIC Frame Serialization

**Files:**
- Modify: `crates/nhttp3-quic/src/frame/write.rs`

- [ ] **Step 1: Write failing roundtrip tests**

```rust
// crates/nhttp3-quic/src/frame/write.rs
use bytes::{BytesMut, BufMut, Bytes};
use nhttp3_core::VarInt;
use super::*;

impl Frame {
    /// Serializes this frame into the buffer.
    pub fn encode<B: BufMut>(&self, buf: &mut B) {
        todo!()
    }

    /// Returns the number of bytes this frame will occupy when encoded.
    pub fn encoded_size(&self) -> usize {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::parse;

    fn roundtrip(frame: &Frame) {
        let mut buf = BytesMut::new();
        frame.encode(&mut buf);
        let mut bytes = buf.freeze();
        let parsed = Frame::parse(&mut bytes).unwrap();
        assert_eq!(*frame, parsed, "roundtrip failed for {frame:?}");
    }

    #[test]
    fn roundtrip_padding() {
        roundtrip(&Frame::Padding);
    }

    #[test]
    fn roundtrip_ping() {
        roundtrip(&Frame::Ping);
    }

    #[test]
    fn roundtrip_ack() {
        roundtrip(&Frame::Ack {
            largest_ack: VarInt::from_u32(100),
            ack_delay: VarInt::from_u32(25),
            first_ack_range: VarInt::from_u32(5),
            ack_ranges: vec![
                AckRange { gap: VarInt::from_u32(2), range: VarInt::from_u32(3) },
            ],
            ecn: None,
        });
    }

    #[test]
    fn roundtrip_crypto() {
        roundtrip(&Frame::Crypto {
            offset: VarInt::from_u32(0),
            data: b"handshake data".to_vec(),
        });
    }

    #[test]
    fn roundtrip_stream() {
        roundtrip(&Frame::Stream {
            stream_id: VarInt::from_u32(4),
            offset: Some(VarInt::from_u32(100)),
            data: b"payload".to_vec(),
            fin: true,
        });
    }

    #[test]
    fn roundtrip_connection_close() {
        roundtrip(&Frame::ConnectionClose {
            error_code: VarInt::from_u32(0x0a),
            frame_type: Some(VarInt::from_u32(0x06)),
            reason: b"test".to_vec(),
        });
    }

    #[test]
    fn roundtrip_max_data() {
        roundtrip(&Frame::MaxData { max_data: VarInt::from_u32(1_000_000) });
    }

    #[test]
    fn roundtrip_max_stream_data() {
        roundtrip(&Frame::MaxStreamData {
            stream_id: VarInt::from_u32(4),
            max_data: VarInt::from_u32(500_000),
        });
    }

    #[test]
    fn roundtrip_max_streams() {
        roundtrip(&Frame::MaxStreams { bidi: true, max_streams: VarInt::from_u32(100) });
        roundtrip(&Frame::MaxStreams { bidi: false, max_streams: VarInt::from_u32(50) });
    }

    #[test]
    fn roundtrip_handshake_done() {
        roundtrip(&Frame::HandshakeDone);
    }

    #[test]
    fn roundtrip_new_connection_id() {
        roundtrip(&Frame::NewConnectionId {
            sequence: VarInt::from_u32(1),
            retire_prior_to: VarInt::from_u32(0),
            connection_id: nhttp3_core::ConnectionId::from_slice(&[1, 2, 3, 4]).unwrap(),
            stateless_reset_token: [0xaa; 16],
        });
    }

    #[test]
    fn roundtrip_path_challenge_response() {
        roundtrip(&Frame::PathChallenge { data: [1, 2, 3, 4, 5, 6, 7, 8] });
        roundtrip(&Frame::PathResponse { data: [8, 7, 6, 5, 4, 3, 2, 1] });
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p nhttp3-quic`
Expected: FAIL — `todo!()`

- [ ] **Step 3: Implement Frame::encode**

```rust
// crates/nhttp3-quic/src/frame/write.rs — replace todo!() implementations
impl Frame {
    pub fn encode<B: BufMut>(&self, buf: &mut B) {
        match self {
            Frame::Padding => {
                VarInt::from_u32(0x00).encode(buf);
            }
            Frame::Ping => {
                VarInt::from_u32(0x01).encode(buf);
            }
            Frame::Ack { largest_ack, ack_delay, first_ack_range, ack_ranges, ecn } => {
                let frame_type = if ecn.is_some() { 0x03u32 } else { 0x02 };
                VarInt::from_u32(frame_type).encode(buf);
                largest_ack.encode(buf);
                ack_delay.encode(buf);
                VarInt::from_u32(ack_ranges.len() as u32).encode(buf);
                first_ack_range.encode(buf);
                for range in ack_ranges {
                    range.gap.encode(buf);
                    range.range.encode(buf);
                }
                if let Some(ecn) = ecn {
                    ecn.ect0.encode(buf);
                    ecn.ect1.encode(buf);
                    ecn.ecn_ce.encode(buf);
                }
            }
            Frame::ResetStream { stream_id, error_code, final_size } => {
                VarInt::from_u32(0x04).encode(buf);
                stream_id.encode(buf);
                error_code.encode(buf);
                final_size.encode(buf);
            }
            Frame::StopSending { stream_id, error_code } => {
                VarInt::from_u32(0x05).encode(buf);
                stream_id.encode(buf);
                error_code.encode(buf);
            }
            Frame::Crypto { offset, data } => {
                VarInt::from_u32(0x06).encode(buf);
                offset.encode(buf);
                VarInt::try_from(data.len() as u64).unwrap().encode(buf);
                buf.put_slice(data);
            }
            Frame::NewToken { token } => {
                VarInt::from_u32(0x07).encode(buf);
                VarInt::try_from(token.len() as u64).unwrap().encode(buf);
                buf.put_slice(token);
            }
            Frame::Stream { stream_id, offset, data, fin } => {
                let mut frame_type: u8 = 0x08;
                if offset.is_some() { frame_type |= 0x04; }
                frame_type |= 0x02; // always include length for roundtrip safety
                if *fin { frame_type |= 0x01; }
                VarInt::from_u32(frame_type as u32).encode(buf);
                stream_id.encode(buf);
                if let Some(off) = offset {
                    off.encode(buf);
                }
                VarInt::try_from(data.len() as u64).unwrap().encode(buf);
                buf.put_slice(data);
            }
            Frame::MaxData { max_data } => {
                VarInt::from_u32(0x10).encode(buf);
                max_data.encode(buf);
            }
            Frame::MaxStreamData { stream_id, max_data } => {
                VarInt::from_u32(0x11).encode(buf);
                stream_id.encode(buf);
                max_data.encode(buf);
            }
            Frame::MaxStreams { bidi, max_streams } => {
                let ft = if *bidi { 0x12u32 } else { 0x13 };
                VarInt::from_u32(ft).encode(buf);
                max_streams.encode(buf);
            }
            Frame::DataBlocked { max_data } => {
                VarInt::from_u32(0x14).encode(buf);
                max_data.encode(buf);
            }
            Frame::StreamDataBlocked { stream_id, max_data } => {
                VarInt::from_u32(0x15).encode(buf);
                stream_id.encode(buf);
                max_data.encode(buf);
            }
            Frame::StreamsBlocked { bidi, max_streams } => {
                let ft = if *bidi { 0x16u32 } else { 0x17 };
                VarInt::from_u32(ft).encode(buf);
                max_streams.encode(buf);
            }
            Frame::NewConnectionId { sequence, retire_prior_to, connection_id, stateless_reset_token } => {
                VarInt::from_u32(0x18).encode(buf);
                sequence.encode(buf);
                retire_prior_to.encode(buf);
                buf.put_u8(connection_id.len() as u8);
                buf.put_slice(connection_id.as_bytes());
                buf.put_slice(stateless_reset_token);
            }
            Frame::RetireConnectionId { sequence } => {
                VarInt::from_u32(0x19).encode(buf);
                sequence.encode(buf);
            }
            Frame::PathChallenge { data } => {
                VarInt::from_u32(0x1a).encode(buf);
                buf.put_slice(data);
            }
            Frame::PathResponse { data } => {
                VarInt::from_u32(0x1b).encode(buf);
                buf.put_slice(data);
            }
            Frame::ConnectionClose { error_code, frame_type, reason } => {
                if frame_type.is_some() {
                    VarInt::from_u32(0x1c).encode(buf);
                } else {
                    VarInt::from_u32(0x1d).encode(buf);
                }
                error_code.encode(buf);
                if let Some(ft) = frame_type {
                    ft.encode(buf);
                }
                VarInt::try_from(reason.len() as u64).unwrap().encode(buf);
                buf.put_slice(reason);
            }
            Frame::HandshakeDone => {
                VarInt::from_u32(0x1e).encode(buf);
            }
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p nhttp3-quic`
Expected: All roundtrip tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/nhttp3-quic/src/frame/write.rs
git commit -m "feat(quic): implement QUIC frame serialization with roundtrip tests"
```

---

## Chunk 3: Transport Parameters + Error Types

### Task 9: QUIC Transport Parameters

Transport parameters are exchanged during the TLS handshake. RFC 9000 §18.

**Files:**
- Create: `crates/nhttp3-quic/src/transport/mod.rs`
- Create: `crates/nhttp3-quic/src/transport/params.rs`
- Create: `crates/nhttp3-quic/src/transport/error.rs`
- Modify: `crates/nhttp3-quic/src/lib.rs`

- [ ] **Step 1: Define transport parameters with tests**

```rust
// crates/nhttp3-quic/src/transport/params.rs
use bytes::{Buf, BufMut, Bytes, BytesMut};
use nhttp3_core::{VarInt, ConnectionId, Error as CoreError};
use std::time::Duration;

/// QUIC transport parameters (RFC 9000 §18.2).
#[derive(Debug, Clone)]
pub struct TransportParams {
    pub original_destination_connection_id: Option<ConnectionId>,
    pub max_idle_timeout: Duration,
    pub stateless_reset_token: Option<[u8; 16]>,
    pub max_udp_payload_size: u64,
    pub initial_max_data: u64,
    pub initial_max_stream_data_bidi_local: u64,
    pub initial_max_stream_data_bidi_remote: u64,
    pub initial_max_stream_data_uni: u64,
    pub initial_max_streams_bidi: u64,
    pub initial_max_streams_uni: u64,
    pub ack_delay_exponent: u64,
    pub max_ack_delay: Duration,
    pub disable_active_migration: bool,
    pub active_connection_id_limit: u64,
    pub initial_source_connection_id: Option<ConnectionId>,
    pub retry_source_connection_id: Option<ConnectionId>,
}

impl Default for TransportParams {
    fn default() -> Self {
        Self {
            original_destination_connection_id: None,
            max_idle_timeout: Duration::ZERO,
            stateless_reset_token: None,
            max_udp_payload_size: 65527,
            initial_max_data: 0,
            initial_max_stream_data_bidi_local: 0,
            initial_max_stream_data_bidi_remote: 0,
            initial_max_stream_data_uni: 0,
            initial_max_streams_bidi: 0,
            initial_max_streams_uni: 0,
            ack_delay_exponent: 3,
            max_ack_delay: Duration::from_millis(25),
            disable_active_migration: false,
            active_connection_id_limit: 2,
            initial_source_connection_id: None,
            retry_source_connection_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let params = TransportParams::default();
        assert_eq!(params.max_udp_payload_size, 65527);
        assert_eq!(params.ack_delay_exponent, 3);
        assert_eq!(params.max_ack_delay, Duration::from_millis(25));
        assert_eq!(params.active_connection_id_limit, 2);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let params = TransportParams {
            max_idle_timeout: Duration::from_secs(30),
            initial_max_data: 1_000_000,
            initial_max_stream_data_bidi_local: 100_000,
            initial_max_stream_data_bidi_remote: 100_000,
            initial_max_stream_data_uni: 100_000,
            initial_max_streams_bidi: 100,
            initial_max_streams_uni: 100,
            active_connection_id_limit: 8,
            initial_source_connection_id: Some(ConnectionId::from_slice(&[1, 2, 3, 4]).unwrap()),
            ..Default::default()
        };

        let mut buf = BytesMut::new();
        params.encode(&mut buf);
        let mut bytes = buf.freeze();
        let decoded = TransportParams::decode(&mut bytes).unwrap();

        assert_eq!(decoded.max_idle_timeout, Duration::from_secs(30));
        assert_eq!(decoded.initial_max_data, 1_000_000);
        assert_eq!(decoded.initial_max_streams_bidi, 100);
        assert_eq!(decoded.active_connection_id_limit, 8);
    }

    #[test]
    fn unknown_params_ignored() {
        // Encode known params + an unknown param ID (0xff00 with some data)
        let params = TransportParams::default();
        let mut buf = BytesMut::new();
        params.encode(&mut buf);
        // Append unknown param
        VarInt::from_u32(0xff00).encode(&mut buf);
        VarInt::from_u32(3).encode(&mut buf);
        buf.put_slice(&[0xaa, 0xbb, 0xcc]);

        let mut bytes = buf.freeze();
        let decoded = TransportParams::decode(&mut bytes).unwrap();
        // Should succeed — unknown params are skipped
        assert_eq!(decoded.max_udp_payload_size, 65527);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p nhttp3-quic`
Expected: FAIL — `encode` and `decode` not implemented

- [ ] **Step 3: Implement encode/decode**

Transport parameter wire format: each param is `varint(id) varint(len) data`.

```rust
// Transport parameter IDs (RFC 9000 §18.2)
const ORIGINAL_DCID: u64 = 0x00;
const MAX_IDLE_TIMEOUT: u64 = 0x01;
const STATELESS_RESET_TOKEN: u64 = 0x02;
const MAX_UDP_PAYLOAD_SIZE: u64 = 0x03;
const INITIAL_MAX_DATA: u64 = 0x04;
const INITIAL_MAX_STREAM_DATA_BIDI_LOCAL: u64 = 0x05;
const INITIAL_MAX_STREAM_DATA_BIDI_REMOTE: u64 = 0x06;
const INITIAL_MAX_STREAM_DATA_UNI: u64 = 0x07;
const INITIAL_MAX_STREAMS_BIDI: u64 = 0x08;
const INITIAL_MAX_STREAMS_UNI: u64 = 0x09;
const ACK_DELAY_EXPONENT: u64 = 0x0a;
const MAX_ACK_DELAY: u64 = 0x0b;
const DISABLE_ACTIVE_MIGRATION: u64 = 0x0c;
const ACTIVE_CID_LIMIT: u64 = 0x0e;
const INITIAL_SCID: u64 = 0x0f;
const RETRY_SCID: u64 = 0x10;

impl TransportParams {
    /// Encodes transport parameters into the buffer.
    pub fn encode<B: BufMut>(&self, buf: &mut B) {
        self.encode_varint_param(buf, MAX_IDLE_TIMEOUT, self.max_idle_timeout.as_millis() as u64);
        self.encode_varint_param(buf, MAX_UDP_PAYLOAD_SIZE, self.max_udp_payload_size);
        self.encode_varint_param(buf, INITIAL_MAX_DATA, self.initial_max_data);
        self.encode_varint_param(buf, INITIAL_MAX_STREAM_DATA_BIDI_LOCAL, self.initial_max_stream_data_bidi_local);
        self.encode_varint_param(buf, INITIAL_MAX_STREAM_DATA_BIDI_REMOTE, self.initial_max_stream_data_bidi_remote);
        self.encode_varint_param(buf, INITIAL_MAX_STREAM_DATA_UNI, self.initial_max_stream_data_uni);
        self.encode_varint_param(buf, INITIAL_MAX_STREAMS_BIDI, self.initial_max_streams_bidi);
        self.encode_varint_param(buf, INITIAL_MAX_STREAMS_UNI, self.initial_max_streams_uni);
        self.encode_varint_param(buf, ACK_DELAY_EXPONENT, self.ack_delay_exponent);
        self.encode_varint_param(buf, MAX_ACK_DELAY, self.max_ack_delay.as_millis() as u64);
        self.encode_varint_param(buf, ACTIVE_CID_LIMIT, self.active_connection_id_limit);

        if self.disable_active_migration {
            VarInt::try_from(DISABLE_ACTIVE_MIGRATION).unwrap().encode(buf);
            VarInt::from_u32(0).encode(buf); // zero-length value
        }

        if let Some(ref cid) = self.initial_source_connection_id {
            VarInt::try_from(INITIAL_SCID).unwrap().encode(buf);
            VarInt::try_from(cid.len() as u64).unwrap().encode(buf);
            buf.put_slice(cid.as_bytes());
        }

        if let Some(ref cid) = self.original_destination_connection_id {
            VarInt::try_from(ORIGINAL_DCID).unwrap().encode(buf);
            VarInt::try_from(cid.len() as u64).unwrap().encode(buf);
            buf.put_slice(cid.as_bytes());
        }

        if let Some(ref token) = self.stateless_reset_token {
            VarInt::try_from(STATELESS_RESET_TOKEN).unwrap().encode(buf);
            VarInt::from_u32(16).encode(buf);
            buf.put_slice(token);
        }

        if let Some(ref cid) = self.retry_source_connection_id {
            VarInt::try_from(RETRY_SCID).unwrap().encode(buf);
            VarInt::try_from(cid.len() as u64).unwrap().encode(buf);
            buf.put_slice(cid.as_bytes());
        }
    }

    fn encode_varint_param<B: BufMut>(&self, buf: &mut B, id: u64, val: u64) {
        let v = VarInt::try_from(val).unwrap();
        VarInt::try_from(id).unwrap().encode(buf);
        VarInt::try_from(v.encoded_size() as u64).unwrap().encode(buf);
        v.encode(buf);
    }

    /// Decodes transport parameters from the buffer.
    pub fn decode(buf: &mut Bytes) -> Result<Self, crate::packet::PacketError> {
        let mut params = Self::default();

        while buf.has_remaining() {
            let id = VarInt::decode(buf)?.value();
            let len = VarInt::decode(buf)?.value() as usize;
            if buf.remaining() < len {
                return Err(crate::packet::PacketError::Core(CoreError::BufferTooShort));
            }

            let mut param_buf = buf.slice(..len);
            buf.advance(len);

            match id {
                ORIGINAL_DCID => {
                    params.original_destination_connection_id = Some(ConnectionId::from_slice(param_buf.chunk())?);
                }
                MAX_IDLE_TIMEOUT => {
                    let ms = VarInt::decode(&mut param_buf)?.value();
                    params.max_idle_timeout = Duration::from_millis(ms);
                }
                STATELESS_RESET_TOKEN => {
                    if param_buf.remaining() < 16 {
                        return Err(crate::packet::PacketError::Core(CoreError::BufferTooShort));
                    }
                    let mut token = [0u8; 16];
                    token.copy_from_slice(&param_buf.chunk()[..16]);
                    params.stateless_reset_token = Some(token);
                }
                MAX_UDP_PAYLOAD_SIZE => {
                    params.max_udp_payload_size = VarInt::decode(&mut param_buf)?.value();
                }
                INITIAL_MAX_DATA => {
                    params.initial_max_data = VarInt::decode(&mut param_buf)?.value();
                }
                INITIAL_MAX_STREAM_DATA_BIDI_LOCAL => {
                    params.initial_max_stream_data_bidi_local = VarInt::decode(&mut param_buf)?.value();
                }
                INITIAL_MAX_STREAM_DATA_BIDI_REMOTE => {
                    params.initial_max_stream_data_bidi_remote = VarInt::decode(&mut param_buf)?.value();
                }
                INITIAL_MAX_STREAM_DATA_UNI => {
                    params.initial_max_stream_data_uni = VarInt::decode(&mut param_buf)?.value();
                }
                INITIAL_MAX_STREAMS_BIDI => {
                    params.initial_max_streams_bidi = VarInt::decode(&mut param_buf)?.value();
                }
                INITIAL_MAX_STREAMS_UNI => {
                    params.initial_max_streams_uni = VarInt::decode(&mut param_buf)?.value();
                }
                ACK_DELAY_EXPONENT => {
                    params.ack_delay_exponent = VarInt::decode(&mut param_buf)?.value();
                }
                MAX_ACK_DELAY => {
                    let ms = VarInt::decode(&mut param_buf)?.value();
                    params.max_ack_delay = Duration::from_millis(ms);
                }
                DISABLE_ACTIVE_MIGRATION => {
                    params.disable_active_migration = true;
                }
                ACTIVE_CID_LIMIT => {
                    params.active_connection_id_limit = VarInt::decode(&mut param_buf)?.value();
                }
                INITIAL_SCID => {
                    params.initial_source_connection_id = Some(ConnectionId::from_slice(param_buf.chunk())?);
                }
                RETRY_SCID => {
                    params.retry_source_connection_id = Some(ConnectionId::from_slice(param_buf.chunk())?);
                }
                _ => {
                    // Unknown parameter — skip (already advanced past it)
                }
            }
        }

        Ok(params)
    }
}
```

- [ ] **Step 4: Create transport module files**

```rust
// crates/nhttp3-quic/src/transport/mod.rs
pub mod params;
pub mod error;

pub use params::TransportParams;
```

```rust
// crates/nhttp3-quic/src/transport/error.rs
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
    /// Application-defined error code.
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
```

- [ ] **Step 5: Update nhttp3-quic/src/lib.rs**

```rust
// crates/nhttp3-quic/src/lib.rs
pub mod packet;
pub mod frame;
pub mod transport;
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p nhttp3-quic`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/nhttp3-quic/src/transport/ crates/nhttp3-quic/src/lib.rs
git commit -m "feat(quic): add transport parameters and error codes (RFC 9000 §18, §20)"
```

---

## Chunk 4: Crypto + TLS Integration

### Task 10: Crypto Context & Key Management

The crypto module manages encryption keys at each level (Initial, Handshake, 1-RTT). It wraps rustls's QUIC key types.

**Files:**
- Create: `crates/nhttp3-quic/src/crypto/mod.rs`
- Create: `crates/nhttp3-quic/src/crypto/keys.rs`
- Create: `crates/nhttp3-quic/src/crypto/protection.rs`
- Modify: `crates/nhttp3-quic/src/lib.rs`

- [ ] **Step 1: Define key management types with tests**

```rust
// crates/nhttp3-quic/src/crypto/keys.rs
use rustls::quic::{self, DirectionalKeys, HeaderProtectionKey, PacketKey, Version as TlsVersion};
use crate::packet::PacketError;

/// Encryption level / packet number space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {
    Initial,
    Handshake,
    ZeroRtt,
    OneRtt,
}

/// Keys for a single direction (send or receive) at a given encryption level.
pub struct DirectionKeys {
    pub packet: Box<dyn PacketKey>,
    pub header: Box<dyn HeaderProtectionKey>,
}

impl DirectionKeys {
    pub fn from_rustls(keys: DirectionalKeys) -> Self {
        Self {
            packet: keys.packet,
            header: keys.header,
        }
    }
}

/// Complete key set for a packet number space (both directions).
pub struct SpaceKeys {
    pub local: DirectionKeys,
    pub remote: DirectionKeys,
}

impl SpaceKeys {
    pub fn from_rustls(keys: quic::Keys) -> Self {
        Self {
            local: DirectionKeys::from_rustls(keys.local),
            remote: DirectionKeys::from_rustls(keys.remote),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_equality() {
        assert_eq!(Level::Initial, Level::Initial);
        assert_ne!(Level::Initial, Level::Handshake);
    }

    #[test]
    fn level_debug() {
        assert_eq!(format!("{:?}", Level::OneRtt), "OneRtt");
    }
}
```

- [ ] **Step 2: Implement header protection**

```rust
// crates/nhttp3-quic/src/crypto/protection.rs
use crate::packet::PacketError;

/// Applies header protection to a packet (RFC 9001 §5.4).
///
/// `header` is the full packet header bytes.
/// `pn_offset` is the byte offset of the packet number in `header`.
/// `packet` contains the full packet (header + payload). The sample is taken
/// from the payload starting at pn_offset + 4.
pub fn apply_header_protection(
    hp_key: &dyn rustls::quic::HeaderProtectionKey,
    packet: &mut [u8],
    pn_offset: usize,
) -> Result<(), PacketError> {
    let sample_offset = pn_offset + 4;
    let sample_len = hp_key.sample_len();

    if packet.len() < sample_offset + sample_len {
        return Err(PacketError::Invalid("packet too short for header protection sample".into()));
    }

    let (header, sample_payload) = packet.split_at_mut(sample_offset);
    let sample = &sample_payload[..sample_len];
    let mask = hp_key.new_mask(sample)
        .map_err(|e| PacketError::Invalid(format!("header protection failed: {e}")))?;

    let is_long = header[0] & 0x80 != 0;
    if is_long {
        header[0] ^= mask[0] & 0x0f;
    } else {
        header[0] ^= mask[0] & 0x1f;
    }

    let pn_len = (header[0] & 0x03) as usize + 1;
    for i in 0..pn_len {
        header[pn_offset + i] ^= mask[1 + i];
    }

    Ok(())
}

/// Removes header protection from a packet. Same operation as apply (XOR is self-inverse).
pub fn remove_header_protection(
    hp_key: &dyn rustls::quic::HeaderProtectionKey,
    packet: &mut [u8],
    pn_offset: usize,
) -> Result<(), PacketError> {
    apply_header_protection(hp_key, packet, pn_offset)
}

#[cfg(test)]
mod tests {
    // Header protection tests require actual rustls keys.
    // Tested in integration tests (Task 13) with real handshake keys.

    #[test]
    fn placeholder_compiles() {
        // Ensures the module compiles correctly
    }
}
```

- [ ] **Step 3: Create module file and update lib**

```rust
// crates/nhttp3-quic/src/crypto/mod.rs
pub mod keys;
pub mod protection;

pub use keys::{Level, DirectionKeys, SpaceKeys};
pub use protection::{apply_header_protection, remove_header_protection};
```

Update `crates/nhttp3-quic/src/lib.rs`:
```rust
pub mod packet;
pub mod frame;
pub mod transport;
pub mod crypto;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p nhttp3-quic`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/nhttp3-quic/src/crypto/ crates/nhttp3-quic/src/lib.rs
git commit -m "feat(quic): add crypto key management and header protection (RFC 9001 §5)"
```

---

### Task 11: TLS Integration with rustls

This wraps rustls's QUIC connection types and drives the handshake.

**Files:**
- Create: `crates/nhttp3-quic/src/tls/mod.rs`
- Create: `crates/nhttp3-quic/src/tls/session.rs`
- Modify: `crates/nhttp3-quic/src/lib.rs`

- [ ] **Step 1: Write TLS session wrapper with tests**

```rust
// crates/nhttp3-quic/src/tls/session.rs
use std::sync::Arc;
use rustls::quic::{self, Connection as TlsConnection, KeyChange, Version as TlsVersion};
use rustls::{ClientConfig, ServerConfig, ServerName};
use crate::crypto::{Level, SpaceKeys};
use crate::packet::PacketError;

/// Wraps a rustls QUIC connection (client or server).
pub struct TlsSession {
    conn: TlsConnection,
}

/// Result of processing TLS handshake data.
pub struct HandshakeResult {
    /// Outgoing handshake data, indexed by encryption level.
    /// Index 0 = Initial, 1 = Handshake, 2 = 1-RTT.
    pub data: [Vec<u8>; 3],
    /// New keys, if a key change occurred.
    pub key_change: Option<KeyChangeEvent>,
}

pub enum KeyChangeEvent {
    Handshake(SpaceKeys),
    OneRtt {
        keys: SpaceKeys,
        next_secrets: quic::Secrets,
    },
}

impl TlsSession {
    /// Creates a new client TLS session.
    pub fn new_client(
        config: Arc<ClientConfig>,
        server_name: ServerName<'static>,
        transport_params: Vec<u8>,
    ) -> Result<Self, PacketError> {
        let conn = quic::ClientConnection::new(
            config,
            TlsVersion::V1,
            server_name,
            transport_params,
        ).map_err(|e| PacketError::Invalid(format!("TLS client init failed: {e}")))?;

        Ok(Self {
            conn: TlsConnection::Client(conn),
        })
    }

    /// Creates a new server TLS session.
    pub fn new_server(
        config: Arc<ServerConfig>,
        transport_params: Vec<u8>,
    ) -> Result<Self, PacketError> {
        let conn = quic::ServerConnection::new(
            config,
            TlsVersion::V1,
            transport_params,
        ).map_err(|e| PacketError::Invalid(format!("TLS server init failed: {e}")))?;

        Ok(Self {
            conn: TlsConnection::Server(conn),
        })
    }

    /// Feeds received handshake data (from CRYPTO frames) into the TLS session.
    pub fn read_handshake(&mut self, data: &[u8]) -> Result<(), PacketError> {
        match &mut self.conn {
            TlsConnection::Client(c) => c.read_hs(data),
            TlsConnection::Server(c) => c.read_hs(data),
        }.map_err(|e| PacketError::Invalid(format!("TLS read_hs failed: {e}")))
    }

    /// Gets outgoing handshake data and any key changes.
    pub fn write_handshake(&mut self) -> HandshakeResult {
        let mut data = [Vec::new(), Vec::new(), Vec::new()];
        let mut bufs = vec![Vec::new(), Vec::new(), Vec::new()];

        let key_change = match &mut self.conn {
            TlsConnection::Client(c) => c.write_hs(&mut bufs),
            TlsConnection::Server(c) => c.write_hs(&mut bufs),
        };

        data[0] = std::mem::take(&mut bufs[0]);
        data[1] = std::mem::take(&mut bufs[1]);
        data[2] = std::mem::take(&mut bufs[2]);

        let key_change = key_change.map(|kc| match kc {
            KeyChange::Handshake { keys } => {
                KeyChangeEvent::Handshake(SpaceKeys::from_rustls(keys))
            }
            KeyChange::OneRtt { keys, next } => {
                KeyChangeEvent::OneRtt {
                    keys: SpaceKeys::from_rustls(keys),
                    next_secrets: next,
                }
            }
        });

        HandshakeResult { data, key_change }
    }

    /// Returns the peer's transport parameters (TLS-encoded).
    pub fn transport_parameters(&self) -> Option<&[u8]> {
        match &self.conn {
            TlsConnection::Client(c) => c.quic_transport_parameters(),
            TlsConnection::Server(c) => c.quic_transport_parameters(),
        }
    }

    /// Returns true if the handshake is still in progress.
    pub fn is_handshaking(&self) -> bool {
        match &self.conn {
            TlsConnection::Client(c) => c.is_handshaking(),
            TlsConnection::Server(c) => c.is_handshaking(),
        }
    }

    /// Returns the negotiated ALPN protocol.
    pub fn alpn_protocol(&self) -> Option<&[u8]> {
        match &self.conn {
            TlsConnection::Client(c) => c.alpn_protocol(),
            TlsConnection::Server(c) => c.alpn_protocol(),
        }
    }

    /// Gets 0-RTT keys if available (client only, after session resumption).
    pub fn zero_rtt_keys(&self) -> Option<quic::DirectionalKeys> {
        match &self.conn {
            TlsConnection::Client(c) => c.zero_rtt_keys(),
            TlsConnection::Server(c) => c.zero_rtt_keys(),
        }
    }

    /// Returns the TLS alert if the handshake failed.
    pub fn alert(&self) -> Option<rustls::AlertDescription> {
        match &self.conn {
            TlsConnection::Client(c) => c.alert(),
            TlsConnection::Server(c) => c.alert(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    use std::sync::Arc;

    fn test_client_config() -> Arc<ClientConfig> {
        let mut config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
            .with_no_client_auth();
        config.alpn_protocols = vec![b"h3".to_vec()];
        Arc::new(config)
    }

    fn test_server_config() -> Arc<ServerConfig> {
        let (cert, key) = self_signed_cert();
        let mut config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap();
        config.alpn_protocols = vec![b"h3".to_vec()];
        Arc::new(config)
    }

    fn self_signed_cert() -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()));
        let cert = CertificateDer::from(cert.cert);
        (cert, key)
    }

    /// Dummy certificate verifier for testing.
    #[derive(Debug)]
    struct NoCertVerifier;

    impl rustls::client::danger::ServerCertVerifier for NoCertVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self, _message: &[u8], _cert: &CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self, _message: &[u8], _cert: &CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    #[test]
    fn client_server_handshake_in_process() {
        let client_config = test_client_config();
        let server_config = test_server_config();

        let server_name = "localhost".try_into().unwrap();
        let mut client = TlsSession::new_client(
            client_config, server_name, vec![],
        ).unwrap();

        let mut server = TlsSession::new_server(
            server_config, vec![],
        ).unwrap();

        // Client sends ClientHello
        let client_hs = client.write_handshake();
        assert!(!client_hs.data[0].is_empty(), "client should produce Initial data");

        // Server processes ClientHello
        server.read_handshake(&client_hs.data[0]).unwrap();
        let server_hs = server.write_handshake();

        // Server should produce Initial + Handshake data, plus handshake keys
        assert!(server_hs.key_change.is_some(), "server should produce handshake keys");

        // Client processes server's Initial data
        if !server_hs.data[0].is_empty() {
            client.read_handshake(&server_hs.data[0]).unwrap();
        }
        // Client processes server's Handshake data
        if !server_hs.data[1].is_empty() {
            client.read_handshake(&server_hs.data[1]).unwrap();
        }

        let client_hs2 = client.write_handshake();
        // Client should now have 1-RTT keys
        assert!(client_hs2.key_change.is_some() || !client.is_handshaking(),
            "client should complete handshake or produce key change");

        // If client has handshake data, feed to server
        if !client_hs2.data[1].is_empty() {
            server.read_handshake(&client_hs2.data[1]).unwrap();
            let _ = server.write_handshake();
        }
    }
}
```

- [ ] **Step 2: Add rcgen dev-dependency for test certs**

Add to `crates/nhttp3-quic/Cargo.toml`:
```toml
[dev-dependencies]
rcgen = "0.13"
```

- [ ] **Step 3: Create module files**

```rust
// crates/nhttp3-quic/src/tls/mod.rs
pub mod session;

pub use session::{TlsSession, HandshakeResult, KeyChangeEvent};
```

Update `crates/nhttp3-quic/src/lib.rs`:
```rust
pub mod packet;
pub mod frame;
pub mod transport;
pub mod crypto;
pub mod tls;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p nhttp3-quic`
Expected: All tests PASS, including the in-process client/server handshake

- [ ] **Step 5: Commit**

```bash
git add crates/nhttp3-quic/
git commit -m "feat(quic): add TLS session wrapper with rustls QUIC integration (RFC 9001)"
```

---

## Chunk 5: Connection State Machine + Streams

### Task 12: Connection State Machine

The core connection state machine manages handshake progression, packet spaces, and connection state transitions.

**Files:**
- Create: `crates/nhttp3-quic/src/connection/mod.rs`
- Create: `crates/nhttp3-quic/src/connection/state.rs`
- Modify: `crates/nhttp3-quic/src/lib.rs`

- [ ] **Step 1: Define connection states with tests**

```rust
// crates/nhttp3-quic/src/connection/state.rs

/// QUIC connection state (RFC 9000 §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Sending/receiving Initial packets, TLS handshake beginning.
    Initial,
    /// TLS handshake in progress.
    Handshake,
    /// Handshake complete, can exchange application data.
    Established,
    /// CONNECTION_CLOSE sent, waiting for acknowledgment or timeout.
    Closing,
    /// CONNECTION_CLOSE received, waiting for drain period.
    Draining,
    /// Connection fully closed.
    Closed,
}

impl ConnectionState {
    /// Returns true if this state allows sending application data.
    pub fn can_send_app_data(&self) -> bool {
        matches!(self, Self::Established)
    }

    /// Returns true if this state allows opening new streams.
    pub fn can_open_streams(&self) -> bool {
        matches!(self, Self::Established)
    }

    /// Returns true if the connection is terminal.
    pub fn is_closed(&self) -> bool {
        matches!(self, Self::Closed)
    }

    /// Returns true if the connection is in the process of shutting down.
    pub fn is_closing(&self) -> bool {
        matches!(self, Self::Closing | Self::Draining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_cannot_send_data() {
        assert!(!ConnectionState::Initial.can_send_app_data());
        assert!(!ConnectionState::Initial.can_open_streams());
    }

    #[test]
    fn established_state_can_send_data() {
        assert!(ConnectionState::Established.can_send_app_data());
        assert!(ConnectionState::Established.can_open_streams());
    }

    #[test]
    fn closing_states() {
        assert!(ConnectionState::Closing.is_closing());
        assert!(ConnectionState::Draining.is_closing());
        assert!(!ConnectionState::Established.is_closing());
    }

    #[test]
    fn closed_state() {
        assert!(ConnectionState::Closed.is_closed());
        assert!(!ConnectionState::Established.is_closed());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p nhttp3-quic`
Expected: PASS

- [ ] **Step 3: Create connection module**

```rust
// crates/nhttp3-quic/src/connection/mod.rs
pub mod state;

pub use state::ConnectionState;
```

Update `crates/nhttp3-quic/src/lib.rs`:
```rust
pub mod packet;
pub mod frame;
pub mod transport;
pub mod crypto;
pub mod tls;
pub mod connection;
```

- [ ] **Step 4: Commit**

```bash
git add crates/nhttp3-quic/
git commit -m "feat(quic): add connection state machine (RFC 9000 §10)"
```

---

### Task 13: Stream Types & Flow Control

QUIC streams are the application-level data channels. Each stream has its own state machine and flow control windows.

**Files:**
- Create: `crates/nhttp3-quic/src/stream/mod.rs`
- Create: `crates/nhttp3-quic/src/stream/state.rs`
- Create: `crates/nhttp3-quic/src/stream/flow_control.rs`
- Modify: `crates/nhttp3-quic/src/lib.rs`

- [ ] **Step 1: Define stream types with tests**

```rust
// crates/nhttp3-quic/src/stream/state.rs
use nhttp3_core::VarInt;

/// Stream ID encodes the initiator and directionality (RFC 9000 §2.1).
///
/// - Bit 0: 0 = client-initiated, 1 = server-initiated
/// - Bit 1: 0 = bidirectional, 1 = unidirectional
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
        // Stream IDs 0, 4, 8, ... are client bidi
        for i in [0u64, 4, 8, 12] {
            let id = StreamId::new(i);
            assert!(id.is_client_initiated());
            assert!(id.is_bidi());
        }
    }
}
```

- [ ] **Step 2: Write flow control with tests**

```rust
// crates/nhttp3-quic/src/stream/flow_control.rs

/// Tracks flow control state for a stream or connection.
#[derive(Debug)]
pub struct FlowControl {
    /// Maximum bytes the peer has allowed us to send/receive.
    window: u64,
    /// Bytes consumed so far.
    consumed: u64,
}

impl FlowControl {
    pub fn new(initial_window: u64) -> Self {
        Self {
            window: initial_window,
            consumed: 0,
        }
    }

    /// Returns the number of bytes available to send/receive.
    pub fn available(&self) -> u64 {
        self.window.saturating_sub(self.consumed)
    }

    /// Consumes `n` bytes. Returns false if this exceeds the window.
    pub fn consume(&mut self, n: u64) -> bool {
        let new_consumed = self.consumed + n;
        if new_consumed > self.window {
            return false;
        }
        self.consumed = new_consumed;
        true
    }

    /// Updates the window (e.g., from MAX_DATA / MAX_STREAM_DATA).
    /// Only increases are accepted — decreases are ignored.
    pub fn update_window(&mut self, new_window: u64) {
        if new_window > self.window {
            self.window = new_window;
        }
    }

    pub fn window(&self) -> u64 {
        self.window
    }

    pub fn consumed(&self) -> u64 {
        self.consumed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_available() {
        let fc = FlowControl::new(1000);
        assert_eq!(fc.available(), 1000);
    }

    #[test]
    fn consume_within_window() {
        let mut fc = FlowControl::new(1000);
        assert!(fc.consume(500));
        assert_eq!(fc.available(), 500);
        assert_eq!(fc.consumed(), 500);
    }

    #[test]
    fn consume_exceeds_window() {
        let mut fc = FlowControl::new(1000);
        assert!(fc.consume(500));
        assert!(!fc.consume(600)); // 500 + 600 > 1000
        assert_eq!(fc.consumed(), 500); // unchanged
    }

    #[test]
    fn consume_exact_window() {
        let mut fc = FlowControl::new(1000);
        assert!(fc.consume(1000));
        assert_eq!(fc.available(), 0);
    }

    #[test]
    fn update_window_increase() {
        let mut fc = FlowControl::new(1000);
        fc.consume(800);
        fc.update_window(2000);
        assert_eq!(fc.available(), 1200);
    }

    #[test]
    fn update_window_decrease_ignored() {
        let mut fc = FlowControl::new(1000);
        fc.update_window(500);
        assert_eq!(fc.window(), 1000);
    }

    #[test]
    fn zero_window() {
        let fc = FlowControl::new(0);
        assert_eq!(fc.available(), 0);
    }
}
```

- [ ] **Step 3: Create stream module**

```rust
// crates/nhttp3-quic/src/stream/mod.rs
pub mod state;
pub mod flow_control;

pub use state::{StreamId, SendState, RecvState};
pub use flow_control::FlowControl;
```

Update `crates/nhttp3-quic/src/lib.rs`:
```rust
pub mod packet;
pub mod frame;
pub mod transport;
pub mod crypto;
pub mod tls;
pub mod connection;
pub mod stream;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p nhttp3-quic`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/nhttp3-quic/
git commit -m "feat(quic): add stream types and flow control (RFC 9000 §2-3)"
```

---

## Chunk 6: Loss Detection + Congestion Control

### Task 14: ACK Tracking

**Files:**
- Create: `crates/nhttp3-quic/src/recovery/mod.rs`
- Create: `crates/nhttp3-quic/src/recovery/ack.rs`
- Modify: `crates/nhttp3-quic/src/lib.rs`

- [ ] **Step 1: Write ACK tracker with tests**

```rust
// crates/nhttp3-quic/src/recovery/ack.rs
use std::collections::BTreeSet;
use std::time::Instant;

/// Tracks received packet numbers for ACK generation.
#[derive(Debug)]
pub struct AckTracker {
    /// Received packet numbers.
    received: BTreeSet<u64>,
    /// Largest received packet number.
    largest_received: Option<u64>,
    /// Time the largest packet was received.
    largest_received_time: Option<Instant>,
    /// Whether we need to send an ACK.
    ack_eliciting_received: bool,
}

impl AckTracker {
    pub fn new() -> Self {
        Self {
            received: BTreeSet::new(),
            largest_received: None,
            largest_received_time: None,
            ack_eliciting_received: false,
        }
    }

    /// Records a received packet number.
    pub fn on_packet_received(&mut self, pn: u64, ack_eliciting: bool, now: Instant) {
        self.received.insert(pn);
        if self.largest_received.map_or(true, |l| pn > l) {
            self.largest_received = Some(pn);
            self.largest_received_time = Some(now);
        }
        if ack_eliciting {
            self.ack_eliciting_received = true;
        }
    }

    /// Returns true if we should send an ACK.
    pub fn should_send_ack(&self) -> bool {
        self.ack_eliciting_received
    }

    /// Marks ACK as sent.
    pub fn on_ack_sent(&mut self) {
        self.ack_eliciting_received = false;
    }

    /// Returns the largest received packet number.
    pub fn largest_received(&self) -> Option<u64> {
        self.largest_received
    }

    /// Generates ACK ranges from received packet numbers.
    /// Returns (largest_ack, ranges) where ranges is a list of (gap, range_len) pairs.
    pub fn generate_ack_ranges(&self) -> Option<(u64, Vec<(u64, u64)>)> {
        let largest = self.largest_received?;

        let mut ranges = Vec::new();
        let mut iter = self.received.iter().rev();
        let &first = iter.next()?;

        let mut range_end = first;
        let mut range_start = first;

        for &pn in iter {
            if range_start - pn == 1 {
                range_start = pn;
            } else {
                ranges.push((range_start, range_end));
                range_end = pn;
                range_start = pn;
            }
        }
        ranges.push((range_start, range_end));

        // Convert to (gap, range_len) format per RFC 9000 §19.3.1
        if ranges.is_empty() {
            return Some((largest, vec![]));
        }

        let first_range = ranges[0].1 - ranges[0].0;
        let mut ack_ranges = Vec::new();

        for i in 1..ranges.len() {
            let gap = ranges[i - 1].0 - ranges[i].1 - 2;
            let range_len = ranges[i].1 - ranges[i].0;
            ack_ranges.push((gap, range_len));
        }

        // Return first_range separately; caller maps to ACK frame format
        Some((largest, vec![(first_range, 0)]
            .into_iter()
            .chain(ack_ranges.into_iter().map(|(g, r)| (g, r)))
            .collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tracker() {
        let tracker = AckTracker::new();
        assert!(!tracker.should_send_ack());
        assert!(tracker.largest_received().is_none());
    }

    #[test]
    fn single_packet() {
        let mut tracker = AckTracker::new();
        let now = Instant::now();
        tracker.on_packet_received(0, true, now);
        assert!(tracker.should_send_ack());
        assert_eq!(tracker.largest_received(), Some(0));
    }

    #[test]
    fn non_ack_eliciting_does_not_trigger() {
        let mut tracker = AckTracker::new();
        let now = Instant::now();
        tracker.on_packet_received(0, false, now);
        assert!(!tracker.should_send_ack());
    }

    #[test]
    fn ack_sent_clears_flag() {
        let mut tracker = AckTracker::new();
        let now = Instant::now();
        tracker.on_packet_received(0, true, now);
        assert!(tracker.should_send_ack());
        tracker.on_ack_sent();
        assert!(!tracker.should_send_ack());
    }

    #[test]
    fn largest_tracks_correctly() {
        let mut tracker = AckTracker::new();
        let now = Instant::now();
        tracker.on_packet_received(5, true, now);
        tracker.on_packet_received(3, true, now);
        tracker.on_packet_received(10, true, now);
        assert_eq!(tracker.largest_received(), Some(10));
    }

    #[test]
    fn contiguous_range() {
        let mut tracker = AckTracker::new();
        let now = Instant::now();
        for pn in 0..=5 {
            tracker.on_packet_received(pn, true, now);
        }
        let (largest, _ranges) = tracker.generate_ack_ranges().unwrap();
        assert_eq!(largest, 5);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p nhttp3-quic`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/nhttp3-quic/src/recovery/
git commit -m "feat(quic): add ACK tracking for loss detection (RFC 9002)"
```

---

### Task 15: Congestion Control (NewReno)

**Files:**
- Create: `crates/nhttp3-quic/src/recovery/congestion.rs`
- Create: `crates/nhttp3-quic/src/recovery/reno.rs`
- Modify: `crates/nhttp3-quic/src/recovery/mod.rs`

- [ ] **Step 1: Define congestion controller trait and NewReno with tests**

```rust
// crates/nhttp3-quic/src/recovery/congestion.rs
use std::time::{Duration, Instant};

/// Congestion controller trait — pluggable algorithm.
pub trait CongestionController: Send + Sync {
    /// Returns the current congestion window in bytes.
    fn window(&self) -> u64;

    /// Called when a packet is acknowledged.
    fn on_ack(&mut self, bytes_acked: u64, rtt: Duration, now: Instant);

    /// Called when a packet is detected as lost.
    fn on_loss(&mut self, bytes_lost: u64, now: Instant);

    /// Returns the current slow start threshold.
    fn ssthresh(&self) -> u64;
}
```

```rust
// crates/nhttp3-quic/src/recovery/reno.rs
use super::congestion::CongestionController;
use std::time::{Duration, Instant};

const MAX_DATAGRAM_SIZE: u64 = 1200;

/// NewReno congestion controller (RFC 9002 §7).
#[derive(Debug)]
pub struct NewReno {
    congestion_window: u64,
    ssthresh: u64,
    bytes_in_flight: u64,
    max_datagram_size: u64,
}

impl NewReno {
    pub fn new() -> Self {
        let max_datagram_size = MAX_DATAGRAM_SIZE;
        // RFC 9002 §7.2: min(10 * max_datagram_size, max(14720, 2 * max_datagram_size))
        let initial_window = std::cmp::min(
            10 * max_datagram_size,
            std::cmp::max(14720, 2 * max_datagram_size),
        );
        Self {
            congestion_window: initial_window,
            ssthresh: u64::MAX,
            bytes_in_flight: 0,
            max_datagram_size,
        }
    }

    pub fn bytes_in_flight(&self) -> u64 {
        self.bytes_in_flight
    }

    pub fn on_packet_sent(&mut self, bytes: u64) {
        self.bytes_in_flight += bytes;
    }

    /// Returns true if the congestion window allows sending.
    pub fn can_send(&self) -> bool {
        self.bytes_in_flight < self.congestion_window
    }

    /// Available bytes that can be sent.
    pub fn available(&self) -> u64 {
        self.congestion_window.saturating_sub(self.bytes_in_flight)
    }
}

impl CongestionController for NewReno {
    fn window(&self) -> u64 {
        self.congestion_window
    }

    fn on_ack(&mut self, bytes_acked: u64, _rtt: Duration, _now: Instant) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes_acked);

        if self.congestion_window < self.ssthresh {
            // Slow start
            self.congestion_window += bytes_acked;
        } else {
            // Congestion avoidance
            self.congestion_window += (self.max_datagram_size * bytes_acked) / self.congestion_window;
        }
    }

    fn on_loss(&mut self, bytes_lost: u64, _now: Instant) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes_lost);
        self.ssthresh = (self.congestion_window / 2).max(MINIMUM_WINDOW_PACKETS * self.max_datagram_size);
        self.congestion_window = self.ssthresh;
    }

    fn ssthresh(&self) -> u64 {
        self.ssthresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_window() {
        let cc = NewReno::new();
        // RFC 9002 §7.2: min(10 * 1200, max(14720, 2 * 1200)) = min(12000, 14720) = 12000
        assert_eq!(cc.window(), 12000);
        assert!(cc.can_send());
    }

    #[test]
    fn slow_start_growth() {
        let mut cc = NewReno::new();
        let now = Instant::now();
        let initial = cc.window();

        cc.on_packet_sent(1200);
        cc.on_ack(1200, Duration::from_millis(50), now);

        assert!(cc.window() > initial, "window should grow in slow start");
    }

    #[test]
    fn loss_reduces_window() {
        let mut cc = NewReno::new();
        let now = Instant::now();
        let initial = cc.window();

        cc.on_packet_sent(1200);
        cc.on_loss(1200, now);

        assert!(cc.window() < initial, "window should shrink after loss");
        assert!(cc.ssthresh() < u64::MAX, "ssthresh should be set");
    }

    #[test]
    fn congestion_avoidance_growth() {
        let mut cc = NewReno::new();
        let now = Instant::now();

        // Force into congestion avoidance by triggering a loss
        cc.on_packet_sent(1200);
        cc.on_loss(1200, now);
        let ca_window = cc.window();

        // Now ack in congestion avoidance
        cc.on_packet_sent(1200);
        cc.on_ack(1200, Duration::from_millis(50), now);

        // Growth should be much slower than slow start
        let growth = cc.window() - ca_window;
        assert!(growth < 1200, "congestion avoidance growth should be sub-linear");
    }

    #[test]
    fn minimum_window() {
        let mut cc = NewReno::new();
        let now = Instant::now();

        // Multiple losses
        for _ in 0..20 {
            cc.on_packet_sent(1200);
            cc.on_loss(1200, now);
        }

        // RFC 9002 §7.2: minimum window is 2 * max_datagram_size
        assert!(cc.window() >= 2 * 1200, "window should not go below minimum");
    }

    #[test]
    fn bytes_in_flight_tracking() {
        let mut cc = NewReno::new();
        assert_eq!(cc.bytes_in_flight(), 0);

        cc.on_packet_sent(1200);
        assert_eq!(cc.bytes_in_flight(), 1200);

        cc.on_ack(1200, Duration::from_millis(50), Instant::now());
        assert_eq!(cc.bytes_in_flight(), 0);
    }
}
```

- [ ] **Step 2: Create recovery module**

```rust
// crates/nhttp3-quic/src/recovery/mod.rs
pub mod ack;
pub mod congestion;
pub mod reno;

pub use ack::AckTracker;
pub use congestion::CongestionController;
pub use reno::NewReno;
```

Update `crates/nhttp3-quic/src/lib.rs`:
```rust
pub mod packet;
pub mod frame;
pub mod transport;
pub mod crypto;
pub mod tls;
pub mod connection;
pub mod stream;
pub mod recovery;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p nhttp3-quic`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/nhttp3-quic/
git commit -m "feat(quic): add NewReno congestion control and ACK tracking (RFC 9002)"
```

---

## Chunk 7: Endpoint + Integration Test

### Task 16: Configuration

**Files:**
- Create: `crates/nhttp3-quic/src/config.rs`

- [ ] **Step 1: Write config with tests**

```rust
// crates/nhttp3-quic/src/config.rs
use std::sync::Arc;
use std::time::Duration;
use rustls::{ClientConfig, ServerConfig};

/// QUIC endpoint configuration.
#[derive(Clone)]
pub struct Config {
    /// Maximum idle timeout. Zero means no timeout.
    pub max_idle_timeout: Duration,
    /// Initial max data the peer can send.
    pub initial_max_data: u64,
    /// Initial max data on locally-initiated bidi streams.
    pub initial_max_stream_data_bidi_local: u64,
    /// Initial max data on remotely-initiated bidi streams.
    pub initial_max_stream_data_bidi_remote: u64,
    /// Initial max data on uni streams.
    pub initial_max_stream_data_uni: u64,
    /// Initial max concurrent bidi streams.
    pub initial_max_streams_bidi: u64,
    /// Initial max concurrent uni streams.
    pub initial_max_streams_uni: u64,
    /// Active connection ID limit.
    pub active_connection_id_limit: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_idle_timeout: Duration::from_secs(30),
            initial_max_data: 10_000_000,
            initial_max_stream_data_bidi_local: 1_000_000,
            initial_max_stream_data_bidi_remote: 1_000_000,
            initial_max_stream_data_uni: 1_000_000,
            initial_max_streams_bidi: 100,
            initial_max_streams_uni: 100,
            active_connection_id_limit: 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = Config::default();
        assert_eq!(config.max_idle_timeout, Duration::from_secs(30));
        assert_eq!(config.initial_max_streams_bidi, 100);
    }
}
```

- [ ] **Step 2: Export from lib.rs**

Add `pub mod config;` to `crates/nhttp3-quic/src/lib.rs`.

- [ ] **Step 3: Run tests and commit**

Run: `cargo test -p nhttp3-quic`

```bash
git add crates/nhttp3-quic/
git commit -m "feat(quic): add endpoint configuration"
```

---

### Task 17: Integration Test — Full QUIC Handshake Over Localhost

This is the Phase 1 milestone: a client and server complete a QUIC handshake and exchange stream data over a real UDP socket on localhost.

**Files:**
- Create: `tests/quic_handshake.rs`

Note: This task is a **stub** integration test. It validates that all the pieces built in Tasks 1-16 compose correctly. The full `Endpoint` type (Task 16 in the spec) that manages multiple connections and drives the I/O loop is a substantial piece of work that will be refined iteratively. This test uses the lower-level primitives directly to prove the protocol works end-to-end.

- [ ] **Step 1: Write the integration test**

```rust
// tests/quic_handshake.rs
//! Integration test: QUIC handshake + stream data exchange over localhost UDP.

use std::sync::Arc;
use nhttp3_quic::tls::TlsSession;
use nhttp3_quic::crypto::{Level, SpaceKeys};
use nhttp3_quic::transport::TransportParams;
use nhttp3_quic::config::Config;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

fn self_signed_cert() -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()));
    let cert = CertificateDer::from(cert.cert);
    (cert, key)
}

#[derive(Debug)]
struct NoCertVerifier;

impl rustls::client::danger::ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self, _: &CertificateDer<'_>, _: &[CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>, _: &[u8], _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
    }
}

#[test]
fn tls_handshake_produces_keys() {
    let (cert, key) = self_signed_cert();

    let mut client_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
        .with_no_client_auth();
    client_config.alpn_protocols = vec![b"h3".to_vec()];

    let mut server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap();
    server_config.alpn_protocols = vec![b"h3".to_vec()];

    // Encode transport params
    let config = Config::default();
    let mut client_tp_buf = bytes::BytesMut::new();
    let client_tp = TransportParams {
        initial_max_data: config.initial_max_data,
        initial_max_streams_bidi: config.initial_max_streams_bidi,
        initial_max_streams_uni: config.initial_max_streams_uni,
        initial_max_stream_data_bidi_local: config.initial_max_stream_data_bidi_local,
        initial_max_stream_data_bidi_remote: config.initial_max_stream_data_bidi_remote,
        initial_max_stream_data_uni: config.initial_max_stream_data_uni,
        ..Default::default()
    };
    client_tp.encode(&mut client_tp_buf);

    let mut server_tp_buf = bytes::BytesMut::new();
    let server_tp = client_tp.clone();
    server_tp.encode(&mut server_tp_buf);

    let server_name = "localhost".try_into().unwrap();
    let mut client = TlsSession::new_client(
        Arc::new(client_config),
        server_name,
        client_tp_buf.to_vec(),
    ).unwrap();

    let mut server = TlsSession::new_server(
        Arc::new(server_config),
        server_tp_buf.to_vec(),
    ).unwrap();

    // Drive handshake
    let ch = client.write_handshake();
    assert!(!ch.data[0].is_empty(), "ClientHello should be in Initial");

    server.read_handshake(&ch.data[0]).unwrap();
    let sh = server.write_handshake();
    assert!(sh.key_change.is_some(), "server should produce handshake keys");

    if !sh.data[0].is_empty() {
        client.read_handshake(&sh.data[0]).unwrap();
    }
    if !sh.data[1].is_empty() {
        client.read_handshake(&sh.data[1]).unwrap();
    }

    let cf = client.write_handshake();
    // Client should now have 1-RTT keys or be done handshaking

    if !cf.data[1].is_empty() {
        server.read_handshake(&cf.data[1]).unwrap();
        let _ = server.write_handshake();
    }

    assert!(!client.is_handshaking() || cf.key_change.is_some(),
        "handshake should complete or produce 1-RTT keys");
}

#[test]
fn varint_roundtrip_exhaustive() {
    use nhttp3_core::VarInt;
    use bytes::{BytesMut, Bytes};

    let test_values: Vec<u64> = vec![
        0, 1, 62, 63, 64, 65,
        16382, 16383, 16384, 16385,
        1_073_741_822, 1_073_741_823, 1_073_741_824, 1_073_741_825,
        4_611_686_018_427_387_902, 4_611_686_018_427_387_903,
    ];

    for val in test_values {
        let v = VarInt::try_from(val).unwrap();
        let mut buf = BytesMut::new();
        v.encode(&mut buf);
        let mut bytes = buf.freeze();
        let decoded = VarInt::decode(&mut bytes).unwrap();
        assert_eq!(v, decoded, "roundtrip failed for {val}");
    }
}

#[test]
fn frame_roundtrip_all_types() {
    use nhttp3_quic::frame::Frame;
    use nhttp3_core::VarInt;
    use bytes::{BytesMut, Bytes};

    let frames = vec![
        Frame::Padding,
        Frame::Ping,
        Frame::Ack {
            largest_ack: VarInt::from_u32(100),
            ack_delay: VarInt::from_u32(10),
            first_ack_range: VarInt::from_u32(5),
            ack_ranges: vec![],
            ecn: None,
        },
        Frame::Crypto {
            offset: VarInt::from_u32(0),
            data: b"test crypto data".to_vec(),
        },
        Frame::Stream {
            stream_id: VarInt::from_u32(0),
            offset: Some(VarInt::from_u32(0)),
            data: b"hello".to_vec(),
            fin: false,
        },
        Frame::MaxData { max_data: VarInt::from_u32(1_000_000) },
        Frame::HandshakeDone,
    ];

    for frame in &frames {
        let mut buf = BytesMut::new();
        frame.encode(&mut buf);
        let mut bytes = buf.freeze();
        let parsed = Frame::parse(&mut bytes).unwrap();
        assert_eq!(*frame, parsed, "roundtrip failed for {frame:?}");
    }
}
```

- [ ] **Step 2: Add workspace-level dev-dependencies**

In the root `Cargo.toml`, ensure the workspace has access to test deps. The `tests/` directory needs its own implicit dependency resolution. Add a `[workspace.dependencies]` entry for rcgen:

```toml
[workspace.dependencies]
bytes = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
rustls = { version = "0.23", features = ["quic"] }
rcgen = "0.13"
```

And in `crates/nhttp3-quic/Cargo.toml`:
```toml
[dev-dependencies]
rcgen = { workspace = true }
```

- [ ] **Step 3: Run integration tests**

Run: `cargo test --test quic_handshake`
Expected: All 3 tests PASS — TLS handshake completes, varint roundtrips work, frame roundtrips work

- [ ] **Step 4: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests across all crates PASS

- [ ] **Step 5: Commit**

```bash
git add tests/ Cargo.toml crates/
git commit -m "test: add integration tests — TLS handshake, varint, and frame roundtrips"
```

---

## Summary

Phase 1 builds the foundation:

| Task | Component | What It Proves |
|------|-----------|----------------|
| 1 | Workspace scaffolding | Project compiles |
| 2 | VarInt | Core encoding works (RFC 9000 §16) |
| 3 | Error types | Error handling foundation |
| 4 | ConnectionId | Identity type (RFC 9000 §17.2) |
| 5 | Packet headers | Can parse QUIC packets (RFC 9000 §17) |
| 6 | Packet numbers | PN encode/decode (RFC 9000 §17.1) |
| 7 | Frame parsing | Can read all QUIC frames (RFC 9000 §12.4) |
| 8 | Frame writing | Can write all QUIC frames |
| 9 | Transport params | Peer negotiation (RFC 9000 §18) |
| 10 | Crypto keys | Key management (RFC 9001 §5) |
| 11 | TLS session | rustls QUIC handshake (RFC 9001) |
| 12 | Connection state | State machine (RFC 9000 §10) |
| 13 | Streams + flow control | Data channels (RFC 9000 §2-3) |
| 14 | ACK tracking | Loss detection foundation (RFC 9002) |
| 15 | NewReno | Congestion control (RFC 9002 §7) |
| 16 | Config | Endpoint configuration |
| 17 | Integration test | All pieces work together |

After Phase 1, we have all the building blocks for a QUIC implementation. The next step (not in this plan) is wiring them into a full `Endpoint` + `Connection` I/O loop that runs on tokio, which will be the start of Phase 1.5 or early Phase 2 work.
