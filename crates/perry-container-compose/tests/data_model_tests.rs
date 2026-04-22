use perry_container_compose::types::*;
use proptest::prelude::*;

// Feature: alloy-container, Property 4: Data model JSON round-trip
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn test_container_spec_roundtrip(image in ".*", name in proptest::option::of(".*"), rm in proptest::option::of(any::<bool>())) {
        let spec = ContainerSpec {
            image,
            name,
            rm,
            ..Default::default()
        };
        let json = serde_json::to_string(&spec).unwrap();
        let decoded: ContainerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, decoded);
    }
}

// Feature: alloy-container, Property 5: ComposeSpec JSON round-trip
#[test]
fn test_compose_spec_simple_roundtrip() {
    let mut services = indexmap::IndexMap::new();
    services.insert("web".to_string(), ComposeService {
        image: Some("nginx".to_string()),
        ..Default::default()
    });
    let spec = ComposeSpec {
        services,
        ..Default::default()
    };
    let json = serde_json::to_string(&spec).unwrap();
    let decoded: ComposeSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(spec.services.len(), decoded.services.len());
}

// Feature: alloy-container, Property 12: depends_on condition validation
#[test]
fn test_depends_on_condition_serialization() {
    let cond = DependsOnCondition::ServiceHealthy;
    let json = serde_json::to_string(&cond).unwrap();
    assert_eq!(json, "\"service_healthy\"");
}

// Feature: alloy-container, Property 13: Volume type validation
#[test]
fn test_volume_type_serialization() {
    let vt = VolumeType::Bind;
    let json = serde_json::to_string(&vt).unwrap();
    assert_eq!(json, "\"bind\"");
}

// Feature: alloy-container, Property 9: Container name generation uniqueness
#[test]
fn test_container_name_generation_uniqueness() {
    use perry_container_compose::service::service_container_name;
    let svc = ComposeService {
        image: Some("nginx".to_string()),
        ..Default::default()
    };
    let name1 = service_container_name(&svc, "web");
    let name2 = service_container_name(&svc, "web");
    assert_ne!(name1, name2);
    assert!(name1.starts_with("web_"));
}

// Feature: alloy-container, Property 7: Topological sort produces valid ordering
#[test]
fn test_topological_sort_ordering() {
    use perry_container_compose::compose::resolve_startup_order;
    use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec};
    use indexmap::IndexMap;

    let mut services = IndexMap::new();
    services.insert("db".to_string(), ComposeService::default());
    services.insert("web".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["db".to_string()])),
        ..Default::default()
    });

    let spec = ComposeSpec { services, ..Default::default() };
    let order = resolve_startup_order(&spec).unwrap();
    assert_eq!(order, vec!["db", "web"]);
}

// Feature: alloy-container, Property 8: Cycle detection is exhaustive
#[test]
fn test_topological_sort_cycle() {
    use perry_container_compose::compose::resolve_startup_order;
    use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec};
    use indexmap::IndexMap;

    let mut services = IndexMap::new();
    services.insert("a".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["b".to_string()])),
        ..Default::default()
    });
    services.insert("b".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["a".to_string()])),
        ..Default::default()
    });

    let spec = ComposeSpec { services, ..Default::default() };
    let result = resolve_startup_order(&spec);
    assert!(result.is_err());
}
