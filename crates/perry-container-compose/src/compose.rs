use std::collections::{HashMap, HashSet, VecDeque, BTreeSet};
use indexmap::IndexMap;
use std::sync::Arc;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{ComposeError, Result};
use crate::types::*;
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig, detect_backend};
use crate::service;

static COMPOSE_HANDLES: Lazy<DashMap<u64, Arc<ComposeEngine>>> = Lazy::new(DashMap::new);
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

pub fn resolve_startup_order(spec: &ComposeSpec) -> Result<Vec<String>> {
    let mut in_degree = HashMap::new();
    let mut adj = HashMap::new();

    for (name, service) in &spec.services {
        in_degree.entry(name.clone()).or_insert(0);
        if let Some(deps) = &service.depends_on {
            let dep_names = deps.service_names();
            for dep in dep_names {
                if !spec.services.contains_key(&dep) {
                    return Err(ComposeError::validation(format!("Service {} depends on unknown service {}", name, dep)));
                }
                adj.entry(dep.clone()).or_insert_with(Vec::new).push(name.clone());
                *in_degree.entry(name.clone()).or_insert(0) += 1;
            }
        }
    }

    let mut queue: BTreeSet<String> = in_degree.iter()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(name, _)| name.clone())
        .collect();

    let mut order: Vec<String> = Vec::new();
    while let Some(name) = queue.pop_first() {
        order.push(name.clone());
        if let Some(neighbors) = adj.get(&name) {
            for next in neighbors {
                let deg = in_degree.get_mut(next).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.insert(next.clone());
                }
            }
        }
    }

    if order.len() < spec.services.len() {
        let cycle_services: Vec<String> = in_degree.iter()
            .filter(|&(_, &deg)| deg > 0)
            .map(|(name, _)| name.clone())
            .collect();
        return Err(ComposeError::DependencyCycle { services: cycle_services });
    }

    Ok(order)
}

pub struct ComposeEngine {
    pub spec: ComposeSpec,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
}

pub struct WorkloadGraphEngine {
    pub graph: WorkloadGraph,
    pub project_name: String,
    pub backend: Arc<dyn ContainerBackend>,
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { spec, project_name, backend }
    }

    pub fn get_engine(handle_id: u64) -> Option<Arc<ComposeEngine>> {
        COMPOSE_HANDLES.get(&handle_id).map(|r| r.value().clone())
    }

    pub fn resolve_startup_order(&self) -> Result<Vec<String>> {
        let mut in_degree = HashMap::new();
        let mut adj = HashMap::new();
        let all_services: HashSet<_> = self.spec.services.keys().cloned().collect();

        for name in &all_services {
            in_degree.insert(name.clone(), 0);
            adj.insert(name.clone(), Vec::new());
        }

        for (name, service) in &self.spec.services {
            if let Some(depends_on) = &service.depends_on {
                let deps = match depends_on {
                    DependsOnSpec::List(l) => l.clone(),
                    DependsOnSpec::Map(m) => m.keys().cloned().collect(),
                };
                for dep in deps {
                    if !all_services.contains(&dep) {
                        return Err(ComposeError::ValidationError { message: format!("Service '{}' depends on unknown service '{}'", name, dep) });
                    }
                    adj.get_mut(&dep).unwrap().push(name.clone());
                    *in_degree.get_mut(name).unwrap() += 1;
                }
            }
        }

        let mut queue: VecDeque<_> = in_degree.iter()
            .filter(|&(_, &d)| d == 0)
            .map(|(k, _)| k.clone())
            .collect();

        // Sort alphabetically for determinism
        let mut queue_vec: Vec<_> = queue.into_iter().collect();
        queue_vec.sort();
        queue = queue_vec.into();

        let mut order = Vec::new();
        while let Some(u) = queue.pop_front() {
            order.push(u.clone());
            for v in adj.get(&u).unwrap() {
                let deg = in_degree.get_mut(v).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(v.clone());
                }
            }
        }

        if order.len() != all_services.len() {
            let cycle_services: Vec<_> = in_degree.iter()
                .filter(|&(_, &d)| d > 0)
                .map(|(k, _)| k.clone())
                .collect();
            return Err(ComposeError::DependencyCycle { services: cycle_services });
        }

        Ok(order)
    }

    pub async fn up(&self, build: bool) -> Result<ComposeHandle> {
        let mut created_networks = Vec::new();
        let mut created_volumes = Vec::new();
        let mut started_containers = Vec::new();

        // 1. Create networks
        if let Some(networks) = &self.spec.networks {
            for (name, net_spec) in networks {
                let net_name = format!("{}_{}", self.project_name, name);
                let config = if let Some(spec) = net_spec {
                    NetworkConfig {
                        driver: spec.driver.clone(),
                        labels: None,
                        internal: spec.internal,
                        enable_ipv6: spec.enable_ipv6,
                    }
                } else {
                    NetworkConfig { driver: None, labels: None, internal: None, enable_ipv6: None }
                };
                if let Err(e) = self.backend.create_network(&net_name, &config).await {
                    self.cleanup_on_failure(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_networks.push(net_name);
            }
        }

        // 2. Create volumes
        if let Some(volumes) = &self.spec.volumes {
            for (name, vol_spec) in volumes {
                let vol_name = format!("{}_{}", self.project_name, name);
                let config = if let Some(spec) = vol_spec {
                    VolumeConfig {
                        driver: spec.driver.clone(),
                        labels: None,
                    }
                } else {
                    VolumeConfig { driver: None, labels: None }
                };
                if let Err(e) = self.backend.create_volume(&vol_name, &config).await {
                    self.cleanup_on_failure(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                created_volumes.push(vol_name);
            }
        }

        // 3. Resolve order and start
        let order = self.resolve_startup_order()?;

        for name in order {
            let service = self.spec.services.get(&name).unwrap();
            let container_name = service::generate_name(&name, service)?;

            // Check if exists and running
            let containers = self.backend.list(true).await.map_err(|e| {
                // Return original backend error but cleanup first
                // self.cleanup_on_failure(...) is async, need to handle
                e
            })?;
            // Note: we can't easily cleanup in the map_err because it's not async.
            // Better to wrap the loop in a block or use a flag.

            let existing = containers.iter().find(|c| c.name == container_name);

            if let Some(c) = existing {
                if c.status.contains("Up") || c.status.contains("running") {
                    started_containers.push((name, c.id.clone()));
                    continue;
                }
                if let Err(e) = self.backend.start(&c.id).await {
                    self.cleanup_on_failure(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
                started_containers.push((name, c.id.clone()));
                continue;
            }

            // Fresh container
            if build || service::needs_build(service) {
                // Build logic
                let context = match &service.build {
                    Some(crate::types::BuildSpec::Context(c)) => c.clone(),
                    Some(crate::types::BuildSpec::Config(cfg)) => cfg.context.clone().unwrap_or_else(|| ".".into()),
                    None => ".".into(),
                };
                let tag = format!("{}/{}:latest", self.project_name, name);
                let build_args = self.backend.build_args(&context, None, None, &[tag.clone()]);
                if let Err(e) = self.backend.exec_ok(&build_args).await {
                    self.cleanup_on_failure(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
            } else if let Some(ref img) = service.image {
                if let Err(e) = self.backend.pull_image(img).await {
                    self.cleanup_on_failure(&started_containers, &created_networks, &created_volumes).await;
                    return Err(ComposeError::ImagePullFailed {
                        service: name.clone(),
                        image: img.clone(),
                        message: e.to_string()
                    });
                }
            }

            let spec = ContainerSpec {
                image: service.image.clone().unwrap_or_else(|| format!("{}/{}:latest", self.project_name, name)),
                name: Some(container_name.clone()),
                ..Default::default()
            };
            match self.backend.run(&spec).await {
                Ok(handle) => {
                    started_containers.push((name, handle.id));
                }
                Err(e) => {
                    self.cleanup_on_failure(&started_containers, &created_networks, &created_volumes).await;
                    return Err(e);
                }
            }
        }

        let stack_id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
        let handle = ComposeHandle {
            stack_id,
            project_name: self.project_name.clone(),
            services: started_containers.iter().map(|(n, _)| n.clone()).collect(),
        };

        Ok(handle)
    }

    async fn cleanup_on_failure(&self, containers: &[(String, String)], networks: &[String], volumes: &[String]) {
        for (_, id) in containers.iter().rev() {
            let _ = self.backend.stop(id, Some(10)).await;
            let _ = self.backend.remove(id, true).await;
        }
        for net in networks {
            let _ = self.backend.remove_network(net).await;
        }
        for vol in volumes {
            let _ = self.backend.remove_volume(vol).await;
        }
    }

    pub async fn down(&self, remove_volumes: bool) -> Result<()> {
        let containers = self.backend.list(true).await?;
        for name in self.spec.services.keys() {
            let container_name = format!("{}_{}", self.project_name, name);
            if let Some(c) = containers.iter().find(|c| c.name == container_name) {
                let _ = self.backend.stop(&c.id, None).await;
                self.backend.remove(&c.id, true).await?;
            }
        }

        if let Some(networks) = &self.spec.networks {
            for name in networks.keys() {
                let net_name = format!("{}_{}", self.project_name, name);
                let _ = self.backend.remove_network(&net_name).await;
            }
        }

        if remove_volumes {
            if let Some(volumes) = &self.spec.volumes {
                for name in volumes.keys() {
                    let vol_name = format!("{}_{}", self.project_name, name);
                    let _ = self.backend.remove_volume(&vol_name).await;
                }
            }
        }

        Ok(())
    }

    pub async fn ps(&self) -> Result<Vec<ContainerInfo>> {
        let all_containers = self.backend.list(true).await?;
        let mut project_containers = Vec::new();
        for name in self.spec.services.keys() {
            let container_name = format!("{}_{}", self.project_name, name);
            if let Some(c) = all_containers.iter().find(|c| c.name == container_name) {
                project_containers.push(c.clone());
            }
        }
        Ok(project_containers)
    }

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        if let Some(name) = service {
            let container_name = format!("{}_{}", self.project_name, name);
            self.backend.logs(&container_name, tail).await
        } else {
            // Aggregate logs from all services (simplified)
            Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
        }
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        let container_name = format!("{}_{}", self.project_name, service);
        self.backend.exec(&container_name, cmd, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        let targets = if services.is_empty() { self.spec.services.keys().cloned().collect() } else { services.to_vec() };
        for name in targets {
            let container_name = format!("{}_{}", self.project_name, name);
            self.backend.start(&container_name).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        let targets = if services.is_empty() { self.spec.services.keys().cloned().collect() } else { services.to_vec() };
        for name in targets {
            let container_name = format!("{}_{}", self.project_name, name);
            self.backend.stop(&container_name, None).await?;
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        self.stop(services).await?;
        self.start(services).await?;
        Ok(())
    }
}

impl WorkloadGraphEngine {
    pub fn new(graph: WorkloadGraph, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { graph, project_name, backend }
    }

    pub async fn run(&self, opts: &RunGraphOptions) -> Result<ComposeHandle> {
        let strategy = opts.strategy.clone().unwrap_or(ExecutionStrategy::DependencyAware);
        let _on_failure = opts.on_failure.clone().unwrap_or(FailureStrategy::RollbackAll);

        // Convert WorkloadGraph to ComposeSpec to reuse orchestration
        let mut services = IndexMap::new();
        for (id, node) in &self.graph.nodes {
            let mut service = ComposeService::default();
            service.image = node.image.clone();
            service.container_name = Some(node.name.clone());
            service.ports = node.ports.as_ref().map(|p| p.iter().map(|s| PortSpec::Short(serde_yaml::Value::String(s.clone()))).collect());
            service.depends_on = node.depends_on.as_ref().map(|d| DependsOnSpec::List(d.clone()));

            // Handle environment variables and WorkloadRefs
            if let Some(env) = &node.env {
                let mut dict = IndexMap::new();
                for (k, v) in env {
                    match v {
                        WorkloadEnvValue::Literal(s) => {
                            dict.insert(k.clone(), Some(serde_yaml::Value::String(s.clone())));
                        }
                        WorkloadEnvValue::Ref(r) => {
                            // Placeholders for resolution later
                            dict.insert(k.clone(), Some(serde_yaml::Value::String(format!("__PERRY_REF__{}:{}:{}__", r.node_id, match r.projection {
                                RefProjection::Endpoint => "endpoint",
                                RefProjection::Ip => "ip",
                                RefProjection::InternalUrl => "internalUrl",
                            }, r.port.as_deref().unwrap_or("")))));
                        }
                    }
                }
                service.environment = Some(ListOrDict::Dict(dict));
            }

            services.insert(id.clone(), service);
        }

        let spec = ComposeSpec {
            name: Some(self.project_name.clone()),
            services,
            ..Default::default()
        };

        let engine = ComposeEngine::new(spec, self.project_name.clone(), Arc::clone(&self.backend));

        // Apply parallel strategy if needed (simplified for MVP)
        match strategy {
            ExecutionStrategy::Sequential => {
                // To be implemented: true sequential
            }
            _ => {}
        }

        let handle = engine.up(false).await?;

        // Resolve WorkloadRefs after startup
        self.resolve_refs(&handle).await?;

        Ok(handle)
    }

    async fn resolve_refs(&self, _handle: &ComposeHandle) -> Result<()> {
        let mut node_info = HashMap::new();
        for (id, node) in &self.graph.nodes {
            let container_name = format!("{}_{}", self.project_name, id);
            if let Ok(info) = self.backend.inspect(&container_name).await {
                node_info.insert(id.clone(), info);
            }
        }

        for (id, node) in &self.graph.nodes {
            if let Some(env) = &node.env {
                for (k, v) in env {
                    if let WorkloadEnvValue::Ref(r) = v {
                        let info = node_info.get(&r.node_id).ok_or_else(|| ComposeError::validation(format!("Ref node '{}' not found", r.node_id)))?;
                        let val = match r.projection {
                            RefProjection::Endpoint => {
                                let port_str = r.port.as_ref().ok_or_else(|| ComposeError::validation("Port required for endpoint projection".into()))?;
                                // Look up mapped port in inspect info
                                let mapped = info.ports.iter().find(|p| p.contains(port_str))
                                    .cloned()
                                    .unwrap_or_else(|| format!("{}:{}", "127.0.0.1", port_str));
                                mapped
                            }
                            RefProjection::Ip => {
                                // In a real setup, extract IP from inspect JSON
                                "127.0.0.1".into()
                            }
                            RefProjection::InternalUrl => {
                                let port = r.port.as_deref().unwrap_or("80");
                                format!("http://{}:{}", "127.0.0.1", port)
                            }
                        };

                        // Inject into container
                        let container_name = format!("{}_{}", self.project_name, id);
                        self.backend.exec(&container_name, &["sh".into(), "-c".into(), format!("export {}={}", k, val)], None, None).await?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn register_engine(engine: Arc<ComposeEngine>, id: u64) {
    COMPOSE_HANDLES.insert(id, engine);
}
