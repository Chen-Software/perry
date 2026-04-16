use perry_container_compose::types::{ComposeSpec, DependsOnCondition, VolumeType};
use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::yaml::interpolate_yaml;
use proptest::prelude::*;
use std::collections::HashMap;

// Feature: perry-container, Property 1: ComposeSpec serialization round-trip
#[test]
fn test_compose_spec_json_round_trip() {
    let json = r#"{
        "services": {
            "web": {
                "image": "nginx",
                "ports": ["80:80"]
            }
        }
    }"#;
    let spec: ComposeSpec = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_string(&spec).unwrap();
    let deserialized: ComposeSpec = serde_json::from_str(&serialized).unwrap();
    assert_eq!(spec.services.len(), deserialized.services.len());
}

// Feature: perry-container, Property 3: Topological sort respects depends_on
#[test]
fn test_topological_sort_respects_deps() {
    let mut spec = ComposeSpec::default();

    let mut db = perry_container_compose::types::ComposeService::default();
    db.image = Some("postgres".into());
    spec.services.insert("db".into(), db);

    let mut web = perry_container_compose::types::ComposeService::default();
    web.image = Some("nginx".into());
    web.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(vec!["db".into()]));
    spec.services.insert("web".into(), web);

    let order = resolve_startup_order(&spec).unwrap();
    let db_idx = order.iter().position(|x| x == "db").unwrap();
    let web_idx = order.iter().position(|x| x == "web").unwrap();
    assert!(db_idx < web_idx);
}

// Feature: perry-container, Property 4: Cycle detection is complete
#[test]
fn test_cycle_detection() {
    let mut spec = ComposeSpec::default();

    let mut s1 = perry_container_compose::types::ComposeService::default();
    s1.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(vec!["s2".into()]));
    spec.services.insert("s1".into(), s1);

    let mut s2 = perry_container_compose::types::ComposeService::default();
    s2.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(vec!["s1".into()]));
    spec.services.insert("s2".into(), s2);

    let result = resolve_startup_order(&spec);
    assert!(result.is_err());
}

// Feature: perry-container, Property 6: Environment variable interpolation correctness
#[test]
fn test_env_interpolation() {
    let mut env = HashMap::new();
    env.insert("FOO".into(), "bar".into());

    let template = "value is ${FOO}";
    let result = interpolate_yaml(template, &env);
    assert_eq!(result, "value is bar");

    let template_default = "value is ${BAR:-default}";
    let result_default = interpolate_yaml(template_default, &env);
    assert_eq!(result_default, "value is default");
}

proptest! {
    #[test]
    fn prop_env_interpolation(s in r"[A-Z]+") {
        let mut env = HashMap::new();
        env.insert(s.clone(), "val".into());
        let template = format!("prefix_${{{}}}_suffix", s);
        let result = interpolate_yaml(&template, &env);
        prop_assert_eq!(result, format!("prefix_val_suffix"));
    }
}
