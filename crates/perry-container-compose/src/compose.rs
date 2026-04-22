use indexmap::IndexMap;
use crate::error::{ComposeError, Result};
use crate::types::{ComposeSpec, ContainerInfo, ContainerLogs, ComposeHandle, ListOrDict, ServiceGraph, ServiceEdge, StackStatus, ServiceStatus, ComposeService};
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::orchestrate::orchestrate_service;
use crate::workload::{WorkloadGraph, RunGraphOptions, RefProjection, WorkloadEnvValue};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

#[derive(Clone)]
pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub backend: Arc<dyn ContainerBackend + Send + Sync>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Self {
        Self { spec, backend }
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

    pub fn resolve_startup_order_method(&self) -> Result<Vec<String>> {
        Self::resolve_startup_order(&self.spec)
    }

    pub async fn up(&self) -> Result<ComposeHandle> {
        let order = Self::resolve_startup_order(&self.spec)?;
        let mut created_networks = Vec::new();
        let mut created_volumes = Vec::new();
        let mut started_containers: Vec<String> = Vec::new();

        if let Some(networks) = &self.spec.networks {
            for (name, config_opt) in networks {
                let config = config_opt.as_ref().cloned().unwrap_or_default();
                if config.external.unwrap_or(false) || self.backend.inspect_network(name).await.is_ok() {
                    continue;
                }

                let net_config = NetworkConfig {
                    driver: config.driver.clone(),
                    labels: match &config.labels {
                        Some(ListOrDict::Dict(d)) => d.iter().map(|(k, v)| (k.clone(), v.as_ref().map_or("".to_string(), |val| format!("{:?}", val)))).collect(),
                        Some(ListOrDict::List(l)) => l.iter().map(|s| (s.clone(), "".to_string())).collect(),
                        None => HashMap::new(),
                    },
                    internal: config.internal.unwrap_or(false),
                    enable_ipv6: config.enable_ipv6.unwrap_or(false),
                };
                if let Err(e) = self.backend.create_network(name, &net_config).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_networks.push(name.clone());
            }
        }

        if let Some(volumes) = &self.spec.volumes {
            for (name, config_opt) in volumes {
                let config = config_opt.as_ref().cloned().unwrap_or_default();
                if config.external.unwrap_or(false) || self.backend.inspect_volume(name).await.is_ok() {
                    continue;
                }

                let vol_config = VolumeConfig {
                    driver: config.driver.clone(),
                    labels: match &config.labels {
                        Some(ListOrDict::Dict(d)) => d.iter().map(|(k, v)| (k.clone(), v.as_ref().map_or("".to_string(), |val| format!("{:?}", val)))).collect(),
                        Some(ListOrDict::List(l)) => l.iter().map(|s| (s.clone(), "".to_string())).collect(),
                        None => HashMap::new(),
                    },
                };
                if let Err(e) = self.backend.create_volume(name, &vol_config).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_volumes.push(name.clone());
            }
        }

        for service_name in order {
            let service = self.spec.services.get(&service_name).unwrap();

            match orchestrate_service(&service_name, service, self.backend.as_ref()).await {
                Ok(_) => {
                    started_containers.push(service_name);
                }
                Err(e) => {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(ComposeError::ServiceStartupFailed {
                        service: service_name,
                        message: e.to_string(),
                    });
                }
            }
        }

        Ok(ComposeHandle {
            stack_id: rand::random(),
            project_name: self.spec.name.clone().unwrap_or_else(|| "default".into()),
            services: started_containers,
        })
    }

    async fn rollback(&self, services: &[String], networks: &[String], volumes: &[String]) {
        for service_key in services.iter().rev() {
            if let Some(service) = self.spec.services.get(service_key) {
                let name = service.name(service_key);
                let _ = self.backend.stop(&name, Some(10)).await;
                let _ = self.backend.remove(&name, true).await;
            }
        }
        for net in networks {
            let _ = self.backend.remove_network(net).await;
        }
        for vol in volumes {
            let _ = self.backend.remove_volume(vol).await;
        }
    }

    pub async fn down(&self, volumes: bool) -> Result<()> {
        let order = Self::resolve_startup_order(&self.spec)?;
        for service_key in order.iter().rev() {
             if let Some(service) = self.spec.services.get(service_key) {
                 let name = service.name(service_key);
                 let _ = self.backend.stop(&name, None).await;
                 let _ = self.backend.remove(&name, true).await;
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
        self.backend.list(true).await
    }

    pub async fn logs(&self, service_key: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        if let Some(key) = service_key {
            if let Some(service) = self.spec.services.get(key) {
                 let name = service.name(key);
                 return self.backend.logs(&name, tail).await;
            }
        }
        Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
    }

    pub async fn exec(&self, service_key: &str, cmd: &[String]) -> Result<ContainerLogs> {
        if let Some(service) = self.spec.services.get(service_key) {
            let name = service.name(service_key);
            return self.backend.exec(&name, cmd, None, None).await;
        }
        Err(ComposeError::NotFound(service_key.into()))
    }

    pub async fn start(&self, service_keys: &[String]) -> Result<()> {
        for key in service_keys {
            if let Some(service) = self.spec.services.get(key) {
                let name = service.name(key);
                self.backend.start(&name).await?;
            }
        }
        Ok(())
    }

    pub async fn stop(&self, service_keys: &[String]) -> Result<()> {
        for key in service_keys {
            if let Some(service) = self.spec.services.get(key) {
                let name = service.name(key);
                self.backend.stop(&name, None).await?;
            }
        }
        Ok(())
    }

    pub async fn restart(&self, service_keys: &[String]) -> Result<()> {
        for key in service_keys {
            if let Some(service) = self.spec.services.get(key) {
                let name = service.name(key);
                let _ = self.backend.stop(&name, None).await;
                self.backend.start(&name).await?;
            }
        }
        Ok(())
    }

    pub fn graph(&self) -> Result<ServiceGraph> {
        let order = Self::resolve_startup_order(&self.spec)?;
        let mut edges = Vec::new();
        for (name, service) in &self.spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    edges.push(ServiceEdge {
                        from: name.clone(),
                        to: dep,
                    });
                }
            }
        }
        Ok(ServiceGraph {
            nodes: order,
            edges,
        })
    }

    pub async fn status(&self) -> Result<StackStatus> {
        let mut services = Vec::new();
        let mut all_running = true;
        let containers = self.backend.list(true).await?;

        for (key, service) in &self.spec.services {
            let name = service.name(key);
            let container = containers.iter().find(|c| c.name == name);
            let state = container.map(|c| c.status.clone()).unwrap_or_else(|| "unknown".into());
            let container_id = container.map(|c| c.id.clone());

            if !state.to_lowercase().contains("running") && !state.to_lowercase().contains("up") {
                all_running = false;
            }

            services.push(ServiceStatus {
                service: key.clone(),
                state,
                container_id,
                error: None,
            });
        }

        Ok(StackStatus {
            services,
            healthy: all_running,
        })
    }
}

pub struct WorkloadGraphEngine {
    pub engine: ComposeEngine,
}

impl WorkloadGraphEngine {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Self {
        Self {
            engine: ComposeEngine::new(spec, backend),
        }
    }

    pub async fn run(&self, graph: WorkloadGraph, _opts: RunGraphOptions) -> Result<u64> {
        let mut spec = ComposeSpec::default();
        spec.name = Some(graph.name.clone());

        for (id, node) in &graph.nodes {
            let mut svc = ComposeService::default();
            svc.image = node.image.clone();
            svc.ports = Some(node.ports.iter().map(|p| crate::types::PortSpec::Short(serde_yaml::Value::String(p.clone()))).collect());
            svc.depends_on = Some(crate::types::DependsOnSpec::List(node.depends_on.clone()));

            let mut env = IndexMap::new();
            for (k, v) in &node.env {
                let val = match v {
                    WorkloadEnvValue::Literal(s) => Some(serde_yaml::Value::String(s.clone())),
                    WorkloadEnvValue::Ref(r) => {
                        let proj_str = match r.projection {
                            RefProjection::Endpoint => "endpoint",
                            RefProjection::Ip => "ip",
                            RefProjection::InternalUrl => "url",
                        };
                        Some(serde_yaml::Value::String(format!("REF:{}:{}", r.node_id, proj_str)))
                    }
                };
                env.insert(k.clone(), val);
            }
            svc.environment = Some(ListOrDict::Dict(env));
            spec.services.insert(id.clone(), svc);
        }

        let engine = ComposeEngine::new(spec, Arc::clone(&self.engine.backend));
        let _handle = engine.up().await?;
        Ok(rand::random())
    }
}
