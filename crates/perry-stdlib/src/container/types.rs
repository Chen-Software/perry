//! Type re-exports for container module

pub use perry_container_compose::types::*;
pub use perry_container_compose::error::ComposeError;

use dashmap::DashMap;
use perry_runtime::StringHeader;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

// ============ Handle Management ============

static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
static INFO_HANDLES: OnceLock<DashMap<u64, ContainerInfo>> = OnceLock::new();
static INFO_LIST_HANDLES: OnceLock<DashMap<u64, Vec<ContainerInfo>>> = OnceLock::new();
static LOG_HANDLES: OnceLock<DashMap<u64, ContainerLogs>> = OnceLock::new();
static IMAGE_INFO_LIST_HANDLES: OnceLock<DashMap<u64, Vec<ImageInfo>>> = OnceLock::new();
static COMPOSE_HANDLES: OnceLock<DashMap<u64, ComposeHandle>> = OnceLock::new();
pub static CONTAINER_LOGS_MAP: OnceLock<DashMap<u64, std::collections::HashMap<String, ContainerLogs>>> = OnceLock::new();

fn next_id() -> u64 {
    NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = next_id();
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    let id = next_id();
    INFO_HANDLES.get_or_init(DashMap::new).insert(id, info);
    id
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let id = next_id();
    INFO_LIST_HANDLES.get_or_init(DashMap::new).insert(id, list);
    id
}

pub fn register_compose_handle(handle: ComposeHandle) -> u64 {
    let id = next_id();
    COMPOSE_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let id = next_id();
    LOG_HANDLES.get_or_init(DashMap::new).insert(id, logs);
    id
}

pub fn register_container_logs_map(map: std::collections::HashMap<String, ContainerLogs>) -> u64 {
    let id = next_id();
    CONTAINER_LOGS_MAP.get_or_init(DashMap::new).insert(id, map);
    id
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    let id = next_id();
    IMAGE_INFO_LIST_HANDLES.get_or_init(DashMap::new).insert(id, list);
    id
}

pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    INFO_LIST_HANDLES.get_or_init(DashMap::new).remove(&id).map(|(_, v)| v)
}

pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    LOG_HANDLES.get_or_init(DashMap::new).remove(&id).map(|(_, v)| v)
}

// ============ String Handling ============

pub unsafe fn string_to_js(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}
