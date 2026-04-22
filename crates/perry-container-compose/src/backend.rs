use crate::error::{ComposeError, Result};
use crate::types::{
    ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
    ComposeNetwork, ComposeVolume, ComposeServiceBuild,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

/// Minimal network creation config — driver and labels only.
/// The compose layer converts ComposeNetwork → NetworkConfig before calling the backend.
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
    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<()>;
    async fn pull_image(&self, reference: &str) -> Result<()>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
    async fn inspect_network(&self, name: &str) -> Result<()>;
}

pub trait CliProtocol: Send + Sync {
    fn protocol_name(&self) -> &str;
    fn subcommand_prefix(&self) -> Option<Vec<String>> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn start_args(&self, id: &str) -> Vec<String> { vec!["start".to_string(), id.to_string()] }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String>;
    fn remove_args(&self, id: &str, force: bool) -> Vec<String>;
    fn list_args(&self, all: bool) -> Vec<String>;
    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".to_string(), "--format".to_string(), "{{json .}}".to_string(), id.to_string()]
    }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String>;
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String>;
    fn build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String>;
    fn pull_image_args(&self, reference: &str) -> Vec<String> { vec!["pull".to_string(), reference.to_string()] }
    fn list_images_args(&self) -> Vec<String> { vec!["images".to_string(), "--format".to_string(), "json".to_string()] }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String>;
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String>;
    fn remove_network_args(&self, name: &str) -> Vec<String> { vec!["network".to_string(), "rm".to_string(), name.to_string()] }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String>;
    fn remove_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".to_string(), "rm".to_string(), name.to_string()] }
    fn inspect_network_args(&self, name: &str) -> Vec<String> {
        vec!["network".to_string(), "inspect".to_string(), name.to_string()]
    }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo>;
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo>;
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo>;
    fn parse_container_id(&self, stdout: &str) -> String { stdout.trim().to_string() }
}

pub struct DockerProtocol;

impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &str { "docker-compatible" }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string(), "-d".to_string()];
        self.apply_common_args(&mut args, spec);
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".to_string()];
        self.apply_common_args(&mut args, spec);
        args
    }

    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".to_string()];
        if let Some(t) = timeout { args.push("-t".to_string()); args.push(t.to_string()); }
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
        if all { args.push("-a".to_string()); }
        args
    }

    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".to_string()];
        if let Some(n) = tail { args.push("--tail".to_string()); args.push(n.to_string()); }
        args.push(id.to_string());
        args
    }

    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".to_string()];
        if let Some(e) = env {
            for (k, v) in e { args.push("-e".to_string()); args.push(format!("{}={}", k, v)); }
        }
        if let Some(w) = workdir { args.push("-w".to_string()); args.push(w.to_string()); }
        args.push(id.to_string());
        args.extend(cmd.iter().cloned());
        args
    }

    fn build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String> {
        let mut args = vec!["build".to_string(), "-t".to_string(), image_name.to_string()];
        if let Some(context) = &spec.context {
            args.push(context.clone());
        } else {
            args.push(".".to_string());
        }
        if let Some(dockerfile) = &spec.dockerfile {
            args.push("-f".to_string());
            args.push(dockerfile.clone());
        }
        if let Some(args_map) = &spec.args {
            for (k, v) in args_map.to_map() {
                args.push("--build-arg".to_string());
                args.push(format!("{}={}", k, v));
            }
        }
        args
    }

    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".to_string()];
        if force { args.push("-f".to_string()); }
        args.push(reference.to_string());
        args
    }

    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".to_string(), "create".to_string()];
        if let Some(driver) = &config.driver { args.push("--driver".to_string()); args.push(driver.clone()); }
        for (k, v) in &config.labels {
            args.push("--label".to_string());
            args.push(format!("{}={}", k, v));
        }
        if config.internal { args.push("--internal".to_string()); }
        if config.enable_ipv6 { args.push("--ipv6".to_string()); }
        args.push(name.to_string());
        args
    }

    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".to_string(), "create".to_string()];
        if let Some(driver) = &config.driver { args.push("--driver".to_string()); args.push(driver.clone()); }
        for (k, v) in &config.labels {
            args.push("--label".to_string());
            args.push(format!("{}={}", k, v));
        }
        args.push(name.to_string());
        args
    }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        stdout.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
    }

    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        serde_json::from_str(stdout).map_err(Into::into)
    }

    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> {
        stdout.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
    }
}

impl DockerProtocol {
    fn apply_common_args(&self, args: &mut Vec<String>, spec: &ContainerSpec) {
        if let Some(name) = &spec.name { args.push("--name".to_string()); args.push(name.clone()); }
        if let Some(ports) = &spec.ports {
            for p in ports { args.push("-p".to_string()); args.push(p.clone()); }
        }
        if let Some(volumes) = &spec.volumes {
            for v in volumes { args.push("-v".to_string()); args.push(v.clone()); }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env { args.push("-e".to_string()); args.push(format!("{}={}", k, v)); }
        }
        if let Some(network) = &spec.network { args.push("--network".to_string()); args.push(network.clone()); }
        if spec.rm.unwrap_or(false) { args.push("--rm".to_string()); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
    }
}

pub struct AppleContainerProtocol;

impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str { "apple/container" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string()];
        DockerProtocol.apply_common_args(&mut args, spec);
        args
    }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> { DockerProtocol.create_args(spec) }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> { DockerProtocol.stop_args(id, timeout) }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> { DockerProtocol.remove_args(id, force) }
    fn list_args(&self, all: bool) -> Vec<String> { DockerProtocol.list_args(all) }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> { DockerProtocol.logs_args(id, tail) }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        DockerProtocol.exec_args(id, cmd, env, workdir)
    }
    fn build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String> { DockerProtocol.build_args(spec, image_name) }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> { DockerProtocol.remove_image_args(reference, force) }
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> { DockerProtocol.create_network_args(name, config) }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> { DockerProtocol.create_volume_args(name, config) }
    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> { DockerProtocol.parse_list_output(stdout) }
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> { DockerProtocol.parse_inspect_output(stdout) }
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> { DockerProtocol.parse_list_images_output(stdout) }
}

pub struct LimaProtocol {
    pub instance: String,
}

impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &str { "lima" }
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec!["shell".to_string(), self.instance.clone(), "nerdctl".to_string()])
    }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> { DockerProtocol.run_args(spec) }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> { DockerProtocol.create_args(spec) }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> { DockerProtocol.stop_args(id, timeout) }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> { DockerProtocol.remove_args(id, force) }
    fn list_args(&self, all: bool) -> Vec<String> { DockerProtocol.list_args(all) }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> { DockerProtocol.logs_args(id, tail) }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        DockerProtocol.exec_args(id, cmd, env, workdir)
    }
    fn build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String> { DockerProtocol.build_args(spec, image_name) }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> { DockerProtocol.remove_image_args(reference, force) }
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> { DockerProtocol.create_network_args(name, config) }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> { DockerProtocol.create_volume_args(name, config) }
    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> { DockerProtocol.parse_list_output(stdout) }
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> { DockerProtocol.parse_inspect_output(stdout) }
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
        cmd.output().await.map_err(Into::into)
    }

    async fn exec_ok(&self, args: Vec<String>) -> Result<String> {
        let out = self.exec_raw(args).await?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).to_string())
        } else {
            Err(ComposeError::BackendError {
                code: out.status.code().unwrap_or(1),
                message: String::from_utf8_lossy(&out.stderr).to_string(),
            })
        }
    }
}

#[async_trait]
impl<P: CliProtocol + Send + Sync> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str { self.bin.file_name().and_then(|n| n.to_str()).unwrap_or("unknown") }

    async fn check_available(&self) -> Result<()> {
        self.exec_raw(vec!["--version".to_string()]).await.map(|_| ())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(self.protocol.run_args(spec)).await?;
        Ok(ContainerHandle { id: self.protocol.parse_container_id(&stdout), name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(self.protocol.create_args(spec)).await?;
        Ok(ContainerHandle { id: self.protocol.parse_container_id(&stdout), name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> { self.exec_ok(self.protocol.start_args(id)).await.map(|_| ()) }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> { self.exec_ok(self.protocol.stop_args(id, timeout)).await.map(|_| ()) }
    async fn remove(&self, id: &str, force: bool) -> Result<()> { self.exec_ok(self.protocol.remove_args(id, force)).await.map(|_| ()) }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let stdout = self.exec_ok(self.protocol.list_args(all)).await?;
        Ok(self.protocol.parse_list_output(&stdout))
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let stdout = self.exec_ok(self.protocol.inspect_args(id)).await?;
        self.protocol.parse_inspect_output(&stdout)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let out = self.exec_raw(self.protocol.logs_args(id, tail)).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&out.stdout).to_string(), stderr: String::from_utf8_lossy(&out.stderr).to_string() })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let out = self.exec_raw(self.protocol.exec_args(id, cmd, env, workdir)).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&out.stdout).to_string(), stderr: String::from_utf8_lossy(&out.stderr).to_string() })
    }

    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<()> {
        self.exec_ok(self.protocol.build_args(spec, image_name)).await.map(|_| ())
    }

    async fn pull_image(&self, reference: &str) -> Result<()> { self.exec_ok(self.protocol.pull_image_args(reference)).await.map(|_| ()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let stdout = self.exec_ok(self.protocol.list_images_args()).await?;
        Ok(self.protocol.parse_list_images_output(&stdout))
    }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> { self.exec_ok(self.protocol.remove_image_args(reference, force)).await.map(|_| ()) }
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> { self.exec_ok(self.protocol.create_network_args(name, config)).await.map(|_| ()) }
    async fn remove_network(&self, name: &str) -> Result<()> { self.exec_ok(self.protocol.remove_network_args(name)).await.map(|_| ()) }
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> { self.exec_ok(self.protocol.create_volume_args(name, config)).await.map(|_| ()) }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec_ok(self.protocol.remove_volume_args(name)).await.map(|_| ()) }
    async fn inspect_network(&self, name: &str) -> Result<()> {
        self.exec_ok(self.protocol.inspect_network_args(name)).await.map(|_| ())
    }
}

fn platform_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        &["apple/container", "orbstack", "colima", "rancher-desktop", "lima", "podman", "nerdctl", "docker"]
    } else if cfg!(target_os = "linux") {
        &["podman", "nerdctl", "docker"]
    } else {
        &["podman", "nerdctl", "docker"]
    }
}

pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend>, Vec<BackendProbeResult>> {
    let mut results = Vec::new();
    for &name in platform_candidates() {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(name)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => results.push(BackendProbeResult { name: name.to_string(), available: false, reason }),
            Err(_) => results.push(BackendProbeResult { name: name.to_string(), available: false, reason: "probe timed out".to_string() }),
        }
    }
    Err(results)
}

async fn probe_candidate(name: &str) -> std::result::Result<Box<dyn ContainerBackend>, String> {
    match name {
        "apple/container" => {
            let bin = which::which("container").map_err(|_| "container not found")?;
            Ok(Box::new(CliBackend::new(bin, AppleContainerProtocol)))
        }
        "podman" => {
            let bin = which::which("podman").map_err(|_| "podman not found")?;
            if cfg!(target_os = "macos") {
                let out = Command::new(&bin).args(&["machine", "list", "--format", "json"]).output().await.map_err(|_| "podman machine list failed")?;
                let json: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| "invalid podman output")?;
                if !json.as_array().map(|a| a.iter().any(|m| m["Running"].as_bool().unwrap_or(false))).unwrap_or(false) {
                    return Err("no podman machine running".into());
                }
            }
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "orbstack" => {
            let bin = which::which("orb").or_else(|_| which::which("docker")).map_err(|_| "orbstack not found")?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "colima" => {
            let bin = which::which("colima").map_err(|_| "colima not found")?;
            let out = Command::new(&bin).arg("status").output().await.map_err(|_| "colima status failed")?;
            if !String::from_utf8_lossy(&out.stdout).contains("running") {
                return Err("colima not running".into());
            }
            let dbin = which::which("docker").map_err(|_| "docker cli not found for colima")?;
            Ok(Box::new(CliBackend::new(dbin, DockerProtocol)))
        }
        "rancher-desktop" => {
            let bin = which::which("nerdctl").map_err(|_| "nerdctl (Rancher Desktop) not found")?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "lima" => {
            let bin = which::which("limactl").map_err(|_| "limactl not found")?;
            let out = Command::new(&bin).args(&["list", "--json"]).output().await.map_err(|_| "limactl list failed")?;
            let instance = String::from_utf8_lossy(&out.stdout).lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| v["status"] == "Running")
                .and_then(|v| v["name"].as_str().map(|s| s.to_string()))
                .ok_or("no running lima instance")?;
            Ok(Box::new(CliBackend::new(bin, LimaProtocol { instance })))
        }
        "nerdctl" => {
            let bin = which::which("nerdctl").map_err(|_| "nerdctl not found")?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "docker" => {
            let bin = which::which("docker").map_err(|_| "docker not found")?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        _ => Err("unknown backend".to_string()),
    }
}
