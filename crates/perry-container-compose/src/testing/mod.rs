use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use crate::backend::ContainerBackend;
use crate::error::ComposeError;
use crate::types::*;

#[derive(Debug, Clone)]
pub enum RecordedCall {
    Run(ContainerSpec),
    Inspect(String),
    Start(String),
    Stop(String, Option<u32>),
    Remove(String, bool),
}

pub struct MockBackend {
    pub calls: Arc<Mutex<Vec<RecordedCall>>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }
    async fn check_available(&self) -> Result<(), ComposeError> { Ok(()) }
    async fn run(&self, spec: &ContainerSpec) -> Result<String, ComposeError> {
        self.calls.lock().unwrap().push(RecordedCall::Run(spec.clone()));
        Ok("mock-id".into())
    }
    async fn create(&self, _spec: &ContainerSpec) -> Result<String, ComposeError> { Ok("mock-id".into()) }
    async fn start(&self, id: &str) -> Result<(), ComposeError> {
        self.calls.lock().unwrap().push(RecordedCall::Start(id.to_string()));
        Ok(())
    }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<(), ComposeError> {
        self.calls.lock().unwrap().push(RecordedCall::Stop(id.to_string(), timeout));
        Ok(())
    }
    async fn remove(&self, id: &str, force: bool) -> Result<(), ComposeError> {
        self.calls.lock().unwrap().push(RecordedCall::Remove(id.to_string(), force));
        Ok(())
    }
    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>, ComposeError> { Ok(vec![]) }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ComposeError> {
        self.calls.lock().unwrap().push(RecordedCall::Inspect(id.to_string()));
        Err(ComposeError::NotFound(id.to_string()))
    }
    async fn logs(&self, _id: &str, _tail: Option<u32>) -> Result<ContainerLogs, ComposeError> { Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }) }
    async fn exec(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs, ComposeError> { Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }) }
    async fn build(&self, _spec: &ComposeServiceBuild, _image_name: &str) -> Result<(), ComposeError> { Ok(()) }
    async fn pull_image(&self, _reference: &str) -> Result<(), ComposeError> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>, ComposeError> { Ok(vec![]) }
    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<(), ComposeError> { Ok(()) }
    async fn create_network(&self, _name: &str, _config: Option<&serde_json::Value>) -> Result<(), ComposeError> { Ok(()) }
    async fn remove_network(&self, _name: &str) -> Result<(), ComposeError> { Ok(()) }
    async fn create_volume(&self, _name: &str, _config: Option<&serde_json::Value>) -> Result<(), ComposeError> { Ok(()) }
    async fn remove_volume(&self, _name: &str) -> Result<(), ComposeError> { Ok(()) }
}
