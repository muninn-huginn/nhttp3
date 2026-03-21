# nhttp3 — Production-Grade HTTP/3 Implementation in Rust

## Overview

`nhttp3` is a from-scratch, production-grade QUIC + HTTP/3 implementation in Rust. It targets comprehensive RFC coverage starting with the core specs, with Python bindings (client + server) and WASM support (browser + general runtimes) as downstream targets.

- **License**: MIT
- **TLS**: rustls
- **Async runtime**: tokio
- **Project name**: nhttp3

## Core RFCs (Phase 1-2)

- RFC 9000 — QUIC Transport
- RFC 9001 — Using TLS to Secure QUIC
- RFC 9002 — QUIC Loss Detection and Congestion Control
- RFC 9114 — HTTP/3
- RFC 9204 — QPACK: Field Compression for HTTP/3

## Architecture: Monorepo with Layered Crates

```
nhttp3/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── nhttp3-core/            # Shared primitives
│   ├── nhttp3-quic/            # QUIC transport
│   ├── nhttp3-qpack/           # QPACK codec
│   ├── nhttp3-h3/              # HTTP/3 protocol
│   └── nhttp3/                 # Umbrella crate
├── nhttp3-ffi/                 # C ABI for Python/WASM bindings
├── nhttp3-python/              # PyO3 bindings
├── nhttp3-wasm/                # WASM target
├── tests/                      # Integration tests across layers
├── benches/                    # Benchmarks
├── fuzz/                       # Fuzz testing targets
└── docs/
```

### Dependency Graph

```
nhttp3-core  <-  nhttp3-quic  <-  nhttp3-h3
                                      ^
nhttp3-core  <-  nhttp3-qpack -------/

nhttp3 (umbrella) re-exports all of the above

nhttp3-ffi     depends on  nhttp3
nhttp3-python  depends on  nhttp3-ffi
nhttp3-wasm    depends on  nhttp3
```

---

## Crate: `nhttp3-core`

Shared primitives with minimal dependencies.

**Contains**:
- Variable-length integer encoding/decoding (QUIC varint format)
- Buffer/bytes management (`bytes` crate integration)
- Common error types
- Connection ID types
- Shared config structs

---

## Crate: `nhttp3-quic`

QUIC transport implementing RFC 9000, 9001, 9002.

### Module Structure

```
nhttp3-quic/src/
├── connection/
│   ├── mod.rs          # Connection state machine
│   ├── handshake.rs    # TLS handshake integration (rustls)
│   ├── id.rs           # Connection ID management & retirement
│   └── migration.rs    # Connection migration & path validation
├── stream/
│   ├── mod.rs          # Stream state machine (bidi + uni)
│   ├── flow_control.rs # Stream & connection-level flow control
│   └── manager.rs      # Stream lifecycle, concurrency limits
├── packet/
│   ├── mod.rs          # Packet types (Initial, Handshake, 0-RTT, 1-RTT)
│   ├── header.rs       # Header parsing, protection/unprotection
│   ├── number.rs       # Packet number encoding, spaces
│   └── coalesce.rs     # Packet coalescing
├── frame/
│   ├── mod.rs          # Frame types (STREAM, ACK, CRYPTO, etc.)
│   ├── parse.rs        # Frame decoding
│   └── write.rs        # Frame encoding
├── recovery/
│   ├── mod.rs          # Loss detection (RFC 9002)
│   ├── ack.rs          # ACK tracking & generation
│   ├── congestion.rs   # Congestion controller trait + implementations
│   ├── reno.rs         # NewReno (default)
│   └── cubic.rs        # CUBIC (optional)
├── crypto/
│   ├── mod.rs          # Crypto context per encryption level
│   ├── keys.rs         # Key derivation, rotation (1-RTT key update)
│   ├── protection.rs   # Header & packet protection (AES-based)
│   └── retry.rs        # Retry token + integrity tag
├── tls/
│   ├── mod.rs          # rustls integration
│   └── session.rs      # QUIC-specific TLS session handling
├── transport/
│   ├── params.rs       # Transport parameter encoding/decoding
│   └── error.rs        # Transport error codes
├── socket.rs           # UDP socket abstraction (tokio::net::UdpSocket)
├── endpoint.rs         # Endpoint: manages multiple connections, dispatches packets
├── config.rs           # Configuration
└── lib.rs
```

### Connection Lifecycle

1. `Endpoint` binds a UDP socket and listens for/initiates connections
2. Incoming packets dispatched to `Connection` by connection ID
3. `Connection` state machine: `Initial -> Handshake -> Established -> Closing -> Draining -> Closed`
4. TLS handshake via rustls, deriving keys at each encryption level
5. Once established, streams can be opened/accepted (bidi and uni)
6. Loss detection runs per-packet-number-space, driving retransmissions
7. Congestion control gates sending

### Public API

```rust
// Endpoint — entry point
let endpoint = Endpoint::bind("0.0.0.0:4433", config).await?;

// Server: accept connections
let conn = endpoint.accept().await?;

// Client: connect
let conn = endpoint.connect("example.com:443").await?;

// Open/accept streams
let (send, recv) = conn.open_bidi_stream().await?;
let send = conn.open_uni_stream().await?;
let (send, recv) = conn.accept_bidi_stream().await?;

// Stream I/O — integrates with tokio::io
send.write_all(b"hello").await?;
let mut buf = [0u8; 1024];
let n = recv.read(&mut buf).await?;

// Connection management
conn.close(0u32.into(), b"done");
conn.stats() // RTT, loss rate, bytes sent/received, etc.
```

### Key Decisions

- Streams implement `tokio::io::AsyncRead` / `AsyncWrite`
- Connection is `Send + Sync` — shared via `Arc`
- Congestion control is a trait (`CongestionController`) with NewReno default, pluggable
- 0-RTT supported but opt-in via config
- Connection migration supported with automatic path validation

---

## Crate: `nhttp3-qpack`

QPACK header compression implementing RFC 9204.

### Module Structure

```
nhttp3-qpack/src/
├── encoder.rs      # Encodes header fields into QPACK instructions
├── decoder.rs      # Decodes QPACK instructions into header fields
├── table/
│   ├── static_.rs  # Static table (predefined entries from RFC)
│   ├── dynamic.rs  # Dynamic table with absolute/relative indexing
│   └── field.rs    # Header field type (name, value, sensitive flag)
├── instruction/
│   ├── encoder.rs  # Encoder stream instructions
│   ├── decoder.rs  # Decoder stream instructions
│   └── parse.rs    # Instruction wire format parsing
├── stream.rs       # Encoder/decoder unidirectional stream integration
├── config.rs       # Max table size, max blocked streams
└── lib.rs
```

### Public API

```rust
let mut encoder = qpack::Encoder::new(config);
let mut decoder = qpack::Decoder::new(config);

// Encoding
let encoded = encoder.encode_header_block(stream_id, &headers)?;
let encoder_instructions = encoder.pending_instructions();

// Decoding
decoder.feed_encoder_instructions(bytes)?;
let headers = decoder.decode_header_block(stream_id, encoded)?;
let decoder_instructions = decoder.pending_instructions();
```

### Key Decisions

- Conservative blocked-stream strategy: encoder won't reference unacknowledged dynamic entries (avoidable HOL blocking). Relaxable via config.
- Encoder heuristic: static match > dynamic match > literal with name ref > literal. No complex cost model in v1.
- Dynamic table eviction: oldest entries first per RFC.

---

## Crate: `nhttp3-h3`

HTTP/3 protocol implementing RFC 9114.

### Module Structure

```
nhttp3-h3/src/
├── connection.rs    # HTTP/3 connection management
├── client.rs        # Client-side API
├── server.rs        # Server-side API
├── stream/
│   ├── request.rs   # Request stream handling (bidi)
│   ├── push.rs      # Server push streams (deferred)
│   └── control.rs   # Control stream: SETTINGS, GOAWAY
├── frame/
│   ├── mod.rs       # HTTP/3 frame types
│   ├── parse.rs     # Frame decoding
│   └── write.rs     # Frame encoding
├── headers.rs       # Pseudo-header validation
├── error.rs         # HTTP/3 error codes
├── config.rs        # HTTP/3 settings
└── lib.rs
```

### Public API

```rust
// --- Server ---
let quic_conn = quic_endpoint.accept().await?;
let mut h3_conn = h3::server::Connection::new(quic_conn, h3_config);

loop {
    let (request, stream) = h3_conn.accept().await?;
    tokio::spawn(async move {
        let body = stream.read_body().await?;
        let response = Response::builder()
            .status(200)
            .header("content-type", "text/plain")
            .body(())?;
        stream.send_response(response).await?;
        stream.send_body(b"hello world").await?;
        stream.finish().await?;
    });
}

// --- Client ---
let quic_conn = quic_endpoint.connect("example.com:443").await?;
let mut h3_conn = h3::client::Connection::new(quic_conn, h3_config);

let request = Request::builder()
    .method("GET")
    .uri("https://example.com/")
    .body(())?;

let mut stream = h3_conn.send_request(request).await?;
let response = stream.recv_response().await?;
let body = stream.read_body().await?;
```

### Key Decisions

- Uses `http` crate types (`Request`, `Response`, `HeaderMap`, `Method`, `Uri`, `StatusCode`)
- Body streaming via `AsyncRead`/`AsyncWrite`-like patterns — no full buffering required
- Server push deferred to later phase
- GOAWAY handled gracefully — in-flight requests complete, new requests rejected
- Unknown frame types ignored per spec
- Trailers supported as final HEADERS frame after DATA

---

## Umbrella Crate: `nhttp3`

Thin re-export:

```rust
pub use nhttp3_core as core;
pub use nhttp3_quic as quic;
pub use nhttp3_qpack as qpack;
pub use nhttp3_h3 as h3;
```

---

## FFI Layer: `nhttp3-ffi`

C-compatible ABI for cross-language bindings.

```
nhttp3-ffi/src/
├── endpoint.rs    # nhttp3_endpoint_new, nhttp3_endpoint_accept, etc.
├── connection.rs  # nhttp3_conn_open_stream, nhttp3_conn_close, etc.
├── stream.rs      # nhttp3_stream_read, nhttp3_stream_write, etc.
├── config.rs      # nhttp3_config_new, nhttp3_config_set_*, etc.
├── error.rs       # Error code mapping
├── types.rs       # Opaque handle types
└── lib.rs
```

- Opaque pointer handles — no exposed struct layouts
- All functions return error codes; out-parameters for results
- Tokio runtime managed internally — FFI consumers don't deal with async
- Callback-based for async events

---

## Python Bindings: `nhttp3-python`

```
nhttp3-python/
├── src/
│   ├── endpoint.rs
│   ├── connection.rs
│   ├── stream.rs
│   ├── config.rs
│   └── lib.rs
├── Cargo.toml
├── pyproject.toml    # maturin build config
└── python/
    └── nhttp3/
        ├── __init__.py
        ├── __init__.pyi  # Type stubs
        └── py.typed
```

### Custom Async Bridge (no pyo3-asyncio dependency)

The async bridge between tokio and Python's asyncio is built from scratch:

- Rust FFI functions return opaque "future handles" representing pending async operations
- Python wraps these into native awaitables
- A background thread runs the tokio runtime
- Completions posted to Python event loop via `loop.call_soon_threadsafe()`

```python
class _RustFuture:
    def __init__(self, handle):
        self._handle = handle
        self._loop = asyncio.get_running_loop()
        self._fut = self._loop.create_future()
        _nhttp3_ffi.register_waker(handle, self._on_complete)

    def _on_complete(self, result, error):
        if error:
            self._fut.set_exception(Nhttp3Error(error))
        else:
            self._fut.set_result(result)

    def __await__(self):
        return self._fut.__await__()
```

### Python API

```python
import nhttp3
import asyncio

async def main():
    config = nhttp3.Config()

    # Client
    endpoint = await nhttp3.Endpoint.bind("0.0.0.0:0", config)
    conn = await endpoint.connect("example.com", 443)
    stream = await conn.send_request(
        method="GET",
        uri="https://example.com/",
        headers={"accept": "text/html"},
    )
    response = await stream.recv_response()
    body = await stream.read_body()

    # Server
    server = await nhttp3.Endpoint.bind("0.0.0.0:4433", config)
    conn = await server.accept()
    request, stream = await conn.accept_request()
    await stream.send_response(200, headers={"content-type": "text/plain"})
    await stream.send_body(b"hello")
    await stream.finish()

asyncio.run(main())
```

### Cleanup

- Primary mechanism: context managers (`async with`) for `Endpoint`, `Connection`, `Stream`
- Fallback: explicit `.close()` methods
- Last resort: destructor sends non-blocking close signal to Rust side

```python
async with nhttp3.Endpoint.bind("0.0.0.0:4433", config) as endpoint:
    async with await endpoint.accept() as conn:
        request, stream = await conn.accept_request()
        # ...
# cleaned up here
```

---

## WASM Target: `nhttp3-wasm`

```
nhttp3-wasm/
├── src/
│   └── lib.rs
├── Cargo.toml
└── pkg/           # wasm-pack output
```

- Built with `wasm-bindgen` + `wasm-pack`
- **Browser**: Uses WebTransport API as underlying transport, nhttp3 handles HTTP/3 framing on top. Browsers don't expose raw UDP sockets.
- **Non-browser WASM** (Cloudflare Workers, Deno): Full stack runs where UDP socket access is available.
- Uses `wasm-bindgen-futures` instead of tokio (tokio doesn't work in WASM)
- JS API returns Promises

```javascript
import { Endpoint, Config } from 'nhttp3';

const config = new Config();
const endpoint = await Endpoint.bind("0.0.0.0:4433", config);
const conn = await endpoint.connect("example.com", 443);
```

---

## Error Handling & Safety

### Error Types

Each crate defines its own error enum with `From` conversions for `?` propagation:

```rust
// nhttp3-core
pub enum Error {
    BufferTooShort,
    InvalidVarInt,
}

// nhttp3-quic
pub enum Error {
    Core(core::Error),
    Tls(rustls::Error),
    TransportError(TransportErrorCode),
    ConnectionClosed,
    StreamReset(u64),
    InvalidState,
    TimedOut,
    Io(std::io::Error),
}

// nhttp3-h3
pub enum Error {
    Quic(quic::Error),
    Qpack(qpack::Error),
    FrameError,
    MalformedHeaders,
    SettingsError,
    ClosedCriticalStream,
    IdError,
}
```

- All errors implement `std::error::Error` and `Display`
- Transport error codes map 1:1 to RFC 9000 Section 20
- HTTP/3 error codes map 1:1 to RFC 9114 Section 8

### Safety

- No `unsafe` in protocol logic — only in FFI boundary (`nhttp3-ffi`)
- FFI `unsafe` is minimal: opaque handle pointer dereferences, callback invocations
- Public APIs validate inputs at boundary; internal code trusts invariants
- Crypto delegated entirely to rustls — no hand-rolled crypto
- Packet parsing is bounds-checked; malformed input returns errors, never panics

### Cancellation & Cleanup

- QUIC connections send `CONNECTION_CLOSE` on drop
- Streams send `RESET_STREAM` / `STOP_SENDING` on drop
- Tokio `CancellationToken` for graceful endpoint shutdown
- Python: context managers (`async with`) as primary cleanup, explicit `.close()` as fallback

---

## Testing Strategy

### Per-Crate Unit Tests

**`nhttp3-core`**:
- Varint encoding/decoding roundtrips (edge cases: 0, max values per 1/2/4/8 byte)
- Buffer management correctness
- Error type conversions

**`nhttp3-quic`**:
- Packet parse/serialize roundtrips for all packet types, header protection/unprotection, packet number encoding
- Frame parse/serialize roundtrips for every frame type, malformed frame rejection
- Connection state machine: valid transitions, invalid transition rejection, timeout handling
- Full TLS handshake with rustls (in-process, no real network), 0-RTT resumption
- Stream open/close lifecycle, flow control enforcement, concurrency limits, bidi and uni
- Loss detection (PTO, time-based), ACK generation, congestion controller behavior
- Key derivation against RFC 9001 Appendix A test vectors, key rotation
- Transport param encode/decode roundtrips, unknown param tolerance
- Connection migration: path validation, preferred address

**`nhttp3-qpack`**:
- Static table lookups (exact match, name match)
- Dynamic table insert/evict/lookup
- Encoder/decoder roundtrips with various header sets
- Blocked stream handling
- RFC 9204 Appendix B test vectors
- Edge cases: empty header values, large header fields, table overflow

**`nhttp3-h3`**:
- Frame parse/serialize roundtrips for DATA, HEADERS, SETTINGS, GOAWAY
- Control stream setup and SETTINGS exchange
- Request/response lifecycle (headers -> data -> trailers -> fin)
- Pseudo-header validation
- Error handling: malformed frames, wrong stream types, protocol violations
- GOAWAY graceful shutdown flow
- Unknown frame type tolerance

**`nhttp3-ffi`**:
- C ABI smoke tests: create/destroy handles, verify no leaks
- Error code propagation
- Callback invocation correctness
- Thread safety under concurrent calls

**`nhttp3-python`**:
- pytest suite: end-to-end client and server flows
- Custom async bridge: future completion, cancellation, error propagation
- `call_soon_threadsafe` correctness under load
- Type stub accuracy (mypy validation)

**`nhttp3-wasm`**:
- `wasm-pack test` with headless browser
- Basic client flow via WebTransport mock
- Promise resolution/rejection

### Integration Tests (`tests/`)

Full protocol exchanges over localhost UDP:

- Handshake: client connects to server, full QUIC handshake completes
- Request/response: client GET, server responds — verify end-to-end
- Streaming: large body transfer, verify flow control and ordering
- Multiplexing: multiple concurrent streams over one connection
- Connection close: graceful GOAWAY, immediate close, idle timeout
- Error paths: invalid frames, version mismatch, TLS failure
- 0-RTT: session resumption with early data
- Key update: 1-RTT key rotation mid-connection
- Interop markers: log format compatible with QUIC interop runner

### Fuzz Testing (`fuzz/`)

Using `cargo-fuzz` / `libfuzzer`:

- Packet parsing (all packet types)
- Frame parsing (QUIC + HTTP/3)
- QPACK decoding (encoder instructions + header blocks)
- Variable-length integer decoding
- Transport parameter decoding

### Benchmarks (`benches/`)

Using `criterion`:

- Packet parse/serialize throughput
- QPACK encode/decode throughput
- Connection handshake latency
- Stream throughput (single stream, saturated)
- Multiplexed stream throughput
- Regression tracking over time

### CI

- `cargo test --workspace` on every commit
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --check`
- Fuzz targets on nightly schedule
- MSRV check
- Cross-platform: Linux, macOS, Windows
- Python: `maturin develop` + `pytest`
- WASM: `wasm-pack test --headless --chrome`

---

## Implementation Phases

### Phase 1 — Foundation (core + QUIC transport)

1. `nhttp3-core`: varint, buffers, shared types
2. `nhttp3-quic` packet parsing/serialization
3. `nhttp3-quic` frame parsing/serialization
4. `nhttp3-quic` crypto/TLS integration with rustls
5. `nhttp3-quic` connection state machine + handshake
6. `nhttp3-quic` streams + flow control
7. `nhttp3-quic` loss detection + congestion control (NewReno)
8. `nhttp3-quic` endpoint (UDP socket, connection dispatch)
9. Integration test: full QUIC handshake + stream I/O over localhost

### Phase 2 — HTTP/3

1. `nhttp3-qpack`: static table, dynamic table, encoder, decoder
2. `nhttp3-h3` frame parsing/serialization
3. `nhttp3-h3` connection: control stream, SETTINGS, QPACK stream management
4. `nhttp3-h3` server API
5. `nhttp3-h3` client API
6. `nhttp3` umbrella crate
7. Integration test: full HTTP/3 request/response over localhost

### Phase 3 — Hardening

1. Fuzz targets for all parsing layers
2. Benchmarks
3. 0-RTT support
4. Key update (1-RTT rotation)
5. Connection migration
6. CUBIC congestion control
7. GOAWAY graceful shutdown
8. Interop testing against other implementations

### Phase 4 — Python Bindings

1. `nhttp3-ffi`: C ABI layer
2. Custom async bridge (tokio <-> asyncio)
3. `nhttp3-python`: PyO3 bindings with maturin
4. Python client + server API
5. pytest suite
6. Type stubs + mypy validation

### Phase 5 — WASM

1. `nhttp3-wasm`: wasm-bindgen target
2. WebTransport adapter for browser
3. Full-stack adapter for non-browser runtimes
4. JS API + npm packaging
5. wasm-pack tests

### Phase 6 — Extensions (iterative)

- QUIC datagrams (RFC 9221)
- WebTransport
- Priority signaling (RFC 9218)
- Additional congestion controllers
- Further RFCs as they mature
