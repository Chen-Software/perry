use crate::error::Result;
use crate::types::{Container, ComposeServiceBuild, ListOrDict, ContainerInfo};
use crate::backend::ContainerBackend;
use md5::{Digest, Md5};
use std::collections::HashMap;

pub struct Service {
    pub image: Option<String>,
    pub name: Option<String>,           // container_name in YAML
    pub ports: Option<Vec<String>>,
    pub environment: Option<ListOrDict>,
    pub labels: Option<ListOrDict>,
    pub volumes: Option<Vec<String>>,
    pub build: Option<ComposeServiceBuild>,
}

impl Service {
    pub fn name(&self, service_name: &str) -> String {
        self.name.clone().unwrap_or_else(|| service_name.to_string())
    }

    pub fn generate_name(&self, service_name: &str) -> String {
        let image = self.image.as_deref().unwrap_or("unknown");
        let mut hasher = Md5::new();
        hasher.update(image.as_bytes());
        let hash = hex::encode(hasher.finalize());
        let short_hash = &hash[..8];

        let random_suffix: u32 = rand::random();

        format!("{}_{}_{:08x}", service_name, short_hash, random_suffix)
    }

    pub async fn exists(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<bool> {
        match backend.inspect(&self.name(service_name)).await {
            Ok(_) => Ok(true),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn is_running(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<bool> {
        match backend.inspect(&self.name(service_name)).await {
            Ok(info) => Ok(info.status.to_lowercase().contains("running") || info.status.to_lowercase().contains("up")),
            Err(crate::error::ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn needs_build(&self, service_name: &str, backend: &dyn ContainerBackend) -> Result<bool> {
        if self.build.is_none() {
            return Ok(false);
        }
        if self.image.is_none() {
            return Ok(true);
        }
        if let Some(image) = &self.image {
             match backend.inspect(image).await {
                 Ok(_) => Ok(false),
                 Err(_) => Ok(true),
             }
        } else {
            Ok(false)
        }
    }
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
