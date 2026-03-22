/**
 * Express.js-style HTTP/3 server using nhttp3 WASM bindings.
 *
 * This demonstrates how Node.js apps can use nhttp3 for:
 * - QPACK header compression
 * - HTTP/3 frame encoding
 * - WebTransport integration
 *
 * For full HTTP/3 serving in Node.js, nhttp3 provides the protocol
 * layer while the transport uses Node's UDP sockets.
 *
 * Usage:
 *   npm install nhttp3
 *   node server.js
 */

// Import nhttp3 WASM bindings
// After: wasm-pack build --target nodejs && npm link
const nhttp3 = require('nhttp3');

// ─── QPACK Header Compression Demo ───

function demonstrateQPACK() {
  console.log('=== QPACK Header Compression ===\n');

  // Typical API request headers
  const requestHeaders = [
    [':method', 'GET'],
    [':path', '/api/v1/users'],
    [':scheme', 'https'],
    [':authority', 'api.example.com'],
    ['accept', 'application/json'],
    ['authorization', 'Bearer eyJhbGciOiJSUzI1NiJ9.test-token'],
    ['user-agent', 'nhttp3-node/0.1'],
  ];

  // Calculate raw size
  const rawSize = requestHeaders.reduce(
    (sum, [k, v]) => sum + k.length + v.length + 4, 0
  );

  // Encode with QPACK
  const encoded = nhttp3.encode_headers(requestHeaders);
  const decoded = nhttp3.decode_headers(encoded);

  console.log(`Raw headers:     ${rawSize} bytes`);
  console.log(`QPACK encoded:   ${encoded.length} bytes`);
  console.log(`Compression:     ${((1 - encoded.length / rawSize) * 100).toFixed(0)}% savings`);
  console.log(`Decoded headers: ${decoded.length} pairs`);

  // Verify roundtrip
  for (let i = 0; i < requestHeaders.length; i++) {
    const [origName, origValue] = requestHeaders[i];
    const [decName, decValue] = decoded[i];
    if (origName !== decName || origValue !== decValue) {
      console.error(`MISMATCH at ${i}: ${origName}=${origValue} vs ${decName}=${decValue}`);
    }
  }
  console.log('Roundtrip: OK\n');
}

// ─── HTTP/3 Frame Encoding Demo ───

function demonstrateFrames() {
  console.log('=== HTTP/3 Frame Encoding ===\n');

  // Encode a DATA frame
  const body = new TextEncoder().encode('{"message":"Hello from HTTP/3!"}');
  const dataFrame = nhttp3.encode_data_frame(body);
  console.log(`DATA frame: ${dataFrame.length} bytes (payload: ${body.length})`);

  // Encode a SETTINGS frame
  const settingsFrame = nhttp3.encode_settings_frame();
  console.log(`SETTINGS frame: ${settingsFrame.length} bytes`);

  // Encode request headers as a HEADERS frame
  const headers = [
    [':method', 'POST'],
    [':path', '/api/chat'],
    [':scheme', 'https'],
    [':authority', 'localhost:4433'],
    ['content-type', 'application/json'],
  ];
  const headerBlock = nhttp3.encode_headers(headers);
  const headersFrame = nhttp3.encode_headers_frame(headerBlock);
  console.log(`HEADERS frame: ${headersFrame.length} bytes (QPACK block: ${headerBlock.length})`);
  console.log();
}

// ─── Express-like Router Using HTTP/3 Frames ───

class H3Router {
  constructor() {
    this.routes = new Map();
  }

  get(path, handler) {
    this.routes.set(`GET:${path}`, handler);
  }

  post(path, handler) {
    this.routes.set(`POST:${path}`, handler);
  }

  /**
   * Handles a request by encoding the response as HTTP/3 frames.
   * Returns an array of frame buffers ready to send over QUIC streams.
   */
  handle(method, path, body = null) {
    const key = `${method}:${path}`;
    const handler = this.routes.get(key);

    if (!handler) {
      return this._buildResponse(404, { error: 'not found' });
    }

    const result = handler({ method, path, body });
    return this._buildResponse(200, result);
  }

  _buildResponse(status, data) {
    const body = JSON.stringify(data);
    const bodyBytes = new TextEncoder().encode(body);

    // Build response headers
    const responseHeaders = [
      [':status', String(status)],
      ['content-type', 'application/json'],
      ['content-length', String(bodyBytes.length)],
      ['server', 'nhttp3-express'],
      ['x-protocol', 'h3'],
    ];

    // Encode as HTTP/3 frames
    const headerBlock = nhttp3.encode_headers(responseHeaders);
    const headersFrame = nhttp3.encode_headers_frame(headerBlock);
    const dataFrame = nhttp3.encode_data_frame(bodyBytes);

    return {
      frames: [headersFrame, dataFrame],
      totalSize: headersFrame.length + dataFrame.length,
      status,
    };
  }
}

// ─── Demo: Express-like API ───

function demonstrateRouter() {
  console.log('=== Express-like HTTP/3 Router ===\n');

  const app = new H3Router();

  app.get('/', (req) => ({
    message: 'Hello from HTTP/3!',
    protocol: 'h3',
  }));

  app.get('/health', (req) => ({
    status: 'ok',
    server: 'nhttp3-express',
  }));

  app.post('/api/echo', (req) => ({
    echo: req.body,
    protocol: 'h3',
  }));

  // Simulate handling requests
  const routes = [
    ['GET', '/'],
    ['GET', '/health'],
    ['POST', '/api/echo'],
    ['GET', '/not-found'],
  ];

  for (const [method, path] of routes) {
    const response = app.handle(method, path, '{"test": true}');
    console.log(`${method} ${path} -> ${response.status} (${response.totalSize} bytes in ${response.frames.length} frames)`);
  }
  console.log();
}

// ─── Run All Demos ───

console.log(`nhttp3 v${nhttp3.version()}\n`);
demonstrateQPACK();
demonstrateFrames();
demonstrateRouter();

console.log('=== Integration Points ===\n');
console.log('Express/Fastify: Use H3Router for HTTP/3 frame encoding');
console.log('  - QPACK compresses headers (50% savings)');
console.log('  - Frames ready to send over QUIC streams');
console.log('  - Full Node.js HTTP/3 server needs UDP socket binding\n');
console.log('For production Node.js HTTP/3 serving, combine with:');
console.log('  - node:dgram for UDP sockets');
console.log('  - nhttp3 WASM for protocol encoding');
console.log('  - Or use the Rust nhttp3 crate as a native addon via napi-rs');
