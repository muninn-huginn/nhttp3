//! nhttp3-ollama — HTTP/3 reverse proxy for Ollama.
//!
//! Accepts HTTP/3 connections from clients, proxies to Ollama's
//! HTTP/1.1 REST API on localhost:11434. Streaming responses flow
//! back over QUIC streams without head-of-line blocking.
//!
//! Real improvement over direct Ollama access:
//!   - 1-RTT connection setup (vs 2-RTT TCP+TLS)
//!   - Streaming tokens on independent QUIC streams (no HOL blocking)
//!   - Connection migration (mobile clients survive network changes)
//!   - QPACK header compression (50% savings)
//!
//! Usage:
//!   cargo run -p nhttp3-server --bin nhttp3-ollama
//!   cargo run -p nhttp3-server --bin nhttp3-ollama -- --ollama-url http://localhost:11434 --port 4433

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use clap::Parser;
use http::{Response, StatusCode};
use quinn::crypto::rustls::QuicServerConfig;

#[derive(Parser)]
#[command(name = "nhttp3-ollama", about = "HTTP/3 proxy for Ollama")]
struct Args {
    /// Ollama backend URL
    #[arg(long, default_value = "http://localhost:11434")]
    ollama_url: String,

    /// Listen port
    #[arg(long, default_value_t = 4433)]
    port: u16,

    /// Listen host
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let args = Args::parse();

    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    // Generate self-signed cert
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()),
    );
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert);

    let mut tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key)?;
    tls.alpn_protocols = vec![b"h3".to_vec()];

    let quic_config = quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(tls)?));
    let endpoint = quinn::Endpoint::server(quic_config, addr)?;

    eprintln!("=== nhttp3-ollama proxy ===");
    eprintln!("Listening:  {} (HTTP/3)", addr);
    eprintln!("Backend:    {}", args.ollama_url);
    eprintln!();
    eprintln!("Endpoints proxied:");
    eprintln!("  POST /api/generate    → streaming text generation");
    eprintln!("  POST /api/chat        → streaming chat");
    eprintln!("  GET  /api/tags        → list models");
    eprintln!("  GET  /api/version     → Ollama version");
    eprintln!("  GET  /health          → proxy health check");
    eprintln!();
    eprintln!("Test:");
    eprintln!("  cargo run -p nhttp3-server --bin nhttp3-client -- -v https://localhost:{}/health", args.port);
    eprintln!("  cargo run -p nhttp3-server --bin nhttp3-client -- -X POST -d '{{\"model\":\"llama3\",\"prompt\":\"Hi\"}}' https://localhost:{}/api/generate", args.port);
    eprintln!();

    let ollama_url = Arc::new(args.ollama_url);
    let http_client = Arc::new(reqwest::Client::new());

    while let Some(incoming) = endpoint.accept().await {
        let ollama = ollama_url.clone();
        let client = http_client.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(incoming, &ollama, &client).await {
                eprintln!("connection error: {e}");
            }
        });
    }

    Ok(())
}

async fn handle_conn(
    incoming: quinn::Incoming,
    ollama_url: &str,
    http_client: &reqwest::Client,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = incoming.await?;
    let remote = conn.remote_address();
    eprintln!("[{remote}] connected via HTTP/3");

    let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await?;

    while let Some(resolver) = h3_conn.accept().await? {
        let ollama = ollama_url.to_string();
        let client = http_client.clone();
        let remote = remote;
        tokio::spawn(async move {
            match resolver.resolve_request().await {
                Ok((req, mut stream)) => {
                    if let Err(e) = proxy_request(req, &mut stream, &ollama, &client, remote).await {
                        eprintln!("[{remote}] error: {e}");
                    }
                }
                Err(e) => eprintln!("[{remote}] resolve error: {e}"),
            }
        });
    }

    eprintln!("[{remote}] disconnected");
    Ok(())
}

async fn proxy_request(
    req: http::Request<()>,
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    ollama_url: &str,
    http_client: &reqwest::Client,
    remote: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    eprintln!("[{remote}] {method} {path}");

    match path.as_str() {
        "/health" => {
            // Proxy health — check if Ollama is reachable
            let ollama_ok = http_client
                .get(format!("{ollama_url}/"))
                .send()
                .await
                .is_ok();

            let body = serde_json::json!({
                "status": "ok",
                "proxy": "nhttp3-ollama",
                "protocol": "h3",
                "ollama_reachable": ollama_ok,
                "backend": ollama_url,
            });
            send_json(stream, StatusCode::OK, &body).await?;
        }

        "/api/tags" | "/api/version" => {
            // Proxy GET requests directly
            let backend_url = format!("{ollama_url}{path}");
            match http_client.get(&backend_url).send().await {
                Ok(resp) => {
                    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                    let body_bytes = resp.bytes().await.unwrap_or_default();
                    send_raw(stream, status, "application/json", &body_bytes).await?;
                }
                Err(e) => {
                    let body = serde_json::json!({"error": format!("backend unreachable: {e}"), "backend": ollama_url});
                    send_json(stream, StatusCode::BAD_GATEWAY, &body).await?;
                }
            }
        }

        "/api/generate" | "/api/chat" => {
            // Read request body from HTTP/3 stream
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

            // Forward to Ollama backend
            let backend_url = format!("{ollama_url}{path}");
            match http_client
                .post(&backend_url)
                .header("content-type", "application/json")
                .body(body_data)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

                    // Stream the response back over HTTP/3
                    let h3_resp = Response::builder()
                        .status(status)
                        .header("content-type", "application/x-ndjson")
                        .header("server", "nhttp3-ollama")
                        .header("alt-svc", r#"h3=":4433"; ma=86400"#)
                        .body(())?;
                    stream.send_response(h3_resp).await?;

                    // Stream chunks from Ollama → client over QUIC
                    use futures_util::StreamExt;
                    let mut byte_stream = resp.bytes_stream();
                    while let Some(chunk) = byte_stream.next().await {
                        match chunk {
                            Ok(data) => {
                                stream.send_data(Bytes::from(data.to_vec())).await?;
                            }
                            Err(e) => {
                                eprintln!("[{remote}] stream error: {e}");
                                break;
                            }
                        }
                    }
                    stream.finish().await?;
                }
                Err(e) => {
                    let body = serde_json::json!({"error": format!("backend error: {e}")});
                    send_json(stream, StatusCode::BAD_GATEWAY, &body).await?;
                }
            }
        }

        _ => {
            let body = serde_json::json!({"error": "not found", "path": path});
            send_json(stream, StatusCode::NOT_FOUND, &body).await?;
        }
    }

    Ok(())
}

async fn send_json(
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    status: StatusCode,
    body: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let data = serde_json::to_vec(body)?;
    send_raw(stream, status, "application/json", &data).await
}

async fn send_raw(
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    status: StatusCode,
    content_type: &str,
    body: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let resp = Response::builder()
        .status(status)
        .header("content-type", content_type)
        .header("server", "nhttp3-ollama")
        .header("alt-svc", r#"h3=":4433"; ma=86400"#)
        .body(())?;
    stream.send_response(resp).await?;
    stream.send_data(Bytes::copy_from_slice(body)).await?;
    stream.finish().await?;
    Ok(())
}
