import { NextResponse } from 'next/server';
import { getNhttp3 } from '../../../lib/nhttp3';

export async function GET() {
  const nhttp3 = getNhttp3();
  if (!nhttp3) {
    return NextResponse.json({ error: 'nhttp3-node not loaded' }, { status: 503 });
  }

  try {
    const start = Date.now();
    const resp = await nhttp3.h3fetch('https://localhost:4433/');
    const elapsed = Date.now() - start;

    return NextResponse.json({
      source: 'nextjs-api-route',
      transport: 'HTTP/3 (QUIC)',
      backend_response: JSON.parse(resp.body),
      timing: {
        quic_connect_ms: resp.connectMs || resp.connect_ms,
        total_ms: resp.totalMs || resp.total_ms,
        nextjs_overhead_ms: elapsed - (resp.totalMs || resp.total_ms || 0),
      },
    });
  } catch (e: any) {
    return NextResponse.json({
      error: e.message,
      hint: 'Is nhttp3-server running? cargo run -p nhttp3-server --bin nhttp3-server',
    }, { status: 502 });
  }
}

export async function POST(request: Request) {
  const nhttp3 = getNhttp3();
  if (!nhttp3) {
    return NextResponse.json({ error: 'nhttp3-node not loaded' }, { status: 503 });
  }

  try {
    const body = await request.text();
    const resp = await nhttp3.h3fetch('https://localhost:4433/echo', {
      method: 'POST',
      body,
    });
    return NextResponse.json({
      source: 'nextjs',
      transport: 'HTTP/3',
      backend_response: JSON.parse(resp.body),
    });
  } catch (e: any) {
    return NextResponse.json({ error: e.message }, { status: 502 });
  }
}
