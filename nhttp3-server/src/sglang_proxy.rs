//! nhttp3-sglang — HTTP/3 reverse proxy for SGLang / any OpenAI-compatible server.
//!
//! Sits in front of SGLang, vLLM, Ollama, or any server with an
//! OpenAI-compatible API and serves it over HTTP/3.
//!
//! Real improvements:
//!   - Token streaming without TCP head-of-line blocking
//!   - 1-RTT handshake (saves ~50ms per new client)
//!   - Multiple concurrent requests on independent QUIC streams
//!   - QPACK compresses repetitive LLM API headers (auth, content-type, etc.)
//!
//! Usage:
//!   # Proxy SGLang
//!   cargo run -p nhttp3-server --bin nhttp3-sglang -- --backend http://localhost:30000
//!
//!   # Proxy vLLM
//!   cargo run -p nhttp3-server --bin nhttp3-sglang -- --backend http://localhost:8000
//!
//!   # Proxy Ollama's OpenAI-compatible endpoint
//!   cargo run -p nhttp3-server --bin nhttp3-sglang -- --backend http://localhost:11434

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use clap::Parser;
use http::{Response, StatusCode};
use quinn::crypto::rustls::QuicServerConfig;

#[derive(Parser)]
#[command(name = "nhttp3-sglang", about = "HTTP/3 proxy for OpenAI-compatible LLM servers")]
struct Args {
    /// Backend server URL (SGLang, vLLM, Ollama, etc.)
    #[arg(long, default_value = "http://localhost:30000")]
    backend: String,

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

    eprintln!("=== nhttp3-sglang proxy ===");
    eprintln!("Listening:  {} (HTTP/3 / QUIC)", addr);
    eprintln!("Backend:    {}", args.backend);
    eprintln!();
    eprintln!("OpenAI-compatible endpoints:");
    eprintln!("  POST /v1/chat/completions   → streaming chat");
    eprintln!("  POST /v1/completions        → streaming completion");
    eprintln!("  GET  /v1/models             → list models");
    eprintln!("  GET  /health                → proxy health");
    eprintln!();
    eprintln!("Why HTTP/3 for LLM serving:");
    eprintln!("  - Streaming tokens arrive without TCP HOL blocking");
    eprintln!("  - 1-RTT handshake saves ~50ms per new client");
    eprintln!("  - Concurrent requests on independent QUIC streams");
    eprintln!("  - Connection survives client network changes");
    eprintln!();

    let backend = Arc::new(args.backend);
    let http_client = Arc::new(reqwest::Client::new());

    while let Some(incoming) = endpoint.accept().await {
        let backend = backend.clone();
        let client = http_client.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(incoming, &backend, &client).await {
                eprintln!("connection error: {e}");
            }
        });
    }

    Ok(())
}

async fn handle_conn(
    incoming: quinn::Incoming,
    backend: &str,
    http_client: &reqwest::Client,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = incoming.await?;
    let remote = conn.remote_address();
    eprintln!("[{remote}] HTTP/3 client connected");

    let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await?;

    while let Some(resolver) = h3_conn.accept().await? {
        let backend = backend.to_string();
        let client = http_client.clone();
        tokio::spawn(async move {
            match resolver.resolve_request().await {
                Ok((req, mut stream)) => {
                    if let Err(e) = proxy_request(req, &mut stream, &backend, &client).await {
                        eprintln!("request error: {e}");
                    }
                }
                Err(e) => eprintln!("resolve error: {e}"),
            }
        });
    }

    eprintln!("[{remote}] disconnected");
    Ok(())
}

async fn proxy_request(
    req: http::Request<()>,
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    backend: &str,
    http_client: &reqwest::Client,
) -> Result<(), Box<dyn std::error::Error>> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    match path.as_str() {
        "/health" => {
            let backend_ok = http_client.get(format!("{backend}/health")).send().await.is_ok();
            let body = serde_json::json!({
                "status": "ok",
                "proxy": "nhttp3-sglang",
                "protocol": "h3",
                "backend_reachable": backend_ok,
            });
            send_json(stream, StatusCode::OK, &body).await?;
        }

        "/v1/models" | "/v1/models/" => {
            // Proxy model listing
            proxy_get(stream, http_client, &format!("{backend}{path}")).await?;
        }

        "/v1/chat/completions" | "/v1/completions" | "/generate" => {
            // Read the request body
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

            // Check if streaming is requested
            let is_streaming = String::from_utf8_lossy(&body_data).contains("\"stream\":true")
                || String::from_utf8_lossy(&body_data).contains("\"stream\": true");

            let backend_url = format!("{backend}{path}");

            match http_client
                .post(&backend_url)
                .header("content-type", "application/json")
                .body(body_data)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = StatusCode::from_u16(resp.status().as_u16())
                        .unwrap_or(StatusCode::BAD_GATEWAY);

                    let content_type = if is_streaming {
                        "text/event-stream"
                    } else {
                        "application/json"
                    };

                    let h3_resp = Response::builder()
                        .status(status)
                        .header("content-type", content_type)
                        .header("server", "nhttp3-sglang")
                        .header("alt-svc", r#"h3=":4433"; ma=86400"#)
                        .body(())?;
                    stream.send_response(h3_resp).await?;

                    // Stream response chunks from backend → HTTP/3 client
                    // This is where the real improvement happens:
                    // Each chunk flows over a QUIC stream without HOL blocking
                    use futures_util::StreamExt;
                    let mut byte_stream = resp.bytes_stream();
                    while let Some(chunk) = byte_stream.next().await {
                        match chunk {
                            Ok(data) => {
                                stream.send_data(Bytes::from(data.to_vec())).await?;
                            }
                            Err(e) => {
                                eprintln!("backend stream error: {e}");
                                break;
                            }
                        }
                    }
                    stream.finish().await?;
                }
                Err(e) => {
                    let body = serde_json::json!({
                        "error": {"message": format!("backend unreachable: {e}"), "type": "proxy_error"}
                    });
                    send_json(stream, StatusCode::BAD_GATEWAY, &body).await?;
                }
            }
        }

        _ => {
            // Try to proxy any other path
            if method == http::Method::GET {
                proxy_get(stream, http_client, &format!("{backend}{path}")).await?;
            } else {
                let body = serde_json::json!({"error": {"message": "not found", "type": "invalid_request_error"}});
                send_json(stream, StatusCode::NOT_FOUND, &body).await?;
            }
        }
    }

    Ok(())
}

async fn proxy_get(
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    http_client: &reqwest::Client,
    url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match http_client.get(url).send().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let body = resp.bytes().await.unwrap_or_default();
            let h3_resp = Response::builder()
                .status(status)
                .header("content-type", "application/json")
                .header("server", "nhttp3-sglang")
                .body(())?;
            stream.send_response(h3_resp).await?;
            stream.send_data(Bytes::from(body.to_vec())).await?;
            stream.finish().await?;
        }
        Err(e) => {
            let body = serde_json::json!({"error": {"message": format!("backend unreachable: {e}")}});
            send_json(stream, StatusCode::BAD_GATEWAY, &body).await?;
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
    let resp = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("server", "nhttp3-sglang")
        .body(())?;
    stream.send_response(resp).await?;
    stream.send_data(Bytes::copy_from_slice(&data)).await?;
    stream.finish().await?;
    Ok(())
}
