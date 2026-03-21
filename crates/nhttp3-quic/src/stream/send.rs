use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::AsyncWrite;
use tokio::sync::Notify;

use crate::connection::inner::ConnectionInner;

/// Send side of a QUIC stream. Implements `tokio::io::AsyncWrite`.
pub struct SendStream {
    stream_id: u64,
    conn: Arc<Mutex<ConnectionInner>>,
    notify: Arc<Notify>,
    finished: bool,
}

impl SendStream {
    pub fn new(stream_id: u64, conn: Arc<Mutex<ConnectionInner>>, notify: Arc<Notify>) -> Self {
        Self {
            stream_id,
            conn,
            notify,
            finished: false,
        }
    }

    pub fn stream_id(&self) -> u64 {
        self.stream_id
    }

    pub fn finish(&mut self) {
        self.finished = true;
        self.notify.notify_one();
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

impl AsyncWrite for SendStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        if this.finished {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "stream finished",
            )));
        }
        let mut conn = this.conn.lock().unwrap();
        let n = conn.streams.write(this.stream_id, buf);
        conn.dirty = true;
        drop(conn);
        this.notify.notify_one();
        Poll::Ready(Ok(n))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.get_mut().finish();
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::stream::manager::StreamManager;
    use tokio::io::AsyncWriteExt;

    // We can't easily unit test SendStream without ConnectionInner.
    // Tested via integration tests in endpoint_integration.rs.
}
