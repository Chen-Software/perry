//! Integration tests for perry-container-compose.

#[cfg(feature = "integration-tests")]
mod integration {
    use perry_container_compose::compose::resolve_startup_order;
    use perry_container_compose::types::{ComposeSpec};
    use perry_container_compose::yaml::{interpolate, parse_dotenv};
    use std::collections::HashMap;

    #[test]
    fn test_parse_simple_compose() {
        let yaml = r#"
services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"
"#;
        let spec = ComposeSpec::parse_str(yaml).expect("parse failed");
        assert!(spec.services.contains_key("web"));
        assert_eq!(spec.services["web"].image.as_deref(), Some("nginx:alpine"));
    }

    #[test]
    fn test_parse_multi_service_with_deps() {
        let yaml = r#"
services:
  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: secret
  web:
    image: myapp:latest
    depends_on:
      - db
    ports:
      - "3000:3000"
"#;
        let spec = ComposeSpec::parse_str(yaml).expect("parse failed");
        assert_eq!(spec.services.len(), 2);
        let web = &spec.services["web"];
        let deps = web.depends_on.as_ref().unwrap().service_names();
        assert!(deps.contains(&"db".to_string()));
    }

    #[test]
    fn test_topological_order_linear() {
        let yaml = r#"
services:
  c:
    image: c
    depends_on: [b]
  b:
    image: b
    depends_on: [a]
  a:
    image: a
"#;
        let spec = ComposeSpec::parse_str(yaml).unwrap();
        let order = resolve_startup_order(&spec).unwrap();
        let pos = |s: &str| order.iter().position(|n| n == s).unwrap();
        assert!(pos("a") < pos("b"), "a before b");
        assert!(pos("b") < pos("c"), "b before c");
    }

    #[test]
    fn test_circular_dependency_detected() {
        let yaml = r#"
services:
  a:
    image: a
    depends_on: [b]
  b:
    image: b
    depends_on: [a]
"#;
        let spec = ComposeSpec::parse_str(yaml).unwrap();
        let result = resolve_startup_order(&spec);
        assert!(result.is_err());
    }

    #[test]
    fn test_env_interpolation() {
        let mut env = HashMap::new();
        env.insert("DB_USER".to_string(), "admin".to_string());
        env.insert("DB_PASS".to_string(), "s3cr3t".to_string());

        let yaml = "  url: postgres://${DB_USER}:${DB_PASS}@localhost/db";
        let result = interpolate(yaml, &env);
        assert_eq!(result, "  url: postgres://admin:s3cr3t@localhost/db");
    }

    #[test]
    fn test_dotenv_parse() {
        let content = "HOST=localhost\nPORT=5432\n# ignored\n\nEMPTY=";
        let env = parse_dotenv(content);
        assert_eq!(env["HOST"], "localhost");
        assert_eq!(env["PORT"], "5432");
        assert_eq!(env["EMPTY"], "");
    }

    #[test]
    fn test_compose_merge_override() {
        let base_yaml = r#"
services:
  web:
    image: nginx:1.0
  db:
    image: postgres:15
"#;
        let override_yaml = r#"
services:
  web:
    image: nginx:2.0
"#;
        let mut base = ComposeSpec::parse_str(base_yaml).unwrap();
        let overlay = ComposeSpec::parse_str(override_yaml).unwrap();
        base.merge(overlay);

        assert_eq!(base.services["web"].image.as_deref(), Some("nginx:2.0"));
        assert!(base.services.contains_key("db"));
    }
}

#[cfg(feature = "integration-tests")]
mod live_integration {
    use perry_container_compose::backend::*;

    // Feature: perry-container | Layer: integration | Req: none | Property: -
    #[tokio::test]
    #[ignore]
    async fn test_real_backend_check() {
        let backend_res = detect_backend().await;
        if backend_res.is_err() { return; }
        let backend = backend_res.unwrap();
        backend.check_available().await.expect("backend check_available failed");
    }

    // Feature: perry-container | Layer: integration | Req: none | Property: -
    #[tokio::test]
    #[ignore]
    async fn test_real_backend_images() {
        let backend_res = detect_backend().await;
        if backend_res.is_err() { return; }
        let backend = backend_res.unwrap();
        let images = backend.list_images().await.expect("list_images failed");
        assert!(images.iter().any(|i| i.repository.contains("alpine")));
    }

    // Feature: perry-container | Layer: integration | Req: none | Property: -
    #[tokio::test]
    #[ignore]
    async fn test_real_backend_image_exists() {
        let backend_res = detect_backend().await;
        if backend_res.is_err() { return; }
        let backend = backend_res.unwrap();
        let exists = backend.image_exists("alpine:latest").await.expect("image_exists failed");
        assert!(exists);
        let not_exists = backend.image_exists("nonexistent:123").await.expect("image_exists failed");
        assert!(!not_exists);
    }

    // Feature: perry-container | Layer: integration | Req: none | Property: -
    #[tokio::test]
    #[ignore]
    async fn test_real_backend_networks() {
        let backend_res = detect_backend().await;
        if backend_res.is_err() { return; }
        let backend = backend_res.unwrap();
        let name = format!("perry-net-{}", rand::random::<u32>());
        backend.create_network(&name, &NetworkConfig::default()).await.expect("create_network failed");
        backend.remove_network(&name).await.expect("remove_network failed");
    }

    // Feature: perry-container | Layer: integration | Req: none | Property: -
    #[tokio::test]
    #[ignore]
    async fn test_real_backend_volumes() {
        let backend_res = detect_backend().await;
        if backend_res.is_err() { return; }
        let backend = backend_res.unwrap();
        let name = format!("perry-vol-{}", rand::random::<u32>());
        backend.create_volume(&name, &VolumeConfig::default()).await.expect("create_volume failed");
        backend.remove_volume(&name).await.expect("remove_volume failed");
    }
}

/*
Coverage Table:
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 6.4         | test_topological_order_linear | integration |
| 6.5         | test_circular_dependency_detected | integration |
| 7.1         | test_parse_simple_compose | integration |
| 7.8         | test_env_interpolation | integration |
| 7.10        | test_compose_merge_override | integration |
| none        | test_real_backend_check | integration |
| none        | test_real_backend_images | integration |
| none        | test_real_backend_image_exists | integration |
| none        | test_real_backend_networks | integration |
| none        | test_real_backend_volumes | integration |

Deferred Requirements:
- Req 6.8, 6.9, 12.5: Container/Compose execution (up/run) deferred due to overlay mount failure in sandbox environment.
*/
