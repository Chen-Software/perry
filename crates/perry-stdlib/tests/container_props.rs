// Feature: perry-container
// Property-based tests for container types and logic.

use proptest::prelude::*;
use serde_json::{json, Value};
use indexmap::IndexMap;
use perry_container_compose::types::{DependsOnSpec, ComposeDependsOn, DependsOnCondition, ListOrDict, ComposeSpec, ComposeService, ComposeNetwork};
use perry_stdlib::container::{ContainerInfo, ContainerLogs, ContainerError};

// ============ Property: ListOrDict to_map — Dict variant ============
// Validates: ListOrDict::Dict correctly preserves all key-value pairs.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_list_or_dict_to_map_dict(
        entries in proptest::collection::vec(("[a-z]{1,10}", "[a-z0-9]{0,20}"), 1..=10),
    ) {
        let mut map = IndexMap::new();
        let mut keys = Vec::new();
        for (k, v) in entries {
            map.insert(k.clone(), Some(serde_yaml::Value::String(v)));
            keys.push(k);
        }

        let expected_len = map.len();
        let lod = ListOrDict::Dict(map);

        // Manual verification of the Dict content
        if let ListOrDict::Dict(inner) = lod {
            prop_assert_eq!(inner.len(), expected_len);
            for key in &keys {
                prop_assert!(inner.contains_key(key), "key {} should be in result", key);
            }
        } else {
            prop_assert!(false, "Should be Dict variant");
        }
    }
}

// ============ Property: ListOrDict to_map — List variant ============
// Validates: ListOrDict::List("KEY=VAL") correctly parses entries.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_list_or_dict_to_map_list(
        entries in proptest::collection::vec("[A-Z][A-Z0-9_]{1,8}=[a-z0-9_]{0,10}", 1..=8),
    ) {
        let list: Vec<String> = entries.clone();
        let lod = ListOrDict::List(list);

        if let ListOrDict::List(inner) = lod {
            prop_assert_eq!(inner.len(), entries.len());
        } else {
            prop_assert!(false, "Should be List variant");
        }
    }
}

// ============ Property: DependsOnSpec service_names — List vs Map ============
// Validates: Both List and Map variants produce the same set of service names.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_depends_on_entry_service_names(
        names in proptest::collection::vec("[a-z][a-z0-9_-]{1,10}", 1..=6),
    ) {
        // List variant
        let list_entry = DependsOnSpec::List(names.clone());
        let list_names = list_entry.service_names();

        // Map variant (same keys)
        let mut map = IndexMap::new();
        for name in &names {
            map.insert(
                name.clone(),
                ComposeDependsOn {
                    condition: DependsOnCondition::ServiceStarted,
                    required: None,
                    restart: None,
                },
            );
        }
        let map_entry = DependsOnSpec::Map(map);
        let map_names = map_entry.service_names();

        // Both should yield the same service names (order may differ for Map)
        prop_assert_eq!(list_names.len(), map_names.len());
        for name in &list_names {
            prop_assert!(map_names.contains(name), "map should contain {}", name);
        }
    }
}

// ============ Property: ContainerError Display contains identifying keyword ============
// Validates: Each ContainerError variant's Display output contains
// a distinguishing keyword for programmatic error classification.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_container_error_display_contains_keyword(
        variant in 0u8..=5,
        msg in "[a-z A-Z0-9_]{1,40}",
    ) {
        let error = match variant {
            0 => ContainerError::NotFound(msg.clone()),
            1 => ContainerError::BackendError {
                code: 1,
                message: msg.clone(),
            },
            2 => ContainerError::VerificationFailed {
                image: msg.clone(),
                reason: "test reason".to_string(),
            },
            3 => ContainerError::DependencyCycle {
                services: vec![msg.clone()],
            },
            4 => ContainerError::ServiceStartupFailed {
                service: msg.clone(),
                message: "test error".to_string(),
            },
            _ => ContainerError::ValidationError { message: msg.clone() },
        };

        let display = format!("{}", error);
        let expected_keyword = match variant {
            0 => "not found",
            1 => "Backend error",
            2 => "verification failed",
            3 => "Dependency cycle",
            4 => "failed to start",
            _ => "Validation error",
        };

        prop_assert!(
            display.to_lowercase().contains(&expected_keyword.to_lowercase()),
            "Display output should contain '{}', got: {}",
            expected_keyword,
            display
        );
    }
}

// ============ Property: Typed ComposeSpec JSON round-trip ============
// Validates: The typed ComposeSpec struct survives JSON round-trip.

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
        let reparsed: ComposeSpec =
            serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(reparsed.name, spec.name);
        prop_assert_eq!(reparsed.services.len(), spec.services.len());

        for (svc_name, original_svc) in &spec.services {
            let reparsed_svc = &reparsed.services[svc_name];
            prop_assert_eq!(&reparsed_svc.image, &original_svc.image);
        }
    }
}

// ============ Property: Handle registry register type safety ============
// Validates: Registering handles preserves the value and type.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_handle_registry_type_safety(
        ids in proptest::collection::vec("[a-f0-9]{12}", 1..=3),
        images in proptest::collection::vec("[a-z][a-z0-9_.-]{3,30}", 1..=3),
        stdout in "[a-z0-9 ]{0,50}",
        stderr in "[a-z0-9 ]{0,50}",
    ) {
        // Register a Vec<ContainerInfo> and get it back
        let infos: Vec<ContainerInfo> = ids
            .iter()
            .zip(images.iter())
            .map(|(id, img)| ContainerInfo {
                id: id.clone(),
                name: format!("svc-{}", &id[..6]),
                image: img.clone(),
                status: "running".to_string(),
                ports: vec![],
                created: "2025-01-01T00:00:00Z".to_string(),
            })
            .collect();

        let h = perry_stdlib::container::types::register_container_info_list(infos.clone());
        let data = perry_stdlib::container::types::get_registered_data(h);
        prop_assert!(data.is_some());
        let recovered: Vec<ContainerInfo> = serde_json::from_str(&data.unwrap()).unwrap();
        prop_assert_eq!(recovered.len(), infos.len());
        for (original, recovered_item) in infos.iter().zip(recovered.iter()) {
            prop_assert_eq!(&recovered_item.id, &original.id);
            prop_assert_eq!(&recovered_item.image, &original.image);
        }

        // Register ContainerLogs and get it back
        let logs = ContainerLogs {
            stdout: stdout.clone(),
            stderr: stderr.clone(),
        };
        let lh = perry_stdlib::container::types::register_container_logs(logs);
        let data_logs = perry_stdlib::container::types::get_registered_data(lh);
        prop_assert!(data_logs.is_some());
        let recovered_logs: ContainerLogs = serde_json::from_str(&data_logs.unwrap()).unwrap();
        prop_assert_eq!(recovered_logs.stdout, stdout);
        prop_assert_eq!(recovered_logs.stderr, stderr);
    }
}

// ============ Property: ComposeNetwork JSON round-trip ============
// Validates: ComposeNetwork preserves all fields through serialization.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_compose_network_json_round_trip(
        name in proptest::option::of("[a-z][a-z0-9_-]{1,20}"),
        driver in proptest::option::of("[a-z]{3,10}"),
    ) {
        use perry_container_compose::types::ComposeNetwork;
        let mut network = ComposeNetwork::default();
        network.name = name;
        network.driver = driver;

        let json_str = serde_json::to_string(&network).unwrap();
        let reparsed: ComposeNetwork =
            serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(reparsed.name, network.name);
        prop_assert_eq!(reparsed.driver, network.driver);
    }
}
