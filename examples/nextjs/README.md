# Next.js + nhttp3

Use HTTP/3 from Next.js API routes and server components.

## What this does

Next.js API routes use `h3fetch()` to make HTTP/3 requests to backend services over QUIC. This gives you 1-RTT connection setup and no head-of-line blocking when calling microservices, LLM APIs, or any HTTP/3 endpoint.

```
Browser → Next.js (HTTP/1.1) → h3fetch() → Backend (HTTP/3/QUIC)
```

## Setup

```bash
# Terminal 1: Build nhttp3-node
cd nhttp3-node && npm install && npx napi build --release

# Terminal 2: Start HTTP/3 backend
cargo run -p nhttp3-server --bin nhttp3-server

# Terminal 3: Start Next.js
cd examples/nextjs && npm install && npm run dev
```

## Test

```bash
# HTTP/3 fetch from API route
curl http://localhost:3000/api/h3

# Health check through HTTP/3
curl http://localhost:3000/api/h3/health

# QPACK compression demo
curl http://localhost:3000/api/h3/headers

# POST through HTTP/3
curl -X POST -d '{"hello":"world"}' http://localhost:3000/api/h3
```

## When this helps

- Your Next.js app calls microservices over **high-latency networks** (cross-region, multi-cloud)
- You're calling **LLM inference APIs** that stream tokens (no HOL blocking over QUIC)
- Your backend-to-backend calls cross **data center boundaries**

## When this doesn't help

- Backend is on the same machine (localhost — use regular HTTP)
- You only need HTTP/3 for the browser → Next.js leg (use the reverse proxy instead)

## API

```typescript
import { h3fetch, encodeHeaders, decodeHeaders } from 'nhttp3-node';

// HTTP/3 GET
const resp = await h3fetch('https://api.example.com:4433/data');
// → { status: 200, body: '...', connectMs: 3.6, totalMs: 5.1 }

// HTTP/3 POST
const resp = await h3fetch('https://api.example.com:4433/infer', {
  method: 'POST',
  body: JSON.stringify({ prompt: 'Hello' }),
});

// QPACK header compression
const encoded = encodeHeaders([[':method','GET'],[':path','/api']]);
const decoded = decodeHeaders(encoded);
```
