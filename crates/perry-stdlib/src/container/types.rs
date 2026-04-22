//! Single-container types and handle registries.

pub use perry_container_compose::types::{
    BuildSpec, ComposeConfig, ComposeDependsOn, ComposeHealthcheck, ComposeLogging,
    ComposeNetwork, ComposeNetworkIpam, ComposeNetworkIpamConfig, ComposeResourceSpec,
    ComposeSecret, ComposeService, ComposeServiceBuild, ComposeServiceNetworkConfig,
    ComposeServicePort, ComposeServiceVolume, ComposeServiceVolumeBind,
    ComposeServiceVolumeImage, ComposeServiceVolumeOpts, ComposeServiceVolumeTmpfs,
    ComposeSpec, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    DependsOnCondition, DependsOnSpec, ImageInfo, ListOrDict, PortSpec, ServiceNetworks,
    VolumeType, ComposeHandle,
};

pub use perry_container_compose::error::ComposeError as ContainerError;

use perry_container_compose::ComposeEngine;
use perry_runtime::{STRING_TAG, JSValue};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use dashmap::DashMap;

static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
static COMPOSE_HANDLES: OnceLock<DashMap<u64, ComposeEngine>> = OnceLock::new();
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

/// Unbox f64 bits into a u64 ID. Handles must be passed as integers.
pub fn unbox_id(bits: f64) -> u64 {
    bits as u64
}

/// Box a u64 ID into f64 bits for JS.
pub fn box_id(id: u64) -> u64 {
    // Standard integer-to-f64 bits conversion for simple numbers.
    // We return the bit pattern of the f64 double so it can be correctly
    // reconstructed by f64::from_bits on the resolution path.
    (id as f64).to_bits()
}

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    box_id(id)
}

pub fn take_container_handle(id: u64) -> Option<ContainerHandle> {
    CONTAINER_HANDLES.get_or_init(DashMap::new).remove(&id).map(|(_, h)| h)
}

pub fn register_compose_handle(engine: ComposeEngine) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    COMPOSE_HANDLES.get_or_init(DashMap::new).insert(id, engine);
    box_id(id)
}

pub fn get_compose_handle(id: u64) -> Option<dashmap::mapref::one::Ref<'static, u64, ComposeEngine>> {
    COMPOSE_HANDLES.get_or_init(DashMap::new).get(&id)
}

pub fn take_compose_handle(id: u64) -> Option<ComposeEngine> {
    COMPOSE_HANDLES.get_or_init(DashMap::new).remove(&id).map(|(_, e)| e)
}

/// Convert a raw StringHeader pointer into correctly tagged JSValue bits.
pub fn box_string_ptr(ptr: *const perry_runtime::StringHeader) -> u64 {
    (ptr as usize as u64) | STRING_TAG
}

// FFI Helpers - returning real JS Objects via JSON parsing for now as it's
// easier than manual object construction in Rust.

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
    let ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
    let val = unsafe { perry_runtime::js_json_parse(ptr) };
    val.bits()
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    let json = serde_json::to_string(&info).unwrap_or_else(|_| "{}".to_string());
    let ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
    let val = unsafe { perry_runtime::js_json_parse(ptr) };
    val.bits()
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let json = serde_json::to_string(&logs).unwrap_or_else(|_| "{\"stdout\":\"\",\"stderr\":\"\"}".to_string());
    let ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
    let val = unsafe { perry_runtime::js_json_parse(ptr) };
    val.bits()
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
    let ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
    let val = unsafe { perry_runtime::js_json_parse(ptr) };
    val.bits()
}
