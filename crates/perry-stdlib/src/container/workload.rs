//! Workload Graph types and resolution — Requirement 14.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::container::types::{ContainerInfo, ContainerError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeSpec {
    Oci,
    Microvm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicySpec {
    pub tier: PolicyTier,
    pub no_network: bool,
    pub read_only_root: bool,
    pub seccomp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RefProjection {
    Endpoint,
    Ip,
    InternalUrl,
}

impl WorkloadRef {
    pub fn resolve(&self, running_nodes: &HashMap<String, ContainerInfo>) -> Result<String, ContainerError> {
        let info = running_nodes.get(&self.node_id)
            .ok_or_else(|| ContainerError::NotFound(format!("Node {} not found in graph", self.node_id)))?;

        match self.projection {
            RefProjection::Ip => Ok(info.id.clone()), // Simplified for now
            RefProjection::Endpoint => {
                let port = self.port.as_deref().unwrap_or("80");
                Ok(format!("{}:{}", info.id, port))
            }
            RefProjection::InternalUrl => {
                Ok(format!("http://{}", info.id))
            }
        }
    }
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
}
