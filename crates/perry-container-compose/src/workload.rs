//! Workload graph types and engine.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use indexmap::IndexMap;
use crate::types::{ContainerInfo, ComposeSpec, ComposeService, PortSpec};
use crate::compose::ComposeEngine;
use crate::backend::ContainerBackend;
use crate::error::Result;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuntimeSpec {
    Oci,
    Microvm { config: Option<serde_json::Value> },
    Wasm { module: Option<String> },
    Auto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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
    #[serde(default)]
    pub no_network: bool,
    #[serde(default)]
    pub read_only_root: bool,
    #[serde(default)]
    pub seccomp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionStrategy {
    Sequential,
    MaxParallel,
    DependencyAware,
    ParallelSafe,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureStrategy {
    RollbackAll,
    PartialContinue,
    HaltGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunGraphOptions {
    pub strategy: ExecutionStrategy,
    pub on_failure: FailureStrategy,
}

pub struct WorkloadGraphEngine {
    pub backend: Arc<dyn ContainerBackend>,
}

impl WorkloadGraphEngine {
    pub fn new(backend: Arc<dyn ContainerBackend>) -> Self {
        Self { backend }
    }

    pub async fn run(&self, graph_json: &str, _opts_json: &str) -> Result<u64> {
        let graph: WorkloadGraph = serde_json::from_str(graph_json).map_err(crate::error::ComposeError::JsonError)?;
        let spec = self.convert_to_compose(&graph);
        let engine = ComposeEngine::new(spec, graph.name, self.backend.clone());
        let handle = Arc::new(engine).up(&[], true, false, false).await?;
        Ok(handle.stack_id)
    }

    fn convert_to_compose(&self, graph: &WorkloadGraph) -> ComposeSpec {
        let mut services = IndexMap::new();

        for (id, node) in &graph.nodes {
            let mut svc = ComposeService {
                image: node.image.clone(),
                ports: Some(node.ports.iter().map(|p| PortSpec::Short(serde_yaml::Value::String(p.clone()))).collect()),
                read_only: Some(node.policy.read_only_root),
                ..Default::default()
            };

            // Handle environment and references (simplified)
            let mut env = IndexMap::new();
            for (k, v) in &node.env {
                match v {
                    WorkloadEnvValue::Literal(s) => {
                        env.insert(k.clone(), Some(serde_yaml::Value::String(s.clone())));
                    }
                    WorkloadEnvValue::Ref(r) => {
                        // In a real implementation, we'd use service names for discovery
                        env.insert(k.clone(), Some(serde_yaml::Value::String(format!("http://{}:{}", r.node_id, r.port.as_deref().unwrap_or("80")))));
                    }
                }
            }
            if !env.is_empty() {
                svc.environment = Some(crate::types::ListOrDict::Dict(env));
            }

            if !node.depends_on.is_empty() {
                svc.depends_on = Some(crate::types::DependsOnSpec::List(node.depends_on.clone()));
            }

            services.insert(id.clone(), svc);
        }

        ComposeSpec {
            name: Some(graph.name.clone()),
            services,
            ..Default::default()
        }
    }
}
