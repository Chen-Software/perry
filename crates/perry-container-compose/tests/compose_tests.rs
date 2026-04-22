//! Tests for the `compose` module.
//!
//! Validates Kahn's algorithm for dependency resolution (startup order)
//! and cycle detection completeness.

use indexmap::IndexMap;
use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::error::ComposeError;
use perry_container_compose::types::{ComposeService, ComposeSpec, DependsOnSpec};
use proptest::prelude::*;
use std::collections::HashMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// ============ Generators ============

prop_compose! {
    /// Generate a valid compose service name string.
    fn arb_service_name()(name in "[a-z][a-z0-9_-]{0,10}") -> String {
        name
    }
}

prop_compose! {
    /// Generate a valid OCI image reference string.
    fn arb_image_ref()(repo in "[a-z0-9]+", tag in "[a-z0-9.]+") -> String {
        format!("{}:{}", repo, tag)
    }
}

prop_compose! {
    /// Generate a ComposeSpec with 1–10 services.
    fn arb_compose_spec()(
        services_vec in proptest::collection::vec(
            (arb_service_name(), arb_image_ref()).prop_map(|(name, image)| {
                let mut svc = ComposeService::default();
                svc.image = Some(image);
                (name, svc)
            }),
            1..=10
        )
    ) -> ComposeSpec {
        let mut services = IndexMap::new();
        for (name, svc) in services_vec {
            services.insert(name, svc);
        }
        ComposeSpec { services, ..Default::default() }
    }
}

prop_compose! {
    /// Generate a ComposeSpec with a valid acyclic depends_on graph.
    /// Build services first, add edges only from higher-index to lower-index services.
    fn arb_compose_spec_dag()(
        service_names in proptest::collection::vec(arb_service_name(), 2..=10).prop_map(|v| {
            let mut v = v;
            v.sort();
            v.dedup();
            v
        })
    )(
        services_with_deps in {
            let n = service_names.len();
            let names = service_names.clone();
            proptest::collection::vec(
                proptest::collection::vec(0..n, 0..n), // Indices of potential dependencies
                n
            ).prop_map(move |deps_indices_list| {
                let mut services = IndexMap::new();
                for (i, deps_indices) in deps_indices_list.into_iter().enumerate() {
                    let mut svc = ComposeService::default();
                    svc.image = Some("img:latest".to_string());

                    // Only keep deps with index < i (guarantees DAG)
                    let mut valid_deps = Vec::new();
                    for idx in deps_indices {
                        let idx = idx % n;
                        if idx < i {
                            valid_deps.push(names[idx].clone());
                        }
                    }
                    valid_deps.sort();
                    valid_deps.dedup();

                    if !valid_deps.is_empty() {
                        svc.depends_on = Some(DependsOnSpec::List(valid_deps));
                    }
                    services.insert(names[i].clone(), svc);
                }
                ComposeSpec { services, ..Default::default() }
            })
        }
    ) -> ComposeSpec {
        services_with_deps
    }
}

prop_compose! {
    /// Generate a ComposeSpec with exactly one cycle.
    /// Start with a valid DAG, then add exactly one back-edge.
    fn arb_compose_spec_cycle()(
        (spec, i, j) in arb_compose_spec_dag().prop_flat_map(|s| {
            let n = s.services.len();
            (Just(s), 0..n, 0..n)
        })
    ) -> ComposeSpec {
        let mut spec = spec;
        let n = spec.services.len();
        let mut i = i % n;
        let mut j = j % n;

        if i == j {
            // Self-cycle
            let name = spec.services.get_index(i).unwrap().0.clone();
            let svc = spec.services.get_mut(&name).unwrap();
            svc.depends_on = Some(DependsOnSpec::List(vec![name.clone()]));
        } else {
            if i > j { std::mem::swap(&mut i, &mut j); }
            // i < j.
            // In our DAG, service[j] can depend on service[i].
            // Force j to depend on i.
            let name_i = spec.services.get_index(i).unwrap().0.clone();
            let name_j = spec.services.get_index(j).unwrap().0.clone();

            {
                let svc_j = spec.services.get_mut(&name_j).unwrap();
                let mut deps_j = match svc_j.depends_on.take() {
                    Some(DependsOnSpec::List(l)) => l,
                    Some(DependsOnSpec::Map(m)) => m.keys().cloned().collect(),
                    None => Vec::new(),
                };
                deps_j.push(name_i.clone());
                deps_j.sort();
                deps_j.dedup();
                svc_j.depends_on = Some(DependsOnSpec::List(deps_j));
            }

            // Add back-edge: i depends on j. Cycle: i -> j -> i.
            {
                let svc_i = spec.services.get_mut(&name_i).unwrap();
                let mut deps_i = match svc_i.depends_on.take() {
                    Some(DependsOnSpec::List(l)) => l,
                    Some(DependsOnSpec::Map(m)) => m.keys().cloned().collect(),
                    None => Vec::new(),
                };
                deps_i.push(name_j);
                deps_i.sort();
                deps_i.dedup();
                svc_i.depends_on = Some(DependsOnSpec::List(deps_i));
            }
        }
        spec
    }
}

// ============ Property Tests ============

// Feature: perry-container | Layer: property | Req: 6.4 | Property: 3
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_dag()) {
        let order = resolve_startup_order(&spec).expect("DAG should be resolvable");
        let pos: HashMap<String, usize> = order.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();

        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    let dep_pos = pos.get(&dep).expect("Dependency should be in order");
                    let name_pos = pos.get(name).expect("Service should be in order");
                    prop_assert!(dep_pos < name_pos, "Dependency {} must come before {}", dep, name);
                }
            }
        }
        prop_assert_eq!(order.len(), spec.services.len());
    }
}

// Feature: perry-container | Layer: property | Req: 6.5 | Property: 4
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_cycle_detection_is_complete(spec in arb_compose_spec_cycle()) {
        let result = resolve_startup_order(&spec);
        match result {
            Err(ComposeError::DependencyCycle { services }) => {
                prop_assert!(!services.is_empty(), "Cycle error should list services");
                for s in &services {
                    prop_assert!(spec.services.contains_key(s), "Listed service should exist");
                }
            }
            Ok(order) => {
                panic!("Expected DependencyCycle error, but got order: {:?}", order);
            }
            Err(e) => panic!("Expected DependencyCycle error, but got: {:?}", e),
        }
    }
}

// ============ Unit Tests ============

fn make_spec(edges: &[(&str, &[&str])]) -> ComposeSpec {
    let mut services = IndexMap::new();
    for (name, deps) in edges {
        let mut svc = ComposeService::default();
        if !deps.is_empty() {
            svc.depends_on = Some(DependsOnSpec::List(deps.iter().map(|s| s.to_string()).collect()));
        }
        services.insert(name.to_string(), svc);
    }
    ComposeSpec { services, ..Default::default() }
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: 3
#[test]
fn test_resolve_startup_order_happy_path() {
    let spec = make_spec(&[("a", &[]), ("b", &["a"]), ("c", &["b"])]);
    let order = resolve_startup_order(&spec).expect("Should succeed");
    assert_eq!(order, vec!["a", "b", "c"]);
}

// Feature: perry-container | Layer: unit | Req: 6.5 | Property: 4
#[test]
fn test_resolve_startup_order_cycle() {
    let spec = make_spec(&[("a", &["b"]), ("b", &["a"])]);
    let err = resolve_startup_order(&spec).unwrap_err();
    match err {
        ComposeError::DependencyCycle { services } => {
            assert!(services.contains(&"a".to_string()));
            assert!(services.contains(&"b".to_string()));
        }
        _ => panic!("Wrong error type"),
    }
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: 3
#[test]
fn test_resolve_startup_order_missing_dep() {
    let spec = make_spec(&[("a", &["missing"])]);
    let err = resolve_startup_order(&spec).unwrap_err();
    assert!(err.to_string().contains("depends on 'missing' which is not defined"));
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
#[test]
fn test_resolve_startup_order_deterministic() {
    // Alphabetical sort for tied in-degree
    let spec = make_spec(&[("c", &[]), ("a", &[]), ("b", &[])]);
    let order = resolve_startup_order(&spec).expect("Should succeed");
    assert_eq!(order, vec!["a", "b", "c"]);
}

// ============ Coverage Table ============
//
// | Requirement | Test name | Layer |
// |-------------|-----------|-------|
// | 6.4         | prop_topological_sort_respects_deps | property |
// | 6.4         | test_resolve_startup_order_happy_path | unit |
// | 6.4         | test_resolve_startup_order_missing_dep | unit |
// | 6.4         | test_resolve_startup_order_deterministic | unit |
// | 6.5         | prop_cycle_detection_is_complete | property |
// | 6.5         | test_resolve_startup_order_cycle | unit |
