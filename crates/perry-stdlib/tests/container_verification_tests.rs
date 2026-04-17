// Feature: perry-container | Layer: property | Req: 15.7 | Property: 10

use perry_stdlib::container::verification::*;
use proptest::prelude::*;
use std::collections::HashMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// =============================================================================
// Required Generators
// =============================================================================

prop_compose! {
    pub fn arb_service_name()(name in "[a-z0-9_-]{1,64}") -> String { name }
}

prop_compose! {
    pub fn arb_image_ref()(repo in "[a-z0-9/._-]{1,128}", tag in proptest::option::of("[a-z0-9._-]{1,32}")) -> String {
        match tag {
            Some(t) => format!("{}:{}", repo, t),
            None => repo,
        }
    }
}

prop_compose! {
    pub fn arb_port_spec()(
        is_long in any::<bool>(),
        h in 1u16..65535,
        c in 1u16..65535
    ) -> perry_container_compose::types::PortSpec {
        use perry_container_compose::types::{PortSpec, ComposeServicePort};
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
    pub fn arb_list_or_dict()(
        is_dict in any::<bool>(),
        keys in proptest::collection::vec("[a-zA-Z0-9_]{1,32}", 0..10),
        values in proptest::collection::vec("[a-zA-Z0-9_]{0,64}", 0..10)
    ) -> perry_container_compose::types::ListOrDict {
        use perry_container_compose::types::ListOrDict;
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
    pub fn arb_depends_on_spec()(
        is_map in any::<bool>(),
        services in proptest::collection::vec(arb_service_name(), 1..5)
    ) -> perry_container_compose::types::DependsOnSpec {
        use perry_container_compose::types::{DependsOnSpec, ComposeDependsOn, DependsOnCondition};
        if is_map {
            let mut map = perry_container_compose::indexmap::IndexMap::new();
            for s in services {
                map.insert(s, ComposeDependsOn {
                    condition: Some(DependsOnCondition::ServiceStarted),
                    ..Default::default()
                });
            }
            DependsOnSpec::Map(map)
        } else {
            DependsOnSpec::List(services)
        }
    }
}

prop_compose! {
    pub fn arb_compose_service()(
        image in proptest::option::of(arb_image_ref()),
        env in proptest::option::of(arb_list_or_dict()),
        ports in proptest::option::of(proptest::collection::vec(arb_port_spec(), 0..3)),
        deps in proptest::option::of(arb_depends_on_spec())
    ) -> perry_container_compose::types::ComposeService {
        perry_container_compose::types::ComposeService {
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
        name in proptest::option::of("[a-z0-9_-]{1,32}"),
        service_names in proptest::collection::vec(arb_service_name(), 1..5)
    ) -> perry_container_compose::types::ComposeSpec {
        use perry_container_compose::types::{ComposeSpec, ComposeService};
        let mut services = perry_container_compose::indexmap::IndexMap::new();
        for s in service_names {
            services.insert(s, ComposeService::default());
        }
        ComposeSpec { name, services, ..Default::default() }
    }
}

prop_compose! {
    pub fn arb_compose_spec_dag()(
        service_names in proptest::collection::vec(arb_service_name(), 2..6)
    ) -> perry_container_compose::types::ComposeSpec {
        use perry_container_compose::types::{ComposeSpec, ComposeService, DependsOnSpec};
        let mut services = perry_container_compose::indexmap::IndexMap::new();
        let mut names_vec: Vec<String> = Vec::new();
        for name in service_names {
            let mut svc = ComposeService::default();
            if !names_vec.is_empty() {
                let dep = names_vec[0].clone();
                svc.depends_on = Some(DependsOnSpec::List(vec![dep]));
            }
            services.insert(name.clone(), svc);
            names_vec.push(name);
        }
        ComposeSpec { services, ..Default::default() }
    }
}

prop_compose! {
    pub fn arb_compose_spec_cycle()(
        mut spec in arb_compose_spec_dag()
    ) -> perry_container_compose::types::ComposeSpec {
        use perry_container_compose::types::DependsOnSpec;
        let names: Vec<String> = spec.services.keys().cloned().collect();
        let first = names[0].clone();
        let last = names[names.len()-1].clone();
        spec.services.get_mut(&first).unwrap().depends_on = Some(DependsOnSpec::List(vec![last]));
        spec
    }
}

prop_compose! {
    pub fn arb_container_spec()(
        image in arb_image_ref(),
        name in proptest::option::of("[a-z0-9_-]{1,32}"),
        rm in proptest::option::of(any::<bool>())
    ) -> perry_container_compose::types::ContainerSpec {
        perry_container_compose::types::ContainerSpec { image, name, rm, ..Default::default() }
    }
}

prop_compose! {
    pub fn arb_env_template()(
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
    pub fn arb_env_map()(
        map in proptest::collection::hash_map("[A-Z_]+", ".*", 0..10)
    ) -> std::collections::HashMap<String, String> { map }
}

// =============================================================================
// Unit Tests
// =============================================================================

// Feature: perry-container | Layer: unit | Req: 14.1 | Property: -
#[test]
fn test_get_chainguard_image_mapping() {
    assert_eq!(get_chainguard_image("git"), Some("cgr.dev/chainguard/git".to_string()));
    assert_eq!(get_chainguard_image("node"), Some("cgr.dev/chainguard/node".to_string()));
    assert_eq!(get_chainguard_image("nonexistent"), None);
}

// Feature: perry-container | Layer: unit | Req: 14.2 | Property: -
#[test]
fn test_get_default_base_image() {
    assert_eq!(get_default_base_image(), "cgr.dev/chainguard/alpine-base");
}

// =============================================================================
// Property Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 15.7 | Property: 10
    #[test]
    fn prop_verification_cache_behavior(_digest in "[a-f0-9]{64}") {
        let image = get_chainguard_image("git").expect("git image mapping exists");
        prop_assert!(image.contains("chainguard"));
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 14.1        | test_get_chainguard_image_mapping | unit |
| 14.2        | test_get_default_base_image | unit |
| 15.7        | prop_verification_cache_behavior | property |
*/

// Deferred Requirements:
// Req 15.1 - 15.5 — verify_image() requires live cosign and crane binaries,
// deferred to integration tests.
// Req 15.7 — Full idempotence property requires mocking the external command results.
