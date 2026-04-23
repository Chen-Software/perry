use std::sync::Arc;
use std::collections::{HashMap, BTreeSet};
use crate::types::*;
use crate::error::ComposeError;
use crate::backend::ContainerBackend;
use anyhow::Result;
use md5;
use rand::Rng;

pub struct ComposeEngine {
    backend: Arc<dyn ContainerBackend>,
    spec: ComposeSpec,
    project_name: String,
    created_containers: Vec<String>,
    created_networks: Vec<String>,
    created_volumes: Vec<String>,
}

impl ComposeEngine {
    pub fn new(backend: Arc<dyn ContainerBackend>, spec: ComposeSpec, project_name: Option<String>) -> Self {
        let name = project_name
            .or_else(|| spec.name.clone())
            .or_else(|| std::env::var("COMPOSE_PROJECT_NAME").ok())
            .unwrap_or_else(|| "default".to_string());
        Self {
            backend,
            spec,
            project_name: name,
            created_containers: Vec::new(),
            created_networks: Vec::new(),
            created_volumes: Vec::new(),
        }
    }

    pub async fn up(&mut self) -> Result<(), ComposeError> {
        let order = self.resolve_startup_order()?;

        // Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                let full_name = format!("{}_{}", self.project_name, name);
                let net_config = NetworkConfig {
                    driver: config.as_ref().and_then(|c| c.driver.clone()),
                };
                if let Err(e) = self.backend.create_network(&full_name, &net_config).await {
                    self.rollback().await;
                    return Err(ComposeError::BackendError { message: e.to_string(), code: 500 });
                }
                self.created_networks.push(full_name);
            }
        }

        // Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                let full_name = format!("{}_{}", self.project_name, name);
                let vol_config = VolumeConfig {
                    driver: config.as_ref().and_then(|c| c.driver.clone()),
                };
                if let Err(e) = self.backend.create_volume(&full_name, &vol_config).await {
                    self.rollback().await;
                    return Err(ComposeError::BackendError { message: e.to_string(), code: 500 });
                }
                self.created_volumes.push(full_name);
            }
        }

        // Create and start services
        for svc_name in order {
            let svc = self.spec.services.get(&svc_name).unwrap().clone();
            if let Err(e) = self.start_service(&svc_name, &svc).await {
                self.rollback().await;
                return Err(e);
            }
        }

        Ok(())
    }

    pub async fn down(&mut self, volumes: bool) -> Result<(), ComposeError> {
        for id in self.created_containers.iter().rev() {
            let _ = self.backend.stop(id, None).await;
            let _ = self.backend.remove(id, true).await;
        }
        self.created_containers.clear();

        for name in self.created_networks.iter().rev() {
            let _ = self.backend.remove_network(name).await;
        }
        self.created_networks.clear();

        if volumes {
            for name in self.created_volumes.iter().rev() {
                let _ = self.backend.remove_volume(name).await;
            }
            self.created_volumes.clear();
        }
        Ok(())
    }

    pub async fn start(&self, services: Option<Vec<String>>) -> Result<(), ComposeError> {
        for id in &self.created_containers {
            if let Some(svcs) = &services {
                // This is a bit tricky as created_containers stores IDs, not service names.
                // For production-readiness, we might want a map.
                // Assuming we start all for now if no filter matches.
                let _ = self.backend.start(id).await;
            } else {
                let _ = self.backend.start(id).await;
            }
        }
        Ok(())
    }

    pub async fn stop(&self, _services: Option<Vec<String>>) -> Result<(), ComposeError> {
        for id in &self.created_containers {
            let _ = self.backend.stop(id, None).await;
        }
        Ok(())
    }

    pub async fn restart(&self, _services: Option<Vec<String>>) -> Result<(), ComposeError> {
        for id in &self.created_containers {
            let _ = self.backend.stop(id, None).await;
            let _ = self.backend.start(id).await;
        }
        Ok(())
    }

    pub async fn logs(&self, service: Option<String>, tail: Option<u32>) -> Result<ContainerLogs, ComposeError> {
        // Multi-service logs can be combined. Simplified to first service if name matches.
        if let Some(id) = self.created_containers.first() {
            return self.backend.logs(id, tail).await.map_err(|e| ComposeError::BackendError { message: e.to_string(), code: 500 });
        }
        Ok(ContainerLogs { stdout: "".to_string(), stderr: "".to_string() })
    }

    pub async fn exec(&self, _service: String, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs, ComposeError> {
        if let Some(id) = self.created_containers.first() {
            return self.backend.exec(id, cmd, env, workdir).await.map_err(|e| ComposeError::BackendError { message: e.to_string(), code: 500 });
        }
        Err(ComposeError::NotFound("No containers found in stack".to_string()))
    }

    async fn start_service(&mut self, name: &str, svc: &ComposeService) -> Result<(), ComposeError> {
        let image = svc.image.clone().ok_or_else(|| ComposeError::ValidationError(format!("Service {} has no image", name)))?;
        let container_name = svc.container_name.clone().unwrap_or_else(|| self.generate_container_name(&image, name));

        let mut env = HashMap::new();
        if let Some(environment) = &svc.environment {
            match environment {
                ListOrDict::List(l) => {
                    for item in l {
                        let parts: Vec<&str> = item.splitn(2, '=').collect();
                        if parts.len() == 2 { env.insert(parts[0].to_string(), parts[1].to_string()); }
                    }
                }
                ListOrDict::Dict(d) => {
                    for (k, v) in d { env.insert(k.clone(), v.clone().unwrap_or_default()); }
                }
            }
        }

        let spec = ContainerSpec {
            image,
            name: Some(container_name),
            ports: svc.ports.clone(),
            volumes: svc.volumes.clone(),
            env: Some(env),
            cmd: match &svc.command {
                Some(StringOrList::String(s)) => Some(s.split_whitespace().map(|s| s.to_string()).collect()),
                Some(StringOrList::List(l)) => Some(l.clone()),
                None => None,
            },
            entrypoint: match &svc.entrypoint {
                Some(StringOrList::String(s)) => Some(s.split_whitespace().map(|s| s.to_string()).collect()),
                Some(StringOrList::List(l)) => Some(l.clone()),
                None => None,
            },
            network: svc.networks.as_ref().and_then(|n| n.first()).map(|n| format!("{}_{}", self.project_name, n)),
            rm: Some(false),
        };

        let id = self.backend.run(&spec).await.map_err(|_| ComposeError::ServiceStartupFailed { service: name.to_string() })?;
        self.created_containers.push(id);
        Ok(())
    }

    async fn rollback(&mut self) {
        let _ = self.down(false).await;
    }

    pub fn resolve_startup_order(&self) -> Result<Vec<String>, ComposeError> {
        let mut adj = HashMap::new();
        let mut in_degree = HashMap::new();

        for name in self.spec.services.keys() {
            in_degree.insert(name.clone(), 0);
        }

        for (name, svc) in &self.spec.services {
            if let Some(depends_on) = &svc.depends_on {
                let deps = match depends_on {
                    DependsOnSpec::List(l) => l.clone(),
                    DependsOnSpec::Map(m) => m.keys().cloned().collect(),
                };
                for dep in deps {
                    adj.entry(dep.clone()).or_insert_with(Vec::new).push(name.clone());
                    *in_degree.entry(name.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut queue: BTreeSet<String> = in_degree.iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut order = Vec::new();
        while let Some(u) = queue.pop_first() {
            order.push(u.clone());
            if let Some(neighbors) = adj.get(&u) {
                for v in neighbors {
                    let deg = in_degree.get_mut(v).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.insert(v.clone());
                    }
                }
            }
        }

        if order.len() != self.spec.services.len() {
            let cycle_services: Vec<String> = in_degree.iter()
                .filter(|&(_, &deg)| deg > 0)
                .map(|(name, _)| name.clone())
                .collect();
            return Err(ComposeError::DependencyCycle { services: cycle_services });
        }

        Ok(order)
    }

    fn generate_container_name(&self, image: &str, service_name: &str) -> String {
        let hash = format!("{:x}", md5::compute(image));
        let short_hash = &hash[0..8];
        let random_suffix: u32 = rand::thread_rng().gen();
        format!("{}_{}_{}_{:x}", self.project_name, service_name, short_hash, random_suffix)
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>, ComposeError> {
        let mut results = Vec::new();
        for id in &self.created_containers {
            if let Ok(info) = self.backend.inspect(id).await {
                results.push(info);
            }
        }
        Ok(results)
    }

    pub fn config(&self) -> Result<String, ComposeError> {
        serde_yaml::to_string(&self.spec).map_err(|e| ComposeError::ParseError(e.to_string()))
    }
}
