use bytes::{Buf, BufMut, Bytes, BytesMut};
use nhttp3_core::VarInt;

use super::*;
use crate::error::Error;

impl H3Frame {
    /// Parses an HTTP/3 frame from the buffer.
    pub fn parse(buf: &mut Bytes) -> Result<Self, Error> {
        let frame_type =
            VarInt::decode(buf).map_err(|e| Error::FrameError(format!("frame type: {e}")))?;
        let length = VarInt::decode(buf)
            .map_err(|e| Error::FrameError(format!("frame length: {e}")))?
            .value() as usize;

        if buf.remaining() < length {
            return Err(Error::FrameError("truncated frame payload".into()));
        }

        let mut payload = buf.slice(..length);
        buf.advance(length);

        match frame_type.value() {
            DATA => Ok(H3Frame::Data {
                data: payload.to_vec(),
            }),
            HEADERS => Ok(H3Frame::Headers {
                block: payload.to_vec(),
            }),
            CANCEL_PUSH => {
                let push_id = VarInt::decode(&mut payload)
                    .map_err(|e| Error::FrameError(format!("cancel_push: {e}")))?;
                Ok(H3Frame::CancelPush { push_id })
            }
            SETTINGS => {
                let mut settings = Vec::new();
                while payload.has_remaining() {
                    let id = VarInt::decode(&mut payload)
                        .map_err(|e| Error::FrameError(format!("settings id: {e}")))?;
                    let value = VarInt::decode(&mut payload)
                        .map_err(|e| Error::FrameError(format!("settings value: {e}")))?;
                    settings.push((id, value));
                }
                Ok(H3Frame::Settings { settings })
            }
            PUSH_PROMISE => {
                let push_id = VarInt::decode(&mut payload)
                    .map_err(|e| Error::FrameError(format!("push_promise: {e}")))?;
                let block = payload.to_vec();
                Ok(H3Frame::PushPromise { push_id, block })
            }
            GOAWAY => {
                let id = VarInt::decode(&mut payload)
                    .map_err(|e| Error::FrameError(format!("goaway: {e}")))?;
                Ok(H3Frame::GoAway { id })
            }
            MAX_PUSH_ID => {
                let push_id = VarInt::decode(&mut payload)
                    .map_err(|e| Error::FrameError(format!("max_push_id: {e}")))?;
                Ok(H3Frame::MaxPushId { push_id })
            }
            _ => Ok(H3Frame::Unknown {
                frame_type,
                data: payload.to_vec(),
            }),
        }
    }

    /// Serializes this frame into the buffer.
    pub fn encode<B: BufMut>(&self, buf: &mut B) {
        match self {
            H3Frame::Data { data } => {
                VarInt::from_u32(DATA as u32).encode(buf);
                VarInt::try_from(data.len() as u64).unwrap().encode(buf);
                buf.put_slice(data);
            }
            H3Frame::Headers { block } => {
                VarInt::from_u32(HEADERS as u32).encode(buf);
                VarInt::try_from(block.len() as u64).unwrap().encode(buf);
                buf.put_slice(block);
            }
            H3Frame::CancelPush { push_id } => {
                VarInt::from_u32(CANCEL_PUSH as u32).encode(buf);
                VarInt::try_from(push_id.encoded_size() as u64)
                    .unwrap()
                    .encode(buf);
                push_id.encode(buf);
            }
            H3Frame::Settings { settings } => {
                VarInt::from_u32(SETTINGS as u32).encode(buf);
                let mut payload = BytesMut::new();
                for (id, value) in settings {
                    id.encode(&mut payload);
                    value.encode(&mut payload);
                }
                VarInt::try_from(payload.len() as u64).unwrap().encode(buf);
                buf.put_slice(&payload);
            }
            H3Frame::PushPromise { push_id, block } => {
                VarInt::from_u32(PUSH_PROMISE as u32).encode(buf);
                let pid_size = push_id.encoded_size();
                VarInt::try_from((pid_size + block.len()) as u64)
                    .unwrap()
                    .encode(buf);
                push_id.encode(buf);
                buf.put_slice(block);
            }
            H3Frame::GoAway { id } => {
                VarInt::from_u32(GOAWAY as u32).encode(buf);
                VarInt::try_from(id.encoded_size() as u64)
                    .unwrap()
                    .encode(buf);
                id.encode(buf);
            }
            H3Frame::MaxPushId { push_id } => {
                VarInt::from_u32(MAX_PUSH_ID as u32).encode(buf);
                VarInt::try_from(push_id.encoded_size() as u64)
                    .unwrap()
                    .encode(buf);
                push_id.encode(buf);
            }
            H3Frame::Unknown { frame_type, data } => {
                frame_type.encode(buf);
                VarInt::try_from(data.len() as u64).unwrap().encode(buf);
                buf.put_slice(data);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(frame: &H3Frame) {
        let mut buf = BytesMut::new();
        frame.encode(&mut buf);
        let mut bytes = buf.freeze();
        let parsed = H3Frame::parse(&mut bytes).unwrap();
        assert_eq!(*frame, parsed, "roundtrip failed for {frame:?}");
    }

    #[test]
    fn roundtrip_data() {
        roundtrip(&H3Frame::Data {
            data: b"hello world".to_vec(),
        });
    }

    #[test]
    fn roundtrip_headers() {
        roundtrip(&H3Frame::Headers {
            block: vec![0x00, 0x00, 0xc1], // mock QPACK block
        });
    }

    #[test]
    fn roundtrip_settings() {
        roundtrip(&H3Frame::Settings {
            settings: vec![
                (VarInt::from_u32(0x06), VarInt::from_u32(4096)),
                (VarInt::from_u32(0x01), VarInt::from_u32(100)),
            ],
        });
    }

    #[test]
    fn roundtrip_goaway() {
        roundtrip(&H3Frame::GoAway {
            id: VarInt::from_u32(0),
        });
    }

    #[test]
    fn roundtrip_empty_data() {
        roundtrip(&H3Frame::Data { data: vec![] });
    }

    #[test]
    fn unknown_frame_type_skipped() {
        let frame = H3Frame::Unknown {
            frame_type: VarInt::from_u32(0xff),
            data: vec![1, 2, 3],
        };
        roundtrip(&frame);
    }

    #[test]
    fn roundtrip_settings_empty() {
        roundtrip(&H3Frame::Settings { settings: vec![] });
    }
}
