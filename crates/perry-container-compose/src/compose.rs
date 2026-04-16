//! `ComposeEngine` — the core compose orchestration engine.

use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::error::{ComposeError, Result};
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs,
};
use indexmap::IndexMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::collections::HashMap;

static COMPOSE_ENGINES: once_cell::sync::Lazy<std::sync::Mutex<IndexMap<u64, Arc<ComposeEngine>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(IndexMap::new()));

static NEXT_STACK_ID: AtomicU64 = AtomicU64::new(1);

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
    started_containers: std::sync::Mutex<Vec<String>>,
}

impl ComposeEngine {
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

    fn register(self: Arc<Self>) -> ComposeHandle {
        let stack_id = NEXT_STACK_ID.fetch_add(1, Ordering::SeqCst);
        let services: Vec<String> = self.spec.services.keys().cloned().collect();
        let handle = ComposeHandle {
            stack_id,
            project_name: self.project_name.clone(),
            services,
        };
        COMPOSE_ENGINES.lock().unwrap().insert(stack_id, self);
        handle
    }

    pub async fn up(
        self: Arc<Self>,
        services: &[String],
        _detach: bool,
        _build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        if let Some(networks) = &self.spec.networks {
            for (net_name, net_config_opt) in networks {
                let external = net_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                if external { continue; }
                let resolved_name = net_config_opt.as_ref()
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(net_name.as_str());
                let labels = net_config_opt.as_ref()
                    .and_then(|c| c.labels.as_ref())
                    .map(|l| l.to_map())
                    .unwrap_or_default();

                let config = NetworkConfig {
                    driver: net_config_opt.as_ref().and_then(|c| c.driver.clone()),
                    labels,
                    internal: net_config_opt.as_ref().map_or(false, |c| c.internal.unwrap_or(false)),
                    enable_ipv6: net_config_opt.as_ref().map_or(false, |c| c.enable_ipv6.unwrap_or(false)),
                };
                self.backend.create_network(resolved_name, &config).await?;
            }
        }

        if let Some(volumes) = &self.spec.volumes {
            for (vol_name, vol_config_opt) in volumes {
                let external = vol_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                if external { continue; }
                let resolved_name = vol_config_opt.as_ref()
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(vol_name.as_str());
                let labels = vol_config_opt.as_ref()
                    .and_then(|c| c.labels.as_ref())
                    .map(|l| l.to_map())
                    .unwrap_or_default();

                let config = VolumeConfig {
                    driver: vol_config_opt.as_ref().and_then(|c| c.driver.clone()),
                    labels,
                };
                self.backend.create_volume(resolved_name, &config).await?;
            }
        }

        for svc_name in target {
            let svc = self.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let container_spec = svc.to_container_spec(svc_name);
            match self.backend.run(&container_spec).await {
                Ok(handle) => {
                    self.started_containers.lock().unwrap().push(handle.id);
                }
                Err(e) => {
                    // Rollback: stop and remove all started containers
                    let _ = self.down(&[], false, false).await;
                    return Err(e);
                }
            }
        }

        Ok(self.register())
    }

    pub async fn down(&self, _services: &[String], _remove_orphans: bool, _remove_volumes: bool) -> Result<()> {
        let containers_to_stop: Vec<String> = {
            let containers = self.started_containers.lock().unwrap();
            containers.iter().cloned().rev().collect()
        };

        for id in containers_to_stop {
            let _ = self.backend.stop(&id, None).await;
            let _ = self.backend.remove(&id, true).await;
        }

        let mut containers = self.started_containers.lock().unwrap();
        containers.clear();
        Ok(())
    }

    pub async fn start(&self, _services: &[String]) -> Result<()> {
        let containers = self.started_containers.lock().unwrap();
        for id in &*containers {
            self.backend.start(id).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, _services: &[String]) -> Result<()> {
        let containers = self.started_containers.lock().unwrap();
        for id in &*containers {
            self.backend.stop(id, None).await?;
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.stop(services).await?;
        self.start(services).await?;
        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        self.backend.list(true).await
    }

    pub async fn logs(&self, _services: &[String], tail: Option<u32>) -> Result<HashMap<String, String>> {
        let mut logs = HashMap::new();
        let containers = self.started_containers.lock().unwrap();
        for id in &*containers {
            let log = self.backend.logs(id, tail).await?;
            logs.insert(id.clone(), log.stdout + &log.stderr);
        }
        Ok(logs)
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        self.backend.exec(service, cmd, None, None).await
    }

    pub fn config(&self) -> Result<String> {
        self.spec.to_yaml()
    }
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
                    return Err(ComposeError::validation(format!("Service '{}' depends on '{}' which is not defined", name, dep)));
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
