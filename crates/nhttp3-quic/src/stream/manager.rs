use std::collections::HashMap;

use super::state::StreamId;

/// Tracks all streams for a connection.
pub struct StreamManager {
    is_client: bool,
    next_bidi: u64,
    next_uni: u64,
    max_bidi: u64,
    max_uni: u64,
    send_buffers: HashMap<u64, Vec<u8>>,
    recv_buffers: HashMap<u64, Vec<u8>>,
    recv_fin: HashMap<u64, bool>,
}

impl StreamManager {
    pub fn new(is_client: bool, max_bidi: u64, max_uni: u64, _initial_stream_window: u64) -> Self {
        let (next_bidi, next_uni) = if is_client { (0, 2) } else { (1, 3) };
        Self {
            is_client,
            next_bidi,
            next_uni,
            max_bidi,
            max_uni,
            send_buffers: HashMap::new(),
            recv_buffers: HashMap::new(),
            recv_fin: HashMap::new(),
        }
    }

    pub fn open_bidi(&mut self) -> Option<StreamId> {
        if self.next_bidi / 4 >= self.max_bidi {
            return None;
        }
        let id = self.next_bidi;
        self.next_bidi += 4;
        self.send_buffers.insert(id, Vec::new());
        self.recv_buffers.insert(id, Vec::new());
        self.recv_fin.insert(id, false);
        Some(StreamId::new(id))
    }

    pub fn open_uni(&mut self) -> Option<StreamId> {
        if self.next_uni / 4 >= self.max_uni {
            return None;
        }
        let id = self.next_uni;
        self.next_uni += 4;
        self.send_buffers.insert(id, Vec::new());
        Some(StreamId::new(id))
    }

    pub fn on_stream_data(&mut self, stream_id: u64, data: &[u8], fin: bool) {
        let buf = self.recv_buffers.entry(stream_id).or_default();
        buf.extend_from_slice(data);
        if fin {
            self.recv_fin.insert(stream_id, true);
        }
    }

    pub fn read(&mut self, stream_id: u64, buf: &mut [u8]) -> (usize, bool) {
        let recv = match self.recv_buffers.get_mut(&stream_id) {
            Some(b) => b,
            None => return (0, false),
        };
        let n = std::cmp::min(buf.len(), recv.len());
        buf[..n].copy_from_slice(&recv[..n]);
        recv.drain(..n);
        let fin = recv.is_empty() && self.recv_fin.get(&stream_id).copied().unwrap_or(false);
        (n, fin)
    }

    pub fn write(&mut self, stream_id: u64, data: &[u8]) -> usize {
        let buf = match self.send_buffers.get_mut(&stream_id) {
            Some(b) => b,
            None => return 0,
        };
        buf.extend_from_slice(data);
        data.len()
    }

    pub fn drain_send(&mut self, stream_id: u64, max: usize) -> Vec<u8> {
        let buf = match self.send_buffers.get_mut(&stream_id) {
            Some(b) => b,
            None => return Vec::new(),
        };
        let n = std::cmp::min(max, buf.len());
        buf.drain(..n).collect()
    }

    pub fn streams_with_pending_data(&self) -> Vec<u64> {
        self.send_buffers
            .iter()
            .filter(|(_, buf)| !buf.is_empty())
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn is_client(&self) -> bool {
        self.is_client
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_bidi_client() {
        let mut mgr = StreamManager::new(true, 100, 100, 1_000_000);
        let s1 = mgr.open_bidi().unwrap();
        let s2 = mgr.open_bidi().unwrap();
        assert_eq!(s1.value(), 0);
        assert_eq!(s2.value(), 4);
        assert!(s1.is_client_initiated());
        assert!(s1.is_bidi());
    }

    #[test]
    fn open_bidi_server() {
        let mut mgr = StreamManager::new(false, 100, 100, 1_000_000);
        assert_eq!(mgr.open_bidi().unwrap().value(), 1);
    }

    #[test]
    fn open_uni_client() {
        let mut mgr = StreamManager::new(true, 100, 100, 1_000_000);
        let s = mgr.open_uni().unwrap();
        assert_eq!(s.value(), 2);
        assert!(s.is_uni());
    }

    #[test]
    fn stream_limit() {
        let mut mgr = StreamManager::new(true, 2, 1, 1_000_000);
        assert!(mgr.open_bidi().is_some());
        assert!(mgr.open_bidi().is_some());
        assert!(mgr.open_bidi().is_none());
    }

    #[test]
    fn write_and_drain() {
        let mut mgr = StreamManager::new(true, 100, 100, 1_000_000);
        let sid = mgr.open_bidi().unwrap().value();
        mgr.write(sid, b"hello");
        assert_eq!(mgr.drain_send(sid, 1024), b"hello");
    }

    #[test]
    fn receive_data() {
        let mut mgr = StreamManager::new(true, 100, 100, 1_000_000);
        mgr.on_stream_data(1, b"world", false);
        let mut buf = [0u8; 10];
        let (n, fin) = mgr.read(1, &mut buf);
        assert_eq!(&buf[..n], b"world");
        assert!(!fin);
    }

    #[test]
    fn receive_fin() {
        let mut mgr = StreamManager::new(true, 100, 100, 1_000_000);
        mgr.on_stream_data(1, b"done", true);
        let mut buf = [0u8; 10];
        let (n, fin) = mgr.read(1, &mut buf);
        assert_eq!(n, 4);
        assert!(fin);
    }

    #[test]
    fn pending_data() {
        let mut mgr = StreamManager::new(true, 100, 100, 1_000_000);
        let s1 = mgr.open_bidi().unwrap();
        let _s2 = mgr.open_bidi().unwrap();
        mgr.write(s1.value(), b"a");
        let pending = mgr.streams_with_pending_data();
        assert_eq!(pending, vec![s1.value()]);
    }
}
