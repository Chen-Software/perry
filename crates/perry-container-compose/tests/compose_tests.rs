use perry_container_compose::compose::*;
use perry_container_compose::types::*;
use perry_container_compose::error::ComposeError;
use proptest::prelude::*;
use indexmap::IndexMap;
use std::collections::HashMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

prop_compose! {
    fn arb_compose_spec_dag()(
        nodes in 1..8
    ) -> ComposeSpec {
        let mut services = IndexMap::new();
        for i in 0..nodes {
            let name = format!("svc_{}", i);
            let mut svc = ComposeService::default();
            svc.image = Some("alpine:latest".to_string());
            if i > 0 {
                svc.depends_on = Some(DependsOnSpec::List(vec![format!("svc_{}", i-1)]));
            }
            services.insert(name, svc);
        }
        ComposeSpec { services, ..Default::default() }
    }
}

prop_compose! {
    fn arb_compose_spec_cycle()(
        nodes in 2..5
    ) -> ComposeSpec {
        let mut services = IndexMap::new();
        for i in 0..nodes {
            let name = format!("svc_{}", i);
            let mut svc = ComposeService::default();
            svc.image = Some("alpine:latest".to_string());
            let dep = format!("svc_{}", (i + 1) % nodes);
            svc.depends_on = Some(DependsOnSpec::List(vec![dep]));
            services.insert(name, svc);
        }
        ComposeSpec { services, ..Default::default() }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 6.4 | Property: 3
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_dag()) {
        let order = resolve_startup_order(&spec).unwrap();
        let pos: HashMap<String, usize> = order.iter().enumerate().map(|(i, s)| (s.clone(), i)).collect();
        for (name, svc) in &spec.services {
            if let Some(deps) = &svc.depends_on {
                for dep in deps.service_names() {
                    assert!(pos[&dep] < pos[name]);
                }
            }
        }
    }

    // Feature: perry-container | Layer: property | Req: 6.5 | Property: 4
    #[test]
    fn prop_cycle_detection_is_complete(spec in arb_compose_spec_cycle()) {
        let result = resolve_startup_order(&spec);
        assert!(matches!(result, Err(ComposeError::DependencyCycle { .. })));
    }
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
#[test]
fn test_resolve_startup_order_simple_chain() {
    let mut services = IndexMap::new();
    let mut b = ComposeService::default();
    b.depends_on = Some(DependsOnSpec::List(vec!["a".to_string()]));
    services.insert("a".to_string(), ComposeService::default());
    services.insert("b".to_string(), b);
    let spec = ComposeSpec { services, ..Default::default() };
    assert_eq!(resolve_startup_order(&spec).unwrap(), vec!["a", "b"]);
}

// Feature: perry-container | Layer: unit | Req: 6.5 | Property: -
#[test]
fn test_resolve_startup_order_cycle() {
    let mut services = IndexMap::new();
    let mut a = ComposeService::default();
    a.depends_on = Some(DependsOnSpec::List(vec!["b".to_string()]));
    let mut b = ComposeService::default();
    b.depends_on = Some(DependsOnSpec::List(vec!["a".to_string()]));
    services.insert("a".to_string(), a);
    services.insert("b".to_string(), b);
    let spec = ComposeSpec { services, ..Default::default() };
    let res = resolve_startup_order(&spec);
    assert!(matches!(res, Err(ComposeError::DependencyCycle { .. })));
}
