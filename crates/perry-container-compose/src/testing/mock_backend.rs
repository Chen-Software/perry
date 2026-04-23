use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use crate::backend::{ContainerBackend, NetworkConfig, VolumeConfig};
use crate::error::Result;
use crate::types::{ContainerSpec, ContainerInfo, ContainerLogs, ImageInfo};

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
    PullImage(String),
    ListImages,
    RemoveImage(String, bool),
    CreateNetwork(String, NetworkConfig),
    RemoveNetwork(String),
    CreateVolume(String, VolumeConfig),
    RemoveVolume(String),
}

#[derive(Debug)]
pub enum MockResponse {
    Run(Result<String>),
    Create(Result<String>),
    Start(Result<()>),
    Stop(Result<()>),
    Remove(Result<()>),
    List(Result<Vec<ContainerInfo>>),
    Inspect(Result<ContainerInfo>),
    Logs(Result<ContainerLogs>),
    Exec(Result<ContainerLogs>),
    PullImage(Result<()>),
    ListImages(Result<Vec<ImageInfo>>),
    RemoveImage(Result<()>),
    CreateNetwork(Result<()>),
    RemoveNetwork(Result<()>),
    CreateVolume(Result<()>),
    RemoveVolume(Result<()>),
}

pub struct MockBackend {
    pub calls: Arc<Mutex<Vec<RecordedCall>>>,
    pub responses: Arc<Mutex<VecDeque<MockResponse>>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn push_response(&self, response: MockResponse) {
        self.responses.lock().unwrap().push_back(response);
    }

    pub fn take_calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().unwrap().drain(..).collect()
    }

    fn pop_response<T, F>(&self, f: F) -> Option<T>
    where F: FnOnce(MockResponse) -> Option<T> {
        let mut resps = self.responses.lock().unwrap();
        // Check if front matches
        if let Some(front) = resps.front() {
             // We can't easily check variant without consuming or using a macro
             // but we want to avoid popping the wrong type
        }
        // For simplicity in this mock, we just pop if it exists.
        // To make it robust, we should only pop if it matches the expected variant.
        // Let's use a simpler approach: if the front matches the variant we want, pop it.
        // Since we can't easily pass a pattern to a function, let's just do it in each method.
        None
    }
}

impl std::fmt::Debug for MockBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockBackend").finish()
    }
}

#[async_trait]
impl ContainerBackend for MockBackend {
    fn name(&self) -> &str { "mock" }

    async fn run(&self, spec: &ContainerSpec) -> Result<String> {
        self.calls.lock().unwrap().push(RecordedCall::Run(spec.clone()));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::Run(_)) = resps.front() {
            if let Some(MockResponse::Run(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok("mock-id".to_string())
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<String> {
        self.calls.lock().unwrap().push(RecordedCall::Create(spec.clone()));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::Create(_)) = resps.front() {
            if let Some(MockResponse::Create(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok("mock-id".to_string())
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Start(id.to_string()));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::Start(_)) = resps.front() {
            if let Some(MockResponse::Start(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Stop(id.to_string(), timeout));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::Stop(_)) = resps.front() {
            if let Some(MockResponse::Stop(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::Remove(id.to_string(), force));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::Remove(_)) = resps.front() {
            if let Some(MockResponse::Remove(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        self.calls.lock().unwrap().push(RecordedCall::List(all));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::List(_)) = resps.front() {
            if let Some(MockResponse::List(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(Vec::new())
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.calls.lock().unwrap().push(RecordedCall::Inspect(id.to_string()));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::Inspect(_)) = resps.front() {
            if let Some(MockResponse::Inspect(r)) = resps.pop_front() {
                return r;
            }
        }
        Err(crate::error::ComposeError::NotFound(id.to_string()))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        self.calls.lock().unwrap().push(RecordedCall::Logs(id.to_string(), tail));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::Logs(_)) = resps.front() {
            if let Some(MockResponse::Logs(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(ContainerLogs { stdout: "".to_string(), stderr: "".to_string() })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<HashMap<String, String>>, workdir: Option<String>) -> Result<ContainerLogs> {
        self.calls.lock().unwrap().push(RecordedCall::Exec(id.to_string(), cmd.to_vec(), env.clone(), workdir.clone()));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::Exec(_)) = resps.front() {
            if let Some(MockResponse::Exec(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(ContainerLogs { stdout: "".to_string(), stderr: "".to_string() })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::PullImage(reference.to_string()));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::PullImage(_)) = resps.front() {
            if let Some(MockResponse::PullImage(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        self.calls.lock().unwrap().push(RecordedCall::ListImages);
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::ListImages(_)) = resps.front() {
            if let Some(MockResponse::ListImages(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(Vec::new())
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::RemoveImage(reference.to_string(), force));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::RemoveImage(_)) = resps.front() {
            if let Some(MockResponse::RemoveImage(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::CreateNetwork(name.to_string(), config.clone()));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::CreateNetwork(_)) = resps.front() {
            if let Some(MockResponse::CreateNetwork(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::RemoveNetwork(name.to_string()));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::RemoveNetwork(_)) = resps.front() {
            if let Some(MockResponse::RemoveNetwork(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(())
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::CreateVolume(name.to_string(), config.clone()));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::CreateVolume(_)) = resps.front() {
            if let Some(MockResponse::CreateVolume(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.calls.lock().unwrap().push(RecordedCall::RemoveVolume(name.to_string()));
        let mut resps = self.responses.lock().unwrap();
        if let Some(MockResponse::RemoveVolume(_)) = resps.front() {
            if let Some(MockResponse::RemoveVolume(r)) = resps.pop_front() {
                return r;
            }
        }
        Ok(())
    }
}
