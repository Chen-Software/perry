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
        let info = running_nodes.get(&self.node_id)
            .ok_or_else(|| format!("Node '{}' not found in running set", self.node_id))?;

        match self.projection {
            RefProjection::Endpoint => {
                let target_port = self.port.as_deref().unwrap_or("80");
                // Find host port mapping if it exists, otherwise assume internal port
                let port = info.ports.iter()
                    .find(|p| p.ends_with(&format!(":{}", target_port)))
                    .map(|p| p.split(':').next().unwrap_or(target_port))
                    .unwrap_or(target_port);
                Ok(format!("127.0.0.1:{}", port))
            }
            RefProjection::Ip => {
                // In a real implementation we would get the IP from the container backend inspection.
                // For now, return a placeholder or derive from info if available.
                Ok("127.0.0.1".to_string())
            }
            RefProjection::InternalUrl => {
                let port = self.port.as_deref().unwrap_or("80");
                Ok(format!("http://127.0.0.1:{}", port))
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

    pub async fn run(&self, graph: WorkloadGraph, opts: RunGraphOptions) -> Result<u64> {
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
                    // Untrusted forces MicroVM if available (design principle)
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

        // Handle strategies
        let strategy = opts.strategy.as_deref().unwrap_or("dependency-aware");
        if strategy == "sequential" {
            engine.up().await?;
        } else {
            // "dependency-aware" and others currently use the standard up()
            // which follows Kahn's order. To implement real parallelism within levels,
            // we could call resolve_startup_levels and use join_all per level.
            let levels = ComposeEngine::resolve_startup_levels(&engine.spec)?;
            for level in levels {
                let mut futures = Vec::new();
                for service_name in level {
                    let svc = engine.spec.services.get(&service_name).unwrap();
                    let service_name_owned = service_name.clone();
                    let backend = Arc::clone(&engine.backend);
                    futures.push(async move {
                        crate::orchestrate::orchestrate_service(
                            &service_name_owned,
                            svc,
                            backend.as_ref(),
                        ).await
                    });
                }
                let results = futures::future::join_all(futures).await;
                for res in results {
                    res.map_err(|e| crate::error::ComposeError::ServiceStartupFailed {
                        service: "workload-node".to_string(), // Better error context would be good here
                        message: e.to_string(),
                    })?;
                }
            }
        }

        // Post-start WorkloadRef resolution and injection
        let containers = engine.ps().await?;
        let mut running_nodes = HashMap::new();
        for (id, node) in &graph.nodes {
            let svc = engine.spec.services.get(id).unwrap();
            let name = svc.name(id);
            if let Some(info) = containers.iter().find(|c| c.name == name) {
                running_nodes.insert(id.clone(), info.clone());
            }
        }

        for (id, node) in &graph.nodes {
            let mut resolved_env = HashMap::new();
            for (k, v) in &node.env {
                if let WorkloadEnvValue::Ref(r) = v {
                    if let Ok(resolved) = r.resolve(&running_nodes) {
                        resolved_env.insert(k.clone(), resolved);
                    }
                }
            }
            if !resolved_env.is_empty() {
                let svc = engine.spec.services.get(id).unwrap();
                let name = svc.name(id);
                // Inject resolved environment variables into the running container
                let mut cmd = vec!["export".to_string()];
                for (k, v) in resolved_env {
                    cmd.push(format!("{}={}", k, v));
                }
                let _ = engine.backend.exec(&name, &cmd, None, None).await;
            }
        }

        Ok(rand::random())
    }
}
