use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec};
use perry_container_compose::error::{ComposeError, compose_error_to_js};
use indexmap::IndexMap;
use proptest::prelude::*;
use std::collections::HashMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

fn make_compose(edges: &[(&str, &[&str])]) -> ComposeSpec {
    let mut services = IndexMap::new();
    for (name, deps) in edges {
        let mut svc = ComposeService::default();
        if !deps.is_empty() {
            svc.depends_on = Some(DependsOnSpec::List(
                deps.iter().map(|s| s.to_string()).collect(),
            ));
        }
        services.insert(name.to_string(), svc);
    }
    ComposeSpec {
        services,
        ..Default::default()
    }
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
#[test]
fn test_simple_chain() {
    let compose = make_compose(&[("web", &["db"]), ("db", &[]), ("proxy", &["web"])]);
    let order = resolve_startup_order(&compose).unwrap();
    let pos = |name: &str| order.iter().position(|s| s == name).unwrap();
    assert!(pos("db") < pos("web"), "db must precede web");
    assert!(pos("web") < pos("proxy"), "web must precede proxy");
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
#[test]
fn test_no_deps() {
    let compose = make_compose(&[("a", &[]), ("b", &[]), ("c", &[])]);
    let order = resolve_startup_order(&compose).unwrap();
    assert_eq!(order.len(), 3);
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
#[test]
fn test_diamond_dependency() {
    let compose = make_compose(&[
        ("a", &[]),
        ("b", &["a"]),
        ("c", &["a"]),
        ("d", &["b", "c"]),
    ]);
    let order = resolve_startup_order(&compose).unwrap();
    let pos = |name: &str| order.iter().position(|s| s == name).unwrap();
    assert!(pos("a") < pos("b"));
    assert!(pos("a") < pos("c"));
    assert!(pos("b") < pos("d"));
    assert!(pos("c") < pos("d"));
}

// Feature: perry-container | Layer: unit | Req: 6.5 | Property: -
#[test]
fn test_cycle_detected() {
    let compose = make_compose(&[("a", &["b"]), ("b", &["a"])]);
    let result = resolve_startup_order(&compose);
    assert!(result.is_err());
}

// Feature: perry-container | Layer: unit | Req: 6.5 | Property: -
#[test]
fn test_cycle_lists_all_services() {
    let compose = make_compose(&[("a", &["c"]), ("b", &["a"]), ("c", &["b"])]);
    let result = resolve_startup_order(&compose);
    assert!(result.is_err());
    if let Err(ComposeError::DependencyCycle { services }) = result {
        assert_eq!(services.len(), 3);
        assert!(services.contains(&"a".to_string()));
        assert!(services.contains(&"b".to_string()));
        assert!(services.contains(&"c".to_string()));
    }
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
#[test]
fn test_invalid_dependency() {
    let compose = make_compose(&[("web", &["nonexistent"])]);
    let result = resolve_startup_order(&compose);
    assert!(result.is_err());
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
#[test]
fn test_deterministic_order() {
    let compose = make_compose(&[("c", &[]), ("a", &[]), ("b", &[])]);
    let order = resolve_startup_order(&compose).unwrap();
    assert_eq!(order, vec!["a", "b", "c"]);
}

// ============ Property Generators ============

prop_compose! {
    fn arb_service_name()(name in "[a-z][a-z0-9_-]{1,10}") -> String {
        name
    }
}

fn arb_unique_names(n: usize) -> impl Strategy<Value = Vec<String>> {
    proptest::collection::hash_set(arb_service_name(), n..=n)
        .prop_map(|set| set.into_iter().collect())
}

prop_compose! {
    fn arb_dag_edges(n: usize)(
        raw_edges in proptest::collection::vec(proptest::collection::vec(0..n, 0..=2), n)
    ) -> Vec<Vec<usize>> {
        raw_edges.into_iter().enumerate().map(|(i, deps)| {
            deps.into_iter().filter(|&d| d < i).collect()
        }).collect()
    }
}

fn arb_compose_spec_dag(n_services: usize) -> impl Strategy<Value = ComposeSpec> {
    (arb_unique_names(n_services), arb_dag_edges(n_services))
        .prop_map(move |(names, edge_indices)| {
            let mut services = IndexMap::new();
            for (i, deps) in edge_indices.into_iter().enumerate() {
                let mut svc = ComposeService::default();
                svc.image = Some("alpine:latest".to_string());
                let dep_names: Vec<String> = deps.into_iter().map(|idx| names[idx].clone()).collect();
                if !dep_names.is_empty() {
                    svc.depends_on = Some(DependsOnSpec::List(dep_names));
                }
                services.insert(names[i].clone(), svc);
            }
            ComposeSpec { services, ..Default::default() }
        })
}

fn arb_compose_spec_cycle(n_services: usize) -> impl Strategy<Value = ComposeSpec> {
    let n = n_services.max(2);
    arb_unique_names(n).prop_map(|names| {
        let mut services = IndexMap::new();
        for i in 0..names.len() {
            let mut svc = ComposeService::default();
            svc.image = Some("alpine:latest".to_string());
            let next_idx = (i + 1) % names.len();
            svc.depends_on = Some(DependsOnSpec::List(vec![names[next_idx].clone()]));
            services.insert(names[i].clone(), svc);
        }
        ComposeSpec { services, ..Default::default() }
    })
}

// Feature: perry-container | Layer: property | Req: 6.4 | Property: 3
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_dag(5)) {
        let order = resolve_startup_order(&spec).unwrap();
        let pos: HashMap<&str, usize> = order.iter().enumerate().map(|(i, s)| (s.as_str(), i)).collect();
        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    prop_assert!(pos[dep.as_str()] < pos[name.as_str()]);
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
    fn prop_cycle_detection_completeness(spec in arb_compose_spec_cycle(4)) {
        let result = resolve_startup_order(&spec);
        prop_assert!(result.is_err());
    }
}

// Feature: perry-container | Layer: property | Req: 12.2 | Property: 11
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_error_propagation_preserves_code_and_message(code in -100i32..500i32, message in ".*") {
        let err = ComposeError::BackendError { code, message: message.clone() };
        let js_json = compose_error_to_js(&err);
        let val: serde_json::Value = serde_json::from_str(&js_json).unwrap();
        prop_assert_eq!(val["code"].as_i64().unwrap() as i32, code);
        prop_assert!(val["message"].as_str().unwrap().contains(&message));
    }
}
