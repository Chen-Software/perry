//! Workload graph types for the perry/workloads module.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use indexmap::IndexMap;
use super::types::{ContainerInfo, ContainerError};

/// Runtime specification for a workload node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RuntimeSpec {
    Oci,
    MicroVm {
        #[serde(default)]
        config: Option<serde_json::Value>
    },
    Wasm {
        #[serde(default)]
        module: Option<String>
    },
    Auto,
}

impl Default for RuntimeSpec {
    fn default() -> Self {
        RuntimeSpec::Auto
    }
}

/// Policy tier for isolation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyTier {
    Default,
    Isolated,
    Hardened,
    Untrusted,
}

/// Policy specification for a workload node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolicySpec {
    pub tier: PolicyTier,
    #[serde(default)]
    pub no_network: bool,
    #[serde(default)]
    pub read_only_root: bool,
    #[serde(default)]
    pub seccomp: bool,
}

impl Default for PolicySpec {
    fn default() -> Self {
        Self {
            tier: PolicyTier::Default,
            no_network: false,
            read_only_root: false,
            seccomp: false,
        }
    }
}

/// Projection type for a workload reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RefProjection {
    Endpoint,
    Ip,
    InternalUrl,
}

/// A reference to another workload node's property.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: RefProjection,
    #[serde(default)]
    pub port: Option<String>,
}

impl WorkloadRef {
    pub fn resolve(&self, running_nodes: &HashMap<String, ContainerInfo>) -> Result<String, ContainerError> {
        let info = running_nodes.get(&self.node_id)
            .ok_or_else(|| ContainerError::NotFound(format!("Node {} not found in running nodes", self.node_id)))?;

        match self.projection {
            RefProjection::Ip => {
                // In a real implementation we would extract the IP from ContainerInfo.
                // For now, we'll return the name as a placeholder for DNS resolution if IP is not easily available.
                Ok(info.name.clone())
            }
            RefProjection::Endpoint => {
                let port = self.port.as_ref().ok_or_else(|| ContainerError::InvalidConfig("Port required for endpoint projection".into()))?;
                // In a real implementation we would map the container port to host port if needed.
                Ok(format!("{}:{}", info.name, port))
            }
            RefProjection::InternalUrl => {
                Ok(format!("http://{}", info.name))
            }
        }
    }
}

/// Environment value that can be either a literal or a reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

/// A single node in a workload graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadNode {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub resources: Option<serde_json::Value>,
    #[serde(default)]
    pub ports: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, WorkloadEnvValue>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub runtime: RuntimeSpec,
    #[serde(default)]
    pub policy: PolicySpec,
}

/// A workload graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNode>,
    #[serde(default)]
    pub edges: Vec<WorkloadEdge>,
}

/// An edge in a workload graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
}

/// Execution strategy for running a graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStrategy {
    Sequential,
    MaxParallel,
    DependencyAware,
    ParallelSafe,
}

/// Strategy for handling failures during graph execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum FailureStrategy {
    RollbackAll,
    PartialContinue,
    HaltGraph,
}

/// Options for running a workload graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGraphOptions {
    #[serde(default = "default_strategy")]
    pub strategy: ExecutionStrategy,
    #[serde(default = "default_failure_strategy")]
    pub on_failure: FailureStrategy,
}

fn default_strategy() -> ExecutionStrategy { ExecutionStrategy::DependencyAware }
fn default_failure_strategy() -> FailureStrategy { FailureStrategy::RollbackAll }

/// State of a workload node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NodeState {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}

/// Status of a workload graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatus {
    pub nodes: HashMap<String, NodeState>,
    pub healthy: bool,
    #[serde(default)]
    pub errors: HashMap<String, String>,
}

/// Metadata about a workload node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    pub node_id: String,
    pub name: String,
    pub container_id: Option<String>,
    pub state: NodeState,
    pub image: Option<String>,
}
