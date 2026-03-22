"""Minimal FastAPI app for testing nhttp3 proxy."""
from fastapi import FastAPI, Request
from fastapi.responses import StreamingResponse
import asyncio

app = FastAPI()

@app.get("/")
async def root():
    return {"message": "Hello from FastAPI!", "backend": "uvicorn", "frontend": "nhttp3"}

@app.get("/health")
async def health():
    return {"status": "ok"}

@app.post("/echo")
async def echo(request: Request):
    body = await request.body()
    return {"echo": body.decode(), "size": len(body)}

@app.get("/stream")
async def stream():
    async def generate():
        for i in range(5):
            yield f"data: {{\"chunk\": {i}}}\n\n"
            await asyncio.sleep(0.1)
        yield "data: [DONE]\n\n"
    return StreamingResponse(generate(), media_type="text/event-stream")
