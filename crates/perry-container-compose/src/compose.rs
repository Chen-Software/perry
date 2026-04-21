use indexmap::IndexMap;
use crate::error::{ComposeError, Result};
use crate::types::{ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec, ComposeHandle, ContainerHandle, ListOrDict};
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::service::{service_container_name, needs_build};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use dashmap::DashMap;
use once_cell::sync::Lazy;

static ENGINES: Lazy<DashMap<u64, Arc<ComposeEngine>>> = Lazy::new(DashMap::new);

#[derive(Clone)]
pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Arc<Self> {
        let engine = Arc::new(Self { spec, project_name, backend });
        engine
    }

    pub fn get_engine(stack_id: u64) -> Option<Arc<Self>> {
        ENGINES.get(&stack_id).map(|e| e.clone())
    }

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

        let mut queue: BTreeSet<String> = in_degree
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
                .iter()
                .filter(|(_, &deg)| deg > 0)
                .map(|(name, _)| name.clone())
                .collect();
            return Err(ComposeError::DependencyCycle { services: cycle_services });
        }

        Ok(order)
    }

    pub async fn up(self: Arc<Self>, services: &[String], _detach: bool, build: bool) -> Result<ComposeHandle> {
        let mut order = Self::resolve_startup_order(&self.spec)?;
        if !services.is_empty() {
             order.retain(|s| services.contains(s));
        }
        let mut created_networks = Vec::new();
        let mut created_volumes = Vec::new();
        let mut started_containers: Vec<String> = Vec::new();

        // 1. Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, config_opt) in networks {
                let net_name = format!("{}_{}", self.project_name, name);
                // Check if exists
                if self.backend.list(true).await.is_ok() { // Best effort check
                     // In a real implementation we'd inspect if it exists
                }
                let config = config_opt.as_ref().map(|c| NetworkConfig {
                    driver: c.driver.clone(),
                    internal: c.internal.unwrap_or(false),
                    enable_ipv6: c.enable_ipv6.unwrap_or(false),
                    labels: match &c.labels {
                        Some(ListOrDict::Dict(d)) => Some(d.iter().map(|(k, v)| (k.clone(), format!("{:?}", v))).collect()),
                        _ => None,
                    },
                }).unwrap_or_default();

                if let Err(e) = self.backend.create_network(&net_name, &config).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_networks.push(net_name);
            }
        }

        // 2. Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, config_opt) in volumes {
                let vol_name = format!("{}_{}", self.project_name, name);
                let config = config_opt.as_ref().map(|c| VolumeConfig {
                    driver: c.driver.clone(),
                    labels: match &c.labels {
                        Some(ListOrDict::Dict(d)) => Some(d.iter().map(|(k, v)| (k.clone(), format!("{:?}", v))).collect()),
                        _ => None,
                    },
                }).unwrap_or_default();

                if let Err(e) = self.backend.create_volume(&vol_name, &config).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_volumes.push(vol_name);
            }
        }

        // 3. Start services in order
        for service_name in order {
            let service = self.spec.services.get(&service_name).unwrap();
            let container_name = service_container_name(service, &service_name);

            // Idempotent flow:
            // Check if container already exists
            let existing = self.backend.list(true).await?.into_iter().find(|c| c.name == container_name);

            if let Some(c) = existing {
                if c.status.contains("Up") || c.status.contains("running") {
                    // Already running
                    started_containers.push(c.id);
                    continue;
                } else {
                    // Exists but stopped, start it
                    if let Err(e) = self.backend.start(&c.id).await {
                        self.rollback(&started_containers, &created_networks, &created_volumes).await;
                        return Err(e);
                    }
                    started_containers.push(c.id);
                    continue;
                }
            }

            // Fresh container
            if build || needs_build(service) {
                if let Some(build_spec) = &service.build {
                     // Build logic... for now we assume image exists or built externally
                     // In a full implementation we'd call backend.build()
                     let _ = build_spec;
                }
            } else if let Some(image) = &service.image {
                if let Err(e) = self.backend.pull_image(image).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(ComposeError::ImagePullFailed {
                        service: service_name.clone(),
                        image: image.clone(),
                        message: e.to_string(),
                    });
                }
            }

            let container_spec = ContainerSpec {
                image: service.image.clone().unwrap_or_else(|| service_name.clone()),
                name: Some(container_name),
                ports: service.ports.as_ref().map(|p| p.iter().map(|ps| format!("{:?}", ps)).collect()),
                volumes: service.volumes.as_ref().map(|v| v.iter().map(|vs| format!("{:?}", vs)).collect()),
                env: match &service.environment {
                    Some(ListOrDict::Dict(d)) => Some(d.iter().map(|(k, v)| (k.clone(), v.as_ref().map(|val| format!("{:?}", val)).unwrap_or_default())).collect()),
                    _ => None,
                },
                cmd: match &service.command {
                    Some(serde_yaml::Value::String(s)) => Some(vec![s.clone()]),
                    Some(serde_yaml::Value::Sequence(seq)) => Some(seq.iter().map(|v| format!("{:?}", v)).collect()),
                    _ => None,
                },
                entrypoint: None,
                network: service.networks.as_ref().map(|_| format!("{}_default", self.project_name)), // simplified
                rm: Some(false),
                read_only: service.read_only,
            };

            match self.backend.run(&container_spec).await {
                Ok(handle) => {
                    started_containers.push(handle.id);
                }
                Err(e) => {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(ComposeError::ServiceStartupFailed {
                        service: service_name,
                        message: e.to_string(),
                    });
                }
            }
        }

        let stack_id = rand::random();
        ENGINES.insert(stack_id, self.clone());

        Ok(ComposeHandle {
            stack_id,
            project_name: self.project_name.clone(),
            services: started_containers,
        })
    }

    async fn rollback(&self, containers: &[String], networks: &[String], volumes: &[String]) {
        for id in containers.iter().rev() {
            let _ = self.backend.stop(id, Some(10)).await;
            let _ = self.backend.remove(id, true).await;
        }
        for net in networks {
            let _ = self.backend.remove_network(net).await;
        }
        for vol in volumes {
            let _ = self.backend.remove_volume(vol).await;
        }
    }

    pub async fn down(&self, services: &[String], volumes: bool) -> Result<()> {
        let containers = self.backend.list(true).await?;
        for c in containers {
            // Check if it belongs to this project
            if c.name.starts_with(&format!("{}-", self.project_name)) {
                if !services.is_empty() {
                    let svc_part = c.name.strip_prefix(&format!("{}-", self.project_name)).unwrap_or("");
                    let svc_name = svc_part.split('-').next().unwrap_or("");
                    if !services.contains(&svc_name.to_string()) {
                        continue;
                    }
                }
                let _ = self.backend.stop(&c.id, None).await;
                let _ = self.backend.remove(&c.id, true).await;
            }
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
        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let all = self.backend.list(true).await?;
        Ok(all.into_iter().filter(|c| c.name.starts_with(&format!("{}-", self.project_name))).collect())
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        if let Some(svc_name) = service {
             let container_name = format!("{}-{}-", self.project_name, svc_name);
             let containers = self.backend.list(true).await?;
             if let Some(c) = containers.into_iter().find(|c| c.name.starts_with(&container_name)) {
                  return self.backend.logs(&c.id, tail).await;
             }
             return Err(ComposeError::NotFound(svc_name.into()));
        }
        Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let container_name = format!("{}-{}-", self.project_name, service);
        let containers = self.backend.list(true).await?;
        if let Some(c) = containers.into_iter().find(|c| c.name.starts_with(&container_name)) {
             return self.backend.exec(&c.id, cmd, None, None).await;
        }
        Err(ComposeError::NotFound(service.into()))
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let containers = self.backend.list(true).await?;
        for svc_name in services {
            let prefix = format!("{}-{}-", self.project_name, svc_name);
            if let Some(c) = containers.iter().find(|c| c.name.starts_with(&prefix)) {
                self.backend.start(&c.id).await?;
            }
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let containers = self.backend.list(true).await?;
        for svc_name in services {
            let prefix = format!("{}-{}-", self.project_name, svc_name);
            if let Some(c) = containers.iter().find(|c| c.name.starts_with(&prefix)) {
                self.backend.stop(&c.id, None).await?;
            }
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        let containers = self.backend.list(true).await?;
        for svc_name in services {
            let prefix = format!("{}-{}-", self.project_name, svc_name);
            if let Some(c) = containers.iter().find(|c| c.name.starts_with(&prefix)) {
                self.backend.stop(&c.id, None).await?;
                self.backend.start(&c.id).await?;
            }
        }
        Ok(())
    }

    pub fn config(&self) -> Result<String> {
        serde_yaml::to_string(&self.spec).map_err(Into::into)
    }
}
