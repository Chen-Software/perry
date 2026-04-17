use perry_container_compose::types::*;
use proptest::prelude::*;
use indexmap::IndexMap;
use serde_yaml::Value as YamlValue;

fn arb_yaml_value() -> impl Strategy<Value = YamlValue> {
    prop_oneof![
        any::<String>().prop_map(YamlValue::String),
        any::<i64>().prop_map(|n| YamlValue::Number(n.into())),
        any::<bool>().prop_map(YamlValue::Bool),
        Just(YamlValue::Null),
    ]
}

fn arb_list_or_dict() -> impl Strategy<Value = ListOrDict> {
    prop_oneof![
        prop::collection::vec(any::<String>(), 0..5).prop_map(ListOrDict::List),
        prop::collection::vec((any::<String>(), prop::option::of(arb_yaml_value())), 0..5)
            .prop_map(|v| ListOrDict::Dict(v.into_iter().collect())),
    ]
}

fn arb_depends_on_condition() -> impl Strategy<Value = DependsOnCondition> {
    prop_oneof![
        Just(DependsOnCondition::ServiceStarted),
        Just(DependsOnCondition::ServiceHealthy),
        Just(DependsOnCondition::ServiceCompletedSuccessfully),
    ]
}

fn arb_compose_depends_on() -> impl Strategy<Value = ComposeDependsOn> {
    (prop::option::of(arb_depends_on_condition()), prop::option::of(any::<bool>()), prop::option::of(any::<bool>()))
        .prop_map(|(condition, required, restart)| ComposeDependsOn { condition, required, restart })
}

fn arb_depends_on_spec(service_names: Vec<String>) -> impl Strategy<Value = DependsOnSpec> {
    let names = service_names.clone();
    prop_oneof![
        prop::collection::vec(prop::sample::select(names.clone()), 0..names.len().max(1))
            .prop_map(DependsOnSpec::List),
        prop::collection::vec((prop::sample::select(names.clone()), arb_compose_depends_on()), 0..names.len().max(1))
            .prop_map(|v| DependsOnSpec::Map(v.into_iter().collect())),
    ]
}

fn arb_port_spec() -> impl Strategy<Value = PortSpec> {
    prop_oneof![
        arb_yaml_value().prop_map(PortSpec::Short),
        (prop::option::of(any::<String>()), prop::option::of(any::<String>()), prop::option::of(any::<String>()), arb_yaml_value(), prop::option::of(arb_yaml_value()), prop::option::of(any::<String>()), prop::option::of(any::<String>()))
            .prop_map(|(name, mode, host_ip, target, published, protocol, app_protocol)| PortSpec::Long(ComposeServicePort { name, mode, host_ip, target, published, protocol, app_protocol })),
    ]
}

fn arb_compose_service(service_names: Vec<String>) -> impl Strategy<Value = ComposeService> {
    let names = service_names.clone();
    (
        prop::option::of(any::<String>()),
        prop::option::of(arb_list_or_dict()),
        prop::option::of(prop::collection::vec(arb_port_spec(), 0..3)),
        prop::option::of(arb_depends_on_spec(names)),
        prop::option::of(any::<String>()),
        prop::option::of(any::<bool>()),
    ).prop_map(|(image, environment, ports, depends_on, container_name, privileged)| ComposeService {
        image,
        environment,
        ports,
        depends_on,
        container_name,
        privileged,
        ..Default::default()
    })
}

fn arb_compose_spec() -> impl Strategy<Value = ComposeSpec> {
    let service_names = vec!["web".to_string(), "db".to_string(), "redis".to_string()];
    let names = service_names.clone();
    prop::collection::vec(arb_compose_service(names), 3..4)
        .prop_map(move |services| {
            let mut spec_services = IndexMap::new();
            for (i, svc) in services.into_iter().enumerate() {
                spec_services.insert(service_names[i].clone(), svc);
            }
            ComposeSpec {
                services: spec_services,
                ..Default::default()
            }
        })
}

fn arb_container_spec() -> impl Strategy<Value = ContainerSpec> {
    (
        any::<String>(),
        prop::option::of(any::<String>()),
        prop::option::of(prop::collection::vec(any::<String>(), 0..3)),
        prop::option::of(prop::collection::vec(any::<String>(), 0..3)),
        prop::option::of(prop::collection::hash_map(any::<String>(), any::<String>(), 0..3)),
        prop::option::of(prop::collection::vec(any::<String>(), 0..3)),
        prop::option::of(any::<bool>()),
    ).prop_map(|(image, name, ports, volumes, env, cmd, rm)| ContainerSpec {
        image, name, ports, volumes, env, cmd, rm, ..Default::default()
    })
}

// Feature: perry-container, Property 1: ComposeSpec serialization round-trip
proptest! {
    #[test]
    fn prop_compose_spec_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ComposeSpec = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&deserialized).unwrap();
        // Since we are using IndexMap, the order should be preserved.
        prop_assert_eq!(json, json2);
    }
}

// Feature: perry-container, Property 2: ContainerSpec serialization round-trip
proptest! {
    #[test]
    fn prop_container_spec_round_trip(spec in arb_container_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ContainerSpec = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&spec, &deserialized);
    }
}

// Feature: perry-container, Property 5: YAML round-trip preserves ComposeSpec
proptest! {
    #[test]
    fn prop_yaml_round_trip(spec in arb_compose_spec()) {
        let yaml = spec.to_yaml().unwrap();
        let deserialized = ComposeSpec::parse_str(&yaml).unwrap();
        let yaml2 = deserialized.to_yaml().unwrap();
        prop_assert_eq!(yaml, yaml2);
    }
}
