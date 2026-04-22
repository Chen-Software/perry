use perry_container_compose::types::*;
use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::yaml::interpolate_yaml;
use proptest::prelude::*;
use indexmap::IndexMap;
use std::collections::HashMap;

// Feature: perry-container, Property 1: ComposeSpec serialization round-trip
// Feature: perry-container, Property 5: YAML round-trip preserves ComposeSpec
// (We test JSON round-trip here as it's the primary FFI transport)
proptest! {
    #[test]
    fn prop_compose_spec_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ComposeSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, deserialized);
    }
}

// Feature: perry-container, Property 3: Topological sort respects depends_on
proptest! {
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_with_dag()) {
        let order = resolve_startup_order(&spec).unwrap();
        let pos: HashMap<String, usize> = order.iter().enumerate()
            .map(|(i, s)| (s.clone(), i)).collect();

        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    assert!(pos[&dep] < pos[name],
                        "Dependency {} must start before {}", dep, name);
                }
            }
        }
    }
}

// Feature: perry-container, Property 4: Cycle detection is complete
proptest! {
    #[test]
    fn prop_cycle_detection(spec in arb_compose_spec_with_cycle()) {
        let res = resolve_startup_order(&spec);
        assert!(res.is_err());
        // In Kahn's algorithm, the error should be a DependencyCycle
    }
}

// Feature: perry-container, Property 6: Environment variable interpolation correctness
proptest! {
    #[test]
    fn prop_interpolation(
        (template, env, expected) in arb_interpolation_case()
    ) {
        let result = interpolate_yaml(&template, &env);
        assert_eq!(result, expected);
    }
}

// Generators

fn arb_compose_spec() -> impl Strategy<Value = ComposeSpec> {
    any::<Vec<String>>().prop_map(|names| {
        let mut services = IndexMap::new();
        for name in names.into_iter().take(5) {
            if name.is_empty() { continue; }
            services.insert(name, ComposeService {
                image: Some("alpine".into()),
                ..Default::default()
            });
        }
        ComposeSpec {
            services,
            ..Default::default()
        }
    })
}

fn arb_compose_spec_with_dag() -> impl Strategy<Value = ComposeSpec> {
    // Generate a simple chain A -> B -> C
    Just(vec!["A", "B", "C"]).prop_map(|names| {
        let mut services = IndexMap::new();
        for (i, name) in names.iter().enumerate() {
            let mut service = ComposeService {
                image: Some("alpine".into()),
                ..Default::default()
            };
            if i > 0 {
                service.depends_on = Some(DependsOnSpec::List(vec![names[i-1].to_string()]));
            }
            services.insert(name.to_string(), service);
        }
        ComposeSpec { services, ..Default::default() }
    })
}

fn arb_compose_spec_with_cycle() -> impl Strategy<Value = ComposeSpec> {
    // Generate A -> B -> A
    Just(()).prop_map(|_| {
        let mut services = IndexMap::new();
        services.insert("A".into(), ComposeService {
            image: Some("alpine".into()),
            depends_on: Some(DependsOnSpec::List(vec!["B".into()])),
            ..Default::default()
        });
        services.insert("B".into(), ComposeService {
            image: Some("alpine".into()),
            depends_on: Some(DependsOnSpec::List(vec!["A".into()])),
            ..Default::default()
        });
        ComposeSpec { services, ..Default::default() }
    })
}

fn arb_interpolation_case() -> impl Strategy<Value = (String, HashMap<String, String>, String)> {
    prop_oneof![
        Just(("${VAR}".into(), [("VAR".into(), "VAL".into())].into(), "VAL".into())),
        Just(("${VAR:-DEF}".into(), [].into(), "DEF".into())),
        Just(("${VAR:-DEF}".into(), [("VAR".into(), "VAL".into())].into(), "VAL".into())),
    ]
}
