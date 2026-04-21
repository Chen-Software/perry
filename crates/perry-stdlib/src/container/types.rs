use std::sync::Arc;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};
pub use perry_container_compose::types::*;
pub use perry_container_compose::error::ComposeError;
pub use perry_container_compose::error::ComposeError as ContainerError;
pub use perry_container_compose::ComposeEngine;

// ============ Global Handle Registries ============

pub static CONTAINER_HANDLES: Lazy<DashMap<u64, ContainerHandle>> = Lazy::new(DashMap::new);
pub static COMPOSE_HANDLES: Lazy<DashMap<u64, Arc<ComposeEngine>>> = Lazy::new(DashMap::new);

// Registries for JSON-serialized responses
pub static CONTAINER_INFO_REGISTRY: Lazy<DashMap<u64, ContainerInfo>> = Lazy::new(DashMap::new);
pub static CONTAINER_INFO_LIST_REGISTRY: Lazy<DashMap<u64, Vec<ContainerInfo>>> = Lazy::new(DashMap::new);
pub static CONTAINER_LOGS_REGISTRY: Lazy<DashMap<u64, ContainerLogs>> = Lazy::new(DashMap::new);
pub static IMAGE_INFO_LIST_REGISTRY: Lazy<DashMap<u64, Vec<ImageInfo>>> = Lazy::new(DashMap::new);

static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_HANDLES.insert(id, handle);
    id
}

pub fn register_compose_handle(handle: ComposeHandle) -> u64 {
    // We already have a stack_id in handle, but we might want our own internal registry ID
    // Actually, let's just use the stack_id from the handle.
    handle.stack_id
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_INFO_REGISTRY.insert(id, info);
    id
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_INFO_LIST_REGISTRY.insert(id, list);
    id
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_LOGS_REGISTRY.insert(id, logs);
    id
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    IMAGE_INFO_LIST_REGISTRY.insert(id, list);
    id
}

pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    CONTAINER_INFO_LIST_REGISTRY.remove(&id).map(|(_, v)| v)
}

pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    CONTAINER_LOGS_REGISTRY.remove(&id).map(|(_, v)| v)
}
