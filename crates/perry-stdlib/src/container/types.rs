//! Type definitions for container module

use perry_runtime::{JSValue, StringHeader};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// ============ Handle Management ============

static NEXT_CONTAINER_HANDLE: AtomicU64 = AtomicU64::new(1);

/// Register a ContainerHandle and return its ID
pub fn register_container_handle(_handle: ContainerHandle) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

/// Register a ContainerInfo and return its ID
pub fn register_container_info(_info: ContainerInfo) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

/// Register a list of ContainerInfo and return its ID
pub fn register_container_info_list(_list: Vec<ContainerInfo>) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

/// Register a ComposeHandle and return its ID
pub fn register_compose_handle(_handle: ComposeHandle) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

/// Register ContainerLogs and return its ID
pub fn register_container_logs(_logs: ContainerLogs) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

/// Register a list of ImageInfo and return its ID
pub fn register_image_info_list(_list: Vec<ImageInfo>) -> u64 {
    NEXT_CONTAINER_HANDLE.fetch_add(1, Ordering::SeqCst)
}

// ============ Container Types ============

/// Configuration for a single container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSpec {
    /// Container image (required)
    pub image: String,
    /// Container name (optional)
    pub name: Option<String>,
    /// Port mappings (e.g., "8080:80")
    pub ports: Option<Vec<String>>,
    /// Volume mounts (e.g., "/host/path:/container/path:ro")
    pub volumes: Option<Vec<String>>,
    /// Environment variables
    pub env: Option<HashMap<String, String>>,
    /// Command to run (overrides image CMD)
    pub cmd: Option<Vec<String>>,
    /// Entrypoint (overrides image ENTRYPOINT)
    pub entrypoint: Option<Vec<String>>,
    /// Network to attach to
    pub network: Option<String>,
    /// Remove container on exit
    pub rm: Option<bool>,
}

/// Handle to a container instance
#[derive(Debug, Clone)]
pub struct ContainerHandle {
    /// Container ID
    pub id: String,
    /// Container name (if specified)
    pub name: Option<String>,
}

/// Information about a container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    /// Container ID
    pub id: String,
    /// Container name
    pub name: String,
    /// Image reference
    pub image: String,
    /// Container status (e.g., "running", "exited")
    pub status: String,
    /// Port mappings
    pub ports: Vec<String>,
    /// Creation timestamp (ISO 8601)
    pub created: String,
}

/// Logs captured from a container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLogs {
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
}

/// Information about a container image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    /// Image ID
    pub id: String,
    /// Repository name
    pub repository: String,
    /// Image tag
    pub tag: String,
    /// Image size in bytes
    pub size: u64,
    /// Creation timestamp (ISO 8601)
    pub created: String,
}

// ============ Compose Types ============

/// Multi-container application specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeSpec {
    /// Compose file version
    pub version: Option<String>,
    /// Service definitions
    pub services: HashMap<String, ComposeService>,
    /// Network definitions
    pub networks: Option<HashMap<String, ComposeNetwork>>,
    /// Volume definitions
    pub volumes: Option<HashMap<String, ComposeVolume>>,
}

/// Service definition in Compose
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeService {
    /// Container image
    pub image: String,
    /// Build configuration
    pub build: Option<ComposeBuild>,
    /// Command to run
    pub command: Option<ComposeCommand>,
    /// Environment variables
    pub environment: Option<ComposeEnvironment>,
    /// Port mappings
    pub ports: Option<Vec<String>>,
    /// Volume mounts
    pub volumes: Option<Vec<String>>,
    /// Networks to attach to
    pub networks: Option<Vec<String>>,
    /// Service dependencies
    pub depends_on: Option<Vec<String>>,
    /// Restart policy
    pub restart: Option<String>,
    /// Healthcheck configuration
    pub healthcheck: Option<ComposeHealthcheck>,
}

/// Build configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeBuild {
    /// Build context directory
    pub context: String,
    /// Dockerfile path (relative to context)
    pub dockerfile: Option<String>,
}

/// Command can be a string or array of strings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeCommand {
    String(String),
    Array(Vec<String>),
}

/// Environment can be a map or array of strings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeEnvironment {
    Map(HashMap<String, String>),
    Array(Vec<String>),
}

/// Healthcheck configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHealthcheck {
    /// Test command (string or array)
    pub test: ComposeCommand,
    /// Check interval (e.g., "30s")
    pub interval: Option<String>,
    /// Timeout (e.g., "10s")
    pub timeout: Option<String>,
    /// Number of retries before unhealthy
    pub retries: Option<u32>,
    /// Startup grace period (e.g., "40s")
    pub start_period: Option<String>,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeNetwork {
    /// Network driver
    pub driver: Option<String>,
    /// External network reference
    pub external: Option<bool>,
    /// Network name
    pub name: Option<String>,
}

/// Volume configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeVolume {
    /// Volume driver
    pub driver: Option<String>,
    /// External volume reference
    pub external: Option<bool>,
    /// Volume name
    pub name: Option<String>,
}

/// Handle to a Compose stack
#[derive(Debug, Clone)]
pub struct ComposeHandle {
    /// Stack name
    pub name: String,
    /// Services in the stack
    pub services: Vec<String>,
    /// Created networks
    pub networks: Vec<String>,
    /// Created volumes
    pub volumes: Vec<String>,
    /// Container handles for each service
    pub containers: HashMap<String, ContainerHandle>,
}

// ============ Error Types ============

/// Container module errors
#[derive(Debug, Clone)]
pub enum ContainerError {
    /// Container not found
    NotFound(String),
    /// Backend execution error
    BackendError { code: i32, message: String },
    /// Image verification failed
    VerificationFailed { image: String, reason: String },
    /// Dependency cycle in compose
    DependencyCycle { cycle: Vec<String> },
    /// Service startup failed
    ServiceStartupFailed { service: String, error: String },
    /// Invalid configuration
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

// ============ JSValue Parsing Functions ============

/// Parse ContainerSpec from JSValue
/// NOTE: This is a simplified implementation. Full JSValue parsing should be
/// done in the compiler's HIR lowering phase, not at runtime.
pub fn parse_container_spec(_spec_ptr: *const JSValue) -> Result<ContainerSpec, String> {
    // TODO: Implement proper JSValue parsing
    // For now, return a default spec that can be used for testing
    // In production, the compiler should convert TypeScript ContainerSpec
    // directly to Rust ContainerSpec via codegen, avoiding runtime parsing.
    Err("ContainerSpec parsing must be done at compile time. The compiler should generate native code that constructs ContainerSpec directly.".to_string())
}

/// Parse ComposeSpec from JSValue
/// NOTE: This is a simplified implementation.
pub fn parse_compose_spec(_spec_ptr: *const JSValue) -> Result<ComposeSpec, String> {
    // TODO: Implement proper JSValue parsing
    // For now, return a default spec for testing
    // In production, the compiler should convert TypeScript ComposeSpec
    // directly to Rust ComposeSpec via codegen.
    Err("ComposeSpec parsing must be done at compile time. The compiler should generate native code that constructs ComposeSpec directly.".to_string())
}
