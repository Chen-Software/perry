use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use crate::backend::ContainerBackend;
use crate::error::ComposeError;
use crate::types::*;

pub struct MockBackend {
    pub calls: Arc<Mutex<Vec<String>>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }
    fn isolation_level(&self) -> IsolationLevel { IsolationLevel::Container }

    async fn check_available(&self) -> Result<(), ComposeError> { Ok(()) }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ComposeError> {
        self.record(format!("run: {}", spec.image));
        Ok(ContainerHandle { id: "mock_id".to_string() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ComposeError> {
        self.record(format!("create: {}", spec.image));
        Ok(ContainerHandle { id: "mock_id".to_string() })
    }
    async fn start(&self, id: &str) -> Result<(), ComposeError> {
        self.record(format!("start: {}", id));
        Ok(())
    }
    async fn stop(&self, id: &str, _timeout: Option<u32>) -> Result<(), ComposeError> {
        self.record(format!("stop: {}", id));
        Ok(())
    }
    async fn remove(&self, id: &str, _force: bool) -> Result<(), ComposeError> {
        self.record(format!("remove: {}", id));
        Ok(())
    }
    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>, ComposeError> { Ok(vec![]) }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ComposeError> {
        Ok(ContainerInfo { id: id.to_string(), status: "running".to_string(), ..Default::default() })
    }
    async fn logs(&self, _id: &str, _tail: Option<u32>) -> Result<ContainerLogs, ComposeError> {
        Ok(ContainerLogs::default())
    }
    async fn exec(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs, ComposeError> {
        Ok(ContainerLogs::default())
    }
    async fn build(&self, _spec: &ComposeServiceBuild, _image_name: &str) -> Result<(), ComposeError> { Ok(()) }
    async fn pull_image(&self, _reference: &str) -> Result<(), ComposeError> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>, ComposeError> { Ok(vec![]) }
    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<(), ComposeError> { Ok(()) }
    async fn create_network(&self, name: &str) -> Result<(), ComposeError> {
        self.record(format!("create_network: {}", name));
        Ok(())
    }
    async fn remove_network(&self, _name: &str) -> Result<(), ComposeError> { Ok(()) }
    async fn create_volume(&self, name: &str) -> Result<(), ComposeError> {
        self.record(format!("create_volume: {}", name));
        Ok(())
    }
    async fn remove_volume(&self, _name: &str) -> Result<(), ComposeError> { Ok(()) }
}
