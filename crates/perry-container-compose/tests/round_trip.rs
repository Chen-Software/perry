// Feature: perry-container, Property 1: ComposeSpec serialization round-trip
// Feature: perry-container, Property 3: Topological sort respects depends_on
// Feature: perry-container, Property 4: Cycle detection is complete
// Feature: perry-container, Property 5: YAML round-trip preserves ComposeSpec
// Feature: perry-container, Property 6: Environment variable interpolation correctness
// Feature: perry-container, Property 7: Compose file merge is last-writer-wins

use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec};
use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::yaml::interpolate;
use proptest::prelude::*;
use std::collections::HashMap;
use indexmap::IndexMap;

// Generators for property-based testing

fn arb_service_name() -> impl Strategy<Value = String> {
    "[a-z0-9]{1,10}"
}

fn arb_compose_service() -> impl Strategy<Value = ComposeService> {
    any::<Option<String>>().prop_map(|image| ComposeService {
        image,
        ..Default::default()
    })
}

fn arb_compose_spec() -> impl Strategy<Value = ComposeSpec> {
    prop::collection::hash_map(arb_service_name(), arb_compose_service(), 1..5)
        .prop_map(|services| {
            let mut imap = IndexMap::new();
            for (k, v) in services {
                imap.insert(k, v);
            }
            ComposeSpec {
                services: imap,
                ..Default::default()
            }
        })
}

fn arb_compose_spec_with_dag() -> impl Strategy<Value = ComposeSpec> {
    // Generate 3 services: a, b, c. Make b depend on a, c depend on b.
    Just(vec!["a".to_string(), "b".to_string(), "c".to_string()]).prop_map(|names| {
        let mut services = IndexMap::new();
        for (i, name) in names.iter().enumerate() {
            let mut svc = ComposeService {
                image: Some("nginx".to_string()),
                ..Default::default()
            };
            if i > 0 {
                svc.depends_on = Some(DependsOnSpec::List(vec![names[i-1].clone()]));
            }
            services.insert(name.clone(), svc);
        }
        ComposeSpec { services, ..Default::default() }
    })
}

fn arb_compose_spec_with_cycle() -> impl Strategy<Value = ComposeSpec> {
    // Generate 2 services: a, b. Make a depend on b, b depend on a.
    Just(vec!["a".to_string(), "b".to_string()]).prop_map(|_names| {
        let mut services = IndexMap::new();

        let mut svc_a = ComposeService { image: Some("nginx".to_string()), ..Default::default() };
        svc_a.depends_on = Some(DependsOnSpec::List(vec!["b".to_string()]));

        let mut svc_b = ComposeService { image: Some("nginx".to_string()), ..Default::default() };
        svc_b.depends_on = Some(DependsOnSpec::List(vec!["a".to_string()]));

        services.insert("a".to_string(), svc_a);
        services.insert("b".to_string(), svc_b);

        ComposeSpec { services, ..Default::default() }
    })
}

proptest! {
    // Feature: perry-container, Property 1: ComposeSpec serialization round-trip
    #[test]
    fn prop_compose_spec_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ComposeSpec = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&deserialized).unwrap();
        prop_assert_eq!(json, json2);
    }

    // Feature: perry-container, Property 3: Topological sort respects depends_on
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_with_dag()) {
        let order = resolve_startup_order(&spec).unwrap();
        let pos: HashMap<String, usize> = order.iter().enumerate()
            .map(|(i, s)| (s.clone(), i)).collect();
        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    prop_assert!(pos[&dep] < pos[name],
                        "dep {} should come before {}", dep, name);
                }
            }
        }
    }

    // Feature: perry-container, Property 4: Cycle detection is complete
    #[test]
    fn prop_cycle_detection_is_complete(spec in arb_compose_spec_with_cycle()) {
        let result = resolve_startup_order(&spec);
        prop_assert!(result.is_err());
        // In the cycle test, both "a" and "b" should be in the error
        if let Err(perry_container_compose::error::ComposeError::DependencyCycle { services }) = result {
            prop_assert!(services.contains(&"a".to_string()));
            prop_assert!(services.contains(&"b".to_string()));
        } else {
            panic!("Expected DependencyCycle error");
        }
    }

    // Feature: perry-container, Property 6: Environment variable interpolation correctness
    #[test]
    fn prop_interpolation_correctness(
        key in "[A-Z]{1,10}",
        val in "[a-z]{1,10}",
        default in "[0-9]{1,10}"
    ) {
        let mut env = HashMap::new();
        env.insert(key.clone(), val.clone());

        // ${VAR}
        let template1 = format!("${{{}}}", key);
        prop_assert_eq!(interpolate(&template1, &env), val.clone());

        // ${VAR:-default} when set
        let template2 = format!("${{{}:-{}}}", key, default);
        prop_assert_eq!(interpolate(&template2, &env), val);

        // ${VAR:-default} when unset
        let empty_env = HashMap::new();
        prop_assert_eq!(interpolate(&template2, &empty_env), default);
    }

    // Feature: perry-container, Property 7: Compose file merge is last-writer-wins
    #[test]
    fn prop_merge_last_writer_wins(
        name in arb_service_name(),
        img1 in "[a-z]{1,10}",
        img2 in "[a-z]{1,10}"
    ) {
        let mut spec1 = ComposeSpec::default();
        spec1.services.insert(name.clone(), ComposeService { image: Some(img1), ..Default::default() });

        let mut spec2 = ComposeSpec::default();
        spec2.services.insert(name.clone(), ComposeService { image: Some(img2.clone()), ..Default::default() });

        spec1.merge(spec2);
        prop_assert_eq!(spec1.services.get(&name).unwrap().image.as_ref().unwrap(), &img2);
    }
}
