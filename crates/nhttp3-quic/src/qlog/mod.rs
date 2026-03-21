//! QLOG support for nhttp3 (draft-ietf-quic-qlog-main-schema).
//!
//! QLOG is a standardized logging format for QUIC and HTTP/3 that enables
//! interop debugging and performance analysis.

use std::io::Write;
use std::time::Instant;

/// QLOG event categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Connectivity,
    Transport,
    Security,
    Recovery,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connectivity => "connectivity",
            Self::Transport => "transport",
            Self::Security => "security",
            Self::Recovery => "recovery",
        }
    }
}

/// QLOG event types.
#[derive(Debug, Clone)]
pub enum Event {
    // Connectivity
    ConnectionStarted { src_cid: String, dst_cid: String },
    ConnectionStateUpdated { old: String, new: String },
    ConnectionClosed { reason: String },

    // Transport
    PacketSent { packet_type: String, size: usize },
    PacketReceived { packet_type: String, size: usize },
    FrameParsed { frame_type: String },
    StreamStateUpdated { stream_id: u64, old: String, new: String },

    // Security
    KeyUpdated { key_type: String, generation: u64 },

    // Recovery
    PacketLost { packet_number: u64 },
    CongestionStateUpdated { old: String, new: String, window: u64 },
    MetricsUpdated { rtt: f64, cwnd: u64, bytes_in_flight: u64 },
}

/// QLOG trace writer.
pub struct QlogWriter {
    events: Vec<(f64, Category, Event)>,
    start: Instant,
    enabled: bool,
}

impl QlogWriter {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            start: Instant::now(),
            enabled: true,
        }
    }

    pub fn disabled() -> Self {
        Self {
            events: Vec::new(),
            start: Instant::now(),
            enabled: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Records an event with the current timestamp.
    pub fn log(&mut self, category: Category, event: Event) {
        if !self.enabled {
            return;
        }
        let elapsed = self.start.elapsed().as_secs_f64() * 1000.0; // ms
        self.events.push((elapsed, category, event));
    }

    /// Returns the number of recorded events.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Writes events as JSON Lines format to the given writer.
    pub fn write_jsonl<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        for (time_ms, category, event) in &self.events {
            let event_str = match event {
                Event::ConnectionStarted { src_cid, dst_cid } => {
                    format!(r#"{{"time":{},"category":"{}","type":"connection_started","data":{{"src_cid":"{}","dst_cid":"{}"}}}}"#,
                        time_ms, category.as_str(), src_cid, dst_cid)
                }
                Event::ConnectionStateUpdated { old, new } => {
                    format!(r#"{{"time":{},"category":"{}","type":"connection_state_updated","data":{{"old":"{}","new":"{}"}}}}"#,
                        time_ms, category.as_str(), old, new)
                }
                Event::ConnectionClosed { reason } => {
                    format!(r#"{{"time":{},"category":"{}","type":"connection_closed","data":{{"reason":"{}"}}}}"#,
                        time_ms, category.as_str(), reason)
                }
                Event::PacketSent { packet_type, size } => {
                    format!(r#"{{"time":{},"category":"{}","type":"packet_sent","data":{{"packet_type":"{}","size":{}}}}}"#,
                        time_ms, category.as_str(), packet_type, size)
                }
                Event::PacketReceived { packet_type, size } => {
                    format!(r#"{{"time":{},"category":"{}","type":"packet_received","data":{{"packet_type":"{}","size":{}}}}}"#,
                        time_ms, category.as_str(), packet_type, size)
                }
                Event::FrameParsed { frame_type } => {
                    format!(r#"{{"time":{},"category":"{}","type":"frame_parsed","data":{{"frame_type":"{}"}}}}"#,
                        time_ms, category.as_str(), frame_type)
                }
                Event::StreamStateUpdated { stream_id, old, new } => {
                    format!(r#"{{"time":{},"category":"{}","type":"stream_state_updated","data":{{"stream_id":{},"old":"{}","new":"{}"}}}}"#,
                        time_ms, category.as_str(), stream_id, old, new)
                }
                Event::KeyUpdated { key_type, generation } => {
                    format!(r#"{{"time":{},"category":"{}","type":"key_updated","data":{{"key_type":"{}","generation":{}}}}}"#,
                        time_ms, category.as_str(), key_type, generation)
                }
                Event::PacketLost { packet_number } => {
                    format!(r#"{{"time":{},"category":"{}","type":"packet_lost","data":{{"packet_number":{}}}}}"#,
                        time_ms, category.as_str(), packet_number)
                }
                Event::CongestionStateUpdated { old, new, window } => {
                    format!(r#"{{"time":{},"category":"{}","type":"congestion_state_updated","data":{{"old":"{}","new":"{}","window":{}}}}}"#,
                        time_ms, category.as_str(), old, new, window)
                }
                Event::MetricsUpdated { rtt, cwnd, bytes_in_flight } => {
                    format!(r#"{{"time":{},"category":"{}","type":"metrics_updated","data":{{"rtt":{},"cwnd":{},"bytes_in_flight":{}}}}}"#,
                        time_ms, category.as_str(), rtt, cwnd, bytes_in_flight)
                }
            };
            writeln!(w, "{}", event_str)?;
        }
        Ok(())
    }
}

impl Default for QlogWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_events() {
        let mut qlog = QlogWriter::new();
        qlog.log(Category::Connectivity, Event::ConnectionStarted {
            src_cid: "01020304".into(),
            dst_cid: "05060708".into(),
        });
        qlog.log(Category::Transport, Event::PacketSent {
            packet_type: "initial".into(),
            size: 1200,
        });
        assert_eq!(qlog.event_count(), 2);
    }

    #[test]
    fn disabled_qlog_ignores_events() {
        let mut qlog = QlogWriter::disabled();
        qlog.log(Category::Transport, Event::PacketSent {
            packet_type: "initial".into(),
            size: 1200,
        });
        assert_eq!(qlog.event_count(), 0);
    }

    #[test]
    fn write_jsonl() {
        let mut qlog = QlogWriter::new();
        qlog.log(Category::Connectivity, Event::ConnectionClosed {
            reason: "idle_timeout".into(),
        });
        let mut buf = Vec::new();
        qlog.write_jsonl(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("connection_closed"));
        assert!(output.contains("idle_timeout"));
    }

    #[test]
    fn metrics_event() {
        let mut qlog = QlogWriter::new();
        qlog.log(Category::Recovery, Event::MetricsUpdated {
            rtt: 50.0,
            cwnd: 12000,
            bytes_in_flight: 4800,
        });
        let mut buf = Vec::new();
        qlog.write_jsonl(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("12000"));
    }
}
