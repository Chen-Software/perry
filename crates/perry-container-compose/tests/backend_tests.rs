use perry_container_compose::backend::*;
use perry_container_compose::types::ContainerSpec;
use std::env;

// Feature: perry-container | Layer: unit | Req: 1.5 | Property: -
#[test]
fn test_platform_candidates_logic() {
    let candidates = platform_candidates();
    if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        assert!(candidates.contains(&"apple/container"));
        assert!(candidates.contains(&"docker"));
    } else if cfg!(target_os = "linux") {
        assert!(candidates.contains(&"podman"));
        assert!(candidates.contains(&"docker"));
    }
}

// Feature: perry-container | Layer: unit | Req: 1.1 | Property: -
#[tokio::test]
async fn test_detect_backend_env_override() {
    env::set_var("PERRY_CONTAINER_BACKEND", "invalid-backend-name");
    let result = detect_backend().await;
    match result {
        Err(probed) => {
            assert_eq!(probed.len(), 1);
            assert_eq!(probed[0].name, "invalid-backend-name");
            assert!(!probed[0].available);
        }
        _ => panic!("Expected failure for invalid backend override"),
    }
    env::remove_var("PERRY_CONTAINER_BACKEND");
}

// Feature: perry-container | Layer: unit | Req: 1.1 | Property: 2
#[test]
fn test_docker_protocol_run_args() {
    let spec = ContainerSpec {
        image: "alpine:latest".into(),
        name: Some("test-srv".into()),
        rm: Some(true),
        ..Default::default()
    };
    let args = DockerProtocol.run_args(&spec);
    assert!(args.contains(&"run".to_string()));
    assert!(args.contains(&"--detach".to_string()));
    assert!(args.contains(&"--name".to_string()));
    assert!(args.contains(&"test-srv".to_string()));
    assert!(args.contains(&"--rm".to_string()));
    assert!(args.contains(&"alpine:latest".to_string()));
}

// Feature: perry-container | Layer: unit | Req: 1.2 | Property: 2
#[test]
fn test_apple_protocol_run_args_no_detach() {
    let spec = ContainerSpec {
        image: "alpine:latest".into(),
        ..Default::default()
    };
    let args = AppleContainerProtocol.run_args(&spec);
    assert!(args.contains(&"run".to_string()));
    assert!(!args.contains(&"--detach".to_string()), "Apple Container should not use --detach");
}
