//! Service runtime state and name generation.

use crate::types::ComposeService;
use md5::{Digest, Md5};

/// Generate a stable container name for a service.
///
/// Format: `{md5_8chars}-{random_hex}`
pub fn generate_name(service_yaml: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(service_yaml.as_bytes());
    let hash = hasher.finalize();
    let short_hash = &hex::encode(hash)[..8];

    let random_suffix: u32 = rand::random();
    format!("{}-{:08x}", short_hash, random_suffix)
}

/// Compute a short hash of the service configuration.
pub fn service_config_hash(svc: &ComposeService) -> String {
    let service_yaml = serde_yaml::to_string(svc).unwrap_or_default();
    let mut hasher = Md5::new();
    hasher.update(service_yaml.as_bytes());
    hex::encode(hasher.finalize())[..8].to_string()
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
pub fn service_container_name(svc: &ComposeService, _service_name: &str) -> String {
    if let Some(explicit) = svc.explicit_name() {
        return explicit.to_string();
    }

    let service_yaml = serde_yaml::to_string(svc).unwrap_or_default();
    generate_name(&service_yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_name_format() {
        let name = generate_name("image: nginx");
        // Format: {md5_8chars}-{random_hex}
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 8);
    }

    #[test]
    fn test_explicit_name() {
        let mut svc = ComposeService::default();
        svc.container_name = Some("my-container".to_string());
        let name = service_container_name(&svc, "web");
        assert_eq!(name, "my-container");
    }
}
