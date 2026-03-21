use rustls::quic::{self, DirectionalKeys, HeaderProtectionKey, PacketKey};

/// Encryption level / packet number space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {
    Initial,
    Handshake,
    ZeroRtt,
    OneRtt,
}

/// Keys for a single direction (send or receive) at a given encryption level.
pub struct DirectionKeys {
    pub packet: Box<dyn PacketKey>,
    pub header: Box<dyn HeaderProtectionKey>,
}

impl DirectionKeys {
    pub fn from_rustls(keys: DirectionalKeys) -> Self {
        Self {
            packet: keys.packet,
            header: keys.header,
        }
    }
}

/// Complete key set for a packet number space (both directions).
pub struct SpaceKeys {
    pub local: DirectionKeys,
    pub remote: DirectionKeys,
}

impl SpaceKeys {
    pub fn from_rustls(keys: quic::Keys) -> Self {
        Self {
            local: DirectionKeys::from_rustls(keys.local),
            remote: DirectionKeys::from_rustls(keys.remote),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_equality() {
        assert_eq!(Level::Initial, Level::Initial);
        assert_ne!(Level::Initial, Level::Handshake);
    }

    #[test]
    fn level_debug() {
        assert_eq!(format!("{:?}", Level::OneRtt), "OneRtt");
    }
}
