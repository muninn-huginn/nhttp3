use std::collections::VecDeque;

use super::field::HeaderField;

/// QPACK dynamic table (RFC 9204 §3.2).
#[derive(Debug)]
pub struct DynamicTable {
    entries: VecDeque<HeaderField>,
    size: usize,
    capacity: usize,
    /// Absolute index of the first entry in the table.
    base: usize,
    /// Total number of entries ever inserted.
    total_inserted: usize,
}

impl DynamicTable {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            size: 0,
            capacity,
            base: 0,
            total_inserted: 0,
        }
    }

    /// Sets the dynamic table capacity, evicting entries if needed.
    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
        self.evict();
    }

    /// Inserts a new entry at the front (newest).
    pub fn insert(&mut self, field: HeaderField) {
        let entry_size = field.size();
        // Evict until there's room
        while self.size + entry_size > self.capacity && !self.entries.is_empty() {
            self.evict_one();
        }
        if self.size + entry_size <= self.capacity {
            self.size += entry_size;
            self.entries.push_front(field);
            self.total_inserted += 1;
        }
    }

    /// Gets an entry by absolute index.
    pub fn get_absolute(&self, abs_index: usize) -> Option<&HeaderField> {
        if abs_index < self.base || abs_index >= self.base + self.entries.len() {
            return None;
        }
        let relative = self.base + self.entries.len() - 1 - abs_index;
        self.entries.get(relative)
    }

    /// Gets an entry by relative index (0 = most recently inserted).
    pub fn get_relative(&self, rel_index: usize) -> Option<&HeaderField> {
        self.entries.get(rel_index)
    }

    /// Find a matching entry. Returns (absolute_index, has_value_match).
    pub fn find(&self, name: &[u8], value: &[u8]) -> Option<(usize, bool)> {
        let mut name_match = None;

        for (i, entry) in self.entries.iter().enumerate() {
            let abs_index = self.base + self.entries.len() - 1 - i;
            if entry.name == name {
                if entry.value == value {
                    return Some((abs_index, true));
                }
                if name_match.is_none() {
                    name_match = Some(abs_index);
                }
            }
        }

        name_match.map(|idx| (idx, false))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn total_inserted(&self) -> usize {
        self.total_inserted
    }

    fn evict(&mut self) {
        while self.size > self.capacity && !self.entries.is_empty() {
            self.evict_one();
        }
    }

    fn evict_one(&mut self) {
        if let Some(entry) = self.entries.pop_back() {
            self.size -= entry.size();
            self.base += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut dt = DynamicTable::new(4096);
        dt.insert(HeaderField::new("foo", "bar"));
        assert_eq!(dt.len(), 1);
        assert_eq!(dt.get_relative(0).unwrap().name, b"foo");
    }

    #[test]
    fn absolute_index() {
        let mut dt = DynamicTable::new(4096);
        dt.insert(HeaderField::new("first", "1"));
        dt.insert(HeaderField::new("second", "2"));
        // first inserted has abs index 0, second has abs index 1
        assert_eq!(dt.get_absolute(0).unwrap().name, b"first");
        assert_eq!(dt.get_absolute(1).unwrap().name, b"second");
    }

    #[test]
    fn eviction_on_capacity() {
        let field_size = HeaderField::new("x", "y").size(); // 1+1+32 = 34
        let mut dt = DynamicTable::new(field_size * 2); // room for 2
        dt.insert(HeaderField::new("a", "1"));
        dt.insert(HeaderField::new("b", "2"));
        assert_eq!(dt.len(), 2);
        // Third insert should evict oldest
        dt.insert(HeaderField::new("c", "3"));
        assert_eq!(dt.len(), 2);
        assert!(dt.get_absolute(0).is_none()); // evicted
        assert!(dt.get_absolute(1).is_some());
        assert!(dt.get_absolute(2).is_some());
    }

    #[test]
    fn set_capacity_evicts() {
        let mut dt = DynamicTable::new(4096);
        dt.insert(HeaderField::new("foo", "bar"));
        dt.insert(HeaderField::new("baz", "qux"));
        dt.set_capacity(0);
        assert!(dt.is_empty());
    }

    #[test]
    fn find_entry() {
        let mut dt = DynamicTable::new(4096);
        dt.insert(HeaderField::new("content-type", "text/html"));
        dt.insert(HeaderField::new("content-type", "application/json"));

        let (idx, exact) = dt.find(b"content-type", b"application/json").unwrap();
        assert!(exact);
        assert_eq!(idx, 1); // most recently inserted match

        let (_idx, exact) = dt.find(b"content-type", b"text/css").unwrap();
        assert!(!exact); // name match only
    }

    #[test]
    fn find_no_match() {
        let dt = DynamicTable::new(4096);
        assert!(dt.find(b"x-custom", b"val").is_none());
    }
}
