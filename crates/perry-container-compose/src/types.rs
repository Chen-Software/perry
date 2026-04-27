use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use indexmap::IndexMap;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum ListOrDict {
    Dict(HashMap<String, Option<serde_json::Value>>),
    List(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub created: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContainerLogs {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    pub created: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum IsolationLevel {
    None,
    Process,
    Container,
    MicroVm,
    Wasm,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackendInfo {
    pub name: String,
    pub available: bool,
    pub reason: Option<String>,
    pub version: Option<String>,
    pub mode: String, // "local" | "remote"
    pub isolation_level: IsolationLevel,
}

// Compose Types
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeSpec {
    pub name: Option<String>,
    pub version: Option<String>,
    pub services: IndexMap<String, ComposeService>,
    pub networks: Option<IndexMap<String, ComposeNetwork>>,
    pub volumes: Option<IndexMap<String, ComposeVolume>>,
    pub secrets: Option<IndexMap<String, ComposeSecret>>,
    pub configs: Option<IndexMap<String, ComposeConfig>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeService {
    pub image: Option<String>,
    pub build: Option<ComposeServiceBuildOrString>,
    pub command: Option<CommandOrString>,
    pub entrypoint: Option<CommandOrString>,
    pub environment: Option<ListOrDict>,
    pub env_file: Option<EnvFile>,
    pub ports: Option<Vec<serde_json::Value>>, // Mix of string, number, or object
    pub volumes: Option<Vec<serde_json::Value>>, // Mix of string or object
    pub networks: Option<serde_json::Value>,
    pub depends_on: Option<DependsOn>,
    pub healthcheck: Option<ComposeHealthcheck>,
    pub deploy: Option<ComposeDeployment>,
    pub logging: Option<ComposeLogging>,
    pub container_name: Option<String>,
    pub labels: Option<ListOrDict>,
    pub extra_hosts: Option<ListOrDict>,
    pub sysctls: Option<ListOrDict>,
    pub read_only: Option<bool>,
    pub isolation_level: Option<IsolationLevel>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum ComposeServiceBuildOrString {
    String(String),
    Build(ComposeServiceBuild),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeServiceBuild {
    pub context: String,
    #[serde(alias = "dockerfile")]
    pub containerfile: Option<String>,
    pub args: Option<ListOrDict>,
    pub labels: Option<ListOrDict>,
    pub target: Option<String>,
    pub network: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum CommandOrString {
    String(String),
    List(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum EnvFile {
    String(String),
    List(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum DependsOn {
    List(Vec<String>),
    Dict(IndexMap<String, ComposeDependsOn>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeDependsOn {
    pub condition: DependsOnCondition,
    pub required: Option<bool>,
    pub restart: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DependsOnCondition {
    ServiceStarted,
    ServiceHealthy,
    ServiceCompletedSuccessfully,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeNetworkIpam {
    pub driver: Option<String>,
    pub config: Option<Vec<HashMap<String, String>>>,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeVolume {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub driver_opts: Option<HashMap<String, String>>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeConfig {
    pub name: Option<String>,
    pub content: Option<String>,
    pub environment: Option<String>,
    pub file: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
    pub template_driver: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeHealthcheck {
    pub test: CommandOrString,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
    pub start_interval: Option<String>,
    pub disable: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeDeployment {
    pub mode: Option<String>,
    pub replicas: Option<u32>,
    pub labels: Option<ListOrDict>,
    pub resources: Option<serde_json::Value>,
    pub restart_policy: Option<serde_json::Value>,
    pub placement: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeLogging {
    pub driver: Option<String>,
    pub options: Option<HashMap<String, String>>,
}
