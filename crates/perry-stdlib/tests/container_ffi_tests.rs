use perry_runtime::{StringHeader, Promise};
use perry_stdlib::container::*;
use std::ptr;

/// Drives the Perry stdlib async bridge to complete a promise.
async fn drive_promise(p: *mut Promise) -> i32 {
    if p.is_null() { return -1; }
    let mut state = unsafe { perry_runtime::js_promise_state(p) };
    let mut iterations = 0;
    while state == 0 && iterations < 500 {
        unsafe { perry_stdlib::common::js_stdlib_process_pending() };
        tokio::task::yield_now().await;
        state = unsafe { perry_runtime::js_promise_state(p) };
        iterations += 1;
    }
    state
}

fn malformed_json_header() -> *const StringHeader {
    let s = "{ \"invalid\": ";
    unsafe { perry_runtime::js_string_from_bytes(s.as_ptr(), s.len() as u32) }
}

macro_rules! test_json_ffi_contract {
    ($name:ident, $func:ident) => {
        // Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
        #[tokio::test]
        async fn $name() {
            unsafe {
                let p1 = $func(ptr::null());
                assert!(!p1.is_null());
                drive_promise(p1).await;

                let p2 = $func(malformed_json_header());
                assert!(!p2.is_null());
                let state = drive_promise(p2).await;
                assert_eq!(state, 2, "{} should reject malformed JSON", stringify!($func));
            }
        }
    };
}

test_json_ffi_contract!(test_js_container_run_contract, js_container_run);
test_json_ffi_contract!(test_js_container_create_contract, js_container_create);
test_json_ffi_contract!(test_js_compose_up_contract, js_compose_up);
test_json_ffi_contract!(test_js_compose_config_contract, js_compose_config);

macro_rules! test_string_ffi_contract {
    ($name:ident, $func:ident, $($extra:expr),*) => {
        // Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
        #[tokio::test]
        async fn $name() {
            unsafe {
                let p1 = $func(ptr::null() $(, $extra)*);
                assert!(!p1.is_null());
                drive_promise(p1).await;
                let p2 = $func(malformed_json_header() $(, $extra)*);
                assert!(!p2.is_null());
                drive_promise(p2).await;
            }
        }
    };
}

test_string_ffi_contract!(test_js_container_start_contract, js_container_start, );
test_string_ffi_contract!(test_js_container_stop_contract, js_container_stop, 10);
test_string_ffi_contract!(test_js_container_remove_contract, js_container_remove, 1);
test_string_ffi_contract!(test_js_container_inspect_contract, js_container_inspect, );
test_string_ffi_contract!(test_js_container_logs_contract, js_container_logs, 100);
test_string_ffi_contract!(test_js_container_image_exists_contract, js_container_imageExists, );
test_string_ffi_contract!(test_js_container_pull_image_contract, js_container_pullImage, );
test_string_ffi_contract!(test_js_container_remove_image_contract, js_container_removeImage, 1);

macro_rules! test_handle_ffi_contract {
    ($name:ident, $func:ident, $($extra:expr),*) => {
        // Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
        #[tokio::test]
        async fn $name() {
            unsafe {
                let p = $func(123 $(, $extra)*);
                assert!(!p.is_null());
                drive_promise(p).await;
            }
        }
    };
}

test_handle_ffi_contract!(test_js_compose_down_contract, js_compose_down, 1);
test_handle_ffi_contract!(test_js_compose_ps_contract, js_compose_ps, );
test_handle_ffi_contract!(test_js_compose_logs_contract, js_compose_logs, ptr::null(), 10);
test_handle_ffi_contract!(test_js_compose_start_contract, js_compose_start, ptr::null());
test_handle_ffi_contract!(test_js_compose_stop_contract, js_compose_stop, ptr::null());
test_handle_ffi_contract!(test_js_compose_restart_contract, js_compose_restart, ptr::null());
test_handle_ffi_contract!(test_js_compose_exec_contract, js_compose_exec, ptr::null(), ptr::null());

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[tokio::test]
async fn test_js_container_list_contract() {
    unsafe {
        let p = js_container_list(0);
        assert!(!p.is_null());
        drive_promise(p).await;
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[tokio::test]
async fn test_js_container_list_images_contract() {
    unsafe {
        let p = js_container_listImages();
        assert!(!p.is_null());
        drive_promise(p).await;
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[tokio::test]
async fn test_js_container_exec_contract() {
    unsafe {
        let p = js_container_exec(ptr::null(), ptr::null(), ptr::null(), ptr::null());
        assert!(!p.is_null());
        drive_promise(p).await;
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[test]
fn test_js_container_get_backend_contract() {
    unsafe {
        let s = js_container_getBackend();
        assert!(!s.is_null());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: none | Property: -
#[tokio::test]
async fn test_js_container_detect_backend_contract() {
    unsafe {
        let p = js_container_detectBackend();
        assert!(!p.is_null());
        drive_promise(p).await;
    }
}

/*
Coverage Table:
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| none        | test_js_container_run_contract | ffi-contract |
| none        | test_js_container_create_contract | ffi-contract |
| none        | test_js_container_start_contract | ffi-contract |
| none        | test_js_container_stop_contract | ffi-contract |
| none        | test_js_container_remove_contract | ffi-contract |
| none        | test_js_container_list_contract | ffi-contract |
| none        | test_js_container_inspect_contract | ffi-contract |
| none        | test_js_container_get_backend_contract | ffi-contract |
| none        | test_js_container_detect_backend_contract | ffi-contract |
| none        | test_js_container_logs_contract | ffi-contract |
| none        | test_js_container_exec_contract | ffi-contract |
| none        | test_js_container_image_exists_contract | ffi-contract |
| none        | test_js_container_pull_image_contract | ffi-contract |
| none        | test_js_container_list_images_contract | ffi-contract |
| none        | test_js_container_remove_image_contract | ffi-contract |
| none        | test_js_compose_up_contract | ffi-contract |
| none        | test_js_compose_down_contract | ffi-contract |
| none        | test_js_compose_ps_contract | ffi-contract |
| none        | test_js_compose_logs_contract | ffi-contract |
| none        | test_js_compose_config_contract | ffi-contract |
| none        | test_js_compose_start_contract | ffi-contract |
| none        | test_js_compose_stop_contract | ffi-contract |
| none        | test_js_compose_restart_contract | ffi-contract |
| none        | test_js_compose_exec_contract | ffi-contract |

Deferred Requirements:
- none
*/
