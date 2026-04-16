use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec,
};
use dashmap::DashMap;
use indexmap::IndexMap;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

static COMPOSE_HANDLES: OnceLock<DashMap<u64, Arc<ComposeEngine>>> = OnceLock::new();
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

pub fn get_engine(id: u64) -> Option<Arc<ComposeEngine>> {
    COMPOSE_HANDLES.get()?.get(&id).map(|e| Arc::clone(e.value()))
}

pub fn take_engine(id: u64) -> Option<Arc<ComposeEngine>> {
    COMPOSE_HANDLES.get()?.remove(&id).map(|(_, e)| e)
}

pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
    // 1. Build adjacency list: service → its dependencies
    let mut in_degree: IndexMap<String, usize> = IndexMap::new();
    let mut dependents: IndexMap<String, Vec<String>> = IndexMap::new();

    // Initialize all services with in-degree 0
    for name in spec.services.keys() {
        in_degree.insert(name.clone(), 0);
        dependents.insert(name.clone(), Vec::new());
    }

    // 2. Compute in-degrees from depends_on
    for (name, service) in &spec.services {
        if let Some(deps) = &service.depends_on {
            for dep in deps.service_names() {
                if !spec.services.contains_key(&dep) {
                    return Err(ComposeError::ValidationError {
                        message: format!("Service '{}' depends on '{}' which is not defined", name, dep),
                    });
                }
                // dep must start before name, so name has dep as a prerequisite
                *in_degree.get_mut(name).unwrap() += 1;
                dependents.get_mut(&dep).unwrap().push(name.clone());
            }
        }
    }

    // 3. Queue all services with in-degree 0 (sorted for determinism)
    let mut queue: BTreeSet<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(name, _)| name.clone())
        .collect();

    // 4. Process queue
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

    // 5. If not all services processed → cycle detected
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

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        Self {
            spec,
            project_name,
            backend,
        }
    }

    pub async fn up(
        self: Arc<Self>,
        services: &[String],
        _detach: bool,
        _build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        let order = resolve_startup_order(&self.spec)?;
        let mut started = Vec::new();

        // Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, net_config) in networks {
                if let Some(config) = net_config {
                    self.backend.create_network(name, config).await?;
                }
            }
        }

        // Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, vol_config) in volumes {
                if let Some(config) = vol_config {
                    self.backend.create_volume(name, config).await?;
                }
            }
        }

        for service_name in order {
            if !services.is_empty() && !services.contains(&service_name) {
                continue;
            }

            let service = self.spec.services.get(&service_name).unwrap();
            let container_name = service
                .explicit_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    service::generate_name(
                        &service.image_ref(&service_name),
                        &format!("{}-{}", self.project_name, service_name),
                    )
                });

            let spec = ContainerSpec {
                image: service.image_ref(&service_name),
                name: Some(container_name.clone()),
                ports: Some(service.port_strings()),
                volumes: Some(service.volume_strings()),
                env: Some(service.resolved_env()),
                cmd: service.command_list(),
                entrypoint: service.entrypoint.as_ref().and_then(|v| {
                    if let serde_yaml::Value::Sequence(seq) = v {
                        Some(seq.iter().filter_map(|val| val.as_str().map(|s| s.to_string())).collect())
                    } else if let serde_yaml::Value::String(s) = v {
                        Some(vec![s.clone()])
                    } else {
                        None
                    }
                }),
                network: None, // Networks are attached differently in OCI
                rm: Some(false),
                ..Default::default()
            };

            match self.backend.run(&spec).await {
                Ok(handle) => {
                    started.push(handle.id);
                }
                Err(e) => {
                    // Rollback: stop and remove started containers
                    for id in started.into_iter().rev() {
                        let _ = self.backend.stop(&id, Some(5)).await;
                        let _ = self.backend.remove(&id, true).await;
                    }
                    return Err(ComposeError::ServiceStartupFailed {
                        service: service_name,
                        message: e.to_string(),
                    });
                }
            }
        }

        let handle_id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
        let handles = COMPOSE_HANDLES.get_or_init(DashMap::new);
        handles.insert(handle_id, Arc::clone(&self));

        Ok(ComposeHandle {
            stack_id: handle_id,
            project_name: self.project_name.clone(),
            services: self.spec.services.keys().cloned().collect(),
        })
    }

    pub async fn down(&self, volumes: bool, _remove_orphans: bool) -> Result<()> {
        for (name, _service) in &self.spec.services {
            let container_name = format!("{}-{}", self.project_name, name);
            if let Ok(Some(c)) = service::get_container(self.backend.as_ref(), &container_name).await {
                let _ = self.backend.stop(&c.id, Some(10)).await;
                let _ = self.backend.remove(&c.id, true).await;
            }
        }

        if let Some(networks) = &self.spec.networks {
            for name in networks.keys() {
                let _ = self.backend.remove_network(name).await;
            }
        }

        if volumes {
            if let Some(vols) = &self.spec.volumes {
                for name in vols.keys() {
                    let _ = self.backend.remove_volume(name).await;
                }
            }
        }

        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut results = Vec::new();
        for name in self.spec.services.keys() {
            let container_name = format!("{}-{}", self.project_name, name);
            if let Ok(Some(c)) = service::get_container(self.backend.as_ref(), &container_name).await {
                results.push(c);
            }
        }
        Ok(results)
    }

    pub async fn logs(&self, service_name: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut stdout = String::new();
        let mut stderr = String::new();

        for (name, _) in &self.spec.services {
            if let Some(filter) = service_name {
                if filter != name {
                    continue;
                }
            }

            let container_name = format!("{}-{}", self.project_name, name);
            if let Ok(Some(c)) = service::get_container(self.backend.as_ref(), &container_name).await {
                if let Ok(logs) = self.backend.logs(&c.id, tail).await {
                    stdout.push_str(&format!("--- {} ---\n", name));
                    stdout.push_str(&logs.stdout);
                    stderr.push_str(&logs.stderr);
                }
            }
        }

        Ok(ContainerLogs { stdout, stderr })
    }

    pub async fn exec(&self, service_name: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let container_name = format!("{}-{}", self.project_name, service_name);
        if let Some(c) = service::get_container(self.backend.as_ref(), &container_name).await? {
            self.backend.exec(&c.id, cmd, None, None).await
        } else {
            Err(ComposeError::NotFound(format!("service {} not found", service_name)))
        }
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        for name in self.get_services(services) {
            let container_name = format!("{}-{}", self.project_name, name);
            if let Some(c) = service::get_container(self.backend.as_ref(), &container_name).await? {
                self.backend.start(&c.id).await?;
            }
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        for name in self.get_services(services) {
            let container_name = format!("{}-{}", self.project_name, name);
            if let Some(c) = service::get_container(self.backend.as_ref(), &container_name).await? {
                self.backend.stop(&c.id, None).await?;
            }
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.stop(services).await?;
        self.start(services).await?;
        Ok(())
    }

    fn get_services(&self, filter: &[String]) -> Vec<String> {
        if filter.is_empty() {
            self.spec.services.keys().cloned().collect()
        } else {
            filter.iter().cloned().collect()
        }
    }
}
