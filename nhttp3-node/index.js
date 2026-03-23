/**
 * nhttp3 — Native HTTP/3 for Node.js.
 *
 * Server:
 *   const { serve } = require('nhttp3-node');
 *   serve(4433, (req) => ({ status: 200, headers: [...], body: '...' }));
 *
 * Client:
 *   const { h3fetch } = require('nhttp3-node');
 *   const resp = await h3fetch('https://localhost:4433/health');
 *   console.log(resp.status, resp.body);
 *
 * QPACK:
 *   const { encodeHeaders, decodeHeaders } = require('nhttp3-node');
 */

const native = require('./nhttp3-node.node');

module.exports = {
  version: native.version,
  serve: native.serve,
  h3fetch: native.h3Fetch,
  encodeHeaders: native.encodeHeaders,
  decodeHeaders: native.decodeHeaders,
};
