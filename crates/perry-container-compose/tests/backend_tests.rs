use perry_container_compose::backend::{CliProtocol, DockerProtocol, AppleContainerProtocol, LimaProtocol};
use perry_container_compose::types::ContainerSpec;
use std::collections::HashMap;
use proptest::prelude::*;

// Feature: perry-container | Layer: unit | Req: 1.1 | Property: -
#[test]
fn test_docker_protocol_flags() {
    let proto = DockerProtocol;
    let mut spec = ContainerSpec::default();
    spec.name = Some("test-container".to_string());
    spec.ports = Some(vec!["8080:80".to_string()]);
    let mut env = HashMap::new();
    env.insert("FOO".to_string(), "BAR".to_string());
    spec.env = Some(env);

    let flags = proto.run_args(&spec);
    assert!(flags.contains(&"--name".to_string()));
    assert!(flags.contains(&"test-container".to_string()));
    assert!(flags.contains(&"-p".to_string()));
    assert!(flags.contains(&"8080:80".to_string()));
    assert!(flags.contains(&"-e".to_string()));
    assert!(flags.contains(&"FOO=BAR".to_string()));
}

// Feature: perry-container | Layer: unit | Req: 1.1 | Property: -
#[test]
fn test_apple_protocol_flags() {
    let proto = AppleContainerProtocol;
    let mut spec = ContainerSpec::default();
    spec.name = Some("apple-test".to_string());

    let flags = proto.run_args(&spec);
    assert!(flags.contains(&"--name".to_string()));
    assert!(flags.contains(&"apple-test".to_string()));
}

// Feature: perry-container | Layer: unit | Req: 1.1 | Property: -
#[test]
fn test_lima_protocol_prefix() {
    let proto = LimaProtocol { instance: "default".to_string() };
    let prefix = proto.subcommand_prefix().unwrap();
    assert_eq!(prefix, vec!["shell".to_string(), "default".to_string(), "nerdctl".to_string()]);
}

// ============ Property Generators ============

prop_compose! {
    fn arb_container_spec()(
        image in "[a-z]{3,10}",
        name in proptest::option::of("[a-z]{3,10}")
    ) -> ContainerSpec {
        ContainerSpec {
            image,
            name,
            ..Default::default()
        }
    }
}

// Feature: perry-container | Layer: property | Req: 12.5 | Property: 2
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn prop_container_spec_cli_round_trip(spec in arb_container_spec()) {
        let protocol = DockerProtocol;
        let args = protocol.run_args(&spec);
        prop_assert!(args.contains(&spec.image));
        if let Some(name) = &spec.name {
            prop_assert!(args.contains(name));
        }
    }
}
