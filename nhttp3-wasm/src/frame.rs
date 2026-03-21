use wasm_bindgen::prelude::*;
use bytes::{BytesMut, Bytes};
use nhttp3_h3::H3Frame;
use nhttp3_core::VarInt;

/// Encode an HTTP/3 HEADERS frame from a QPACK-encoded block.
#[wasm_bindgen]
pub fn encode_headers_frame(qpack_block: &[u8]) -> Vec<u8> {
    let frame = H3Frame::Headers {
        block: qpack_block.to_vec(),
    };
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    buf.to_vec()
}

/// Encode an HTTP/3 DATA frame.
#[wasm_bindgen]
pub fn encode_data_frame(data: &[u8]) -> Vec<u8> {
    let frame = H3Frame::Data {
        data: data.to_vec(),
    };
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    buf.to_vec()
}

/// Encode an HTTP/3 SETTINGS frame with default settings.
#[wasm_bindgen]
pub fn encode_settings_frame() -> Vec<u8> {
    let frame = H3Frame::Settings {
        settings: vec![
            // QPACK max table capacity
            (VarInt::from_u32(0x01), VarInt::from_u32(4096)),
            // QPACK max blocked streams
            (VarInt::from_u32(0x07), VarInt::from_u32(100)),
        ],
    };
    let mut buf = BytesMut::new();
    frame.encode(&mut buf);
    buf.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_data_frame_works() {
        let encoded = encode_data_frame(b"hello");
        assert!(!encoded.is_empty());
        // Parse it back
        let mut bytes = Bytes::from(encoded);
        let frame = H3Frame::parse(&mut bytes).unwrap();
        match frame {
            H3Frame::Data { data } => assert_eq!(data, b"hello"),
            _ => panic!("expected Data frame"),
        }
    }

    #[test]
    fn encode_headers_frame_works() {
        let block = vec![0x00, 0x00, 0xc1]; // mock QPACK block
        let encoded = encode_headers_frame(&block);
        let mut bytes = Bytes::from(encoded);
        let frame = H3Frame::parse(&mut bytes).unwrap();
        match frame {
            H3Frame::Headers { block: b } => assert_eq!(b, block),
            _ => panic!("expected Headers frame"),
        }
    }

    #[test]
    fn encode_settings_frame_works() {
        let encoded = encode_settings_frame();
        let mut bytes = Bytes::from(encoded);
        let frame = H3Frame::parse(&mut bytes).unwrap();
        match frame {
            H3Frame::Settings { settings } => {
                assert_eq!(settings.len(), 2);
            }
            _ => panic!("expected Settings frame"),
        }
    }
}
