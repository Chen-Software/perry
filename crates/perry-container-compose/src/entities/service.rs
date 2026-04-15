//! Service entity — full compose-spec service definition.
//!
//! All field names conform to the official compose-spec JSON schema.

use crate::error::Result;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============ ListOrDict ============

/// compose-spec `list_or_dict` — either a mapping or a KEY=VALUE list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ListOrDict {
    Dict(HashMap<String, Option<serde_json::Value>>),
    List(Vec<String>),
}

impl ListOrDict {
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

// ============ String | List ============

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependsOnCondition {
    /// "service_started" | "service_healthy" | "service_completed_successfully"
    pub condition: Option<String>,
    pub required: Option<bool>,
    pub restart: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependsOn {
    List(Vec<String>),
    Map(HashMap<String, DependsOnCondition>),
}

impl DependsOn {
    pub fn service_names(&self) -> Vec<String> {
        match self {
            DependsOn::List(names) => names.clone(),
            DependsOn::Map(map) => map.keys().cloned().collect(),
        }
    }
}

// ============ Build ============

/// Full build configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Build {
    pub context: Option<String>,
    pub dockerfile: Option<String>,
    pub dockerfile_inline: Option<String>,
    #[serde(default)]
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

/// `build` field: string shorthand or full object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BuildEntry {
    String(String),
    Object(Build),
}

impl BuildEntry {
    pub fn context(&self) -> Option<&str> {
        match self {
            BuildEntry::String(s) => Some(s.as_str()),
            BuildEntry::Object(b) => b.context.as_deref(),
        }
    }
    pub fn as_build(&self) -> Build {
        match self {
            BuildEntry::String(ctx) => Build {
                context: Some(ctx.clone()),
                ..Default::default()
            },
            BuildEntry::Object(b) => b.clone(),
        }
    }
}

// ============ Port ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePort {
    pub name: Option<String>,
    pub mode: Option<String>,
    pub host_ip: Option<String>,
    pub target: serde_json::Value,
    pub published: Option<serde_json::Value>,
    pub protocol: Option<String>,
    pub app_protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PortEntry {
    Short(serde_json::Value),
    Long(ServicePort),
}

impl PortEntry {
    /// Convert to "host:container" string form for backend CLI args.
    pub fn to_string_form(&self) -> String {
        match self {
            PortEntry::Short(v) => v.to_string().trim_matches('"').to_owned(),
            PortEntry::Long(p) => {
                let container = p.target.to_string().trim_matches('"').to_owned();
                match &p.published {
                    Some(pub_) => {
                        let host = pub_.to_string().trim_matches('"').to_owned();
                        format!("{}:{}", host, container)
                    }
                    None => container,
                }
            }
        }
    }
}

// ============ Volume Mount ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceVolume {
    #[serde(rename = "type")]
    pub volume_type: String,
    pub source: Option<String>,
    pub target: Option<String>,
    pub read_only: Option<bool>,
    pub consistency: Option<String>,
    pub bind: Option<serde_json::Value>,
    pub volume: Option<serde_json::Value>,
    pub tmpfs: Option<serde_json::Value>,
    pub image: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VolumeEntry {
    Short(String),
    Long(ServiceVolume),
}

impl VolumeEntry {
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

// ============ Networks on service ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceNetworkConfig {
    pub aliases: Option<Vec<String>>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServiceNetworks {
    List(Vec<String>),
    Map(HashMap<String, Option<ServiceNetworkConfig>>),
}

impl ServiceNetworks {
    pub fn names(&self) -> Vec<String> {
        match self {
            ServiceNetworks::List(v) => v.clone(),
            ServiceNetworks::Map(m) => m.keys().cloned().collect(),
        }
    }
}

// ============ Healthcheck ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Healthcheck {
    pub test: serde_json::Value,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
    pub start_interval: Option<String>,
    pub disable: Option<bool>,
}

// ============ Logging ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logging {
    pub driver: Option<String>,
    pub options: Option<HashMap<String, Option<String>>>,
}

// ============ Deploy ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeployResourceSpec {
    pub cpus: Option<serde_json::Value>,
    pub memory: Option<String>,
    pub pids: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeployResources {
    pub limits: Option<DeployResourceSpec>,
    pub reservations: Option<DeployResourceSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeployRestartPolicy {
    pub condition: Option<String>,
    pub delay: Option<String>,
    pub max_attempts: Option<u32>,
    pub window: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeployUpdateConfig {
    pub parallelism: Option<u32>,
    pub delay: Option<String>,
    pub failure_action: Option<String>,
    pub monitor: Option<String>,
    pub max_failure_ratio: Option<f64>,
    pub order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Deploy {
    pub mode: Option<String>,
    pub replicas: Option<u32>,
    pub labels: Option<ListOrDict>,
    pub resources: Option<DeployResources>,
    pub restart_policy: Option<DeployRestartPolicy>,
    pub update_config: Option<DeployUpdateConfig>,
    pub rollback_config: Option<DeployUpdateConfig>,
    pub placement: Option<serde_json::Value>,
}

// ============ Restart Policy ============

/// Typed restart policy (legacy enum form, used in CLI display).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    No,
    Always,
    OnFailure,
    UnlessStopped,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        RestartPolicy::No
    }
}

// ============ Service ============

/// A full compose-spec service definition.
///
/// All field names match Docker Compose YAML conventions and the
/// official compose-spec JSON schema.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Service {
    // ── image / build ──
    pub image: Option<String>,
    pub build: Option<BuildEntry>,

    // ── command / entrypoint ──
    pub command: Option<StringOrList>,
    pub entrypoint: Option<StringOrList>,

    // ── environment ──
    pub environment: Option<ListOrDict>,
    pub env_file: Option<serde_json::Value>,

    // ── networking ──
    pub ports: Option<Vec<PortEntry>>,
    pub networks: Option<ServiceNetworks>,
    pub network_mode: Option<String>,
    pub hostname: Option<String>,
    pub extra_hosts: Option<ListOrDict>,
    pub dns: Option<serde_json::Value>,
    pub dns_search: Option<serde_json::Value>,
    pub expose: Option<Vec<serde_json::Value>>,

    // ── storage ──
    pub volumes: Option<Vec<VolumeEntry>>,
    pub tmpfs: Option<serde_json::Value>,
    pub shm_size: Option<serde_json::Value>,

    // ── dependencies ──
    pub depends_on: Option<DependsOn>,

    // ── container identity ──
    #[serde(rename = "container_name", default)]
    pub name: Option<String>,
    pub labels: Option<ListOrDict>,

    // ── lifecycle ──
    pub restart: Option<String>,
    pub stop_signal: Option<String>,
    pub stop_grace_period: Option<String>,

    // ── healthcheck ──
    pub healthcheck: Option<Healthcheck>,

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

    // ── resources (short-form) ──
    pub mem_limit: Option<serde_json::Value>,
    pub memswap_limit: Option<serde_json::Value>,
    pub cpus: Option<serde_json::Value>,
    pub cpu_shares: Option<i64>,

    // ── deploy ──
    pub deploy: Option<Deploy>,
    pub develop: Option<serde_json::Value>,
    pub scale: Option<u32>,

    // ── logging ──
    pub logging: Option<Logging>,

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

// ============ Service Methods ============

impl Service {
    /// Generate a unique container name.
    ///
    /// Returns `container_name` if explicitly set, otherwise derives:
    /// `{safe_service_name}_{md5(image)[..8]}`
    pub fn generate_name(&self, service_name: &str) -> Result<String> {
        if let Some(explicit) = &self.name {
            return Ok(explicit.clone());
        }

        let image = self.image.as_deref().unwrap_or(service_name);

        let mut hasher = Md5::new();
        hasher.update(image.as_bytes());
        let hash = hasher.finalize();
        let hash_str = hex::encode(hash);
        let prefix = &hash_str[..8];

        let safe_name: String = service_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
            .collect();

        Ok(format!("{}_{}", safe_name, prefix))
    }

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
    pub fn resolved_env(&self) -> HashMap<String, String> {
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
}
