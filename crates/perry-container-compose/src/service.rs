use serde::{Deserialize, Serialize};
use crate::types::*;
use crate::backend::ContainerBackend;
use crate::error::ComposeError;
use md5::{Md5, Digest};
use rand::Rng;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Service {
    pub image: Option<String>,
    pub container_name: Option<String>,
    pub ports: Option<Vec<String>>,
    pub environment: Option<ListOrDict>,
    pub labels: Option<ListOrDict>,
    pub volumes: Option<Vec<String>>,
    pub build: Option<ComposeServiceBuild>,
}

impl Service {
    pub fn name(&self, default: &str) -> String {
        self.container_name.clone().unwrap_or_else(|| default.to_string())
    }

    pub fn generate_name(image: &str, service_name: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(image.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let mut rng = rand::thread_rng();
        let suffix: u32 = rng.gen();
        format!("{}_{}_{}", service_name, &hash[0..8], suffix)
    }

    pub async fn exists(&self, backend: &dyn ContainerBackend, name: &str) -> Result<bool, ComposeError> {
        match backend.inspect(name).await {
            Ok(_) => Ok(true),
            Err(ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn is_running(&self, backend: &dyn ContainerBackend, name: &str) -> Result<bool, ComposeError> {
        match backend.inspect(name).await {
            Ok(info) => Ok(info.status == "running"),
            Err(ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn needs_build(&self) -> bool {
        self.build.is_some() && self.image.is_none()
    }
}

impl From<ComposeService> for Service {
    fn from(cs: ComposeService) -> Self {
        let build = match cs.build {
            Some(ComposeServiceBuildOrString::Build(b)) => Some(b),
            _ => None,
        };

        let ports = cs.ports.map(|ps| {
            ps.into_iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
        });

        let volumes = cs.volumes.map(|vs| {
            vs.into_iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
        });

        Service {
            image: cs.image,
            container_name: cs.container_name,
            ports,
            environment: cs.environment,
            labels: cs.labels,
            volumes,
            build,
        }
    }
}
