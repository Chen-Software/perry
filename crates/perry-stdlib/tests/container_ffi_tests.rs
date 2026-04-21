use perry_stdlib::container::{
    js_container_run, js_container_create, js_container_composeUp,
    js_container_compose_down,
};
use std::ptr;

// Feature: perry-container | Layer: ffi | Req: 11.7 | Property: -
#[test]
fn test_js_container_run_null_guard() {
    unsafe {
        let result = js_container_run(ptr::null());
        assert!(result.is_null());
    }
}

// Feature: perry-container | Layer: ffi | Req: 11.7 | Property: -
#[test]
fn test_js_container_create_null_guard() {
    unsafe {
        let result = js_container_create(ptr::null());
        assert!(result.is_null());
    }
}

// Feature: perry-container | Layer: ffi | Req: 11.7 | Property: -
#[test]
fn test_js_container_compose_up_null_guard() {
    unsafe {
        let result = js_container_composeUp(ptr::null());
        assert!(result.is_null());
    }
}

// Feature: perry-container | Layer: ffi | Req: 11.7 | Property: -
#[test]
fn test_js_container_compose_down_invalid_handle() {
    unsafe {
        let result = js_container_compose_down(9999, 0);
        assert!(result.is_null());
    }
}
