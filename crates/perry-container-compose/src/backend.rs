use std::collections::HashMap;
use std::path::{PathBuf};
use std::sync::{Arc, OnceLock};
use tokio::process::Command;
use tokio::sync::Mutex;
use async_trait::async_trait;
use std::time::Duration;

use crate::error::{BackendProbeResult, ComposeError};
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, IsolationLevel, ComposeServiceBuild};

pub enum BackendDriver {
    AppleContainer { bin: PathBuf },
    Orbstack { bin: PathBuf },
    Colima { bin: PathBuf },
    RancherDesktop { bin: PathBuf },
    Lima { bin: PathBuf },
    Podman { bin: PathBuf },
    Nerdctl { bin: PathBuf },
    Docker { bin: PathBuf },
}

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn backend_name(&self) -> &str;
    async fn check_available(&self) -> Result<(), ComposeError>;
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ComposeError>;
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ComposeError>;
    async fn start(&self, id: &str) -> Result<(), ComposeError>;
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<(), ComposeError>;
    async fn remove(&self, id: &str, force: bool) -> Result<(), ComposeError>;
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>, ComposeError>;
    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ComposeError>;
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs, ComposeError>;
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs, ComposeError>;
    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<(), ComposeError>;
    async fn pull_image(&self, reference: &str) -> Result<(), ComposeError>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>, ComposeError>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<(), ComposeError>;

    // Network & Volume ops
    async fn create_network(&self, name: &str) -> Result<(), ComposeError>;
    async fn remove_network(&self, name: &str) -> Result<(), ComposeError>;
    async fn create_volume(&self, name: &str) -> Result<(), ComposeError>;
    async fn remove_volume(&self, name: &str) -> Result<(), ComposeError>;

    fn isolation_level(&self) -> IsolationLevel;
}

pub trait CliProtocol: Send + Sync {
    fn build_args(&self, cmd: &str, args: &[&str]) -> Vec<String>;
}

#[derive(Clone)]
pub struct DockerProtocol;
impl CliProtocol for DockerProtocol {
    fn build_args(&self, cmd: &str, args: &[&str]) -> Vec<String> {
        let mut v = vec![cmd.to_string()];
        for a in args { v.push(a.to_string()); }
        v
    }
}

#[derive(Clone)]
pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn build_args(&self, cmd: &str, args: &[&str]) -> Vec<String> {
        let mut v = vec![cmd.to_string()];
        for a in args { v.push(a.to_string()); }
        v
    }
}

#[derive(Clone)]
pub struct LimaProtocol { pub instance: String }
impl CliProtocol for LimaProtocol {
    fn build_args(&self, cmd: &str, args: &[&str]) -> Vec<String> {
        let mut v = vec!["shell".to_string(), self.instance.clone(), "nerdctl".to_string(), cmd.to_string()];
        for a in args { v.push(a.to_string()); }
        v
    }
}

pub struct CliBackend<P: CliProtocol> {
    pub bin: PathBuf,
    pub name: String,
    pub protocol: P,
    pub isolation: IsolationLevel,
}

#[async_trait]
impl<P: CliProtocol> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str { &self.name }
    fn isolation_level(&self) -> IsolationLevel { self.isolation }

    async fn check_available(&self) -> Result<(), ComposeError> {
        let output = Command::new(&self.bin)
            .args(self.protocol.build_args("--version", &[]))
            .output()
            .await
            .map_err(|e| ComposeError::BackendError { code: -1, message: e.to_string() })?;

        if output.status.success() {
            Ok(())
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string()
            })
        }
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle, ComposeError> {
        let mut args = vec!["run", "-d"];
        if spec.rm.unwrap_or(false) { args.push("--rm"); }
        if let Some(name) = &spec.name {
            args.push("--name");
            args.push(name);
        }
        args.push(&spec.image);
        if let Some(cmd) = &spec.cmd {
            for c in cmd { args.push(c); }
        }

        let full_args = self.protocol.build_args("run", &args[1..]);
        let output = Command::new(&self.bin)
            .args(full_args)
            .output()
            .await
            .map_err(|e| ComposeError::BackendError { code: -1, message: e.to_string() })?;

        if output.status.success() {
            Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string() })
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string()
            })
        }
    }

    async fn create(&self, _spec: &ContainerSpec) -> Result<ContainerHandle, ComposeError> { todo!() }
    async fn start(&self, id: &str) -> Result<(), ComposeError> {
        let args = self.protocol.build_args("start", &[id]);
        self.exec_void(args).await
    }
    async fn stop(&self, id: &str, _timeout: Option<u32>) -> Result<(), ComposeError> {
        let args = self.protocol.build_args("stop", &[id]);
        self.exec_void(args).await
    }
    async fn remove(&self, id: &str, force: bool) -> Result<(), ComposeError> {
        let mut args = vec!["rm"];
        if force { args.push("-f"); }
        args.push(id);
        let full_args = self.protocol.build_args("rm", &args[1..]);
        self.exec_void(full_args).await
    }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>, ComposeError> {
        let mut args = vec!["ps", "--format", "json"];
        if all { args.push("-a"); }
        let full_args = self.protocol.build_args("ps", &args[1..]);
        let output = Command::new(&self.bin).args(full_args).output().await.map_err(|e| ComposeError::BackendError { code: -1, message: e.to_string() })?;
        if !output.status.success() { return Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() }); }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut containers = Vec::new();
        for line in stdout.lines() {
            if let Ok(info) = serde_json::from_str::<ContainerInfo>(line) {
                containers.push(info);
            }
        }
        Ok(containers)
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ComposeError> {
        let args = self.protocol.build_args("inspect", &[id, "--format", "json"]);
        let output = Command::new(&self.bin).args(args).output().await.map_err(|e| ComposeError::BackendError { code: -1, message: e.to_string() })?;
        if !output.status.success() { return Err(ComposeError::NotFound(id.to_string())); }
        let infos: Vec<ContainerInfo> = serde_json::from_slice(&output.stdout).map_err(|e| ComposeError::BackendError { code: -1, message: e.to_string() })?;
        infos.into_iter().next().ok_or_else(|| ComposeError::NotFound(id.to_string()))
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs, ComposeError> {
        let mut args = vec!["logs"];
        let tail_str;
        if let Some(n) = tail {
            tail_str = n.to_string();
            args.push("--tail");
            args.push(&tail_str);
        }
        args.push(id);
        let full_args = self.protocol.build_args("logs", &args[1..]);
        let output = Command::new(&self.bin).args(full_args).output().await.map_err(|e| ComposeError::BackendError { code: -1, message: e.to_string() })?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
    async fn exec(&self, id: &str, cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs, ComposeError> {
        let mut args = vec!["exec", id];
        for c in cmd { args.push(c); }
        let full_args = self.protocol.build_args("exec", &args[1..]);
        let output = Command::new(&self.bin).args(full_args).output().await.map_err(|e| ComposeError::BackendError { code: -1, message: e.to_string() })?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<(), ComposeError> {
        let mut args = vec!["build", "-t", image_name];
        if let Some(df) = &spec.containerfile {
            args.push("-f");
            args.push(df);
        }
        args.push(&spec.context);
        let full_args = self.protocol.build_args("build", &args[1..]);
        self.exec_void(full_args).await
    }
    async fn pull_image(&self, reference: &str) -> Result<(), ComposeError> {
        let args = self.protocol.build_args("pull", &[reference]);
        self.exec_void(args).await
    }
    async fn list_images(&self) -> Result<Vec<ImageInfo>, ComposeError> { todo!() }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<(), ComposeError> {
        let mut args = vec!["rmi"];
        if force { args.push("-f"); }
        args.push(reference);
        let full_args = self.protocol.build_args("rmi", &args[1..]);
        self.exec_void(full_args).await
    }
    async fn create_network(&self, name: &str) -> Result<(), ComposeError> {
        let args = self.protocol.build_args("network", &["create", name]);
        self.exec_void(args).await
    }
    async fn remove_network(&self, name: &str) -> Result<(), ComposeError> {
        let args = self.protocol.build_args("network", &["rm", name]);
        self.exec_void(args).await
    }
    async fn create_volume(&self, name: &str) -> Result<(), ComposeError> {
        let args = self.protocol.build_args("volume", &["create", name]);
        self.exec_void(args).await
    }
    async fn remove_volume(&self, name: &str) -> Result<(), ComposeError> {
        let args = self.protocol.build_args("volume", &["rm", name]);
        self.exec_void(args).await
    }
}

impl<P: CliProtocol> CliBackend<P> {
    async fn exec_void(&self, args: Vec<String>) -> Result<(), ComposeError> {
        let output = Command::new(&self.bin)
            .args(args)
            .output()
            .await
            .map_err(|e| ComposeError::BackendError { code: -1, message: e.to_string() })?;
        if output.status.success() {
            Ok(())
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string()
            })
        }
    }
}

pub async fn detect_backend() -> Result<Arc<dyn ContainerBackend>, ComposeError> {
    if let Ok(override_name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        let bin = PathBuf::from(&override_name);
        return Ok(Arc::new(CliBackend {
            bin,
            name: override_name,
            protocol: DockerProtocol,
            isolation: IsolationLevel::Container,
        }));
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(bin) = probe_candidate("container").await {
            return Ok(Arc::new(CliBackend { bin, name: "apple/container".to_string(), protocol: AppleContainerProtocol, isolation: IsolationLevel::Container }));
        }
        if let Ok(bin) = probe_candidate("orb").await {
            return Ok(Arc::new(CliBackend { bin, name: "orbstack".to_string(), protocol: DockerProtocol, isolation: IsolationLevel::MicroVm }));
        }
        if let Ok(bin) = probe_candidate("colima").await {
            return Ok(Arc::new(CliBackend { bin, name: "colima".to_string(), protocol: DockerProtocol, isolation: IsolationLevel::Container }));
        }
    }

    if let Ok(bin) = probe_candidate("podman").await {
        return Ok(Arc::new(CliBackend { bin, name: "podman".to_string(), protocol: DockerProtocol, isolation: IsolationLevel::Container }));
    }
    if let Ok(bin) = probe_candidate("docker").await {
        return Ok(Arc::new(CliBackend { bin, name: "docker".to_string(), protocol: DockerProtocol, isolation: IsolationLevel::Container }));
    }

    Err(ComposeError::NoBackendFound { probed: vec![] })
}

async fn probe_candidate(bin_name: &str) -> Result<PathBuf, ComposeError> {
    let path = which::which(bin_name).map_err(|_| ComposeError::NotFound(bin_name.to_string()))?;
    let mut cmd = Command::new(&path);
    cmd.arg("--version");

    let output = tokio::time::timeout(Duration::from_secs(2), cmd.output()).await
        .map_err(|_| ComposeError::BackendError { code: -1, message: "Probe timeout".to_string() })?
        .map_err(|e| ComposeError::BackendError { code: -1, message: e.to_string() })?;

    if output.status.success() {
        Ok(path)
    } else {
        Err(ComposeError::BackendError { code: -1, message: "Probe failed".to_string() })
    }
}

static GLOBAL_BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();
static BACKEND_MUTEX: Mutex<()> = Mutex::const_new(());

pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, ComposeError> {
    if let Some(backend) = GLOBAL_BACKEND.get() {
        return Ok(backend.clone());
    }

    let _guard = BACKEND_MUTEX.lock().await;
    if let Some(backend) = GLOBAL_BACKEND.get() {
        return Ok(backend.clone());
    }

    let backend = detect_backend().await?;
    let _ = GLOBAL_BACKEND.set(backend.clone());
    Ok(backend)
}
