// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -

use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::types::*;
use perry_container_compose::error::ComposeError;
use perry_container_compose::indexmap::IndexMap;
use proptest::prelude::*;
use std::collections::HashMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// =============================================================================
// Unit Tests
// =============================================================================

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
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
    // Peers "left" and "right" should be in alphabetical order
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
        }
        _ => panic!("Expected DependencyCycle error"),
    }
}

// Feature: perry-container | Layer: unit | Req: none | Property: -
#[test]
fn test_resolve_startup_order_missing_dep() {
    let mut services = IndexMap::new();
    services.insert("a".to_string(), ComposeService {
        depends_on: Some(DependsOnSpec::List(vec!["missing".to_string()])),
        ..Default::default()
    });
    let spec = ComposeSpec { services, ..Default::default() };
    let result = resolve_startup_order(&spec);
    match result {
        Err(ComposeError::ValidationError { message }) => {
            assert!(message.contains("missing"));
        }
        _ => panic!("Expected ValidationError error"),
    }
}

// =============================================================================
// Required Generators
// =============================================================================

prop_compose! {
    pub fn arb_service_name()(name in "[a-z0-9_-]{1,64}") -> String { name }
}

prop_compose! {
    pub fn arb_image_ref()(repo in "[a-z0-9/._-]{1,128}", tag in proptest::option::of("[a-z0-9._-]{1,32}")) -> String {
        match tag {
            Some(t) => format!("{}:{}", repo, t),
            None => repo,
        }
    }
}

prop_compose! {
    pub fn arb_port_spec()(
        is_long in any::<bool>(),
        h in 1u16..65535,
        c in 1u16..65535
    ) -> PortSpec {
        if is_long {
            PortSpec::Long(ComposeServicePort {
                target: serde_yaml::Value::Number(c.into()),
                published: Some(serde_yaml::Value::Number(h.into())),
                ..Default::default()
            })
        } else {
            PortSpec::Short(serde_yaml::Value::String(format!("{}:{}", h, c)))
        }
    }
}

prop_compose! {
    pub fn arb_list_or_dict()(
        is_dict in any::<bool>(),
        keys in proptest::collection::vec("[a-zA-Z0-9_]{1,32}", 0..10),
        values in proptest::collection::vec("[a-zA-Z0-9_]{0,64}", 0..10)
    ) -> ListOrDict {
        if is_dict {
            let mut map = IndexMap::new();
            for (k, v) in keys.into_iter().zip(values.into_iter()) {
                map.insert(k, Some(serde_yaml::Value::String(v)));
            }
            ListOrDict::Dict(map)
        } else {
            ListOrDict::List(keys.into_iter().zip(values.into_iter()).map(|(k, v)| format!("{}={}", k, v)).collect())
        }
    }
}

prop_compose! {
    pub fn arb_depends_on_spec()(
        is_map in any::<bool>(),
        services in proptest::collection::vec(arb_service_name(), 1..5)
    ) -> DependsOnSpec {
        if is_map {
            let mut map = IndexMap::new();
            for s in services {
                map.insert(s, ComposeDependsOn {
                    condition: Some(DependsOnCondition::ServiceStarted),
                    ..Default::default()
                });
            }
            DependsOnSpec::Map(map)
        } else {
            DependsOnSpec::List(services)
        }
    }
}

prop_compose! {
    pub fn arb_compose_service()(
        image in proptest::option::of(arb_image_ref()),
        env in proptest::option::of(arb_list_or_dict()),
        ports in proptest::option::of(proptest::collection::vec(arb_port_spec(), 0..3)),
        deps in proptest::option::of(arb_depends_on_spec())
    ) -> ComposeService {
        ComposeService {
            image,
            environment: env,
            ports,
            depends_on: deps,
            ..Default::default()
        }
    }
}

prop_compose! {
    pub fn arb_compose_spec()(
        name in proptest::option::of("[a-z0-9_-]{1,32}"),
        service_names in proptest::collection::vec(arb_service_name(), 1..5)
    ) -> ComposeSpec {
        let mut services = IndexMap::new();
        for s in service_names {
            services.insert(s, ComposeService::default());
        }
        ComposeSpec { name, services, ..Default::default() }
    }
}

prop_compose! {
    pub fn arb_compose_spec_dag()(
        service_names in proptest::collection::vec(arb_service_name(), 2..6)
    ) -> ComposeSpec {
        let mut services = IndexMap::new();
        let mut names_vec: Vec<String> = Vec::new();
        for name in service_names {
            let mut svc = ComposeService::default();
            if !names_vec.is_empty() {
                // Guaranteed acyclic: only depend on earlier nodes
                let dep = names_vec[0].clone();
                svc.depends_on = Some(DependsOnSpec::List(vec![dep]));
            }
            services.insert(name.clone(), svc);
            names_vec.push(name);
        }
        ComposeSpec { services, ..Default::default() }
    }
}

prop_compose! {
    pub fn arb_compose_spec_cycle()(
        mut spec in arb_compose_spec_dag()
    ) -> ComposeSpec {
        let names: Vec<String> = spec.services.keys().cloned().collect();
        let first = names[0].clone();
        let last = names[names.len()-1].clone();
        // Guaranteed cycle: last depends on first
        spec.services.get_mut(&first).unwrap().depends_on = Some(DependsOnSpec::List(vec![last]));
        spec
    }
}

prop_compose! {
    pub fn arb_container_spec()(
        image in arb_image_ref(),
        name in proptest::option::of("[a-z0-9_-]{1,32}"),
        rm in proptest::option::of(any::<bool>())
    ) -> ContainerSpec {
        ContainerSpec { image, name, rm, ..Default::default() }
    }
}

prop_compose! {
    pub fn arb_env_template()(
        var in "[A-Z_][A-Z0-9_]*",
        default in proptest::option::of("[a-z0-9]*")
    ) -> String {
        match default {
            Some(d) => format!("${{{}:-{}}}", var, d),
            None => format!("${{{}}}", var),
        }
    }
}

prop_compose! {
    pub fn arb_env_map()(
        map in proptest::collection::hash_map("[A-Z_]+", ".*", 0..10)
    ) -> HashMap<String, String> { map }
}

// =============================================================================
// Property Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 6.4 | Property: 3
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_dag()) {
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
    fn prop_cycle_detection_is_complete(spec in arb_compose_spec_cycle()) {
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
*/

// Deferred Requirements:
// Req 6.1, 6.6-6.10: ComposeEngine lifecycle methods require live container backend for integration testing.
