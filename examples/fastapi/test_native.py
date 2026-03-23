"""Test nhttp3.serve() with a real FastAPI app — no proxy, native HTTP/3."""

from fastapi import FastAPI
import nhttp3

app = FastAPI()

@app.get("/")
async def root():
    return {"message": "Hello from FastAPI over native HTTP/3!", "proxy": False}

@app.get("/health")
async def health():
    return {"status": "ok", "server": "nhttp3", "transport": "native_quic"}

@app.post("/echo")
async def echo(request):
    body = await request.body()
    return {"echo": body.decode(), "size": len(body)}

# This is the real thing — no uvicorn, no proxy.
# nhttp3 starts a QUIC server and calls FastAPI directly.
if __name__ == "__main__":
    print("Starting FastAPI on native HTTP/3 (no uvicorn, no proxy)...")
    nhttp3.serve(app, host="0.0.0.0", port=4433)
