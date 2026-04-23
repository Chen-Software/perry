use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::error::Result;
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, ComposeServiceBuild};

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
    Exec(String, Vec<String>),
    PullImage(String),
    ListImages,
    RemoveImage(String, bool),
    CreateNetwork(String),
    RemoveNetwork(String),
    CreateVolume(String),
    RemoveVolume(String),
}

pub struct MockBackend {
    pub calls: Mutex<Vec<RecordedCall>>,
    pub inspect_results: Mutex<std::collections::VecDeque<Result<ContainerInfo>>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            inspect_results: Mutex::new(std::collections::VecDeque::new()),
        }
    }

    pub fn calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().unwrap().clone()
    }

    pub fn push_inspect_result(&self, res: Result<ContainerInfo>) {
        self.inspect_results.lock().unwrap().push_back(res);
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }

    async fn check_available(&self) -> Result<()> { Ok(()) }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.calls.lock().unwrap().push(RecordedCall::Run(spec.clone()));
        Ok(ContainerHandle { id: format!("id-{}", spec.name.as_deref().unwrap_or("unknown")), name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.calls.lock().unwrap().push(RecordedCall::Create(spec.clone()));
        Ok(ContainerHandle { id: format!("id-{}", spec.name.as_deref().unwrap_or("unknown")), name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Start(id.into()));
        Ok(())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Stop(id.into(), timeout));
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Remove(id.into(), force));
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        self.calls.lock().unwrap().push(RecordedCall::List(all));
        Ok(vec![])
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.calls.lock().unwrap().push(RecordedCall::Inspect(id.into()));
        if let Some(res) = self.inspect_results.lock().unwrap().pop_front() {
            return res;
        }
        Err(crate::error::ComposeError::NotFound(id.into()))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        self.calls.lock().unwrap().push(RecordedCall::Logs(id.into(), tail));
        Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
    }

    async fn exec(&self, id: &str, cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        self.calls.lock().unwrap().push(RecordedCall::Exec(id.into(), cmd.to_vec()));
        Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::PullImage(reference.into()));
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        self.calls.lock().unwrap().push(RecordedCall::ListImages);
        Ok(vec![])
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::RemoveImage(reference.into(), force));
        Ok(())
    }

    async fn create_network(&self, name: &str, _config: &NetworkConfig) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::CreateNetwork(name.into()));
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::RemoveNetwork(name.into()));
        Ok(())
    }

    async fn create_volume(&self, name: &str, _config: &VolumeConfig) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::CreateVolume(name.into()));
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::RemoveVolume(name.into()));
        Ok(())
    }

    async fn build(&self, _spec: &ComposeServiceBuild, _image_name: &str) -> Result<()> {
        Ok(())
    }

    async fn inspect_network(&self, _name: &str) -> Result<()> {
        Ok(())
    }

    async fn inspect_volume(&self, _name: &str) -> Result<()> {
        Ok(())
    }
}
