//! Service runtime state and name generation.

use crate::backend::ContainerBackend;
use crate::error::Result;
use crate::types::{ComposeService, ContainerHandle, ContainerInfo, ComposeServiceBuild};
use md5::{Digest, Md5};
use std::collections::HashMap;

pub struct Service {
    pub image: Option<String>,
    pub name: Option<String>, // container_name in YAML
    pub ports: Option<Vec<String>>,
    pub environment: Option<HashMap<String, String>>,
    pub labels: Option<HashMap<String, String>>,
    pub volumes: Option<Vec<String>>,
    pub build: Option<ComposeServiceBuild>,
}

impl Service {
    pub fn from_compose(name: &str, svc: &ComposeService) -> Self {
        Self {
            image: svc.image.clone(),
            name: svc.container_name.clone().or_else(|| Some(name.to_string())),
            ports: Some(svc.port_strings()),
            environment: Some(svc.resolved_env()),
            labels: Some(svc.labels.as_ref().map(|l| l.to_map()).unwrap_or_default()),
            volumes: Some(svc.volume_strings()),
            build: svc.build.as_ref().map(|b| b.as_build()),
        }
    }

    pub async fn exists(&self, backend: &dyn ContainerBackend) -> bool {
        if let Some(name) = &self.name {
            backend.inspect(name).await.is_ok()
        } else {
            false
        }
    }

    pub async fn is_running(&self, backend: &dyn ContainerBackend) -> bool {
        if let Some(name) = &self.name {
            if let Ok(info) = backend.inspect(name).await {
                return info.status == "running";
            }
        }
        false
    }

    pub fn needs_build(&self) -> bool {
        self.build.is_some() && self.image.is_none()
    }

    pub fn generate_name(&self) -> String {
        let image = self.image.as_deref().unwrap_or("unknown");
        generate_name(image, self.name.as_deref().unwrap_or("service"))
    }
}

/// Generate a unique container name for a service.
///
/// Format: `{safe_name}_{short_hash}{random_suffix_hex}`
/// e.g. `web_a1b2c3d4f0e1d2c3`
pub fn generate_name(image: &str, service_name: &str) -> String {
    // MD5 hash of the image name for a stable prefix
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);
    let short_hash = &hash_str[..8];

    // Random suffix for uniqueness across multiple instances of the same image
    let random_suffix: u32 = rand::random();

    // Sanitize service name: replace non-alphanumeric (except hyphen) with underscore
    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}_{}{:08x}", safe_name, short_hash, random_suffix)
}

/// Service runtime state tracking.
pub struct ServiceState {
    /// Container ID
    pub container_id: String,
    /// Container name
    pub container_name: String,
    /// Whether the service container is running
    pub running: bool,
}

impl ServiceState {
    /// Create a service state from an explicit container name.
    pub fn new(container_id: String, container_name: String, running: bool) -> Self {
        ServiceState {
            container_id,
            container_name,
            running,
        }
    }
}

/// Generate a container name for a service, using explicit name if set.
pub fn service_container_name(svc: &ComposeService, service_name: &str) -> String {
    if let Some(explicit) = svc.explicit_name() {
        return explicit.to_string();
    }

    let image = svc.image.as_deref().unwrap_or(service_name);
    generate_name(image, service_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_name_format() {
        let name = generate_name("nginx:latest", "web");
        // Format: {safe_name}_{short_hash}{random_suffix_hex}
        let parts: Vec<&str> = name.split('_').collect();
        assert_eq!(parts[0], "web");
        assert_eq!(parts[1].len(), 16); // 8 hash + 8 random
    }

    #[test]
    fn test_same_image_same_hash_prefix() {
        let name1 = generate_name("nginx:latest", "web");
        let name2 = generate_name("nginx:latest", "api");
        // Same image → same hash prefix
        let hash1 = &name1[name1.find('_').unwrap() + 1..name1.find('_').unwrap() + 9];
        let hash2 = &name2[name2.find('_').unwrap() + 1..name2.find('_').unwrap() + 9];
        assert_eq!(hash1, hash2, "same image must produce same hash prefix");
    }

    #[test]
    fn test_explicit_name() {
        let mut svc = ComposeService::default();
        svc.container_name = Some("my-container".to_string());
        let name = service_container_name(&svc, "web");
        assert_eq!(name, "my-container");
    }

    #[test]
    fn test_sanitize_service_name() {
        let name = generate_name("img", "my.service");
        assert!(name.starts_with("my_service_"), "dots should be replaced");
    }
}
