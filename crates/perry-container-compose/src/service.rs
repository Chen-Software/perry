use md5::{Digest, Md5};

pub fn service_container_name(service: &crate::types::ComposeService, _service_name: &str) -> String {
    if let Some(name) = service.container_name.as_ref() {
        return name.clone();
    }
    let image = service.image.as_deref().unwrap_or("unknown");
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let short_hash = &hash[..8];
    let random_suffix: u32 = rand::random();
    format!("{}-{:08x}", short_hash, random_suffix)
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}
