use bytes::{Buf, Bytes};
use nhttp3_core::{ConnectionId, Error as CoreError, VarInt};

use super::*;
use crate::packet::PacketError;

/// Maximum allowed frame payload size (16 MB) to prevent DoS.
const MAX_FRAME_PAYLOAD: usize = 16 * 1024 * 1024;

fn check_length(len: usize) -> Result<(), PacketError> {
    if len > MAX_FRAME_PAYLOAD {
        return Err(PacketError::Invalid(format!(
            "frame payload {} exceeds max {}",
            len, MAX_FRAME_PAYLOAD
        )));
    }
    Ok(())
}

impl Frame {
    /// Parses a single frame from the buffer.
    pub fn parse(buf: &mut Bytes) -> Result<Self, PacketError> {
        let frame_type = VarInt::decode(buf)?;

        match frame_type.value() {
            0x00 => Ok(Frame::Padding),
            0x01 => Ok(Frame::Ping),
            0x02 | 0x03 => {
                let has_ecn = frame_type.value() == 0x03;
                let largest_ack = VarInt::decode(buf)?;
                let ack_delay = VarInt::decode(buf)?;
                let ack_range_count = VarInt::decode(buf)?;
                let first_ack_range = VarInt::decode(buf)?;

                // Security: cap ACK ranges to prevent DoS (aioquic #549)
                const MAX_ACK_RANGES: u64 = 256;
                if ack_range_count.value() > MAX_ACK_RANGES {
                    return Err(PacketError::Invalid(format!(
                        "ACK range count {} exceeds limit {}",
                        ack_range_count.value(),
                        MAX_ACK_RANGES
                    )));
                }

                let mut ack_ranges = Vec::new();
                for _ in 0..ack_range_count.value() {
                    let gap = VarInt::decode(buf)?;
                    let range = VarInt::decode(buf)?;
                    ack_ranges.push(AckRange { gap, range });
                }

                let ecn = if has_ecn {
                    Some(EcnCounts {
                        ect0: VarInt::decode(buf)?,
                        ect1: VarInt::decode(buf)?,
                        ecn_ce: VarInt::decode(buf)?,
                    })
                } else {
                    None
                };

                Ok(Frame::Ack {
                    largest_ack,
                    ack_delay,
                    first_ack_range,
                    ack_ranges,
                    ecn,
                })
            }
            0x04 => Ok(Frame::ResetStream {
                stream_id: VarInt::decode(buf)?,
                error_code: VarInt::decode(buf)?,
                final_size: VarInt::decode(buf)?,
            }),
            0x05 => Ok(Frame::StopSending {
                stream_id: VarInt::decode(buf)?,
                error_code: VarInt::decode(buf)?,
            }),
            0x06 => {
                let offset = VarInt::decode(buf)?;
                let len = VarInt::decode(buf)?.value() as usize;
                check_length(len)?;
                if buf.remaining() < len {
                    return Err(PacketError::Core(CoreError::BufferTooShort));
                }
                let data = buf.copy_to_bytes(len).to_vec();
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
                    let d = buf.chunk().to_vec();
                    buf.advance(d.len());
                    d
                };

                Ok(Frame::Stream {
                    stream_id,
                    offset,
                    data,
                    fin,
                })
            }
            0x10 => Ok(Frame::MaxData {
                max_data: VarInt::decode(buf)?,
            }),
            0x11 => Ok(Frame::MaxStreamData {
                stream_id: VarInt::decode(buf)?,
                max_data: VarInt::decode(buf)?,
            }),
            0x12 | 0x13 => Ok(Frame::MaxStreams {
                bidi: frame_type.value() == 0x12,
                max_streams: VarInt::decode(buf)?,
            }),
            0x14 => Ok(Frame::DataBlocked {
                max_data: VarInt::decode(buf)?,
            }),
            0x15 => Ok(Frame::StreamDataBlocked {
                stream_id: VarInt::decode(buf)?,
                max_data: VarInt::decode(buf)?,
            }),
            0x16 | 0x17 => Ok(Frame::StreamsBlocked {
                bidi: frame_type.value() == 0x16,
                max_streams: VarInt::decode(buf)?,
            }),
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
                Ok(Frame::NewConnectionId {
                    sequence,
                    retire_prior_to,
                    connection_id,
                    stateless_reset_token: token,
                })
            }
            0x19 => Ok(Frame::RetireConnectionId {
                sequence: VarInt::decode(buf)?,
            }),
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
                let ft = Some(VarInt::decode(buf)?);
                let len = VarInt::decode(buf)?.value() as usize;
                if buf.remaining() < len {
                    return Err(PacketError::Core(CoreError::BufferTooShort));
                }
                let reason = buf.chunk()[..len].to_vec();
                buf.advance(len);
                Ok(Frame::ConnectionClose {
                    error_code,
                    frame_type: ft,
                    reason,
                })
            }
            0x1d => {
                let error_code = VarInt::decode(buf)?;
                let len = VarInt::decode(buf)?.value() as usize;
                if buf.remaining() < len {
                    return Err(PacketError::Core(CoreError::BufferTooShort));
                }
                let reason = buf.chunk()[..len].to_vec();
                buf.advance(len);
                Ok(Frame::ConnectionClose {
                    error_code,
                    frame_type: None,
                    reason,
                })
            }
            0x1e => Ok(Frame::HandshakeDone),
            other => Err(PacketError::Invalid(format!("unknown frame type: {other}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    fn encode_varint(val: u64) -> Vec<u8> {
        let v = VarInt::try_from(val).unwrap();
        let mut buf = BytesMut::new();
        v.encode(&mut buf);
        buf.to_vec()
    }

    #[test]
    fn parse_padding() {
        let mut buf = Bytes::from_static(&[0x00]);
        assert_eq!(Frame::parse(&mut buf).unwrap(), Frame::Padding);
    }

    #[test]
    fn parse_ping() {
        let mut buf = Bytes::from_static(&[0x01]);
        assert_eq!(Frame::parse(&mut buf).unwrap(), Frame::Ping);
    }

    #[test]
    fn parse_crypto() {
        let mut data = vec![0x06];
        data.extend_from_slice(&encode_varint(0));
        data.extend_from_slice(&encode_varint(5));
        data.extend_from_slice(b"hello");
        let mut buf = Bytes::from(data);
        match Frame::parse(&mut buf).unwrap() {
            Frame::Crypto { offset, data } => {
                assert_eq!(offset.value(), 0);
                assert_eq!(data, b"hello");
            }
            _ => panic!("expected Crypto"),
        }
    }

    #[test]
    fn parse_stream_with_offset_and_fin() {
        let mut data = vec![0x0f];
        data.extend_from_slice(&encode_varint(4));
        data.extend_from_slice(&encode_varint(100));
        data.extend_from_slice(&encode_varint(3));
        data.extend_from_slice(b"hey");
        let mut buf = Bytes::from(data);
        match Frame::parse(&mut buf).unwrap() {
            Frame::Stream {
                stream_id,
                offset,
                data,
                fin,
            } => {
                assert_eq!(stream_id.value(), 4);
                assert_eq!(offset.unwrap().value(), 100);
                assert_eq!(data, b"hey");
                assert!(fin);
            }
            _ => panic!("expected Stream"),
        }
    }

    #[test]
    fn parse_connection_close() {
        let mut data = vec![0x1c];
        data.extend_from_slice(&encode_varint(0x0a));
        data.extend_from_slice(&encode_varint(0x06));
        data.extend_from_slice(&encode_varint(4));
        data.extend_from_slice(b"oops");
        let mut buf = Bytes::from(data);
        match Frame::parse(&mut buf).unwrap() {
            Frame::ConnectionClose {
                error_code,
                frame_type,
                reason,
            } => {
                assert_eq!(error_code.value(), 0x0a);
                assert_eq!(frame_type.unwrap().value(), 0x06);
                assert_eq!(reason, b"oops");
            }
            _ => panic!("expected ConnectionClose"),
        }
    }

    #[test]
    fn parse_ack_simple() {
        let mut data = vec![0x02];
        data.extend_from_slice(&encode_varint(10));
        data.extend_from_slice(&encode_varint(0));
        data.extend_from_slice(&encode_varint(0));
        data.extend_from_slice(&encode_varint(10));
        let mut buf = Bytes::from(data);
        match Frame::parse(&mut buf).unwrap() {
            Frame::Ack {
                largest_ack,
                first_ack_range,
                ack_ranges,
                ecn,
                ..
            } => {
                assert_eq!(largest_ack.value(), 10);
                assert_eq!(first_ack_range.value(), 10);
                assert!(ack_ranges.is_empty());
                assert!(ecn.is_none());
            }
            _ => panic!("expected Ack"),
        }
    }

    #[test]
    fn parse_max_data() {
        let mut data = vec![0x10];
        data.extend_from_slice(&encode_varint(1_000_000));
        let mut buf = Bytes::from(data);
        match Frame::parse(&mut buf).unwrap() {
            Frame::MaxData { max_data } => assert_eq!(max_data.value(), 1_000_000),
            _ => panic!("expected MaxData"),
        }
    }

    #[test]
    fn parse_handshake_done() {
        let mut buf = Bytes::from_static(&[0x1e]);
        assert_eq!(Frame::parse(&mut buf).unwrap(), Frame::HandshakeDone);
    }
}
