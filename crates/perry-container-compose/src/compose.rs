use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

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
    /// Cached `service_name → container_name` map, populated by `up()`.
    ///
    /// `service::service_container_name` regenerates a fresh random suffix
    /// per call (`{md5_8}-{random_hex8}`), so any post-`up` operation
    /// (`exec`, `logs`, `down`, `ps`) that recomputes the name from the
    /// service spec ends up with a different name than the one the
    /// container was actually created with → "No such container" errors.
    /// `up()` resolves the name once at startup and stores it here; later
    /// methods read this map instead of regenerating.
    service_container_names: Mutex<HashMap<String, String>>,
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
            service_container_names: Mutex::new(HashMap::new()),
        }
    }

    /// Resolve the container name for a given service, preferring the cached
    /// name set during `up()` and falling back to a fresh derivation only
    /// when no entry exists yet (e.g. for callers that operate on services
    /// before `up()` registered them — rare).
    pub fn resolve_container_name(&self, service_name: &str) -> String {
        if let Some(cached) = self
            .service_container_names
            .lock()
            .unwrap()
            .get(service_name)
            .cloned()
        {
            return cached;
        }
        let svc = self.spec.services.get(service_name);
        match svc {
            Some(s) => service::service_container_name(s, service_name),
            None => format!("{}-unknown", service_name),
        }
    }

    fn cache_container_name(&self, service_name: &str, container_name: &str) {
        self.service_container_names
            .lock()
            .unwrap()
            .insert(service_name.to_string(), container_name.to_string());
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
        // 1. Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let external = config.as_ref().and_then(|c| c.external).unwrap_or(false);
                let exists = self.backend.inspect_network(name).await.is_ok();

                if external {
                    if !exists {
                        return Err(ComposeError::ValidationError {
                            message: format!("External network '{}' not found", name),
                        });
                    }
                    // Do not track external networks for rollback
                } else if !exists {
                    if let Some(cfg) = config {
                        self.backend.create_network(name, cfg).await?;
                    } else {
                        self.backend
                            .create_network(name, &Default::default())
                            .await?;
                    }
                    self.session_networks.lock().unwrap().push(name.clone());
                }
            }
        }

        // 2. Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                let external = config.as_ref().and_then(|c| c.external).unwrap_or(false);
                let exists = self.backend.inspect_volume(name).await.is_ok();

                if external {
                    if !exists {
                        return Err(ComposeError::ValidationError {
                            message: format!("External volume '{}' not found", name),
                        });
                    }
                    // Do not track external volumes for rollback
                } else if !exists {
                    if let Some(cfg) = config {
                        self.backend.create_volume(name, cfg).await?;
                    } else {
                        self.backend
                            .create_volume(name, &Default::default())
                            .await?;
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

        let mut started = Vec::new();
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();

            // 3a. Build if necessary
            if svc.needs_build() && (build || self.backend.inspect_image(&svc.image_ref(svc_name)).await.is_err()) {
                if let Some(build_spec) = &svc.build {
                    self.backend.build(&build_spec.as_build(), &svc.image_ref(svc_name)).await?;
                }
            }

            // 3b. Resolve container name, checking for existing labeled containers first
            let mut container_name = self
                .service_container_names
                .lock()
                .unwrap()
                .get(svc_name)
                .cloned()
                .unwrap_or_default();

            if container_name.is_empty() {
                // Try to find by labels
                let containers = self.backend.list(true).await?;
                if let Some(existing) = containers.into_iter().find(|c| {
                    c.labels.get("perry.compose.project") == Some(&self.project_name) &&
                    c.labels.get("perry.compose.service") == Some(svc_name)
                }) {
                    container_name = existing.name;
                } else {
                    container_name = service::service_container_name(svc, svc_name);
                }
                self.cache_container_name(svc_name, &container_name);
            }

            // Extract primary network if any
            let network = match &svc.networks {
                Some(crate::types::ServiceNetworks::List(l)) => l.first().cloned(),
                Some(crate::types::ServiceNetworks::Map(m)) => m.keys().next().cloned(),
                None => None,
            };

            let mut labels = svc.labels.as_ref().map(|l| l.to_map()).unwrap_or_default();
            labels.insert(
                "perry.compose.project".to_string(),
                self.project_name.clone(),
            );
            labels.insert("perry.compose.service".to_string(), svc_name.clone());

            let container_spec = ContainerSpec {
                image: svc.image.clone().unwrap_or_default(),
                name: Some(container_name.clone()),
                ports: Some(
                    svc.ports
                        .as_ref()
                        .map(|p| {
                            p.iter()
                                .map(|ps| match ps {
                                    crate::types::PortSpec::Short(v) => match v {
                                        serde_yaml::Value::String(s) => s.clone(),
                                        serde_yaml::Value::Number(n) => n.to_string(),
                                        _ => v.as_str().unwrap_or_default().to_string(),
                                    },
                                    crate::types::PortSpec::Long(lp) => {
                                        let publ = lp
                                            .published
                                            .as_ref()
                                            .map(|v| match v {
                                                serde_yaml::Value::String(s) => s.clone(),
                                                serde_yaml::Value::Number(n) => n.to_string(),
                                                _ => v.as_str().unwrap_or_default().to_string(),
                                            })
                                            .unwrap_or_default();
                                        let target = match &lp.target {
                                            serde_yaml::Value::String(s) => s.clone(),
                                            serde_yaml::Value::Number(n) => n.to_string(),
                                            _ => lp.target.as_str().unwrap_or_default().to_string(),
                                        };
                                        format!("{}:{}", publ, target)
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                ),
                volumes: Some(
                    svc.volumes
                        .as_ref()
                        .map(|v| {
                            v.iter()
                                .map(|vs| match vs {
                                    serde_yaml::Value::String(s) => s.clone(),
                                    _ => vs.as_str().unwrap_or_default().to_string(),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                ),
                env: Some(match &svc.environment {
                    Some(crate::types::ListOrDict::Dict(d)) => d
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                v.as_ref()
                                    .map(|vv| match vv {
                                        serde_yaml::Value::String(s) => s.clone(),
                                        serde_yaml::Value::Number(n) => n.to_string(),
                                        serde_yaml::Value::Bool(b) => b.to_string(),
                                        _ => vv.as_str().unwrap_or_default().to_string(),
                                    })
                                    .unwrap_or_default(),
                            )
                        })
                        .collect(),
                    Some(crate::types::ListOrDict::List(l)) => l
                        .iter()
                        .filter_map(|s| s.split_once('='))
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                    None => HashMap::new(),
                }),
                cmd: Some(match &svc.command {
                    Some(serde_yaml::Value::String(s)) => vec![s.clone()],
                    Some(serde_yaml::Value::Sequence(seq)) => seq
                        .iter()
                        .map(|v| v.as_str().unwrap_or_default().to_string())
                        .collect(),
                    _ => vec![],
                }),
                entrypoint: None,
                network,
                rm: None,
                read_only: svc.read_only,
                labels: Some(labels),
                privileged: svc.privileged,
                user: svc.user.clone(),
                workdir: svc.working_dir.clone(),
                cap_add: svc.cap_add.clone(),
                cap_drop: svc.cap_drop.clone(),
            };

            let profile = crate::backend::SecurityProfile {
                read_only_root: svc.read_only.unwrap_or(false),
                seccomp: None, // Could be parsed from security_opt
            };

            // Idempotency: skip if already running
            let mut skip = false;
            if let Ok(info) = self.backend.inspect(&container_name).await {
                if info.status == "running" {
                    skip = true;
                } else {
                    // Start existing stopped container
                    self.backend.start(&container_name).await?;
                    skip = true;
                }
            }

            if !skip {
                match self
                    .backend
                    .run_with_security(&container_spec, &profile)
                    .await
                {
                    Ok(handle) => {
                        self.session_containers.lock().unwrap().push(handle.id);
                        started.push(container_name);
                    }
                    Err(e) => {
                        // Rollback
                        self.rollback().await;
                        return Err(ComposeError::ServiceStartupFailed {
                            service: svc_name.clone(),
                            message: e.to_string(),
                        });
                    }
                }
            }
        }

        Ok(self.register())
    }

    async fn rollback(&self) {
        let containers = self
            .session_containers
            .lock()
            .unwrap()
            .drain(..)
            .collect::<Vec<_>>();
        for id in containers.into_iter().rev() {
            let _ = self.backend.stop(&id, Some(5)).await;
            let _ = self.backend.remove(&id, true).await;
        }

        let networks = self
            .session_networks
            .lock()
            .unwrap()
            .drain(..)
            .collect::<Vec<_>>();
        for name in networks.into_iter().rev() {
            let _ = self.backend.remove_network(&name).await;
        }

        let volumes = self
            .session_volumes
            .lock()
            .unwrap()
            .drain(..)
            .collect::<Vec<_>>();
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
        // `rollback()` removes `session_volumes` unconditionally — that's
        // correct semantics during an `up()` failure (those volumes were
        // just created and the caller wanted nothing to persist), but it
        // contradicts `remove_volumes=false` when called from `down()`.
        // Snapshot session_volumes around the rollback when the caller
        // opted to PRESERVE volumes so the unconditional drain inside
        // rollback doesn't strip them.
        if !remove_volumes {
            let saved_volumes: Vec<String> = self
                .session_volumes
                .lock()
                .unwrap()
                .drain(..)
                .collect();
            self.rollback().await;
            *self.session_volumes.lock().unwrap() = saved_volumes;
        } else {
            self.rollback().await;
        }

        // 2. Clean up requested services (even if not in session)
        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        let mut final_order = target;
        final_order.reverse();

        for svc_name in final_order {
            let container_info = self.backend.list(true).await?;
            let containers_to_remove: Vec<String> = container_info
                .into_iter()
                .filter(|c| {
                    c.labels
                        .get("perry.compose.project")
                        .map(|v| v == &self.project_name)
                        .unwrap_or(false)
                        && c.labels
                            .get("perry.compose.service")
                            .map(|v| v == svc_name)
                            .unwrap_or(false)
                })
                .map(|c| c.id)
                .collect();

            for cid in containers_to_remove {
                let _ = self.backend.stop(&cid, Some(10)).await;
                let _ = self.backend.remove(&cid, true).await;
            }

            let container_name = self.resolve_container_name(svc_name);
            let _ = self.backend.stop(&container_name, Some(10)).await;
            let _ = self.backend.remove(&container_name, true).await;
        }

        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let external = config.as_ref().and_then(|c| c.external).unwrap_or(false);
                if !external {
                    let _ = self.backend.remove_network(name).await;
                }
            }
        }

        if remove_volumes {
            if let Some(volumes) = &self.spec.volumes {
                for (name, config) in volumes {
                    let external = config.as_ref().and_then(|c| c.external).unwrap_or(false);
                    if !external {
                        let _ = self.backend.remove_volume(name).await;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut infos = Vec::new();
        for svc_name in self.spec.services.keys() {
            let container_name = self.resolve_container_name(svc_name);
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
            let container_name = self.resolve_container_name(svc_name);
            if let Ok(logs) = self.backend.logs(&container_name, tail).await {
                all_logs.insert(
                    svc_name.clone(),
                    format!("STDOUT:\n{}\nSTDERR:\n{}", logs.stdout, logs.stderr),
                );
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
        if !self.spec.services.contains_key(service) {
            return Err(ComposeError::NotFound(service.into()));
        }
        let container_name = self.resolve_container_name(service);
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
            let container_name = self.resolve_container_name(svc_name);
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
            let container_name = self.resolve_container_name(svc_name);
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
                        message: format!(
                            "Service '{}' depends on '{}' which is not defined",
                            name, dep
                        ),
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
        return Err(ComposeError::DependencyCycle {
            services: cycle_services,
        });
    }

    Ok(order)
}
