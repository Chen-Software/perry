use crate::container::context::ContainerContext;
use crate::container::types::*;
use crate::container::workload::*;
use perry_runtime::{Promise, StringHeader};
use perry_container_compose::error::ComposeError;
use perry_container_compose::compose::ComposeHandle;
use crate::common::handle::{register_handle, get_handle};
use std::collections::HashMap;

#[no_mangle]
pub unsafe extern "C" fn js_container_module_init() {
    let _ = tokio::runtime::Handle::current().spawn(async {
        let _ = ContainerContext::global().get_backend().await;
    });
}

#[no_mangle]
pub unsafe extern "C" fn js_container_run(spec_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let spec_json = perry_runtime::string_from_header(spec_ptr);
        let spec: ContainerSpec = serde_json::from_str(&spec_json).map_err(|e| ComposeError::InvalidConfig(e.to_string()))?;

        let backend = ContainerContext::global().get_backend().await?;
        let handle = backend.run(&spec).await?;

        Ok(serde_json::to_string(&handle).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_create(spec_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let spec_json = perry_runtime::string_from_header(spec_ptr);
        let spec: ContainerSpec = serde_json::from_str(&spec_json).map_err(|e| ComposeError::InvalidConfig(e.to_string()))?;

        let backend = ContainerContext::global().get_backend().await?;
        let handle = backend.create(&spec).await?;

        Ok(serde_json::to_string(&handle).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_start(id_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let id = perry_runtime::string_from_header(id_ptr);
        let backend = ContainerContext::global().get_backend().await?;
        backend.start(&id).await?;
        Ok("{}".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list(all: f64) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let backend = ContainerContext::global().get_backend().await?;
        let list = backend.list(all != 0.0).await?;
        Ok(serde_json::to_string(&list).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_inspect(id_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let id = perry_runtime::string_from_header(id_ptr);
        let backend = ContainerContext::global().get_backend().await?;
        let info = backend.inspect(&id).await?;
        Ok(serde_json::to_string(&info).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_logs(id_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let id = perry_runtime::string_from_header(id_ptr);
        let backend = ContainerContext::global().get_backend().await?;
        let tail_val = if tail > 0.0 { Some(tail as u32) } else { None };
        let logs = backend.logs(&id, tail_val).await?;
        Ok(serde_json::to_string(&logs).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_exec(id_ptr: *const StringHeader, cmd_ptr: *const StringHeader, env_ptr: *const StringHeader, workdir_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let id = perry_runtime::string_from_header(id_ptr);
        let cmd_json = perry_runtime::string_from_header(cmd_ptr);
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).map_err(|e| ComposeError::InvalidConfig(e.to_string()))?;

        let env_json = if env_ptr.is_null() { None } else { Some(perry_runtime::string_from_header(env_ptr)) };
        let env = env_json.and_then(|s| serde_json::from_str::<HashMap<String, String>>(&s).ok());

        let workdir = if workdir_ptr.is_null() { None } else { Some(perry_runtime::string_from_header(workdir_ptr)) };

        let backend = ContainerContext::global().get_backend().await?;
        let logs = backend.exec(&id, &cmd, env.as_ref(), workdir.as_deref()).await?;
        Ok(serde_json::to_string(&logs).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_stop(id_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let id = perry_runtime::string_from_header(id_ptr);
        let backend = ContainerContext::global().get_backend().await?;
        backend.stop(&id, None).await?;
        Ok("{}".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove(id_ptr: *const StringHeader, force: f64) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let id = perry_runtime::string_from_header(id_ptr);
        let backend = ContainerContext::global().get_backend().await?;
        backend.remove(&id, force != 0.0).await?;
        Ok("{}".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_pull_image(ref_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let reference = perry_runtime::string_from_header(ref_ptr);
        let backend = ContainerContext::global().get_backend().await?;
        backend.pull_image(&reference).await?;
        Ok("{}".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_list_images() -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let backend = ContainerContext::global().get_backend().await?;
        let list = backend.list_images().await?;
        Ok(serde_json::to_string(&list).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_remove_image(ref_ptr: *const StringHeader, force: f64) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let reference = perry_runtime::string_from_header(ref_ptr);
        let backend = ContainerContext::global().get_backend().await?;
        backend.remove_image(&reference, force != 0.0).await?;
        Ok("{}".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_get_backend() -> *const StringHeader {
    let ctx = ContainerContext::global();
    let name = if let Some(b) = ctx.backend.get() {
        b.backend_name()
    } else {
        "none"
    };
    perry_runtime::js_string_from_bytes(name.as_ptr(), name.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_detect_backend() -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        match perry_container_compose::backend::detect_backend().await {
            Ok(b) => {
                let info = BackendInfo {
                    name: b.backend_name().to_string(),
                    available: true,
                    reason: None,
                    version: None,
                    mode: "local".to_string(),
                    isolation_level: b.isolation_level(),
                };
                Ok(serde_json::to_string(&vec![info]).unwrap())
            }
            Err(ComposeError::NoBackendFound { probed }) => {
                let infos: Vec<BackendInfo> = probed.into_iter().map(|p| BackendInfo {
                    name: p.name,
                    available: p.available,
                    reason: Some(p.reason),
                    version: None,
                    mode: "local".to_string(),
                    isolation_level: IsolationLevel::None,
                }).collect();
                Ok(serde_json::to_string(&infos).unwrap())
            }
            Err(e) => Err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_up(spec_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let spec_json = perry_runtime::string_from_header(spec_ptr);
        let spec: ComposeSpec = if spec_json.trim().starts_with('{') {
             serde_json::from_str(&spec_json).map_err(|e| ComposeError::InvalidConfig(e.to_string()))?
        } else {
             let content = std::fs::read_to_string(&spec_json).map_err(|e| ComposeError::NotFound(e.to_string()))?;
             serde_yaml::from_str(&content).map_err(|e| ComposeError::InvalidConfig(e.to_string()))?
        };

        let backend = ContainerContext::global().get_backend().await?;
        let engine = perry_container_compose::compose::ComposeEngine::new(backend);
        let handle = engine.up(&spec).await?;
        let handle_id = register_handle(handle);

        Ok(handle_id.to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_down(handle_id: i64, volumes: i32) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let handle = get_handle::<ComposeHandle>(handle_id).ok_or_else(|| ComposeError::NotFound(handle_id.to_string()))?;
        handle.down(volumes != 0).await?;
        Ok("{}".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_ps(handle_id: i64) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let handle = get_handle::<ComposeHandle>(handle_id).ok_or_else(|| ComposeError::NotFound(handle_id.to_string()))?;
        let status = handle.ps().await?;
        Ok(serde_json::to_string(&status).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_status(handle_id: i64) -> *mut Promise {
    let handle = get_handle::<ComposeHandle>(handle_id);
    if handle.is_none() {
        return perry_runtime::spawn_for_promise(async move {
            Err(ComposeError::NotFound(handle_id.to_string()))
        });
    }
    js_container_compose_ps(handle_id)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_logs(handle_id: i64, service_ptr: *const StringHeader, tail: f64) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let handle = get_handle::<ComposeHandle>(handle_id).ok_or_else(|| ComposeError::NotFound(handle_id.to_string()))?;
        let service = if service_ptr.is_null() { None } else { Some(perry_runtime::string_from_header(service_ptr)) };
        let tail_val = if tail > 0.0 { Some(tail as u32) } else { None };
        let logs = handle.logs(service.as_deref(), tail_val).await?;
        Ok(serde_json::to_string(&logs).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_exec(handle_id: i64, service_ptr: *const StringHeader, cmd_ptr: *const StringHeader, _opts_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let handle = get_handle::<ComposeHandle>(handle_id).ok_or_else(|| ComposeError::NotFound(handle_id.to_string()))?;
        let service = perry_runtime::string_from_header(service_ptr);
        let cmd_json = perry_runtime::string_from_header(cmd_ptr);
        let cmd: Vec<String> = serde_json::from_str(&cmd_json).map_err(|e| ComposeError::InvalidConfig(e.to_string()))?;
        let logs = handle.exec(&service, &cmd).await?;
        Ok(serde_json::to_string(&logs).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_config(handle_id: i64) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let handle = get_handle::<ComposeHandle>(handle_id).ok_or_else(|| ComposeError::NotFound(handle_id.to_string()))?;
        let config = handle.config();
        Ok(serde_json::to_string(&config).unwrap())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_graph(handle_id: i64) -> *const StringHeader {
    let handle = get_handle::<ComposeHandle>(handle_id);
    let json = if let Some(h) = handle {
        serde_json::to_string(&h.graph()).unwrap()
    } else {
        "{}".to_string()
    };
    perry_runtime::js_string_from_bytes(json.as_ptr(), json.len() as u32)
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_start(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let handle = get_handle::<ComposeHandle>(handle_id).ok_or_else(|| ComposeError::NotFound(handle_id.to_string()))?;
        let services_json = if services_ptr.is_null() { None } else { Some(perry_runtime::string_from_header(services_ptr)) };
        let services = services_json.and_then(|s| serde_json::from_str(&s).ok());
        handle.start(services).await?;
        Ok("{}".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_stop(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let handle = get_handle::<ComposeHandle>(handle_id).ok_or_else(|| ComposeError::NotFound(handle_id.to_string()))?;
        let services_json = if services_ptr.is_null() { None } else { Some(perry_runtime::string_from_header(services_ptr)) };
        let services = services_json.and_then(|s| serde_json::from_str(&s).ok());
        handle.stop(services).await?;
        Ok("{}".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_container_compose_restart(handle_id: i64, services_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let handle = get_handle::<ComposeHandle>(handle_id).ok_or_else(|| ComposeError::NotFound(handle_id.to_string()))?;
        let services_json = if services_ptr.is_null() { None } else { Some(perry_runtime::string_from_header(services_ptr)) };
        let services = services_json.and_then(|s| serde_json::from_str(&s).ok());
        handle.restart(services).await?;
        Ok("{}".to_string())
    })
}

#[no_mangle]
pub unsafe extern "C" fn js_workload_run_graph(graph_ptr: *const StringHeader) -> *mut Promise {
    perry_runtime::spawn_for_promise(async move {
        let graph_json = perry_runtime::string_from_header(graph_ptr);
        let graph: WorkloadGraph = serde_json::from_str(&graph_json).map_err(|e| ComposeError::InvalidConfig(e.to_string()))?;

        let backend = ContainerContext::global().get_backend().await?;
        let engine = perry_container_compose::workload::WorkloadGraphEngine::new(backend);
        engine.run(&graph).await?;

        Ok("{}".to_string())
    })
}
