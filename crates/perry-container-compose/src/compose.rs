//! Orchestration engine for multi-container applications.
//!
//! Implements Kahn's algorithm for dependency resolution and the
//! `ComposeEngine` for managing the lifecycle of a stack.

use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{ComposeHandle, ComposeSpec, ContainerSpec};
use indexmap::IndexMap;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::Arc;

/// The orchestrator for a compose stack.
#[derive(Serialize)]
pub struct ComposeEngine {
    spec: ComposeSpec,
    project_name: String,
    #[serde(skip)]
    backend: Arc<dyn ContainerBackend>,
}

impl ComposeEngine {
    /// Create a new engine for the given spec and project.
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        Self {
            spec,
            project_name,
            backend,
        }
    }

    /// Bring the stack up.
    pub async fn up(
        &self,
        services: &[String],
        _detach: bool,
        build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;

        // Filter services if requested
        let services_to_start = if services.is_empty() {
            order
        } else {
            order
                .into_iter()
                .filter(|s| services.contains(s))
                .collect()
        };

        // 1. Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let net_name = format!("{}_{}", self.project_name, name);
                let cfg = config.clone().unwrap_or_default();
                self.backend.create_network(&net_name, &cfg).await?;
            }
        }

        // 2. Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                let vol_name = format!("{}_{}", self.project_name, name);
                let cfg = config.clone().unwrap_or_default();
                self.backend.create_volume(&vol_name, &cfg).await?;
            }
        }

        // 3. Start services in order
        let mut started_services = Vec::new();
        for svc_name in &services_to_start {
            let svc = self.spec.services.get(svc_name).ok_or_else(|| {
                ComposeError::NotFound(format!("Service {} not found in spec", svc_name))
            })?;

            let container_name = service::service_container_name(svc, svc_name);

            // Check if container already exists and is running (Idempotency)
            if let Ok(info) = self.backend.inspect(&container_name).await {
                if info.status.to_lowercase().contains("running") || info.status.to_lowercase().contains("up") {
                    started_services.push(svc_name.clone());
                    continue;
                }
                // If it exists but not running, start it
                self.backend.start(&container_name).await?;
                started_services.push(svc_name.clone());
                continue;
            }

            let image = svc.image_ref(svc_name);

            if build && svc.build.is_some() {
                self.backend.build(&image, &svc.build.as_ref().unwrap().as_build()).await?;
            } else {
                // Explicitly pull image if it doesn't exist locally
                let images = self.backend.list_images().await.unwrap_or_default();
                if !images.iter().any(|img| img.repository == image || format!("{}:{}", img.repository, img.tag) == image) {
                    self.backend.pull_image(&image).await?;
                }
            }

            // Map ComposeService to ContainerSpec
            let spec = ContainerSpec {
                image: image.clone(),
                name: Some(container_name.clone()),
                ports: Some(svc.port_strings()),
                volumes: Some(svc.volume_strings()),
                env: Some(svc.resolved_env()),
                cmd: svc.command_list(),
                entrypoint: None, // TODO: Map entrypoint
                network: svc.networks.as_ref().and_then(|n| n.names().first().cloned()), // Simple pick first
                rm: Some(false),
            };

            match self.backend.run(&spec).await {
                Ok(_) => started_services.push(svc_name.clone()),
                Err(e) => {
                    // Rollback started services in reverse order
                    for started in started_services.into_iter().rev() {
                        let s = self.spec.services.get(&started).unwrap();
                        let name = service::service_container_name(s, &started);
                        let _ = self.backend.stop(&name, Some(10)).await;
                        let _ = self.backend.remove(&name, true).await;
                    }
                    return Err(ComposeError::ServiceStartupFailed {
                        service: svc_name.clone(),
                        message: e.to_string(),
                    });
                }
            }
        }

        Ok(ComposeHandle {
            stack_id: rand::random(),
            project_name: self.project_name.clone(),
            services: services_to_start,
        })
    }

    /// Tear down the stack.
    pub async fn down(&self, volumes: bool, _remove_orphans: bool) -> Result<()> {
        // Stop and remove services in reverse dependency order
        let mut order = resolve_startup_order(&self.spec)?;
        order.reverse();

        for svc_name in order {
            let svc = self.spec.services.get(&svc_name).unwrap();
            let name = service::service_container_name(svc, &svc_name);
            let _ = self.backend.stop(&name, Some(10)).await;
            let _ = self.backend.remove(&name, true).await;
        }

        // Remove networks
        if let Some(networks) = &self.spec.networks {
            for name in networks.keys() {
                let net_name = format!("{}_{}", self.project_name, name);
                let _ = self.backend.remove_network(&net_name).await;
            }
        }

        // Remove volumes if requested
        if volumes {
            if let Some(vols) = &self.spec.volumes {
                for name in vols.keys() {
                    let vol_name = format!("{}_{}", self.project_name, name);
                    let _ = self.backend.remove_volume(&vol_name).await;
                }
            }
        }

        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<crate::types::ContainerInfo>> {
        let mut infos = Vec::new();
        for (svc_name, svc) in &self.spec.services {
            let name = service::service_container_name(svc, svc_name);
            if let Ok(info) = self.backend.inspect(&name).await {
                infos.push(info);
            }
        }
        Ok(infos)
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<crate::types::ContainerLogs> {
        if let Some(svc_name) = service {
            let svc = self.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.to_string()))?;
            let name = service::service_container_name(svc, svc_name);
            self.backend.logs(&name, tail).await
        } else {
            // Combined logs - for now just return empty or first
            Ok(crate::types::ContainerLogs { stdout: String::new(), stderr: String::new() })
        }
    }

    pub async fn exec(&self, service_name: &str, cmd: &[String]) -> Result<crate::types::ContainerLogs> {
        let svc = self.spec.services.get(service_name).ok_or_else(|| ComposeError::NotFound(service_name.to_string()))?;
        let name = service::service_container_name(svc, service_name);
        self.backend.exec(&name, cmd, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        for svc_name in services {
            let svc = self.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.to_string()))?;
            let name = service::service_container_name(svc, svc_name);
            self.backend.start(&name).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        for svc_name in services {
            let svc = self.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.to_string()))?;
            let name = service::service_container_name(svc, svc_name);
            self.backend.stop(&name, None).await?;
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.stop(services).await?;
        self.start(services).await?;
        Ok(())
    }
}

/// Resolve the deterministic startup order using Kahn's algorithm (BFS).
pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
    let mut in_degree: IndexMap<String, usize> = IndexMap::new();
    let mut dependents: IndexMap<String, Vec<String>> = IndexMap::new();

    // Initialize
    for name in spec.services.keys() {
        in_degree.insert(name.clone(), 0);
        dependents.insert(name.clone(), Vec::new());
    }

    // Compute degrees
    for (name, svc) in &spec.services {
        if let Some(deps) = &svc.depends_on {
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

    // Queue nodes with degree 0
    let mut queue: BTreeSet<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(name, _)| name.clone())
        .collect();

    let mut order = Vec::new();
    while let Some(name) = queue.pop_first() {
        order.push(name.clone());
        if let Some(deps) = dependents.get(&name) {
            for dependent in deps {
                let deg = in_degree.get_mut(dependent).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.insert(dependent.clone());
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ComposeService, DependsOnSpec};

    #[test]
    fn test_resolve_startup_order_simple() {
        let mut spec = ComposeSpec::default();
        spec.services.insert("web".into(), ComposeService {
            depends_on: Some(DependsOnSpec::List(vec!["db".into()])),
            ..Default::default()
        });
        spec.services.insert("db".into(), ComposeService::default());

        let order = resolve_startup_order(&spec).unwrap();
        assert_eq!(order, vec!["db", "web"]);
    }

    #[test]
    fn test_resolve_startup_order_cycle() {
        let mut spec = ComposeSpec::default();
        spec.services.insert("a".into(), ComposeService {
            depends_on: Some(DependsOnSpec::List(vec!["b".into()])),
            ..Default::default()
        });
        spec.services.insert("b".into(), ComposeService {
            depends_on: Some(DependsOnSpec::List(vec!["a".into()])),
            ..Default::default()
        });

        let err = resolve_startup_order(&spec).unwrap_err();
        if let ComposeError::DependencyCycle { services } = err {
            assert!(services.contains(&"a".to_string()));
            assert!(services.contains(&"b".to_string()));
        } else {
            panic!("Expected DependencyCycle error");
        }
    }
}
