use crate::error::{ComposeError, Result};
use crate::service;
use crate::types::{
    ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec,
};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use crate::backend::ContainerBackend;

static COMPOSE_ENGINES: once_cell::sync::Lazy<std::sync::Mutex<IndexMap<u64, Arc<ComposeEngine>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(IndexMap::new()));

static NEXT_STACK_ID: AtomicU64 = AtomicU64::new(1);

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
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
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                self.backend.create_network(name, &config.clone().unwrap_or_default().into()).await?;
            }
        }
        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                self.backend.create_volume(name, &config.clone().unwrap_or_default().into()).await?;
            }
        }

        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        let mut started = Vec::new();
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, &self.project_name, svc_name);

            let network = match &svc.networks {
                Some(crate::types::ServiceNetworks::List(l)) => l.first().cloned(),
                Some(crate::types::ServiceNetworks::Map(m)) => m.keys().next().cloned(),
                None => None,
            };

            let container_spec = ContainerSpec {
                image: svc.image.clone().unwrap_or_default(),
                name: Some(container_name.clone()),
                ports: Some(svc.ports.as_ref().map(|p| p.iter().map(|ps| match ps {
                    crate::types::PortSpec::Short(v) => v.as_str().unwrap_or_default().to_string(),
                    crate::types::PortSpec::Long(lp) => format!("{}:{}", lp.published.as_ref().and_then(|v| v.as_str()).unwrap_or_default(), lp.target.as_str().unwrap_or_default()),
                }).collect()).unwrap_or_default()),
                volumes: Some(svc.volumes.as_ref().map(|v| v.iter().map(|vs| vs.as_str().unwrap_or_default().to_string()).collect()).unwrap_or_default()),
                env: Some(svc.environment.as_ref().map(|e| e.to_map()).unwrap_or_default()),
                cmd: Some(match &svc.command {
                    Some(serde_yaml::Value::String(s)) => vec![s.clone()],
                    Some(serde_yaml::Value::Sequence(seq)) => seq.iter().map(|v| v.as_str().unwrap_or_default().to_string()).collect(),
                    _ => vec![],
                }),
                entrypoint: None,
                network,
                rm: None,
            };

            match self.backend.run(&container_spec).await {
                Ok(_) => started.push(container_name),
                Err(e) => {
                    for name in started.iter().rev() {
                        let _ = self.backend.stop(name, Some(10)).await;
                        let _ = self.backend.remove(name, true).await;
                    }
                    return Err(ComposeError::ServiceStartupFailed { service: svc_name.clone(), message: e.to_string() });
                }
            }
        }
        Ok(self.register())
    }

    pub async fn down(&self, services: &[String], _remove_orphans: bool, remove_volumes: bool) -> Result<()> {
        let order = resolve_startup_order(&self.spec)?;
        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order.iter().filter(|s| services.contains(s)).collect()
        };

        for svc_name in target.iter().rev() {
            let svc = self.spec.services.get(*svc_name).unwrap();
            let container_name = service::service_container_name(svc, &self.project_name, svc_name);
            let _ = self.backend.stop(&container_name, Some(10)).await;
            let _ = self.backend.remove(&container_name, true).await;
        }

        if let Some(networks) = &self.spec.networks {
            for name in networks.keys() { let _ = self.backend.remove_network(name).await; }
        }
        if remove_volumes {
            if let Some(volumes) = &self.spec.volumes {
                for name in volumes.keys() { let _ = self.backend.remove_volume(name).await; }
            }
        }
        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let mut infos = Vec::new();
        for (svc_name, svc) in &self.spec.services {
            let container_name = service::service_container_name(svc, &self.project_name, svc_name);
            if let Ok(info) = self.backend.inspect(&container_name).await { infos.push(info); }
        }
        Ok(infos)
    }

    pub async fn logs(&self, services: &[String], tail: Option<u32>) -> Result<HashMap<String, String>> {
        let mut all_logs = HashMap::new();
        let target: Vec<&String> = if services.is_empty() { self.spec.services.keys().collect() } else { services.iter().collect() };
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, &self.project_name, svc_name);
            if let Ok(logs) = self.backend.logs(&container_name, tail).await {
                all_logs.insert(svc_name.clone(), format!("STDOUT:\n{}\nSTDERR:\n{}", logs.stdout, logs.stderr));
            }
        }
        Ok(all_logs)
    }

    pub async fn exec(&self, service: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let svc = self.spec.services.get(service).ok_or_else(|| ComposeError::NotFound(service.into()))?;
        let container_name = service::service_container_name(svc, &self.project_name, service);
        self.backend.exec(&container_name, cmd, env, workdir).await
    }

    pub fn config(&self) -> Result<String> { serde_yaml::to_string(&self.spec).map_err(ComposeError::ParseError) }
    pub async fn start(&self, services: &[String]) -> Result<()> {
        let target: Vec<&String> = if services.is_empty() { self.spec.services.keys().collect() } else { services.iter().collect() };
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, &self.project_name, svc_name);
            self.backend.start(&container_name).await?;
        }
        Ok(())
    }
    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let target: Vec<&String> = if services.is_empty() { self.spec.services.keys().collect() } else { services.iter().collect() };
        for svc_name in target {
            let svc = self.spec.services.get(svc_name).unwrap();
            let container_name = service::service_container_name(svc, &self.project_name, svc_name);
            self.backend.stop(&container_name, None).await?;
        }
        Ok(())
    }
    pub async fn restart(&self, services: &[String]) -> Result<()> { self.stop(services).await?; self.start(services).await }
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
            if *deg == 0 { queue.insert(dependent); }
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

pub fn get_compose_engine(stack_id: u64) -> Option<Arc<ComposeEngine>> {
    COMPOSE_ENGINES.lock().unwrap().get(&stack_id).cloned()
}
