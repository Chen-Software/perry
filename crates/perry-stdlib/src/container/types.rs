//! Type definitions for the perry/container module.

use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use dashmap::DashMap;
use perry_container_compose::ComposeEngine;

use crate::common::handle::{self, Handle};

// Re-export core types from the compose crate to avoid duplication and mismatch.
pub use perry_container_compose::types::{
    ComposeHandle, ComposeSpec, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo, ListOrDict,
};

// ============ Typed Global Handle Registries ============

/// Global registry mapping handle IDs to `ContainerHandle` values.
pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();

/// Global registry mapping handle IDs to live `ComposeEngine` instances.
pub static COMPOSE_HANDLES: OnceLock<DashMap<u64, ComposeEngine>> = OnceLock::new();

/// Monotonically increasing counter for typed handle IDs.
pub static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

/// Get (or lazily initialise) the global `CONTAINER_HANDLES` map.
fn container_handles() -> &'static DashMap<u64, ContainerHandle> {
    CONTAINER_HANDLES.get_or_init(DashMap::new)
}

/// Get (or lazily initialise) the global `COMPOSE_HANDLES` map.
fn compose_handles() -> &'static DashMap<u64, ComposeEngine> {
    COMPOSE_HANDLES.get_or_init(DashMap::new)
}

// ============ Handle Registry ============

/// Register a `ContainerHandle` in the typed registry and return its ID.
pub fn register_container_handle(h: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    container_handles().insert(id, h);
    id
}

/// Look up a `ContainerHandle` by ID (returns a cloned value).
pub fn get_container_handle_typed(id: u64) -> Option<ContainerHandle> {
    container_handles().get(&id).map(|r| r.clone())
}

/// Remove and return a `ContainerHandle` from the typed registry.
pub fn take_container_handle_typed(id: u64) -> Option<ContainerHandle> {
    container_handles().remove(&id).map(|(_, v)| v)
}

/// Register a `ComposeEngine` in the typed registry and return its ID.
pub fn register_compose_engine(engine: ComposeEngine) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    compose_handles().insert(id, engine);
    id
}

/// Borrow a `ComposeEngine` from the typed registry.
pub fn with_compose_engine<R>(id: u64, f: impl FnOnce(&ComposeEngine) -> R) -> Option<R> {
    compose_handles().get(&id).map(|r| f(&*r))
}

/// Get an `Arc<ComposeEngine>` from the typed registry.
pub fn get_compose_engine_arc(id: u64) -> Option<Arc<ComposeEngine>> {
    compose_handles().get(&id).map(|r| Arc::new(r.clone()))
}

/// Remove and return a `ComposeEngine` from the typed registry.
pub fn take_compose_engine(id: u64) -> Option<ComposeEngine> {
    compose_handles().remove(&id).map(|(_, v)| v)
}

// ---- Legacy generic-handle helpers (kept for backward compatibility) ----

pub fn get_container_handle(id: u64) -> Option<Handle> {
    let h = id as Handle;
    if handle::handle_exists(h) {
        Some(h)
    } else {
        None
    }
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    handle::register_handle(info) as u64
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    handle::register_handle(list) as u64
}

pub fn with_container_info_list<R>(id: u64, f: impl FnOnce(&Vec<ContainerInfo>) -> R) -> Option<R> {
    handle::with_handle(id as Handle, f)
}

pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    handle::take_handle(id as Handle)
}

pub fn register_compose_handle(h: ComposeHandle) -> u64 {
    handle::register_handle(h) as u64
}

pub fn get_compose_handle(id: u64) -> Option<&'static ComposeHandle> {
    handle::get_handle(id as Handle)
}

pub fn take_compose_handle(id: u64) -> Option<ComposeHandle> {
    handle::take_handle(id as Handle)
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    handle::register_handle(logs) as u64
}

pub fn with_container_logs<R>(id: u64, f: impl FnOnce(&ContainerLogs) -> R) -> Option<R> {
    handle::with_handle(id as Handle, f)
}

pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    handle::take_handle(id as Handle)
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    handle::register_handle(list) as u64
}

pub fn with_image_info_list<R>(id: u64, f: impl FnOnce(&Vec<ImageInfo>) -> R) -> Option<R> {
    handle::with_handle(id as Handle, f)
}

pub fn take_image_info_list(id: u64) -> Option<Vec<ImageInfo>> {
    handle::take_handle(id as Handle)
}

pub fn drop_container_handle(id: u64) -> bool {
    handle::drop_handle(id as Handle)
}

// ============ Error Types ============

/// Container module errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerError {
    NotFound(String),
    BackendError {
        code: i32,
        message: String,
    },
    VerificationFailed {
        image: String,
        reason: String,
    },
    DependencyCycle {
        cycle: Vec<String>,
    },
    ServiceStartupFailed {
        service: String,
        error: String,
    },
    InvalidConfig(String),
    NoBackendFound {
        probed: Vec<crate::container::backend::BackendProbeResult>,
    },
    BackendNotAvailable {
        name: String,
        reason: String,
    },
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
            ContainerError::NoBackendFound { probed } => {
                write!(f, "No container backend found. Probed: {:?}", probed)
            }
            ContainerError::BackendNotAvailable { name, reason } => {
                write!(f, "Backend '{}' not available: {}", name, reason)
            }
        }
    }
}

impl std::error::Error for ContainerError {}

impl From<perry_container_compose::error::ComposeError> for ContainerError {
    fn from(e: perry_container_compose::error::ComposeError) -> Self {
        match e {
            perry_container_compose::error::ComposeError::NotFound(s) => ContainerError::NotFound(s),
            perry_container_compose::error::ComposeError::BackendError { code, message } => {
                ContainerError::BackendError { code, message }
            }
            perry_container_compose::error::ComposeError::VerificationFailed { image, reason } => {
                ContainerError::VerificationFailed { image, reason }
            }
            perry_container_compose::error::ComposeError::DependencyCycle { services } => {
                ContainerError::DependencyCycle { cycle: services }
            }
            perry_container_compose::error::ComposeError::ServiceStartupFailed {
                service,
                message,
            } => ContainerError::ServiceStartupFailed {
                service,
                error: message,
            },
            perry_container_compose::error::ComposeError::ValidationError { message } => {
                ContainerError::InvalidConfig(message)
            }
            perry_container_compose::error::ComposeError::NoBackendFound { probed } => {
                ContainerError::NoBackendFound { probed }
            }
            perry_container_compose::error::ComposeError::BackendNotAvailable { name, reason } => {
                ContainerError::BackendNotAvailable { name, reason }
            }
            perry_container_compose::error::ComposeError::ParseError(e) => {
                ContainerError::InvalidConfig(e.to_string())
            }
            perry_container_compose::error::ComposeError::JsonError(e) => {
                ContainerError::InvalidConfig(e.to_string())
            }
            perry_container_compose::error::ComposeError::IoError(e) => {
                ContainerError::BackendError {
                    code: -1,
                    message: e.to_string(),
                }
            }
            perry_container_compose::error::ComposeError::FileNotFound { path } => {
                ContainerError::NotFound(format!("File not found: {}", path))
            }
        }
    }
}

// ============ JSON Parsing ============

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

/// Parse `ContainerSpec` from a JSON StringHeader pointer.
pub fn parse_container_spec(spec_ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let json = unsafe { string_from_header(spec_ptr) }.ok_or("Invalid spec pointer")?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

/// Parse `ComposeSpec` from a JSON StringHeader pointer.
pub fn parse_compose_spec(spec_ptr: *const StringHeader) -> Result<ComposeSpec, String> {
    let json = unsafe { string_from_header(spec_ptr) }.ok_or("Invalid spec pointer")?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}
