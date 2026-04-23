use crate::error::Result;
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig, SecurityProfile};
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, ComposeServiceBuild};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

pub enum MockResponse {
    Run(ContainerHandle),
    Create(ContainerHandle),
    Inspect(ContainerInfo),
    Logs(ContainerLogs),
    List(Vec<ContainerInfo>),
    ListImages(Vec<ImageInfo>),
    Ok,
    Err(String),
}

pub struct MockBackend {
    pub responses: Mutex<VecDeque<MockResponse>>,
    pub calls: Mutex<Vec<String>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn push_response(&self, resp: MockResponse) {
        self.responses.lock().unwrap().push_back(resp);
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }
    async fn check_available(&self) -> Result<()> { Ok(()) }

    async fn run(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.calls.lock().unwrap().push("run".into());
        match self.responses.lock().unwrap().pop_front().unwrap() {
            MockResponse::Run(h) => Ok(h),
            _ => panic!("Wrong mock response"),
        }
    }

    async fn create(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.calls.lock().unwrap().push("create".into());
        match self.responses.lock().unwrap().pop_front().unwrap() {
            MockResponse::Create(h) => Ok(h),
            _ => panic!("Wrong mock response"),
        }
    }

    async fn start(&self, _id: &str) -> Result<()> {
        self.calls.lock().unwrap().push("start".into());
        Ok(())
    }

    async fn stop(&self, _id: &str, _timeout: Option<u32>) -> Result<()> {
        self.calls.lock().unwrap().push("stop".into());
        Ok(())
    }

    async fn remove(&self, _id: &str, _force: bool) -> Result<()> {
        self.calls.lock().unwrap().push("remove".into());
        Ok(())
    }

    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> {
        self.calls.lock().unwrap().push("list".into());
        match self.responses.lock().unwrap().pop_front().unwrap() {
            MockResponse::List(l) => Ok(l),
            _ => panic!("Wrong mock response"),
        }
    }

    async fn inspect(&self, _id: &str) -> Result<ContainerInfo> {
        self.calls.lock().unwrap().push("inspect".into());
        match self.responses.lock().unwrap().pop_front().unwrap() {
            MockResponse::Inspect(i) => Ok(i),
            _ => panic!("Wrong mock response"),
        }
    }

    async fn logs(&self, _id: &str, _tail: Option<u32>) -> Result<ContainerLogs> {
        self.calls.lock().unwrap().push("logs".into());
        match self.responses.lock().unwrap().pop_front().unwrap() {
            MockResponse::Logs(l) => Ok(l),
            _ => panic!("Wrong mock response"),
        }
    }

    async fn exec(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        self.calls.lock().unwrap().push("exec".into());
        Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
    }

    async fn pull_image(&self, _reference: &str) -> Result<()> {
        self.calls.lock().unwrap().push("pull_image".into());
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        self.calls.lock().unwrap().push("list_images".into());
        match self.responses.lock().unwrap().pop_front().unwrap() {
            MockResponse::ListImages(l) => Ok(l),
            _ => panic!("Wrong mock response"),
        }
    }

    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<()> {
        self.calls.lock().unwrap().push("remove_image".into());
        Ok(())
    }

    async fn create_network(&self, _name: &str, _config: &NetworkConfig) -> Result<()> {
        self.calls.lock().unwrap().push("create_network".into());
        Ok(())
    }

    async fn remove_network(&self, _name: &str) -> Result<()> {
        self.calls.lock().unwrap().push("remove_network".into());
        Ok(())
    }

    async fn create_volume(&self, _name: &str, _config: &VolumeConfig) -> Result<()> {
        self.calls.lock().unwrap().push("create_volume".into());
        Ok(())
    }

    async fn remove_volume(&self, _name: &str) -> Result<()> {
        self.calls.lock().unwrap().push("remove_volume".into());
        Ok(())
    }

    async fn inspect_network(&self, _name: &str) -> Result<serde_json::Value> {
        Ok(serde_json::json!({}))
    }

    async fn inspect_volume(&self, _name: &str) -> Result<serde_json::Value> {
        Ok(serde_json::json!({}))
    }

    async fn build_image(&self, _context: &str, _tag: &str, _dockerfile: Option<&str>, _args: Option<&HashMap<String, String>>) -> Result<()> {
        self.calls.lock().unwrap().push("build_image".into());
        Ok(())
    }

    async fn inspect_image(&self, _reference: &str) -> Result<serde_json::Value> {
        Ok(serde_json::json!({}))
    }

    async fn manifest_inspect(&self, _reference: &str) -> Result<serde_json::Value> {
        Ok(serde_json::json!({}))
    }

    async fn run_with_security(&self, _spec: &ContainerSpec, _profile: &SecurityProfile) -> Result<ContainerHandle> {
        Ok(ContainerHandle { id: "mock".into(), name: None })
    }

    async fn wait_and_logs(&self, _id: &str) -> Result<ContainerLogs> {
        Ok(ContainerLogs { stdout: "".into(), stderr: "".into() })
    }
}
