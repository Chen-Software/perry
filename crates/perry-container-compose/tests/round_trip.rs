//! Property-based tests for perry-container-compose.
//!
//! Uses the `proptest` crate to verify correctness properties
//! across serialization, dependency resolution, YAML parsing,
//! env interpolation, and type validation.

use indexmap::IndexMap;
use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::error::ComposeError;
use perry_container_compose::backend::{CliProtocol, DockerProtocol};
use perry_container_compose::error::compose_error_to_js;
use perry_container_compose::types::{
    ComposeService, ComposeSpec, ContainerSpec, DependsOnCondition, DependsOnSpec, VolumeType,
};
use perry_container_compose::yaml::interpolate;
use proptest::prelude::*;
use std::collections::HashMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// ============ Arbitrary Strategies ============

prop_compose! {
    /// Generate a valid image reference string.
    fn arb_image_ref()(s in "[a-z][a-z0-9_-]{1,15}(:[a-z0-9._-]+)?") -> String {
        s
    }
}

prop_compose! {
    /// Generate a valid service name.
    fn arb_service_name()(s in "[a-z][a-z0-9_-]{1,10}") -> String {
        s
    }
}

prop_compose! {
    /// Generate an arbitrary ComposeSpec with 1–10 services.
    fn arb_compose_spec()(
        services_vec in proptest::collection::vec(
            (arb_service_name(), arb_image_ref()).prop_map(|(name, image)| {
                let mut svc = ComposeService::default();
                svc.image = Some(image);
                (name, svc)
            }),
            1..=10,
        )
    ) -> ComposeSpec {
        let mut services = IndexMap::new();
        for (name, svc) in services_vec {
            services.insert(name, svc);
        }
        ComposeSpec {
            services,
            ..Default::default()
        }
    }
}

prop_compose! {
    /// Generate a ComposeSpec with a valid (acyclic) depends_on DAG.
    fn arb_compose_spec_dag()(
        items in proptest::collection::vec(
            (arb_service_name(), proptest::collection::vec(0..10usize, 0..=3)),
            2..=8,
        )
    ) -> ComposeSpec {
        // Build a valid DAG: only allow deps on services that appear
        // earlier in the list (forward references only).
        let mut services = IndexMap::new();
        let names: Vec<String> = items.iter().map(|(n, _)| n.clone()).collect();

        for (i, (name, dep_indices)) in items.into_iter().enumerate() {
            let mut svc = ComposeService::default();
            svc.image = Some(format!("{}:latest", name));

            // Only keep deps that point to earlier services (guarantees no cycles)
            let valid_deps: Vec<String> = dep_indices
                .into_iter()
                .filter(|&idx| idx < i)
                .map(|idx| names[idx].clone())
                .collect();

            if !valid_deps.is_empty() {
                svc.depends_on = Some(DependsOnSpec::List(valid_deps));
            }
            services.insert(name, svc);
        }

        ComposeSpec {
            services,
            ..Default::default()
        }
    }
}

prop_compose! {
    /// Generate a ComposeSpec with exactly one cycle.
    fn arb_compose_spec_cycle()(
        spec in arb_compose_spec_dag(),
        back_edge in (0..8usize, 0..8usize)
    ) -> ComposeSpec {
        let mut services = spec.services;
        let names: Vec<String> = services.keys().cloned().collect();
        let n = names.len();

        let i = back_edge.0 % n;
        let j = back_edge.1 % n;

        let (j, i) = if j < i { (j, i) } else if j > i { (i, j) } else {
            if i + 1 < n { (i, i+1) } else if i > 0 { (i-1, i) } else { (0, 0) }
        };

        if i == j {
            let name = &names[i];
            let svc = services.get_mut(name).unwrap();
            svc.depends_on = Some(DependsOnSpec::List(vec![name.clone()]));
        } else {
            let name_i = &names[i];
            let name_j = &names[j];

            let svc_i = services.get_mut(name_i).unwrap();
            svc_i.depends_on = Some(DependsOnSpec::List(vec![name_j.clone()]));

            let svc_j = services.get_mut(name_j).unwrap();
            svc_j.depends_on = Some(DependsOnSpec::List(vec![name_i.clone()]));
        }

        ComposeSpec {
            services,
            ..Default::default()
        }
    }
}

prop_compose! {
    /// Generate an arbitrary ContainerSpec.
    fn arb_container_spec()(
        image in arb_image_ref(),
        name in proptest::option::of(arb_service_name()),
        ports in proptest::option::of(proptest::collection::vec("[0-9]{2,5}:[0-9]{2,5}", 0..=3)),
        volumes in proptest::option::of(proptest::collection::vec("/[a-z]:/[a-z]", 0..=3)),
    ) -> ContainerSpec {
        ContainerSpec {
            image,
            name,
            ports,
            volumes,
            ..Default::default()
        }
    }
}

prop_compose! {
    /// Generate environment variable name.
    fn arb_env_name()(s in "[A-Z][A-Z0-9_]{1,8}") -> String {
        s
    }
}

prop_compose! {
    /// Generate a template string containing ${VAR} and ${VAR:-default} patterns.
    fn arb_env_template_data()(
        var1 in arb_env_name(),
        var2 in arb_env_name(),
        default in "[a-z0-9_]{0,10}"
    ) -> (String, HashMap<String, String>) {
        let mut env = HashMap::new();
        env.insert(var1.clone(), "value1".to_string());
        // var2 is intentionally missing from env to test defaults

        let template = format!("prefix_${{{}}}_mid_${{{}:-{}}}_suffix", var1, var2, default);
        (template, env)
    }
}

// ============ Property 2: ContainerSpec CLI argument round-trip ============
// Feature: perry-container | Layer: property | Req: 12.5 | Property: 2
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_container_spec_cli_round_trip(spec in arb_container_spec()) {
        let protocol = DockerProtocol;
        let args = protocol.run_args(&spec);

        if let Some(name) = &spec.name {
            prop_assert!(args.contains(&"--name".to_string()));
            prop_assert!(args.contains(name));
        }
        prop_assert!(args.contains(&spec.image));
    }
}

// ============ Property 11: Error propagation preserves code and message ============
// Feature: perry-container | Layer: property | Req: 2.6 | Property: 11
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_error_propagation(code in -100i32..500i32, message in ".*") {
        let err = ComposeError::BackendError { code, message: message.clone() };
        let js_json = compose_error_to_js(&err);
        let val: serde_json::Value = serde_json::from_str(&js_json).map_err(|e| TestCaseError::fail(e.to_string()))?;

        prop_assert_eq!(val["code"].as_i64().ok_or(TestCaseError::fail("missing code"))? as i32, code);
        prop_assert!(val["message"].as_str().ok_or(TestCaseError::fail("missing message"))?.contains(&message));
    }
}

// ============ Property 1: ComposeSpec JSON round-trip ============
// Feature: perry-container | Layer: property | Req: 7.12 | Property: 1
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_compose_spec_json_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let deserialized: ComposeSpec = serde_json::from_str(&json).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let json2 = serde_json::to_string(&deserialized).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(json, json2);
    }
}

// ============ Property 3: Topological sort respects depends_on ============
// Feature: perry-container | Layer: property | Req: 6.4 | Property: 3
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_dag()) {
        let order = resolve_startup_order(&spec).map_err(|e| TestCaseError::fail(e.to_string()))?;

        let pos: HashMap<&str, usize> = order
            .iter()
            .enumerate()
            .map(|(i, s)| (s.as_str(), i))
            .collect();

        for (name, service) in &spec.services {
            if let Some(deps) = &service.depends_on {
                for dep in deps.service_names() {
                    if let (Some(&dep_pos), Some(&name_pos)) =
                        (pos.get(dep.as_str()), pos.get(name.as_str()))
                    {
                        prop_assert!(
                            dep_pos < name_pos,
                            "dep {} (pos {}) should come before {} (pos {})",
                            dep, dep_pos, name, name_pos
                        );
                    }
                }
            }
        }

        prop_assert_eq!(order.len(), spec.services.len());
    }
}

// ============ Property 4: Cycle detection is complete ============
// Feature: perry-container | Layer: property | Req: 6.5 | Property: 4
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_cycle_detection_completeness(spec in arb_compose_spec_cycle()) {
        let result = resolve_startup_order(&spec);
        prop_assert!(result.is_err(), "cycle should be detected");

        match result {
            Err(ComposeError::DependencyCycle { services }) => {
                prop_assert!(!services.is_empty(), "cycle must list at least one service");
                for svc in &services {
                    prop_assert!(spec.services.contains_key(svc), "cycle service {} should be defined", svc);
                }
            }
            _ => panic!("expected DependencyCycle error"),
        }
    }
}

// ============ Property 5: YAML round-trip ============
// Feature: perry-container | Layer: property | Req: 7.1 | Property: 5
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_yaml_round_trip(spec in arb_compose_spec()) {
        let yaml = serde_yaml::to_string(&spec).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let reparsed: ComposeSpec = ComposeSpec::parse_str(&yaml).map_err(|e| TestCaseError::fail(e.to_string()))?;

        prop_assert_eq!(
            reparsed.services.keys().collect::<Vec<_>>(),
            spec.services.keys().collect::<Vec<_>>()
        );

        for (name, svc) in &spec.services {
            let reparsed_svc = &reparsed.services[name];
            prop_assert_eq!(reparsed_svc.image.as_deref(), svc.image.as_deref());
        }
    }
}

// ============ Property 6: Environment variable interpolation ============
// Feature: perry-container | Layer: property | Req: 7.8 | Property: 6
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_env_interpolation(data in arb_env_template_data()) {
        let (template, env) = data;
        let result = interpolate(&template, &env);

        prop_assert!(!result.contains("${"), "template should be fully expanded");
        prop_assert!(result.starts_with("prefix_value1_mid_"));
        prop_assert!(result.ends_with("_suffix"));
    }
}

// ============ Property 7: Compose file merge last-writer-wins ============
// Feature: perry-container | Layer: property | Req: 7.10 | Property: 7
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_merge_last_writer_wins(
        common_svc in arb_service_name(),
        only_a_svc in arb_service_name(),
        img_a in arb_image_ref(),
        img_b in arb_image_ref(),
    ) {
        prop_assume!(common_svc != only_a_svc);
        prop_assume!(img_a != img_b);

        let mut spec_a = ComposeSpec::default();
        let mut svc_a_common = ComposeService::default();
        svc_a_common.image = Some(img_a.clone());
        spec_a.services.insert(common_svc.clone(), svc_a_common);

        let mut svc_a_only = ComposeService::default();
        svc_a_only.image = Some(format!("onlya-{}", &common_svc));
        spec_a.services.insert(only_a_svc.clone(), svc_a_only);

        let mut spec_b = ComposeSpec::default();
        let mut svc_b_common = ComposeService::default();
        svc_b_common.image = Some(img_b.clone());
        spec_b.services.insert(common_svc.clone(), svc_b_common);

        spec_a.merge(spec_b);

        prop_assert_eq!(spec_a.services[&common_svc].image.as_deref(), Some(img_b.as_str()));
        prop_assert!(spec_a.services.contains_key(&only_a_svc));
    }
}

// ============ Property 8: DependsOnCondition rejects invalid values ============
// Feature: perry-container | Layer: property | Req: 7.14 | Property: 8
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_depends_on_condition_rejects_invalid(invalid in "[a-z]{3,20}") {
        let valid_values = ["service_started", "service_healthy", "service_completed_successfully"];
        prop_assume!(!valid_values.contains(&invalid.as_str()));

        let yaml = format!("\"{}\"", invalid);
        let result = serde_yaml::from_str::<DependsOnCondition>(&yaml);
        prop_assert!(result.is_err());
    }
}

// ============ Property 9: VolumeType rejects invalid values ============
// Feature: perry-container | Layer: property | Req: 10.14 | Property: 9
proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]
    #[test]
    fn prop_volume_type_rejects_invalid(invalid in "[a-z]{3,20}") {
        let valid_values = ["bind", "volume", "tmpfs", "cluster", "npipe", "image"];
        prop_assume!(!valid_values.contains(&invalid.as_str()));

        let yaml = format!("\"{}\"", invalid);
        let result = serde_yaml::from_str::<VolumeType>(&yaml);
        prop_assert!(result.is_err());
    }
}
