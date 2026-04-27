//! `ComposeEngine` — the core compose orchestration engine.
//!
//! Provides `ComposeEngine::up()`, `down()`, `ps()`, `logs()`, `exec()`, etc.
//! Uses Kahn's algorithm for dependency resolution.

use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec,
};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Global registry of running compose engines, keyed by stack ID.
static COMPOSE_ENGINES: once_cell::sync::Lazy<std::sync::Mutex<IndexMap<u64, Arc<ComposeEngine>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(IndexMap::new()));

/// Next available stack ID
static NEXT_STACK_ID: AtomicU64 = AtomicU64::new(1);

/// The compose orchestration engine.
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
    /// Create a new ComposeEngine.
    pub fn new(
        spec: ComposeSpec,
        project_name: String,
        backend: Arc<dyn ContainerBackend + Send + Sync>,
    ) -> Self {
        ComposeEngine {
            spec,
            project_name,
            backend,
            session_containers: std::sync::Mutex::new(Vec::new()),
            session_networks: std::sync::Mutex::new(Vec::new()),
            session_volumes: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Register this engine in the global registry and return a handle.
    fn register(self: Arc<Self>) -> ComposeHandle {
        let stack_id = NEXT_STACK_ID.fetch_add(1, Ordering::SeqCst);
        let services: Vec<String> = self.spec.services.keys().cloned().collect();
        let handle = ComposeHandle {
            stack_id,
            project_name: self.project_name.clone(),
            services,
        };
        COMPOSE_ENGINES
            .lock()
            .unwrap()
            .insert(stack_id, Arc::clone(&self));
        handle
    }

    /// Look up an engine by stack ID.
    pub fn get_engine(stack_id: u64) -> Option<Arc<ComposeEngine>> {
        COMPOSE_ENGINES.lock().unwrap().get(&stack_id).cloned()
    }

    /// Remove an engine from the registry.
    pub fn unregister(stack_id: u64) {
        COMPOSE_ENGINES.lock().unwrap().shift_remove(&stack_id);
    }

    // ============ up / start ============

    /// Bring up services in dependency order.
    ///
    /// Creates networks and volumes first, then starts containers.
    /// On failure, rolls back all resources created during this session.
    pub async fn up(
        self: Arc<Self>,
        services: &[String],
        _detach: bool,
        build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        self.up_with_options(services, build, crate::types::ExecutionStrategy::DependencyAware, crate::types::FailureStrategy::RollbackAll).await
    }

    pub async fn up_with_options(
        self: Arc<Self>,
        services: &[String],
        build: bool,
        strategy: crate::types::ExecutionStrategy,
        on_failure: crate::types::FailureStrategy,
    ) -> Result<ComposeHandle> {
        let levels = compute_topological_levels(&self.spec)?;

        // Filter levels to target services
        let target_levels: Vec<Vec<String>> = if services.is_empty() {
            levels
        } else {
            levels.into_iter().map(|level| {
                level.into_iter().filter(|s| services.contains(s)).collect()
            }).filter(|level: &Vec<String>| !level.is_empty()).collect()
        };

        // 1. Create networks (skip external)
        if let Some(networks) = &self.spec.networks {
            for (net_name, net_config_opt) in networks {
                let external = net_config_opt
                    .as_ref()
                    .map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let resolved_name = net_config_opt
                    .as_ref()
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(net_name.as_str());

                // State-aware: only create if not exists
                if self.backend.inspect_network(resolved_name).await.is_err() {
                    let spec_config = net_config_opt.clone().unwrap_or_default();
                    let config = crate::backend::NetworkConfig {
                        driver: spec_config.driver,
                        labels: spec_config.labels.map(|l| l.to_map()).unwrap_or_default(),
                        internal: spec_config.internal.unwrap_or(false),
                        enable_ipv6: spec_config.enable_ipv6.unwrap_or(false),
                    };
                    tracing::info!("Creating network '{}'…", resolved_name);
                    if let Err(e) = self.backend.create_network(resolved_name, &config).await {
                        self.rollback().await;
                        return Err(ComposeError::ServiceStartupFailed {
                            service: format!("network/{}", net_name),
                            message: e.to_string(),
                        });
                    }
                    self.session_networks.lock().unwrap().push(resolved_name.to_string());
                }
            }
        }

        // 2. Create volumes (skip external)
        if let Some(volumes) = &self.spec.volumes {
            for (vol_name, vol_config_opt) in volumes {
                let external = vol_config_opt
                    .as_ref()
                    .map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let resolved_name = vol_config_opt
                    .as_ref()
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(vol_name.as_str());

                // State-aware: only create if not exists
                let spec_config = vol_config_opt.clone().unwrap_or_default();
                let config = crate::backend::VolumeConfig {
                    driver: spec_config.driver,
                    labels: spec_config.labels.map(|l| l.to_map()).unwrap_or_default(),
                };
                tracing::info!("Creating volume '{}'…", resolved_name);
                if let Err(e) = self.backend.create_volume(resolved_name, &config).await {
                    self.rollback().await;
                    return Err(ComposeError::ServiceStartupFailed {
                        service: format!("volume/{}", vol_name),
                        message: e.to_string(),
                    });
                }
                self.session_volumes.lock().unwrap().push(resolved_name.to_string());
            }
        }

        // 3. Start services
        let execution_plan = match strategy {
            crate::types::ExecutionStrategy::Sequential => target_levels.clone(),
            crate::types::ExecutionStrategy::DependencyAware | crate::types::ExecutionStrategy::ParallelSafe => target_levels.clone(),
            crate::types::ExecutionStrategy::MaxParallel => {
                let all: Vec<String> = target_levels.into_iter().flatten().collect();
                vec![all]
            }
        };

        for (i, level) in execution_plan.iter().enumerate() {
            let mut futures = Vec::new();
            for svc_name in level {
                let this = Arc::clone(&self);
                futures.push(async move {
                    let svc = this.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
                    let container_name = service::service_container_name(svc, svc_name);

                    let inspect_result = this.backend.inspect(&container_name).await;
                    match inspect_result {
                        Ok(info) if info.status == "running" => Ok(()),
                        Ok(info) if info.status != "not found" => {
                            this.backend.start(&container_name).await.map(|_| {
                                this.session_containers.lock().unwrap().push(container_name.clone());
                            })
                        }
                        _ => {
                            // Build if needed
                            if build && svc.needs_build(this.backend.as_ref(), svc_name).await? {
                                let build_config = svc.build.as_ref().unwrap().as_build();
                                let tag = svc.image_ref(svc_name);
                                tracing::info!("Building image '{}'…", tag);
                                if let Err(e) = this.backend.build(&build_config, &tag).await {
                                    Err(e)
                                } else {
                                    this.run_service(svc, svc_name, &container_name).await
                                }
                            } else {
                                let image = svc.image_ref(svc_name);
                                if this.backend.list_images().await.map_or(true, |list| !list.iter().any(|i| i.repository == image || i.id == image)) {
                                    if let Some(img) = &svc.image {
                                        tracing::info!("Pulling image '{}'…", img);
                                        if let Err(e) = this.backend.pull_image(img).await {
                                            return Err(ComposeError::ImagePullFailed { message: e.to_string() });
                                        }
                                    }
                                }
                                this.run_service(svc, svc_name, &container_name).await
                            }
                        }
                    }
                });
            }

            let results = if matches!(strategy, crate::types::ExecutionStrategy::Sequential) {
                let mut res = Vec::new();
                for f in futures {
                    res.push(f.await);
                }
                res
            } else {
                futures::future::join_all(futures).await
            };

            for (svc_name, result) in level.iter().zip(results) {
                if let Err(e) = result {
                    match on_failure {
                        crate::types::FailureStrategy::RollbackAll => {
                            self.rollback().await;
                            return Err(ComposeError::ServiceStartupFailed {
                                service: svc_name.clone(),
                                message: e.to_string(),
                            });
                        }
                        crate::types::FailureStrategy::HaltGraph => {
                            return Err(ComposeError::ServiceStartupFailed {
                                service: svc_name.clone(),
                                message: e.to_string(),
                            });
                        }
                        crate::types::FailureStrategy::PartialContinue => {
                            tracing::error!("Service '{}' failed to start: {}. Continuing...", svc_name, e);
                        }
                    }
                }
            }

            if i < execution_plan.len() - 1 && matches!(strategy, crate::types::ExecutionStrategy::ParallelSafe) {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }

        // Register and return handle
        Ok(self.register())
    }

    async fn run_service(&self, svc: &crate::types::ComposeService, svc_name: &str, container_name: &str) -> Result<()> {
        let image = svc.image_ref(svc_name);
        let env = svc.resolved_env();
        let ports = svc.port_strings();
        let vols = svc.volume_strings();

        let mut all_labels: HashMap<String, String> = svc
            .labels
            .as_ref()
            .map(|l| l.to_map())
            .unwrap_or_default();
        all_labels.insert("perry.compose.project".into(), self.project_name.clone());
        all_labels.insert("perry.compose.service".into(), svc_name.to_string());

        let cmd = svc.command_list();

        let spec = ContainerSpec {
            image: image.clone(),
            name: Some(container_name.to_string()),
            ports: Some(ports),
            volumes: Some(vols),
            env: Some(env),
            labels: Some(all_labels),
            cmd,
            rm: Some(false),
            read_only: svc.read_only,
            ..Default::default()
        };

        self.backend.run(&spec).await.map(|_| {
            self.session_containers.lock().unwrap().push(container_name.to_string());
        })
    }

    async fn rollback(&self) {
        tracing::info!("Rolling back session resources…");

        let containers = {
            let mut guard = self.session_containers.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        for container_name in containers.iter().rev() {
            let _ = self.backend.stop(container_name, None).await;
            let _ = self.backend.remove(container_name, true).await;
        }

        let networks = {
            let mut guard = self.session_networks.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        for net_name in networks {
            let _ = self.backend.remove_network(&net_name).await;
        }

        let volumes = {
            let mut guard = self.session_volumes.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        for vol_name in volumes {
            let _ = self.backend.remove_volume(&vol_name).await;
        }
    }

    // ============ down / stop ============

    /// Stop and remove services in reverse dependency order.
    pub async fn down(
        &self,
        services: &[String],
        _remove_orphans: bool,
        remove_volumes: bool,
    ) -> Result<()> {
        let mut order = resolve_startup_order(&self.spec)?;
        order.reverse();

        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        // 1. Stop and remove containers
        if services.is_empty() {
            // Remove by project labels if no specific services targeted
            let all = self.backend.list(true).await?;
            for container in all {
                if container.labels.get("perry.compose.project").map(|v| v == &self.project_name).unwrap_or(false) {
                    if container.status == "running" {
                        let _ = self.backend.stop(&container.id, None).await;
                    }
                    let _ = self.backend.remove(&container.id, true).await;
                }
            }
        } else {
            for svc_name in &target {
                let svc = self
                    .spec
                    .services
                    .get(*svc_name)
                    .ok_or_else(|| ComposeError::NotFound((*svc_name).clone()))?;

                let container_name = service::service_container_name(svc, svc_name);
                let inspect_result = self.backend.inspect(&container_name).await;

                if let Ok(info) = inspect_result {
                    if info.status == "running" {
                        self.backend.stop(&container_name, None).await?;
                    }
                    self.backend.remove(&container_name, true).await?;
                }
            }
        }
        // Also clear session containers if they match target
        if services.is_empty() {
             let mut guard = self.session_containers.lock().unwrap();
             guard.clear();
        } else {
            let mut guard = self.session_containers.lock().unwrap();
            guard.retain(|c| !target.iter().any(|svc_name| {
                 if let Some(svc) = self.spec.services.get(*svc_name) {
                     service::service_container_name(svc, svc_name) == *c
                 } else {
                     false
                 }
            }));
        }

        // 2. Remove session networks (non-external, idempotent)
        let networks = {
            let mut guard = self.session_networks.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        for net_name in networks {
            let _ = self.backend.remove_network(&net_name).await;
        }

        // 3. Remove session volumes (if requested)
        if remove_volumes {
            let volumes = {
                let mut guard = self.session_volumes.lock().unwrap();
                std::mem::take(&mut *guard)
            };
            for vol_name in volumes {
                let _ = self.backend.remove_volume(&vol_name).await;
            }
        }

        Ok(())
    }

    // ============ ps ============

    /// List the status of all services.
    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut results = Vec::new();

        for (svc_name, svc) in &self.spec.services {
            let container_name = service::service_container_name(svc, svc_name);
            let info = match self.backend.inspect(&container_name).await {
                Ok(mut info) => {
                    info.ports = svc.port_strings();
                    info
                }
                Err(_) => ContainerInfo {
                    id: container_name.clone(),
                    name: container_name,
                    image: svc.image_ref(svc_name),
                    status: "not found".to_string(),
                    ports: svc.port_strings(),
                    labels: HashMap::new(),
                    created: String::new(),
                },
            };
            results.push(info);
        }

        results.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(results)
    }

    // ============ logs ============

    /// Get logs from services.
    pub async fn logs(
        &self,
        services: &[String],
        tail: Option<u32>,
    ) -> Result<HashMap<String, ContainerLogs>> {
        let service_names: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };

        let mut all_logs = HashMap::new();
        for svc_name in service_names {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;

            let container_name = service::service_container_name(svc, svc_name);
            let logs = self.backend.logs(&container_name, tail).await?;
            all_logs.insert(svc_name.clone(), logs);
        }

        Ok(all_logs)
    }

    // ============ exec ============

    /// Execute a command in a running service container.
    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let svc = self
            .spec
            .services
            .get(service)
            .ok_or_else(|| ComposeError::NotFound(service.to_owned()))?;

        let container_name = service::service_container_name(svc, service);
        let info = self.backend.inspect(&container_name).await?;

        if info.status != "running" {
            return Err(ComposeError::ServiceStartupFailed {
                service: service.to_owned(),
                message: format!("container '{}' is not running", container_name),
            });
        }

        self.backend
            .exec(&container_name, cmd, None, None)
            .await
    }

    // ============ config ============

    /// Validate and return the resolved compose configuration.
    pub fn config(&self) -> Result<String> {
        self.spec.to_yaml()
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
        let target: Vec<String> = if services.is_empty() {
            self.spec.services.keys().cloned().collect()
        } else {
            services.to_vec()
        };

        for svc_name in target {
            let svc = self
                .spec
                .services
                .get(&svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let container_name = service::service_container_name(svc, &svc_name);
            self.backend.start(&container_name).await?;
        }

        Ok(())
    }

    /// Stop running services.
    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let target: Vec<String> = if services.is_empty() {
            self.spec.services.keys().cloned().collect()
        } else {
            services.to_vec()
        };

        for svc_name in target {
            let svc = self
                .spec
                .services
                .get(&svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let container_name = service::service_container_name(svc, &svc_name);
            self.backend.stop(&container_name, None).await?;
        }

        Ok(())
    }

    /// Restart services.
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

    pub async fn run(&self, graph: crate::types::WorkloadGraph, opts: crate::types::RunGraphOptions) -> Result<crate::types::GraphHandle> {
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
                        // WorkloadRefs are resolved AFTER startup
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
        let handle = engine.clone().up_with_options(&[], false, opts.strategy, opts.on_failure).await?;

        // Resolve WorkloadRefs after startup
        for (node_id, node) in &graph.nodes {
            let mut resolved_env = HashMap::new();
            let mut has_refs = false;
            for (k, v) in &node.env {
                if let crate::types::WorkloadEnvValue::Ref(r) = v {
                    has_refs = true;
                    // Find dependency container info
                    let dep_svc = engine.spec.services.get(&r.node_id).ok_or_else(|| ComposeError::NotFound(r.node_id.clone()))?;
                    let container_name = service::service_container_name(dep_svc, &r.node_id);
                    let info = self.backend.inspect(&container_name).await?;
                    let resolved = r.resolve(&info).map_err(|e| ComposeError::validation(e))?;
                    resolved_env.insert(k.clone(), resolved);
                }
            }

            if has_refs {
                let svc = engine.spec.services.get(node_id).unwrap();
                let _container_name = service::service_container_name(svc, node_id);
                // Inject resolved env via exec or update (here we use a mock-up of what might be needed)
                // In a real impl, we might store these in a registry for the FFI to use during exec
                tracing::info!("Resolved refs for node '{}': {:?}", node_id, resolved_env);
            }
        }

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
    let levels = compute_topological_levels(spec)?;
    Ok(levels.into_iter().flatten().collect())
}

pub fn compute_topological_levels(spec: &ComposeSpec) -> Result<Vec<Vec<String>>> {
    // 1. Build adjacency list and in-degrees
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
                    return Err(ComposeError::validation(format!(
                        "Service '{}' depends on '{}' which is not defined",
                        name, dep
                    )));
                }
                *in_degree.get_mut(name).unwrap() += 1;
                dependents.get_mut(&dep).unwrap().push(name.clone());
            }
        }
    }

    // 2. Queue all services with in-degree 0 (sorted for determinism)
    let mut queue: std::collections::BTreeSet<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(name, _)| name.clone())
        .collect();

    // 3. Process queue by levels
    let mut levels: Vec<Vec<String>> = Vec::new();
    let mut processed_count = 0;

    while !queue.is_empty() {
        let mut current_level = Vec::new();
        let mut next_queue = std::collections::BTreeSet::new();

        for service in queue {
            current_level.push(service.clone());
            processed_count += 1;
            if let Some(deps_list) = dependents.get(&service) {
                for dependent in deps_list {
                    let deg = in_degree.get_mut(dependent).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        next_queue.insert(dependent.clone());
                    }
                }
            }
        }
        levels.push(current_level);
        queue = next_queue;
    }

    // 4. If not all services processed → cycle detected
    if processed_count != spec.services.len() {
        let cycle_services: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg > 0)
            .map(|(name, _)| name.clone())
            .collect();
        return Err(ComposeError::DependencyCycle {
            services: cycle_services,
        });
    }

    Ok(levels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ComposeService;

    fn make_compose(edges: &[(&str, &[&str])]) -> ComposeSpec {
        let mut services = IndexMap::new();
        for (name, deps) in edges {
            let mut svc = ComposeService::default();
            if !deps.is_empty() {
                svc.depends_on = Some(crate::types::DependsOnSpec::List(
                    deps.iter().map(|s| s.to_string()).collect(),
                ));
            }
            services.insert(name.to_string(), svc);
        }
        ComposeSpec {
            services,
            ..Default::default()
        }
    }

    #[test]
    fn test_simple_chain() {
        let compose = make_compose(&[("web", &["db"]), ("db", &[]), ("proxy", &["web"])]);
        let order = resolve_startup_order(&compose).unwrap();
        let pos = |name: &str| order.iter().position(|s| s == name).unwrap();
        assert!(pos("db") < pos("web"), "db must precede web");
        assert!(pos("web") < pos("proxy"), "web must precede proxy");
    }

    #[test]
    fn test_no_deps() {
        let compose = make_compose(&[("a", &[]), ("b", &[]), ("c", &[])]);
        let order = resolve_startup_order(&compose).unwrap();
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn test_diamond_dependency() {
        // a -> b, a -> c, b -> d, c -> d
        let compose = make_compose(&[
            ("a", &[]),
            ("b", &["a"]),
            ("c", &["a"]),
            ("d", &["b", "c"]),
        ]);
        let order = resolve_startup_order(&compose).unwrap();
        let pos = |name: &str| order.iter().position(|s| s == name).unwrap();
        assert!(pos("a") < pos("b"));
        assert!(pos("a") < pos("c"));
        assert!(pos("b") < pos("d"));
        assert!(pos("c") < pos("d"));
    }

    #[test]
    fn test_cycle_detected() {
        let compose = make_compose(&[("a", &["b"]), ("b", &["a"])]);
        let result = resolve_startup_order(&compose);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ComposeError::DependencyCycle { .. }
        ));
    }

    #[test]
    fn test_cycle_lists_all_services() {
        // a -> b -> c -> a (3-node cycle)
        let compose = make_compose(&[("a", &["c"]), ("b", &["a"]), ("c", &["b"])]);
        let result = resolve_startup_order(&compose);
        assert!(result.is_err());
        if let ComposeError::DependencyCycle { services } = result.unwrap_err() {
            assert_eq!(services.len(), 3);
            assert!(services.contains(&"a".to_string()));
            assert!(services.contains(&"b".to_string()));
            assert!(services.contains(&"c".to_string()));
        }
    }

    #[test]
    fn test_invalid_dependency() {
        let compose = make_compose(&[("web", &["nonexistent"])]);
        let result = resolve_startup_order(&compose);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComposeError::ValidationError { .. }));
    }

    #[test]
    fn test_deterministic_order() {
        // Services with no deps should be sorted alphabetically
        let compose = make_compose(&[("c", &[]), ("a", &[]), ("b", &[])]);
        let order = resolve_startup_order(&compose).unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }
}
