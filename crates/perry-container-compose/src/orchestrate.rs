use crate::error::Result;
use crate::service::Service;
use crate::backend::ContainerBackend;
use crate::commands::ContainerCommand;
use crate::commands::build::BuildCommand;
use crate::commands::run::RunCommand;
use crate::commands::start::StartCommand;
use crate::types::{Container, ListOrDict};
use std::collections::HashMap;

pub async fn orchestrate_service(
    service_name: &str,
    service: &Service,
    backend: &dyn ContainerBackend
) -> Result<()> {
    if service.is_running(service_name, backend).await? {
        tracing::info!(service = service_name, "already running, skipping");
        return Ok(());
    }

    if service.exists(service_name, backend).await? {
        tracing::info!(service = service_name, "exists but stopped, starting");
        let cmd = StartCommand { id: service.name(service_name) };
        cmd.exec(backend).await?;
    } else {
        if service.needs_build(service_name, backend).await? {
            tracing::info!(service = service_name, "building image");
            let build_spec = service.build.clone().unwrap();
            let image_name = service.image.clone().unwrap_or_else(|| format!("{}_image", service_name));
            let cmd = BuildCommand { spec: build_spec, image_name: image_name.clone() };
            cmd.exec(backend).await?;
        }
        tracing::info!(service = service_name, "creating and running");

        let container_name = service.name(service_name);
        let image = service.image.clone().unwrap_or_else(|| format!("{}_image", service_name));

        let mut env_map = HashMap::new();
        if let Some(env) = &service.environment {
             match env {
                 ListOrDict::Dict(d) => {
                     for (k, v) in d {
                         env_map.insert(k.clone(), v.as_ref().map(|val| format!("{:?}", val)).unwrap_or_default());
                     }
                 }
                 ListOrDict::List(l) => {
                     for s in l {
                         if let Some((k, v)) = s.split_once('=') {
                             env_map.insert(k.to_string(), v.to_string());
                         }
                     }
                 }
             }
        }

        let spec = Container {
            image,
            name: Some(container_name),
            ports: service.ports.clone(),
            volumes: service.volumes.clone(),
            env: if env_map.is_empty() { None } else { Some(env_map) },
            cmd: None, // From image or build
            entrypoint: None,
            network: None,
            rm: Some(false),
            read_only: Some(false),
        };

        let cmd = RunCommand { spec };
        cmd.exec(backend).await?;
    }
    Ok(())
}
