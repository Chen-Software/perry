//! Type re-exports for container module

pub use perry_container_compose::types::*;
pub use perry_container_compose::error::ComposeError;

use perry_runtime::{JSValue, StringHeader};
use perry_runtime::object::{js_object_alloc_with_shape, js_object_set_field};
use perry_runtime::array::{js_array_alloc, js_array_push};

// Unique shape IDs for container types
const SHAPE_CONTAINER_HANDLE: u32 = 0x1000_0001;
const SHAPE_CONTAINER_INFO: u32   = 0x1000_0002;
const SHAPE_CONTAINER_LOGS: u32   = 0x1000_0003;
const SHAPE_IMAGE_INFO: u32       = 0x1000_0004;
const SHAPE_COMPOSE_HANDLE: u32   = 0x1000_0005;

unsafe fn string_to_js(s: &str) -> f64 {
    let ptr = perry_runtime::js_string_from_bytes(s.as_bytes().as_ptr(), s.as_bytes().len() as u32);
    f64::from_bits(0x7FFF_0000_0000_0000 | (ptr as u64 & 0x0000_FFFF_FFFF_FFFF))
}

unsafe fn val_to_js(v: f64) -> JSValue {
    JSValue::from_bits(v.to_bits())
}

pub unsafe fn register_container_handle(handle: ContainerHandle) -> u64 {
    let packed_keys = b"id\0name";
    let obj = js_object_alloc_with_shape(SHAPE_CONTAINER_HANDLE, 2, packed_keys.as_ptr(), packed_keys.len() as u32);
    js_object_set_field(obj, 0, val_to_js(string_to_js(&handle.id)));
    js_object_set_field(obj, 1, match handle.name {
        Some(ref n) => val_to_js(string_to_js(n)),
        None => JSValue::null(),
    });
    (obj as u64) | 0x7FFD_0000_0000_0000
}

pub unsafe fn register_container_info(info: ContainerInfo) -> u64 {
    let packed_keys = b"id\0name\0image\0status\0ports\0created";
    let obj = js_object_alloc_with_shape(SHAPE_CONTAINER_INFO, 6, packed_keys.as_ptr(), packed_keys.len() as u32);
    js_object_set_field(obj, 0, val_to_js(string_to_js(&info.id)));
    js_object_set_field(obj, 1, val_to_js(string_to_js(&info.name)));
    js_object_set_field(obj, 2, val_to_js(string_to_js(&info.image)));
    js_object_set_field(obj, 3, val_to_js(string_to_js(&info.status)));

    let ports_arr = js_array_alloc(info.ports.len() as u32);
    for p in info.ports {
        js_array_push(ports_arr, val_to_js(string_to_js(&p)));
    }
    js_object_set_field(obj, 4, JSValue::from_bits((ports_arr as u64) | 0x7FFD_0000_0000_0000));

    js_object_set_field(obj, 5, val_to_js(string_to_js(&info.created)));
    (obj as u64) | 0x7FFD_0000_0000_0000
}

pub unsafe fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let arr = js_array_alloc(list.len() as u32);
    for info in list {
        let obj_bits = register_container_info(info);
        js_array_push(arr, JSValue::from_bits(obj_bits));
    }
    (arr as u64) | 0x7FFD_0000_0000_0000
}

pub unsafe fn register_compose_handle(handle: ComposeHandle) -> u64 {
    let packed_keys = b"stack_id\0project_name\0services";
    let obj = js_object_alloc_with_shape(SHAPE_COMPOSE_HANDLE, 3, packed_keys.as_ptr(), packed_keys.len() as u32);
    js_object_set_field(obj, 0, JSValue::number(handle.stack_id as f64));
    js_object_set_field(obj, 1, val_to_js(string_to_js(&handle.project_name)));

    let services_arr = js_array_alloc(handle.services.len() as u32);
    for s in handle.services {
        js_array_push(services_arr, val_to_js(string_to_js(&s)));
    }
    js_object_set_field(obj, 2, JSValue::from_bits((services_arr as u64) | 0x7FFD_0000_0000_0000));

    (obj as u64) | 0x7FFD_0000_0000_0000
}

pub unsafe fn register_container_logs(logs: ContainerLogs) -> u64 {
    let packed_keys = b"stdout\0stderr";
    let obj = js_object_alloc_with_shape(SHAPE_CONTAINER_LOGS, 2, packed_keys.as_ptr(), packed_keys.len() as u32);
    js_object_set_field(obj, 0, val_to_js(string_to_js(&logs.stdout)));
    js_object_set_field(obj, 1, val_to_js(string_to_js(&logs.stderr)));
    (obj as u64) | 0x7FFD_0000_0000_0000
}

pub unsafe fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    let arr = js_array_alloc(list.len() as u32);
    for info in list {
        let packed_keys = b"id\0repository\0tag\0size\0created";
        let obj = js_object_alloc_with_shape(SHAPE_IMAGE_INFO, 5, packed_keys.as_ptr(), packed_keys.len() as u32);
        js_object_set_field(obj, 0, val_to_js(string_to_js(&info.id)));
        js_object_set_field(obj, 1, val_to_js(string_to_js(&info.repository)));
        js_object_set_field(obj, 2, val_to_js(string_to_js(&info.tag)));
        js_object_set_field(obj, 3, JSValue::number(info.size as f64));
        js_object_set_field(obj, 4, val_to_js(string_to_js(&info.created)));
        js_array_push(arr, JSValue::from_bits((obj as u64) | 0x7FFD_0000_0000_0000));
    }
    (arr as u64) | 0x7FFD_0000_0000_0000
}

// ============ JSValue Parsing Functions ============

pub fn parse_container_spec(_spec_ptr: *const JSValue) -> Result<ContainerSpec, String> {
    Err("ContainerSpec parsing must be done at compile time via JSON.stringify.".to_string())
}

pub fn parse_compose_spec(_spec_ptr: *const JSValue) -> Result<ComposeSpec, String> {
    Err("ComposeSpec parsing must be done at compile time via JSON.stringify.".to_string())
}
