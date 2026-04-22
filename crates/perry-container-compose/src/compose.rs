use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use crate::backend::ContainerBackend;

pub static COMPOSE_ENGINES: once_cell::sync::Lazy<std::sync::Mutex<IndexMap<u64, Arc<ComposeEngine>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(IndexMap::new()));

static NEXT_STACK_ID: AtomicU64 = AtomicU64::new(1);

use indexmap::IndexMap;
use crate::service::Service;

pub struct Compose {
    pub services: IndexMap<String, Service>,
}

impl Compose {
    pub fn parse(yaml: &str) -> Result<Self> {
        let spec = ComposeSpec::parse_str(yaml)?;
        let mut services = IndexMap::new();
        for (name, svc_spec) in spec.services {
            let mut service = Service::new(name.clone(), svc_spec.image.clone());
            service.container_name = svc_spec.container_name.clone();
            service.ports = Some(svc_spec.port_strings());
            service.environment = svc_spec.environment.clone();
            service.labels = svc_spec.labels.clone();
            service.volumes = Some(svc_spec.volume_strings());
            service.build = svc_spec.build.clone();
            services.insert(name, service);
        }
        Ok(Compose { services })
    }
}

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
}

impl ComposeEngine {
    pub fn resolve_startup_order(&self) -> crate::error::Result<Vec<String>> {
        resolve_startup_order(&self.spec)
    }

    pub fn new(
        spec: ComposeSpec,
        project_name: String,
        backend: Arc<dyn ContainerBackend>,
    ) -> Self {
        ComposeEngine {
            spec,
            project_name,
            backend,
        }
    }

    fn register(&self) -> ComposeHandle {
        let stack_id = NEXT_STACK_ID.fetch_add(1, Ordering::SeqCst);
        let services: Vec<String> = self.spec.services.keys().cloned().collect();
        let handle = ComposeHandle {
            stack_id,
            project_name: self.project_name.clone(),
            services,
        };
        COMPOSE_ENGINES.lock().unwrap().insert(stack_id, Arc::new(ComposeEngine::new(
            self.spec.clone(),
            self.project_name.clone(),
            Arc::clone(&self.backend),
        )));
        handle
    }

    pub async fn up(
        &self,
        services: &[String],
        _detach: bool,
        _build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        let mut created_networks = Vec::new();
        let mut created_volumes = Vec::new();
        let mut started_containers = Vec::new();

        // 1. Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let res = if let Some(cfg) = config {
                    self.backend.create_network(name, cfg).await
                } else {
                    self.backend.create_network(name, &Default::default()).await
                };
                match res {
                    Ok(()) => created_networks.push(name.clone()),
                    Err(e) => {
                        let msg = e.to_string().to_lowercase();
                        if !msg.contains("already exists") {
                            self.rollback(&started_containers, &created_networks, &created_volumes).await;
                            return Err(e);
                        }
                    }
                }
            }
        }

        // 2. Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                let res = if let Some(cfg) = config {
                    self.backend.create_volume(name, cfg).await
                } else {
                    self.backend.create_volume(name, &Default::default()).await
                };
                match res {
                    Ok(()) => created_volumes.push(name.clone()),
                    Err(e) => {
                        let msg = e.to_string().to_lowercase();
                        if !msg.contains("already exists") {
                            self.rollback(&started_containers, &created_networks, &created_volumes).await;
                            return Err(e);
                        }
                    }
                }
            }
        }

        // 3. Resolve order and start services
        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);

            let mut service = Service::new(svc_name.clone(), svc.image.clone());
            service.container_name = Some(container_name.clone());
            service.ports = Some(svc.port_strings());
            service.environment = svc.environment.clone();
            service.labels = svc.labels.clone();
            service.volumes = Some(svc.volume_strings());
            service.build = svc.build.clone();
            service.stable_id = Some(rand::random());

            match crate::orchestrate::orchestrate_service(&service, self.backend.as_ref()).await {
                Ok(_) => {
                    started_containers.push(container_name);
                }
                Err(e) => {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(ComposeError::ServiceStartupFailed {
                        service: svc_name.clone(),
                        message: e.to_string(),
                    });
                }
            }
        }

        Ok(self.register())
    }

    async fn rollback(&self, containers: &[String], networks: &[String], volumes: &[String]) {
        for name in containers.iter().rev() {
            let _ = self.backend.stop(name, Some(10)).await;
            let _ = self.backend.remove(name, true).await;
        }
        for name in networks {
            let _ = self.backend.remove_network(name).await;
        }
        for name in volumes {
            let _ = self.backend.remove_volume(name).await;
        }
    }

    pub async fn down(
        &self,
        services: &[String],
        _remove_orphans: bool,
        remove_volumes: bool,
    ) -> Result<()> {
        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        for svc_name in target.iter().rev() {
            let svc = self.spec.services.get(*svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            let _ = self.backend.stop(&container_name, Some(10)).await;
            let _ = self.backend.remove(&container_name, true).await;
        }

        if let Some(networks) = &self.spec.networks {
            for name in networks.keys() {
                let _ = self.backend.remove_network(name).await;
            }
        }

        if remove_volumes {
            if let Some(volumes) = &self.spec.volumes {
                for name in volumes.keys() {
                    let _ = self.backend.remove_volume(name).await;
                }
            }
        }

        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut infos = Vec::new();
        for (svc_name, svc) in &self.spec.services {
            let container_name = service::service_container_name(svc, svc_name);
            if let Ok(info) = self.backend.inspect(&container_name).await {
                infos.push(info);
            }
        }
        Ok(infos)
    }

    pub async fn logs(
        &self,
        services: &[String],
        tail: Option<u32>,
    ) -> Result<HashMap<String, String>> {
        let mut all_logs = HashMap::new();
        let target: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };

        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            if let Ok(logs) = self.backend.logs(&container_name, tail).await {
                all_logs.insert(svc_name.clone(), format!("STDOUT:\n{}\nSTDERR:\n{}", logs.stdout, logs.stderr));
            }
        }
        Ok(all_logs)
    }

    pub async fn exec(
        &self,
        service: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        let svc = self.spec.services.get(service).ok_or_else(|| ComposeError::NotFound(service.into()))?;
        let container_name = service::service_container_name(svc, service);
        self.backend.exec(&container_name, cmd, env, workdir).await
    }

    pub fn config(&self) -> Result<String> {
        serde_yaml::to_string(&self.spec).map_err(ComposeError::ParseError)
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let target: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            self.backend.start(&container_name).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let target: Vec<&String> = if services.is_empty() {
            self.spec.services.keys().collect()
        } else {
            services.iter().collect()
        };
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);
            self.backend.stop(&container_name, None).await?;
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
        return Err(ComposeError::DependencyCycle { services: cycle_services });
    }

    Ok(order)
}
