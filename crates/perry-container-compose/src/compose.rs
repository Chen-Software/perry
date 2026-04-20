use crate::types::{ComposeSpec, ComposeHandle, ContainerSpec, ContainerInfo, ContainerLogs};
use crate::error::{ComposeError, Result};
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::service;
use indexmap::IndexMap;
use std::sync::Arc;
use std::collections::{HashMap, BTreeSet, HashSet};

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
}

fn yaml_val_to_string(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        _ => "".to_string(),
    }
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { spec, project_name, backend }
    }

    pub async fn up(&self, services: &[String], _detach: bool, build: bool, _remove_orphans: bool) -> Result<ComposeHandle> {
        let order = self.resolve_startup_order()?;
        let target_services: HashSet<String> = if services.is_empty() {
            self.spec.services.keys().cloned().collect()
        } else {
            services.iter().cloned().collect()
        };

        // Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, _net) in networks {
                let net_name = format!("{}_{}", self.project_name, name);
                self.backend.create_network(&net_name, &NetworkConfig::default()).await?;
            }
        }

        // Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, _vol) in volumes {
                let vol_name = format!("{}_{}", self.project_name, name);
                self.backend.create_volume(&vol_name, &VolumeConfig::default()).await?;
            }
        }

        let mut started_containers = Vec::new();

        for svc_name in &order {
            if !target_services.contains(svc_name) { continue; }

            let svc = self.spec.services.get(svc_name).unwrap();

            // Generate stable name
            let container_name = service::generate_name(svc)?;

            // Check if already running
            let containers = self.backend.list(true).await?;
            let existing = containers.iter().find(|c| c.name == container_name);

            if let Some(c) = existing {
                if c.status.to_lowercase().contains("up") || c.status.to_lowercase().contains("running") {
                    // Already running
                    started_containers.push(container_name);
                    continue;
                } else {
                    // Exists but stopped, start it
                    self.backend.start(&c.id).await?;
                    started_containers.push(container_name);
                    continue;
                }
            }

            // Fresh container: build or pull
            if build || service::needs_build(svc) {
                if let Some(build_spec) = &svc.build {
                    let build_config = match build_spec {
                        crate::types::BuildSpec::Context(ctx) => crate::types::ComposeServiceBuild {
                            context: Some(ctx.clone()),
                            ..Default::default()
                        },
                        crate::types::BuildSpec::Config(cfg) => cfg.clone(),
                    };
                    let context = build_config.context.clone().unwrap_or_else(|| ".".into());
                    let tag = svc.image.clone().unwrap_or_else(|| format!("{}_{}", self.project_name, svc_name));
                    self.backend.build(&context, &build_config, &tag).await?;
                }
            } else if let Some(image) = &svc.image {
                self.backend.pull_image(image).await.map_err(|e| ComposeError::ImagePullFailed {
                    service: svc_name.clone(),
                    image: image.clone(),
                    message: e.to_string()
                })?;
            }

            let spec = ContainerSpec {
                image: svc.image.clone().unwrap_or_default(),
                name: Some(container_name.clone()),
                ports: svc.ports.as_ref().map(|ps| ps.iter().map(|p| match p {
                    crate::types::PortSpec::Short(v) => yaml_val_to_string(v),
                    crate::types::PortSpec::Long(p) => {
                        let pub_str = p.published.as_ref().map(yaml_val_to_string).unwrap_or_default();
                        let target_str = yaml_val_to_string(&p.target);
                        format!("{}:{}", pub_str, target_str)
                    }
                }).collect()),
                ..Default::default()
            };

            match self.backend.run(&spec).await {
                Ok(_) => started_containers.push(container_name),
                Err(e) => {
                    // Rollback started containers
                    for name in started_containers.iter().rev() {
                        let _ = self.backend.stop(name, None).await;
                        let _ = self.backend.remove(name, true).await;
                    }
                    return Err(ComposeError::ServiceStartupFailed { service: svc_name.clone(), message: e.to_string() });
                }
            }
        }

        Ok(ComposeHandle {
            stack_id: rand::random(),
            project_name: self.project_name.clone(),
            services: order,
        })
    }

    pub fn resolve_startup_order_spec(spec: &ComposeSpec) -> Result<Vec<String>> {
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

    pub fn resolve_startup_order(&self) -> Result<Vec<String>> {
        Self::resolve_startup_order_spec(&self.spec)
    }

    pub async fn down(&self, volumes: bool, _remove_orphans: bool) -> Result<()> {
        let order = self.resolve_startup_order()?;
        for svc_name in order.iter().rev() {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::generate_name(svc)?;
            let _ = self.backend.stop(&container_name, None).await;
            let _ = self.backend.remove(&container_name, true).await;
        }

        if let Some(networks) = &self.spec.networks {
            for (name, _) in networks {
                let net_name = format!("{}_{}", self.project_name, name);
                let _ = self.backend.remove_network(&net_name).await;
            }
        }

        if volumes {
            if let Some(vols) = &self.spec.volumes {
                for (name, _) in vols {
                    let vol_name = format!("{}_{}", self.project_name, name);
                    let _ = self.backend.remove_volume(&vol_name).await;
                }
            }
        }

        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut results = Vec::new();
        let all_containers = self.backend.list(true).await?;
        for svc_name in self.spec.services.keys() {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::generate_name(svc)?;
            if let Some(c) = all_containers.iter().find(|c| c.name == container_name) {
                results.push(c.clone());
            }
        }
        Ok(results)
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut stdout = String::new();
        let mut stderr = String::new();

        let services_to_log: Vec<String> = if let Some(s) = service {
            vec![s.to_string()]
        } else {
            self.spec.services.keys().cloned().collect()
        };

        for svc_name in services_to_log {
            if let Some(svc) = self.spec.services.get(&svc_name) {
                let container_name = service::generate_name(svc)?;
                if let Ok(l) = self.backend.logs(&container_name, tail).await {
                    stdout.push_str(&format!("--- {} ---\n", svc_name));
                    stdout.push_str(&l.stdout);
                    stderr.push_str(&l.stderr);
                }
            }
        }
        Ok(ContainerLogs { stdout, stderr })
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let svc = self.spec.services.get(service).ok_or_else(|| ComposeError::NotFound(service.into()))?;
        let container_name = service::generate_name(svc)?;
        self.backend.exec(&container_name, cmd, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        for svc_name in services {
            let svc = self.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.into()))?;
            let container_name = service::generate_name(svc)?;
            self.backend.start(&container_name).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        for svc_name in services {
            let svc = self.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.into()))?;
            let container_name = service::generate_name(svc)?;
            self.backend.stop(&container_name, None).await?;
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        for svc_name in services {
            let svc = self.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.into()))?;
            let container_name = service::generate_name(svc)?;
            let _ = self.backend.stop(&container_name, None).await;
            self.backend.start(&container_name).await?;
        }
        Ok(())
    }
}
