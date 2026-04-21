use proptest::prelude::*;
use std::collections::HashMap;
use indexmap::IndexMap;
use perry_container_compose::types::*;
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::error::ComposeError;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// --- Generators ---

prop_compose! {
    pub fn arb_service_name()(name in "[a-z0-9_-]{1,32}") -> String {
        name
    }
}

prop_compose! {
    pub fn arb_image_ref()(
        repo in "[a-z0-9]{1,16}",
        tag in "[a-z0-9.-]{1,16}"
    ) -> String {
        format!("{}:{}", repo, tag)
    }
}

prop_compose! {
    pub fn arb_port_spec()(
        target in 1u32..65535,
        published in prop::option::weighted(0.5, 1u32..65535),
        is_long in any::<bool>()
    ) -> PortSpec {
        if is_long {
            PortSpec::Long(ComposeServicePort {
                target: serde_yaml::Value::Number(target.into()),
                published: published.map(|p| serde_yaml::Value::Number(p.into())),
                ..Default::default()
            })
        } else {
            let s = if let Some(p) = published {
                format!("{}:{}", p, target)
            } else {
                target.to_string()
            };
            PortSpec::Short(serde_yaml::Value::String(s))
        }
    }
}

prop_compose! {
    pub fn arb_list_or_dict()(
        is_dict in any::<bool>(),
        map in prop::collection::hash_map("[a-z]{1,8}", "[a-z]{1,8}", 0..5),
        list in prop::collection::vec("[a-z]{1,8}=[a-z]{1,8}", 0..5)
    ) -> ListOrDict {
        if is_dict {
            let mut dict = IndexMap::new();
            for (k, v) in map {
                dict.insert(k, Some(serde_yaml::Value::String(v)));
            }
            ListOrDict::Dict(dict)
        } else {
            ListOrDict::List(list)
        }
    }
}

prop_compose! {
    pub fn arb_depends_on_spec()(
        names in prop::collection::vec(arb_service_name(), 0..3),
        is_map in any::<bool>()
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
        image in prop::option::weighted(0.9, arb_image_ref()),
        env in prop::option::of(arb_list_or_dict()),
        ports in prop::option::of(prop::collection::vec(arb_port_spec(), 0..3)),
        depends_on in prop::option::of(arb_depends_on_spec())
    ) -> ComposeService {
        ComposeService {
            image,
            environment: env,
            ports: ports,
            depends_on,
            ..Default::default()
        }
    }
}

prop_compose! {
    pub fn arb_compose_spec()(
        name in prop::option::of(arb_service_name()),
        services in prop::collection::vec(arb_compose_service(), 1..10)
    ) -> ComposeSpec {
        let mut spec = ComposeSpec::default();
        spec.name = name;
        for (i, svc) in services.into_iter().enumerate() {
            spec.services.insert(format!("svc-{}", i), svc);
        }
        spec
    }
}

prop_compose! {
    pub fn arb_compose_spec_dag(num_services: usize)(
        images in prop::collection::vec(arb_image_ref(), num_services),
        deps in prop::collection::vec(0..num_services, num_services)
    ) -> ComposeSpec {
        let mut spec = ComposeSpec::default();
        for i in 0..num_services {
            let name = format!("svc-{}", i);
            let mut svc = ComposeService {
                image: Some(images[i].clone()),
                ..Default::default()
            };
            if i > 0 {
                let dep_idx = deps[i] % i;
                svc.depends_on = Some(DependsOnSpec::List(vec![format!("svc-{}", dep_idx)]));
            }
            spec.services.insert(name, svc);
        }
        spec
    }
}

prop_compose! {
    pub fn arb_compose_spec_cycle(num_services: usize)(
        mut spec in arb_compose_spec_dag(num_services)
    ) -> ComposeSpec {
        if num_services >= 2 {
            let first_name = "svc-0".to_string();
            let last_name = format!("svc-{}", num_services - 1);
            spec.services.get_mut(&first_name).unwrap().depends_on =
                Some(DependsOnSpec::List(vec![last_name]));
        }
        spec
    }
}

prop_compose! {
    pub fn arb_container_spec()(
        image in arb_image_ref(),
        name in prop::option::of(arb_service_name())
    ) -> ContainerSpec {
        ContainerSpec {
            image,
            name,
            ..Default::default()
        }
    }
}

prop_compose! {
    pub fn arb_env_template()(
        var in "[A-Z_]{1,8}",
        def in "[a-z]{1,8}",
        val in "[a-z]{1,8}",
        kind in 0..3
    ) -> String {
        match kind {
            0 => format!("${{{}}}", var),
            1 => format!("${{{}:-{}}}", var, def),
            2 => format!("${{{}:+{}}}", var, val),
            _ => unreachable!(),
        }
    }
}

prop_compose! {
    pub fn arb_env_map()(
        map in prop::collection::hash_map("[A-Z_]{1,8}", "[a-z]{1,8}", 0..20)
    ) -> HashMap<String, String> {
        map
    }
}

// --- Unit Tests ---

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
#[test]
fn test_resolve_startup_order_simple_chain() {
    let mut spec = ComposeSpec::default();
    let mut s1 = ComposeService::default();
    s1.depends_on = Some(DependsOnSpec::List(vec!["db".to_string()]));
    spec.services.insert("web".to_string(), s1);
    spec.services.insert("db".to_string(), ComposeService::default());

    let order = ComposeEngine::resolve_startup_order(&spec).expect("Should resolve");
    assert_eq!(order, vec!["db", "web"]);
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
#[test]
fn test_resolve_startup_order_isolated() {
    let mut spec = ComposeSpec::default();
    spec.services.insert("a".to_string(), ComposeService::default());
    spec.services.insert("b".to_string(), ComposeService::default());

    let order = ComposeEngine::resolve_startup_order(&spec).expect("Should resolve");
    assert_eq!(order, vec!["a", "b"]);
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: -
#[test]
fn test_resolve_startup_order_missing_dep() {
    let mut spec = ComposeSpec::default();
    let mut s1 = ComposeService::default();
    s1.depends_on = Some(DependsOnSpec::List(vec!["missing".to_string()]));
    spec.services.insert("web".to_string(), s1);

    let res = ComposeEngine::resolve_startup_order(&spec);
    match res {
        Err(ComposeError::ValidationError { message }) => {
            assert!(message.contains("depends on 'missing' which is not defined"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

// --- Property Tests ---

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 6.4 | Property: 3
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_dag(5)) {
        let order = ComposeEngine::resolve_startup_order(&spec).expect("Should resolve DAG");
        let pos: HashMap<String, usize> = order.iter().enumerate()
            .map(|(i, s)| (s.clone(), i)).collect();

        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    assert!(pos[&dep] < pos[name], "Dep {} should be before {}", dep, name);
                }
            }
        }
    }

    // Feature: perry-container | Layer: property | Req: 6.5 | Property: 4
    #[test]
    fn prop_cycle_detection_is_complete(spec in arb_compose_spec_cycle(5)) {
        let res = ComposeEngine::resolve_startup_order(&spec);
        match res {
            Err(ComposeError::DependencyCycle { services }) => {
                assert!(!services.is_empty());
                assert!(services.len() >= 2);
            }
            _ => panic!("Expected DependencyCycle"),
        }
    }
}

// --- Coverage ---
// | Requirement | Test name | Layer |
// |-------------|-----------|-------|
// | 6.4         | test_resolve_startup_order_simple_chain | unit |
// | 6.4         | test_resolve_startup_order_isolated | unit |
// | 6.4         | test_resolve_startup_order_missing_dep | unit |
// | 6.4         | prop_topological_sort_respects_deps | property |
// | 6.5         | prop_cycle_detection_is_complete | property |

// Deferred requirements:
// Req 6.10 — Rollback logic involves async I/O with backend, deferred to integration tests.
