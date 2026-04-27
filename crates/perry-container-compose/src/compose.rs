use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use crate::backend::ContainerBackend;
use crate::error::ComposeError;
use crate::types::{ComposeSpec, StackStatus, ServiceStatus, ServiceState, ServiceGraph, ServiceEdge, ContainerLogs};
use crate::service::Service;
use crate::orchestrate::orchestrate_service;

pub struct ComposeEngine {
    pub backend: Arc<dyn ContainerBackend>,
}

#[derive(Clone)]
pub struct ComposeHandle {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub services: Vec<String>,
    pub backend: Arc<dyn ContainerBackend>,
}

impl ComposeEngine {
    pub fn new(backend: Arc<dyn ContainerBackend>) -> Self {
        Self { backend }
    }

    pub async fn up(&self, spec: &ComposeSpec) -> Result<ComposeHandle, ComposeError> {
        let order = resolve_startup_order(spec)?;

        if let Some(networks) = &spec.networks {
            for name in networks.keys() {
                self.backend.create_network(name).await?;
            }
        }
        if let Some(volumes) = &spec.volumes {
            for name in volumes.keys() {
                self.backend.create_volume(name).await?;
            }
        }

        let mut started = Vec::new();
        for service_name in order {
            let config = spec.services.get(&service_name).unwrap();
            let service = Service::new(service_name.clone(), config.clone());

            match orchestrate_service(&service, self.backend.as_ref()).await {
                Ok(_) => started.push(service_name),
                Err(e) => {
                    self.rollback(&started, spec).await;
                    return Err(e);
                }
            }
        }

        Ok(ComposeHandle {
            spec: spec.clone(),
            project_name: spec.name.clone().unwrap_or_else(|| "default".to_string()),
            services: started,
            backend: self.backend.clone(),
        })
    }

    async fn rollback(&self, started: &[String], spec: &ComposeSpec) {
        for name in started.iter().rev() {
            let config = spec.services.get(name).unwrap();
            let container_name = config.container_name.as_deref().unwrap_or(name);
            let _ = self.backend.stop(container_name, None).await;
            let _ = self.backend.remove(container_name, true).await;
        }
    }
}

impl ComposeHandle {
    pub async fn down(&self, _volumes: bool) -> Result<(), ComposeError> {
        for name in self.services.iter().rev() {
            let config = self.spec.services.get(name).unwrap();
            let container_name = config.container_name.as_deref().unwrap_or(name);
            let _ = self.backend.stop(container_name, None).await;
            let _ = self.backend.remove(container_name, true).await;
        }
        Ok(())
    }

    pub async fn ps(&self) -> Result<StackStatus, ComposeError> {
        let mut services = Vec::new();
        let mut healthy = true;

        for name in &self.services {
            let config = self.spec.services.get(name).unwrap();
            let container_name = config.container_name.as_deref().unwrap_or(name);
            let info = self.backend.inspect(container_name).await;
            match info {
                Ok(i) => {
                    let state = if i.status.contains("running") { ServiceState::Running } else { ServiceState::Stopped };
                    if state != ServiceState::Running { healthy = false; }
                    services.push(ServiceStatus {
                        service: name.clone(),
                        state,
                        container_id: Some(i.id),
                        error: None,
                    });
                }
                Err(e) => {
                    healthy = false;
                    services.push(ServiceStatus {
                        service: name.clone(),
                        state: ServiceState::Failed,
                        container_id: None,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(StackStatus { services, healthy })
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs, ComposeError> {
        if let Some(s) = service {
            let config = self.spec.services.get(s).ok_or_else(|| ComposeError::NotFound(s.to_string()))?;
            let container_name = config.container_name.as_deref().unwrap_or(s);
            self.backend.logs(container_name, tail).await
        } else {
            let mut all_stdout = String::new();
            let mut all_stderr = String::new();
            for s in &self.services {
                let config = self.spec.services.get(s).unwrap();
                let container_name = config.container_name.as_deref().unwrap_or(s);
                let l = self.backend.logs(container_name, tail).await?;
                all_stdout.push_str(&format!("[{}] {}\n", s, l.stdout));
                all_stderr.push_str(&format!("[{}] {}\n", s, l.stderr));
            }
            Ok(ContainerLogs { stdout: all_stdout, stderr: all_stderr })
        }
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs, ComposeError> {
        let config = self.spec.services.get(service).ok_or_else(|| ComposeError::NotFound(service.to_string()))?;
        let container_name = config.container_name.as_deref().unwrap_or(service);
        self.backend.exec(container_name, cmd, None, None).await
    }

    pub async fn start(&self, services: Option<Vec<String>>) -> Result<(), ComposeError> {
        let targets = services.as_ref().unwrap_or(&self.services);
        for s in targets {
            let config = self.spec.services.get(s).ok_or_else(|| ComposeError::NotFound(s.to_string()))?;
            let container_name = config.container_name.as_deref().unwrap_or(s);
            self.backend.start(container_name).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, services: Option<Vec<String>>) -> Result<(), ComposeError> {
        let targets = services.as_ref().unwrap_or(&self.services);
        for s in targets {
            let config = self.spec.services.get(s).ok_or_else(|| ComposeError::NotFound(s.to_string()))?;
            let container_name = config.container_name.as_deref().unwrap_or(s);
            self.backend.stop(container_name, None).await?;
        }
        Ok(())
    }

    pub async fn restart(&self, services: Option<Vec<String>>) -> Result<(), ComposeError> {
        let targets = services.as_ref().unwrap_or(&self.services);
        for s in targets {
            let config = self.spec.services.get(s).ok_or_else(|| ComposeError::NotFound(s.to_string()))?;
            let container_name = config.container_name.as_deref().unwrap_or(s);
            self.backend.stop(container_name, None).await?;
            self.backend.start(container_name).await?;
        }
        Ok(())
    }

    pub fn graph(&self) -> ServiceGraph {
        let nodes = self.services.clone();
        let mut edges = Vec::new();
        for name in &self.services {
            if let Some(service) = self.spec.services.get(name) {
                if let Some(depends_on) = &service.depends_on {
                    let deps = match depends_on {
                        crate::types::DependsOnOrList::List(l) => l.clone(),
                        crate::types::DependsOnOrList::Dict(d) => d.keys().cloned().collect(),
                    };
                    for dep in deps {
                        edges.push(ServiceEdge { from: name.clone(), to: dep });
                    }
                }
            }
        }
        ServiceGraph { nodes, edges }
    }

    pub fn config(&self) -> ComposeSpec {
        self.spec.clone()
    }
}

pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>, ComposeError> {
    let mut in_degree = HashMap::new();
    let mut adj = HashMap::new();

    for name in spec.services.keys() {
        in_degree.insert(name.clone(), 0);
        adj.insert(name.clone(), Vec::new());
    }

    for (name, service) in &spec.services {
        if let Some(depends_on) = &service.depends_on {
            let deps = match depends_on {
                crate::types::DependsOnOrList::List(l) => l.clone(),
                crate::types::DependsOnOrList::Dict(d) => d.keys().cloned().collect(),
            };

            for dep in deps {
                if !spec.services.contains_key(&dep) {
                    return Err(ComposeError::InvalidConfig(format!("Service '{}' depends on unknown service '{}'", name, dep)));
                }
                adj.get_mut(&dep).unwrap().push(name.clone());
                *in_degree.get_mut(name).unwrap() += 1;
            }
        }
    }

    let mut queue = VecDeque::new();
    for (name, degree) in &in_degree {
        if *degree == 0 {
            queue.push_back(name.clone());
        }
    }

    let mut order = Vec::new();
    while let Some(u) = queue.pop_front() {
        order.push(u.clone());
        for v in &adj[&u] {
            let degree = in_degree.get_mut(v).unwrap();
            *degree -= 1;
            if *degree == 0 {
                queue.push_back(v.clone());
            }
        }
    }

    if order.len() != spec.services.len() {
        let mut cycle_members = Vec::new();
        for (name, degree) in in_degree {
            if degree > 0 {
                cycle_members.push(name);
            }
        }
        return Err(ComposeError::DependencyCycle { cycle: cycle_members });
    }

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ComposeSpec, ComposeService, DependsOnOrList};
    use indexmap::IndexMap;

    #[test]
    fn test_resolve_startup_order_linear() {
        let mut services = IndexMap::new();
        services.insert("a".to_string(), ComposeService {
            depends_on: Some(DependsOnOrList::List(vec!["b".to_string()])),
            ..Default::default()
        });
        services.insert("b".to_string(), ComposeService::default());

        let spec = ComposeSpec { services, ..Default::default() };
        let order = resolve_startup_order(&spec).unwrap();
        assert_eq!(order, vec!["b", "a"]);
    }

    #[test]
    fn test_resolve_startup_order_cycle() {
        let mut services = IndexMap::new();
        services.insert("a".to_string(), ComposeService {
            depends_on: Some(DependsOnOrList::List(vec!["b".to_string()])),
            ..Default::default()
        });
        services.insert("b".to_string(), ComposeService {
            depends_on: Some(DependsOnOrList::List(vec!["a".to_string()])),
            ..Default::default()
        });

        let spec = ComposeSpec { services, ..Default::default() };
        let err = resolve_startup_order(&spec).unwrap_err();
        if let ComposeError::DependencyCycle { cycle } = err {
            assert!(cycle.contains(&"a".to_string()));
            assert!(cycle.contains(&"b".to_string()));
        } else {
            panic!("Expected DependencyCycle error");
        }
    }
}
