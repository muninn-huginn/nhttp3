# CLAUDE.md — nhttp3

## What this project is

HTTP/3 server implementation in Rust with Python and Node.js bindings. Uses quinn for QUIC transport, nhttp3's own QPACK/H3 layers for protocol handling. Everything is tested and working.

## Quick commands

```bash
# Run all tests (212 tests, ~2s)
cargo test --workspace --exclude nhttp3-python --exclude nhttp3-node

# Run e2e HTTP/3 tests (5 tests, 0.12s, real UDP)
cargo test -p nhttp3-server --test e2e

# Start the HTTP/3 server
cargo run -p nhttp3-server --bin nhttp3-server

# Hit it with the client
cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/
cargo run -p nhttp3-server --bin nhttp3-client -- -v https://localhost:4433/health
cargo run -p nhttp3-server --bin nhttp3-client -- -X POST -d '{"k":"v"}' https://localhost:4433/echo

# Reverse proxy mode (put HTTP/3 in front of any HTTP server)
cargo run -p nhttp3-server --bin nhttp3-server -- --proxy http://localhost:8000

# Benchmarks
cargo bench -p nhttp3-quic --bench codec
cargo run -p nhttp3-server --bin nhttp3-benchmark

# Python
cd nhttp3-python && python3 -m venv .venv && source .venv/bin/activate
pip install maturin fastapi && maturin develop
python3 -c "from fastapi import FastAPI; import nhttp3; app = FastAPI(); nhttp3.serve(app, port=4433)"

# Node.js
cd nhttp3-node && npm install && npx napi build --release
node -e "const{serve}=require('./index');serve(4433,(r)=>({status:200,headers:[['content-type','application/json']],body:'{\"ok\":true}'}))"
```

## Architecture

```
crates/nhttp3-core     → VarInt, ConnectionId, errors
crates/nhttp3-quic     → QUIC transport (packets, frames, TLS, streams, recovery)
crates/nhttp3-qpack    → QPACK header compression
crates/nhttp3-h3       → HTTP/3 framing
nhttp3-server          → Working server + client + benchmarks (uses quinn for transport)
nhttp3-python          → Python bindings: nhttp3.serve(asgi_app) via PyO3
nhttp3-node            → Node.js addon: serve(port, handler) via napi-rs
```

## How requests flow

**Python (nhttp3.serve):**
```
HTTP/3 request → quinn (QUIC) → h3 (HTTP/3) → PyO3 → ASGI app → PyO3 → h3 → quinn → QUIC response
```

**Node.js (serve):**
```
HTTP/3 request → quinn (QUIC) → h3 (HTTP/3) → napi → JS callback → napi → h3 → quinn → QUIC response
```

**Rust (nhttp3-server):**
```
HTTP/3 request → quinn → h3 → handler → h3 → quinn → QUIC response
```

## Test patterns

Tests are in two places:
- `crates/*/src/*.rs` — unit tests (#[cfg(test)])
- `nhttp3-server/tests/e2e.rs` — real HTTP/3 over real UDP

To add a new endpoint: edit `nhttp3-server/src/main.rs` in the `demo_request` match block, add an e2e test in `tests/e2e.rs`.

## Do NOT do

- Don't add proxies that claim to "accelerate" — benchmarks proved localhost proxies add latency
- Don't mock HTTP/3 — use the real quinn+h3 server for all testing
- Don't use nhttp3-quic's Endpoint directly — it doesn't encrypt packets. Use quinn.
- nhttp3-python and nhttp3-node exclude from `cargo test` (need their runtimes)

## Key decisions

- **quinn for QUIC transport**: nhttp3-quic has all protocol building blocks but doesn't do AEAD encryption. quinn handles the crypto. Our code handles QPACK, HTTP/3 framing, and application bridging.
- **PyO3 for Python**: ASGI app called directly from Rust via `Python::attach()` + `asyncio.run()`. GIL released during QUIC I/O.
- **napi-rs for Node.js**: ThreadsafeFunction bridges JS callbacks across tokio threads.
- **Self-signed certs**: Auto-generated via rcgen for dev. Production would load from files.
