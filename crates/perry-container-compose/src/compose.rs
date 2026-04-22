//! `ComposeEngine` — the core compose orchestration engine.

use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use dashmap::DashMap;
use once_cell::sync::Lazy;

static COMPOSE_ENGINES: Lazy<DashMap<u64, Arc<ComposeEngine>>> = Lazy::new(DashMap::new);
static NEXT_STACK_ID: AtomicU64 = AtomicU64::new(1);

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
    started_containers: std::sync::Mutex<Vec<String>>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        ComposeEngine { spec, project_name, backend, started_containers: std::sync::Mutex::new(Vec::new()) }
    }

    fn register(self: Arc<Self>) -> ComposeHandle {
        let id = NEXT_STACK_ID.fetch_add(1, Ordering::SeqCst);
        let services = self.spec.services.keys().cloned().collect();
        COMPOSE_ENGINES.insert(id, Arc::clone(&self));
        ComposeHandle { stack_id: id, project_name: self.project_name.clone(), services }
    }

    pub fn get_engine(stack_id: u64) -> Option<Arc<ComposeEngine>> { COMPOSE_ENGINES.get(&stack_id).map(|r| Arc::clone(r.value())) }

    pub async fn up(&self, services: &[String], detach: bool, build: bool, _remove_orphans: bool) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() { order.iter().collect() } else { order.iter().filter(|s| services.contains(s)).collect() };

        let mut created_networks: Vec<String> = Vec::new();
        let mut created_volumes: Vec<String> = Vec::new();

        if let Some(networks) = &self.spec.networks {
            for (name, conf) in networks {
                let conf = conf.as_ref().cloned().unwrap_or_default();
                if conf.external.unwrap_or(false) { continue; }
                let resolved = conf.name.as_deref().unwrap_or(name);
                if self.backend.inspect_network(resolved).await.is_err() {
                    self.backend.create_network(resolved, &NetworkConfig::from(&conf)).await.map_err(|e| {
                        for n in &created_networks { let _ = self.backend.remove_network(n); }
                        ComposeError::ServiceStartupFailed { service: format!("network/{}", name), message: e.to_string() }
                    })?;
                    created_networks.push(resolved.to_string());
                }
            }
        }

        if let Some(volumes) = &self.spec.volumes {
            for (name, conf) in volumes {
                let conf = conf.as_ref().cloned().unwrap_or_default();
                if conf.external.unwrap_or(false) { continue; }
                let resolved = conf.name.as_deref().unwrap_or(name);
                if self.backend.inspect_volume(resolved).await.is_err() {
                    self.backend.create_volume(resolved, &VolumeConfig::from(&conf)).await.map_err(|e| {
                        for v in &created_volumes { let _ = self.backend.remove_volume(v); }
                        for n in &created_networks { let _ = self.backend.remove_network(n); }
                        ComposeError::ServiceStartupFailed { service: format!("volume/{}", name), message: e.to_string() }
                    })?;
                    created_volumes.push(resolved.to_string());
                }
            }
        }

        let mut started: Vec<String> = Vec::new();
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).ok_or_else(|| ComposeError::NotFound(svc_name.clone()))?;
            let image_ref = svc.image_ref(svc_name);
            let container_name = service::service_container_name(svc, svc_name);

            let res = match self.backend.inspect(&container_name).await {
                Ok(info) if info.status.to_lowercase().contains("running") || info.status.to_lowercase().contains("up") => Ok(()),
                Ok(_) => self.backend.start(&container_name).await,
                Err(_) => {
                    if build || svc.needs_build() {
                        if let Some(b) = &svc.build {
                            let bc = b.as_build();
                            self.backend.build_image(bc.context.as_deref().unwrap_or("."), &image_ref, bc.dockerfile.as_deref(), bc.args.as_ref().map(|l| l.to_map()).as_ref()).await?;
                        }
                    } else {
                        self.backend.pull_image(&image_ref).await?;
                    }
                    let mut labels = svc.labels.as_ref().map(|l| l.to_map()).unwrap_or_default();
                    labels.insert("com.docker.compose.project".into(), self.project_name.clone());
                    labels.insert("com.docker.compose.service".into(), svc_name.clone());
                    let spec = ContainerSpec { image: image_ref, name: Some(container_name.clone()), ports: Some(svc.port_strings()), volumes: Some(svc.volume_strings()), env: Some(svc.resolved_env()), labels: Some(labels), cmd: svc.command_list(), rm: Some(false), read_only: Some(svc.read_only.unwrap_or(false)), ..Default::default() };
                    if detach { self.backend.run(&spec).await.map(|_| ()) }
                    else { self.backend.create(&spec).await?; self.backend.start(&container_name).await }
                }
            };

            if let Err(e) = res {
                for c in started.iter().rev() { let _ = self.backend.stop(c, None).await; let _ = self.backend.remove(c, true).await; }
                for v in &created_volumes { let _ = self.backend.remove_volume(v); }
                for n in &created_networks { let _ = self.backend.remove_network(n); }
                return Err(ComposeError::ServiceStartupFailed { service: svc_name.clone(), message: e.to_string() });
            }
            started.push(container_name);
        }
        self.started_containers.lock().unwrap().extend(started);
        Ok(Arc::new(ComposeEngine::new(self.spec.clone(), self.project_name.clone(), Arc::clone(&self.backend))).register())
    }

    pub async fn down(&self, services: &[String], remove_orphans: bool, remove_volumes: bool) -> Result<()> {
        let mut order = resolve_startup_order(&self.spec)?;
        order.reverse();
        let target: Vec<&String> = if services.is_empty() { order.iter().collect() } else { order.iter().filter(|s| services.contains(s)).collect() };

        for name in target {
            if let Some(info) = self.find_container_for_service(name).await? {
                if info.status.to_lowercase().contains("running") || info.status.to_lowercase().contains("up") { let _ = self.backend.stop(&info.id, None).await; }
                let _ = self.backend.remove(&info.id, true).await;
            }
        }
        if let Some(networks) = &self.spec.networks {
            for (name, conf) in networks {
                let conf = conf.as_ref().cloned().unwrap_or_default();
                if conf.external.unwrap_or(false) { continue; }
                let _ = self.backend.remove_network(conf.name.as_deref().unwrap_or(name)).await;
            }
        }
        if remove_orphans { self.remove_orphans().await?; }
        if remove_volumes {
            if let Some(volumes) = &self.spec.volumes {
                for (name, conf) in volumes {
                    let conf = conf.as_ref().cloned().unwrap_or_default();
                    if conf.external.unwrap_or(false) { continue; }
                    let _ = self.backend.remove_volume(conf.name.as_deref().unwrap_or(name)).await;
                }
            }
        }
        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut res = Vec::new();
        for (name, svc) in &self.spec.services {
            match self.find_container_for_service(name).await? {
                Some(info) => res.push(info),
                None => res.push(ContainerInfo { id: "".into(), name: "".into(), image: svc.image_ref(name), status: "not found".into(), ports: svc.port_strings(), labels: HashMap::new(), created: "".into() }),
            }
        }
        res.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(res)
    }

    pub async fn logs(&self, services: &[String], tail: Option<u32>) -> Result<HashMap<String, ContainerLogs>> {
        let mut res = HashMap::new();
        let targets = if services.is_empty() { self.spec.services.keys().cloned().collect() } else { services.to_vec() };
        for name in targets {
            if let Some(info) = self.find_container_for_service(&name).await? {
                res.insert(name, self.backend.logs(&info.id, tail, false).await?);
            }
        }
        Ok(res)
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let info = self.find_container_for_service(service).await?.ok_or_else(|| ComposeError::NotFound(service.to_string()))?;
        self.backend.exec(&info.id, cmd, None, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let targets = if services.is_empty() { self.spec.services.keys().cloned().collect() } else { services.to_vec() };
        for name in targets { if let Some(info) = self.find_container_for_service(&name).await? { self.backend.start(&info.id).await?; } }
        Ok(())
    }
    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let targets = if services.is_empty() { self.spec.services.keys().cloned().collect() } else { services.to_vec() };
        for name in targets { if let Some(info) = self.find_container_for_service(&name).await? { self.backend.stop(&info.id, None).await?; } }
        Ok(())
    }
    pub async fn restart(&self, services: &[String]) -> Result<()> { self.stop(services).await?; self.start(services).await }
    pub fn config(&self) -> Result<String> { serde_yaml::to_string(&self.spec).map_err(ComposeError::ParseError) }

    async fn remove_orphans(&self) -> Result<()> {
        for c in self.backend.list(true).await? {
            if c.labels.get("com.docker.compose.project").map(|v| v == &self.project_name).unwrap_or(false) {
                if let Some(s) = c.labels.get("com.docker.compose.service") {
                    if !self.spec.services.contains_key(s) {
                        if c.status.to_lowercase().contains("running") { let _ = self.backend.stop(&c.id, None).await; }
                        let _ = self.backend.remove(&c.id, true).await;
                    }
                }
            }
        }
        Ok(())
    }

    async fn find_container_for_service(&self, svc: &str) -> Result<Option<ContainerInfo>> {
        for c in self.backend.list(true).await? {
            if c.labels.get("com.docker.compose.project").map(|v| v == &self.project_name).unwrap_or(false) && c.labels.get("com.docker.compose.service").map(|v| v == svc).unwrap_or(false) { return Ok(Some(c)); }
        }
        Ok(None)
    }
}

pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
    let mut in_degree = IndexMap::new();
    let mut deps = IndexMap::new();
    for name in spec.services.keys() { in_degree.insert(name.clone(), 0); deps.insert(name.clone(), Vec::new()); }
    for (name, svc) in &spec.services {
        if let Some(d) = &svc.depends_on {
            for dep in d.service_names() {
                if !spec.services.contains_key(&dep) { return Err(ComposeError::validation(format!("Service '{}' depends on '{}' not defined", name, dep))); }
                *in_degree.get_mut(name).unwrap() += 1;
                deps.get_mut(&dep).unwrap().push(name.clone());
            }
        }
    }
    let mut queue: std::collections::BTreeSet<String> = in_degree.iter().filter(|(_, &d)| d == 0).map(|(n, _)| n.clone()).collect();
    let mut order = Vec::new();
    while let Some(s) = queue.pop_first() {
        order.push(s.clone());
        if let Some(dependent_list) = deps.get(&s) {
            for dep in dependent_list {
                let d = in_degree.get_mut(dep).unwrap();
                *d -= 1;
                if *d == 0 { queue.insert(dep.clone()); }
            }
        }
    }
    if order.len() != spec.services.len() {
        let cycle = in_degree.iter().filter(|(_, &d)| d > 0).map(|(n, _)| n.clone()).collect();
        return Err(ComposeError::DependencyCycle { services: cycle });
    }
    Ok(order)
}
