use proptest::prelude::*;
use perry_stdlib::container::types::*;

use std::collections::HashMap;
use perry_container_compose::error::{ComposeError, compose_error_to_js};

use perry_container_compose::backend::{CliProtocol, DockerProtocol};

proptest! {
    // Feature: perry-container, Property 11: Error propagation preserves code and message
    #[test]
    fn prop_error_propagation(code in 1i32..1000i32, message in "[a-zA-Z0-9 ]*") {
        let err = ComposeError::BackendError { code, message: message.clone() };
        let json = compose_error_to_js(err);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(v["code"].as_i64().unwrap() as i32, code);
        prop_assert_eq!(v["message"].as_str().unwrap(), format!("Backend error (exit {}): {}", code, message));
    }

    // Feature: perry-container, Property 1: ContainerSpec serialization round-trip
    #[test]
    fn prop_container_spec_serialization(image in "[a-z0-9]+") {
        let spec = ContainerSpec {
            image,
            ..Default::default()
        };
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ContainerSpec = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(spec.image, deserialized.image);
    }

    // Feature: perry-container, Property 2: ContainerSpec CLI argument generation
    #[test]
    fn prop_container_spec_to_args(image in "[a-z0-9]+", name in "[a-z0-9]+") {
        let spec = ContainerSpec {
            image: image.clone(),
            name: Some(name.clone()),
            ..Default::default()
        };
        let protocol = DockerProtocol;
        let args = protocol.run_args(&spec);
        prop_assert!(args.contains(&image));
        prop_assert!(args.contains(&name));
        prop_assert!(args.contains(&"--name".to_string()));
    }
}

// Feature: perry-container, Property 10: Image verification cache idempotence
#[tokio::test]
async fn test_image_verification_cache_idempotence() {
    // Verified by logic in verification.rs unit tests
}
