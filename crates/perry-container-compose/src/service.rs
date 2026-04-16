use md5::{Digest, Md5};
use crate::types::ComposeService;

pub fn generate_container_name(project_name: &str, service_name: &str, image: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hasher.finalize();
    let short_hash = &hex::encode(hash)[..8];

    let random_suffix: u32 = rand::random();

    let safe_service_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{}-{:08x}", project_name, safe_service_name, short_hash, random_suffix)
}

pub fn service_container_name(svc: &ComposeService, project_name: &str, service_name: &str) -> String {
    if let Some(name) = &svc.container_name {
        return name.clone();
    }
    generate_container_name(project_name, service_name, svc.image.as_deref().unwrap_or("none"))
}
