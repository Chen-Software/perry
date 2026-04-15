//! Container backend system.
//!
//! Structured as four layers:
//! 1. `ContainerBackend` trait: Abstract operations (runtime-agnostic)
//! 2. `CliProtocol` trait: CLI argument building + output parsing
//! 3. `CliBackend` struct: Binary executor implementing Layer 1 via Layer 2
//! 4. `detect_backend()`: Runtime detection and auto-configuration

use crate::error::{BackendProbeResult, ComposeError, Result};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use which::which;

// ============ Layer 1: Abstract Operations ============

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
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

// ============ Layer 2: CLI Protocol ============

pub trait CliProtocol: Send + Sync {
    fn subcommand_prefix(&self) -> Option<&str> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn start_args(&self, id: &str) -> Vec<String>;
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String>;
    fn remove_args(&self, id: &str, force: bool) -> Vec<String>;
    fn list_args(&self, all: bool) -> Vec<String>;
    fn inspect_args(&self, id: &str) -> Vec<String>;
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String>;
    fn exec_args(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String>;
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

/// Docker-compatible CLI protocol (podman, nerdctl, orbstack, docker, colima).
pub struct DockerProtocol;

impl CliProtocol for DockerProtocol {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into(), "--detach".into()];
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        if let Some(ports) = &spec.ports {
            for p in ports { args.extend(["-p".into(), p.clone()]); }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols { args.extend(["-v".into(), v.clone()]); }
        }
        if let Some(envs) = &spec.env {
            for (k, v) in envs { args.extend(["-e".into(), format!("{}={}", k, v)]); }
        }
        if let Some(net) = &spec.network { args.extend(["--network".into(), net.clone()]); }
        if let Some(ep) = &spec.entrypoint {
             args.push("--entrypoint".into());
             args.push(ep.join(" "));
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        if let Some(ports) = &spec.ports {
            for p in ports { args.extend(["-p".into(), p.clone()]); }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols { args.extend(["-v".into(), v.clone()]); }
        }
        if let Some(envs) = &spec.env {
            for (k, v) in envs { args.extend(["-e".into(), format!("{}={}", k, v)]); }
        }
        if let Some(net) = &spec.network { args.extend(["--network".into(), net.clone()]); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }

    fn start_args(&self, id: &str) -> Vec<String> { vec!["start".into(), id.into()] }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout { args.extend(["--time".into(), t.to_string()]); }
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
        if let Some(t) = tail { args.extend(["--tail".into(), t.to_string()]); }
        args.push(id.into());
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(wd) = workdir { args.extend(["--workdir".into(), wd.into()]); }
        if let Some(envs) = env {
            for (k, v) in envs { args.extend(["-e".into(), format!("{}={}", k, v)]); }
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
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(driver) = &config.driver { args.extend(["--driver".into(), driver.clone()]); }
        args.push(name.into());
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> { vec!["network".into(), "rm".into(), name.into()] }
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(driver) = &config.driver { args.extend(["--driver".into(), driver.clone()]); }
        args.push(name.into());
        args
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".into(), "rm".into(), name.into()] }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        #[derive(serde::Deserialize)]
        struct DockerPs {
            ID: String, Names: String, Image: String, Status: String, Ports: String, CreatedAt: String,
        }
        let entries: Vec<DockerPs> = stdout.lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        Ok(entries.into_iter().map(|e| ContainerInfo {
            id: e.ID, name: e.Names, image: e.Image, status: e.Status,
            ports: e.Ports.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            created: e.CreatedAt,
        }).collect())
    }

    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        #[derive(serde::Deserialize)]
        struct DockerInspect {
            Id: String, Name: String, Config: DockerInspectConfig, State: DockerInspectState, Created: String,
        }
        #[derive(serde::Deserialize)]
        struct DockerInspectConfig { Image: String }
        #[derive(serde::Deserialize)]
        struct DockerInspectState { Status: String }

        let list: Vec<DockerInspect> = serde_json::from_str(stdout)?;
        let e = list.into_iter().next().ok_or_else(|| ComposeError::NotFound("inspect output empty".into()))?;
        Ok(ContainerInfo {
            id: e.Id, name: e.Name.trim_start_matches('/').to_string(), image: e.Config.Image,
            status: e.State.Status, ports: vec![], created: e.Created,
        })
    }

    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        #[derive(serde::Deserialize)]
        struct DockerImage {
            ID: String, Repository: String, Tag: String, Size: String, CreatedAt: String,
        }
        let entries: Vec<DockerImage> = stdout.lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        Ok(entries.into_iter().map(|e| ImageInfo {
            id: e.ID, repository: e.Repository, tag: e.Tag,
            size: 0, // Parsing size strings ("100MB") is complex, skip for now
            created: e.CreatedAt,
        }).collect())
    }

    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        Ok(stdout.trim().to_string())
    }
}

/// Apple Container protocol.
pub struct AppleContainerProtocol;

impl CliProtocol for AppleContainerProtocol {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        if let Some(ports) = &spec.ports {
            for p in ports { args.extend(["-p".into(), p.clone()]); }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols { args.extend(["-v".into(), v.clone()]); }
        }
        if let Some(envs) = &spec.env {
            for (k, v) in envs { args.extend(["-e".into(), format!("{}={}", k, v)]); }
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        args.push(spec.image.clone());
        args
    }

    fn start_args(&self, id: &str) -> Vec<String> { vec!["start".into(), id.into()] }
    fn stop_args(&self, id: &str, _timeout: Option<u32>) -> Vec<String> { vec!["stop".into(), id.into()] }
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
        if let Some(t) = tail { args.extend(["--tail".into(), t.to_string()]); }
        args.push(id.into());
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".into(), id.into()];
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
    fn create_network_args(&self, name: &str, _config: &ComposeNetwork) -> Vec<String> { vec!["network".into(), "create".into(), name.into()] }
    fn remove_network_args(&self, name: &str) -> Vec<String> { vec!["network".into(), "rm".into(), name.into()] }
    fn create_volume_args(&self, name: &str, _config: &ComposeVolume) -> Vec<String> { vec!["volume".into(), "create".into(), name.into()] }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".into(), "rm".into(), name.into()] }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        DockerProtocol.parse_list_output(stdout)
    }

    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        DockerProtocol.parse_inspect_output(stdout)
    }

    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        DockerProtocol.parse_list_images_output(stdout)
    }

    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        Ok(stdout.trim().to_string())
    }
}

/// Lima protocol (limactl shell <instance> nerdctl <cmd>).
pub struct LimaProtocol {
    pub instance: String,
}

impl CliProtocol for LimaProtocol {
    fn subcommand_prefix(&self) -> Option<&str> { Some("shell") }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.run_args(spec));
        args
    }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.create_args(spec));
        args
    }
    fn start_args(&self, id: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.start_args(id));
        args
    }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.stop_args(id, timeout));
        args
    }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_args(id, force));
        args
    }
    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.list_args(all));
        args
    }
    fn inspect_args(&self, id: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.inspect_args(id));
        args
    }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.logs_args(id, tail));
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.exec_args(id, cmd, env, workdir));
        args
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.pull_image_args(reference));
        args
    }
    fn list_images_args(&self) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.list_images_args());
        args
    }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_image_args(reference, force));
        args
    }
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.create_network_args(name, config));
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_network_args(name));
        args
    }
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.create_volume_args(name, config));
        args
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_volume_args(name));
        args
    }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> { DockerProtocol.parse_list_output(stdout) }
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> { DockerProtocol.parse_inspect_output(stdout) }
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> { DockerProtocol.parse_list_images_output(stdout) }
    fn parse_container_id(&self, stdout: &str) -> Result<String> { DockerProtocol.parse_container_id(stdout) }
}

// ============ Layer 3: CLI Executor ============

pub struct CliBackend {
    pub bin: PathBuf,
    pub protocol: Box<dyn CliProtocol>,
}

impl CliBackend {
    pub fn new(bin: PathBuf, protocol: Box<dyn CliProtocol>) -> Self {
        Self { bin, protocol }
    }

    async fn exec_raw(&self, args: &[String]) -> Result<(String, String)> {
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
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr,
            })
        }
    }
}

#[async_trait]
impl ContainerBackend for CliBackend {
    fn backend_name(&self) -> &str {
        self.bin.file_name().and_then(|n| n.to_str()).unwrap_or("unknown")
    }

    async fn check_available(&self) -> Result<()> {
        let mut args = Vec::new();
        if let Some(prefix) = self.protocol.subcommand_prefix() {
            args.push(prefix.to_string());
        }
        args.push("--version".to_string());
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let (stdout, _) = self.exec_raw(&self.protocol.run_args(spec)).await?;
        let id = self.protocol.parse_container_id(&stdout)?;
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let (stdout, _) = self.exec_raw(&self.protocol.create_args(spec)).await?;
        let id = self.protocol.parse_container_id(&stdout)?;
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.exec_raw(&self.protocol.start_args(id)).await.map(|_| ())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.exec_raw(&self.protocol.stop_args(id, timeout)).await.map(|_| ())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.exec_raw(&self.protocol.remove_args(id, force)).await.map(|_| ())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let (stdout, _) = self.exec_raw(&self.protocol.list_args(all)).await?;
        self.protocol.parse_list_output(&stdout)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let (stdout, _) = self.exec_raw(&self.protocol.inspect_args(id)).await?;
        self.protocol.parse_inspect_output(&stdout)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let (stdout, stderr) = self.exec_raw(&self.protocol.logs_args(id, tail)).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let (stdout, stderr) = self.exec_raw(&self.protocol.exec_args(id, cmd, env, workdir)).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.exec_raw(&self.protocol.pull_image_args(reference)).await.map(|_| ())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let (stdout, _) = self.exec_raw(&self.protocol.list_images_args()).await?;
        self.protocol.parse_list_images_output(&stdout)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.exec_raw(&self.protocol.remove_image_args(reference, force)).await.map(|_| ())
    }

    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> {
        self.exec_raw(&self.protocol.create_network_args(name, config)).await.map(|_| ())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.exec_raw(&self.protocol.remove_network_args(name)).await.map(|_| ())
    }

    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        self.exec_raw(&self.protocol.create_volume_args(name, config)).await.map(|_| ())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.exec_raw(&self.protocol.remove_volume_args(name)).await.map(|_| ())
    }
}

// ============ Layer 4: Runtime Detection ============

pub async fn detect_backend() -> std::result::Result<CliBackend, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await
            .map_err(|reason| vec![BackendProbeResult {
                name: name.clone(), available: false, reason,
            }]);
    }

    let candidates = platform_candidates();
    let mut results = Vec::new();

    for candidate in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason }),
            Err(_) => results.push(BackendProbeResult {
                name: candidate.to_string(), available: false,
                reason: "probe timed out after 2s".to_string(),
            }),
        }
    }

    Err(results)
}

async fn probe_candidate(name: &str) -> std::result::Result<CliBackend, String> {
    match name {
        "apple/container" => {
            let bin = which("container").map_err(|_| "container binary not found".to_string())?;
            Ok(CliBackend::new(bin, Box::new(AppleContainerProtocol)))
        }
        "podman" => {
            let bin = which("podman").map_err(|_| "podman binary not found".to_string())?;
            if cfg!(target_os = "macos") {
                check_podman_machine_running(&bin).await?;
            }
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "orbstack" => {
            let bin = which("orb").or_else(|_| which("docker"))
                .map_err(|_| "orbstack not found".to_string())?;
            check_orbstack_available(&bin).await?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "colima" => {
            let bin = which("colima").map_err(|_| "colima binary not found".to_string())?;
            check_colima_running(&bin).await?;
            let docker_bin = which("docker").map_err(|_| "docker CLI not found (needed for colima)".to_string())?;
            Ok(CliBackend::new(docker_bin, Box::new(DockerProtocol)))
        }
        "rancher-desktop" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl binary not found".to_string())?;
            check_rancher_available().await?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "lima" => {
            let bin = which("limactl").map_err(|_| "limactl binary not found".to_string())?;
            let instance = check_lima_running_instance(&bin).await?;
            Ok(CliBackend::new(bin, Box::new(LimaProtocol { instance })))
        }
        "nerdctl" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl binary not found".to_string())?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "docker" => {
            let bin = which("docker").map_err(|_| "docker binary not found".to_string())?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        other => Err(format!("unknown backend: {other}")),
    }
}

fn platform_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"]
    } else if cfg!(target_os = "linux") {
        &["podman", "nerdctl", "docker"]
    } else {
        &["podman", "nerdctl", "docker"]
    }
}

async fn check_podman_machine_running(bin: &Path) -> std::result::Result<(), String> {
    let output = Command::new(bin).args(["machine", "list", "--format", "json"]).output().await
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("\"Running\":true") || stdout.contains("\"Running\": true") {
        Ok(())
    } else {
        Err("no running podman machine found".into())
    }
}

async fn check_colima_running(bin: &Path) -> std::result::Result<(), String> {
    let output = Command::new(bin).arg("status").output().await.map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.to_lowercase().contains("running") {
        Ok(())
    } else {
        Err("colima is not running".into())
    }
}

async fn check_orbstack_available(_bin: &Path) -> std::result::Result<(), String> {
    let socket = home::home_dir().map(|h| h.join(".orbstack/run/docker.sock"));
    if let Some(s) = socket {
        if s.exists() { return Ok(()); }
    }
    Err("orbstack socket not found".into())
}

async fn check_rancher_available() -> std::result::Result<(), String> {
    let socket = home::home_dir().map(|h| h.join(".rd/run/containerd-shim.sock"));
    if let Some(s) = socket {
        if s.exists() { return Ok(()); }
    }
    Err("rancher desktop socket not found".into())
}

async fn check_lima_running_instance(bin: &Path) -> std::result::Result<String, String> {
    let output = Command::new(bin).args(["list", "--json"]).output().await.map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if val["status"] == "Running" {
                if let Some(name) = val["name"].as_str() {
                    return Ok(name.to_string());
                }
            }
        }
    }
    Err("no running lima instance found".into())
}
