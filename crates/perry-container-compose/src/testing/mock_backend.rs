use async_trait::async_trait;
use crate::error::{ComposeError, Result};
use crate::backend::{ContainerBackend, ExecutionStrategy, NetworkConfig, VolumeConfig};
use crate::types::{Container, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, ComposeServiceBuild, IsolationLevel};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

pub struct MockBackend {
    pub calls: Arc<Mutex<Vec<RecordedCall>>>,
    pub responses: Arc<Mutex<VecDeque<MockResponse>>>,
}

#[derive(Debug, Clone)]
pub enum RecordedCall {
    BackendName,
    CheckAvailable,
    Run(Container),
    Create(Container),
    Start(String),
    Stop(String, Option<u32>),
    Remove(String, bool),
    List(bool),
    Inspect(String),
    Logs(String, Option<u32>),
    Exec(String, Vec<String>),
    Build(String),
    PullImage(String),
    ListImages,
    RemoveImage(String, bool),
    CreateNetwork(String),
    RemoveNetwork(String),
    CreateVolume(String),
    RemoveVolume(String),
    InspectNetwork(String),
}

pub enum MockResponse {
    ResultOk,
    ResultErr(ComposeError),
    ContainerHandle(ContainerHandle),
    ContainerInfo(ContainerInfo),
    ContainerInfoList(Vec<ContainerInfo>),
    ContainerLogs(ContainerLogs),
    ImageInfoList(Vec<ImageInfo>),
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn record(&self, call: RecordedCall) {
        self.calls.lock().unwrap().push(call);
    }

    fn pop_response(&self) -> MockResponse {
        self.responses.lock().unwrap().pop_front().unwrap_or(MockResponse::ResultOk)
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str {
        self.record(RecordedCall::BackendName);
        "mock"
    }
    async fn check_available(&self) -> Result<()> {
        self.record(RecordedCall::CheckAvailable);
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    async fn run(&self, spec: &Container) -> Result<ContainerHandle> {
        self.record(RecordedCall::Run(spec.clone()));
        match self.pop_response() {
            MockResponse::ContainerHandle(h) => Ok(h),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(ContainerHandle { id: "mock_id".into() }),
        }
    }
    async fn create(&self, spec: &Container) -> Result<ContainerHandle> {
        self.record(RecordedCall::Create(spec.clone()));
        match self.pop_response() {
            MockResponse::ContainerHandle(h) => Ok(h),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(ContainerHandle { id: "mock_id".into() }),
        }
    }
    async fn start(&self, id: &str) -> Result<()> {
        self.record(RecordedCall::Start(id.into()));
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.record(RecordedCall::Stop(id.into(), timeout));
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.record(RecordedCall::Remove(id.into(), force));
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        self.record(RecordedCall::List(all));
        match self.pop_response() {
            MockResponse::ContainerInfoList(l) => Ok(l),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(vec![]),
        }
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.record(RecordedCall::Inspect(id.into()));
        match self.pop_response() {
            MockResponse::ContainerInfo(i) => Ok(i),
            MockResponse::ResultErr(e) => Err(e),
            _ => Err(ComposeError::NotFound(id.to_string())),
        }
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        self.record(RecordedCall::Logs(id.into(), tail));
        match self.pop_response() {
            MockResponse::ContainerLogs(l) => Ok(l),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }),
        }
    }
    async fn exec(&self, id: &str, cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        self.record(RecordedCall::Exec(id.into(), cmd.to_vec()));
        match self.pop_response() {
            MockResponse::ContainerLogs(l) => Ok(l),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }),
        }
    }
    async fn build(&self, _spec: &ComposeServiceBuild, image_name: &str) -> Result<()> {
        self.record(RecordedCall::Build(image_name.into()));
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.record(RecordedCall::PullImage(reference.into()));
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        self.record(RecordedCall::ListImages);
        match self.pop_response() {
            MockResponse::ImageInfoList(l) => Ok(l),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(vec![]),
        }
    }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.record(RecordedCall::RemoveImage(reference.into(), force));
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    async fn create_network(&self, name: &str, _config: &NetworkConfig) -> Result<()> {
        self.record(RecordedCall::CreateNetwork(name.into()));
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    async fn remove_network(&self, name: &str) -> Result<()> {
        self.record(RecordedCall::RemoveNetwork(name.into()));
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    async fn create_volume(&self, name: &str, _config: &VolumeConfig) -> Result<()> {
        self.record(RecordedCall::CreateVolume(name.into()));
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.record(RecordedCall::RemoveVolume(name.into()));
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    async fn inspect_network(&self, name: &str) -> Result<()> {
        self.record(RecordedCall::InspectNetwork(name.into()));
        match self.pop_response() {
            MockResponse::ResultOk => Ok(()),
            MockResponse::ResultErr(e) => Err(e),
            _ => Ok(()),
        }
    }
    fn strategy(&self) -> ExecutionStrategy {
        ExecutionStrategy::VmSpawn { config: "mock".into() }
    }
    fn isolation_level(&self) -> IsolationLevel {
        IsolationLevel::MicroVm
    }
}
