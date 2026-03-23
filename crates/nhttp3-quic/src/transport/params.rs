use bytes::{Buf, BufMut, Bytes};
use nhttp3_core::{ConnectionId, Error as CoreError, VarInt};
use std::time::Duration;

use crate::packet::PacketError;

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

impl TransportParams {
    /// Encodes transport parameters into the buffer.
    pub fn encode<B: BufMut>(&self, buf: &mut B) {
        self.encode_varint_param(
            buf,
            MAX_IDLE_TIMEOUT,
            self.max_idle_timeout.as_millis() as u64,
        );
        self.encode_varint_param(buf, MAX_UDP_PAYLOAD_SIZE, self.max_udp_payload_size);
        self.encode_varint_param(buf, INITIAL_MAX_DATA, self.initial_max_data);
        self.encode_varint_param(
            buf,
            INITIAL_MAX_STREAM_DATA_BIDI_LOCAL,
            self.initial_max_stream_data_bidi_local,
        );
        self.encode_varint_param(
            buf,
            INITIAL_MAX_STREAM_DATA_BIDI_REMOTE,
            self.initial_max_stream_data_bidi_remote,
        );
        self.encode_varint_param(
            buf,
            INITIAL_MAX_STREAM_DATA_UNI,
            self.initial_max_stream_data_uni,
        );
        self.encode_varint_param(buf, INITIAL_MAX_STREAMS_BIDI, self.initial_max_streams_bidi);
        self.encode_varint_param(buf, INITIAL_MAX_STREAMS_UNI, self.initial_max_streams_uni);
        self.encode_varint_param(buf, ACK_DELAY_EXPONENT, self.ack_delay_exponent);
        self.encode_varint_param(buf, MAX_ACK_DELAY, self.max_ack_delay.as_millis() as u64);
        self.encode_varint_param(buf, ACTIVE_CID_LIMIT, self.active_connection_id_limit);

        if self.disable_active_migration {
            VarInt::try_from(DISABLE_ACTIVE_MIGRATION)
                .unwrap()
                .encode(buf);
            VarInt::from_u32(0).encode(buf);
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
        VarInt::try_from(v.encoded_size() as u64)
            .unwrap()
            .encode(buf);
        v.encode(buf);
    }

    /// Decodes transport parameters from the buffer.
    pub fn decode(buf: &mut Bytes) -> Result<Self, PacketError> {
        let mut params = Self::default();

        while buf.has_remaining() {
            let id = VarInt::decode(buf)?.value();
            let len = VarInt::decode(buf)?.value() as usize;
            if buf.remaining() < len {
                return Err(PacketError::Core(CoreError::BufferTooShort));
            }

            let mut param_buf = buf.slice(..len);
            buf.advance(len);

            match id {
                ORIGINAL_DCID => {
                    params.original_destination_connection_id =
                        Some(ConnectionId::from_slice(param_buf.chunk())?);
                }
                MAX_IDLE_TIMEOUT => {
                    let ms = VarInt::decode(&mut param_buf)?.value();
                    params.max_idle_timeout = Duration::from_millis(ms);
                }
                STATELESS_RESET_TOKEN => {
                    if param_buf.remaining() < 16 {
                        return Err(PacketError::Core(CoreError::BufferTooShort));
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
                    params.initial_max_stream_data_bidi_local =
                        VarInt::decode(&mut param_buf)?.value();
                }
                INITIAL_MAX_STREAM_DATA_BIDI_REMOTE => {
                    params.initial_max_stream_data_bidi_remote =
                        VarInt::decode(&mut param_buf)?.value();
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
                    params.initial_source_connection_id =
                        Some(ConnectionId::from_slice(param_buf.chunk())?);
                }
                RETRY_SCID => {
                    params.retry_source_connection_id =
                        Some(ConnectionId::from_slice(param_buf.chunk())?);
                }
                _ => { /* Unknown parameter — skip */ }
            }
        }

        Ok(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

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
        let params = TransportParams::default();
        let mut buf = BytesMut::new();
        params.encode(&mut buf);
        // Append unknown param
        VarInt::from_u32(0xff00).encode(&mut buf);
        VarInt::from_u32(3).encode(&mut buf);
        buf.put_slice(&[0xaa, 0xbb, 0xcc]);

        let mut bytes = buf.freeze();
        let decoded = TransportParams::decode(&mut bytes).unwrap();
        assert_eq!(decoded.max_udp_payload_size, 65527);
    }
}
