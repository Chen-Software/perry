//! Property-based tests for the perry-stdlib container module.

use proptest::prelude::*;
use serde_json::{json, Value};
use perry_container_compose::indexmap::IndexMap;
use perry_stdlib::container::types::*;

// ============ Property 2: ContainerSpec CLI argument round-trip ============
// Feature: perry-container, Property 2: ContainerSpec CLI argument round-trip
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
        let mut env_obj = std::collections::HashMap::new();
        for key in &env_keys {
            env_obj.insert(key.clone(), format!("val_{}", key));
        }

        let spec = ContainerSpec {
            image: image.clone(),
            name: name.clone(),
            ports: ports.clone(),
            env: Some(env_obj),
            cmd: Some(vec!["echo".to_string(), "hello".to_string()]),
            rm: Some(true),
            ..Default::default()
        };

        let spec_json = serde_json::to_string(&spec).unwrap();
        let reparsed: ContainerSpec = serde_json::from_str(&spec_json).unwrap();

        prop_assert_eq!(&reparsed.image, &spec.image);
        prop_assert_eq!(&reparsed.name, &spec.name);
        prop_assert_eq!(&reparsed.ports, &spec.ports);
        prop_assert_eq!(&reparsed.env, &spec.env);
        prop_assert_eq!(&reparsed.cmd, &spec.cmd);
        prop_assert_eq!(&reparsed.rm, &spec.rm);
    }
}

#[test]
fn test_error_code_mapping() {
    use perry_container_compose::error::{ComposeError, compose_error_to_js};

    let err = ComposeError::NotFound("foo".into());
    let js_err = compose_error_to_js(&err);
    assert!(js_err.contains("\"code\":404"));

    let err = ComposeError::DependencyCycle { services: vec!["a".into()] };
    let js_err = compose_error_to_js(&err);
    assert!(js_err.contains("\"code\":422"));

    let err = ComposeError::validation("bad");
    let js_err = compose_error_to_js(&err);
    assert!(js_err.contains("\"code\":400"));
}
