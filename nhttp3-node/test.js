/**
 * Test the native HTTP/3 Node.js server.
 *
 * Run:
 *   cd nhttp3-node && npm run build && node test.js
 *
 * Then from another terminal:
 *   cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/
 */

const { serve, encodeHeaders, decodeHeaders, version } = require('./index');

console.log(`nhttp3 v${version()}`);

// Test QPACK
const headers = [[':method', 'GET'], [':path', '/api'], ['accept', 'application/json']];
const encoded = encodeHeaders(headers);
const decoded = decodeHeaders(encoded);
console.log(`QPACK: ${headers.length} headers → ${encoded.length} bytes → ${decoded.length} decoded`);

// Start HTTP/3 server
console.log('\nStarting HTTP/3 server on :4433...');
console.log('Test with: cargo run -p nhttp3-server --bin nhttp3-client -- https://localhost:4433/\n');

serve(4433, (req) => {
  console.log(`[${new Date().toISOString()}] ${req.method} ${req.path}`);

  if (req.path === '/') {
    return {
      status: 200,
      headers: [['content-type', 'application/json']],
      body: JSON.stringify({
        message: 'Hello from Node.js over native HTTP/3!',
        proxy: false,
        runtime: 'node',
        transport: 'native_quic',
      }),
    };
  }

  if (req.path === '/health') {
    return {
      status: 200,
      headers: [['content-type', 'application/json']],
      body: JSON.stringify({ status: 'ok', server: 'nhttp3-node' }),
    };
  }

  return {
    status: 404,
    headers: [['content-type', 'application/json']],
    body: JSON.stringify({ error: 'not found', path: req.path }),
  };
});
