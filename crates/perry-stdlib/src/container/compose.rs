//! ComposeEngine wrapper — thin adapter over `perry_container_compose::ComposeEngine`.

use super::backend::ContainerBackend;
use super::types::{ComposeHandle, ComposeSpec, ContainerError};
use perry_container_compose::ComposeEngine;
use std::sync::Arc;

pub struct ComposeWrapper {
    inner: Arc<ComposeEngine>,
}

impl ComposeWrapper {
    pub fn new(spec: ComposeSpec, backend: Arc<dyn ContainerBackend>) -> Self {
        Self {
            inner: Arc::new(ComposeEngine::new(
                spec,
                "perry-compose-stack".to_string(),
                backend,
            )),
        }
    }

    pub async fn up(&self) -> Result<ComposeHandle, ContainerError> {
        self.inner
            .up(&[], true, true, true)
            .await
            .map_err(ContainerError::from)
    }

    pub async fn down(
        &self,
        handle: &ComposeHandle,
        remove_volumes: bool,
    ) -> Result<(), ContainerError> {
        // We need to find the engine for this handle or use the current one if it matches.
        // Actually, the handle contains stack_id.
        if let Some(engine) = ComposeEngine::get_engine(handle.stack_id) {
            engine
                .down(remove_volumes, true)
                .await
                .map_err(ContainerError::from)?;
            ComposeEngine::unregister(handle.stack_id);
            Ok(())
        } else {
            // Fallback to internal engine if not in registry
            self.inner
                .down(remove_volumes, true)
                .await
                .map_err(ContainerError::from)
        }
    }

    pub async fn ps(
        &self,
        handle: &ComposeHandle,
    ) -> Result<Vec<super::types::ContainerInfo>, ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).unwrap_or(Arc::clone(&self.inner));
        let infos = engine.ps().await.map_err(ContainerError::from)?;
        Ok(infos
            .into_iter()
            .map(|i| super::types::ContainerInfo {
                id: i.id,
                name: i.name,
                image: i.image,
                status: i.status,
                ports: i.ports,
                created: i.created,
            })
            .collect())
    }

    pub async fn logs(
        &self,
        handle: &ComposeHandle,
        service: Option<&str>,
        tail: Option<u32>,
    ) -> Result<super::types::ContainerLogs, ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).unwrap_or(Arc::clone(&self.inner));
        let logs = engine
            .logs(service, tail)
            .await
            .map_err(ContainerError::from)?;
        Ok(super::types::ContainerLogs {
            stdout: logs.stdout,
            stderr: logs.stderr,
        })
    }

    pub async fn exec(
        &self,
        handle: &ComposeHandle,
        service: &str,
        cmd: &[String],
    ) -> Result<super::types::ContainerLogs, ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).unwrap_or(Arc::clone(&self.inner));
        let logs = engine.exec(service, cmd).await.map_err(ContainerError::from)?;
        Ok(super::types::ContainerLogs {
            stdout: logs.stdout,
            stderr: logs.stderr,
        })
    }

    pub async fn start(
        &self,
        handle: &ComposeHandle,
        services: &[String],
    ) -> Result<(), ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).unwrap_or(Arc::clone(&self.inner));
        engine.start(services).await.map_err(ContainerError::from)
    }

    pub async fn stop(
        &self,
        handle: &ComposeHandle,
        services: &[String],
    ) -> Result<(), ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).unwrap_or(Arc::clone(&self.inner));
        engine.stop(services).await.map_err(ContainerError::from)
    }

    pub async fn restart(
        &self,
        handle: &ComposeHandle,
        services: &[String],
    ) -> Result<(), ContainerError> {
        let engine = ComposeEngine::get_engine(handle.stack_id).unwrap_or(Arc::clone(&self.inner));
        engine.restart(services).await.map_err(ContainerError::from)
    }
}
