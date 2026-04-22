//! Tests for the `types` module.
//!
//! Validates `ComposeSpec` serialization round-trips.

use perry_container_compose::types::{ComposeService, ComposeSpec, PortSpec, ListOrDict, DependsOnSpec, ComposeDependsOn, DependsOnCondition, ComposeServicePort};
use indexmap::IndexMap;
use proptest::prelude::*;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// ============ Generators ============

prop_compose! {
    fn arb_service_name()(name in "[a-z][a-z0-9_-]{0,10}") -> String {
        name
    }
}

prop_compose! {
    fn arb_image_ref()(repo in "[a-z0-9]+", tag in "[a-z0-9.]+") -> String {
        format!("{}:{}", repo, tag)
    }
}

prop_compose! {
    fn arb_port_spec()(
        is_long in proptest::bool::ANY,
        short in "[0-9]+:[0-9]+",
        target in 1u32..65535,
        published in 1u32..65535
    ) -> PortSpec {
        if is_long {
            PortSpec::Long(ComposeServicePort {
                name: None,
                mode: None,
                host_ip: None,
                target: serde_yaml::Value::Number(target.into()),
                published: Some(serde_yaml::Value::Number(published.into())),
                protocol: None,
                app_protocol: None,
            })
        } else {
            PortSpec::Short(serde_yaml::Value::String(short))
        }
    }
}

prop_compose! {
    fn arb_list_or_dict()(
        is_list in proptest::bool::ANY,
        list in proptest::collection::vec("[a-zA-Z0-9_]+=[a-zA-Z0-9_]+", 0..5),
        dict in proptest::collection::hash_map("[a-zA-Z0-9_]+", "[a-zA-Z0-9_]+", 0..5)
    ) -> ListOrDict {
        if is_list {
            ListOrDict::List(list)
        } else {
            let mut imap = IndexMap::new();
            for (k, v) in dict {
                imap.insert(k, Some(serde_yaml::Value::String(v)));
            }
            ListOrDict::Dict(imap)
        }
    }
}

prop_compose! {
    fn arb_depends_on_spec()(
        is_map in proptest::bool::ANY,
        names in proptest::collection::vec(arb_service_name(), 1..3)
    ) -> DependsOnSpec {
        if is_map {
            let mut map = IndexMap::new();
            for name in names {
                map.insert(name, ComposeDependsOn {
                    condition: Some(DependsOnCondition::ServiceStarted),
                    required: None,
                    restart: None,
                });
            }
            DependsOnSpec::Map(map)
        } else {
            DependsOnSpec::List(names)
        }
    }
}

prop_compose! {
    fn arb_compose_service()(
        image in proptest::option::of(arb_image_ref()),
        ports in proptest::option::of(proptest::collection::vec(arb_port_spec(), 0..3)),
        environment in proptest::option::of(arb_list_or_dict()),
        depends_on in proptest::option::of(arb_depends_on_spec())
    ) -> ComposeService {
        ComposeService {
            image,
            ports,
            environment,
            depends_on,
            ..Default::default()
        }
    }
}

prop_compose! {
    fn arb_compose_spec()(
        services_vec in proptest::collection::vec(
            (arb_service_name(), arb_compose_service()),
            1..5
        )
    ) -> ComposeSpec {
        let mut services = IndexMap::new();
        for (name, svc) in services_vec {
            services.insert(name, svc);
        }
        ComposeSpec { services, ..Default::default() }
    }
}

// ============ Property Tests ============

// Feature: perry-container | Layer: property | Req: 10.13 | Property: 1
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_compose_spec_serialization_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).expect("Should serialize to JSON");
        let deserialized: ComposeSpec = serde_json::from_str(&json).expect("Should deserialize from JSON");

        let json2 = serde_json::to_string(&deserialized).expect("Should re-serialize to JSON");
        prop_assert_eq!(json, json2);
    }
}

// ============ Coverage Table ============
//
// | Requirement | Test name | Layer |
// |-------------|-----------|-------|
// | 10.13       | prop_compose_spec_serialization_round_trip | property |
