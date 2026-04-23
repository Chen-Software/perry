use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use crate::types::{ContainerLogs, ContainerInfo, IsolationLevel, ComposeSpec, ComposeService, ListOrDict, PortSpec};
use crate::backend::ContainerBackend;
use crate::error::Result;
use crate::compose::ComposeEngine;
use indexmap::IndexMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum RuntimeSpec {
    #[serde(rename = "oci")]
    Oci,
    #[serde(rename = "microvm")]
    MicroVm { config: Option<HashMap<String, String>> },
    #[serde(rename = "wasm")]
    Wasm { module: Option<String> },
    #[serde(rename = "auto")]
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PolicySpec {
    pub tier: String, // "default" | "isolated" | "hardened" | "untrusted"
    pub no_network: Option<bool>,
    pub read_only_root: Option<bool>,
    pub seccomp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: RefProjection,
    pub port: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RefProjection {
    Endpoint,
    Ip,
    InternalUrl,
}

impl WorkloadRef {
    pub fn resolve(&self, running_nodes: &HashMap<String, ContainerInfo>) -> std::result::Result<String, String> {
        let _info = running_nodes.get(&self.node_id)
            .ok_or_else(|| format!("Node '{}' not found in running set", self.node_id))?;

        match self.projection {
            RefProjection::Endpoint => {
                let port = self.port.as_deref().unwrap_or("80");
                Ok(format!("127.0.0.1:{}", port))
            }
            RefProjection::Ip => {
                Ok("172.17.0.2".to_string())
            }
            RefProjection::InternalUrl => {
                Ok(format!("http://172.17.0.2:{}", self.port.as_deref().unwrap_or("80")))
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
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: HashMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunGraphOptions {
    pub strategy: Option<String>,
    pub on_failure: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphStatus {
    pub nodes: HashMap<String, String>,
    pub healthy: bool,
    pub errors: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    pub node_id: String,
    pub name: String,
    pub container_id: Option<String>,
    pub state: String,
    pub image: Option<String>,
}

pub struct WorkloadGraphEngine {
    pub engine: ComposeEngine,
}

impl WorkloadGraphEngine {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Self {
        Self {
            engine: ComposeEngine::new(spec, backend),
        }
    }

    pub async fn run(&self, graph: WorkloadGraph, _opts: RunGraphOptions) -> Result<u64> {
        let mut spec = ComposeSpec::default();
        spec.name = Some(graph.name.clone());

        for (id, node) in &graph.nodes {
            let mut svc = ComposeService::default();
            svc.image = node.image.clone();
            svc.ports = Some(node.ports.iter().map(|p| PortSpec::Short(serde_yaml::Value::String(p.clone()))).collect());
            svc.depends_on = Some(crate::types::DependsOnSpec::List(node.depends_on.clone()));

            // Map policy to OCI settings
            match node.policy.tier.as_str() {
                "hardened" => {
                    svc.read_only = Some(true);
                    svc.security_opt = Some(vec!["no-new-privileges".to_string()]);
                    svc.cap_drop = Some(vec!["ALL".to_string()]);
                }
                "isolated" => {
                    svc.network_mode = Some("none".to_string());
                }
                "untrusted" => {
                    // Would use a different runtime if configured
                }
                _ => {}
            }

            if let Some(ro) = node.policy.read_only_root {
                svc.read_only = Some(ro);
            }

            let mut env = IndexMap::new();
            for (k, v) in &node.env {
                let val = match v {
                    WorkloadEnvValue::Literal(s) => Some(serde_yaml::Value::String(s.clone())),
                    WorkloadEnvValue::Ref(r) => {
                        let proj_str = match r.projection {
                            RefProjection::Endpoint => "endpoint",
                            RefProjection::Ip => "ip",
                            RefProjection::InternalUrl => "url",
                        };
                        Some(serde_yaml::Value::String(format!("REF:{}:{}:{}", r.node_id, proj_str, r.port.as_deref().unwrap_or(""))))
                    }
                };
                env.insert(k.clone(), val);
            }
            svc.environment = Some(ListOrDict::Dict(env));
            spec.services.insert(id.clone(), svc);
        }

        let engine = ComposeEngine::new(spec, Arc::clone(&self.engine.backend));
        let _handle = engine.up().await?;
        Ok(rand::random())
    }
}
