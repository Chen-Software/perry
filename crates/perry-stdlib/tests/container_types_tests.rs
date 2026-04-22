//! Tests for the stdlib container types.
//!
//! Validates `ContainerSpec` serialization and other stdlib-specific properties.

use perry_container_compose::types::ContainerSpec;
use proptest::prelude::*;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// ============ Generators ============

prop_compose! {
    fn arb_image_ref()(repo in "[a-z0-9]+", tag in "[a-z0-9.]+") -> String {
        format!("{}:{}", repo, tag)
    }
}

prop_compose! {
    fn arb_container_spec()(
        image in arb_image_ref(),
        name in proptest::option::of("[a-z][a-z0-9_-]{0,10}"),
        ports in proptest::option::of(proptest::collection::vec("[0-9]+:[0-9]+", 0..3)),
        rm in proptest::option::of(proptest::bool::ANY)
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

// ============ Property Tests ============

// Feature: perry-container | Layer: property | Req: 12.5 | Property: 2
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_container_spec_serialization_round_trip(spec in arb_container_spec()) {
        let json = serde_json::to_string(&spec).expect("Should serialize to JSON");
        let deserialized: ContainerSpec = serde_json::from_str(&json).expect("Should deserialize from JSON");

        prop_assert_eq!(spec.image, deserialized.image);
        prop_assert_eq!(spec.name, deserialized.name);
        prop_assert_eq!(spec.ports, deserialized.ports);
        prop_assert_eq!(spec.rm, deserialized.rm);
    }
}

// ============ Coverage Table ============
//
// | Requirement | Test name | Layer |
// |-------------|-----------|-------|
// | 12.5        | prop_container_spec_serialization_round_trip | property |

// Deferred Requirements:
// Req 15.7 — requires live cosign binary, integration test deferred.
