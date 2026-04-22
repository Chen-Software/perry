use crate::error::Result;
use crate::types::ComposeService;
use md5::{Digest, Md5};

/// Generate a unique, stable container name for a service.
/// Uses the MD5 hash of the full service YAML + a random suffix.
pub fn generate_name(service_name: &str, service: &ComposeService) -> Result<String> {
    if let Some(explicit) = &service.container_name {
        return Ok(explicit.clone());
    }

    // Serialize full service to YAML for hashing
    let yaml = serde_yaml::to_string(service).map_err(crate::error::ComposeError::ParseError)?;
    let mut hasher = Md5::new();
    hasher.update(yaml.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let short_hash = &hash[..8];

    // Random suffix for uniqueness across runs/projects
    let random_suffix: u32 = rand::random();

    let safe_service_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    Ok(format!("{}-{}-{:08x}", safe_service_name, short_hash, random_suffix))
}

/// Whether the service needs to build an image.
pub fn needs_build(service: &ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}

#[derive(Debug, Clone)]
pub struct ServiceState {
    pub id: Option<String>,
    pub name: String,
    pub running: bool,
}
