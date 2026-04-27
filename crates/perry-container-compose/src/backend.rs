use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::error::{ComposeError, BackendProbeResult};
use crate::types::*;
use crate::installer::BackendInstaller;

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn backend_name(&self) -> &str;
    async fn check_available(&self) -> Result<(), ComposeError>;
    async fn run(&self, spec: &ContainerSpec) -> Result<String, ComposeError>; // Returns container ID
    async fn create(&self, spec: &ContainerSpec) -> Result<String, ComposeError>;
    async fn start(&self, id: &str) -> Result<(), ComposeError>;
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<(), ComposeError>;
    async fn remove(&self, id: &str, force: bool) -> Result<(), ComposeError>;
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>, ComposeError>;
    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ComposeError>;
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs, ComposeError>;
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs, ComposeError>;
    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<(), ComposeError>;
    async fn pull_image(&self, reference: &str) -> Result<(), ComposeError>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>, ComposeError>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<(), ComposeError>;
    async fn create_network(&self, name: &str, config: Option<&serde_json::Value>) -> Result<(), ComposeError>;
    async fn remove_network(&self, name: &str) -> Result<(), ComposeError>;
    async fn create_volume(&self, name: &str, config: Option<&serde_json::Value>) -> Result<(), ComposeError>;
    async fn remove_volume(&self, name: &str) -> Result<(), ComposeError>;
}

pub trait CliProtocol: Send + Sync {
    fn name(&self) -> &str;
    fn build_run_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn build_create_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn build_start_args(&self, id: &str) -> Vec<String>;
    fn build_stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String>;
    fn build_remove_args(&self, id: &str, force: bool) -> Vec<String>;
    fn build_list_args(&self, all: bool) -> Vec<String>;
    fn build_inspect_args(&self, id: &str) -> Vec<String>;
    fn build_logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String>;
    fn build_exec_args(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String>;
    fn build_build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String>;
    fn build_pull_args(&self, reference: &str) -> Vec<String>;
    fn build_list_images_args(&self) -> Vec<String>;
    fn build_remove_image_args(&self, reference: &str, force: bool) -> Vec<String>;
    fn build_create_network_args(&self, name: &str, config: Option<&serde_json::Value>) -> Vec<String>;
    fn build_remove_network_args(&self, name: &str) -> Vec<String>;
    fn build_create_volume_args(&self, name: &str, config: Option<&serde_json::Value>) -> Vec<String>;
    fn build_remove_volume_args(&self, name: &str) -> Vec<String>;
}

pub struct DockerProtocol;

impl CliProtocol for DockerProtocol {
    fn name(&self) -> &str { "docker" }
    fn build_run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string(), "-d".to_string()];
        if let Some(name) = &spec.name {
            args.push("--name".to_string());
            args.push(name.clone());
        }
        if let Some(ports) = &spec.ports {
            for p in ports {
                args.push("-p".to_string());
                args.push(p.clone());
            }
        }
        if let Some(volumes) = &spec.volumes {
            for v in volumes {
                args.push("-v".to_string());
                args.push(v.clone());
            }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env {
                args.push("-e".to_string());
                args.push(format!("{}={}", k, v));
            }
        }
        if let Some(network) = &spec.network {
            args.push("--network".to_string());
            args.push(network.clone());
        }
        if spec.rm.unwrap_or(false) {
            args.push("--rm".to_string());
        }
        if let Some(entrypoint) = &spec.entrypoint {
            args.push("--entrypoint".to_string());
            args.push(entrypoint[0].clone()); // Simplified
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.clone());
        }
        args
    }
    fn build_create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".to_string()];
        if let Some(name) = &spec.name {
            args.push("--name".to_string());
            args.push(name.clone());
        }
        args.push(spec.image.clone());
        args
    }
    fn build_start_args(&self, id: &str) -> Vec<String> { vec!["start".to_string(), id.to_string()] }
    fn build_stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".to_string()];
        if let Some(t) = timeout {
            args.push("-t".to_string());
            args.push(t.to_string());
        }
        args.push(id.to_string());
        args
    }
    fn build_remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".to_string()];
        if force { args.push("-f".to_string()); }
        args.push(id.to_string());
        args
    }
    fn build_list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["ps".to_string(), "--format".to_string(), "json".to_string()];
        if all { args.push("-a".to_string()); }
        args
    }
    fn build_inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".to_string(), id.to_string()]
    }
    fn build_logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".to_string()];
        if let Some(n) = tail {
            args.push("--tail".to_string());
            args.push(n.to_string());
        }
        args.push(id.to_string());
        args
    }
    fn build_exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".to_string()];
        if let Some(e) = env {
            for (k, v) in e {
                args.push("-e".to_string());
                args.push(format!("{}={}", k, v));
            }
        }
        if let Some(w) = workdir {
            args.push("-w".to_string());
            args.push(w.to_string());
        }
        args.push(id.to_string());
        args.extend(cmd.iter().cloned());
        args
    }
    fn build_build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String> {
        let mut args = vec!["build".to_string(), "-t".to_string(), image_name.to_string()];
        if let Some(f) = &spec.containerfile {
            args.push("-f".to_string());
            args.push(f.clone());
        }
        args.push(spec.context.clone());
        args
    }
    fn build_pull_args(&self, reference: &str) -> Vec<String> { vec!["pull".to_string(), reference.to_string()] }
    fn build_list_images_args(&self) -> Vec<String> { vec!["images".to_string(), "--format".to_string(), "json".to_string()] }
    fn build_remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".to_string()];
        if force { args.push("-f".to_string()); }
        args.push(reference.to_string());
        args
    }
    fn build_create_network_args(&self, name: &str, _config: Option<&serde_json::Value>) -> Vec<String> {
        vec!["network".to_string(), "create".to_string(), name.to_string()]
    }
    fn build_remove_network_args(&self, name: &str) -> Vec<String> {
        vec!["network".to_string(), "rm".to_string(), name.to_string()]
    }
    fn build_create_volume_args(&self, name: &str, _config: Option<&serde_json::Value>) -> Vec<String> {
        vec!["volume".to_string(), "create".to_string(), name.to_string()]
    }
    fn build_remove_volume_args(&self, name: &str) -> Vec<String> {
        vec!["volume".to_string(), "rm".to_string(), name.to_string()]
    }
}

pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn name(&self) -> &str { "apple-container" }
    fn build_run_args(&self, _spec: &ContainerSpec) -> Vec<String> { vec![] }
    fn build_create_args(&self, _spec: &ContainerSpec) -> Vec<String> { vec![] }
    fn build_start_args(&self, _id: &str) -> Vec<String> { vec![] }
    fn build_stop_args(&self, _id: &str, _timeout: Option<u32>) -> Vec<String> { vec![] }
    fn build_remove_args(&self, _id: &str, _force: bool) -> Vec<String> { vec![] }
    fn build_list_args(&self, _all: bool) -> Vec<String> { vec![] }
    fn build_inspect_args(&self, _id: &str) -> Vec<String> { vec![] }
    fn build_logs_args(&self, _id: &str, _tail: Option<u32>) -> Vec<String> { vec![] }
    fn build_exec_args(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Vec<String> { vec![] }
    fn build_build_args(&self, _spec: &ComposeServiceBuild, _image_name: &str) -> Vec<String> { vec![] }
    fn build_pull_args(&self, _reference: &str) -> Vec<String> { vec![] }
    fn build_list_images_args(&self) -> Vec<String> { vec![] }
    fn build_remove_image_args(&self, _reference: &str, _force: bool) -> Vec<String> { vec![] }
    fn build_create_network_args(&self, _name: &str, _config: Option<&serde_json::Value>) -> Vec<String> { vec![] }
    fn build_remove_network_args(&self, _name: &str) -> Vec<String> { vec![] }
    fn build_create_volume_args(&self, _name: &str, _config: Option<&serde_json::Value>) -> Vec<String> { vec![] }
    fn build_remove_volume_args(&self, _name: &str) -> Vec<String> { vec![] }
}

pub struct CliBackend {
    pub name: String,
    pub bin: PathBuf,
    pub protocol: Box<dyn CliProtocol>,
}

#[async_trait]
impl ContainerBackend for CliBackend {
    fn backend_name(&self) -> &str { &self.name }
    async fn check_available(&self) -> Result<(), ComposeError> {
        let output = tokio::process::Command::new(&self.bin)
            .arg("--version")
            .output()
            .await
            .map_err(|e| ComposeError::BackendNotAvailable { name: self.name.clone(), reason: e.to_string() })?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendNotAvailable { name: self.name.clone(), reason: "Version check failed".to_string() })
        }
    }
    async fn run(&self, spec: &ContainerSpec) -> Result<String, ComposeError> {
        let args = self.protocol.build_run_args(spec);
        let output = tokio::process::Command::new(&self.bin)
            .args(&args)
            .output()
            .await
            .map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<String, ComposeError> {
        let args = self.protocol.build_create_args(spec);
        let output = tokio::process::Command::new(&self.bin)
            .args(&args)
            .output()
            .await
            .map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn start(&self, id: &str) -> Result<(), ComposeError> {
        let args = self.protocol.build_start_args(id);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<(), ComposeError> {
        let args = self.protocol.build_stop_args(id, timeout);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn remove(&self, id: &str, force: bool) -> Result<(), ComposeError> {
        let args = self.protocol.build_remove_args(id, force);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>, ComposeError> {
        let args = self.protocol.build_list_args(all);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() {
            Ok(vec![])
        } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo, ComposeError> {
        let args = self.protocol.build_inspect_args(id);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() {
            Ok(ContainerInfo { id: id.to_string(), name: id.to_string(), image: "".into(), status: "running".into(), ports: vec![], created: "".into() })
        } else {
            Err(ComposeError::NotFound(id.to_string()))
        }
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs, ComposeError> {
        let args = self.protocol.build_logs_args(id, tail);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs, ComposeError> {
        let args = self.protocol.build_exec_args(id, cmd, env, workdir);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<(), ComposeError> {
        let args = self.protocol.build_build_args(spec, image_name);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn pull_image(&self, reference: &str) -> Result<(), ComposeError> {
        let args = self.protocol.build_pull_args(reference);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn list_images(&self) -> Result<Vec<ImageInfo>, ComposeError> {
        let args = self.protocol.build_list_images_args();
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() { Ok(vec![]) } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<(), ComposeError> {
        let args = self.protocol.build_remove_image_args(reference, force);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn create_network(&self, name: &str, config: Option<&serde_json::Value>) -> Result<(), ComposeError> {
        let args = self.protocol.build_create_network_args(name, config);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn remove_network(&self, name: &str) -> Result<(), ComposeError> {
        let args = self.protocol.build_remove_network_args(name);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn create_volume(&self, name: &str, config: Option<&serde_json::Value>) -> Result<(), ComposeError> {
        let args = self.protocol.build_create_volume_args(name, config);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
    async fn remove_volume(&self, name: &str) -> Result<(), ComposeError> {
        let args = self.protocol.build_remove_volume_args(name);
        let output = tokio::process::Command::new(&self.bin).args(&args).output().await.map_err(|e| ComposeError::Other(e.to_string()))?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&output.stderr).to_string() })
        }
    }
}

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

impl BackendDriver {
    pub fn name(&self) -> &str {
        match self {
            BackendDriver::AppleContainer { .. } => "apple/container",
            BackendDriver::Orbstack { .. } => "orbstack",
            BackendDriver::Colima { .. } => "colima",
            BackendDriver::RancherDesktop { .. } => "rancher-desktop",
            BackendDriver::Lima { .. } => "lima",
            BackendDriver::Podman { .. } => "podman",
            BackendDriver::Nerdctl { .. } => "nerdctl",
            BackendDriver::Docker { .. } => "docker",
        }
    }
}

pub async fn detect_backend() -> Result<Arc<dyn ContainerBackend>, ComposeError> {
    if let Ok(val) = std::env::var("PERRY_CONTAINER_BACKEND") {
        let bin = PathBuf::from(&val);
        let protocol: Box<dyn CliProtocol> = if val.contains("apple") { Box::new(AppleContainerProtocol) } else { Box::new(DockerProtocol) };
        let backend = Arc::new(CliBackend { name: val.clone(), bin, protocol });
        backend.check_available().await?;
        return Ok(backend);
    }

    let candidates = platform_candidates();
    let mut probed = Vec::new();

    for driver in candidates {
        match probe_candidate(&driver).await {
            Ok(backend) => return Ok(backend),
            Err(e) => probed.push(BackendProbeResult {
                name: driver.name().to_string(),
                available: false,
                reason: e.to_string(),
                version: None,
                mode: "local".into(),
                isolation_level: IsolationLevel::Container,
            }),
        }
    }

    if let Ok(backend) = BackendInstaller::run().await {
        return Ok(backend);
    }

    Err(ComposeError::NoBackendFound { probed })
}

fn platform_candidates() -> Vec<BackendDriver> {
    vec![
        BackendDriver::Podman { bin: "podman".into() },
        BackendDriver::Docker { bin: "docker".into() },
    ]
}

async fn probe_candidate(driver: &BackendDriver) -> Result<Arc<dyn ContainerBackend>, ComposeError> {
    let bin = match driver {
        BackendDriver::Podman { bin } => bin,
        BackendDriver::Docker { bin } => bin,
        _ => return Err(ComposeError::Other("Unsupported driver".into())),
    };
    let protocol: Box<dyn CliProtocol> = Box::new(DockerProtocol);
    let backend = Arc::new(CliBackend { name: driver.name().to_string(), bin: bin.clone(), protocol });
    backend.check_available().await?;
    Ok(backend)
}

static GLOBAL_BACKEND: Mutex<Option<Arc<dyn ContainerBackend>>> = Mutex::const_new(None);

pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend>, ComposeError> {
    let mut lock = GLOBAL_BACKEND.lock().await;
    if let Some(backend) = &*lock {
        return Ok(backend.clone());
    }
    let backend = detect_backend().await?;
    *lock = Some(backend.clone());
    Ok(backend)
}
