// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -

#[cfg(test)]
mod tests {
    use perry_container_compose::service::service_container_name;
    use perry_container_compose::types::ComposeService;

    #[test]
    fn test_service_container_name_deterministic_prefix() {
        let svc = ComposeService {
            image: Some("nginx:latest".to_string()),
            ..Default::default()
        };
        let name1 = service_container_name(&svc, "web");
        let name2 = service_container_name(&svc, "web");

        // Format is {service}-{md5_prefix}-{random_hex}
        let parts1: Vec<&str> = name1.split('-').collect();
        let parts2: Vec<&str> = name2.split('-').collect();

        assert_eq!(parts1[0], "web");
        assert_eq!(parts1[1], parts2[1], "MD5 prefix must be deterministic for same image");
        assert_ne!(parts1[2], parts2[2], "Suffix must be random");
    }

    #[test]
    fn test_service_container_name_explicit() {
        let svc = ComposeService {
            container_name: Some("my-db".to_string()),
            ..Default::default()
        };
        let name = service_container_name(&svc, "database");
        assert_eq!(name, "my-db");
    }

    #[test]
    fn test_service_container_name_sanitization() {
        let svc = ComposeService {
            image: Some("alpine".to_string()),
            ..Default::default()
        };
        // Non-alphanumeric (except hyphen) replaced with underscore
        let name = service_container_name(&svc, "web.app_1");
        assert!(name.starts_with("web_app_1-"));
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 6.13        | test_service_container_name_deterministic_prefix | unit |
| 6.13        | test_service_container_name_explicit | unit |
| 6.13        | test_service_container_name_sanitization | unit |
*/

// Deferred Requirements: none
