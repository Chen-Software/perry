use crate::error::Result;
use crate::types::ComposeService;
use md5::{Md5, Digest};
use rand::Rng;

pub struct ServiceState {
    pub container_id: Option<String>,
    pub name: String,
    pub running: bool,
}

pub fn generate_name(service: &ComposeService) -> Result<String> {
    if let Some(name) = &service.container_name {
        return Ok(name.clone());
    }

    // Stable prefix based on service config
    let yaml = serde_yaml::to_string(service).map_err(|e| crate::error::ComposeError::ParseError(e.to_string()))?;
    let mut hasher = Md5::new();
    hasher.update(yaml.as_bytes());
    let hash = hasher.finalize();
    let prefix = hex::encode(&hash[..4]); // 8 chars

    // Random suffix
    let mut rng = rand::thread_rng();
    let suffix: u32 = rng.gen();

    Ok(format!("{}-{:08x}", prefix, suffix))
}

pub fn needs_build(service: &ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}
