//! Type definitions for the perry/container module.

pub use perry_container_compose::types::{
    BuildSpec, ComposeDependsOn, ComposeDeployment,
    ComposeDeploymentResources, ComposeHealthcheck, ComposeLogging, ComposeNetwork,
    ComposeNetworkIpam, ComposeNetworkIpamConfig, ComposeResourceSpec, ComposeService,
    ComposeServiceBuild, ComposeServiceNetworkConfig, ComposeServicePort, ComposeServiceVolume,
    ComposeServiceVolumeBind, ComposeServiceVolumeImage, ComposeServiceVolumeOpts,
    ComposeServiceVolumeTmpfs, ComposeSpec, PortSpec, ServiceNetworks, VolumeEntry, VolumeType,
    ComposeHandle as RawComposeHandle, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, ContainerSpec,
    ListOrDict, DependsOnSpec
};

pub type ComposeVolume = perry_container_compose::types::ComposeVolume;

use perry_runtime::StringHeader;
use serde::{Deserialize, Serialize};
use crate::common::handle::{self, Handle};

// ============ Handle Registry ============

/// Register a `ContainerHandle` and return an opaque integer handle.
pub fn register_container_handle(h: ContainerHandle) -> u64 {
    handle::register_handle(h) as u64
}

/// Register a single `ContainerInfo` and return an opaque integer handle.
pub fn register_container_info(info: ContainerInfo) -> u64 {
    handle::register_handle(info) as u64
}

/// Register a `Vec<ContainerInfo>` (list result from `list` / `ps`) and return an opaque integer handle.
pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    handle::register_handle(list) as u64
}

/// Register a `ComposeEngine` and return an opaque integer handle.
pub fn register_compose_engine(stack_id: u64) -> u64 {
    stack_id
}

/// Retrieve a `ComposeEngine` by handle id.
pub fn get_compose_engine(id: u64) -> Option<std::sync::Arc<perry_container_compose::ComposeEngine>> {
    perry_container_compose::ComposeEngine::get_engine(id)
}

/// Take (remove and return) the `ComposeEngine` from the registry.
pub fn take_compose_engine(id: u64) -> Option<std::sync::Arc<perry_container_compose::ComposeEngine>> {
    perry_container_compose::ComposeEngine::remove_engine(id)
}

/// Register a string and return an opaque integer handle.
pub fn register_string(s: String) -> u64 {
    handle::register_handle(s) as u64
}

/// Register `ContainerLogs` and return an opaque integer handle.
pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    handle::register_handle(logs) as u64
}

/// Register a `Vec<ImageInfo>` and return an opaque integer handle.
pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    handle::register_handle(list) as u64
}

/// Take (remove and return) the container info list from the registry.
pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    handle::take_handle(id as Handle)
}

/// Take (remove and return) `ContainerLogs` from the registry.
pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    handle::take_handle(id as Handle)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHandle {
    pub stack_id: u64,
    pub project_name: String,
    pub services: Vec<String>,
}

impl From<RawComposeHandle> for ComposeHandle {
    fn from(raw: RawComposeHandle) -> Self {
        Self {
            stack_id: raw.stack_id,
            project_name: raw.project_name,
            services: raw.services,
        }
    }
}

// ============ Error Types ============

/// Container module errors.
#[derive(Debug, Clone)]
pub enum ContainerError {
    NotFound(String),
    BackendError { code: i32, message: String },
    VerificationFailed { image: String, reason: String },
    DependencyCycle { cycle: Vec<String> },
    ServiceStartupFailed { service: String, error: String },
    InvalidConfig(String),
    NoBackendFound { probed: Vec<perry_container_compose::backend::BackendProbeResult> },
    BackendNotAvailable { name: String, reason: String },
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
            ContainerError::NoBackendFound { probed } => write!(f, "No container backend found. Probed: {:?}", probed),
            ContainerError::BackendNotAvailable { name, reason } => write!(f, "Backend {} not available: {}", name, reason),
        }
    }
}

impl std::error::Error for ContainerError {}

impl From<perry_container_compose::error::ComposeError> for ContainerError {
    fn from(e: perry_container_compose::error::ComposeError) -> Self {
        match e {
            perry_container_compose::error::ComposeError::NotFound(id) => {
                ContainerError::NotFound(id)
            }
            perry_container_compose::error::ComposeError::DependencyCycle { services } => {
                ContainerError::DependencyCycle { cycle: services }
            }
            perry_container_compose::error::ComposeError::ServiceStartupFailed { service, message } => {
                ContainerError::ServiceStartupFailed { service, error: message }
            }
            perry_container_compose::error::ComposeError::ValidationError { message } => {
                ContainerError::InvalidConfig(message)
            }
            perry_container_compose::error::ComposeError::NoBackendFound { probed } => {
                ContainerError::NoBackendFound { probed }
            }
            perry_container_compose::error::ComposeError::BackendNotAvailable { name, reason } => {
                ContainerError::BackendNotAvailable { name, reason }
            }
            other => ContainerError::BackendError {
                code: -1,
                message: other.to_string(),
            },
        }
    }
}

// ============ StringHeader Parsing ============

/// Parse `ContainerSpec` from a JSON StringHeader pointer.
pub unsafe fn parse_container_spec_json(ptr: *const StringHeader) -> Result<ContainerSpec, String> {
    let s = string_from_header(ptr).ok_or("Invalid spec pointer")?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

/// Parse `ComposeSpec` from a JSON StringHeader pointer.
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
