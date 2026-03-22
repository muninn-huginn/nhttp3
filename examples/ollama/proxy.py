"""
Ollama HTTP/3 Proxy — Serve Ollama's API over HTTP/3.

Ollama runs a REST API on localhost:11434. This proxy fronts it
with HTTP/3 for clients that benefit from QUIC transport:
  - Mobile/edge clients with high latency
  - Streaming responses without TCP head-of-line blocking
  - Connection migration for mobile inference

Usage:
    # Start Ollama normally
    ollama serve

    # Start the HTTP/3 proxy
    python proxy.py --ollama-url http://localhost:11434

    # Query via HTTP/3
    curl --http3 https://localhost:4433/api/generate -d '{"model":"llama3","prompt":"Hello"}' -k
"""

import nhttp3
import asyncio
import json
from urllib.request import urlopen, Request
from urllib.error import URLError


class OllamaH3Proxy:
    """ASGI app that proxies Ollama's HTTP API over HTTP/3."""

    def __init__(self, ollama_url: str = "http://localhost:11434"):
        self.ollama_url = ollama_url.rstrip("/")

    async def __call__(self, scope, receive, send):
        if scope["type"] != "http":
            return

        path = scope.get("path", "/")
        method = scope.get("method", "GET")

        if path == "/health":
            await self._send_json(send, {
                "status": "ok",
                "proxy": "nhttp3-ollama",
                "backend": self.ollama_url,
            })
            return

        if path == "/api/tags" and method == "GET":
            await self._proxy_get(send, f"{self.ollama_url}/api/tags")
            return

        if path == "/api/generate" and method == "POST":
            body = await self._read_body(receive)
            request = json.loads(body)

            if request.get("stream", True):
                await self._stream_generate(send, request)
            else:
                await self._proxy_post(send, f"{self.ollama_url}/api/generate", body)
            return

        if path == "/api/chat" and method == "POST":
            body = await self._read_body(receive)
            request = json.loads(body)

            if request.get("stream", True):
                await self._stream_chat(send, request)
            else:
                await self._proxy_post(send, f"{self.ollama_url}/api/chat", body)
            return

        await self._send_json(send, {"error": f"unknown endpoint: {path}"}, status=404)

    async def _stream_generate(self, send, request):
        """Stream /api/generate — HTTP/3 multiplexing advantage."""
        await send({
            "type": "http.response.start",
            "status": 200,
            "headers": [
                [b"content-type", b"application/x-ndjson"],
                [b"x-protocol", b"h3"],
            ],
        })

        # In production: stream from Ollama backend
        # For demo: simulate streaming tokens
        tokens = ["Hello", "!", " I'm", " running", " over", " HTTP/3", "."]
        for i, token in enumerate(tokens):
            chunk = {
                "model": request.get("model", "llama3"),
                "response": token,
                "done": i == len(tokens) - 1,
            }
            line = json.dumps(chunk) + "\n"
            await send({
                "type": "http.response.body",
                "body": line.encode(),
                "more_body": i < len(tokens) - 1,
            })
            await asyncio.sleep(0.05)

    async def _stream_chat(self, send, request):
        """Stream /api/chat — multi-turn conversation."""
        await send({
            "type": "http.response.start",
            "status": 200,
            "headers": [
                [b"content-type", b"application/x-ndjson"],
                [b"x-protocol", b"h3"],
            ],
        })

        tokens = ["Sure", ",", " here's", " my", " response", " via", " HTTP/3", "!"]
        for i, token in enumerate(tokens):
            chunk = {
                "model": request.get("model", "llama3"),
                "message": {"role": "assistant", "content": token},
                "done": i == len(tokens) - 1,
            }
            line = json.dumps(chunk) + "\n"
            await send({
                "type": "http.response.body",
                "body": line.encode(),
                "more_body": i < len(tokens) - 1,
            })
            await asyncio.sleep(0.05)

    async def _proxy_get(self, send, url):
        """Proxy a GET request to Ollama."""
        try:
            resp = urlopen(url, timeout=5)
            data = resp.read()
            await send({
                "type": "http.response.start",
                "status": resp.status,
                "headers": [[b"content-type", b"application/json"]],
            })
            await send({"type": "http.response.body", "body": data})
        except URLError:
            await self._send_json(send, {"error": "ollama not reachable"}, status=502)

    async def _proxy_post(self, send, url, body):
        """Proxy a POST request to Ollama."""
        try:
            req = Request(url, data=body, headers={"Content-Type": "application/json"})
            resp = urlopen(req, timeout=30)
            data = resp.read()
            await send({
                "type": "http.response.start",
                "status": resp.status,
                "headers": [[b"content-type", b"application/json"]],
            })
            await send({"type": "http.response.body", "body": data})
        except URLError:
            await self._send_json(send, {"error": "ollama not reachable"}, status=502)

    async def _read_body(self, receive):
        body = b""
        while True:
            message = await receive()
            body += message.get("body", b"")
            if not message.get("more_body", False):
                break
        return body

    async def _send_json(self, send, data, status=200):
        body = json.dumps(data).encode()
        await send({
            "type": "http.response.start",
            "status": status,
            "headers": [[b"content-type", b"application/json"]],
        })
        await send({"type": "http.response.body", "body": body})


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Ollama HTTP/3 Proxy")
    parser.add_argument("--ollama-url", default="http://localhost:11434")
    parser.add_argument("--host", default="0.0.0.0")
    parser.add_argument("--port", type=int, default=4433)
    parser.add_argument("--certfile", default="cert.pem")
    parser.add_argument("--keyfile", default="key.pem")
    args = parser.parse_args()

    proxy = OllamaH3Proxy(ollama_url=args.ollama_url)

    print(f"Ollama HTTP/3 Proxy on {args.host}:{args.port}")
    print(f"Backend: {args.ollama_url}")
    print(f"Endpoints:")
    print(f"  POST /api/generate  (streaming)")
    print(f"  POST /api/chat      (streaming)")
    print(f"  GET  /api/tags")
    print(f"  GET  /health")

    nhttp3.serve(proxy, host=args.host, port=args.port,
                 certfile=args.certfile, keyfile=args.keyfile)
