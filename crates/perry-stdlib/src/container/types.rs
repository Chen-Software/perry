//! Type definitions for the perry/container module.
//!
//! All types here conform to the [compose-spec JSON schema](https://github.com/compose-spec/compose-spec/blob/main/schema/compose-spec.json)
//! and are used both as the TypeScript-facing API surface and as the internal
//! Rust representation passed to the ComposeEngine.

use perry_runtime::{JSValue, StringHeader};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::common::handle::{self, Handle};

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

/// Register a `ComposeEngine` and return an opaque integer handle.
pub fn register_compose_engine(engine: perry_container_compose::ComposeEngine, stack_id: u64) -> u64 {
    handle::register_handle_with_id(engine, stack_id as Handle) as u64
}

/// Retrieve a `ComposeEngine` by handle id.
pub fn get_compose_engine(id: u64) -> Option<&'static perry_container_compose::ComposeEngine> {
    handle::get_handle(id as Handle)
}

/// Take (remove and return) the `ComposeEngine` from the registry.
pub fn take_compose_engine(id: u64) -> Option<perry_container_compose::ComposeEngine> {
    handle::take_handle(id as Handle)
}

/// Register a string and return an opaque integer handle.
pub fn register_string(s: String) -> u64 {
    handle::register_handle(s) as u64
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

// ============ Core Container Types ============

/// Configuration for a single container.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerSpec {
    /// Container image (required)
    pub image: String,
    /// Container name (optional)
    pub name: Option<String>,
    /// Port mappings e.g. "8080:80"
    pub ports: Option<Vec<String>>,
    /// Volume mounts e.g. "/host:/container:ro"
    pub volumes: Option<Vec<String>>,
    /// Environment variables
    pub env: Option<HashMap<String, String>>,
    /// Command override
    pub cmd: Option<Vec<String>>,
    /// Entrypoint override
    pub entrypoint: Option<Vec<String>>,
    /// Network to attach to
    pub network: Option<String>,
    /// Remove container on exit
    pub rm: Option<bool>,
}

/// Opaque handle returned by `run()` / `create()`.
#[derive(Debug, Clone)]
pub struct ContainerHandle {
    pub id: String,
    pub name: Option<String>,
}

/// Metadata about a container instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    /// ISO 8601
    pub created: String,
}

impl From<perry_container_compose::types::ContainerInfo> for ContainerInfo {
    fn from(info: perry_container_compose::types::ContainerInfo) -> Self {
        Self {
            id: info.id,
            name: info.name,
            image: info.image,
            status: info.status,
            ports: info.ports,
            created: info.created,
        }
    }
}

impl From<serde_json::Error> for ContainerError {
    fn from(e: serde_json::Error) -> Self {
        ContainerError::InvalidConfig(e.to_string())
    }
}

/// Stdout + stderr captured from a container operation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerLogs {
    pub stdout: String,
    pub stderr: String,
}

impl From<perry_container_compose::types::ContainerLogs> for ContainerLogs {
    fn from(logs: perry_container_compose::types::ContainerLogs) -> Self {
        Self {
            stdout: logs.stdout,
            stderr: logs.stderr,
        }
    }
}

/// Metadata about a locally-available OCI image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    /// ISO 8601
    pub created: String,
}

// ============ Compose: ListOrDict ============

/// Compose-spec `list_or_dict` pattern.
/// Can be either a mapping (`Record<string, string|number|boolean|null>`) or a
/// `KEY=VALUE` string list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ListOrDict {
    Dict(HashMap<String, Option<serde_json::Value>>),
    List(Vec<String>),
}

impl ListOrDict {
    /// Resolve to a flat `HashMap<String, String>`.
    pub fn to_map(&self) -> HashMap<String, String> {
        match self {
            ListOrDict::Dict(map) => map
                .iter()
                .map(|(k, v)| {
                    let val = match v {
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(serde_json::Value::Number(n)) => n.to_string(),
                        Some(serde_json::Value::Bool(b)) => b.to_string(),
                        Some(serde_json::Value::Null) | None => String::new(),
                        Some(other) => other.to_string(),
                    };
                    (k.clone(), val)
                })
                .collect(),
            ListOrDict::List(list) => list
                .iter()
                .filter_map(|entry| {
                    let mut parts = entry.splitn(2, '=');
                    let key = parts.next()?.to_owned();
                    let val = parts.next().unwrap_or("").to_owned();
                    Some((key, val))
                })
                .collect(),
        }
    }
}

// ============ Compose: Port ============

/// Long-form port mapping (compose-spec `ports` entry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServicePort {
    pub name: Option<String>,
    pub mode: Option<String>,
    pub host_ip: Option<String>,
    /// Container port (number or string range e.g. "80-90")
    pub target: serde_json::Value,
    /// Published/host port (string or number)
    pub published: Option<serde_json::Value>,
    pub protocol: Option<String>,
    pub app_protocol: Option<String>,
}

/// `ports` entry: either a short string/number form or a long object form.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposePortEntry {
    Short(serde_json::Value),  // string or number
    Long(ComposeServicePort),
}

// ============ Compose: Volume Mount ============

/// Bind-mount options.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeBindOptions {
    pub propagation: Option<String>,
    pub create_host_path: Option<bool>,
    /// "enabled" | "disabled" | "writable" | "readonly"
    pub recursive: Option<String>,
    /// "z" | "Z"
    pub selinux: Option<String>,
}

/// Named-volume mount options.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeVolumeOptions {
    pub labels: Option<ListOrDict>,
    pub nocopy: Option<bool>,
    pub subpath: Option<String>,
}

/// Tmpfs mount options.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeTmpfsOptions {
    pub size: Option<serde_json::Value>,
    pub mode: Option<u32>,
}

/// Image-based volume options.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeImageVolumeOptions {
    pub subpath: Option<String>,
}

/// Long-form volume mount (compose-spec `volumes` entry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceVolume {
    /// "bind" | "volume" | "tmpfs" | "cluster" | "npipe" | "image"
    #[serde(rename = "type")]
    pub volume_type: String,
    pub source: Option<String>,
    pub target: Option<String>,
    pub read_only: Option<bool>,
    pub consistency: Option<String>,
    pub bind: Option<ComposeBindOptions>,
    pub volume: Option<ComposeVolumeOptions>,
    pub tmpfs: Option<ComposeTmpfsOptions>,
    pub image: Option<ComposeImageVolumeOptions>,
}

/// `volumes` entry: either a short string form or a long object form.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeVolumeEntry {
    Short(String),
    Long(ComposeServiceVolume),
}

// ============ Compose: depends_on ============

/// Object-form condition for a single dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeDependsOn {
    /// "service_started" | "service_healthy" | "service_completed_successfully"
    pub condition: String,
    pub required: Option<bool>,
    pub restart: Option<bool>,
}

/// `depends_on`: either a list of service names or an object map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeDependsOnEntry {
    List(Vec<String>),
    Map(HashMap<String, ComposeDependsOn>),
}

impl ComposeDependsOnEntry {
    pub fn service_names(&self) -> Vec<String> {
        match self {
            ComposeDependsOnEntry::List(names) => names.clone(),
            ComposeDependsOnEntry::Map(map) => map.keys().cloned().collect(),
        }
    }
}

// ============ Compose: Healthcheck ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHealthcheck {
    pub test: serde_json::Value, // string | string[]
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
    pub start_interval: Option<String>,
    pub disable: Option<bool>,
}

// ============ Compose: Logging ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeLogging {
    pub driver: Option<String>,
    pub options: Option<HashMap<String, Option<String>>>,
}

// ============ Compose: Deploy ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeResourceLimit {
    pub cpus: Option<serde_json::Value>,
    pub memory: Option<String>,
    pub pids: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeployResources {
    pub limits: Option<ComposeResourceLimit>,
    pub reservations: Option<ComposeResourceLimit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeployRestartPolicy {
    pub condition: Option<String>,
    pub delay: Option<String>,
    pub max_attempts: Option<u32>,
    pub window: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeployUpdateConfig {
    pub parallelism: Option<u32>,
    pub delay: Option<String>,
    pub failure_action: Option<String>,
    pub monitor: Option<String>,
    pub max_failure_ratio: Option<f64>,
    pub order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeployment {
    pub mode: Option<String>,
    pub replicas: Option<u32>,
    pub labels: Option<ListOrDict>,
    pub resources: Option<ComposeDeployResources>,
    pub restart_policy: Option<ComposeDeployRestartPolicy>,
    pub update_config: Option<ComposeDeployUpdateConfig>,
    pub rollback_config: Option<ComposeDeployUpdateConfig>,
    pub placement: Option<serde_json::Value>,
}

// ============ Compose: Build ============

/// Full build configuration (compose-spec `build` object form).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceBuild {
    pub context: Option<String>,
    pub dockerfile: Option<String>,
    pub dockerfile_inline: Option<String>,
    pub args: Option<ListOrDict>,
    pub ssh: Option<serde_json::Value>,
    pub labels: Option<ListOrDict>,
    pub cache_from: Option<Vec<String>>,
    pub cache_to: Option<Vec<String>>,
    pub no_cache: Option<bool>,
    pub additional_contexts: Option<serde_json::Value>,
    pub network: Option<String>,
    pub target: Option<String>,
    pub shm_size: Option<serde_json::Value>,
    pub extra_hosts: Option<ListOrDict>,
    pub isolation: Option<String>,
    pub privileged: Option<bool>,
    pub secrets: Option<Vec<serde_json::Value>>,
    pub tags: Option<Vec<String>>,
    pub platforms: Option<Vec<String>>,
    pub pull: Option<bool>,
    pub provenance: Option<serde_json::Value>,
    pub sbom: Option<serde_json::Value>,
    pub entitlements: Option<Vec<String>>,
    pub ulimits: Option<serde_json::Value>,
}

/// `build` field: either a string shorthand (context path) or a full object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeBuildEntry {
    String(String),
    Object(ComposeServiceBuild),
}

// ============ Compose: NetworkConfig ============

/// Per-service network attachment config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceNetworkConfig {
    pub aliases: Option<Vec<String>>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub priority: Option<i32>,
}

/// `networks` on a service: either a list or an object map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeServiceNetworks {
    List(Vec<String>),
    Map(HashMap<String, Option<ComposeServiceNetworkConfig>>),
}

// ============ Compose: Service ============

/// A single service definition (compose-spec `service` schema).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeService {
    // ── image / build ──
    pub image: Option<String>,
    pub build: Option<ComposeBuildEntry>,

    // ── command / entrypoint ──
    pub command: Option<serde_json::Value>,
    pub entrypoint: Option<serde_json::Value>,

    // ── environment ──
    pub environment: Option<ListOrDict>,
    pub env_file: Option<serde_json::Value>,

    // ── networking ──
    pub ports: Option<Vec<ComposePortEntry>>,
    pub networks: Option<ComposeServiceNetworks>,
    pub network_mode: Option<String>,
    pub hostname: Option<String>,
    pub extra_hosts: Option<ListOrDict>,
    pub dns: Option<serde_json::Value>,
    pub dns_search: Option<serde_json::Value>,
    pub expose: Option<Vec<serde_json::Value>>,

    // ── storage ──
    pub volumes: Option<Vec<ComposeVolumeEntry>>,
    pub tmpfs: Option<serde_json::Value>,
    pub shm_size: Option<serde_json::Value>,

    // ── dependencies ──
    pub depends_on: Option<ComposeDependsOnEntry>,

    // ── container identity ──
    pub container_name: Option<String>,
    pub labels: Option<ListOrDict>,

    // ── lifecycle ──
    pub restart: Option<String>,
    pub stop_signal: Option<String>,
    pub stop_grace_period: Option<String>,

    // ── healthcheck ──
    pub healthcheck: Option<ComposeHealthcheck>,

    // ── security ──
    pub privileged: Option<bool>,
    pub read_only: Option<bool>,
    pub user: Option<String>,
    pub cap_add: Option<Vec<String>>,
    pub cap_drop: Option<Vec<String>>,
    pub security_opt: Option<Vec<String>>,
    pub sysctls: Option<ListOrDict>,
    pub ulimits: Option<serde_json::Value>,
    pub pid: Option<String>,

    // ── i/o ──
    pub stdin_open: Option<bool>,
    pub tty: Option<bool>,
    pub working_dir: Option<String>,

    // ── resources (short-form, no deploy) ──
    pub mem_limit: Option<serde_json::Value>,
    pub memswap_limit: Option<serde_json::Value>,
    pub cpus: Option<serde_json::Value>,
    pub cpu_shares: Option<i64>,

    // ── deploy ──
    pub deploy: Option<ComposeDeployment>,
    pub develop: Option<serde_json::Value>,
    pub scale: Option<u32>,

    // ── logging ──
    pub logging: Option<ComposeLogging>,

    // ── platform ──
    pub platform: Option<String>,
    pub pull_policy: Option<String>,
    pub profiles: Option<Vec<String>>,

    // ── secrets / configs ──
    pub secrets: Option<Vec<serde_json::Value>>,
    pub configs: Option<Vec<serde_json::Value>>,

    // ── extension / advanced ──
    pub extends: Option<serde_json::Value>,
    pub post_start: Option<Vec<serde_json::Value>>,
    pub pre_stop: Option<Vec<serde_json::Value>>,
}

// ============ Compose: Network ============

/// IPAM subnet config entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetworkIpamConfig {
    pub subnet: Option<String>,
    pub ip_range: Option<String>,
    pub gateway: Option<String>,
    pub aux_addresses: Option<HashMap<String, String>>,
}

/// IPAM configuration block.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetworkIpam {
    pub driver: Option<String>,
    pub config: Option<Vec<ComposeNetworkIpamConfig>>,
    pub options: Option<HashMap<String, String>>,
}

/// Top-level network definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetwork {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub ipam: Option<ComposeNetworkIpam>,
    pub external: Option<bool>,
    pub internal: Option<bool>,
    pub enable_ipv4: Option<bool>,
    pub enable_ipv6: Option<bool>,
    pub attachable: Option<bool>,
    pub labels: Option<ListOrDict>,
}

// ============ Compose: Volume ============

/// Top-level volume definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeVolume {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
}

// ============ Compose: Secret ============

/// Top-level secret definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeSecret {
    pub name: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub template_driver: Option<String>,
}

// ============ Compose: Config ============

/// Top-level config definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeConfig {
    pub name: Option<String>,
    pub content: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
    pub template_driver: Option<String>,
}

// ============ ComposeSpec (root) ============

/// Root compose specification — conforms to the official compose-spec JSON schema.
///
/// This is the sole accepted input format for `composeUp()`.
/// No YAML file paths are accepted by the TypeScript API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeSpec {
    /// Optional stack name
    pub name: Option<String>,
    /// Deprecated but accepted; not used for validation
    pub version: Option<String>,
    /// Service definitions (required)
    #[serde(default)]
    pub services: HashMap<String, ComposeService>,
    /// Top-level network definitions
    pub networks: Option<HashMap<String, Option<ComposeNetwork>>>,
    /// Top-level volume definitions
    pub volumes: Option<HashMap<String, Option<ComposeVolume>>>,
    /// Top-level secret definitions
    pub secrets: Option<HashMap<String, Option<ComposeSecret>>>,
    /// Top-level config definitions
    pub configs: Option<HashMap<String, Option<ComposeConfig>>>,
    /// Included compose files (object form from compose-spec)
    pub include: Option<Vec<serde_json::Value>>,
    /// AI model definitions (compose-spec extension)
    pub models: Option<HashMap<String, serde_json::Value>>,
}

// ============ ComposeHandle ============

/// Opaque handle to a running compose stack, returned by `composeUp()`.
#[derive(Debug, Clone)]
pub struct ComposeHandle {
    pub stack_id: u64,
    pub project_name: String,
    pub services: Vec<String>,
}

impl From<perry_container_compose::types::ComposeHandle> for ComposeHandle {
    fn from(h: perry_container_compose::types::ComposeHandle) -> Self {
        Self {
            stack_id: h.stack_id,
            project_name: h.project_name,
            services: h.services,
        }
    }
}

// ============ Error Types ============

/// Container module errors.
#[derive(Debug, Clone)]
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
        probed: Vec<perry_container_compose::backend::BackendProbeResult>,
    },
    ImagePullFailed {
        service: String,
        image: String,
        message: String,
    },
}

impl ContainerError {
    pub fn to_json(&self) -> String {
        let code = match self {
            ContainerError::NotFound(_) => 404,
            ContainerError::BackendError { code, .. } => *code,
            ContainerError::VerificationFailed { .. } => 403,
            ContainerError::DependencyCycle { .. } => 422,
            ContainerError::NoBackendFound { .. } => 503,
            ContainerError::InvalidConfig(_) => 400,
            _ => 500,
        };
        serde_json::json!({
            "message": self.to_string(),
            "code": code
        })
        .to_string()
    }
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
            ContainerError::ImagePullFailed { service, image, message } => {
                write!(f, "Image pull failed for service '{}' (image '{}'): {}", service, image, message)
            }
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
            perry_container_compose::error::ComposeError::BackendError { code, message } => {
                ContainerError::BackendError { code, message }
            }
            perry_container_compose::error::ComposeError::NoBackendFound { probed } => {
                ContainerError::NoBackendFound { probed }
            }
            perry_container_compose::error::ComposeError::ImagePullFailed {
                service,
                image,
                message,
            } => ContainerError::ImagePullFailed {
                service,
                image,
                message,
            },
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
