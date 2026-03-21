use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
use nhttp3_qpack::HeaderField;

use crate::error::Error;

/// Converts HTTP request parts to QPACK header fields.
pub fn request_to_fields(
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
) -> Vec<HeaderField> {
    let mut fields = Vec::new();

    fields.push(HeaderField::new(":method", method.as_str().as_bytes()));
    fields.push(HeaderField::new(
        ":scheme",
        uri.scheme_str().unwrap_or("https").as_bytes(),
    ));
    fields.push(HeaderField::new(
        ":authority",
        uri.authority()
            .map(|a| a.as_str())
            .unwrap_or("")
            .as_bytes(),
    ));
    fields.push(HeaderField::new(
        ":path",
        uri.path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/")
            .as_bytes(),
    ));

    for (name, value) in headers.iter() {
        fields.push(HeaderField::new(
            name.as_str().as_bytes(),
            value.as_bytes(),
        ));
    }

    fields
}

/// Converts HTTP response status + headers to QPACK header fields.
pub fn response_to_fields(status: StatusCode, headers: &HeaderMap) -> Vec<HeaderField> {
    let mut fields = Vec::new();

    fields.push(HeaderField::new(
        ":status",
        status.as_str().as_bytes(),
    ));

    for (name, value) in headers.iter() {
        fields.push(HeaderField::new(
            name.as_str().as_bytes(),
            value.as_bytes(),
        ));
    }

    fields
}

/// Extracts the :status pseudo-header from decoded fields.
pub fn extract_status(fields: &[HeaderField]) -> Result<StatusCode, Error> {
    for field in fields {
        if field.name == b":status" {
            let status_str = std::str::from_utf8(&field.value)
                .map_err(|_| Error::MalformedHeaders)?;
            let code: u16 = status_str
                .parse()
                .map_err(|_| Error::MalformedHeaders)?;
            return StatusCode::from_u16(code).map_err(|_| Error::MalformedHeaders);
        }
    }
    Err(Error::MalformedHeaders)
}

/// Extracts regular headers (non-pseudo) from decoded fields into a HeaderMap.
pub fn fields_to_headermap(fields: &[HeaderField]) -> Result<HeaderMap, Error> {
    let mut map = HeaderMap::new();
    for field in fields {
        if field.name.starts_with(b":") {
            continue; // skip pseudo-headers
        }
        let name = HeaderName::from_bytes(&field.name)
            .map_err(|_| Error::MalformedHeaders)?;
        let value = HeaderValue::from_bytes(&field.value)
            .map_err(|_| Error::MalformedHeaders)?;
        map.insert(name, value);
    }
    Ok(map)
}

/// Extracts pseudo-headers for requests.
pub fn extract_request_pseudo(fields: &[HeaderField]) -> Result<(Method, Uri), Error> {
    let mut method = None;
    let mut scheme = None;
    let mut authority = None;
    let mut path = None;

    for field in fields {
        match field.name.as_slice() {
            b":method" => {
                method = Some(
                    Method::from_bytes(&field.value).map_err(|_| Error::MalformedHeaders)?,
                );
            }
            b":scheme" => scheme = Some(std::str::from_utf8(&field.value).map_err(|_| Error::MalformedHeaders)?),
            b":authority" => authority = Some(std::str::from_utf8(&field.value).map_err(|_| Error::MalformedHeaders)?),
            b":path" => path = Some(std::str::from_utf8(&field.value).map_err(|_| Error::MalformedHeaders)?),
            _ => {}
        }
    }

    let method = method.ok_or(Error::MalformedHeaders)?;

    let uri_str = format!(
        "{}://{}{}",
        scheme.unwrap_or("https"),
        authority.unwrap_or(""),
        path.unwrap_or("/")
    );
    let uri: Uri = uri_str.parse().map_err(|_| Error::MalformedHeaders)?;

    Ok((method, uri))
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header;

    #[test]
    fn request_to_fields_basic() {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());

        let fields = request_to_fields(
            &Method::GET,
            &"https://example.com/path".parse().unwrap(),
            &headers,
        );

        assert!(fields.iter().any(|f| f.name == b":method" && f.value == b"GET"));
        assert!(fields.iter().any(|f| f.name == b":path" && f.value == b"/path"));
        assert!(fields.iter().any(|f| f.name == b":scheme" && f.value == b"https"));
        assert!(fields.iter().any(|f| f.name == b"content-type" && f.value == b"text/plain"));
    }

    #[test]
    fn response_to_fields_basic() {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "text/html".parse().unwrap());

        let fields = response_to_fields(StatusCode::OK, &headers);

        assert!(fields.iter().any(|f| f.name == b":status" && f.value == b"200"));
        assert!(fields.iter().any(|f| f.name == b"content-type" && f.value == b"text/html"));
    }

    #[test]
    fn extract_status_ok() {
        let fields = vec![HeaderField::new(":status", "200")];
        assert_eq!(extract_status(&fields).unwrap(), StatusCode::OK);
    }

    #[test]
    fn extract_request_pseudo_ok() {
        let fields = vec![
            HeaderField::new(":method", "POST"),
            HeaderField::new(":scheme", "https"),
            HeaderField::new(":authority", "example.com"),
            HeaderField::new(":path", "/api"),
        ];
        let (method, uri) = extract_request_pseudo(&fields).unwrap();
        assert_eq!(method, Method::POST);
        assert_eq!(uri.path(), "/api");
    }

    #[test]
    fn fields_to_headermap_skips_pseudo() {
        let fields = vec![
            HeaderField::new(":status", "200"),
            HeaderField::new("content-type", "text/html"),
            HeaderField::new("x-custom", "value"),
        ];
        let map = fields_to_headermap(&fields).unwrap();
        assert_eq!(map.len(), 2);
        assert!(map.get("content-type").is_some());
        assert!(map.get("x-custom").is_some());
    }
}
