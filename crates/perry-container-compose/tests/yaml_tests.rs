// Feature: perry-container | Layer: property | Req: 7.8 | Property: 6

use proptest::prelude::*;
use perry_container_compose::yaml::*;
use std::collections::HashMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// =============================================================================
// Required Generators
// =============================================================================

prop_compose! {
    pub fn arb_service_name()(name in "[a-z0-9_-]{1,64}") -> String { name }
}

prop_compose! {
    pub fn arb_image_ref()(repo in "[a-z0-9/._-]{1,128}", tag in proptest::option::of("[a-z0-9._-]{1,32}")) -> String {
        match tag {
            Some(t) => format!("{}:{}", repo, t),
            None => repo,
        }
    }
}

prop_compose! {
    pub fn arb_port_spec()(
        is_long in any::<bool>(),
        h in 1u16..65535,
        c in 1u16..65535
    ) -> perry_container_compose::types::PortSpec {
        use perry_container_compose::types::{PortSpec, ComposeServicePort};
        if is_long {
            PortSpec::Long(ComposeServicePort {
                target: serde_yaml::Value::Number(c.into()),
                published: Some(serde_yaml::Value::Number(h.into())),
                ..Default::default()
            })
        } else {
            PortSpec::Short(serde_yaml::Value::String(format!("{}:{}", h, c)))
        }
    }
}

prop_compose! {
    pub fn arb_list_or_dict()(
        is_dict in any::<bool>(),
        keys in proptest::collection::vec("[a-zA-Z0-9_]{1,32}", 0..10),
        values in proptest::collection::vec("[a-zA-Z0-9_]{0,64}", 0..10)
    ) -> perry_container_compose::types::ListOrDict {
        use perry_container_compose::types::ListOrDict;
        if is_dict {
            let mut map = indexmap::IndexMap::new();
            for (k, v) in keys.into_iter().zip(values.into_iter()) {
                map.insert(k, Some(serde_yaml::Value::String(v)));
            }
            ListOrDict::Dict(map)
        } else {
            ListOrDict::List(keys.into_iter().zip(values.into_iter()).map(|(k, v)| format!("{}={}", k, v)).collect())
        }
    }
}

prop_compose! {
    pub fn arb_depends_on_spec()(
        is_map in any::<bool>(),
        services in proptest::collection::vec(arb_service_name(), 1..5)
    ) -> perry_container_compose::types::DependsOnSpec {
        use perry_container_compose::types::{DependsOnSpec, ComposeDependsOn, DependsOnCondition};
        if is_map {
            let mut map = indexmap::IndexMap::new();
            for s in services {
                map.insert(s, ComposeDependsOn {
                    condition: Some(DependsOnCondition::ServiceStarted),
                    ..Default::default()
                });
            }
            DependsOnSpec::Map(map)
        } else {
            DependsOnSpec::List(services)
        }
    }
}

prop_compose! {
    pub fn arb_compose_service()(
        image in proptest::option::of(arb_image_ref()),
        env in proptest::option::of(arb_list_or_dict()),
        ports in proptest::option::of(proptest::collection::vec(arb_port_spec(), 0..3)),
        deps in proptest::option::of(arb_depends_on_spec())
    ) -> perry_container_compose::types::ComposeService {
        perry_container_compose::types::ComposeService {
            image,
            environment: env,
            ports,
            depends_on: deps,
            ..Default::default()
        }
    }
}

prop_compose! {
    pub fn arb_compose_spec()(
        name in proptest::option::of("[a-z0-9_-]{1,32}"),
        service_names in proptest::collection::vec(arb_service_name(), 1..5)
    ) -> perry_container_compose::types::ComposeSpec {
        use perry_container_compose::types::{ComposeSpec, ComposeService};
        let mut services = indexmap::IndexMap::new();
        for s in service_names {
            services.insert(s, ComposeService::default());
        }
        ComposeSpec { name, services, ..Default::default() }
    }
}

prop_compose! {
    pub fn arb_compose_spec_dag()(
        service_names in proptest::collection::vec(arb_service_name(), 2..6)
    ) -> perry_container_compose::types::ComposeSpec {
        use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec};
        let mut services = indexmap::IndexMap::new();
        let mut names_vec: Vec<String> = Vec::new();
        for name in service_names {
            let mut svc = ComposeService::default();
            if !names_vec.is_empty() {
                let dep = names_vec[0].clone();
                svc.depends_on = Some(DependsOnSpec::List(vec![dep]));
            }
            services.insert(name.clone(), svc);
            names_vec.push(name);
        }
        ComposeSpec { services, ..Default::default() }
    }
}

prop_compose! {
    pub fn arb_compose_spec_cycle()(
        mut spec in arb_compose_spec_dag()
    ) -> perry_container_compose::types::ComposeSpec {
        use perry_container_compose::types::DependsOnSpec;
        let names: Vec<String> = spec.services.keys().cloned().collect();
        let first = names[0].clone();
        let last = names[names.len()-1].clone();
        spec.services.get_mut(&first).unwrap().depends_on = Some(DependsOnSpec::List(vec![last]));
        spec
    }
}

prop_compose! {
    pub fn arb_container_spec()(
        image in arb_image_ref(),
        name in proptest::option::of("[a-z0-9_-]{1,32}"),
        rm in proptest::option::of(any::<bool>())
    ) -> perry_container_compose::types::ContainerSpec {
        perry_container_compose::types::ContainerSpec { image, name, rm, ..Default::default() }
    }
}

prop_compose! {
    fn arb_env_key()(key in "[A-Z_][A-Z0-9_]{0,31}") -> String { key }
}

prop_compose! {
    fn arb_env_val()(val in "[a-zA-Z0-9._/-]{0,64}") -> String { val }
}

prop_compose! {
    pub fn arb_env_map()(
        map in proptest::collection::hash_map(arb_env_key(), arb_env_val(), 0..20)
    ) -> HashMap<String, String> { map }
}

prop_compose! {
    pub fn arb_env_template()(
        var in "[A-Z_][A-Z0-9_]*",
        default in proptest::option::of("[a-z0-9]*")
    ) -> String {
        match default {
            Some(d) => format!("${{{}:-{}}}", var, d),
            None => format!("${{{}}}", var),
        }
    }
}

prop_compose! {
    fn arb_env_template_parts()(
        keys in proptest::collection::vec(arb_env_key(), 1..5),
        defaults in proptest::collection::vec(proptest::option::of(arb_env_val()), 5),
        literals in proptest::collection::vec("[a-z ]{1,10}", 6)
    ) -> (String, Vec<(String, Option<String>)>) {
        let mut template = literals[0].clone();
        let mut expected_parts = Vec::new();

        for i in 0..keys.len() {
            let key = keys[i].clone();
            let default = defaults[i].clone();

            match default {
                Some(ref d) => {
                    template.push_str(&format!("${{{}:-{}}}", key, d));
                    expected_parts.push((key, Some(d.clone())));
                }
                None => {
                    template.push_str(&format!("${{{}}}", key));
                    expected_parts.push((key, None));
                }
            }
            template.push_str(&literals[i+1]);
        }

        (template, expected_parts)
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

// Feature: perry-container | Layer: unit | Req: 7.9 | Property: -
#[test]
fn test_parse_dotenv_semantics() {
    let content = "KEY=val\n# comment\nSPACED = val \nQUOTED=\"val with # hash\"\nEMPTY=";
    let env = parse_dotenv(content);

    assert_eq!(env.get("KEY").map(|s| s.as_str()), Some("val"));
    assert_eq!(env.get("SPACED").map(|s| s.as_str()), Some("val"));
    assert_eq!(env.get("QUOTED").map(|s| s.as_str()), Some("val with # hash"));
    assert_eq!(env.get("EMPTY").map(|s| s.as_str()), Some(""));
    assert!(env.get("#").is_none());
}

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: -
#[test]
fn test_interpolation_modifiers() {
    let mut env = HashMap::new();
    env.insert("SET".into(), "ok".into());

    // ${VAR:+value} -> "value" if SET is non-empty
    assert_eq!(interpolate("${SET:+yes}", &env), "yes");
    assert_eq!(interpolate("${UNSET:+yes}", &env), "");

    // $$ escape
    assert_eq!(interpolate("$$VAR", &env), "$VAR");
}

// Feature: perry-container | Layer: unit | Req: 7.1 | Property: -
#[test]
fn test_parse_compose_yaml_errors() {
    let env = HashMap::new();
    let malformed = "services: [unclosed";
    let result = parse_compose_yaml(malformed, &env);

    match result {
        Err(perry_container_compose::error::ComposeError::ParseError(_)) => {}
        _ => panic!("Expected ParseError"),
    }
}

// =============================================================================
// Property Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 7.8 | Property: 6
    #[test]
    fn prop_env_interpolation(
        (template, parts) in arb_env_template_parts(),
        env in arb_env_map()
    ) {
        let result = interpolate_yaml(&template, &env);

        for (key, default) in parts {
            let expected = match env.get(&key) {
                Some(v) if !v.is_empty() => v.clone(),
                _ => {
                    match std::env::var(&key) {
                        Ok(v) if !v.is_empty() => v,
                        _ => default.unwrap_or_default(),
                    }
                }
            };
            prop_assert!(result.contains(&expected));
        }
    }

    // Feature: perry-container | Layer: property | Req: 7.10 | Property: 7
    #[test]
    fn prop_compose_file_merge_last_writer_wins(
        name1 in "[a-z]{5}",
        name2 in "[a-z]{5}",
        img1 in "[a-z]{5}",
        img2 in "[a-z]{5}"
    ) {
        use perry_container_compose::types::ComposeSpec;

        let yaml1 = format!("services:\n  {}:\n    image: {}\n  shared:\n    image: old", name1, img1);
        let yaml2 = format!("services:\n  {}:\n    image: {}\n  shared:\n    image: new", name2, img2);

        let mut spec1 = ComposeSpec::parse_str(&yaml1).expect("YAML 1 invalid");
        let spec2 = ComposeSpec::parse_str(&yaml2).expect("YAML 2 invalid");

        spec1.merge(spec2);

        prop_assert!(spec1.services.contains_key(&name1));
        prop_assert!(spec1.services.contains_key(&name2));

        let shared = spec1.services.get("shared").expect("shared missing");
        prop_assert_eq!(shared.image.as_deref(), Some("new"));
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 7.1         | test_parse_compose_yaml_errors | unit |
| 7.8         | prop_env_interpolation | property |
| 7.8         | test_interpolation_modifiers | unit |
| 7.9         | test_parse_dotenv_semantics | unit |
| 7.10        | prop_compose_file_merge_last_writer_wins | property |
| 9.2         | prop_compose_file_merge_last_writer_wins | property |
*/

// Deferred Requirements:
// Req 9.1, 9.5, 9.6, 9.8 — parse_and_merge_files() requires disk I/O, deferred to integration.
