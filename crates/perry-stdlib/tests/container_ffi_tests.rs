// Feature: perry-container | Layer: ffi-contract | Req: 11.1 | Property: -

use perry_runtime::{Promise, StringHeader};
use perry_stdlib::container::*;
use perry_container_compose::types::*;
use perry_stdlib::types::ContainerSpec;
use proptest::prelude::*;
use std::collections::HashMap;
use std::ptr;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// =============================================================================
// Helper Functions for FFI Testing
// =============================================================================

/// Drives a promise to completion using the runtime microtask runner and stdlib pump.
unsafe fn await_promise_sync(promise: *mut Promise) -> f64 {
    assert!(!promise.is_null(), "Promise pointer must not be null");
    let mut iterations = 0;

    // 0 is Pending in PromiseState
    while perry_runtime::js_promise_state(promise) == 0 && iterations < 2000 {
        // Pump the stdlib async bridge to move resolutions to the microtask queue
        perry_stdlib::common::js_stdlib_process_pending();
        perry_runtime::js_promise_run_microtasks();
        std::thread::sleep(std::time::Duration::from_millis(1));
        iterations += 1;
    }

    let state = perry_runtime::js_promise_state(promise);
    assert!(state != 0, "Promise timed out after {} iterations", iterations);

    perry_runtime::js_promise_result(promise)
}

/// Creates a StringHeader from a Rust string for passing to FFI.
unsafe fn make_js_string(s: &str) -> *const StringHeader {
    perry_runtime::js_string_from_bytes(s.as_ptr(), s.len() as u32)
}

/// Verifies that a JSValue bits represent a specific error JSON payload.
unsafe fn assert_is_error_json(val_bits: f64, expected_code: i64) {
    let bits = val_bits.to_bits();
    // STRING_TAG is 0x7FFF according to CLAUDE.md
    assert_eq!(bits >> 48, 0x7FFF, "Result should be a NaN-boxed string (tag 0x7FFF), got 0x{:X}", bits >> 48);

    let ptr = (bits & 0x0000_FFFF_FFFF_FFFF) as *const StringHeader;
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    let s = String::from_utf8_lossy(bytes);

    let v: serde_json::Value = serde_json::from_str(&s).expect("Result should be valid JSON");
    assert!(v.get("message").is_some(), "Error should have a message");
    assert_eq!(v["code"], expected_code, "Error code mismatch");
}

// =============================================================================
// FFI Contract Tests
// =============================================================================

macro_rules! test_ffi_contract {
    ($name:ident, $func:ident($($arg:expr),*)) => {
        #[test]
        fn $name() {
            unsafe {
                let p = $func($($arg),*);
                assert!(!p.is_null());
                assert_is_error_json(await_promise_sync(p), 400);
            }
        }
    };
}

// perry/container
test_ffi_contract!(test_js_container_run_null, js_container_run(ptr::null()));
test_ffi_contract!(test_js_container_create_null, js_container_create(ptr::null()));
test_ffi_contract!(test_js_container_start_null, js_container_start(ptr::null()));
test_ffi_contract!(test_js_container_stop_null, js_container_stop(ptr::null(), -1));
test_ffi_contract!(test_js_container_remove_null, js_container_remove(ptr::null(), 0));
test_ffi_contract!(test_js_container_inspect_null, js_container_inspect(ptr::null()));
test_ffi_contract!(test_js_container_logs_null, js_container_logs(ptr::null(), -1));
test_ffi_contract!(test_js_container_exec_null, js_container_exec(ptr::null(), ptr::null(), ptr::null(), ptr::null()));
test_ffi_contract!(test_js_container_pull_image_null, js_container_pullImage(ptr::null()));
test_ffi_contract!(test_js_container_remove_image_null, js_container_removeImage(ptr::null(), 0));
test_ffi_contract!(test_js_container_compose_up_null, js_container_composeUp(ptr::null()));

// perry/compose
test_ffi_contract!(test_js_compose_up_null, js_compose_up(ptr::null()));
test_ffi_contract!(test_js_compose_down_null, js_compose_down(0, 0));
test_ffi_contract!(test_js_compose_ps_null, js_compose_ps(0));
test_ffi_contract!(test_js_compose_logs_null, js_compose_logs(0, ptr::null(), -1));
test_ffi_contract!(test_js_compose_exec_null, js_compose_exec(0, ptr::null(), ptr::null()));
test_ffi_contract!(test_js_compose_config_null, js_compose_config(ptr::null()));
test_ffi_contract!(test_js_compose_start_null, js_compose_start(0, ptr::null()));
test_ffi_contract!(test_js_compose_stop_null, js_compose_stop(0, ptr::null()));
test_ffi_contract!(test_js_compose_restart_null, js_compose_restart(0, ptr::null()));

// Feature: perry-container | Layer: ffi-contract | Req: 1.4 | Property: -
#[test]
fn test_js_container_get_backend_contract() {
    unsafe {
        let ptr = js_container_getBackend();
        assert!(!ptr.is_null());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 3.1 | Property: -
#[test]
fn test_js_container_list_contract() {
    unsafe {
        let p = js_container_list(0);
        assert!(!p.is_null());
    }
}

// Feature: perry-container | Layer: ffi-contract | Req: 5.2 | Property: -
#[test]
fn test_js_container_list_images_contract() {
    unsafe {
        let p = js_container_listImages();
        assert!(!p.is_null());
    }
}

// =============================================================================
// Property-Based Generators
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
                let dep = names_vec[0].clone(); // Simple DAG
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
// Coverage Table
// =============================================================================

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 1.4         | test_js_container_get_backend_contract | ffi-contract |
| 2.1         | test_js_container_run_null | ffi-contract |
| 2.2         | test_js_container_create_null | ffi-contract |
| 2.3         | test_js_container_start_null | ffi-contract |
| 2.4         | test_js_container_stop_null | ffi-contract |
| 2.5         | test_js_container_remove_null | ffi-contract |
| 3.1         | test_js_container_list_contract | ffi-contract |
| 3.2         | test_js_container_inspect_null | ffi-contract |
| 4.1         | test_js_container_logs_null | ffi-contract |
| 4.3         | test_js_container_exec_null | ffi-contract |
| 5.1         | test_js_container_pull_image_null | ffi-contract |
| 5.2         | test_js_container_list_images_contract | ffi-contract |
| 5.3         | test_js_container_remove_image_null | ffi-contract |
| 6.1         | test_js_container_compose_up_null | ffi-contract |
| 11.2        | test_js_compose_up_null | ffi-contract |
| 6.6         | test_js_compose_down_null | ffi-contract |
| 6.6         | test_js_compose_ps_null | ffi-contract |
| 6.6         | test_js_compose_logs_null | ffi-contract |
| 6.6         | test_js_compose_exec_null | ffi-contract |
| 8.7         | test_js_compose_config_null | ffi-contract |
| 8.2         | test_js_compose_start_null | ffi-contract |
| 8.2         | test_js_compose_stop_null | ffi-contract |
| 8.2         | test_js_compose_restart_null | ffi-contract |
*/

// Deferred Requirements:
// Req 1.8 — detectBackend() async probing requires complex platform mock setup.
// Req 11.6 — js_container_module_init() is a no-op side effect.
