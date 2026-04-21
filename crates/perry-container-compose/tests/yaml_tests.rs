use perry_container_compose::yaml::{interpolate_yaml, parse_compose_yaml, load_env};
use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnCondition, VolumeType};
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;
use proptest::prelude::*;
use indexmap::IndexMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: -
#[test]
fn test_interpolation() {
    let mut env = HashMap::new();
    env.insert("VAR".to_string(), "val".to_string());
    env.insert("EMPTY".to_string(), "".to_string());

    assert_eq!(interpolate_yaml("${VAR}", &env), "val");
    assert_eq!(interpolate_yaml("${VAR:-default}", &env), "val");
    assert_eq!(interpolate_yaml("${MISSING:-default}", &env), "default");
    assert_eq!(interpolate_yaml("${EMPTY:-default}", &env), "default");
    assert_eq!(interpolate_yaml("${VAR:+fixed}", &env), "fixed");
    assert_eq!(interpolate_yaml("${MISSING:+fixed}", &env), "");
}

// Feature: perry-container | Layer: unit | Req: 7.9 | Property: -
#[test]
fn test_load_env() {
    let dir = tempdir().unwrap();
    let env_path = dir.path().join(".env");
    fs::write(&env_path, "KEY=VAL\n# comment\nKEY2=\"quoted value\"\n").unwrap();

    let env = load_env(dir.path(), &[]);
    assert_eq!(env.get("KEY").unwrap(), "VAL");
    assert_eq!(env.get("KEY2").unwrap(), "quoted value");
}

// Feature: perry-container | Layer: unit | Req: 7.10 | Property: -
#[test]
fn test_merge_specs() {
    let env = HashMap::new();
    let yaml1 = "
services:
  web:
    image: nginx
    environment:
      - K1=V1
";
    let yaml2 = "
services:
  web:
    image: nginx:latest
    environment:
      - K2=V2
  db:
    image: postgres
";
    let mut spec1 = parse_compose_yaml(yaml1, &env).unwrap();
    let spec2 = parse_compose_yaml(yaml2, &env).unwrap();

    spec1.merge(spec2);

    assert_eq!(spec1.services.len(), 2);
    assert_eq!(spec1.services.get("web").unwrap().image.as_ref().unwrap(), "nginx:latest");
    assert!(spec1.services.contains_key("db"));
}

// ============ Property Generators ============

prop_compose! {
    fn arb_env_template()(
        var_name in "[A-Z0-9_]{1,5}",
        default_val in "[a-z0-9]{1,5}",
        has_default in proptest::bool::ANY
    ) -> (String, String, String) {
        if has_default {
            (format!("${{{}:-{}}}", var_name, default_val), var_name, default_val)
        } else {
            (format!("${{{}}}", var_name), var_name, "".to_string())
        }
    }
}

prop_compose! {
    fn arb_service_name()(name in "[a-z][a-z0-9_-]{1,10}") -> String {
        name
    }
}

prop_compose! {
    fn arb_image()(name in "[a-z]{3,10}(:[a-z0-9.]+)?") -> String {
        name
    }
}

fn arb_compose_spec(n_services: usize) -> impl Strategy<Value = ComposeSpec> {
    proptest::collection::vec(
        (arb_service_name(), arb_image()).prop_map(|(name, image)| {
            let mut svc = ComposeService::default();
            svc.image = Some(image);
            (name, svc)
        }),
        n_services..=n_services
    ).prop_map(|services_vec| {
        let mut services = IndexMap::new();
        for (name, svc) in services_vec {
            services.insert(name, svc);
        }
        ComposeSpec { services, ..Default::default() }
    })
}

// Feature: perry-container | Layer: property | Req: 7.8 | Property: 6
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_interpolation_correctness(t in arb_env_template()) {
        let (template, var, default) = t;
        let mut env = HashMap::new();

        // Case 1: Var set in map (overrides process env)
        env.insert(var.clone(), "actual".to_string());
        let result = interpolate_yaml(&template, &env);
        prop_assert_eq!(result, "actual");

        // Case 2: Var missing from map (falls back to process env)
        env.remove(&var);
        let result2 = interpolate_yaml(&template, &env);
        let proc_val = std::env::var(&var).unwrap_or_default();

        if template.contains(":-") {
            if proc_val.is_empty() {
                prop_assert_eq!(result2, default);
            } else {
                prop_assert_eq!(result2, proc_val);
            }
        } else {
            prop_assert_eq!(result2, proc_val);
        }
    }
}

// Feature: perry-container | Layer: property | Req: 7.14 | Property: 8
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_depends_on_condition_rejects_invalid(invalid in "[a-z]{3,20}") {
        let valid_values = ["service_started", "service_healthy", "service_completed_successfully"];
        prop_assume!(!valid_values.contains(&invalid.as_str()));
        let yaml = format!("\"{}\"", invalid);
        let result = serde_yaml::from_str::<DependsOnCondition>(&yaml);
        prop_assert!(result.is_err());
    }
}

// Feature: perry-container | Layer: property | Req: 10.14 | Property: 9
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_volume_type_rejects_invalid(invalid in "[a-z]{3,20}") {
        let valid_values = ["bind", "volume", "tmpfs", "cluster", "npipe", "image"];
        prop_assume!(!valid_values.contains(&invalid.as_str()));
        let yaml = format!("\"{}\"", invalid);
        let result = serde_yaml::from_str::<VolumeType>(&yaml);
        prop_assert!(result.is_err());
    }
}

// Feature: perry-container | Layer: property | Req: 7.12 | Property: 1
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_compose_spec_json_round_trip(spec in arb_compose_spec(3)) {
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ComposeSpec = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&deserialized).unwrap();
        prop_assert_eq!(json, json2);
    }
}

// Feature: perry-container | Layer: property | Req: 7.1 | Property: 5
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_yaml_round_trip(spec in arb_compose_spec(3)) {
        let yaml = serde_yaml::to_string(&spec).unwrap();
        let env = HashMap::new();
        let reparsed = parse_compose_yaml(&yaml, &env).unwrap();
        prop_assert_eq!(reparsed.services.len(), spec.services.len());
    }
}

// Feature: perry-container | Layer: property | Req: 7.10 | Property: 7
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_merge_last_writer_wins(
        common_svc in arb_service_name(),
        img_a in arb_image(),
        img_b in arb_image(),
    ) {
        prop_assume!(img_a != img_b);
        let mut spec_a = ComposeSpec::default();
        let mut svc_a = ComposeService::default();
        svc_a.image = Some(img_a.clone());
        spec_a.services.insert(common_svc.clone(), svc_a);

        let mut spec_b = ComposeSpec::default();
        let mut svc_b = ComposeService::default();
        svc_b.image = Some(img_b.clone());
        spec_b.services.insert(common_svc.clone(), svc_b);

        spec_a.merge(spec_b);
        prop_assert_eq!(spec_a.services[&common_svc].image.as_ref().unwrap(), &img_b);
    }
}
