use crate::error::{ComposeError, Result};
use crate::backend::{detect_backend, ContainerBackend};
use std::sync::Arc;

pub struct BackendInstaller;

impl BackendInstaller {
    pub async fn run() -> Result<Arc<dyn ContainerBackend>> {
        // Implementation would use dialoguer::Select to show menu:
        // macOS: apple/container, orbstack, colima, podman, docker
        // Linux: podman, nerdctl, docker
        // Windows: podman, docker

        println!("Perry needs a container runtime to continue.");
        println!("No container runtime was found on this system.");

        // For now, we return error as if user declined or no TTY
        Err(ComposeError::NoBackendFound { probed: vec![] })
    }
}
