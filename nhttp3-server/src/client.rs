//! nhttp3-client — Real HTTP/3 client.
//!
//! Usage:
//!   cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/
//!   cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/health
//!   cargo run -p nhttp3-server --bin nhttp3-client -- -X POST -d '{"msg":"hi"}' https://localhost:4433/echo

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use quinn::crypto::rustls::QuicClientConfig;

#[derive(Debug)]
struct NoCertVerifier;
impl rustls::client::danger::ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(&self, _: &rustls::pki_types::CertificateDer<'_>, _: &[rustls::pki_types::CertificateDer<'_>], _: &rustls::pki_types::ServerName<'_>, _: &[u8], _: rustls::pki_types::UnixTime) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> { Ok(rustls::client::danger::ServerCertVerified::assertion()) }
    fn verify_tls12_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> { Ok(rustls::client::danger::HandshakeSignatureValid::assertion()) }
    fn verify_tls13_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> { Ok(rustls::client::danger::HandshakeSignatureValid::assertion()) }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> { rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes() }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let args: Vec<String> = std::env::args().collect();

    let mut method = "GET".to_string();
    let mut body: Option<String> = None;
    let mut url = String::new();
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-X" => { i += 1; method = args[i].clone(); }
            "-d" => { i += 1; body = Some(args[i].clone()); }
            "-v" => { verbose = true; }
            s if s.starts_with("http") => { url = s.to_string(); }
            _ => { url = args[i].clone(); }
        }
        i += 1;
    }

    if url.is_empty() {
        eprintln!("Usage: nhttp3-client [-X METHOD] [-d BODY] [-v] URL");
        eprintln!("Example: nhttp3-client https://localhost:4433/");
        std::process::exit(1);
    }

    // Parse URL
    let url = if !url.starts_with("https://") {
        format!("https://{}", url)
    } else {
        url
    };
    let parsed: http::Uri = url.parse()?;
    let host = parsed.host().unwrap_or("localhost");
    let port = parsed.port_u16().unwrap_or(4433);
    let path = parsed.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");

    if verbose {
        eprintln!("* Connecting to {}:{} over QUIC...", host, port);
    }

    // Setup QUIC client
    let mut tls = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
        .with_no_client_auth();
    tls.alpn_protocols = vec![b"h3".to_vec()];

    let client_config = quinn::ClientConfig::new(Arc::new(
        QuicClientConfig::try_from(tls)?,
    ));

    let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse()?)?;
    endpoint.set_default_client_config(client_config);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()
        .unwrap_or_else(|_| format!("127.0.0.1:{}", port).parse().unwrap());

    let start = Instant::now();

    // QUIC connect
    let conn = endpoint.connect(addr, host)?.await?;
    let connect_time = start.elapsed();

    if verbose {
        eprintln!("* QUIC connected in {:?}", connect_time);
        eprintln!("* Remote: {}", conn.remote_address());
    }

    // H3 connection
    let (mut driver, mut send_request) =
        h3::client::new(h3_quinn::Connection::new(conn)).await?;

    tokio::spawn(async move {
        let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });

    // Build request
    let mut req_builder = http::Request::builder()
        .method(method.as_str())
        .uri(format!("https://{}:{}{}", host, port, path));

    if body.is_some() {
        req_builder = req_builder.header("content-type", "application/json");
    }

    let req = req_builder.body(())?;

    if verbose {
        eprintln!("> {} {} HTTP/3", method, path);
        for (k, v) in req.headers() {
            eprintln!("> {}: {}", k, v.to_str().unwrap_or("?"));
        }
        eprintln!(">");
    }

    // Send request
    let mut stream = send_request.send_request(req).await?;

    if let Some(body_data) = &body {
        stream.send_data(Bytes::from(body_data.clone().into_bytes())).await?;
    }
    stream.finish().await?;

    // Receive response
    let resp = stream.recv_response().await?;
    let response_time = start.elapsed();

    if verbose {
        eprintln!("< HTTP/3 {}", resp.status());
        for (k, v) in resp.headers() {
            eprintln!("< {}: {}", k, v.to_str().unwrap_or("?"));
        }
        eprintln!("<");
    }

    // Read body
    let mut resp_body = Vec::new();
    while let Some(chunk) = stream.recv_data().await? {
        use bytes::Buf;
        let mut c = chunk;
        while c.has_remaining() {
            let b = c.chunk();
            resp_body.extend_from_slice(b);
            let l = b.len();
            c.advance(l);
        }
    }
    let total_time = start.elapsed();

    // Print response
    println!("{}", String::from_utf8_lossy(&resp_body));

    if verbose {
        eprintln!();
        eprintln!("* Connect:  {:?}", connect_time);
        eprintln!("* Response: {:?}", response_time);
        eprintln!("* Total:    {:?}", total_time);
        eprintln!("* Bytes:    {}", resp_body.len());
    }

    drop(send_request);
    endpoint.close(0u32.into(), b"done");
    endpoint.wait_idle().await;

    Ok(())
}
