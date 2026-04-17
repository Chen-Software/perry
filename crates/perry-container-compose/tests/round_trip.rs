use perry_container_compose::types::*;
use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::backend::CliProtocol;
use proptest::prelude::*;
use std::collections::HashMap;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

prop_compose! {
    fn arb_service_name()(s in "[a-z][a-z0-9_-]{0,10}") -> String { s }
}

prop_compose! {
    fn arb_image_ref()(repo in "[a-z0-9]+", tag in "[a-z0-9]+") -> String {
        format!("{}:{}", repo, tag)
    }
}

prop_compose! {
    fn arb_list_or_dict()(
        map in proptest::collection::hash_map("[a-z]+", proptest::option::of(any::<String>()), 0..10),
        use_list in any::<bool>()
    ) -> ListOrDict {
        if use_list {
            ListOrDict::List(map.into_iter().map(|(k, v)| format!("{}={}", k, v.unwrap_or_default())).collect())
        } else {
            let mut imap = indexmap::IndexMap::new();
            for (k, v) in map {
                imap.insert(k, v.map(serde_yaml::Value::String));
            }
            ListOrDict::Dict(imap)
        }
    }
}

prop_compose! {
    fn arb_port_spec()(
        target in 1u32..65535,
        published in proptest::option::of(1u32..65535),
        use_long in any::<bool>()
    ) -> PortSpec {
        if use_long {
            PortSpec::Long(ComposeServicePort {
                target: serde_yaml::Value::Number(target.into()),
                published: published.map(|p| serde_yaml::Value::Number(p.into())),
                ..Default::default()
            })
        } else {
            match published {
                Some(p) => PortSpec::Short(serde_yaml::Value::String(format!("{}:{}", p, target))),
                None => PortSpec::Short(serde_yaml::Value::Number(target.into())),
            }
        }
    }
}

prop_compose! {
    fn arb_depends_on_spec()(
        services in proptest::collection::vec(arb_service_name(), 0..5),
        use_map in any::<bool>()
    ) -> DependsOnSpec {
        if use_map {
            let mut map = indexmap::IndexMap::new();
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
    fn arb_compose_service()(
        image in proptest::option::of(arb_image_ref()),
        ports in proptest::option::of(proptest::collection::vec(arb_port_spec(), 0..3)),
        env in proptest::option::of(arb_list_or_dict()),
        depends_on in proptest::option::of(arb_depends_on_spec())
    ) -> ComposeService {
        ComposeService {
            image,
            ports,
            environment: env,
            depends_on,
            ..Default::default()
        }
    }
}

prop_compose! {
    fn arb_compose_spec()(
        name in proptest::option::of("[a-z]+"),
        services in proptest::collection::hash_map(arb_service_name(), arb_compose_service(), 1..10)
    ) -> ComposeSpec {
        let mut imap = indexmap::IndexMap::new();
        for (k, v) in services {
            imap.insert(k, v);
        }
        ComposeSpec {
            name,
            services: imap,
            ..Default::default()
        }
    }
}

fn arb_compose_spec_dag() -> impl Strategy<Value = ComposeSpec> {
    proptest::collection::vec(arb_service_name(), 2..10).prop_flat_map(|names| {
        let mut uniq_names = Vec::new();
        for name in names {
            if !uniq_names.contains(&name) {
                uniq_names.push(name);
            }
        }
        if uniq_names.len() < 2 {
             uniq_names.push("fallback1".to_string());
             uniq_names.push("fallback2".to_string());
        }

        let mut svc_deps = Vec::new();
        for (i, name) in uniq_names.iter().enumerate() {
            let deps = if i > 0 {
                vec![uniq_names[0].clone()]
            } else {
                vec![]
            };
            svc_deps.push((name.clone(), deps));
        }
        Just(svc_deps)
    }).prop_map(|svc_deps| {
        let mut services = indexmap::IndexMap::new();
        for (name, deps) in svc_deps {
            let mut svc = ComposeService::default();
            if !deps.is_empty() {
                svc.depends_on = Some(DependsOnSpec::List(deps));
            }
            services.insert(name, svc);
        }
        ComposeSpec { services, ..Default::default() }
    })
}

fn arb_compose_spec_cycle() -> impl Strategy<Value = ComposeSpec> {
    arb_compose_spec_dag().prop_flat_map(|spec| {
        let names: Vec<String> = spec.services.keys().cloned().collect();
        let len = names.len();
        (Just(spec), Just(len))
    }).prop_map(|(mut spec, len)| {
        let names: Vec<String> = spec.services.keys().cloned().collect();
        let svc0 = spec.services.get_mut(&names[0]).unwrap();
        svc0.depends_on = Some(DependsOnSpec::List(vec![names[len-1].clone()]));
        spec
    })
}

prop_compose! {
    fn arb_container_spec()(
        image in arb_image_ref(),
        name in proptest::option::of("[a-z]+"),
        ports in proptest::option::of(proptest::collection::vec("[0-9]+:[0-9]+", 0..3)),
        env in proptest::option::of(proptest::collection::hash_map("[A-Z]+", "[a-z]+", 0..5))
    ) -> ContainerSpec {
        ContainerSpec {
            image,
            name,
            ports,
            env: env.map(|m| m.into_iter().collect()),
            ..Default::default()
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 7.12 | Property: 1
    #[test]
    fn prop_compose_spec_json_round_trip(spec in arb_compose_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: ComposeSpec = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&deserialized).unwrap();
        prop_assert_eq!(json, json2);
    }

    // Feature: perry-container | Layer: property | Req: 6.4 | Property: 3
    #[test]
    fn prop_topological_sort_respects_deps(spec in arb_compose_spec_dag()) {
        let order = resolve_startup_order(&spec).unwrap();
        let positions: HashMap<String, usize> = order.into_iter().enumerate().map(|(i, s)| (s, i)).collect();

        for (name, svc) in &spec.services {
            if let Some(deps) = &svc.depends_on {
                for dep in deps.service_names() {
                    prop_assert!(positions[&dep] < positions[name]);
                }
            }
        }
    }

    // Feature: perry-container | Layer: property | Req: 6.5 | Property: 4
    #[test]
    fn prop_cycle_detection_completeness(spec in arb_compose_spec_cycle()) {
        let res = resolve_startup_order(&spec);
        prop_assert!(res.is_err());
        // Verify it returns DependencyCycle
        match res {
            Err(perry_container_compose::error::ComposeError::DependencyCycle { services }) => {
                prop_assert!(!services.is_empty());
            }
            _ => prop_assert!(false, "Expected DependencyCycle error"),
        }
    }

    // Feature: perry-container | Layer: property | Req: 7.1 | Property: 5
    #[test]
    fn prop_yaml_round_trip(spec in arb_compose_spec()) {
        let yaml = spec.to_yaml().unwrap();
        let parsed = ComposeSpec::parse_str(&yaml).unwrap();
        let yaml2 = parsed.to_yaml().unwrap();
        prop_assert_eq!(yaml, yaml2);
    }

    // Feature: perry-container | Layer: property | Req: 7.10 | Property: 7
    #[test]
    fn prop_merge_last_writer_wins(
        spec1 in arb_compose_spec(),
        spec2 in arb_compose_spec()
    ) {
        let mut merged = spec1.clone();
        merged.merge(spec2.clone());

        for (name, svc2) in &spec2.services {
            let m_svc = merged.services.get(name).unwrap();
            prop_assert_eq!(serde_json::to_value(m_svc).unwrap(), serde_json::to_value(svc2).unwrap());
        }

        for (name, svc1) in &spec1.services {
            if !spec2.services.contains_key(name) {
                let m_svc = merged.services.get(name).unwrap();
                prop_assert_eq!(serde_json::to_value(m_svc).unwrap(), serde_json::to_value(svc1).unwrap());
            }
        }
    }

    // Feature: perry-container | Layer: property | Req: 12.5 | Property: 2
    #[test]
    fn prop_container_spec_cli_round_trip(spec in arb_container_spec()) {
        let protocol = perry_container_compose::backend::DockerProtocol;
        let args = protocol.run_args(&spec);
        // Minimal check that critical fields are in args
        prop_assert!(args.iter().any(|a: &String| a.contains(&spec.image)));
        if let Some(name) = &spec.name {
            prop_assert!(args.iter().any(|a: &String| a == name));
        }
    }

    // Feature: perry-container | Layer: property | Req: 12.2 | Property: 11
    #[test]
    fn prop_error_propagation(
        code in -127i32..127i32,
        msg in "[a-zA-Z0-9 ]*"
    ) {
        let err = perry_container_compose::error::ComposeError::BackendError { code, message: msg.clone() };
        let js = perry_container_compose::error::compose_error_to_js(&err);
        let expected_code = format!("\"code\":{}", code);
        prop_assert!(js.contains(&expected_code));
    }
}
