use crate::backend::ContainerBackend;
use crate::error::Result;
use crate::types::{
    ComposeNetwork, ComposeServiceBuild, ComposeVolume, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub enum RecordedCall {
    Run(ContainerSpec),
    Create(ContainerSpec),
    Start(String),
    Stop(String, Option<u32>),
    Remove(String, bool),
    List(bool),
    Inspect(String),
    Logs(String, Option<u32>),
    Exec(String, Vec<String>, Option<HashMap<String, String>>, Option<String>),
    Build(ComposeServiceBuild, String),
    PullImage(String),
    ListImages,
    InspectImage(String),
    RemoveImage(String, bool),
    CreateNetwork(String, ComposeNetwork),
    RemoveNetwork(String),
    InspectNetwork(String),
    CreateVolume(String, ComposeVolume),
    RemoveVolume(String),
}

pub enum MockResponse {
    Run(ContainerHandle),
    Create(ContainerHandle),
    List(Vec<ContainerInfo>),
    Inspect(ContainerInfo),
    Logs(ContainerLogs),
    Exec(ContainerLogs),
    ListImages(Vec<ImageInfo>),
    InspectImage(ImageInfo),
    Ok,
    Err(String),
}

pub struct MockBackend {
    pub calls: Mutex<Vec<RecordedCall>>,
    pub responses: Mutex<VecDeque<MockResponse>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            responses: Mutex::new(VecDeque::new()),
        }
    }

    pub fn push_response(&self, response: MockResponse) {
        self.responses.lock().unwrap().push_back(response);
    }

    fn record(&self, call: RecordedCall) {
        self.calls.lock().unwrap().push(call);
    }

    fn next_response(&self) -> MockResponse {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(MockResponse::Ok)
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str {
        "mock"
    }

    async fn check_available(&self) -> Result<()> {
        Ok(())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record(RecordedCall::Run(spec.clone()));
        match self.next_response() {
            MockResponse::Run(h) => Ok(h),
            MockResponse::Err(e) => Err(crate::error::ComposeError::BackendError {
                code: -1,
                message: e,
            }),
            _ => Ok(ContainerHandle {
                id: "mock-id".to_string(),
                name: spec.name.clone(),
            }),
        }
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record(RecordedCall::Create(spec.clone()));
        match self.next_response() {
            MockResponse::Create(h) => Ok(h),
            MockResponse::Err(e) => Err(crate::error::ComposeError::BackendError {
                code: -1,
                message: e,
            }),
            _ => Ok(ContainerHandle {
                id: "mock-id".to_string(),
                name: spec.name.clone(),
            }),
        }
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.record(RecordedCall::Start(id.to_string()));
        match self.next_response() {
            MockResponse::Err(e) => Err(crate::error::ComposeError::BackendError {
                code: -1,
                message: e,
            }),
            _ => Ok(()),
        }
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.record(RecordedCall::Stop(id.to_string(), timeout));
        match self.next_response() {
            MockResponse::Err(e) => Err(crate::error::ComposeError::BackendError {
                code: -1,
                message: e,
            }),
            _ => Ok(()),
        }
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.record(RecordedCall::Remove(id.to_string(), force));
        match self.next_response() {
            MockResponse::Err(e) => Err(crate::error::ComposeError::BackendError {
                code: -1,
                message: e,
            }),
            _ => Ok(()),
        }
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        self.record(RecordedCall::List(all));
        match self.next_response() {
            MockResponse::List(l) => Ok(l),
            MockResponse::Err(e) => Err(crate::error::ComposeError::BackendError {
                code: -1,
                message: e,
            }),
            _ => Ok(Vec::new()),
        }
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.record(RecordedCall::Inspect(id.to_string()));
        match self.next_response() {
            MockResponse::Inspect(i) => Ok(i),
            MockResponse::Err(e) => Err(crate::error::ComposeError::BackendError {
                code: -1,
                message: e,
            }),
            _ => Err(crate::error::ComposeError::NotFound(id.to_string())),
        }
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        self.record(RecordedCall::Logs(id.to_string(), tail));
        match self.next_response() {
            MockResponse::Logs(l) => Ok(l),
            MockResponse::Err(e) => Err(crate::error::ComposeError::BackendError {
                code: -1,
                message: e,
            }),
            _ => Ok(ContainerLogs {
                stdout: "".into(),
                stderr: "".into(),
            }),
        }
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        self.record(RecordedCall::Exec(
            id.to_string(),
            cmd.to_vec(),
            env.cloned(),
            workdir.map(|s| s.to_string()),
        ));
        match self.next_response() {
            MockResponse::Exec(l) => Ok(l),
            MockResponse::Err(e) => Err(crate::error::ComposeError::BackendError {
                code: -1,
                message: e,
            }),
            _ => Ok(ContainerLogs {
                stdout: "".into(),
                stderr: "".into(),
            }),
        }
    }

    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<()> {
        self.record(RecordedCall::Build(spec.clone(), image_name.to_string()));
        Ok(())
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.record(RecordedCall::PullImage(reference.to_string()));
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        self.record(RecordedCall::ListImages);
        match self.next_response() {
            MockResponse::ListImages(l) => Ok(l),
            MockResponse::Err(e) => Err(crate::error::ComposeError::BackendError {
                code: -1,
                message: e,
            }),
            _ => Ok(Vec::new()),
        }
    }

    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo> {
        self.record(RecordedCall::InspectImage(reference.to_string()));
        match self.next_response() {
            MockResponse::InspectImage(i) => Ok(i),
            _ => Err(crate::error::ComposeError::NotFound(reference.to_string())),
        }
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.record(RecordedCall::RemoveImage(reference.to_string(), force));
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> {
        self.record(RecordedCall::CreateNetwork(name.to_string(), config.clone()));
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.record(RecordedCall::RemoveNetwork(name.to_string()));
        Ok(())
    }

    async fn inspect_network(&self, name: &str) -> Result<()> {
        self.record(RecordedCall::InspectNetwork(name.to_string()));
        Ok(())
    }

    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        self.record(RecordedCall::CreateVolume(name.to_string(), config.clone()));
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.record(RecordedCall::RemoveVolume(name.to_string()));
        Ok(())
    }
}
