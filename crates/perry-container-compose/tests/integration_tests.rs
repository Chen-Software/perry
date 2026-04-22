use std::collections::HashMap;
use std::path::PathBuf;
use perry_container_compose::yaml::{interpolate_yaml, parse_dotenv, parse_compose_yaml};
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::types::{ComposeService, ComposeSpec, DependsOnSpec};

#[test]
fn test_dotenv_parse() {
    let content = "VAR1=val1\n# comment\nVAR2=val2";
    let env = parse_dotenv(content);
    assert_eq!(env.get("VAR1").unwrap(), "val1");
    assert_eq!(env.get("VAR2").unwrap(), "val2");
}

#[test]
fn test_env_interpolation() {
    let mut env = HashMap::new();
    env.insert("NAME".into(), "perry".into());
    let yaml = "image: ${NAME}-image";
    let interpolated = interpolate_yaml(yaml, &env);
    assert_eq!(interpolated, "image: perry-image");
}

#[test]
fn test_topological_order_linear() {
    let mut spec = ComposeSpec::default();

    let mut s1 = ComposeService::default();
    s1.image = Some("img1".into());

    let mut s2 = ComposeService::default();
    s2.image = Some("img2".into());
    s2.depends_on = Some(DependsOnSpec::List(vec!["s1".into()]));

    spec.services.insert("s1".into(), s1);
    spec.services.insert("s2".into(), s2);

    let order = ComposeEngine::resolve_startup_order(&spec).unwrap();
    assert_eq!(order, vec!["s1", "s2"]);
}

#[test]
fn test_circular_dependency_detected() {
    let mut spec = ComposeSpec::default();

    let mut s1 = ComposeService::default();
    s1.depends_on = Some(DependsOnSpec::List(vec!["s2".into()]));

    let mut s2 = ComposeService::default();
    s2.depends_on = Some(DependsOnSpec::List(vec!["s1".into()]));

    spec.services.insert("s1".into(), s1);
    spec.services.insert("s2".into(), s2);

    let res = ComposeEngine::resolve_startup_order(&spec);
    assert!(res.is_err());
}

#[test]
fn test_compose_merge_override() {
    let mut s1 = ComposeSpec::default();
    let mut svc1 = ComposeService::default();
    svc1.image = Some("v1".into());
    s1.services.insert("web".into(), svc1);

    let mut s2 = ComposeSpec::default();
    let mut svc2 = ComposeService::default();
    svc2.image = Some("v2".into());
    s2.services.insert("web".into(), svc2);

    s1.merge(s2);
    assert_eq!(s1.services.get("web").unwrap().image.as_ref().unwrap(), "v2");
}

#[test]
fn test_parse_simple_compose() {
    let yaml = r#"
services:
  web:
    image: nginx
    ports:
      - "80:80"
"#;
    let env = HashMap::new();
    let spec = parse_compose_yaml(yaml, &env).unwrap();
    assert!(spec.services.contains_key("web"));
    assert_eq!(spec.services.get("web").unwrap().image.as_ref().unwrap(), "nginx");
}

#[test]
fn test_parse_multi_service_with_deps() {
    let yaml = r#"
services:
  db:
    image: postgres
  api:
    image: node
    depends_on:
      - db
"#;
    let env = HashMap::new();
    let spec = parse_compose_yaml(yaml, &env).unwrap();
    assert_eq!(spec.services.len(), 2);
    let deps = spec.services.get("api").unwrap().depends_on.as_ref().unwrap();
    assert_eq!(deps.service_names(), vec!["db"]);
}
