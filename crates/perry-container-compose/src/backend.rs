//! Container backend abstraction.

use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use std::process::Stdio;

/// Abstraction over different container backends.
#[async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Backend name for display (e.g. "apple-container", "podman")
    fn backend_name(&self) -> &str;

    /// Check whether the backend is available.
    async fn check_available(&self) -> Result<()>;

    /// Run a container.
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Create a container.
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Start a container.
    async fn start(&self, id: &str) -> Result<()>;

    /// Stop a container.
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;

    /// Remove a container.
    async fn remove(&self, id: &str, force: bool) -> Result<()>;

    /// List containers.
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;

    /// Inspect a container.
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;

    /// Fetch logs.
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs>;

    /// Execute command.
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs>;

    /// Pull image.
    async fn pull_image(&self, reference: &str) -> Result<()>;

    /// List images.
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;

    /// Remove image.
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;

    /// Create network.
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;

    /// Remove network.
    async fn remove_network(&self, name: &str) -> Result<()>;

    /// Create volume.
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;

    /// Remove volume.
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

/// Translates abstract container operations into CLI arguments.
pub trait CliProtocol: Send + Sync {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn start_args(&self, id: &str) -> Vec<String>;
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String>;
    fn remove_args(&self, id: &str, force: bool) -> Vec<String>;
    fn list_args(&self, all: bool) -> Vec<String>;
    fn inspect_args(&self, id: &str) -> Vec<String>;
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String>;
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String>;
    fn pull_image_args(&self, reference: &str) -> Vec<String>;
    fn list_images_args(&self) -> Vec<String>;
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String>;
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String>;
    fn remove_network_args(&self, name: &str) -> Vec<String>;
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String>;
    fn remove_volume_args(&self, name: &str) -> Vec<String>;

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>>;
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo>;
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>>;
    fn parse_container_id(&self, stdout: &str) -> Result<String>;
}

pub struct CliBackend {
    pub bin: PathBuf,
    pub protocol: Box<dyn CliProtocol>,
}

impl CliBackend {
    pub fn new(bin: PathBuf, protocol: Box<dyn CliProtocol>) -> Self {
        Self { bin, protocol }
    }

    async fn execute(&self, args: &[String]) -> Result<(String, String)> {
        let output = Command::new(&self.bin)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(ComposeError::IoError)?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok((stdout, stderr))
        } else {
            let code = output.status.code().unwrap_or(-1);
            Err(ComposeError::BackendError { code, message: stderr })
        }
    }
}

#[async_trait]
impl ContainerBackend for CliBackend {
    fn backend_name(&self) -> &str {
        self.bin.file_name().and_then(|n| n.to_str()).unwrap_or("unknown")
    }

    async fn check_available(&self) -> Result<()> {
        let _ = self.execute(&["--version".to_string()]).await?;
        Ok(())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.run_args(spec);
        let (stdout, _) = self.execute(&args).await?;
        let id = self.protocol.parse_container_id(&stdout)?;
        Ok(ContainerHandle { id: id.clone(), name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.create_args(spec);
        let (stdout, _) = self.execute(&args).await?;
        let id = self.protocol.parse_container_id(&stdout)?;
        Ok(ContainerHandle { id: id.clone(), name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> {
        let args = self.protocol.start_args(id);
        let _ = self.execute(&args).await?;
        Ok(())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let args = self.protocol.stop_args(id, timeout);
        let _ = self.execute(&args).await?;
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_args(id, force);
        let _ = self.execute(&args).await?;
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let args = self.protocol.list_args(all);
        let (stdout, _) = self.execute(&args).await?;
        self.protocol.parse_list_output(&stdout)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let args = self.protocol.inspect_args(id);
        let (stdout, _) = self.execute(&args).await?;
        self.protocol.parse_inspect_output(&stdout)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let args = self.protocol.logs_args(id, tail);
        let (stdout, stderr) = self.execute(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let args = self.protocol.exec_args(id, cmd, env, workdir);
        let (stdout, stderr) = self.execute(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let args = self.protocol.pull_image_args(reference);
        let _ = self.execute(&args).await?;
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let args = self.protocol.list_images_args();
        let (stdout, _) = self.execute(&args).await?;
        self.protocol.parse_list_images_output(&stdout)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_image_args(reference, force);
        let _ = self.execute(&args).await?;
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> {
        let args = self.protocol.create_network_args(name, config);
        let _ = self.execute(&args).await?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_network_args(name);
        let res = self.execute(&args).await;
        match res {
            Ok(_) => Ok(()),
            Err(ComposeError::BackendError { message, .. }) if message.contains("not found") || message.contains("No such") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        let args = self.protocol.create_volume_args(name, config);
        let _ = self.execute(&args).await?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_volume_args(name);
        let res = self.execute(&args).await;
        match res {
            Ok(_) => Ok(()),
            Err(ComposeError::BackendError { message, .. }) if message.contains("not found") || message.contains("No such") => Ok(()),
            Err(e) => Err(e),
        }
    }
}

pub struct DockerProtocol;

impl CliProtocol for DockerProtocol {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string(), "-d".to_string()];
        if let Some(name) = &spec.name { args.extend([ "--name".to_string(), name.clone() ]); }
        if let Some(ports) = &spec.ports { for p in ports { args.extend([ "-p".to_string(), p.clone() ]); } }
        if let Some(vols) = &spec.volumes { for v in vols { args.extend([ "-v".to_string(), v.clone() ]); } }
        if let Some(envs) = &spec.env { for (k, v) in envs { args.extend([ "-e".to_string(), format!("{}={}", k, v) ]); } }
        if let Some(net) = &spec.network { args.extend([ "--network".to_string(), net.clone() ]); }
        if spec.rm.unwrap_or(false) { args.push("--rm".to_string()); }
        if let Some(ep) = &spec.entrypoint { args.extend([ "--entrypoint".to_string(), ep.join(" ") ]); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".to_string()];
        if let Some(name) = &spec.name { args.extend([ "--name".to_string(), name.clone() ]); }
        if let Some(ports) = &spec.ports { for p in ports { args.extend([ "-p".to_string(), p.clone() ]); } }
        if let Some(vols) = &spec.volumes { for v in vols { args.extend([ "-v".to_string(), v.clone() ]); } }
        if let Some(envs) = &spec.env { for (k, v) in envs { args.extend([ "-e".to_string(), format!("{}={}", k, v) ]); } }
        if let Some(net) = &spec.network { args.extend([ "--network".to_string(), net.clone() ]); }
        if let Some(ep) = &spec.entrypoint { args.extend([ "--entrypoint".to_string(), ep.join(" ") ]); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }
    fn start_args(&self, id: &str) -> Vec<String> { vec!["start".to_string(), id.to_string()] }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".to_string()];
        if let Some(t) = timeout { args.extend([ "--time".to_string(), t.to_string() ]); }
        args.push(id.to_string());
        args
    }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".to_string()];
        if force { args.push("-f".to_string()); }
        args.push(id.to_string());
        args
    }
    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["ps".to_string(), "--format".to_string(), "json".to_string()];
        if all { args.push("--all".to_string()); }
        args
    }
    fn inspect_args(&self, id: &str) -> Vec<String> { vec!["inspect".to_string(), "--format".to_string(), "json".to_string(), id.to_string()] }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".to_string()];
        if let Some(t) = tail { args.extend([ "--tail".to_string(), t.to_string() ]); }
        args.push(id.to_string());
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".to_string()];
        if let Some(envs) = env { for (k, v) in envs { args.extend([ "-e".to_string(), format!("{}={}", k, v) ]); } }
        if let Some(wd) = workdir { args.extend([ "--workdir".to_string(), wd.to_string() ]); }
        args.push(id.to_string());
        args.extend(cmd.iter().cloned());
        args
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> { vec!["pull".to_string(), reference.to_string()] }
    fn list_images_args(&self) -> Vec<String> { vec!["images".to_string(), "--format".to_string(), "json".to_string()] }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".to_string()];
        if force { args.push("-f".to_string()); }
        args.push(reference.to_string());
        args
    }
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> {
        let mut args = vec!["network".to_string(), "create".to_string()];
        if let Some(d) = &config.driver { args.extend([ "--driver".to_string(), d.clone() ]); }
        if let Some(lbls) = &config.labels { for (k, v) in lbls.to_map() { args.extend([ "--label".to_string(), format!("{}={}", k, v) ]); } }
        args.push(name.to_string());
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> { vec!["network".to_string(), "rm".to_string(), name.to_string()] }
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> {
        let mut args = vec!["volume".to_string(), "create".to_string()];
        if let Some(d) = &config.driver { args.extend([ "--driver".to_string(), d.clone() ]); }
        if let Some(lbls) = &config.labels { for (k, v) in lbls.to_map() { args.extend([ "--label".to_string(), format!("{}={}", k, v) ]); } }
        args.push(name.to_string());
        args
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".to_string(), "rm".to_string(), name.to_string()] }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        #[derive(serde::Deserialize)]
        struct DockerPs { ID: String, Names: String, Image: String, Status: String, Ports: String, CreatedAt: String }
        let entries: Vec<DockerPs> = serde_json::from_str(stdout).unwrap_or_default();
        Ok(entries.into_iter().map(|e| ContainerInfo {
            id: e.ID, name: e.Names, image: e.Image, status: e.Status, ports: vec![e.Ports], created: e.CreatedAt
        }).collect())
    }
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        #[derive(serde::Deserialize)]
        struct DockerInspect { Id: String, Name: String, Config: DockerConfig, State: DockerState, Created: String }
        #[derive(serde::Deserialize)]
        struct DockerConfig { Image: String }
        #[derive(serde::Deserialize)]
        struct DockerState { Status: String }
        let entries: Vec<DockerInspect> = serde_json::from_str(stdout).map_err(ComposeError::JsonError)?;
        let e = entries.into_iter().next().ok_or_else(|| ComposeError::NotFound("inspect output empty".to_string()))?;
        Ok(ContainerInfo {
            id: e.Id, name: e.Name.trim_start_matches('/').to_string(), image: e.Config.Image, status: e.State.Status, ports: vec![], created: e.Created
        })
    }
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        #[derive(serde::Deserialize)]
        struct DockerImage { ID: String, Repository: String, Tag: String, Size: String, CreatedAt: String }
        let entries: Vec<DockerImage> = serde_json::from_str(stdout).unwrap_or_default();
        Ok(entries.into_iter().map(|e| ImageInfo {
            id: e.ID, repository: e.Repository, tag: e.Tag, size: 0, created: e.CreatedAt
        }).collect())
    }
    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        Ok(stdout.trim().to_string())
    }
}

pub struct AppleContainerProtocol;

impl CliProtocol for AppleContainerProtocol {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string()];
        if spec.rm.unwrap_or(false) { args.push("--rm".to_string()); }
        if let Some(name) = &spec.name { args.extend([ "--name".to_string(), name.clone() ]); }
        if let Some(ports) = &spec.ports { for p in ports { args.extend([ "-p".to_string(), p.clone() ]); } }
        if let Some(vols) = &spec.volumes { for v in vols { args.extend([ "-v".to_string(), v.clone() ]); } }
        if let Some(envs) = &spec.env { for (k, v) in envs { args.extend([ "-e".to_string(), format!("{}={}", k, v) ]); } }
        if let Some(net) = &spec.network { args.extend([ "--network".to_string(), net.clone() ]); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".to_string()];
        if let Some(name) = &spec.name { args.extend([ "--name".to_string(), name.clone() ]); }
        if let Some(ports) = &spec.ports { for p in ports { args.extend([ "-p".to_string(), p.clone() ]); } }
        if let Some(vols) = &spec.volumes { for v in vols { args.extend([ "-v".to_string(), v.clone() ]); } }
        if let Some(envs) = &spec.env { for (k, v) in envs { args.extend([ "-e".to_string(), format!("{}={}", k, v) ]); } }
        if let Some(net) = &spec.network { args.extend([ "--network".to_string(), net.clone() ]); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }
    fn start_args(&self, id: &str) -> Vec<String> { vec!["start".to_string(), id.to_string()] }
    fn stop_args(&self, id: &str, _timeout: Option<u32>) -> Vec<String> { vec!["stop".to_string(), id.to_string()] }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".to_string()];
        if force { args.push("-f".to_string()); }
        args.push(id.to_string());
        args
    }
    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["ps".to_string(), "--format".to_string(), "json".to_string()];
        if all { args.push("--all".to_string()); }
        args
    }
    fn inspect_args(&self, id: &str) -> Vec<String> { vec!["inspect".to_string(), "--format".to_string(), "json".to_string(), id.to_string()] }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".to_string()];
        if let Some(t) = tail { args.extend([ "--tail".to_string(), t.to_string() ]); }
        args.push(id.to_string());
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".to_string()];
        if let Some(envs) = env { for (k, v) in envs { args.extend([ "-e".to_string(), format!("{}={}", k, v) ]); } }
        if let Some(wd) = workdir { args.extend([ "--workdir".to_string(), wd.to_string() ]); }
        args.push(id.to_string());
        args.extend(cmd.iter().cloned());
        args
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> { vec!["pull".to_string(), reference.to_string()] }
    fn list_images_args(&self) -> Vec<String> { vec!["images".to_string(), "--format".to_string(), "json".to_string()] }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".to_string()];
        if force { args.push("-f".to_string()); }
        args.push(reference.to_string());
        args
    }
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> {
        let mut args = vec!["network".to_string(), "create".to_string()];
        if let Some(d) = &config.driver { args.extend([ "--driver".to_string(), d.clone() ]); }
        if let Some(lbls) = &config.labels { for (k, v) in lbls.to_map() { args.extend([ "--label".to_string(), format!("{}={}", k, v) ]); } }
        args.push(name.to_string());
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> { vec!["network".to_string(), "rm".to_string(), name.to_string()] }
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> {
        let mut args = vec!["volume".to_string(), "create".to_string()];
        if let Some(d) = &config.driver { args.extend([ "--driver".to_string(), d.clone() ]); }
        if let Some(lbls) = &config.labels { for (k, v) in lbls.to_map() { args.extend([ "--label".to_string(), format!("{}={}", k, v) ]); } }
        args.push(name.to_string());
        args
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".to_string(), "rm".to_string(), name.to_string()] }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        #[derive(serde::Deserialize)]
        struct ApplePs { ID: String, Names: Vec<String>, Image: String, Status: String, Ports: Vec<String>, Created: String }
        let entries: Vec<ApplePs> = serde_json::from_str(stdout).unwrap_or_default();
        Ok(entries.into_iter().map(|e| ContainerInfo {
            id: e.ID, name: e.Names.into_iter().next().unwrap_or_default(), image: e.Image, status: e.Status, ports: e.Ports, created: e.Created
        }).collect())
    }
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        #[derive(serde::Deserialize)]
        struct AppleInspect { State: AppleState }
        #[derive(serde::Deserialize)]
        struct AppleState { Running: bool }
        let info: AppleInspect = serde_json::from_str(stdout).map_err(ComposeError::JsonError)?;
        Ok(ContainerInfo {
            id: String::new(), name: String::new(), image: String::new(), status: if info.State.Running { "running".to_string() } else { "stopped".to_string() }, ports: vec![], created: String::new()
        })
    }
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        #[derive(serde::Deserialize)]
        struct AppleImage { ID: String, Repository: String, Tag: String, Size: u64, Created: String }
        let entries: Vec<AppleImage> = serde_json::from_str(stdout).unwrap_or_default();
        Ok(entries.into_iter().map(|e| ImageInfo {
            id: e.ID, repository: e.Repository, tag: e.Tag, size: e.Size, created: e.Created
        }).collect())
    }
    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        Ok(stdout.trim().to_string())
    }
}

pub async fn detect_backend() -> Result<Box<dyn ContainerBackend>> {
    let candidates = if cfg!(target_os = "macos") {
        vec!["apple/container", "orbstack", "colima", "podman", "docker"]
    } else {
        vec!["podman", "nerdctl", "docker"]
    };

    let mut errors = Vec::new();
    for name in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(name)).await {
            Ok(Ok(backend)) => return Ok(Box::new(backend)),
            Ok(Err(e)) => errors.push(BackendProbeResult { name: name.to_string(), available: false, reason: e }),
            Err(_) => errors.push(BackendProbeResult { name: name.to_string(), available: false, reason: "timeout".to_string() }),
        }
    }

    Err(ComposeError::NoBackendFound { probed: errors })
}

async fn probe_candidate(name: &str) -> std::result::Result<CliBackend, String> {
    let bin_name = match name {
        "apple/container" => "container",
        "orbstack" | "docker" | "colima" => "docker",
        "podman" => "podman",
        "nerdctl" => "nerdctl",
        _ => name,
    };

    let bin = which::which(bin_name).map_err(|_| format!("{} binary not found", bin_name))?;

    let protocol: Box<dyn CliProtocol> = if name == "apple/container" {
        Box::new(AppleContainerProtocol)
    } else {
        Box::new(DockerProtocol)
    };

    let backend = CliBackend::new(bin, protocol);
    backend.check_available().await.map_err(|e| e.to_string())?;

    if name == "colima" {
        let output = Command::new(&backend.bin).arg("status").output().await.map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.contains("running") { return Err("colima is not running".to_string()); }
    }

    Ok(backend)
}

// ============ Legacy Backend trait (for backward compat) ============

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerStatus { Running, Stopped, NotFound }
#[derive(Debug, Clone)]
pub struct ExecResult { pub stdout: String, pub stderr: String, pub exit_code: i32 }

#[async_trait]
pub trait Backend: Send + Sync {
    fn name(&self) -> &'static str;
    async fn build(&self, context: &str, dockerfile: Option<&str>, tag: &str, args: Option<&HashMap<String, String>>, target: Option<&str>, network: Option<&str>) -> Result<()>;
    async fn run(&self, image: &str, name: &str, ports: Option<&[String]>, env: Option<&HashMap<String, String>>, volumes: Option<&[String]>, labels: Option<&HashMap<String, String>>, cmd: Option<&[String]>, detach: bool) -> Result<()>;
    async fn start(&self, name: &str) -> Result<()>;
    async fn stop(&self, name: &str) -> Result<()>;
    async fn remove(&self, name: &str, force: bool) -> Result<()>;
    async fn inspect(&self, name: &str) -> Result<ContainerStatus>;
    async fn list(&self, label_filter: Option<&str>) -> Result<Vec<ContainerInfo>>;
    async fn logs(&self, name: &str, tail: Option<u32>, follow: bool) -> Result<String>;
    async fn exec(&self, name: &str, cmd: &[String], user: Option<&str>, workdir: Option<&str>, env: Option<&HashMap<String, String>>) -> Result<ExecResult>;
    async fn create_network(&self, name: &str, driver: Option<&str>, labels: Option<&HashMap<String, String>>) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, driver: Option<&str>, labels: Option<&HashMap<String, String>>) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

pub fn get_backend() -> Result<Box<dyn Backend>> {
    Err(ComposeError::validation("get_backend is deprecated, use detect_backend"))
}

pub fn get_container_backend() -> Result<Box<dyn ContainerBackend>> {
    Err(ComposeError::validation("get_container_backend is deprecated, use detect_backend"))
}
