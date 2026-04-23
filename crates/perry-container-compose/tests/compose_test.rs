use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec};
use perry_container_compose::compose::resolve_startup_order;
use indexmap::IndexMap;

#[test]
fn test_topological_sort_simple() {
    let mut services = IndexMap::new();
    services.insert("db".to_string(), ComposeService::default());
    services.insert("api".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["db".to_string()])),
        ..Default::default()
    });

    let spec = ComposeSpec { services, ..Default::default() };
    let order = resolve_startup_order(&spec).unwrap();
    assert_eq!(order, vec!["db", "api"]);
}

#[test]
fn test_topological_sort_diamond() {
    let mut services = IndexMap::new();
    services.insert("base".to_string(), ComposeService::default());
    services.insert("dep1".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["base".to_string()])),
        ..Default::default()
    });
    services.insert("dep2".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["base".to_string()])),
        ..Default::default()
    });
    services.insert("top".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["dep1".to_string(), "dep2".to_string()])),
        ..Default::default()
    });

    let spec = ComposeSpec { services, ..Default::default() };
    let order = resolve_startup_order(&spec).unwrap();
    assert_eq!(order[0], "base");
    assert!(order[1] == "dep1" || order[1] == "dep2");
    assert!(order[2] == "dep1" || order[2] == "dep2");
    assert_eq!(order[3], "top");
}

#[test]
fn test_cycle_detection() {
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
