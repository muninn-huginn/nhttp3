use nhttp3_core::VarInt;

/// Handles STOP_SENDING → RESET_STREAM protocol interaction (RFC 9000 §3.5).
///
/// When a receiver sends STOP_SENDING, the sender MUST respond with RESET_STREAM
/// using the correct final_size (the total number of bytes sent on the stream,
/// including those in unacked STREAM frames).
///
/// Bug reference: aioquic #629 — STOP_SENDING triggers RESET_STREAM with
/// invalid final_size and locked error code.
#[derive(Debug)]
pub struct ResetState {
    /// Total bytes sent on this stream (including unacked).
    bytes_sent: u64,
    /// Whether RESET_STREAM has been sent.
    reset_sent: bool,
    /// The error code used in RESET_STREAM.
    reset_error_code: Option<VarInt>,
}

impl ResetState {
    pub fn new() -> Self {
        Self {
            bytes_sent: 0,
            reset_sent: false,
            reset_error_code: None,
        }
    }

    /// Records bytes written to the stream.
    pub fn on_bytes_sent(&mut self, n: u64) {
        self.bytes_sent += n;
    }

    /// Generates a RESET_STREAM response to a STOP_SENDING frame.
    /// Returns (error_code, final_size) for the RESET_STREAM frame.
    pub fn on_stop_sending(&mut self, error_code: VarInt) -> (VarInt, VarInt) {
        self.reset_sent = true;
        self.reset_error_code = Some(error_code);
        (
            error_code,
            VarInt::try_from(self.bytes_sent).unwrap_or(VarInt::from_u32(0)),
        )
    }

    /// Generates a RESET_STREAM for application-initiated reset.
    pub fn reset(&mut self, error_code: VarInt) -> (VarInt, VarInt) {
        self.reset_sent = true;
        self.reset_error_code = Some(error_code);
        (
            error_code,
            VarInt::try_from(self.bytes_sent).unwrap_or(VarInt::from_u32(0)),
        )
    }

    pub fn is_reset(&self) -> bool {
        self.reset_sent
    }

    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent
    }
}

impl Default for ResetState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_sending_produces_correct_final_size() {
        let mut state = ResetState::new();
        state.on_bytes_sent(1000);
        state.on_bytes_sent(500);

        let (code, final_size) = state.on_stop_sending(VarInt::from_u32(0x42));
        assert_eq!(code.value(), 0x42);
        assert_eq!(final_size.value(), 1500); // total bytes sent
        assert!(state.is_reset());
    }

    #[test]
    fn reset_produces_correct_final_size() {
        let mut state = ResetState::new();
        state.on_bytes_sent(2048);

        let (code, final_size) = state.reset(VarInt::from_u32(0x00));
        assert_eq!(final_size.value(), 2048);
    }

    #[test]
    fn no_data_sent_final_size_zero() {
        let mut state = ResetState::new();
        let (_, final_size) = state.on_stop_sending(VarInt::from_u32(0x01));
        assert_eq!(final_size.value(), 0);
    }
}
