//! Type definitions for the perry/container module.

use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};

// Re-export core types from the compose crate to avoid duplication and mismatch.
pub use perry_container_compose::types::{
    ComposeHandle, ComposeSpec, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo, ListOrDict,
};
pub use perry_container_compose::ComposeEngine;

// ============ Global Registries ============

pub static CONTAINER_HANDLES: OnceLock<DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static COMPOSE_HANDLES: OnceLock<DashMap<u64, ArcComposeEngine>> = OnceLock::new();
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct ArcComposeEngine(pub std::sync::Arc<ComposeEngine>);

pub fn register_container_handle(h: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    CONTAINER_HANDLES.get_or_init(DashMap::new).insert(id, h);
    id
}

pub static INFO_LIST_HANDLES: OnceLock<DashMap<u64, Vec<ContainerInfo>>> = OnceLock::new();
pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    INFO_LIST_HANDLES.get_or_init(DashMap::new).insert(id, list);
    id
}
pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    INFO_LIST_HANDLES.get_or_init(DashMap::new).remove(&id).map(|(_, v)| v)
}

pub static LOG_HANDLES: OnceLock<DashMap<u64, ContainerLogs>> = OnceLock::new();
pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    LOG_HANDLES.get_or_init(DashMap::new).insert(id, logs);
    id
}
pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    LOG_HANDLES.get_or_init(DashMap::new).remove(&id).map(|(_, v)| v)
}

pub fn register_compose_handle(engine: ComposeEngine) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    COMPOSE_HANDLES.get_or_init(DashMap::new).insert(id, ArcComposeEngine(std::sync::Arc::new(engine)));
    id
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
            perry_container_compose::error::ComposeError::ImagePullFailed { service, image, message } => {
                ContainerError::ServiceStartupFailed {
                    service: format!("{} (pull: {})", service, image),
                    error: message,
                }
            }
        }
    }
}

// ============ JSON Parsing ============

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

/// Parse `ContainerSpec` from a JSON StringHeader pointer.
pub fn parse_container_spec(spec_ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let json = unsafe { string_from_header(spec_ptr) }.ok_or("Invalid spec pointer")?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

/// Convert a `ContainerError` to a JSON error string for TS.
pub fn container_error_to_json(e: ContainerError) -> String {
    let code = match &e {
        ContainerError::NotFound(_) => 404,
        ContainerError::BackendError { code, .. } => *code,
        ContainerError::DependencyCycle { .. } => 422,
        ContainerError::InvalidConfig(_) => 400,
        ContainerError::VerificationFailed { .. } => 403,
        ContainerError::NoBackendFound { .. } => 503,
        ContainerError::BackendNotAvailable { .. } => 503,
        _ => 500,
    };
    serde_json::json!({
        "message": e.to_string(),
        "code": code
    })
    .to_string()
}

/// Parse `ComposeSpec` from a JSON StringHeader pointer.
pub fn parse_compose_spec(spec_ptr: *const StringHeader) -> Result<ComposeSpec, String> {
    let json = unsafe { string_from_header(spec_ptr) }.ok_or("Invalid spec pointer")?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}
