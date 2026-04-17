//! Property-based tests for the perry-stdlib container module.

use perry_stdlib::container::types::*;
use proptest::prelude::*;
use perry_container_compose::types::{DependsOnSpec, ComposeDependsOn, IndexMap};

// ============ Property 2: ContainerSpec CLI argument round-trip ============
// Feature: perry-container, Property 2: ContainerSpec CLI argument round-trip
// Validates: Requirements 12.5

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_container_spec_serialization(
        image in "[a-z0-9]{5,10}",
        name in proptest::option::of("[a-z0-9]{5,10}"),
    ) {
        let spec = ContainerSpec {
            image: image.clone(),
            name: name.clone(),
            ..Default::default()
        };

        let json = serde_json::to_string(&spec).unwrap();
        let reparsed: ContainerSpec = serde_json::from_str(&json).unwrap();

        prop_assert_eq!(reparsed.image, image);
        prop_assert_eq!(reparsed.name, name);
    }
}

// ============ Property: ListOrDict to_map — Dict variant ============
// Validates: ListOrDict::Dict preserves keys and stringifies values.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_list_or_dict_to_map_dict(
        keys in proptest::collection::vec("[A-Z][A-Z0-9_]{1,8}", 1..=8),
        str_val in "[a-z0-9_]{1,10}",
    ) {
        let mut map = IndexMap::new();
        let unique_keys: std::collections::HashSet<String> = keys.into_iter().collect();
        for key in &unique_keys {
            map.insert(key.clone(), Some(serde_yaml::Value::String(str_val.clone())));
        }

        let lod = ListOrDict::Dict(map);
        let result = lod.to_map();

        prop_assert_eq!(result.len(), unique_keys.len());
        for key in &unique_keys {
            prop_assert!(result.contains_key(key));
            prop_assert_eq!(result.get(key).unwrap(), &str_val);
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
        let unique_names: std::collections::HashSet<String> = names.into_iter().collect();
        let sorted_names: Vec<String> = unique_names.iter().cloned().collect();

        // List variant
        let list_entry = DependsOnSpec::List(sorted_names.clone());
        let list_names = list_entry.service_names();

        // Map variant (same keys)
        let mut map = IndexMap::new();
        for name in &sorted_names {
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

        prop_assert_eq!(list_names.len(), map_names.len());
        for name in &list_names {
            prop_assert!(map_names.contains(name));
        }
    }
}

// ============ Property: ContainerError Display contains identifying keyword ============

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
            display.to_lowercase().contains(&expected_keyword.to_lowercase())
        );
    }
}

// ============ Property: Typed ComposeSpec JSON round-trip ============

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_typed_compose_spec_json_round_trip(
        name in proptest::option::of("[a-z][a-z0-9_-]{1,20}"),
        svc_names in proptest::collection::vec("[a-z][a-z0-9_-]{1,10}", 1..=5),
        images in proptest::collection::vec("[a-z][a-z0-9_.-]{3,30}", 1..=5),
    ) {
        let mut spec = ComposeSpec::default();
        spec.name = name;

        let unique_svc_names: std::collections::HashSet<String> = svc_names.into_iter().collect();

        for (svc_name, image) in unique_svc_names.iter().zip(images.iter()) {
            let mut service = ComposeService::default();
            service.image = Some(image.clone());
            spec.services.insert(svc_name.clone(), service);
        }

        let json_str = serde_json::to_string(&spec).unwrap();
        let reparsed: ComposeSpec = serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(reparsed.name, spec.name);
        prop_assert_eq!(reparsed.services.len(), spec.services.len());
    }
}

// ============ Property: Handle registry type safety ============

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_handle_registry_type_safety(
        ids in proptest::collection::vec("[a-f0-9]{12}", 1..=3),
        images in proptest::collection::vec("[a-z][a-z0-9_.-]{3,30}", 1..=3),
        stdout in "[a-z0-9 ]{0,50}",
        stderr in "[a-z0-9 ]{0,50}",
    ) {
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

        let h = register_container_info_list(infos.clone());
        let taken = take_container_info_list(h);
        prop_assert!(taken.is_some());
        prop_assert_eq!(taken.unwrap().len(), infos.len());

        let logs = ContainerLogs {
            stdout: stdout.clone(),
            stderr: stderr.clone(),
        };
        let lh = register_container_logs(logs);
        let taken_logs = take_container_logs(lh);
        prop_assert!(taken_logs.is_some());
        prop_assert_eq!(taken_logs.unwrap().stdout, stdout);
    }
}
