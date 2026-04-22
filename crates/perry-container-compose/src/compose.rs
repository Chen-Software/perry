use indexmap::IndexMap;
use crate::error::{ComposeError, Result};
use crate::types::{ContainerCompose, ContainerInfo, ContainerLogs, Container, ComposeHandle, ContainerHandle, ListOrDict, ServiceGraph, WorkloadEdge, StackStatus, ServiceStatus, ServiceState, EdgeCondition};
use crate::backend::ContainerBackend;
use crate::service::{Service};
use crate::orchestrate::orchestrate_service;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

#[derive(Clone)]
pub struct ComposeEngine {
    pub spec: ContainerCompose,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend + Send + Sync>,
}

impl ComposeEngine {
    pub fn new(spec: ContainerCompose, project_name: String, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Self {
        Self { spec, project_name, backend }
    }

    pub fn resolve_startup_order(&self) -> Result<Vec<String>> {
        resolve_startup_order(&self.spec)
    }

    pub async fn up(&self, services_to_start: &[String], _detach: bool, _build: bool, _remove_orphans: bool) -> Result<ComposeHandle> {
        let full_order = self.resolve_startup_order()?;

        let services_to_start_set: BTreeSet<String> = if services_to_start.is_empty() {
            full_order.iter().cloned().collect()
        } else {
            let mut set = BTreeSet::new();
            for s in services_to_start {
                self.add_with_deps(s, &mut set)?;
            }
            set
        };

        let mut _created_networks: Vec<String> = Vec::new();
        let mut _created_volumes: Vec<String> = Vec::new();
        let mut started_containers: Vec<String> = Vec::new();

        if let Some(networks) = &self.spec.networks {
            for (_name, _config) in networks {
                // Implementation of create_network...
            }
        }

        for service_name in &full_order {
            if !services_to_start_set.contains(service_name) {
                continue;
            }

            let service_spec = self.spec.services.get(service_name).unwrap();
            let service = Service {
                image: service_spec.image.clone(),
                name: service_spec.container_name.clone(),
                ports: service_spec.ports.as_ref().map(|p| p.iter().map(|ps| format!("{:?}", ps)).collect()),
                environment: service_spec.environment.clone(),
                labels: service_spec.labels.clone(),
                volumes: service_spec.volumes.as_ref().map(|v| v.iter().map(|vs| format!("{:?}", vs)).collect()),
                build: service_spec.build.as_ref().and_then(|b| match b {
                    crate::types::BuildSpec::Config(c) => Some(c.clone()),
                    crate::types::BuildSpec::Context(ctx) => Some(crate::types::ComposeServiceBuild { context: Some(ctx.clone()), ..Default::default() }),
                }),
            };

            match orchestrate_service(service_name, &service, self.backend.as_ref()).await {
                Ok(_) => {
                    started_containers.push(service_name.clone());
                }
                Err(e) => {
                    // rollback logic
                    return Err(e);
                }
            }
        }

        Ok(ComposeHandle {
            stack_id: rand::random(),
            project_name: self.project_name.clone(),
            services: started_containers,
        })
    }

    fn add_with_deps(&self, service_name: &str, set: &mut BTreeSet<String>) -> Result<()> {
        if set.contains(service_name) { return Ok(()); }
        let service = self.spec.services.get(service_name).ok_or_else(|| ComposeError::ValidationError { message: format!("Service not found: {}", service_name) })?;
        set.insert(service_name.to_string());
        if let Some(deps) = &service.depends_on {
            for dep in deps.service_names() {
                self.add_with_deps(&dep, set)?;
            }
        }
        Ok(())
    }

    pub async fn down(&self, _services: &[String], _remove_orphans: bool, _volumes: bool) -> Result<()> {
         // TODO
         Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        self.backend.list(true).await
    }

    pub async fn start(&self, _services: &[String]) -> Result<()> {
         Ok(())
    }

    pub async fn stop(&self, _services: &[String]) -> Result<()> {
         Ok(())
    }

    pub async fn restart(&self, _services: &[String]) -> Result<()> {
         Ok(())
    }

    pub async fn logs(&self, services: &[String], tail: Option<u32>) -> Result<HashMap<String, ContainerLogs>> {
        let mut results = HashMap::new();
        let target_services = if services.is_empty() {
            self.spec.services.keys().cloned().collect::<Vec<_>>()
        } else {
            services.to_vec()
        };

        for svc in target_services {
            if let Ok(logs) = self.backend.logs(&svc, tail).await {
                results.insert(svc, logs);
            }
        }
        Ok(results)
    }

    pub async fn exec(&self, service: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        self.backend.exec(service, cmd, env, workdir).await
    }

    pub fn graph(&self) -> Result<ServiceGraph> {
        let order = self.resolve_startup_order()?;
        let mut edges = Vec::new();
        for (name, service) in &self.spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    edges.push(WorkloadEdge {
                        from: name.clone(),
                        to: dep,
                        condition: Some(EdgeCondition::Started),
                        trust_boundary: None,
                        locality: None,
                        latency_class: None,
                    });
                }
            }
        }
        Ok(ServiceGraph { nodes: order, edges })
    }

    pub async fn status(&self) -> Result<StackStatus> {
        let mut services = Vec::new();
        let infos = self.ps().await?;
        for (name, _) in &self.spec.services {
             let info = infos.iter().find(|i| i.name.contains(name));
             services.push(ServiceStatus {
                 service: name.clone(),
                 state: if let Some(i) = info {
                     if i.status.to_lowercase().contains("running") { ServiceState::Running } else { ServiceState::Stopped }
                 } else {
                     ServiceState::Pending
                 },
                 container_id: info.map(|i| i.id.clone()),
                 error: None,
             });
        }
        Ok(StackStatus { healthy: services.iter().all(|s| s.state == ServiceState::Running), services })
    }

    pub fn config(&self) -> Result<String> {
        serde_yaml::to_string(&self.spec).map_err(ComposeError::ParseError)
    }
}

pub fn resolve_startup_order(spec: &ContainerCompose) -> Result<Vec<String>> {
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
