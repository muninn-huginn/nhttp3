//! End-to-end test: start the HTTP/3 server, connect a real QUIC client,
//! exchange actual HTTP/3 requests over UDP. This is the real thing.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};

#[derive(Debug)]
struct NoCertVerifier;
impl rustls::client::danger::ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn setup() -> (quinn::Endpoint, SocketAddr) {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
        cert.key_pair.serialize_der(),
    ));
    let cert = rustls::pki_types::CertificateDer::from(cert.cert);

    let mut tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap();
    tls.alpn_protocols = vec![b"h3".to_vec()];

    let server_config =
        quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(tls).unwrap()));

    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let endpoint = quinn::Endpoint::server(server_config, addr).unwrap();
    let local_addr = endpoint.local_addr().unwrap();

    (endpoint, local_addr)
}

fn client_endpoint() -> quinn::Endpoint {
    let mut tls = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
        .with_no_client_auth();
    tls.alpn_protocols = vec![b"h3".to_vec()];

    let client_config =
        quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(tls).unwrap()));

    let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse().unwrap()).unwrap();
    endpoint.set_default_client_config(client_config);
    endpoint
}

async fn serve_one(endpoint: &quinn::Endpoint) {
    let incoming = endpoint.accept().await.unwrap();
    let conn = incoming.await.unwrap();
    let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn))
        .await
        .unwrap();

    while let Ok(Some(resolver)) = h3_conn.accept().await {
        let (req, mut stream) = resolver.resolve_request().await.unwrap();
        let path = req.uri().path().to_string();

        let (status, body) = match path.as_str() {
            "/" => (
                http::StatusCode::OK,
                r#"{"message":"Hello from nhttp3!","protocol":"h3"}"#.to_string(),
            ),
            "/health" => (http::StatusCode::OK, r#"{"status":"ok"}"#.to_string()),
            "/qpack-demo" => {
                let headers = vec![
                    nhttp3_qpack::HeaderField::new(":method", "GET"),
                    nhttp3_qpack::HeaderField::new(":path", "/api"),
                    nhttp3_qpack::HeaderField::new("accept", "application/json"),
                ];
                let encoder = nhttp3_qpack::Encoder::new(0);
                let decoder = nhttp3_qpack::Decoder::new(0);
                let encoded = encoder.encode_header_block(&headers);
                let decoded = decoder.decode_header_block(&encoded).unwrap();
                let raw: usize = headers.iter().map(|h| h.name.len() + h.value.len()).sum();
                (
                    http::StatusCode::OK,
                    format!(
                        r#"{{"headers":{},"raw":{},"qpack":{},"roundtrip":{}}}"#,
                        headers.len(),
                        raw,
                        encoded.len(),
                        decoded.len() == headers.len()
                    ),
                )
            }
            "/echo" => {
                // Read body
                let mut body_data = Vec::new();
                while let Some(chunk) = stream.recv_data().await.unwrap() {
                    use bytes::Buf;
                    let mut c = chunk;
                    while c.has_remaining() {
                        let b = c.chunk();
                        body_data.extend_from_slice(b);
                        let l = b.len();
                        c.advance(l);
                    }
                }
                let echo = String::from_utf8_lossy(&body_data);
                (
                    http::StatusCode::OK,
                    format!(r#"{{"echo":"{}","size":{}}}"#, echo, body_data.len()),
                )
            }
            "/stream" => {
                let resp = http::Response::builder()
                    .status(http::StatusCode::OK)
                    .header("content-type", "text/event-stream")
                    .body(())
                    .unwrap();
                stream.send_response(resp).await.unwrap();
                for i in 0..5 {
                    stream
                        .send_data(Bytes::from(format!("data: chunk {i}\n\n")))
                        .await
                        .unwrap();
                }
                stream.finish().await.unwrap();
                continue; // skip the generic response below
            }
            _ => (
                http::StatusCode::NOT_FOUND,
                r#"{"error":"not found"}"#.to_string(),
            ),
        };

        let resp = http::Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(())
            .unwrap();
        stream.send_response(resp).await.unwrap();
        stream.send_data(Bytes::from(body)).await.unwrap();
        stream.finish().await.unwrap();
    }
}

#[tokio::test]
async fn real_http3_get() {
    let (server, addr) = setup();

    // Start server in background
    let server_handle = tokio::spawn(async move { serve_one(&server).await });

    // Connect client
    let client = client_endpoint();
    let conn = client.connect(addr, "localhost").unwrap().await.unwrap();
    eprintln!("QUIC connected to {}", conn.remote_address());

    let (mut driver, mut send_request) = h3::client::new(h3_quinn::Connection::new(conn))
        .await
        .unwrap();

    tokio::spawn(async move {
        let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });

    // GET /
    let req = http::Request::builder()
        .uri(format!("https://localhost:{}/", addr.port()))
        .body(())
        .unwrap();
    let mut stream = send_request.send_request(req).await.unwrap();
    stream.finish().await.unwrap();
    let resp = stream.recv_response().await.unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json"
    );

    let mut body = Vec::new();
    while let Some(chunk) = stream.recv_data().await.unwrap() {
        use bytes::Buf;
        let mut c = chunk;
        while c.has_remaining() {
            let b = c.chunk();
            body.extend_from_slice(b);
            let l = b.len();
            c.advance(l);
        }
    }
    let body_str = String::from_utf8(body).unwrap();
    eprintln!("Response: {body_str}");

    assert!(body_str.contains("Hello from nhttp3!"));
    assert!(body_str.contains("h3"));

    // Close client cleanly so server's accept loop exits
    drop(send_request);
    client.close(0u32.into(), b"done");
    client.wait_idle().await;
    server_handle.abort();
}

#[tokio::test]
async fn real_http3_qpack_roundtrip() {
    let (server, addr) = setup();
    let server_handle = tokio::spawn(async move { serve_one(&server).await });

    let client = client_endpoint();
    let conn = client.connect(addr, "localhost").unwrap().await.unwrap();

    let (mut driver, mut send_request) = h3::client::new(h3_quinn::Connection::new(conn))
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });

    // GET /qpack-demo
    let req = http::Request::builder()
        .uri(format!("https://localhost:{}/qpack-demo", addr.port()))
        .body(())
        .unwrap();
    let mut stream = send_request.send_request(req).await.unwrap();
    stream.finish().await.unwrap();
    let resp = stream.recv_response().await.unwrap();
    assert_eq!(resp.status(), 200);

    let mut body = Vec::new();
    while let Some(chunk) = stream.recv_data().await.unwrap() {
        use bytes::Buf;
        let mut c = chunk;
        while c.has_remaining() {
            let b = c.chunk();
            body.extend_from_slice(b);
            let l = b.len();
            c.advance(l);
        }
    }
    let body_str = String::from_utf8(body).unwrap();
    eprintln!("QPACK demo: {body_str}");

    assert!(body_str.contains("\"roundtrip\":true"));
    assert!(body_str.contains("\"qpack\":"));

    drop(send_request);
    client.close(0u32.into(), b"done");
    client.wait_idle().await;
    server_handle.abort();
}

// Helper to read h3 response body
async fn read_body(
    stream: &mut h3::client::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
) -> String {
    let mut body = Vec::new();
    while let Some(chunk) = stream.recv_data().await.unwrap() {
        use bytes::Buf;
        let mut c = chunk;
        while c.has_remaining() {
            let b = c.chunk();
            body.extend_from_slice(b);
            let l = b.len();
            c.advance(l);
        }
    }
    String::from_utf8(body).unwrap()
}

#[tokio::test]
async fn real_http3_post_echo() {
    let (server, addr) = setup();
    let server_handle = tokio::spawn(async move { serve_one(&server).await });

    let client = client_endpoint();
    let conn = client.connect(addr, "localhost").unwrap().await.unwrap();
    let (mut driver, mut send_request) = h3::client::new(h3_quinn::Connection::new(conn))
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });

    let req = http::Request::builder()
        .method("POST")
        .uri(format!("https://localhost:{}/echo", addr.port()))
        .body(())
        .unwrap();
    let mut stream = send_request.send_request(req).await.unwrap();
    stream
        .send_data(Bytes::from(r#"{"test":true}"#))
        .await
        .unwrap();
    stream.finish().await.unwrap();

    let resp = stream.recv_response().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body_str = read_body(&mut stream).await;
    eprintln!("Echo: {body_str}");
    assert!(body_str.contains(r#"{"test":true}"#));

    drop(send_request);
    client.close(0u32.into(), b"done");
    client.wait_idle().await;
    server_handle.abort();
}

#[tokio::test]
async fn real_http3_streaming() {
    let (server, addr) = setup();
    let server_handle = tokio::spawn(async move { serve_one(&server).await });

    let client = client_endpoint();
    let conn = client.connect(addr, "localhost").unwrap().await.unwrap();
    let (mut driver, mut send_request) = h3::client::new(h3_quinn::Connection::new(conn))
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });

    let req = http::Request::builder()
        .uri(format!("https://localhost:{}/stream", addr.port()))
        .body(())
        .unwrap();
    let mut stream = send_request.send_request(req).await.unwrap();
    stream.finish().await.unwrap();

    let resp = stream.recv_response().await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    let body_str = read_body(&mut stream).await;
    eprintln!("Stream chunks: {body_str}");
    assert!(body_str.contains("data: chunk 0"));
    assert!(body_str.contains("data: chunk 4"));

    drop(send_request);
    client.close(0u32.into(), b"done");
    client.wait_idle().await;
    server_handle.abort();
}

#[tokio::test]
async fn real_http3_404() {
    let (server, addr) = setup();
    let server_handle = tokio::spawn(async move { serve_one(&server).await });

    let client = client_endpoint();
    let conn = client.connect(addr, "localhost").unwrap().await.unwrap();
    let (mut driver, mut send_request) = h3::client::new(h3_quinn::Connection::new(conn))
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });

    let req = http::Request::builder()
        .uri(format!("https://localhost:{}/nonexistent", addr.port()))
        .body(())
        .unwrap();
    let mut stream = send_request.send_request(req).await.unwrap();
    stream.finish().await.unwrap();

    let resp = stream.recv_response().await.unwrap();
    assert_eq!(resp.status(), 404);

    drop(send_request);
    client.close(0u32.into(), b"done");
    client.wait_idle().await;
    server_handle.abort();
}
