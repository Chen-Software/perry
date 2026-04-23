use std::collections::{HashMap, VecDeque, BTreeSet};
use std::sync::Arc;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use crate::error::{ComposeError, Result};
use crate::types::{ComposeSpec, ComposeHandle, ContainerSpec, ContainerInfo, ContainerLogs};
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::service;
use crate::orchestrate::orchestrate_service;

static COMPOSE_HANDLES: Lazy<DashMap<u64, Arc<ComposeEngine>>> = Lazy::new(DashMap::new);
static NEXT_STACK_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
    pub stack_id: u64,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Arc<Self> {
        let stack_id = NEXT_STACK_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let engine = Arc::new(Self {
            spec,
            project_name,
            backend,
            stack_id,
        });
        COMPOSE_HANDLES.insert(stack_id, engine.clone());
        engine
    }

    pub fn get_engine(stack_id: u64) -> Option<Arc<Self>> {
        COMPOSE_HANDLES.get(&stack_id).map(|r| r.value().clone())
    }

    pub async fn up(&self, _detach: bool, _build: bool, _remove_orphans: bool) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;

        // Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let net_name = format!("{}_{}", self.project_name, name);
                let net_config = config.as_ref().map(|c| NetworkConfig {
                    driver: c.driver.clone(),
                    labels: c.labels.clone(),
                    internal: c.internal,
                    enable_ipv6: c.enable_ipv6,
                }).unwrap_or(NetworkConfig { driver: None, labels: None, internal: None, enable_ipv6: None });
                self.backend.create_network(&net_name, &net_config).await?;
            }
        }

        // Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                let vol_name = format!("{}_{}", self.project_name, name);
                let vol_config = config.as_ref().map(|c| VolumeConfig {
                    driver: c.driver.clone(),
                    labels: c.labels.clone(),
                }).unwrap_or(VolumeConfig { driver: None, labels: None });
                self.backend.create_volume(&vol_name, &vol_config).await?;
            }
        }

        let mut started_containers = Vec::new();

        for svc_name in &order {
            let service = self.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            match orchestrate_service(&self.project_name, svc_name, service, self.backend.as_ref()).await {
                Ok(container_name) => {
                    started_containers.push(container_name);
                }
                Err(e) => {
                    // Rollback
                    for name in started_containers.iter().rev() {
                        let _ = self.backend.stop(name, None).await;
                        let _ = self.backend.remove(name, true).await;
                    }
                    return Err(e);
                }
            }
        }

        let handle = ComposeHandle {
            stack_id: self.stack_id,
            project_name: self.project_name.clone(),
            services: order,
        };

        Ok(handle)
    }

    pub async fn down(&self, volumes: bool, _remove_orphans: bool) -> Result<()> {
        let order = resolve_startup_order(&self.spec)?;
        for svc_name in order.into_iter().rev() {
            let service = self.spec.services.get(&svc_name).unwrap();
            let container_name = service::generate_name(&self.project_name, &svc_name, service)?;
            let _ = self.backend.stop(&container_name, None).await;
            let _ = self.backend.remove(&container_name, true).await;
        }

        if let Some(networks) = &self.spec.networks {
            for name in networks.keys() {
                let net_name = format!("{}_{}", self.project_name, name);
                let _ = self.backend.remove_network(&net_name).await;
            }
        }

        if volumes {
            if let Some(vols) = &self.spec.volumes {
                for name in vols.keys() {
                    let vol_name = format!("{}_{}", self.project_name, name);
                    let _ = self.backend.remove_volume(&vol_name).await;
                }
            }
        }

        COMPOSE_HANDLES.remove(&self.stack_id);
        Ok(())
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let targets = if services.is_empty() {
            self.spec.services.keys().cloned().collect::<Vec<_>>()
        } else {
            services.to_vec()
        };
        for svc in targets {
            let service = self.spec.services.get(&svc).ok_or_else(|| ComposeError::NotFound(svc.clone()))?;
            let container_name = service::generate_name(&self.project_name, &svc, service)?;
            self.backend.start(&container_name).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let targets = if services.is_empty() {
            self.spec.services.keys().cloned().collect::<Vec<_>>()
        } else {
            services.to_vec()
        };
        for svc in targets {
            let service = self.spec.services.get(&svc).ok_or_else(|| ComposeError::NotFound(svc.clone()))?;
            let container_name = service::generate_name(&self.project_name, &svc, service)?;
            self.backend.stop(&container_name, None).await?;
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        let targets = if services.is_empty() {
            self.spec.services.keys().cloned().collect::<Vec<_>>()
        } else {
            services.to_vec()
        };
        for svc in targets {
            let service = self.spec.services.get(&svc).ok_or_else(|| ComposeError::NotFound(svc.clone()))?;
            let container_name = service::generate_name(&self.project_name, &svc, service)?;
            let _ = self.backend.stop(&container_name, None).await;
            self.backend.start(&container_name).await?;
        }
        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let all_containers = self.backend.list(true).await?;
        let mut project_containers = Vec::new();

        for svc_name in self.spec.services.keys() {
            let service = self.spec.services.get(svc_name).unwrap();
            let container_name = service::generate_name(&self.project_name, svc_name, service)?;
            if let Some(info) = all_containers.iter().find(|c| c.name == container_name || c.name == format!("/{}", container_name)) {
                project_containers.push(info.clone());
            }
        }

        Ok(project_containers)
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        let container_name = if let Some(svc) = service {
            let spec = self.spec.services.get(svc).ok_or_else(|| ComposeError::NotFound(svc.to_string()))?;
            service::generate_name(&self.project_name, svc, spec)?
        } else {
            return Ok(ContainerLogs { stdout: String::new(), stderr: String::new() });
        };
        self.backend.logs(&container_name, tail).await
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let spec = self.spec.services.get(service).ok_or_else(|| ComposeError::NotFound(service.to_string()))?;
        let container_name = service::generate_name(&self.project_name, service, spec)?;
        self.backend.exec(&container_name, cmd, None, None).await
    }
}

pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let services = spec.services.keys().cloned().collect::<BTreeSet<_>>();

    for (name, svc) in &spec.services {
        in_degree.entry(name.clone()).or_insert(0);
        if let Some(depends_on) = &svc.depends_on {
            let targets = depends_on.service_names();
            for target in targets {
                if !spec.services.contains_key(&target) {
                    return Err(ComposeError::ValidationError(format!("Service {} depends on non-existent service {}", name, target)));
                }
                adjacency.entry(target.clone()).or_insert_with(Vec::new).push(name.clone());
                *in_degree.entry(name.clone()).or_insert(0) += 1;
            }
        }
    }

    let mut queue = VecDeque::new();
    for svc in &services {
        if *in_degree.get(svc).unwrap_or(&0) == 0 {
            queue.push_back(svc.clone());
        }
    }

    let mut result = Vec::new();
    while let Some(svc) = queue.pop_front() {
        result.push(svc.clone());
        if let Some(neighbors) = adjacency.get(&svc) {
            for neighbor in neighbors {
                let degree = in_degree.get_mut(neighbor).unwrap();
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }

    if result.len() != services.len() {
        let mut cycle_services = Vec::new();
        for svc in services {
            if *in_degree.get(&svc).unwrap_or(&0) > 0 {
                cycle_services.push(svc);
            }
        }
        return Err(ComposeError::DependencyCycle { services: cycle_services });
    }

    Ok(result)
}
