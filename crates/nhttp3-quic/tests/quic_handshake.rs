//! Integration test: QUIC handshake + frame roundtrip + cross-layer validation.

use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use nhttp3_core::{ConnectionId, VarInt};
use nhttp3_quic::config::Config;
use nhttp3_quic::frame::Frame;
use nhttp3_quic::packet::Header;
use nhttp3_quic::tls::TlsSession;
use nhttp3_quic::transport::TransportParams;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};

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

#[test]
fn tls_handshake_produces_keys() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let (cert, key) = self_signed_cert();

    let mut client_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
        .with_no_client_auth();
    client_config.alpn_protocols = vec![b"h3".to_vec()];

    let mut server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap();
    server_config.alpn_protocols = vec![b"h3".to_vec()];

    // Encode transport params
    let config = Config::default();
    let mut tp_buf = BytesMut::new();
    let tp = TransportParams {
        initial_max_data: config.initial_max_data,
        initial_max_streams_bidi: config.initial_max_streams_bidi,
        initial_max_streams_uni: config.initial_max_streams_uni,
        initial_max_stream_data_bidi_local: config.initial_max_stream_data_bidi_local,
        initial_max_stream_data_bidi_remote: config.initial_max_stream_data_bidi_remote,
        initial_max_stream_data_uni: config.initial_max_stream_data_uni,
        ..Default::default()
    };
    tp.encode(&mut tp_buf);

    let server_name: ServerName<'static> = "localhost".try_into().unwrap();
    let mut client = TlsSession::new_client(
        Arc::new(client_config),
        server_name,
        tp_buf.to_vec(),
    )
    .unwrap();
    let mut server =
        TlsSession::new_server(Arc::new(server_config), tp_buf.to_vec()).unwrap();

    // Drive handshake
    let ch = client.write_handshake();
    assert!(!ch.data.is_empty(), "ClientHello should be produced");

    server.read_handshake(&ch.data).unwrap();
    let sh = server.write_handshake();
    assert!(sh.key_change.is_some(), "server should produce handshake keys");

    client.read_handshake(&sh.data).unwrap();
    let cf = client.write_handshake();

    if !cf.data.is_empty() {
        server.read_handshake(&cf.data).unwrap();
        let _ = server.write_handshake();
    }

    assert!(
        !client.is_handshaking() || cf.key_change.is_some(),
        "handshake should complete or produce 1-RTT keys"
    );
}

#[test]
fn varint_roundtrip_exhaustive() {
    let test_values: Vec<u64> = vec![
        0, 1, 62, 63, 64, 65, 16382, 16383, 16384, 16385, 1_073_741_822, 1_073_741_823,
        1_073_741_824, 1_073_741_825, 4_611_686_018_427_387_902, 4_611_686_018_427_387_903,
    ];

    for val in test_values {
        let v = VarInt::try_from(val).unwrap();
        let mut buf = BytesMut::new();
        v.encode(&mut buf);
        let mut bytes = buf.freeze();
        let decoded = VarInt::decode(&mut bytes).unwrap();
        assert_eq!(v, decoded, "roundtrip failed for {val}");
    }
}

#[test]
fn frame_roundtrip_all_types() {
    let frames = vec![
        Frame::Padding,
        Frame::Ping,
        Frame::Ack {
            largest_ack: VarInt::from_u32(100),
            ack_delay: VarInt::from_u32(10),
            first_ack_range: VarInt::from_u32(5),
            ack_ranges: vec![],
            ecn: None,
        },
        Frame::Crypto {
            offset: VarInt::from_u32(0),
            data: b"test crypto data".to_vec(),
        },
        Frame::Stream {
            stream_id: VarInt::from_u32(0),
            offset: Some(VarInt::from_u32(0)),
            data: b"hello".to_vec(),
            fin: false,
        },
        Frame::MaxData {
            max_data: VarInt::from_u32(1_000_000),
        },
        Frame::MaxStreamData {
            stream_id: VarInt::from_u32(4),
            max_data: VarInt::from_u32(500_000),
        },
        Frame::ConnectionClose {
            error_code: VarInt::from_u32(0x0a),
            frame_type: Some(VarInt::from_u32(0x06)),
            reason: b"test close".to_vec(),
        },
        Frame::HandshakeDone,
    ];

    for frame in &frames {
        let mut buf = BytesMut::new();
        frame.encode(&mut buf);
        let mut bytes = buf.freeze();
        let parsed = Frame::parse(&mut bytes).unwrap();
        assert_eq!(*frame, parsed, "roundtrip failed for {frame:?}");
    }
}

#[test]
fn packet_header_initial_parse() {
    let data = vec![
        0xc0, // Initial long header
        0x00, 0x00, 0x00, 0x01, // Version 1
        0x08, // DCID length
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // DCID
        0x00, // SCID length
        0x00, // Token length
        0x10, // Payload length (16)
    ];
    let mut buf = Bytes::from(data);
    let header = Header::parse(&mut buf, 0).unwrap();

    match header {
        Header::Long(h) => {
            assert_eq!(h.dcid.len(), 8);
            assert_eq!(h.payload_length, 16);
            assert_eq!(h.pn_offset, 17); // 1+4+1+8+1+1(token_len)+1(payload_len)
        }
        _ => panic!("expected long header"),
    }
}

#[test]
fn transport_params_roundtrip() {
    let params = TransportParams {
        max_idle_timeout: std::time::Duration::from_secs(60),
        initial_max_data: 5_000_000,
        initial_max_stream_data_bidi_local: 500_000,
        initial_max_stream_data_bidi_remote: 500_000,
        initial_max_stream_data_uni: 500_000,
        initial_max_streams_bidi: 200,
        initial_max_streams_uni: 200,
        active_connection_id_limit: 4,
        disable_active_migration: true,
        initial_source_connection_id: Some(ConnectionId::from_slice(&[0xde, 0xad]).unwrap()),
        ..Default::default()
    };

    let mut buf = BytesMut::new();
    params.encode(&mut buf);
    let mut bytes = buf.freeze();
    let decoded = TransportParams::decode(&mut bytes).unwrap();

    assert_eq!(
        decoded.max_idle_timeout,
        std::time::Duration::from_secs(60)
    );
    assert_eq!(decoded.initial_max_data, 5_000_000);
    assert_eq!(decoded.initial_max_streams_bidi, 200);
    assert_eq!(decoded.active_connection_id_limit, 4);
    assert!(decoded.disable_active_migration);
    assert_eq!(
        decoded.initial_source_connection_id.unwrap().as_bytes(),
        &[0xde, 0xad]
    );
}

#[test]
fn congestion_control_lifecycle() {
    use nhttp3_quic::recovery::{CongestionController, NewReno};
    use std::time::{Duration, Instant};

    let mut cc = NewReno::new();
    let now = Instant::now();

    // Initial state
    assert_eq!(cc.window(), 12000); // RFC 9002 §7.2
    assert!(cc.can_send());

    // Send and ack — slow start growth
    cc.on_packet_sent(1200);
    let window_before = cc.window();
    cc.on_ack(1200, Duration::from_millis(50), now);
    assert!(cc.window() > window_before, "slow start should grow");

    // Loss — window halves
    cc.on_packet_sent(1200);
    let window_before_loss = cc.window();
    cc.on_loss(1200, now);
    assert!(cc.window() < window_before_loss, "loss should reduce window");
    assert!(cc.window() >= 2400, "window should not go below minimum");
}
