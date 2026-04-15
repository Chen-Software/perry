//! Integration tests for perry-container-compose

use perry_container_compose::entities::compose::Compose;
use perry_container_compose::entities::service::Service;
use perry_container_compose::orchestrate::deps::topological_order;
use perry_container_compose::orchestrate::env::{interpolate, parse_dotenv};

// ============ YAML Parsing Tests ============

#[test]
fn test_parse_simple_compose() {
    let yaml = r#"
version: "3.8"
services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"
    labels:
      app: nginx
"#;
    let compose = Compose::parse_str(yaml).expect("parse failed");
    assert!(compose.services.contains_key("web"));
    let web = &compose.services["web"];
    assert_eq!(web.image.as_deref(), Some("nginx:alpine"));
    assert_eq!(web.ports.as_ref().unwrap().len(), 1);
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
    let compose = Compose::parse_str(yaml).expect("parse failed");
    assert_eq!(compose.services.len(), 2);
    let web = &compose.services["web"];
    let deps = web.depends_on.as_ref().unwrap().service_names();
    assert!(deps.contains(&"db".to_string()));
}

#[test]
fn test_parse_build_config() {
    let yaml = r#"
services:
  app:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        BUILD_ENV: production
    ports:
      - "8080:8080"
"#;
    let compose = Compose::parse_str(yaml).expect("parse failed");
    let app = &compose.services["app"];
    let build = app.build.as_ref().expect("no build config");
    assert_eq!(build.context.as_deref(), Some("."));
    assert_eq!(build.dockerfile.as_deref(), Some("Dockerfile"));
}

#[test]
fn test_parse_environment_list() {
    let yaml = r#"
services:
  web:
    image: nginx
    environment:
      - FOO=bar
      - BAZ=qux
"#;
    let compose = Compose::parse_str(yaml).expect("parse failed");
    let env = compose.services["web"].resolved_env();
    assert_eq!(env.get("FOO").map(String::as_str), Some("bar"));
    assert_eq!(env.get("BAZ").map(String::as_str), Some("qux"));
}

#[test]
fn test_parse_environment_map() {
    let yaml = r#"
services:
  web:
    image: nginx
    environment:
      FOO: bar
      BAZ: qux
"#;
    let compose = Compose::parse_str(yaml).expect("parse failed");
    let env = compose.services["web"].resolved_env();
    assert_eq!(env.get("FOO").map(String::as_str), Some("bar"));
}

#[test]
fn test_invalid_yaml_returns_error() {
    let result = Compose::parse_str("not: valid: yaml: [");
    assert!(result.is_err());
}

// ============ Name Generation Tests ============

#[test]
fn test_generate_name_with_explicit_name() {
    let mut svc = Service::default();
    svc.name = Some("my-container".to_string());
    let name = svc.generate_name("web").unwrap();
    assert_eq!(name, "my-container");
}

#[test]
fn test_generate_name_from_image() {
    let mut svc = Service::default();
    svc.image = Some("nginx:alpine".to_string());
    let name = svc.generate_name("web").unwrap();
    assert!(name.starts_with("web_"));
    assert_eq!(name.len(), "web_".len() + 8); // 8 hex chars
}

#[test]
fn test_generate_name_deterministic() {
    let mut svc = Service::default();
    svc.image = Some("nginx:alpine".to_string());
    let name1 = svc.generate_name("web").unwrap();
    let name2 = svc.generate_name("web").unwrap();
    assert_eq!(name1, name2, "name generation must be deterministic");
}

// ============ Dependency Resolution Tests ============

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
    let compose = Compose::parse_str(yaml).unwrap();
    let order = topological_order(&compose).unwrap();
    let pos = |s: &str| order.iter().position(|n| n == s).unwrap();
    assert!(pos("a") < pos("b"), "a before b");
    assert!(pos("b") < pos("c"), "b before c");
}

#[test]
fn test_topological_order_diamond() {
    let yaml = r#"
services:
  a:
    image: a
  b:
    image: b
    depends_on: [a]
  c:
    image: c
    depends_on: [a]
  d:
    image: d
    depends_on: [b, c]
"#;
    let compose = Compose::parse_str(yaml).unwrap();
    let order = topological_order(&compose).unwrap();
    let pos = |s: &str| order.iter().position(|n| n == s).unwrap();
    assert!(pos("a") < pos("b"));
    assert!(pos("a") < pos("c"));
    assert!(pos("b") < pos("d"));
    assert!(pos("c") < pos("d"));
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
    let compose = Compose::parse_str(yaml).unwrap();
    let result = topological_order(&compose);
    assert!(result.is_err());
}

#[test]
fn test_missing_dependency_detected() {
    let yaml = r#"
services:
  web:
    image: nginx
    depends_on: [missing-service]
"#;
    let compose = Compose::parse_str(yaml).unwrap();
    let result = topological_order(&compose);
    assert!(result.is_err());
}

// ============ Environment Interpolation Tests ============

#[test]
fn test_dotenv_parse_basic() {
    let content = "HOST=localhost\nPORT=5432\n# ignored\n\nEMPTY=";
    let env = parse_dotenv(content);
    assert_eq!(env["HOST"], "localhost");
    assert_eq!(env["PORT"], "5432");
    assert_eq!(env["EMPTY"], "");
}

#[test]
fn test_interpolate_in_yaml() {
    use std::collections::HashMap;
    let mut env = HashMap::new();
    env.insert("DB_USER".to_string(), "admin".to_string());
    env.insert("DB_PASS".to_string(), "s3cr3t".to_string());

    let yaml = "  url: postgres://${DB_USER}:${DB_PASS}@localhost/db";
    let result = interpolate(yaml, &env);
    assert_eq!(result, "  url: postgres://admin:s3cr3t@localhost/db");
}

#[test]
fn test_interpolate_default_value() {
    let env = std::collections::HashMap::new();
    let result = interpolate("${MISSING:-fallback}", &env);
    assert_eq!(result, "fallback");
}

// ============ Compose Merging Tests ============

#[test]
fn test_compose_merge_override() {
    let base_yaml = r#"
services:
  web:
    image: nginx:1.0
    ports: ["80:80"]
  db:
    image: postgres:15
"#;
    let override_yaml = r#"
services:
  web:
    image: nginx:2.0
"#;
    let mut base = Compose::parse_str(base_yaml).unwrap();
    let overlay = Compose::parse_str(override_yaml).unwrap();
    base.merge(overlay);

    assert_eq!(base.services["web"].image.as_deref(), Some("nginx:2.0"));
    // db should still be present
    assert!(base.services.contains_key("db"));
}

// ============ Needs Build Tests ============

#[test]
fn test_needs_build_true() {
    let mut svc = Service::default();
    svc.build = Some(perry_container_compose::entities::service::Build {
        context: Some(".".to_string()),
        ..Default::default()
    });
    assert!(svc.needs_build());
}

#[test]
fn test_needs_build_false_has_image() {
    let mut svc = Service::default();
    svc.image = Some("nginx".to_string());
    svc.build = Some(perry_container_compose::entities::service::Build {
        context: Some(".".to_string()),
        ..Default::default()
    });
    assert!(!svc.needs_build()); // has explicit image, no build needed
}
