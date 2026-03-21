use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::AsyncRead;
use tokio::sync::Notify;

use crate::connection::inner::ConnectionInner;

/// Receive side of a QUIC stream. Implements `tokio::io::AsyncRead`.
pub struct RecvStream {
    stream_id: u64,
    conn: Arc<Mutex<ConnectionInner>>,
    notify: Arc<Notify>,
}

impl RecvStream {
    pub fn new(stream_id: u64, conn: Arc<Mutex<ConnectionInner>>, notify: Arc<Notify>) -> Self {
        Self {
            stream_id,
            conn,
            notify,
        }
    }

    pub fn stream_id(&self) -> u64 {
        self.stream_id
    }
}

impl AsyncRead for RecvStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let mut conn = this.conn.lock().unwrap();
        let mut tmp = vec![0u8; buf.remaining()];
        let (n, fin) = conn.streams.read(this.stream_id, &mut tmp);
        drop(conn);

        if n > 0 {
            buf.put_slice(&tmp[..n]);
            Poll::Ready(Ok(()))
        } else if fin {
            Poll::Ready(Ok(())) // EOF
        } else {
            // Register waker for later notification
            let waker = cx.waker().clone();
            let notify = this.notify.clone();
            // Use a simple spawn to bridge notify → waker
            std::thread::spawn(move || {
                // Block on the notify in a new thread to avoid Send issues
                let rt = tokio::runtime::Handle::try_current();
                if let Ok(handle) = rt {
                    handle.block_on(async { notify.notified().await });
                }
                waker.wake();
            });
            Poll::Pending
        }
    }
}
