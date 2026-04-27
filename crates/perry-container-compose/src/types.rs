use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use indexmap::IndexMap;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum ListOrDict {
    Dict(HashMap<String, Option<String>>),
    List(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContainerHandle {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub created: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ContainerLogs {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: u64,
    pub created: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    None,
    Process,
    Container,
    MicroVm,
    Wasm,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackendInfo {
    pub name: String,
    pub available: bool,
    pub reason: Option<String>,
    pub version: Option<String>,
    pub mode: String, // "local" | "remote"
    pub isolation_level: IsolationLevel,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ComposeSpec {
    pub name: Option<String>,
    pub version: Option<String>,
    pub services: IndexMap<String, ComposeService>,
    pub networks: Option<HashMap<String, ComposeNetwork>>,
    pub volumes: Option<HashMap<String, ComposeVolume>>,
    pub secrets: Option<HashMap<String, ComposeSecret>>,
    pub configs: Option<HashMap<String, ComposeConfig>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ComposeService {
    pub image: Option<String>,
    pub build: Option<ComposeServiceBuildOrString>,
    pub command: Option<CommandOrArgs>,
    pub entrypoint: Option<CommandOrArgs>,
    pub environment: Option<ListOrDict>,
    pub env_file: Option<StringOrList>,
    pub ports: Option<Vec<PortOrString>>,
    pub volumes: Option<Vec<VolumeOrString>>,
    pub networks: Option<NetworksOrList>,
    pub depends_on: Option<DependsOnOrList>,
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
    pub ulimits: Option<HashMap<String, Ulimit>>,
    pub logging: Option<ComposeLogging>,
    pub deploy: Option<ComposeDeployment>,
    pub expose: Option<Vec<String>>,
    pub extra_hosts: Option<ListOrDict>,
    pub dns: Option<StringOrList>,
    pub dns_search: Option<StringOrList>,
    pub tmpfs: Option<StringOrList>,
    pub isolation_level: Option<IsolationLevel>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum ComposeServiceBuildOrString {
    String(String),
    Build(ComposeServiceBuild),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub struct ComposeServiceBuild {
    pub context: String,
    #[serde(alias = "dockerfile")]
    pub containerfile: Option<String>,
    pub dockerfile_inline: Option<String>,
    pub args: Option<ListOrDict>,
    pub labels: Option<ListOrDict>,
    pub target: Option<String>,
    pub network: Option<String>,
    pub platforms: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum CommandOrArgs {
    String(String),
    List(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum StringOrList {
    String(String),
    List(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum PortOrString {
    String(String),
    Number(u32),
    Port(ComposeServicePort),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ComposeServicePort {
    pub name: Option<String>,
    pub mode: Option<String>,
    pub host_ip: Option<String>,
    pub target: u32,
    pub published: Option<u32>,
    pub protocol: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum VolumeOrString {
    String(String),
    Volume(ComposeServiceVolume),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ComposeServiceVolume {
    #[serde(rename = "type")]
    pub volume_type: VolumeType,
    pub source: Option<String>,
    pub target: Option<String>,
    pub read_only: Option<bool>,
    pub bind: Option<ComposeVolumeBind>,
    pub volume: Option<ComposeVolumeOpts>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VolumeType {
    Bind,
    Volume,
    Tmpfs,
    Cluster,
    Npipe,
    Image,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ComposeVolumeBind {
    pub propagation: Option<String>,
    pub create_host_path: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ComposeVolumeOpts {
    pub nocopy: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum NetworksOrList {
    List(Vec<String>),
    Dict(HashMap<String, Option<ComposeServiceNetworkConfig>>),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ComposeServiceNetworkConfig {
    pub aliases: Option<Vec<String>>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum DependsOnOrList {
    List(Vec<String>),
    Dict(HashMap<String, ComposeDependsOn>),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ComposeDependsOn {
    pub condition: Option<DependsOnCondition>,
    pub required: Option<bool>,
    pub restart: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependsOnCondition {
    ServiceStarted,
    ServiceHealthy,
    ServiceCompletedSuccessfully,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeHealthcheck {
    pub test: CommandOrArgs,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<u32>,
    pub start_period: Option<String>,
    pub disable: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeLogging {
    pub driver: String,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeDeployment {
    pub replicas: Option<u32>,
    pub resources: Option<ComposeResources>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeResources {
    pub limits: Option<ResourceLimit>,
    pub reservations: Option<ResourceLimit>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResourceLimit {
    pub cpus: Option<String>,
    pub memory: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Ulimit {
    Single(u32),
    SoftHard { soft: u32, hard: u32 },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeNetwork {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeVolume {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub external: Option<bool>,
    pub labels: Option<ListOrDict>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeSecret {
    pub file: Option<String>,
    pub external: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComposeConfig {
    pub file: Option<String>,
    pub external: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceGraph {
    pub nodes: Vec<String>,
    pub edges: Vec<ServiceEdge>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StackStatus {
    pub services: Vec<ServiceStatus>,
    pub healthy: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceStatus {
    pub service: String,
    pub state: ServiceState,
    pub container_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceState {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}
