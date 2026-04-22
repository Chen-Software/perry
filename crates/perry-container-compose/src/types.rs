//! All compose-spec Rust types.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Convert a `serde_yaml::Value` to a string representation.
fn yaml_value_to_str(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => String::new(),
        _ => serde_yaml::to_string(v).unwrap_or_default().trim().to_owned(),
    }
}

// ============ ListOrDict ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ListOrDict {
    Dict(IndexMap<String, Option<serde_yaml::Value>>),
    List(Vec<String>),
}

impl ListOrDict {
    pub fn to_map(&self) -> std::collections::HashMap<String, String> {
        match self {
            ListOrDict::Dict(map) => map
                .iter()
                .map(|(k, v)| {
                    let val = match v {
                        Some(v) => yaml_value_to_str(v),
                        None => String::new(),
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

// ============ DependsOn ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependsOnCondition {
    ServiceStarted,
    ServiceHealthy,
    ServiceCompletedSuccessfully,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeDependsOn {
    pub condition: DependsOnCondition,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub restart: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

// ============ Volume ============

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceVolume {
    #[serde(rename = "type")]
    pub volume_type: VolumeType,
    pub source: Option<String>,
    pub target: Option<String>,
    pub read_only: Option<bool>,
    pub consistency: Option<String>,
    pub bind: Option<ComposeServiceVolumeBind>,
    pub volume: Option<ComposeServiceVolumeOpts>,
    pub tmpfs: Option<ComposeServiceVolumeTmpfs>,
    pub image: Option<ComposeServiceVolumeImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceVolumeBind {
    pub propagation: Option<String>,
    pub create_host_path: Option<bool>,
    pub recursive: Option<String>,
    pub selinux: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceVolumeOpts {
    pub labels: Option<ListOrDict>,
    pub nocopy: Option<bool>,
    pub subpath: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceVolumeTmpfs {
    pub size: Option<serde_yaml::Value>,
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServiceVolumeImage {
    pub subpath: Option<String>,
}

// ============ Port ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeServicePort {
    pub name: Option<String>,
    pub mode: Option<String>,
    pub host_ip: Option<String>,
    pub target: serde_yaml::Value,
    pub published: Option<serde_yaml::Value>,
    pub protocol: Option<String>,
    pub app_protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PortSpec {
    Short(serde_yaml::Value),
    Long(ComposeServicePort),
}

impl PortSpec {
    pub fn to_string_form(&self) -> String {
        match self {
            PortSpec::Short(v) => yaml_value_to_str(v),
            PortSpec::Long(p) => {
                let container = yaml_value_to_str(&p.target);
                match &p.published {
                    Some(pub_) => format!("{}:{}", yaml_value_to_str(pub_), container),
                    None => container,
                }
            }
        }
    }
}

// ============ Networks ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceNetworkConfig {
    pub aliases: Option<Vec<String>>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServiceNetworks {
    List(Vec<String>),
    Map(IndexMap<String, Option<ComposeServiceNetworkConfig>>),
}

// ============ Build ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BuildSpec {
    Context(String),
    Config(ComposeServiceBuild),
}

impl BuildSpec {
    pub fn as_build(&self) -> ComposeServiceBuild {
        match self {
            BuildSpec::Context(ctx) => ComposeServiceBuild {
                context: Some(ctx.clone()),
                ..Default::default()
            },
            BuildSpec::Config(b) => b.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceBuild {
    pub context: Option<String>,
    pub dockerfile: Option<String>,
    pub dockerfile_inline: Option<String>,
    pub args: Option<ListOrDict>,
    pub ssh: Option<serde_yaml::Value>,
    pub labels: Option<ListOrDict>,
    pub cache_from: Option<Vec<String>>,
    pub cache_to: Option<Vec<String>>,
    pub no_cache: Option<bool>,
    pub additional_contexts: Option<IndexMap<String, String>>,
    pub network: Option<String>,
    pub provenance: Option<serde_yaml::Value>,
    pub sbom: Option<serde_yaml::Value>,
    pub pull: Option<bool>,
    pub target: Option<String>,
    pub shm_size: Option<serde_yaml::Value>,
    pub extra_hosts: Option<ListOrDict>,
    pub isolation: Option<String>,
    pub privileged: Option<bool>,
    pub secrets: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub ulimits: Option<serde_yaml::Value>,
    pub platforms: Option<Vec<String>>,
    pub entitlements: Option<Vec<String>>,
}

// ============ Healthcheck ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHealthcheck {
    pub test: serde_yaml::Value,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
    pub start_interval: Option<String>,
    pub disable: Option<bool>,
}

// ============ Deployment ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeployment {
    pub mode: Option<String>,
    pub replicas: Option<u32>,
    pub labels: Option<ListOrDict>,
    pub resources: Option<ComposeDeploymentResources>,
    pub restart_policy: Option<serde_yaml::Value>,
    pub placement: Option<serde_yaml::Value>,
    pub update_config: Option<serde_yaml::Value>,
    pub rollback_config: Option<serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeploymentResources {
    pub limits: Option<ComposeResourceSpec>,
    pub reservations: Option<ComposeResourceSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeResourceSpec {
    pub cpus: Option<serde_yaml::Value>,
    pub memory: Option<String>,
    pub pids: Option<i64>,
}

// ============ Logging ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeLogging {
    pub driver: Option<String>,
    pub options: Option<IndexMap<String, serde_yaml::Value>>,
}

// ============ Top-level Definitions ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetworkIpamConfig {
    pub subnet: Option<String>,
    pub ip_range: Option<String>,
    pub gateway: Option<String>,
    pub aux_addresses: Option<IndexMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetworkIpam {
    pub driver: Option<String>,
    pub config: Option<Vec<ComposeNetworkIpamConfig>>,
    pub options: Option<IndexMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetwork {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<IndexMap<String, String>>,
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
    pub driver_opts: Option<IndexMap<String, String>>,
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
    pub driver_opts: Option<IndexMap<String, String>>,
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

// ============ ComposeService ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeService {
    pub image: Option<String>,
    pub build: Option<BuildSpec>,
    pub command: Option<serde_yaml::Value>,
    pub entrypoint: Option<serde_yaml::Value>,
    pub environment: Option<ListOrDict>,
    pub env_file: Option<serde_yaml::Value>,
    pub ports: Option<Vec<PortSpec>>,
    pub volumes: Option<Vec<serde_yaml::Value>>,
    pub networks: Option<ServiceNetworks>,
    pub depends_on: Option<DependsOnSpec>,
    pub restart: Option<String>,
    pub healthcheck: Option<ComposeHealthcheck>,
    pub container_name: Option<String>,
    pub labels: Option<ListOrDict>,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub working_dir: Option<String>,
    pub privileged: Option<bool>,
    pub read_only: Option<bool>,
    pub stdin_open: Option<bool>,
    pub tty: Option<bool>,
    pub stop_signal: Option<String>,
    pub stop_grace_period: Option<String>,
    pub network_mode: Option<String>,
    pub pid: Option<String>,
    pub cap_add: Option<Vec<String>>,
    pub cap_drop: Option<Vec<String>>,
    pub security_opt: Option<Vec<String>>,
    pub sysctls: Option<ListOrDict>,
    pub ulimits: Option<serde_yaml::Value>,
    pub logging: Option<ComposeLogging>,
    pub deploy: Option<ComposeDeployment>,
    pub develop: Option<serde_yaml::Value>,
    pub secrets: Option<Vec<String>>,
    pub configs: Option<Vec<String>>,
    pub expose: Option<Vec<serde_yaml::Value>>,
    pub extra_hosts: Option<ListOrDict>,
    pub dns: Option<serde_yaml::Value>,
    pub dns_search: Option<serde_yaml::Value>,
    pub tmpfs: Option<serde_yaml::Value>,
    pub shm_size: Option<serde_yaml::Value>,
    pub mem_limit: Option<serde_yaml::Value>,
    pub memswap_limit: Option<serde_yaml::Value>,
    pub cpus: Option<serde_yaml::Value>,
    pub cpu_shares: Option<i64>,
    pub platform: Option<String>,
    pub pull_policy: Option<String>,
    pub profiles: Option<Vec<String>>,
    pub scale: Option<u32>,
    pub extends: Option<serde_yaml::Value>,
    pub post_start: Option<Vec<serde_yaml::Value>>,
    pub pre_stop: Option<Vec<serde_yaml::Value>>,
}

impl ComposeService {
    pub fn needs_build(&self) -> bool { self.build.is_some() && self.image.is_none() }
    pub fn image_ref(&self, service_name: &str) -> String { self.image.clone().unwrap_or_else(|| format!("{}-image", service_name)) }
    pub fn resolved_env(&self) -> std::collections::HashMap<String, String> { self.environment.as_ref().map(|e| e.to_map()).unwrap_or_default() }
    pub fn port_strings(&self) -> Vec<String> { self.ports.as_deref().unwrap_or(&[]).iter().map(|p| p.to_string_form()).collect() }
    pub fn volume_strings(&self) -> Vec<String> {
        self.volumes.as_deref().unwrap_or(&[]).iter().map(|v| match serde_yaml::from_value::<ComposeServiceVolume>(v.clone()) {
            Ok(v) => {
                let src = v.source.as_deref().unwrap_or("");
                let tgt = v.target.as_deref().unwrap_or("");
                if v.read_only.unwrap_or(false) { format!("{}:{}:ro", src, tgt) } else { format!("{}:{}", src, tgt) }
            }
            Err(_) => yaml_value_to_str(v)
        }).collect()
    }
    pub fn command_list(&self) -> Option<Vec<String>> {
        self.command.as_ref().map(|c| match c {
            serde_yaml::Value::String(s) => vec![s.clone()],
            serde_yaml::Value::Sequence(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
            _ => vec![],
        })
    }
}

// ============ ComposeSpec ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    pub fn parse_str(yaml: &str) -> crate::error::Result<Self> {
        serde_yaml::from_str(yaml).map_err(crate::error::ComposeError::ParseError)
    }
    pub fn merge(&mut self, other: ComposeSpec) {
        for (k, v) in other.services { self.services.insert(k, v); }
        if let Some(nets) = other.networks { let ex = self.networks.get_or_insert_with(IndexMap::new); for (k, v) in nets { ex.insert(k, v); } }
        if let Some(vols) = other.volumes { let ex = self.volumes.get_or_insert_with(IndexMap::new); for (k, v) in vols { ex.insert(k, v); } }
        if let Some(secs) = other.secrets { let ex = self.secrets.get_or_insert_with(IndexMap::new); for (k, v) in secs { ex.insert(k, v); } }
        if let Some(cfgs) = other.configs { let ex = self.configs.get_or_insert_with(IndexMap::new); for (k, v) in cfgs { ex.insert(k, v); } }
        if other.name.is_some() { self.name = other.name; }
        if other.version.is_some() { self.version = other.version; }
        for (k, v) in other.extensions { self.extensions.insert(k, v); }
    }
}

// ============ Handles and Specs ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHandle {
    pub stack_id: u64,
    pub project_name: String,
    pub services: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerSpec {
    pub image: String,
    pub name: Option<String>,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub labels: Option<std::collections::HashMap<String, String>>,
    pub cmd: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub network: Option<String>,
    pub rm: Option<bool>,
    pub read_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerHandle { pub id: String, pub name: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub labels: std::collections::HashMap<String, String>,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLogs { pub stdout: String, pub stderr: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    pub created: String,
}
