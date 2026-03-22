# FastAPI + nhttp3

Expose FastAPI over HTTP/3 using nhttp3 as a reverse proxy.

## How it works

```
Browser/curl ──HTTP/3──▶ nhttp3-server ──HTTP/1.1──▶ uvicorn (FastAPI)
  (QUIC, 1-RTT)           (port 4433)                 (port 8000)
```

nhttp3-server handles the HTTP/3 frontend. uvicorn handles the app logic.
This gives external clients the benefits of QUIC (1-RTT, no HOL blocking)
while keeping your FastAPI code completely unchanged.

## When this helps

- Clients on **high-latency networks** (mobile, cross-continent): saves 2.5 RTTs per connection
- Clients on **lossy networks**: no head-of-line blocking
- Clients that **change networks** (WiFi → cellular): connection migration

## When this doesn't help

- Localhost-to-localhost: TCP is faster (no TLS overhead to amortize)
- Run `nhttp3-benchmark` to see the numbers

## Setup

```bash
# Terminal 1: Start FastAPI
pip install fastapi uvicorn
uvicorn app:app --port 8000

# Terminal 2: Start HTTP/3 proxy
cargo run -p nhttp3-server -- --proxy http://localhost:8000

# Terminal 3: Test
cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/
cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/health
cargo run -p nhttp3-server --bin nhttp3-client -- -X POST -d '{"msg":"hi"}' https://localhost:4433/echo
```
