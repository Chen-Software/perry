use crate::error::{ComposeError, Result};
use crate::types::{
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use std::time::Duration;

// ── Layer 1: Abstract Operations ──────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
    pub internal: bool,
    pub enable_ipv6: bool,
}

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
    async fn image_exists(&self, reference: &str) -> Result<bool>;
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
    async fn wait(&self, id: &str) -> Result<i32>;
    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle>;
}

#[derive(Debug, Clone, Default)]
pub struct SecurityProfile {
    pub read_only_rootfs: bool,
    pub seccomp_profile: Option<String>,
}

// ── Layer 2: CLI Protocol ──────────────────────────────────────────────────

pub trait CliProtocol: Send + Sync {
    fn protocol_name(&self) -> &str;
    fn subcommand_prefix(&self) -> Option<Vec<String>> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        self.docker_run_flags(spec, true)
    }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        self.docker_run_flags(spec, false)
    }

    fn docker_run_flags(&self, spec: &ContainerSpec, detach: bool) -> Vec<String> {
        let mut args = vec!["run".into()];
        if detach { args.push("-d".into()); }
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        if let Some(ports) = &spec.ports {
            for p in ports { args.extend(["-p".into(), p.clone()]); }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols { args.extend(["-v".into(), v.clone()]); }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env { args.extend(["-e".into(), format!("{}={}", k, v)]); }
        }
        if let Some(net) = &spec.network { args.extend(["--network".into(), net.clone()]); }
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
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
        if all { args.push("-a".into()); }
        args
    }
    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".into(), "--format".into(), "{{json .}}".into(), id.into()]
    }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if let Some(t) = tail { args.extend(["--tail".into(), t.to_string()]); }
        args.push(id.into());
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(e) = env {
            for (k, v) in e { args.extend(["-e".into(), format!("{}={}", k, v)]); }
        }
        if let Some(w) = workdir { args.extend(["--workdir".into(), w.into()]); }
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
    fn image_exists_args(&self, reference: &str) -> Vec<String> {
        vec!["images".into(), "-q".into(), reference.into()]
    }
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        for (k, v) in &config.labels { args.extend(["--label".into(), format!("{}={}", k, v)]); }
        if config.internal { args.push("--internal".into()); }
        if config.enable_ipv6 { args.push("--ipv6".into()); }
        args.push(name.into());
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> { vec!["network".into(), "rm".into(), name.into()] }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        for (k, v) in &config.labels { args.extend(["--label".into(), format!("{}={}", k, v)]); }
        args.push(name.into());
        args
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".into(), "rm".into(), name.into()] }
    fn wait_args(&self, id: &str) -> Vec<String> { vec!["wait".into(), id.into()] }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        stdout.lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .map(|val| ContainerInfo {
                id: val["ID"].as_str().unwrap_or_default().to_string(),
                name: val["Names"].as_str().unwrap_or_default().to_string(),
                image: val["Image"].as_str().unwrap_or_default().to_string(),
                status: val["Status"].as_str().unwrap_or_default().to_string(),
                ports: vec![val["Ports"].as_str().unwrap_or_default().to_string()],
                created: val["CreatedAt"].as_str().unwrap_or_default().to_string(),
            })
            .collect()
    }

    fn parse_inspect_output(&self, _id: &str, stdout: &str) -> Option<ContainerInfo> {
        let val: serde_json::Value = serde_json::from_str(stdout).ok()?;
        Some(ContainerInfo {
            id: val["Id"].as_str().unwrap_or_default().to_string(),
            name: val["Name"].as_str().unwrap_or_default().trim_start_matches('/').to_string(),
            image: val["Config"]["Image"].as_str().unwrap_or_default().to_string(),
            status: val["State"]["Status"].as_str().unwrap_or_default().to_string(),
            ports: vec![],
            created: val["Created"].as_str().unwrap_or_default().to_string(),
        })
    }

    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> {
        stdout.lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .map(|val| ImageInfo {
                id: val["ID"].as_str().unwrap_or_default().to_string(),
                repository: val["Repository"].as_str().unwrap_or_default().to_string(),
                tag: val["Tag"].as_str().unwrap_or_default().to_string(),
                size: 0,
                created: val["CreatedAt"].as_str().unwrap_or_default().to_string(),
            })
            .collect()
    }

    fn parse_container_id(&self, stdout: &str) -> String { stdout.trim().to_string() }
}

pub struct DockerProtocol;
impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &str { "docker-compatible" }
}

pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str { "apple/container" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        self.docker_run_flags(spec, false)
    }
}

pub struct LimaProtocol {
    pub instance: String,
}
impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &str { "lima" }
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec!["shell".into(), self.instance.clone(), "nerdctl".into()])
    }
}

// ── Layer 3: CLI Executor ──────────────────────────────────────────────────

pub struct CliBackend<P: CliProtocol> {
    pub bin: PathBuf,
    pub protocol: P,
}

pub type DockerBackend = CliBackend<DockerProtocol>;
pub type AppleBackend = CliBackend<AppleContainerProtocol>;
pub type LimaBackend = CliBackend<LimaProtocol>;

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P) -> Self { Self { bin, protocol } }

    async fn exec_raw(&self, subcommand_args: Vec<String>) -> Result<(String, String)> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() {
            cmd.args(prefix);
        }
        cmd.args(subcommand_args);

        let output = cmd.output().await.map_err(ComposeError::IoError)?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok((stdout, stderr))
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr
            })
        }
    }

    async fn exec_ok(&self, args: Vec<String>) -> Result<String> {
        let (stdout, _) = self.exec_raw(args).await?;
        Ok(stdout)
    }
}

#[async_trait]
impl<P: CliProtocol + Send + Sync> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str {
        self.protocol.protocol_name()
    }

    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("--version");
        let _ = cmd.output().await.map_err(ComposeError::IoError)?;
        Ok(())
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

    async fn start(&self, id: &str) -> Result<()> {
        self.exec_ok(self.protocol.start_args(id)).await?;
        Ok(())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.exec_ok(self.protocol.stop_args(id, timeout)).await?;
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.exec_ok(self.protocol.remove_args(id, force)).await?;
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let stdout = self.exec_ok(self.protocol.list_args(all)).await?;
        Ok(self.protocol.parse_list_output(&stdout))
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let stdout = self.exec_ok(self.protocol.inspect_args(id)).await?;
        self.protocol.parse_inspect_output(id, &stdout)
            .ok_or_else(|| ComposeError::NotFound(id.to_string()))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let (stdout, stderr) = self.exec_raw(self.protocol.logs_args(id, tail)).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let (stdout, stderr) = self.exec_raw(self.protocol.exec_args(id, cmd, env, workdir)).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.exec_ok(self.protocol.pull_image_args(reference)).await?;
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let stdout = self.exec_ok(self.protocol.list_images_args()).await?;
        Ok(self.protocol.parse_list_images_output(&stdout))
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.exec_ok(self.protocol.remove_image_args(reference, force)).await?;
        Ok(())
    }

    async fn image_exists(&self, reference: &str) -> Result<bool> {
        let stdout = self.exec_ok(self.protocol.image_exists_args(reference)).await?;
        Ok(!stdout.trim().is_empty())
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        self.exec_ok(self.protocol.create_network_args(name, config)).await?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.exec_ok(self.protocol.remove_network_args(name)).await?;
        Ok(())
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        self.exec_ok(self.protocol.create_volume_args(name, config)).await?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.exec_ok(self.protocol.remove_volume_args(name)).await?;
        Ok(())
    }

    async fn wait(&self, id: &str) -> Result<i32> {
        let stdout = self.exec_ok(self.protocol.wait_args(id)).await?;
        stdout.trim().parse::<i32>().map_err(|e| ComposeError::BackendError {
            code: -1,
            message: format!("Failed to parse exit code: {}", e)
        })
    }

    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle> {
        let mut args = self.protocol.run_args(spec);
        if profile.read_only_rootfs {
            if let Some(pos) = args.iter().position(|a| a == "run") {
                args.insert(pos + 1, "--read-only".into());
            }
        }
        let stdout = self.exec_ok(args).await?;
        let id = self.protocol.parse_container_id(&stdout);
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }
}

// ── Layer 4: Runtime Detection ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await
            .map_err(|reason| vec![BackendProbeResult { name: name.clone(), available: false, reason }]);
    }

    let candidates = match std::env::consts::OS {
        "macos" | "ios" => vec!["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"],
        _ => vec!["podman", "nerdctl", "docker"],
    };

    let mut results = Vec::new();
    for &name in &candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(name)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => results.push(BackendProbeResult { name: name.to_string(), available: false, reason }),
            Err(_) => results.push(BackendProbeResult { name: name.to_string(), available: false, reason: "timeout".into() }),
        }
    }
    Err(results)
}

async fn probe_candidate(name: &str) -> std::result::Result<Box<dyn ContainerBackend>, String> {
    let which = |n| which::which(n).map_err(|_| format!("{n} not found"));

    match name {
        "apple/container" => {
            let bin = which("container")?;
            Ok(Box::new(CliBackend::new(bin, AppleContainerProtocol)))
        }
        "podman" => {
            let bin = which("podman")?;
            if cfg!(target_os = "macos") {
                let out = Command::new(&bin).args(["machine", "list", "--format", "json"]).output().await.map_err(|e| e.to_string())?;
                let machines: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
                if !machines.as_array().map(|a| a.iter().any(|m| m["Running"] == true)).unwrap_or(false) {
                    return Err("no running podman machine".into());
                }
            }
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "orbstack" => {
            let bin = which("orb").or_else(|_| which("docker"))?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "docker" => {
            let bin = which("docker")?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "nerdctl" => {
            let bin = which("nerdctl")?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "colima" => {
            let _ = which("colima")?;
            let bin = which("docker")?;
            let out = Command::new("colima").arg("status").output().await.map_err(|e| e.to_string())?;
            if !String::from_utf8_lossy(&out.stdout).contains("running") {
                return Err("colima is not running".into());
            }
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "lima" => {
            let bin = which("limactl")?;
            let out = Command::new(&bin).args(["list", "--json"]).output().await.map_err(|e| e.to_string())?;
            let instance = String::from_utf8_lossy(&out.stdout).lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| v["status"] == "Running")
                .and_then(|v| v["name"].as_str().map(|s| s.to_string()))
                .ok_or("no running lima instance")?;
            Ok(Box::new(CliBackend::new(bin, LimaProtocol { instance })))
        }
        _ => Err("unknown backend".into()),
    }
}
