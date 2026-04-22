//! Workload graph types and implementation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::container::types::{ContainerInfo, ContainerLogs};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RuntimeSpec {
    Oci,
    MicroVm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: std::collections::HashMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
    pub condition: Option<EdgeCondition>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EdgeCondition {
    Started,
    Healthy,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunGraphOptions {
    pub strategy: Option<ExecutionStrategy>,
    pub on_failure: Option<FailureStrategy>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStrategy {
    Sequential,
    MaxParallel,
    DependencyAware,
    ParallelSafe,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FailureStrategy {
    RollbackAll,
    PartialContinue,
    HaltGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatus {
    pub nodes: HashMap<String, NodeState>,
    pub healthy: bool,
    pub errors: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeState {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub name: String,
    pub container_id: Option<String>,
    pub state: NodeState,
    pub image: Option<String>,
}

pub struct GraphHandle {
    pub id: u64,
}

impl WorkloadRef {
    pub fn resolve(&self, running_nodes: &HashMap<String, ContainerInfo>) -> Result<String, crate::container::types::ContainerError> {
        let container = running_nodes.get(&self.node_id).ok_or_else(|| {
            crate::container::types::ContainerError::WorkloadRefResolutionFailed {
                node_id: self.node_id.clone(),
                projection: format!("{:?}", self.projection),
                reason: "Node not found in running set".into(),
            }
        })?;

        match self.projection {
            RefProjection::Endpoint => {
                let port = self.port.as_deref().ok_or_else(|| {
                    crate::container::types::ContainerError::WorkloadRefResolutionFailed {
                        node_id: self.node_id.clone(),
                        projection: "endpoint".into(),
                        reason: "Port not specified for endpoint projection".into(),
                    }
                })?;

                // container.ports format is "host_ip:host_port:container_port"
                for p_mapping in &container.ports {
                    let parts: Vec<&str> = p_mapping.split(':').collect();
                    if parts.len() == 3 && parts[2] == port {
                        return Ok(format!("{}:{}", parts[0], parts[1]));
                    }
                }

                Err(crate::container::types::ContainerError::WorkloadRefResolutionFailed {
                    node_id: self.node_id.clone(),
                    projection: "endpoint".into(),
                    reason: format!("Port {} not mapped for node", port),
                })
            }
            RefProjection::Ip => {
                container.ip.clone().ok_or_else(|| {
                    crate::container::types::ContainerError::WorkloadRefResolutionFailed {
                        node_id: self.node_id.clone(),
                        projection: "ip".into(),
                        reason: "IP address not available for node".into(),
                    }
                })
            }
            RefProjection::InternalUrl => {
                let ip = container.ip.as_deref().ok_or_else(|| {
                    crate::container::types::ContainerError::WorkloadRefResolutionFailed {
                        node_id: self.node_id.clone(),
                        projection: "internalUrl".into(),
                        reason: "IP address not available for node".into(),
                    }
                })?;

                let port = if let Some(p) = &self.port {
                    p.clone()
                } else if let Some(p_mapping) = container.ports.first() {
                    let parts: Vec<&str> = p_mapping.split(':').collect();
                    if parts.len() == 3 {
                        parts[2].to_string()
                    } else {
                        "80".to_string()
                    }
                } else {
                    "80".to_string()
                };

                Ok(format!("http://{}:{}", ip, port))
            }
        }
    }
}
