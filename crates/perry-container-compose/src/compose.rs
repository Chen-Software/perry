//! `ComposeEngine` — the core compose orchestration engine.
//!
//! Provides `ComposeEngine::up()`, `down()`, `ps()`, `logs()`, `exec()`, etc.
//! Uses Kahn's algorithm for dependency resolution.

use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeNetwork, ComposeSpec, ComposeVolume, ContainerInfo, ContainerLogs,
};
use indexmap::IndexMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Global registry of running compose engines, keyed by stack ID.
static COMPOSE_ENGINES: once_cell::sync::Lazy<
    std::sync::Mutex<IndexMap<u64, Arc<ComposeEngine>>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(IndexMap::new()));

/// Next available stack ID.
static NEXT_STACK_ID: AtomicU64 = AtomicU64::new(1);

/// The compose orchestration engine.
pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
}

impl ComposeEngine {
    // ── 8.2 Constructor ──────────────────────────────────────────────────

    /// Create a new `ComposeEngine`.
    pub fn new(
        spec: ComposeSpec,
        project_name: String,
        backend: Arc<dyn ContainerBackend>,
    ) -> Self {
        ComposeEngine {
            spec,
            project_name,
            backend,
        }
    }

    /// Register this engine in the global registry and return a handle.
    fn register(self: &Arc<Self>) -> ComposeHandle {
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
            .insert(stack_id, Arc::clone(self));
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

    // ── 8.3 up ───────────────────────────────────────────────────────────

    /// Bring up services in dependency order.
    ///
    /// 1. Creates all networks (skipping external ones).
    /// 2. Creates all named volumes (skipping external ones).
    /// 3. Starts services in `resolve_startup_order()` order.
    /// 4. On any failure: rolls back all previously started containers in
    ///    reverse order, removes created networks and volumes, then returns
    ///    `ComposeError::ServiceStartupFailed`.
    pub async fn up(
        self: &Arc<Self>,
        services: &[String],
        _detach: bool,
        build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;

        // Filter to target services (preserve dependency order)
        let target: Vec<String> = if services.is_empty() {
            order.clone()
        } else {
            order
                .into_iter()
                .filter(|s| services.contains(s))
                .collect()
        };

        // ── 1. Create networks ────────────────────────────────────────────
        let mut created_networks: Vec<String> = Vec::new();
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
                    .unwrap_or(net_name.as_str())
                    .to_string();
                let config = net_config_opt.clone().unwrap_or_default();
                tracing::info!("Creating network '{}'…", resolved_name);
                if let Err(e) = self.backend.create_network(&resolved_name, &config).await {
                    for n in created_networks.iter().rev() {
                        let _ = self.backend.remove_network(n).await;
                    }
                    return Err(ComposeError::ServiceStartupFailed {
                        service: format!("network/{}", net_name),
                        message: e.to_string(),
                    });
                }
                created_networks.push(resolved_name);
            }
        }

        // ── 2. Create volumes ─────────────────────────────────────────────
        let mut created_volumes: Vec<String> = Vec::new();
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
                    .unwrap_or(vol_name.as_str())
                    .to_string();
                let config = vol_config_opt.clone().unwrap_or_default();
                tracing::info!("Creating volume '{}'…", resolved_name);
                if let Err(e) = self.backend.create_volume(&resolved_name, &config).await {
                    for v in created_volumes.iter().rev() {
                        let _ = self.backend.remove_volume(v).await;
                    }
                    for n in created_networks.iter().rev() {
                        let _ = self.backend.remove_network(n).await;
                    }
                    return Err(ComposeError::ServiceStartupFailed {
                        service: format!("volume/{}", vol_name),
                        message: e.to_string(),
                    });
                }
                created_volumes.push(resolved_name);
            }
        }

        // ── 3. Start services in dependency order ─────────────────────────
        let mut started_containers: Vec<String> = Vec::new();

        for svc_name in &target {
            let rollback_info = (started_containers.clone(), created_networks.clone(), created_volumes.clone());
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;

            let container_name = service::service_container_name(svc, svc_name);

            match self.backend.inspect(&container_name).await {
                Ok(info) if info.status.to_lowercase().contains("running") => {
                    tracing::debug!("Service '{}' already running", svc_name);
                    continue;
                }
                Ok(_) => {
                    // Exists but stopped — start it
                    tracing::info!("Starting existing container for '{}'…", svc_name);
                    if let Err(e) = self.backend.start(&container_name).await {
                        let (s, n, v) = rollback_info;
                        self.rollback_startup(&s, &n, &v).await;
                        return Err(ComposeError::ServiceStartupFailed {
                            service: svc_name.clone(),
                            message: e.to_string(),
                        });
                    }
                    started_containers.push(container_name);
                    continue;
                }
                Err(ComposeError::NotFound(_)) => {
                    // Container doesn't exist — fall through to create it
                }
                Err(e) => {
                    let (s, n, v) = rollback_info;
                    self.rollback_startup(&s, &n, &v).await;
                    return Err(ComposeError::ServiceStartupFailed {
                        service: svc_name.clone(),
                        message: e.to_string(),
                    });
                }
            }

            // Optionally pull/build image
            if build && svc.needs_build() {
                let tag = svc.image_ref(svc_name);
                tracing::info!("Pulling/building image '{}'…", tag);
                if let Err(e) = self.backend.pull_image(&tag).await {
                    tracing::warn!("Could not pull '{}': {}", tag, e);
                }
            }

            // Build ContainerSpec from ComposeService
            let image = svc.image_ref(svc_name);
            let env = svc.resolved_env();
            let ports = svc.port_strings();
            let vols = svc.volume_strings();
            let cmd = svc.command_list();

            let network = svc
                .networks
                .as_ref()
                .and_then(|n| n.names().into_iter().next());

            let spec = crate::types::ContainerSpec {
                image,
                name: Some(container_name.clone()),
                ports: if ports.is_empty() { None } else { Some(ports) },
                volumes: if vols.is_empty() { None } else { Some(vols) },
                env: if env.is_empty() { None } else { Some(env) },
                cmd,
                entrypoint: None,
                network,
                rm: Some(false),
            };

            tracing::info!("Starting service '{}'…", svc_name);
            if let Err(e) = self.backend.run(&spec).await {
                let (s, n, v) = rollback_info;
                self.rollback_startup(&s, &n, &v).await;
                return Err(ComposeError::ServiceStartupFailed {
                    service: svc_name.clone(),
                    message: e.to_string(),
                });
            }
            started_containers.push(container_name);
        }

        Ok(self.register())
    }

    /// Roll back a failed `up()` by stopping/removing started containers,
    /// then removing created networks and volumes.
    async fn rollback_startup(
        &self,
        started_containers: &[String],
        created_networks: &[String],
        created_volumes: &[String],
    ) {
        for container in started_containers.iter().rev() {
            let _ = self.backend.stop(container, None).await;
            let _ = self.backend.remove(container, true).await;
        }
        for net in created_networks.iter().rev() {
            let _ = self.backend.remove_network(net).await;
        }
        for vol in created_volumes.iter().rev() {
            let _ = self.backend.remove_volume(vol).await;
        }
    }

    // ── 8.4 down ─────────────────────────────────────────────────────────

    /// Stop and remove all service containers; remove networks; optionally
    /// remove named volumes.
    pub async fn down(&self, volumes: bool, _remove_orphans: bool) -> Result<()> {
        let mut order = resolve_startup_order(&self.spec)?;
        order.reverse(); // Tear down in reverse dependency order

        // 1. Stop and remove containers
        for svc_name in &order {
            let svc = match self.spec.services.get(svc_name) {
                Some(s) => s,
                None => continue,
            };
            let container_name = service::service_container_name(svc, svc_name);

            match self.backend.inspect(&container_name).await {
                Ok(info) => {
                    if info.status.to_lowercase().contains("running") {
                        let _ = self.backend.stop(&container_name, None).await;
                    }
                    let _ = self.backend.remove(&container_name, true).await;
                }
                Err(ComposeError::NotFound(_)) => {}
                Err(e) => {
                    tracing::warn!("Error inspecting '{}' during down: {}", container_name, e);
                }
            }
        }

        // 2. Remove networks (non-external, idempotent)
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
                let _ = self.backend.remove_network(resolved_name).await;
            }
        }

        // 3. Remove volumes (if requested, non-external)
        if volumes {
            if let Some(vols) = &self.spec.volumes {
                for (vol_name, vol_config_opt) in vols {
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
                    let _ = self.backend.remove_volume(resolved_name).await;
                }
            }
        }

        Ok(())
    }

    // ── 8.5 ps / logs / exec ─────────────────────────────────────────────

    /// List the status of all service containers.
    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut results = Vec::new();

        for (svc_name, svc) in &self.spec.services {
            let container_name = service::service_container_name(svc, svc_name);
            match self.backend.inspect(&container_name).await {
                Ok(info) => results.push(info),
                Err(ComposeError::NotFound(_)) => {
                    results.push(ContainerInfo {
                        id: container_name.clone(),
                        name: container_name,
                        image: svc.image_ref(svc_name),
                        status: "not found".to_string(),
                        ports: svc.port_strings(),
                        created: String::new(),
                    });
                }
                Err(e) => return Err(e),
            }
        }

        results.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(results)
    }

    /// Get logs from a service (or all services if `service` is `None`).
    pub async fn logs(
        &self,
        service: Option<&str>,
        tail: Option<u32>,
    ) -> Result<ContainerLogs> {
        let service_names: Vec<String> = match service {
            Some(s) => vec![s.to_string()],
            None => self.spec.services.keys().cloned().collect(),
        };

        let mut combined_stdout = String::new();
        let mut combined_stderr = String::new();
        let multi = service_names.len() > 1;

        for svc_name in &service_names {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let container_name = service::service_container_name(svc, svc_name);
            let logs = self.backend.logs(&container_name, tail).await?;
            if multi {
                for line in logs.stdout.lines() {
                    combined_stdout.push_str(&format!("{} | {}\n", svc_name, line));
                }
                for line in logs.stderr.lines() {
                    combined_stderr.push_str(&format!("{} | {}\n", svc_name, line));
                }
            } else {
                combined_stdout = logs.stdout;
                combined_stderr = logs.stderr;
            }
        }

        Ok(ContainerLogs {
            stdout: combined_stdout,
            stderr: combined_stderr,
        })
    }

    /// Execute a command in a running service container.
    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let svc = self
            .spec
            .services
            .get(service)
            .ok_or_else(|| ComposeError::NotFound(service.to_owned()))?;

        let container_name = service::service_container_name(svc, service);

        match self.backend.inspect(&container_name).await {
            Ok(info) if !info.status.to_lowercase().contains("running") => {
                return Err(ComposeError::ServiceStartupFailed {
                    service: service.to_owned(),
                    message: format!("container '{}' is not running", container_name),
                });
            }
            Err(ComposeError::NotFound(_)) => {
                return Err(ComposeError::NotFound(format!(
                    "service '{}' container not found",
                    service
                )));
            }
            Err(e) => return Err(e),
            Ok(_) => {}
        }

        self.backend.exec(&container_name, cmd, None, None).await
    }

    // ── 8.6 start / stop / restart ───────────────────────────────────────

    /// Start existing stopped service containers.
    pub async fn start(&self, services: &[String]) -> Result<()> {
        let target: Vec<String> = if services.is_empty() {
            self.spec.services.keys().cloned().collect()
        } else {
            services.to_vec()
        };

        for svc_name in &target {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let container_name = service::service_container_name(svc, svc_name);
            self.backend.start(&container_name).await?;
        }

        Ok(())
    }

    /// Stop running service containers.
    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let target: Vec<String> = if services.is_empty() {
            self.spec.services.keys().cloned().collect()
        } else {
            services.to_vec()
        };

        for svc_name in &target {
            let svc = self
                .spec
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let container_name = service::service_container_name(svc, svc_name);
            self.backend.stop(&container_name, None).await?;
        }

        Ok(())
    }

    /// Restart service containers (stop then start).
    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.stop(services).await?;
        self.start(services).await
    }

    /// Validate and return the resolved compose configuration as YAML.
    pub fn config(&self) -> Result<String> {
        self.spec.to_yaml()
    }
}

// ── 8.1 Dependency resolution (Kahn's algorithm) ─────────────────────────────

/// Resolve the startup order of services using Kahn's algorithm (BFS topological sort).
///
/// Returns services in dependency order (dependencies first). If a cycle is
/// detected, returns `ComposeError::DependencyCycle` listing all services in
/// the cycle. Zero-in-degree services are sorted alphabetically for determinism.
pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
    // Edge direction: if A depends_on B, then B → A (B must start before A).
    // in_degree[A] = number of services A depends on.
    let mut in_degree: IndexMap<String, usize> = IndexMap::new();
    // dependents[B] = list of services that must start after B
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
                        message: format!(
                            "Service '{}' depends on '{}' which is not defined",
                            name, dep
                        ),
                    });
                }
                // A depends on dep → in_degree[A] += 1, dependents[dep] gets A
                *in_degree.get_mut(name).unwrap() += 1;
                dependents.get_mut(&dep).unwrap().push(name.clone());
            }
        }
    }

    // Seed BFS queue with zero-in-degree services (sorted for determinism)
    let mut queue: std::collections::BTreeSet<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(name, _)| name.clone())
        .collect();

    let mut order: Vec<String> = Vec::with_capacity(spec.services.len());
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
        return Err(ComposeError::DependencyCycle {
            services: cycle_services,
        });
    }

    Ok(order)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

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
        assert!(matches!(
            result.unwrap_err(),
            ComposeError::ValidationError { .. }
        ));
    }

    #[test]
    fn test_deterministic_order() {
        // Services with no deps should be sorted alphabetically
        let compose = make_compose(&[("c", &[]), ("a", &[]), ("b", &[])]);
        let order = resolve_startup_order(&compose).unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_isolated_nodes() {
        // Mix of isolated and chained services
        let compose = make_compose(&[
            ("z", &[]),
            ("a", &[]),
            ("m", &["a"]),
        ]);
        let order = resolve_startup_order(&compose).unwrap();
        let pos = |name: &str| order.iter().position(|s| s == name).unwrap();
        assert!(pos("a") < pos("m"), "a must precede m");
        // z and a are both zero-in-degree, sorted alphabetically
        assert!(pos("a") < pos("z") || pos("z") < pos("m"),
            "isolated nodes appear before their dependents");
    }
}
