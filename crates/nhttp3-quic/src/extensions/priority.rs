/// HTTP/3 Extensible Priorities (RFC 9218).
///
/// Allows clients and servers to signal the relative importance
/// of HTTP/3 streams via the Priority header field and PRIORITY_UPDATE frames.

/// Priority urgency level (0-7, default 3).
/// Lower values indicate higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Priority {
    /// Urgency level (0 = highest, 7 = lowest). Default: 3.
    pub urgency: u8,
    /// Whether the response can be delivered incrementally.
    /// Default: false.
    pub incremental: bool,
}

impl Default for Priority {
    fn default() -> Self {
        Self {
            urgency: 3,
            incremental: false,
        }
    }
}

impl Priority {
    pub fn new(urgency: u8, incremental: bool) -> Self {
        Self {
            urgency: urgency.min(7),
            incremental,
        }
    }

    /// Serializes to the Structured Fields format used in the Priority header.
    /// Example: "u=3, i" or "u=0"
    pub fn to_header_value(&self) -> String {
        if self.incremental {
            format!("u={}, i", self.urgency)
        } else {
            format!("u={}", self.urgency)
        }
    }

    /// Parses from a Priority header field value.
    pub fn from_header_value(value: &str) -> Self {
        let mut priority = Self::default();

        for param in value.split(',') {
            let param = param.trim();
            if let Some(val) = param.strip_prefix("u=") {
                if let Ok(u) = val.trim().parse::<u8>() {
                    priority.urgency = u.min(7);
                }
            } else if param == "i" {
                priority.incremental = true;
            }
        }

        priority
    }
}

/// PRIORITY_UPDATE frame type for request streams (RFC 9218 §7.1).
pub const PRIORITY_UPDATE_REQUEST: u64 = 0xf0700;
/// PRIORITY_UPDATE frame type for push streams (RFC 9218 §7.2).
pub const PRIORITY_UPDATE_PUSH: u64 = 0xf0701;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_priority() {
        let p = Priority::default();
        assert_eq!(p.urgency, 3);
        assert!(!p.incremental);
    }

    #[test]
    fn priority_clamped() {
        let p = Priority::new(10, false);
        assert_eq!(p.urgency, 7);
    }

    #[test]
    fn to_header_value_basic() {
        assert_eq!(Priority::default().to_header_value(), "u=3");
        assert_eq!(Priority::new(0, true).to_header_value(), "u=0, i");
    }

    #[test]
    fn from_header_value_basic() {
        let p = Priority::from_header_value("u=1, i");
        assert_eq!(p.urgency, 1);
        assert!(p.incremental);
    }

    #[test]
    fn from_header_value_urgency_only() {
        let p = Priority::from_header_value("u=5");
        assert_eq!(p.urgency, 5);
        assert!(!p.incremental);
    }

    #[test]
    fn from_header_value_empty() {
        let p = Priority::from_header_value("");
        assert_eq!(p, Priority::default());
    }

    #[test]
    fn roundtrip() {
        let p = Priority::new(2, true);
        let header = p.to_header_value();
        let parsed = Priority::from_header_value(&header);
        assert_eq!(p, parsed);
    }
}
