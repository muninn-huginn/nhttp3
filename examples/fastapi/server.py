"""
FastAPI over HTTP/3 — drop-in replacement for uvicorn.

Before:
    uvicorn main:app --host 0.0.0.0 --port 8000

After:
    python server.py

Or with the CLI:
    nhttp3 run main:app --host 0.0.0.0 --port 4433 --certfile cert.pem --keyfile key.pem
"""

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, StreamingResponse
import nhttp3
import asyncio
from typing import AsyncGenerator

app = FastAPI(title="nhttp3 FastAPI Example")


@app.get("/")
async def root():
    return {"message": "Hello from HTTP/3!", "protocol": "h3"}


@app.get("/health")
async def health():
    return {"status": "ok", "server": "nhttp3"}


@app.post("/echo")
async def echo(request: Request):
    body = await request.body()
    return JSONResponse(
        content={"echo": body.decode(), "size": len(body)},
        headers={"x-protocol": "h3"},
    )


@app.get("/stream")
async def stream_response():
    """HTTP/3 shines here — no head-of-line blocking on multiplexed streams."""

    async def generate() -> AsyncGenerator[str, None]:
        for i in range(10):
            yield f"data: chunk {i}\n\n"
            await asyncio.sleep(0.1)

    return StreamingResponse(generate(), media_type="text/event-stream")


@app.get("/large")
async def large_response():
    """Large response — tests QUIC flow control and congestion control."""
    data = "x" * 1_000_000  # 1MB
    return JSONResponse(content={"size": len(data), "data": data[:100] + "..."})


if __name__ == "__main__":
    # One-liner: serve FastAPI over HTTP/3
    # Equivalent to: uvicorn main:app --host 0.0.0.0 --port 8000
    nhttp3.serve(
        app,
        host="0.0.0.0",
        port=4433,
        certfile="cert.pem",
        keyfile="key.pem",
    )
