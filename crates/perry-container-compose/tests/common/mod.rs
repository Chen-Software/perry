//! Common test utilities.

use async_trait::async_trait;
use perry_container_compose::backend::{ContainerBackend, NetworkConfig, VolumeConfig, SecurityProfile};
use perry_container_compose::error::{ComposeError, Result};
use perry_container_compose::types::{
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo,
};
use std::collections::HashMap;
use std::sync::Mutex;

pub struct MockBackend {
    pub containers: Mutex<HashMap<String, ContainerInfo>>,
    pub images: Mutex<Vec<ImageInfo>>,
    pub networks: Mutex<Vec<String>>,
    pub volumes: Mutex<Vec<String>>,
}

impl MockBackend {
    pub fn new() -> Self {
        MockBackend {
            containers: Mutex::new(HashMap::new()),
            images: Mutex::new(Vec::new()),
            networks: Mutex::new(Vec::new()),
            volumes: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }
    async fn check_available(&self) -> Result<()> { Ok(()) }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let id = spec.name.clone().unwrap_or_else(|| "mock-id".into());
        let info = ContainerInfo {
            id: id.clone(),
            name: id.clone(),
            image: spec.image.clone(),
            status: "running".into(),
            ports: spec.ports.clone().unwrap_or_default(),
            labels: spec.labels.clone().unwrap_or_default(),
            created: "".into(),
        };
        self.containers.lock().unwrap().insert(id.clone(), info);
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let id = spec.name.clone().unwrap_or_else(|| "mock-id".into());
        let info = ContainerInfo {
            id: id.clone(),
            name: id.clone(),
            image: spec.image.clone(),
            status: "created".into(),
            ports: spec.ports.clone().unwrap_or_default(),
            labels: spec.labels.clone().unwrap_or_default(),
            created: "".into(),
        };
        self.containers.lock().unwrap().insert(id.clone(), info);
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }
    async fn start(&self, id: &str) -> Result<()> {
        if let Some(c) = self.containers.lock().unwrap().get_mut(id) { c.status = "running".into(); Ok(()) }
        else { Err(ComposeError::NotFound(id.into())) }
    }
    async fn stop(&self, id: &str, _timeout: Option<u32>) -> Result<()> {
        if let Some(c) = self.containers.lock().unwrap().get_mut(id) { c.status = "stopped".into(); Ok(()) }
        else { Err(ComposeError::NotFound(id.into())) }
    }
    async fn remove(&self, id: &str, _force: bool) -> Result<()> {
        self.containers.lock().unwrap().remove(id); Ok(())
    }
    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> {
        Ok(self.containers.lock().unwrap().values().cloned().collect())
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.containers.lock().unwrap().get(id).cloned().ok_or_else(|| ComposeError::NotFound(id.into()))
    }
    async fn logs(&self, _id: &str, _tail: Option<u32>, _follow: bool) -> Result<ContainerLogs> {
        Ok(ContainerLogs { stdout: "mock stdout".into(), stderr: "".into() })
    }
    async fn exec(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>, _user: Option<&str>) -> Result<ContainerLogs> {
        Ok(ContainerLogs { stdout: "mock exec output".into(), stderr: "".into() })
    }
    async fn pull_image(&self, _reference: &str) -> Result<()> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(self.images.lock().unwrap().clone()) }
    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn create_network(&self, name: &str, _config: &NetworkConfig) -> Result<()> {
        self.networks.lock().unwrap().push(name.into()); Ok(())
    }
    async fn remove_network(&self, name: &str) -> Result<()> {
        self.networks.lock().unwrap().retain(|n| n != name); Ok(())
    }
    async fn create_volume(&self, name: &str, _config: &VolumeConfig) -> Result<()> {
        self.volumes.lock().unwrap().push(name.into()); Ok(())
    }
    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.volumes.lock().unwrap().retain(|v| v != name); Ok(())
    }
    async fn inspect_network(&self, name: &str) -> Result<serde_json::Value> {
        if self.networks.lock().unwrap().contains(&name.into()) { Ok(serde_json::json!({})) }
        else { Err(ComposeError::NotFound(name.into())) }
    }
    async fn inspect_volume(&self, name: &str) -> Result<serde_json::Value> {
        if self.volumes.lock().unwrap().contains(&name.into()) { Ok(serde_json::json!({})) }
        else { Err(ComposeError::NotFound(name.into())) }
    }
    async fn build_image(&self, _context: &str, _tag: &str, _dockerfile: Option<&str>, _args: Option<&HashMap<String, String>>) -> Result<()> { Ok(()) }
    async fn wait(&self, _id: &str) -> Result<()> { Ok(()) }
    async fn inspect_image(&self, _reference: &str) -> Result<serde_json::Value> { Ok(serde_json::json!({})) }
    async fn manifest_inspect(&self, _reference: &str) -> Result<serde_json::Value> { Ok(serde_json::json!({})) }
    async fn run_with_security(&self, spec: &ContainerSpec, _profile: &SecurityProfile) -> Result<ContainerHandle> { self.run(spec).await }
    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs> { self.logs(id, None, false).await }
}
