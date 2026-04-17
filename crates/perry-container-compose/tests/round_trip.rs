use perry_container_compose::types::{ComposeSpec, ContainerSpec, ComposeService, DependsOnSpec};
use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::yaml::interpolate;
use proptest::prelude::*;
use std::collections::HashMap;

fn arb_container_spec() -> impl Strategy<Value = ContainerSpec> {
    any::<String>().prop_map(|image| ContainerSpec {
        image,
        name: Some("test".into()),
        ..Default::default()
    })
}

proptest! {
    #[test]
    fn prop_container_spec_round_trip(spec in arb_container_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ContainerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec.image, deserialized.image);
    }

    #[test]
    fn prop_topological_sort_respects_deps(
        s1 in "[a-z0-9]{5,10}",
        s2 in "[a-z0-9]{5,10}"
    ) {
        prop_assume!(s1 != s2);
        let mut spec = ComposeSpec::default();
        let svc1 = ComposeService::default();
        let mut svc2 = ComposeService::default();

        svc2.depends_on = Some(DependsOnSpec::List(vec![s1.clone()]));

        spec.services.insert(s1.clone(), svc1);
        spec.services.insert(s2.clone(), svc2);

        let order = resolve_startup_order(&spec).unwrap();
        let pos1 = order.iter().position(|s| s == &s1).unwrap();
        let pos2 = order.iter().position(|s| s == &s2).unwrap();
        assert!(pos1 < pos2);
    }

    #[test]
    fn prop_interpolation_works(
        key in "[A-Z0-9_]{1,10}",
        val in "[a-z0-9]{1,10}"
    ) {
        let mut env = HashMap::new();
        env.insert(key.clone(), val.clone());
        let template = format!("image: ${{{}}}", key);
        let result = interpolate(&template, &env);
        assert_eq!(result, format!("image: {}", val));
    }

    #[test]
    fn prop_interpolation_default_works(
        key in "[A-Z0-9_]{1,10}",
        default_val in "[a-z0-9]{1,10}"
    ) {
        let env = HashMap::new();
        let template = format!("image: ${{{}:-{}}}", key, default_val);
        let result = interpolate(&template, &env);
        assert_eq!(result, format!("image: {}", default_val));
    }
}
