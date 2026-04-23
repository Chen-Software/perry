// Feature: perry-container | Layer: property | Req: 10.13 | Property: 1
use perry_container_compose::types::*;
use proptest::prelude::*;
use indexmap::IndexMap;
use serde_yaml;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// ============ Generators ============

prop_compose! {
    // Feature: perry-container | Layer: property | Req: none | Property: -
    fn arb_service_name()(name in "[a-z][a-z0-9_-]{1,10}") -> String {
        name
    }
}

prop_compose! {
    // Feature: perry-container | Layer: property | Req: none | Property: -
    fn arb_image_ref()(repo in "[a-z]{3,10}", tag in "[a-z0-9]{3,5}") -> String {
        format!("{}:{}", repo, tag)
    }
}

prop_compose! {
    // Feature: perry-container | Layer: property | Req: 10.8 | Property: -
    fn arb_port_spec()(
        target in 1u16..65535,
        published in proptest::option::of(1u16..65535),
        protocol in proptest::option::of(prop_oneof!["tcp", "udp"])
    ) -> PortSpec {
        if let Some(p) = published {
            PortSpec::Long(ComposeServicePort {
                target: (target as u32).into(),
                published: Some((p as u32).into()),
                protocol: protocol.map(|p| p.to_string()),
                name: None,
                mode: None,
                host_ip: None,
                app_protocol: None,
            })
        } else {
            PortSpec::Short((target as u32).into())
        }
    }
}

prop_compose! {
    // Feature: perry-container | Layer: property | Req: 6.3 | Property: -
    fn arb_list_or_dict()(
        is_list in proptest::bool::ANY,
        list in proptest::collection::vec(".*", 0..5),
        dict in proptest::collection::vec(("[a-z]+", ".*"), 0..5)
    ) -> ListOrDict {
        if is_list {
            ListOrDict::List(list)
        } else {
            let mut map = IndexMap::new();
            for (k, v) in dict {
                map.insert(k, Some(serde_yaml::Value::String(v)));
            }
            ListOrDict::Dict(map)
        }
    }
}

prop_compose! {
    // Feature: perry-container | Layer: property | Req: 6.3 | Property: -
    fn arb_depends_on_spec()(
        is_list in proptest::bool::ANY,
        services in proptest::collection::vec(arb_service_name(), 1..3)
    ) -> DependsOnSpec {
        if is_list {
            DependsOnSpec::List(services)
        } else {
            let mut map = IndexMap::new();
            for s in services {
                map.insert(s, ComposeDependsOn {
                    condition: Some(DependsOnCondition::ServiceStarted),
                    required: Some(true),
                    restart: Some(false),
                });
            }
            DependsOnSpec::Map(map)
        }
    }
}

prop_compose! {
    // Feature: perry-container | Layer: property | Req: 6.3 | Property: -
    fn arb_compose_service()(
        image in proptest::option::of(arb_image_ref()),
        command in proptest::option::of(prop_oneof![
            Just(serde_yaml::Value::String("ls".to_string())),
            Just(serde_yaml::Value::Sequence(vec![serde_yaml::Value::String("ls".to_string())]))
        ]),
        ports in proptest::option::of(proptest::collection::vec(arb_port_spec(), 0..3)),
        depends_on in proptest::option::of(arb_depends_on_spec())
    ) -> ComposeService {
        ComposeService {
            image,
            command,
            ports,
            depends_on,
            ..Default::default()
        }
    }
}

prop_compose! {
    // Feature: perry-container | Layer: property | Req: 6.2 | Property: 1
    fn arb_compose_spec()(
        name in proptest::option::of(arb_service_name()),
        services in proptest::collection::vec((arb_service_name(), arb_compose_service()), 1..5)
    ) -> ComposeSpec {
        let mut map = IndexMap::new();
        for (k, v) in services {
            map.insert(k, v);
        }
        ComposeSpec {
            name,
            services: map,
            ..Default::default()
        }
    }
}

// ============ Tests ============

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 10.13 | Property: 1
    #[test]
    fn prop_compose_spec_json_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).expect("serialize");
        let deserialized: ComposeSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(spec.name, deserialized.name);
        assert_eq!(spec.services.len(), deserialized.services.len());
    }

    // Feature: perry-container | Layer: property | Req: 7.14 | Property: 8
    #[test]
    fn prop_depends_on_condition_rejects_invalid(invalid in "[a-z]{3,20}") {
        let valid_values = ["service_started", "service_healthy", "service_completed_successfully"];
        prop_assume!(!valid_values.contains(&invalid.as_str()));
        let yaml = format!("\"{}\"", invalid);
        let result = serde_yaml::from_str::<DependsOnCondition>(&yaml);
        assert!(result.is_err());
    }

    // Feature: perry-container | Layer: property | Req: 10.14 | Property: 9
    #[test]
    fn prop_volume_type_rejects_invalid(invalid in "[a-z]{3,20}") {
        let valid_values = ["bind", "volume", "tmpfs", "cluster", "npipe", "image"];
        prop_assume!(!valid_values.contains(&invalid.as_str()));
        let yaml = format!("\"{}\"", invalid);
        let result = serde_yaml::from_str::<VolumeType>(&yaml);
        assert!(result.is_err());
    }
}

// Feature: perry-container | Layer: unit | Req: 6.3 | Property: -
#[test]
fn test_depends_on_spec_service_names() {
    let list = DependsOnSpec::List(vec!["a".to_string(), "b".to_string()]);
    assert_eq!(list.service_names(), vec!["a", "b"]);

    let mut map = IndexMap::new();
    map.insert("c".to_string(), ComposeDependsOn {
        condition: Some(DependsOnCondition::ServiceStarted),
        required: Some(true),
        restart: Some(false),
    });
    let spec_map = DependsOnSpec::Map(map);
    assert_eq!(spec_map.service_names(), vec!["c"]);
}

// Feature: perry-container | Layer: unit | Req: 7.14 | Property: -
#[test]
fn test_depends_on_condition_variants() {
    let yaml = "service_healthy";
    let cond: DependsOnCondition = serde_yaml::from_str(yaml).expect("parse healthy");
    assert!(matches!(cond, DependsOnCondition::ServiceHealthy));

    let yaml = "service_started";
    let cond: DependsOnCondition = serde_yaml::from_str(yaml).expect("parse started");
    assert!(matches!(cond, DependsOnCondition::ServiceStarted));

    let yaml = "service_completed_successfully";
    let cond: DependsOnCondition = serde_yaml::from_str(yaml).expect("parse completed");
    assert!(matches!(cond, DependsOnCondition::ServiceCompletedSuccessfully));
}
