# curl Guide

Test any nhttp3 server from the command line. Requires curl 7.88+ with HTTP/3 support.

## Check support

```bash
curl --version | grep -i "http3\|quic"
```

If missing: `brew install curl` (macOS) or `sudo apt install curl` (Ubuntu 24+).

## Basic requests

```bash
# GET
curl --http3 https://localhost:4433/ -k

# POST with JSON
curl --http3 https://localhost:4433/echo \
  -X POST -H "Content-Type: application/json" \
  -d '{"message": "hello"}' -k

# Health check
curl --http3 https://localhost:4433/health -k
```

`-k` skips certificate verification (needed for self-signed certs).

## Streaming

```bash
# Stream SSE (server-sent events)
curl --http3 https://localhost:4433/stream -k --no-buffer

# Stream LLM tokens (OpenAI-compatible)
curl --http3 https://localhost:4433/v1/chat/completions \
  -X POST -H "Content-Type: application/json" \
  -d '{"model":"llama3","messages":[{"role":"user","content":"Hello!"}],"stream":true}' \
  -k --no-buffer
```

## Parallel multiplexing

```bash
curl --http3 --parallel --parallel-max 10 \
  https://localhost:4433/api/1 \
  https://localhost:4433/api/2 \
  https://localhost:4433/api/3 -k
```

Each request uses its own QUIC stream — no head-of-line blocking.

## Timing

```bash
curl --http3 https://localhost:4433/ -k -o /dev/null -s \
  -w "Connect: %{time_connect}s\nTLS: %{time_appconnect}s\nTotal: %{time_total}s\n"
```

## Verbose (see QUIC details)

```bash
curl --http3 https://localhost:4433/ -k -v 2>&1 | grep -i "quic\|h3\|alt-svc"
```

## Or use nhttp3-client instead

If curl doesn't have HTTP/3, use the built-in client:

```bash
cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/
cargo run -p nhttp3-server --bin nhttp3-client -- -v https://localhost:4433/health
```
