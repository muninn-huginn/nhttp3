import { NextResponse } from 'next/server';
import { getNhttp3 } from '../../../../lib/nhttp3';

export async function GET() {
  const nhttp3 = getNhttp3();
  if (!nhttp3) return NextResponse.json({ error: 'nhttp3-node not loaded' }, { status: 503 });
  try {
    const headers = [
      [':method', 'GET'], [':path', '/api/users'], [':scheme', 'https'],
      ['accept', 'application/json'], ['authorization', 'Bearer token123'],
      ['user-agent', 'nhttp3-nextjs/0.1'], ['accept-encoding', 'gzip, br'],
    ];
    const rawSize = headers.reduce((s: number, [k, v]: string[]) => s + k.length + v.length + 4, 0);
    const encoded = nhttp3.encodeHeaders(headers);
    const decoded = nhttp3.decodeHeaders(encoded);
    return NextResponse.json({
      demo: 'qpack-compression', headers_count: headers.length,
      raw_bytes: rawSize, qpack_bytes: encoded.length,
      savings_pct: Math.round((1 - encoded.length / rawSize) * 100),
      roundtrip_ok: decoded.length === headers.length,
    });
  } catch (e: any) {
    return NextResponse.json({ error: e.message }, { status: 500 });
  }
}
