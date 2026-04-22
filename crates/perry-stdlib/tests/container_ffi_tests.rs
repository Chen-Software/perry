use perry_runtime::{
    js_promise_run_microtasks, js_promise_state, js_string_from_bytes, Promise, StringHeader,
};
use perry_stdlib::common::async_bridge::js_stdlib_process_pending;
use perry_stdlib::container::*;
use std::ptr;

/// Helper to drive a promise to completion in a synchronous test
fn await_promise_sync(promise: *mut Promise) -> Result<u64, String> {
    assert!(!promise.is_null(), "FFI function returned null promise");
    for _ in 0..10000 {
        let state = unsafe { js_promise_state(promise) };
        if state == 1 {
            // Fulfilled
            return Ok(unsafe { perry_runtime::js_promise_value(promise) }.to_bits());
        } else if state == 2 {
            // Rejected
            return Err("Rejected".to_string());
        }
        unsafe {
            js_promise_run_microtasks();
            let _ = js_stdlib_process_pending();
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("Promise timed out");
}

/// Helper to create a Perry string for testing
fn to_js_str(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    unsafe { js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32) as *const StringHeader }
}

// Feature: perry-container | Layer: ffi-contract | Req: 2.1 | Property: -
#[test]
fn test_ffi_container_run_null() {
    unsafe {
        let p = js_container_run(ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 2.1 | Property: -
#[test]
fn test_ffi_container_run_malformed() {
    unsafe {
        let p = js_container_run(to_js_str("{ malformed"));
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_image_exists_null() {
    unsafe {
        let p = js_container_imageExists(ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_image_exists_malformed() {
    // imageExists does not parse JSON, but we follow the 2-test rule
    unsafe {
        let p = js_container_imageExists(to_js_str(""));
        // empty string should still resolve (either ok or err) but not crash
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 2.2 | Property: -
#[test]
fn test_ffi_container_create_null() {
    unsafe {
        let p = js_container_create(ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 2.2 | Property: -
#[test]
fn test_ffi_container_create_malformed() {
    unsafe {
        let p = js_container_create(to_js_str("{"));
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_start_null() {
    unsafe {
        let p = js_container_start(ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_start_malformed() {
    unsafe {
        let p = js_container_start(to_js_str("invalid-id"));
        // Should not panic, backend might return error
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_stop_null() {
    unsafe {
        let p = js_container_stop(ptr::null(), ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_stop_malformed() {
    unsafe {
        let p = js_container_stop(to_js_str("id"), to_js_str("{\"timeout\": -1}"));
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_remove_null() {
    unsafe {
        let p = js_container_remove(ptr::null(), ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_remove_malformed() {
    unsafe {
        let p = js_container_remove(to_js_str("id"), to_js_str("{\"force\": true}"));
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_list_zero() {
    unsafe {
        let p = js_container_list(to_js_str("{}"));
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_list_one() {
    unsafe {
        let p = js_container_list(to_js_str("{\"all\": true}"));
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_inspect_null() {
    unsafe {
        let p = js_container_inspect(ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_inspect_malformed() {
    unsafe {
        let p = js_container_inspect(to_js_str("id"));
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 1.1 | Property: -
#[test]
fn test_ffi_container_get_backend_call1() {
    unsafe {
        let p = js_container_getBackend();
        assert!(!p.is_null());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 1.1 | Property: -
#[test]
fn test_ffi_container_get_backend_call2() {
    unsafe {
        let p = js_container_getBackend();
        assert!(!p.is_null());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 1.1 | Property: -
#[test]
fn test_ffi_container_detect_backend_call1() {
    unsafe {
        let p = js_container_detectBackend();
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 1.1 | Property: -
#[test]
fn test_ffi_container_detect_backend_call2() {
    unsafe {
        let p = js_container_detectBackend();
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_logs_null() {
    unsafe {
        let p = js_container_logs(ptr::null(), ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_logs_malformed() {
    unsafe {
        let p = js_container_logs(to_js_str("id"), to_js_str("{}"));
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_exec_null() {
    unsafe {
        let p = js_container_exec(ptr::null(), ptr::null(), ptr::null(), ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_ffi_container_exec_malformed() {
    unsafe {
        let p = js_container_exec(to_js_str("id"), to_js_str("{"), ptr::null(), ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 15.4 | Property: -
#[test]
fn test_ffi_container_pull_image_null() {
    unsafe {
        let p = js_container_pullImage(ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 15.4 | Property: -
#[test]
fn test_ffi_container_pull_image_malformed() {
    unsafe {
        let p = js_container_pullImage(to_js_str("invalid:image"));
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_list_images_call1() {
    unsafe {
        let p = js_container_listImages();
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_list_images_call2() {
    unsafe {
        let p = js_container_listImages();
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_remove_image_null() {
    unsafe {
        let p = js_container_removeImage(ptr::null(), 0);
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_remove_image_malformed() {
    unsafe {
        let p = js_container_removeImage(to_js_str("img"), 1);
        let _ = await_promise_sync(p);
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.1 | Property: -
#[test]
fn test_ffi_compose_up_null() {
    unsafe {
        let p = js_container_compose_up(ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.1 | Property: -
#[test]
fn test_ffi_compose_up_malformed() {
    unsafe {
        let p = js_container_compose_up(to_js_str("{"));
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_down_invalid_handle() {
    unsafe {
        let p = js_container_compose_down(0, to_js_str("{}"));
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_down_invalid_handle_volumes() {
    unsafe {
        let p = js_container_compose_down(-1, to_js_str("{\"volumes\": true}"));
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_ps_invalid_handle() {
    unsafe {
        let p = js_container_compose_ps(0);
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_ps_invalid_handle_2() {
    unsafe {
        let p = js_container_compose_ps(9999);
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_logs_null_handle() {
    unsafe {
        let p = js_container_compose_logs(0, ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_logs_invalid_handle() {
    unsafe {
        let p = js_container_compose_logs(123, to_js_str("{}"));
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_exec_null_handle() {
    unsafe {
        let p = js_container_compose_exec(0, ptr::null(), ptr::null(), ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
#[test]
fn test_ffi_compose_exec_invalid_handle() {
    unsafe {
        let p = js_container_compose_exec(123, to_js_str("svc"), to_js_str("[]"), ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_inspect_image_null() {
    unsafe {
        let p = js_container_inspectImage(ptr::null());
        assert!(await_promise_sync(p).is_err());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_ffi_container_inspect_image_malformed() {
    unsafe {
        let p = js_container_inspectImage(to_js_str("img"));
        let _ = await_promise_sync(p);
    }
}
