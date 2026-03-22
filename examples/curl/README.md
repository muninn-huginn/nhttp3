# Testing nhttp3 with curl

curl supports HTTP/3 via the `--http3` flag (requires curl 7.88+ built with HTTP/3 support).

## Check curl HTTP/3 support

```bash
curl --version | grep HTTP3
# Should show: HTTP3

# Or check features:
curl --version | grep -i quic
```

## Install curl with HTTP/3

```bash
# macOS (Homebrew)
brew install curl

# Ubuntu/Debian (curl 8.x has HTTP/3 built-in)
sudo apt install curl

# Or build from source with quiche/ngtcp2
```

## Basic requests

```bash
# GET request
curl --http3 https://localhost:4433/ -k -v

# POST with JSON body
curl --http3 https://localhost:4433/api/echo \
  -X POST \
  -H "Content-Type: application/json" \
  -d '{"message": "hello"}' \
  -k

# Health check
curl --http3 https://localhost:4433/health -k

# With timing info
curl --http3 https://localhost:4433/ -k -w "\n\nDNS: %{time_namelookup}s\nConnect: %{time_connect}s\nTLS: %{time_appconnect}s\nTotal: %{time_total}s\n"
```

## Streaming responses (LLM)

```bash
# Stream chat completions (SGLang/OpenAI compatible)
curl --http3 https://localhost:4433/v1/chat/completions \
  -X POST \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }' \
  -k --no-buffer

# Stream from Ollama proxy
curl --http3 https://localhost:4433/api/generate \
  -d '{"model": "llama3", "prompt": "Why is HTTP/3 faster?"}' \
  -k --no-buffer
```

## Compare HTTP/2 vs HTTP/3

```bash
# HTTP/2 (TCP + TLS = 2 RTT)
time curl --http2 https://localhost:8443/ -k -o /dev/null -s

# HTTP/3 (QUIC + TLS = 1 RTT)
time curl --http3 https://localhost:4433/ -k -o /dev/null -s
```

## Parallel requests (multiplexing)

```bash
# HTTP/3 multiplexes requests on independent QUIC streams
# No head-of-line blocking — one slow response doesn't delay others
curl --http3 --parallel --parallel-max 10 \
  https://localhost:4433/api/item/1 \
  https://localhost:4433/api/item/2 \
  https://localhost:4433/api/item/3 \
  https://localhost:4433/api/item/4 \
  https://localhost:4433/api/item/5 \
  -k
```

## Verbose output (see QUIC details)

```bash
curl --http3 https://localhost:4433/ -k -v 2>&1 | grep -i "quic\|h3\|alt-svc"
```

## Alt-Svc header

HTTP/3 is discovered via the `Alt-Svc` header. When a server supports both
HTTP/2 and HTTP/3, it sends:

```
Alt-Svc: h3=":4433"; ma=86400
```

This tells the client: "I also speak HTTP/3 on port 4433, and this info
is valid for 24 hours." curl will then upgrade to HTTP/3 on subsequent
requests.

## Testing from Python

```bash
# Using httpx with HTTP/3 support
pip install httpx[http2] httpx-h3

python -c "
import httpx
client = httpx.Client(http2=True, verify=False)
resp = client.get('https://localhost:4433/')
print(resp.http_version)  # HTTP/3 if supported
print(resp.json())
"
```
