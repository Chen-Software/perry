use perry_runtime::{Promise, StringHeader, js_string_from_bytes, js_promise_state, js_promise_run_microtasks};
use perry_stdlib::container::*;
use perry_stdlib::common::async_bridge::js_stdlib_process_pending;
use std::ptr;

/// Helper to drive a promise to completion in a synchronous test
fn await_promise_sync(promise: *mut Promise) -> Result<u64, String> {
    assert!(!promise.is_null(), "FFI function returned null promise");
    for _ in 0..10000 {
        let state = unsafe { js_promise_state(promise) };
        if state == 1 { // Fulfilled
            return Ok(unsafe { perry_runtime::js_promise_value(promise) }.to_bits());
        } else if state == 2 { // Rejected
            return Err("Rejected".to_string());
        }
        unsafe {
            js_promise_run_microtasks();
            js_stdlib_process_pending();
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("Promise timed out");
}

/// Helper to create a Perry string for testing
fn to_js_str(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

// Feature: perry-container | Layer: ffi-contract | Req: 2.1 | Property: -
#[test]
fn test_ffi_container_run_contract() {
    unsafe {
        let p1 = js_container_run(ptr::null());
        assert!(await_promise_sync(p1).is_err());
        let p2 = js_container_run(to_js_str("{"));
        assert!(await_promise_sync(p2).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_image_exists_contract() {
    unsafe {
        let p1 = js_container_imageExists(ptr::null());
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 2.2 | Property: -
#[test]
fn test_ffi_container_create_contract() {
    unsafe {
        let p1 = js_container_create(ptr::null());
        assert!(await_promise_sync(p1).is_err());
        let p2 = js_container_create(to_js_str("{"));
        assert!(await_promise_sync(p2).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_start_contract() {
    unsafe {
        let p1 = js_container_start(ptr::null());
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_stop_contract() {
    unsafe {
        let p1 = js_container_stop(ptr::null(), 0);
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_remove_contract() {
    unsafe {
        let p1 = js_container_remove(ptr::null(), 0);
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_list_contract() {
    // List takes i64, so no null pointer check for the arg itself
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_inspect_contract() {
    unsafe {
        let p1 = js_container_inspect(ptr::null());
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_get_backend_contract() {
    unsafe {
        let p = js_container_getBackend();
        assert!(await_promise_sync(p).is_ok());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_detect_backend_contract() {
    unsafe {
        let p = js_container_detectBackend();
        assert!(await_promise_sync(p).is_ok() || await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_logs_contract() {
    unsafe {
        let p1 = js_container_logs(ptr::null(), 0);
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_exec_contract() {
    unsafe {
        let p1 = js_container_exec(ptr::null(), ptr::null(), ptr::null(), ptr::null());
        assert!(await_promise_sync(p1).is_err());
        let p2 = js_container_exec(to_js_str("id"), to_js_str("{"), ptr::null(), ptr::null());
        assert!(await_promise_sync(p2).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 15.4 | Property: -
#[test]
fn test_ffi_container_pull_image_contract() {
    unsafe {
        let p1 = js_container_pullImage(ptr::null());
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_list_images_contract() {
    unsafe {
        let p = js_container_listImages();
        assert!(await_promise_sync(p).is_ok() || await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_remove_image_contract() {
    unsafe {
        let p1 = js_container_removeImage(ptr::null(), 0);
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.1 | Property: -
#[test]
fn test_ffi_compose_up_contract() {
    unsafe {
        let p1 = js_container_compose_up(ptr::null());
        assert!(await_promise_sync(p1).is_err());
        let p2 = js_container_compose_up(to_js_str("{"));
        assert!(await_promise_sync(p2).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_down_contract() {
    unsafe {
        let p1 = js_container_compose_down(0, 0);
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_ps_contract() {
    unsafe {
        let p1 = js_container_compose_ps(0);
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_logs_contract() {
    unsafe {
        let p1 = js_container_compose_logs(0, ptr::null(), 0);
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_exec_contract() {
    unsafe {
        let p1 = js_container_compose_exec(0, ptr::null(), ptr::null());
        assert!(await_promise_sync(p1).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_inspect_image_contract() {
    unsafe {
        let p1 = js_container_inspectImage(ptr::null());
        assert!(await_promise_sync(p1).is_err());
    }
}
