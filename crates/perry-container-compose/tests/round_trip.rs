use perry_container_compose::types::ComposeSpec;
use perry_container_compose::compose::ComposeEngine;
use proptest::prelude::*;

// Feature: perry-container, Property 1: ComposeSpec serialization round-trip
proptest! {
    #[test]
    fn prop_compose_spec_round_trip(name in ".*", version in ".*") {
        let mut spec = ComposeSpec::default();
        spec.name = Some(name.clone());
        spec.version = Some(version.clone());

        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ComposeSpec = serde_json::from_str(&json).unwrap();

        assert_eq!(spec.name, deserialized.name);
        assert_eq!(spec.version, deserialized.version);
    }
}

// Feature: perry-container, Property 3: Topological sort respects depends_on
#[test]
fn test_topological_sort_respects_deps() {
    let mut spec = ComposeSpec::default();

    let mut web = perry_container_compose::types::ComposeService::default();
    web.image = Some("nginx".into());
    web.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(vec!["db".into()]));

    let mut db = perry_container_compose::types::ComposeService::default();
    db.image = Some("postgres".into());

    spec.services.insert("web".into(), web);
    spec.services.insert("db".into(), db);

    let order = ComposeEngine::resolve_startup_order(&spec).unwrap();

    let web_idx = order.iter().position(|s| s == "web").unwrap();
    let db_idx = order.iter().position(|s| s == "db").unwrap();

    assert!(db_idx < web_idx, "db should start before web");
}

// Feature: perry-container, Property 4: Cycle detection is complete
#[test]
fn test_cycle_detection() {
    let mut spec = ComposeSpec::default();

    let mut s1 = perry_container_compose::types::ComposeService::default();
    s1.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(vec!["s2".into()]));

    let mut s2 = perry_container_compose::types::ComposeService::default();
    s2.depends_on = Some(perry_container_compose::types::DependsOnSpec::List(vec!["s1".into()]));

    spec.services.insert("s1".into(), s1);
    spec.services.insert("s2".into(), s2);

    let res = ComposeEngine::resolve_startup_order(&spec);
    assert!(res.is_err());
    if let Err(perry_container_compose::error::ComposeError::DependencyCycle { services }) = res {
        assert!(services.contains(&"s1".to_string()));
        assert!(services.contains(&"s2".to_string()));
    } else {
        panic!("Expected DependencyCycle error");
    }
}
