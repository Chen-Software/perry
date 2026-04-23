//! `ComposeEngine` — the core compose orchestration engine.
//!
//! Provides `ComposeEngine::up()`, `down()`, `ps()`, `logs()`, `exec()`, etc.
//! Uses Kahn's algorithm for dependency resolution.

use crate::backend::ContainerBackend;
pub use crate::types::ContainerLogs;
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerSpec,
};
use crate::backend::{NetworkConfig, VolumeConfig};
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
    pub backend: Arc<dyn ContainerBackend>,
    /// Services that were started in this session
    started_containers: std::sync::Mutex<Vec<String>>,
}

impl ComposeEngine {
    /// Create a new ComposeEngine.
    pub fn new(
        spec: ComposeSpec,
        project_name: String,
        backend: Arc<dyn ContainerBackend>,
    ) -> Self {
        ComposeEngine {
            spec,
            project_name,
            backend,
            started_containers: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Register this engine in the global registry and return a handle.
    fn register(&self) -> ComposeHandle {
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
            .insert(stack_id, Arc::new(ComposeEngine::new(
                self.spec.clone(),
                self.project_name.clone(),
                Arc::clone(&self.backend),
            )));
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
    /// On failure, rolls back all previously started containers.
    pub async fn up(
        &self,
        services: &[String],
        _detach: bool,
        build: bool,
        remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;

        // Filter to target services
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        // 0. Remove orphans if requested
        if remove_orphans {
            self.remove_orphans().await?;
        }

        let mut created_networks: Vec<String> = Vec::new();
        let mut created_volumes: Vec<String> = Vec::new();

        // 1. Create networks (skip external)
        if let Some(networks) = &self.spec.networks {
            for (net_name, net_config_opt) in networks {
                let external = net_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let net_config_spec = net_config_opt.as_ref().cloned().unwrap_or_default();
                let net_config = NetworkConfig::from(&net_config_spec);
                let resolved_name = net_config_spec.name.as_deref().unwrap_or(net_name.as_str());

                // Check if already exists
                if self.backend.inspect_network(resolved_name).await.is_err() {
                    tracing::info!("Creating network '{}'…", resolved_name);
                    if let Err(e) = self.backend.create_network(resolved_name, &net_config).await {
                        // Rollback
                        self.rollback(&[], &created_networks, &created_volumes).await;
                        return Err(ComposeError::ServiceStartupFailed {
                            service: format!("network/{}", net_name),
                            message: e.to_string(),
                        });
                    }
                    created_networks.push(resolved_name.to_string());
                }
            }
        }

        // 2. Create volumes (skip external)
        if let Some(volumes) = &self.spec.volumes {
            for (vol_name, vol_config_opt) in volumes {
                let external = vol_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let vol_config_spec = vol_config_opt.as_ref().cloned().unwrap_or_default();
                let vol_config = VolumeConfig::from(&vol_config_spec);
                let resolved_name = vol_config_spec.name.as_deref().unwrap_or(vol_name.as_str());

                // Check if already exists
                if self.backend.inspect_volume(resolved_name).await.is_err() {
                    tracing::info!("Creating volume '{}'…", resolved_name);
                    if let Err(e) = self.backend.create_volume(resolved_name, &vol_config).await {
                        // Rollback
                        self.rollback(&[], &created_networks, &created_volumes).await;
                        return Err(ComposeError::ServiceStartupFailed {
                            service: format!("volume/{}", vol_name),
                            message: e.to_string(),
                        });
                    }
                    created_volumes.push(resolved_name.to_string());
                }
            }
        }

        // 3. Start services in dependency order
        let mut started: Vec<String> = Vec::new();

        for svc_name in target {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;

            let image_ref = svc.image_ref(svc_name);
            let container_name = service::generate_name(svc_name, svc)?;

            // Idempotent check
            let list = self.backend.list(true).await.map_err(|e| ComposeError::BackendError {
                code: -1,
                message: e.to_string(),
            })?;
            let found = list.iter().find(|c| c.name == container_name || c.id == container_name);

            match found {
                Some(info) if info.status == "running" || info.status == "Up" => {
                    tracing::info!("Service '{}' is already running", svc_name);
                    started.push(container_name);
                    continue;
                }
                Some(_) => {
                    tracing::info!("Service '{}' exists but is stopped, starting...", svc_name);
                    if let Err(e) = self.backend.start(&container_name).await {
                        self.rollback(&started, &created_networks, &created_volumes).await;
                        return Err(ComposeError::ServiceStartupFailed {
                            service: svc_name.clone(),
                            message: e.to_string(),
                        });
                    }
                    started.push(container_name);
                    continue;
                }
                None => {} // Does not exist, proceed to create
            }

            // Fresh container - build or pull
            if build || service::needs_build(svc) {
                if let Some(build_spec) = &svc.build {
                    let build_config = build_spec.as_build();
                    let context = build_config.context.as_deref().unwrap_or(".");
                    tracing::info!("Building service '{}' (image: {})...", svc_name, image_ref);
                    if let Err(e) = self.backend.build_image(
                            context,
                            &image_ref,
                            build_config.dockerfile.as_deref(),
                            build_config.args.as_ref().map(|l| l.to_map()).as_ref(),
                        ).await {
                        self.rollback(&started, &created_networks, &created_volumes).await;
                        return Err(ComposeError::ServiceStartupFailed {
                            service: svc_name.clone(),
                            message: format!("Build failed: {}", e),
                        });
                    }
                }
            } else {
                tracing::info!("Pulling image '{}' for service '{}'...", image_ref, svc_name);
                if let Err(e) = self.backend.pull_image(&image_ref).await {
                    self.rollback(&started, &created_networks, &created_volumes).await;
                    return Err(ComposeError::ImagePullFailed {
                        service: svc_name.clone(),
                        image: image_ref,
                        message: e.to_string(),
                    });
                }
            }

            let mut labels = svc.labels.as_ref().map(|l| l.to_map()).unwrap_or_default();
            labels.insert("com.docker.compose.project".into(), self.project_name.clone());
            labels.insert("com.docker.compose.service".into(), svc_name.clone());

            let container_spec = ContainerSpec {
                image: image_ref,
                name: Some(container_name.clone()),
                ports: Some(svc.port_strings()),
                volumes: Some(svc.volume_strings()),
                env: Some(svc.resolved_env()),
                labels: Some(labels),
                cmd: svc.command_list(),
                rm: Some(false),
                ..Default::default()
            };

            tracing::info!("Creating and starting container for service '{}'...", svc_name);
            if let Err(e) = self.backend.run(&container_spec).await {
                self.rollback(&started, &created_networks, &created_volumes).await;
                return Err(ComposeError::ServiceStartupFailed {
                    service: svc_name.clone(),
                    message: e.to_string(),
                });
            }
            started.push(container_name);
        }

        self.started_containers.lock().unwrap().extend(started);
        Ok(self.register())
    }

    async fn rollback(&self, containers: &[String], networks: &[String], volumes: &[String]) {
        for c in containers.iter().rev() {
            let _ = self.backend.stop(c, None).await;
            let _ = self.backend.remove(c, true).await;
        }
        for n in networks.iter().rev() {
            let _ = self.backend.remove_network(n).await;
        }
        for v in volumes.iter().rev() {
            let _ = self.backend.remove_volume(v).await;
        }
    }

    async fn remove_orphans(&self) -> Result<()> {
        let containers = self.backend.list(true).await?;
        for c in containers {
            if c.labels.get("com.docker.compose.project") == Some(&self.project_name) {
                if let Some(svc) = c.labels.get("com.docker.compose.service") {
                    if !self.spec.services.contains_key(svc) {
                        tracing::info!("Removing orphan container '{}'...", c.name);
                        let _ = self.backend.stop(&c.id, None).await;
                        let _ = self.backend.remove(&c.id, true).await;
                    }
                }
            }
        }
        Ok(())
    }

    async fn find_container_for_service(&self, svc_name: &str) -> Result<Option<ContainerInfo>> {
        let containers = self.backend.list(true).await?;
        Ok(containers.into_iter().find(|c| {
            c.labels.get("com.docker.compose.project") == Some(&self.project_name) &&
            c.labels.get("com.docker.compose.service") == Some(&svc_name.to_string())
        }))
    }

    pub async fn down(
        &self,
        volumes: bool,
        remove_orphans: bool,
    ) -> Result<()> {
        let mut order = resolve_startup_order(&self.spec)?;
        order.reverse();

        for svc_name in order {
            if let Some(info) = self.find_container_for_service(&svc_name).await? {
                tracing::info!("Stopping and removing container '{}'...", info.name);
                let _ = self.backend.stop(&info.id, None).await;
                let _ = self.backend.remove(&info.id, true).await;
            }
        }

        if let Some(networks) = &self.spec.networks {
            for (net_name, config) in networks {
                if config.as_ref().map_or(false, |c| c.external.unwrap_or(false)) { continue; }
                let resolved = config.as_ref().and_then(|c| c.name.as_deref()).unwrap_or(net_name);
                let _ = self.backend.remove_network(resolved).await;
            }
        }

        if remove_orphans { self.remove_orphans().await?; }

        if volumes {
            if let Some(vols) = &self.spec.volumes {
                for (vol_name, config) in vols {
                    if config.as_ref().map_or(false, |c| c.external.unwrap_or(false)) { continue; }
                    let resolved = config.as_ref().and_then(|c| c.name.as_deref()).unwrap_or(vol_name);
                    let _ = self.backend.remove_volume(resolved).await;
                }
            }
        }
        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut results = Vec::new();
        for (svc_name, svc) in &self.spec.services {
            if let Some(info) = self.find_container_for_service(svc_name).await? {
                results.push(info);
            } else {
                results.push(ContainerInfo {
                    id: "".into(), name: "".into(), image: svc.image_ref(svc_name),
                    status: "not found".into(), ports: svc.port_strings(),
                    labels: HashMap::new(), created: "".into()
                });
            }
        }
        results.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(results)
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut stdout = String::new();
        let mut stderr = String::new();
        let target = service.map(|s| vec![s.to_string()]).unwrap_or_else(|| self.spec.services.keys().cloned().collect());
        for svc_name in target {
            if let Some(info) = self.find_container_for_service(&svc_name).await? {
                let logs = self.backend.logs(&info.id, tail).await?;
                stdout.push_str(&format!("--- {} ---\n{}", svc_name, logs.stdout));
                stderr.push_str(&format!("--- {} ---\n{}", svc_name, logs.stderr));
            }
        }
        Ok(ContainerLogs { stdout, stderr })
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let info = self.find_container_for_service(service).await?
            .ok_or_else(|| ComposeError::NotFound(service.to_string()))?;
        self.backend.exec(&info.id, cmd, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let target = if services.is_empty() { self.spec.services.keys().cloned().collect() } else { services.to_vec() };
        for svc in target {
            if let Some(info) = self.find_container_for_service(&svc).await? {
                self.backend.start(&info.id).await?;
            }
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let target = if services.is_empty() { self.spec.services.keys().cloned().collect() } else { services.to_vec() };
        for svc in target {
            if let Some(info) = self.find_container_for_service(&svc).await? {
                self.backend.stop(&info.id, None).await?;
            }
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.stop(services).await?;
        self.start(services).await
    }

    pub fn config(&self) -> Result<String> {
        self.spec.to_yaml()
    }

    /// Resolve startup order using Kahn's algorithm.
    pub fn resolve_startup_order(&self) -> Result<Vec<String>> {
        resolve_startup_order(&self.spec)
    }
}

pub struct WorkloadGraphEngine {
    pub backend: Arc<dyn ContainerBackend>,
}

impl WorkloadGraphEngine {
    pub fn new(backend: Arc<dyn ContainerBackend>) -> Self {
        Self { backend }
    }

    pub async fn run(&self, _graph: serde_json::Value, _opts: serde_json::Value) -> Result<u64> {
        Ok(1)
    }
}

pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
    let mut in_degree = IndexMap::new();
    let mut dependents = IndexMap::new();
    for name in spec.services.keys() {
        in_degree.insert(name.clone(), 0);
        dependents.insert(name.clone(), Vec::new());
    }
    for (name, service) in &spec.services {
        if let Some(deps) = &service.depends_on {
            for dep in deps.service_names() {
                if !spec.services.contains_key(&dep) {
                    return Err(ComposeError::validation(format!("Service '{}' depends on undefined '{}'", name, dep)));
                }
                *in_degree.get_mut(name).unwrap() += 1;
                dependents.get_mut(&dep).unwrap().push(name.clone());
            }
        }
    }
    let mut queue: std::collections::BTreeSet<String> = in_degree.iter().filter(|(_, &d)| d == 0).map(|(n, _)| n.clone()).collect();
    let mut order = Vec::new();
    while let Some(service) = queue.pop_first() {
        order.push(service.clone());
        for dependent in dependents.get(&service).unwrap().clone() {
            let deg = in_degree.get_mut(&dependent).unwrap();
            *deg -= 1;
            if *deg == 0 { queue.insert(dependent); }
        }
    }
    if order.len() != spec.services.len() {
        return Err(ComposeError::DependencyCycle { services: in_degree.iter().filter(|(_, &d)| d > 0).map(|(n, _)| n.clone()).collect() });
    }
    Ok(order)
}
