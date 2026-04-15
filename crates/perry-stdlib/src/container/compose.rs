//! ComposeEngine implementation
//!
//! Provides native multi-container orchestration without external CLI tools.

use super::backend::ContainerBackend;
use super::types::{
    ComposeHandle, ComposeNetwork, ComposeService, ComposeSpec, ComposeVolume, ContainerError,
    ContainerHandle,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// ComposeEngine for orchestrating multi-container applications
pub struct ComposeEngine {
    spec: ComposeSpec,
    backend: Arc<dyn ContainerBackend>,
}

impl ComposeEngine {
    /// Create a new ComposeEngine
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { spec, backend }
    }

    /// Bring up the compose stack
    pub async fn up(&self) -> Result<ComposeHandle, ContainerError> {
        // 1. Validate dependency graph
        let startup_order = self.resolve_startup_order()?;

        // 2. Create networks
        let mut created_networks = Vec::new();
        if let Some(networks) = &self.spec.networks {
            for (name, _network) in networks {
                self.create_network(name).await?;
                created_networks.push(name.clone());
            }
        }

        // 3. Create volumes
        let mut created_volumes = Vec::new();
        if let Some(volumes) = &self.spec.volumes {
            for (name, _volume) in volumes {
                self.create_volume(name).await?;
                created_volumes.push(name.clone());
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
                            let _ = self.backend.stop(&handle.id, 10).await;
                            let _ = self.backend.remove(&handle.id, true).await;
                        }
                        // Remove created networks and volumes
                        for network in &created_networks {
                            let _ = self.remove_network(network).await;
                        }
                        for volume in &created_volumes {
                            let _ = self.remove_volume(volume).await;
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
            name: "perry-compose-stack".to_string(),
            services: started_services,
            networks: created_networks,
            volumes: created_volumes,
            containers: started_containers,
        })
    }

    /// Resolve service startup order based on dependencies
    fn resolve_startup_order(&self) -> Result<Vec<String>, ContainerError> {
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

    /// DFS visit for topological sort
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
            // Cycle detected
            return Err(ContainerError::DependencyCycle {
                cycle: visiting.iter().cloned().chain([service.to_string()]).collect(),
            });
        }

        visiting.insert(service.to_string());

        // Visit dependencies
        if let Some(service_spec) = self.spec.services.get(service) {
            if let Some(deps) = &service_spec.depends_on {
                for dep in deps {
                    if self.spec.services.contains_key(dep) {
                        self.visit(dep, visited, visiting, order)?;
                    }
                }
            }
        }

        visiting.remove(service);
        visited.insert(service.to_string());
        order.push(service.to_string());

        Ok(())
    }

    /// Start a single service
    async fn start_service(
        &self,
        name: &str,
        service: &ComposeService,
    ) -> Result<ContainerHandle, ContainerError> {
        use super::types::ContainerSpec;

        // Convert ComposeService to ContainerSpec
        let mut env = HashMap::new();
        if let Some(compose_env) = &service.environment {
            match compose_env {
                super::types::ComposeEnvironment::Map(map) => {
                    env = map.clone();
                }
                super::types::ComposeEnvironment::Array(arr) => {
                    for item in arr {
                        if let Some((k, v)) = item.split_once('=') {
                            env.insert(k.to_string(), v.to_string());
                        }
                    }
                }
            }
        }

        let mut cmd = None;
        if let Some(command) = &service.command {
            match command {
                super::types::ComposeCommand::String(s) => {
                    cmd = Some(vec![s.clone()]);
                }
                super::types::ComposeCommand::Array(arr) => {
                    cmd = Some(arr.clone());
                }
            }
        }

        let mut network = None;
        if let Some(networks) = &service.networks {
            network = networks.first().cloned();
        }

        let spec = ContainerSpec {
            image: service.image.clone(),
            name: Some(format!("{}_{}", name, std::process::id())),
            ports: service.ports.clone(),
            volumes: service.volumes.clone(),
            env: if env.is_empty() { None } else { Some(env) },
            cmd,
            entrypoint: None, // TODO: add support
            network,
            rm: Some(true), // Remove on exit by default for compose
        };

        // Build support - TODO: implement in future task
        if service.build.is_some() {
            return Err(ContainerError::InvalidConfig(
                "Build configuration not yet supported".to_string(),
            ));
        }

        // Run the container
        self.backend.run(&spec).await
    }

    /// Create a network
    async fn create_network(&self, name: &str) -> Result<(), ContainerError> {
        // TODO: Implement network creation using backend
        // For now, just log
        eprintln!("Creating network: {}", name);
        Ok(())
    }

    /// Create a volume
    async fn create_volume(&self, name: &str) -> Result<(), ContainerError> {
        // TODO: Implement volume creation using backend
        // For now, just log
        eprintln!("Creating volume: {}", name);
        Ok(())
    }

    /// Remove a network
    async fn remove_network(&self, name: &str) -> Result<(), ContainerError> {
        // TODO: Implement network removal
        eprintln!("Removing network: {}", name);
        Ok(())
    }

    /// Remove a volume
    async fn remove_volume(&self, name: &str) -> Result<(), ContainerError> {
        // TODO: Implement volume removal
        eprintln!("Removing volume: {}", name);
        Ok(())
    }

    /// Stop and remove all resources in the compose stack
    pub async fn down(&self, handle: &ComposeHandle, remove_volumes: bool) -> Result<(), ContainerError> {
        // Stop and remove containers
        for (name, container) in &handle.containers {
            let _ = self.backend.stop(&container.id, 10).await;
            let _ = self.backend.remove(&container.id, true).await;
            eprintln!("Stopped and removed service: {}", name);
        }

        // Remove networks
        for network in &handle.networks {
            let _ = self.remove_network(network).await;
        }

        // Remove volumes if requested
        if remove_volumes {
            for volume in &handle.volumes {
                let _ = self.remove_volume(volume).await;
            }
        }

        Ok(())
    }

    /// Get container info for all services in the stack
    pub async fn ps(&self, handle: &ComposeHandle) -> Result<Vec<super::types::ContainerInfo>, ContainerError> {
        let mut result = Vec::new();

        for container in handle.containers.values() {
            match self.backend.inspect(&container.id).await {
                Ok(info) => result.push(info),
                Err(_) => {
                    // Container might not exist anymore
                    continue;
                }
            }
        }

        Ok(result)
    }

    /// Get logs for a specific service
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
            Err(ContainerError::NotFound(format!("Service not found: {}", service_name)))
        } else {
            // Get logs from all services
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
    }

    /// Execute a command in a service container
    pub async fn exec(
        &self,
        handle: &ComposeHandle,
        service: &str,
        cmd: &[String],
    ) -> Result<super::types::ContainerLogs, ContainerError> {
        if let Some(container) = handle.containers.get(service) {
            self.backend.exec(&container.id, cmd, None).await
        } else {
            Err(ContainerError::NotFound(format!("Service not found: {}", service)))
        }
    }
}
