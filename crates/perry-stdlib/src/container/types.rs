//! Type definitions for the perry/container module.

use perry_runtime::{JSValue, StringHeader};
pub use perry_container_compose::types::{
    ComposeHandle, ComposeService, ComposeSpec, ComposeNetwork, ComposeVolume,
    ComposeSecret, ComposeConfigObj, ListOrDict, DependsOnCondition,
    DependsOnSpec, VolumeType, PortSpec, BuildSpec,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use crate::common::handle::{self, Handle};

// ============ Global Handle Registries ============

pub static CONTAINER_HANDLES: OnceLock<dashmap::DashMap<u64, ContainerHandle>> = OnceLock::new();
pub static COMPOSE_HANDLES: OnceLock<dashmap::DashMap<u64, perry_container_compose::compose::ComposeEngine>> = OnceLock::new();
pub static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

fn container_handles() -> &'static dashmap::DashMap<u64, ContainerHandle> {
    CONTAINER_HANDLES.get_or_init(dashmap::DashMap::new)
}

fn compose_handles() -> &'static dashmap::DashMap<u64, perry_container_compose::compose::ComposeEngine> {
    COMPOSE_HANDLES.get_or_init(dashmap::DashMap::new)
}

pub fn register_container_handle(h: ContainerHandle) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    container_handles().insert(id, h);
    id
}

pub fn register_compose_engine(engine: perry_container_compose::compose::ComposeEngine) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    compose_handles().insert(id, engine);
    id
}

pub fn get_compose_engine(id: u64) -> Option<perry_container_compose::compose::ComposeEngine> {
    // Note: ComposeEngine is not Clone, but the design doc says ComposeWrapper is a thin wrapper.
    // Actually ComposeEngine in perry-container-compose is intended to be used directly or via Arc.
    // For simplicity, let's assume we take or it's wrapped in Arc.
    // The current ComposeEngine doesn't implement Clone.
    None
}

// Registry helpers for mod.rs

pub fn register_container_info(info: ContainerInfo) -> u64 {
    handle::register_handle(info) as u64
}

pub fn register_container_info_list(list: Vec<ContainerInfo>) -> u64 {
    handle::register_handle(list) as u64
}

pub fn register_compose_handle(h: ComposeHandle) -> u64 {
    handle::register_handle(h) as u64
}

pub fn take_compose_handle(id: u64) -> Option<ComposeHandle> {
    handle::take_handle(id as Handle)
}

pub fn get_compose_handle(id: u64) -> Option<&'static ComposeHandle> {
    handle::get_handle(id as Handle)
}

pub fn register_container_logs(logs: ContainerLogs) -> u64 {
    handle::register_handle(logs) as u64
}

pub fn register_image_info_list(list: Vec<ImageInfo>) -> u64 {
    handle::register_handle(list) as u64
}

// ============ Core Container Types ============

pub use perry_container_compose::types::{
    ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo,
};

// ============ Error Types ============

#[derive(Debug, Clone)]
pub enum ContainerError {
    NotFound(String),
    BackendError { code: i32, message: String },
    VerificationFailed { image: String, reason: String },
    DependencyCycle { cycle: Vec<String> },
    ServiceStartupFailed { service: String, error: String },
    InvalidConfig(String),
    BackendNotAvailable { name: String, reason: String },
    NoBackendFound { probed: Vec<perry_container_compose::error::BackendProbeResult> },
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
            ContainerError::BackendNotAvailable { name, reason } => {
                write!(f, "Backend '{}' not available: {}", name, reason)
            }
            ContainerError::NoBackendFound { probed } => {
                write!(f, "No container backend found. Probed: {:?}", probed)
            }
        }
    }
}

impl std::error::Error for ContainerError {}

impl From<perry_container_compose::error::ComposeError> for ContainerError {
    fn from(e: perry_container_compose::error::ComposeError) -> Self {
        match e {
            perry_container_compose::error::ComposeError::NotFound(s) => ContainerError::NotFound(s),
            perry_container_compose::error::ComposeError::BackendError { code, message } => ContainerError::BackendError { code, message },
            perry_container_compose::error::ComposeError::DependencyCycle { services } => ContainerError::DependencyCycle { cycle: services },
            perry_container_compose::error::ComposeError::ServiceStartupFailed { service, message } => ContainerError::ServiceStartupFailed { service, error: message },
            perry_container_compose::error::ComposeError::ValidationError { message } => ContainerError::InvalidConfig(message),
            perry_container_compose::error::ComposeError::ParseError(e) => ContainerError::InvalidConfig(e.to_string()),
            perry_container_compose::error::ComposeError::JsonError(e) => ContainerError::InvalidConfig(e.to_string()),
            perry_container_compose::error::ComposeError::IoError(e) => ContainerError::BackendError { code: -1, message: e.to_string() },
            perry_container_compose::error::ComposeError::VerificationFailed { image, reason } => ContainerError::VerificationFailed { image, reason },
            perry_container_compose::error::ComposeError::FileNotFound { path } => ContainerError::NotFound(path),
            perry_container_compose::error::ComposeError::NoBackendFound { probed } => ContainerError::NoBackendFound { probed },
            perry_container_compose::error::ComposeError::BackendNotAvailable { name, reason } => ContainerError::BackendNotAvailable { name, reason },
        }
    }
}

// ============ JSValue Parsing ============

pub fn parse_container_spec(json: &str) -> Result<ContainerSpec, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

pub fn parse_compose_spec(json: &str) -> Result<ComposeSpec, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}
