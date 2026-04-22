use md5::{Digest, Md5};
use rand::Rng;

pub struct Service;

impl Service {
    pub fn generate_name(image: &str, service_name: &str) -> String {
        // MD5 hash of the image name for a stable prefix
        let mut hasher = Md5::new();
        hasher.update(image.as_bytes());
        let hash = hasher.finalize();
        let hash_str = hex::encode(hash);
        let short_hash = &hash_str[..8];

        // Random suffix for uniqueness across multiple instances of the same image
        let random_suffix: u32 = rand::thread_rng().gen();

        // Sanitize service name: replace non-alphanumeric (except hyphen) with underscore
        let safe_name: String = service_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
            .collect();

        format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix)
    }
}
