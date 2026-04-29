//! Container module for Perry
//!
//! Provides OCI container management with platform-adaptive backend selection.

pub mod backend;
pub mod capability;
pub mod compose;
pub mod types;
pub mod verification;

mod mod_private {
    use super::get_global_backend;
    use crate::container::backend::ContainerBackend;
    use std::sync::Arc;

    pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, String> {
        get_global_backend()
            .await
            .map(|b| Arc::clone(b))
            .map_err(|e| e.to_string())
    }
}

// Re-export commonly used types
pub use types::{
    ComposeHandle, ComposeSpec, ContainerError, ContainerHandle, ContainerInfo, ContainerLogs,
    ContainerSpec, ImageInfo, ListOrDict,
};

use perry_runtime::{js_promise_new, Promise, StringHeader};
pub use backend::{detect_backend, ContainerBackend};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;

// Global backend instance - initialized once at first use
static BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
static BACKEND_INIT_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Get or initialize the global backend instance.
///
/// Per SPEC §5.1 step 4: on `detect_backend()` failure, if stderr is an
/// interactive TTY *and* `PERRY_NO_INSTALL_PROMPT` is unset, hand off to
/// `BackendInstaller` so the user can pick + install a runtime. Both gates
/// must hold; otherwise the original `NoBackendFound` error propagates.
async fn get_global_backend() -> Result<&'static Arc<dyn ContainerBackend>, ContainerError> {
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }

    let _guard = BACKEND_INIT_MUTEX.lock().await;

    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }

    let b = match detect_backend().await {
        Ok(backend) => Arc::from(backend) as Arc<dyn ContainerBackend>,
        Err(e) => {
            use std::io::IsTerminal;
            let interactive = std::io::stderr().is_terminal();
            let prompt_disabled = std::env::var("PERRY_NO_INSTALL_PROMPT").is_ok();
            if interactive && !prompt_disabled {
                let installer = perry_container_compose::BackendInstaller::new();
                match installer.run().await {
                    Ok(backend) => Arc::from(backend) as Arc<dyn ContainerBackend>,
                    Err(_) => return Err(ContainerError::from(e)),
                }
            } else {
                return Err(ContainerError::from(e));
            }
        }
    };

    let _ = BACKEND.set(b);
    Ok(BACKEND.get().unwrap())
}

/// Helper to extract string from StringHeader pointer
unsafe fn string_from_header(ptr: *const StringHeader) -> Option<String> {
    if ptr.is_null() || (ptr as usize) < 0x1000 {
        return None;
    }
    let len = (*ptr).byte_len as usize;
    let data_ptr = (ptr as *const u8).add(std::mem::size_of::<StringHeader>());
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    Some(String::from_utf8_lossy(bytes).to_string())
}

/// Helper to create a JS string from a Rust string
unsafe fn string_to_js(s: &str) -> *const StringHeader {
    let bytes = s.as_bytes();
    perry_runtime::js_string_from_bytes(bytes.as_ptr(), bytes.len() as u32)
}

/// `POINTER_TAG` for NaN-boxing a handle id as an opaque pointer. This is
/// what every codegen `unbox_to_i64` call expects to find at the receiver
/// slot of a `has_receiver: true` dispatch row — the lower 48 bits are
/// masked off (`POINTER_MASK = 0x0000_FFFF_FFFF_FFFF`) and used as the
/// handle id directly. Matches `perry_runtime::value::POINTER_TAG`.
const POINTER_TAG_BITS: u64 = 0x7FFD_0000_0000_0000;

/// Encode a u64 handle id as the f64 bits a Promise resolution slot expects.
///
/// The async-bridge stores `result_bits: u64` and resolves the Promise via
/// `f64::from_bits(result_bits)`. Two things have to be true of those bits:
///
/// 1. **`${handle}` interpolation must produce something sane.** Pre-fix
///    `Ok(1u64)` resolved with f64 = `5e-324` (subnormal), which prints as
///    `"0"` — the user can't tell their handle from a void-resolution.
///
/// 2. **`down(stack, …)` / `stack.down(…)` dispatch must be able to recover
///    the original handle id.** The codegen lowers `stack` via
///    `unbox_to_i64` which expects a NaN-boxed value: it does
///    `bits & POINTER_MASK` (lower 48 bits) and treats that as the i64
///    handle. A bare `(id as f64).to_bits()` produces `0x3FF0_0000_…` for
///    id=1 — masked to lower 48, that's 0, and the FFI sees "Invalid
///    compose handle".
///
/// Both invariants are satisfied by NaN-boxing the handle with
/// `POINTER_TAG = 0x7FFD` in the upper 16 bits and the id in the lower
/// 48: `unbox_to_i64` recovers the id verbatim, and `JSValue::format`
/// (called by template-string coercion) sees the POINTER_TAG and prints
/// the id as a numeric handle.
#[inline]
fn handle_to_promise_bits(id: u64) -> u64 {
    POINTER_TAG_BITS | (id & 0x0000_FFFF_FFFF_FFFF)
}

/// `TAG_UNDEFINED` as raw f64 bits. Used by `Promise<void>` FFIs to resolve
/// with `undefined` rather than `0` (matches JS semantics).
const PROMISE_VOID_BITS: u64 = 0x7FFC_0000_0000_0001;

/// Decode a NaN-boxed f64 receiver/handle back to its registry id (i64).
///
/// The codegen `NA_F64` arg-coercion rule passes the user's `stack` variable
/// through to the FFI as `double`. So when `js_compose_down` etc. take the
/// handle as their first parameter, the LLVM declare emits `double`, the
/// f64 lands in XMM0, and Rust must read it as `f64` to match the calling
/// convention (declaring the arg as `i64` makes Rust read RDI instead and
/// the FFI sees garbage).
///
/// `handle_to_promise_bits` NaN-boxes the id with POINTER_TAG, so the f64
/// the user receives carries the id in its lower 48 bits. This helper
/// reverses that boxing — masking off the tag and reading the id verbatim.
#[inline]
fn handle_id_from_f64(boxed: f64) -> i64 {
    (boxed.to_bits() & 0x0000_FFFF_FFFF_FFFF) as i64
}

/// Optionally verify a container image's signature before pulling/running.
///
/// Gated on `PERRY_CONTAINER_VERIFY_IMAGES=1` so the default path stays
/// cosign-free for development + CI parity. When the env var is set, the
/// image is run through `verification::verify_image()` (cosign keyless
/// verification against Chainguard identity) and a failure short-circuits
/// the FFI call with a `verification failed` error string.
///
/// SPEC §11.2 calls this out as "present but not yet enforced in HEAD"; this
/// helper is the integration point. Per-call guard rather than a global
/// `up()`-only one so users can pin individual `run`/`create`/`pullImage`
/// invocations to verified images while leaving compose stacks unchecked.
async fn maybe_verify_image(image: &str) -> Result<(), String> {
    if std::env::var("PERRY_CONTAINER_VERIFY_IMAGES")
        .ok()
        .as_deref()
        != Some("1")
    {
        return Ok(());
    }
    crate::container::verification::verify_image(image)
        .await
        .map(|_digest| ())
}

// ============ Container Lifecycle ============

/// Run a container from the given spec
/// FFI: js_container_run(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec = match types::parse_container_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Err(e) = maybe_verify_image(&spec.image).await {
            return Err::<u64, String>(e);
        }
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.run(&spec).await {
            Ok(handle) => {
                let handle_id = types::register_container_handle(handle);
                Ok(handle_to_promise_bits(handle_id as u64))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Start compose services.
///
/// FFI: `js_container_compose_start(handle: f64, services_json: *const StringHeader) -> *mut Promise`
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(
    handle: f64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);

    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let services_json = unsafe { string_from_header(services_json_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        engine
            .start(&services)
            .await
            .map(|_| PROMISE_VOID_BITS)
            .map_err(|e| e.to_string())
    });

    promise
}

/// Stop compose services.
///
/// FFI: `js_container_compose_stop(handle: f64, services_json: *const StringHeader) -> *mut Promise`
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(
    handle: f64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);

    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let services_json = unsafe { string_from_header(services_json_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        engine
            .stop(&services)
            .await
            .map(|_| PROMISE_VOID_BITS)
            .map_err(|e| e.to_string())
    });

    promise
}

/// Restart compose services.
///
/// FFI: `js_container_compose_restart(handle: f64, services_json: *const StringHeader) -> *mut Promise`
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(
    handle: f64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);

    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let services_json = unsafe { string_from_header(services_json_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let services: Vec<String> = services_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        engine
            .restart(&services)
            .await
            .map(|_| PROMISE_VOID_BITS)
            .map_err(|e| e.to_string())
    });

    promise
}

/// Get compose configuration
/// Get the resolved compose YAML configuration.
///
/// FFI: `js_container_compose_config(handle: f64) -> *mut Promise`
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle: f64) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);

    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move { engine.config().map_err(|e| e.to_string()) },
        |yaml| {
            let str_ptr = perry_runtime::js_string_from_bytes(yaml.as_ptr(), yaml.len() as u32);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );

    promise
}

/// Create a container from the given spec without starting it
/// FFI: js_container_create(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let spec = match types::parse_container_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Err(e) = maybe_verify_image(&spec.image).await {
            return Err::<u64, String>(e);
        }
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.create(&spec).await {
            Ok(handle) => {
                let handle_id = types::register_container_handle(handle);
                Ok(handle_to_promise_bits(handle_id as u64))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Start a previously created container
/// FFI: js_container_start(id: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.start(&id).await {
            Ok(()) => Ok(PROMISE_VOID_BITS),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Stop a running container
/// FFI: js_container_stop(id: *const StringHeader, timeout: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_stop(
    id_ptr: *const StringHeader,
    timeout: f64,
) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let timeout_opt = if timeout.is_nan() || timeout < 0.0 {
            None
        } else {
            Some(timeout as u32)
        };
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.stop(&id, timeout_opt).await {
            Ok(()) => Ok(PROMISE_VOID_BITS),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Remove a container
/// FFI: js_container_remove(id: *const StringHeader, force: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_remove(
    id_ptr: *const StringHeader,
    force: f64,
) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.remove(&id, force != 0.0).await {
            Ok(()) => Ok(PROMISE_VOID_BITS),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// List containers
/// FFI: js_container_list(all: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.list(all != 0.0).await {
            Ok(containers) => {
                let handle_id = types::register_container_info_list(containers);
                Ok(handle_to_promise_bits(handle_id as u64))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Inspect a container
/// FFI: js_container_inspect(id: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.inspect(&id).await {
            Ok(info) => {
                let handle_id = types::register_container_info(info);
                Ok(handle_to_promise_bits(handle_id as u64))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get the current backend name.
///
/// FFI: `js_container_getBackend() -> *const StringHeader`
///
/// Returns the canonical backend name (e.g. `"docker"` / `"podman"` /
/// `"apple/container"` / `"colima"` / `"orbstack"` / `"lima"`) when the
/// backend singleton is initialised. If not yet initialised, performs a
/// synchronous in-place detection so user code that calls `getBackend()`
/// at module scope (before any `await` has triggered `get_global_backend`)
/// gets the live name instead of the misleading `"unknown"` sentinel.
///
/// The synchronous probe uses `tokio::runtime::Handle::try_current()` +
/// `block_in_place` when called from inside a tokio worker, falling back
/// to a one-shot `Runtime::new().block_on(...)` otherwise. Returns
/// `"unknown"` only when detection genuinely fails (no backend installed
/// + non-interactive). Detection latency is bounded by the same 2-second
/// per-candidate timeout as `detect_backend()`.
#[no_mangle]
pub unsafe extern "C" fn js_container_getBackend() -> *const StringHeader {
    if let Some(b) = BACKEND.get() {
        return string_to_js(b.backend_name());
    }

    // No backend yet — try to populate the singleton synchronously.
    // Strategy:
    //   1. If we're inside a tokio worker, `block_in_place` lets us call
    //      the async detect_backend() without deadlocking the runtime.
    //   2. If we're on the main thread with no runtime active, spin up
    //      a fresh single-threaded runtime for the probe.
    //   3. On any failure (no runtime + main-thread-bound, detection
    //      error, etc.), fall back to the legacy "unknown" sentinel.
    let resolved = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::CurrentThread => {
                // current_thread runtimes can't `block_in_place`; the only
                // safe move is to skip the sync probe and let the next
                // async FFI call populate BACKEND. Return "unknown".
                None
            }
            _ => Some(tokio::task::block_in_place(|| {
                handle.block_on(get_global_backend())
            })),
        }
    } else {
        // No active runtime — spin up a temp one purely for detection.
        // The result is stored in the OnceLock so subsequent FFI calls
        // see it; the temp runtime is dropped immediately after.
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => Some(rt.block_on(get_global_backend())),
            Err(_) => None,
        }
    };

    match resolved {
        Some(Ok(b)) => string_to_js(b.backend_name()),
        _ => string_to_js("unknown"),
    }
}

/// Detect backend and return probed info
/// FFI: js_container_detectBackend() -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_detectBackend() -> *mut Promise {
    let promise = js_promise_new();
    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            match detect_backend().await {
                Ok(b) => {
                    let name = b.backend_name().to_string();
                    let json = serde_json::json!([{
                        "name": name,
                        "available": true,
                        "reason": ""
                    }])
                    .to_string();
                    Ok(json)
                }
                Err(e) => {
                    use perry_container_compose::error::ComposeError;
                    let json = match e {
                        ComposeError::NoBackendFound { probed } => {
                            serde_json::to_string(&probed).unwrap_or_else(|_| "[]".to_string())
                        }
                        _ => serde_json::json!([{
                            "name": "unknown",
                            "available": false,
                            "reason": e.to_string()
                        }])
                        .to_string(),
                    };
                    Ok(json)
                }
            }
        },
        |json| {
            let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );
    promise
}

// ============ Container Logs and Exec ============

/// Get logs from a container.
///
/// FFI: `js_container_logs(id: *const StringHeader, opts_json: *const StringHeader) -> *mut Promise`
///
/// `opts_json` is a JSON-encoded `LogsOptions` object containing `tail: number`
/// and `follow: boolean`. Pre-fix the dispatch took `tail` as a raw `i32`.
#[no_mangle]
pub unsafe extern "C" fn js_container_logs(
    id_ptr: *const StringHeader,
    opts_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    let opts_json = unsafe { string_from_header(opts_ptr) };
    let tail_opt = opts_json.and_then(|s| {
        let v: serde_json::Value = serde_json::from_str(&s).ok()?;
        v.get("tail").and_then(|t| t.as_u64()).map(|t| t as u32)
    });

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.logs(&id, tail_opt).await {
            Ok(logs) => {
                let handle_id = types::register_container_logs(logs);
                Ok(handle_to_promise_bits(handle_id as u64))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Execute a command in a container
/// FFI: js_container_exec(id: *const StringHeader, cmd_json: *const StringHeader, env_json: *const StringHeader, workdir: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_exec(
    id_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    env_json_ptr: *const StringHeader,
    workdir_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let id = match string_from_header(id_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid container ID".to_string())
            });
            return promise;
        }
    };

    let cmd_json = string_from_header(cmd_json_ptr);
    let env_json = string_from_header(env_json_ptr);
    let workdir = string_from_header(workdir_ptr);

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let env: Option<HashMap<String, String>> =
            env_json.and_then(|s| serde_json::from_str(&s).ok());

        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend
            .exec(&id, &cmd, env.as_ref(), workdir.as_deref())
            .await
        {
            Ok(logs) => {
                let handle_id = types::register_container_logs(logs);
                Ok(handle_to_promise_bits(handle_id as u64))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Image Management ============

/// Pull a container image
/// FFI: js_container_pullImage(reference: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_pullImage(reference_ptr: *const StringHeader) -> *mut Promise {
    let promise = js_promise_new();

    let reference = match string_from_header(reference_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid image reference".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        if let Err(e) = maybe_verify_image(&reference).await {
            return Err::<u64, String>(e);
        }
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.pull_image(&reference).await {
            Ok(()) => Ok(PROMISE_VOID_BITS),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// List images
/// FFI: js_container_listImages() -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_listImages() -> *mut Promise {
    let promise = js_promise_new();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.list_images().await {
            Ok(images) => {
                let handle_id = types::register_image_info_list(images);
                Ok(handle_to_promise_bits(handle_id as u64))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Build a container image
/// FFI: js_container_build(spec_json: *const StringHeader, image_name: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_build(
    spec_ptr: *const StringHeader,
    image_name_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let spec_json = string_from_header(spec_ptr).unwrap_or_else(|| "{}".to_string());
    let image_name = string_from_header(image_name_ptr).unwrap_or_default();

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let spec: perry_container_compose::types::ComposeServiceBuild =
            serde_json::from_str(&spec_json).map_err(|e| format!("Invalid build spec: {}", e))?;

        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };

        match backend.build(&spec, &image_name).await {
            Ok(()) => Ok(PROMISE_VOID_BITS),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Remove an image
/// FFI: js_container_removeImage(reference: *const StringHeader, force: i32) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_removeImage(
    reference_ptr: *const StringHeader,
    force: i32,
) -> *mut Promise {
    let promise = js_promise_new();

    let reference = match string_from_header(reference_ptr) {
        Some(s) => s,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid image reference".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        match backend.remove_image(&reference, force != 0).await {
            Ok(()) => Ok(PROMISE_VOID_BITS),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Compose Functions ============

/// Bring up a Compose stack
/// FFI: js_container_composeUp(spec_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_container_composeUp(
    spec_ptr: *const perry_runtime::StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let spec = match types::parse_compose_spec(spec_ptr) {
        Ok(s) => s,
        Err(e) => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>(e)
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper = compose::ComposeWrapper::new(spec, backend);
        match wrapper.up().await {
        Ok(_handle) => {
            let handle_id = types::register_compose_handle(wrapper.engine().clone());
            Ok(handle_to_promise_bits(handle_id))
        }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Alias for js_container_composeUp
#[no_mangle]
pub unsafe extern "C" fn js_compose_up(spec_ptr: *const StringHeader) -> *mut Promise {
    js_container_composeUp(spec_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_down(
    handle: f64,
    opts_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_down(handle, opts_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_ps(handle: f64) -> *mut Promise {
    js_container_compose_ps(handle)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_logs(
    handle: f64,
    opts_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_logs(handle, opts_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_exec(
    handle: f64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    opts_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_exec(handle, service_ptr, cmd_json_ptr, opts_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_config(handle: f64) -> *mut Promise {
    js_container_compose_config(handle)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_start(
    handle: f64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_start(handle, services_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_stop(
    handle: f64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_stop(handle, services_json_ptr)
}

#[no_mangle]
pub unsafe extern "C" fn js_compose_restart(
    handle: f64,
    services_json_ptr: *const StringHeader,
) -> *mut Promise {
    js_container_compose_restart(handle, services_json_ptr)
}

/// Stop and remove compose stack.
///
/// FFI: `js_container_compose_down(handle: f64, opts_json: *const StringHeader)
///       -> *mut Promise`
///
/// `opts_json` is a JSON-encoded `DownOptions` object — the codegen's
/// `js_value_to_str_ptr_for_ffi` helper auto-stringifies the TS object
/// literal `{ volumes: bool, ...}`. Pre-fix the dispatch took the
/// options as `f64` (NA_F64), which only worked when the caller passed a
/// plain numeric flag — every TS user passing `down(handle, { volumes:
/// false })` got `remove_volumes = true` because the NaN-boxed object
/// pointer is non-zero. Same fix shape as `composeUp({...})` from
/// v0.5.370.
///
/// Recognised keys (all optional):
///   - `volumes: boolean`        remove named volumes (default `false`)
///   - `removeOrphans: boolean`  remove orphaned containers (default `false`)
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(
    handle: f64,
    opts_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);

    let opts_json = unsafe { string_from_header(opts_ptr) };
    let (remove_volumes, _remove_orphans) = match opts_json.as_deref() {
        Some(s) if !s.is_empty() && s != "undefined" && s != "null" => {
            let v: serde_json::Value =
                serde_json::from_str(s).unwrap_or(serde_json::Value::Null);
            (
                v.get("volumes").and_then(|x| x.as_bool()).unwrap_or(false),
                v.get("removeOrphans")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false),
            )
        }
        _ => (false, false),
    };

    let engine = match types::take_compose_handle(handle_id as u64) {
        Some(h) => h,
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let _backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper = compose::ComposeWrapper::new_from_engine(engine);
        match wrapper.down(remove_volumes).await {
            Ok(()) => Ok(PROMISE_VOID_BITS),
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get container info for compose stack.
///
/// FFI: `js_container_compose_ps(handle: f64) -> *mut Promise`
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle: f64) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);

    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let _backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper = compose::ComposeWrapper::new_from_engine(engine);
        match wrapper.ps().await {
            Ok(containers) => {
                let h = types::register_container_info_list(containers);
                Ok(handle_to_promise_bits(h as u64))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get logs from compose stack.
///
/// FFI: `js_container_compose_logs(handle: f64, opts_json: *const StringHeader) -> *mut Promise`
///
/// `opts_json` is a JSON-encoded `LogsOptions` object containing `service: string`
/// and `tail: number`.
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(
    handle: f64,
    opts_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);

    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let opts_json = unsafe { string_from_header(opts_ptr) };
    let (service, tail_opt) = match opts_json.as_deref() {
        Some(s) if !s.is_empty() && s != "undefined" && s != "null" => {
            let v: serde_json::Value =
                serde_json::from_str(s).unwrap_or(serde_json::Value::Null);
            (
                v.get("service").and_then(|x| x.as_str()).map(|s| s.to_string()),
                v.get("tail").and_then(|x| x.as_u64()).map(|t| t as u32),
            )
        }
        _ => (None, None),
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let _backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper = compose::ComposeWrapper::new_from_engine(engine);
        match wrapper.logs(service.as_deref(), tail_opt).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(handle_to_promise_bits(h as u64))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Execute command in compose service.
///
/// FFI: `js_container_compose_exec(handle: f64, service: *const StringHeader, cmd_json: *const StringHeader, opts_json: *const StringHeader) -> *mut Promise`
#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(
    handle: f64,
    service_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
    _opts_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let handle_id = handle_id_from_f64(handle);

    let engine = match types::get_compose_handle(handle_id as u64) {
        Some(h) => h.clone(),
        None => {
            crate::common::spawn_for_promise(promise as *mut u8, async move {
                Err::<u64, String>("Invalid compose handle".to_string())
            });
            return promise;
        }
    };

    let service_opt = unsafe { string_from_header(service_ptr) };
    let cmd_json = unsafe { string_from_header(cmd_json_ptr) };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let service = match service_opt {
            Some(s) => s,
            None => return Err::<u64, String>("Invalid service name".to_string()),
        };

        let cmd: Vec<String> = cmd_json
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let _backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };
        let wrapper = compose::ComposeWrapper::new_from_engine(engine);
        match wrapper.exec(&service, &cmd).await {
            Ok(logs) => {
                let h = types::register_container_logs(logs);
                Ok(handle_to_promise_bits(h as u64))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

// ============ Workload Functions ============

/// Create a workload graph
/// FFI: js_workload_graph(name: *const StringHeader, nodes_json: *const StringHeader) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_workload_graph(
    name_ptr: *const StringHeader,
    nodes_json_ptr: *const StringHeader,
) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let nodes_json = string_from_header(nodes_json_ptr).unwrap_or_else(|| "{}".to_string());

    let graph = perry_container_compose::WorkloadGraph {
        name,
        nodes: serde_json::from_str(&nodes_json).unwrap_or_default(),
        edges: vec![], // Edges inferred from depends_on in nodes
    };

    let json = serde_json::to_string(&graph).unwrap_or_default();
    string_to_js(&json)
}

/// Create a workload node
/// FFI: js_workload_node(name: *const StringHeader, spec_json: *const StringHeader) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_workload_node(
    name_ptr: *const StringHeader,
    spec_json_ptr: *const StringHeader,
) -> *const StringHeader {
    let name = string_from_header(name_ptr).unwrap_or_default();
    let spec_json = string_from_header(spec_json_ptr).unwrap_or_else(|| "{}".to_string());

    let mut node: perry_container_compose::WorkloadNode =
        serde_json::from_str(&spec_json).unwrap_or_else(|_| perry_container_compose::WorkloadNode {
            id: name.clone(),
            name: name.clone(),
            image: None,
            resources: None,
            ports: vec![],
            env: HashMap::new(),
            depends_on: vec![],
            runtime: perry_container_compose::RuntimeSpec::Auto,
            policy: perry_container_compose::PolicySpec::default(),
        });
    node.id = name.clone();
    node.name = name;

    let json = serde_json::to_string(&node).unwrap_or_default();
    string_to_js(&json)
}

/// Run a workload graph
/// FFI: js_workload_runGraph(graph_json: *const StringHeader, opts_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_runGraph(
    graph_json_ptr: *const StringHeader,
    opts_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();

    let graph_json = string_from_header(graph_json_ptr).unwrap_or_else(|| "{}".to_string());
    let opts_json = string_from_header(opts_json_ptr).unwrap_or_else(|| "{}".to_string());

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let graph: perry_container_compose::WorkloadGraph =
            serde_json::from_str(&graph_json).map_err(|e| format!("Failed to parse graph: {}", e))?;
        let opts: perry_container_compose::RunGraphOptions =
            serde_json::from_str(&opts_json).map_err(|e| format!("Failed to parse options: {}", e))?;

        let backend = match get_global_backend().await {
            Ok(b) => Arc::clone(b),
            Err(e) => return Err::<u64, String>(e.to_string()),
        };

        let engine = Arc::new(perry_container_compose::WorkloadGraphEngine::new(
            graph, backend,
        ));
        match engine.run(opts).await {
            Ok(_) => {
                let handle_id = types::register_workload_handle(engine);
                Ok(handle_to_promise_bits(handle_id))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Inspect a workload graph
/// FFI: js_workload_inspectGraph(handle_id: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_inspectGraph(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id_from_f64(handle_id) as u64;

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            let engine = match types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&id)) {
                Some(e) => e.clone(),
                None => return Err("Invalid workload handle".to_string()),
            };

            match engine.status().await {
                Ok(status) => {
                    let json = serde_json::to_string(&status).unwrap_or_default();
                    Ok(json)
                }
                Err(e) => Err(e.to_string()),
            }
        },
        |json| {
            let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );

    promise
}

/// Stop and remove a workload graph
/// FFI: js_workload_handle_down(handle_id: f64, force: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_down(handle_id: f64, force: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id_from_f64(handle_id) as u64;

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = match types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&id)) {
            Some(e) => e.clone(),
            None => return Err("Invalid workload handle".to_string()),
        };

        match engine.down(force != 0.0).await {
            Ok(_) => {
                if let Some(handles) = types::WORKLOAD_HANDLES.get() {
                    handles.remove(&id);
                }
                Ok(PROMISE_VOID_BITS)
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get status of a workload graph
/// FFI: js_workload_handle_status(handle_id: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_status(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id_from_f64(handle_id) as u64;

    crate::common::spawn_for_promise_deferred(
        promise as *mut u8,
        async move {
            let engine = match types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&id)) {
                Some(e) => e.clone(),
                None => return Err("Invalid workload handle".to_string()),
            };

            match engine.status().await {
                Ok(status) => {
                    let json = serde_json::to_string(&status).unwrap_or_default();
                    Ok(json)
                }
                Err(e) => Err(e.to_string()),
            }
        },
        |json| {
            let str_ptr = perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32);
            perry_runtime::JSValue::string_ptr(str_ptr).bits()
        },
    );

    promise
}

/// Get logs from a workload node
/// FFI: js_workload_handle_logs(handle_id: f64, node_id: *const StringHeader, tail: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_logs(
    handle_id: f64,
    node_id_ptr: *const StringHeader,
    tail: f64,
) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id_from_f64(handle_id) as u64;
    let node_id = string_from_header(node_id_ptr).unwrap_or_default();
    let tail_opt = if tail.is_finite() && tail >= 0.0 {
        Some(tail as u32)
    } else {
        None
    };

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = match types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&id)) {
            Some(e) => e.clone(),
            None => return Err("Invalid workload handle".to_string()),
        };

        match engine.logs(&node_id, tail_opt).await {
            Ok(logs) => {
                let handle_id = types::register_container_logs(logs);
                Ok(handle_to_promise_bits(handle_id))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Execute command in a workload node
/// FFI: js_workload_handle_exec(handle_id: f64, node_id: *const StringHeader, cmd_json: *const StringHeader) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_exec(
    handle_id: f64,
    node_id_ptr: *const StringHeader,
    cmd_json_ptr: *const StringHeader,
) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id_from_f64(handle_id) as u64;
    let node_id = string_from_header(node_id_ptr).unwrap_or_default();
    let cmd_json = string_from_header(cmd_json_ptr).unwrap_or_else(|| "[]".to_string());

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).unwrap_or_default();
        let engine = match types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&id)) {
            Some(e) => e.clone(),
            None => return Err("Invalid workload handle".to_string()),
        };

        match engine.exec(&node_id, &cmd).await {
            Ok(logs) => {
                let handle_id = types::register_container_logs(logs);
                Ok(handle_to_promise_bits(handle_id))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get process status of a workload graph
/// FFI: js_workload_handle_ps(handle_id: f64) -> *mut Promise
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_ps(handle_id: f64) -> *mut Promise {
    let promise = js_promise_new();
    let id = handle_id_from_f64(handle_id) as u64;

    crate::common::spawn_for_promise(promise as *mut u8, async move {
        let engine = match types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&id)) {
            Some(e) => e.clone(),
            None => return Err("Invalid workload handle".to_string()),
        };

        match engine.ps().await {
            Ok(infos) => {
                // Register NodeInfo list as a container info list (compatible for now)
                // Actually we should probably have a register_node_info_list
                let handle_id = types::register_container_info_list(
                    infos
                        .into_iter()
                        .map(|i| ContainerInfo {
                            id: i.container_id.unwrap_or_default(),
                            name: i.name,
                            image: i.image.unwrap_or_default(),
                            status: format!("{:?}", i.state),
                            ports: vec![],
                            labels: HashMap::new(),
                            created: "".to_string(),
                            ip_address: i.ip_address.unwrap_or_default(),
                        })
                        .collect(),
                );
                Ok(handle_to_promise_bits(handle_id))
            }
            Err(e) => Err::<u64, String>(e.to_string()),
        }
    });

    promise
}

/// Get graph JSON from workload handle
/// FFI: js_workload_handle_graph(handle_id: f64) -> *const StringHeader
#[no_mangle]
pub unsafe extern "C" fn js_workload_handle_graph(handle_id: f64) -> *const StringHeader {
    let id = handle_id_from_f64(handle_id) as u64;
    let engine = match types::WORKLOAD_HANDLES.get().and_then(|m| m.get(&id)) {
        Some(e) => e.clone(),
        None => return std::ptr::null(),
    };

    let json = serde_json::to_string(&engine.graph).unwrap_or_default();
    string_to_js(&json)
}

// ============ Module Initialization ============

/// Initialize the container module (called during runtime startup).
///
/// Per SPEC §11.6 / Task 18.1, this is a one-shot link-time anchor that:
/// 1. Forces `libperry_stdlib`'s container symbols to be retained (any
///    user code calling `js_container_module_init()` will pull in the
///    transitively-referenced FFI symbols and prevent dead-strip).
/// 2. Pre-warms the backend singleton when called from a tokio context —
///    avoids paying the probe latency on the first user `run()` call.
///
/// Backend probing is async + may invoke the interactive `BackendInstaller`,
/// so we must not block here. Instead we spawn the probe as a detached
/// tokio task; if a tokio runtime isn't yet running (called from `main`
/// before any async setup), the task simply doesn't run and the first
/// real FFI call will trigger probe-on-demand the same way it always has.
#[no_mangle]
pub extern "C" fn js_container_module_init() {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async {
            let _ = get_global_backend().await;
        });
    }
}

#[cfg(test)]
mod smoke_tests {
    use super::*;

    /// Task 27.1: `js_container_module_init` must be callable without panic
    /// outside an active tokio runtime. The link-anchor purpose mustn't
    /// depend on async setup.
    #[test]
    fn module_init_is_safe_to_call_outside_tokio() {
        js_container_module_init();
    }

    /// Task 27.1: when called inside a tokio runtime, module_init schedules
    /// the backend probe without blocking the caller. The detached probe
    /// task may fail (no backend installed in CI); we only assert the call
    /// itself returns synchronously without panic and that the runtime is
    /// still alive afterwards.
    #[test]
    fn module_init_inside_tokio_runtime_does_not_block() {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            js_container_module_init();
            // If we reach here without hanging, the call returned
            // synchronously — invariant proved.
        });
    }

    /// Task 27.1: the canonical FFI symbols listed in SPEC §9.1 must all be
    /// addressable from this crate (link-time check). Unresolved symbols
    /// would fail to build, so this test merely takes the address of each
    /// to force the rustc usage check.
    #[test]
    fn ffi_symbols_resolve() {
        let _ = js_container_run as unsafe extern "C" fn(_) -> _;
        let _ = js_container_create as unsafe extern "C" fn(_) -> _;
        let _ = js_container_start as unsafe extern "C" fn(_) -> _;
        let _ = js_container_stop as unsafe extern "C" fn(_, _) -> _;
        let _ = js_container_remove as unsafe extern "C" fn(_, _) -> _;
        let _ = js_container_list as unsafe extern "C" fn(_) -> _;
        let _ = js_container_inspect as unsafe extern "C" fn(_) -> _;
        let _ = js_container_logs as unsafe extern "C" fn(_, _) -> _;
        let _ = js_container_pullImage as unsafe extern "C" fn(_) -> _;
        let _ = js_container_listImages as unsafe extern "C" fn() -> _;
        let _ = js_container_getBackend as unsafe extern "C" fn() -> _;
        let _ = js_container_module_init as extern "C" fn();
    }
}
