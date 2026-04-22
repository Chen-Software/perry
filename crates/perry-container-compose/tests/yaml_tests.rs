//! Tests for the `yaml` module.
//!
//! Validates YAML parsing, interpolation, and merging.

use perry_container_compose::yaml::{interpolate, parse_compose_yaml};
use perry_container_compose::types::{ComposeSpec, ComposeService};
use std::collections::HashMap;
use proptest::prelude::*;
use indexmap::IndexMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// ============ Generators ============

prop_compose! {
    fn arb_env_name()(name in "[A-Z][A-Z0-9_]{1,10}") -> String {
        name
    }
}

prop_compose! {
    fn arb_env_map()(map in proptest::collection::hash_map(arb_env_name(), "[a-z0-9]{1,10}", 1..10)) -> HashMap<String, String> {
        map
    }
}

fn arb_env_template() -> impl Strategy<Value = (String, HashMap<String, String>, String)> {
    arb_env_map().prop_flat_map(|env| {
        let env_keys: Vec<String> = env.keys().cloned().collect();
        let env_clone = env.clone();
        proptest::collection::vec(
            prop_oneof![
                "[a-z ]+".prop_map(|s| (s, false)), // Literal
                proptest::sample::select(env_keys).prop_map(|k| (format!("${{{}}}", k), true))
            ],
            1..5
        ).prop_map(move |parts| {
            let mut template = String::new();
            let mut expected = String::new();
            for (part, is_var) in parts {
                template.push_str(&part);
                if is_var {
                    let key = part.trim_start_matches("${").trim_end_matches("}");
                    expected.push_str(env_clone.get(key).unwrap());
                } else {
                    expected.push_str(&part);
                }
            }
            (template, env_clone.clone(), expected)
        })
    })
}

// ============ Property Tests ============

// Feature: perry-container | Layer: property | Req: 7.8 | Property: 6
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_env_interpolation_correctness(t in arb_env_template()) {
        let (template, env, expected) = t;
        let result = interpolate(&template, &env);
        prop_assert_eq!(result, expected);
    }
}

// Feature: perry-container | Layer: property | Req: 7.1 | Property: 5
#[test]
fn test_yaml_round_trip_basic() {
    let mut services = IndexMap::new();
    let mut svc = ComposeService::default();
    svc.image = Some("nginx:latest".to_string());
    services.insert("web".to_string(), svc);
    let spec = ComposeSpec { services, ..Default::default() };

    let yaml = serde_yaml::to_string(&spec).unwrap();
    let reparsed = parse_compose_yaml(&yaml, &HashMap::new()).expect("Should re-parse");

    assert_eq!(reparsed.services.len(), 1);
    assert_eq!(reparsed.services["web"].image, Some("nginx:latest".to_string()));
}

// Feature: perry-container | Layer: property | Req: 7.10 | Property: 7
#[test]
fn test_merge_last_writer_wins() {
    let mut spec1 = ComposeSpec::default();
    let mut svc1 = ComposeService::default();
    svc1.image = Some("nginx:1.0".to_string());
    spec1.services.insert("web".to_string(), svc1);

    let mut spec2 = ComposeSpec::default();
    let mut svc2 = ComposeService::default();
    svc2.image = Some("nginx:2.0".to_string());
    spec2.services.insert("web".to_string(), svc2);

    spec1.merge(spec2);
    assert_eq!(spec1.services["web"].image, Some("nginx:2.0".to_string()));
}

// ============ Coverage Table ============
//
// | Requirement | Test name | Layer |
// |-------------|-----------|-------|
// | 7.1         | test_yaml_round_trip_basic | unit |
// | 7.8         | prop_env_interpolation_correctness | property |
// | 7.10        | test_merge_last_writer_wins | unit |
