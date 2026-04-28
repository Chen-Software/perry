use md5::{Digest, Md5};

pub fn generate_name(project_name: &str, service_name: &str, service: &crate::types::ComposeService) -> String {
    if let Some(name) = service.container_name.as_ref() {
        return name.clone();
    }

    let image = service.image.as_deref().unwrap_or("unknown");
    let mut hasher = Md5::new();
    hasher.update(image.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let short_image_hash = &hash[..8];

    // Deterministic suffix based on project and service name
    let mut hasher = Md5::new();
    hasher.update(project_name.as_bytes());
    hasher.update(service_name.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let short_project_hash = &hash[..8];

    let safe_name: String = service_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    format!("{}-{}-{}", safe_name, short_image_hash, short_project_hash)
}

pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ComposeService;

    #[test]
    fn test_generate_name_format() {
        let svc = ComposeService {
            image: Some("redis:7".to_string()),
            ..Default::default()
        };
        let name = generate_name("test-proj", "cache", &svc);

        // Format: {service_name}-{image_hash_8}-{project_service_hash_8}
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "cache");
        assert_eq!(parts[1].len(), 8);
        assert_eq!(parts[2].len(), 8);
    }

    #[test]
    fn test_generate_name_stability() {
        let svc = ComposeService {
            image: Some("postgres:16".to_string()),
            ..Default::default()
        };

        let n1 = generate_name("test-proj", "db", &svc);
        let n2 = generate_name("test-proj", "db", &svc);

        assert_eq!(n1, n2);

        let n3 = generate_name("other-proj", "db", &svc);
        assert_ne!(n1, n3);
    }

    #[test]
    fn test_generate_name_override() {
        let svc = ComposeService {
            container_name: Some("my-custom-name".to_string()),
            ..Default::default()
        };
        let name = generate_name("proj", "ignored", &svc);
        assert_eq!(name, "my-custom-name");
    }
}
