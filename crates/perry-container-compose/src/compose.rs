use indexmap::IndexMap;
use crate::error::{ComposeError, Result};
use crate::types::{ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec, ComposeHandle, ContainerHandle, ListOrDict, PortSpec, ComposeServiceVolume, ServiceNetworks};
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::service::generate_name;
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

    pub async fn up(&self, services_to_start: &[String], _detach: bool, _build: bool, _remove_orphans: bool) -> Result<ComposeHandle> {
        let full_order = Self::resolve_startup_order(&self.spec)?;

        let services_to_start: BTreeSet<String> = if services_to_start.is_empty() {
            full_order.iter().cloned().collect()
        } else {
            let mut set = BTreeSet::new();
            for s in services_to_start {
                self.add_with_deps(s, &mut set)?;
            }
            set
        };

        let mut created_networks = Vec::new();
        let mut created_volumes = Vec::new();
        let mut started_containers: Vec<(String, ContainerHandle)> = Vec::new();

        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let config = config.clone().unwrap_or_default();
                let net_cfg = NetworkConfig {
                    driver: config.driver.clone(),
                    labels: match &config.labels {
                        Some(ListOrDict::Dict(d)) => Some(d.iter().map(|(k, v)| (k.clone(), format!("{:?}", v))).collect()),
                        _ => None,
                    },
                    internal: config.internal.unwrap_or(false),
                    enable_ipv6: config.enable_ipv6.unwrap_or(false),
                };
                if let Err(e) = self.backend.create_network(name, &net_cfg).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_networks.push(name.clone());
            }
        }

        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                let config = config.clone().unwrap_or_default();
                let vol_cfg = VolumeConfig {
                    driver: config.driver.clone(),
                    labels: match &config.labels {
                        Some(ListOrDict::Dict(d)) => Some(d.iter().map(|(k, v)| (k.clone(), format!("{:?}", v))).collect()),
                        _ => None,
                    },
                };
                if let Err(e) = self.backend.create_volume(name, &vol_cfg).await {
                    self.rollback(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_volumes.push(name.clone());
            }
        }

        for service_name in full_order {
            if !services_to_start.contains(&service_name) {
                continue;
            }

            let service = self.spec.services.get(&service_name).unwrap();
            let image = service.image.clone().unwrap_or_default();

            let mut ports = Vec::new();
            if let Some(service_ports) = &service.ports {
                for p in service_ports {
                    match p {
                        PortSpec::Short(v) => ports.push(format!("{:?}", v)),
                        PortSpec::Long(p) => {
                            let mut s = String::new();
                            if let Some(ip) = &p.host_ip { s.push_str(ip); s.push(':'); }
                            if let Some(pub_port) = &p.published { s.push_str(&format!("{:?}", pub_port)); s.push(':'); }
                            s.push_str(&format!("{:?}", p.target));
                            if let Some(proto) = &p.protocol { s.push('/'); s.push_str(proto); }
                            ports.push(s);
                        }
                    }
                }
            }

            let mut volumes = Vec::new();
            if let Some(service_vols) = &service.volumes {
                for v in service_vols {
                    match v {
                        serde_yaml::Value::String(s) => volumes.push(s.clone()),
                        serde_yaml::Value::Mapping(_) => {
                            let sv: ComposeServiceVolume = serde_yaml::from_value(v.clone()).map_err(|e| ComposeError::ValidationError { message: e.to_string() })?;
                            let mut s = String::new();
                            if let Some(src) = &sv.source { s.push_str(src); s.push(':'); }
                            if let Some(tgt) = &sv.target { s.push_str(tgt); }
                            if let Some(ro) = sv.read_only { if ro { s.push_str(":ro"); } }
                            volumes.push(s);
                        }
                        _ => {}
                    }
                }
            }

            let mut env = HashMap::new();
            if let Some(list_or_dict) = &service.environment {
                match list_or_dict {
                    ListOrDict::Dict(d) => {
                        for (k, v) in d {
                            env.insert(k.clone(), v.as_ref().map(|val| match val {
                                serde_yaml::Value::String(s) => s.clone(),
                                _ => format!("{:?}", val),
                            }).unwrap_or_default());
                        }
                    }
                    ListOrDict::List(l) => {
                        for entry in l {
                            if let Some((k, v)) = entry.split_once('=') {
                                env.insert(k.to_string(), v.to_string());
                            } else {
                                env.insert(entry.clone(), std::env::var(entry).unwrap_or_default());
                            }
                        }
                    }
                }
            }

            let container_spec = ContainerSpec {
                image: image.clone(),
                name: Some(generate_name(&image, &service_name)),
                ports: if ports.is_empty() { None } else { Some(ports) },
                volumes: if volumes.is_empty() { None } else { Some(volumes) },
                env: if env.is_empty() { None } else { Some(env) },
                cmd: match &service.command {
                    Some(serde_yaml::Value::String(s)) => Some(vec![s.clone()]),
                    Some(serde_yaml::Value::Sequence(seq)) => Some(seq.iter().map(|v| match v {
                        serde_yaml::Value::String(s) => s.clone(),
                        _ => format!("{:?}", v),
                    }).collect()),
                    _ => None,
                },
                entrypoint: match &service.entrypoint {
                    Some(serde_yaml::Value::String(s)) => Some(vec![s.clone()]),
                    Some(serde_yaml::Value::Sequence(seq)) => Some(seq.iter().map(|v| match v {
                        serde_yaml::Value::String(s) => s.clone(),
                        _ => format!("{:?}", v),
                    }).collect()),
                    _ => None,
                },
                network: match &service.networks {
                    Some(ServiceNetworks::List(l)) => l.first().cloned(),
                    Some(ServiceNetworks::Map(m)) => m.keys().next().cloned(),
                    None => None,
                },
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

    pub async fn down(&self, services: &[String], _remove_orphans: bool, volumes: bool) -> Result<()> {
        let order = Self::resolve_startup_order(&self.spec)?;
        let services_to_stop: BTreeSet<String> = if services.is_empty() {
            order.iter().cloned().collect()
        } else {
            services.iter().cloned().collect()
        };

        for service_name in order.iter().rev() {
            if services_to_stop.contains(service_name) {
                let _ = self.backend.stop(service_name, None).await;
                let _ = self.backend.remove(service_name, true).await;
            }
        }

        if services.is_empty() {
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
        }
        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        self.backend.list(true).await
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

    pub fn config(&self) -> Result<String> {
        serde_yaml::to_string(&self.spec).map_err(ComposeError::ParseError)
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let target = if services.is_empty() { self.spec.services.keys().cloned().collect::<Vec<_>>() } else { services.to_vec() };
        for svc in target { self.backend.start(&svc).await?; }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let target = if services.is_empty() { self.spec.services.keys().cloned().collect::<Vec<_>>() } else { services.to_vec() };
        for svc in target { self.backend.stop(&svc, None).await?; }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        let target = if services.is_empty() { self.spec.services.keys().cloned().collect::<Vec<_>>() } else { services.to_vec() };
        for svc in target {
            self.backend.stop(&svc, None).await?;
            self.backend.start(&svc).await?;
        }
        Ok(())
    }
}
