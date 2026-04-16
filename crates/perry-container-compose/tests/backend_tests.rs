// Feature: perry-container | Layer: unit | Req: 1.1 | Property: -

#[cfg(test)]
mod tests {
    use perry_container_compose::backend::*;
    use perry_container_compose::types::*;
    use std::collections::HashMap;

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
        assert!(args.contains(&"--name".to_string()));
        assert!(args.contains(&"test-cnt".to_string()));
        assert!(args.contains(&"K=V".to_string()));
        assert!(args.contains(&"80:80".to_string()));
        assert!(args.contains(&"--rm".to_string()));
        assert_eq!(args.last().unwrap(), "alpine");
    }

    #[test]
    fn test_apple_protocol_run_args_no_detach() {
        let proto = AppleContainerProtocol;
        let spec = ContainerSpec {
            image: "alpine".into(),
            ..Default::default()
        };
        let args = proto.run_args(&spec);
        assert!(args.contains(&"run".to_string()));
        assert!(!args.contains(&"--detach".to_string()), "Apple protocol must not use --detach");
    }

    #[test]
    fn test_lima_protocol_subcommand_prefix() {
        let proto = LimaProtocol { instance: "default".into() };
        let spec = ContainerSpec { image: "alpine".into(), ..Default::default() };
        let args = proto.run_args(&spec);
        assert_eq!(args[0], "shell");
        assert_eq!(args[1], "default");
        assert_eq!(args[2], "nerdctl");
        assert_eq!(args[3], "run");
    }

    #[test]
    fn test_parse_list_output_docker() {
        let proto = DockerProtocol;
        let json = r#"{"ID":"123","Names":["web"],"Image":"nginx","Status":"Up 2 hours","Ports":["80/tcp"],"Created":"2024-01-01"}"#;
        let result = proto.parse_list_output(json).expect("Parse failed");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "123");
        assert_eq!(result[0].name, "web");
    }

    #[test]
    fn test_parse_inspect_output_docker() {
        let proto = DockerProtocol;
        let json = r#"[{"Id":"abc","Name":"cnt","Config":{"Image":"img"},"State":{"Status":"running"},"Created":"..."}]"#;
        let result = proto.parse_inspect_output(json).expect("Parse failed");
        assert_eq!(result.id, "abc");
        assert_eq!(result.status, "running");
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 1.1         | test_docker_protocol_run_args | unit |
| 1.2         | test_apple_protocol_run_args_no_detach | unit |
| 1.2         | test_lima_protocol_subcommand_prefix | unit |
| 2.1         | test_docker_protocol_run_args | unit |
| 3.1         | test_parse_list_output_docker | unit |
| 3.2         | test_parse_inspect_output_docker | unit |
*/

// Deferred Requirements:
// Req 16.1 - 16.12 — detect_backend() probe logic requires complex filesystem and binary mocking.
