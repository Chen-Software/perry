//! Workload graph execution engine.

use crate::backend::ContainerBackend;
use crate::compose::ComposeEngine;
use crate::error::Result;
use crate::types::{
    WorkloadGraph, StackStatus, ComposeHandle, ListOrDict,
    DependsOnSpec, PortSpec, ComposeService, ComposeSpec, WorkloadEnvValue,
    RefProjection,
};
use std::sync::Arc;
use indexmap::IndexMap;

pub struct WorkloadGraphEngine {
    pub backend: Arc<dyn ContainerBackend>,
}

impl WorkloadGraphEngine {
    pub fn new(backend: Arc<dyn ContainerBackend>) -> Self {
        Self { backend }
    }

    pub async fn run(&self, graph_json: &str, _opts_json: &str) -> Result<ComposeHandle> {
        let graph: WorkloadGraph = serde_json::from_str(graph_json).map_err(crate::error::ComposeError::JsonError)?;
        let spec = graph.to_compose_spec();
        let engine = Arc::new(ComposeEngine::new(spec, graph.name.clone(), Arc::clone(&self.backend)));

        // Handle options if necessary. For now, use ComposeEngine's default up logic.
        engine.up(&[], true, false, false).await
    }

    pub async fn status(&self, graph_json: &str) -> Result<StackStatus> {
        let graph: WorkloadGraph = serde_json::from_str(graph_json).map_err(crate::error::ComposeError::JsonError)?;
        let spec = graph.to_compose_spec();
        let engine = ComposeEngine::new(spec, graph.name.clone(), Arc::clone(&self.backend));
        engine.status().await
    }
}

impl WorkloadGraph {
    pub fn to_compose_spec(&self) -> ComposeSpec {
        let mut services = IndexMap::new();
        for (id, node) in &self.nodes {
            let mut env = IndexMap::new();
            for (k, v) in &node.env {
                let val = match v {
                    WorkloadEnvValue::Literal(s) => serde_yaml::Value::String(s.clone()),
                    WorkloadEnvValue::Ref(r) => {
                        let resolved = match r.projection {
                            RefProjection::Endpoint => {
                                format!("{}:{}", r.node_id, r.port.as_deref().unwrap_or("80"))
                            }
                            RefProjection::Ip => r.node_id.clone(),
                            RefProjection::InternalUrl => {
                                format!("http://{}:{}", r.node_id, r.port.as_deref().unwrap_or("80"))
                            }
                        };
                        serde_yaml::Value::String(resolved)
                    }
                };
                env.insert(k.clone(), Some(val));
            }

            let svc = ComposeService {
                image: node.image.clone(),
                ports: Some(node.ports.iter().map(|p| PortSpec::Short(serde_yaml::Value::String(p.clone()))).collect()),
                environment: Some(ListOrDict::Dict(env)),
                depends_on: if node.depends_on.is_empty() {
                    None
                } else {
                    Some(DependsOnSpec::List(node.depends_on.clone()))
                },
                read_only: Some(node.policy.read_only_root),
                privileged: Some(node.policy.tier == crate::types::PolicyTier::Untrusted),
                cap_drop: if node.policy.seccomp { Some(vec!["ALL".to_string()]) } else { None },
                network_mode: if node.policy.no_network { Some("none".to_string()) } else { None },
                isolation: match &node.runtime {
                    crate::types::RuntimeSpec::Microvm { .. } => Some("hyperv".to_string()),
                    crate::types::RuntimeSpec::Wasm { .. } => Some("wasm".to_string()),
                    _ => None,
                },
                ..Default::default()
            };
            services.insert(id.clone(), svc);
        }

        ComposeSpec {
            name: Some(self.name.clone()),
            services,
            ..Default::default()
        }
    }
}
