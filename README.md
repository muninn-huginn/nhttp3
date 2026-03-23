# nhttp3

HTTP/3 server for Python, Node.js, and Rust. No proxy — native QUIC transport.

## 30-Second Start

### Python (FastAPI)

```bash
pip install nhttp3 fastapi
```

```python
from fastapi import FastAPI
import nhttp3

app = FastAPI()

@app.get("/")
async def root():
    return {"hello": "world"}

nhttp3.serve(app, port=4433)  # That's it. Native HTTP/3. No uvicorn needed.
```

### Node.js

```bash
cd nhttp3-node && npx napi build --release
```

```javascript
const { serve } = require('nhttp3-node');

serve(4433, (req) => ({
  status: 200,
  headers: [['content-type', 'application/json']],
  body: JSON.stringify({ hello: 'world' }),
}));
```

### Rust

```bash
cargo run -p nhttp3-server
```

### Test any of them

```bash
cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/
# → {"hello":"world"}
```

## What This Actually Is

An HTTP/3 server that accepts QUIC connections and calls your app directly. No proxy. No uvicorn. No nginx. Your app's response goes straight to a QUIC stream.

```
HTTP/3 request → QUIC → TLS 1.3 → nhttp3 → your app → nhttp3 → QUIC → client
```

## What's Proven (all tested, all real)

| Component | Status | Evidence |
|-----------|--------|----------|
| Python `nhttp3.serve(fastapi_app)` | **Works** | FastAPI served over native QUIC, verified with client |
| Node.js `serve(port, handler)` | **Works** | Native addon built with napi-rs, tested end-to-end |
| Rust HTTP/3 server | **Works** | 5 e2e tests over real UDP in 0.12s |
| Rust HTTP/3 client | **Works** | GET, POST, streaming, verbose timing |
| QPACK header compression | **Works** | Roundtrip tested, 50% compression on realistic headers |
| Benchmark suite | **Works** | HTTP/3 vs HTTP/1.1 with latency projections |
| 212 unit tests | **Pass** | Protocol codec, TLS handshake, frames, packets |

## When HTTP/3 Helps (and When It Doesn't)

```
cargo run -p nhttp3-server --bin nhttp3-benchmark
```

| Network | HTTP/1.1 TTFT | HTTP/3 TTFT | Winner |
|---------|---------------|-------------|--------|
| Localhost (0ms) | 10ms | 16ms | HTTP/1.1 |
| LAN (1ms RTT) | 14ms | 17ms | HTTP/1.1 |
| Regional (20ms) | 80ms | 36ms | **HTTP/3 saves 44ms** |
| Cross-continent (100ms) | 360ms | 116ms | **HTTP/3 saves 244ms** |
| Mobile (200ms) | 710ms | 216ms | **HTTP/3 saves 494ms** |

**The math:** TCP needs 3.5 RTTs to start (SYN + SYN-ACK + TLS). QUIC needs 1 RTT. Savings = 2.5 × RTT per new connection.

**Localhost:** HTTP/1.1 is faster (no TLS overhead to amortize).
**Real networks:** HTTP/3 wins. The further away your client, the bigger the win.

## For AI Agents

### CLAUDE.md / Agent Instructions

If you're an AI agent working with this codebase:

```
# Build and test
cargo test --workspace --exclude nhttp3-python --exclude nhttp3-node
cargo test -p nhttp3-server --test e2e  # 5 real HTTP/3 tests, 0.12s

# Run the server
cargo run -p nhttp3-server --bin nhttp3-server

# Hit it with the client
cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/
cargo run -p nhttp3-server --bin nhttp3-client -- -v https://localhost:4433/health
cargo run -p nhttp3-server --bin nhttp3-client -- -X POST -d '{"test":true}' https://localhost:4433/echo

# Run benchmarks
cargo run -p nhttp3-server --bin nhttp3-benchmark

# Build Python bindings
cd nhttp3-python && python3 -m venv .venv && source .venv/bin/activate
pip install maturin fastapi && maturin develop

# Build Node.js addon
cd nhttp3-node && npm install && npx napi build --release
```

### Project Layout

```
nhttp3/
├── crates/
│   ├── nhttp3-core/          # VarInt, ConnectionId, errors (19 tests)
│   ├── nhttp3-quic/          # QUIC transport, TLS, streams (134 tests)
│   │   ├── src/
│   │   │   ├── packet/       # Header parsing, PN encoding, packet builder
│   │   │   ├── frame/        # All 21 QUIC frame types (parse + serialize)
│   │   │   ├── transport/    # Transport params, error codes
│   │   │   ├── crypto/       # Key management, header protection, stateless reset
│   │   │   ├── tls/          # rustls QUIC integration
│   │   │   ├── connection/   # State machine, CID map, ConnectionInner
│   │   │   ├── stream/       # SendStream, RecvStream, flow control, manager
│   │   │   ├── recovery/     # ACK tracking, NewReno, CUBIC, BBR
│   │   │   ├── extensions/   # Datagrams (RFC 9221), Priority (RFC 9218)
│   │   │   ├── qlog/         # QLOG event logging
│   │   │   ├── endpoint.rs   # Multi-connection endpoint
│   │   │   └── io_loop.rs    # Background UDP I/O loop
│   │   ├── benches/          # Codec + connection + comparison benchmarks
│   │   └── tests/            # E2E: handshake, frames, QPACK, endpoint
│   ├── nhttp3-qpack/         # QPACK header compression (25 tests)
│   ├── nhttp3-h3/            # HTTP/3 framing (13 tests)
│   └── nhttp3/               # Umbrella re-export
├── nhttp3-server/             # Working HTTP/3 server + client + benchmarks
│   ├── src/
│   │   ├── main.rs           # Server with demo mode + --proxy mode
│   │   ├── client.rs         # HTTP/3 client (GET, POST, verbose, timing)
│   │   ├── inference.rs      # Native QUIC token streaming (LLM)
│   │   └── benchmark.rs      # HTTP/3 vs HTTP/1.1 comparison
│   └── tests/e2e.rs          # 5 real HTTP/3 tests (GET, POST, stream, QPACK, 404)
├── nhttp3-python/             # Python ASGI server (PyO3 + quinn)
│   ├── src/asgi.rs           # Real: quinn → h3 → PyO3 → ASGI app
│   └── python/nhttp3/        # nhttp3.serve(), Config, type stubs
├── nhttp3-node/               # Node.js native addon (napi-rs + quinn)
│   ├── src/lib.rs            # Real: quinn → h3 → napi callback → JS handler
│   └── test.js               # Express-like serve(port, handler)
├── nhttp3-ffi/                # C ABI layer
└── nhttp3-wasm/               # WASM bindings (QPACK + frame encoding)
```

### Key Files to Read

| Want to understand... | Read this |
|----------------------|-----------|
| How QUIC packets are parsed | `crates/nhttp3-quic/src/packet/header.rs` |
| How frames work | `crates/nhttp3-quic/src/frame/parse.rs` + `write.rs` |
| How TLS integrates | `crates/nhttp3-quic/src/tls/session.rs` |
| How the server works | `nhttp3-server/src/main.rs` |
| How Python calls work | `nhttp3-python/src/asgi.rs` |
| How Node.js calls work | `nhttp3-node/src/lib.rs` |
| How QPACK compresses | `crates/nhttp3-qpack/src/encoder.rs` + `decoder.rs` |
| Security mitigations | `crates/nhttp3-quic/src/packet/validation.rs` |

### Reverse Proxy Mode

Put HTTP/3 in front of any existing HTTP server:

```bash
# Start your app normally
uvicorn myapp:app --port 8000  # or Express, or Django, or anything

# Add HTTP/3 frontend
cargo run -p nhttp3-server --bin nhttp3-server -- --proxy http://localhost:8000
```

This helps when clients are on high-latency/lossy networks. It does NOT help for localhost (run `nhttp3-benchmark` to verify).

## RFC Coverage

| RFC | Title | Status |
|-----|-------|--------|
| [9000](https://datatracker.ietf.org/doc/html/rfc9000) | QUIC Transport | Implemented |
| [9001](https://datatracker.ietf.org/doc/html/rfc9001) | Using TLS to Secure QUIC | Implemented (rustls) |
| [9002](https://datatracker.ietf.org/doc/html/rfc9002) | Loss Detection + Congestion Control | Implemented (NewReno/CUBIC/BBR) |
| [9114](https://datatracker.ietf.org/doc/html/rfc9114) | HTTP/3 | Implemented |
| [9204](https://datatracker.ietf.org/doc/html/rfc9204) | QPACK | Implemented |
| [9218](https://datatracker.ietf.org/doc/html/rfc9218) | Extensible Priorities | Implemented |
| [9221](https://datatracker.ietf.org/doc/html/rfc9221) | QUIC Datagrams | Implemented |

## Security

Audited against 200+ issues from [aioquic](https://github.com/aiortc/aioquic/issues) and [quiche](https://github.com/cloudflare/quiche/issues). Zero `unsafe` in protocol code.

| Mitigation | Detail |
|-----------|--------|
| ACK range limit | Max 256 per frame (DoS) |
| Frame size cap | 16MB max payload |
| Initial packet size | ≥ 1200 bytes enforced |
| CRYPTO buffer | 128KB cap (OOM) |
| Stateless reset | Constant-time comparison |
| CID generation | Entropy-mixed + atomic counter |
| Idle timeout | min(local, remote) per RFC |
| Mutex safety | Graceful poisoned-lock handling |

## License

MIT
