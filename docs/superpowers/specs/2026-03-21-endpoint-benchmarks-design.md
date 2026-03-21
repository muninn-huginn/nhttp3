# Endpoint I/O Loop + Benchmarks — Design Spec

## Overview

Wire up the QUIC Endpoint that manages multiple connections over a single UDP socket, providing a production-ready `connect()` / `accept()` API. Add comprehensive benchmarks covering codec micro-performance, connection-level throughput, and HTTP/3 vs HTTP/2 comparison.

## Endpoint Architecture

### Core Types

- **`Endpoint`** — Owns the UDP socket, manages all connections. Dual-role (client + server). User-facing handle.
- **`Connection`** — User-facing handle (`Arc<Mutex<ConnectionInner>>`). `Clone + Send + Sync`.
- **`ConnectionInner`** — Mutable state: TLS session, stream manager, flow control, recovery, packet assembly.
- **`SendStream`** / **`RecvStream`** — Per-stream handles implementing `tokio::io::AsyncWrite` / `AsyncRead`.
- **`StreamManager`** — Manages stream lifecycle, ID allocation, concurrency limits.
- **`CidMap`** — `HashMap<ConnectionId, Arc<Mutex<ConnectionInner>>>` for packet dispatch.

### Internal Flow

```
Endpoint::bind() spawns background I/O loop task

I/O Loop:
  loop {
      select! {
          (data, addr) = socket.recv_from() => dispatch_packet()
          _ = sleep_until(next_timeout) => handle_timeouts()
      }
      // Poll dirty connections for outgoing packets
      for conn in dirty_connections {
          while let Some(pkt) = conn.poll_transmit() {
              socket.send_to(pkt.data, pkt.addr)
          }
      }
  }
```

### Packet Receive Path

1. `socket.recv_from()` → raw bytes + source address
2. Peek first byte → long or short header
3. Extract destination CID
4. Look up `ConnectionInner` in CID map
5. If not found + Initial packet → create new server connection, send to `accept_tx`
6. If found → `conn.on_packet_received(data, addr)`
7. Inside connection: remove header protection → decode PN → decrypt payload → parse frames
8. Dispatch frames: CRYPTO → TLS, STREAM → receive buffer, ACK → recovery, flow control frames → update windows

### Packet Send Path

1. User calls `send_stream.write(data)` → queues data in per-stream send buffer
2. I/O loop polls each dirty connection: `conn.poll_transmit()`
3. Connection assembles packet: pick frames (STREAM, ACK, flow control), serialize, encrypt, apply header protection
4. I/O loop sends via `socket.send_to()`

### Timer Management

- Each `ConnectionInner` tracks: idle timeout, loss detection timer (PTO), ACK delay timer
- I/O loop computes `next_timeout = min(all connection timeouts)`
- Uses `tokio::select!` with `tokio::time::sleep_until`
- On timeout: `conn.on_timeout()` triggers retransmission or connection close

### Public API

```rust
// Bind endpoint
let endpoint = Endpoint::bind("0.0.0.0:4433", config, tls_config).await?;

// Server: accept connections
let conn = endpoint.accept().await?;

// Client: connect
let conn = endpoint.connect(addr, "example.com").await?;

// Open/accept streams
let (send, recv) = conn.open_bidi_stream().await?;
let send = conn.open_uni_stream().await?;
let (send, recv) = conn.accept_bidi_stream().await?;
let recv = conn.accept_uni_stream().await?;

// Stream I/O (tokio::io traits)
send.write_all(b"hello").await?;
let mut buf = [0u8; 1024];
let n = recv.read(&mut buf).await?;

// Shutdown
send.shutdown().await?;      // FIN
send.reset(0x00)?;           // RESET_STREAM
recv.stop(0x00)?;            // STOP_SENDING

// Connection management
conn.close(0u32, b"done");
let stats = conn.stats();    // RTT, loss, bytes, etc.
endpoint.close().await;
```

### Connection Handle

`Connection` is `Clone + Send + Sync`:
- Multiple tasks can hold the same connection
- Each stream is independently owned
- `SendStream` and `RecvStream` can be moved to different tasks

### Stream API

**`SendStream`** (implements `tokio::io::AsyncWrite`):
- `write()` queues data, respects flow control (blocks if window exhausted)
- `shutdown()` sends FIN
- `reset(error_code)` sends RESET_STREAM
- Drop triggers RESET_STREAM if not finished

**`RecvStream`** (implements `tokio::io::AsyncRead`):
- `read()` pulls from receive buffer, waits via `Notify` if empty
- Detects FIN → returns `Ok(0)` (EOF)
- `stop(error_code)` sends STOP_SENDING

### Flow Control Integration

- Per-stream + connection-level `FlowControl` (existing)
- `SendStream::write()` blocks when window exhausted, resumes on MAX_STREAM_DATA
- Receive buffer drain triggers MAX_STREAM_DATA / MAX_DATA updates to peer

### Concurrency

- `Endpoint` internals: I/O loop runs as a single background tokio task
- `Connection` access: `Arc<Mutex<ConnectionInner>>` — lock contention is minimal since the I/O loop only holds the lock briefly per packet
- Streams: each `SendStream`/`RecvStream` has its own buffer and `Notify`, no global lock for reads/writes

---

## File Structure

### New/Modified in `nhttp3-quic`

```
crates/nhttp3-quic/src/
├── endpoint.rs              # Endpoint: bind, accept, connect, close
├── io_loop.rs               # Background task: recv, dispatch, send, timers
├── connection/
│   ├── mod.rs               # Connection (Arc wrapper) + re-exports
│   ├── inner.rs             # ConnectionInner: packet processing, frame dispatch
│   ├── id_map.rs            # CID → Connection lookup map
│   ├── state.rs             # (existing)
│   └── migration.rs         # (existing)
├── stream/
│   ├── mod.rs               # (existing, updated re-exports)
│   ├── send.rs              # SendStream (AsyncWrite)
│   ├── recv.rs              # RecvStream (AsyncRead)
│   ├── manager.rs           # StreamManager: open/accept/lookup
│   ├── state.rs             # (existing)
│   └── flow_control.rs      # (existing)
```

### Benchmarks

```
benches/
├── codec.rs                 # Micro-benchmarks
└── connection.rs            # Connection + comparison benchmarks
```

---

## Benchmarks

### Micro-benchmarks (`benches/codec.rs`)

Using `criterion`:

| Benchmark | What It Measures |
|-----------|-----------------|
| `varint_encode_1byte` | VarInt encode (small values) |
| `varint_encode_8byte` | VarInt encode (large values) |
| `varint_decode` | VarInt decode (mixed sizes) |
| `packet_header_parse_initial` | Initial packet header parse |
| `packet_header_parse_short` | Short packet header parse |
| `frame_parse_stream` | STREAM frame parse |
| `frame_parse_ack` | ACK frame parse |
| `frame_encode_stream` | STREAM frame serialize |
| `frame_encode_ack` | ACK frame serialize |
| `qpack_encode_request` | QPACK encode typical request (4 headers) |
| `qpack_decode_request` | QPACK decode typical request |
| `qpack_encode_response` | QPACK encode typical response |
| `transport_params_encode` | Transport params encode |
| `transport_params_decode` | Transport params decode |

### Connection benchmarks (`benches/connection.rs`)

| Benchmark | What It Measures |
|-----------|-----------------|
| `handshake_localhost` | Time from connect() to established |
| `stream_throughput_1mb` | Single stream, 1MB transfer |
| `stream_throughput_100mb` | Single stream, 100MB transfer |
| `multiplex_10_streams` | 10 concurrent streams aggregate |
| `stream_open_close_1000` | Open and close 1000 streams |
| `small_message_rtt` | 1-byte write → read roundtrip |

### HTTP/3 vs HTTP/2 comparison (`benches/comparison.rs`)

Using `h2` + `tokio-rustls` as the HTTP/2 baseline:

| Benchmark | nhttp3 | h2 |
|-----------|--------|-----|
| `handshake_latency` | QUIC 1-RTT | TCP+TLS 2-RTT |
| `single_stream_1mb` | QUIC stream | h2 stream |
| `single_stream_100mb` | QUIC stream | h2 stream |
| `multiplex_10_streams` | No HOL blocking | Shared TCP |
| `small_message_rtt` | QUIC roundtrip | h2 roundtrip |
| `header_compression` | QPACK | HPACK |

**Dev-dependencies:** `h2`, `tokio-rustls` for comparison benchmarks.

### Benchmark Infrastructure

- All connection benchmarks use localhost UDP/TCP (`127.0.0.1`)
- Server runs in a background tokio task within the bench
- Handshake benchmarks: fresh connection per iteration
- Throughput benchmarks: reuse connection, fresh stream per iteration
- Criterion's built-in regression detection tracks performance over time

---

## Testing Strategy

### Unit Tests

- `endpoint.rs`: create/drop endpoint, bind to port 0
- `connection/inner.rs`: feed raw packet bytes, verify frame processing, verify outgoing packets
- `connection/id_map.rs`: insert/lookup/remove CIDs, collision handling
- `stream/send.rs`: write data → verify STREAM frames queued, flow control blocking
- `stream/recv.rs`: feed STREAM frames → read data back, EOF on FIN
- `stream/manager.rs`: open/accept bidi/uni, stream ID allocation, concurrency limits
- `io_loop.rs`: mock socket, inject packets, capture sent packets, verify dispatch

### Integration Tests

- Full handshake over localhost UDP (client + server in same process)
- Request/response: client sends, server responds — verify end-to-end
- Large transfer: 1MB stream data, verify ordering and completeness
- Multiplexing: 10 concurrent streams, verify all complete
- Idle timeout: verify connection closes after configured duration
- Graceful close: `conn.close()` sends CONNECTION_CLOSE, peer receives it

### Benchmark Smoke Tests

All benchmarks double as integration smoke tests — if they run without panicking, the code path works.
