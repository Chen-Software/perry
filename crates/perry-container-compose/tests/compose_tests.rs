use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::types::{
    ComposeService, ComposeSpec, DependsOnSpec, PortSpec, ListOrDict, DependsOnCondition, ComposeDependsOn
};
use perry_container_compose::error::ComposeError;
use indexmap::IndexMap;
use proptest::prelude::*;
use std::collections::HashMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// ============ Required Generators ============

prop_compose! {
    pub fn arb_service_name()(name in "[a-z][a-z0-9_-]{1,10}") -> String {
        name
    }
}

prop_compose! {
    pub fn arb_image_ref()(repo in "[a-z]{3,10}", tag in "[a-z0-9.]{1,5}") -> String {
        format!("{}:{}", repo, tag)
    }
}

prop_compose! {
    pub fn arb_port_spec()(
        is_short in proptest::bool::ANY,
        h_port in 1024..65535u32,
        c_port in 1..65535u32
    ) -> PortSpec {
        if is_short {
            PortSpec::Short(serde_yaml::Value::String(format!("{}:{}", h_port, c_port)))
        } else {
            PortSpec::Long(perry_container_compose::types::ComposeServicePort {
                name: None,
                mode: None,
                host_ip: None,
                target: serde_yaml::Value::Number(c_port.into()),
                published: Some(serde_yaml::Value::Number(h_port.into())),
                protocol: None,
                app_protocol: None,
            })
        }
    }
}

prop_compose! {
    pub fn arb_list_or_dict()(
        is_dict in proptest::bool::ANY,
        keys in proptest::collection::vec("[A-Z_]{1,10}", 0..5),
        vals in proptest::collection::vec("[a-z0-9]{1,10}", 0..5)
    ) -> ListOrDict {
        if is_dict {
            let mut map = IndexMap::new();
            for (k, v) in keys.into_iter().zip(vals.into_iter()) {
                map.insert(k, Some(serde_yaml::Value::String(v)));
            }
            ListOrDict::Dict(map)
        } else {
            let list = keys.into_iter().zip(vals.into_iter())
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            ListOrDict::List(list)
        }
    }
}

prop_compose! {
    pub fn arb_depends_on_spec()(
        is_map in proptest::bool::ANY,
        names in proptest::collection::vec(arb_service_name(), 1..3)
    ) -> DependsOnSpec {
        if is_map {
            let mut map = IndexMap::new();
            for name in names {
                map.insert(name, ComposeDependsOn {
                    condition: DependsOnCondition::ServiceStarted,
                    required: Some(true),
                    restart: Some(false),
                });
            }
            DependsOnSpec::Map(map)
        } else {
            DependsOnSpec::List(names)
        }
    }
}

prop_compose! {
    pub fn arb_compose_service()(
        image in proptest::option::of(arb_image_ref()),
        ports in proptest::option::of(proptest::collection::vec(arb_port_spec(), 0..3)),
        env in proptest::option::of(arb_list_or_dict()),
        deps in proptest::option::of(arb_depends_on_spec())
    ) -> ComposeService {
        ComposeService {
            image,
            ports,
            environment: env,
            depends_on: deps,
            ..Default::default()
        }
    }
}

prop_compose! {
    pub fn arb_compose_spec()(
        services_vec in proptest::collection::vec((arb_service_name(), arb_compose_service()), 1..5)
    ) -> ComposeSpec {
        let mut services = IndexMap::new();
        for (name, svc) in services_vec {
            services.insert(name, svc);
        }
        ComposeSpec { services, ..Default::default() }
    }
}

prop_compose! {
    pub fn arb_compose_spec_dag(max_services: usize)(
        count in 2..=max_services
    ) -> ComposeSpec {
        let mut services = IndexMap::new();
        let names: Vec<String> = (0..count).map(|i| format!("svc_{}", i)).collect();
        for i in 0..count {
            let mut svc = ComposeService::default();
            svc.image = Some("alpine:latest".into());
            // i depends on i-1 (Guaranteed DAG)
            if i > 0 {
                svc.depends_on = Some(DependsOnSpec::List(vec![names[i-1].clone()]));
            }
            services.insert(names[i].clone(), svc);
        }
        ComposeSpec { services, ..Default::default() }
    }
}

prop_compose! {
    pub fn arb_compose_spec_cycle()(
        spec in arb_compose_spec_dag(4)
    ) -> ComposeSpec {
        let mut spec = spec;
        let names: Vec<String> = spec.services.keys().cloned().collect();
        // spec is currently 0 <- 1 <- 2 <- 3 (i depends on i-1)
        // Add edge 0 -> 3 creates cycle 0 -> 3 -> 2 -> 1 -> 0
        let first = names[0].clone();
        let last = names[names.len() - 1].clone();

        let svc = spec.services.get_mut(&first).unwrap();
        svc.depends_on = Some(DependsOnSpec::List(vec![last]));
        spec
    }
}

// ============ Property Tests ============

// Feature: perry-container | Layer: property | Req: 6.4 | Property: 3
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_dag(10)) {
        let order = resolve_startup_order(&spec).expect("DAG should be resolvable");
        let pos: HashMap<String, usize> = order.iter().enumerate()
            .map(|(i, s)| (s.clone(), i)).collect();

        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    let dep_pos = *pos.get(&dep).expect("dep should be in order");
                    let name_pos = *pos.get(name).expect("name should be in order");
                    // dep must start before name
                    prop_assert!(dep_pos < name_pos, "{} (pos {}) must be before {} (pos {})", dep, dep_pos, name, name_pos);
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
    fn prop_cycle_detection_completeness(spec in arb_compose_spec_cycle()) {
        let result = resolve_startup_order(&spec);
        match result {
            Err(ComposeError::DependencyCycle { services }) => {
                prop_assert!(!services.is_empty(), "Cycle error should list services");
                for s in &services {
                    prop_assert!(spec.services.contains_key(s), "Cycle service {} must exist", s);
                }
            }
            _ => panic!("Expected DependencyCycle error, got {:?}", result),
        }
    }
}

// ============ Unit Tests ============

#[test]
fn test_resolve_order_determinism() {
    let mut spec = ComposeSpec::default();
    spec.services.insert("b".into(), ComposeService::default());
    spec.services.insert("a".into(), ComposeService::default());
    let order = resolve_startup_order(&spec).unwrap();
    assert_eq!(order, vec!["a", "b"]);
}

#[test]
fn test_resolve_order_diamond() {
    let mut spec = ComposeSpec::default();
    let mut sb = ComposeService::default();
    sb.depends_on = Some(DependsOnSpec::List(vec!["a".into()]));
    let mut sc = ComposeService::default();
    sc.depends_on = Some(DependsOnSpec::List(vec!["a".into()]));
    let mut sd = ComposeService::default();
    sd.depends_on = Some(DependsOnSpec::List(vec!["b".into(), "c".into()]));

    spec.services.insert("a".into(), ComposeService::default());
    spec.services.insert("b".into(), sb);
    spec.services.insert("c".into(), sc);
    spec.services.insert("d".into(), sd);

    let order = resolve_startup_order(&spec).unwrap();
    assert_eq!(order[0], "a");
    assert_eq!(order[3], "d");
    assert_eq!(order[1], "b");
    assert_eq!(order[2], "c");
}

#[test]
fn test_resolve_order_empty() {
    let spec = ComposeSpec::default();
    let order = resolve_startup_order(&spec).unwrap();
    assert!(order.is_empty());
}

#[test]
fn test_resolve_order_simple() {
    let mut spec = ComposeSpec::default();
    let mut s2 = ComposeService::default();
    s2.depends_on = Some(DependsOnSpec::List(vec!["s1".into()]));
    spec.services.insert("s1".into(), ComposeService::default());
    spec.services.insert("s2".into(), s2);
    let order = resolve_startup_order(&spec).unwrap();
    assert_eq!(order, vec!["s1", "s2"]);
}

#[test]
fn test_resolve_order_cycle() {
    let mut spec = ComposeSpec::default();
    let mut s1 = ComposeService::default();
    s1.depends_on = Some(DependsOnSpec::List(vec!["s2".into()]));
    let mut s2 = ComposeService::default();
    s2.depends_on = Some(DependsOnSpec::List(vec!["s1".into()]));
    spec.services.insert("s1".into(), s1);
    spec.services.insert("s2".into(), s2);
    let result = resolve_startup_order(&spec);
    assert!(matches!(result, Err(ComposeError::DependencyCycle { .. })));
}

#[test]
fn test_resolve_order_missing_dep() {
    let mut spec = ComposeSpec::default();
    let mut s1 = ComposeService::default();
    s1.depends_on = Some(DependsOnSpec::List(vec!["missing".into()]));
    spec.services.insert("s1".into(), s1);
    let result = resolve_startup_order(&spec);
    assert!(matches!(result, Err(ComposeError::ValidationError { .. })));
}

// Coverage Table
// | Requirement | Test name | Layer |
// |-------------|-----------|-------|
// | 6.4         | prop_topological_sort_respects_deps | property |
// | 6.5         | prop_cycle_detection_completeness | property |
// | 6.4         | test_resolve_order_simple | unit |
// | 6.5         | test_resolve_order_cycle | unit |
// | 6.4         | test_resolve_order_missing_dep | unit |
// | 6.4         | test_resolve_order_determinism | unit |
// | 6.4         | test_resolve_order_diamond | unit |
// | 6.4         | test_resolve_order_empty | unit |

// Deferred Requirements:
// None
