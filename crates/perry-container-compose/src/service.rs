use crate::error::ComposeError;
use crate::types::ComposeService;
use md5::{Digest, Md5};

pub fn service_container_name(svc: &ComposeService, service_name: &str) -> String {
    if let Some(name) = &svc.container_name {
        return name.clone();
    }

    // Stable hash of the service config (simplified here to image + name)
    let config_str = format!("{:?}-{:?}", svc.image, service_name);
    let mut hasher = Md5::new();
    hasher.update(config_str.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let short_hash = &hash[..8];
    let random_suffix: u32 = rand::random();

    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
