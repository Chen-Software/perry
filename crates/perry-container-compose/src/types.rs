use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ListOrDict {
    Dict(IndexMap<String, Option<serde_yaml::Value>>),
    List(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependsOnCondition {
    ServiceStarted,
    ServiceHealthy,
    ServiceCompletedSuccessfully,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeDependsOn {
    pub condition: DependsOnCondition,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub restart: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DependsOnSpec {
    List(Vec<String>),
    Map(IndexMap<String, ComposeDependsOn>),
}

impl DependsOnSpec {
    pub fn service_names(&self) -> Vec<String> {
        match self {
            DependsOnSpec::List(names) => names.clone(),
            DependsOnSpec::Map(map) => map.keys().cloned().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VolumeType {
    Bind,
    Volume,
    Tmpfs,
    Cluster,
    Npipe,
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeServiceVolume {
    #[serde(rename = "type")]
    pub volume_type: VolumeType,
    pub source: Option<String>,
    pub target: Option<String>,
    #[serde(alias = "read_only")]
    pub read_only: Option<bool>,
    pub consistency: Option<String>,
    pub bind: Option<ComposeServiceVolumeBind>,
    pub volume: Option<ComposeServiceVolumeOpts>,
    pub tmpfs: Option<ComposeServiceVolumeTmpfs>,
    pub image: Option<ComposeServiceVolumeImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeServiceVolumeBind {
    pub propagation: Option<String>,
    #[serde(alias = "create_host_path")]
    pub create_host_path: Option<bool>,
    pub recursive: Option<String>,
    pub selinux: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeServiceVolumeOpts {
    pub labels: Option<ListOrDict>,
    pub nocopy: Option<bool>,
    pub subpath: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeServiceVolumeTmpfs {
    pub size: Option<serde_yaml::Value>,
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeServiceVolumeImage {
    pub subpath: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeServicePort {
    pub name: Option<String>,
    pub mode: Option<String>,
    #[serde(alias = "host_ip")]
    pub host_ip: Option<String>,
    pub target: serde_yaml::Value,
    pub published: Option<serde_yaml::Value>,
    pub protocol: Option<String>,
    #[serde(alias = "app_protocol")]
    pub app_protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PortSpec {
    Short(serde_yaml::Value),
    Long(ComposeServicePort),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeServiceNetworkConfig {
    pub aliases: Option<Vec<String>>,
    #[serde(alias = "ipv4_address")]
    pub ipv4_address: Option<String>,
    #[serde(alias = "ipv6_address")]
    pub ipv6_address: Option<String>,
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ServiceNetworks {
    List(Vec<String>),
    Map(IndexMap<String, Option<ComposeServiceNetworkConfig>>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum BuildSpec {
    Context(String),
    Config(ComposeServiceBuild),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeServiceBuild {
    pub context: Option<String>,
    pub dockerfile: Option<String>,
    #[serde(alias = "dockerfile_inline")]
    pub dockerfile_inline: Option<String>,
    pub args: Option<ListOrDict>,
    pub ssh: Option<serde_yaml::Value>,
    pub labels: Option<ListOrDict>,
    #[serde(alias = "cache_from")]
    pub cache_from: Option<Vec<String>>,
    #[serde(alias = "cache_to")]
    pub cache_to: Option<Vec<String>>,
    #[serde(alias = "no_cache")]
    pub no_cache: Option<bool>,
    #[serde(alias = "additional_contexts")]
    pub additional_contexts: Option<IndexMap<String, String>>,
    pub network: Option<String>,
    pub provenance: Option<serde_yaml::Value>,
    pub sbom: Option<serde_yaml::Value>,
    pub pull: Option<bool>,
    pub target: Option<String>,
    #[serde(alias = "shm_size")]
    pub shm_size: Option<serde_yaml::Value>,
    #[serde(alias = "extra_hosts")]
    pub extra_hosts: Option<ListOrDict>,
    pub isolation: Option<String>,
    pub privileged: Option<bool>,
    pub secrets: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub ulimits: Option<serde_yaml::Value>,
    pub platforms: Option<Vec<String>>,
    pub entitlements: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeHealthcheck {
    pub test: serde_yaml::Value,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    #[serde(alias = "start_period")]
    pub start_period: Option<String>,
    #[serde(alias = "start_interval")]
    pub start_interval: Option<String>,
    pub disable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeDeployment {
    pub mode: Option<String>,
    pub replicas: Option<u32>,
    pub labels: Option<ListOrDict>,
    pub resources: Option<ComposeDeploymentResources>,
    #[serde(alias = "restart_policy")]
    pub restart_policy: Option<serde_yaml::Value>,
    pub placement: Option<serde_yaml::Value>,
    #[serde(alias = "update_config")]
    pub update_config: Option<serde_yaml::Value>,
    #[serde(alias = "rollback_config")]
    pub rollback_config: Option<serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeDeploymentResources {
    pub limits: Option<ComposeResourceSpec>,
    pub reservations: Option<ComposeResourceSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeResourceSpec {
    pub cpus: Option<serde_yaml::Value>,
    pub memory: Option<String>,
    pub pids: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeLogging {
    pub driver: Option<String>,
    pub options: Option<IndexMap<String, serde_yaml::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeNetworkIpam {
    pub driver: Option<String>,
    pub config: Option<Vec<ComposeNetworkIpamConfig>>,
    pub options: Option<IndexMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeNetworkIpamConfig {
    pub subnet: Option<String>,
    #[serde(alias = "ip_range")]
    pub ip_range: Option<String>,
    pub gateway: Option<String>,
    #[serde(alias = "aux_addresses")]
    pub aux_addresses: Option<IndexMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeNetwork {
    pub name: Option<String>,
    pub driver: Option<String>,
    #[serde(alias = "driver_opts")]
    pub driver_opts: Option<IndexMap<String, String>>,
    pub ipam: Option<ComposeNetworkIpam>,
    pub external: Option<bool>,
    pub internal: Option<bool>,
    #[serde(alias = "enable_ipv4")]
    pub enable_ipv4: Option<bool>,
    #[serde(alias = "enable_ipv6")]
    pub enable_ipv6: Option<bool>,
    pub attachable: Option<bool>,
    pub labels: Option<ListOrDict>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeVolume {
    pub name: Option<String>,
    pub driver: Option<String>,
    #[serde(alias = "driver_opts")]
    pub driver_opts: Option<IndexMap<String, String>>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeSecret {
    pub name: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
    pub driver: Option<String>,
    #[serde(alias = "driver_opts")]
    pub driver_opts: Option<IndexMap<String, String>>,
    #[serde(alias = "template_driver")]
    pub template_driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeConfig {
    pub name: Option<String>,
    pub content: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
    #[serde(alias = "template_driver")]
    pub template_driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeService {
    pub image: Option<String>,
    pub build: Option<BuildSpec>,
    pub command: Option<serde_yaml::Value>,
    pub entrypoint: Option<serde_yaml::Value>,
    pub environment: Option<ListOrDict>,
    #[serde(alias = "env_file")]
    pub env_file: Option<serde_yaml::Value>,
    pub ports: Option<Vec<PortSpec>>,
    pub volumes: Option<Vec<serde_yaml::Value>>,
    pub networks: Option<ServiceNetworks>,
    #[serde(alias = "depends_on")]
    pub depends_on: Option<DependsOnSpec>,
    pub restart: Option<String>,
    pub healthcheck: Option<ComposeHealthcheck>,
    #[serde(alias = "container_name")]
    pub container_name: Option<String>,
    pub labels: Option<ListOrDict>,
    pub hostname: Option<String>,
    pub user: Option<String>,
    #[serde(alias = "working_dir")]
    pub working_dir: Option<String>,
    pub privileged: Option<bool>,
    #[serde(alias = "read_only")]
    pub read_only: Option<bool>,
    #[serde(alias = "stdin_open")]
    pub stdin_open: Option<bool>,
    pub tty: Option<bool>,
    #[serde(alias = "stop_signal")]
    pub stop_signal: Option<String>,
    #[serde(alias = "stop_grace_period")]
    pub stop_grace_period: Option<String>,
    #[serde(alias = "network_mode")]
    pub network_mode: Option<String>,
    pub pid: Option<String>,
    #[serde(alias = "cap_add")]
    pub cap_add: Option<Vec<String>>,
    #[serde(alias = "cap_drop")]
    pub cap_drop: Option<Vec<String>>,
    #[serde(alias = "security_opt")]
    pub security_opt: Option<Vec<String>>,
    pub sysctls: Option<ListOrDict>,
    pub ulimits: Option<serde_yaml::Value>,
    pub logging: Option<ComposeLogging>,
    pub deploy: Option<ComposeDeployment>,
    pub develop: Option<serde_yaml::Value>,
    pub secrets: Option<Vec<String>>,
    pub configs: Option<Vec<String>>,
    pub expose: Option<Vec<serde_yaml::Value>>,
    #[serde(alias = "extra_hosts")]
    pub extra_hosts: Option<ListOrDict>,
    pub dns: Option<serde_yaml::Value>,
    #[serde(alias = "dns_search")]
    pub dns_search: Option<serde_yaml::Value>,
    pub tmpfs: Option<serde_yaml::Value>,
    #[serde(alias = "shm_size")]
    pub shm_size: Option<serde_yaml::Value>,
    #[serde(alias = "mem_limit")]
    pub mem_limit: Option<serde_yaml::Value>,
    #[serde(alias = "memswap_limit")]
    pub memswap_limit: Option<serde_yaml::Value>,
    pub cpus: Option<serde_yaml::Value>,
    #[serde(alias = "cpu_shares")]
    pub cpu_shares: Option<i64>,
    pub platform: Option<String>,
    #[serde(alias = "pull_policy")]
    pub pull_policy: Option<String>,
    pub profiles: Option<Vec<String>>,
    pub scale: Option<u32>,
    pub extends: Option<serde_yaml::Value>,
    #[serde(alias = "post_start")]
    pub post_start: Option<Vec<serde_yaml::Value>>,
    #[serde(alias = "pre_stop")]
    pub pre_stop: Option<Vec<serde_yaml::Value>>,
    #[serde(alias = "isolation_level")]
    pub isolation_level: Option<IsolationLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeSpec {
    pub name: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub services: IndexMap<String, ComposeService>,
    pub networks: Option<IndexMap<String, Option<ComposeNetwork>>>,
    pub volumes: Option<IndexMap<String, Option<ComposeVolume>>>,
    pub secrets: Option<IndexMap<String, Option<ComposeSecret>>>,
    pub configs: Option<IndexMap<String, Option<ComposeConfig>>>,
    pub include: Option<Vec<serde_yaml::Value>>,
    pub models: Option<IndexMap<String, serde_yaml::Value>>,
    #[serde(flatten)]
    pub extensions: IndexMap<String, serde_yaml::Value>,
}

impl ComposeSpec {
    pub fn parse_str(s: &str) -> Result<Self, crate::error::ComposeError> {
        serde_yaml::from_str(s).map_err(crate::error::ComposeError::ParseError)
    }

    pub fn merge(&mut self, other: ComposeSpec) {
        if other.name.is_some() { self.name = other.name; }
        for (k, v) in other.services { self.services.insert(k, v); }
        if let Some(other_nets) = other.networks {
            let nets = self.networks.get_or_insert_with(IndexMap::new);
            for (k, v) in other_nets { nets.insert(k, v); }
        }
        if let Some(other_vols) = other.volumes {
            let vols = self.volumes.get_or_insert_with(IndexMap::new);
            for (k, v) in other_vols { vols.insert(k, v); }
        }
        if let Some(other_secrets) = other.secrets {
            let secrets = self.secrets.get_or_insert_with(IndexMap::new);
            for (k, v) in other_secrets { secrets.insert(k, v); }
        }
        if let Some(other_configs) = other.configs {
            let configs = self.configs.get_or_insert_with(IndexMap::new);
            for (k, v) in other_configs { configs.insert(k, v); }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHandle {
    pub stack_id: u64,
    pub project_name: String,
    pub services: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceGraph {
    pub nodes: Vec<String>,
    pub edges: Vec<ServiceEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub service: String,
    pub state: String,
    pub container_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StackStatus {
    pub services: Vec<ServiceStatus>,
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ContainerSpec {
    pub image: String,
    pub name: Option<String>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub cmd: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub network: Option<String>,
    pub rm: Option<bool>,
    pub read_only: Option<bool>,
    pub privileged: Option<bool>,
    pub cap_add: Option<Vec<String>>,
    pub cap_drop: Option<Vec<String>>,
    pub security_opt: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerHandle { pub id: String, pub name: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerInfo {
    pub id: String, pub name: String, pub image: String,
    pub status: String, pub ports: Vec<String>, pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerLogs { pub stdout: String, pub stderr: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageInfo {
    pub id: String, pub repository: String, pub tag: String,
    pub size: u64, pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IsolationLevel {
    None,
    Process,
    Container,
    MicroVm,
    Wasm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackendInfo {
    pub name: String,
    pub available: bool,
    pub reason: Option<String>,
    pub version: Option<String>,
    pub mode: String, // "local" | "remote"
    pub isolation_level: IsolationLevel,
}
