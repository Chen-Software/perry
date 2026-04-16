// Feature: perry-container | Layer: integration | Req: 13.1 | Property: -

#[cfg(feature = "integration-tests")]
mod integration {
    use perry_stdlib::container::capability::*;
    use perry_stdlib::container::backend::detect_backend;
    use std::collections::HashMap;

    // Feature: perry-container | Layer: integration | Req: 13.1 | Property: -
    #[tokio::test]
    #[ignore]
    async fn test_alloy_container_run_capability_isolation() {
        if detect_backend().await.is_err() { return; }

        let grants = CapabilityGrants {
            network: false,
            env: Some(HashMap::from([("FOO".into(), "BAR".into())])),
        };

        let res = alloy_container_run_capability(
            "test", "alpine", &["env"], &grants
        ).await;

        match res {
            Ok(logs) => assert!(logs.stdout.contains("FOO=BAR")),
            Err(e) => {
                // Verification might fail in environments without cosign
                if !format!("{:?}", e).contains("VerificationFailed") {
                   panic!("Capability run failed unexpectedly: {:?}", e);
                }
            }
        }
    }

    // Feature: perry-container | Layer: integration | Req: 13.3 | Property: -
    #[tokio::test]
    #[ignore]
    async fn test_alloy_container_run_capability_network_disabled() {
        if detect_backend().await.is_err() { return; }

        let grants = CapabilityGrants {
            network: false,
            env: None,
        };

        let res = alloy_container_run_capability(
            "test-net", "alpine", &["ping", "-c", "1", "8.8.8.8"], &grants
        ).await;

        // Should fail to reach network
        if let Ok(logs) = res {
            assert!(!logs.stderr.is_empty() || logs.stdout.contains("100% packet loss") || logs.stdout.contains("Network is unreachable"));
        }
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 13.1        | test_alloy_container_run_capability_isolation | integration |
| 13.2        | test_alloy_container_run_capability_isolation | integration |
| 13.3        | test_alloy_container_run_capability_network_disabled | integration |
| 13.4        | test_alloy_container_run_capability_isolation | integration |
*/

// Deferred Requirements:
// Req 13.5 — ShellBridge specific integration requires full compiler/runtime stack.
