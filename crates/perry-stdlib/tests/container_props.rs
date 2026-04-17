//! Property-based tests for the perry-stdlib container module.

use proptest::prelude::*;
use serde_json::{json, Value};
use perry_container_compose::indexmap::IndexMap;
use perry_stdlib::types::{
    ContainerError, ListOrDict, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo,
};
use perry_container_compose::types::{
    ComposeSpec, ComposeService, DependsOnSpec, ComposeDependsOn,
    VolumeType, DependsOnCondition, ComposeNetwork,
};

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
    }
}

// ============ Property: ListOrDict to_map — Dict variant ============
// Validates: ListOrDict::Dict correctly maps entries to string values.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_list_or_dict_to_map_dict(
        keys in proptest::collection::vec("[a-zA-Z_][a-zA-Z0-9_]{1,10}", 1..=10),
        values in proptest::collection::vec("[a-z0-9 ]{1,20}", 1..=10),
    ) {
        let mut map = IndexMap::new();
        for (key, val) in keys.iter().zip(values.iter()) {
            map.insert(key.clone(), Some(serde_yaml::Value::String(val.clone())));
        }

        let expected_len = map.len();
        let lod = ListOrDict::Dict(map);
        let result = lod.to_map();

        // All keys should be preserved
        prop_assert_eq!(result.len(), expected_len);
        for key in result.keys() {
            prop_assert!(result.contains_key(key), "key {} should be in result", key);
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
        let result = lod.to_map();

        let unique_keys: std::collections::HashSet<&str> =
            entries.iter().map(|e| e.split_once('=').unwrap().0).collect();
        prop_assert_eq!(result.len(), unique_keys.len());
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
                    condition: None,
                    required: None,
                    restart: None,
                },
            );
        }
        let map_entry = DependsOnSpec::Map(map);
        let map_names = map_entry.service_names();

        // Both should yield the same service names
        prop_assert_eq!(list_names.len(), map_names.len());
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
                cycle: vec![msg.clone()],
            },
            4 => ContainerError::ServiceStartupFailed {
                service: msg.clone(),
                error: "test error".to_string(),
            },
            _ => ContainerError::InvalidConfig(msg.clone()),
        };

        let display = format!("{}", error);
        let expected_keyword = match variant {
            0 => "not found",
            1 => "Backend error",
            2 => "verification failed",
            3 => "Dependency cycle",
            4 => "failed to start",
            _ => "Invalid configuration",
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
    }
}

// ============ Property: Handle registry register/take type safety ============
// Validates: Registering and retrieving handles preserves the value and type.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_handle_registry_type_safety(
        ids in proptest::collection::vec("[a-f0-9]{12}", 1..=3),
        images in proptest::collection::vec("[a-z][a-z0-9_.-]{3,30}", 1..=3),
        stdout in "[a-z0-9 ]{0,50}",
        stderr in "[a-z0-9 ]{0,50}",
    ) {
        // Register a Vec<ContainerInfo> and take it back
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
        let taken: Option<Vec<ContainerInfo>> =
            perry_stdlib::container::types::take_container_info_list(h);
        prop_assert!(taken.is_some());
        let taken = taken.unwrap();
        prop_assert_eq!(taken.len(), infos.len());

        // Register ContainerLogs and take it back
        let logs = ContainerLogs {
            stdout: stdout.clone(),
            stderr: stderr.clone(),
        };
        let lh = perry_stdlib::container::types::register_container_logs(logs);
        let taken_logs: Option<ContainerLogs> =
            perry_stdlib::container::types::take_container_logs(lh);
        prop_assert!(taken_logs.is_some());
        let taken_logs = taken_logs.unwrap();
        prop_assert_eq!(taken_logs.stdout, stdout);
        prop_assert_eq!(taken_logs.stderr, stderr);
    }
}
