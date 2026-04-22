use proptest::prelude::*;
use perry_container_compose::types::*;
use indexmap::IndexMap;

fn arb_compose_spec_with_name(name: String) -> impl Strategy<Value = ComposeSpec> {
    prop::collection::vec(arb_service_name(), 1..3).prop_map(move |service_names| {
        let mut spec = ComposeSpec::default();
        spec.name = Some(name.clone());
        let mut services_map = IndexMap::new();
        for svc_name in service_names {
            let mut svc = ComposeService::default();
            svc.image = Some(format!("{}-image", svc_name));
            services_map.insert(svc_name, svc);
        }
        spec.services = services_map;
        spec
    })
}

fn arb_service_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("web".to_string()),
        Just("db".to_string()),
        Just("cache".to_string()),
        Just("worker".to_string()),
    ]
}

proptest! {
    #[test]
    fn test_merge_preserves_unique_services(
        mut spec1 in arb_compose_spec_with_name("p1".into()),
        spec2 in arb_compose_spec_with_name("p2".into())
    ) {
        let s1_keys: std::collections::HashSet<_> = spec1.services.keys().cloned().collect();
        let s2_keys: std::collections::HashSet<_> = spec2.services.keys().cloned().collect();

        spec1.merge(spec2.clone());

        for k in s1_keys {
            assert!(spec1.services.contains_key(&k));
        }
        for k in s2_keys {
            assert!(spec1.services.contains_key(&k));
        }
    }

    #[test]
    fn test_merge_last_writer_wins(
        mut spec1 in arb_compose_spec_with_name("p1".into()),
        spec2 in arb_compose_spec_with_name("p2".into())
    ) {
        // Find common keys
        let common_keys: Vec<_> = spec1.services.keys()
            .filter(|k| spec2.services.contains_key(*k))
            .cloned()
            .collect();

        spec1.merge(spec2.clone());

        for k in common_keys {
            assert_eq!(spec1.services.get(&k).unwrap().image, spec2.services.get(&k).unwrap().image);
        }

        // Name should also be updated
        assert_eq!(spec1.name, spec2.name);
    }
}
