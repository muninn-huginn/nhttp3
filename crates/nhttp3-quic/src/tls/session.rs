use std::sync::Arc;

use rustls::pki_types::ServerName;
use rustls::quic::{self, Connection as TlsConnection, KeyChange, Version as TlsVersion};
use rustls::{ClientConfig, ServerConfig};

use crate::crypto::SpaceKeys;
use crate::packet::PacketError;

/// Wraps a rustls QUIC connection (client or server).
pub struct TlsSession {
    conn: TlsConnection,
}

/// Result of processing TLS handshake data.
pub struct HandshakeResult {
    /// Outgoing handshake data (all levels combined).
    pub data: Vec<u8>,
    /// New keys, if a key change occurred.
    pub key_change: Option<KeyChangeEvent>,
}

pub enum KeyChangeEvent {
    Handshake(SpaceKeys),
    OneRtt {
        keys: SpaceKeys,
        next_secrets: quic::Secrets,
    },
}

impl TlsSession {
    /// Creates a new client TLS session.
    pub fn new_client(
        config: Arc<ClientConfig>,
        server_name: ServerName<'static>,
        transport_params: Vec<u8>,
    ) -> Result<Self, PacketError> {
        let conn =
            quic::ClientConnection::new(config, TlsVersion::V1, server_name, transport_params)
                .map_err(|e| PacketError::Invalid(format!("TLS client init failed: {e}")))?;

        Ok(Self {
            conn: TlsConnection::Client(conn),
        })
    }

    /// Creates a new server TLS session.
    pub fn new_server(
        config: Arc<ServerConfig>,
        transport_params: Vec<u8>,
    ) -> Result<Self, PacketError> {
        let conn = quic::ServerConnection::new(config, TlsVersion::V1, transport_params)
            .map_err(|e| PacketError::Invalid(format!("TLS server init failed: {e}")))?;

        Ok(Self {
            conn: TlsConnection::Server(conn),
        })
    }

    /// Feeds received handshake data into the TLS session.
    pub fn read_handshake(&mut self, data: &[u8]) -> Result<(), PacketError> {
        match &mut self.conn {
            TlsConnection::Client(c) => c.read_hs(data),
            TlsConnection::Server(c) => c.read_hs(data),
        }
        .map_err(|e| PacketError::Invalid(format!("TLS read_hs failed: {e}")))
    }

    /// Gets outgoing handshake data and any key changes.
    pub fn write_handshake(&mut self) -> HandshakeResult {
        let mut buf = Vec::new();

        let key_change = match &mut self.conn {
            TlsConnection::Client(c) => c.write_hs(&mut buf),
            TlsConnection::Server(c) => c.write_hs(&mut buf),
        };

        let key_change = key_change.map(|kc| match kc {
            KeyChange::Handshake { keys } => {
                KeyChangeEvent::Handshake(SpaceKeys::from_rustls(keys))
            }
            KeyChange::OneRtt { keys, next } => KeyChangeEvent::OneRtt {
                keys: SpaceKeys::from_rustls(keys),
                next_secrets: next,
            },
        });

        HandshakeResult { data: buf, key_change }
    }

    /// Returns the peer's transport parameters (TLS-encoded).
    pub fn transport_parameters(&self) -> Option<&[u8]> {
        match &self.conn {
            TlsConnection::Client(c) => c.quic_transport_parameters(),
            TlsConnection::Server(c) => c.quic_transport_parameters(),
        }
    }

    /// Returns true if the handshake is still in progress.
    pub fn is_handshaking(&self) -> bool {
        match &self.conn {
            TlsConnection::Client(c) => c.is_handshaking(),
            TlsConnection::Server(c) => c.is_handshaking(),
        }
    }

    /// Returns the negotiated ALPN protocol.
    pub fn alpn_protocol(&self) -> Option<&[u8]> {
        match &self.conn {
            TlsConnection::Client(c) => c.alpn_protocol(),
            TlsConnection::Server(c) => c.alpn_protocol(),
        }
    }

    /// Gets 0-RTT keys if available.
    pub fn zero_rtt_keys(&self) -> Option<quic::DirectionalKeys> {
        match &self.conn {
            TlsConnection::Client(c) => c.zero_rtt_keys(),
            TlsConnection::Server(c) => c.zero_rtt_keys(),
        }
    }

    /// Returns the TLS alert if the handshake failed.
    pub fn alert(&self) -> Option<rustls::AlertDescription> {
        match &self.conn {
            TlsConnection::Client(c) => c.alert(),
            TlsConnection::Server(c) => c.alert(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn client_server_handshake_in_process() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let (cert, key) = self_signed_cert();

        let mut client_config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
            .with_no_client_auth();
        client_config.alpn_protocols = vec![b"h3".to_vec()];

        let mut server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap();
        server_config.alpn_protocols = vec![b"h3".to_vec()];

        let server_name: ServerName<'static> = "localhost".try_into().unwrap();
        let mut client =
            TlsSession::new_client(Arc::new(client_config), server_name, vec![]).unwrap();
        let mut server = TlsSession::new_server(Arc::new(server_config), vec![]).unwrap();

        // Client sends ClientHello
        let ch = client.write_handshake();
        assert!(!ch.data.is_empty(), "client should produce Initial data");

        // Server processes ClientHello and responds
        server.read_handshake(&ch.data).unwrap();
        let sh = server.write_handshake();
        assert!(
            sh.key_change.is_some(),
            "server should produce handshake keys"
        );
        assert!(!sh.data.is_empty(), "server should produce handshake data");

        // Client processes server handshake
        client.read_handshake(&sh.data).unwrap();
        let cf = client.write_handshake();

        // Feed client's finished back to server
        if !cf.data.is_empty() {
            server.read_handshake(&cf.data).unwrap();
            let sf = server.write_handshake();
            // Server may produce 1-RTT keys here
            let _ = sf;
        }

        assert!(
            !client.is_handshaking() || cf.key_change.is_some(),
            "handshake should complete or produce 1-RTT keys"
        );
    }
}
