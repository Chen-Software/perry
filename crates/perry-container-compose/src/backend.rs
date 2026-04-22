use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, BackendMode};
use which::which;

/// Minimal network creation config — driver and labels only.
/// The compose layer converts ComposeNetwork → NetworkConfig before calling the backend.
#[derive(Debug, Clone, Default)]
pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
    pub internal: bool,
    pub enable_ipv4: bool,
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
    fn mode(&self) -> BackendMode;
    async fn check_available(&self) -> Result<()>;
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;
    async fn start(&self, id: &str) -> Result<()>;
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;
    async fn remove(&self, id: &str, force: bool) -> Result<()>;
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs>;
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs>;
    async fn pull_image(&self, reference: &str) -> Result<()>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
    async fn build(&self, spec: &crate::types::ComposeServiceBuild, image_name: &str) -> Result<()>;
    async fn inspect_network(&self, name: &str) -> Result<()>;
    async fn wait(&self, id: &str) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendDriver {
    AppleContainer,
    Orbstack,
    Colima,
    RancherDesktop,
    Podman,
    Lima,
    Nerdctl,
    Docker,
}

impl BackendDriver {
    pub fn name(&self) -> &'static str {
        match self {
            BackendDriver::AppleContainer => "apple/container",
            BackendDriver::Orbstack => "orbstack",
            BackendDriver::Colima => "colima",
            BackendDriver::RancherDesktop => "rancher-desktop",
            BackendDriver::Podman => "podman",
            BackendDriver::Lima => "lima",
            BackendDriver::Nerdctl => "nerdctl",
            BackendDriver::Docker => "docker",
        }
    }
}

pub struct OciCommandBuilder;

impl OciCommandBuilder {
    pub fn run_args(driver: BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        let include_detach = !matches!(driver, BackendDriver::AppleContainer);
        Self::docker_run_flags(spec, include_detach)
    }

    pub fn create_args(_driver: BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        Self::docker_run_flags(spec, false)
    }

    fn docker_run_flags(spec: &ContainerSpec, include_detach: bool) -> Vec<String> {
        let mut args = vec!["run".to_string()];
        if include_detach { args.push("-d".to_string()); }
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        if let Some(ports) = &spec.ports { for p in ports { args.extend(["-p".into(), p.clone()]); } }
        if let Some(volumes) = &spec.volumes { for v in volumes { args.extend(["-v".into(), v.clone()]); } }
        if let Some(env) = &spec.env { for (k, v) in env { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
        if let Some(network) = &spec.network { args.extend(["--network".into(), network.clone()]); }
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        if spec.read_only.unwrap_or(false) { args.push("--read-only".into()); }
        if let Some(entrypoint) = &spec.entrypoint {
            args.push("--entrypoint".into());
            args.push(entrypoint.join(" "));
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
        args
    }

    pub fn start_args(id: &str) -> Vec<String> { vec!["start".into(), id.into()] }
    pub fn stop_args(id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout { args.extend(["--time".into(), t.to_string()]); }
        args.push(id.into());
        args
    }
    pub fn remove_args(id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".into()];
        if force { args.push("-f".into()); }
        args.push(id.into());
        args
    }
    pub fn list_args(all: bool) -> Vec<String> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all { args.push("-a".into()); }
        args
    }
    pub fn inspect_args(id: &str) -> Vec<String> {
        vec!["inspect".into(), "--format".into(), "json".into(), id.into()]
    }
    pub fn logs_args(id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if let Some(n) = tail { args.extend(["--tail".into(), n.to_string()]); }
        args.push(id.into());
        args
    }
    pub fn exec_args(id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(e) = env { for (k, v) in e { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
        if let Some(w) = workdir { args.extend(["-w".into(), w.into()]); }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        args
    }
    pub fn pull_image_args(reference: &str) -> Vec<String> { vec!["pull".into(), reference.into()] }
    pub fn list_images_args() -> Vec<String> { vec!["images".into(), "--format".into(), "json".into()] }
    pub fn remove_image_args(reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".into()];
        if force { args.push("-f".into()); }
        args.push(reference.into());
        args
    }
    pub fn create_network_args(name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        for (k, v) in &config.labels { args.extend(["--label".into(), format!("{}={}", k, v)]); }
        if config.internal { args.push("--internal".into()); }
        if config.enable_ipv4 { args.push("--ipv4".into()); }
        if config.enable_ipv6 { args.push("--ipv6".into()); }
        args.push(name.into());
        args
    }
    pub fn remove_network_args(name: &str) -> Vec<String> { vec!["network".into(), "rm".into(), name.into()] }
    pub fn create_volume_args(name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        for (k, v) in &config.labels { args.extend(["--label".into(), format!("{}={}", k, v)]); }
        args.push(name.into());
        args
    }
    pub fn remove_volume_args(name: &str) -> Vec<String> { vec!["volume".into(), "rm".into(), name.into()] }
    pub fn build_args(spec: &crate::types::ComposeServiceBuild, image_name: &str) -> Vec<String> {
        let mut args = vec!["build".into()];
        if let Some(context) = &spec.context { args.extend(["--build-arg".into(), format!("CONTEXT={}", context)]); }
        if let Some(dockerfile) = &spec.dockerfile { args.extend(["--file".into(), dockerfile.clone()]); }
        args.extend(["-t".into(), image_name.to_string()]);
        args.push(".".into());
        args
    }
    pub fn inspect_network_args(name: &str) -> Vec<String> {
        vec!["network".into(), "inspect".into(), name.into()]
    }
    pub fn wait_args(id: &str) -> Vec<String> { vec!["wait".into(), id.into()] }
}

pub struct OciBackend {
    pub driver: BackendDriver,
    pub bin: PathBuf,
    pub lima_instance: Option<String>,
    pub mode: BackendMode,
}

impl OciBackend {
    async fn exec_raw(&self, subcommand_args: Vec<String>) -> Result<std::process::Output> {
        let mut cmd = Command::new(&self.bin);
        if let Some(instance) = &self.lima_instance {
            cmd.args(["shell", instance, "nerdctl"]);
        }
        cmd.args(subcommand_args);
        let output = cmd.output().await.map_err(ComposeError::IoError)?;
        Ok(output)
    }

    async fn exec_ok(&self, args: Vec<String>) -> Result<String> {
        let output = self.exec_raw(args).await?;
        if !output.status.success() {
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(1),
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[async_trait]
impl ContainerBackend for OciBackend {
    fn backend_name(&self) -> &str {
        self.driver.name()
    }

    fn mode(&self) -> BackendMode {
        self.mode
    }

    async fn check_available(&self) -> Result<()> {
        let mut args = if let Some(instance) = &self.lima_instance {
            vec!["shell".into(), instance.clone(), "nerdctl".into()]
        } else {
            Vec::new()
        };
        args.push("--version".into());
        let mut cmd = Command::new(&self.bin);
        cmd.args(args);
        let _ = timeout(Duration::from_secs(2), cmd.output()).await
            .map_err(|_| ComposeError::BackendError { code: 125, message: "check_available timed out".into() })?
            .map_err(ComposeError::IoError)?;
        Ok(())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(OciCommandBuilder::run_args(self.driver, spec)).await?;
        let id = stdout.trim().to_string();
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(OciCommandBuilder::create_args(self.driver, spec)).await?;
        let id = stdout.trim().to_string();
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.exec_ok(OciCommandBuilder::start_args(id)).await?;
        Ok(())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.exec_ok(OciCommandBuilder::stop_args(id, timeout)).await?;
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.exec_ok(OciCommandBuilder::remove_args(id, force)).await?;
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let stdout = self.exec_ok(OciCommandBuilder::list_args(all)).await?;
        Ok(serde_json::from_str::<serde_json::Value>(&stdout).ok()
            .and_then(|v| v.as_array().map(|a| a.iter().filter_map(parse_container_info_from_json).collect()))
            .unwrap_or_default())
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let stdout = self.exec_ok(OciCommandBuilder::inspect_args(id)).await?;
        let json: serde_json::Value = serde_json::from_str(&stdout).map_err(ComposeError::JsonError)?;
        let info_json = if json.is_array() { json.as_array().and_then(|a| a.first()) } else { Some(&json) };
        info_json.and_then(parse_container_info_from_json).ok_or_else(|| ComposeError::NotFound(id.into()))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let output = self.exec_raw(OciCommandBuilder::logs_args(id, tail)).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let output = self.exec_raw(OciCommandBuilder::exec_args(id, cmd, env, workdir)).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.exec_ok(OciCommandBuilder::pull_image_args(reference)).await?;
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let stdout = self.exec_ok(OciCommandBuilder::list_images_args()).await?;
        Ok(serde_json::from_str::<serde_json::Value>(&stdout).ok()
            .and_then(|v| v.as_array().map(|a| a.iter().filter_map(parse_image_info_from_json).collect()))
            .unwrap_or_default())
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.exec_ok(OciCommandBuilder::remove_image_args(reference, force)).await?;
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        self.exec_ok(OciCommandBuilder::create_network_args(name, config)).await?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.exec_ok(OciCommandBuilder::remove_network_args(name)).await?;
        Ok(())
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        self.exec_ok(OciCommandBuilder::create_volume_args(name, config)).await?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.exec_ok(OciCommandBuilder::remove_volume_args(name)).await?;
        Ok(())
    }

    async fn build(&self, spec: &crate::types::ComposeServiceBuild, image_name: &str) -> Result<()> {
        self.exec_ok(OciCommandBuilder::build_args(spec, image_name)).await?;
        Ok(())
    }

    async fn inspect_network(&self, name: &str) -> Result<()> {
        self.exec_ok(OciCommandBuilder::inspect_network_args(name)).await?;
        Ok(())
    }

    async fn wait(&self, id: &str) -> Result<()> {
        self.exec_ok(OciCommandBuilder::wait_args(id)).await?;
        Ok(())
    }
}

fn parse_container_info_from_json(json: &serde_json::Value) -> Option<ContainerInfo> {
    let id = json["Id"].as_str().or(json["ID"].as_str())?.to_string();
    let name = json["Names"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str())
        .or(json["Name"].as_str())?
        .trim_start_matches('/')
        .to_string();
    let image = json["Image"].as_str()?.to_string();
    let status = json["Status"].as_str().or_else(|| json["State"].get("Status").and_then(|v| v.as_str())).unwrap_or("").to_string();
    Some(ContainerInfo { id, name, image, status, ports: Vec::new(), created: json["Created"].as_str().unwrap_or("").to_string() })
}

fn parse_image_info_from_json(json: &serde_json::Value) -> Option<ImageInfo> {
    let id = json["Id"].as_str().or(json["ID"].as_str())?.to_string();
    Some(ImageInfo {
        id,
        repository: json["Repository"].as_str().unwrap_or("").to_string(),
        tag: json["Tag"].as_str().unwrap_or("").to_string(),
        size: json["Size"].as_u64().unwrap_or(0),
        created: json["Created"].as_str().unwrap_or("").to_string(),
    })
}

pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return match probe_candidate(&name).await {
            Ok(backend) => Ok(backend),
            Err(reason) => Err(vec![BackendProbeResult { name, available: false, reason }]),
        };
    }

    let candidates = match std::env::consts::OS {
        "macos" | "ios" => vec!["apple/container", "orbstack", "colima", "rancher-desktop", "lima", "podman", "nerdctl", "docker"],
        _ => vec!["podman", "nerdctl", "docker"],
    };

    let mut results = Vec::new();
    for name in candidates {
        match timeout(Duration::from_secs(2), probe_candidate(name)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => results.push(BackendProbeResult { name: name.to_string(), available: false, reason }),
            Err(_) => results.push(BackendProbeResult { name: name.to_string(), available: false, reason: "probe timed out".into() }),
        }
    }
    Err(results)
}

async fn probe_candidate(name: &str) -> std::result::Result<Box<dyn ContainerBackend>, String> {
    let mode = if std::env::var("PERRY_CONTAINER_MODE").unwrap_or_default() == "server-first" {
        BackendMode::Remote
    } else {
        BackendMode::Local
    };
    match name {
        "apple/container" => {
            let bin = which("container").map_err(|_| "container not found".to_string())?;
            Ok(Box::new(OciBackend { driver: BackendDriver::AppleContainer, bin, lima_instance: None, mode }))
        }
        "podman" => {
            let bin = which("podman").map_err(|_| "podman not found".to_string())?;
            if std::env::consts::OS == "macos" {
                let mut cmd = Command::new(&bin);
                cmd.args(["machine", "list", "--format", "json"]);
                let output = cmd.output().await.map_err(|e| e.to_string())?;
                let val: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
                if !val.as_array().map_or(false, |a| a.iter().any(|m| m["Running"].as_bool() == Some(true))) {
                    return Err("no running podman machine".into());
                }
            }
            Ok(Box::new(OciBackend { driver: BackendDriver::Podman, bin, lima_instance: None, mode }))
        }
        "orbstack" => {
            let bin = which("orb").or_else(|_| which("docker")).map_err(|_| "orbstack not found".to_string())?;
            let socket = home::home_dir().map(|h| h.join(".orbstack/run/docker.sock"));
            if !socket.map_or(false, |s| s.exists()) {
                return Err("orbstack socket not found".into());
            }
            Ok(Box::new(OciBackend { driver: BackendDriver::Orbstack, bin, lima_instance: None, mode }))
        }
        "colima" => {
            let bin = which("colima").map_err(|_| "colima not found".to_string())?;
            let mut cmd = Command::new(&bin);
            cmd.arg("status");
            let output = cmd.output().await.map_err(|e| e.to_string())?;
            if !String::from_utf8_lossy(&output.stdout).contains("running") {
                return Err("colima not running".into());
            }
            let docker_bin = which("docker").map_err(|_| "docker CLI not found (needed for colima)".to_string())?;
            Ok(Box::new(OciBackend { driver: BackendDriver::Colima, bin: docker_bin, lima_instance: None, mode }))
        }
        "rancher-desktop" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl not found".to_string())?;
            let socket = home::home_dir().map(|h| h.join(".rd/run/containerd-shim.sock"));
            if !socket.map_or(false, |s| s.exists()) {
                return Err("rancher desktop socket not found".into());
            }
            Ok(Box::new(OciBackend { driver: BackendDriver::RancherDesktop, bin, lima_instance: None, mode }))
        }
        "lima" => {
            let bin = which("limactl").map_err(|_| "limactl not found".to_string())?;
            let mut cmd = Command::new(&bin);
            cmd.args(["list", "--json"]);
            let output = cmd.output().await.map_err(|e| e.to_string())?;
            let mut instance = None;
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                    if val["status"].as_str() == Some("Running") {
                        instance = val["name"].as_str().map(|s| s.to_string());
                        break;
                    }
                }
            }
            let instance = instance.ok_or_else(|| "no running lima instance".to_string())?;
            Ok(Box::new(OciBackend { driver: BackendDriver::Lima, bin, lima_instance: Some(instance), mode }))
        }
        "nerdctl" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl not found".to_string())?;
            Ok(Box::new(OciBackend { driver: BackendDriver::Nerdctl, bin, lima_instance: None, mode }))
        }
        "docker" => {
            let bin = which("docker").map_err(|_| "docker not found".to_string())?;
            Ok(Box::new(OciBackend { driver: BackendDriver::Docker, bin, lima_instance: None, mode }))
        }
        _ => Err("unknown backend".to_string()),
    }
}
