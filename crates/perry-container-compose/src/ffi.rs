//! FFI exports for standalone linking

use crate::types::{ComposeSpec, ContainerSpec};
use crate::error::ComposeError;

#[no_mangle]
pub unsafe extern "C" fn js_compose_up_internal(spec_json: *const u8, len: usize) -> i64 {
    // Basic FFI entry point for non-stdlib linking scenarios
    0
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_down_internal(stack_id: i64, volumes: i32) -> i32 {
    0
}
