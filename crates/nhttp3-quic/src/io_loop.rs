use std::sync::{Arc, Mutex};

use nhttp3_core::ConnectionId;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::connection::id_map::CidMap;
use crate::connection::inner::ConnectionInner;
use crate::endpoint::Connection;
use crate::packet::Header;
use crate::tls::TlsSession;

/// Runs the background I/O loop for the endpoint.
pub async fn run(
    socket: Arc<UdpSocket>,
    connections: Arc<Mutex<CidMap<ConnectionInner>>>,
    accept_tx: mpsc::Sender<Connection>,
    server_tls_config: Option<Arc<rustls::ServerConfig>>,
    config: Config,
) {
    let mut buf = vec![0u8; 65535];

    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, addr)) => {
                handle_packet(
                    &buf[..len],
                    addr,
                    &socket,
                    &connections,
                    &accept_tx,
                    &server_tls_config,
                    &config,
                )
                .await;
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::ConnectionReset {
                    continue;
                }
                break;
            }
        }
    }
}

async fn handle_packet(
    data: &[u8],
    addr: std::net::SocketAddr,
    socket: &Arc<UdpSocket>,
    connections: &Arc<Mutex<CidMap<ConnectionInner>>>,
    accept_tx: &mpsc::Sender<Connection>,
    server_tls_config: &Option<Arc<rustls::ServerConfig>>,
    config: &Config,
) {
    if data.is_empty() {
        return;
    }

    // Security: validate Initial packet minimum size (RFC 9000 §14.1)
    if !crate::packet::validate_initial_packet_size(data) {
        return; // Drop undersized Initial packets
    }

    let first_byte = data[0];
    let is_long = Header::is_long_header(first_byte);

    let dcid = if is_long {
        if data.len() < 6 {
            return;
        }
        let dcid_len = data[5] as usize;
        if data.len() < 6 + dcid_len {
            return;
        }
        match ConnectionId::from_slice(&data[6..6 + dcid_len]) {
            Ok(cid) => cid,
            Err(_) => return,
        }
    } else {
        if data.len() < 9 {
            return;
        }
        match ConnectionId::from_slice(&data[1..9]) {
            Ok(cid) => cid,
            Err(_) => return,
        }
    };

    let conn = connections.lock().unwrap().get(&dcid);

    if let Some(conn) = conn {
        let transmits = {
            let mut inner = conn.lock().unwrap();
            let _ = inner.on_handshake_data(data);
            inner.dirty = true;
            inner.poll_transmit()
        }; // MutexGuard dropped here

        for t in transmits {
            let _ = socket.send_to(&t.data, t.addr).await;
        }
    } else if is_long && (first_byte & 0x30) >> 4 == 0x00 {
        if let Some(server_tls) = server_tls_config {
            let local_cid = dcid.clone();
            let remote_cid = if data.len() >= 6 + data[5] as usize + 1 {
                let scid_offset = 6 + data[5] as usize;
                if data.len() > scid_offset {
                    let scid_len = data[scid_offset] as usize;
                    if data.len() >= scid_offset + 1 + scid_len {
                        ConnectionId::from_slice(&data[scid_offset + 1..scid_offset + 1 + scid_len])
                            .unwrap_or_else(|_| ConnectionId::empty())
                    } else {
                        ConnectionId::empty()
                    }
                } else {
                    ConnectionId::empty()
                }
            } else {
                ConnectionId::empty()
            };

            let tls = match TlsSession::new_server(server_tls.clone(), vec![]) {
                Ok(t) => t,
                Err(_) => return,
            };

            let mut conn_inner =
                ConnectionInner::new(local_cid.clone(), remote_cid, addr, tls, config.clone(), false);

            let _ = conn_inner.on_handshake_data(data);
            let transmits = conn_inner.poll_transmit();

            let inner = Arc::new(Mutex::new(conn_inner));
            let notify = Arc::new(tokio::sync::Notify::new());

            connections
                .lock()
                .unwrap()
                .insert(&local_cid, inner.clone());

            for t in transmits {
                let _ = socket.send_to(&t.data, t.addr).await;
            }

            let conn = Connection::new(inner, notify);
            let _ = accept_tx.send(conn).await;
        }
    }
}
