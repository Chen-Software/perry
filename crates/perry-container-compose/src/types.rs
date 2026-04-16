//! Root types for perry-container-compose.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

// ============ compose-spec §list_or_dict ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ListOrDict {
    List(Vec<String>),
    Dict(IndexMap<String, String>),
}

impl Default for ListOrDict {
    fn default() -> Self {
        ListOrDict::List(Vec::new())
    }
}

impl ListOrDict {
    pub fn to_map(&self) -> std::collections::HashMap<String, String> {
        match self {
            ListOrDict::Dict(m) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
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

// ============ compose-spec §depends_on ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependsOnCondition {
    ServiceStarted,
    ServiceHealthy,
    ServiceCompletedSuccessfully,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeDependsOn {
    pub condition: DependsOnCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependsOnSpec {
    List(Vec<String>),
    Dict(IndexMap<String, ComposeDependsOn>),
}

impl DependsOnSpec {
    pub fn service_names(&self) -> Vec<String> {
        match self {
            DependsOnSpec::List(v) => v.clone(),
            DependsOnSpec::Dict(m) => m.keys().cloned().collect(),
        }
    }
}

// ============ compose-spec §build ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildSpec {
    pub context: Option<String>,
    pub dockerfile: Option<String>,
    pub args: Option<ListOrDict>,
    pub target: Option<String>,
}

// ============ compose-spec §healthcheck ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeHealthcheck {
    pub test: Option<serde_yaml::Value>,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
}

// ============ compose-spec §deploy ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeployment {
    pub resources: Option<ComposeDeploymentResources>,
    pub replicas: Option<u32>,
    pub restart_policy: Option<serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeploymentResources {
    pub limits: Option<ComposeResourceSpec>,
    pub reservations: Option<ComposeResourceSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeResourceSpec {
    pub cpus: Option<String>,
    pub memory: Option<String>,
}

// ============ compose-spec §logging ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeLogging {
    pub driver: Option<String>,
    pub options: Option<IndexMap<String, String>>,
}

// ============ Ports ============

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PortSpec {
    Short(String),
    Long(ComposeServicePort),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServicePort {
    pub target: u32,
    pub published: Option<u32>,
    pub protocol: Option<String>,
    pub mode: Option<String>,
}

impl PortSpec {
    pub fn to_string_form(&self) -> String {
        match self {
            PortSpec::Short(s) => s.clone(),
            PortSpec::Long(p) => {
                if let Some(pub_port) = p.published {
                    format!("{}:{}/{}", pub_port, p.target, p.protocol.as_deref().unwrap_or("tcp"))
                } else {
                    format!("{}/{}", p.target, p.protocol.as_deref().unwrap_or("tcp"))
                }
            }
        }
    }
}

// ============ Networks ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceNetworks {
    #[serde(flatten)]
    pub networks: IndexMap<String, Option<ComposeServiceNetworkConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceNetworkConfig {
    pub aliases: Option<Vec<String>>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeNetwork {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<IndexMap<String, String>>,
    pub external: Option<bool>,
    pub internal: Option<bool>,
    pub enable_ipv6: Option<bool>,
    pub labels: Option<ListOrDict>,
}

// ============ Volumes ============

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
#[serde(untagged)]
pub enum VolumeEntry {
    Short(String),
    Long(ComposeServiceVolume),
}

impl VolumeEntry {
    pub fn to_string_form(&self) -> String {
        match self {
            VolumeEntry::Short(s) => s.clone(),
            VolumeEntry::Long(v) => {
                format!("{}:{}:{}", v.source.as_deref().unwrap_or(""), v.target, v.read_only.map(|r| if r { "ro" } else { "rw" }).unwrap_or("rw"))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceVolume {
    #[serde(rename = "type")]
    pub volume_type: Option<VolumeType>,
    pub source: Option<String>,
    pub target: String,
    pub read_only: Option<bool>,
    pub bind: Option<ComposeServiceVolumeBind>,
    pub volume: Option<ComposeServiceVolumeOpts>,
    pub tmpfs: Option<ComposeServiceVolumeTmpfs>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceVolumeBind {
    pub propagation: Option<String>,
    pub create_host_path: Option<bool>,
    pub selinux: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceVolumeOpts {
    pub nocopy: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeServiceVolumeTmpfs {
    pub size: Option<serde_yaml::Value>,
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeVolume {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<IndexMap<String, String>>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
}

// ============ Secret ============

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

// ============ Config ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeConfigObj {
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
    pub fn image_ref(&self, service_name: &str) -> String {
        if let Some(image) = &self.image {
            return image.clone();
        }
        format!("{}-image", service_name)
    }

    pub fn resolved_env(&self) -> std::collections::HashMap<String, String> {
        self.environment
            .as_ref()
            .map(|e| e.to_map())
            .unwrap_or_default()
    }

    pub fn port_strings(&self) -> Vec<String> {
        self.ports
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|p| p.to_string_form())
            .collect()
    }

    pub fn volume_strings(&self) -> Vec<String> {
        self.volumes
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|v| {
                if let Ok(short) = serde_yaml::from_value::<VolumeEntry>(v.clone()) {
                    return Some(short.to_string_form());
                }
                v.as_str().map(String::from)
            })
            .collect()
    }

    pub fn explicit_name(&self) -> Option<&str> {
        self.container_name.as_deref()
    }

    pub fn command_list(&self) -> Option<Vec<String>> {
        self.command.as_ref().map(|c| match c {
            serde_yaml::Value::String(s) => vec![s.clone()],
            serde_yaml::Value::Sequence(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
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
    pub configs: Option<IndexMap<String, Option<ComposeConfigObj>>>,
    pub include: Option<Vec<serde_yaml::Value>>,
    #[serde(flatten)]
    pub extensions: IndexMap<String, serde_yaml::Value>,
}

impl ComposeSpec {
    pub fn parse_str(yaml: &str) -> Result<Self, crate::error::ComposeError> {
        serde_yaml::from_str(yaml).map_err(crate::error::ComposeError::ParseError)
    }

    pub fn to_yaml(&self) -> Result<String, crate::error::ComposeError> {
        serde_yaml::to_string(self).map_err(crate::error::ComposeError::ParseError)
    }

    pub fn merge(&mut self, other: ComposeSpec) {
        for (name, service) in other.services {
            self.services.insert(name, service);
        }
        if let Some(nets) = other.networks {
            let existing = self.networks.get_or_insert_with(IndexMap::new);
            for (name, net) in nets { existing.insert(name, net); }
        }
        if let Some(vols) = other.volumes {
            let existing = self.volumes.get_or_insert_with(IndexMap::new);
            for (name, vol) in vols { existing.insert(name, vol); }
        }
        if let Some(secs) = other.secrets {
            let existing = self.secrets.get_or_insert_with(IndexMap::new);
            for (name, sec) in secs { existing.insert(name, sec); }
        }
        if let Some(cfgs) = other.configs {
            let existing = self.configs.get_or_insert_with(IndexMap::new);
            for (name, cfg) in cfgs { existing.insert(name, cfg); }
        }
        if other.name.is_some() { self.name = other.name; }
        if other.version.is_some() { self.version = other.version; }
        for (k, v) in other.extensions { self.extensions.insert(k, v); }
    }
}

// ============ ComposeHandle ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeHandle {
    pub stack_id: u64,
    pub project_name: String,
    pub services: Vec<String>,
}

// ============ Container types ============

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
