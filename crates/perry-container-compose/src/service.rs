use crate::error::Result;
use crate::types::ComposeService;
use md5::{Md5, Digest};

pub struct ServiceState {
    pub container_id: Option<String>,
    pub name: String,
    pub running: bool,
}

pub fn generate_name(project: &str, service_name: &str, service: &ComposeService) -> Result<String> {
    if let Some(name) = &service.container_name {
        return Ok(name.clone());
    }

    // Stable name based on project and service name
    // We can also incorporate a hash of the config for uniqueness if multiple versions exist,
    // but typically compose just uses project_service_index.
    // Requirement 6.17 mentioned MD5 hash of service YAML.

    let yaml = serde_yaml::to_string(service).map_err(|e| crate::error::ComposeError::ParseError(e.to_string()))?;
    let mut hasher = Md5::new();
    hasher.update(project.as_bytes());
    hasher.update(service_name.as_bytes());
    hasher.update(yaml.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(&hash[..4]); // 8 chars

    // For MVP, we'll use a stable name that includes the hash so it's unique to the config
    Ok(format!("{}_{}_{}", project, service_name, hash_str))
}

pub fn needs_build(service: &ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}
