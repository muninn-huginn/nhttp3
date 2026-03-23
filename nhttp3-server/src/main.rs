//! nhttp3-server — HTTP/3 server with optional reverse proxy mode.
//!
//! Two modes:
//!   1. Demo mode (default): serves built-in endpoints for testing
//!   2. Proxy mode: proxies HTTP/3 → HTTP/1.1 to a backend (FastAPI, etc.)
//!
//! Demo mode:
//!   cargo run -p nhttp3-server
//!
//! Proxy mode (put HTTP/3 in front of FastAPI):
//!   uvicorn myapp:app --port 8000 &
//!   cargo run -p nhttp3-server -- --proxy http://localhost:8000
//!
//! What this actually does:
//!   External clients connect over HTTP/3 (1-RTT, no HOL blocking).
//!   The proxy translates to HTTP/1.1 for the backend.
//!   This helps when clients are on high-latency/lossy networks.
//!   It does NOT help for localhost-to-localhost (see benchmark).

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use clap::Parser;
use http::{Response, StatusCode};
use quinn::crypto::rustls::QuicServerConfig;

#[derive(Parser)]
#[command(name = "nhttp3-server", about = "HTTP/3 server + reverse proxy")]
struct Args {
    /// Listen port
    #[arg(long, default_value_t = 4433)]
    port: u16,

    /// Listen host
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Reverse proxy target (e.g., http://localhost:8000 for FastAPI/uvicorn)
    /// When set, all requests are proxied to this backend.
    #[arg(long)]
    proxy: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let args = Args::parse();
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
        cert.key_pair.serialize_der(),
    ));
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert);

    let mut tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key)?;
    tls.alpn_protocols = vec![b"h3".to_vec()];
    tls.max_early_data_size = 0;

    let quic_config = quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(tls)?));
    let endpoint = quinn::Endpoint::server(quic_config, addr)?;

    if let Some(ref backend) = args.proxy {
        eprintln!("=== nhttp3 reverse proxy ===");
        eprintln!("HTTP/3 frontend: {addr}");
        eprintln!("HTTP/1.1 backend: {backend}");
        eprintln!();
        eprintln!("This helps when clients are on high-latency/lossy networks.");
        eprintln!("Saves 2.5 RTTs per new connection vs TCP+TLS.");
        eprintln!("Does NOT help for localhost-to-localhost (see nhttp3-benchmark).");
        eprintln!();
        eprintln!("Example: expose FastAPI over HTTP/3:");
        eprintln!("  uvicorn myapp:app --port 8000 &");
        eprintln!("  nhttp3-server --proxy http://localhost:8000");
    } else {
        eprintln!("=== nhttp3 server (demo mode) ===");
        eprintln!("Listening: {addr} (HTTP/3)");
        eprintln!();
        eprintln!("Endpoints:");
        eprintln!("  GET  /                      JSON hello");
        eprintln!("  GET  /health                Health check");
        eprintln!("  POST /echo                  Echo body");
        eprintln!("  GET  /headers               QPACK compression stats");
        eprintln!("  GET  /qpack-demo            QPACK roundtrip");
        eprintln!("  GET  /stream                SSE streaming");
        eprintln!("  POST /v1/chat/completions   OpenAI streaming");
        eprintln!();
        eprintln!("Proxy mode: nhttp3-server --proxy http://localhost:8000");
    }
    eprintln!();

    let proxy_target = args.proxy.map(Arc::new);
    let http_client = Arc::new(reqwest::Client::new());

    while let Some(incoming) = endpoint.accept().await {
        let proxy = proxy_target.clone();
        let client = http_client.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(incoming, proxy, client).await {
                eprintln!("connection error: {e}");
            }
        });
    }

    Ok(())
}

async fn handle_connection(
    incoming: quinn::Incoming,
    proxy_target: Option<Arc<String>>,
    http_client: Arc<reqwest::Client>,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = incoming.await?;
    let remote = conn.remote_address();

    let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await?;

    while let Some(resolver) = h3_conn.accept().await? {
        let proxy = proxy_target.clone();
        let client = http_client.clone();
        tokio::spawn(async move {
            match resolver.resolve_request().await {
                Ok((req, mut stream)) => {
                    let result = if let Some(ref backend) = proxy {
                        proxy_request(req, &mut stream, backend, &client).await
                    } else {
                        demo_request(req, &mut stream).await
                    };
                    if let Err(e) = result {
                        eprintln!("[{remote}] error: {e}");
                    }
                }
                Err(e) => eprintln!("[{remote}] resolve error: {e}"),
            }
        });
    }
    Ok(())
}

// ─── Proxy Mode ───

async fn proxy_request(
    req: http::Request<()>,
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    backend: &str,
    http_client: &reqwest::Client,
) -> Result<(), Box<dyn std::error::Error>> {
    let method = req.method().clone();
    let path = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let url = format!("{backend}{path}");

    // Read request body
    let mut body_data = Vec::new();
    while let Some(chunk) = stream.recv_data().await? {
        use bytes::Buf;
        let mut c = chunk;
        while c.has_remaining() {
            let b = c.chunk();
            body_data.extend_from_slice(b);
            let l = b.len();
            c.advance(l);
        }
    }

    // Forward to backend
    let mut backend_req = http_client.request(
        reqwest::Method::from_bytes(method.as_str().as_bytes())?,
        &url,
    );

    // Forward relevant headers
    for (k, v) in req.headers() {
        if k != "host" && !k.as_str().starts_with(':') {
            backend_req = backend_req.header(k.as_str(), v.as_bytes());
        }
    }

    if !body_data.is_empty() {
        backend_req = backend_req.body(body_data);
    }

    match backend_req.send().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16())?;
            let content_type = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();

            let is_streaming =
                content_type.contains("event-stream") || content_type.contains("ndjson");

            let mut h3_resp = Response::builder()
                .status(status)
                .header("content-type", &content_type)
                .header("server", "nhttp3")
                .header("alt-svc", r#"h3=":4433"; ma=86400"#);

            // Forward response headers
            for (k, v) in resp.headers() {
                let name = k.as_str();
                if name != "transfer-encoding" && name != "connection" && name != "content-type" {
                    h3_resp = h3_resp.header(name, v.as_bytes());
                }
            }

            stream.send_response(h3_resp.body(())?).await?;

            if is_streaming {
                // Stream response chunks
                use futures_util::StreamExt;
                let mut byte_stream = resp.bytes_stream();
                while let Some(chunk) = byte_stream.next().await {
                    match chunk {
                        Ok(data) => {
                            stream.send_data(Bytes::from(data.to_vec())).await?;
                        }
                        Err(_) => break,
                    }
                }
            } else {
                let body = resp.bytes().await?;
                stream.send_data(Bytes::from(body.to_vec())).await?;
            }
            stream.finish().await?;
        }
        Err(e) => {
            let body = format!(r#"{{"error":"backend unreachable: {e}"}}"#);
            send_response(
                stream,
                StatusCode::BAD_GATEWAY,
                "application/json",
                body.as_bytes(),
            )
            .await?;
        }
    }
    Ok(())
}

// ─── Demo Mode ───

async fn demo_request(
    req: http::Request<()>,
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = req.uri().path().to_string();

    match path.as_str() {
        "/" => {
            let body = r#"{"message":"Hello from nhttp3!","protocol":"h3"}"#;
            send_response(stream, StatusCode::OK, "application/json", body.as_bytes()).await?;
        }
        "/health" => {
            let body = r#"{"status":"ok","protocol":"h3"}"#;
            send_response(stream, StatusCode::OK, "application/json", body.as_bytes()).await?;
        }
        "/echo" => {
            let mut body_data = Vec::new();
            while let Some(chunk) = stream.recv_data().await? {
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
            let body = format!(
                r#"{{"echo":"{echo}","size":{},"protocol":"h3"}}"#,
                body_data.len()
            );
            send_response(stream, StatusCode::OK, "application/json", body.as_bytes()).await?;
        }
        "/headers" => {
            let headers: Vec<nhttp3_qpack::HeaderField> = req
                .headers()
                .iter()
                .map(|(k, v)| nhttp3_qpack::HeaderField::new(k.as_str().as_bytes(), v.as_bytes()))
                .collect();
            let encoder = nhttp3_qpack::Encoder::new(0);
            let encoded = encoder.encode_header_block(&headers);
            let raw: usize = headers
                .iter()
                .map(|h| h.name.len() + h.value.len() + 4)
                .sum();
            let body = format!(
                r#"{{"count":{},"raw_bytes":{},"qpack_bytes":{},"savings":"{}%","protocol":"h3"}}"#,
                headers.len(),
                raw,
                encoded.len(),
                ((1.0 - encoded.len() as f64 / raw.max(1) as f64) * 100.0) as u32,
            );
            send_response(stream, StatusCode::OK, "application/json", body.as_bytes()).await?;
        }
        "/qpack-demo" => {
            let demo = vec![
                nhttp3_qpack::HeaderField::new(":method", "GET"),
                nhttp3_qpack::HeaderField::new(":path", "/api/v1/data"),
                nhttp3_qpack::HeaderField::new(":scheme", "https"),
                nhttp3_qpack::HeaderField::new("accept", "application/json"),
                nhttp3_qpack::HeaderField::new("authorization", "Bearer token123"),
                nhttp3_qpack::HeaderField::new("user-agent", "nhttp3/0.1"),
            ];
            let enc = nhttp3_qpack::Encoder::new(0);
            let dec = nhttp3_qpack::Decoder::new(0);
            let encoded = enc.encode_header_block(&demo);
            let decoded = dec.decode_header_block(&encoded).unwrap();
            let raw: usize = demo.iter().map(|h| h.name.len() + h.value.len()).sum();
            let body = format!(
                r#"{{"headers":{},"raw":{},"qpack":{},"savings":"{}%","roundtrip":{}}}"#,
                demo.len(),
                raw,
                encoded.len(),
                ((1.0 - encoded.len() as f64 / raw as f64) * 100.0) as u32,
                decoded.len() == demo.len(),
            );
            send_response(stream, StatusCode::OK, "application/json", body.as_bytes()).await?;
        }
        "/stream" => {
            let resp = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .header("server", "nhttp3")
                .body(())?;
            stream.send_response(resp).await?;
            for i in 0..10 {
                let chunk = format!("data: {{\"chunk\":{i},\"protocol\":\"h3\"}}\n\n");
                stream.send_data(Bytes::from(chunk)).await?;
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            stream
                .send_data(Bytes::from_static(b"data: [DONE]\n\n"))
                .await?;
            stream.finish().await?;
            return Ok(());
        }
        "/v1/chat/completions" => {
            let _body_data = {
                let mut d = Vec::new();
                while let Some(chunk) = stream.recv_data().await? {
                    use bytes::Buf;
                    let mut c = chunk;
                    while c.has_remaining() {
                        let b = c.chunk();
                        d.extend_from_slice(b);
                        let l = b.len();
                        c.advance(l);
                    }
                }
                d
            };
            let resp = Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .header("server", "nhttp3")
                .body(())?;
            stream.send_response(resp).await?;
            let tokens = ["Hello", "!", " I'm", " serving", " over", " HTTP/3", "."];
            for (i, tok) in tokens.iter().enumerate() {
                let done = i == tokens.len() - 1;
                let chunk = format!(
                    "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{tok}\"}},\"finish_reason\":{}}}]}}\n\n",
                    if done { "\"stop\"" } else { "null" }
                );
                stream.send_data(Bytes::from(chunk)).await?;
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            stream
                .send_data(Bytes::from_static(b"data: [DONE]\n\n"))
                .await?;
            stream.finish().await?;
            return Ok(());
        }
        _ => {
            let body = format!(r#"{{"error":"not found","path":"{path}"}}"#);
            send_response(
                stream,
                StatusCode::NOT_FOUND,
                "application/json",
                body.as_bytes(),
            )
            .await?;
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
