use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, atomic::{AtomicU64, Ordering}};
use dashmap::DashMap;
use perry_container_compose::ComposeEngine;

// Re-export core types from the compose crate for consistency
pub use perry_container_compose::types::{
    ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo,
    ComposeSpec, ComposeService, ComposeNetwork, ComposeVolume, ComposeSecret, ComposeConfigObj,
    ComposeHealthcheck, ComposeDeployment, ListOrDict, ComposeHandle,
};

// ============ Global Handle Registry ============

static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
static COMPOSE_HANDLES: OnceLock<DashMap<u64, Arc<ComposeEngine>>> = OnceLock::new();
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

pub fn register_container_handle(handle: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, handle);
    id
}

pub fn get_container_handle(id: u64) -> Option<ContainerHandle> {
    CONTAINER_HANDLES.get_or_init(DashMap::new).get(&id).map(|h| h.clone())
}

pub fn register_compose_engine(engine: ComposeEngine) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    COMPOSE_HANDLES.get_or_init(DashMap::new).insert(id, Arc::new(engine));
    id
}

pub fn get_compose_engine(id: u64) -> Option<Arc<ComposeEngine>> {
    COMPOSE_HANDLES.get_or_init(DashMap::new).get(&id).map(|e| e.clone())
}

// ============ Registry for Info and Logs ============

static CONTAINER_INFO: OnceLock<DashMap<u64, ContainerInfo>> = OnceLock::new();
static CONTAINER_INFO_LISTS: OnceLock<DashMap<u64, Vec<ContainerInfo>>> = OnceLock::new();
static CONTAINER_LOGS: OnceLock<DashMap<u64, ContainerLogs>> = OnceLock::new();
static IMAGE_INFO_LISTS: OnceLock<DashMap<u64, Vec<ImageInfo>>> = OnceLock::new();
static STRINGS: OnceLock<DashMap<u64, String>> = OnceLock::new();

pub fn register_container_info(info: ContainerInfo) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_INFO.get_or_init(DashMap::new).insert(id, info);
    id
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_INFO_LISTS.get_or_init(DashMap::new).insert(id, list);
    id
}

pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    CONTAINER_INFO_LISTS.get_or_init(DashMap::new).remove(&id).map(|(_, v)| v)
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_LOGS.get_or_init(DashMap::new).insert(id, logs);
    id
}

pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    CONTAINER_LOGS.get_or_init(DashMap::new).remove(&id).map(|(_, v)| v)
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    IMAGE_INFO_LISTS.get_or_init(DashMap::new).insert(id, list);
    id
}

pub fn register_string(s: String) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    STRINGS.get_or_init(DashMap::new).insert(id, s);
    id
}

// ============ Error Types ============

/// Container module errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerError {
    NotFound(String),
    BackendError { code: i32, message: String },
    VerificationFailed { image: String, reason: String },
    DependencyCycle { cycle: Vec<String> },
    ServiceStartupFailed { service: String, error: String },
    InvalidConfig(String),
}

impl std::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerError::NotFound(id) => write!(f, "Container not found: {}", id),
            ContainerError::BackendError { code, message } => {
                write!(f, "Backend error (code {}): {}", code, message)
            }
            ContainerError::VerificationFailed { image, reason } => {
                write!(f, "Image verification failed for {}: {}", image, reason)
            }
            ContainerError::DependencyCycle { cycle } => {
                write!(f, "Dependency cycle detected: {}", cycle.join(" -> "))
            }
            ContainerError::ServiceStartupFailed { service, error } => {
                write!(f, "Service {} failed to start: {}", service, error)
            }
            ContainerError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
        }
    }
}

impl std::error::Error for ContainerError {}

// ============ StringHeader Parsing ============

/// Parse `ContainerSpec` from a JSON StringHeader pointer.
pub unsafe fn parse_container_spec_json(ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let s = string_from_header(ptr).ok_or("Invalid spec pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

/// Parse `ComposeSpec` from a JSON StringHeader pointer.
pub unsafe fn parse_compose_spec_json(ptr: *const StringHeader) -> Result<perry_container_compose::types::ComposeSpec, String> {
    let s = string_from_header(ptr).ok_or("Invalid spec pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 { return None; }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}
