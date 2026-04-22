use crate::backend::ContainerBackend;
use crate::error::Result;
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum MockResponse {
    Ok,
    OkHandle(ContainerHandle),
    OkInfo(ContainerInfo),
    OkList(Vec<ContainerInfo>),
    OkLogs(ContainerLogs),
    OkImages(Vec<ImageInfo>),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct RecordedCall {
    pub method: String,
    pub args: Vec<String>,
}

pub struct MockBackendInner {
    pub calls: Vec<RecordedCall>,
    pub responses: VecDeque<MockResponse>,
}

#[derive(Clone)]
pub struct MockBackend {
    pub inner: Arc<Mutex<MockBackendInner>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MockBackendInner {
                calls: Vec::new(),
                responses: VecDeque::new(),
            })),
        }
    }

    pub fn push_response(&self, response: MockResponse) {
        self.inner.lock().unwrap().responses.push_back(response);
    }

    pub fn calls(&self) -> Vec<RecordedCall> {
        self.inner.lock().unwrap().calls.clone()
    }

    fn record(&self, method: &str, args: Vec<String>) {
        self.inner.lock().unwrap().calls.push(RecordedCall {
            method: method.to_string(),
            args,
        });
    }

    fn pop_response(&self) -> MockResponse {
        self.inner.lock().unwrap().responses.pop_front().unwrap_or(MockResponse::Ok)
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }

    async fn check_available(&self) -> Result<()> { Ok(()) }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record("run", vec![spec.image.clone()]);
        match self.pop_response() {
            MockResponse::OkHandle(h) => Ok(h),
            MockResponse::Error(e) => Err(crate::error::ComposeError::BackendError { code: -1, message: e }),
            _ => Ok(ContainerHandle { id: "mock-id".into(), name: spec.name.clone() }),
        }
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.record("create", vec![spec.image.clone()]);
        match self.pop_response() {
            MockResponse::OkHandle(h) => Ok(h),
            MockResponse::Error(e) => Err(crate::error::ComposeError::BackendError { code: -1, message: e }),
            _ => Ok(ContainerHandle { id: "mock-id".into(), name: spec.name.clone() }),
        }
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.record("start", vec![id.into()]);
        match self.pop_response() {
            MockResponse::Error(e) => Err(crate::error::ComposeError::BackendError { code: -1, message: e }),
            _ => Ok(()),
        }
    }

    async fn stop(&self, id: &str, _timeout: Option<u32>) -> Result<()> {
        self.record("stop", vec![id.into()]);
        match self.pop_response() {
            MockResponse::Error(e) => Err(crate::error::ComposeError::BackendError { code: -1, message: e }),
            _ => Ok(()),
        }
    }

    async fn remove(&self, id: &str, _force: bool) -> Result<()> {
        self.record("remove", vec![id.into()]);
        match self.pop_response() {
            MockResponse::Error(e) => Err(crate::error::ComposeError::BackendError { code: -1, message: e }),
            _ => Ok(()),
        }
    }

    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> {
        self.record("list", vec![]);
        match self.pop_response() {
            MockResponse::OkList(l) => Ok(l),
            MockResponse::Error(e) => Err(crate::error::ComposeError::BackendError { code: -1, message: e }),
            _ => Ok(vec![]),
        }
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.record("inspect", vec![id.into()]);
        match self.pop_response() {
            MockResponse::OkInfo(i) => Ok(i),
            MockResponse::Error(e) => Err(crate::error::ComposeError::NotFound(e)),
            _ => Ok(ContainerInfo {
                id: id.into(),
                name: id.into(),
                image: "mock-image".into(),
                status: "running".into(),
                ports: vec![],
                created: "".into(),
            }),
        }
    }

    async fn logs(&self, id: &str, _tail: Option<u32>) -> Result<ContainerLogs> {
        self.record("logs", vec![id.into()]);
        match self.pop_response() {
            MockResponse::OkLogs(l) => Ok(l),
            MockResponse::Error(e) => Err(crate::error::ComposeError::BackendError { code: -1, message: e }),
            _ => Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }),
        }
    }

    async fn exec(&self, id: &str, cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        self.record("exec", vec![id.into(), cmd.join(" ")]);
        match self.pop_response() {
            MockResponse::OkLogs(l) => Ok(l),
            MockResponse::Error(e) => Err(crate::error::ComposeError::BackendError { code: -1, message: e }),
            _ => Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }),
        }
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.record("pull", vec![reference.into()]);
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        match self.pop_response() {
            MockResponse::OkImages(l) => Ok(l),
            _ => Ok(vec![]),
        }
    }

    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo> {
        self.record("inspect_image", vec![reference.into()]);
        Ok(ImageInfo {
            id: reference.into(),
            repository: reference.into(),
            tag: "latest".into(),
            size: 0,
            created: "".into(),
        })
    }

    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<()> { Ok(()) }

    async fn create_network(&self, name: &str, _config: &ComposeNetwork) -> Result<()> {
        self.record("create_network", vec![name.into()]);
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.record("remove_network", vec![name.into()]);
        Ok(())
    }

    async fn create_volume(&self, name: &str, _config: &ComposeVolume) -> Result<()> {
        self.record("create_volume", vec![name.into()]);
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.record("remove_volume", vec![name.into()]);
        Ok(())
    }

    async fn build(&self, _spec: &crate::types::ComposeServiceBuild, image_name: &str) -> Result<()> {
        self.record("build", vec![image_name.into()]);
        Ok(())
    }

    async fn inspect_network(&self, name: &str) -> Result<()> {
        self.record("inspect_network", vec![name.into()]);
        Ok(())
    }
}
