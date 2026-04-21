//! Type re-exports for container module

pub use perry_container_compose::types::*;
pub use perry_container_compose::error::{ComposeError, compose_error_to_js};

use perry_runtime::JSValue;
use std::sync::atomic::{AtomicU64, Ordering};
use dashmap::DashMap;
use std::sync::OnceLock;

// ============ Handle Management ============

pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static CONTAINER_INFO_HANDLES: OnceLock<DashMap<u64, ContainerInfo>> = OnceLock::new();
pub static CONTAINER_INFO_LIST_HANDLES: OnceLock<DashMap<u64, Vec<ContainerInfo>>> = OnceLock::new();
pub static COMPOSE_HANDLES: OnceLock<DashMap<u64, ComposeHandle>> = OnceLock::new();
pub static CONTAINER_LOGS_HANDLES: OnceLock<DashMap<u64, ContainerLogs>> = OnceLock::new();
pub static IMAGE_INFO_LIST_HANDLES: OnceLock<DashMap<u64, Vec<ImageInfo>>> = OnceLock::new();

static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_INFO_HANDLES.get_or_init(DashMap::new).insert(id, info);
    id
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_INFO_LIST_HANDLES.get_or_init(DashMap::new).insert(id, list);
    id
}

pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    CONTAINER_INFO_LIST_HANDLES.get()?.remove(&id).map(|(_, v)| v)
}

pub fn register_compose_handle(handle: ComposeHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    COMPOSE_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_LOGS_HANDLES.get_or_init(DashMap::new).insert(id, logs);
    id
}

pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    CONTAINER_LOGS_HANDLES.get()?.remove(&id).map(|(_, v)| v)
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    IMAGE_INFO_LIST_HANDLES.get_or_init(DashMap::new).insert(id, list);
    id
}

// ============ JSValue Parsing Functions ============

pub fn parse_container_spec(_spec_ptr: *const JSValue) -> Result<ContainerSpec, String> {
    Err("ContainerSpec parsing must be done at compile time.".to_string())
}

pub fn parse_compose_spec(_spec_ptr: *const JSValue) -> Result<ComposeSpec, String> {
    Err("ComposeSpec parsing must be done at compile time.".to_string())
}
