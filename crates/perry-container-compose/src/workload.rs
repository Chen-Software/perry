use crate::error::{ComposeError, Result};
use crate::types::{ContainerInfo, ContainerLogs, ContainerSpec};
use crate::backend::ContainerBackend;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use indexmap::IndexMap;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStrategy {
    Sequential,
    MaxParallel,
    DependencyAware,
    ParallelSafe,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FailureStrategy {
    RollbackAll,
    PartialContinue,
    HaltGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeType {
    Oci,
    Microvm,
    Wasm,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityPolicy {
    pub no_network: bool,
    pub read_only_root: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        SecurityPolicy {
            no_network: false,
            read_only_root: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadNodeConfig {
    pub image: String,
    pub ports: Option<Vec<String>>,
    pub volumes: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub cmd: Option<Vec<String>>,
    pub depends_on: Option<Vec<String>>,
    pub runtime: Option<RuntimeType>,
    pub policy: Option<SecurityPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadGraph {
    pub name: String,
    pub nodes: IndexMap<String, WorkloadNodeConfig>,
}

pub struct WorkloadGraphEngine {
    pub graph: WorkloadGraph,
    pub backend: Arc<dyn ContainerBackend>,
    pub project_name: String,
    pub active_containers: Mutex<HashMap<String, String>>, // node_name -> container_id
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphHandle {
    pub handle_id: u64,
    pub graph_name: String,
}

static WORKLOAD_INSTANCES: once_cell::sync::Lazy<Mutex<HashMap<u64, Arc<WorkloadGraphEngine>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

impl WorkloadGraphEngine {
    pub fn new(graph: WorkloadGraph, backend: Arc<dyn ContainerBackend>) -> Self {
        let project_name = format!("workload-{}", graph.name);
        WorkloadGraphEngine {
            graph,
            backend,
            project_name,
            active_containers: Mutex::new(HashMap::new()),
        }
    }

    pub async fn run(
        self: Arc<Self>,
        strategy: ExecutionStrategy,
        on_failure: FailureStrategy,
    ) -> Result<u64> {
        let order = self.resolve_order()?;

        for node_name in order {
            if let Err(e) = self.start_node(&node_name).await {
                match on_failure {
                    FailureStrategy::RollbackAll => {
                        self.rollback().await;
                        return Err(e);
                    }
                    FailureStrategy::HaltGraph => return Err(e),
                    FailureStrategy::PartialContinue => continue,
                }
            }
        }

        let handle_id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
        WORKLOAD_INSTANCES.lock().unwrap().insert(handle_id, self);
        Ok(handle_id)
    }

    async fn start_node(&self, name: &str) -> Result<()> {
        let config = self.graph.nodes.get(name).ok_or_else(|| ComposeError::NotFound(name.into()))?;

        let container_name = format!("{}_{}", self.project_name, name);

        // Resolve env capabilities (typed dependencies)
        let mut env = config.env.clone().unwrap_or_default();
        self.resolve_capabilities(&mut env).await?;

        let spec = ContainerSpec {
            image: config.image.clone(),
            name: Some(container_name.clone()),
            ports: config.ports.clone(),
            volumes: config.volumes.clone(),
            env: Some(env),
            cmd: config.cmd.clone(),
            entrypoint: None,
            network: Some(self.project_name.clone()),
            rm: None,
        };

        let handle = self.backend.run(&spec).await?;
        self.active_containers.lock().unwrap().insert(name.to_string(), handle.id);
        Ok(())
    }

    async fn resolve_capabilities(&self, env: &mut HashMap<String, String>) -> Result<()> {
        let active = self.active_containers.lock().unwrap().clone();

        for value in env.values_mut() {
            if value.starts_with("__PERRY_REF_") && value.ends_with("__") {
                let parts: Vec<&str> = value.trim_matches('_').split('_').collect();
                // parts = ["PERRY", "REF", <NODE>, <CAP>, ...]
                if parts.len() >= 4 && parts[0] == "PERRY" && parts[1] == "REF" {
                    let target_node = parts[2];
                    let cap = parts[3];

                    if let Some(id) = active.get(target_node) {
                        match self.backend.inspect(id).await {
                            Ok(info) => {
                                // Simple resolution for IP
                                if cap == "IP" {
                                    *value = "127.0.0.1".to_string(); // In bridge mode, usually use 127.0.0.1 with port mapping
                                } else if cap == "ENDPOINT" && parts.len() >= 5 {
                                    let port = parts[4];
                                    *value = format!("127.0.0.1:{}", port);
                                } else if cap == "INTERNALURL" && parts.len() >= 5 {
                                    let port = parts[4];
                                    *value = format!("http://127.0.0.1:{}", port);
                                }
                            }
                            Err(_) => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn rollback(&self) {
        let containers: Vec<(String, String)> = self.active_containers.lock().unwrap().drain().collect();
        for (_, id) in containers {
            let _ = self.backend.stop(&id, Some(5)).await;
            let _ = self.backend.remove(&id, true).await;
        }
        // best effort network removal
        let _ = self.backend.remove_network(&self.project_name).await;
    }

    pub async fn down(&self) -> Result<()> {
        self.rollback().await;
        Ok(())
    }

    fn resolve_order(&self) -> Result<Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        for name in self.graph.nodes.keys() {
            in_degree.insert(name.clone(), 0);
            dependents.insert(name.clone(), Vec::new());
        }

        for (name, node) in &self.graph.nodes {
            if let Some(deps) = &node.depends_on {
                for dep in deps {
                    if !self.graph.nodes.contains_key(dep) {
                        return Err(ComposeError::ValidationError {
                            message: format!("Node '{}' depends on '{}' which is not in graph", name, dep)
                        });
                    }
                    *in_degree.get_mut(name).unwrap() += 1;
                    dependents.get_mut(dep).unwrap().push(name.clone());
                }
            }
        }

        let mut queue: std::collections::BTreeSet<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut order = Vec::new();
        while let Some(name) = queue.pop_first() {
            order.push(name.clone());
            if let Some(deps) = dependents.get(&name) {
                for dep in deps {
                    let deg = in_degree.get_mut(dep).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.insert(dep.clone());
                    }
                }
            }
        }

        if order.len() != self.graph.nodes.len() {
            return Err(ComposeError::DependencyCycle {
                services: in_degree.keys().cloned().collect(),
            });
        }

        Ok(order)
    }

    pub async fn status(&self) -> Result<IndexMap<String, String>> {
        let mut statuses = IndexMap::new();
        let containers: Vec<(String, String)> = self.active_containers.lock().unwrap().iter().map(|(k,v)| (k.clone(), v.clone())).collect();
        for (name, id) in containers {
            match self.backend.inspect(&id).await {
                Ok(info) => statuses.insert(name, info.status),
                Err(_) => statuses.insert(name, "unknown".to_string()),
            };
        }
        Ok(statuses)
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut infos = Vec::new();
        let ids: Vec<String> = self.active_containers.lock().unwrap().values().cloned().collect();
        for id in ids {
            if let Ok(info) = self.backend.inspect(&id).await {
                infos.push(info);
            }
        }
        Ok(infos)
    }

    pub async fn logs(&self, node: Option<&str>, tail: Option<u32>) -> Result<HashMap<String, String>> {
        let mut all_logs = HashMap::new();
        let containers: HashMap<String, String> = self.active_containers.lock().unwrap().clone();

        if let Some(n) = node {
            if let Some(id) = containers.get(n) {
                if let Ok(logs) = self.backend.logs(id, tail).await {
                    all_logs.insert(n.to_string(), format!("STDOUT:\n{}\nSTDERR:\n{}", logs.stdout, logs.stderr));
                }
            }
        } else {
            for (name, id) in containers {
                if let Ok(logs) = self.backend.logs(&id, tail).await {
                    all_logs.insert(name, format!("STDOUT:\n{}\nSTDERR:\n{}", logs.stdout, logs.stderr));
                }
            }
        }
        Ok(all_logs)
    }

    pub async fn exec(&self, node: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let id = {
            let containers = self.active_containers.lock().unwrap();
            containers.get(node).cloned().ok_or_else(|| ComposeError::NotFound(node.into()))?
        };
        self.backend.exec(&id, cmd, env, workdir).await
    }
}

pub fn get_workload_engine(handle_id: u64) -> Option<Arc<WorkloadGraphEngine>> {
    WORKLOAD_INSTANCES.lock().unwrap().get(&handle_id).cloned()
}

static GRAPH_BUILDERS: once_cell::sync::Lazy<Mutex<HashMap<u64, WorkloadGraph>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

pub fn create_graph_builder(name: String) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    GRAPH_BUILDERS.lock().unwrap().insert(id, WorkloadGraph { name, nodes: IndexMap::new() });
    id
}

pub fn add_node_to_graph(graph_id: u64, node_name: String, config: WorkloadNodeConfig) {
    if let Some(graph) = GRAPH_BUILDERS.lock().unwrap().get_mut(&graph_id) {
        graph.nodes.insert(node_name, config);
    }
}

pub fn get_graph_from_builder(graph_id: u64) -> Option<WorkloadGraph> {
    GRAPH_BUILDERS.lock().unwrap().remove(&graph_id)
}
