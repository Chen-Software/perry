//! Workload Graph types for perry-container-compose.

use serde::{Deserialize, Serialize};

/// Runtime specification for a workload node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuntimeSpec {
    Oci,
    Microvm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
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

/// Policy tier defining isolation level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PolicyTier {
    Default,
    Isolated,
    Hardened,
    Untrusted,
}

/// Typed cross-node reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: RefProjection,
    pub port: Option<String>,
}

/// Projection type for WorkloadRef.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RefProjection {
    Endpoint,
    Ip,
    InternalUrl,
}

/// Condition that must be met for a dependency edge.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum EdgeCondition {
    Started,
    Healthy,
    Completed,
}

/// Execution strategy for the graph.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionStrategy {
    Sequential,
    MaxParallel,
    DependencyAware,
    ParallelSafe,
}

/// Strategy to handle node startup failures.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum FailureStrategy {
    RollbackAll,
    PartialContinue,
    HaltGraph,
}

/// State of a node in the graph.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum NodeState {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}

/// Environment variable value, either a literal or a reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

/// A node in the workload graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadNode {
    pub id: String,
    pub name: String,
    pub image: Option<String>,
    pub resources: Option<WorkloadResources>,
    pub ports: Vec<String>,
    pub env: std::collections::HashMap<String, WorkloadEnvValue>,
    pub depends_on: Vec<String>,
    pub runtime: RuntimeSpec,
    pub policy: PolicySpec,
}

/// Resource constraints for a workload node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadResources {
    pub cpu: Option<String>,
    pub memory: Option<String>,
}

/// A DAG of workload nodes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: indexmap::IndexMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

/// An edge between nodes in the workload graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
    pub condition: EdgeCondition,
}

/// Options for running a workload graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGraphOptions {
    pub strategy: Option<ExecutionStrategy>,
    pub on_failure: Option<FailureStrategy>,
}

/// Current status of the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatus {
    pub nodes: std::collections::HashMap<String, NodeState>,
    pub healthy: bool,
    pub errors: std::collections::HashMap<String, String>,
}

/// Information about a node in a running graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    pub node_id: String,
    pub name: String,
    pub container_id: Option<String>,
    pub state: NodeState,
    pub image: Option<String>,
}

/// Handle to a running workload graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphHandle {
    pub graph_id: u64,
    pub project_name: String,
    pub node_ids: Vec<String>,
}
