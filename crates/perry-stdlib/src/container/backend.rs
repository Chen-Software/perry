//! Backend abstraction for container runtimes.
//!
//! Re-exports the core backend from perry-container-compose and provides
//! a process-global singleton for the stdlib.

pub use perry_container_compose::backend::{ContainerBackend, detect_backend, BackendProbeResult};
use super::types::ContainerError;
use std::sync::{Arc, OnceLock};

static GLOBAL_BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

pub async fn get_backend_instance() -> Result<Arc<dyn ContainerBackend>, ContainerError> {
    if let Some(b) = GLOBAL_BACKEND.get() {
        return Ok(b.clone());
    }

    let b = detect_backend().await
        .map_err(|e| ContainerError::BackendError {
            code: 503,
            message: format!("No container backend found: {:?}", e),
        })?;

    // We don't care if it's already set by a concurrent call
    let _ = GLOBAL_BACKEND.set(b.clone());
    Ok(b)
}

/// Bridges stdlib's async functions with compose crate's requirements if needed.
/// Currently we use the same types, so this is an identity adapter.
pub struct BackendAdapter {
    pub inner: Arc<dyn ContainerBackend>,
}

#[async_trait::async_trait]
impl perry_container_compose::backend::ContainerBackend for BackendAdapter {
    fn backend_name(&self) -> &str { self.inner.backend_name() }
    async fn check_available(&self) -> perry_container_compose::Result<()> { self.inner.check_available().await }
    async fn run(&self, spec: &perry_container_compose::types::ContainerSpec) -> perry_container_compose::Result<perry_container_compose::types::ContainerHandle> { self.inner.run(spec).await }
    async fn create(&self, spec: &perry_container_compose::types::ContainerSpec) -> perry_container_compose::Result<perry_container_compose::types::ContainerHandle> { self.inner.create(spec).await }
    async fn start(&self, id: &str) -> perry_container_compose::Result<()> { self.inner.start(id).await }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> perry_container_compose::Result<()> { self.inner.stop(id, timeout).await }
    async fn remove(&self, id: &str, force: bool) -> perry_container_compose::Result<()> { self.inner.remove(id, force).await }
    async fn list(&self, all: bool) -> perry_container_compose::Result<Vec<perry_container_compose::types::ContainerInfo>> { self.inner.list(all).await }
    async fn inspect(&self, id: &str) -> perry_container_compose::Result<perry_container_compose::types::ContainerInfo> { self.inner.inspect(id).await }
    async fn logs(&self, id: &str, tail: Option<u32>) -> perry_container_compose::Result<perry_container_compose::types::ContainerLogs> { self.inner.logs(id, tail).await }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&std::collections::HashMap<String, String>>, workdir: Option<&str>) -> perry_container_compose::Result<perry_container_compose::types::ContainerLogs> { self.inner.exec(id, cmd, env, workdir).await }
    async fn pull_image(&self, reference: &str) -> perry_container_compose::Result<()> { self.inner.pull_image(reference).await }
    async fn list_images(&self) -> perry_container_compose::Result<Vec<perry_container_compose::types::ImageInfo>> { self.inner.list_images().await }
    async fn build(&self, spec: &perry_container_compose::types::ComposeServiceBuild, image_name: &str) -> perry_container_compose::Result<()> { self.inner.build(spec, image_name).await }
    async fn remove_image(&self, reference: &str, force: bool) -> perry_container_compose::Result<()> { self.inner.remove_image(reference, force).await }
    async fn create_network(&self, name: &str, config: &perry_container_compose::backend::NetworkConfig) -> perry_container_compose::Result<()> { self.inner.create_network(name, config).await }
    async fn remove_network(&self, name: &str) -> perry_container_compose::Result<()> { self.inner.remove_network(name).await }
    async fn create_volume(&self, name: &str, config: &perry_container_compose::backend::VolumeConfig) -> perry_container_compose::Result<()> { self.inner.create_volume(name, config).await }
    async fn remove_volume(&self, name: &str) -> perry_container_compose::Result<()> { self.inner.remove_volume(name).await }
}
