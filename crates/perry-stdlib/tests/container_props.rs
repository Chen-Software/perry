use proptest::prelude::*;
use perry_stdlib::container::types::*;

proptest! {
    #[test]
    fn prop_container_spec_serialization(image in "[a-z0-9]+") {
        let spec = ContainerSpec {
            image,
            ..Default::default()
        };
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ContainerSpec = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(spec.image, deserialized.image);
    }
}
