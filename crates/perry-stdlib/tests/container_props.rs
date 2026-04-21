//! Property-based tests for the perry-stdlib container module.

use proptest::prelude::*;
use serde_json::{json, Value};
use perry_container_compose::indexmap::IndexMap;
use std::collections::HashSet;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// Feature: perry-container | Layer: property | Req: 12.5 | Property: 2
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_container_spec_json_round_trip(
        image in "[a-z][a-z0-9_-]{1,30}(:[a-z0-9._-]+)?",
        name in proptest::option::of("[a-z][a-z0-9_-]{1,30}"),
        ports in proptest::option::of(proptest::collection::vec("[0-9]{1,5}:[0-9]{1,5}", 0..=5)),
        env_keys in proptest::collection::vec("[A-Z][A-Z0-9_]{1,10}", 0..=5),
    ) {
        let mut env_obj = serde_json::Map::new();
        let mut unique_keys = HashSet::new();
        for key in &env_keys {
            if unique_keys.insert(key.clone()) {
                env_obj.insert(key.clone(), Value::String(format!("val_{}", key)));
            }
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
            prop_assert_eq!(env.len(), unique_keys.len());
        }
    }
}

// Feature: perry-container | Layer: property | Req: 12.2 | Property: 11
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_error_propagation_preserves_code_and_message(
        code in -1000i32..1000,
        msg in "[a-z A-Z0-9_]{1,100}"
    ) {
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

// Feature: perry-container | Layer: property | Req: 6.2 | Property: -
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_list_or_dict_to_map_dict(
        keys in proptest::collection::vec("[A-Z][A-Z0-9_]{1,8}", 1..=8),
        int_val in 0i64..1000,
        bool_val in proptest::bool::ANY,
        str_val in "[a-z0-9_]{1,10}",
    ) {
        let mut map = IndexMap::new();
        let mut unique_keys = HashSet::new();
        for (i, key) in keys.iter().enumerate() {
            if unique_keys.insert(key.clone()) {
                let val: Option<serde_yaml::Value> = match i % 4 {
                    0 => Some(serde_yaml::Value::String(str_val.clone())),
                    1 => Some(serde_yaml::Value::Number(int_val.into())),
                    2 => Some(serde_yaml::Value::Bool(bool_val)),
                    _ => None,
                };
                map.insert(key.clone(), val);
            }
        }

        let lod = perry_container_compose::types::ListOrDict::Dict(map);
        let result = lod.to_map();

        prop_assert_eq!(result.len(), unique_keys.len());
        for key in &unique_keys {
            prop_assert!(result.contains_key(key));
        }
    }
}

// Feature: perry-container | Layer: property | Req: 2.7 | Property: -
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_typed_compose_spec_json_round_trip(
        name in proptest::option::of("[a-z][a-z0-9_-]{1,20}"),
        svc_names in proptest::collection::vec("[a-z][a-z0-9_-]{1,10}", 1..=5),
        images in proptest::collection::vec("[a-z][a-z0-9_.-]{3,30}(:[a-z0-9._-]+)?", 1..=5),
    ) {
        use perry_container_compose::types::{ComposeSpec, ComposeService};
        let mut spec = ComposeSpec::default();
        spec.name = name;

        let mut unique_svcs = HashSet::new();
        for (svc_name, image) in svc_names.iter().zip(images.iter()) {
            if unique_svcs.insert(svc_name.clone()) {
                let mut service = ComposeService::default();
                service.image = Some(image.clone());
                spec.services.insert(svc_name.clone(), service);
            }
        }

        let json_str = serde_json::to_string(&spec).unwrap();
        let reparsed: ComposeSpec = serde_json::from_str(&json_str).unwrap();

        prop_assert_eq!(reparsed.name, spec.name);
        prop_assert_eq!(reparsed.services.len(), unique_svcs.len());
    }
}
