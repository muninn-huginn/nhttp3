import { NextResponse } from 'next/server';
import { getNhttp3 } from '../../../../lib/nhttp3';

export async function GET() {
  const nhttp3 = getNhttp3();
  if (!nhttp3) return NextResponse.json({ error: 'nhttp3-node not loaded' }, { status: 503 });
  try {
    const resp = await nhttp3.h3fetch('https://localhost:4433/health');
    return NextResponse.json({
      nextjs: 'ok', backend: JSON.parse(resp.body), transport: 'HTTP/3',
      connect_ms: resp.connectMs || resp.connect_ms,
    });
  } catch (e: any) {
    return NextResponse.json({ error: e.message }, { status: 502 });
  }
}
