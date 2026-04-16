//! FFI exports for Perry TypeScript integration.

use crate::compose::ComposeEngine;
use crate::types::ComposeSpec;
use std::sync::Arc;

// Minimal StringHeader for standalone FFI (not used when linked with stdlib)
#[repr(C)]
pub struct StringHeader {
    pub length: u32,
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).length as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).into_owned())
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_up_stub(_spec_json_ptr: *const StringHeader) -> u64 {
    0
}
