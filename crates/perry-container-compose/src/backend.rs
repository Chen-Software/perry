use crate::error::{ComposeError, Result};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn backend_name(&self) -> &'static str;
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
    async fn wait(&self, id: &str) -> Result<i32>;
    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle>;
}

#[derive(Debug, Clone, Default)]
pub struct SecurityProfile {
    pub read_only_rootfs: bool,
    pub seccomp_profile: Option<String>,
}

#[derive(Debug, Clone)]
pub enum BackendDriver {
    AppleContainer { bin: PathBuf },
    Podman { bin: PathBuf },
    OrbStack { bin: PathBuf },
    Colima { bin: PathBuf },
    RancherDesktop { bin: PathBuf },
    Lima { bin: PathBuf, instance: String },
    Nerdctl { bin: PathBuf },
    Docker { bin: PathBuf },
}

impl BackendDriver {
    pub fn name(&self) -> &'static str {
        match self {
            Self::AppleContainer { .. } => "apple/container",
            Self::Podman { .. } => "podman",
            Self::OrbStack { .. } => "orbstack",
            Self::Colima { .. } => "colima",
            Self::RancherDesktop { .. } => "rancher-desktop",
            Self::Lima { .. } => "lima",
            Self::Nerdctl { .. } => "nerdctl",
            Self::Docker { .. } => "docker",
        }
    }

    pub fn bin(&self) -> &Path {
        match self {
            Self::AppleContainer { bin } | Self::Podman { bin } | Self::OrbStack { bin } |
            Self::Colima { bin } | Self::RancherDesktop { bin } | Self::Lima { bin, .. } |
            Self::Nerdctl { bin } | Self::Docker { bin } => bin,
        }
    }

    pub fn is_docker_compatible(&self) -> bool {
        !matches!(self, Self::AppleContainer { .. })
    }
}

pub struct OciCommandBuilder;

impl OciCommandBuilder {
    pub fn run_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        if driver.is_docker_compatible() {
            Self::docker_run_args(spec)
        } else {
            Self::apple_run_args(spec)
        }
    }

    fn docker_run_args(spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into(), "-d".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        for port in spec.ports.as_ref().unwrap_or(&vec![]) { args.extend(["-p".into(), port.clone()]); }
        for vol in spec.volumes.as_ref().unwrap_or(&vec![]) { args.extend(["-v".into(), vol.clone()]); }
        for (k, v) in spec.env.as_ref().unwrap_or(&HashMap::new()) { args.extend(["-e".into(), format!("{k}={v}")]); }
        if let Some(net) = &spec.network { args.extend(["--network".into(), net.clone()]); }
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }

    fn apple_run_args(spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }
}

pub struct OciBackend {
    pub driver: BackendDriver,
}

impl OciBackend {
    pub fn new(driver: BackendDriver) -> Self { Self { driver } }

    async fn exec_cli(&self, args: &[String]) -> Result<(String, String)> {
        let mut cmd = match &self.driver {
            BackendDriver::Lima { bin, instance } => {
                let mut c = Command::new(bin);
                c.args(["shell", instance, "nerdctl"]);
                c
            }
            _ => Command::new(self.driver.bin()),
        };

        let output = cmd.args(args).output().await.map_err(ComposeError::IoError)?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok((stdout, stderr))
        } else {
            Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: stderr })
        }
    }
}

#[async_trait]
impl ContainerBackend for OciBackend {
    fn backend_name(&self) -> &'static str { self.driver.name() }

    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(self.driver.bin());
        cmd.arg("--version").output().await.map_err(ComposeError::IoError)?;
        Ok(())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::run_args(&self.driver, spec);
        let (stdout, _) = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: stdout.trim().to_string(), name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut args = vec!["create".to_string()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        args.push(spec.image.clone());
        let (stdout, _) = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: stdout.trim().to_string(), name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> { self.exec_cli(&["start".into(), id.into()]).await.map(|_| ()) }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout { args.extend(["--time".into(), t.to_string()]); }
        args.push(id.into());
        self.exec_cli(&args).await.map(|_| ())
    }
    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let mut args = vec!["rm".into()];
        if force { args.push("-f".into()); }
        args.push(id.into());
        self.exec_cli(&args).await.map(|_| ())
    }
    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> { Ok(vec![]) } // Stub
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        Ok(ContainerInfo { id: id.to_string(), name: "".into(), image: "".into(), status: "".into(), ports: vec![], created: "".into() })
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut args = vec!["logs".into()];
        if let Some(t) = tail { args.extend(["--tail".into(), t.to_string()]); }
        args.push(id.into());
        let (stdout, stderr) = self.exec_cli(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> {
        let mut args = vec!["exec".into()];
        if let Some(e) = env { for (k, v) in e { args.extend(["-e".into(), format!("{k}={v}")]); } }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        let (stdout, stderr) = self.exec_cli(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }
    async fn pull_image(&self, reference: &str) -> Result<()> { self.exec_cli(&["pull".into(), reference.into()]).await.map(|_| ()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(vec![]) }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let mut args = vec!["rmi".into()];
        if force { args.push("-f".into()); }
        args.push(reference.into());
        self.exec_cli(&args).await.map(|_| ())
    }
    async fn create_network(&self, name: &str, _config: &ComposeNetwork) -> Result<()> { self.exec_cli(&["network".into(), "create".into(), name.into()]).await.map(|_| ()) }
    async fn remove_network(&self, name: &str) -> Result<()> { self.exec_cli(&["network".into(), "rm".into(), name.into()]).await.map(|_| ()) }
    async fn create_volume(&self, name: &str, _config: &ComposeVolume) -> Result<()> { self.exec_cli(&["volume".into(), "create".into(), name.into()]).await.map(|_| ()) }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec_cli(&["volume".into(), "rm".into(), name.into()]).await.map(|_| ()) }
    async fn wait(&self, id: &str) -> Result<i32> {
        let (stdout, _) = self.exec_cli(&["wait".into(), id.into()]).await?;
        stdout.trim().parse::<i32>().map_err(|e| ComposeError::BackendError { code: -1, message: format!("Failed to parse exit code: {}", e) })
    }
    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle> {
        let mut args = OciCommandBuilder::run_args(&self.driver, spec);
        if profile.read_only_rootfs {
            // Find insertion point for flags (after 'run')
            args.insert(1, "--read-only".into());
        }
        let (stdout, _) = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: stdout.trim().to_string(), name: spec.name.clone() })
    }
}

pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await
            .map(|b| Box::new(b) as Box<dyn ContainerBackend>)
            .map_err(|reason| vec![BackendProbeResult { name, available: false, reason }]);
    }
    let candidates = match std::env::consts::OS {
        "macos" | "ios" => vec!["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"],
        _ => vec!["podman", "nerdctl", "docker"],
    };
    let mut results = Vec::new();
    for &name in &candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(name)).await {
            Ok(Ok(b)) => return Ok(Box::new(b)),
            Ok(Err(reason)) => results.push(BackendProbeResult { name: name.to_string(), available: false, reason }),
            Err(_) => results.push(BackendProbeResult { name: name.to_string(), available: false, reason: "timeout".into() }),
        }
    }
    Err(results)
}

async fn probe_candidate(name: &str) -> std::result::Result<OciBackend, String> {
    let which = |n| which::which(n).map_err(|_| format!("{n} not found"));
    match name {
        "apple/container" => Ok(OciBackend::new(BackendDriver::AppleContainer { bin: which("container")? })),
        "podman" => Ok(OciBackend::new(BackendDriver::Podman { bin: which("podman")? })),
        "orbstack" => Ok(OciBackend::new(BackendDriver::OrbStack { bin: which("orb").or_else(|_| which("docker"))? })),
        "docker" => Ok(OciBackend::new(BackendDriver::Docker { bin: which("docker")? })),
        "nerdctl" => Ok(OciBackend::new(BackendDriver::Nerdctl { bin: which("nerdctl")? })),
        "lima" => {
            let bin = which("limactl")?;
            let out = Command::new(&bin).args(["list", "--json"]).output().await.map_err(|e| e.to_string())?;
            let instance = String::from_utf8_lossy(&out.stdout).lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| v["status"] == "Running")
                .and_then(|v| v["name"].as_str().map(|s| s.to_string()))
                .ok_or("no running lima instance")?;
            Ok(OciBackend::new(BackendDriver::Lima { bin, instance }))
        }
        _ => Err("unknown backend".into()),
    }
}
