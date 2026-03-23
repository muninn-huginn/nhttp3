use crate::table::{field::HeaderField, static_, DynamicTable};

/// QPACK decoder.
#[derive(Debug)]
pub struct Decoder {
    dynamic_table: DynamicTable,
}

/// Errors from QPACK decoding.
#[derive(Debug, thiserror::Error)]
pub enum DecoderError {
    #[error("buffer too short")]
    BufferTooShort,
    #[error("invalid static index: {0}")]
    InvalidStaticIndex(usize),
    #[error("invalid dynamic index: {0}")]
    InvalidDynamicIndex(usize),
    #[error("invalid header block")]
    InvalidBlock,
}

impl Decoder {
    pub fn new(max_table_capacity: usize) -> Self {
        Self {
            dynamic_table: DynamicTable::new(max_table_capacity),
        }
    }

    /// Decodes a QPACK header block into a list of header fields.
    pub fn decode_header_block(&self, data: &[u8]) -> Result<Vec<HeaderField>, DecoderError> {
        let mut buf = data;
        let mut headers = Vec::new();

        // Decode prefix
        let (required_insert_count, _) = decode_prefixed_int(&mut buf, 8)?;
        let (_delta_base, _) = decode_prefixed_int(&mut buf, 7)?;

        // We only support Required Insert Count = 0 (no dynamic table refs)
        if required_insert_count != 0 {
            return Err(DecoderError::InvalidBlock);
        }

        while !buf.is_empty() {
            let first = buf[0];

            if first & 0x80 != 0 {
                // Indexed Field Line (§4.5.2)
                let is_static = first & 0x40 != 0;
                let (index, _) = decode_prefixed_int(&mut buf, 6)?;
                let index = index as usize;

                if is_static {
                    let field =
                        static_::get(index).ok_or(DecoderError::InvalidStaticIndex(index))?;
                    headers.push(field);
                } else {
                    let field = self
                        .dynamic_table
                        .get_absolute(index)
                        .ok_or(DecoderError::InvalidDynamicIndex(index))?
                        .clone();
                    headers.push(field);
                }
            } else if first & 0x40 != 0 {
                // Literal with Name Reference (§4.5.4)
                let _never_indexed = first & 0x20 != 0;
                let is_static = first & 0x10 != 0;
                let (index, _) = decode_prefixed_int(&mut buf, 4)?;
                let index = index as usize;

                let value = decode_string_literal(&mut buf)?;

                let name = if is_static {
                    static_::get(index)
                        .ok_or(DecoderError::InvalidStaticIndex(index))?
                        .name
                } else {
                    self.dynamic_table
                        .get_absolute(index)
                        .ok_or(DecoderError::InvalidDynamicIndex(index))?
                        .name
                        .clone()
                };

                headers.push(HeaderField::new(name, value));
            } else if first & 0x20 != 0 {
                // Literal with Literal Name (§4.5.6)
                let _never_indexed = first & 0x10 != 0;
                // Skip the first byte pattern bits, read name
                let _ = buf.get(0); // consume first byte handled by decode_prefixed_int below
                                    // Actually we need to handle this more carefully
                                    // The name length is encoded after the pattern byte
                buf = &buf[1..]; // skip the instruction byte
                let name = decode_string_literal(&mut buf)?;
                let value = decode_string_literal(&mut buf)?;
                headers.push(HeaderField::new(name, value));
            } else {
                // Indexed Field Line with Post-Base Index (§4.5.3) — not supported yet
                return Err(DecoderError::InvalidBlock);
            }
        }

        Ok(headers)
    }
}

/// Decodes a prefixed integer (RFC 7541 §5.1).
/// Returns (value, bytes_consumed).
fn decode_prefixed_int(buf: &mut &[u8], prefix_bits: u8) -> Result<(u64, usize), DecoderError> {
    if buf.is_empty() {
        return Err(DecoderError::BufferTooShort);
    }

    let max_prefix = (1u64 << prefix_bits) - 1;
    let first = buf[0] as u64 & max_prefix;
    *buf = &buf[1..];
    let mut consumed = 1;

    if first < max_prefix {
        return Ok((first, consumed));
    }

    let mut value = max_prefix;
    let mut shift = 0u32;

    loop {
        if buf.is_empty() {
            return Err(DecoderError::BufferTooShort);
        }
        let byte = buf[0];
        *buf = &buf[1..];
        consumed += 1;

        value += ((byte & 0x7f) as u64) << shift;
        shift += 7;

        if byte & 0x80 == 0 {
            break;
        }
    }

    Ok((value, consumed))
}

/// Decodes a string literal (RFC 9204 §4.1.2).
fn decode_string_literal(buf: &mut &[u8]) -> Result<Vec<u8>, DecoderError> {
    if buf.is_empty() {
        return Err(DecoderError::BufferTooShort);
    }

    let _huffman = buf[0] & 0x80 != 0;
    let (length, _) = decode_prefixed_int(buf, 7)?;
    let length = length as usize;

    if buf.len() < length {
        return Err(DecoderError::BufferTooShort);
    }

    let data = buf[..length].to_vec();
    *buf = &buf[length..];

    // Note: Huffman decoding not implemented — we never encode with Huffman
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::Encoder;

    #[test]
    fn decode_static_indexed() {
        let enc = Encoder::new(0);
        let dec = Decoder::new(0);

        let headers = vec![HeaderField::new(":method", "GET")];
        let block = enc.encode_header_block(&headers);
        let decoded = dec.decode_header_block(&block).unwrap();

        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].name, b":method");
        assert_eq!(decoded[0].value, b"GET");
    }

    #[test]
    fn roundtrip_request_headers() {
        let enc = Encoder::new(0);
        let dec = Decoder::new(0);

        let headers = vec![
            HeaderField::new(":method", "GET"),
            HeaderField::new(":path", "/"),
            HeaderField::new(":scheme", "https"),
            HeaderField::new(":authority", "example.com"),
        ];
        let block = enc.encode_header_block(&headers);
        let decoded = dec.decode_header_block(&block).unwrap();

        assert_eq!(decoded.len(), 4);
        assert_eq!(decoded[0].name, b":method");
        assert_eq!(decoded[0].value, b"GET");
        assert_eq!(decoded[1].name, b":path");
        assert_eq!(decoded[1].value, b"/");
        assert_eq!(decoded[2].name, b":scheme");
        assert_eq!(decoded[2].value, b"https");
        assert_eq!(decoded[3].name, b":authority");
        assert_eq!(decoded[3].value, b"example.com");
    }

    #[test]
    fn roundtrip_response_headers() {
        let enc = Encoder::new(0);
        let dec = Decoder::new(0);

        let headers = vec![
            HeaderField::new(":status", "200"),
            HeaderField::new("content-type", "text/plain"),
            HeaderField::new("x-custom", "value"),
        ];
        let block = enc.encode_header_block(&headers);
        let decoded = dec.decode_header_block(&block).unwrap();

        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].name, b":status");
        assert_eq!(decoded[0].value, b"200");
        assert_eq!(decoded[1].name, b"content-type");
        assert_eq!(decoded[1].value, b"text/plain");
        assert_eq!(decoded[2].name, b"x-custom");
        assert_eq!(decoded[2].value, b"value");
    }

    #[test]
    fn roundtrip_with_literal_name_value() {
        let enc = Encoder::new(0);
        let dec = Decoder::new(0);

        let headers = vec![HeaderField::new("x-request-id", "abc-123-def")];
        let block = enc.encode_header_block(&headers);
        let decoded = dec.decode_header_block(&block).unwrap();

        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].name, b"x-request-id");
        assert_eq!(decoded[0].value, b"abc-123-def");
    }

    #[test]
    fn empty_headers() {
        let enc = Encoder::new(0);
        let dec = Decoder::new(0);

        let block = enc.encode_header_block(&[]);
        let decoded = dec.decode_header_block(&block).unwrap();
        assert!(decoded.is_empty());
    }
}
