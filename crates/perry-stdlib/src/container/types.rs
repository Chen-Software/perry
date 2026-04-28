//! Type definitions for the perry/container module.

use perry_runtime::StringHeader;
pub use perry_container_compose::types::{
    ComposeHandle, ComposeSpec, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo, ListOrDict, ComposeServiceBuild,
};
pub use perry_container_compose::error::BackendProbeResult;
use perry_container_compose::error::ComposeError;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use dashmap::DashMap;

use crate::common::handle::{self, Handle};

pub struct ArcComposeEngine(pub Arc<perry_container_compose::ComposeEngine>);
pub static COMPOSE_HANDLES: OnceLock<DashMap<u64, ArcComposeEngine>> = OnceLock::new();

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

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    handle::register_handle(list) as u64
}

// ============ Error Types ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerError {
    NotFound(String),
    BackendError { code: i32, message: String },
    VerificationFailed { image: String, reason: String },
    DependencyCycle { cycle: Vec<String> },
    ServiceStartupFailed { service: String, error: String },
    InvalidConfig(String),
    NoBackendFound { probed: Vec<BackendProbeResult> },
    BackendNotAvailable { name: String, reason: String },
}

impl std::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerError::NotFound(id) => write!(f, "Not found: {}", id),
            ContainerError::BackendError { code, message } => write!(f, "Backend error ({}): {}", code, message),
            ContainerError::VerificationFailed { image, reason } => write!(f, "Verification failed for {}: {}", image, reason),
            ContainerError::DependencyCycle { cycle } => write!(f, "Dependency cycle: {:?}", cycle),
            ContainerError::ServiceStartupFailed { service, error } => write!(f, "Service {} failed: {}", service, error),
            ContainerError::InvalidConfig(m) => write!(f, "Invalid config: {}", m),
            ContainerError::NoBackendFound { probed } => write!(f, "No backend found. Probed: {:?}", probed),
            ContainerError::BackendNotAvailable { name, reason } => write!(f, "Backend {} not available: {}", name, reason),
        }
    }
}

impl std::error::Error for ContainerError {}

impl From<ComposeError> for ContainerError {
    fn from(e: ComposeError) -> Self {
        match e {
            ComposeError::NotFound(s) => ContainerError::NotFound(s),
            ComposeError::BackendError { code, message } => ContainerError::BackendError { code, message },
            ComposeError::VerificationFailed { image, reason } => ContainerError::VerificationFailed { image, reason },
            ComposeError::DependencyCycle { services } => ContainerError::DependencyCycle { cycle: services },
            ComposeError::ServiceStartupFailed { service, message } => ContainerError::ServiceStartupFailed { service, error: message },
            ComposeError::ValidationError { message } => ContainerError::InvalidConfig(message),
            ComposeError::NoBackendFound { probed } => ContainerError::NoBackendFound { probed },
            ComposeError::BackendNotAvailable { name, reason } => ContainerError::BackendNotAvailable { name, reason },
            ComposeError::ParseError(e) => ContainerError::InvalidConfig(e.to_string()),
            ComposeError::JsonError(e) => ContainerError::InvalidConfig(e.to_string()),
            ComposeError::IoError(e) => ContainerError::BackendError { code: -1, message: e.to_string() },
            ComposeError::FileNotFound { path } => ContainerError::NotFound(format!("File not found: {}", path)),
        }
    }
}

pub fn parse_container_spec(spec_ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let json = unsafe { string_from_header(spec_ptr) }.ok_or("Invalid spec pointer")?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

pub fn parse_compose_spec(spec_ptr: *const StringHeader) -> Result<ComposeSpec, String> {
    let json = unsafe { string_from_header(spec_ptr) }.ok_or("Invalid spec pointer")?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 { return None; }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

pub fn container_error_to_json(e: ContainerError) -> String {
    let code = match &e {
        ContainerError::NotFound(_) => 404,
        ContainerError::BackendError { code, .. } => *code,
        ContainerError::DependencyCycle { .. } => 422,
        _ => 500,
    };
    serde_json::json!({ "message": e.to_string(), "code": code }).to_string()
}
