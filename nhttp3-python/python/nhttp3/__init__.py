"""nhttp3 — Production-grade HTTP/3 for Python, powered by Rust.

Quick start:
    import nhttp3

    # As ASGI server (drop-in for uvicorn):
    from fastapi import FastAPI
    app = FastAPI()

    @app.get("/")
    async def root():
        return {"hello": "world"}

    nhttp3.serve(app, host="0.0.0.0", port=4433,
                 certfile="cert.pem", keyfile="key.pem")

    # Low-level QUIC client:
    async with nhttp3.Endpoint.bind("0.0.0.0", 0) as ep:
        conn = await ep.connect("example.com", 443)
        send, recv = await conn.open_bidi_stream()
        await send.write(b"hello")
        data = await recv.read(1024)
"""

from nhttp3._nhttp3 import (
    Config,
    Endpoint,
    Connection,
    SendStream,
    RecvStream,
    H3Server,
    __version__,
)


def serve(app, *, host="0.0.0.0", port=4433, certfile=None, keyfile=None):
    """Run an ASGI application over HTTP/3.

    Drop-in replacement for `uvicorn.run()`:

        # Before (HTTP/1.1 + HTTP/2):
        uvicorn.run(app, host="0.0.0.0", port=8000)

        # After (HTTP/3):
        nhttp3.serve(app, host="0.0.0.0", port=4433,
                      certfile="cert.pem", keyfile="key.pem")
    """
    server = H3Server(
        app, host=host, port=port, certfile=certfile, keyfile=keyfile
    )
    server.serve()  # Blocks until Ctrl+C — runs QUIC server on tokio


__all__ = [
    "Config",
    "Endpoint",
    "Connection",
    "SendStream",
    "RecvStream",
    "H3Server",
    "serve",
    "__version__",
]
