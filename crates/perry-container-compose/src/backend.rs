//! Container backend abstraction.
//!
//! Defines the `ContainerBackend` async trait, `CliProtocol` trait for CLI command building,
//! and `CliBackend` generic executor.

use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{
    ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

/// Minimal network creation config — driver and labels only.
#[derive(Debug, Clone, Default)]
pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
    pub internal: bool,
    pub enable_ipv6: bool,
}

/// Minimal volume creation config — driver and labels only.
#[derive(Debug, Clone, Default)]
pub struct VolumeConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
}

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn backend_name(&self) -> &str;
    async fn check_available(&self) -> Result<()>;
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;
    async fn start(&self, id: &str) -> Result<()>;
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;
    async fn remove(&self, id: &str, force: bool) -> Result<()>;
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs>;
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs>;
    async fn pull_image(&self, reference: &str) -> Result<()>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

/// Translates abstract container operations into CLI arguments.
pub trait CliProtocol: Send + Sync {
    fn protocol_name(&self) -> &str;
    fn subcommand_prefix(&self) -> Option<Vec<String>> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn start_args(&self, id: &str) -> Vec<String> { vec!["start".into(), id.into()] }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout {
            args.push("-t".into());
            args.push(t.to_string());
        }
        args.push(id.into());
        args
    }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".into()];
        if force { args.push("-f".into()); }
        args.push(id.into());
        args
    }
    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all { args.push("--all".into()); }
        args
    }
    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".into(), "--format".into(), "json".into(), id.into()]
    }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if let Some(t) = tail {
            args.push("--tail".into());
            args.push(t.to_string());
        }
        args.push(id.into());
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(wd) = workdir {
            args.push("--workdir".into());
            args.push(wd.into());
        }
        if let Some(envs) = env {
            for (k, v) in envs {
                args.push("-e".into());
                args.push(format!("{}={}", k, v));
            }
        }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        args
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> { vec!["pull".into(), reference.into()] }
    fn list_images_args(&self) -> Vec<String> { vec!["images".into(), "--format".into(), "json".into()] }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".into()];
        if force { args.push("-f".into()); }
        args.push(reference.into());
        args
    }
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver {
            args.push("--driver".into());
            args.push(d.clone());
        }
        for (k, v) in &config.labels {
            args.push("--label".into());
            args.push(format!("{}={}", k, v));
        }
        if config.internal { args.push("--internal".into()); }
        args.push(name.into());
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> { vec!["network".into(), "rm".into(), name.into()] }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver {
            args.push("--driver".into());
            args.push(d.clone());
        }
        for (k, v) in &config.labels {
            args.push("--label".into());
            args.push(format!("{}={}", k, v));
        }
        args.push(name.into());
        args
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".into(), "rm".into(), name.into()] }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo>;
    fn parse_inspect_output(&self, id: &str, stdout: &str) -> Option<ContainerInfo>;
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo>;
    fn parse_container_id(&self, stdout: &str) -> String { stdout.trim().to_string() }
}

fn docker_run_flags(spec: &ContainerSpec, include_detach: bool) -> Vec<String> {
    let mut args = vec!["run".into()];
    if include_detach { args.push("-d".into()); }
    if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
    if let Some(name) = &spec.name {
        args.push("--name".into());
        args.push(name.clone());
    }
    if let Some(network) = &spec.network {
        args.push("--network".into());
        args.push(network.clone());
    }
    if let Some(ports) = &spec.ports {
        for p in ports { args.push("-p".into()); args.push(p.clone()); }
    }
    if let Some(vols) = &spec.volumes {
        for v in vols { args.push("-v".into()); args.push(v.clone()); }
    }
    if let Some(envs) = &spec.env {
        for (k, v) in envs {
            args.push("-e".into());
            args.push(format!("{}={}", k, v));
        }
    }
    args.push(spec.image.clone());
    if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
    args
}

pub struct DockerProtocol;
impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &str { "docker-compatible" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> { docker_run_flags(spec, true) }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> { docker_run_flags(spec, false) }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        #[derive(Deserialize)]
        struct Entry {
            #[serde(rename = "ID", alias = "id")] id: String,
            #[serde(rename = "Names", alias = "names")] names: Option<Vec<String>>,
            #[serde(rename = "Image", alias = "image")] image: String,
            #[serde(rename = "Status", alias = "status")] status: String,
            #[serde(rename = "Ports", alias = "ports")] ports: Option<Vec<String>>,
            #[serde(rename = "Created", alias = "created")] created: String,
        }
        serde_json::from_str::<Vec<Entry>>(stdout).unwrap_or_default()
            .into_iter().map(|e| ContainerInfo {
                id: e.id,
                name: e.names.and_then(|v| v.into_iter().next()).unwrap_or_default(),
                image: e.image,
                status: e.status,
                ports: e.ports.unwrap_or_default(),
                created: e.created,
            }).collect()
    }

    fn parse_inspect_output(&self, id: &str, stdout: &str) -> Option<ContainerInfo> {
        #[derive(Deserialize)]
        struct Inspect {
            #[serde(rename = "State")] state: Option<State>,
        }
        #[derive(Deserialize)]
        struct State {
            #[serde(rename = "Running")] running: Option<bool>,
        }
        let v: Vec<Inspect> = serde_json::from_str(stdout).ok()?;
        let info = v.into_iter().next()?;
        let running = info.state.and_then(|s| s.running).unwrap_or(false);
        Some(ContainerInfo {
            id: id.to_string(), name: id.to_string(), image: String::new(),
            status: if running { "running" } else { "stopped" }.to_string(),
            ports: vec![], created: String::new(),
        })
    }

    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> {
        #[derive(Deserialize)]
        struct Image {
            #[serde(rename = "ID")] id: String,
            #[serde(rename = "Repository")] repository: String,
            #[serde(rename = "Tag")] tag: String,
            #[serde(rename = "Size")] size: u64,
            #[serde(rename = "Created")] created: String,
        }
        serde_json::from_str::<Vec<Image>>(stdout).unwrap_or_default()
            .into_iter().map(|e| ImageInfo {
                id: e.id, repository: e.repository, tag: e.tag, size: e.size, created: e.created,
            }).collect()
    }
}

pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str { "apple/container" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> { docker_run_flags(spec, false) }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> { docker_run_flags(spec, false) }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> { DockerProtocol.parse_list_output(stdout) }
    fn parse_inspect_output(&self, id: &str, stdout: &str) -> Option<ContainerInfo> { DockerProtocol.parse_inspect_output(id, stdout) }
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> { DockerProtocol.parse_list_images_output(stdout) }
}

pub struct LimaProtocol { pub instance: String }
impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &str { "lima" }
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec!["shell".into(), self.instance.clone(), "nerdctl".into()])
    }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> { docker_run_flags(spec, true) }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> { docker_run_flags(spec, false) }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> { DockerProtocol.parse_list_output(stdout) }
    fn parse_inspect_output(&self, id: &str, stdout: &str) -> Option<ContainerInfo> { DockerProtocol.parse_inspect_output(id, stdout) }
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> { DockerProtocol.parse_list_images_output(stdout) }
}

pub struct CliBackend<P: CliProtocol> {
    pub bin: PathBuf,
    pub protocol: P,
}

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P) -> Self { Self { bin, protocol } }

    async fn exec_raw(&self, subcommand_args: Vec<String>) -> Result<std::process::Output> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() {
            cmd.args(prefix);
        }
        cmd.args(subcommand_args);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd.output().await.map_err(ComposeError::IoError)
    }

    async fn exec_ok(&self, args: Vec<String>) -> Result<String> {
        let output = self.exec_raw(args).await?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(ComposeError::BackendError { code, message: stderr })
        }
    }
}

#[async_trait]
impl<P: CliProtocol> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str {
        self.bin.file_name().and_then(|n| n.to_str()).unwrap_or("unknown")
    }
    async fn check_available(&self) -> Result<()> {
        let mut args = self.protocol.subcommand_prefix().unwrap_or_default();
        args.push("--version".into());
        let mut cmd = Command::new(&self.bin);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
        let status = cmd.status().await.map_err(ComposeError::IoError)?;
        if status.success() { Ok(()) } else {
            Err(ComposeError::BackendNotAvailable {
                name: self.backend_name().to_string(),
                reason: "version check failed".to_string(),
            })
        }
    }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(self.protocol.run_args(spec)).await?;
        let id = self.protocol.parse_container_id(&stdout);
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(self.protocol.create_args(spec)).await?;
        let id = self.protocol.parse_container_id(&stdout);
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }
    async fn start(&self, id: &str) -> Result<()> { self.exec_ok(self.protocol.start_args(id)).await?; Ok(()) }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> { self.exec_ok(self.protocol.stop_args(id, timeout)).await?; Ok(()) }
    async fn remove(&self, id: &str, force: bool) -> Result<()> { self.exec_ok(self.protocol.remove_args(id, force)).await?; Ok(()) }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let stdout = self.exec_ok(self.protocol.list_args(all)).await?;
        Ok(self.protocol.parse_list_output(&stdout))
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let stdout = self.exec_ok(self.protocol.inspect_args(id)).await?;
        self.protocol.parse_inspect_output(id, &stdout).ok_or_else(|| ComposeError::NotFound(id.to_string()))
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let stdout = self.exec_ok(self.protocol.logs_args(id, tail)).await?;
        Ok(ContainerLogs { stdout, stderr: String::new(), exit_code: 0 })
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let output = self.exec_raw(self.protocol.exec_args(id, cmd, env, workdir)).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(0),
        })
    }
    async fn pull_image(&self, reference: &str) -> Result<()> { self.exec_ok(self.protocol.pull_image_args(reference)).await?; Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let stdout = self.exec_ok(self.protocol.list_images_args()).await?;
        Ok(self.protocol.parse_list_images_output(&stdout))
    }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> { self.exec_ok(self.protocol.remove_image_args(reference, force)).await?; Ok(()) }
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> { self.exec_ok(self.protocol.create_network_args(name, config)).await?; Ok(()) }
    async fn remove_network(&self, name: &str) -> Result<()> { self.exec_ok(self.protocol.remove_network_args(name)).await?; Ok(()) }
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> { self.exec_ok(self.protocol.create_volume_args(name, config)).await?; Ok(()) }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec_ok(self.protocol.remove_volume_args(name)).await?; Ok(()) }
}

pub async fn detect_backend() -> Result<Box<dyn ContainerBackend>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await.map_err(|reason| ComposeError::BackendNotAvailable { name, reason: reason.to_string() });
    }

    let candidates = if cfg!(target_os = "macos") {
        vec!["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"]
    } else {
        vec!["podman", "nerdctl", "docker"]
    };

    let mut probed = Vec::new();
    for name in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(name)).await {
            Ok(Ok(b)) => return Ok(b),
            Ok(Err(reason)) => probed.push(BackendProbeResult { name: name.to_string(), available: false, reason: reason.to_string() }),
            Err(_) => probed.push(BackendProbeResult { name: name.to_string(), available: false, reason: "timeout".into() }),
        }
    }
    Err(ComposeError::NoBackendFound { probed })
}

async fn probe_candidate(name: &str) -> Result<Box<dyn ContainerBackend>> {
    match name {
        "apple/container" => {
            let bin = which::which("container").map_err(|e| ComposeError::from(e.to_string()))?;
            Ok(Box::new(CliBackend::new(bin, AppleContainerProtocol)))
        }
        "podman" => {
            let bin = which::which("podman").map_err(|e| ComposeError::from(e.to_string()))?;
            if cfg!(target_os = "macos") {
                let output = Command::new(&bin).args(["machine", "list", "--format", "json"]).output().await.map_err(|e| ComposeError::from(e.to_string()))?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                if !stdout.contains("\"Running\":true") && !stdout.contains("\"Running\": true") {
                    return Err(ComposeError::BackendNotAvailable { name: "podman".into(), reason: "no running podman machine found".into() });
                }
            }
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "docker" => {
            let bin = which::which("docker").map_err(|e| ComposeError::from(e.to_string()))?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "orbstack" => {
            let bin = which::which("orb").map_err(|e| ComposeError::from(e.to_string()))?;
            // OrbStack also checks for socket at ~/.orbstack/run/docker.sock or orb --version
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "colima" => {
            let bin = which::which("colima").map_err(|e| ComposeError::from(e.to_string()))?;
            let output = Command::new(&bin).arg("status").output().await.map_err(|e| ComposeError::from(e.to_string()))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.contains("running") {
                return Err(ComposeError::BackendNotAvailable { name: "colima".into(), reason: "colima is not running".into() });
            }
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "lima" => {
            let bin = which::which("limactl").map_err(|e| ComposeError::from(e.to_string()))?;
            let output = Command::new(&bin).args(["list", "--json"]).output().await.map_err(|e| ComposeError::from(e.to_string()))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.contains("\"Running\"") {
                return Err(ComposeError::BackendNotAvailable { name: "lima".into(), reason: "no running lima instance found".into() });
            }
            Ok(Box::new(CliBackend::new(bin, LimaProtocol { instance: "default".into() })))
        }
        "nerdctl" => {
            let bin = which::which("nerdctl").map_err(|e| ComposeError::from(e.to_string()))?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "rancher-desktop" => {
            let bin = which::which("nerdctl").map_err(|e| ComposeError::from(e.to_string()))?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        _ => Err(ComposeError::Generic("unknown backend".to_string())),
    }
}

pub async fn get_backend() -> Result<Box<dyn ContainerBackend>> {
    detect_backend().await
}
