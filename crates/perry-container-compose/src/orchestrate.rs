use crate::service::Service;
use crate::backend::ContainerBackend;
use crate::error::ComposeError;
use crate::types::ContainerSpec;
use tracing::info;

pub async fn orchestrate_service(
    service_name: &str,
    service: &Service,
    backend: &dyn ContainerBackend,
) -> Result<(), ComposeError> {
    let name = service.name(service_name);

    if service.is_running(backend, &name).await? {
        info!(service = %name, "already running, skipping");
        return Ok(());
    }

    if service.exists(backend, &name).await? {
        info!(service = %name, "exists but stopped, starting");
        backend.start(&name).await?;
    } else {
        if service.needs_build() {
            if let Some(build) = &service.build {
                info!(service = %name, "building image");
                backend.build(build, service_name).await?;
            }
        }

        info!(service = %name, "creating and running");
        let spec = ContainerSpec {
            image: service.image.clone().unwrap_or_else(|| service_name.to_string()),
            name: Some(name.clone()),
            ports: service.ports.clone(),
            volumes: service.volumes.clone(),
            env: match &service.environment {
                Some(crate::types::ListOrDict::Dict(d)) => {
                    let mut env = std::collections::HashMap::new();
                    for (k, v) in d {
                        if let Some(val) = v {
                            env.insert(k.clone(), val.to_string());
                        }
                    }
                    Some(env)
                }
                _ => None,
            },
            cmd: None,
            entrypoint: None,
            network: None,
            rm: Some(false),
        };
        backend.run(&spec).await?;
    }

    Ok(())
}
