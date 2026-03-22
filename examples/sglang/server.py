"""
SGLang LLM serving over HTTP/3.

SGLang is a fast serving framework for large language models.
HTTP/3 benefits for LLM serving:
  - Streaming responses without head-of-line blocking
  - Faster connection setup for high-throughput inference
  - Better performance on high-latency networks (edge inference)

Before:
    python -m sglang.launch_server --model meta-llama/Llama-3-8B --port 8000

After:
    python sglang_h3.py --model meta-llama/Llama-3-8B --port 4433
"""

import nhttp3
import asyncio
import json
from typing import AsyncGenerator


# Mock SGLang-compatible ASGI app for demonstration
# In production, this wraps the real SGLang server
class SGLangH3App:
    """ASGI wrapper for SGLang that serves over HTTP/3."""

    def __init__(self, model_path: str = "mock-model"):
        self.model_path = model_path
        self.request_count = 0

    async def __call__(self, scope, receive, send):
        if scope["type"] != "http":
            return

        path = scope.get("path", "/")
        method = scope.get("method", "GET")

        if path == "/health":
            await self._send_json(send, {"status": "ok", "model": self.model_path})

        elif path == "/v1/chat/completions" and method == "POST":
            body = await self._read_body(receive)
            request = json.loads(body)
            stream = request.get("stream", False)

            if stream:
                await self._stream_completion(send, request)
            else:
                await self._completion(send, request)

        elif path == "/v1/completions" and method == "POST":
            body = await self._read_body(receive)
            request = json.loads(body)
            await self._completion(send, request)

        elif path == "/generate" and method == "POST":
            # SGLang native endpoint
            body = await self._read_body(receive)
            request = json.loads(body)
            await self._sglang_generate(send, request)

        else:
            await self._send_json(send, {"error": "not found"}, status=404)

    async def _completion(self, send, request):
        """OpenAI-compatible completion endpoint."""
        self.request_count += 1
        response = {
            "id": f"chatcmpl-{self.request_count}",
            "object": "chat.completion",
            "model": self.model_path,
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "This is a response served over HTTP/3 via nhttp3!",
                    },
                    "finish_reason": "stop",
                }
            ],
            "usage": {"prompt_tokens": 10, "completion_tokens": 15, "total_tokens": 25},
        }
        await self._send_json(send, response)

    async def _stream_completion(self, send, request):
        """Streaming completion — HTTP/3 advantage: no HOL blocking."""
        self.request_count += 1

        await send({
            "type": "http.response.start",
            "status": 200,
            "headers": [
                [b"content-type", b"text/event-stream"],
                [b"x-protocol", b"h3"],
            ],
        })

        tokens = ["This", " is", " a", " streaming", " response", " over", " HTTP/3", "!"]
        for i, token in enumerate(tokens):
            chunk = {
                "id": f"chatcmpl-{self.request_count}",
                "object": "chat.completion.chunk",
                "choices": [
                    {
                        "index": 0,
                        "delta": {"content": token},
                        "finish_reason": None if i < len(tokens) - 1 else "stop",
                    }
                ],
            }
            data = f"data: {json.dumps(chunk)}\n\n"
            await send({
                "type": "http.response.body",
                "body": data.encode(),
                "more_body": i < len(tokens) - 1,
            })
            await asyncio.sleep(0.05)  # Simulate token generation

    async def _sglang_generate(self, send, request):
        """SGLang native /generate endpoint."""
        self.request_count += 1
        response = {
            "text": "Generated text over HTTP/3",
            "meta_info": {
                "prompt_tokens": 5,
                "completion_tokens": 6,
                "protocol": "h3",
            },
        }
        await self._send_json(send, response)

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
            "headers": [
                [b"content-type", b"application/json"],
                [b"x-protocol", b"h3"],
            ],
        })
        await send({
            "type": "http.response.body",
            "body": body,
        })


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="SGLang over HTTP/3")
    parser.add_argument("--model", default="mock-model", help="Model path")
    parser.add_argument("--host", default="0.0.0.0")
    parser.add_argument("--port", type=int, default=4433)
    parser.add_argument("--certfile", default="cert.pem")
    parser.add_argument("--keyfile", default="key.pem")
    args = parser.parse_args()

    app = SGLangH3App(model_path=args.model)

    print(f"SGLang HTTP/3 server starting on {args.host}:{args.port}")
    print(f"Model: {args.model}")
    print(f"Endpoints:")
    print(f"  POST /v1/chat/completions  (OpenAI-compatible)")
    print(f"  POST /v1/completions       (OpenAI-compatible)")
    print(f"  POST /generate             (SGLang native)")
    print(f"  GET  /health")

    nhttp3.serve(
        app,
        host=args.host,
        port=args.port,
        certfile=args.certfile,
        keyfile=args.keyfile,
    )
