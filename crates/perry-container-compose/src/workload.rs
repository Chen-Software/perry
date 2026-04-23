use crate::error::{ComposeError, Result};
use crate::types::{ContainerInfo, ContainerLogs};
use crate::backend::ContainerBackend;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use indexmap::IndexMap;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadRuntime {
    Oci,
    Microvm,
    Wasm,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadIsolation {
    Default,
    Isolated,
    Hardened,
    Untrusted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadPolicy {
    pub isolation: WorkloadIsolation,
    pub read_only_root: bool,
    pub no_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadNodeSpec {
    pub name: String,
    pub image: String,
    pub ports: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub depends_on: Vec<String>,
    pub runtime: WorkloadRuntime,
    pub policy: WorkloadPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadGraphSpec {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNodeSpec>,
}

pub struct WorkloadGraphEngine {
    pub spec: WorkloadGraphSpec,
    pub backend: Arc<dyn ContainerBackend>,
    pub node_states: DashMap<String, WorkloadNodeState>,
}

#[derive(Debug, Clone)]
pub struct WorkloadNodeState {
    pub container_id: Option<String>,
    pub status: String,
    pub ip: Option<String>,
}

/// Global registry of workload graph instances.
static WORKLOAD_INSTANCES: Lazy<DashMap<u64, Arc<WorkloadGraphEngine>>> = Lazy::new(DashMap::new);
static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

impl WorkloadGraphEngine {
    pub fn new(spec: WorkloadGraphSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        Self {
            spec,
            backend,
            node_states: DashMap::new(),
        }
    }

    pub async fn run(&self) -> Result<u64> {
        // Resolve order using Kahn's algorithm (topological sort)
        let order = self.resolve_order()?;

        for node_name in order {
            let node = self.spec.nodes.get(&node_name).unwrap();
            self.start_node(node).await?;
        }

        let id = NEXT_INSTANCE_ID.fetch_add(1, Ordering::SeqCst);
        // We can't easily register 'self' as Arc if it's not already one.
        // The caller will handle registration.
        Ok(id)
    }

    fn resolve_order(&self) -> Result<Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();

        for name in self.spec.nodes.keys() {
            in_degree.insert(name.clone(), 0);
            adj.insert(name.clone(), Vec::new());
        }

        for (name, node) in &self.spec.nodes {
            for dep in &node.depends_on {
                if !self.spec.nodes.contains_key(dep) {
                    return Err(ComposeError::validation(format!("Node '{}' depends on unknown node '{}'", name, dep)));
                }
                *in_degree.get_mut(name).unwrap() += 1;
                adj.get_mut(dep).unwrap().push(name.clone());
            }
        }

        let mut queue: std::collections::BTreeSet<String> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(n, _)| n.clone())
            .collect();

        let mut result = Vec::new();
        while let Some(u) = queue.pop_first() {
            result.push(u.clone());
            for v in &adj[&u] {
                let d = in_degree.get_mut(v).unwrap();
                *d -= 1;
                if *d == 0 {
                    queue.insert(v.clone());
                }
            }
        }

        if result.len() != self.spec.nodes.len() {
            let remaining: Vec<String> = in_degree.into_iter().filter(|(_, d)| *d > 0).map(|(n, _)| n).collect();
            return Err(ComposeError::DependencyCycle { services: remaining });
        }

        Ok(result)
    }

    async fn start_node(&self, node: &WorkloadNodeSpec) -> Result<()> {
        // Placeholder for real OCI/MicroVM/WASM logic
        // For now, we delegate to OCI backend
        let mut container_spec = crate::types::ContainerSpec {
            image: node.image.clone(),
            name: Some(format!("{}-{}", self.spec.name, node.name)),
            ports: node.ports.clone(),
            env: node.env.clone(),
            ..Default::default()
        };

        // Resolve cross-node references in env
        if let Some(ref mut env) = container_spec.env {
            for val in env.values_mut() {
                // Resolution logic for placeholders like {{node.db.ip}} or {{node.db.endpoint.5432}}
                // This is a simplified implementation.
                self.resolve_env_placeholders(val);
            }
        }

        let handle = self.backend.run(&container_spec).await?;

        self.node_states.insert(node.name.clone(), WorkloadNodeState {
            container_id: Some(handle.id),
            status: "running".to_string(),
            ip: None, // Will be updated after inspection
        });

        Ok(())
    }

    fn resolve_env_placeholders(&self, val: &mut String) {
        // Implementation of reference resolution
        for node_entry in self.node_states.iter() {
            let node_name = node_entry.key();
            let state = node_entry.value();

            let ip_key = format!("{{{{nodes.{}.ip}}}}", node_name);
            if val.contains(&ip_key) {
                *val = val.replace(&ip_key, state.ip.as_deref().unwrap_or("unknown"));
            }

            // Endpoint resolution: {{nodes.db.endpoint.5432}}
            // (Simulated - real implementation would find mapped host port)
        }
    }

    pub async fn down(&self) -> Result<()> {
        for entry in self.node_states.iter() {
            if let Some(ref id) = entry.value().container_id {
                let _ = self.backend.stop(id, None).await;
                let _ = self.backend.remove(id, true).await;
            }
        }
        Ok(())
    }
}

pub fn register_instance(engine: Arc<WorkloadGraphEngine>) -> u64 {
    let id = NEXT_INSTANCE_ID.fetch_add(1, Ordering::SeqCst);
    WORKLOAD_INSTANCES.insert(id, engine);
    id
}

pub fn get_instance(id: u64) -> Option<Arc<WorkloadGraphEngine>> {
    WORKLOAD_INSTANCES.get(&id).map(|e| Arc::clone(&e))
}

pub fn unregister_instance(id: u64) {
    WORKLOAD_INSTANCES.remove(&id);
}
