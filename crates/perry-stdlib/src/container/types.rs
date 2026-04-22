//! Type definitions for the perry/container module.
//!
//! Re-exports types from perry-container-compose and adds stdlib-specific
//! handle registries.

pub use perry_container_compose::types::*;
pub use perry_container_compose::error::ComposeError as ContainerError;
use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use once_cell::sync::Lazy;
use dashmap::DashMap;

use crate::common::handle::{self, Handle};

// ============ Global Handle Registries ============

static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

/// Registry for running compose engines.
pub static COMPOSE_ENGINES: Lazy<DashMap<u64, std::sync::Arc<perry_container_compose::compose::ComposeEngine>>> =
    Lazy::new(DashMap::new);

/// Register a container handle and return an opaque integer handle.
pub fn register_container_handle(h: perry_container_compose::types::ContainerHandle) -> u64 {
    handle::register_handle(h) as u64
}

/// Register a compose engine and return an opaque integer handle.
pub fn register_compose_engine(engine: std::sync::Arc<perry_container_compose::compose::ComposeEngine>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    COMPOSE_ENGINES.insert(id, engine);
    id
}

/// Retrieve a compose engine by handle id.
pub fn get_compose_engine(id: u64) -> Option<std::sync::Arc<perry_container_compose::compose::ComposeEngine>> {
    COMPOSE_ENGINES.get(&id).map(|r| std::sync::Arc::clone(&r))
}

/// Remove and return a compose engine from the registry.
pub fn take_compose_engine(id: u64) -> Option<std::sync::Arc<perry_container_compose::compose::ComposeEngine>> {
    COMPOSE_ENGINES.remove(&id).map(|(_, e)| e)
}

/// Register `ContainerLogs` and return an opaque integer handle.
pub fn register_container_logs(logs: perry_container_compose::types::ContainerLogs) -> u64 {
    handle::register_handle(logs) as u64
}

/// Register `ContainerInfo` and return an opaque integer handle.
pub fn register_container_info(info: perry_container_compose::types::ContainerInfo) -> u64 {
    handle::register_handle(info) as u64
}

/// Register `Vec<ContainerInfo>` and return an opaque integer handle.
pub fn register_container_info_list(list: Vec<perry_container_compose::types::ContainerInfo>) -> u64 {
    handle::register_handle(list) as u64
}

/// Register `Vec<ImageInfo>` and return an opaque integer handle.
pub fn register_image_info_list(list: Vec<perry_container_compose::types::ImageInfo>) -> u64 {
    handle::register_handle(list) as u64
}

/// Remove a container info list from the handle registry.
pub fn take_container_info_list(h: u64) -> Option<Vec<perry_container_compose::types::ContainerInfo>> {
    handle::take_handle::<Vec<perry_container_compose::types::ContainerInfo>>(h as handle::Handle)
}

/// Remove container logs from the handle registry.
pub fn take_container_logs(h: u64) -> Option<perry_container_compose::types::ContainerLogs> {
    handle::take_handle::<perry_container_compose::types::ContainerLogs>(h as handle::Handle)
}

// ============ FFI JSON Mapping ============

/// Helper to extract string from StringHeader pointer
pub unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

/// Error type for FFI bridge
#[derive(Debug, Serialize, Deserialize)]
pub struct FfiError {
    pub message: String,
    pub code: i32,
}

impl FfiError {
    pub fn new(message: impl Into<String>, code: i32) -> Self {
        FfiError { message: message.into(), code }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}
