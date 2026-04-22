//! FFI contract tests for the perry-stdlib container module.
//!
//! Validates the FFI boundary for both perry/container and perry/container-compose,
//! ensuring that null pointers and malformed inputs are handled gracefully without panicking.

use perry_runtime::{js_promise_state, js_jsvalue_to_string, Promise, StringHeader};
use perry_stdlib::container::*;
use perry_stdlib::js_stdlib_process_pending;
use std::ptr;
use std::time::Duration;

extern "C" {
    fn js_promise_reason(promise: *mut Promise) -> f64;
}

/// Helper to drive async stdlib operations to completion in tests.
fn drive_async(promise: *mut Promise) {
    for _ in 0..100 {
        if js_promise_state(promise) != 0 {
            break;
        }
        js_stdlib_process_pending();
        perry_runtime::js_promise_run_microtasks();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Helper to check if a promise resolved to an error containing a substring.
fn assert_promise_error(promise: *mut Promise, expected_msg: &str) {
    assert!(!promise.is_null(), "Promise pointer should not be null");
    drive_async(promise);
    let state = js_promise_state(promise);
    assert_eq!(state, 2, "Promise should be rejected, got state {}", state);
    let val = unsafe { js_promise_reason(promise) };
    let err_str = unsafe {
        let s_ptr = js_jsvalue_to_string(val);
        if s_ptr.is_null() {
            "0".to_string()
        } else {
            let len = (*s_ptr).byte_len as usize;
            let data_ptr = (s_ptr as *const u8).add(std::mem::size_of::<StringHeader>());
            let bytes = std::slice::from_raw_parts(data_ptr, len);
            String::from_utf8_lossy(bytes).into_owned()
        }
    };
    assert!(err_str.contains(expected_msg), "Error '{}' should contain '{}'", err_str, expected_msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_runtime() {
        perry_runtime::gc::js_gc_init();
    }

    // --- perry/container FFI ---

    // Feature: perry-container | Layer: ffi-contract | Req: 2.1 | Property: -
    #[test]
    fn test_js_container_run_null() {
        let p = unsafe { js_container_run(ptr::null()) };
        assert_promise_error(p, "Invalid spec JSON pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 2.1 | Property: -
    #[test]
    fn test_js_container_run_malformed() {
        let json = unsafe { perry_runtime::js_string_from_bytes(b"{invalid".as_ptr(), 8) };
        let p = unsafe { js_container_run(json) };
        assert_promise_error(p, "Invalid ContainerSpec JSON");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 2.2 | Property: -
    #[test]
    fn test_js_container_create_null() {
        let p = unsafe { js_container_create(ptr::null()) };
        assert_promise_error(p, "Invalid spec JSON pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 2.2 | Property: -
    #[test]
    fn test_js_container_create_malformed() {
        let json = unsafe { perry_runtime::js_string_from_bytes(b"{invalid".as_ptr(), 8) };
        let p = unsafe { js_container_create(json) };
        assert_promise_error(p, "Invalid ContainerSpec JSON");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 2.3 | Property: -
    #[test]
    fn test_js_container_start_null() {
        let p = unsafe { js_container_start(ptr::null()) };
        assert_promise_error(p, "Invalid container ID pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 2.3 | Property: -
    #[test]
    fn test_js_container_start_malformed() {
        let p = unsafe { js_container_start(1 as *const StringHeader) };
        assert_promise_error(p, "Invalid container ID pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 2.4 | Property: -
    #[test]
    fn test_js_container_stop_null() {
        let p = unsafe { js_container_stop(ptr::null(), -1) };
        assert_promise_error(p, "Invalid container ID pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 2.4 | Property: -
    #[test]
    fn test_js_container_stop_malformed() {
        let p = unsafe { js_container_stop(1 as *const StringHeader, -1) };
        assert_promise_error(p, "Invalid container ID pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 2.5 | Property: -
    #[test]
    fn test_js_container_remove_null() {
        let p = unsafe { js_container_remove(ptr::null(), 0) };
        assert_promise_error(p, "Invalid container ID pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 2.5 | Property: -
    #[test]
    fn test_js_container_remove_malformed() {
        let p = unsafe { js_container_remove(1 as *const StringHeader, 0) };
        assert_promise_error(p, "Invalid container ID pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 3.2 | Property: -
    #[test]
    fn test_js_container_inspect_null() {
        let p = unsafe { js_container_inspect(ptr::null()) };
        assert_promise_error(p, "Invalid container ID pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 3.2 | Property: -
    #[test]
    fn test_js_container_inspect_malformed() {
        let p = unsafe { js_container_inspect(1 as *const StringHeader) };
        assert_promise_error(p, "Invalid container ID pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 4.1 | Property: -
    #[test]
    fn test_js_container_logs_null() {
        let p = unsafe { js_container_logs(ptr::null(), -1) };
        assert_promise_error(p, "Invalid container ID pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 4.1 | Property: -
    #[test]
    fn test_js_container_logs_malformed() {
        let p = unsafe { js_container_logs(1 as *const StringHeader, -1) };
        assert_promise_error(p, "Invalid container ID pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 4.3 | Property: -
    #[test]
    fn test_js_container_exec_null() {
        let p = unsafe { js_container_exec(ptr::null(), ptr::null(), ptr::null(), ptr::null()) };
        assert_promise_error(p, "Invalid container ID pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 4.3 | Property: -
    #[test]
    fn test_js_container_exec_malformed() {
        let id = unsafe { perry_runtime::js_string_from_bytes(b"id".as_ptr(), 2) };
        let cmd = unsafe { perry_runtime::js_string_from_bytes(b"{invalid".as_ptr(), 8) };
        let p = unsafe { js_container_exec(id, cmd, ptr::null(), ptr::null()) };
        assert_promise_error(p, "Invalid command JSON");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 5.1 | Property: -
    #[test]
    fn test_js_container_pull_image_null() {
        let p = unsafe { js_container_pullImage(ptr::null()) };
        assert_promise_error(p, "Invalid image reference pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 5.1 | Property: -
    #[test]
    fn test_js_container_pull_image_malformed() {
        let p = unsafe { js_container_pullImage(1 as *const StringHeader) };
        assert_promise_error(p, "Invalid image reference pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
    #[test]
    fn test_js_container_image_exists_null() {
        let p = unsafe { js_container_imageExists(ptr::null()) };
        assert_promise_error(p, "Invalid image reference pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
    #[test]
    fn test_js_container_image_exists_malformed() {
        let p = unsafe { js_container_imageExists(1 as *const StringHeader) };
        assert_promise_error(p, "Invalid image reference pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
    #[test]
    fn test_js_container_inspect_image_null() {
        let p = unsafe { js_container_inspectImage(ptr::null()) };
        assert_promise_error(p, "Invalid image reference pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
    #[test]
    fn test_js_container_inspect_image_malformed() {
        let p = unsafe { js_container_inspectImage(1 as *const StringHeader) };
        assert_promise_error(p, "Invalid image reference pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 5.3 | Property: -
    #[test]
    fn test_js_container_remove_image_null() {
        let p = unsafe { js_container_removeImage(ptr::null(), 0) };
        assert_promise_error(p, "Invalid image reference pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 5.3 | Property: -
    #[test]
    fn test_js_container_remove_image_malformed() {
        let p = unsafe { js_container_removeImage(1 as *const StringHeader, 0) };
        assert_promise_error(p, "Invalid image reference pointer");
    }

    // --- perry/container-compose FFI ---

    // Feature: perry-container | Layer: ffi-contract | Req: 6.1 | Property: -
    #[test]
    fn test_js_compose_up_null() {
        let p = unsafe { js_compose_up(ptr::null()) };
        assert_promise_error(p, "Invalid spec JSON pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 6.1 | Property: -
    #[test]
    fn test_js_compose_up_malformed() {
        let json = unsafe { perry_runtime::js_string_from_bytes(b"{invalid".as_ptr(), 8) };
        let p = unsafe { js_compose_up(json) };
        assert_promise_error(p, "Invalid ComposeSpec JSON");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
    #[test]
    fn test_js_compose_logs_null() {
        let p = unsafe { js_compose_logs(0.0, ptr::null(), -1) };
        assert_promise_error(p, "Invalid compose handle");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
    #[test]
    fn test_js_compose_logs_malformed() {
        let p = unsafe { js_compose_logs(0.0, 1 as *const StringHeader, -1) };
        assert_promise_error(p, "Invalid service name pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
    #[test]
    fn test_js_compose_exec_null() {
        let p = unsafe { js_compose_exec(0.0, ptr::null(), ptr::null()) };
        assert_promise_error(p, "Invalid service name pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
    #[test]
    fn test_js_compose_exec_malformed() {
        let svc = unsafe { perry_runtime::js_string_from_bytes(b"svc".as_ptr(), 3) };
        let cmd = unsafe { perry_runtime::js_string_from_bytes(b"{invalid".as_ptr(), 8) };
        let p = unsafe { js_compose_exec(0.0, svc, cmd) };
        assert_promise_error(p, "Invalid compose handle");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 8.7 | Property: -
    #[test]
    fn test_js_compose_config_null() {
        let p = unsafe { js_compose_config(ptr::null()) };
        assert_promise_error(p, "Invalid spec JSON pointer");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 8.7 | Property: -
    #[test]
    fn test_js_compose_config_malformed() {
        let json = unsafe { perry_runtime::js_string_from_bytes(b"{invalid".as_ptr(), 8) };
        let p = unsafe { js_compose_config(json) };
        assert_promise_error(p, "Invalid ComposeSpec JSON");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 8.2 | Property: -
    #[test]
    fn test_js_compose_start_null() {
        let p = unsafe { js_compose_start(0.0, ptr::null()) };
        assert_promise_error(p, "Invalid compose handle");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 8.2 | Property: -
    #[test]
    fn test_js_compose_start_malformed() {
        let json = unsafe { perry_runtime::js_string_from_bytes(b"{invalid".as_ptr(), 8) };
        let p = unsafe { js_compose_start(0.0, json) };
        assert_promise_error(p, "Invalid services JSON");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 8.2 | Property: -
    #[test]
    fn test_js_compose_stop_null() {
        let p = unsafe { js_compose_stop(0.0, ptr::null()) };
        assert_promise_error(p, "Invalid compose handle");
    }

    // Feature: perry-container | Layer: ffi-contract | Req: 8.2 | Property: -
    #[test]
    fn test_js_compose_stop_malformed() {
        let json = unsafe { perry_runtime::js_string_from_bytes(b"{invalid".as_ptr(), 8) };
        let p = unsafe { js_compose_stop(0.0, json) };
        assert_promise_error(p, "Invalid services JSON");
    }
}
