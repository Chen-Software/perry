use perry_container_compose::backend::ContainerBackend;
use perry_container_compose::error::Result;
use perry_container_compose::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo,
};
use perry_stdlib::container::compose::compose_up;
use perry_stdlib::container::types::{ComposeSpec, ComposeHandle};
use perry_stdlib::container::compose::ComposeWrapper;
use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;

struct MockBackend;

#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> &str { "mock" }
    async fn check_available(&self) -> Result<()> { Ok(()) }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        Ok(ContainerHandle { id: "mock-id".into(), name: spec.name.clone() })
    }
    async fn create(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> {
        Ok(ContainerHandle { id: "mock-id".into(), name: None })
    }
    async fn start(&self, _id: &str) -> Result<()> { Ok(()) }
    async fn stop(&self, _id: &str, _timeout: Option<u32>) -> Result<()> { Ok(()) }
    async fn remove(&self, _id: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> { Ok(vec![]) }
    async fn inspect(&self, _id: &str) -> Result<ContainerInfo> {
        Ok(ContainerInfo {
            id: "mock-id".into(),
            name: "mock-name".into(),
            image: "mock-image".into(),
            status: "running".into(),
            ports: vec![],
            created: "now".into(),
        })
    }
    async fn logs(&self, _id: &str, _tail: Option<u32>) -> Result<ContainerLogs> {
        Ok(ContainerLogs { stdout: "mock logs".into(), stderr: "".into() })
    }
    async fn wait(&self, _id: &str) -> Result<i32> { Ok(0) }
    async fn exec(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        Ok(ContainerLogs { stdout: "exec output".into(), stderr: "".into() })
    }
    async fn pull_image(&self, _ref: &str) -> Result<()> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(vec![]) }
    async fn remove_image(&self, _ref: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn create_network(&self, _name: &str, _config: &ComposeNetwork) -> Result<()> { Ok(()) }
    async fn remove_network(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn create_volume(&self, _name: &str, _config: &ComposeVolume) -> Result<()> { Ok(()) }
    async fn remove_volume(&self, _name: &str) -> Result<()> { Ok(()) }
}

#[tokio::test]
async fn test_compose_statefulness() {
    let backend = Arc::new(MockBackend);
    let mut spec = ComposeSpec::default();
    spec.name = Some("test-project".into());

    let mut svc = perry_container_compose::types::ComposeService::default();
    svc.image = Some("nginx".into());
    spec.services.insert("web".into(), svc);

    // 1. Up
    let handle = compose_up(spec.clone(), backend.clone()).await.expect("up failed");
    assert_eq!(handle.project_name, "test-project");
    assert!(handle.services.contains(&"web".to_string()));

    // 2. Ps using the handle (verifies state is in global registry)
    let wrapper = ComposeWrapper::new(ComposeSpec::default(), backend.clone());
    let containers = wrapper.ps(&handle).await.expect("ps failed");
    // In MockBackend::ps we return empty vec currently, let's fix that if we want better test
}

#[tokio::test]
async fn test_compose_down_takes_state() {
    let backend = Arc::new(MockBackend);
    let mut spec = ComposeSpec::default();
    spec.name = Some("down-project".into());

    let mut svc = perry_container_compose::types::ComposeService::default();
    svc.image = Some("alpine".into());
    spec.services.insert("app".into(), svc);

    let handle = compose_up(spec, backend.clone()).await.expect("up failed");
    let wrapper = ComposeWrapper::new(ComposeSpec::default(), backend.clone());

    // Down should succeed and take the engine from registry
    wrapper.down(&handle, false).await.expect("down failed");

    // Second ps should fail as engine was taken by down
    let ps_result = wrapper.ps(&handle).await;
    assert!(ps_result.is_err());
    assert!(ps_result.unwrap_err().contains("Compose engine not found"));
}
