use crate::error::{Result, ComposeError};
use crate::types::{ListOrDict, ContainerSpec, ContainerInfo};
use crate::backend::ContainerBackend;
use md5::{Digest, Md5};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceBuild {
    pub context: String,
    pub dockerfile: Option<String>,
    pub args: Option<HashMap<String, String>>,
    pub labels: Option<ListOrDict>,
    pub target: Option<String>,
    pub network: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub image: Option<String>,
    #[serde(rename = "container_name")]
    pub name: Option<String>,           // container_name in YAML
    pub ports: Option<Vec<String>>,
    pub environment: Option<ListOrDict>,
    pub labels: Option<ListOrDict>,
    pub volumes: Option<Vec<String>>,
    pub build: Option<ServiceBuild>,
    #[serde(skip)]
    pub service_name: String,           // The key in the services map
}

impl Service {
    pub fn name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.service_name.clone())
    }

    pub fn generate_name(&self) -> String {
        let image = self.image.as_deref().unwrap_or("");
        let mut hasher = Md5::new();
        hasher.update(image.as_bytes());
        let hash = hex::encode(hasher.finalize());
        let short_hash = &hash[..8];
        let random_u32: u32 = rand::random();
        format!("{}_{}_{}", self.name(), short_hash, random_u32)
    }

    pub async fn exists(&self, backend: &dyn ContainerBackend) -> Result<bool> {
        let name = self.name();
        let containers = backend.list(true).await?;
        Ok(containers.iter().any(|c| c.name == name || c.name.contains(&name)))
    }

    pub async fn is_running(&self, backend: &dyn ContainerBackend) -> Result<bool> {
        let name = self.name();
        let containers = backend.list(false).await?;
        Ok(containers.iter().any(|c| c.name == name || c.name.contains(&name)))
    }

    pub fn needs_build(&self) -> bool {
        self.build.is_some() && self.image.is_none()
    }

    pub async fn run_command(&self, backend: &dyn ContainerBackend) -> Result<()> {
        if self.needs_build() {
            self.build_command(backend).await?;
        }

        let spec = ContainerSpec {
            image: self.image.clone().unwrap_or_else(|| self.name()),
            name: Some(self.generate_name()),
            ports: self.ports.clone(),
            volumes: self.volumes.clone(),
            env: self.environment.as_ref().map(|e| e.to_map()),
            cmd: None,
            entrypoint: None,
            network: None,
            rm: Some(false),
            read_only: None,
        };
        backend.run(&spec).await?;
        Ok(())
    }

    pub async fn start_command(&self, backend: &dyn ContainerBackend) -> Result<()> {
        let name = self.name();
        let containers = backend.list(true).await?;
        if let Some(c) = containers.iter().find(|c| c.name == name || c.name.contains(&name)) {
            backend.start(&c.id).await?;
            Ok(())
        } else {
            Err(ComposeError::NotFound(name))
        }
    }

    pub async fn build_command(&self, backend: &dyn ContainerBackend) -> Result<()> {
        if let Some(build) = &self.build {
            backend.build(&build.context, build.dockerfile.as_deref(), &[self.image.clone().unwrap_or_else(|| self.name())]).await?;
            Ok(())
        } else {
            Ok(())
        }
    }

    pub async fn inspect_command(&self, backend: &dyn ContainerBackend) -> Result<ContainerInfo> {
        let name = self.name();
        let containers = backend.list(true).await?;
        if let Some(c) = containers.into_iter().find(|c| c.name == name || c.name.contains(&name)) {
            backend.inspect(&c.id).await
        } else {
            Err(ComposeError::NotFound(name))
        }
    }
}

pub fn service_container_name(service: &crate::types::ComposeService, service_name: &str) -> String {
    if let Some(name) = service.container_name.as_ref() {
        return name.clone();
    }

    let service_yaml = serde_yaml::to_string(service).unwrap_or_default();
    let mut hasher = Md5::new();
    hasher.update(service_yaml.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let short_hash = &hash[..8];

    let random_suffix: u32 = rand::random();

    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
}

pub fn needs_build(service: &crate::types::ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
