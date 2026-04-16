//! Service runtime state and name generation.

use md5::{Digest, Md5};

/// Generate a unique container name using MD5 hash of image + random suffix.
/// Format: {service_name}-{md5_8chars}-{random_hex}
pub fn generate_name(image: &str, service_name: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);
    let short_hash = &hash_str[..8];

    let random_suffix: u32 = rand::random();

    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
}

/// Service runtime state tracking.
pub struct ServiceState {
    pub container_id: Option<String>,
    pub name: String,
    pub running: bool,
}
