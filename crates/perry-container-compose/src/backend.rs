use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use tokio::process::Command;
use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{ContainerSpec, ContainerInfo, ContainerLogs, ImageInfo};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: Option<crate::types::ListOrDict>,
    pub internal: Option<bool>,
    pub enable_ipv6: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeConfig {
    pub driver: Option<String>,
    pub labels: Option<crate::types::ListOrDict>,
}

#[async_trait]
pub trait ContainerBackend: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    async fn run(&self, spec: &ContainerSpec) -> Result<String>;
    async fn create(&self, spec: &ContainerSpec) -> Result<String>;
    async fn start(&self, id: &str) -> Result<()>;
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;
    async fn remove(&self, id: &str, force: bool) -> Result<()>;
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs>;
    async fn exec(&self, id: &str, cmd: &[String], env: Option<std::collections::HashMap<String, String>>, workdir: Option<String>) -> Result<ContainerLogs>;
    async fn pull_image(&self, reference: &str) -> Result<()>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

pub trait CliProtocol: Send + Sync + std::fmt::Debug {
    fn protocol_name(&self) -> &str;
    fn subcommand_prefix(&self) -> Option<Vec<String>> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string(), "--detach".to_string()];
        if let Some(name) = &spec.name {
            args.push("--name".to_string());
            args.push(name.clone());
        }
        if spec.rm.unwrap_or(false) {
            args.push("--rm".to_string());
        }
        if let Some(network) = &spec.network {
            args.push("--network".to_string());
            args.push(network.clone());
        }
        if let Some(ports) = &spec.ports {
            for port in ports {
                args.push("-p".to_string());
                args.push(port.clone());
            }
        }
        if let Some(volumes) = &spec.volumes {
            for vol in volumes {
                args.push("-v".to_string());
                args.push(vol.clone());
            }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env {
                args.push("-e".to_string());
                args.push(format!("{}={}", k, v));
            }
        }
        if let Some(entrypoint) = &spec.entrypoint {
            args.push("--entrypoint".to_string());
            args.push(entrypoint.join(" "));
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.clone());
        }
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = self.run_args(spec);
        if let Some(idx) = args.iter().position(|x| x == "run") {
            args[idx] = "create".to_string();
        }
        args.retain(|x| x != "--detach");
        args
    }

    fn start_args(&self, id: &str) -> Vec<String> {
        vec!["start".to_string(), id.to_string()]
    }

    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".to_string()];
        if let Some(t) = timeout {
            args.push("--time".to_string());
            args.push(t.to_string());
        }
        args.push(id.to_string());
        args
    }

    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".to_string()];
        if force {
            args.push("--force".to_string());
        }
        args.push(id.to_string());
        args
    }

    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["ps".to_string(), "--format".to_string(), "json".to_string()];
        if all {
            args.push("--all".to_string());
        }
        args
    }

    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".to_string(), "--format".to_string(), "json".to_string(), id.to_string()]
    }

    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".to_string()];
        if let Some(n) = tail {
            args.push("--tail".to_string());
            args.push(n.to_string());
        }
        args.push(id.to_string());
        args
    }

    fn exec_args(&self, id: &str, cmd: &[String], env: Option<std::collections::HashMap<String, String>>, workdir: Option<String>) -> Vec<String> {
        let mut args = vec!["exec".to_string()];
        if let Some(w) = workdir {
            args.push("--workdir".to_string());
            args.push(w);
        }
        if let Some(e) = env {
            for (k, v) in e {
                args.push("-e".to_string());
                args.push(format!("{}={}", k, v));
            }
        }
        args.push(id.to_string());
        args.extend(cmd.iter().cloned());
        args
    }

    fn pull_args(&self, reference: &str) -> Vec<String> {
        vec!["pull".to_string(), reference.to_string()]
    }

    fn list_images_args(&self) -> Vec<String> {
        vec!["images".to_string(), "--format".to_string(), "json".to_string()]
    }

    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".to_string()];
        if force {
            args.push("--force".to_string());
        }
        args.push(reference.to_string());
        args
    }

    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".to_string(), "create".to_string()];
        if let Some(driver) = &config.driver {
            args.push("--driver".to_string());
            args.push(driver.clone());
        }
        if config.internal.unwrap_or(false) {
            args.push("--internal".to_string());
        }
        if config.enable_ipv6.unwrap_or(false) {
            args.push("--ipv6".to_string());
        }
        args.push(name.to_string());
        args
    }

    fn remove_network_args(&self, name: &str) -> Vec<String> {
        vec!["network".to_string(), "rm".to_string(), name.to_string()]
    }

    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".to_string(), "create".to_string()];
        if let Some(driver) = &config.driver {
            args.push("--driver".to_string());
            args.push(driver.clone());
        }
        args.push(name.to_string());
        args
    }

    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        vec!["volume".to_string(), "rm".to_string(), name.to_string()]
    }

    fn parse_container_id(&self, output: &str) -> Result<String> {
        Ok(output.trim().to_string())
    }

    fn parse_list_output(&self, output: &str) -> Result<Vec<ContainerInfo>> {
        let mut infos = Vec::new();
        for line in output.lines() {
            if let Ok(info) = serde_json::from_str::<ContainerInfo>(line) {
                infos.push(info);
            }
        }
        Ok(infos)
    }

    fn parse_inspect_output(&self, output: &str) -> Result<ContainerInfo> {
        serde_json::from_str(output).map_err(|e| ComposeError::JsonError(e.to_string()))
    }

    fn parse_list_images_output(&self, output: &str) -> Result<Vec<ImageInfo>> {
        let mut infos = Vec::new();
        for line in output.lines() {
            if let Ok(info) = serde_json::from_str::<ImageInfo>(line) {
                infos.push(info);
            }
        }
        Ok(infos)
    }
}

#[derive(Debug)]
pub struct DockerProtocol;
impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &str { "docker" }
}

#[derive(Debug)]
pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str { "apple/container" }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string()]; // No --detach
        if let Some(name) = &spec.name {
            args.push("--name".to_string());
            args.push(name.clone());
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.clone());
        }
        args
    }

    fn parse_list_output(&self, output: &str) -> Result<Vec<ContainerInfo>> {
        // Simple placeholder for Apple's JSON schema parsing
        Ok(Vec::new())
    }

    fn parse_inspect_output(&self, output: &str) -> Result<ContainerInfo> {
        // Simple placeholder for Apple's JSON schema parsing
        Err(ComposeError::ParseError("Not implemented".to_string()))
    }
}

#[derive(Debug)]
pub struct LimaProtocol {
    pub instance: String,
}
impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &str { "lima" }
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec!["shell".to_string(), self.instance.clone(), "nerdctl".to_string()])
    }
}

#[derive(Debug)]
pub struct CliBackend<P: CliProtocol> {
    pub bin: PathBuf,
    pub protocol: P,
}

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P) -> Self {
        Self { bin, protocol }
    }

    async fn exec_raw(&self, args: &[String]) -> Result<(i32, String, String)> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() {
            cmd.args(prefix);
        }
        cmd.args(args);

        let output = cmd.output().await.map_err(ComposeError::IoError)?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(-1);

        Ok((code, stdout, stderr))
    }

    async fn exec_ok(&self, args: &[String]) -> Result<String> {
        let (code, stdout, stderr) = self.exec_raw(args).await?;
        if code == 0 {
            Ok(stdout)
        } else {
            Err(ComposeError::BackendError {
                message: stderr.trim().to_string(),
                code
            })
        }
    }
}

#[async_trait]
impl<P: CliProtocol + Send + Sync + std::fmt::Debug> ContainerBackend for CliBackend<P> {
    fn name(&self) -> &str {
        self.protocol.protocol_name()
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<String> {
        let args = self.protocol.run_args(spec);
        let output = self.exec_ok(&args).await?;
        self.protocol.parse_container_id(&output)
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<String> {
        let args = self.protocol.create_args(spec);
        let output = self.exec_ok(&args).await?;
        self.protocol.parse_container_id(&output)
    }

    async fn start(&self, id: &str) -> Result<()> {
        let args = self.protocol.start_args(id);
        self.exec_ok(&args).await.map(|_| ())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let args = self.protocol.stop_args(id, timeout);
        self.exec_ok(&args).await.map(|_| ())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_args(id, force);
        self.exec_ok(&args).await.map(|_| ())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let args = self.protocol.list_args(all);
        let output = self.exec_ok(&args).await?;
        self.protocol.parse_list_output(&output)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let args = self.protocol.inspect_args(id);
        let output = self.exec_ok(&args).await?;
        self.protocol.parse_inspect_output(&output)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let args = self.protocol.logs_args(id, tail);
        let (code, stdout, stderr) = self.exec_raw(&args).await?;
        if code == 0 {
            Ok(ContainerLogs { stdout, stderr })
        } else {
            Err(ComposeError::BackendError { message: stderr, code })
        }
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<std::collections::HashMap<String, String>>, workdir: Option<String>) -> Result<ContainerLogs> {
        let args = self.protocol.exec_args(id, cmd, env, workdir);
        let (code, stdout, stderr) = self.exec_raw(&args).await?;
        if code == 0 {
            Ok(ContainerLogs { stdout, stderr })
        } else {
            Err(ComposeError::BackendError { message: stderr, code })
        }
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let args = self.protocol.pull_args(reference);
        self.exec_ok(&args).await.map(|_| ())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let args = self.protocol.list_images_args();
        let output = self.exec_ok(&args).await?;
        self.protocol.parse_list_images_output(&output)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_image_args(reference, force);
        self.exec_ok(&args).await.map(|_| ())
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        let args = self.protocol.create_network_args(name, config);
        self.exec_ok(&args).await.map(|_| ())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_network_args(name);
        self.exec_ok(&args).await.map(|_| ())
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        let args = self.protocol.create_volume_args(name, config);
        self.exec_ok(&args).await.map(|_| ())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_volume_args(name);
        self.exec_ok(&args).await.map(|_| ())
    }
}

pub type DockerBackend = CliBackend<DockerProtocol>;
pub type AppleBackend = CliBackend<AppleContainerProtocol>;
pub type LimaBackend = CliBackend<LimaProtocol>;

pub async fn detect_backend() -> Result<Box<dyn ContainerBackend>> {
    if let Ok(val) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_named_backend(&val).await;
    }

    let os = std::env::consts::OS;
    let candidates = match os {
        "macos" | "ios" => vec![
            ("apple/container", "container"),
            ("orbstack", "docker"),
            ("colima", "docker"),
            ("rancher-desktop", "nerdctl"),
            ("podman", "podman"),
            ("lima", "limactl"),
            ("docker", "docker"),
        ],
        _ => vec![
            ("podman", "podman"),
            ("nerdctl", "nerdctl"),
            ("docker", "docker"),
        ],
    };

    let mut probed = Vec::new();
    for (name, bin) in candidates {
        match probe_backend(name, bin).await {
            Ok(backend) => return Ok(backend),
            Err(reason) => probed.push(BackendProbeResult {
                name: name.to_string(),
                available: false,
                reason: Some(reason),
                version: None,
            }),
        }
    }

    Err(ComposeError::NoBackendFound { probed })
}

async fn probe_named_backend(name: &str) -> Result<Box<dyn ContainerBackend>> {
    let bin = match name {
        "apple/container" => "container",
        "orbstack" | "docker" | "colima" => "docker",
        "rancher-desktop" | "nerdctl" => "nerdctl",
        "podman" => "podman",
        "lima" => "limactl",
        _ => return Err(ComposeError::BackendNotAvailable { name: name.to_string(), reason: "Unknown backend name".to_string() }),
    };

    probe_backend(name, bin).await.map_err(|reason| ComposeError::BackendNotAvailable {
        name: name.to_string(),
        reason
    })
}

async fn probe_backend(name: &str, bin: &str) -> std::result::Result<Box<dyn ContainerBackend>, String> {
    // Check if binary exists
    let bin_path = match which::which(bin) {
        Ok(p) => p,
        Err(_) => return Err(format!("Binary {} not found", bin)),
    };

    // Run --version with timeout
    let mut cmd = Command::new(&bin_path);
    cmd.arg("--version");

    let result = tokio::time::timeout(Duration::from_secs(2), cmd.output()).await;
    match result {
        Ok(Ok(output)) if output.status.success() => {
            // Success, now specific checks
            match name {
                "colima" => {
                    let status = Command::new("colima").arg("status").output().await;
                    if let Ok(out) = status {
                        if !String::from_utf8_lossy(&out.stdout).contains("running") {
                            return Err("Colima is not running".to_string());
                        }
                    }
                }
                "podman" if std::env::consts::OS == "macos" => {
                    let status = Command::new("podman").args(["machine", "list", "--format", "json"]).output().await;
                    if let Ok(out) = status {
                        if !String::from_utf8_lossy(&out.stdout).contains("\"Running\": true") {
                            return Err("Podman machine is not running".to_string());
                        }
                    }
                }
                "lima" => {
                    let status = Command::new("limactl").args(["list", "--json"]).output().await;
                    if let Ok(out) = status {
                        if !String::from_utf8_lossy(&out.stdout).contains("\"Running\"") {
                            return Err("No running Lima instance".to_string());
                        }
                    }
                }
                _ => {}
            }

            // Construct the backend
            let backend: Box<dyn ContainerBackend> = match name {
                "apple/container" => Box::new(CliBackend::new(bin_path, AppleContainerProtocol)),
                "lima" => Box::new(CliBackend::new(bin_path, LimaProtocol { instance: "default".to_string() })),
                _ => Box::new(CliBackend::new(bin_path, DockerProtocol)),
            };
            Ok(backend)
        }
        Ok(Ok(_)) => Err(format!("{} --version failed", bin)),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("Probe timed out".to_string()),
    }
}
