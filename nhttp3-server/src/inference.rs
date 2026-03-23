//! nhttp3-inference — Native QUIC LLM inference server.
//!
//! Tokens flow directly from generation → QUIC stream → client.
//! No proxy hop, no HTTP/1.1 serialization, no TCP buffering.
//!
//! Architecture:
//!   Model generates token → encode as SSE chunk → write to QUIC stream
//!   That's it. One hop. The token is on the wire in microseconds.
//!
//! Pluggable backends:
//!   - MockBackend: deterministic token generation (for benchmarking)
//!   - LlamaCppBackend: llama.cpp via llama-cpp-2 crate (add as dependency)
//!   - CandleBackend: HuggingFace candle (add as dependency)
//!
//! Usage:
//!   cargo run -p nhttp3-server --bin nhttp3-inference
//!
//! Benchmark comparison:
//!   # Native QUIC (this server)
//!   cargo run -p nhttp3-server --bin nhttp3-client -- -v \
//!     -X POST -d '{"prompt":"Hello","max_tokens":100}' \
//!     https://localhost:4433/v1/completions
//!
//!   # vs Ollama through proxy (for comparison)
//!   cargo run -p nhttp3-server --bin nhttp3-ollama &
//!   cargo run -p nhttp3-server --bin nhttp3-client -- -v \
//!     -X POST -d '{"model":"llama3","prompt":"Hello"}' \
//!     https://localhost:4433/api/generate

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use clap::Parser;
use http::{Response, StatusCode};
use quinn::crypto::rustls::QuicServerConfig;
use serde::{Deserialize, Serialize};

// ─── Pluggable Inference Backend ───

trait InferenceBackend: Send + Sync {
    /// Generate tokens one at a time. Each call returns the next token
    /// or None when generation is complete.
    fn generate_next(&self, state: &mut GenerationState) -> Option<String>;

    /// Name of this backend.
    fn name(&self) -> &str;
}

struct GenerationState {
    prompt: String,
    tokens_generated: usize,
    max_tokens: usize,
    start_time: Instant,
}

// ─── Mock Backend (deterministic, for benchmarking) ───

struct MockBackend {
    /// Tokens per second to simulate
    tokens_per_sec: f64,
    /// Vocabulary to sample from
    vocab: Vec<String>,
}

impl MockBackend {
    fn new(tokens_per_sec: f64) -> Self {
        Self {
            tokens_per_sec,
            vocab: vec![
                "The".into(),
                " quick".into(),
                " brown".into(),
                " fox".into(),
                " jumps".into(),
                " over".into(),
                " the".into(),
                " lazy".into(),
                " dog".into(),
                ".".into(),
                " In".into(),
                " a".into(),
                " world".into(),
                " where".into(),
                " HTTP/3".into(),
                " enables".into(),
                " faster".into(),
                " communication".into(),
                ",".into(),
                " we".into(),
                " can".into(),
                " stream".into(),
                " tokens".into(),
                " without".into(),
                " head".into(),
                "-of".into(),
                "-line".into(),
                " blocking".into(),
                " using".into(),
                " QUIC".into(),
                " streams".into(),
                "!".into(),
            ],
        }
    }
}

impl InferenceBackend for MockBackend {
    fn generate_next(&self, state: &mut GenerationState) -> Option<String> {
        if state.tokens_generated >= state.max_tokens {
            return None;
        }

        // Simulate generation latency
        let delay = Duration::from_secs_f64(1.0 / self.tokens_per_sec);
        std::thread::sleep(delay);

        let idx = state.tokens_generated % self.vocab.len();
        state.tokens_generated += 1;
        Some(self.vocab[idx].clone())
    }

    fn name(&self) -> &str {
        "mock"
    }
}

// ─── Request/Response Types (OpenAI-compatible) ───

#[derive(Deserialize)]
struct CompletionRequest {
    #[serde(default = "default_prompt")]
    prompt: String,
    #[serde(default)]
    messages: Vec<Message>,
    #[serde(default = "default_max_tokens")]
    max_tokens: usize,
    #[serde(default = "default_stream")]
    stream: bool,
    #[serde(default = "default_model")]
    model: String,
}

#[derive(Deserialize, Serialize)]
struct Message {
    role: String,
    content: String,
}

fn default_prompt() -> String {
    String::new()
}
fn default_max_tokens() -> usize {
    100
}
fn default_stream() -> bool {
    true
}
fn default_model() -> String {
    "nhttp3-native".into()
}

// ─── Server ───

#[derive(Parser)]
#[command(name = "nhttp3-inference", about = "Native QUIC LLM inference server")]
struct Args {
    #[arg(long, default_value_t = 4433)]
    port: u16,
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
    /// Simulated tokens per second (mock backend)
    #[arg(long, default_value_t = 50.0)]
    tokens_per_sec: f64,
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

    let quic_config = quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(tls)?));
    let endpoint = quinn::Endpoint::server(quic_config, addr)?;

    let backend: Arc<dyn InferenceBackend> = Arc::new(MockBackend::new(args.tokens_per_sec));

    eprintln!("=== nhttp3-inference (native QUIC) ===");
    eprintln!("Listening:    {} (HTTP/3)", addr);
    eprintln!(
        "Backend:      {} ({:.0} tok/s)",
        backend.name(),
        args.tokens_per_sec
    );
    eprintln!("Architecture: Token → QUIC stream → Client (1 hop, no proxy)");
    eprintln!();
    eprintln!("Endpoints:");
    eprintln!("  POST /v1/completions        streaming completion");
    eprintln!("  POST /v1/chat/completions   streaming chat");
    eprintln!("  GET  /v1/models             model info");
    eprintln!("  GET  /health                server health");
    eprintln!();
    eprintln!("Test:");
    eprintln!("  cargo run -p nhttp3-server --bin nhttp3-client -- -X POST \\");
    eprintln!("    -d '{{\"prompt\":\"Hello\",\"max_tokens\":20}}' \\");
    eprintln!("    https://localhost:{}/v1/completions", args.port);
    eprintln!();

    while let Some(incoming) = endpoint.accept().await {
        let backend = backend.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(incoming, backend).await {
                eprintln!("error: {e}");
            }
        });
    }

    Ok(())
}

async fn handle_conn(
    incoming: quinn::Incoming,
    backend: Arc<dyn InferenceBackend>,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = incoming.await?;
    let remote = conn.remote_address();
    eprintln!("[{remote}] connected");

    let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await?;

    while let Some(resolver) = h3_conn.accept().await? {
        let backend = backend.clone();
        tokio::spawn(async move {
            match resolver.resolve_request().await {
                Ok((req, mut stream)) => {
                    if let Err(e) = handle_request(req, &mut stream, &backend).await {
                        eprintln!("request error: {e}");
                    }
                }
                Err(e) => eprintln!("resolve error: {e}"),
            }
        });
    }
    Ok(())
}

async fn handle_request(
    req: http::Request<()>,
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    backend: &Arc<dyn InferenceBackend>,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = req.uri().path().to_string();

    match path.as_str() {
        "/health" => {
            let body = serde_json::json!({
                "status": "ok",
                "server": "nhttp3-inference",
                "backend": backend.name(),
                "protocol": "h3",
                "architecture": "native_quic (no proxy)",
            });
            send_json(stream, StatusCode::OK, &body).await?;
        }

        "/v1/models" => {
            let body = serde_json::json!({
                "data": [{"id": "nhttp3-native", "object": "model", "owned_by": "nhttp3"}],
                "object": "list"
            });
            send_json(stream, StatusCode::OK, &body).await?;
        }

        "/v1/completions" | "/v1/chat/completions" => {
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

            let request: CompletionRequest =
                serde_json::from_slice(&body_data).unwrap_or(CompletionRequest {
                    prompt: "Hello".into(),
                    messages: vec![],
                    max_tokens: 50,
                    stream: true,
                    model: "nhttp3-native".into(),
                });

            let prompt = if !request.prompt.is_empty() {
                request.prompt.clone()
            } else if let Some(msg) = request.messages.last() {
                msg.content.clone()
            } else {
                "Hello".into()
            };

            if request.stream {
                stream_completion(stream, backend, &prompt, request.max_tokens).await?;
            } else {
                batch_completion(stream, backend, &prompt, request.max_tokens).await?;
            }
        }

        _ => {
            let body = serde_json::json!({"error": {"message": "not found"}});
            send_json(stream, StatusCode::NOT_FOUND, &body).await?;
        }
    }

    Ok(())
}

/// Stream tokens directly from backend → QUIC stream.
/// This is the native path: token generation → QUIC, no intermediary.
async fn stream_completion(
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    backend: &Arc<dyn InferenceBackend>,
    prompt: &str,
    max_tokens: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("server", "nhttp3-inference")
        .header("x-transport", "native-quic")
        .body(())?;
    stream.send_response(resp).await?;

    let start = Instant::now();
    let backend_clone = backend.clone();
    let prompt_owned = prompt.to_string();

    // Run inference on a blocking thread (simulates GPU work)
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Option<String>>(32);

    tokio::task::spawn_blocking(move || {
        let mut state = GenerationState {
            prompt: prompt_owned,
            tokens_generated: 0,
            max_tokens,
            start_time: Instant::now(),
        };

        while let Some(token) = backend_clone.generate_next(&mut state) {
            if tx.blocking_send(Some(token)).is_err() {
                break;
            }
        }
        let _ = tx.blocking_send(None); // Signal completion
    });

    // Stream tokens as they arrive → directly to QUIC stream
    let mut token_count = 0;
    while let Some(maybe_token) = rx.recv().await {
        match maybe_token {
            Some(token) => {
                token_count += 1;
                let chunk = serde_json::json!({
                    "id": "cmpl-nhttp3",
                    "object": "text_completion",
                    "choices": [{
                        "text": token,
                        "index": 0,
                        "finish_reason": serde_json::Value::Null,
                    }],
                });
                let data = format!("data: {}\n\n", serde_json::to_string(&chunk)?);

                // THIS IS THE KEY: token → QUIC stream, no proxy, no TCP
                stream.send_data(Bytes::from(data)).await?;
            }
            None => {
                // Final chunk
                let elapsed = start.elapsed();
                let tps = token_count as f64 / elapsed.as_secs_f64();

                let chunk = serde_json::json!({
                    "id": "cmpl-nhttp3",
                    "object": "text_completion",
                    "choices": [{
                        "text": "",
                        "index": 0,
                        "finish_reason": "stop",
                    }],
                    "usage": {
                        "completion_tokens": token_count,
                        "total_time_ms": elapsed.as_millis(),
                        "tokens_per_second": format!("{:.1}", tps),
                        "transport": "native_quic",
                    }
                });
                let data = format!(
                    "data: {}\n\ndata: [DONE]\n\n",
                    serde_json::to_string(&chunk)?
                );
                stream.send_data(Bytes::from(data)).await?;
                stream.finish().await?;

                eprintln!(
                    "  generated {} tokens in {:?} ({:.1} tok/s, native QUIC)",
                    token_count, elapsed, tps
                );
                break;
            }
        }
    }

    Ok(())
}

/// Non-streaming completion — returns all tokens at once.
async fn batch_completion(
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    backend: &Arc<dyn InferenceBackend>,
    prompt: &str,
    max_tokens: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let backend_clone = backend.clone();
    let prompt_owned = prompt.to_string();

    let (text, token_count, elapsed) = tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let mut state = GenerationState {
            prompt: prompt_owned,
            tokens_generated: 0,
            max_tokens,
            start_time: start,
        };
        let mut text = String::new();
        while let Some(token) = backend_clone.generate_next(&mut state) {
            text.push_str(&token);
        }
        (text, state.tokens_generated, start.elapsed())
    })
    .await?;

    let tps = token_count as f64 / elapsed.as_secs_f64();
    let body = serde_json::json!({
        "id": "cmpl-nhttp3",
        "object": "text_completion",
        "choices": [{"text": text, "index": 0, "finish_reason": "stop"}],
        "usage": {
            "completion_tokens": token_count,
            "total_time_ms": elapsed.as_millis(),
            "tokens_per_second": format!("{:.1}", tps),
            "transport": "native_quic",
        }
    });
    send_json(stream, StatusCode::OK, &body).await?;
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
        .header("server", "nhttp3-inference")
        .body(())?;
    stream.send_response(resp).await?;
    stream.send_data(Bytes::copy_from_slice(&data)).await?;
    stream.finish().await?;
    Ok(())
}
