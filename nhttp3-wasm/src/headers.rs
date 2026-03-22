use wasm_bindgen::prelude::*;
use nhttp3_qpack::{Encoder, Decoder, HeaderField};

/// Encodes HTTP request headers into a QPACK header block.
///
/// @param {Array<Array<string>>} headers - Array of [name, value] pairs.
///   Must include pseudo-headers: :method, :path, :scheme, :authority
/// @returns {Uint8Array} QPACK-encoded header block
#[wasm_bindgen]
pub fn encode_headers(headers: Vec<JsValue>) -> Vec<u8> {
    let encoder = Encoder::new(0);
    let fields: Vec<HeaderField> = headers
        .iter()
        .filter_map(|pair| {
            let arr = js_sys::Array::from(pair);
            let name = arr.get(0).as_string()?;
            let value = arr.get(1).as_string()?;
            Some(HeaderField::new(name.into_bytes(), value.into_bytes()))
        })
        .collect();

    encoder.encode_header_block(&fields)
}

/// Decodes a QPACK header block into an array of [name, value] pairs.
///
/// @param {Uint8Array} block - QPACK-encoded header block
/// @returns {Array<Array<string>>} Decoded header pairs
#[wasm_bindgen]
pub fn decode_headers(block: &[u8]) -> Result<JsValue, JsValue> {
    let decoder = Decoder::new(0);
    let fields = decoder
        .decode_header_block(block)
        .map_err(|e| JsValue::from_str(&format!("QPACK decode error: {e}")))?;

    let arr = js_sys::Array::new();
    for field in fields {
        let pair = js_sys::Array::new();
        pair.push(&JsValue::from_str(
            &String::from_utf8_lossy(&field.name),
        ));
        pair.push(&JsValue::from_str(
            &String::from_utf8_lossy(&field.value),
        ));
        arr.push(&pair);
    }

    Ok(arr.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    // WASM-specific tests need wasm_bindgen_test — skipped in cargo test.
    // The encode/decode logic is tested via nhttp3-qpack unit tests.
}
