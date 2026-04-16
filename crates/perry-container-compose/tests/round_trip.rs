use perry_container_compose::types::*;
use perry_container_compose::compose::resolve_startup_order;
use proptest::prelude::*;
use std::collections::HashMap;

// Feature: perry-container, Property 1: ComposeSpec serialization round-trip
proptest! {
    #[test]
    fn prop_compose_spec_round_trip(
        name in any::<Option<String>>(),
        version in any::<Option<String>>(),
    ) {
        let mut spec = ComposeSpec::default();
        spec.name = name;
        spec.version = version;

        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ComposeSpec = serde_json::from_str(&json).unwrap();

        assert_eq!(spec.name, deserialized.name);
        assert_eq!(spec.version, deserialized.version);
    }
}

// Feature: perry-container, Property 3: Topological sort respects depends_on
#[test]
fn test_topological_sort_basic() {
    let mut spec = ComposeSpec::default();

    let mut db = ComposeService::default();
    db.image = Some("postgres".into());

    let mut web = ComposeService::default();
    web.image = Some("nginx".into());
    web.depends_on = Some(DependsOnSpec::List(vec!["db".into()]));

    spec.services.insert("db".into(), db);
    spec.services.insert("web".into(), web);

    let order = resolve_startup_order(&spec).unwrap();
    assert_eq!(order, vec!["db", "web"]);
}

#[test]
fn test_topological_sort_cycle() {
    let mut spec = ComposeSpec::default();

    let mut s1 = ComposeService::default();
    s1.depends_on = Some(DependsOnSpec::List(vec!["s2".into()]));

    let mut s2 = ComposeService::default();
    s2.depends_on = Some(DependsOnSpec::List(vec!["s1".into()]));

    spec.services.insert("s1".into(), s1);
    spec.services.insert("s2".into(), s2);

    let result = resolve_startup_order(&spec);
    assert!(result.is_err());
}
