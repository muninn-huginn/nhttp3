/**
 * nhttp3 — Native HTTP/3 server for Node.js.
 *
 * Usage:
 *   const { serve, encodeHeaders, decodeHeaders, version } = require('nhttp3-node');
 *
 *   // Express-like API
 *   serve(4433, (req) => {
 *     console.log(`${req.method} ${req.path}`);
 *     return {
 *       status: 200,
 *       headers: [['content-type', 'application/json']],
 *       body: JSON.stringify({ hello: 'world', protocol: 'h3' }),
 *     };
 *   });
 *
 *   // QPACK header compression
 *   const encoded = encodeHeaders([[':method', 'GET'], [':path', '/']]);
 *   const decoded = decodeHeaders(encoded);
 */

const native = require('./nhttp3-node.node');

module.exports = {
  version: native.version,
  serve: native.serve,
  encodeHeaders: native.encodeHeaders,
  decodeHeaders: native.decodeHeaders,
};
