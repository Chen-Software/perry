use perry_container_compose::types::*;
use proptest::prelude::*;

// Feature: perry-container | Layer: unit | Req: 7.14 | Property: 8
#[test]
fn test_depends_on_condition_deserialization() {
    let cases = vec![
        ("service_started", DependsOnCondition::ServiceStarted),
        ("service_healthy", DependsOnCondition::ServiceHealthy),
        ("service_completed_successfully", DependsOnCondition::ServiceCompletedSuccessfully),
    ];
    for (s, expected) in cases {
        let json = format!("\"{}\"", s);
        let decoded: DependsOnCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, expected);
    }

    // Invalid
    let res: Result<DependsOnCondition, _> = serde_json::from_str("\"invalid\"");
    assert!(res.is_err());
}

// Feature: perry-container | Layer: unit | Req: 10.14 | Property: 9
#[test]
fn test_volume_type_deserialization() {
    let cases = vec![
        ("bind", VolumeType::Bind),
        ("volume", VolumeType::Volume),
        ("tmpfs", VolumeType::Tmpfs),
    ];
    for (s, expected) in cases {
        let json = format!("\"{}\"", s);
        let decoded: VolumeType = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, expected);
    }

    let res: Result<VolumeType, _> = serde_json::from_str("\"invalid\"");
    assert!(res.is_err());
}

prop_compose! {
    fn arb_container_spec()(
        image in "[a-z]+",
        name in proptest::option::of("[a-z]+"),
    ) -> ContainerSpec {
        ContainerSpec {
            image,
            name,
            ..Default::default()
        }
    }
}

prop_compose! {
    fn arb_compose_spec()(
        name in proptest::option::of("[a-z]+")
    ) -> ComposeSpec {
        ComposeSpec {
            name,
            ..Default::default()
        }
    }
}

proptest! {
    // Feature: perry-container | Layer: property | Req: 12.6 | Property: 1
    #[test]
    fn prop_compose_spec_json_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let decoded: ComposeSpec = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(serde_json::to_value(spec).unwrap(), serde_json::to_value(decoded).unwrap());
    }

    // Feature: perry-container | Layer: property | Req: none | Property: 1
    #[test]
    fn prop_container_spec_json_round_trip(spec in arb_container_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let decoded: ContainerSpec = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(serde_json::to_value(spec).unwrap(), serde_json::to_value(decoded).unwrap());
    }
}
