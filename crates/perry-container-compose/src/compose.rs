use std::collections::{HashMap, VecDeque, BTreeSet};
use std::sync::Arc;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use crate::error::{ComposeError, Result};
use crate::types::{ComposeSpec, ComposeHandle, ContainerSpec, ContainerInfo, ContainerLogs};
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::service;

static COMPOSE_HANDLES: Lazy<DashMap<u64, Arc<ComposeEngine>>> = Lazy::new(DashMap::new);
static NEXT_STACK_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
    pub stack_id: u64,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        let stack_id = NEXT_STACK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self { spec, project_name, backend, stack_id }
    }

    pub async fn up(&self, detach: bool, build: bool, _remove_orphans: bool) -> Result<ComposeHandle> {
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

        let mut started_services = Vec::new();

        for svc_name in order {
            let service = self.spec.services.get(&svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let container_name = service::generate_name(service)?;

            // Idempotent flow:
            match self.backend.inspect(&container_name).await {
                Ok(info) => {
                    if info.status.contains("running") || info.status.contains("Up") {
                        // Already running, skip
                        started_services.push(svc_name);
                        continue;
                    } else {
                        // Exists but stopped, restart
                        self.backend.start(&container_name).await?;
                        started_services.push(svc_name);
                        continue;
                    }
                }
                Err(_) => {
                    // Does not exist, create fresh
                }
            }

            if build || service::needs_build(service) {
                // build logic - placeholder
            } else if let Some(image) = &service.image {
                self.backend.pull_image(image).await?;
            }

            let container_spec = ContainerSpec {
                image: service.image.clone().unwrap_or_default(),
                name: Some(container_name.clone()),
                cmd: service.command.as_ref().and_then(|v| {
                    if let Some(s) = v.as_str() {
                        Some(vec![s.to_string()])
                    } else if let Some(arr) = v.as_sequence() {
                        Some(arr.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect())
                    } else {
                        None
                    }
                }),
                ..Default::default()
            };

            self.backend.run(&container_spec).await?;
            started_services.push(svc_name);
        }

        let handle = ComposeHandle {
            stack_id: self.stack_id,
            project_name: self.project_name.clone(),
            services: started_services,
        };

        Ok(handle)
    }

    pub async fn down(&self, volumes: bool, _remove_orphans: bool) -> Result<()> {
        // Stop and remove containers in reverse order
        let order = resolve_startup_order(&self.spec)?;
        for svc_name in order.into_iter().rev() {
            let container_name = format!("{}_{}_1", self.project_name, svc_name);
            let _ = self.backend.stop(&container_name, None).await;
            let _ = self.backend.remove(&container_name, true).await;
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

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        // This should filter backend.list() by project labels
        self.backend.list(true).await
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        let container_name = if let Some(svc) = service {
            format!("{}_{}_1", self.project_name, svc)
        } else {
            // Combined logs for all services - placeholder
            return Ok(ContainerLogs { stdout: String::new(), stderr: String::new() });
        };
        self.backend.logs(&container_name, tail).await
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let container_name = format!("{}_{}_1", self.project_name, service);
        self.backend.exec(&container_name, cmd, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let services = if services.is_empty() {
            self.spec.services.keys().cloned().collect::<Vec<_>>()
        } else {
            services.to_vec()
        };
        for svc in services {
            let container_name = format!("{}_{}_1", self.project_name, svc);
            self.backend.start(&container_name).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let services = if services.is_empty() {
            self.spec.services.keys().cloned().collect::<Vec<_>>()
        } else {
            services.to_vec()
        };
        for svc in services {
            let container_name = format!("{}_{}_1", self.project_name, svc);
            self.backend.stop(&container_name, None).await?;
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        let services = if services.is_empty() {
            self.spec.services.keys().cloned().collect::<Vec<_>>()
        } else {
            services.to_vec()
        };
        for svc in services {
            let container_name = format!("{}_{}_1", self.project_name, svc);
            self.backend.stop(&container_name, None).await.ok();
            self.backend.start(&container_name).await?;
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ComposeSpec, ComposeService, DependsOnSpec};
    use indexmap::IndexMap;

    #[test]
    fn test_resolve_startup_order_simple() {
        let mut services = IndexMap::new();
        services.insert("db".to_string(), ComposeService::default());
        services.insert("web".to_string(), ComposeService {
            depends_on: Some(DependsOnSpec::List(vec!["db".to_string()])),
            ..Default::default()
        });

        let spec = ComposeSpec {
            services,
            ..Default::default()
        };

        let order = resolve_startup_order(&spec).unwrap();
        assert_eq!(order, vec!["db", "web"]);
    }

    #[test]
    fn test_resolve_startup_order_cycle() {
        let mut services = IndexMap::new();
        services.insert("a".to_string(), ComposeService {
            depends_on: Some(DependsOnSpec::List(vec!["b".to_string()])),
            ..Default::default()
        });
        services.insert("b".to_string(), ComposeService {
            depends_on: Some(DependsOnSpec::List(vec!["a".to_string()])),
            ..Default::default()
        });

        let spec = ComposeSpec {
            services,
            ..Default::default()
        };

        let result = resolve_startup_order(&spec);
        assert!(result.is_err());
    }
}
