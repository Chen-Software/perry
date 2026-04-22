use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec,
};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use crate::backend::ContainerBackend;

static COMPOSE_ENGINES: once_cell::sync::Lazy<std::sync::Mutex<IndexMap<u64, Arc<ComposeEngine>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(IndexMap::new()));

static NEXT_STACK_ID: AtomicU64 = AtomicU64::new(1);

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend + Send + Sync>,
    /// Resources that were created in this session
    session_containers: std::sync::Mutex<Vec<String>>,
    session_networks: std::sync::Mutex<Vec<String>>,
    session_volumes: std::sync::Mutex<Vec<String>>,
}

impl ComposeEngine {
    pub fn new(
        spec: ComposeSpec,
        project_name: String,
        backend: Arc<dyn ContainerBackend + Send + Sync>,
    ) -> Self {
        ComposeEngine {
            spec,
            project_name,
            backend,
        }
    }

    fn register(&self) -> ComposeHandle {
        let stack_id = NEXT_STACK_ID.fetch_add(1, Ordering::SeqCst);
        let services: Vec<String> = self.spec.services.keys().cloned().collect();
        let handle = ComposeHandle {
            stack_id,
            project_name: self.project_name.clone(),
            services,
        };
        COMPOSE_ENGINES.lock().unwrap().insert(stack_id, Arc::new(ComposeEngine::new(
            self.spec.clone(),
            self.project_name.clone(),
            Arc::clone(&self.backend),
        )));
        handle
    }

    pub async fn up(
        &self,
        services: &[String],
        _detach: bool,
        _build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        // 1. Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                if let Some(cfg) = config {
                    self.backend.create_network(name, cfg).await?;
                } else {
                    self.backend.create_network(name, &Default::default()).await?;
                }
            }
        }

        // 2. Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                if let Some(cfg) = config {
                    self.backend.create_volume(name, cfg).await?;
                } else {
                    self.backend.create_volume(name, &Default::default()).await?;
                }
            }
        }

        // 3. Resolve order and start services
        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        let mut started = Vec::new();
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);

            // Extract primary network if any
            let network = match &svc.networks {
                Some(crate::types::ServiceNetworks::List(l)) => l.first().cloned(),
                Some(crate::types::ServiceNetworks::Map(m)) => m.keys().next().cloned(),
                None => None,
            };

            let container_spec = ContainerSpec {
                image: svc.image.clone().unwrap_or_default(),
                name: Some(container_name.clone()),
                ports: Some(svc.ports.as_ref().map(|p| p.iter().map(|ps| match ps {
                    crate::types::PortSpec::Short(v) => match v {
                        serde_yaml::Value::String(s) => s.clone(),
                        serde_yaml::Value::Number(n) => n.to_string(),
                        _ => v.as_str().unwrap_or_default().to_string(),
                    },
                    crate::types::PortSpec::Long(lp) => {
                        let publ = lp.published.as_ref().map(|v| match v {
                            serde_yaml::Value::String(s) => s.clone(),
                            serde_yaml::Value::Number(n) => n.to_string(),
                            _ => v.as_str().unwrap_or_default().to_string(),
                        }).unwrap_or_default();
                        let target = match &lp.target {
                            serde_yaml::Value::String(s) => s.clone(),
                            serde_yaml::Value::Number(n) => n.to_string(),
                            _ => lp.target.as_str().unwrap_or_default().to_string(),
                        };
                        format!("{}:{}", publ, target)
                    },
                }).collect()).unwrap_or_default()),
                volumes: Some(svc.volumes.as_ref().map(|v| v.iter().map(|vs| match vs {
                    serde_yaml::Value::String(s) => s.clone(),
                    _ => vs.as_str().unwrap_or_default().to_string(),
                }).collect()).unwrap_or_default()),
                env: Some(match &svc.environment {
                    Some(crate::types::ListOrDict::Dict(d)) => d.iter().map(|(k, v)| (k.clone(), v.as_ref().map(|vv| match vv {
                        serde_yaml::Value::String(s) => s.clone(),
                        serde_yaml::Value::Number(n) => n.to_string(),
                        serde_yaml::Value::Bool(b) => b.to_string(),
                        _ => vv.as_str().unwrap_or_default().to_string(),
                    }).unwrap_or_default())).collect(),
                    Some(crate::types::ListOrDict::List(l)) => l.iter().filter_map(|s| s.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect(),
                    None => HashMap::new(),
                }),
                cmd: Some(match &svc.command {
                    Some(serde_yaml::Value::String(s)) => vec![s.clone()],
                    Some(serde_yaml::Value::Sequence(seq)) => seq.iter().map(|v| v.as_str().unwrap_or_default().to_string()).collect(),
                    _ => vec![],
                }),
                entrypoint: None,
                network,
                rm: None,
                read_only: svc.read_only,
            };

            match self.backend.run(&container_spec).await {
                Ok(_) => {
                    started.push(container_name);
                }
                Err(e) => {
                    // Rollback
                    for name in started.iter().rev() {
                        let _ = self.backend.stop(name, Some(10)).await;
                        let _ = self.backend.remove(name, true).await;
                    }
                    return Err(ComposeError::ServiceStartupFailed {
                        service: svc_name.clone(),
                        message: e.to_string(),
                    });
                }
            }
        }

        // 3. Start services in dependency order
        for svc_name in target {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;

            let container_name = service::service_container_name(svc, svc_name);
            let inspect_result = self.backend.inspect(&container_name).await;

            let res = match inspect_result {
                Ok(info) if info.status == "running" => Ok(()),
                Ok(info) if info.status != "not found" => {
                    self.backend.start(&container_name).await.map(|_| {
                        self.session_containers.lock().unwrap().push(container_name.clone());
                    })
                }
                _ => {
                    // Build if needed
                    if build && svc.needs_build(self.backend.as_ref(), svc_name).await? {
                        let build_config = svc.build.as_ref().unwrap().as_build();
                        let tag = svc.image_ref(svc_name);
                        tracing::info!("Building image '{}'…", tag);
                        if let Err(e) = self.backend.build(&build_config, &tag).await {
                            Err(e)
                        } else {
                            self.run_service(svc, svc_name, &container_name).await
                        }
                    } else {
                        // Check if image exists, if not and image_ref is set, try to pull
                        let image = svc.image_ref(svc_name);
                        if self.backend.list_images().await.map_or(true, |list| !list.iter().any(|i| i.repository == image || i.id == image)) {
                            if let Some(img) = &svc.image {
                                tracing::info!("Pulling image '{}'…", img);
                                if let Err(e) = self.backend.pull_image(img).await {
                                    return Err(ComposeError::ImagePullFailed { message: e.to_string() });
                                }
                            }
                        }
                        self.run_service(svc, svc_name, &container_name).await
                    }
                }
            };

            if let Err(e) = res {
                self.rollback().await;
                return Err(ComposeError::ServiceStartupFailed {
                    service: svc_name.clone(),
                    message: e.to_string(),
                });
            }
        }

        // Register and return handle
        Ok(self.register())
    }

    pub async fn down(
        &self,
        services: &[String],
        _remove_orphans: bool,
        remove_volumes: bool,
    ) -> Result<()> {
        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        for svc_name in target.iter().rev() {
            let svc = self.spec.services.get(*svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            let _ = self.backend.stop(&container_name, Some(10)).await;
            let _ = self.backend.remove(&container_name, true).await;
        }

        if let Some(networks) = &self.spec.networks {
            for name in networks.keys() {
                let _ = self.backend.remove_network(name).await;
            }
        }

        if remove_volumes {
            if let Some(volumes) = &self.spec.volumes {
                for name in volumes.keys() {
                    let _ = self.backend.remove_volume(name).await;
                }
            }
        }

        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut infos = Vec::new();
        for (svc_name, svc) in &self.spec.services {
            let container_name = service::service_container_name(svc, svc_name);
            if let Ok(info) = self.backend.inspect(&container_name).await {
                infos.push(info);
            }
        }
        Ok(infos)
    }

    pub async fn logs(
        &self,
        services: &[String],
        tail: Option<u32>,
    ) -> Result<HashMap<String, String>> {
        let mut all_logs = HashMap::new();
        let target: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };

        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            if let Ok(logs) = self.backend.logs(&container_name, tail).await {
                all_logs.insert(svc_name.clone(), format!("STDOUT:\n{}\nSTDERR:\n{}", logs.stdout, logs.stderr));
            }
        }
        Ok(all_logs)
    }

    pub async fn exec(
        &self,
        service: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        let svc = self.spec.services.get(service).ok_or_else(|| ComposeError::NotFound(service.into()))?;
        let container_name = service::service_container_name(svc, service);
        self.backend.exec(&container_name, cmd, env, workdir).await
    }

    pub fn config(&self) -> Result<String> {
        serde_yaml::to_string(&self.spec).map_err(ComposeError::ParseError)
    }

    /// Resolve the startup order of services using Kahn's algorithm.
    pub fn resolve_startup_order(&self) -> Result<Vec<String>> {
        resolve_startup_order(&self.spec)
    }

    pub fn graph(&self) -> Result<crate::types::ServiceGraph> {
        let nodes = self.resolve_startup_order()?;
        let mut edges = Vec::new();
        for (name, svc) in &self.spec.services {
            if let Some(deps) = &svc.depends_on {
                for dep in deps.service_names() {
                    edges.push(crate::types::ServiceEdge {
                        from: name.clone(),
                        to: dep,
                    });
                }
            }
        }
        Ok(crate::types::ServiceGraph { nodes, edges })
    }

    pub async fn status(&self) -> Result<crate::types::StackStatus> {
        let mut services = Vec::new();
        let mut healthy = true;

        for (svc_name, svc) in &self.spec.services {
            let container_name = service::service_container_name(svc, svc_name);
            let (state, container_id, error) = match self.backend.inspect(&container_name).await {
                Ok(info) => {
                    let s = info.status.to_lowercase();
                    if s != "running" {
                        healthy = false;
                    }
                    (s, Some(info.id), None)
                }
                Err(e) => {
                    healthy = false;
                    ("not found".to_string(), None, Some(e.to_string()))
                }
            };

            services.push(crate::types::ServiceStatus {
                service: svc_name.clone(),
                state,
                container_id,
                error,
            });
        }

        Ok(crate::types::StackStatus { services, healthy })
    }

    // ============ start / stop / restart ============

    /// Start existing stopped services.
    pub async fn start(&self, services: &[String]) -> Result<()> {
        let target: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            self.backend.start(&container_name).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let target: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            self.backend.stop(&container_name, None).await?;
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.stop(services).await?;
        self.start(services).await
    }
}

// ============ Workload Graph Engine ============

pub struct WorkloadGraphEngine {
    pub backend: Arc<dyn ContainerBackend + Send + Sync>,
    pub project_name: String,
}

impl WorkloadGraphEngine {
    pub fn new(backend: Arc<dyn ContainerBackend + Send + Sync>, project_name: String) -> Self {
        Self {
            backend,
            project_name,
        }
    }

    pub async fn run(&self, graph: crate::types::WorkloadGraph, _opts: crate::types::RunGraphOptions) -> Result<crate::types::GraphHandle> {
        // Convert WorkloadGraph to ComposeSpec for execution
        let mut services = IndexMap::new();
        for (id, node) in &graph.nodes {
            let mut svc = crate::types::ComposeService::default();
            svc.image = node.image.clone();
            svc.ports = Some(node.ports.iter().map(|p| crate::types::PortSpec::Short(serde_yaml::Value::String(p.clone()))).collect());

            // Convert workload policy to service flags
            match node.policy.tier {
                crate::types::PolicyTier::Untrusted => {
                    svc.read_only = Some(true);
                    svc.network_mode = Some("none".into());
                    // untrusted forces microvm isolation
                    svc.isolation_level = Some(crate::types::IsolationLevel::MicroVm);
                }
                crate::types::PolicyTier::Hardened => {
                    svc.read_only = Some(true);
                }
                crate::types::PolicyTier::Isolated => {
                    svc.network_mode = Some("none".into());
                }
                _ => {}
            }

            if node.policy.read_only_root {
                svc.read_only = Some(true);
            }
            if node.policy.no_network {
                svc.network_mode = Some("none".into());
            }

            let mut env = IndexMap::new();
            for (k, v) in &node.env {
                match v {
                    crate::types::WorkloadEnvValue::Literal(s) => {
                        env.insert(k.clone(), Some(serde_yaml::Value::String(s.clone())));
                    }
                    crate::types::WorkloadEnvValue::Ref(r) => {
                        // WorkloadRefs are resolved AFTER startup, for now we leave as placeholder
                        env.insert(k.clone(), Some(serde_yaml::Value::String(format!("__REF__:{}:{}:{:?}", r.node_id, r.port.as_deref().unwrap_or(""), r.projection))));
                    }
                }
            }
            svc.environment = Some(crate::types::ListOrDict::Dict(env));
            svc.depends_on = Some(crate::types::DependsOnSpec::List(node.depends_on.clone()));

            services.insert(id.clone(), svc);
        }

        let spec = ComposeSpec {
            name: Some(graph.name.clone()),
            services,
            ..Default::default()
        };

        let engine = Arc::new(ComposeEngine::new(spec, self.project_name.clone(), Arc::clone(&self.backend)));
        let handle = engine.up(&[], true, false, false).await?;

        Ok(crate::types::GraphHandle {
            stack_id: handle.stack_id,
            graph_name: graph.name,
            nodes: graph.nodes.keys().cloned().collect(),
        })
    }
}

// ============ Dependency resolution (Kahn's algorithm) ============

/// Resolve the startup order of services using Kahn's algorithm (BFS topological sort).
///
/// Returns services in dependency order. If a cycle is detected, returns
/// `ComposeError::DependencyCycle` listing all services in the cycle.
pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
    let mut in_degree: IndexMap<String, usize> = IndexMap::new();
    let mut dependents: IndexMap<String, Vec<String>> = IndexMap::new();

    for name in spec.services.keys() {
        in_degree.insert(name.clone(), 0);
        dependents.insert(name.clone(), Vec::new());
    }

    for (name, service) in &spec.services {
        if let Some(deps) = &service.depends_on {
            for dep in deps.service_names() {
                if !spec.services.contains_key(&dep) {
                    return Err(ComposeError::ValidationError {
                        message: format!("Service '{}' depends on '{}' which is not defined", name, dep)
                    });
                }
                *in_degree.get_mut(name).unwrap() += 1;
                dependents.get_mut(&dep).unwrap().push(name.clone());
            }
        }
    }

    let mut queue: std::collections::BTreeSet<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(name, _)| name.clone())
        .collect();

    let mut order: Vec<String> = Vec::new();
    while let Some(service) = queue.pop_first() {
        order.push(service.clone());
        for dependent in dependents.get(&service).unwrap_or(&Vec::new()).clone() {
            let deg = in_degree.get_mut(&dependent).unwrap();
            *deg -= 1;
            if *deg == 0 {
                queue.insert(dependent);
            }
        }
    }

    if order.len() != spec.services.len() {
        let cycle_services: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg > 0)
            .map(|(name, _)| name.clone())
            .collect();
        return Err(ComposeError::DependencyCycle { services: cycle_services });
    }

    Ok(order)
}
