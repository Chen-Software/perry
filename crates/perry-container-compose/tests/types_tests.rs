// Feature: perry-container | Layer: property | Req: 10.13 | Property: 1

use proptest::prelude::*;
use perry_container_compose::types::*;
use serde_json;
use serde_yaml;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// =============================================================================
// Property-Based Generators
// =============================================================================

prop_compose! {
    fn arb_service_name()(name in "[a-z0-9_-]{1,32}") -> String { name }
}

prop_compose! {
    fn arb_image_ref()(repo in "[a-z0-9/._-]{1,64}", tag in proptest::option::of("[a-z0-9._-]{1,16}")) -> String {
        match tag {
            Some(t) => format!("{}:{}", repo, t),
            None => repo,
        }
    }
}

prop_compose! {
    fn arb_port_spec()(
        is_long in any::<bool>(),
        h in 1u16..65535,
        c in 1u16..65535
    ) -> PortSpec {
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
    fn arb_list_or_dict()(
        is_dict in any::<bool>(),
        keys in proptest::collection::vec("[a-zA-Z0-9_]{1,16}", 0..5),
        values in proptest::collection::vec("[a-zA-Z0-9_]{0,32}", 0..5)
    ) -> ListOrDict {
        if is_dict {
            let mut map = perry_container_compose::indexmap::IndexMap::new();
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
    fn arb_depends_on_spec()(
        is_map in any::<bool>(),
        services in proptest::collection::vec(arb_service_name(), 0..3)
    ) -> DependsOnSpec {
        if is_map {
            let mut map = perry_container_compose::indexmap::IndexMap::new();
            for s in services {
                map.insert(s, ComposeDependsOn {
                    condition: Some(DependsOnCondition::ServiceStarted),
                    required: Some(true),
                    restart: Some(false),
                });
            }
            DependsOnSpec::Map(map)
        } else {
            DependsOnSpec::List(services)
        }
    }
}

prop_compose! {
    fn arb_compose_service()(
        image in proptest::option::of(arb_image_ref()),
        env in proptest::option::of(arb_list_or_dict()),
        ports in proptest::option::of(proptest::collection::vec(arb_port_spec(), 0..2)),
        deps in proptest::option::of(arb_depends_on_spec()),
        restart in proptest::option::of(prop_oneof!["always", "on-failure", "no", "unless-stopped"])
    ) -> ComposeService {
        ComposeService {
            image,
            environment: env,
            ports,
            depends_on: deps,
            restart,
            ..Default::default()
        }
    }
}

prop_compose! {
    fn arb_compose_spec()(
        name in proptest::option::of("[a-z0-9_-]{1,16}"),
        service_names in proptest::collection::vec(arb_service_name(), 1..5)
    ) -> ComposeSpec {
        let mut services = perry_container_compose::indexmap::IndexMap::new();
        for s in service_names {
            services.insert(s, ComposeService::default());
        }
        ComposeSpec { name, services, ..Default::default() }
    }
}

prop_compose! {
    fn arb_container_spec()(
        image in arb_image_ref(),
        name in proptest::option::of("[a-z0-9_-]{1,32}"),
        rm in proptest::option::of(any::<bool>())
    ) -> ContainerSpec {
        ContainerSpec { image, name, rm, ..Default::default() }
    }
}

prop_compose! {
    fn arb_env_template()(
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
    fn arb_env_map()(
        map in proptest::collection::hash_map("[A-Z_]+", ".*", 0..10)
    ) -> std::collections::HashMap<String, String> { map }
}

// =============================================================================
// Property Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 7.12 | Property: 1
    #[test]
    fn prop_compose_spec_json_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).expect("Should serialize");
        let deserialized: ComposeSpec = serde_json::from_str(&json).expect("Should deserialize");
        let json2 = serde_json::to_string(&deserialized).expect("Should serialize again");
        prop_assert_eq!(json, json2);
    }

    // Feature: perry-container | Layer: property | Req: 7.1 | Property: 5
    #[test]
    fn prop_yaml_round_trip(spec in arb_compose_spec()) {
        let yaml = spec.to_yaml().expect("Should serialize to YAML");
        let deserialized = ComposeSpec::parse_str(&yaml).expect("Should parse from YAML");
        let yaml2 = deserialized.to_yaml().expect("Should serialize to YAML again");
        prop_assert_eq!(yaml, yaml2);
    }

    // Feature: perry-container | Layer: property | Req: 7.14 | Property: 8
    #[test]
    fn prop_depends_on_condition_rejects_invalid(s in "[a-z0-9]{1,16}") {
        // Condition is an enum, so we test string deserialization.
        // Rejects anything that is not one of the 3 valid snakes.
        if s != "service_started" && s != "service_healthy" && s != "service_completed_successfully" {
            let json = format!("\"{}\"", s);
            let result: Result<DependsOnCondition, _> = serde_json::from_str(&json);
            prop_assert!(result.is_err());
        }
    }

    // Feature: perry-container | Layer: property | Req: 10.14 | Property: 9
    #[test]
    fn prop_volume_type_rejects_invalid(s in "[a-z0-9]{1,16}") {
        let valids = ["bind", "volume", "tmpfs", "cluster", "npipe", "image"];
        if !valids.contains(&s.as_str()) {
            let json = format!("\"{}\"", s);
            let result: Result<VolumeType, _> = serde_json::from_str(&json);
            prop_assert!(result.is_err());
        }
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 7.1         | prop_yaml_round_trip | property |
| 7.12        | prop_compose_spec_json_round_trip | property |
| 7.14        | prop_depends_on_condition_rejects_invalid | property |
| 10.13       | prop_compose_spec_json_round_trip | property |
| 10.14       | prop_volume_type_rejects_invalid | property |
| 12.6        | prop_compose_spec_json_round_trip | property |
*/

// Deferred Requirements:
// Req 12.5 — ContainerSpec CLI round-trip is tested in backend_tests.rs unit tests.
