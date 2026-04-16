//! Thin wrapper around `perry_container_compose::ComposeEngine`.

use super::backend::ContainerBackend;
use super::types::{ComposeHandle, ComposeSpec, ContainerError};
use std::sync::Arc;

pub struct ComposeWrapper {
    backend: Arc<dyn ContainerBackend>,
}

impl ComposeWrapper {
    pub fn new(_spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        Self { backend }
    }

    pub async fn up(
        &self,
        spec: &ComposeSpec,
        services: &[String],
    ) -> Result<ComposeHandle, ContainerError> {
        let compose_spec = spec_to_compose(spec).map_err(|e| ContainerError::InvalidConfig(e.to_string()))?;
        let engine = Arc::new(perry_container_compose::ComposeEngine::new(
            compose_spec,
            spec.name.clone().unwrap_or_else(|| "default".to_string()),
            Arc::clone(&self.backend),
        ));

        let handle = engine.up(services, true, false, false).await
            .map_err(map_compose_err)?;

        Ok(ComposeHandle {
            name: handle.project_name,
            services: handle.services,
            networks: Vec::new(),
            volumes: Vec::new(),
            containers: std::collections::HashMap::new(),
        })
    }

    pub async fn down(
        &self,
        _handle: &ComposeHandle,
        _remove_volumes: bool,
    ) -> Result<(), ContainerError> {
        Ok(())
    }

    pub async fn ps(
        &self,
        _handle: &ComposeHandle,
    ) -> Result<Vec<super::types::ContainerInfo>, ContainerError> {
        let list = self.backend.list(true).await.map_err(map_compose_err)?;
        Ok(list.into_iter().map(|info| super::types::ContainerInfo {
            id: info.id,
            name: info.name,
            image: info.image,
            status: info.status,
            ports: info.ports,
            created: info.created,
        }).collect())
    }

    pub async fn logs(
        &self,
        _handle: &ComposeHandle,
        service: Option<&str>,
        tail: Option<u32>,
    ) -> Result<super::types::ContainerLogs, ContainerError> {
        if let Some(s) = service {
            let logs = self.backend.logs(s, tail).await.map_err(map_compose_err)?;
            Ok(super::types::ContainerLogs {
                stdout: logs.stdout,
                stderr: logs.stderr,
                exit_code: logs.exit_code,
            })
        } else {
            Ok(super::types::ContainerLogs {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            })
        }
    }

    pub async fn exec(
        &self,
        _handle: &ComposeHandle,
        service: &str,
        cmd: &[String],
    ) -> Result<super::types::ContainerLogs, ContainerError> {
        let logs = self.backend.exec(service, cmd, None, None).await.map_err(map_compose_err)?;
        Ok(super::types::ContainerLogs {
            stdout: logs.stdout,
            stderr: logs.stderr,
            exit_code: logs.exit_code,
        })
    }
}

fn spec_to_compose(
    spec: &ComposeSpec,
) -> Result<perry_container_compose::types::ComposeSpec, serde_json::Error> {
    let json = serde_json::to_value(spec)?;
    serde_json::from_value(json)
}

fn map_compose_err(e: perry_container_compose::error::ComposeError) -> ContainerError {
    match e {
        perry_container_compose::error::ComposeError::NotFound(id) => {
            ContainerError::NotFound(id)
        }
        perry_container_compose::error::ComposeError::DependencyCycle { services } => {
            ContainerError::DependencyCycle { cycle: services }
        }
        perry_container_compose::error::ComposeError::ServiceStartupFailed { service, message } => {
            ContainerError::ServiceStartupFailed { service, error: message }
        }
        perry_container_compose::error::ComposeError::ValidationError { message } => {
            ContainerError::InvalidConfig(message)
        }
        other => ContainerError::BackendError {
            code: -1,
            message: other.to_string(),
        },
    }
}
