// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -

use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::types::*;
use perry_container_compose::error::ComposeError;
use indexmap::IndexMap;
use proptest::prelude::*;
use std::collections::HashMap;

#[test]
fn test_resolve_startup_order_simple_chain() {
    let mut services = IndexMap::new();
    services.insert("db".to_string(), ComposeService::default());
    services.insert("api".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["db".to_string()])),
        ..Default::default()
    });
    services.insert("web".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["api".to_string()])),
        ..Default::default()
    });
    let spec = ComposeSpec { services, ..Default::default() };
    let order = resolve_startup_order(&spec).expect("Should resolve");
    assert_eq!(order, vec!["db", "api", "web"]);
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
#[test]
fn test_resolve_startup_order_diamond() {
    let mut services = IndexMap::new();
    services.insert("base".to_string(), ComposeService::default());
    services.insert("left".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["base".to_string()])),
        ..Default::default()
    });
    services.insert("right".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["base".to_string()])),
        ..Default::default()
    });
    services.insert("top".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["left".to_string(), "right".to_string()])),
        ..Default::default()
    });
    let spec = ComposeSpec { services, ..Default::default() };
    let order = resolve_startup_order(&spec).expect("Should resolve");
    assert_eq!(order[0], "base");
    assert_eq!(order[3], "top");
    // Peers "left" and "right" should be in alphabetical order due to BTreeSet in Kahn's
    assert_eq!(order[1], "left");
    assert_eq!(order[2], "right");
}

// Feature: perry-container | Layer: unit | Req: 6.5 | Property: -
#[test]
fn test_resolve_startup_order_cycle_detection() {
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
    match result {
        Err(ComposeError::DependencyCycle { services }) => {
            assert!(services.contains(&"a".to_string()));
            assert!(services.contains(&"b".to_string()));
            assert_eq!(services.len(), 2);
        }
        _ => panic!("Expected DependencyCycle error"),
    }
}

// Feature: perry-container | Layer: unit | Req: none | Property: -
#[test]
fn test_resolve_startup_order_missing_dep() {
    let mut services = IndexMap::new();
    services.insert("a".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["nonexistent".to_string()])),
        ..Default::default()
    });
    let spec = ComposeSpec { services, ..Default::default() };
    let result = resolve_startup_order(&spec);
    match result {
        Err(ComposeError::ValidationError { message }) => {
            assert!(message.contains("nonexistent"));
        }
        _ => panic!("Expected ValidationError error"),
    }
}

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

prop_compose! {
    fn arb_service_name()(name in "[a-z0-9_-]{1,64}") -> String { name }
}

prop_compose! {
    fn arb_compose_spec_dag(max_size: usize)(
        names in proptest::collection::vec(arb_service_name(), 2..=max_size)
    ) -> ComposeSpec {
        let mut services = IndexMap::new();
        let mut unique_names = Vec::new();
        for n in names {
            if !unique_names.contains(&n) {
                unique_names.push(n);
            }
        }

        for i in 0..unique_names.len() {
            let mut svc = ComposeService::default();
            if i > 0 {
                // To guarantee DAG, depend only on services with lower index
                let dep_idx = i - 1;
                svc.depends_on = Some(DependsOnSpec::List(vec![unique_names[dep_idx].clone()]));
            }
            services.insert(unique_names[i].clone(), svc);
        }
        ComposeSpec { services, ..Default::default() }
    }
}

prop_compose! {
    fn arb_compose_spec_cycle(max_size: usize)(
        mut spec in arb_compose_spec_dag(max_size)
    ) -> ComposeSpec {
        let names: Vec<String> = spec.services.keys().cloned().collect();
        let first = names[0].clone();
        let last = names[names.len()-1].clone();
        // Add a back-edge to create a cycle
        spec.services.get_mut(&first).unwrap().depends_on = Some(DependsOnSpec::List(vec![last]));
        spec
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 6.4 | Property: 3
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_dag(10)) {
        let order = resolve_startup_order(&spec).expect("Should resolve DAG");
        let pos: HashMap<String, usize> = order.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    prop_assert!(pos[&dep] < pos[name], "Dependency {} must start before {}", dep, name);
                }
            }
        }
    }

    // Feature: perry-container | Layer: property | Req: 6.5 | Property: 4
    #[test]
    fn prop_cycle_detection_is_complete(spec in arb_compose_spec_cycle(10)) {
        let result = resolve_startup_order(&spec);
        match result {
            Err(ComposeError::DependencyCycle { services }) => {
                prop_assert!(!services.is_empty());
            }
            _ => panic!("Expected DependencyCycle"),
        }
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 6.4         | test_resolve_startup_order_simple_chain | unit |
| 6.4         | test_resolve_startup_order_diamond | unit |
| 6.4         | prop_topological_sort_respects_deps | property |
| 6.5         | test_resolve_startup_order_cycle_detection | unit |
| 6.5         | prop_cycle_detection_is_complete | property |
| none        | test_resolve_startup_order_missing_dep | unit |
*/

// Deferred Requirements:
// Req 6.8 - 6.10: Orchestration side effects (creating networks/volumes, rollback)
// require full integration tests with a live backend.
