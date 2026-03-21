use std::sync::Arc;
use tokio::runtime;

/// Manages the tokio runtime for FFI consumers.
///
/// FFI consumers don't need to know about async — the runtime is
/// managed internally. A background thread runs the tokio runtime,
/// and completions are posted back via callbacks.
pub struct Runtime {
    rt: Arc<runtime::Runtime>,
}

impl Runtime {
    /// Creates a new multi-threaded tokio runtime.
    pub fn new() -> Result<Self, String> {
        let rt = runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to create tokio runtime: {e}"))?;

        Ok(Self { rt: Arc::new(rt) })
    }

    /// Returns a handle to the runtime for spawning tasks.
    pub fn handle(&self) -> &runtime::Handle {
        self.rt.handle()
    }

    /// Blocks on a future (for synchronous FFI calls).
    pub fn block_on<F: std::future::Future>(&self, f: F) -> F::Output {
        self.rt.block_on(f)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new().expect("failed to create default runtime")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_runtime() {
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async { 42 });
        assert_eq!(result, 42);
    }

    #[test]
    fn spawn_and_join() {
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async {
            let handle = tokio::spawn(async { 100 });
            handle.await.unwrap()
        });
        assert_eq!(result, 100);
    }
}
