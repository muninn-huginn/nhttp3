use bytes::BufMut;
use nhttp3_core::VarInt;

use super::*;

impl Frame {
    /// Serializes this frame into the buffer.
    pub fn encode<B: BufMut>(&self, buf: &mut B) {
        match self {
            Frame::Padding => {
                VarInt::from_u32(0x00).encode(buf);
            }
            Frame::Ping => {
                VarInt::from_u32(0x01).encode(buf);
            }
            Frame::Ack {
                largest_ack,
                ack_delay,
                first_ack_range,
                ack_ranges,
                ecn,
            } => {
                let ft = if ecn.is_some() { 0x03u32 } else { 0x02 };
                VarInt::from_u32(ft).encode(buf);
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
            Frame::ResetStream {
                stream_id,
                error_code,
                final_size,
            } => {
                VarInt::from_u32(0x04).encode(buf);
                stream_id.encode(buf);
                error_code.encode(buf);
                final_size.encode(buf);
            }
            Frame::StopSending {
                stream_id,
                error_code,
            } => {
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
            Frame::Stream {
                stream_id,
                offset,
                data,
                fin,
            } => {
                let mut ft: u8 = 0x08;
                if offset.is_some() {
                    ft |= 0x04;
                }
                ft |= 0x02; // always include length for roundtrip safety
                if *fin {
                    ft |= 0x01;
                }
                VarInt::from_u32(ft as u32).encode(buf);
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
            Frame::MaxStreamData {
                stream_id,
                max_data,
            } => {
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
            Frame::StreamDataBlocked {
                stream_id,
                max_data,
            } => {
                VarInt::from_u32(0x15).encode(buf);
                stream_id.encode(buf);
                max_data.encode(buf);
            }
            Frame::StreamsBlocked { bidi, max_streams } => {
                let ft = if *bidi { 0x16u32 } else { 0x17 };
                VarInt::from_u32(ft).encode(buf);
                max_streams.encode(buf);
            }
            Frame::NewConnectionId {
                sequence,
                retire_prior_to,
                connection_id,
                stateless_reset_token,
            } => {
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
            Frame::ConnectionClose {
                error_code,
                frame_type,
                reason,
            } => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

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
            ack_ranges: vec![AckRange {
                gap: VarInt::from_u32(2),
                range: VarInt::from_u32(3),
            }],
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
        roundtrip(&Frame::MaxData {
            max_data: VarInt::from_u32(1_000_000),
        });
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
        roundtrip(&Frame::MaxStreams {
            bidi: true,
            max_streams: VarInt::from_u32(100),
        });
        roundtrip(&Frame::MaxStreams {
            bidi: false,
            max_streams: VarInt::from_u32(50),
        });
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
        roundtrip(&Frame::PathChallenge {
            data: [1, 2, 3, 4, 5, 6, 7, 8],
        });
        roundtrip(&Frame::PathResponse {
            data: [8, 7, 6, 5, 4, 3, 2, 1],
        });
    }
}
