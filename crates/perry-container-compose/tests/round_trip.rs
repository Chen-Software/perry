use proptest::prelude::*;
use perry_container_compose::types::*;
use perry_container_compose::compose::resolve_startup_order;
use indexmap::IndexMap;

// Helper to generate a valid ComposeSpec
fn arb_compose_spec() -> impl Strategy<Value = ComposeSpec> {
    prop::collection::vec(arb_service(), 1..5).prop_map(|services| {
        let mut spec = ComposeSpec::default();
        let mut services_map = IndexMap::new();
        for (i, svc) in services.into_iter().enumerate() {
            services_map.insert(format!("svc-{}", i), svc);
        }
        spec.services = services_map;
        spec
    })
}

fn arb_service() -> impl Strategy<Value = ComposeService> {
    prop_oneof![
        Just(ComposeService {
            image: Some("nginx:latest".into()),
            ..Default::default()
        }),
        Just(ComposeService {
            image: Some("redis:alpine".into()),
            ..Default::default()
        }),
    ]
}

proptest! {
    #[test]
    fn test_compose_spec_serialization_roundtrip(spec in arb_compose_spec()) {
        let yaml = spec.to_yaml().expect("Failed to serialize to YAML");
        let parsed = ComposeSpec::parse_str(&yaml).expect("Failed to parse YAML back");
        assert_eq!(spec.services.len(), parsed.services.len());
    }

    #[test]
    fn test_resolve_startup_order_no_deps(spec in arb_compose_spec()) {
        let order = resolve_startup_order(&spec).expect("Should resolve order");
        assert_eq!(order.len(), spec.services.len());
    }
}

#[test]
fn test_dependency_cycle() {
    let mut spec = ComposeSpec::default();
    let mut services = IndexMap::new();

    let mut s1 = ComposeService::default();
    s1.depends_on = Some(DependsOnSpec::List(vec!["svc2".into()]));

    let mut s2 = ComposeService::default();
    s2.depends_on = Some(DependsOnSpec::List(vec!["svc1".into()]));

    services.insert("svc1".to_string(), s1);
    services.insert("svc2".to_string(), s2);
    spec.services = services;

    let result = resolve_startup_order(&spec);
    assert!(result.is_err());
}

#[test]
fn test_complex_dependency_cycle() {
    let mut spec = ComposeSpec::default();
    let mut services = IndexMap::new();

    // 1 -> 2 -> 3 -> 1
    // 4 -> 2

    let mut s1 = ComposeService::default();
    s1.depends_on = Some(DependsOnSpec::List(vec!["svc2".into()]));

    let mut s2 = ComposeService::default();
    s2.depends_on = Some(DependsOnSpec::List(vec!["svc3".into()]));

    let mut s3 = ComposeService::default();
    s3.depends_on = Some(DependsOnSpec::List(vec!["svc1".into()]));

    let mut s4 = ComposeService::default();
    s4.depends_on = Some(DependsOnSpec::List(vec!["svc2".into()]));

    services.insert("svc1".to_string(), s1);
    services.insert("svc2".to_string(), s2);
    services.insert("svc3".to_string(), s3);
    services.insert("svc4".to_string(), s4);
    spec.services = services;

    let result = resolve_startup_order(&spec);
    match result {
        Err(perry_container_compose::error::ComposeError::DependencyCycle { services }) => {
            assert!(services.contains(&"svc1".to_string()));
            assert!(services.contains(&"svc2".to_string()));
            assert!(services.contains(&"svc3".to_string()));
            assert!(services.contains(&"svc4".to_string()));
        }
        _ => panic!("Expected DependencyCycle error"),
    }
}
