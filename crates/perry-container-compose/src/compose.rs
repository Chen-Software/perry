//! `ComposeEngine` — core orchestration using Kahn's algorithm.

use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static NEXT_STACK_ID: AtomicU64 = AtomicU64::new(1);

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
    pub containers: dashmap::DashMap<String, String>, // service -> container_id
}

impl ComposeEngine {
    pub fn new(
        spec: ComposeSpec,
        project_name: String,
        backend: Arc<dyn ContainerBackend>,
    ) -> Self {
        Self {
            spec,
            project_name,
            backend,
            containers: dashmap::DashMap::new(),
        }
    }

    pub async fn up(
        &self,
        _services: &[String],
        _detach: bool,
        _build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;

        // 1. Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                if let Some(cfg) = config {
                    self.backend.create_network(name, cfg).await?;
                }
            }
        }

        // 2. Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                if let Some(cfg) = config {
                    self.backend.create_volume(name, cfg).await?;
                }
            }
        }

        // 3. Start containers in order
        for svc_name in order {
            let svc = self.spec.services.get(&svc_name).unwrap();
            let image = svc.image.as_deref().unwrap_or("alpine");
            let container_name = service::generate_name(image, &svc_name);

            // Build ContainerSpec from ComposeService
            let spec = crate::types::ContainerSpec {
                image: image.to_string(),
                name: Some(container_name.clone()),
                ports: svc.ports.as_ref().map(|p| p.iter().map(|_| "todo".to_string()).collect()), // simplified
                ..Default::default()
            };

            match self.backend.run(&spec).await {
                Ok(handle) => {
                    self.containers.insert(svc_name, handle.id);
                }
                Err(e) => {
                    // Best effort rollback
                    self.down(false, false).await.ok();
                    return Err(ComposeError::ServiceStartupFailed {
                        service: svc_name,
                        message: e.to_string(),
                    });
                }
            }
        }

        Ok(ComposeHandle {
            stack_id: NEXT_STACK_ID.fetch_add(1, Ordering::SeqCst),
            project_name: self.project_name.clone(),
            services: self.spec.services.keys().cloned().collect(),
        })
    }

    pub async fn down(&self, volumes: bool, _remove_orphans: bool) -> Result<()> {
        for entry in &self.containers {
            self.backend.stop(entry.value(), None).await.ok();
            self.backend.remove(entry.value(), true).await.ok();
        }
        self.containers.clear();

        if let Some(networks) = &self.spec.networks {
            for name in networks.keys() {
                self.backend.remove_network(name).await.ok();
            }
        }

        if volumes {
            if let Some(volumes) = &self.spec.volumes {
                for name in volumes.keys() {
                    self.backend.remove_volume(name).await.ok();
                }
            }
        }

        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut results = Vec::new();
        for entry in &self.containers {
            if let Ok(info) = self.backend.inspect(entry.value()).await {
                results.push(info);
            }
        }
        Ok(results)
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        if let Some(svc) = service {
            if let Some(id) = self.containers.get(svc) {
                return self.backend.logs(id.value(), tail).await;
            }
            return Err(ComposeError::NotFound(svc.to_string()));
        }
        // Simplified: return empty logs if no service specified (real impl would aggregate)
        Ok(ContainerLogs { stdout: String::new(), stderr: String::new() })
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        if let Some(id) = self.containers.get(service) {
            return self.backend.exec(id.value(), cmd, None, None).await;
        }
        Err(ComposeError::NotFound(service.to_string()))
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        for svc in services {
            if let Some(id) = self.containers.get(svc) {
                self.backend.start(id.value()).await?;
            }
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        for svc in services {
            if let Some(id) = self.containers.get(svc) {
                self.backend.stop(id.value(), None).await?;
            }
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.stop(services).await?;
        self.start(services).await
    }
}

pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

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
        if let Some(deps) = dependents.get(&service) {
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
            .into_iter()
            .filter(|(_, deg)| *deg > 0)
            .map(|(name, _)| name)
            .collect();
        return Err(ComposeError::DependencyCycle { services: cycle_services });
    }

    Ok(order)
}
