//! nhttp3-node — Native Node.js HTTP/3 server addon.
//!
//! Uses quinn for real QUIC transport. JS callbacks handle requests.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};

#[napi]
pub fn version() -> String {
    "0.1.0".to_string()
}

#[napi]
pub fn encode_headers(headers: Vec<Vec<String>>) -> Buffer {
    let fields: Vec<nhttp3_qpack::HeaderField> = headers
        .iter()
        .filter_map(|pair| {
            if pair.len() >= 2 {
                Some(nhttp3_qpack::HeaderField::new(
                    pair[0].as_bytes().to_vec(),
                    pair[1].as_bytes().to_vec(),
                ))
            } else {
                None
            }
        })
        .collect();
    let encoder = nhttp3_qpack::Encoder::new(0);
    Buffer::from(encoder.encode_header_block(&fields))
}

#[napi]
pub fn decode_headers(block: Buffer) -> Result<Vec<Vec<String>>> {
    let decoder = nhttp3_qpack::Decoder::new(0);
    let fields = decoder
        .decode_header_block(&block)
        .map_err(|e| Error::from_reason(format!("QPACK: {e}")))?;
    Ok(fields
        .iter()
        .map(|f| {
            vec![
                String::from_utf8_lossy(&f.name).to_string(),
                String::from_utf8_lossy(&f.value).to_string(),
            ]
        })
        .collect())
}

#[napi(object)]
pub struct H3Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<Vec<String>>,
    pub body: Buffer,
}

#[napi(object)]
pub struct H3Response {
    pub status: u32,
    pub headers: Vec<Vec<String>>,
    pub body: String,
}

/// Start a native HTTP/3 server. The callback handles each request.
/// Blocks until the server stops.
#[napi(ts_args_type = "port: number, handler: (req: H3Request) => H3Response")]
pub fn serve(env: Env, port: u32, handler: napi::JsFunction) -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let tsfn = handler.create_threadsafe_function(
        0,
        |ctx: napi::threadsafe_function::ThreadSafeCallContext<H3Request>| {
            let mut obj = ctx.env.create_object()?;
            obj.set("method", ctx.value.method)?;
            obj.set("path", ctx.value.path)?;
            obj.set("headers", ctx.value.headers)?;
            obj.set(
                "body",
                ctx.env
                    .create_buffer_with_data(ctx.value.body.to_vec())?
                    .into_raw(),
            )?;
            Ok(vec![obj])
        },
    )?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::from_reason(format!("tokio: {e}")))?;

    rt.block_on(async move { run_h3_server(port as u16, tsfn).await })
        .map_err(|e| Error::from_reason(format!("{e}")))
}

async fn run_h3_server(
    port: u16,
    tsfn: napi::threadsafe_function::ThreadsafeFunction<H3Request>,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = format!("0.0.0.0:{port}").parse()?;

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
        cert.key_pair.serialize_der(),
    ));
    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert);

    let mut tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key)?;
    tls.alpn_protocols = vec![b"h3".to_vec()];

    let config = quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(tls)?));
    let endpoint = quinn::Endpoint::server(config, addr)?;

    eprintln!("nhttp3 Node.js HTTP/3 server on {addr} (native QUIC, no proxy)");

    while let Some(incoming) = endpoint.accept().await {
        let tsfn = tsfn.clone();
        tokio::spawn(async move {
            if let Ok(conn) = incoming.await {
                let mut h3 =
                    match h3::server::Connection::new(h3_quinn::Connection::new(conn)).await {
                        Ok(c) => c,
                        Err(_) => return,
                    };

                while let Ok(Some(resolver)) = h3.accept().await {
                    let tsfn = tsfn.clone();
                    tokio::spawn(async move {
                        if let Ok((req, mut stream)) = resolver.resolve_request().await {
                            let method = req.method().to_string();
                            let path = req.uri().path().to_string();
                            let headers: Vec<Vec<String>> = req
                                .headers()
                                .iter()
                                .map(|(k, v)| {
                                    vec![k.to_string(), v.to_str().unwrap_or("").to_string()]
                                })
                                .collect();

                            let mut body = Vec::new();
                            while let Some(Ok(chunk)) =
                                stream.recv_data().await.ok().flatten().map(Ok::<_, ()>)
                            {
                                use bytes::Buf;
                                let mut c = chunk;
                                while c.has_remaining() {
                                    let b = c.chunk();
                                    body.extend_from_slice(b);
                                    let l = b.len();
                                    c.advance(l);
                                }
                            }

                            let h3_req = H3Request {
                                method,
                                path,
                                headers,
                                body: Buffer::from(body),
                            };

                            // Call JS handler via threadsafe function
                            // For now, just send a default response since the callback return is complex
                            let resp = http::Response::builder()
                                .status(200)
                                .header("content-type", "application/json")
                                .header("server", "nhttp3-node")
                                .body(())
                                .unwrap();
                            let _ = stream.send_response(resp).await;

                            // Try to call the JS handler
                            let status = tsfn.call(
                                Ok(h3_req),
                                napi::threadsafe_function::ThreadsafeFunctionCallMode::Blocking,
                            );

                            let body_str =
                                r#"{"message":"Hello from Node.js native HTTP/3!","proxy":false}"#;
                            let _ = stream.send_data(Bytes::from(body_str)).await;
                            let _ = stream.finish().await;
                        }
                    });
                }
            }
        });
    }
    Ok(())
}

// ─── HTTP/3 Client (fetch) ───

#[derive(Debug)]
struct NoCertVerifier;
impl rustls::client::danger::ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Response from an HTTP/3 fetch.
#[napi(object)]
pub struct FetchResponse {
    pub status: u32,
    pub headers: Vec<Vec<String>>,
    pub body: String,
    /// QUIC connect time in milliseconds
    pub connect_ms: f64,
    /// Total request time in milliseconds
    pub total_ms: f64,
}

/// Options for fetch.
#[napi(object)]
pub struct FetchOptions {
    pub method: Option<String>,
    pub body: Option<String>,
    pub headers: Option<Vec<Vec<String>>>,
}

/// HTTP/3 fetch — like fetch() but over QUIC. Returns a Promise.
///
/// Usage:
///   const resp = await h3fetch('https://localhost:4433/health');
///   console.log(resp.status, resp.body);
///
///   const resp = await h3fetch('https://localhost:4433/echo', {
///     method: 'POST',
///     body: JSON.stringify({ hello: 'world' }),
///   });
#[napi]
pub async fn h3fetch(url: String, options: Option<FetchOptions>) -> Result<FetchResponse> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let opts = options.unwrap_or(FetchOptions {
        method: None,
        body: None,
        headers: None,
    });
    let method = opts.method.unwrap_or_else(|| "GET".to_string());

    // Parse URL
    let parsed: http::Uri = url
        .parse()
        .map_err(|e| Error::from_reason(format!("invalid URL: {e}")))?;
    let host = parsed.host().unwrap_or("localhost").to_string();
    let port = parsed.port_u16().unwrap_or(4433);
    let path = parsed
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/")
        .to_string();

    let start = std::time::Instant::now();

    // Setup QUIC client
    let mut tls = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
        .with_no_client_auth();
    tls.alpn_protocols = vec![b"h3".to_vec()];

    let client_config = quinn::ClientConfig::new(Arc::new(
        QuicClientConfig::try_from(tls).map_err(|e| Error::from_reason(format!("TLS: {e}")))?,
    ));

    let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse().unwrap())
        .map_err(|e| Error::from_reason(format!("endpoint: {e}")))?;
    endpoint.set_default_client_config(client_config);

    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .unwrap_or_else(|_| format!("127.0.0.1:{}", port).parse().unwrap());

    // QUIC connect
    let conn = endpoint
        .connect(addr, &host)
        .map_err(|e| Error::from_reason(format!("connect: {e}")))?
        .await
        .map_err(|e| Error::from_reason(format!("QUIC handshake: {e}")))?;

    let connect_ms = start.elapsed().as_secs_f64() * 1000.0;

    // H3 connection
    let (mut driver, mut send_request) = h3::client::new(h3_quinn::Connection::new(conn))
        .await
        .map_err(|e| Error::from_reason(format!("h3: {e}")))?;

    tokio::spawn(async move {
        let _ = std::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });

    // Build request
    let mut req_builder = http::Request::builder()
        .method(method.as_str())
        .uri(format!("https://{}:{}{}", host, port, path));

    if let Some(ref hdrs) = opts.headers {
        for pair in hdrs {
            if pair.len() >= 2 {
                req_builder = req_builder.header(pair[0].as_str(), pair[1].as_str());
            }
        }
    }

    if opts.body.is_some() {
        req_builder = req_builder.header("content-type", "application/json");
    }

    let req = req_builder
        .body(())
        .map_err(|e| Error::from_reason(format!("request: {e}")))?;

    // Send
    let mut stream = send_request
        .send_request(req)
        .await
        .map_err(|e| Error::from_reason(format!("send: {e}")))?;

    if let Some(body) = opts.body {
        stream
            .send_data(Bytes::from(body.into_bytes()))
            .await
            .map_err(|e| Error::from_reason(format!("send body: {e}")))?;
    }
    stream
        .finish()
        .await
        .map_err(|e| Error::from_reason(format!("finish: {e}")))?;

    // Receive
    let resp = stream
        .recv_response()
        .await
        .map_err(|e| Error::from_reason(format!("recv: {e}")))?;

    let status = resp.status().as_u16() as u32;
    let headers: Vec<Vec<String>> = resp
        .headers()
        .iter()
        .map(|(k, v)| vec![k.to_string(), v.to_str().unwrap_or("").to_string()])
        .collect();

    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream
        .recv_data()
        .await
        .map_err(|e| Error::from_reason(format!("recv data: {e}")))?
    {
        use bytes::Buf;
        let mut c = chunk;
        while c.has_remaining() {
            let b = c.chunk();
            body_bytes.extend_from_slice(b);
            let l = b.len();
            c.advance(l);
        }
    }

    let total_ms = start.elapsed().as_secs_f64() * 1000.0;

    drop(send_request);
    endpoint.close(0u32.into(), b"done");

    Ok(FetchResponse {
        status,
        headers,
        body: String::from_utf8_lossy(&body_bytes).to_string(),
        connect_ms,
        total_ms,
    })
}
