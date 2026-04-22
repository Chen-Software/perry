//! Type re-exports for container module

pub use perry_container_compose::types::*;
pub use perry_container_compose::error::ComposeError;

use dashmap::DashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;

// ============ Global Handle Registries ============

pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static CONTAINER_INFO: OnceLock<DashMap<u64, ContainerInfo>> = OnceLock::new();
pub static CONTAINER_INFO_LIST: OnceLock<DashMap<u64, Vec<ContainerInfo>>> = OnceLock::new();
pub static CONTAINER_LOGS: OnceLock<DashMap<u64, ContainerLogs>> = OnceLock::new();
pub static IMAGE_INFO_LIST: OnceLock<DashMap<u64, Vec<ImageInfo>>> = OnceLock::new();
pub static CONTAINER_LOGS_MAP: OnceLock<DashMap<u64, HashMap<String, ContainerLogs>>> = OnceLock::new();

static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

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
    CONTAINER_INFO.get_or_init(DashMap::new).insert(id, info);
    id
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let id = next_id();
    CONTAINER_INFO_LIST.get_or_init(DashMap::new).insert(id, list);
    id
}

pub fn register_compose_handle(handle: ComposeHandle) -> u64 {
    // Note: ComposeHandle is just a struct returned to TS.
    // The engine itself is registered in mod.rs:COMPOSE_ENGINES.
    handle.stack_id
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let id = next_id();
    CONTAINER_LOGS.get_or_init(DashMap::new).insert(id, logs);
    id
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    let id = next_id();
    IMAGE_INFO_LIST.get_or_init(DashMap::new).insert(id, list);
    id
}

pub fn register_container_logs_map(map: HashMap<String, ContainerLogs>) -> u64 {
    let id = next_id();
    CONTAINER_LOGS_MAP.get_or_init(DashMap::new).insert(id, map);
    id
}
