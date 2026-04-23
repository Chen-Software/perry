use crate::error::{Result, ComposeError};
use crate::backend::ContainerBackend;
use crate::types::ContainerSpec;
use crate::service;

pub async fn orchestrate_service(
    project_name: &str,
    svc_name: &str,
    spec: &crate::types::ComposeService,
    backend: &dyn ContainerBackend,
) -> Result<String> {
    let container_name = service::generate_name(project_name, svc_name, spec)?;

    // Idempotent flow:
    match backend.inspect(&container_name).await {
        Ok(info) => {
            if info.status.contains("running") || info.status.contains("Up") {
                // Already running, skip
                tracing::info!(service = %svc_name, "already running, skipping");
                return Ok(container_name);
            } else {
                // Exists but stopped, restart
                tracing::info!(service = %svc_name, "exists but stopped, starting");
                backend.start(&container_name).await?;
                return Ok(container_name);
            }
        }
        Err(_) => {
            // Does not exist, create fresh
        }
    }

    if service::needs_build(spec) {
        // build logic - placeholder for now
        tracing::info!(service = %svc_name, "building image (placeholder)");
    } else if let Some(image) = &spec.image {
        tracing::info!(service = %svc_name, "pulling image {}", image);
        backend.pull_image(image).await?;
    }

    let container_spec = ContainerSpec {
        image: spec.image.clone().unwrap_or_default(),
        name: Some(container_name.clone()),
        cmd: spec.command.as_ref().and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(vec![s.to_string()])
            } else if let Some(arr) = v.as_sequence() {
                Some(arr.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect())
            } else {
                None
            }
        }),
        ..Default::default()
    };

    tracing::info!(service = %svc_name, "creating and running");
    backend.run(&container_spec).await?;
    Ok(container_name)
}
