use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::types::*;
use proptest::prelude::*;
use perry_container_compose::IndexMap;

fn arb_compose_spec_with_dag() -> impl Strategy<Value = ComposeSpec> {
    // Generate a simple DAG: db -> web, redis -> web
    // For simplicity, we'll just return a fixed DAG spec in this property test
    // or we could use a more complex generator.
    // Let's create a 3-service spec where web depends on db and redis.
    let mut services = IndexMap::new();

    services.insert("db".to_string(), ComposeService::default());
    services.insert("redis".to_string(), ComposeService::default());
    services.insert("web".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["db".to_string(), "redis".to_string()])),
        ..Default::default()
    });

    Just(ComposeSpec {
        services,
        ..Default::default()
    })
}

// Feature: perry-container, Property 3: Topological sort respects depends_on
proptest! {
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_with_dag()) {
        let order = resolve_startup_order(&spec).unwrap();
        let pos: std::collections::HashMap<&str, usize> = order.iter().enumerate()
            .map(|(i, s)| (s.as_str(), i)).collect();
        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    prop_assert!(pos[dep.as_str()] < pos[name.as_str()],
                        "dep {} should come before {}", dep, name);
                }
            }
        }
    }
}

// Feature: perry-container, Property 4: Cycle detection is complete
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
    let res = resolve_startup_order(&spec);
    assert!(res.is_err());
}
