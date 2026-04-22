//! Service runtime state and name generation.

use crate::types::ComposeService;
use md5::{Digest, Md5};

/// Generate a unique container name for a service based on its configuration.
///
/// Format: `{service_name}-{md5_prefix_8}-{random_hex_8}`
/// e.g. `web-a1b2c3d4-f0e1d2c3`
pub fn generate_name(svc: &ComposeService, service_name: &str) -> String {
    // MD5 hash of the full service JSON for a stable prefix that changes with config
    let mut hasher = Md5::new();
    let svc_json = serde_json::to_string(svc).unwrap_or_default();
    hasher.update(svc_json.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);
    let short_hash = &hash_str[..8];

    // Random suffix for uniqueness across multiple instances of the same service
    let random_suffix: u32 = rand::random();

    // Sanitize service name: replace non-alphanumeric (except hyphen) with underscore
    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
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

    /// Check if the container exists on the backend.
    pub async fn exists(&self, backend: &dyn crate::backend::ContainerBackend) -> bool {
        backend.inspect(&self.container_name).await.is_ok()
    }

    /// Check if the container is running on the backend.
    pub async fn is_running(&self, backend: &dyn crate::backend::ContainerBackend) -> bool {
        match backend.inspect(&self.container_name).await {
            Ok(info) => info.status.to_lowercase().contains("running") || info.status.to_lowercase().contains("up"),
            Err(_) => false,
        }
    }
}

/// Generate a container name for a service, using explicit name if set.
pub fn service_container_name(svc: &ComposeService, service_name: &str) -> String {
    if let Some(explicit) = svc.explicit_name() {
        return explicit.to_string();
    }

    generate_name(svc, service_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_name_format() {
        let svc = ComposeService { image: Some("nginx:latest".into()), ..Default::default() };
        let name = generate_name(&svc, "web");
        // Format: {safe_name}-{hash_8}-{random_8}
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts[0], "web");
        assert_eq!(parts[1].len(), 8);
        assert_eq!(parts[2].len(), 8);
    }

    #[test]
    fn test_same_config_same_hash_prefix() {
        let svc = ComposeService { image: Some("nginx:latest".into()), ..Default::default() };
        let name1 = generate_name(&svc, "web");
        let name2 = generate_name(&svc, "api");
        // Same config → same hash prefix
        let hash1 = &name1[name1.find('-').unwrap() + 1..name1.find('-').unwrap() + 9];
        let hash2 = &name2[name2.find('-').unwrap() + 1..name2.find('-').unwrap() + 9];
        assert_eq!(hash1, hash2, "same config must produce same hash prefix");
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
        let svc = ComposeService::default();
        let name = generate_name(&svc, "my.service");
        assert!(name.starts_with("my_service-"), "dots should be replaced");
    }
}
