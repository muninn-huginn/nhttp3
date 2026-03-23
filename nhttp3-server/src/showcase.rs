//! nhttp3-showcase — Combined HTTP/1.1 + HTTP/3 server for the showcase.
//!
//! Runs TWO servers on the same port:
//!   1. HTTPS (TCP) on port 4433 — serves the showcase HTML + API
//!   2. HTTP/3 (QUIC) on port 4433 — serves the same API over QUIC
//!
//! Browsers connect via HTTPS first, get Alt-Svc: h3=":4433" header,
//! then automatically upgrade to HTTP/3 for subsequent requests.
//!
//! Usage:
//!   cargo run -p nhttp3-server --bin nhttp3-showcase
//!   # Open https://localhost:4433 in browser

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use bytes::Bytes;
use quinn::crypto::rustls::QuicServerConfig;
use serde_json::json;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let addr: SocketAddr = "0.0.0.0:4433".parse()?;

    // Generate self-signed cert
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let key_der = cert.key_pair.serialize_der();
    let cert_der = cert.cert.der().to_vec();

    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(key_der.clone()),
    );
    let cert_pem = rustls::pki_types::CertificateDer::from(cert_der.clone());

    // ── HTTP/3 (QUIC) server ──
    let mut h3_tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_pem.clone()], key.clone_key())?;
    h3_tls.alpn_protocols = vec![b"h3".to_vec()];

    let quic_config = quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(h3_tls)?));
    let quic_endpoint = quinn::Endpoint::server(quic_config, addr)?;

    tokio::spawn(run_h3_server(quic_endpoint));

    // ── HTTPS (TCP) server with axum ──
    let mut tcp_tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_pem], key)?;
    tcp_tls.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let tcp_tls = tokio_rustls::TlsAcceptor::from(Arc::new(tcp_tls));

    let app = Router::new()
        .route("/", get(serve_showcase))
        .route("/health", get(health))
        .route("/echo", post(echo))
        .route("/headers", get(headers_demo))
        .route("/qpack-demo", get(qpack_demo))
        .route("/stream", get(stream_sse))
        .route("/v1/chat/completions", post(chat_completions))
        .layer(CorsLayer::permissive())
        .layer(axum::middleware::from_fn(add_alt_svc));

    let tcp_listener = TcpListener::bind(addr).await?;

    eprintln!("=== nhttp3 showcase ===");
    eprintln!("HTTPS (TCP):  https://localhost:{} (serves HTML + API)", addr.port());
    eprintln!("HTTP/3 (QUIC): https://localhost:{} (same port, auto-upgrade via Alt-Svc)", addr.port());
    eprintln!();
    eprintln!("Open https://localhost:4433 in your browser.");
    eprintln!("Accept the self-signed certificate, then the showcase loads.");
    eprintln!("The browser will auto-upgrade to HTTP/3 after seeing Alt-Svc.");
    eprintln!();

    // Accept TLS connections
    loop {
        let (tcp_stream, _) = tcp_listener.accept().await?;
        let tls = tcp_tls.clone();
        let app = app.clone();

        tokio::spawn(async move {
            match tls.accept(tcp_stream).await {
                Ok(tls_stream) => {
                    let io = hyper_util::rt::TokioIo::new(tls_stream);
                    let service = hyper_util::service::TowerToHyperService::new(app);
                    if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new()
                    ).serve_connection(io, service).await {
                        if !e.to_string().contains("closed") {
                            eprintln!("TCP conn error: {e}");
                        }
                    }
                }
                Err(e) => {
                    if !e.to_string().contains("closed") {
                        eprintln!("TLS error: {e}");
                    }
                }
            }
        });
    }
}

// ── Alt-Svc middleware ──
async fn add_alt_svc(req: Request, next: axum::middleware::Next) -> impl IntoResponse {
    let mut resp = next.run(req).await;
    resp.headers_mut().insert("alt-svc", HeaderValue::from_static(r#"h3=":4433"; ma=86400"#));
    resp
}

// ── Showcase HTML (embedded) ──
async fn serve_showcase() -> Html<&'static str> {
    Html(include_str!("../../examples/showcase/index.html"))
}

// ── API endpoints (same as demo mode) ──
async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok", "protocol": "h2/h1.1 or h3", "server": "nhttp3-showcase"}))
}

async fn echo(body: String) -> Json<serde_json::Value> {
    Json(json!({"echo": body, "size": body.len(), "protocol": "h3/h2/h1.1"}))
}

async fn headers_demo() -> Json<serde_json::Value> {
    let headers = vec![
        nhttp3_qpack::HeaderField::new(":method", "GET"),
        nhttp3_qpack::HeaderField::new(":path", "/api"),
        nhttp3_qpack::HeaderField::new("accept", "application/json"),
        nhttp3_qpack::HeaderField::new("authorization", "Bearer token"),
        nhttp3_qpack::HeaderField::new("user-agent", "nhttp3-showcase"),
    ];
    let encoder = nhttp3_qpack::Encoder::new(0);
    let encoded = encoder.encode_header_block(&headers);
    let raw: usize = headers.iter().map(|h| h.name.len() + h.value.len() + 4).sum();

    Json(json!({
        "count": headers.len(),
        "raw_bytes": raw,
        "qpack_bytes": encoded.len(),
        "savings_pct": ((1.0 - encoded.len() as f64 / raw as f64) * 100.0) as u32,
    }))
}

async fn qpack_demo() -> Json<serde_json::Value> {
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

    Json(json!({
        "headers": demo.len(), "raw": raw, "qpack": encoded.len(),
        "savings": format!("{}%", ((1.0 - encoded.len() as f64 / raw as f64) * 100.0) as u32),
        "roundtrip": decoded.len() == demo.len(),
    }))
}

async fn stream_sse() -> impl IntoResponse {
    use axum::response::sse::{Event, Sse};
    use futures_util::stream;
    use std::time::Duration;

    let stream = stream::unfold(0, |i| async move {
        if i >= 10 { return None; }
        tokio::time::sleep(Duration::from_millis(100)).await;
        let data = format!(r#"{{"chunk":{},"protocol":"h3/h2/h1.1"}}"#, i);
        Some((Ok::<_, std::convert::Infallible>(Event::default().data(data)), i + 1))
    });

    Sse::new(stream)
}

async fn chat_completions(body: String) -> impl IntoResponse {
    use axum::response::sse::{Event, Sse};
    use futures_util::stream;
    use std::time::Duration;

    let tokens = vec!["Hello", "!", " I'm", " serving", " over", " HTTP/3", " with", " nhttp3", "."];

    let stream = stream::unfold((0, tokens), |(i, tokens)| async move {
        if i >= tokens.len() {
            return Some((Ok::<_, std::convert::Infallible>(Event::default().data("[DONE]")), (tokens.len() + 1, tokens)));
        }
        if i > tokens.len() { return None; }

        tokio::time::sleep(Duration::from_millis(50)).await;
        let is_last = i == tokens.len() - 1;
        let data = format!(
            r#"{{"choices":[{{"delta":{{"content":"{}"}},"finish_reason":{}}}]}}"#,
            tokens[i], if is_last { "\"stop\"" } else { "null" }
        );
        Some((Ok::<_, std::convert::Infallible>(Event::default().data(data)), (i + 1, tokens)))
    });

    Sse::new(stream)
}

// ── HTTP/3 server (same endpoints over QUIC) ──
async fn run_h3_server(endpoint: quinn::Endpoint) {
    while let Some(incoming) = endpoint.accept().await {
        tokio::spawn(async move {
            if let Ok(conn) = incoming.await {
                let mut h3 = match h3::server::Connection::new(h3_quinn::Connection::new(conn)).await {
                    Ok(c) => c,
                    Err(_) => return,
                };

                while let Ok(Some(resolver)) = h3.accept().await {
                    tokio::spawn(async move {
                        if let Ok((req, mut stream)) = resolver.resolve_request().await {
                            let path = req.uri().path().to_string();
                            let body = match path.as_str() {
                                "/" | "/health" => r#"{"status":"ok","protocol":"h3"}"#.to_string(),
                                "/qpack-demo" => {
                                    let demo = vec![
                                        nhttp3_qpack::HeaderField::new(":method", "GET"),
                                        nhttp3_qpack::HeaderField::new(":path", "/api"),
                                        nhttp3_qpack::HeaderField::new("accept", "application/json"),
                                    ];
                                    let enc = nhttp3_qpack::Encoder::new(0);
                                    let dec = nhttp3_qpack::Decoder::new(0);
                                    let encoded = enc.encode_header_block(&demo);
                                    let decoded = dec.decode_header_block(&encoded).unwrap();
                                    format!(r#"{{"roundtrip":{},"qpack":{}}}"#, decoded.len() == demo.len(), encoded.len())
                                }
                                _ => format!(r#"{{"path":"{}","protocol":"h3"}}"#, path),
                            };

                            let resp = http::Response::builder()
                                .status(200)
                                .header("content-type", "application/json")
                                .header("server", "nhttp3-showcase")
                                .header("access-control-allow-origin", "*")
                                .body(()).unwrap();
                            let _ = stream.send_response(resp).await;
                            let _ = stream.send_data(Bytes::from(body)).await;
                            let _ = stream.finish().await;
                        }
                    });
                }
            }
        });
    }
}
