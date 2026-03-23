# Rust Server Guide

HTTP/3 server with demo mode and reverse proxy mode.

## Run (demo mode)

```bash
cargo run -p nhttp3-server --bin nhttp3-server
```

Endpoints:
- `GET /` — JSON hello
- `GET /health` — Health check
- `POST /echo` — Echo request body
- `GET /headers` — QPACK compression stats
- `GET /qpack-demo` — QPACK roundtrip demo
- `GET /stream` — SSE streaming (10 chunks)
- `POST /v1/chat/completions` — OpenAI-compatible streaming
- `GET /big` — 1MB response

## Run (reverse proxy)

Put HTTP/3 in front of any existing HTTP server:

```bash
# Start your backend
uvicorn myapp:app --port 8000    # or Express, Django, Rails, anything

# Add HTTP/3 frontend
cargo run -p nhttp3-server --bin nhttp3-server -- --proxy http://localhost:8000
```

Helps when clients are on high-latency networks. Does NOT help localhost (see benchmark).

## Client

```bash
cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/
cargo run -p nhttp3-server --bin nhttp3-client -- -v https://localhost:4433/health
cargo run -p nhttp3-server --bin nhttp3-client -- -X POST -d '{"k":"v"}' https://localhost:4433/echo
```

Flags: `-v` verbose (timing), `-X METHOD`, `-d BODY`

## Benchmark

```bash
cargo run -p nhttp3-server --bin nhttp3-benchmark
```

Compares HTTP/3 vs HTTP/1.1: connect time, TTFT, throughput, with latency projections.

## Native Inference Server

Token streaming directly over QUIC (no proxy hop):

```bash
cargo run -p nhttp3-server --bin nhttp3-inference -- --tokens-per-sec 100
```

OpenAI-compatible `/v1/completions` and `/v1/chat/completions` with streaming.

## E2E Tests

```bash
cargo test -p nhttp3-server --test e2e
# 5 tests, 0.12s — real HTTP/3 over real UDP
```
