# Python Server Guide

Serve any ASGI app (FastAPI, Starlette, Django) over native HTTP/3. No uvicorn. No proxy.

## Install

```bash
cd nhttp3-python
python3 -m venv .venv && source .venv/bin/activate
pip install maturin fastapi
maturin develop
```

## Serve a FastAPI app

```python
from fastapi import FastAPI
import nhttp3

app = FastAPI()

@app.get("/")
async def root():
    return {"hello": "world"}

@app.get("/health")
async def health():
    return {"status": "ok"}

@app.post("/echo")
async def echo(request):
    body = await request.body()
    return {"echo": body.decode(), "size": len(body)}

nhttp3.serve(app, port=4433)
```

## Test

```bash
# From another terminal
cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/
# → {"hello":"world"}

cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/health
# → {"status":"ok"}

cargo run -p nhttp3-server --bin nhttp3-client -- -X POST -d '{"test":true}' https://localhost:4433/echo
# → {"echo":"{\"test\":true}","size":13}
```

## How it works

```
HTTP/3 request → quinn (QUIC) → h3 (HTTP/3) → PyO3 → ASGI app → PyO3 → h3 → quinn → response
```

No uvicorn. No proxy. The Rust QUIC server calls your Python app directly via PyO3.

## Config

```python
config = nhttp3.Config()
config.max_idle_timeout = 60.0          # seconds
config.initial_max_streams_bidi = 200   # concurrent request streams
config.initial_max_data = 50_000_000    # 50MB flow control window
```

## Limitations

- Self-signed cert auto-generated (production: load from file)
- Streaming responses (SSE) not yet wired through ASGI send()
- WebSocket upgrade not implemented
