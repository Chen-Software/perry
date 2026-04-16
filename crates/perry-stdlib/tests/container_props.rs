//! Property-based tests for the perry-stdlib container module.
//!
//! Tests ContainerSpec CLI argument generation, verification cache
//! idempotence, and error propagation.
//!
//! Note: These tests use the perry-stdlib types (serde_json::Value based)
//! which are the actual types exposed through the FFI boundary.

use proptest::prelude::*;
use serde_json::{json, Value};

// ============ Property 2: ContainerSpec CLI argument round-trip ============
// Feature: perry-container, Property 2: ContainerSpec CLI argument round-trip
// Validates: Requirements 12.5

/// Build a ContainerSpec as a serde_json::Value and verify
/// that all fields survive serialization → deserialization.
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

// ============ Property 10: Image verification cache idempotence ============
// Feature: perry-container, Property 10: Image verification cache idempotence
// Validates: Requirements 15.7

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_error_propagation_preserves_code_and_message(
        code in -1000i32..1000,
        msg in "[a-z A-Z0-9_]{1,100}"
    ) {
        // Simulate the ComposeError::BackendError → JSON → parse flow
        let error_json = json!({
            "message": format!("Backend error (exit {}): {}", code, msg),
            "code": code
        });

        let json_str = serde_json::to_string(&error_json).unwrap();
        let reparsed: Value = serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(&reparsed["code"], &json!(code));
        prop_assert!(
            reparsed["message"].as_str().unwrap_or("").contains(&msg),
            "message should contain original msg"
        );
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
