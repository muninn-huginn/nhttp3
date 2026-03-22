# FastAPI + nhttp3

Serve any FastAPI app over HTTP/3 with one line change.

## Setup

```bash
pip install nhttp3 fastapi

# Generate self-signed cert for testing
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout key.pem -out cert.pem -days 365 -nodes -subj '/CN=localhost'
```

## Run

```bash
python server.py
```

## Test with curl

```bash
curl --http3 https://localhost:4433/ -k
curl --http3 https://localhost:4433/health -k
curl --http3 https://localhost:4433/echo -X POST -d "hello" -k
curl --http3 https://localhost:4433/stream -k
```

## Migration from uvicorn

```python
# Before (HTTP/1.1 + HTTP/2):
import uvicorn
uvicorn.run(app, host="0.0.0.0", port=8000)

# After (HTTP/3):
import nhttp3
nhttp3.serve(app, host="0.0.0.0", port=4433,
             certfile="cert.pem", keyfile="key.pem")
```

## Why HTTP/3 for APIs?

- **1-RTT handshake** — Faster connection setup than TCP+TLS (2-RTT)
- **No head-of-line blocking** — Multiplexed streams are independent
- **Connection migration** — Mobile clients survive network changes
- **Better on lossy networks** — Single packet loss doesn't stall everything
