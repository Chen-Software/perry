use crate::error::Result;
use crate::types::{ComposeService, ContainerInfo};
use crate::backend::ContainerBackend;
use crate::commands::{BuildCommand, RunCommand, StartCommand, ContainerCommand};
use md5::{Digest, Md5};

/// Generate a unique, stable container name for a service.
/// Uses the MD5 hash of the service configuration to ensure idempotency.
pub fn generate_name(service_name: &str, service: &ComposeService) -> Result<String> {
    if let Some(explicit) = &service.container_name {
        return Ok(explicit.clone());
    }

    // Serialize the service to YAML to get a stable representation of its config.
    // This matches the Go reference project and ensures the name only changes
    // if the service definition changes.
    let yaml = serde_yaml::to_string(service).map_err(|e| crate::error::ComposeError::ValidationError { message: e.to_string() })?;

    let mut hasher = Md5::new();
    hasher.update(yaml.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let short_hash = &hash[..8];

    let safe_service_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect();

    // {service_name}-{config_hash_prefix}
    Ok(format!("{}-{}", safe_service_name, short_hash))
}

/// Whether the service needs to build an image.
pub fn needs_build(service: &ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}

pub async fn exists(service_name: &str, service: &ComposeService, backend: &dyn ContainerBackend) -> Result<bool> {
    let name = generate_name(service_name, service)?;
    let list = backend.list(true).await?;
    Ok(list.iter().any(|c| c.name == name || c.id == name))
}

pub async fn is_running(service_name: &str, service: &ComposeService, backend: &dyn ContainerBackend) -> Result<bool> {
    let name = generate_name(service_name, service)?;
    let list = backend.list(false).await?;
    Ok(list.iter().any(|c| c.name == name || c.id == name))
}

pub async fn run_command(service_name: &str, service: &ComposeService, backend: &dyn ContainerBackend) -> Result<()> {
    RunCommand { service_name, service }.exec(backend).await
}

pub async fn start_command(service_name: &str, service: &ComposeService, backend: &dyn ContainerBackend) -> Result<()> {
    let name = generate_name(service_name, service)?;
    StartCommand { container_id: name }.exec(backend).await
}

pub async fn build_command(service_name: &str, service: &ComposeService, backend: &dyn ContainerBackend) -> Result<()> {
    BuildCommand { service_name, service }.exec(backend).await
}

pub async fn inspect_command(service_name: &str, service: &ComposeService, backend: &dyn ContainerBackend) -> Result<ContainerInfo> {
    let name = generate_name(service_name, service)?;
    backend.inspect(&name).await
}

#[derive(Debug, Clone)]
pub struct ServiceState {
    pub id: Option<String>,
    pub name: String,
    pub running: bool,
}
