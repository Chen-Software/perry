//! All compose-spec Rust types.
//!
//! This module contains every struct and enum needed to represent a
//! compose-spec YAML document, plus the opaque `ComposeHandle` returned by
//! `ComposeEngine::up()`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Convert a `serde_yaml::Value` to a string representation.
fn yaml_value_to_str(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => String::new(),
        _ => format!("{}", serde_yaml::to_string(v).unwrap_or_default())
            .trim()
            .to_owned(),
    }
}

// ============ ListOrDict ============

/// compose-spec `list_or_dict` pattern.
/// Used for environment, labels, extra_hosts, sysctls, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ListOrDict {
    Dict(IndexMap<String, Option<serde_yaml::Value>>),
    List(Vec<String>),
}

impl ListOrDict {
    /// Convert to a flat `HashMap<String, String>`.
    /// Dict values are stringified; List entries are split on `=`.
    pub fn to_map(&self) -> std::collections::HashMap<String, String> {
        match self {
            ListOrDict::Dict(map) => map
                .iter()
                .map(|(k, v)| {
                    let val = match v {
                        Some(serde_yaml::Value::String(s)) => s.clone(),
                        Some(serde_yaml::Value::Number(n)) => n.to_string(),
                        Some(serde_yaml::Value::Bool(b)) => b.to_string(),
                        Some(serde_yaml::Value::Null) | None => String::new(),
                        Some(other) => match other {
                            serde_yaml::Value::String(s) => s.clone(),
                            _ => serde_yaml::to_string(other).unwrap_or_else(|_| "{}".to_string()),
                        },
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

// ============ StringOrList ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrList {
    String(String),
    List(Vec<String>),
}

impl StringOrList {
    pub fn to_list(&self) -> Vec<String> {
        match self {
            StringOrList::String(s) => vec![s.clone()],
            StringOrList::List(l) => l.clone(),
        }
    }
}

// ============ DependsOn ============

/// `depends_on` condition values (compose-spec §service.depends_on)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependsOnCondition {
    ServiceStarted,
    ServiceHealthy,
    ServiceCompletedSuccessfully,
}

/// Per-dependency entry in the object form of depends_on
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ComposeDependsOn {
    pub condition: Option<DependsOnCondition>,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub restart: Option<bool>,
}

/// `depends_on` can be a list of service names or a map with conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependsOnSpec {
    List(Vec<String>),
    Map(IndexMap<String, ComposeDependsOn>),
}

impl DependsOnSpec {
    /// Return all dependency service names.
    pub fn service_names(&self) -> Vec<String> {
        match self {
            DependsOnSpec::List(names) => names.clone(),
            DependsOnSpec::Map(map) => map.keys().cloned().collect(),
        }
    }
}

// ============ Volume ============

/// Volume mount type (compose-spec §service.volumes[].type)
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

/// Long-form volume mount (compose-spec §service.volumes[])
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
#[serde(rename_all = "snake_case")]
pub struct ComposeServiceVolumeBind {
    pub propagation: Option<String>,
    pub create_host_path: Option<bool>,
    pub recursive: Option<String>,
    pub selinux: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

/// Short or long volume form
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VolumeEntry {
    Short(String),
    Long(ComposeServiceVolume),
}

impl VolumeEntry {
    /// Convert to "source:target[:ro]" string form for backend CLI args.
    pub fn to_string_form(&self) -> String {
        match self {
            VolumeEntry::Short(s) => s.clone(),
            VolumeEntry::Long(v) => {
                let src = v.source.as_deref().unwrap_or("");
                let tgt = v.target.as_deref().unwrap_or("");
                if v.read_only.unwrap_or(false) {
                    format!("{}:{}:ro", src, tgt)
                } else {
                    format!("{}:{}", src, tgt)
                }
            }
        }
    }
}

// ============ Port ============

/// Port mapping (long form, compose-spec §service.ports[])
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ComposeServicePort {
    pub name: Option<String>,
    pub mode: Option<String>,
    pub host_ip: Option<String>,
    pub target: serde_yaml::Value,
    pub published: Option<serde_yaml::Value>,
    pub protocol: Option<String>,
    pub app_protocol: Option<String>,
}

/// Port can be a short string/number or a long-form object
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PortSpec {
    Short(serde_yaml::Value),
    Long(ComposeServicePort),
}

impl PortSpec {
    /// Convert to "host:container" string form for backend CLI args.
    pub fn to_string_form(&self) -> String {
        match self {
            PortSpec::Short(v) => yaml_value_to_str(v),
            PortSpec::Long(p) => {
                let container = yaml_value_to_str(&p.target);
                match &p.published {
                    Some(pub_) => {
                        let host = yaml_value_to_str(pub_);
                        format!("{}:{}", host, container)
                    }
                    None => container,
                }
            }
        }
    }
}

// ============ Networks on service ============

/// Service network attachment config
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ComposeServiceNetworkConfig {
    pub aliases: Option<Vec<String>>,
    pub interface_name: Option<String>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub link_local_ips: Option<Vec<String>>,
    pub mac_address: Option<String>,
    pub driver_opts: Option<IndexMap<String, serde_yaml::Value>>,
    pub priority: Option<f64>,
    pub gw_priority: Option<f64>,
}

/// `networks` field on a service: list or map
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServiceNetworks {
    List(Vec<String>),
    Map(IndexMap<String, Option<ComposeServiceNetworkConfig>>),
}

impl ServiceNetworks {
    pub fn names(&self) -> Vec<String> {
        match self {
            ServiceNetworks::List(v) => v.clone(),
            ServiceNetworks::Map(m) => m.keys().cloned().collect(),
        }
    }
}

// ============ EnvFileEntry ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnvFileEntry {
    Short(String),
    Long(EnvFileConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvFileConfig {
    pub path: String,
    #[serde(default = "default_true")]
    pub required: bool,
    pub format: Option<String>,
}

fn default_true() -> bool {
    true
}

// ============ Ulimit ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Ulimit {
    Single(i64),
    SoftHard { soft: i64, hard: i64 },
}

// ============ DeviceEntry ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DeviceEntry {
    Short(String),
    Long(DeviceRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRequest {
    pub source: String,
    pub target: Option<String>,
    pub permissions: Option<String>,
}

// ============ BlkioConfig ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlkioConfig {
    pub device_read_bps: Option<Vec<BlkioLimit>>,
    pub device_read_iops: Option<Vec<BlkioLimit>>,
    pub device_write_bps: Option<Vec<BlkioLimit>>,
    pub device_write_iops: Option<Vec<BlkioLimit>>,
    pub weight: Option<u16>,
    pub weight_device: Option<Vec<BlkioWeight>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlkioLimit {
    pub path: String,
    pub rate: serde_yaml::Value, // string or int
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlkioWeight {
    pub path: String,
    pub weight: u16,
}

// ============ Build ============

/// Build configuration (string shorthand or full object)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BuildSpec {
    Context(String),
    Config(ComposeServiceBuild),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ComposeServiceBuild {
    pub context: Option<String>,
    #[serde(alias = "dockerfile")]
    pub containerfile: Option<String>,
    pub dockerfile_inline: Option<String>,
    pub args: Option<ListOrDict>,
    pub ssh: Option<ListOrDict>,
    pub labels: Option<ListOrDict>,
    pub cache_from: Option<Vec<String>>,
    pub cache_to: Option<Vec<String>>,
    pub no_cache: Option<bool>,
    pub additional_contexts: Option<IndexMap<String, String>>,
    pub network: Option<String>,
    pub provenance: Option<serde_yaml::Value>, // bool or string
    pub sbom: Option<serde_yaml::Value>,       // bool or string
    pub pull: Option<bool>,
    pub target: Option<String>,
    pub shm_size: Option<serde_yaml::Value>, // int or string
    pub extra_hosts: Option<ListOrDict>,
    pub isolation: Option<String>,
    pub privileged: Option<bool>,
    pub secrets: Option<Vec<ServiceSecret>>,
    pub tags: Option<Vec<String>>,
    pub ulimits: Option<IndexMap<String, Ulimit>>,
    pub platforms: Option<Vec<String>>,
    pub entitlements: Option<Vec<String>>,
}

impl BuildSpec {
    pub fn context(&self) -> Option<&str> {
        match self {
            BuildSpec::Context(s) => Some(s.as_str()),
            BuildSpec::Config(b) => b.context.as_deref(),
        }
    }

    pub fn as_build(&self) -> ComposeServiceBuild {
        match self {
            BuildSpec::Context(ctx) => ComposeServiceBuild {
                context: Some(ctx.clone()),
                containerfile: None,
                ..Default::default()
            },
            BuildSpec::Config(b) => b.clone(),
        }
    }
}

// ============ Extends ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendsSpec {
    pub file: Option<String>,
    pub service: String,
}

// ============ Develop ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopConfig {
    pub watch: Option<Vec<WatchEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnvFileSpec {
    String(String),
    List(Vec<EnvFileEntry>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEntry {
    pub path: String,
    pub action: String, // sync, rebuild, sync+restart
    pub target: Option<String>,
    pub ignore: Option<Vec<String>>,
}

// ============ Lifecycle Hooks ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleHook {
    pub command: StringOrList,
    pub user: Option<String>,
    pub privileged: Option<bool>,
    pub working_dir: Option<String>,
    pub environment: Option<ListOrDict>,
}

// ============ CredentialSpec ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSpec {
    pub config: Option<String>,
    pub file: Option<String>,
    pub registry: Option<String>,
}

// ============ Models ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeModel {
    pub name: Option<String>,
    pub model: String,
    pub context_size: Option<i64>,
    pub runtime_flags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServiceModels {
    List(Vec<String>),
    Map(IndexMap<String, ServiceModelConfig>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceModelConfig {
    pub endpoint_var: Option<String>,
    pub model_var: Option<String>,
}

// ============ Healthcheck ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HealthcheckTest {
    String(String),
    List(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ComposeHealthcheck {
    pub test: Option<HealthcheckTest>,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
    pub start_interval: Option<String>,
    pub disable: Option<bool>,
}

// ============ Deployment ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ComposeDeployment {
    pub mode: Option<String>,
    pub replicas: Option<u32>,
    pub labels: Option<ListOrDict>,
    pub resources: Option<ComposeDeploymentResources>,
    pub restart_policy: Option<ComposeRestartPolicy>,
    pub placement: Option<ComposePlacement>,
    pub update_config: Option<ComposeUpdateConfig>,
    pub rollback_config: Option<ComposeUpdateConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeRestartPolicy {
    pub condition: Option<String>,
    pub delay: Option<String>,
    pub max_attempts: Option<u32>,
    pub window: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposePlacement {
    pub constraints: Option<Vec<String>>,
    pub preferences: Option<Vec<serde_yaml::Value>>,
    pub max_replicas_per_node: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeUpdateConfig {
    pub parallelism: Option<u32>,
    pub delay: Option<String>,
    pub failure_action: Option<String>,
    pub monitor: Option<String>,
    pub max_failure_ratio: Option<f32>,
    pub order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ComposeDeploymentResources {
    pub limits: Option<ComposeResourceSpec>,
    pub reservations: Option<ComposeResourceSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeResourceSpec {
    pub cpus: Option<serde_yaml::Value>, // float or string
    pub memory: Option<String>,
    pub pids: Option<i64>,
    pub devices: Option<Vec<ComposeDeviceRequest>>,
    pub generic_resources: Option<Vec<serde_yaml::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeviceRequest {
    pub driver: Option<String>,
    pub count: Option<serde_yaml::Value>,
    pub device_ids: Option<Vec<String>>,
    pub capabilities: Option<Vec<String>>,
    pub options: Option<ListOrDict>,
}

// ============ Logging ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeLogging {
    pub driver: Option<String>,
    pub options: Option<IndexMap<String, serde_yaml::Value>>,
}

// ============ Network ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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

/// Top-level network definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ComposeNetwork {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<IndexMap<String, serde_yaml::Value>>,
    pub ipam: Option<ComposeNetworkIpam>,
    pub external: Option<serde_yaml::Value>, // bool or object
    pub internal: Option<bool>,
    pub enable_ipv4: Option<bool>,
    pub enable_ipv6: Option<bool>,
    pub attachable: Option<bool>,
    pub labels: Option<ListOrDict>,
}

// ============ Volume ============

/// Top-level volume definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ComposeVolume {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<IndexMap<String, serde_yaml::Value>>,
    pub external: Option<serde_yaml::Value>, // bool or object
    pub labels: Option<ListOrDict>,
}

// ============ Secret ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServiceSecret {
    Short(String),
    Long(ServiceSecretConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSecretConfig {
    pub source: String,
    pub target: Option<String>,
    pub uid: Option<String>,
    pub gid: Option<String>,
    pub mode: Option<serde_yaml::Value>, // Can be octal string or int
}

/// Top-level secret definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ComposeSecret {
    pub name: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<serde_yaml::Value>, // bool or object
    pub labels: Option<ListOrDict>,
    pub driver: Option<String>,
    pub driver_opts: Option<IndexMap<String, serde_yaml::Value>>,
    pub template_driver: Option<String>,
}

// ============ Config ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServiceConfigRef {
    Short(String),
    Long(ServiceConfigConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfigConfig {
    pub source: String,
    pub target: Option<String>,
    pub uid: Option<String>,
    pub gid: Option<String>,
    pub mode: Option<serde_yaml::Value>, // Can be octal string or int
}

/// Top-level config definition (compose-spec `config` object)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ComposeConfig {
    pub name: Option<String>,
    pub content: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<serde_yaml::Value>, // bool or object
    pub labels: Option<ListOrDict>,
    pub template_driver: Option<String>,
}

// ============ ComposeService ============

/// Full service definition (compose-spec §service)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ComposeService {
    pub image: Option<String>,
    pub build: Option<BuildSpec>,
    pub blkio_config: Option<BlkioConfig>,
    pub command: Option<StringOrList>,
    pub container_name: Option<String>,
    pub cpu_count: Option<serde_yaml::Value>,
    pub cpu_percent: Option<serde_yaml::Value>,
    pub cpu_shares: Option<serde_yaml::Value>,
    pub cpu_period: Option<serde_yaml::Value>,
    pub cpu_quota: Option<serde_yaml::Value>,
    pub cpu_rt_period: Option<serde_yaml::Value>,
    pub cpu_rt_runtime: Option<serde_yaml::Value>,
    pub cpus: Option<serde_yaml::Value>,
    pub cpuset: Option<String>,
    pub cap_add: Option<Vec<String>>,
    pub cap_drop: Option<Vec<String>>,
    pub cgroup: Option<String>,
    pub cgroup_parent: Option<String>,
    pub configs: Option<Vec<ServiceConfigRef>>,
    pub credential_spec: Option<CredentialSpec>,
    pub depends_on: Option<DependsOnSpec>,
    pub deploy: Option<ComposeDeployment>,
    pub device_cgroup_rules: Option<Vec<String>>,
    pub devices: Option<Vec<DeviceEntry>>,
    pub dns: Option<StringOrList>,
    pub dns_opt: Option<Vec<String>>,
    pub dns_search: Option<StringOrList>,
    pub domainname: Option<String>,
    pub entrypoint: Option<StringOrList>,
    pub env_file: Option<EnvFileSpec>,
    pub environment: Option<ListOrDict>,
    pub expose: Option<Vec<ExposeSpec>>,
    pub extends: Option<ExtendsSpec>,
    pub external_links: Option<Vec<String>>,
    pub extra_hosts: Option<ListOrDict>,
    pub group_add: Option<Vec<serde_yaml::Value>>,
    pub healthcheck: Option<ComposeHealthcheck>,
    pub hostname: Option<String>,
    pub init: Option<serde_yaml::Value>, // bool or string
    pub ipc: Option<String>,
    pub isolation: Option<String>,
    pub labels: Option<ListOrDict>,
    pub links: Option<Vec<String>>,
    pub logging: Option<ComposeLogging>,
    pub models: Option<ServiceModels>,
    pub network_mode: Option<String>,
    pub networks: Option<ServiceNetworks>,
    pub mac_address: Option<String>,
    pub mem_limit: Option<serde_yaml::Value>,
    pub mem_reservation: Option<serde_yaml::Value>,
    pub mem_swappiness: Option<serde_yaml::Value>,
    pub memswap_limit: Option<serde_yaml::Value>,
    pub oom_kill_disable: Option<serde_yaml::Value>,
    pub oom_score_adj: Option<serde_yaml::Value>,
    pub pid: Option<String>,
    pub pids_limit: Option<serde_yaml::Value>,
    pub platform: Option<String>,
    pub ports: Option<Vec<PortSpec>>,
    pub privileged: Option<serde_yaml::Value>,
    pub profiles: Option<Vec<String>>,
    pub pull_policy: Option<String>,
    pub read_only: Option<serde_yaml::Value>,
    pub restart: Option<String>,
    pub runtime: Option<String>,
    pub scale: Option<serde_yaml::Value>,
    pub secrets: Option<Vec<ServiceSecret>>,
    pub security_opt: Option<Vec<String>>,
    pub shm_size: Option<serde_yaml::Value>,
    pub stdin_open: Option<serde_yaml::Value>,
    pub stop_grace_period: Option<String>,
    pub stop_signal: Option<String>,
    pub storage_opt: Option<IndexMap<String, String>>,
    pub sysctls: Option<ListOrDict>,
    pub tmpfs: Option<StringOrList>,
    pub tty: Option<serde_yaml::Value>,
    pub ulimits: Option<IndexMap<String, Ulimit>>,
    pub user: Option<String>,
    pub userns_mode: Option<String>,
    pub uts: Option<String>,
    pub volumes: Option<Vec<VolumeEntry>>,
    pub volumes_from: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub develop: Option<DevelopConfig>,
    pub post_start: Option<Vec<LifecycleHook>>,
    pub pre_stop: Option<Vec<LifecycleHook>>,
}

impl ComposeService {
    /// Whether the service needs to build an image before running.
    pub fn needs_build(&self) -> bool {
        self.build.is_some() && self.image.is_none()
    }

    /// Return the image tag to use for this service.
    pub fn image_ref(&self, service_name: &str) -> String {
        if let Some(image) = &self.image {
            return image.clone();
        }
        format!("{}-image", service_name)
    }

    /// Get resolved environment as a flat map.
    pub fn resolved_env(&self) -> std::collections::HashMap<String, String> {
        self.environment
            .as_ref()
            .map(|e| e.to_map())
            .unwrap_or_default()
    }

    /// Get port strings in "host:container" form.
    pub fn port_strings(&self) -> Vec<String> {
        self.ports
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|p| p.to_string_form())
            .collect()
    }

    /// Get volume mount strings.
    pub fn volume_strings(&self) -> Vec<String> {
        self.volumes
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|v| v.to_string_form())
            .collect()
    }

    /// Get the explicit container_name, if set.
    pub fn explicit_name(&self) -> Option<&str> {
        self.container_name.as_deref()
    }

    /// Get command as a list of strings.
    pub fn command_list(&self) -> Option<Vec<String>> {
        self.command.as_ref().map(|c| c.to_list())
    }

    /// Get entrypoint as a list of strings.
    pub fn entrypoint_list(&self) -> Option<Vec<String>> {
        self.entrypoint.as_ref().map(|e| e.to_list())
    }

    /// Build a `ContainerSpec` from this service's compose-spec config.
    ///
    /// Used by [`Self::run_command`] and any caller that needs the canonical
    /// runtime-side spec produced from a YAML service entry. Mirrors the
    /// inline conversion in [`crate::compose::ComposeEngine::up`] so both
    /// orchestration paths produce identical containers.
    ///
    /// `service_name` (separate from `container_name`) is the compose-spec
    /// service key — used to derive the build-time image tag via
    /// [`Self::image_ref`] when no `image:` is declared. Without this, a
    /// build-only service would resolve to an empty image name in the spec
    /// and fail at `backend.run`.
    pub fn to_container_spec(&self, service_name: &str, container_name: &str) -> ContainerSpec {
        let network = match &self.networks {
            Some(crate::types::ServiceNetworks::List(l)) => l.first().cloned(),
            Some(crate::types::ServiceNetworks::Map(m)) => m.keys().next().cloned(),
            None => None,
        };
        let labels = self.labels.as_ref().map(|l| l.to_map());
        ContainerSpec {
            image: self.image_ref(service_name),
            name: Some(container_name.to_string()),
            ports: Some(self.port_strings()),
            volumes: Some(self.volume_strings()),
            env: Some(self.resolved_env()),
            cmd: self.command_list(),
            entrypoint: self.entrypoint_list(),
            network,
            rm: None,
            read_only: self.read_only.as_ref().and_then(|v| v.as_bool()),
            labels,
            privileged: self.privileged.as_ref().and_then(|v| v.as_bool()),
            user: self.user.clone(),
            workdir: self.working_dir.clone(),
            cap_add: self.cap_add.clone(),
            cap_drop: self.cap_drop.clone(),
            // network_aliases is populated separately by ComposeEngine::up
            // (using the service KEY + any long-form `aliases` from the
            // compose-spec) — this single-service `to_container_spec`
            // helper has no service-graph context to derive them from.
            network_aliases: None,
        }
    }

    /// Whether this service's container currently exists on the backend.
    ///
    /// Returns `Ok(true)` if `inspect` resolves; `Ok(false)` for a NotFound
    /// or any backend error treated as "not found" (matches Go reference's
    /// container-compose `Service::Exists` semantics — "no answer" → "no
    /// container"). Genuine connectivity errors are folded into `false`
    /// because the caller's next step is always to re-create.
    pub async fn exists(
        &self,
        backend: &dyn crate::backend::ContainerBackend,
        service_name: &str,
    ) -> crate::error::Result<bool> {
        let container_name = crate::service::service_container_name(self, service_name);
        Ok(backend.inspect(&container_name).await.is_ok())
    }

    /// Whether this service's container is currently running.
    ///
    /// Returns `Ok(false)` if the container doesn't exist OR exists but its
    /// status is anything other than "running". Errors propagate only from
    /// genuine inspect-call failures other than NotFound.
    pub async fn is_running(
        &self,
        backend: &dyn crate::backend::ContainerBackend,
        service_name: &str,
    ) -> crate::error::Result<bool> {
        let container_name = crate::service::service_container_name(self, service_name);
        match backend.inspect(&container_name).await {
            Ok(info) => Ok(info.status == "running"),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(false),
            Err(_) => Ok(false),
        }
    }

    /// Build the service's image (when `build` is set). No-op for image-only
    /// services. Mirrors the Go reference's `Service::BuildCommand`.
    pub async fn build_command(
        &self,
        backend: &dyn crate::backend::ContainerBackend,
        service_name: &str,
    ) -> crate::error::Result<()> {
        if let Some(build) = &self.build {
            let image_name = self.image_ref(service_name);
            backend.build(&build.as_build(), &image_name).await?;
        }
        Ok(())
    }

    /// Create-and-run the service's container.
    ///
    /// Caller is responsible for having invoked [`Self::build_command`]
    /// first when `needs_build()` is true; the canonical orchestrator in
    /// `orchestrate.rs` handles that ordering. The returned handle is the
    /// backend's container id (also tracked by `ComposeEngine` for rollback).
    pub async fn run_command(
        &self,
        backend: &dyn crate::backend::ContainerBackend,
        service_name: &str,
    ) -> crate::error::Result<ContainerHandle> {
        let container_name = crate::service::service_container_name(self, service_name);
        let spec = self.to_container_spec(service_name, &container_name);
        backend.run(&spec).await
    }

    /// Start an already-created (stopped) container.
    pub async fn start_command(
        &self,
        backend: &dyn crate::backend::ContainerBackend,
        service_name: &str,
    ) -> crate::error::Result<()> {
        let container_name = crate::service::service_container_name(self, service_name);
        backend.start(&container_name).await
    }

    /// Inspect the service's container.
    pub async fn inspect_command(
        &self,
        backend: &dyn crate::backend::ContainerBackend,
        service_name: &str,
    ) -> crate::error::Result<ContainerInfo> {
        let container_name = crate::service::service_container_name(self, service_name);
        backend.inspect(&container_name).await
    }
}

// ============ Include ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IncludeEntry {
    Short(String),
    Long(IncludeConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncludeConfig {
    pub path: StringOrList,
    pub project_directory: Option<String>,
    pub env_file: Option<StringOrList>,
}

// ============ Expose ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExposeSpec {
    String(String),
    Integer(u16),
}

// ============ Namespace Modes ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum NamespaceMode {
    Host(String),    // e.g. "host"
    Service(String), // e.g. "service:foo"
    Other(String),
}

// ============ ComposeSpec ============

/// Root compose spec (compose-spec §root)
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
    pub include: Option<Vec<IncludeEntry>>,
    pub models: Option<IndexMap<String, ComposeModel>>,
    #[serde(flatten)]
    pub extensions: IndexMap<String, serde_yaml::Value>,
}

impl ComposeSpec {
    /// Parse from a YAML string.
    pub fn parse_str(yaml: &str) -> Result<Self, crate::error::ComposeError> {
        serde_yaml::from_str(yaml).map_err(crate::error::ComposeError::ParseError)
    }

    /// Parse from raw YAML bytes.
    pub fn parse(yaml: &[u8]) -> Result<Self, crate::error::ComposeError> {
        serde_yaml::from_slice(yaml).map_err(crate::error::ComposeError::ParseError)
    }

    /// Serialize to YAML.
    pub fn to_yaml(&self) -> Result<String, crate::error::ComposeError> {
        serde_yaml::to_string(self).map_err(|e| crate::error::ComposeError::ParseError(e))
    }

    /// Merge another ComposeSpec into this one (last-writer-wins for all maps).
    pub fn merge(&mut self, other: ComposeSpec) {
        for (name, service) in other.services {
            self.services.insert(name, service);
        }

        if let Some(nets) = other.networks {
            let existing = self.networks.get_or_insert_with(IndexMap::new);
            for (name, net) in nets {
                existing.insert(name, net);
            }
        }

        if let Some(vols) = other.volumes {
            let existing = self.volumes.get_or_insert_with(IndexMap::new);
            for (name, vol) in vols {
                existing.insert(name, vol);
            }
        }

        if let Some(secs) = other.secrets {
            let existing = self.secrets.get_or_insert_with(IndexMap::new);
            for (name, sec) in secs {
                existing.insert(name, sec);
            }
        }

        if let Some(cfgs) = other.configs {
            let existing = self.configs.get_or_insert_with(IndexMap::new);
            for (name, cfg) in cfgs {
                existing.insert(name, cfg);
            }
        }

        if other.name.is_some() {
            self.name = other.name;
        }
        if other.version.is_some() {
            self.version = other.version;
        }

        // Merge extensions
        for (k, v) in other.extensions {
            self.extensions.insert(k, v);
        }
    }
}

// ============ ComposeHandle ============

/// Opaque handle to a running compose stack.
/// The stack ID is used to look up the live ComposeEngine in a global registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ComposeHandle {
    pub stack_id: u64,
    pub project_name: String,
    pub services: Vec<String>,
}

// ============ Container types (for single-container API) ============

/// Specification for running a single container.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    pub labels: Option<std::collections::HashMap<String, String>>,
    pub privileged: Option<bool>,
    pub user: Option<String>,
    pub workdir: Option<String>,
    pub cap_add: Option<Vec<String>>,
    pub cap_drop: Option<Vec<String>>,
    /// Additional DNS-resolvable names this container should answer to
    /// on its attached network (`--network-alias <name>` per entry).
    /// Populated by `ComposeEngine::up()` from the service key plus any
    /// long-form `networks: { foo: { aliases: [...] } }` in the spec.
    /// Sibling containers on the same network can then resolve the
    /// service key (e.g. `db:5432`) via the runtime's embedded DNS,
    /// matching docker-compose semantics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_aliases: Option<Vec<String>>,
}

/// Handle returned after creating/running a container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerHandle {
    pub id: String,
    pub name: Option<String>,
}

/// Information about a running (or stopped) container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub labels: std::collections::HashMap<String, String>,
    pub created: String,
    #[serde(default)]
    pub ip_address: String,
}

/// Logs from a container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLogs {
    pub stdout: String,
    pub stderr: String,
}

/// Information about a container image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    pub created: String,
}
