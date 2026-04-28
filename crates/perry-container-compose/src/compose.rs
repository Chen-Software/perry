//! `ComposeEngine` — the core compose orchestration engine.

use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
pub use crate::types::ContainerLogs;
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerSpec,
};
use indexmap::IndexMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static COMPOSE_ENGINES: once_cell::sync::Lazy<Mutex<IndexMap<u64, Arc<ComposeEngine>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(IndexMap::new()));

static NEXT_STACK_ID: AtomicU64 = AtomicU64::new(1);

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
    session_containers: Mutex<Vec<String>>,
    session_networks: Mutex<Vec<String>>,
    session_volumes: Mutex<Vec<String>>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        ComposeEngine {
            spec, project_name, backend,
            session_containers: Mutex::new(Vec::new()),
            session_networks: Mutex::new(Vec::new()),
            session_volumes: Mutex::new(Vec::new()),
        }
    }

    fn register(self: Arc<Self>) -> ComposeHandle {
        let stack_id = NEXT_STACK_ID.fetch_add(1, Ordering::SeqCst);
        let services = self.spec.services.keys().cloned().collect();
        let handle = ComposeHandle { stack_id, project_name: self.project_name.clone(), services };
        COMPOSE_ENGINES.lock().unwrap().insert(stack_id, Arc::clone(&self));
        handle
    }

    pub async fn up(self: Arc<Self>, services: &[String], _detach: bool, _build: bool, _remove_orphans: bool) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() { order.iter().collect() } else { order.iter().filter(|s| services.contains(s)).collect() };

        if let Some(networks) = &self.spec.networks {
            for (net_name, net_config_opt) in networks {
                let external = net_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                if external { continue; }
                let net_config_spec = net_config_opt.as_ref().cloned().unwrap_or_default();
                let net_config = NetworkConfig::from(&net_config_spec);
                let resolved_name = net_config_spec.name.as_deref().unwrap_or(net_name.as_str());
                if self.backend.inspect_network(resolved_name).await.is_err() {
                    self.backend.create_network(resolved_name, &net_config).await?;
                    self.session_networks.lock().unwrap().push(resolved_name.to_string());
                }
            }
        }

        if let Some(volumes) = &self.spec.volumes {
            for (vol_name, vol_config_opt) in volumes {
                let external = vol_config_opt.as_ref().map_or(false, |c| c.external.unwrap_or(false));
                if external { continue; }
                let vol_config_spec = vol_config_opt.as_ref().cloned().unwrap_or_default();
                let vol_config = VolumeConfig::from(&vol_config_spec);
                let resolved_name = vol_config_spec.name.as_deref().unwrap_or(vol_name.as_str());
                if self.backend.inspect_volume(resolved_name).await.is_err() {
                    self.backend.create_volume(resolved_name, &vol_config).await?;
                    self.session_volumes.lock().unwrap().push(resolved_name.to_string());
                }
            }
        }

        for svc_name in target {
            let svc = self.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let container_name = service::service_container_name(svc, svc_name);
            if let Ok(info) = self.backend.inspect(&container_name).await {
                if info.status != "running" { self.backend.start(&container_name).await?; }
            } else {
                let spec = ContainerSpec {
                    image: svc.image_ref(svc_name), name: Some(container_name.clone()),
                    ports: Some(svc.port_strings()), volumes: Some(svc.volume_strings()),
                    env: Some(svc.resolved_env()), cmd: svc.command_list(), rm: Some(false), ..Default::default()
                };
                self.backend.run(&spec).await?;
            }
            self.session_containers.lock().unwrap().push(container_name);
        }
        Ok(self.register())
    }

    pub async fn down(&self, _services: &[String], _remove_orphans: bool, remove_volumes: bool) -> Result<()> {
        let containers = { let mut c = self.session_containers.lock().unwrap(); std::mem::take(&mut *c) };
        for c_name in containers.iter().rev() {
            let _ = self.backend.stop(c_name, None).await;
            let _ = self.backend.remove(c_name, true).await;
        }
        let networks = { let mut n = self.session_networks.lock().unwrap(); std::mem::take(&mut *n) };
        for n_name in networks { let _ = self.backend.remove_network(&n_name).await; }
        if remove_volumes {
            let volumes = { let mut v = self.session_volumes.lock().unwrap(); std::mem::take(&mut *v) };
            for v_name in volumes { let _ = self.backend.remove_volume(&v_name).await; }
        }
        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let names = { self.session_containers.lock().unwrap().clone() };
        let mut results = Vec::new();
        for c_name in names { if let Ok(info) = self.backend.inspect(&c_name).await { results.push(info); } }
        Ok(results)
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        let names = { self.session_containers.lock().unwrap().clone() };
        let mut stdout = String::new();
        let mut stderr = String::new();
        for c_name in names {
            if let Some(s) = service { if !c_name.contains(s) { continue; } }
            if let Ok(l) = self.backend.logs(&c_name, tail).await {
                stdout.push_str(&l.stdout);
                stderr.push_str(&l.stderr);
            }
        }
        Ok(ContainerLogs { stdout, stderr })
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let c_name = {
            let c = self.session_containers.lock().unwrap();
            c.iter().find(|n| n.contains(service)).cloned()
        }.ok_or_else(|| ComposeError::NotFound(service.to_string()))?;
        self.backend.exec(&c_name, cmd, None, None).await
    }

    pub fn config(&self) -> Result<String> { self.spec.to_yaml() }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let names = { self.session_containers.lock().unwrap().clone() };
        for c_name in names {
            if services.is_empty() || services.iter().any(|s| c_name.contains(s)) {
                self.backend.start(&c_name).await?;
            }
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let names = { self.session_containers.lock().unwrap().clone() };
        for c_name in names {
            if services.is_empty() || services.iter().any(|s| c_name.contains(s)) {
                self.backend.stop(&c_name, None).await?;
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
    let mut in_degree: IndexMap<String, usize> = IndexMap::new();
    let mut dependents: IndexMap<String, Vec<String>> = IndexMap::new();
    for name in spec.services.keys() { in_degree.insert(name.clone(), 0); dependents.insert(name.clone(), Vec::new()); }
    for (name, service) in &spec.services {
        if let Some(deps) = &service.depends_on {
            for dep in deps.service_names() {
                if !spec.services.contains_key(&dep) { return Err(ComposeError::validation(format!("Service '{}' depends on '{}' which is not defined", name, dep))); }
                *in_degree.get_mut(name).unwrap() += 1;
                dependents.get_mut(&dep).unwrap().push(name.clone());
            }
        }
    }
    let mut queue: std::collections::BTreeSet<String> = in_degree.iter().filter(|(_, &deg)| deg == 0).map(|(name, _)| name.clone()).collect();
    let mut order: Vec<String> = Vec::new();
    while let Some(service) = queue.pop_first() {
        order.push(service.clone());
        for dependent in dependents.get(&service).unwrap_or(&Vec::new()).clone() {
            let deg = in_degree.get_mut(&dependent).unwrap();
            *deg -= 1;
            if *deg == 0 { queue.insert(dependent); }
        }
    }
    if order.len() != spec.services.len() {
        let cycle_services: Vec<String> = in_degree.iter().filter(|(_, &deg)| deg > 0).map(|(name, _)| name.clone()).collect();
        return Err(ComposeError::DependencyCycle { services: cycle_services });
    }
    Ok(order)
}
