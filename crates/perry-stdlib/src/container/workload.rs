use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::types::{ContainerLogs, ContainerInfo, IsolationLevel};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum RuntimeSpec {
    #[serde(rename = "oci")]
    Oci,
    #[serde(rename = "microvm")]
    MicroVm { config: Option<HashMap<String, String>> },
    #[serde(rename = "wasm")]
    Wasm { module: Option<String> },
    #[serde(rename = "auto")]
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolicySpec {
    pub tier: String, // "default" | "isolated" | "hardened" | "untrusted"
    pub no_network: Option<bool>,
    pub read_only_root: Option<bool>,
    pub seccomp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: String, // "endpoint" | "ip" | "internalUrl"
    pub port: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadNode {
    pub id: String,
    pub name: String,
    pub image: Option<String>,
    pub resources: Option<HashMap<String, String>>,
    pub ports: Vec<String>,
    pub env: HashMap<String, WorkloadEnvValue>,
    pub depends_on: Vec<String>,
    pub runtime: RuntimeSpec,
    pub policy: PolicySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: HashMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunGraphOptions {
    pub strategy: Option<String>, // "sequential" | "max-parallel" | "dependency-aware" | "parallel-safe"
    pub on_failure: Option<String>, // "rollback-all" | "partial-continue" | "halt-graph"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphStatus {
    pub nodes: HashMap<String, String>, // node_id -> NodeState
    pub healthy: bool,
    pub errors: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    pub node_id: String,
    pub name: String,
    pub container_id: Option<String>,
    pub state: String,
    pub image: Option<String>,
}
