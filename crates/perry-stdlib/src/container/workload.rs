use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::container::types::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RuntimeSpec {
    Oci,
    Microvm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolicySpec {
    pub tier: PolicyTier,
    pub no_network: Option<bool>,
    pub read_only_root: Option<bool>,
    pub seccomp: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyTier {
    Default,
    Isolated,
    Hardened,
    Untrusted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: RefProjection,
    pub port: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RefProjection {
    Endpoint,
    Ip,
    InternalUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadNode {
    pub id: String,
    pub name: String,
    pub image: Option<String>,
    pub resources: Option<serde_json::Value>,
    pub ports: Vec<String>,
    pub env: HashMap<String, WorkloadEnvValue>,
    pub depends_on: Vec<String>,
    pub runtime: RuntimeSpec,
    pub policy: PolicySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNode>,
    pub edges: Vec<perry_container_compose::types::WorkloadEdge>,
}

pub struct WorkloadGraphEngine {
    pub graph: WorkloadGraph,
    pub engine: perry_container_compose::ComposeEngine,
}

impl WorkloadGraphEngine {
    pub fn new(graph: WorkloadGraph, engine: perry_container_compose::ComposeEngine) -> Self {
        Self { graph, engine }
    }
}
