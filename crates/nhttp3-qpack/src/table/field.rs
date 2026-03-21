/// A header field (name-value pair).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderField {
    pub name: Vec<u8>,
    pub value: Vec<u8>,
    pub sensitive: bool,
}

impl HeaderField {
    pub fn new(name: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            sensitive: false,
        }
    }

    pub fn sensitive(name: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            sensitive: true,
        }
    }

    /// Returns the size of this field for dynamic table accounting.
    /// RFC 9204 §3.2.1: size = len(name) + len(value) + 32
    pub fn size(&self) -> usize {
        self.name.len() + self.value.len() + 32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_size() {
        let f = HeaderField::new("content-type", "text/html");
        assert_eq!(f.size(), 12 + 9 + 32);
    }

    #[test]
    fn sensitive_field() {
        let f = HeaderField::sensitive("authorization", "Bearer tok");
        assert!(f.sensitive);
    }
}
