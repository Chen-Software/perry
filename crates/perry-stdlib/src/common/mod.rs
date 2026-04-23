//! Common utilities for stdlib modules

pub mod handle;
// Tokio-backed promise/runtime bridge — only needed when an async feature
// (http-server/client, websocket, databases, email, scheduler, rate-limit,
// crypto's bcrypt path, …) pulls in `async-runtime`. Always-on code that
// references it must also be `#[cfg(feature = "async-runtime")]`-gated.
#[cfg(feature = "async-runtime")]
pub mod async_bridge;
pub mod dispatch;

pub use handle::*;
#[cfg(feature = "async-runtime")]
pub use async_bridge::*;
pub use dispatch::*;

use perry_runtime::StringHeader;

pub unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

pub unsafe fn js_promise_reject_str(msg: &str) -> *mut perry_runtime::Promise {
    let promise = perry_runtime::js_promise_new();
    let str_ptr = perry_runtime::js_string_from_bytes(msg.as_ptr(), msg.len() as u32);
    perry_runtime::js_promise_reject(promise, perry_runtime::JSValue::string_ptr(str_ptr).bits() as f64);
    promise
}
