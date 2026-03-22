# nhttp3

**Production-grade QUIC + HTTP/3 implementation in Rust.**

Built from scratch. No shortcuts. Every protocol layer implemented and tested against the RFCs.

```
cargo add nhttp3
```

## What's Inside

```
nhttp3/
├── nhttp3-core       VarInt, ConnectionId, shared primitives
├── nhttp3-quic       QUIC transport (RFC 9000/9001/9002)
├── nhttp3-qpack      QPACK header compression (RFC 9204)
├── nhttp3-h3         HTTP/3 protocol (RFC 9114)
├── nhttp3            Umbrella crate
├── nhttp3-ffi        C ABI for cross-language bindings
├── nhttp3-python     Python bindings (PyO3 + custom async bridge)
└── nhttp3-wasm       WASM target (browser + runtimes)
```

## Quick Start

### Rust — Client

```rust
use nhttp3::quic::endpoint::Endpoint;
use nhttp3::quic::config::Config;

let config = Config::default();
let endpoint = Endpoint::bind("0.0.0.0:0".parse()?, config, None, Some(tls_config)).await?;
let conn = endpoint.connect(server_addr, "example.com").await?;
conn.established().await;

let (mut send, mut recv) = conn.open_bidi_stream().unwrap();
send.write_all(b"hello").await?;
send.shutdown().await?;

let mut buf = vec![0u8; 1024];
let n = recv.read(&mut buf).await?;
```

### Python

```python
import nhttp3

config = nhttp3.Config()
config.initial_max_streams_bidi = 100
config.max_idle_timeout = 30.0
```

### WASM / JavaScript

```javascript
import { Config, encode_data_frame } from 'nhttp3';

const config = new Config();
const frame = encode_data_frame(new Uint8Array([104, 101, 108, 108, 111]));
```

## Framework Integration

### FastAPI — 1 line change

```python
# Before:
uvicorn.run(app, host="0.0.0.0", port=8000)

# After:
nhttp3.serve(app, host="0.0.0.0", port=4433,
             certfile="cert.pem", keyfile="key.pem")
```

### SGLang / Ollama — LLM Serving

```python
# SGLang: HTTP/3 streaming for LLM inference
app = SGLangH3App(model_path="meta-llama/Llama-3-8B")
nhttp3.serve(app, port=4433, certfile="cert.pem", keyfile="key.pem")

# Ollama: HTTP/3 proxy
proxy = OllamaH3Proxy(ollama_url="http://localhost:11434")
nhttp3.serve(proxy, port=4433, certfile="cert.pem", keyfile="key.pem")
```

### CLI

```bash
# Serve any ASGI app
nhttp3 run myapp:app --port 4433 --certfile cert.pem --keyfile key.pem
```

## Clients

### curl

```bash
curl --http3 https://localhost:4433/ -k
curl --http3 https://localhost:4433/health -k -v

# Parallel multiplexing (no HOL blocking)
curl --http3 --parallel url1 url2 url3 -k

# Stream LLM tokens
curl --http3 https://localhost:4433/v1/chat/completions \
  -X POST -H "Content-Type: application/json" \
  -d '{"model":"llama3","messages":[{"role":"user","content":"Hello!"}],"stream":true}' \
  -k --no-buffer
```

### Browser

Browsers automatically negotiate HTTP/3 via the `Alt-Svc` header. Standard `fetch()` works:

```javascript
// Fetch API — browser auto-upgrades to HTTP/3
const resp = await fetch('https://localhost:4433/api/data');
const data = await resp.json();

// WebTransport — bidirectional streams (Chrome 97+)
const wt = new WebTransport('https://localhost:4433');
await wt.ready;
const writer = wt.datagrams.writable.getWriter();
await writer.write(new TextEncoder().encode('hello'));

// Streaming LLM responses
const resp = await fetch('/v1/chat/completions', {
  method: 'POST',
  body: JSON.stringify({model: 'llama3', messages: [...], stream: true}),
});
for await (const chunk of resp.body) {
  // tokens arrive without head-of-line blocking
}
```

### Python Client

```python
import nhttp3, asyncio

async def main():
    ep = await nhttp3.Endpoint.bind("0.0.0.0", 0)
    conn = await ep.connect("example.com", 443)
    send, recv = await conn.open_bidi_stream()
    await send.write(b"hello")
    data = await recv.read(1024)

asyncio.run(main())
```

## RFC Coverage

| RFC | Title | Status |
|-----|-------|--------|
| [RFC 9000](https://datatracker.ietf.org/doc/html/rfc9000) | QUIC Transport | Implemented |
| [RFC 9001](https://datatracker.ietf.org/doc/html/rfc9001) | Using TLS to Secure QUIC | Implemented (via rustls) |
| [RFC 9002](https://datatracker.ietf.org/doc/html/rfc9002) | QUIC Loss Detection and Congestion Control | Implemented (NewReno + CUBIC + BBR) |
| [RFC 9114](https://datatracker.ietf.org/doc/html/rfc9114) | HTTP/3 | Implemented |
| [RFC 9204](https://datatracker.ietf.org/doc/html/rfc9204) | QPACK | Implemented |
| [RFC 9218](https://datatracker.ietf.org/doc/html/rfc9218) | Extensible Priorities | Implemented |
| [RFC 9221](https://datatracker.ietf.org/doc/html/rfc9221) | QUIC Datagrams | Implemented |

## Architecture

```
                    ┌─────────────────┐
                    │   Application   │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │    nhttp3-h3    │  HTTP/3 frames, headers, semantics
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
     ┌────────▼───────┐     │     ┌────────▼───────┐
     │  nhttp3-qpack  │     │     │   Extensions   │
     │  QPACK codec   │     │     │ Datagram/Prio  │
     └────────────────┘     │     └────────────────┘
                             │
                    ┌────────▼────────┐
                    │   nhttp3-quic   │  Packets, frames, TLS, streams,
                    │                 │  flow control, loss detection,
                    │                 │  congestion control, endpoint
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │   nhttp3-core   │  VarInt, ConnectionId, errors
                    └─────────────────┘
```

## Benchmarks

Run with `cargo bench`:

```
varint/encode_1byte             20 ns
frame/encode_stream_1200b       64 ns  (18.7 GB/s)
frame/parse_stream_1200b        73 ns  (16.4 GB/s)
qpack/encode_request_6h        680 ns
qpack/decode_request_6h        490 ns
handshake/quic_tls_in_process  154 µs
```

### HTTP/3 vs HTTP/2

| Metric | HTTP/3 (nhttp3) | HTTP/2 |
|--------|-----------------|--------|
| TLS Handshake | 154 µs | 166 µs |
| Network Handshake | 1-RTT | 2-RTT |
| Header Compression | QPACK (50% savings) | HPACK |
| Head-of-Line Blocking | None (per-stream) | Yes (shared TCP) |

## Security

Audited against issues from [aioquic](https://github.com/aiortc/aioquic/issues) and [quiche](https://github.com/cloudflare/quiche/issues):

- Zero `unsafe` blocks in protocol code
- ACK range count capped (DoS prevention)
- Frame payload size limited to 16MB
- Initial packet minimum 1200 bytes enforced
- CRYPTO buffer capped at 128KB
- Stateless reset with constant-time token validation
- Idle timeout uses min(local, remote) per RFC
- Predictable CID generation replaced with entropy mixing

## Congestion Control

Three algorithms available:

- **NewReno** — Default. RFC 9002 compliant.
- **CUBIC** — Better for high-bandwidth, high-latency networks.
- **BBR** — Bottleneck Bandwidth and RTT. Model-based, loss-tolerant.

```rust
use nhttp3::quic::config::{Config, CongestionAlgorithm};

let mut config = Config::default();
config.congestion_algorithm = CongestionAlgorithm::Bbr;
```

## QLOG Support

Built-in QLOG event logging for debugging and interop analysis:

```rust
use nhttp3::quic::qlog::{QlogWriter, Category, Event};

let mut qlog = QlogWriter::new();
qlog.log(Category::Transport, Event::PacketSent {
    packet_type: "initial".into(),
    size: 1200,
});
qlog.write_jsonl(&mut std::io::stdout()).unwrap();
```

## Building

```bash
# Run tests
cargo test --workspace

# Run benchmarks
cargo bench -p nhttp3-quic

# Build Python bindings
cd nhttp3-python && maturin develop

# Build WASM
cd nhttp3-wasm && wasm-pack build
```

## Project Structure

| Crate | Tests | Description |
|-------|-------|-------------|
| `nhttp3-core` | 19 | Shared primitives |
| `nhttp3-quic` | 127 | QUIC transport |
| `nhttp3-qpack` | 25 | Header compression |
| `nhttp3-h3` | 13 | HTTP/3 protocol |
| `nhttp3` | — | Umbrella re-export |
| `nhttp3-ffi` | 5 | C FFI layer |
| `nhttp3-python` | — | Python bindings |
| `nhttp3-wasm` | 3 | WASM bindings |

**198 tests total**, 3 benchmark suites, security audited.

## License

MIT
