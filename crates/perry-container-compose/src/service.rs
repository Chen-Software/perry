use crate::backend::ContainerBackend;
use crate::error::{ComposeError, Result};
use crate::types::{ContainerInfo, ContainerSpec, ListOrDict};
use md5::{Digest, Md5};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Service {
    pub image: Option<String>,
    pub name: Option<String>, // container_name in YAML
    pub ports: Option<Vec<String>>,
    pub environment: Option<ListOrDict>,
    pub labels: Option<ListOrDict>,
    pub volumes: Option<Vec<String>>,
    pub build: Option<ServiceBuild>,
}

#[derive(Debug, Clone)]
pub struct ServiceBuild {
    pub context: String,
    pub dockerfile: Option<String>,
    pub args: Option<HashMap<String, String>>,
    pub labels: Option<ListOrDict>,
    pub target: Option<String>,
    pub network: Option<String>,
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

impl Service {
    pub fn generate_name(&self, service_name: &str) -> String {
        if let Some(name) = self.name.as_ref() {
            return name.clone();
        }

        let image = self.image.as_deref().unwrap_or("unknown");
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

    pub async fn exists(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<bool> {
        let name = self.generate_name(service_name);
        match backend.inspect(&name).await {
            Ok(_) => Ok(true),
            Err(ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn is_running(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<bool> {
        let name = self.generate_name(service_name);
        match backend.inspect(&name).await {
            Ok(info) => Ok(info.status.to_lowercase().contains("running")),
            Err(ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn needs_build(&self, backend: &dyn ContainerBackend) -> bool {
        if self.build.is_none() {
            return false;
        }

        if self.image.is_none() {
            return true;
        }

        if let Some(image) = &self.image {
            return backend.inspect_image(image).await.is_err();
        }

        false
    }

    pub async fn run_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<()> {
        if self.needs_build(backend).await {
            self.build_command(service_name, backend).await?;
        }

        let spec = self.to_container_spec(service_name);
        backend.run(&spec).await.map(|_| ())
    }

    pub async fn start_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<()> {
        let name = self.generate_name(service_name);
        backend.start(&name).await
    }

    pub async fn build_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<()> {
        if let Some(build) = &self.build {
            let image_name = self.image.clone().unwrap_or_else(|| format!("{}-image", service_name));
            let build_spec = crate::types::ComposeServiceBuild {
                context: Some(build.context.clone()),
                dockerfile: build.dockerfile.clone(),
                args: build.args.as_ref().map(|m| ListOrDict::Dict(m.iter().map(|(k, v)| (k.clone(), Some(serde_yaml::Value::String(v.clone())))).collect())),
                labels: build.labels.clone(),
                target: build.target.clone(),
                network: build.network.clone(),
                ..Default::default()
            };
            backend.build(&build_spec, &image_name).await
        } else {
            Ok(())
        }
    }

    pub async fn inspect_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<ContainerInfo> {
        let name = self.generate_name(service_name);
        backend.inspect(&name).await
    }

    fn to_container_spec(&self, service_name: &str) -> ContainerSpec {
        let name = self.generate_name(service_name);
        ContainerSpec {
            image: self.image.clone().unwrap_or_else(|| format!("{}-image", service_name)),
            name: Some(name),
            ports: self.ports.clone(),
            volumes: self.volumes.clone(),
            env: self.environment.as_ref().map(|e| e.to_map()),
            cmd: None, // Service command is handled separately in ComposeService
            entrypoint: None,
            network: None,
            rm: Some(false),
        }
    }
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
