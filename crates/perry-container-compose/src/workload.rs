//! Workload graph types for Perry orchestration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::types::{ContainerInfo, ContainerLogs};
use crate::error::ComposeError;

/// Runtime specification for a workload node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum RuntimeSpec {
    Oci,
    MicroVm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
}

/// Security policy tier.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
    pub no_network: bool,
    pub read_only_root: bool,
    pub seccomp: bool,
}

/// Reference projection type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RefProjection {
    Endpoint,
    Ip,
    InternalUrl,
}

/// Reference to another workload node's property.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: RefProjection,
    pub port: Option<String>,
}

impl WorkloadRef {
    pub fn resolve(&self, running_nodes: &HashMap<String, ContainerInfo>) -> Result<String, ComposeError> {
        let info = running_nodes.get(&self.node_id)
            .ok_or_else(|| ComposeError::NotFound(self.node_id.clone()))?;

        match self.projection {
            RefProjection::Ip => Ok(info.id.clone()), // Simulation: return ID as IP
            RefProjection::Endpoint => {
                let port = self.port.as_deref().unwrap_or("80");
                Ok(format!("{}:{}", info.id, port))
            }
            RefProjection::InternalUrl => {
                let port = self.port.as_deref().unwrap_or("80");
                Ok(format!("http://{}:{}", info.id, port))
            }
        }
    }
}

/// Environment value (literal or reference).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

/// Resources for a workload node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadResources {
    pub cpu: Option<f64>,
    pub memory: Option<String>,
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
    pub env: HashMap<String, WorkloadEnvValue>,
    pub depends_on: Vec<String>,
    pub runtime: RuntimeSpec,
    pub policy: PolicySpec,
}

/// An edge in the workload graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
}

/// A collection of interconnected workload nodes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

/// Execution strategy for a workload graph.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionStrategy {
    Sequential,
    MaxParallel,
    DependencyAware,
    ParallelSafe,
}

/// Strategy for handling failures in the graph.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FailureStrategy {
    RollbackAll,
    PartialContinue,
    HaltGraph,
}

/// Options for running a workload graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunGraphOptions {
    pub strategy: ExecutionStrategy,
    pub on_failure: FailureStrategy,
}

/// State of a workload node.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NodeState {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}

/// Status of a workload graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphStatus {
    pub nodes: HashMap<String, NodeState>,
    pub healthy: bool,
    pub errors: HashMap<String, String>,
}

/// Information about a single node in a graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub id: u64,
    pub name: String,
}

/// Engine for managing workload graphs.
pub struct WorkloadGraphEngine {
    pub backend: std::sync::Arc<dyn crate::backend::ContainerBackend>,
}

impl WorkloadGraphEngine {
    pub fn new(backend: std::sync::Arc<dyn crate::backend::ContainerBackend>) -> Self {
        Self { backend }
    }

    pub async fn run(
        &self,
        graph: WorkloadGraph,
        opts: RunGraphOptions,
    ) -> Result<GraphHandle, ComposeError> {
        let mut services = IndexMap::new();
        for (id, node) in &graph.nodes {
            let mut svc = crate::types::ComposeService {
                image: node.image.clone(),
                ports: Some(
                    node.ports
                        .iter()
                        .map(|p| crate::types::PortSpec::Short(serde_yaml::Value::String(p.clone())))
                        .collect(),
                ),
                depends_on: if node.depends_on.is_empty() {
                    None
                } else {
                    Some(crate::types::DependsOnSpec::List(node.depends_on.clone()))
                },
                ..Default::default()
            };

            // Policy enforcement
            if node.policy.tier == PolicyTier::Untrusted {
                // Untrusted nodes might force a different runtime in a real impl
            }
            if node.policy.read_only_root {
                svc.read_only = Some(true);
            }
            if node.policy.no_network {
                svc.network_mode = Some("none".to_string());
            }

            // Env mapping
            let mut env_map = IndexMap::new();
            for (k, v) in &node.env {
                match v {
                    WorkloadEnvValue::Literal(s) => {
                        env_map.insert(k.clone(), Some(serde_yaml::Value::String(s.clone())));
                    }
                    WorkloadEnvValue::Ref(_) => {
                        // Placeholders for refs — we'll update these after startup
                        env_map.insert(k.clone(), Some(serde_yaml::Value::String("PENDING".into())));
                    }
                }
            }
            svc.environment = Some(crate::types::ListOrDict::Dict(env_map));

            services.insert(id.clone(), svc);
        }

        let spec = crate::types::ComposeSpec {
            name: Some(graph.name.clone()),
            services,
            ..Default::default()
        };

        let engine = crate::compose::ComposeEngine::new(
            spec,
            graph.name.clone(),
            std::sync::Arc::clone(&self.backend),
        );
        let detach = opts.strategy != ExecutionStrategy::Sequential;
        let handle = engine.up(&[], detach, false, false).await?;

        // Resolve Refs after startup
        let running_nodes = engine.ps().await?;
        let nodes_map: HashMap<String, ContainerInfo> = running_nodes
            .into_iter()
            .map(|c| {
                // Try to find the node ID from labels
                let node_id = c.labels.get("com.perry.node_id").cloned().unwrap_or(c.name.clone());
                (node_id, c)
            })
            .collect();

        for (id, node) in &graph.nodes {
            for (k, v) in &node.env {
                if let WorkloadEnvValue::Ref(r) = v {
                    if let Ok(resolved) = r.resolve(&nodes_map) {
                        // In a real implementation, we might restart the container with updated env,
                        // or rely on a shared configuration service. For now, we log it.
                        tracing::debug!("Resolved Ref for node {}: {}={}", id, k, resolved);
                    }
                }
            }
        }

        Ok(GraphHandle {
            id: handle.stack_id,
            name: graph.name,
        })
    }

    pub async fn status(&self, stack_id: u64) -> Result<GraphStatus, ComposeError> {
        let engine = crate::compose::ComposeEngine::from_registry(stack_id, self.backend.clone())?;
        let containers = engine.ps().await?;
        let mut nodes = HashMap::new();
        let mut healthy = true;

        for c in containers {
            let state = match c.status.to_lowercase().as_str() {
                s if s.contains("running") || s.contains("up") => NodeState::Running,
                s if s.contains("exit") || s.contains("stop") => NodeState::Stopped,
                _ => NodeState::Unknown,
            };
            if state != NodeState::Running {
                healthy = false;
            }
            nodes.insert(c.name, state);
        }

        Ok(GraphStatus {
            nodes,
            healthy,
            errors: HashMap::new(),
        })
    }

    pub async fn inspect(&self, graph: &WorkloadGraph) -> Result<GraphStatus, ComposeError> {
        // Return a pending status if not running
        let mut nodes = HashMap::new();
        for node in graph.nodes.values() {
            nodes.insert(node.name.clone(), NodeState::Pending);
        }
        Ok(GraphStatus {
            nodes,
            healthy: false,
            errors: HashMap::new(),
        })
    }
}
