# Browser Guide

Browsers support HTTP/3 natively. No special client library needed.

## Fetch API

Browsers auto-negotiate HTTP/3 via the `Alt-Svc` response header. Standard `fetch()` works:

```javascript
const resp = await fetch('https://localhost:4433/api/data');
const data = await resp.json();
```

## Verify HTTP/3 is being used

1. Open Chrome DevTools (F12)
2. Go to Network tab
3. Right-click column headers → enable "Protocol"
4. Look for `h3` in the Protocol column

## Streaming responses

```javascript
const resp = await fetch('/v1/chat/completions', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    model: 'llama3',
    messages: [{ role: 'user', content: 'Hello!' }],
    stream: true,
  }),
});

const reader = resp.body.getReader();
const decoder = new TextDecoder();
while (true) {
  const { value, done } = await reader.read();
  if (done) break;
  console.log(decoder.decode(value));
}
```

## WebTransport (Chrome 97+)

```javascript
const wt = new WebTransport('https://localhost:4433');
await wt.ready;

// Send datagram (unreliable, low-latency)
const writer = wt.datagrams.writable.getWriter();
await writer.write(new TextEncoder().encode('hello'));

// Bidirectional stream (reliable)
const stream = await wt.createBidirectionalStream();
const sw = stream.writable.getWriter();
await sw.write(new TextEncoder().encode('request'));
```

## Self-signed cert warning

For development with self-signed certs:
1. Visit `https://localhost:4433` directly in the browser
2. Click "Advanced" → "Proceed to localhost"
3. Then `fetch()` calls will work

## Browser support

| Browser | HTTP/3 | WebTransport |
|---------|--------|-------------|
| Chrome 87+ | Yes | Yes (97+) |
| Firefox 88+ | Yes | Yes (114+) |
| Safari 16+ | Yes | Partial |
| Edge 87+ | Yes | Yes (97+) |

## Interactive demo

See [browser client demo](../client.html) — taste-lab styled UI with Fetch, WebTransport, and streaming tabs.
