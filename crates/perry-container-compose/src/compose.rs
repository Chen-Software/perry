use indexmap::IndexMap;
use crate::error::{ComposeError, Result};
use crate::types::{ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec, ComposeHandle, ContainerHandle, ListOrDict, BuildSpec};
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::service::{generate_name, needs_build};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

#[derive(Clone)]
pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend + Send + Sync>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend + Send + Sync>) -> Self {
        Self { spec, project_name, backend }
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

    pub async fn up(&self, services: &[String], _detach: bool, build: bool, _remove_orphans: bool) -> Result<ComposeHandle> {
        let full_order = Self::resolve_startup_order(&self.spec)?;
        let order = if services.is_empty() {
            full_order
        } else {
            full_order.into_iter().filter(|s| services.contains(s)).collect()
        };

        let mut created_networks = Vec::new();
        let mut created_volumes = Vec::new();
        let mut started_containers: Vec<(String, ContainerHandle)> = Vec::new();

        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let config_obj = config.as_ref().cloned().unwrap_or_default();
                let net_config = NetworkConfig {
                    driver: config_obj.driver.clone(),
                    labels: self.list_or_dict_to_map(config_obj.labels.as_ref()),
                    internal: config_obj.internal.unwrap_or(false),
                    enable_ipv6: config_obj.enable_ipv6.unwrap_or(false),
                };
                if let Err(e) = self.backend.create_network(name, &net_config).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_networks.push(name.clone());
            }
        }

        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                let config_obj = config.as_ref().cloned().unwrap_or_default();
                let vol_config = VolumeConfig {
                    driver: config_obj.driver.clone(),
                    labels: self.list_or_dict_to_map(config_obj.labels.as_ref()),
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

            // Idempotent flow
            let containers = self.backend.list(true).await?;
            let existing = containers.iter().find(|c| c.name == service_name || c.name == format!("{}_{}", self.project_name, service_name));

            if let Some(c) = existing {
                if c.status.contains("Up") || c.status.contains("running") {
                    started_containers.push((service_name.clone(), ContainerHandle { id: c.id.clone(), name: Some(c.name.clone()) }));
                    continue;
                } else {
                    self.backend.start(&c.id).await?;
                    started_containers.push((service_name.clone(), ContainerHandle { id: c.id.clone(), name: Some(c.name.clone()) }));
                    continue;
                }
            }

            let container_name = generate_name(service, &service_name)?;

            if build || needs_build(service) {
                if let Some(build_spec) = &service.build {
                    let (context, build_obj) = match build_spec {
                        BuildSpec::Context(c) => (c.clone(), crate::types::ComposeServiceBuild::default()),
                        BuildSpec::Config(c) => (c.context.clone().unwrap_or_else(|| ".".into()), c.clone()),
                    };
                    let tag = service.image.clone().unwrap_or_else(|| format!("{}-{}", self.project_name, service_name));
                    self.backend.build_image(&context, &build_obj, &tag).await?;
                }
            } else if let Some(image) = &service.image {
                if let Err(e) = self.backend.pull_image(image).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(ComposeError::ImagePullFailed { service: service_name.clone(), image: image.clone(), message: e.to_string() });
                }
            }

            let container_spec = ContainerSpec {
                image: service.image.clone().unwrap_or_else(|| format!("{}-{}", self.project_name, service_name)),
                name: Some(container_name),
                ports: service.ports.as_ref().map(|p| p.iter().map(|ps| match ps {
                    crate::types::PortSpec::Short(v) => self.yaml_to_string(v),
                    crate::types::PortSpec::Long(lp) => format!("{}:{}", self.yaml_to_string(lp.published.as_ref().unwrap_or(&serde_yaml::Value::Null)), self.yaml_to_string(&lp.target)),
                }).collect()),
                volumes: service.volumes.as_ref().map(|v| v.iter().map(|vs| self.yaml_to_string(vs)).collect()),
                env: match &service.environment {
                    Some(ListOrDict::Dict(d)) => Some(d.iter().map(|(k, v)| (k.clone(), v.as_ref().map(|val| self.yaml_to_string(val)).unwrap_or_default())).collect()),
                    Some(ListOrDict::List(l)) => Some(l.iter().filter_map(|s| {
                        let mut parts = s.splitn(2, '=');
                        Some((parts.next()?.to_string(), parts.next()?.to_string()))
                    }).collect()),
                    _ => None,
                },
                cmd: match &service.command {
                    Some(serde_yaml::Value::String(s)) => Some(vec![s.clone()]),
                    Some(serde_yaml::Value::Sequence(seq)) => Some(seq.iter().map(|v| self.yaml_to_string(v)).collect()),
                    _ => None,
                },
                entrypoint: match &service.entrypoint {
                    Some(serde_yaml::Value::String(s)) => Some(vec![s.clone()]),
                    Some(serde_yaml::Value::Sequence(seq)) => Some(seq.iter().map(|v| self.yaml_to_string(v)).collect()),
                    _ => None,
                },
                network: None, // TODO: handle networks properly
                rm: Some(false),
                read_only: service.read_only,
            };

            match self.backend.run(&container_spec).await {
                Ok(handle) => {
                    started_containers.push((service_name, handle));
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
            project_name: self.project_name.clone(),
            services: started_containers.iter().map(|(n, _)| n.clone()).collect(),
        })
    }

    fn yaml_to_string(&self, v: &serde_yaml::Value) -> String {
        match v {
            serde_yaml::Value::String(s) => s.clone(),
            serde_yaml::Value::Number(n) => n.to_string(),
            serde_yaml::Value::Bool(b) => b.to_string(),
            _ => format!("{:?}", v),
        }
    }

    fn list_or_dict_to_map(&self, ld: Option<&ListOrDict>) -> HashMap<String, String> {
        let mut map = HashMap::new();
        match ld {
            Some(ListOrDict::Dict(d)) => {
                for (k, v) in d {
                    map.insert(k.clone(), v.as_ref().map(|val| self.yaml_to_string(val)).unwrap_or_default());
                }
            }
            Some(ListOrDict::List(l)) => {
                for s in l {
                    let mut parts = s.splitn(2, '=');
                    if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                        map.insert(k.to_string(), v.to_string());
                    }
                }
            }
            None => {}
        }
        map
    }

    async fn rollback(&self, containers: &[(String, ContainerHandle)], networks: &[String], volumes: &[String]) {
        for (_, handle) in containers.iter().rev() {
            let _ = self.backend.stop(&handle.id, Some(10)).await;
            let _ = self.backend.remove(&handle.id, true).await;
        }
        for net in networks {
            let _ = self.backend.remove_network(net).await;
        }
        for vol in volumes {
            let _ = self.backend.remove_volume(vol).await;
        }
    }

    pub async fn down(&self, _services: &[String], _remove_orphans: bool, volumes: bool) -> Result<()> {
        let order = Self::resolve_startup_order(&self.spec)?;
        for service_name in order.iter().rev() {
             let _ = self.backend.remove(service_name, true).await;
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

    pub async fn logs(&self, services: &[String], tail: Option<u32>) -> Result<HashMap<String, String>> {
        let mut logs = HashMap::new();
        if services.is_empty() {
             for svc in self.spec.services.keys() {
                 let l = self.backend.logs(svc, tail).await?;
                 logs.insert(svc.clone(), l.stdout);
             }
        } else {
             for svc in services {
                 let l = self.backend.logs(svc, tail).await?;
                 logs.insert(svc.clone(), l.stdout);
             }
        }
        Ok(logs)
    }

    pub async fn exec(&self, service: &str, cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        self.backend.exec(service, cmd, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        for svc in services { self.backend.start(svc).await?; }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        for svc in services { self.backend.stop(svc, None).await?; }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        for svc in services {
            self.backend.stop(svc, None).await?;
            self.backend.start(svc).await?;
        }
        Ok(())
    }

    pub fn config(&self) -> Result<String> {
        serde_yaml::to_string(&self.spec).map_err(ComposeError::ParseError)
    }
}
