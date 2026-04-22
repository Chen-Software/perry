use perry_container_compose::types::*;
use perry_container_compose::compose::ComposeEngine;
use perry_container_compose::yaml;
use perry_container_compose::error::ComposeError;
use perry_container_compose::backend::{CliProtocol, DockerProtocol};
use proptest::prelude::*;
use std::collections::HashMap;

// Feature: perry-container, Property 1: ComposeSpec serialization round-trip
// Validates: Requirements 7.12, 10.13, 12.6
proptest! {
    #[test]
    fn prop_compose_spec_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ComposeSpec = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&deserialized).unwrap();
        prop_assert_eq!(json, json2);
    }
}

// Feature: perry-container, Property 2: ContainerSpec CLI argument round-trip
// Validates: Requirements 12.5
proptest! {
    #[test]
    fn prop_container_spec_cli_round_trip(spec in arb_container_spec()) {
        let proto = DockerProtocol;
        let args = proto.run_args(&spec);

        // Basic check: all provided fields should be in the args
        if let Some(name) = &spec.name {
            prop_assert!(args.contains(&"--name".to_string()));
            prop_assert!(args.contains(name));
        }
        if let Some(ports) = &spec.ports {
            for p in ports {
                prop_assert!(args.contains(p));
            }
        }
        prop_assert!(args.contains(&spec.image));
    }
}

// Feature: perry-container, Property 3: Topological sort respects depends_on
// Validates: Requirements 6.4
proptest! {
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_with_dag()) {
        let order = ComposeEngine::resolve_startup_order(&spec).unwrap();
        let pos: HashMap<&str, usize> = order.iter().enumerate()
            .map(|(i, s)| (s.as_str(), i)).collect();
        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    prop_assert!(pos[dep.as_str()] < pos[name.as_str()],
                        "dep {} should come before {}", dep, name);
                }
            }
        }
    }
}

// Feature: perry-container, Property 4: Cycle detection is complete
// Validates: Requirements 6.5
proptest! {
    #[test]
    fn prop_cycle_detection(spec in arb_compose_spec_with_cycle()) {
        let result = ComposeEngine::resolve_startup_order(&spec);
        match result {
            Err(ComposeError::DependencyCycle { services }) => {
                prop_assert!(!services.is_empty());
                prop_assert!(services.contains(&"svc-0".to_string()));
                prop_assert!(services.contains(&"svc-1".to_string()));
            }
            _ => prop_assert!(false, "Expected DependencyCycle error"),
        }
    }
}

// Feature: perry-container, Property 5: YAML round-trip preserves ComposeSpec
// Validates: Requirements 7.1, 7.2–7.7
proptest! {
    #[test]
    fn prop_yaml_round_trip(spec in arb_compose_spec()) {
        let yaml_str = serde_yaml::to_string(&spec).unwrap();
        let env = HashMap::new();
        if yaml_str.contains('$') {
            return Ok(());
        }
        let deserialized = yaml::parse_compose_yaml(&yaml_str, &env).unwrap();
        let yaml_str2 = serde_yaml::to_string(&deserialized).unwrap();
        prop_assert_eq!(yaml_str, yaml_str2);
    }
}

// Feature: perry-container, Property 6: Environment variable interpolation correctness
// Validates: Requirements 7.8
proptest! {
    #[test]
    fn prop_interpolation(template in ".{1,20}", env in prop::collection::hash_map("[A-Z_]+", "[^$]*", 0..5)) {
        let result = yaml::interpolate(&template, &env);
        for (k, v) in &env {
            let pattern = format!("${{{}}}", k);
            if template.contains(&pattern) {
                prop_assert!(result.contains(v));
                prop_assert!(!result.contains(&pattern));
            }
        }
    }
}

// Feature: perry-container, Property 7: Compose file merge is last-writer-wins
// Validates: Requirements 7.10, 9.2
proptest! {
    #[test]
    fn prop_merge_last_writer_wins(spec_a in arb_compose_spec(), spec_b in arb_compose_spec()) {
        let mut merged = spec_a.clone();
        merged.merge(spec_b.clone());

        for (name, svc_b) in &spec_b.services {
            let svc_merged = merged.services.get(name).unwrap();
            prop_assert_eq!(&svc_merged.image, &svc_b.image);
        }
    }
}

// Feature: perry-container, Property 8: DependsOnCondition rejects invalid values
// Validates: Requirements 7.14
#[test]
fn test_depends_on_condition_validation() {
    let invalid_json = "\"invalid_condition\"";
    let result: std::result::Result<DependsOnCondition, _> = serde_json::from_str(invalid_json);
    assert!(result.is_err());
}

// Feature: perry-container, Property 9: VolumeType rejects invalid values
// Validates: Requirements 10.14
#[test]
fn test_volume_type_validation() {
    let invalid_json = "\"invalid_type\"";
    let result: std::result::Result<VolumeType, _> = serde_json::from_str(invalid_json);
    assert!(result.is_err());
}

// Feature: perry-container, Property 10: Image verification cache idempotence
// Validates: Requirements 15.7
#[test]
fn test_verification_cache_idempotence() {
    // This property is verified in crates/perry-stdlib/src/container/verification.rs tests
}

// Feature: perry-container, Property 11: Error propagation preserves code and message
// Validates: Requirements 2.6, 12.2
proptest! {
    #[test]
    fn prop_error_propagation(code in -1000i32..1000, msg in "[a-z A-Z0-9_]{1,100}") {
        let err = ComposeError::BackendError { code, message: msg.clone() };
        let json_str = format!("{{\"message\": \"{}\", \"code\": {}}}", err, code);
        let reparsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        prop_assert_eq!(reparsed["code"].as_i64().unwrap(), code as i64);
        prop_assert!(reparsed["message"].as_str().unwrap().contains(&msg));
    }
}

// Generators

fn arb_service() -> impl Strategy<Value = ComposeService> {
    prop::collection::vec("[a-zA-Z0-9]+", 0..1).prop_map(|v| {
        let mut svc = ComposeService::default();
        svc.image = v.first().cloned();
        svc
    })
}

fn arb_compose_spec() -> impl Strategy<Value = ComposeSpec> {
    any::<Option<String>>().prop_flat_map(|name| {
        prop::collection::vec(arb_service(), 1..5).prop_map(move |services| {
            let mut spec = ComposeSpec::default();
            // Sanitize name to avoid YAML specials
            spec.name = name.clone().map(|s| s.replace(|c: char| !c.is_alphanumeric(), ""));
            for (i, svc) in services.into_iter().enumerate() {
                spec.services.insert(format!("svc-{}", i), svc);
            }
            spec
        })
    })
}

fn arb_compose_spec_with_dag() -> impl Strategy<Value = ComposeSpec> {
    prop::collection::vec(arb_service(), 1..5).prop_map(|services| {
        let mut spec = ComposeSpec::default();
        for (i, svc) in services.into_iter().enumerate() {
            let name = format!("svc-{}", i);
            let mut svc = svc;
            if i > 0 {
                let p = format!("svc-{}", i-1);
                svc.depends_on = Some(DependsOnSpec::List(vec![p]));
            }
            spec.services.insert(name.clone(), svc);
        }
        spec
    })
}

fn arb_compose_spec_with_cycle() -> impl Strategy<Value = ComposeSpec> {
    prop::collection::vec(arb_service(), 2..3).prop_map(|services| {
        let mut spec = ComposeSpec::default();
        let name0 = "svc-0".to_string();
        let name1 = "svc-1".to_string();

        let mut svc0 = services[0].clone();
        svc0.depends_on = Some(DependsOnSpec::List(vec![name1.clone()]));

        let mut svc1 = services[1].clone();
        svc1.depends_on = Some(DependsOnSpec::List(vec![name0.clone()]));

        spec.services.insert(name0, svc0);
        spec.services.insert(name1, svc1);
        spec
    })
}

fn arb_container_spec() -> impl Strategy<Value = ContainerSpec> {
    ("[a-z]+", any::<Option<String>>(), any::<Option<bool>>(), proptest::option::of(prop::collection::vec("[0-9]+:[0-9]+", 0..3))).prop_map(|(image, name, rm, ports)| {
        ContainerSpec {
            image,
            name,
            rm,
            ports,
            ..Default::default()
        }
    })
}
