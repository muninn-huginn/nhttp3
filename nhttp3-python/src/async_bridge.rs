//! Custom async bridge: tokio ↔ Python asyncio.

use pyo3::prelude::*;
use pyo3::{IntoPyObjectExt, Py};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();

pub fn runtime() -> Arc<Runtime> {
    RUNTIME
        .get_or_init(|| {
            Arc::new(
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(2)
                    .thread_name("nhttp3-tokio")
                    .build()
                    .expect("failed to create tokio runtime"),
            )
        })
        .clone()
}

/// Spawns a future on tokio and resolves a Python asyncio.Future.
pub fn spawn_and_resolve<F, T>(py: Python<'_>, fut: F) -> PyResult<Py<PyAny>>
where
    F: std::future::Future<Output = PyResult<T>> + Send + 'static,
    T: for<'py> IntoPyObject<'py> + Send + 'static,
{
    let asyncio = py.import("asyncio")?;
    let loop_ = asyncio.call_method0("get_running_loop")?;
    let py_future = loop_.call_method0("create_future")?;

    let future_ref: Py<PyAny> = py_future.clone().unbind();
    let loop_ref: Py<PyAny> = loop_.clone().unbind();

    let rt = runtime();
    rt.spawn(async move {
        let result = fut.await;

        Python::attach(|py| {
            let loop_ = loop_ref.bind(py);
            let future = future_ref.bind(py);

            match result {
                Ok(val) => {
                    if let Ok(set_result) = future.getattr("set_result") {
                        if let Ok(val_py) = val.into_py_any(py) {
                            let _ =
                                loop_.call_method1("call_soon_threadsafe", (set_result, val_py));
                        }
                    }
                }
                Err(err) => {
                    if let Ok(set_exception) = future.getattr("set_exception") {
                        let err_obj = err.value(py);
                        let _ =
                            loop_.call_method1("call_soon_threadsafe", (set_exception, err_obj));
                    }
                }
            }
        });
    });

    Ok(py_future.unbind())
}

pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    runtime().block_on(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_creates_successfully() {
        let rt = runtime();
        let result = rt.block_on(async { 42 });
        assert_eq!(result, 42);
    }
}
