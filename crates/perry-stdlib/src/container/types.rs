//! Type definitions for the perry/container module.
//!
//! All types here conform to the [compose-spec JSON schema](https://github.com/compose-spec/compose-spec/blob/main/schema/compose-spec.json)
//! and are used both as the TypeScript-facing API surface and as the internal
//! Rust representation passed to the ComposeEngine.

use perry_runtime::StringHeader;
pub use perry_container_compose::types::{
    ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo,
    ComposeSpec, ComposeService, ComposeHandle, ListOrDict,
    ComposeNetwork, ComposeVolume,
};

use crate::common::handle::{self, Handle};

// ============ Handle Registry ============

pub fn register_container_handle(h: ContainerHandle) -> u64 {
    handle::register_handle(h) as u64
}

pub fn register_container_info(info: ContainerInfo) -> u64 {
    handle::register_handle(info) as u64
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    handle::register_handle(list) as u64
}

pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    handle::take_handle::<Vec<ContainerInfo>>(id as Handle)
}

pub fn register_compose_engine(engine: std::sync::Arc<perry_container_compose::ComposeEngine>, stack_id: u64) -> u64 {
    handle::register_handle_with_id(engine, stack_id as Handle) as u64
}

pub fn get_compose_engine(id: u64) -> Option<std::sync::Arc<perry_container_compose::ComposeEngine>> {
    handle::get_handle::<std::sync::Arc<perry_container_compose::ComposeEngine>>(id as Handle).cloned()
}

pub fn register_string(s: String) -> u64 {
    handle::register_handle(s) as u64
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    handle::register_handle(logs) as u64
}

pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    handle::take_handle::<ContainerLogs>(id as Handle)
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    handle::register_handle(list) as u64
}

// ============ Error Types ============

#[derive(Debug, Clone)]
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

impl From<perry_container_compose::ComposeError> for ContainerError {
    fn from(e: perry_container_compose::ComposeError) -> Self {
        match e {
            perry_container_compose::ComposeError::NotFound(id) => ContainerError::NotFound(id),
            perry_container_compose::ComposeError::DependencyCycle { services } => {
                ContainerError::DependencyCycle { cycle: services }
            }
            perry_container_compose::ComposeError::ServiceStartupFailed { service, message } => {
                ContainerError::ServiceStartupFailed {
                    service,
                    error: message,
                }
            }
            perry_container_compose::ComposeError::ValidationError { message } => {
                ContainerError::InvalidConfig(message)
            }
            other => ContainerError::BackendError {
                code: -1,
                message: other.to_string(),
            },
        }
    }
}

// ============ StringHeader Parsing ============

pub unsafe fn parse_container_spec_json(ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let s = string_from_header(ptr).ok_or("Invalid spec pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

pub unsafe fn parse_compose_spec_json(ptr: *const StringHeader) -> Result<perry_container_compose::ComposeSpec, String> {
    let s = string_from_header(ptr).ok_or("Invalid spec pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 { return None; }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}
