use crate::error::Result;
use md5::{Digest, Md5};

pub fn service_container_name(service: &crate::types::ComposeService, service_name: &str) -> String {
    if let Some(name) = service.container_name.as_ref() {
        return name.clone();
    }

    let service_yaml = serde_yaml::to_string(service).unwrap_or_default();
    let mut hasher = Md5::new();
    hasher.update(service_yaml.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let short_hash = &hash[..8];

    let random_suffix: u32 = rand::random();

    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
}

pub fn needs_build(service: &crate::types::ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
