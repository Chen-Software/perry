use crate::container::types::*;
use crate::container::context::ContainerContext;
use perry_container_compose::error::ComposeError;

pub async fn alloy_container_run_capability(
    _name: &str,
    image: &str,
    cmd: &[&str],
    _grants: &HashMap<String, bool>,
) -> Result<ContainerLogs, ComposeError> {
    crate::container::verification::verify_image(image).await?;

    let backend = ContainerContext::global().get_backend().await?;
    let spec = ContainerSpec {
        image: image.to_string(),
        cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
        rm: Some(true),
        ..Default::default()
    };

    let handle = backend.run(&spec).await?;
    backend.logs(&handle.id, None).await
}

use std::collections::HashMap;
