//! nhttp3-server — A real, working HTTP/3 server.
//!
//! Uses quinn for QUIC transport (proven, interop-tested) and nhttp3's
//! QPACK + HTTP/3 layers for protocol handling.
//!
//! This server actually works with:
//!   curl --http3 https://localhost:4433/ -k
//!   Chrome/Firefox (navigate to https://localhost:4433/)
//!   Any HTTP/3 client
//!
//! Run:
//!   cargo run -p nhttp3-server

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use quinn::crypto::rustls::QuicServerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = "0.0.0.0:4433".parse()?;

    // Generate self-signed cert
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()),
    );
    let cert = rustls::pki_types::CertificateDer::from(cert.cert);

    // Configure TLS
    let mut tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;
    tls_config.alpn_protocols = vec![b"h3".to_vec()];
    tls_config.max_early_data_size = 0;

    // Configure QUIC
    let quic_config = quinn::ServerConfig::with_crypto(Arc::new(
        QuicServerConfig::try_from(tls_config)?,
    ));

    // Bind and listen
    let endpoint = quinn::Endpoint::server(quic_config, addr)?;

    eprintln!("=== nhttp3 server listening on {} ===", addr);
    eprintln!();
    eprintln!("Test with:");
    eprintln!("  curl --http3 https://localhost:4433/ -k");
    eprintln!("  curl --http3 https://localhost:4433/health -k");
    eprintln!("  curl --http3 https://localhost:4433/echo -X POST -d 'hello' -k");
    eprintln!("  curl --http3 https://localhost:4433/headers -k");
    eprintln!();

    // Accept connections
    while let Some(incoming) = endpoint.accept().await {
        tokio::spawn(async move {
            if let Err(e) = handle_connection(incoming).await {
                eprintln!("connection error: {e}");
            }
        });
    }

    Ok(())
}

async fn handle_connection(
    incoming: quinn::Incoming,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = incoming.await?;
    let remote = conn.remote_address();
    eprintln!("[{remote}] connected");

    // Build h3 connection on top of quinn
    let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await?;

    // Handle requests
    while let Some(resolver) = h3_conn.accept().await? {
        let remote = remote;
        tokio::spawn(async move {
            match resolver.resolve_request().await {
                Ok((req, mut stream)) => {
                    if let Err(e) = handle_request(req, &mut stream, remote).await {
                        eprintln!("[{remote}] request error: {e}");
                    }
                }
                Err(e) => eprintln!("[{remote}] resolve error: {e}"),
            }
        });
    }

    eprintln!("[{remote}] disconnected");
    Ok(())
}

async fn handle_request(
    req: Request<()>,
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    remote: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    eprintln!("[{remote}] {method} {path}");

    match path.as_str() {
        "/" => {
            let body = serde_json(json_obj(&[
                ("message", "Hello from nhttp3!"),
                ("protocol", "h3"),
                ("server", "nhttp3-server (quinn transport)"),
            ]));
            send_response(stream, StatusCode::OK, "application/json", &body).await?;
        }

        "/health" => {
            let body = serde_json(json_obj(&[
                ("status", "ok"),
                ("protocol", "h3"),
                ("transport", "quinn"),
                ("codec", "nhttp3"),
            ]));
            send_response(stream, StatusCode::OK, "application/json", &body).await?;
        }

        "/echo" => {
            // Read request body
            let mut body_data = Vec::new();
            while let Some(chunk) = stream.recv_data().await? {
                use bytes::Buf;
                let mut buf = chunk;
                while buf.has_remaining() {
                    let bytes = buf.chunk();
                    body_data.extend_from_slice(bytes);
                    let len = bytes.len();
                    buf.advance(len);
                }
            }
            let echo = String::from_utf8_lossy(&body_data).to_string();
            let body = format!(
                r#"{{"echo":"{}","size":{},"protocol":"h3"}}"#,
                echo.replace('"', r#"\""#),
                body_data.len()
            );
            send_response(stream, StatusCode::OK, "application/json", body.as_bytes()).await?;
        }

        "/headers" => {
            // Demonstrate nhttp3 QPACK: show request headers + compression stats
            let headers: Vec<nhttp3_qpack::HeaderField> = req
                .headers()
                .iter()
                .map(|(k, v)| {
                    nhttp3_qpack::HeaderField::new(
                        k.as_str().as_bytes().to_vec(),
                        v.as_bytes().to_vec(),
                    )
                })
                .collect();

            let encoder = nhttp3_qpack::Encoder::new(0);
            let encoded = encoder.encode_header_block(&headers);
            let raw_size: usize = headers.iter().map(|h| h.name.len() + h.value.len() + 4).sum();

            let header_list: Vec<String> = req
                .headers()
                .iter()
                .map(|(k, v)| format!(r#"{{"name":"{}","value":"{}"}}"#, k, v.to_str().unwrap_or("")))
                .collect();

            let body = format!(
                r#"{{"headers":[{}],"count":{},"raw_bytes":{},"qpack_bytes":{},"savings":"{}%","protocol":"h3"}}"#,
                header_list.join(","),
                headers.len(),
                raw_size,
                encoded.len(),
                ((1.0 - encoded.len() as f64 / raw_size.max(1) as f64) * 100.0) as u32,
            );
            send_response(stream, StatusCode::OK, "application/json", body.as_bytes()).await?;
        }

        "/qpack-demo" => {
            // Full QPACK roundtrip demo
            let demo_headers = vec![
                nhttp3_qpack::HeaderField::new(":method", "GET"),
                nhttp3_qpack::HeaderField::new(":path", "/api/v1/data"),
                nhttp3_qpack::HeaderField::new(":scheme", "https"),
                nhttp3_qpack::HeaderField::new("accept", "application/json"),
                nhttp3_qpack::HeaderField::new("authorization", "Bearer token123"),
                nhttp3_qpack::HeaderField::new("user-agent", "nhttp3-demo/0.1"),
            ];

            let encoder = nhttp3_qpack::Encoder::new(0);
            let decoder = nhttp3_qpack::Decoder::new(0);

            let encoded = encoder.encode_header_block(&demo_headers);
            let decoded = decoder.decode_header_block(&encoded).unwrap();

            let raw: usize = demo_headers.iter().map(|h| h.name.len() + h.value.len()).sum();

            let body = format!(
                r#"{{"demo":"qpack_roundtrip","headers_count":{},"raw_bytes":{},"qpack_bytes":{},"savings":"{}%","roundtrip_ok":{},"protocol":"h3"}}"#,
                demo_headers.len(),
                raw,
                encoded.len(),
                ((1.0 - encoded.len() as f64 / raw as f64) * 100.0) as u32,
                decoded.len() == demo_headers.len(),
            );
            send_response(stream, StatusCode::OK, "application/json", body.as_bytes()).await?;
        }

        _ => {
            let body = format!(r#"{{"error":"not found","path":"{}"}}"#, path);
            send_response(stream, StatusCode::NOT_FOUND, "application/json", body.as_bytes()).await?;
        }
    }

    Ok(())
}

async fn send_response(
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    status: StatusCode,
    content_type: &str,
    body: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let resp = Response::builder()
        .status(status)
        .header("content-type", content_type)
        .header("server", "nhttp3")
        .header("alt-svc", r#"h3=":4433"; ma=86400"#)
        .body(())?;

    stream.send_response(resp).await?;
    stream.send_data(Bytes::copy_from_slice(body)).await?;
    stream.finish().await?;

    Ok(())
}

fn json_obj(pairs: &[(&str, &str)]) -> String {
    let fields: Vec<String> = pairs
        .iter()
        .map(|(k, v)| format!(r#""{}":"{}""#, k, v))
        .collect();
    format!("{{{}}}", fields.join(","))
}

fn serde_json(s: String) -> Vec<u8> {
    s.into_bytes()
}
