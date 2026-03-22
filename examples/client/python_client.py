"""
nhttp3 Python HTTP/3 client examples.

Works with any HTTP/3 server (nhttp3, nginx, cloudflare, etc.)
"""

import nhttp3
import asyncio


async def simple_get():
    """Simple GET request over HTTP/3."""
    config = nhttp3.Config()
    ep = await nhttp3.Endpoint.bind("0.0.0.0", 0, config)
    conn = await ep.connect("localhost", 4433, server_name="localhost")

    send, recv = await conn.open_bidi_stream()
    await send.write(b"GET / HTTP/3\r\n\r\n")
    await send.finish()

    data = await recv.read(65536)
    print(f"Response: {data}")
    conn.close()


async def streaming_chat():
    """Stream LLM responses over HTTP/3 — no HOL blocking."""
    config = nhttp3.Config()
    ep = await nhttp3.Endpoint.bind("0.0.0.0", 0, config)
    conn = await ep.connect("localhost", 4433)

    send, recv = await conn.open_bidi_stream()

    # Send request
    import json
    request = json.dumps({
        "model": "llama3",
        "messages": [{"role": "user", "content": "Hello!"}],
        "stream": True,
    }).encode()
    await send.write(request)
    await send.finish()

    # Stream response tokens
    while True:
        chunk = await recv.read(4096)
        if not chunk:
            break
        print(chunk.decode(), end="", flush=True)

    print()
    conn.close()


async def parallel_requests():
    """Multiple concurrent requests — HTTP/3 multiplexing advantage."""
    config = nhttp3.Config()
    ep = await nhttp3.Endpoint.bind("0.0.0.0", 0, config)
    conn = await ep.connect("localhost", 4433)

    async def fetch(path: str):
        send, recv = await conn.open_bidi_stream()
        await send.write(f"GET {path} HTTP/3\r\n\r\n".encode())
        await send.finish()
        return await recv.read(65536)

    # Fire 10 requests in parallel — each on its own QUIC stream
    # No head-of-line blocking: one slow response doesn't block others
    results = await asyncio.gather(
        *[fetch(f"/api/item/{i}") for i in range(10)]
    )

    for i, result in enumerate(results):
        print(f"Item {i}: {len(result)} bytes")

    conn.close()


if __name__ == "__main__":
    print("=== Simple GET ===")
    asyncio.run(simple_get())
