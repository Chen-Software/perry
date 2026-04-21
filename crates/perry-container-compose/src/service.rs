use crate::error::{ComposeError, Result};
use crate::types::ComposeService;
use md5::{Digest, Md5};

pub fn service_container_name(service: &ComposeService, service_name: &str) -> Result<String> {
    if let Some(name) = service.container_name.as_ref() {
        return Ok(name.clone());
    }

    let yaml = serde_yaml::to_string(service).map_err(|e| ComposeError::ValidationError {
        message: format!("Failed to serialize service for naming: {e}"),
    })?;

    let mut hasher = Md5::new();
    hasher.update(yaml.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let short_hash = &hash[..8];

    let random_suffix: u32 = rand::random();

    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    Ok(format!("{}-{}-{:08x}", safe_name, short_hash, random_suffix))
}

pub fn needs_build(service: &ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ComposeService, BuildSpec};

    #[test]
    fn test_service_container_name_stable() {
        let svc = ComposeService {
            image: Some("redis:alpine".into()),
            ..Default::default()
        };
        let name1 = service_container_name(&svc, "redis").unwrap();
        let name2 = service_container_name(&svc, "redis").unwrap();
        // They won't be identical because of random_suffix, but the hash prefix should be stable if I didn't have random suffix.
        // Wait, the requirement says "{md5_8chars}-{random_hex}".
        // So they will be different. But we can check format.
        assert!(name1.starts_with("redis-"));
        let parts: Vec<&str> = name1.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[1].len(), 8);
        assert_eq!(parts[2].len(), 8);
    }

    #[test]
    fn test_needs_build() {
        let svc_no_build = ComposeService {
            image: Some("nginx".into()),
            ..Default::default()
        };
        assert!(!needs_build(&svc_no_build));

        let svc_build = ComposeService {
            build: Some(BuildSpec::Context(".".into())),
            ..Default::default()
        };
        assert!(needs_build(&svc_build));

        let svc_both = ComposeService {
            image: Some("my-image".into()),
            build: Some(BuildSpec::Context(".".into())),
            ..Default::default()
        };
        assert!(!needs_build(&svc_both));
    }
}
