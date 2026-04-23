use async_trait::async_trait;
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::types::*;
use crate::error::{ComposeError, Result};
use std::sync::Mutex;
use std::collections::{VecDeque, HashMap};
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum RecordedCall {
    BackendName,
    CheckAvailable,
    Run(ContainerSpec),
    Create(ContainerSpec),
    Start(String),
    Stop(String, Option<u32>),
    Remove(String, bool),
    List(bool),
    Inspect(String),
    Logs(String, Option<u32>),
    Exec(String, Vec<String>, Option<HashMap<String, String>>, Option<String>),
    Build(ComposeServiceBuild, String, String),
    PullImage(String),
    ListImages,
    RemoveImage(String, bool),
    CreateNetwork(String, NetworkConfig),
    RemoveNetwork(String),
    CreateVolume(String, VolumeConfig),
    RemoveVolume(String),
}

pub enum MockResponse {
    Ok,
    ContainerHandle(ContainerHandle),
    ContainerInfo(ContainerInfo),
    ContainerLogs(ContainerLogs),
    ImageList(Vec<ImageInfo>),
    ContainerList(Vec<ContainerInfo>),
    Json(Value),
    Error(ComposeError),
}

pub struct MockBackend {
    pub name: String,
    pub calls: Mutex<Vec<RecordedCall>>,
    pub responses: Mutex<VecDeque<MockResponse>>,
}

impl MockBackend {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            calls: Mutex::new(Vec::new()),
            responses: Mutex::new(VecDeque::new()),
        }
    }

    pub fn push_response(&self, response: MockResponse) {
        self.responses.lock().unwrap().push_back(response);
    }

    pub fn take_calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().unwrap().drain(..).collect()
    }

    fn record(&self, call: RecordedCall) {
        self.calls.lock().unwrap().push(call);
    }

    fn pop_response(&self) -> Result<MockResponse> {
        Ok(self.responses.lock().unwrap().pop_front().unwrap_or(MockResponse::Ok))
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str {
        self.record(RecordedCall::BackendName);
        &self.name
    }

    fn into_arc(self: Box<Self>) -> std::sync::Arc<dyn ContainerBackend + Send + Sync> {
        std::sync::Arc::new(*self)
    }

    async fn check_available(&self) -> Result<()> {
        self.record(RecordedCall::CheckAvailable);
        match self.pop_response()? {
            MockResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record(RecordedCall::Run(spec.clone()));
        match self.pop_response()? {
            MockResponse::ContainerHandle(h) => Ok(h),
            MockResponse::Error(e) => Err(e),
            _ => Ok(ContainerHandle { id: "mock-id".into(), name: None }),
        }
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record(RecordedCall::Create(spec.clone()));
        match self.pop_response()? {
            MockResponse::ContainerHandle(h) => Ok(h),
            MockResponse::Error(e) => Err(e),
            _ => Ok(ContainerHandle { id: "mock-id".into(), name: None }),
        }
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.record(RecordedCall::Start(id.to_string()));
        match self.pop_response()? {
            MockResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.record(RecordedCall::Stop(id.to_string(), timeout));
        match self.pop_response()? {
            MockResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.record(RecordedCall::Remove(id.to_string(), force));
        match self.pop_response()? {
            MockResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        self.record(RecordedCall::List(all));
        match self.pop_response()? {
            MockResponse::ContainerList(l) => Ok(l),
            MockResponse::Error(e) => Err(e),
            _ => Ok(vec![]),
        }
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.record(RecordedCall::Inspect(id.to_string()));
        match self.pop_response()? {
            MockResponse::ContainerInfo(i) => Ok(i),
            MockResponse::Error(e) => Err(e),
            _ => Ok(ContainerInfo {
                id: id.to_string(),
                name: "mock-name".into(),
                image: "mock-image".into(),
                status: "running".into(),
                ports: vec![],
                created: "2023-01-01T00:00:00Z".into(),
            }),
        }
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        self.record(RecordedCall::Logs(id.to_string(), tail));
        match self.pop_response()? {
            MockResponse::ContainerLogs(l) => Ok(l),
            MockResponse::Error(e) => Err(e),
            _ => Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }),
        }
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        self.record(RecordedCall::Exec(id.to_string(), cmd.to_vec(), env.cloned(), workdir.map(|s| s.to_string())));
        match self.pop_response()? {
            MockResponse::ContainerLogs(l) => Ok(l),
            MockResponse::Error(e) => Err(e),
            _ => Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }),
        }
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.record(RecordedCall::PullImage(reference.to_string()));
        match self.pop_response()? {
            MockResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        self.record(RecordedCall::ListImages);
        match self.pop_response()? {
            MockResponse::ImageList(l) => Ok(l),
            MockResponse::Error(e) => Err(e),
            _ => Ok(vec![]),
        }
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.record(RecordedCall::RemoveImage(reference.to_string(), force));
        match self.pop_response()? {
            MockResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        self.record(RecordedCall::CreateNetwork(name.to_string(), config.clone()));
        match self.pop_response()? {
            MockResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.record(RecordedCall::RemoveNetwork(name.to_string()));
        match self.pop_response()? {
            MockResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        self.record(RecordedCall::CreateVolume(name.to_string(), config.clone()));
        match self.pop_response()? {
            MockResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.record(RecordedCall::RemoveVolume(name.to_string()));
        match self.pop_response()? {
            MockResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }

    async fn build_image(&self, context: &str, build: &ComposeServiceBuild, tag: &str) -> Result<()> {
        self.record(RecordedCall::Build(build.clone(), context.to_string(), tag.to_string()));
        match self.pop_response()? {
            MockResponse::Error(e) => Err(e),
            _ => Ok(()),
        }
    }
}
