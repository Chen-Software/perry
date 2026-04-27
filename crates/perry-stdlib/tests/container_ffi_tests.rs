//! FFI contract tests for perry/container and perry/compose.
//!
//! These tests verify that FFI functions handle null pointers and malformed
//! JSON correctly by returning a valid promise that eventually rejects.

use perry_runtime::{js_promise_state, js_promise_run_microtasks, Promise, StringHeader};
use perry_stdlib::container::*;
use std::ptr;

const PROMISE_STATE_PENDING: i32 = 0;
const PROMISE_STATE_FULFILLED: i32 = 1;
const PROMISE_STATE_REJECTED: i32 = 2;

/// Helper to create a fake StringHeader on the stack for testing.
fn make_string_header(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let len = bytes.len() as u32;
    let mut header_bytes = vec![0u8; std::mem::size_of::<StringHeader>() + bytes.len()];
    unsafe {
        let header = header_bytes.as_mut_ptr() as *mut StringHeader;
        (*header).utf16_len = s.chars().count() as u32;
        (*header).byte_len = len;
        (*header).capacity = len;
        (*header).refcount = 0;
        let data_ptr = header_bytes.as_mut_ptr().add(std::mem::size_of::<StringHeader>());
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data_ptr, bytes.len());
    }
    header_bytes
}

/// Drive the promise to completion by running microtasks and processing pending stdlib ops.
fn drive_promise(promise: *mut Promise) {
    let mut iterations = 0;
    while js_promise_state(promise) == PROMISE_STATE_PENDING && iterations < 100 {
        unsafe {
            perry_stdlib::common::js_stdlib_process_pending();
            js_promise_run_microtasks();
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        iterations += 1;
    }
}

// ============ js_container_run ============

#[test]
fn test_js_container_run_null() {
    unsafe {
        let p = js_container_run(ptr::null());
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}

#[test]
fn test_js_container_run_malformed() {
    let header = make_string_header("{invalid json}");
    unsafe {
        let p = js_container_run(header.as_ptr() as *const StringHeader);
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}

// ============ js_container_composeUp ============

#[test]
fn test_js_container_compose_up_null() {
    unsafe {
        let p = js_container_composeUp(ptr::null());
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}

#[test]
fn test_js_container_compose_up_malformed() {
    let header = make_string_header("not a json object");
    unsafe {
        let p = js_container_composeUp(header.as_ptr() as *const StringHeader);
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}

// ============ js_container_compose_ps ============

#[test]
fn test_js_container_compose_ps_not_found() {
    unsafe {
        let p = js_container_compose_ps(99999.0);
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}

// ============ js_container_inspect ============

#[test]
fn test_js_container_inspect_null() {
    unsafe {
        let p = js_container_inspect(ptr::null());
        assert!(!p.is_null());
        drive_promise(p);
        assert_eq!(js_promise_state(p), PROMISE_STATE_REJECTED);
    }
}
