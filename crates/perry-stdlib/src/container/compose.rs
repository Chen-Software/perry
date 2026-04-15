//! ComposeWrapper — thin orchestration adapter over `ContainerBackend`.
//!
//! Wraps individual `ContainerBackend` calls into compose workflows
//! (up/down/ps/logs/exec) with dependency-ordered service startup and
//! rollback on failure.
//!
//! Uses `perry_container_compose::compose::resolve_startup_order` for
//! Kahn's algorithm–based topological sort.

use super::backend::ContainerBackend;
use super::types::{
    ComposeDependsOnEntry, ComposeHandle, ComposeNetwork, ComposePortEntry, ComposeService,
    ComposeServiceNetworks, ComposeSpec, ComposeVolume, ComposeVolumeEntry, ContainerError,
    ContainerHandle, ContainerSpec, ListOrDict,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Thin compose orchestration wrapper over `ContainerBackend`.
///
/// This is **not** the full `perry_container_compose::ComposeEngine`
/// (which has its own type system based on `serde_yaml` + `IndexMap`).
/// Instead, it orchestrates the stdlib's `ContainerBackend` calls with
/// compose-spec semantics (dependency order, rollback, etc.).
pub struct ComposeWrapper {
    spec: ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
}

impl ComposeWrapper {
    /// Create a new ComposeWrapper.
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { spec, backend }
    }

    /// Bring up the compose stack.
    ///
    /// Creates networks and volumes first, then starts containers in
    /// dependency order. On failure, rolls back all previously started
    /// containers and created resources.
    pub async fn up(&self) -> Result<ComposeHandle, ContainerError> {
        // 1. Validate dependency graph via compose crate's Kahn's algorithm
        let startup_order = self.resolve_startup_order()?;

        // 2. Create networks (skip external)
        let mut created_networks = Vec::new();
        if let Some(networks) = &self.spec.networks {
            for (name, network_opt) in networks {
                if let Some(network) = network_opt {
                    if network.external.unwrap_or(false) {
                        continue;
                    }
                }
                let resolved_name = network_opt
                    .as_ref()
                    .and_then(|n| n.name.as_deref())
                    .unwrap_or(name.as_str());
                let config = network_opt
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(ComposeNetwork::default);
                self.backend
                    .create_network(resolved_name, &config)
                    .await?;
                created_networks.push(resolved_name.to_string());
            }
        }

        // 3. Create volumes (skip external)
        let mut created_volumes = Vec::new();
        if let Some(volumes) = &self.spec.volumes {
            for (name, volume_opt) in volumes {
                if let Some(volume) = volume_opt {
                    if volume.external.unwrap_or(false) {
                        continue;
                    }
                }
                let resolved_name = volume_opt
                    .as_ref()
                    .and_then(|v| v.name.as_deref())
                    .unwrap_or(name.as_str());
                let config = volume_opt
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(ComposeVolume::default);
                self.backend
                    .create_volume(resolved_name, &config)
                    .await?;
                created_volumes.push(resolved_name.to_string());
            }
        }

        // 4. Start services in dependency order
        let mut started_containers = HashMap::new();
        let mut started_services = Vec::new();

        for service_name in &startup_order {
            if let Some(service) = self.spec.services.get(service_name) {
                match self.start_service(service_name, service).await {
                    Ok(handle) => {
                        started_containers.insert(service_name.clone(), handle);
                        started_services.push(service_name.clone());
                    }
                    Err(e) => {
                        // Rollback: stop and remove all started containers
                        for (name, handle) in &started_containers {
                            let _ = self.backend.stop(&handle.id, Some(10)).await;
                            let _ = self.backend.remove(&handle.id, true).await;
                        }
                        // Remove created networks and volumes
                        for network in &created_networks {
                            let _ = self.backend.remove_network(network).await;
                        }
                        for volume in &created_volumes {
                            let _ = self.backend.remove_volume(volume).await;
                        }
                        return Err(ContainerError::ServiceStartupFailed {
                            service: service_name.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            }
        }

        Ok(ComposeHandle {
            name: self
                .spec
                .name
                .clone()
                .unwrap_or_else(|| "perry-compose-stack".to_string()),
            services: started_services,
            networks: created_networks,
            volumes: created_volumes,
            containers: started_containers,
        })
    }

    /// Resolve service startup order using the compose crate's Kahn's algorithm.
    ///
    /// This delegates to `perry_container_compose::compose::resolve_startup_order`
    /// after converting the stdlib `ComposeSpec` to the compose crate's type.
    /// Falls back to local DFS if the conversion fails (e.g. incompatible values).
    fn resolve_startup_order(&self) -> Result<Vec<String>, ContainerError> {
        // Attempt to use compose crate's Kahn's algorithm via JSON round-trip.
        // The compose crate's ComposeSpec uses serde_yaml, but both types
        // are (de)serializable, so we can go through JSON as a common format.
        if let Ok(compose_spec) = spec_to_compose(&self.spec) {
            return perry_container_compose::compose::resolve_startup_order(&compose_spec)
                .map_err(|e| ContainerError::DependencyCycle {
                    cycle: match e {
                        perry_container_compose::error::ComposeError::DependencyCycle { services } => services,
                        _ => vec![],
                    },
                });
        }

        // Fallback: local DFS topological sort
        self.resolve_startup_order_dfs()
    }

    /// DFS-based topological sort (fallback).
    fn resolve_startup_order_dfs(&self) -> Result<Vec<String>, ContainerError> {
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        let mut order = Vec::new();

        for service_name in self.spec.services.keys() {
            if !visited.contains(service_name) {
                self.visit(service_name, &mut visited, &mut visiting, &mut order)?;
            }
        }

        Ok(order)
    }

    /// DFS visit for topological sort.
    fn visit(
        &self,
        service: &str,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) -> Result<(), ContainerError> {
        if visited.contains(service) {
            return Ok(());
        }

        if visiting.contains(service) {
            return Err(ContainerError::DependencyCycle {
                cycle: visiting
                    .iter()
                    .cloned()
                    .chain(std::iter::once(service.to_string()))
                    .collect(),
            });
        }

        visiting.insert(service.to_string());

        if let Some(service_spec) = self.spec.services.get(service) {
            if let Some(deps) = &service_spec.depends_on {
                for dep in deps.service_names() {
                    if self.spec.services.contains_key(&dep) {
                        self.visit(&dep, visited, visiting, order)?;
                    }
                }
            }
        }

        visiting.remove(service);
        visited.insert(service.to_string());
        order.push(service.to_string());

        Ok(())
    }

    /// Start a single service.
    async fn start_service(
        &self,
        name: &str,
        service: &ComposeService,
    ) -> Result<ContainerHandle, ContainerError> {
        // Build support - check early
        if service.build.is_some() {
            return Err(ContainerError::InvalidConfig(
                "Build configuration not yet supported".to_string(),
            ));
        }

        // Resolve image (required when no build)
        let image = service
            .image
            .clone()
            .ok_or_else(|| ContainerError::InvalidConfig(format!(
                "Service '{}' has no image or build configuration",
                name
            )))?;

        // ── Environment: ListOrDict → HashMap<String, String> ──
        let env: Option<HashMap<String, String>> = service
            .environment
            .as_ref()
            .map(|e| e.to_map())
            .filter(|m| !m.is_empty());

        // ── Command: serde_json::Value → Option<Vec<String>> ──
        let cmd: Option<Vec<String>> = service.command.as_ref().and_then(|v| {
            match v {
                serde_json::Value::String(s) => Some(vec![s.clone()]),
                serde_json::Value::Array(arr) => {
                    let strs: Option<Vec<String>> =
                        arr.iter().map(|item| item.as_str().map(String::from)).collect();
                    strs.filter(|v| !v.is_empty())
                }
                _ => None,
            }
        });

        // ── Entrypoint: same shape as command ──
        let entrypoint: Option<Vec<String>> = service.entrypoint.as_ref().and_then(|v| {
            match v {
                serde_json::Value::String(s) => Some(vec![s.clone()]),
                serde_json::Value::Array(arr) => {
                    let strs: Option<Vec<String>> =
                        arr.iter().map(|item| item.as_str().map(String::from)).collect();
                    strs.filter(|v| !v.is_empty())
                }
                _ => None,
            }
        });

        // ── Network: ComposeServiceNetworks → Option<String> ──
        let network: Option<String> = service.networks.as_ref().and_then(|n| match n {
            ComposeServiceNetworks::List(names) => names.first().cloned(),
            ComposeServiceNetworks::Map(map) => map.keys().next().cloned(),
        });

        // ── Ports: Vec<ComposePortEntry> → Vec<String> ──
        let ports: Option<Vec<String>> = service.ports.as_ref().map(|entries| {
            entries
                .iter()
                .map(|entry| match entry {
                    ComposePortEntry::Short(v) => v.to_string(),
                    ComposePortEntry::Long(p) => {
                        let published = p
                            .published
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or_default();
                        let target = p.target.to_string();
                        let protocol = p
                            .protocol
                            .as_deref()
                            .unwrap_or("tcp");
                        if published.is_empty() {
                            target
                        } else {
                            format!("{}:{}/{}", published, target, protocol)
                        }
                    }
                })
                .collect()
        });

        // ── Volumes: Vec<ComposeVolumeEntry> → Vec<String> ──
        let volumes: Option<Vec<String>> = service.volumes.as_ref().map(|entries| {
            entries
                .iter()
                .map(|entry| match entry {
                    ComposeVolumeEntry::Short(s) => s.clone(),
                    ComposeVolumeEntry::Long(v) => {
                        let source = v.source.as_deref().unwrap_or("");
                        let target = v.target.as_deref().unwrap_or("");
                        let ro = if v.read_only.unwrap_or(false) {
                            ":ro"
                        } else {
                            ""
                        };
                        format!("{}:{}{}", source, target, ro)
                    }
                })
                .collect()
        });

        // ── Container name ──
        let container_name = service
            .container_name
            .clone()
            .unwrap_or_else(|| format!("{}_{}", name, std::process::id()));

        let spec = ContainerSpec {
            image,
            name: Some(container_name),
            ports,
            volumes,
            env,
            cmd,
            entrypoint,
            network,
            rm: Some(true),
        };

        self.backend.run(&spec).await
    }

    /// Stop and remove all resources in the compose stack.
    pub async fn down(
        &self,
        handle: &ComposeHandle,
        remove_volumes: bool,
    ) -> Result<(), ContainerError> {
        for (name, container) in &handle.containers {
            let _ = self.backend.stop(&container.id, Some(10)).await;
            let _ = self.backend.remove(&container.id, true).await;
            eprintln!("[perry-compose] Stopped and removed service: {}", name);
        }

        for network in &handle.networks {
            let _ = self.backend.remove_network(network).await;
        }

        if remove_volumes {
            for volume in &handle.volumes {
                let _ = self.backend.remove_volume(volume).await;
            }
        }

        Ok(())
    }

    /// Get container info for all services in the stack.
    pub async fn ps(
        &self,
        handle: &ComposeHandle,
    ) -> Result<Vec<super::types::ContainerInfo>, ContainerError> {
        let mut result = Vec::new();

        for container in handle.containers.values() {
            match self.backend.inspect(&container.id).await {
                Ok(info) => result.push(info),
                Err(_) => continue,
            }
        }

        Ok(result)
    }

    /// Get logs for a specific service (or all services).
    pub async fn logs(
        &self,
        handle: &ComposeHandle,
        service: Option<&str>,
        tail: Option<u32>,
    ) -> Result<super::types::ContainerLogs, ContainerError> {
        if let Some(service_name) = service {
            if let Some(container) = handle.containers.get(service_name) {
                return self.backend.logs(&container.id, tail).await;
            }
            return Err(ContainerError::NotFound(format!(
                "Service not found: {}",
                service_name
            )));
        }

        let mut combined_stdout = String::new();
        let mut combined_stderr = String::new();

        for (name, container) in &handle.containers {
            match self.backend.logs(&container.id, tail).await {
                Ok(logs) => {
                    combined_stdout.push_str(&format!("=== {} ===\n{}\n", name, logs.stdout));
                    combined_stderr.push_str(&format!("=== {} ===\n{}\n", name, logs.stderr));
                }
                Err(_) => continue,
            }
        }

        Ok(super::types::ContainerLogs {
            stdout: combined_stdout,
            stderr: combined_stderr,
        })
    }

    /// Execute a command in a service container.
    pub async fn exec(
        &self,
        handle: &ComposeHandle,
        service: &str,
        cmd: &[String],
    ) -> Result<super::types::ContainerLogs, ContainerError> {
        if let Some(container) = handle.containers.get(service) {
            self.backend.exec(&container.id, cmd, None, None).await
        } else {
            Err(ContainerError::NotFound(format!(
                "Service not found: {}",
                service
            )))
        }
    }
}

// ─── Spec conversion helpers ─────────────────────────────────────────────────

/// Attempt to convert a stdlib `ComposeSpec` to the compose crate's type
/// via JSON round-trip. This works because both types are (de)serializable
/// with serde.
fn spec_to_compose(
    spec: &ComposeSpec,
) -> Result<perry_container_compose::types::ComposeSpec, serde_json::Error> {
    let json = serde_json::to_value(spec)?;
    serde_json::from_value(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_to_compose_basic() {
        let mut spec = ComposeSpec::default();
        spec.name = Some("test-stack".to_string());

        let mut svc = ComposeService::default();
        svc.image = Some("nginx:latest".to_string());
        spec.services.insert("web".to_string(), svc);

        let result = spec_to_compose(&spec).unwrap();
        assert_eq!(result.name.as_deref(), Some("test-stack"));
        assert!(result.services.contains_key("web"));
    }

    #[test]
    fn test_spec_to_compose_with_depends_on() {
        let mut spec = ComposeSpec::default();

        let mut db = ComposeService::default();
        db.image = Some("postgres:16".to_string());
        spec.services.insert("db".to_string(), db);

        let mut web = ComposeService::default();
        web.image = Some("nginx:latest".to_string());
        web.depends_on = Some(ComposeDependsOnEntry::List(vec![
            "db".to_string(),
        ]));
        spec.services.insert("web".to_string(), web);

        let result = spec_to_compose(&spec).unwrap();
        assert_eq!(result.services.len(), 2);
        let web_svc = &result.services["web"];
        assert!(web_svc.depends_on.is_some());
    }

    #[test]
    fn test_spec_to_compose_with_env_list() {
        let mut spec = ComposeSpec::default();

        let mut svc = ComposeService::default();
        svc.image = Some("redis:7".to_string());
        svc.environment = Some(ListOrDict::List(vec![
            "REDIS_HOST=localhost".to_string(),
            "REDIS_PORT=6379".to_string(),
        ]));
        spec.services.insert("cache".to_string(), svc);

        let result = spec_to_compose(&spec).unwrap();
        let cache_svc = &result.services["cache"];
        assert!(cache_svc.environment.is_some());
    }

    #[test]
    fn test_spec_to_compose_preserves_networks() {
        let mut spec = ComposeSpec::default();

        let mut net = HashMap::new();
        net.insert("frontend".to_string(), None);
        spec.networks = Some(net);

        let result = spec_to_compose(&spec).unwrap();
        assert!(result.networks.is_some());
    }
}
