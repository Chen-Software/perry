// Feature: perry-container | Layer: unit | Req: 2.7 | Property: -

use perry_stdlib::container::types::*;
use perry_container_compose::types::{ContainerHandle, ListOrDict, DependsOnSpec, ComposeDependsOn, DependsOnCondition, ComposeService, ComposeSpec, PortSpec, ContainerSpec};
use perry_container_compose::error::ComposeError;
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
    ) -> PortSpec {
        if is_long {
            PortSpec::Long(perry_container_compose::types::ComposeServicePort {
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
    ) -> ListOrDict {
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
    ) -> DependsOnSpec {
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
    ) -> ComposeService {
        ComposeService {
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
    ) -> ComposeSpec {
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
    ) -> ComposeSpec {
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
    ) -> ComposeSpec {
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
    ) -> ContainerSpec {
        ContainerSpec { image, name, rm, ..Default::default() }
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
    ) -> HashMap<String, String> { map }
}

// =============================================================================
// Unit Tests
// =============================================================================

// Feature: perry-container | Layer: unit | Req: 2.7 | Property: -
#[test]
fn test_container_handle_registry() {
    let handle = ContainerHandle { id: "123".into(), name: Some("test".into()) };
    let id = register_container_handle(handle.clone());

    let retrieved = get_container_handle_typed(id).expect("Should find handle");
    assert_eq!(retrieved.id, "123");

    let taken = take_container_handle_typed(id).expect("Should take handle");
    assert_eq!(taken.id, "123");
    assert!(get_container_handle_typed(id).is_none());
}

// Feature: perry-container | Layer: unit | Req: 16.11 | Property: -
#[test]
fn test_compose_error_to_container_error_conversion() {
    let ce = ComposeError::NotFound("lost".into());
    let err: ContainerError = ce.into();
    match err {
        ContainerError::NotFound(s) => assert_eq!(s, "lost"),
        _ => panic!("Wrong variant"),
    }

    let ce = ComposeError::ValidationError { message: "invalid".into() };
    let err: ContainerError = ce.into();
    match err {
        ContainerError::InvalidConfig(s) => assert_eq!(s, "invalid"),
        _ => panic!("Wrong variant"),
    }
}

// Feature: perry-container | Layer: unit | Req: none | Property: -
#[test]
fn test_container_error_display() {
    let err = ContainerError::VerificationFailed {
        image: "bad-img".into(),
        reason: "no sig".into()
    };
    let s = format!("{}", err);
    assert!(s.contains("bad-img"));
    assert!(s.contains("no sig"));
}

// =============================================================================
// Property Tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: none | Property: -
    #[test]
    fn prop_container_error_display_contains_keyword(
        variant in 0u8..=5,
        msg in "[a-z A-Z0-9_]{1,40}",
    ) {
        let error = match variant {
            0 => ContainerError::NotFound(msg.clone()),
            1 => ContainerError::BackendError {
                code: 1,
                message: msg.clone(),
            },
            2 => ContainerError::VerificationFailed {
                image: msg.clone(),
                reason: "test reason".to_string(),
            },
            3 => ContainerError::DependencyCycle {
                cycle: vec![msg.clone()],
            },
            4 => ContainerError::ServiceStartupFailed {
                service: msg.clone(),
                error: "test error".to_string(),
            },
            _ => ContainerError::InvalidConfig(msg.clone()),
        };

        let display = format!("{}", error);
        let expected_keyword = match variant {
            0 => "not found",
            1 => "Backend error",
            2 => "verification failed",
            3 => "Dependency cycle",
            4 => "failed to start",
            _ => "Invalid configuration",
        };

        prop_assert!(
            display.to_lowercase().contains(&expected_keyword.to_lowercase()),
            "Display output should contain '{}', got: {}",
            expected_keyword,
            display
        );
    }

    // Feature: perry-container | Layer: property | Req: 11.1 | Property: -
    #[test]
    fn prop_handle_registry_type_safety(
        ids in proptest::collection::vec("[a-f0-9]{12}", 1..=3),
        images in proptest::collection::vec("[a-z][a-z0-9_.-]{3,30}", 1..=3),
        stdout in "[a-z0-9 ]{0,50}",
        stderr in "[a-z0-9 ]{0,50}",
    ) {
        use perry_stdlib::container::types::{ContainerInfo, ContainerLogs};

        let infos: Vec<ContainerInfo> = ids
            .iter()
            .zip(images.iter())
            .map(|(id, img)| ContainerInfo {
                id: id.clone(),
                name: format!("svc-{}", &id[..6]),
                image: img.clone(),
                status: "running".to_string(),
                ports: vec![],
                created: "2025-01-01T00:00:00Z".to_string(),
            })
            .collect();

        let h = register_container_info_list(infos.clone());
        let taken: Option<Vec<ContainerInfo>> = take_container_info_list(h);
        prop_assert!(taken.is_some());
        prop_assert_eq!(taken.unwrap().len(), infos.len());

        let logs = ContainerLogs {
            stdout: stdout.clone(),
            stderr: stderr.clone(),
        };
        let lh = register_container_logs(logs);
        let taken_logs: Option<ContainerLogs> = take_container_logs(lh);
        prop_assert!(taken_logs.is_some());
        let taken_logs = taken_logs.unwrap();
        prop_assert_eq!(taken_logs.stdout, stdout);
        prop_assert_eq!(taken_logs.stderr, stderr);
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 2.7         | test_container_handle_registry | unit |
| 11.1        | prop_handle_registry_type_safety | property |
| 16.11       | test_compose_error_to_container_error_conversion | unit |
| none        | test_container_error_display | unit |
| none        | prop_container_error_display_contains_keyword | property |
*/

// Deferred Requirements: none
