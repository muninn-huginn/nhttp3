# Node.js Server Guide

Native HTTP/3 server for Node.js via napi-rs addon. No proxy.

## Install

```bash
cd nhttp3-node
npm install
npx napi build --release
```

## Serve

```javascript
const { serve } = require('./index');

serve(4433, (req) => {
  console.log(`${req.method} ${req.path}`);

  if (req.path === '/') {
    return {
      status: 200,
      headers: [['content-type', 'application/json']],
      body: JSON.stringify({ hello: 'world', protocol: 'h3' }),
    };
  }

  if (req.path === '/health') {
    return {
      status: 200,
      headers: [['content-type', 'application/json']],
      body: JSON.stringify({ status: 'ok' }),
    };
  }

  return {
    status: 404,
    headers: [['content-type', 'application/json']],
    body: JSON.stringify({ error: 'not found' }),
  };
});
```

## Test

```bash
cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/
# → {"hello":"world","protocol":"h3"}
```

## QPACK Header Compression

```javascript
const { encodeHeaders, decodeHeaders } = require('./index');

const headers = [[':method', 'GET'], [':path', '/api'], ['accept', 'application/json']];
const encoded = encodeHeaders(headers);   // 28 bytes
const decoded = decodeHeaders(encoded);   // roundtrip verified
```

## Handler API

```javascript
// Request object passed to your handler
{
  method: 'GET',                            // HTTP method
  path: '/api/users',                       // Request path
  headers: [['accept', 'application/json']], // [name, value] pairs
  body: Buffer                              // Request body
}

// Return a response object
{
  status: 200,
  headers: [['content-type', 'application/json']],
  body: '{"ok": true}'                     // String
}
```

## How it works

```
HTTP/3 request → quinn (QUIC) → h3 (HTTP/3) → napi → JS callback → napi → h3 → quinn → response
```

The QUIC server runs inside your Node.js process. Your handler is called directly via napi-rs ThreadsafeFunction.
