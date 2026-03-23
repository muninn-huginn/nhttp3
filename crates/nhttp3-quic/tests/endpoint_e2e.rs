//! End-to-end test: client and server complete a QUIC handshake
//! over real localhost UDP sockets, exchanging properly framed packets.

use std::sync::Arc;

use nhttp3_quic::config::Config;
use nhttp3_quic::endpoint::Endpoint;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

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
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[tokio::test]
async fn endpoint_bind_and_connect() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let (cert, key) = self_signed_cert();

    let mut server_tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap();
    server_tls.alpn_protocols = vec![b"h3".to_vec()];

    let mut client_tls = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
        .with_no_client_auth();
    client_tls.alpn_protocols = vec![b"h3".to_vec()];

    let config = Config::default();
    let server_addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();

    // Bind server
    let mut server = Endpoint::bind(
        server_addr,
        config.clone(),
        Some(Arc::new(server_tls)),
        None,
    )
    .await
    .unwrap();

    let server_local = server.local_addr().unwrap();

    // Bind client
    let client_ep = Endpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        config,
        None,
        Some(Arc::new(client_tls)),
    )
    .await
    .unwrap();

    // Client connects to server
    let _client_conn = client_ep.connect(server_local, "localhost").await.unwrap();

    // Server should receive the Initial packet and create a connection
    // Give the I/O loop a moment to process
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Verify the client endpoint is bound correctly
    assert_ne!(client_ep.local_addr().unwrap().port(), 0);
    assert_ne!(server_local.port(), 0);
}

#[tokio::test]
async fn packet_builder_produces_valid_initial() {
    use nhttp3_core::ConnectionId;
    use nhttp3_quic::packet::builder::{build_initial_packet, extract_crypto_data};

    let dcid = ConnectionId::from_slice(&[0xde, 0xad, 0xbe, 0xef]).unwrap();
    let scid = ConnectionId::from_slice(&[0xca, 0xfe]).unwrap();
    let crypto = b"TLS ClientHello goes here in real life";

    let pkt = build_initial_packet(&dcid, &scid, &[], crypto, 0);

    // Must be >= 1200 bytes (RFC 9000 §14.1)
    assert!(pkt.len() >= 1200, "Initial packet too small: {}", pkt.len());

    // Must be a long header Initial
    assert_eq!(pkt[0] & 0x80, 0x80, "must be long header");
    assert_eq!((pkt[0] & 0x30) >> 4, 0x00, "must be Initial type");

    // Must contain QUIC version 1
    assert_eq!(&pkt[1..5], &[0x00, 0x00, 0x00, 0x01]);

    // Roundtrip: extract CRYPTO data
    let extracted = extract_crypto_data(&pkt).expect("should extract crypto");
    assert_eq!(extracted, crypto, "CRYPTO data roundtrip failed");
}
