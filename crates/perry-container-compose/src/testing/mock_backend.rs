use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig, SecurityProfile};
use crate::error::{ComposeError, Result};
use crate::types::{
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo, ComposeServiceBuild,
};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::any::Any;

pub struct MockBackend {
    pub state: Arc<Mutex<MockBackendState>>,
    pub responses: Arc<Mutex<VecDeque<MockResponse>>>,
}

#[derive(Default)]
pub struct MockBackendState {
    pub containers: Vec<String>,
    pub networks: Vec<String>,
    pub volumes: Vec<String>,
    pub calls: Vec<RecordedCall>,
}

#[derive(Debug, Clone)]
pub struct RecordedCall {
    pub method: String,
    pub args: Vec<String>,
}

pub enum MockResponse {
    Ok(Box<dyn Any + Send>),
    Err(ComposeError),
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockBackendState::default())),
            responses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn calls(&self) -> Vec<RecordedCall> {
        self.state.lock().unwrap().calls.clone()
    }

    pub fn push_response(&self, response: MockResponse) {
        self.responses.lock().unwrap().push_back(response);
    }

    fn record_call(&self, method: &str, args: Vec<String>) {
        self.state.lock().unwrap().calls.push(RecordedCall {
            method: method.to_string(),
            args,
        });
    }

    fn pop_response<T: 'static + Send>(&self) -> Option<Result<T>> {
        self.responses.lock().unwrap().pop_front().map(|resp| match resp {
            MockResponse::Ok(any) => Ok(*any.downcast::<T>().expect("Mock response type mismatch")),
            MockResponse::Err(e) => Err(e),
        })
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str {
        "mock"
    }

    async fn check_available(&self) -> Result<()> {
        self.record_call("check_available", vec![]);
        self.pop_response::<()>().unwrap_or(Ok(()))
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record_call("run", vec![spec.name.clone().unwrap_or_default()]);
        self.pop_response::<ContainerHandle>().unwrap_or_else(|| {
            let id = spec.name.clone().unwrap_or_else(|| "mock-id".to_string());
            Ok(ContainerHandle {
                id: id.clone(),
                name: spec.name.clone(),
            })
        })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record_call("create", vec![spec.name.clone().unwrap_or_default()]);
        self.pop_response::<ContainerHandle>().unwrap_or_else(|| {
            let id = spec.name.clone().unwrap_or_else(|| "mock-id".to_string());
            Ok(ContainerHandle {
                id: id.clone(),
                name: spec.name.clone(),
            })
        })
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.record_call("start", vec![id.to_string()]);
        self.pop_response::<()>().unwrap_or(Ok(()))
    }

    async fn stop(&self, id: &str, _timeout: Option<u32>) -> Result<()> {
        self.record_call("stop", vec![id.to_string()]);
        self.pop_response::<()>().unwrap_or(Ok(()))
    }

    async fn remove(&self, id: &str, _force: bool) -> Result<()> {
        self.record_call("remove", vec![id.to_string()]);
        self.pop_response::<()>().unwrap_or(Ok(()))
    }

    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> {
        self.record_call("list", vec![]);
        self.pop_response::<Vec<ContainerInfo>>().unwrap_or(Ok(vec![]))
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.record_call("inspect", vec![id.to_string()]);
        self.pop_response::<ContainerInfo>().unwrap_or_else(|| {
            Ok(ContainerInfo {
                id: id.to_string(),
                name: id.to_string(),
                image: "mock-image".to_string(),
                status: "running".to_string(),
                ports: vec![],
                labels: HashMap::new(),
                created: "".to_string(),
            })
        })
    }

    async fn logs(&self, id: &str, _tail: Option<u32>) -> Result<ContainerLogs> {
        self.record_call("logs", vec![id.to_string()]);
        self.pop_response::<ContainerLogs>().unwrap_or_else(|| {
            Ok(ContainerLogs {
                stdout: "mock stdout".to_string(),
                stderr: "".to_string(),
            })
        })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        _env: Option<&HashMap<String, String>>,
        _workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        self.record_call("exec", vec![id.to_string(), cmd.join(" ")]);
        self.pop_response::<ContainerLogs>().unwrap_or_else(|| {
            Ok(ContainerLogs {
                stdout: "mock exec stdout".to_string(),
                stderr: "".to_string(),
            })
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.record_call("pull_image", vec![reference.to_string()]);
        self.pop_response::<()>().unwrap_or(Ok(()))
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        self.record_call("list_images", vec![]);
        self.pop_response::<Vec<ImageInfo>>().unwrap_or(Ok(vec![]))
    }

    async fn remove_image(&self, reference: &str, _force: bool) -> Result<()> {
        self.record_call("remove_image", vec![reference.to_string()]);
        self.pop_response::<()>().unwrap_or(Ok(()))
    }

    async fn create_network(&self, name: &str, _config: &NetworkConfig) -> Result<()> {
        self.record_call("create_network", vec![name.to_string()]);
        self.pop_response::<()>().unwrap_or(Ok(()))
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.record_call("remove_network", vec![name.to_string()]);
        self.pop_response::<()>().unwrap_or(Ok(()))
    }

    async fn create_volume(&self, name: &str, _config: &VolumeConfig) -> Result<()> {
        self.record_call("create_volume", vec![name.to_string()]);
        self.pop_response::<()>().unwrap_or(Ok(()))
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.record_call("remove_volume", vec![name.to_string()]);
        self.pop_response::<()>().unwrap_or(Ok(()))
    }

    async fn inspect_network(&self, name: &str) -> Result<serde_json::Value> {
        self.record_call("inspect_network", vec![name.to_string()]);
        self.pop_response::<serde_json::Value>().unwrap_or(Ok(serde_json::json!({})))
    }

    async fn inspect_volume(&self, name: &str) -> Result<serde_json::Value> {
        self.record_call("inspect_volume", vec![name.to_string()]);
        self.pop_response::<serde_json::Value>().unwrap_or(Ok(serde_json::json!({})))
    }

    async fn build(&self, _spec: &ComposeServiceBuild, image_name: &str) -> Result<()> {
        self.record_call("build", vec![image_name.to_string()]);
        self.pop_response::<()>().unwrap_or(Ok(()))
    }

    async fn inspect_image(&self, reference: &str) -> Result<serde_json::Value> {
        self.record_call("inspect_image", vec![reference.to_string()]);
        self.pop_response::<serde_json::Value>().unwrap_or(Ok(serde_json::json!({})))
    }

    async fn manifest_inspect(&self, reference: &str) -> Result<serde_json::Value> {
        self.record_call("manifest_inspect", vec![reference.to_string()]);
        self.pop_response::<serde_json::Value>().unwrap_or(Ok(serde_json::json!({})))
    }

    async fn run_with_security(
        &self,
        spec: &ContainerSpec,
        _profile: &SecurityProfile,
    ) -> Result<ContainerHandle> {
        self.record_call("run_with_security", vec![spec.name.clone().unwrap_or_default()]);
        self.run(spec).await
    }

    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs> {
        self.record_call("wait_and_logs", vec![id.to_string()]);
        self.logs(id, None).await
    }
}
