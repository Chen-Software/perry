//! Container module types matching the design.

use perry_runtime::{JSValue, StringHeader};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::common::{register_handle, get_handle, Handle};

// ============ Single Container Types ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerSpec {
    pub image: String,
    pub name: Option<String>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub cmd: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub network: Option<String>,
    pub rm: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerHandle {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLogs {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    pub created: String,
}

// ============ Compose Types ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ListOrDict {
    List(Vec<String>),
    Dict(HashMap<String, Option<serde_json::Value>>),
}

impl ListOrDict {
    pub fn to_map(&self) -> HashMap<String, String> {
        match self {
            ListOrDict::Dict(m) => m
                .iter()
                .map(|(k, v)| {
                    let val_str = v.as_ref().and_then(|val| val.as_str()).unwrap_or("");
                    (k.clone(), val_str.to_string())
                })
                .collect(),
            ListOrDict::List(v) => v
                .iter()
                .filter_map(|s| {
                    let mut parts = s.splitn(2, '=');
                    let k = parts.next()?.to_owned();
                    let v = parts.next().unwrap_or("").to_owned();
                    Some((k, v))
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComposeDependsOnCondition {
    #[serde(rename = "service_started")]
    ServiceStarted,
    #[serde(rename = "service_healthy")]
    ServiceHealthy,
    #[serde(rename = "service_completed_successfully")]
    ServiceCompletedSuccessfully,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeDependsOn {
    pub condition: ComposeDependsOnCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeDependsOnEntry {
    List(Vec<String>),
    Map(HashMap<String, ComposeDependsOn>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHealthcheck {
    pub test: Option<serde_json::Value>,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeployment {
    pub resources: Option<serde_json::Value>,
    pub replicas: Option<u32>,
    pub restart_policy: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeLogging {
    pub driver: Option<String>,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposePortEntry {
    Short(serde_json::Value),
    Long(ComposeServicePort),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServicePort {
    pub target: u32,
    pub published: Option<u32>,
    pub protocol: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeVolumeEntry {
    Short(String),
    Long(ComposeServiceVolume),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceVolume {
    #[serde(rename = "type")]
    pub type_str: Option<String>,
    pub source: Option<String>,
    pub target: Option<String>,
    pub read_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceBuild {
    pub context: Option<String>,
    pub dockerfile: Option<String>,
    pub args: Option<HashMap<String, String>>,
    pub pull: Option<bool>,
    pub provenance: Option<serde_json::Value>,
    pub sbom: Option<serde_json::Value>,
    pub entitlements: Option<Vec<String>>,
    pub ulimits: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeBuildEntry {
    String(String),
    Object(ComposeServiceBuild),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceNetworkConfig {
    pub aliases: Option<Vec<String>>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComposeServiceNetworks {
    List(Vec<String>),
    Map(HashMap<String, Option<ComposeServiceNetworkConfig>>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeService {
    pub image: Option<String>,
    pub build: Option<ComposeBuildEntry>,
    pub command: Option<serde_json::Value>,
    pub entrypoint: Option<serde_json::Value>,
    pub environment: Option<ListOrDict>,
    pub env_file: Option<serde_json::Value>,
    pub ports: Option<Vec<ComposePortEntry>>,
    pub networks: Option<ComposeServiceNetworks>,
    pub network_mode: Option<String>,
    pub hostname: Option<String>,
    pub extra_hosts: Option<ListOrDict>,
    pub dns: Option<serde_json::Value>,
    pub dns_search: Option<serde_json::Value>,
    pub expose: Option<Vec<serde_json::Value>>,
    pub volumes: Option<Vec<ComposeVolumeEntry>>,
    pub tmpfs: Option<serde_json::Value>,
    pub shm_size: Option<serde_json::Value>,
    pub depends_on: Option<ComposeDependsOnEntry>,
    pub container_name: Option<String>,
    pub labels: Option<ListOrDict>,
    pub restart: Option<String>,
    pub stop_signal: Option<String>,
    pub stop_grace_period: Option<String>,
    pub healthcheck: Option<ComposeHealthcheck>,
    pub privileged: Option<bool>,
    pub read_only: Option<bool>,
    pub user: Option<String>,
    pub cap_add: Option<Vec<String>>,
    pub cap_drop: Option<Vec<String>>,
    pub security_opt: Option<Vec<String>>,
    pub sysctls: Option<ListOrDict>,
    pub ulimits: Option<serde_json::Value>,
    pub pid: Option<String>,
    pub stdin_open: Option<bool>,
    pub tty: Option<bool>,
    pub working_dir: Option<String>,
    pub mem_limit: Option<serde_json::Value>,
    pub memswap_limit: Option<serde_json::Value>,
    pub cpus: Option<serde_json::Value>,
    pub cpu_shares: Option<i64>,
    pub deploy: Option<ComposeDeployment>,
    pub develop: Option<serde_json::Value>,
    pub scale: Option<u32>,
    pub logging: Option<ComposeLogging>,
    pub platform: Option<String>,
    pub pull_policy: Option<String>,
    pub profiles: Option<Vec<String>>,
    pub secrets: Option<Vec<serde_json::Value>>,
    pub configs: Option<Vec<serde_json::Value>>,
    pub extends: Option<serde_json::Value>,
    pub post_start: Option<Vec<serde_json::Value>>,
    pub pre_stop: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetworkIpamConfig {
    pub subnet: Option<String>,
    pub ip_range: Option<String>,
    pub gateway: Option<String>,
    pub aux_addresses: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetworkIpam {
    pub driver: Option<String>,
    pub config: Option<Vec<ComposeNetworkIpamConfig>>,
    pub options: Option<HashMap<String, String>>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeVolume {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeSpec {
    pub name: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub services: HashMap<String, ComposeService>,
    pub networks: Option<HashMap<String, Option<ComposeNetwork>>>,
    pub volumes: Option<HashMap<String, Option<ComposeVolume>>>,
    pub secrets: Option<HashMap<String, Option<ComposeSecret>>>,
    pub configs: Option<HashMap<String, Option<ComposeConfig>>>,
    pub include: Option<Vec<serde_json::Value>>,
    pub models: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone)]
pub struct ComposeHandle {
    pub name: String,
    pub services: Vec<String>,
    pub networks: Vec<String>,
    pub volumes: Vec<String>,
    pub containers: HashMap<String, ContainerHandle>,
}

// ============ Global Registries ============

pub fn register_container_handle(h: ContainerHandle) -> Handle {
    register_handle(h)
}

pub fn get_container_handle(id: Handle) -> Option<&'static ContainerHandle> {
    get_handle::<ContainerHandle>(id)
}

pub fn register_compose_handle(h: ComposeHandle) -> Handle {
    register_handle(h)
}

pub fn get_compose_handle(id: Handle) -> Option<&'static ComposeHandle> {
    get_handle::<ComposeHandle>(id)
}

pub fn register_container_info(h: ContainerInfo) -> Handle {
    register_handle(h)
}

pub fn register_container_logs(h: ContainerLogs) -> Handle {
    register_handle(h)
}

pub fn register_image_info(h: ImageInfo) -> Handle {
    register_handle(h)
}

// ============ Error Types ============

#[derive(Debug, Clone)]
pub enum ContainerError {
    NotFound(String),
    BackendError { code: i32, message: String },
    NoBackendFound { probed: Vec<perry_container_compose::error::BackendProbeResult> },
    BackendNotAvailable { name: String, reason: String },
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
            ContainerError::NoBackendFound { probed } => {
                write!(f, "No container backend found. Probed: {:?}", probed)
            }
            ContainerError::BackendNotAvailable { name, reason } => {
                write!(f, "Backend {} is not available: {}", name, reason)
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

pub fn parse_container_spec(_spec_ptr: *const JSValue) -> Result<ContainerSpec, String> {
    Err("ContainerSpec must be constructed via native codegen".to_string())
}

pub fn parse_compose_spec(_spec_ptr: *const JSValue) -> Result<ComposeSpec, String> {
    Err("ComposeSpec must be constructed via native codegen".to_string())
}
