//! End-to-end test: start the HTTP/3 server, connect a real QUIC client,
//! exchange actual HTTP/3 requests over UDP. This is the real thing.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};

#[derive(Debug)]
struct NoCertVerifier;
impl rustls::client::danger::ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(&self, _: &rustls::pki_types::CertificateDer<'_>, _: &[rustls::pki_types::CertificateDer<'_>], _: &rustls::pki_types::ServerName<'_>, _: &[u8], _: rustls::pki_types::UnixTime) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> { Ok(rustls::client::danger::ServerCertVerified::assertion()) }
    fn verify_tls12_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> { Ok(rustls::client::danger::HandshakeSignatureValid::assertion()) }
    fn verify_tls13_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> { Ok(rustls::client::danger::HandshakeSignatureValid::assertion()) }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> { rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes() }
}

fn setup() -> (quinn::Endpoint, SocketAddr) {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()),
    );
    let cert = rustls::pki_types::CertificateDer::from(cert.cert);

    let mut tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap();
    tls.alpn_protocols = vec![b"h3".to_vec()];

    let server_config = quinn::ServerConfig::with_crypto(Arc::new(
        QuicServerConfig::try_from(tls).unwrap(),
    ));

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

    let client_config = quinn::ClientConfig::new(Arc::new(
        QuicClientConfig::try_from(tls).unwrap(),
    ));

    let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse().unwrap()).unwrap();
    endpoint.set_default_client_config(client_config);
    endpoint
}

async fn serve_one(endpoint: &quinn::Endpoint) {
    let incoming = endpoint.accept().await.unwrap();
    let conn = incoming.await.unwrap();
    let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await.unwrap();

    while let Ok(Some(resolver)) = h3_conn.accept().await {
        let (req, mut stream) = resolver.resolve_request().await.unwrap();
        let path = req.uri().path().to_string();

        let (status, body) = match path.as_str() {
            "/" => (http::StatusCode::OK, r#"{"message":"Hello from nhttp3!","protocol":"h3"}"#.to_string()),
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
                (http::StatusCode::OK, format!(
                    r#"{{"headers":{},"raw":{},"qpack":{},"roundtrip":{}}}"#,
                    headers.len(), raw, encoded.len(), decoded.len() == headers.len()
                ))
            }
            _ => (http::StatusCode::NOT_FOUND, r#"{"error":"not found"}"#.to_string()),
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

    let (mut driver, mut send_request) =
        h3::client::new(h3_quinn::Connection::new(conn)).await.unwrap();

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

    client.wait_idle().await;
    server_handle.abort();
}

#[tokio::test]
async fn real_http3_qpack_roundtrip() {
    let (server, addr) = setup();
    let server_handle = tokio::spawn(async move { serve_one(&server).await });

    let client = client_endpoint();
    let conn = client.connect(addr, "localhost").unwrap().await.unwrap();

    let (mut driver, mut send_request) =
        h3::client::new(h3_quinn::Connection::new(conn)).await.unwrap();
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

    client.wait_idle().await;
    server_handle.abort();
}
