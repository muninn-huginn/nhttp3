use bytes::{BufMut, BytesMut};

use crate::table::{field::HeaderField, static_, DynamicTable};

/// QPACK encoder.
///
/// Uses a conservative strategy: only references static table entries and
/// literals. Dynamic table references are deferred to avoid blocked streams.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Encoder {
    dynamic_table: DynamicTable,
    max_table_capacity: usize,
}

impl Encoder {
    pub fn new(max_table_capacity: usize) -> Self {
        Self {
            dynamic_table: DynamicTable::new(max_table_capacity),
            max_table_capacity,
        }
    }

    /// Encodes a list of header fields into a QPACK header block.
    ///
    /// Returns the encoded header block prefix + field lines.
    /// This uses a conservative strategy: static refs and literals only.
    pub fn encode_header_block(&self, headers: &[HeaderField]) -> Vec<u8> {
        let mut buf = BytesMut::new();

        // Header block prefix (RFC 9204 §4.5.1):
        // Required Insert Count = 0 (we don't use dynamic table refs)
        // Delta Base = 0
        encode_prefixed_int(&mut buf, 0, 0, 8); // Required Insert Count
        encode_prefixed_int(&mut buf, 0, 0, 7); // Delta Base (sign bit = 0)

        for field in headers {
            self.encode_field(&mut buf, field);
        }

        buf.to_vec()
    }

    fn encode_field(&self, buf: &mut BytesMut, field: &HeaderField) {
        // Try static table first
        if let Some((idx, has_value)) = static_::find(&field.name, &field.value) {
            if has_value && !field.sensitive {
                // Indexed Field Line (static) — RFC 9204 §4.5.2
                // 1TNNNNN: T=1 (static), N = index
                encode_prefixed_int(buf, idx as u64, 0xc0, 6);
            } else if !field.sensitive {
                // Literal with Name Reference (static) — RFC 9204 §4.5.4
                // 01NTNNN: N=0 (not sensitive), T=1 (static)
                encode_prefixed_int(buf, idx as u64, 0x50, 4);
                encode_string_literal(buf, &field.value);
            } else {
                // Literal with Name Reference, sensitive
                encode_prefixed_int(buf, idx as u64, 0x70, 4); // N=1 (never-indexed)
                encode_string_literal(buf, &field.value);
            }
        } else {
            // Literal with Literal Name — RFC 9204 §4.5.6
            if field.sensitive {
                buf.put_u8(0x38); // 001NHNNN: N=1 (never-indexed), H=1 (could huffman, we don't)
            } else {
                buf.put_u8(0x20); // 001NHNNN: N=0, H=0
            }
            encode_string_literal(buf, &field.name);
            encode_string_literal(buf, &field.value);
        }
    }
}

/// Encodes a prefixed integer (RFC 7541 §5.1, used by QPACK).
fn encode_prefixed_int(buf: &mut BytesMut, value: u64, prefix_byte: u8, prefix_bits: u8) {
    let max_prefix = (1u64 << prefix_bits) - 1;

    if value < max_prefix {
        buf.put_u8(prefix_byte | value as u8);
    } else {
        buf.put_u8(prefix_byte | max_prefix as u8);
        let mut remaining = value - max_prefix;
        while remaining >= 128 {
            buf.put_u8((remaining as u8 & 0x7f) | 0x80);
            remaining >>= 7;
        }
        buf.put_u8(remaining as u8);
    }
}

/// Encodes a string literal (RFC 9204 §4.1.2). No Huffman encoding for simplicity.
fn encode_string_literal(buf: &mut BytesMut, data: &[u8]) {
    // H=0 (no Huffman), length as prefixed integer with 7-bit prefix
    encode_prefixed_int(buf, data.len() as u64, 0x00, 7);
    buf.put_slice(data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_static_indexed() {
        let enc = Encoder::new(0);
        let headers = vec![HeaderField::new(":method", "GET")];
        let block = enc.encode_header_block(&headers);
        // Should have prefix (2 bytes) + indexed static ref
        assert!(block.len() >= 3);
    }

    #[test]
    fn encode_static_name_ref_with_literal_value() {
        let enc = Encoder::new(0);
        let headers = vec![HeaderField::new(":status", "201")];
        let block = enc.encode_header_block(&headers);
        // :status exists in static table but "201" doesn't
        assert!(block.len() > 3);
    }

    #[test]
    fn encode_fully_literal() {
        let enc = Encoder::new(0);
        let headers = vec![HeaderField::new("x-custom", "value")];
        let block = enc.encode_header_block(&headers);
        assert!(block.len() > 10); // prefix + name + value
    }

    #[test]
    fn encode_multiple_fields() {
        let enc = Encoder::new(0);
        let headers = vec![
            HeaderField::new(":method", "GET"),
            HeaderField::new(":path", "/"),
            HeaderField::new(":scheme", "https"),
            HeaderField::new(":authority", "example.com"),
        ];
        let block = enc.encode_header_block(&headers);
        assert!(!block.is_empty());
    }

    #[test]
    fn encode_sensitive_field() {
        let enc = Encoder::new(0);
        let headers = vec![HeaderField::sensitive("authorization", "Bearer secret")];
        let block = enc.encode_header_block(&headers);
        assert!(!block.is_empty());
    }
}
