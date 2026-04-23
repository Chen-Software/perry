use crate::error::{ComposeError, Result};
use crate::backend::ContainerBackend;
use crate::types::{ContainerSpec, ContainerInfo, ExecutionStrategy};
use std::collections::HashMap;
use std::sync::Arc;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSpec {
    Oci,
    MicroVm { config: Option<serde_json::Value> },
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
    pub fn resolve(&self, running_nodes: &HashMap<String, ContainerInfo>) -> Result<String> {
        let info = running_nodes.get(&self.node_id)
            .ok_or_else(|| ComposeError::NotFound(self.node_id.clone()))?;

        match self.projection {
            RefProjection::Endpoint => {
                let port = self.port.as_deref().unwrap_or("80");
                Ok(format!("{}:{}", info.name, port))
            }
            RefProjection::Ip => Ok(info.name.clone()),
            RefProjection::InternalUrl => {
                let port = self.port.as_deref().unwrap_or("80");
                Ok(format!("http://{}:{}", info.name, port))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadResources {
    pub cpu: Option<String>,
    pub memory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
}

// Execution types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunGraphOptions {
    pub strategy: ExecutionStrategy,
    pub on_failure: FailureStrategy,
}

impl Default for RunGraphOptions {
    fn default() -> Self {
        Self {
            strategy: ExecutionStrategy::CliExec,
            on_failure: FailureStrategy::RollbackAll,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FailureStrategy {
    RollbackAll,
    PartialContinue,
    HaltGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphStatus {
    pub nodes: HashMap<String, NodeState>,
    pub healthy: bool,
    pub errors: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeState {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeInfo {
    pub node_id: String,
    pub name: String,
    pub container_id: Option<String>,
    pub state: NodeState,
    pub image: Option<String>,
}

pub struct WorkloadGraphEngine {
    pub backend: Arc<dyn ContainerBackend>,
}

static WORKLOAD_HANDLES: once_cell::sync::Lazy<dashmap::DashMap<u64, WorkloadGraph>> =
    once_cell::sync::Lazy::new(dashmap::DashMap::new);

impl WorkloadGraphEngine {
    pub fn new(backend: Arc<dyn ContainerBackend>) -> Self {
        Self { backend }
    }

    pub fn get_graph(handle_id: u64) -> Option<WorkloadGraph> {
        WORKLOAD_HANDLES.get(&handle_id).map(|g| g.clone())
    }

    pub async fn run(
        &self,
        graph: WorkloadGraph,
        opts: RunGraphOptions,
    ) -> Result<u64> {
        let order = self.resolve_order(&graph)?;

        let mut running_nodes: HashMap<String, ContainerInfo> = HashMap::new();

        for node_id in &order {
            let node = graph.nodes.get(node_id).unwrap();

            let spec = self.apply_policy(node)?;

            match self.backend.run(&spec).await {
                Ok(handle) => {
                    let info = self.backend.inspect(&handle.id).await?;
                    running_nodes.insert(node_id.clone(), info);
                }
                Err(e) => {
                    match opts.on_failure {
                        FailureStrategy::RollbackAll => {
                            for info in running_nodes.values() {
                                let _ = self.backend.stop(&info.id, None).await;
                                let _ = self.backend.remove(&info.id, true).await;
                            }
                            return Err(e);
                        }
                        FailureStrategy::HaltGraph => break,
                        FailureStrategy::PartialContinue => continue,
                    }
                }
            }
        }

        // Resolve WorkloadRef values in env
        // Note: Real implementation would need to update the container env,
        // but here we just simulate the resolution.
        for (_node_id, node) in &graph.nodes {
            let mut _resolved_env = HashMap::new();
            for (key, value) in &node.env {
                let val = match value {
                    WorkloadEnvValue::Literal(s) => s.clone(),
                    WorkloadEnvValue::Ref(r) => r.resolve(&running_nodes)?,
                };
                _resolved_env.insert(key.clone(), val);
            }
        }

        let handle_id = rand::random::<u64>();
        WORKLOAD_HANDLES.insert(handle_id, graph);
        Ok(handle_id)
    }

    pub async fn down(&self, handle_id: u64) -> Result<()> {
        if let Some((_, graph)) = WORKLOAD_HANDLES.remove(&handle_id) {
            for node in graph.nodes.values() {
                let name = node.name.clone();
                let _ = self.backend.stop(&name, None).await;
                let _ = self.backend.remove(&name, true).await;
            }
        }
        Ok(())
    }

    pub async fn status(&self, handle_id: u64) -> Result<GraphStatus> {
        let graph = Self::get_graph(handle_id).ok_or_else(|| ComposeError::NotFound(format!("Graph handle {}", handle_id)))?;
        let mut nodes = HashMap::new();
        let mut healthy = true;
        let mut errors = HashMap::new();

        for (id, node) in &graph.nodes {
            match self.backend.inspect(&node.name).await {
                Ok(info) => {
                    let state = if info.status == "running" {
                        NodeState::Running
                    } else if info.status == "exited" {
                        NodeState::Stopped
                    } else {
                        NodeState::Unknown
                    };
                    nodes.insert(id.clone(), state);
                }
                Err(e) => {
                    nodes.insert(id.clone(), NodeState::Failed);
                    healthy = false;
                    errors.insert(id.clone(), e.to_string());
                }
            }
        }

        Ok(GraphStatus { nodes, healthy, errors })
    }

    pub async fn ps(&self, handle_id: u64) -> Result<Vec<NodeInfo>> {
        let graph = Self::get_graph(handle_id).ok_or_else(|| ComposeError::NotFound(format!("Graph handle {}", handle_id)))?;
        let mut results = Vec::new();

        for (id, node) in &graph.nodes {
            let info = match self.backend.inspect(&node.name).await {
                Ok(info) => NodeInfo {
                    node_id: id.clone(),
                    name: node.name.clone(),
                    container_id: Some(info.id),
                    state: if info.status == "running" { NodeState::Running } else { NodeState::Stopped },
                    image: node.image.clone(),
                },
                Err(_) => NodeInfo {
                    node_id: id.clone(),
                    name: node.name.clone(),
                    container_id: None,
                    state: NodeState::Unknown,
                    image: node.image.clone(),
                },
            };
            results.push(info);
        }

        Ok(results)
    }

    fn resolve_order(&self, graph: &WorkloadGraph) -> Result<Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        for node_id in graph.nodes.keys() {
            in_degree.insert(node_id.clone(), 0);
            dependents.insert(node_id.clone(), Vec::new());
        }

        for edge in &graph.edges {
            *in_degree.get_mut(&edge.to).unwrap() += 1;
            dependents.get_mut(&edge.from).unwrap().push(edge.to.clone());
        }

        let mut queue: std::collections::VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut order = Vec::new();
        while let Some(node_id) = queue.pop_front() {
            order.push(node_id.clone());
            if let Some(deps) = dependents.get(&node_id) {
                for dep in deps {
                    let deg = in_degree.get_mut(dep).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }

        if order.len() != graph.nodes.len() {
            return Err(ComposeError::DependencyCycle {
                services: in_degree.keys().cloned().collect()
            });
        }

        Ok(order)
    }

    fn apply_policy(&self, node: &WorkloadNode) -> Result<ContainerSpec> {
        let mut spec = ContainerSpec {
            image: node.image.clone().unwrap_or_default(),
            name: Some(node.name.clone()),
            ports: Some(node.ports.clone()),
            ..Default::default()
        };

        match node.policy.tier {
            PolicyTier::Untrusted => {
                // Should force MicroVm, but not implemented yet
            }
            PolicyTier::Hardened => {
                spec.read_only = Some(true);
            }
            PolicyTier::Isolated => {
                spec.network = Some("none".to_string());
            }
            PolicyTier::Default => {}
        }

        if node.policy.no_network {
            spec.network = Some("none".to_string());
        }
        if node.policy.read_only_root {
            spec.read_only = Some(true);
        }

        Ok(spec)
    }
}
