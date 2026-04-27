use std::collections::HashMap;
use crate::types::{ComposeService, ContainerSpec};
use crate::backend::ContainerBackend;
use crate::error::ComposeError;

pub struct Service {
    pub name: String,
    pub config: ComposeService,
}

impl Service {
    pub fn new(name: String, config: ComposeService) -> Self {
        Self { name, config }
    }

    pub fn container_name(&self) -> &str {
        self.config.container_name.as_deref().unwrap_or(&self.name)
    }

    pub async fn exists(&self, backend: &dyn ContainerBackend) -> Result<bool, ComposeError> {
        match backend.inspect(self.container_name()).await {
            Ok(_) => Ok(true),
            Err(ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn is_running(&self, backend: &dyn ContainerBackend) -> Result<bool, ComposeError> {
        match backend.inspect(self.container_name()).await {
            Ok(info) => Ok(info.status.contains("running")),
            Err(ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn needs_build(&self) -> bool {
        self.config.build.is_some() && self.config.image.is_none()
    }

    pub async fn run_command(&self, backend: &dyn ContainerBackend) -> Result<(), ComposeError> {
        let spec = ContainerSpec {
            image: self.config.image.clone().unwrap_or_else(|| format!("{}_image", self.name)),
            name: Some(self.container_name().to_string()),
            ports: self.config.ports.as_ref().map(|p| p.iter().map(|i| match i {
                crate::types::PortOrString::String(s) => s.clone(),
                crate::types::PortOrString::Number(n) => n.to_string(),
                crate::types::PortOrString::Port(p) => format!("{}:{}", p.published.unwrap_or(0), p.target),
            }).collect()),
            volumes: self.config.volumes.as_ref().map(|v| v.iter().map(|i| match i {
                crate::types::VolumeOrString::String(s) => s.clone(),
                crate::types::VolumeOrString::Volume(v) => format!("{}:{}", v.source.as_deref().unwrap_or(""), v.target.as_deref().unwrap_or("")),
            }).collect()),
            env: self.config.environment.as_ref().and_then(|e| match e {
                crate::types::ListOrDict::Dict(d) => {
                    let mut m = HashMap::new();
                    for (k, v) in d {
                        m.insert(k.clone(), v.clone().unwrap_or_default());
                    }
                    Some(m)
                }
                _ => None,
            }),
            cmd: self.config.command.as_ref().map(|c| match c {
                crate::types::CommandOrArgs::String(s) => vec![s.clone()],
                crate::types::CommandOrArgs::List(l) => l.clone(),
            }),
            entrypoint: None,
            network: None,
            rm: Some(false),
        };
        backend.run(&spec).await?;
        Ok(())
    }

    pub async fn start_command(&self, backend: &dyn ContainerBackend) -> Result<(), ComposeError> {
        backend.start(self.container_name()).await
    }

    pub async fn build_command(&self, backend: &dyn ContainerBackend) -> Result<(), ComposeError> {
        if let Some(build_opt) = &self.config.build {
            let build = match build_opt {
                crate::types::ComposeServiceBuildOrString::String(s) => {
                    crate::types::ComposeServiceBuild {
                        context: s.clone(),
                        ..Default::default()
                    }
                }
                crate::types::ComposeServiceBuildOrString::Build(b) => b.clone(),
            };
            let image_name = self.config.image.clone().unwrap_or_else(|| format!("{}_image", self.name));
            backend.build(&build, &image_name).await?;
        }
        Ok(())
    }
}

pub fn generate_name(image: &str, service_name: &str) -> String {
    let hash = format!("{:x}", md5::compute(image));
    format!("{}_{}_{}", service_name, &hash[..8], rand::random::<u32>())
}
