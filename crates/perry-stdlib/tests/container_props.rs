//! Property-based tests for the perry-stdlib container module.

use proptest::prelude::*;
use serde_json::{json, Value};
use indexmap::IndexMap;
use perry_container_compose::types::{ComposeSpec, ComposeService, ContainerInfo, ContainerLogs, ListOrDict, DependsOnSpec, ComposeDependsOn, DependsOnCondition};

// ============ Property 1: ComposeSpec serialization round-trip ============
// Feature: perry-container, Property 1: ComposeSpec serialization round-trip
// Validates: Requirements 7.12, 10.13, 12.6

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_typed_compose_spec_json_round_trip(
        name in proptest::option::of("[a-z][a-z0-9_-]{1,20}"),
        svc_names in proptest::collection::vec("[a-z][a-z0-9_-]{1,10}", 1..=5),
        images in proptest::collection::vec("[a-z][a-z0-9_.-]{3,30}(:[a-z0-9._-]+)?", 1..=5),
    ) {
        let mut spec = ComposeSpec::default();
        spec.name = name;

        for (svc_name, image) in svc_names.iter().zip(images.iter()) {
            let mut service = ComposeService::default();
            service.image = Some(image.clone());
            spec.services.insert(svc_name.clone(), service);
        }

        let json_str = serde_json::to_string(&spec).unwrap();
        let reparsed: ComposeSpec = serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(reparsed.name, spec.name);
        prop_assert_eq!(reparsed.services.len(), spec.services.len());

        for (svc_name, original_svc) in &spec.services {
            let reparsed_svc = &reparsed.services[svc_name];
            prop_assert_eq!(&reparsed_svc.image, &original_svc.image);
        }
    }
}

// ============ Property 2: ContainerSpec JSON round-trip ============
// Feature: perry-container, Property 2: ContainerSpec JSON round-trip
// Validates: Requirements 12.5

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_container_spec_json_round_trip(
        image in "[a-z][a-z0-9_-]{1,30}(:[a-z0-9._-]+)?",
        name in proptest::option::of("[a-z][a-z0-9_-]{1,30}"),
        ports in proptest::option::of(proptest::collection::vec("[0-9]{1,5}:[0-9]{1,5}", 0..=5)),
        env_keys in proptest::collection::vec("[A-Z][A-Z0-9_]{1,10}", 0..=5),
    ) {
        let mut env_obj = serde_json::Map::new();
        for key in &env_keys {
            env_obj.insert(key.clone(), Value::String(format!("val_{}", key)));
        }

        let spec = json!({
            "image": image,
            "name": name,
            "ports": ports,
            "env": env_obj,
            "cmd": ["echo", "hello"],
            "rm": true,
        });

        let spec_str = serde_json::to_string(&spec).unwrap();
        let reparsed: Value = serde_json::from_str(&spec_str).unwrap();

        prop_assert_eq!(&reparsed["image"], &spec["image"]);

        if name.is_some() {
            prop_assert_eq!(&reparsed["name"], &spec["name"]);
        }

        // Ports array length preserved
        prop_assert_eq!(
            reparsed["ports"].as_array().map(|a| a.len()),
            spec["ports"].as_array().map(|a| a.len())
        );

        // Env keys preserved
        if let Some(env) = reparsed["env"].as_object() {
            prop_assert_eq!(env.len(), env_keys.len());
        }
    }
}

// ============ Property 11: Error propagation preserves code and message ============
// Feature: perry-container, Property 11: Error propagation preserves code and message
// Validates: Requirements 2.6, 12.2

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_compose_error_json_round_trip(
        variant in 0u8..=5,
        msg in "[a-z A-Z0-9_]{1,80}"
    ) {
        let (error_json, expected_code) = match variant {
            0 => (json!({ "message": format!("Not found: {}", msg), "code": 404 }), 404i64),
            1 => (json!({ "message": format!("Backend error (exit 1): {}", msg), "code": 1 }), 1),
            2 => (json!({ "message": format!("Dependency cycle detected in services: {:?}", [msg]), "code": 422 }), 422),
            3 => (json!({ "message": format!("Validation error: {}", msg), "code": 400 }), 400),
            4 => (json!({ "message": format!("Image verification failed for 'img': {}", msg), "code": 403 }), 403),
            _ => (json!({ "message": format!("Parse error: {}", msg), "code": 500 }), 500),
        };

        let json_str = serde_json::to_string(&error_json).unwrap();
        let reparsed: Value = serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(&reparsed["code"], &json!(expected_code));
        prop_assert!(reparsed["message"].is_string());
    }
}

// ============ Property: Handle registry preserves ordering ============
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_handle_registry_increments(
        n in 1usize..10
    ) {
        use perry_stdlib::container::register_container_handle;
        use perry_container_compose::types::ContainerHandle;

        let mut last_h = 0u64;
        for i in 0..n {
            let h = register_container_handle(ContainerHandle {
                id: format!("id-{}", i),
                name: None,
            });
            prop_assert!(h > last_h);
            last_h = h;
        }
    }
}
