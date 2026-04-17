use crate::types::ComposeService;
use crate::error::{ComposeError, Result};
use md5::{Digest, Md5};

pub fn generate_name(service: &ComposeService) -> Result<String> {
    let yaml = serde_yaml::to_string(service)
        .map_err(ComposeError::ParseError)?;

    let mut hasher = Md5::new();
    hasher.update(yaml.as_bytes());
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);

    let short_hash = &hash_str[..8];
    let random_suffix: u32 = rand::random();
    Ok(format!("{}-{:08x}", short_hash, random_suffix))
}

pub fn needs_build(service: &ComposeService) -> bool {
    service.build.is_some() && service.image.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ComposeService;

    #[test]
    fn test_name_generation_stable() {
        let mut svc = ComposeService::default();
        svc.image = Some("nginx".to_string());

        let name1 = generate_name(&svc).unwrap();
        let name2 = generate_name(&svc).unwrap();

        // Format: {md5_8chars}-{random_hex}
        assert_eq!(name1.len(), 17); // 8 + 1 + 8
        assert_eq!(&name1[..9], &name2[..9]);
        assert_ne!(name1, name2); // Full name should be unique due to random suffix
    }

    #[test]
    fn test_name_generation_changes_with_config() {
        let mut svc = ComposeService::default();
        svc.image = Some("nginx".to_string());
        let name1 = generate_name(&svc).unwrap();

        svc.image = Some("redis".to_string());
        let name2 = generate_name(&svc).unwrap();

        assert_ne!(&name1[..8], &name2[..8]);
    }

    #[test]
    fn test_needs_build() {
        let mut svc = ComposeService::default();
        assert_eq!(needs_build(&svc), false);

        svc.image = Some("nginx".to_string());
        assert_eq!(needs_build(&svc), false);

        svc.image = None;
        svc.build = Some(crate::types::BuildSpec::Context(".".to_string()));
        assert_eq!(needs_build(&svc), true);

        svc.image = Some("nginx".to_string());
        assert_eq!(needs_build(&svc), false);
    }
}
