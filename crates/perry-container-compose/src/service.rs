use md5::{Digest, Md5};
use rand::Rng;
use crate::error::Result;
use crate::types::ContainerInfo;
use crate::backend::ContainerBackend;

pub struct ServiceState {
    pub container_id: Option<String>,
    pub name: String,
    pub is_running: bool,
}

pub fn generate_name(image: &str, service_name: &str) -> String {
    // MD5 hash of the image name for a stable prefix
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);
    let short_hash = &hash_str[..8];

    // Random suffix for uniqueness across multiple instances of the same image
    let mut rng = rand::thread_rng();
    let random_suffix: u32 = rng.gen();

    // Sanitize service name: replace non-alphanumeric (except hyphen) with underscore
    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
}

pub async fn exists(backend: &dyn ContainerBackend, name: &str) -> Result<bool> {
    let containers = backend.list(true).await?;
    Ok(containers.iter().any(|c| c.name == name))
}

pub async fn is_running(backend: &dyn ContainerBackend, name: &str) -> Result<bool> {
    let containers = backend.list(false).await?;
    Ok(containers.iter().any(|c| c.name == name))
}

pub async fn get_container(backend: &dyn ContainerBackend, name: &str) -> Result<Option<ContainerInfo>> {
    let containers = backend.list(true).await?;
    Ok(containers.into_iter().find(|c| c.name == name))
}
