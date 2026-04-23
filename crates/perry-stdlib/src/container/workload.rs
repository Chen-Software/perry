use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::container::types::{ContainerInfo, ContainerLogs};
use perry_container_compose::error::ComposeError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuntimeSpec {
    Oci,
    Microvm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PolicyTier {
    Default,
    Isolated,
    Hardened,
    Untrusted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySpec {
    pub tier: PolicyTier,
    pub no_network: Option<bool>,
    pub read_only_root: Option<bool>,
    pub seccomp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
#[serde(rename_all = "camelCase")]
pub enum RefProjection {
    Endpoint,
    Ip,
    InternalUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: RefProjection,
    pub port: Option<String>,
}

impl WorkloadRef {
    pub fn resolve(&self, running_nodes: &HashMap<String, ContainerInfo>) -> Result<String, String> {
        let info = running_nodes.get(&self.node_id)
            .ok_or_else(|| format!("Node '{}' not found", self.node_id))?;

        match self.projection {
            RefProjection::Endpoint => {
                let port = self.port.as_deref().ok_or("Port required for Endpoint projection")?;
                // Simplified port lookup: find matching container port and return host mapping
                // In a real implementation we'd parse the ports strings
                Ok(format!("{}:{}", "127.0.0.1", port))
            }
            RefProjection::Ip => {
                Ok("127.0.0.1".into()) // Simplified
            }
            RefProjection::InternalUrl => {
                Ok(format!("http://{}:{}", "127.0.0.1", "80")) // Simplified
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGraphOptions {
    pub strategy: Option<String>, // "sequential" | "max-parallel" | "dependency-aware" | "parallel-safe"
    pub on_failure: Option<String>, // "rollback-all" | "partial-continue" | "halt-graph"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum NodeState {
    Running,
    Stopped,
    Failed,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStatus {
    pub nodes: HashMap<String, NodeState>,
    pub healthy: bool,
    pub errors: Option<HashMap<String, String>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphHandle {
    pub handle_id: u64,
}

pub struct WorkloadGraphState {
    pub graph: WorkloadGraph,
    pub nodes: DashMap<String, NodeInfo>,
    pub backend: std::sync::Arc<dyn perry_container_compose::backend::ContainerBackend>,
}

use dashmap::DashMap;

impl WorkloadGraphState {
    pub fn new(graph: WorkloadGraph, backend: std::sync::Arc<dyn perry_container_compose::backend::ContainerBackend>) -> Self {
        Self {
            graph,
            nodes: DashMap::new(),
            backend,
        }
    }

    pub async fn run(&self) -> Result<(), ComposeError> {
        let mut services = IndexMap::new();
        for (id, node) in &self.graph.nodes {
            let mut service = perry_container_compose::types::ComposeService {
                image: node.image.clone(),
                ports: Some(node.ports.iter().map(|p| perry_container_compose::types::PortSpec::Short(serde_yaml::Value::String(p.clone()))).collect()),
                depends_on: Some(perry_container_compose::types::DependsOnSpec::List(node.depends_on.clone())),
                ..Default::default()
            };

            let mut env = IndexMap::new();
            for (k, v) in &node.env {
                match v {
                    WorkloadEnvValue::Literal(s) => {
                        env.insert(k.clone(), Some(serde_yaml::Value::String(s.clone())));
                    }
                    WorkloadEnvValue::Ref(r) => {
                        // For now, we don't have full resolution here as it needs running state.
                        // In a real implementation we'd resolve this before starting the container
                        // or use backend-specific features.
                        env.insert(k.clone(), Some(serde_yaml::Value::String(format!("REF:{}:{}", r.node_id, r.projection as i32))));
                    }
                }
            }
            service.environment = Some(perry_container_compose::types::ListOrDict::Dict(env));

            services.insert(id.clone(), service);
        }

        let spec = perry_container_compose::types::ComposeSpec {
            name: Some(self.graph.name.clone()),
            services,
            ..Default::default()
        };

        let engine = perry_container_compose::ComposeEngine::new(spec, self.graph.name.clone(), Arc::clone(&self.backend));
        engine.clone().up(&[], true, false, false).await?;

        // Update local status
        let ps = engine.ps().await?;
        for info in ps {
            // Find which node this container belongs to.
            // ComposeEngine generates names like {service_name}-{md5_prefix_8}-{random_hex_8}
            for node_id in self.graph.nodes.keys() {
                if info.name.starts_with(node_id) {
                    self.nodes.insert(node_id.clone(), NodeInfo {
                        node_id: node_id.clone(),
                        name: info.name.clone(),
                        container_id: Some(info.id.clone()),
                        state: if info.status.contains("Up") { NodeState::Running } else { NodeState::Stopped },
                        image: Some(info.image.clone()),
                    });
                }
            }
        }

        Ok(())
    }

    pub async fn status(&self) -> GraphStatus {
        let mut nodes = HashMap::new();
        let mut healthy = true;

        for node_id in self.graph.nodes.keys() {
            let state = if let Some(info) = self.nodes.get(node_id) {
                if info.state != NodeState::Running { healthy = false; }
                info.state.clone()
            } else {
                healthy = false;
                NodeState::Pending
            };
            nodes.insert(node_id.clone(), state);
        }

        GraphStatus { nodes, healthy, errors: None }
    }

    pub fn ps(&self) -> Vec<NodeInfo> {
        self.nodes.iter().map(|r| r.value().clone()).collect()
    }

    pub async fn logs(&self, node_id: &str, tail: Option<u32>) -> Result<ContainerLogs, ComposeError> {
        let info = self.nodes.get(node_id).ok_or_else(|| ComposeError::NotFound(node_id.into()))?;
        let container_id = info.container_id.as_ref().ok_or_else(|| ComposeError::BackendError { code: 404, message: "Container not started".into() })?;
        self.backend.logs(container_id, tail).await
    }

    pub async fn exec(&self, node_id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs, ComposeError> {
        let info = self.nodes.get(node_id).ok_or_else(|| ComposeError::NotFound(node_id.into()))?;
        let container_id = info.container_id.as_ref().ok_or_else(|| ComposeError::BackendError { code: 404, message: "Container not started".into() })?;
        self.backend.exec(container_id, cmd, env, workdir).await
    }
}

use std::sync::Arc;
