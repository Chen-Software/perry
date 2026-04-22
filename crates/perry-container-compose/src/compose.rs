use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::service::Service;
use crate::types::{ComposeHandle, ComposeSpec, ContainerInfo, ContainerLogs, ContainerSpec};
use indexmap::IndexMap;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use serde::Serialize;
use std::fmt;

#[derive(Serialize)]
pub struct ComposeEngine {
    spec: ComposeSpec,
    project_name: String,
    #[serde(skip)]
    backend: Arc<dyn ContainerBackend>,
}

impl fmt::Debug for ComposeEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComposeEngine")
            .field("spec", &self.spec)
            .field("project_name", &self.project_name)
            .field("backend", &self.backend.backend_name())
            .finish()
    }
}

impl ComposeEngine {
    pub fn new(spec: ComposeSpec, project_name: String, backend: Arc<dyn ContainerBackend>) -> Self {
        Self {
            spec,
            project_name,
            backend,
        }
    }

    pub async fn up(
        &self,
        services: &[String],
        _detach: bool,
        _build: bool,
        _remove_orphans: bool,
    ) -> Result<ComposeHandle> {
        let startup_order = resolve_startup_order(&self.spec)?;

        // Filter by requested services if provided
        let targets: Vec<String> = if services.is_empty() {
            startup_order
        } else {
            startup_order.into_iter().filter(|s| services.contains(s)).collect()
        };

        // Requirement 6.8 & 6.9: Create networks and volumes before containers
        if let Some(networks) = &self.spec.networks {
            for (name, config) in networks {
                if let Some(conf) = config {
                    self.backend.create_network(name, conf).await?;
                }
            }
        }
        if let Some(volumes) = &self.spec.volumes {
            for (name, config) in volumes {
                if let Some(conf) = config {
                    self.backend.create_volume(name, conf).await?;
                }
            }
        }

        let mut started = Vec::new();

        // Requirement 6.13: Generate names and start services
        for svc_name in &targets {
            let svc = self.spec.services.get(svc_name).ok_or_else(|| {
                ComposeError::ServiceNotFound { name: svc_name.clone() }
            })?;

            let container_name = Service::generate_name(svc.image.as_deref().unwrap_or(""), svc_name);

            let container_spec = ContainerSpec {
                image: svc.image.clone().unwrap_or_default(),
                name: Some(container_name.clone()),
                ports: svc.ports.as_ref().map(|p| p.iter().map(|ps| format!("{:?}", ps)).collect::<Vec<String>>()),
                env: match &svc.environment {
                    Some(crate::types::ListOrDict::Dict(d)) => Some(d.iter().map(|(k, v)| (k.clone(), format!("{:?}", v))).collect()),
                    _ => None,
                },
                cmd: match &svc.command {
                    Some(serde_yaml::Value::String(s)) => Some(vec![s.clone()]),
                    Some(serde_yaml::Value::Sequence(seq)) => Some(seq.iter().map(|v| format!("{:?}", v)).collect()),
                    _ => None,
                },
                ..Default::default()
            };

            match self.backend.run(&container_spec).await {
                Ok(_) => {
                    started.push(svc_name.clone());
                }
                Err(e) => {
                    // Requirement 6.10: Rollback on failure
                    for to_stop in started.into_iter().rev() {
                         let _ = self.backend.remove(&to_stop, true).await;
                    }
                    return Err(ComposeError::ServiceStartupFailed {
                        service: svc_name.clone(),
                        message: e.to_string()
                    });
                }
            }
        }

        Ok(ComposeHandle {
            stack_id: rand::random(),
            project_name: self.project_name.clone(),
            services: targets,
        })
    }

    pub async fn down(&self, volumes: bool, _remove_orphans: bool) -> Result<()> {
        let order = resolve_startup_order(&self.spec)?;
        for svc_name in order.into_iter().rev() {
            // Best effort removal
            let _ = self.backend.remove(&svc_name, true).await;
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

    pub async fn logs(&self, service: Option<&str>, tail: Option<u32>) -> Result<ContainerLogs> {
        if let Some(svc) = service {
             self.backend.logs(svc, tail).await
        } else {
             // Concatenate all logs
             let mut stdout = String::new();
             let mut stderr = String::new();
             for svc_name in self.spec.services.keys() {
                 if let Ok(l) = self.backend.logs(svc_name, tail).await {
                     stdout.push_str(&format!("--- {} ---\n{}", svc_name, l.stdout));
                     stderr.push_str(&format!("--- {} ---\n{}", svc_name, l.stderr));
                 }
             }
             Ok(ContainerLogs { stdout, stderr })
        }
    }

    pub async fn exec(&self, service: &str, cmd: &[String]) -> Result<ContainerLogs> {
        self.backend.exec(service, cmd, None, None).await
    }

    pub async fn start(&self, services: &[String]) -> Result<()> {
        for svc in services {
            self.backend.start(svc).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, services: &[String]) -> Result<()> {
        for svc in services {
            self.backend.stop(svc, None).await?;
        }
        Ok(())
    }

    pub async fn restart(&self, services: &[String]) -> Result<()> {
        for svc in services {
            self.backend.stop(svc, None).await?;
            self.backend.start(svc).await?;
        }
        Ok(())
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
                        message: format!("Service '{}' depends on '{}' which is not defined", name, dep),
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

    let mut order = Vec::new();
    while let Some(service) = queue.pop_first() {
        order.push(service.clone());
        if let Some(deps_list) = dependents.get(&service) {
            for dependent in deps_list {
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
        return Err(ComposeError::DependencyCycle {
            services: cycle_services,
        });
    }

    Ok(order)
}

pub struct Orchestrator {
    engine: ComposeEngine,
}

impl Orchestrator {
    pub fn new(
        files: &[std::path::PathBuf],
        project_name: Option<String>,
        env_files: &[std::path::PathBuf],
    ) -> Result<Self> {
        let project = crate::project::ComposeProject::load(files, project_name, env_files)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio");
        let backend = runtime.block_on(crate::backend::detect_backend()).map_err(|probed| {
            ComposeError::NoBackendFound { probed }
        })?;

        Ok(Self {
            engine: ComposeEngine::new(
                project.spec,
                project.project_name,
                Arc::from(backend as Box<dyn ContainerBackend>),
            ),
        })
    }

    pub async fn up(&self, services: &[String], detach: bool, build: bool) -> Result<()> {
        self.engine.up(services, detach, build, false).await.map(|_| ())
    }

    pub async fn down(&self, services: &[String], volumes: bool, remove_orphans: bool) -> Result<()> {
        // Scoped down is not fully implemented in this stub
        self.engine.down(volumes, remove_orphans).await
    }

    pub async fn ps(&self) -> Result<Vec<ServiceStatus>> {
        let containers = self.engine.ps().await?;
        Ok(containers.into_iter().map(|c| ServiceStatus {
            service_name: c.name.clone(),
            container_name: c.name,
            status: ContainerStatus::Running,
        }).collect())
    }

    pub async fn logs(&self, services: &[String], tail: Option<u32>, _follow: bool) -> Result<HashMap<String, String>> {
        let mut map = HashMap::new();
        for svc in services {
            let logs = self.engine.logs(Some(svc), tail).await?;
            map.insert(svc.clone(), logs.stdout);
        }
        Ok(map)
    }

    pub async fn exec(&self, service: &str, cmd: &[String], _user: Option<&str>, _workdir: Option<&str>, _env: Option<&HashMap<String, String>>) -> Result<ExecResult> {
        let logs = self.engine.exec(service, cmd).await?;
        Ok(ExecResult {
            stdout: logs.stdout,
            stderr: logs.stderr,
            exit_code: 0,
        })
    }

    pub fn config(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.engine.spec).unwrap_or_default())
    }
}

pub struct ServiceStatus {
    pub service_name: String,
    pub container_name: String,
    pub status: ContainerStatus,
}

pub enum ContainerStatus {
    Running,
    Stopped,
    NotFound,
}

pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
