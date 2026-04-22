//! Type re-exports and registry for container module

pub use perry_container_compose::types::*;
pub use perry_container_compose::error::ComposeError;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use dashmap::DashMap;

// ============ Registry ============

static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

// Opaque handles stored in stdlib registry
static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
// JSON data queries store their results in this registry so FFI can return a handle ID
static DATA_REGISTRY: OnceLock<DashMap<u64, String>> = OnceLock::new();

fn get_data_registry() -> &'static DashMap<u64, String> {
    DATA_REGISTRY.get_or_init(DashMap::new)
}

fn get_container_handles() -> &'static DashMap<u64, ContainerHandle> {
    CONTAINER_HANDLES.get_or_init(DashMap::new)
}

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    get_container_handles().insert(id, handle);
    id
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    let json = serde_json::to_string(&info).unwrap_or_default();
    get_data_registry().insert(id, json);
    id
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    let json = serde_json::to_string(&list).unwrap_or_default();
    get_data_registry().insert(id, json);
    id
}

pub fn register_compose_handle(handle: ComposeHandle) -> u64 {
    // Note: ComposeEngine is stored in COMPOSE_ENGINES in mod.rs,
    // this ID is used by TS to reference both the engine and the handle.
    let id = handle.stack_id;
    let handle_json = serde_json::to_string(&handle).unwrap_or_default();
    get_data_registry().insert(id, handle_json);
    id
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    let json = serde_json::to_string(&logs).unwrap_or_default();
    get_data_registry().insert(id, json);
    id
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    let json = serde_json::to_string(&list).unwrap_or_default();
    get_data_registry().insert(id, json);
    id
}

pub fn get_registered_data(id: u64) -> Option<String> {
    get_data_registry().get(&id).map(|v| v.clone())
}

pub fn get_container_handle(id: u64) -> Option<ContainerHandle> {
    get_container_handles().get(&id).map(|v| v.clone())
}

pub fn register_data(data: String) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    get_data_registry().insert(id, data);
    id
}
