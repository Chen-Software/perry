use perry_stdlib::container::*;
use perry_runtime::{js_promise_new, Promise, StringHeader};
use std::ptr;

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_js_container_run_null_input() {
    unsafe {
        let promise = js_container_run(ptr::null());
        assert!(!promise.is_null());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -
#[test]
fn test_js_container_run_malformed_json() {
    unsafe {
        let malformed = "{\"image\": "; // Invalid JSON
        let bytes = malformed.as_bytes();
        let layout = std::alloc::Layout::from_size_align(
            std::mem::size_of::<StringHeader>() + bytes.len(),
            std::mem::align_of::<StringHeader>()
        ).unwrap();
        let header = std::alloc::alloc(layout) as *mut StringHeader;
        (*header).byte_len = bytes.len() as u32;
        ptr::copy_nonoverlapping(bytes.as_ptr(), (header as *mut u8).add(std::mem::size_of::<StringHeader>()), bytes.len());

        let promise = js_container_run(header);
        assert!(!promise.is_null());

        // Cleanup would normally be handled by the runtime, but we're in a unit test
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 11.2 | Property: -
#[test]
fn test_js_compose_up_null_input() {
    unsafe {
        let promise = js_container_composeUp(ptr::null());
        assert!(!promise.is_null());
    }
}

// Coverage Table:
// | Requirement | Test name | Layer |
// |-------------|-----------|-------|
// | 11.1        | test_js_container_run_null_input | ffi-contract |
// | 11.1        | test_js_container_run_malformed_json | ffi-contract |
// | 11.2        | test_js_compose_up_null_input | ffi-contract |
