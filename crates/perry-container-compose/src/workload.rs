use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use indexmap::IndexMap;
use crate::backend::ContainerBackend;
use crate::error::ComposeError;
use crate::compose::ComposeEngine;
use crate::types::{ComposeSpec, ComposeService};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNode>,
    pub edges: Vec<WorkloadEdge>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkloadNode {
    pub id: String,
    pub name: String,
    pub image: Option<String>,
    pub ports: Vec<String>,
    pub env: HashMap<String, WorkloadEnvValue>,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum WorkloadEnvValue {
    Literal(String),
    Ref(WorkloadRef),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkloadRef {
    pub node_id: String,
    pub projection: String, // "endpoint" | "ip" | "internalUrl"
    pub port: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkloadEdge {
    pub from: String,
    pub to: String,
}

pub struct WorkloadGraphEngine {
    pub engine: ComposeEngine,
}

impl WorkloadGraphEngine {
    pub fn new(backend: std::sync::Arc<dyn ContainerBackend>) -> Self {
        Self { engine: ComposeEngine::new(backend) }
    }

    pub async fn run(&self, graph: &WorkloadGraph) -> Result<(), ComposeError> {
        let spec = self.translate(graph);
        self.engine.up(&spec).await?;
        Ok(())
    }

    fn translate(&self, graph: &WorkloadGraph) -> ComposeSpec {
        let mut services = IndexMap::new();
        for (id, node) in &graph.nodes {
            let service = ComposeService {
                image: node.image.clone(),
                ports: Some(node.ports.iter().map(|p| crate::types::PortOrString::String(p.clone())).collect()),
                depends_on: Some(crate::types::DependsOnOrList::List(node.depends_on.clone())),
                ..Default::default()
            };
            services.insert(id.clone(), service);
        }
        ComposeSpec {
            name: Some(graph.name.clone()),
            services,
            ..Default::default()
        }
    }
}
