use crate::error::{ComposeError, Result};
use crate::backend::ContainerBackend;
use crate::types::{ComposeService, ContainerInfo, ContainerSpec, ListOrDict};
use md5::{Digest, Md5};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub image: Option<String>,
    pub name: Option<String>,           // container_name in YAML
    pub ports: Option<Vec<String>>,
    pub environment: Option<ListOrDict>,
    pub labels: Option<ListOrDict>,
    pub volumes: Option<Vec<String>>,
    pub build: Option<ServiceBuild>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceBuild {
    pub context: String,
    pub containerfile: Option<String>,
    pub args: Option<HashMap<String, String>>,
    pub labels: Option<ListOrDict>,
    pub target: Option<String>,
    pub network: Option<String>,
}

impl Service {
    pub fn generate_name(image: &str, service_name: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(image.as_bytes());
        let hash = hex::encode(hasher.finalize());
        let short_hash = &hash[..8];

        let random_suffix: u32 = rand::random();

        let safe_name: String = service_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
            .collect();

        format!("{}_{}_{}", safe_name, short_hash, random_suffix)
    }

    pub fn name(&self, service_name: &str) -> String {
        if let Some(name) = &self.name {
            name.clone()
        } else {
            Self::generate_name(self.image.as_deref().unwrap_or("unknown"), service_name)
        }
    }

    pub async fn exists(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<bool> {
        let name = self.name(service_name);
        match backend.inspect(&name).await {
            Ok(_) => Ok(true),
            Err(ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn is_running(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<bool> {
        let name = self.name(service_name);
        match backend.inspect(&name).await {
            Ok(info) => Ok(info.status == "running" || info.status.to_lowercase().contains("up")),
            Err(ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn needs_build(&self) -> bool {
        self.build.is_some() && self.image.is_none()
    }

    pub async fn run_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<()> {
        if self.needs_build() {
            self.build_command(service_name, backend).await?;
        }

        let container_name = self.name(service_name);
        let spec = ContainerSpec {
            image: self.image.clone().unwrap_or_else(|| format!("{}-image", service_name)),
            name: Some(container_name),
            ports: self.ports.clone(),
            volumes: self.volumes.clone(),
            env: self.environment.as_ref().map(|e| e.to_map()),
            cmd: None, // Service entity doesn't have cmd in Task 0.1 list
            rm: Some(false),
            ..Default::default()
        };

        backend.run(&spec).await.map(|_| ())
    }

    pub async fn start_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<()> {
        let container_name = self.name(service_name);
        backend.start(&container_name).await
    }

    pub async fn build_command(&self, _service_name: &str, backend: &dyn ContainerBackend) -> Result<()> {
        if let Some(build) = &self.build {
            // Mapping ServiceBuild to ComposeServiceBuild for the backend
            let spec = crate::types::ComposeServiceBuild {
                context: Some(build.context.clone()),
                containerfile: build.containerfile.clone(),
                args: build.args.as_ref().map(|a| ListOrDict::Dict(a.iter().map(|(k, v)| (k.clone(), Some(serde_yaml::Value::String(v.clone())))).collect())),
                labels: build.labels.clone(),
                target: build.target.clone(),
                network: build.network.clone(),
                ..Default::default()
            };
            let image_name = self.image.clone().unwrap_or_else(|| format!("{}-image", _service_name));
            backend.build(&spec, &image_name).await
        } else {
            Ok(())
        }
    }

    pub async fn inspect_command(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<ContainerInfo> {
        let container_name = self.name(service_name);
        backend.inspect(&container_name).await
    }
}

// Keep existing function for backward compatibility or migration
pub fn service_container_name(service: &ComposeService, service_name: &str) -> String {
    if let Some(name) = service.container_name.as_ref() {
        return name.clone();
    }
    Service::generate_name(service.image.as_deref().unwrap_or("unknown"), service_name)
}
