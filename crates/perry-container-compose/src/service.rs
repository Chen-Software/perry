use crate::error::Result;
use crate::types::ComposeService;
use md5::{Md5, Digest};
use rand::Rng;

pub fn generate_name(service_name: &str, service: &ComposeService) -> Result<String> {
    let yaml = serde_yaml::to_string(service)?;
    let mut hasher = Md5::new();
    hasher.update(yaml.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(&hash[..4]); // 8 chars

    let mut rng = rand::thread_rng();
    let random_suffix: u32 = rng.gen();
    let random_str = hex::encode(random_suffix.to_be_bytes());

    Ok(format!("{}-{}-{}", service_name, hash_str, random_str))
}

pub fn needs_build(service: &ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}
