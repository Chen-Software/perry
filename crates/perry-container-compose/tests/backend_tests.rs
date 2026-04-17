// Feature: perry-container | Layer: unit | Req: 1.1 | Property: -

#[cfg(test)]
mod tests {
    use perry_container_compose::backend::*;
    use perry_container_compose::types::*;
    use std::collections::HashMap;

    // Feature: perry-container | Layer: unit | Req: 1.1 | Property: 2
    #[test]
    fn test_docker_protocol_run_args() {
        let proto = DockerProtocol;
        let mut env = HashMap::new();
        env.insert("K".into(), "V".into());
        let spec = ContainerSpec {
            image: "alpine".into(),
            name: Some("test-cnt".into()),
            env: Some(env),
            ports: Some(vec!["80:80".into()]),
            rm: Some(true),
            ..Default::default()
        };
        let args = proto.run_args(&spec);
        assert!(args.contains(&"run".to_string()));
        assert!(args.contains(&"--detach".to_string()));
        assert!(args.contains(&"K=V".to_string()));
    }

    // Feature: perry-container | Layer: unit | Req: 1.2 | Property: -
    #[test]
    fn test_apple_protocol_run_args_no_detach() {
        let proto = AppleContainerProtocol;
        let spec = ContainerSpec {
            image: "alpine".into(),
            ..Default::default()
        };
        let args = proto.run_args(&spec);
        assert!(args.contains(&"run".to_string()));
        assert!(!args.contains(&"--detach".to_string()));
    }

    // Feature: perry-container | Layer: unit | Req: 1.2 | Property: -
    #[test]
    fn test_lima_protocol_subcommand_prefix() {
        let proto = LimaProtocol { instance: "default".into() };
        let spec = ContainerSpec { image: "alpine".into(), ..Default::default() };
        let args = proto.run_args(&spec);
        assert_eq!(args[0], "shell");
        assert_eq!(args[1], "default");
        assert_eq!(args[2], "nerdctl");
    }

    // Feature: perry-container | Layer: unit | Req: 3.1 | Property: -
    #[test]
    fn test_parse_list_output_docker() {
        let proto = DockerProtocol;
        let json = r#"{"ID":"123","Names":["web"],"Image":"nginx","Status":"Up","Ports":[],"Created":"..."}"#;
        let res = proto.parse_list_output(json);
        match res {
            Ok(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0].id, "123");
            }
            Err(_) => panic!("Should parse"),
        }
    }

    // Feature: perry-container | Layer: unit | Req: 3.2 | Property: -
    #[test]
    fn test_parse_inspect_output_docker() {
        let proto = DockerProtocol;
        let json = r#"[{"Id":"abc","Name":"cnt","Config":{"Image":"img"},"State":{"Status":"running"},"Created":"..."}]"#;
        let res = proto.parse_inspect_output(json);
        match res {
            Ok(v) => assert_eq!(v.id, "abc"),
            Err(_) => panic!("Should parse"),
        }
    }

    // Feature: perry-container | Layer: unit | Req: 1.6 | Property: -
    #[tokio::test]
    async fn test_detect_backend_env_override() {
        std::env::set_var("PERRY_CONTAINER_BACKEND", "nonexistent");
        let result = detect_backend().await;
        match result {
            Err(probes) => {
                assert_eq!(probes.len(), 1);
                assert_eq!(probes[0].name, "nonexistent");
            }
            Ok(_) => {
                std::env::remove_var("PERRY_CONTAINER_BACKEND");
                panic!("Should fail");
            }
        }
        std::env::remove_var("PERRY_CONTAINER_BACKEND");
    }
}

// Feature: perry-container | Layer: property | Req: 12.5 | Property: 2
use proptest::prelude::*;
use perry_container_compose::backend::*;
use perry_container_compose::types::*;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// Required Generators
prop_compose! { pub fn arb_service_name()(name in "[a-z0-9_-]{1,64}") -> String { name } }
prop_compose! { pub fn arb_image_ref()(repo in "[a-z0-9/._-]{1,128}", tag in proptest::option::of("[a-z0-9._-]{1,32}")) -> String { match tag { Some(t) => format!("{}:{}", repo, t), None => repo } } }
prop_compose! { pub fn arb_port_spec()(is_long in any::<bool>(), h in 1u16..65535, c in 1u16..65535) -> PortSpec { if is_long { PortSpec::Long(ComposeServicePort { target: serde_yaml::Value::Number(c.into()), published: Some(serde_yaml::Value::Number(h.into())), ..Default::default() }) } else { PortSpec::Short(serde_yaml::Value::String(format!("{}:{}", h, c))) } } }
prop_compose! { pub fn arb_list_or_dict()(is_dict in any::<bool>(), keys in proptest::collection::vec("[a-zA-Z0-9_]{1,32}", 0..10), values in proptest::collection::vec("[a-zA-Z0-9_]{0,64}", 0..10)) -> ListOrDict { if is_dict { let mut map = indexmap::IndexMap::new(); for (k, v) in keys.into_iter().zip(values.into_iter()) { map.insert(k, Some(serde_yaml::Value::String(v))); } ListOrDict::Dict(map) } else { ListOrDict::List(keys.into_iter().zip(values.into_iter()).map(|(k, v)| format!("{}={}", k, v)).collect()) } } }
prop_compose! { pub fn arb_depends_on_spec()(is_map in any::<bool>(), services in proptest::collection::vec(arb_service_name(), 1..5)) -> DependsOnSpec { if is_map { let mut map = indexmap::IndexMap::new(); for s in services { map.insert(s, ComposeDependsOn { condition: Some(DependsOnCondition::ServiceStarted), ..Default::default() }); } DependsOnSpec::Map(map) } else { DependsOnSpec::List(services) } } }
prop_compose! { pub fn arb_compose_service()(image in proptest::option::of(arb_image_ref()), env in proptest::option::of(arb_list_or_dict()), ports in proptest::option::of(proptest::collection::vec(arb_port_spec(), 0..3)), deps in proptest::option::of(arb_depends_on_spec())) -> ComposeService { ComposeService { image, environment: env, ports, depends_on: deps, ..Default::default() } } }
prop_compose! { pub fn arb_compose_spec()(name in proptest::option::of("[a-z0-9_-]{1,32}"), service_names in proptest::collection::vec(arb_service_name(), 1..5)) -> ComposeSpec { let mut services = indexmap::IndexMap::new(); for s in service_names { services.insert(s, ComposeService::default()); } ComposeSpec { name, services, ..Default::default() } } }
prop_compose! { pub fn arb_compose_spec_dag()(service_names in proptest::collection::vec(arb_service_name(), 2..6)) -> ComposeSpec { let mut services = indexmap::IndexMap::new(); let mut names_vec: Vec<String> = Vec::new(); for name in service_names { let mut svc = ComposeService::default(); if !names_vec.is_empty() { let dep = names_vec[0].clone(); svc.depends_on = Some(DependsOnSpec::List(vec![dep])); } services.insert(name.clone(), svc); names_vec.push(name); } ComposeSpec { services, ..Default::default() } } }
prop_compose! { pub fn arb_compose_spec_cycle()(mut spec in arb_compose_spec_dag()) -> ComposeSpec { let names: Vec<String> = spec.services.keys().cloned().collect(); let first = names[0].clone(); let last = names[names.len()-1].clone(); spec.services.get_mut(&first).unwrap().depends_on = Some(DependsOnSpec::List(vec![last])); spec } }
prop_compose! { pub fn arb_container_spec()(image in arb_image_ref(), name in proptest::option::of("[a-z0-9_-]{1,32}"), rm in proptest::option::of(any::<bool>())) -> ContainerSpec { ContainerSpec { image, name, rm, ..Default::default() } } }
prop_compose! { pub fn arb_env_template()(var in "[A-Z_][A-Z0-9_]*", default in proptest::option::of("[a-z0-9]*")) -> String { match default { Some(d) => format!("${{{}:-{}}}", var, d), None => format!("${{{}}}", var) } } }
prop_compose! { pub fn arb_env_map()(map in proptest::collection::hash_map("[A-Z_]+", ".*", 0..10)) -> std::collections::HashMap<String, String> { map } }

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 12.5 | Property: 2
    #[test]
    fn prop_container_spec_cli_round_trip(spec in arb_container_spec()) {
        let protocol = DockerProtocol;
        let args = protocol.run_args(&spec);
        prop_assert!(args.contains(&spec.image));
        if let Some(ref name) = spec.name {
            prop_assert!(args.contains(name));
        }
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 1.1         | test_docker_protocol_run_args | unit |
| 1.2         | test_apple_protocol_run_args_no_detach | unit |
| 1.2         | test_lima_protocol_subcommand_prefix | unit |
| 1.6         | test_detect_backend_env_override | unit |
| 2.1         | test_docker_protocol_run_args | unit |
| 3.1         | test_parse_list_output_docker | unit |
| 3.2         | test_parse_inspect_output_docker | unit |
| 12.5        | prop_container_spec_cli_round_trip | property |
*/

// Deferred Requirements:
// Req 16.1-16.12 — Full detect_backend() probe logic requires complex FS/binary mocks.
