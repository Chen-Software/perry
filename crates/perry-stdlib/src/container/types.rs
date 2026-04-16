//! Type definitions for the perry/container module.
//!
//! All types here conform to the [compose-spec JSON schema](https://github.com/compose-spec/compose-spec/blob/main/schema/compose-spec.json)
//! and are used both as the TypeScript-facing API surface and as the internal
//! Rust representation passed to the ComposeEngine.

use perry_runtime::{JSValue, StringHeader};
use serde::{Deserialize, Serialize};

use crate::common::handle::{self, Handle};

pub use perry_container_compose::types::{
    ComposeSpec, ComposeService, ComposeHandle,
    ComposeNetwork, ComposeVolume, ComposeSecret, ComposeConfigObj,
    ComposeDependsOn, DependsOnSpec, DependsOnCondition,
    ListOrDict, PortSpec, ServiceNetworks, BuildSpec,
    ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo,
};

// ============ Handle Registry ============
//
// All container-related opaque objects are stored in the global DashMap-based
// handle registry (crate::common::handle) so they can be retrieved later by
// their integer handle from the JS side (e.g. composeHandle.ps(), etc.).

/// Register a `ContainerHandle` and return an opaque integer handle.
pub fn register_container_handle(h: ContainerHandle) -> u64 {
    handle::register_handle(h) as u64
}

/// Retrieve a `ContainerHandle` by handle id (read-only).
pub fn get_container_handle(id: u64) -> Option<handle::Handle> {
    let h = id as Handle;
    if handle::handle_exists(h) { Some(h) } else { None }
}

/// Register a single `ContainerInfo` and return an opaque integer handle.
pub fn register_container_info(info: ContainerInfo) -> u64 {
    handle::register_handle(info) as u64
}

/// Register a `Vec<ContainerInfo>` (list result from `list` / `ps`) and return an opaque integer handle.
pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    handle::register_handle(list) as u64
}

/// Retrieve the container info list associated with a handle.
pub fn with_container_info_list<R>(id: u64, f: impl FnOnce(&Vec<ContainerInfo>) -> R) -> Option<R> {
    handle::with_handle(id as Handle, f)
}

/// Take (remove and return) the container info list from the registry.
pub fn take_container_info_list(id: u64) -> Option<Vec<ContainerInfo>> {
    handle::take_handle(id as Handle)
}

/// Register a `ComposeHandle` and return an opaque integer handle.
pub fn register_compose_handle(h: ComposeHandle) -> u64 {
    handle::register_handle(h) as u64
}

/// Retrieve a `ComposeHandle` by handle id.
pub fn get_compose_handle(id: u64) -> Option<&'static ComposeHandle> {
    handle::get_handle(id as Handle)
}

/// Take (remove and return) the `ComposeHandle` from the registry.
pub fn take_compose_handle(id: u64) -> Option<ComposeHandle> {
    handle::take_handle(id as Handle)
}

/// Register `ContainerLogs` and return an opaque integer handle.
pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    handle::register_handle(logs) as u64
}

/// Retrieve `ContainerLogs` by handle id (read-only).
pub fn with_container_logs<R>(id: u64, f: impl FnOnce(&ContainerLogs) -> R) -> Option<R> {
    handle::with_handle(id as Handle, f)
}

/// Take (remove and return) `ContainerLogs` from the registry.
pub fn take_container_logs(id: u64) -> Option<ContainerLogs> {
    handle::take_handle(id as Handle)
}

/// Register a `Vec<ImageInfo>` and return an opaque integer handle.
pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    handle::register_handle(list) as u64
}

/// Retrieve the image info list associated with a handle.
pub fn with_image_info_list<R>(id: u64, f: impl FnOnce(&Vec<ImageInfo>) -> R) -> Option<R> {
    handle::with_handle(id as Handle, f)
}

/// Take (remove and return) the image info list from the registry.
pub fn take_image_info_list(id: u64) -> Option<Vec<ImageInfo>> {
    handle::take_handle(id as Handle)
}

/// Drop a handle from the registry (force cleanup from JS GC / explicit close).
pub fn drop_container_handle(id: u64) -> bool {
    handle::drop_handle(id as Handle)
}

// ============ Error Types ============

/// Container module errors.
#[derive(Debug, Clone, Serialize)]
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
            ContainerError::NoBackendFound { probed } => {
                write!(f, "No container backend found. Probed: {:?}", probed)
            }
            ContainerError::BackendNotAvailable { name, reason } => {
                write!(f, "Backend '{}' is not available: {}", name, reason)
            }
        }
    }
}

impl std::error::Error for ContainerError {}

impl From<perry_container_compose::error::ComposeError> for ContainerError {
    fn from(e: perry_container_compose::error::ComposeError) -> Self {
        match e {
            perry_container_compose::error::ComposeError::NotFound(id) => ContainerError::NotFound(id),
            perry_container_compose::error::ComposeError::BackendError { code, message } => ContainerError::BackendError { code, message },
            perry_container_compose::error::ComposeError::VerificationFailed { image, reason } => ContainerError::VerificationFailed { image, reason },
            perry_container_compose::error::ComposeError::DependencyCycle { services } => ContainerError::DependencyCycle { cycle: services },
            perry_container_compose::error::ComposeError::ServiceStartupFailed { service, message } => ContainerError::ServiceStartupFailed { service, error: message },
            perry_container_compose::error::ComposeError::ValidationError { message } => ContainerError::InvalidConfig(message),
            perry_container_compose::error::ComposeError::NoBackendFound { probed } => ContainerError::NoBackendFound { probed },
            perry_container_compose::error::ComposeError::BackendNotAvailable { name, reason } => ContainerError::BackendNotAvailable { name, reason },
            other => ContainerError::BackendError { code: -1, message: other.to_string() },
        }
    }
}
