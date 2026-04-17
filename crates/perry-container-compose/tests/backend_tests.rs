use perry_container_compose::backend::*;
use std::env;

// Feature: perry-container | Layer: unit | Req: 1.1 | Property: -
#[test]
fn test_platform_candidates_logic() {
    let candidates = platform_candidates();
    assert!(!candidates.is_empty());

    #[cfg(target_os = "macos")]
    {
        assert_eq!(candidates[0], "apple/container");
    }

    #[cfg(target_os = "linux")]
    {
        assert_eq!(candidates[0], "podman");
    }
}

// Feature: perry-container | Layer: unit | Req: 1.1 | Property: -
#[tokio::test]
async fn test_detect_backend_env_override() {
    env::set_var("PERRY_CONTAINER_BACKEND", "nonexistent-backend");
    let res = detect_backend().await;

    match res {
        Err(probed) => {
            assert!(probed.iter().any(|r| r.name == "nonexistent-backend" && !r.available));
        }
        _ => panic!("Expected error for nonexistent backend override"),
    }
    env::remove_var("PERRY_CONTAINER_BACKEND");
}

// Feature: perry-container | Layer: unit | Req: 1.2 | Property: 2
#[test]
fn test_docker_protocol_run_args() {
    let protocol = DockerProtocol;
    let spec = perry_container_compose::types::ContainerSpec {
        image: "alpine".into(),
        name: Some("test".into()),
        ..Default::default()
    };
    let args = protocol.run_args(&spec);
    assert!(args.contains(&"run".into()));
    assert!(args.contains(&"--name".into()));
    assert!(args.contains(&"test".into()));
    assert!(args.contains(&"alpine".into()));
    assert!(args.contains(&"--detach".into()));
}

// Feature: perry-container | Layer: unit | Req: 1.2 | Property: 2
#[test]
fn test_apple_protocol_run_args_no_detach() {
    let protocol = AppleContainerProtocol;
    let spec = perry_container_compose::types::ContainerSpec {
        image: "alpine".into(),
        ..Default::default()
    };
    let args = protocol.run_args(&spec);
    assert!(args.contains(&"run".into()));
    assert!(!args.contains(&"-d".into()));
    assert!(!args.contains(&"--detach".into()));
}
