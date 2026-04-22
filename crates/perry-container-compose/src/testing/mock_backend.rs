use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig, SecurityProfile};
use crate::error::{ComposeError, Result};
use crate::types::{ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub enum RecordedCall {
    Run(ContainerSpec),
    Start(String),
    Stop(String, Option<u32>),
    Remove(String, bool),
    Inspect(String),
    Build(String, String),
    CreateNetwork(String),
    CreateVolume(String),
}

pub enum MockResponse {
    Run(Result<ContainerHandle>),
    Inspect(Result<ContainerInfo>),
    Status(Result<()>),
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
    pub fn push_response(&self, res: MockResponse) {
        self.responses.lock().unwrap().push_back(res);
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }
    async fn check_available(&self) -> Result<()> { Ok(()) }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.calls.lock().unwrap().push(RecordedCall::Run(spec.clone()));
        match self.responses.lock().unwrap().pop_front() {
            Some(MockResponse::Run(r)) => r,
            _ => Ok(ContainerHandle { id: "mock-id".into(), name: spec.name.clone() })
        }
    }
    async fn create(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> { Ok(ContainerHandle { id: "mock-id".into(), name: None }) }
    async fn start(&self, id: &str) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Start(id.to_string()));
        Ok(())
    }
    async fn stop(&self, id: &str, t: Option<u32>) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Stop(id.to_string(), t));
        Ok(())
    }
    async fn remove(&self, id: &str, f: bool) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Remove(id.to_string(), f));
        Ok(())
    }
    async fn list(&self, _a: bool) -> Result<Vec<ContainerInfo>> { Ok(vec![]) }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.calls.lock().unwrap().push(RecordedCall::Inspect(id.to_string()));
        match self.responses.lock().unwrap().pop_front() {
            Some(MockResponse::Inspect(r)) => r,
            _ => Err(ComposeError::NotFound(id.to_string()))
        }
    }
    async fn logs(&self, _id: &str, _t: Option<u32>, _f: bool) -> Result<ContainerLogs> { Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }) }
    async fn exec(&self, _id: &str, _c: &[String], _e: Option<&HashMap<String, String>>, _w: Option<&str>, _u: Option<&str>) -> Result<ContainerLogs> { Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }) }
    async fn pull_image(&self, _r: &str) -> Result<()> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(vec![]) }
    async fn remove_image(&self, _r: &str, _f: bool) -> Result<()> { Ok(()) }
    async fn create_network(&self, name: &str, _c: &NetworkConfig) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::CreateNetwork(name.to_string()));
        Ok(())
    }
    async fn remove_network(&self, _n: &str) -> Result<()> { Ok(()) }
    async fn create_volume(&self, name: &str, _c: &VolumeConfig) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::CreateVolume(name.to_string()));
        Ok(())
    }
    async fn remove_volume(&self, _n: &str) -> Result<()> { Ok(()) }
    async fn inspect_network(&self, _n: &str) -> Result<serde_json::Value> { Ok(serde_json::Value::Null) }
    async fn inspect_volume(&self, _n: &str) -> Result<serde_json::Value> { Ok(serde_json::Value::Null) }
    async fn build_image(&self, context: &str, tag: &str, _d: Option<&str>, _a: Option<&HashMap<String, String>>) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Build(context.to_string(), tag.to_string()));
        Ok(())
    }
    async fn wait(&self, _id: &str) -> Result<()> { Ok(()) }
    async fn inspect_image(&self, _r: &str) -> Result<serde_json::Value> { Ok(serde_json::Value::Null) }
    async fn manifest_inspect(&self, _r: &str) -> Result<serde_json::Value> { Ok(serde_json::Value::Null) }
    async fn run_with_security(&self, _s: &ContainerSpec, _p: &SecurityProfile) -> Result<ContainerHandle> { Ok(ContainerHandle { id: "sec-id".into(), name: None }) }
    async fn wait_and_logs(&self, _id: &str) -> Result<ContainerLogs> { Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }) }
}
