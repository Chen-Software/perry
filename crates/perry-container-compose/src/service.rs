use crate::error::{ComposeError, Result};
use crate::types::ComposeService;
use md5::{Digest, Md5};

pub fn generate_name(service: &ComposeService, service_name: &str) -> Result<String> {
    if let Some(name) = service.container_name.as_ref() {
        return Ok(name.clone());
    }

    // Serialize the entire service config to YAML for a stable, config-based hash.
    let yaml = serde_yaml::to_string(service)
        .map_err(ComposeError::ParseError)?;

    let mut hasher = Md5::new();
    hasher.update(yaml.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);

    // Use the first 8 chars of the hash as a stable, human-readable suffix
    let short_hash = &hash_str[..8];

    // Random suffix for uniqueness across multiple instances of the same service config
    let random_suffix: u32 = rand::random();

    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    Ok(format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix))
}

/// Returns true if this service needs to be built before running.
pub fn needs_build(service: &ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
