use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use nhttp3_core::ConnectionId;
use rustls::pki_types::ServerName;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::connection::id_map::CidMap;
use crate::connection::inner::{ConnectionInner, Transmit};
use crate::packet::PacketError;
use crate::stream::{RecvStream, SendStream};
use crate::tls::TlsSession;

/// User-facing QUIC connection handle.
#[derive(Clone)]
pub struct Connection {
    inner: Arc<Mutex<ConnectionInner>>,
    notify: Arc<tokio::sync::Notify>,
}

impl Connection {
    pub fn new(inner: Arc<Mutex<ConnectionInner>>, notify: Arc<tokio::sync::Notify>) -> Self {
        Self { inner, notify }
    }

    pub fn open_bidi_stream(&self) -> Option<(SendStream, RecvStream)> {
        let mut conn = self.inner.lock().unwrap();
        let sid = conn.streams.open_bidi()?;
        let send = SendStream::new(sid.value(), self.inner.clone(), self.notify.clone());
        let recv = RecvStream::new(sid.value(), self.inner.clone(), self.notify.clone());
        Some((send, recv))
    }

    pub fn open_uni_stream(&self) -> Option<SendStream> {
        let mut conn = self.inner.lock().unwrap();
        let sid = conn.streams.open_uni()?;
        Some(SendStream::new(
            sid.value(),
            self.inner.clone(),
            self.notify.clone(),
        ))
    }

    pub async fn established(&self) {
        loop {
            {
                if self.inner.lock().unwrap().is_established() {
                    return;
                }
            }
            self.notify.notified().await;
        }
    }

    pub fn is_established(&self) -> bool {
        self.inner.lock().unwrap().is_established()
    }
}

/// QUIC Endpoint — manages a UDP socket and multiple connections.
pub struct Endpoint {
    socket: Arc<UdpSocket>,
    config: Config,
    client_tls_config: Option<Arc<rustls::ClientConfig>>,
    #[allow(dead_code)]
    server_tls_config: Option<Arc<rustls::ServerConfig>>,
    connections: Arc<Mutex<CidMap<ConnectionInner>>>,
    accept_rx: mpsc::Receiver<Connection>,
}

impl Endpoint {
    pub async fn bind(
        addr: SocketAddr,
        config: Config,
        server_tls_config: Option<Arc<rustls::ServerConfig>>,
        client_tls_config: Option<Arc<rustls::ClientConfig>>,
    ) -> Result<Self, std::io::Error> {
        let socket = Arc::new(UdpSocket::bind(addr).await?);
        let (accept_tx, accept_rx) = mpsc::channel(256);
        let connections = Arc::new(Mutex::new(CidMap::new()));

        let io_socket = socket.clone();
        let io_conns = connections.clone();
        let io_accept_tx = accept_tx;
        let io_server_tls = server_tls_config.clone();
        let io_config = config.clone();

        tokio::spawn(async move {
            crate::io_loop::run(io_socket, io_conns, io_accept_tx, io_server_tls, io_config).await;
        });

        Ok(Self {
            socket,
            config,
            client_tls_config,
            server_tls_config,
            connections,
            accept_rx,
        })
    }

    pub async fn accept(&mut self) -> Option<Connection> {
        self.accept_rx.recv().await
    }

    pub async fn connect(
        &self,
        addr: SocketAddr,
        server_name: &str,
    ) -> Result<Connection, PacketError> {
        let tls_config = self
            .client_tls_config
            .as_ref()
            .ok_or_else(|| PacketError::Invalid("no client TLS config".into()))?;

        let local_cid = ConnectionId::from_slice(&rand_cid()).unwrap();
        let remote_cid = ConnectionId::from_slice(&rand_cid()).unwrap();

        let sni: ServerName<'static> = server_name
            .to_string()
            .try_into()
            .map_err(|_| PacketError::Invalid("invalid server name".into()))?;

        let tls = TlsSession::new_client(tls_config.clone(), sni, vec![])?;
        let mut conn_inner =
            ConnectionInner::new(local_cid.clone(), remote_cid, addr, tls, self.config.clone(), true);

        conn_inner.drive_handshake();
        let transmits = conn_inner.poll_transmit();
        for t in transmits {
            let _ = self.socket.send_to(&t.data, t.addr).await;
        }

        let inner = Arc::new(Mutex::new(conn_inner));
        let notify = Arc::new(tokio::sync::Notify::new());

        self.connections
            .lock()
            .unwrap()
            .insert(&local_cid, inner.clone());

        Ok(Connection::new(inner, notify))
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

fn rand_cid() -> [u8; 8] {
    // Use multiple entropy sources to avoid predictable/colliding CIDs
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = now.as_nanos() as u64;
    let ptr_entropy = &now as *const _ as u64;
    let thread_id = std::thread::current().id();
    let tid_hash = format!("{:?}", thread_id).len() as u64;

    // Mix entropy sources
    let mixed = nanos
        .wrapping_mul(6364136223846793005)
        .wrapping_add(ptr_entropy)
        .wrapping_mul(1442695040888963407)
        .wrapping_add(tid_hash);

    // Add a monotonic counter to prevent same-nanosecond collisions
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    (mixed.wrapping_add(count)).to_le_bytes()
}
