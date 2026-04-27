use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec,
};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use crate::backend::ContainerBackend;

static COMPOSE_ENGINES: once_cell::sync::Lazy<std::sync::Mutex<IndexMap<u64, Arc<ComposeEngine>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(IndexMap::new()));

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
    pub fn new(
        spec: ComposeSpec,
        project_name: String,
        backend: Arc<dyn ContainerBackend>,
    ) -> Self {
        ComposeEngine {
            spec,
            project_name,
            backend,
            session_containers: Mutex::new(Vec::new()),
            session_networks: Mutex::new(Vec::new()),
            session_volumes: Mutex::new(Vec::new()),
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
        build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        // 1. Create networks (skip external)
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let is_external = config.as_ref().map(|c| c.external.unwrap_or(false)).unwrap_or(false);
                if is_external { continue; }

                if self.backend.inspect_network(name).await.is_err() {
                    if let Some(cfg) = config {
                        self.backend.create_network(name, cfg).await?;
                    } else {
                        self.backend.create_network(name, &Default::default()).await?;
                    }
                    self.session_networks.lock().unwrap().push(name.clone());
                }
            }
        }

        // 2. Create volumes (skip external)
        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                let is_external = config.as_ref().map(|c| c.external.unwrap_or(false)).unwrap_or(false);
                if is_external { continue; }

                if self.backend.inspect_volume(name).await.is_err() {
                    if let Some(cfg) = config {
                        self.backend.create_volume(name, cfg).await?;
                    } else {
                        self.backend.create_volume(name, &Default::default()).await?;
                    }
                    self.session_volumes.lock().unwrap().push(name.clone());
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

            // Idempotency check
            let mut skip = false;
            if let Ok(info) = self.backend.inspect(&container_name).await {
                if info.status.contains("running") || info.status.contains("Up") {
                    tracing::debug!(service = %svc_name, "already running");
                    skip = true;
                } else {
                    tracing::debug!(service = %svc_name, "restarting stopped container");
                    self.backend.start(&container_name).await?;
                    skip = true;
                }
            }

            if skip { continue; }

            // Build if needed
            if build || svc.needs_build() {
                if let Some(build_spec) = &svc.build {
                    tracing::info!(service = %svc_name, "building image");
                    let image_name = svc.image_ref(svc_name);
                    self.backend.build(&build_spec.as_build(), &image_name).await?;
                }
            }

            // Pull if no build and image not present
            if !svc.needs_build() {
                if let Some(image) = &svc.image {
                    if self.backend.inspect_image(image).await.is_err() {
                        tracing::info!(image = %image, "pulling image");
                        self.backend.pull_image(image).await?;
                    }
                }
            }

            let network = svc.networks.as_ref().and_then(|n| n.names().first().cloned());
            let mut labels = svc.labels.as_ref().map(|l| l.to_map()).unwrap_or_default();
            labels.insert("perry.compose.project".to_string(), self.project_name.clone());
            labels.insert("perry.compose.service".to_string(), svc_name.clone());

            let container_spec = ContainerSpec {
                image: svc.image_ref(svc_name),
                name: Some(container_name.clone()),
                ports: Some(svc.port_strings()),
                volumes: Some(svc.volume_strings()),
                env: Some(svc.resolved_env()),
                cmd: svc.command_list(),
                entrypoint: None,
                network,
                rm: None,
                read_only: svc.read_only,
                labels: Some(labels),
            };

            let profile = crate::backend::SecurityProfile {
                read_only_root: svc.read_only.unwrap_or(false),
                seccomp: svc.security_opt.as_ref().and_then(|opts| {
                    opts.iter().find(|o| o.starts_with("seccomp=")).map(|o| o[8..].to_string())
                }),
            };

            match self.backend.run_with_security(&container_spec, &profile).await {
                Ok(handle) => {
                    self.session_containers.lock().unwrap().push(handle.id);
                }
                Err(e) => {
                    tracing::error!(service = %svc_name, error = %e, "startup failed, rolling back");
                    self.rollback().await;
                    return Err(ComposeError::ServiceStartupFailed {
                        service: svc_name.clone(),
                        message: e.to_string(),
                    });
                }
            }
        }

        Ok(self.register())
    }

    async fn rollback(&self) {
        let containers = self.session_containers.lock().unwrap().drain(..).collect::<Vec<_>>();
        for id in containers.into_iter().rev() {
            let _ = self.backend.stop(&id, Some(5)).await;
            let _ = self.backend.remove(&id, true).await;
        }

        let networks = self.session_networks.lock().unwrap().drain(..).collect::<Vec<_>>();
        for name in networks.into_iter().rev() {
            let _ = self.backend.remove_network(&name).await;
        }

        let volumes = self.session_volumes.lock().unwrap().drain(..).collect::<Vec<_>>();
        for name in volumes.into_iter().rev() {
            let _ = self.backend.remove_volume(&name).await;
        }
    }

    pub async fn down(
        &self,
        services: &[String],
        _remove_orphans: bool,
        remove_volumes: bool,
    ) -> Result<()> {
        // 1. Clean up session tracked resources
        self.rollback().await;

        // 2. Clean up requested services (even if not in session)
        let order = resolve_startup_order(&self.spec)?;
        let mut target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        // Teardown in reverse dependency order
        target.reverse();

        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, svc_name);

            // Try to stop and remove by name
            let _ = self.backend.stop(&container_name, Some(10)).await;
            let _ = self.backend.remove(&container_name, true).await;

            // Also check for containers with project/service labels
            if let Ok(all_containers) = self.backend.list(true).await {
                for c in all_containers {
                    let matches_project = c.labels.get("perry.compose.project").map(|v| v == &self.project_name).unwrap_or(false);
                    let matches_service = c.labels.get("perry.compose.service").map(|v| v == svc_name).unwrap_or(false);
                    if matches_project && matches_service {
                        let _ = self.backend.stop(&c.id, Some(10)).await;
                        let _ = self.backend.remove(&c.id, true).await;
                    }
                }
            }
        }

        // 3. Remove non-external networks
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let is_external = config.as_ref().map(|c| c.external.unwrap_or(false)).unwrap_or(false);
                if !is_external {
                    let _ = self.backend.remove_network(name).await;
                }
            }
        }

        // 4. Remove non-external volumes if requested
        if remove_volumes {
            if let Some(volumes) = &self.spec.volumes {
                for (name, config) in volumes {
                    let is_external = config.as_ref().map(|c| c.external.unwrap_or(false)).unwrap_or(false);
                    if !is_external {
                        let _ = self.backend.remove_volume(name).await;
                    }
                }
            }
        }

        // 5. Unregister engine
        // We need to find our stack_id. We can do this by searching COMPOSE_ENGINES.
        let mut engines = COMPOSE_ENGINES.lock().unwrap();
        let mut to_remove = None;
        for (id, engine) in engines.iter() {
            if Arc::ptr_eq(engine, &Arc::new(ComposeEngine::new(self.spec.clone(), self.project_name.clone(), self.backend.clone()))) {
                // This ptr_eq won't work easily because we're inside &self.
                // We'll search by project name and spec instead.
            }
            if engine.project_name == self.project_name {
                to_remove = Some(*id);
                break;
            }
        }
        if let Some(id) = to_remove {
            engines.shift_remove(&id);
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
