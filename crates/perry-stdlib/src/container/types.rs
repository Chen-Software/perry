//! Type definitions for the perry/container module.

pub use perry_container_compose::types::{
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ComposeHandle
};
use perry_runtime::StringHeader;
use std::sync::OnceLock;
use dashmap::DashMap;
use perry_container_compose::ComposeEngine;

// ============ Handle Registry ============

pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static COMPOSE_HANDLES: OnceLock<DashMap<u64, ArcComposeEngine>> = OnceLock::new();

pub struct ArcComposeEngine(pub std::sync::Arc<ComposeEngine>);

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

// ============ Helper for StringHeader ============

pub unsafe fn string_from_header(header: *const StringHeader) -> Option<String> {
    if header.is_null() || (header as usize) < 0x1000 {
        return None;
    }
    let s = (*header).as_str();
    Some(s.to_string())
}
