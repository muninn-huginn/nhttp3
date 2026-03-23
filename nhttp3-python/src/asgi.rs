//! Real ASGI HTTP/3 server — no proxy, no fakes.
//!
//! Rust (quinn) accepts HTTP/3 connections → calls Python ASGI app via PyO3 →
//! streams response back over QUIC.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use quinn::crypto::rustls::QuicServerConfig;

use crate::async_bridge;

#[pyclass]
pub struct H3Server {
    app: Py<PyAny>,
    host: String,
    port: u16,
    certfile: Option<String>,
    keyfile: Option<String>,
}

#[pymethods]
impl H3Server {
    #[new]
    #[pyo3(signature = (app, host="0.0.0.0".to_string(), port=4433, certfile=None, keyfile=None))]
    fn new(
        app: Py<PyAny>,
        host: String,
        port: u16,
        certfile: Option<String>,
        keyfile: Option<String>,
    ) -> Self {
        Self {
            app,
            host,
            port,
            certfile,
            keyfile,
        }
    }

    /// Start the server. Blocks until Ctrl+C.
    fn serve(&self, py: Python<'_>) -> PyResult<()> {
        let app: Py<PyAny> = self.app.clone_ref(py);
        let host = self.host.clone();
        let port = self.port;

        // Release GIL and run the server on tokio
        py.detach(|| {
            let rt = async_bridge::runtime();
            rt.block_on(async move {
                run_server(app, &host, port)
                    .await
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{e}")))
            })
        })
    }

    fn __repr__(&self) -> String {
        format!("H3Server({}:{})", self.host, self.port)
    }
}

async fn run_server(
    app: Py<PyAny>,
    host: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let addr: SocketAddr = format!("{host}:{port}").parse()?;

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
        cert.key_pair.serialize_der(),
    ));
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert);

    let mut tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key)?;
    tls.alpn_protocols = vec![b"h3".to_vec()];

    let quic_config = quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(tls)?));
    let endpoint = quinn::Endpoint::server(quic_config, addr)?;

    eprintln!("nhttp3 HTTP/3 server on {addr} (native QUIC, no proxy)");

    let app = Arc::new(app);

    loop {
        tokio::select! {
            incoming = endpoint.accept() => {
                match incoming {
                    Some(inc) => {
                        let app = app.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(inc, app).await {
                                eprintln!("conn error: {e}");
                            }
                        });
                    }
                    None => break,
                }
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\nshutting down");
                break;
            }
        }
    }

    endpoint.close(0u32.into(), b"shutdown");
    Ok(())
}

async fn handle_connection(
    incoming: quinn::Incoming,
    app: Arc<Py<PyAny>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = incoming.await?;
    let mut h3 = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await?;

    while let Some(resolver) = h3.accept().await? {
        let app = app.clone();
        tokio::spawn(async move {
            match resolver.resolve_request().await {
                Ok((req, mut stream)) => {
                    if let Err(e) = handle_request(req, &mut stream, &app).await {
                        eprintln!("req error: {e}");
                    }
                }
                Err(e) => eprintln!("resolve error: {e}"),
            }
        });
    }
    Ok(())
}

async fn handle_request(
    req: http::Request<()>,
    stream: &mut h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
    app: &Py<PyAny>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("").to_string();
    let headers: Vec<(Vec<u8>, Vec<u8>)> = req
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().as_bytes().to_vec(), v.as_bytes().to_vec()))
        .collect();

    // Read body
    let mut body = Vec::new();
    while let Some(chunk) = stream.recv_data().await? {
        use bytes::Buf;
        let mut c = chunk;
        while c.has_remaining() {
            let b = c.chunk();
            body.extend_from_slice(b);
            let l = b.len();
            c.advance(l);
        }
    }

    // Call ASGI app on a blocking thread (Python needs GIL)
    let app_owned: Py<PyAny> = Python::attach(|py| app.clone_ref(py));
    let (status, resp_headers, resp_body) = tokio::task::spawn_blocking(move || {
        Python::attach(|py| -> PyResult<(u16, Vec<(Vec<u8>, Vec<u8>)>, Vec<u8>)> {
            call_asgi(py, &app_owned, &method, &path, &query, &headers, &body)
        })
    })
    .await??;

    // Send response over QUIC
    let mut builder = http::Response::builder()
        .status(status)
        .header("server", "nhttp3");
    for (name, value) in &resp_headers {
        if let (Ok(n), Ok(v)) = (std::str::from_utf8(name), std::str::from_utf8(value)) {
            builder = builder.header(n, v);
        }
    }
    stream.send_response(builder.body(())?).await?;
    stream.send_data(Bytes::from(resp_body)).await?;
    stream.finish().await?;
    Ok(())
}

/// Calls a Python ASGI app. Uses asyncio.run() to drive the coroutine.
fn call_asgi(
    py: Python<'_>,
    app: &Py<PyAny>,
    method: &str,
    path: &str,
    query: &str,
    headers: &[(Vec<u8>, Vec<u8>)],
    body: &[u8],
) -> PyResult<(u16, Vec<(Vec<u8>, Vec<u8>)>, Vec<u8>)> {
    // Build scope dict
    let scope = PyDict::new(py);
    scope.set_item("type", "http")?;
    scope.set_item("http_version", "3")?;
    scope.set_item("method", method)?;
    scope.set_item("path", path)?;
    scope.set_item("root_path", "")?;
    scope.set_item("scheme", "https")?;
    scope.set_item("query_string", PyBytes::new(py, query.as_bytes()))?;

    let py_headers = PyList::empty(py);
    for (name, value) in headers {
        let pair = PyList::empty(py);
        pair.append(PyBytes::new(py, name))?;
        pair.append(PyBytes::new(py, value))?;
        py_headers.append(pair)?;
    }
    scope.set_item("headers", py_headers)?;

    // Run the ASGI app via a Python helper that handles async
    let helper = py.run(
        c"
import asyncio

async def _nhttp3_run_asgi(app, scope, body_bytes):
    status = [200]
    headers = [[]]
    body_parts = []

    async def receive():
        return {'type': 'http.request', 'body': body_bytes, 'more_body': False}

    async def send(message):
        if message['type'] == 'http.response.start':
            status[0] = message.get('status', 200)
            headers[0] = message.get('headers', [])
        elif message['type'] == 'http.response.body':
            body_parts.append(message.get('body', b''))

    await app(scope, receive, send)
    # Normalize headers to list of lists (ASGI allows tuples)
    normalized = [[bytes(h[0]), bytes(h[1])] for h in headers[0]]
    return (status[0], normalized, b''.join(body_parts))
",
        None,
        None,
    )?;

    let run_asgi = py.eval(c"_nhttp3_run_asgi", None, None)?;
    let asyncio = py.import("asyncio")?;

    let body_py = PyBytes::new(py, body);
    let coro = run_asgi.call1((app.bind(py), scope, body_py))?;
    let result = asyncio.call_method1("run", (coro,))?;

    let status: u16 = result.get_item(0)?.extract()?;
    let py_resp_headers = result.get_item(1)?;
    let resp_body: Vec<u8> = result.get_item(2)?.extract()?;

    let mut resp_headers = Vec::new();
    let header_list = py_resp_headers.downcast::<PyList>()?;
    for i in 0..header_list.len() {
        let pair = header_list.get_item(i)?;
        let pair_list = pair.downcast::<PyList>()?;
        let name: Vec<u8> = pair_list.get_item(0)?.extract()?;
        let value: Vec<u8> = pair_list.get_item(1)?.extract()?;
        resp_headers.push((name, value));
    }

    Ok((status, resp_headers, resp_body))
}
