// Feature: perry-container | Layer: property | Req: 7.12 | Property: 1
#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

use perry_container_compose::types::*;
use proptest::prelude::*;
use indexmap::IndexMap;
use serde_json;

// --- Generators ---

prop_compose! {
    pub fn arb_service_name()(name in "[a-z0-9-]{1,10}") -> String { name }
}

prop_compose! {
    pub fn arb_image_ref()(name in "[a-z0-9/._-]{1,20}", tag in ":[a-z0-9._-]{1,10}") -> String {
        format!("{}{}", name, tag)
    }
}

prop_compose! {
    pub fn arb_list_or_dict()(
        is_list in prop::bool::ANY,
        items in prop::collection::vec("[a-zA-Z0-9_-]{1,10}", 0..5)
    ) -> ListOrDict {
        if is_list {
            ListOrDict::List(items)
        } else {
            let mut map = IndexMap::new();
            for item in items {
                map.insert(item, Some(serde_yaml::Value::String("val".into())));
            }
            ListOrDict::Dict(map)
        }
    }
}

prop_compose! {
    pub fn arb_port_spec()(
        is_short in prop::bool::ANY,
        port in 1..65535u16
    ) -> PortSpec {
        if is_short {
            PortSpec::Short(serde_yaml::Value::Number(port.into()))
        } else {
            PortSpec::Long(ComposeServicePort {
                target: serde_yaml::Value::Number(port.into()),
                published: Some(serde_yaml::Value::Number((port + 1).into())),
                protocol: Some("tcp".into()),
                ..Default::default()
            })
        }
    }
}

prop_compose! {
    pub fn arb_depends_on_spec(possible_deps: Vec<String>)(
        indices in prop::collection::vec(0..possible_deps.len().max(1), 0..3),
        is_list in prop::bool::ANY
    ) -> DependsOnSpec {
        let names: Vec<String> = if possible_deps.is_empty() {
            vec![]
        } else {
            indices.into_iter().map(|i| possible_deps[i % possible_deps.len()].clone()).collect()
        };
        if is_list {
            DependsOnSpec::List(names)
        } else {
            let mut map = IndexMap::new();
            for name in names {
                map.insert(name, ComposeDependsOn {
                    condition: Some(DependsOnCondition::ServiceStarted),
                    ..Default::default()
                });
            }
            DependsOnSpec::Map(map)
        }
    }
}

prop_compose! {
    pub fn arb_compose_service(possible_deps: Vec<String>)(
        image in prop::option::of(arb_image_ref()),
        env in prop::option::of(arb_list_or_dict()),
        ports in prop::option::of(prop::collection::vec(arb_port_spec(), 0..3)),
        deps in prop::option::of(arb_depends_on_spec(possible_deps))
    ) -> ComposeService {
        ComposeService {
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
        service_names in prop::collection::vec(arb_service_name(), 1..10)
    )(
        services in service_names.iter().enumerate().map(|(i, name)| {
            let possible_deps = service_names[..i].to_vec();
            (Just(name.clone()), arb_compose_service(possible_deps))
        }).collect::<Vec<_>>()
    ) -> ComposeSpec {
        let mut spec_services = IndexMap::new();
        for (name, svc) in services {
            spec_services.insert(name, svc);
        }
        ComposeSpec {
            services: spec_services,
            ..Default::default()
        }
    }
}

prop_compose! {
    pub fn arb_container_spec()(
        image in arb_image_ref(),
        name in prop::option::of("[a-z0-9-]{1,10}"),
        ports in prop::option::of(prop::collection::vec("[0-9]{2,5}:[0-9]{2,5}", 0..3)),
        rm in prop::option::of(prop::bool::ANY)
    ) -> ContainerSpec {
        ContainerSpec {
            image,
            name,
            ports,
            rm,
            ..Default::default()
        }
    }
}

// --- Property Tests ---

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 7.12 | Property: 1
    #[test]
    fn prop_compose_spec_json_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).expect("Serialization failed");
        let deserialized: ComposeSpec = serde_json::from_str(&json).expect("Deserialization failed");
        let json2 = serde_json::to_string(&deserialized).expect("Reserialization failed");
        assert_eq!(json, json2);
    }

    // Feature: perry-container | Layer: property | Req: 12.6 | Property: 1
    #[test]
    fn prop_container_spec_json_round_trip(spec in arb_container_spec()) {
        let json = serde_json::to_string(&spec).expect("Serialization failed");
        let deserialized: ContainerSpec = serde_json::from_str(&json).expect("Deserialization failed");
        let json2 = serde_json::to_string(&deserialized).expect("Reserialization failed");
        assert_eq!(json, json2);
    }
}

// Feature: perry-container | Layer: unit | Req: 7.14 | Property: 8
#[test]
fn test_depends_on_condition_deserialization() {
    let json = "\"service_started\"";
    let cond: DependsOnCondition = serde_json::from_str(json).unwrap();
    assert!(matches!(cond, DependsOnCondition::ServiceStarted));

    let invalid = "\"invalid_condition\"";
    let result: Result<DependsOnCondition, _> = serde_json::from_str(invalid);
    assert!(result.is_err());
}

// Feature: perry-container | Layer: unit | Req: 10.14 | Property: 9
#[test]
fn test_volume_type_deserialization() {
    let json = "\"bind\"";
    let vt: VolumeType = serde_json::from_str(json).unwrap();
    assert!(matches!(vt, VolumeType::Bind));

    let invalid = "\"invalid_type\"";
    let result: Result<VolumeType, _> = serde_json::from_str(invalid);
    assert!(result.is_err());
}
