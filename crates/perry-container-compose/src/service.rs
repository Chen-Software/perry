//! Service runtime state and name generation.

use crate::types::{ComposeService, ContainerSpec};
use md5::{Digest, Md5};

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

pub fn service_container_name(svc: &ComposeService, service_name: &str) -> String {
    if let Some(name) = svc.explicit_name() {
        name.to_string()
    } else {
        generate_name(&svc.image_ref(service_name), service_name)
    }
}

impl ComposeService {
    pub fn to_container_spec(&self, service_name: &str) -> ContainerSpec {
        ContainerSpec {
            image: self.image_ref(service_name),
            name: Some(service_container_name(self, service_name)),
            ports: Some(self.port_strings()),
            volumes: Some(self.volume_strings()),
            env: Some(self.resolved_env()),
            cmd: self.command_list(),
            entrypoint: None,
            network: None,
            rm: Some(true),
        }
    }
}
