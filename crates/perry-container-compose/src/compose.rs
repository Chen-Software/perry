//! `ComposeEngine` — the core compose orchestration engine.
//!
//! Provides `ComposeEngine::up()`, `down()`, `ps()`, `logs()`, `exec()`, etc.
//! Uses Kahn's algorithm for dependency resolution.

use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig, BuildConfig};
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerSpec, ContainerLogs,
};
use indexmap::IndexMap;
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
    pub(crate) started_containers: std::sync::Mutex<Vec<String>>,
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

        let engine_arc = Arc::new(ComposeEngine::new(
            self.spec.clone(),
            self.project_name.clone(),
            Arc::clone(&self.backend),
        ));

        // Copy over any started containers from this transient engine to the registered one
        {
            let started = self.started_containers.lock().unwrap();
            let mut registered_started = engine_arc.started_containers.lock().unwrap();
            *registered_started = started.clone();
        }

        COMPOSE_ENGINES
            .lock()
            .unwrap()
            .insert(stack_id, engine_arc);
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
        detach: bool,
        build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;

        // Filter to target services
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        let mut created_networks = Vec::new();
        let mut created_volumes = Vec::new();
        let mut session_started = Vec::new();

        // 1. Create networks (skip external)
        if let Some(networks) = &self.spec.networks {
            for (net_name, net_config_opt) in networks {
                let external = net_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let resolved_name = net_config_opt.as_ref()
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(net_name.as_str());
                let config = NetworkConfig {
                    driver: net_config_opt.as_ref().and_then(|c| c.driver.clone()),
                    labels: net_config_opt.as_ref()
                        .and_then(|c| c.labels.as_ref())
                        .map(|l| l.to_map())
                        .unwrap_or_default(),
                    internal: net_config_opt.as_ref().map_or(false, |c| c.internal.unwrap_or(false)),
                    enable_ipv6: net_config_opt.as_ref().map_or(false, |c| c.enable_ipv6.unwrap_or(false)),
                };
                tracing::info!("Creating network '{}'…", resolved_name);
                match self.backend.create_network(resolved_name, &config).await {
                    Ok(_) => created_networks.push(resolved_name.to_string()),
                    Err(e) => {
                        tracing::warn!("Failed to create network '{}': {}", resolved_name, e);
                    }
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
                let resolved_name = vol_config_opt.as_ref()
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(vol_name.as_str());
                let config = VolumeConfig {
                    driver: vol_config_opt.as_ref().and_then(|c| c.driver.clone()),
                    labels: vol_config_opt.as_ref()
                        .and_then(|c| c.labels.as_ref())
                        .map(|l| l.to_map())
                        .unwrap_or_default(),
                };
                tracing::info!("Creating volume '{}'…", resolved_name);
                match self.backend.create_volume(resolved_name, &config).await {
                    Ok(_) => created_volumes.push(resolved_name.to_string()),
                    Err(e) => {
                        tracing::warn!("Failed to create volume '{}': {}", resolved_name, e);
                    }
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

            let res: Result<()> = match self.backend.inspect(&container_name).await {
                Ok(_) => {
                    // Container exists, ensure it is started
                    self.backend.start(&container_name).await.map_err(|e| {
                        ComposeError::ServiceStartupFailed {
                            service: svc_name.clone(),
                            message: e.to_string(),
                        }
                    })
                }
                Err(ComposeError::NotFound(_)) => {
                    // Build if needed
                    if build && svc.needs_build() {
                        if let Some(build_spec) = &svc.build {
                            let build_config = build_spec.as_build();
                            let config = BuildConfig {
                                context: build_config.context.clone().unwrap_or_else(|| ".".into()),
                                dockerfile: build_config.dockerfile.clone(),
                                args: build_config.args.as_ref().map(|l| l.to_map()).unwrap_or_default(),
                                target: build_config.target.clone(),
                                network: build_config.network.clone(),
                            };
                            let tag = svc.image_ref(svc_name);
                            tracing::info!("Building image '{}'…", tag);
                            self.backend.build(&config, &tag).await.map_err(|e| {
                                ComposeError::ServiceStartupFailed {
                                    service: svc_name.clone(),
                                    message: format!("Build failed: {}", e),
                                }
                            })?;
                        }
                    }

                    // Create and start
                    let image = svc.image_ref(svc_name);
                    let mut env = svc.resolved_env();
                    env.insert("COMPOSE_PROJECT".into(), self.project_name.clone());
                    env.insert("COMPOSE_SERVICE".into(), svc_name.clone());

                    let ports = svc.port_strings();
                    let vols = svc.volume_strings();

                    let container_spec = ContainerSpec {
                        image,
                        name: Some(container_name.clone()),
                        ports: if ports.is_empty() { None } else { Some(ports) },
                        volumes: if vols.is_empty() { None } else { Some(vols) },
                        env: if env.is_empty() { None } else { Some(env) },
                        cmd: svc.command_list(),
                        ..Default::default()
                    };

                    if detach {
                        self.backend.run(&container_spec).await.map(|_| ()).map_err(|e| {
                            ComposeError::ServiceStartupFailed {
                                service: svc_name.clone(),
                                message: e.to_string(),
                            }
                        })
                    } else {
                        match self.backend.create(&container_spec).await {
                            Ok(_) => self.backend.start(&container_name).await.map_err(|e| {
                                ComposeError::ServiceStartupFailed {
                                    service: svc_name.clone(),
                                    message: e.to_string(),
                                }
                            }),
                            Err(e) => Err(ComposeError::ServiceStartupFailed {
                                service: svc_name.clone(),
                                message: e.to_string(),
                            }),
                        }
                    }
                }
                Err(e) => Err(e),
            };

            match res {
                Ok(_) => session_started.push(container_name.clone()),
                Err(e) => {
                    // ROLLBACK: stop and remove all containers started in THIS session in reverse order
                    for name in session_started.iter().rev() {
                        let _ = self.backend.stop(name, Some(10)).await;
                        let _ = self.backend.remove(name, true).await;
                    }
                    // Best effort remove created networks
                    for net in created_networks.iter().rev() {
                        let _ = self.backend.remove_network(net).await;
                    }
                    // Best effort remove created volumes
                    for vol in created_volumes.iter().rev() {
                        let _ = self.backend.remove_volume(vol).await;
                    }
                    return Err(e);
                }
            }
        }

        // Record started containers
        self.started_containers.lock().unwrap().extend(session_started);

        // Register and return handle
        Ok(self.register())
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
        for svc_name in target {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;

            let container_name = service::service_container_name(svc, svc_name);
            if let Ok(info) = self.backend.inspect(&container_name).await {
                let status = info.status.to_lowercase();
                if status.contains("running") || status.contains("up") {
                    let _ = self.backend.stop(&container_name, None).await;
                }
                let _ = self.backend.remove(&container_name, true).await;
            }
        }

        // 2. Remove networks (non-external, idempotent)
        if let Some(networks) = &self.spec.networks {
            for (net_name, net_config_opt) in networks {
                let external = net_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                if external {
                    continue;
                }
                let resolved_name = net_config_opt.as_ref()
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(net_name.as_str());
                let _ = self.backend.remove_network(resolved_name).await;
            }
        }

        // 3. Remove volumes (if requested)
        if remove_volumes {
            if let Some(volumes) = &self.spec.volumes {
                for (vol_name, vol_config_opt) in volumes {
                    let external = vol_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                    if external {
                        continue;
                    }
                    let resolved_name = vol_config_opt.as_ref()
                        .and_then(|c| c.name.as_deref())
                        .unwrap_or(vol_name.as_str());
                    let _ = self.backend.remove_volume(resolved_name).await;
                }
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
            match self.backend.inspect(&container_name).await {
                Ok(info) => results.push(info),
                Err(ComposeError::NotFound(_)) => {
                    results.push(ContainerInfo {
                        id: "".into(),
                        name: container_name,
                        image: svc.image_ref(svc_name),
                        status: "not found".into(),
                        ports: vec![],
                        created: "".into(),
                    });
                }
                Err(e) => return Err(e),
            }
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
    ) -> Result<ContainerLogs> {
        let service_names: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };

        let mut all_stdout = String::new();
        let mut all_stderr = String::new();

        for svc_name in service_names {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;

            let container_name = service::service_container_name(svc, svc_name);
            if let Ok(logs) = self.backend.logs(&container_name, tail).await {
                if !logs.stdout.is_empty() {
                    all_stdout.push_str(&format!("--- {} stdout ---\n", svc_name));
                    all_stdout.push_str(&logs.stdout);
                }
                if !logs.stderr.is_empty() {
                    all_stderr.push_str(&format!("--- {} stderr ---\n", svc_name));
                    all_stderr.push_str(&logs.stderr);
                }
            }
        }

        Ok(ContainerLogs {
            stdout: all_stdout,
            stderr: all_stderr,
            exit_code: 0,
        })
    }

    // ============ exec ============

    /// Execute a command in a running service container.
    pub async fn exec(
        &self,
        service: &str,
        cmd: &[String],
    ) -> Result<ContainerLogs> {
        let svc = self
            .spec
            .services
            .get(service)
            .ok_or_else(|| ComposeError::NotFound(service.to_owned()))?;

        let container_name = service::service_container_name(svc, service);
        let info = self.backend.inspect(&container_name).await?;
        let status = info.status.to_lowercase();

        if !status.contains("running") && !status.contains("up") {
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

// ============ Dependency resolution (Kahn's algorithm) ============

/// Resolve the startup order of services using Kahn's algorithm (BFS topological sort).
///
/// Returns services in dependency order. If a cycle is detected, returns
/// `ComposeError::DependencyCycle` listing all services in the cycle.
pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
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

    // 3. Process queue
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

    // 4. If not all services processed → cycle detected
    if order.len() != spec.services.len() {
        let cycle_services: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg > 0)
            .map(|(name, _)| name.clone())
            .collect();
        return Err(ComposeError::DependencyCycle {
            services: cycle_services,
        });
    }

    Ok(order)
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
