//! Core orchestration logic — start, stop, ps, logs, exec, config commands.
//!
//! Mirrors cmd/start/cmd.go and sibling command files from the original Go project.

pub mod deps;
pub mod env;
pub mod project;

use crate::backend::{get_backend, Backend};
use crate::commands::ContainerStatus;
use crate::error::{ComposeError, Result};
use crate::orchestrate::deps::topological_order;
use crate::orchestrate::project::Project;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

// ============ Service Status ============

/// Service status entry used by the `ps` command
#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub service_name: String,
    pub container_name: String,
    pub status: ContainerStatus,
}

// ============ Orchestration core ============

/// Orchestrator holds the project and backend, providing high-level compose operations.
pub struct Orchestrator {
    pub project: Project,
    pub backend: Arc<dyn Backend>,
}

impl Orchestrator {
    /// Create an orchestrator from command-line options.
    pub fn new(
        files: &[PathBuf],
        project_name: Option<&str>,
        env_files: &[PathBuf],
    ) -> Result<Self> {
        let project = Project::load(files, project_name, env_files)?;
        let backend = Arc::from(get_backend()?);
        Ok(Orchestrator { project, backend })
    }

    // ============ up / start ============

    /// Bring up all services (or a subset), starting them in dependency order.
    pub async fn up(&self, services: &[String], detach: bool, _build: bool) -> Result<()> {
        let order = topological_order(&self.project.compose)?;

        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order
                .iter()
                .filter(|s| services.contains(s))
                .collect()
        };

        for svc_name in target {
            let svc = self.project.compose.services.get(svc_name).unwrap();
            info!("Starting service '{}'…", svc_name);

            let container_name = svc.generate_name(svc_name)?;
            let status = self.backend.inspect(&container_name).await?;

            match status {
                ContainerStatus::Running => {
                    info!("Service '{}' already running — skip", svc_name);
                }
                ContainerStatus::Stopped => {
                    info!("Service '{}' exists but stopped — restarting", svc_name);
                    self.backend.start(&container_name).await.map_err(|e| {
                        ComposeError::ExecError {
                            service: svc_name.clone(),
                            message: e.to_string(),
                        }
                    })?;
                    info!("Service '{}' started", svc_name);
                }
                ContainerStatus::NotFound => {
                    // Build if needed
                    if svc.needs_build() {
                        let build = svc.build.as_ref().unwrap();
                        let context = build
                            .context
                            .as_deref()
                            .unwrap_or(".");
                        let tag = svc.image_ref(svc_name);
                        info!("Building image '{}' for service '{}'…", tag, svc_name);
                        self.backend
                            .build(
                                context,
                                build.dockerfile.as_deref(),
                                &tag,
                                build.args.as_ref(),
                                build.target.as_deref(),
                                build.network.as_deref(),
                            )
                            .await
                            .map_err(|e| ComposeError::ExecError {
                                service: svc_name.clone(),
                                message: e.to_string(),
                            })?;
                    }

                    let image = svc.image_ref(svc_name);
                    let env = svc.resolved_env();
                    let labels = svc.labels.as_ref().cloned().unwrap_or_default();

                    // Add project label for later filtering
                    let mut all_labels = labels;
                    all_labels.insert(
                        "perry.compose.project".into(),
                        self.project.name.clone(),
                    );
                    all_labels.insert(
                        "perry.compose.service".into(),
                        svc_name.clone(),
                    );

                    info!("Running container '{}' for service '{}'", container_name, svc_name);
                    self.backend
                        .run(
                            &image,
                            &container_name,
                            svc.ports.as_deref(),
                            if env.is_empty() { None } else { Some(&env) },
                            svc.volumes.as_deref(),
                            Some(&all_labels),
                            svc.command.as_ref().map(|c| c.to_list()).as_deref(),
                            detach,
                        )
                        .await
                        .map_err(|e| ComposeError::ExecError {
                            service: svc_name.clone(),
                            message: e.to_string(),
                        })?;
                    info!("Service '{}' started", svc_name);
                }
            }
        }

        Ok(())
    }

    // ============ down / stop ============

    /// Stop and remove all (or specified) services, in reverse dependency order.
    pub async fn down(
        &self,
        services: &[String],
        _remove_orphans: bool,
        _volumes: bool,
    ) -> Result<()> {
        let mut order = topological_order(&self.project.compose)?;
        order.reverse(); // stop in reverse dependency order

        let target: Vec<&String> = if services.is_empty() {
            order.iter().collect()
        } else {
            order
                .iter()
                .filter(|s| services.contains(s))
                .collect()
        };

        for svc_name in target {
            let svc = self.project.compose.services.get(svc_name).unwrap();
            let container_name = svc.generate_name(svc_name)?;
            let status = self.backend.inspect(&container_name).await?;

            if status == ContainerStatus::Running {
                info!("Stopping service '{}'…", svc_name);
                self.backend.stop(&container_name).await.map_err(|e| {
                    ComposeError::ExecError {
                        service: svc_name.clone(),
                        message: e.to_string(),
                    }
                })?;
            }

            if status != ContainerStatus::NotFound {
                info!("Removing container '{}' for service '{}'…", container_name, svc_name);
                self.backend.remove(&container_name, true).await.map_err(|e| {
                    ComposeError::ExecError {
                        service: svc_name.clone(),
                        message: e.to_string(),
                    }
                })?;
                info!("Service '{}' removed", svc_name);
            }
        }

        Ok(())
    }

    // ============ ps ============

    /// List the status of all services
    pub async fn ps(&self) -> Result<Vec<ServiceStatus>> {
        let mut results = Vec::new();

        for (svc_name, svc) in &self.project.compose.services {
            let container_name = svc.generate_name(svc_name)?;
            let status = self.backend.inspect(&container_name).await?;
            results.push(ServiceStatus {
                service_name: svc_name.clone(),
                container_name,
                status,
            });
        }

        // Sort by service name for consistent output
        results.sort_by(|a, b| a.service_name.cmp(&b.service_name));
        Ok(results)
    }

    // ============ logs ============

    /// Get logs from one or more services
    pub async fn logs(
        &self,
        services: &[String],
        tail: Option<u32>,
        follow: bool,
    ) -> Result<HashMap<String, String>> {
        let service_names: Vec<&String> = if services.is_empty() {
            self.project.compose.services.keys().collect()
        } else {
            services.iter().collect()
        };

        let mut all_logs = HashMap::new();

        for svc_name in service_names {
            let svc = self
                .project
                .compose
                .services
                .get(svc_name)
                .ok_or_else(|| ComposeError::ServiceNotFound {
                    name: svc_name.clone(),
                })?;

            let container_name = svc.generate_name(svc_name)?;
            let logs = self
                .backend
                .logs(&container_name, tail, follow)
                .await
                .map_err(|e| ComposeError::ExecError {
                    service: svc_name.clone(),
                    message: e.to_string(),
                })?;
            all_logs.insert(svc_name.clone(), logs);
        }

        Ok(all_logs)
    }

    // ============ exec ============

    /// Execute a command in a running service container
    pub async fn exec(
        &self,
        service: &str,
        cmd: &[String],
        user: Option<&str>,
        workdir: Option<&str>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<crate::backend::ExecResult> {
        let svc = self
            .project
            .compose
            .services
            .get(service)
            .ok_or_else(|| ComposeError::ServiceNotFound {
                name: service.to_owned(),
            })?;

        let container_name = svc.generate_name(service)?;
        let status = self.backend.inspect(&container_name).await?;

        if status != ContainerStatus::Running {
            return Err(ComposeError::ExecError {
                service: service.to_owned(),
                message: format!(
                    "container '{}' is not running",
                    container_name
                ),
            });
        }

        self.backend.exec(&container_name, cmd, user, workdir, env).await
    }

    // ============ config ============

    /// Validate and display the parsed compose configuration
    pub fn config(&self) -> Result<String> {
        self.project.compose.to_yaml()
    }
}
