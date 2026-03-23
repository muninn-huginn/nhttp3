/// Opaque handle for FFI consumers.
///
/// All nhttp3 objects are exposed as opaque pointers across the FFI boundary.
/// This prevents ABI compatibility issues and keeps internal types private.

/// Opaque future handle returned by async FFI operations.
/// The Python side wraps this into a native awaitable.
#[repr(C)]
pub struct FutureHandle {
    _opaque: [u8; 0],
}

/// Callback type for async operation completion.
/// Called from the tokio runtime thread — implementations must be thread-safe.
pub type CompletionCallback =
    extern "C" fn(context: *mut std::ffi::c_void, result: i32, data: *const u8, data_len: usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_type_is_extern_c() {
        // Just verifies the type compiles with extern "C"
        extern "C" fn test_cb(_: *mut std::ffi::c_void, _: i32, _: *const u8, _: usize) {}
        let _: CompletionCallback = test_cb;
    }
}
