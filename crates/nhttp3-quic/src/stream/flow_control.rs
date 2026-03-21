/// Tracks flow control state for a stream or connection.
#[derive(Debug)]
pub struct FlowControl {
    window: u64,
    consumed: u64,
}

impl FlowControl {
    pub fn new(initial_window: u64) -> Self {
        Self {
            window: initial_window,
            consumed: 0,
        }
    }

    pub fn available(&self) -> u64 {
        self.window.saturating_sub(self.consumed)
    }

    pub fn consume(&mut self, n: u64) -> bool {
        let new_consumed = self.consumed + n;
        if new_consumed > self.window {
            return false;
        }
        self.consumed = new_consumed;
        true
    }

    pub fn update_window(&mut self, new_window: u64) {
        if new_window > self.window {
            self.window = new_window;
        }
    }

    pub fn window(&self) -> u64 {
        self.window
    }

    pub fn consumed(&self) -> u64 {
        self.consumed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_available() {
        let fc = FlowControl::new(1000);
        assert_eq!(fc.available(), 1000);
    }

    #[test]
    fn consume_within_window() {
        let mut fc = FlowControl::new(1000);
        assert!(fc.consume(500));
        assert_eq!(fc.available(), 500);
        assert_eq!(fc.consumed(), 500);
    }

    #[test]
    fn consume_exceeds_window() {
        let mut fc = FlowControl::new(1000);
        assert!(fc.consume(500));
        assert!(!fc.consume(600));
        assert_eq!(fc.consumed(), 500);
    }

    #[test]
    fn consume_exact_window() {
        let mut fc = FlowControl::new(1000);
        assert!(fc.consume(1000));
        assert_eq!(fc.available(), 0);
    }

    #[test]
    fn update_window_increase() {
        let mut fc = FlowControl::new(1000);
        fc.consume(800);
        fc.update_window(2000);
        assert_eq!(fc.available(), 1200);
    }

    #[test]
    fn update_window_decrease_ignored() {
        let mut fc = FlowControl::new(1000);
        fc.update_window(500);
        assert_eq!(fc.window(), 1000);
    }

    #[test]
    fn zero_window() {
        let fc = FlowControl::new(0);
        assert_eq!(fc.available(), 0);
    }
}
