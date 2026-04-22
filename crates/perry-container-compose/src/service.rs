use crate::error::Result;
use crate::backend::ContainerBackend;
use crate::types::{ContainerSpec, ContainerInfo, ListOrDict, BuildSpec, ComposeServiceBuild};
use md5::{Digest, Md5};
use std::collections::HashMap;

pub struct Service {
    pub name: String,
    pub image: Option<String>,
    pub container_name: Option<String>,
    pub ports: Option<Vec<String>>,
    pub environment: Option<ListOrDict>,
    pub labels: Option<ListOrDict>,
    pub volumes: Option<Vec<String>>,
    pub build: Option<BuildSpec>,
    /// Stable suffix for container name generation.
    /// If None, a random one is generated and persisted during orchestration.
    pub stable_id: Option<u32>,
}

impl Service {
    pub fn new(name: String, image: Option<String>) -> Self {
        Self {
            name,
            image,
            container_name: None,
            ports: None,
            environment: None,
            labels: None,
            volumes: None,
            build: None,
            stable_id: None,
        }
    }

    pub fn generate_name(&self) -> String {
        if let Some(name) = &self.container_name {
            return name.clone();
        }

        let image = self.image.as_deref().unwrap_or("unknown");
        let mut hasher = Md5::new();
        hasher.update(image.as_bytes());
        let hash = hex::encode(hasher.finalize());
        let short_hash = &hash[..8];

        let suffix = self.stable_id.unwrap_or(0);

        let safe_name: String = self.name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
            .collect();

        format!("{}_{}_{:08x}", safe_name, short_hash, suffix)
    }

    pub async fn exists(&self, backend: &dyn ContainerBackend) -> Result<bool> {
        let name = self.generate_name();
        match backend.inspect(&name).await {
            Ok(_) => Ok(true),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn is_running(&self, backend: &dyn ContainerBackend) -> Result<bool> {
        let name = self.generate_name();
        match backend.inspect(&name).await {
            Ok(info) => Ok(info.status.to_lowercase().contains("running")),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn needs_build(&self, backend: &dyn ContainerBackend) -> Result<bool> {
        if self.build.is_none() {
            return Ok(false);
        }
        if self.image.is_none() {
            return Ok(true);
        }
        let image_ref = self.image.as_ref().unwrap();
        match backend.inspect_image(image_ref).await {
            Ok(_) => Ok(false),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(true),
            Err(e) => Err(e),
        }
    }

    pub async fn run_command(&self, backend: &dyn ContainerBackend) -> Result<()> {
        if self.needs_build(backend).await? {
            self.build_command(backend).await?;
        }
        let spec = self.to_container_spec();
        backend.run(&spec).await.map(|_| ())
    }

    pub async fn start_command(&self, backend: &dyn ContainerBackend) -> Result<()> {
        let name = self.generate_name();
        backend.start(&name).await
    }

    pub async fn build_command(&self, backend: &dyn ContainerBackend) -> Result<()> {
        if let Some(build) = &self.build {
            let image_name = self.image.clone().unwrap_or_else(|| format!("{}-image", self.name));
            backend.build(&build.as_build(), &image_name).await?;
        }
        Ok(())
    }

    pub async fn inspect_command(&self, backend: &dyn ContainerBackend) -> Result<ContainerInfo> {
        let name = self.generate_name();
        backend.inspect(&name).await
    }

    fn to_container_spec(&self) -> ContainerSpec {
        ContainerSpec {
            image: self.image.clone().unwrap_or_default(),
            name: Some(self.generate_name()),
            ports: self.ports.clone(),
            volumes: self.volumes.clone(),
            env: self.environment.as_ref().map(|e| e.to_map()),
            cmd: None, // Logic for cmd/entrypoint would go here
            entrypoint: None,
            network: None,
            rm: None,
        }
    }
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}

pub fn service_container_name(service: &crate::types::ComposeService, service_name: &str) -> String {
    if let Some(name) = service.container_name.as_ref() {
        return name.clone();
    }

    let image = service.image.as_deref().unwrap_or("unknown");
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let short_hash = &hash[..8];

    let random_suffix: u32 = rand::random();

    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
}
