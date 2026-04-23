use serde::{Deserialize, Serialize};
use std::collections::{HashMap, BTreeSet};
use indexmap::IndexMap;
use crate::types::{ContainerLogs, ContainerSpec, ContainerHandle};
use crate::error::{ComposeError, Result};
use crate::backend::ContainerBackend;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use dashmap::DashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RuntimeSpec {
    Oci,
    MicroVm { config: Option<serde_json::Value> },
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyTier {
    Default,
    Isolated,
    Hardened,
    Untrusted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: RefProjection,
    pub port: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RefProjection {
    Endpoint,
    Ip,
    InternalUrl,
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
    pub resources: Option<WorkloadResources>,
    pub ports: Vec<String>,
    pub env: HashMap<String, WorkloadEnvValue>,
    pub depends_on: Vec<String>,
    pub runtime: RuntimeSpec,
    pub policy: PolicySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadResources {
    pub cpu: Option<String>,
    pub memory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGraphOptions {
    pub strategy: Option<ExecutionStrategy>,
    pub on_failure: Option<FailureStrategy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStrategy {
    Sequential,
    MaxParallel,
    DependencyAware,
    ParallelSafe,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NodeState {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    pub node_id: String,
    pub name: String,
    pub container_id: Option<String>,
    pub state: NodeState,
    pub image: Option<String>,
}

pub struct GraphHandle {
    pub stack_id: u64,
    pub name: String,
    pub started_nodes: HashMap<String, ContainerHandle>,
}

#[derive(Clone)]
pub struct WorkloadGraphEngine {
    pub graph: WorkloadGraph,
    pub backend: Arc<dyn ContainerBackend>,
    pub started_nodes: Arc<DashMap<String, ContainerHandle>>,
}

impl WorkloadGraphEngine {
    pub fn new(graph: WorkloadGraph, backend: Arc<dyn ContainerBackend>) -> Self {
        Self {
            graph,
            backend,
            started_nodes: Arc::new(DashMap::new()),
        }
    }

    pub async fn run(&self, opts: RunGraphOptions) -> Result<GraphHandle> {
        let strategy = opts.strategy.unwrap_or(ExecutionStrategy::DependencyAware);
        let on_failure = opts.on_failure.unwrap_or(FailureStrategy::RollbackAll);

        let levels = self.compute_topological_levels()?;

        for level in levels {
            let mut level_futures = Vec::new();

            // For each level, we resolve all refs before starting ANY node in the level.
            // Since we use topological levels, all dependencies of nodes in this level
            // MUST already be running from previous levels.

            for node_id in &level {
                let node = self.graph.nodes.get(node_id).ok_or_else(|| ComposeError::NotFound(node_id.clone()))?;
                let resolved_env = self.resolve_env_for_node(node).await?;

                if strategy == ExecutionStrategy::Sequential {
                    // Start and wait for each
                    let res = self.start_node(node, resolved_env).await;
                    match res {
                        Ok(handle) => { self.started_nodes.insert(node_id.clone(), handle); }
                        Err(e) => return self.handle_failure(on_failure, e).await,
                    }
                } else {
                    level_futures.push(async move {
                        let res = self.start_node(node, resolved_env).await;
                        (node_id.clone(), res)
                    });
                }
            }

            if strategy != ExecutionStrategy::Sequential {
                let results = futures::future::join_all(level_futures).await;
                for (node_id, res) in results {
                    match res {
                        Ok(handle) => { self.started_nodes.insert(node_id, handle); }
                        Err(e) => return self.handle_failure(on_failure, e).await,
                    }
                }
            }

            if strategy == ExecutionStrategy::ParallelSafe {
                sleep(Duration::from_millis(500)).await;
            }
        }

        Ok(GraphHandle {
            stack_id: rand::random(),
            name: self.graph.name.clone(),
            started_nodes: self.started_nodes.iter().map(|r| (r.key().clone(), r.value().clone())).collect(),
        })
    }

    async fn resolve_env_for_node(&self, node: &WorkloadNode) -> Result<HashMap<String, String>> {
        let mut resolved_env = HashMap::new();
        for (key, val) in &node.env {
            match val {
                WorkloadEnvValue::Literal(s) => {
                    resolved_env.insert(key.clone(), s.clone());
                }
                WorkloadEnvValue::Ref(wref) => {
                    let target_handle = self.started_nodes.get(&wref.node_id)
                        .ok_or_else(|| ComposeError::validation(format!("Referenced node {} not started yet", wref.node_id)))?;

                    let target_info = self.backend.inspect(&target_handle.id).await?;

                    let resolved_val = match wref.projection {
                        RefProjection::Ip => target_info.id.clone(), // Use ID as surrogate for IP
                        RefProjection::Endpoint => {
                            let port = wref.port.as_deref().unwrap_or("80");
                            format!("{}:{}", target_info.id, port)
                        }
                        RefProjection::InternalUrl => {
                            format!("http://{}", target_info.id)
                        }
                    };
                    resolved_env.insert(key.clone(), resolved_val);
                }
            }
        }
        Ok(resolved_env)
    }

    async fn start_node(&self, node: &WorkloadNode, env: HashMap<String, String>) -> Result<ContainerHandle> {
        let mut spec = ContainerSpec {
            image: node.image.clone().unwrap_or_else(|| "alpine:latest".into()),
            name: Some(node.name.clone()),
            ports: Some(node.ports.clone()),
            env: Some(env),
            rm: Some(false),
            ..Default::default()
        };

        // Apply policy
        match node.policy.tier {
            PolicyTier::Isolated => {
                spec.network = Some("none".into());
            }
            PolicyTier::Hardened => {
                spec.read_only = Some(true);
            }
            PolicyTier::Untrusted => {
                spec.read_only = Some(true);
                spec.network = Some("none".into());
            }
            _ => {}
        }

        if let Some(ro) = node.policy.read_only_root { spec.read_only = Some(ro); }
        if let Some(true) = node.policy.no_network { spec.network = Some("none".into()); }

        self.backend.run(&spec).await
    }

    async fn handle_failure(&self, strategy: FailureStrategy, err: ComposeError) -> Result<GraphHandle> {
        match strategy {
            FailureStrategy::RollbackAll => {
                for entry in self.started_nodes.iter() {
                    let handle = entry.value();
                    let _ = self.backend.stop(&handle.id, Some(5)).await;
                    let _ = self.backend.remove(&handle.id, true).await;
                }
                Err(err)
            }
            FailureStrategy::HaltGraph | FailureStrategy::PartialContinue => {
                // Return what we have
                Ok(GraphHandle {
                    stack_id: rand::random(),
                    name: self.graph.name.clone(),
                    started_nodes: self.started_nodes.iter().map(|r| (r.key().clone(), r.value().clone())).collect(),
                })
            }
        }
    }

    fn compute_topological_levels(&self) -> Result<Vec<Vec<String>>> {
        let mut in_degree = HashMap::new();
        let mut adj = HashMap::new();

        for node_id in self.graph.nodes.keys() {
            in_degree.insert(node_id.clone(), 0);
            adj.insert(node_id.clone(), Vec::new());
        }

        // 1. Build adjacency list from both depends_on and explicit edges.
        // Standard: edge from Prerequisite to Dependent.
        for (id, node) in &self.graph.nodes {
            for dep in &node.depends_on {
                if self.graph.nodes.contains_key(dep) {
                    *in_degree.get_mut(id).unwrap() += 1;
                    adj.get_mut(dep).unwrap().push(id.clone());
                }
            }
        }

        for edge in &self.graph.edges {
            if self.graph.nodes.contains_key(&edge.from) && self.graph.nodes.contains_key(&edge.to) {
                *in_degree.get_mut(&edge.to).unwrap() += 1;
                adj.get_mut(&edge.from).unwrap().push(edge.to.clone());
            }
        }

        let mut levels = Vec::new();
        let mut current_level: BTreeSet<String> = in_degree.iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        while !current_level.is_empty() {
            let level_vec: Vec<String> = current_level.iter().cloned().collect();
            levels.push(level_vec.clone());

            let mut next_level = BTreeSet::new();
            for node_id in level_vec {
                for dependent in &adj[&node_id] {
                    let deg = in_degree.get_mut(dependent).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        next_level.insert(dependent.clone());
                    }
                }
            }
            current_level = next_level;
        }

        let processed_count: usize = levels.iter().map(|l| l.len()).sum();
        if processed_count != self.graph.nodes.len() {
            return Err(ComposeError::DependencyCycle { services: vec![] });
        }

        Ok(levels)
    }


    pub async fn down(&self, started: &DashMap<String, ContainerHandle>) -> Result<()> {
        for entry in started.iter() {
            let handle = entry.value();
            let _ = self.backend.stop(&handle.id, Some(5)).await;
            let _ = self.backend.remove(&handle.id, true).await;
        }
        Ok(())
    }

    pub async fn ps(&self, started: &DashMap<String, ContainerHandle>) -> Result<Vec<NodeInfo>> {
        let mut results = Vec::new();
        for entry in started.iter() {
            let (node_id, handle) = entry.pair();
            let info = self.backend.inspect(&handle.id).await;
            match info {
                Ok(i) => {
                    results.push(NodeInfo {
                        node_id: node_id.clone(),
                        name: i.name,
                        container_id: Some(i.id),
                        state: if i.status == "running" { NodeState::Running } else { NodeState::Stopped },
                        image: Some(i.image),
                    });
                }
                Err(_) => {
                    results.push(NodeInfo {
                        node_id: node_id.clone(),
                        name: node_id.clone(),
                        container_id: None,
                        state: NodeState::Failed,
                        image: None,
                    });
                }
            }
        }
        Ok(results)
    }

    pub async fn status(&self, started: &DashMap<String, ContainerHandle>) -> Result<GraphStatus> {
        let ps = self.ps(started).await?;
        let mut nodes = HashMap::new();
        let mut healthy = true;
        for info in ps {
            nodes.insert(info.node_id, info.state.clone());
            if info.state != NodeState::Running {
                healthy = false;
            }
        }
        Ok(GraphStatus { nodes, healthy, errors: None })
    }

    pub async fn logs(&self, started: &DashMap<String, ContainerHandle>, node_id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let handle = started.get(node_id).ok_or_else(|| ComposeError::NotFound(node_id.into()))?;
        self.backend.logs(&handle.id, tail).await
    }

    pub async fn exec(&self, started: &DashMap<String, ContainerHandle>, node_id: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let handle = started.get(node_id).ok_or_else(|| ComposeError::NotFound(node_id.into()))?;
        self.backend.exec(&handle.id, cmd, None, None).await
    }

    pub async fn inspect(&self) -> Result<GraphStatus> {
        let mut nodes = HashMap::new();
        let mut healthy = true;

        // Batch list all containers to avoid N backend calls
        let all_containers = self.backend.list(true).await?;

        for (id, node) in &self.graph.nodes {
            let state = if let Some(c) = all_containers.iter().find(|c| c.name == node.name) {
                if c.status.contains("running") || c.status.contains("Up") {
                    NodeState::Running
                } else {
                    NodeState::Stopped
                }
            } else {
                NodeState::Pending
            };

            if state != NodeState::Running {
                healthy = false;
            }
            nodes.insert(id.clone(), state);
        }

        Ok(GraphStatus { nodes, healthy, errors: None })
    }
}
