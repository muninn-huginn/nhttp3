use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nhttp3_quic::tls::TlsSession;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::sync::Arc;

fn self_signed_cert() -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()));
    let cert = CertificateDer::from(cert.cert);
    (cert, key)
}

#[derive(Debug)]
struct NoCertVerifier;
impl rustls::client::danger::ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self, _: &CertificateDer<'_>, _: &[CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>, _: &[u8], _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self, _: &[u8], _: &CertificateDer<'_>, _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self, _: &[u8], _: &CertificateDer<'_>, _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn tls_handshake_benchmark(c: &mut Criterion) {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let mut group = c.benchmark_group("handshake");

    let (cert, key) = self_signed_cert();

    let mut cc = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
        .with_no_client_auth();
    cc.alpn_protocols = vec![b"h3".to_vec()];
    let cc = Arc::new(cc);

    let mut sc = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap();
    sc.alpn_protocols = vec![b"h3".to_vec()];
    let sc = Arc::new(sc);

    group.bench_function("quic_tls_in_process", |b| {
        b.iter(|| {
            let sni = "localhost".try_into().unwrap();
            let mut client = TlsSession::new_client(cc.clone(), sni, vec![]).unwrap();
            let mut server = TlsSession::new_server(sc.clone(), vec![]).unwrap();

            let ch = client.write_handshake();
            server.read_handshake(&ch.data).unwrap();
            let sh = server.write_handshake();
            client.read_handshake(&sh.data).unwrap();
            let cf = client.write_handshake();
            if !cf.data.is_empty() {
                server.read_handshake(&cf.data).unwrap();
                let _ = server.write_handshake();
            }
            black_box(client.is_handshaking());
        });
    });

    group.finish();
}

fn qpack_compression_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    let headers = vec![
        nhttp3_qpack::HeaderField::new(":method", "GET"),
        nhttp3_qpack::HeaderField::new(":path", "/api/v1/users?page=1&limit=50"),
        nhttp3_qpack::HeaderField::new(":scheme", "https"),
        nhttp3_qpack::HeaderField::new(":authority", "api.example.com"),
        nhttp3_qpack::HeaderField::new("accept", "application/json"),
        nhttp3_qpack::HeaderField::new(
            "authorization",
            "Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0",
        ),
        nhttp3_qpack::HeaderField::new("content-type", "application/json"),
        nhttp3_qpack::HeaderField::new("user-agent", "nhttp3-bench/0.1"),
    ];

    let encoder = nhttp3_qpack::Encoder::new(0);
    let decoder = nhttp3_qpack::Decoder::new(0);

    let encoded = encoder.encode_header_block(&headers);
    let raw_size: usize = headers.iter().map(|h| h.name.len() + h.value.len() + 2).sum();

    group.bench_function("qpack_encode_8h_realistic", |b| {
        b.iter(|| {
            black_box(encoder.encode_header_block(black_box(&headers)));
        });
    });

    group.bench_function("qpack_decode_8h_realistic", |b| {
        b.iter(|| {
            black_box(decoder.decode_header_block(black_box(&encoded)).unwrap());
        });
    });

    group.finish();

    println!(
        "\n  QPACK: {} raw bytes -> {} encoded ({:.0}%)",
        raw_size,
        encoded.len(),
        (encoded.len() as f64 / raw_size as f64) * 100.0
    );
}

criterion_group!(benches, tls_handshake_benchmark, qpack_compression_benchmark,);
criterion_main!(benches);
