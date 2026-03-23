// nhttp3-core is not re-exported to avoid shadowing std::core.
// Consumers depend on nhttp3_core directly if needed.
pub use nhttp3_h3 as h3;
pub use nhttp3_qpack as qpack;
pub use nhttp3_quic as quic;
