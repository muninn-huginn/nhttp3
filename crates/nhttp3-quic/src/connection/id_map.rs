use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nhttp3_core::ConnectionId;

/// Maps Connection IDs to connection handles.
pub struct CidMap<T> {
    map: HashMap<Vec<u8>, Arc<Mutex<T>>>,
}

impl<T> CidMap<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, cid: &ConnectionId, conn: Arc<Mutex<T>>) {
        self.map.insert(cid.as_bytes().to_vec(), conn);
    }

    pub fn get(&self, cid: &ConnectionId) -> Option<Arc<Mutex<T>>> {
        self.map.get(cid.as_bytes()).cloned()
    }

    pub fn remove(&mut self, cid: &ConnectionId) -> bool {
        self.map.remove(cid.as_bytes()).is_some()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl<T> Default for CidMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut map: CidMap<u32> = CidMap::new();
        let cid = ConnectionId::from_slice(&[1, 2, 3, 4]).unwrap();
        let conn = Arc::new(Mutex::new(42u32));
        map.insert(&cid, conn);
        assert_eq!(*map.get(&cid).unwrap().lock().unwrap(), 42);
    }

    #[test]
    fn get_missing() {
        let map: CidMap<u32> = CidMap::new();
        let cid = ConnectionId::from_slice(&[1, 2, 3, 4]).unwrap();
        assert!(map.get(&cid).is_none());
    }

    #[test]
    fn remove() {
        let mut map: CidMap<u32> = CidMap::new();
        let cid = ConnectionId::from_slice(&[1, 2]).unwrap();
        map.insert(&cid, Arc::new(Mutex::new(1)));
        assert!(map.remove(&cid));
        assert!(map.get(&cid).is_none());
    }

    #[test]
    fn multiple_cids_same_connection() {
        let mut map: CidMap<u32> = CidMap::new();
        let conn = Arc::new(Mutex::new(99u32));
        let cid1 = ConnectionId::from_slice(&[1]).unwrap();
        let cid2 = ConnectionId::from_slice(&[2]).unwrap();
        map.insert(&cid1, conn.clone());
        map.insert(&cid2, conn);
        assert!(Arc::ptr_eq(
            &map.get(&cid1).unwrap(),
            &map.get(&cid2).unwrap()
        ));
        assert_eq!(map.len(), 2);
    }
}
