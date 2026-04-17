use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec, DependsOnCondition, ComposeDependsOn};
use perry_container_compose::error::ComposeError;
use indexmap::IndexMap;
use proptest::prelude::*;
use std::collections::HashMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// --- Generators (Duplicated from types_tests as per rule but named locally) ---

prop_compose! {
    fn loc_service_name()(name in "[a-z0-9-]{1,10}") -> String { name }
}

prop_compose! {
    fn loc_depends_on_spec(possible_deps: Vec<String>)(
        indices in prop::collection::vec(0..possible_deps.len().max(1), 0..3),
        is_list in prop::bool::ANY
    ) -> DependsOnSpec {
        let names: Vec<String> = if possible_deps.is_empty() {
            vec![]
        } else {
            indices.into_iter().map(|i| possible_deps[i % possible_deps.len()].clone()).collect()
        };
        if is_list {
            DependsOnSpec::List(names)
        } else {
            let mut map = IndexMap::new();
            for name in names {
                map.insert(name, ComposeDependsOn {
                    condition: Some(DependsOnCondition::ServiceStarted),
                    ..Default::default()
                });
            }
            DependsOnSpec::Map(map)
        }
    }
}

prop_compose! {
    fn arb_compose_spec_dag()(
        service_names in prop::collection::vec(loc_service_name(), 1..10)
            .prop_map(|v| {
                let mut seen = std::collections::HashSet::new();
                v.into_iter().filter(|s| seen.insert(s.clone())).collect::<Vec<_>>()
            })
    )(
        services_with_deps in service_names.iter().cloned().enumerate().map(|(i, name)| {
            let possible_deps = service_names[..i].to_vec();
            (Just(name), loc_depends_on_spec(possible_deps))
        }).collect::<Vec<_>>()
    ) -> ComposeSpec {
        let mut services = IndexMap::new();
        for (name, deps) in services_with_deps {
            services.insert(name, ComposeService {
                image: Some("alpine".into()),
                depends_on: Some(deps),
                ..Default::default()
            });
        }
        ComposeSpec { services, ..Default::default() }
    }
}

prop_compose! {
    fn arb_compose_spec_cycle()(
        spec in arb_compose_spec_dag(),
        edge in (0..10usize, 0..10usize)
    ) -> ComposeSpec {
        let mut services = spec.services;
        if services.len() < 2 { return ComposeSpec { services, ..Default::default() }; }
        let names: Vec<String> = services.keys().cloned().collect();
        let u = &names[edge.0 % names.len()];
        let v = &names[edge.1 % names.len()];
        let svc = services.get_mut(u).unwrap();
        let mut current_deps = svc.depends_on.as_ref().map(|d| d.service_names()).unwrap_or_default();
        current_deps.push(v.clone());
        svc.depends_on = Some(DependsOnSpec::List(current_deps));
        ComposeSpec { services, ..Default::default() }
    }
}

// --- Unit Tests ---

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: 3
#[test]
fn test_resolve_startup_order_happy_path() {
    let mut services = IndexMap::new();
    services.insert("web".into(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["db".into()])),
        ..Default::default()
    });
    services.insert("db".into(), ComposeService::default());
    let spec = ComposeSpec { services, ..Default::default() };
    let order = resolve_startup_order(&spec).expect("Should resolve");
    assert_eq!(order, vec!["db", "web"]);
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: 3
#[test]
fn test_resolve_startup_order_isolated_nodes() {
    let mut services = IndexMap::new();
    services.insert("b".into(), ComposeService::default());
    services.insert("a".into(), ComposeService::default());
    let spec = ComposeSpec { services, ..Default::default() };
    let order = resolve_startup_order(&spec).expect("Should resolve");
    assert_eq!(order, vec!["a", "b"]); // Sorted alphabetically for determinism
}

// Feature: perry-container | Layer: unit | Req: 6.5 | Property: 4
#[test]
fn test_resolve_startup_order_cycle() {
    let mut services = IndexMap::new();
    services.insert("a".into(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["b".into()])),
        ..Default::default()
    });
    services.insert("b".into(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["a".into()])),
        ..Default::default()
    });
    let spec = ComposeSpec { services, ..Default::default() };
    match resolve_startup_order(&spec) {
        Err(ComposeError::DependencyCycle { services }) => {
            assert!(services.contains(&"a".to_string()) && services.contains(&"b".to_string()));
        }
        _ => panic!("Expected DependencyCycle error"),
    }
}

// --- Property Tests ---

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 6.4 | Property: 3
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_dag()) {
        let order = resolve_startup_order(&spec).unwrap();
        let pos: HashMap<String, usize> = order.into_iter().enumerate().map(|(i, s)| (s, i)).collect();
        for (name, svc) in &spec.services {
            if let Some(deps) = &svc.depends_on {
                for dep in deps.service_names() {
                    assert!(pos[&dep] < pos[name], "Dep {} must precede {}", dep, name);
                }
            }
        }
    }

    // Feature: perry-container | Layer: property | Req: 6.5 | Property: 4
    #[test]
    fn prop_cycle_detection_is_complete(spec in arb_compose_spec_cycle()) {
        let result = resolve_startup_order(&spec);
        if let Err(ComposeError::DependencyCycle { services }) = result {
            assert!(!services.is_empty());
        } else if let Ok(order) = result {
             let pos: HashMap<String, usize> = order.into_iter().enumerate().map(|(i, s)| (s, i)).collect();
             for (name, svc) in &spec.services {
                 if let Some(deps) = &svc.depends_on {
                     for dep in deps.service_names() {
                         assert!(pos[&dep] < pos[name]);
                     }
                 }
             }
        }
    }
}
