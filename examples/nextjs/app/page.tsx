export default function Home() {
  return (
    <main style={{ fontFamily: 'monospace', padding: '2rem', background: '#0a0a0a', color: '#e0e0e0', minHeight: '100vh' }}>
      <h1 style={{ color: '#2dd4bf' }}>nhttp3 + Next.js</h1>
      <p style={{ color: '#888' }}>HTTP/3 requests from Next.js server components and API routes.</p>

      <h2 style={{ marginTop: '2rem' }}>Try these endpoints:</h2>
      <ul style={{ lineHeight: 2 }}>
        <li><a href="/api/h3" style={{ color: '#4a9eff' }}>/api/h3</a> — HTTP/3 fetch from API route</li>
        <li><a href="/api/h3/health" style={{ color: '#4a9eff' }}>/api/h3/health</a> — Health check over HTTP/3</li>
        <li><a href="/api/h3/echo" style={{ color: '#4a9eff' }}>/api/h3/echo</a> — POST echo over HTTP/3</li>
        <li><a href="/api/h3/headers" style={{ color: '#4a9eff' }}>/api/h3/headers</a> — QPACK compression demo</li>
      </ul>

      <h2 style={{ marginTop: '2rem' }}>Setup:</h2>
      <pre style={{ background: '#141414', padding: '1rem', border: '1px solid #2a2a2a' }}>{`# Terminal 1: Start the HTTP/3 backend
cargo run -p nhttp3-server --bin nhttp3-server

# Terminal 2: Start Next.js
cd examples/nextjs && npm install && npm run dev

# Terminal 3: Test
curl http://localhost:3000/api/h3`}</pre>
    </main>
  );
}
