use perry_runtime::{Promise, StringHeader};
use perry_stdlib::container::*;
use std::ptr;

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
fn to_header(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
fn await_promise_sync(p: *mut Promise) -> Result<u64, String> {
    if p.is_null() { return Err("Null promise".into()); }
    for _ in 0..100 {
        let state = unsafe { perry_runtime::js_promise_state(p) };
        if state == 1 { return Ok(unsafe { perry_runtime::js_promise_value(p) } as u64); }
        if state == 2 { return Err("Promise rejected".into()); }
        unsafe {
            perry_stdlib::common::js_stdlib_process_pending();
            perry_runtime::js_promise_run_microtasks();
        }
        std::thread::yield_now();
    }
    Err("Timeout".into())
}

macro_rules! test_ffi_contract {
    ($name:ident, $func:ident, $req:expr, json) => {
        #[test]
        // Feature: perry-container | Layer: ffi-contract | Req: $req | Property: -
        fn $name() {
            let p = unsafe { $func(ptr::null()) };
            assert!(!p.is_null());
            assert!(await_promise_sync(p).is_err());

            let bad = to_header("{");
            let p2 = unsafe { $func(bad) };
            assert!(!p2.is_null());
            assert!(await_promise_sync(p2).is_err());
        }
    };
    ($name:ident, $func:ident, $req:expr, string) => {
        #[test]
        // Feature: perry-container | Layer: ffi-contract | Req: $req | Property: -
        fn $name() {
            let p = unsafe { $func(ptr::null()) };
            assert!(!p.is_null());
            assert!(await_promise_sync(p).is_err());
        }
    };
}

test_ffi_contract!(test_run_contract, js_container_run, "2.1", json);
test_ffi_contract!(test_create_contract, js_container_create, "2.2", json);
test_ffi_contract!(test_start_contract, js_container_start, "2.3", string);
test_ffi_contract!(test_stop_contract, js_container_stop_wrap, "2.4", string);
test_ffi_contract!(test_remove_contract, js_container_remove_wrap, "2.5", string);
test_ffi_contract!(test_inspect_contract, js_container_inspect, "3.2", string);
test_ffi_contract!(test_logs_contract, js_container_logs_wrap, "4.1", string);
test_ffi_contract!(test_exec_contract, js_container_exec_wrap, "4.3", string);
test_ffi_contract!(test_pull_contract, js_container_pullImage, "5.1", string);
test_ffi_contract!(test_remove_img_contract, js_container_removeImage_wrap, "5.3", string);
test_ffi_contract!(test_compose_up_contract, js_compose_up, "6.1", json);

// Wrappers for functions with extra args to fit the macro
unsafe fn js_container_stop_wrap(id: *const StringHeader) -> *mut Promise { js_container_stop(id, 10) }
unsafe fn js_container_remove_wrap(id: *const StringHeader) -> *mut Promise { js_container_remove(id, 0) }
unsafe fn js_container_logs_wrap(id: *const StringHeader) -> *mut Promise { js_container_logs(id, 10) }
unsafe fn js_container_exec_wrap(id: *const StringHeader) -> *mut Promise { js_container_exec(id, ptr::null(), ptr::null(), ptr::null()) }
unsafe fn js_container_removeImage_wrap(id: *const StringHeader) -> *mut Promise { js_container_removeImage(id, 0) }

#[test]
// Feature: perry-container | Layer: ffi-contract | Req: 1.5 | Property: -
fn test_get_backend_contract() {
    let res = unsafe { js_container_getBackend() };
    assert!(!res.is_null());
}

#[test]
// Feature: perry-container | Layer: ffi-contract | Req: 6.6 | Property: -
fn test_compose_down_contract() {
    let p = unsafe { js_compose_down(1, 0) };
    assert!(!p.is_null());
    // Should reject because stack 1 doesn't exist
    assert!(await_promise_sync(p).is_err());
}

// Coverage Table
// | Requirement | Test name | Layer |
// |-------------|-----------|-------|
// | 2.1         | test_run_contract | ffi-contract |
// | 2.2         | test_create_contract | ffi-contract |
// | 2.3         | test_start_contract | ffi-contract |
// | 2.4         | test_stop_contract | ffi-contract |
// | 2.5         | test_remove_contract | ffi-contract |
// | 3.2         | test_inspect_contract | ffi-contract |
// | 4.1         | test_logs_contract | ffi-contract |
// | 4.3         | test_exec_contract | ffi-contract |
// | 5.1         | test_pull_contract | ffi-contract |
// | 5.3         | test_remove_img_contract | ffi-contract |
// | 6.1         | test_compose_up_contract | ffi-contract |
// | 1.5         | test_get_backend_contract | ffi-contract |
// | 6.6         | test_compose_down_contract | ffi-contract |

// Deferred Requirements:
// Req 6.6 (ps/logs/exec/start/stop/restart) - similar to down, tested via integration.
