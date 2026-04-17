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

/// Identifies the detected container runtime and its resolved CLI binary path.
#[derive(Debug, Clone)]
pub enum BackendDriver {
    AppleContainer { bin: PathBuf },
    Podman { bin: PathBuf },
    OrbStack { bin: PathBuf },
    Colima { bin: PathBuf },
    RancherDesktop { bin: PathBuf }, // uses nerdctl
    Lima { bin: PathBuf, instance: String }, // uses limactl
    Nerdctl { bin: PathBuf },
    Docker { bin: PathBuf },
}

impl BackendDriver {
    /// Returns the human-readable name used in getBackend() and PERRY_CONTAINER_BACKEND.
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

    /// Returns the binary path for this driver.
    pub fn bin(&self) -> &Path {
        match self {
            Self::AppleContainer { bin } | Self::Podman { bin } | Self::OrbStack { bin } |
            Self::Colima { bin } | Self::RancherDesktop { bin } | Self::Lima { bin, .. } |
            Self::Nerdctl { bin } | Self::Docker { bin } => bin,
        }
    }

    /// Returns true if this driver accepts Docker-compatible CLI flags.
    pub fn is_docker_compatible(&self) -> bool {
        !matches!(self, Self::AppleContainer { .. } | Self::Lima { .. })
    }
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
    async fn network_exists(&self, name: &str) -> Result<bool>;
    async fn volume_exists(&self, name: &str) -> Result<bool>;
}

pub struct OciCommandBuilder;

impl OciCommandBuilder {
    pub fn run_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }

        if driver.is_docker_compatible() || matches!(driver, BackendDriver::Lima { .. }) {
            args.push("run".into());
            if !matches!(driver, BackendDriver::AppleContainer { .. }) {
                args.push("--detach".into());
            }
            if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
            for port in spec.ports.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-p".into(), port.clone()]); }
            for vol in spec.volumes.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-v".into(), vol.clone()]); }
            for (k, v) in spec.env.as_ref().iter().flat_map(|m| m.iter()) { args.extend(["-e".into(), format!("{k}={v}")]); }
            if let Some(net) = &spec.network { args.extend(["--network".into(), net.clone()]); }
            if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
            if spec.read_only.unwrap_or(false) { args.push("--read-only".into()); }
            if let Some(ep) = &spec.entrypoint {
                args.push("--entrypoint".into());
                args.push(ep.join(" "));
            }
            args.push(spec.image.clone());
            for c in spec.cmd.as_ref().iter().flat_map(|v| v.iter()) { args.push(c.clone()); }
        } else {
            // Apple Container
            args.push("run".into());
            if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
            if spec.read_only.unwrap_or(false) { args.push("--read-only".into()); }
            if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
            if let Some(network) = &spec.network { args.extend(["--network".into(), network.clone()]); }
            for port in spec.ports.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-p".into(), port.clone()]); }
            for vol in spec.volumes.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-v".into(), vol.clone()]); }
            for (k, v) in spec.env.as_ref().iter().flat_map(|m| m.iter()) { args.extend(["-e".into(), format!("{k}={v}")]); }
            args.push(spec.image.clone());
            for c in spec.cmd.as_ref().iter().flat_map(|v| v.iter()) { args.push(c.clone()); }
        }
        args
    }

    pub fn create_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.push("create".into());
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        for port in spec.ports.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-p".into(), port.clone()]); }
        for vol in spec.volumes.as_ref().iter().flat_map(|v| v.iter()) { args.extend(["-v".into(), vol.clone()]); }
        for (k, v) in spec.env.as_ref().iter().flat_map(|m| m.iter()) { args.extend(["-e".into(), format!("{k}={v}")]); }
        if let Some(net) = &spec.network { args.extend(["--network".into(), net.clone()]); }
        if spec.read_only.unwrap_or(false) { args.push("--read-only".into()); }
        if let Some(ep) = &spec.entrypoint {
            args.push("--entrypoint".into());
            args.push(ep.join(" "));
        }
        args.push(spec.image.clone());
        for c in spec.cmd.as_ref().iter().flat_map(|v| v.iter()) { args.push(c.clone()); }
        args
    }

    pub fn start_args(driver: &BackendDriver, id: &str) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.extend(vec!["start".into(), id.into()]);
        args
    }

    pub fn stop_args(driver: &BackendDriver, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.push("stop".into());
        if let Some(t) = timeout { args.extend(["--time".into(), t.to_string()]); }
        args.push(id.into());
        args
    }

    pub fn remove_args(driver: &BackendDriver, id: &str, force: bool) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.push("rm".into());
        if force { args.push("-f".into()); }
        args.push(id.into());
        args
    }

    pub fn list_args(driver: &BackendDriver, all: bool) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.extend(vec!["ps".into(), "--format".into(), "json".into()]);
        if all { args.push("--all".into()); }
        args
    }

    pub fn inspect_args(driver: &BackendDriver, id: &str) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.extend(vec!["inspect".into(), "--format".into(), "json".into(), id.into()]);
        args
    }

    pub fn logs_args(driver: &BackendDriver, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.push("logs".into());
        if let Some(t) = tail { args.extend(["--tail".into(), t.to_string()]); }
        args.push(id.into());
        args
    }

    pub fn exec_args(driver: &BackendDriver, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.push("exec".into());
        if let Some(w) = workdir { args.extend(["--workdir".into(), w.into()]); }
        if let Some(e) = env {
            for (k, v) in e { args.extend(["-e".into(), format!("{k}={v}")]); }
        }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        args
    }

    pub fn pull_image_args(driver: &BackendDriver, reference: &str) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.extend(vec!["pull".into(), reference.into()]);
        args
    }

    pub fn list_images_args(driver: &BackendDriver) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.extend(vec!["images".into(), "--format".into(), "json".into()]);
        args
    }

    pub fn remove_image_args(driver: &BackendDriver, reference: &str, force: bool) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.push("rmi".into());
        if force { args.push("-f".into()); }
        args.push(reference.into());
        args
    }

    pub fn create_network_args(driver: &BackendDriver, name: &str, config: &ComposeNetwork) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.extend(vec!["network".into(), "create".into()]);
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        if let Some(lbls) = &config.labels {
            for (k, v) in lbls.to_map() {
                args.extend(["--label".into(), format!("{k}={v}")]);
            }
        }
        args.push(name.into());
        args
    }

    pub fn remove_network_args(driver: &BackendDriver, name: &str) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.extend(vec!["network".into(), "rm".into(), name.into()]);
        args
    }

    pub fn create_volume_args(driver: &BackendDriver, name: &str, config: &ComposeVolume) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.extend(vec!["volume".into(), "create".into()]);
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        if let Some(lbls) = &config.labels {
            for (k, v) in lbls.to_map() {
                args.extend(["--label".into(), format!("{k}={v}")]);
            }
        }
        args.push(name.into());
        args
    }

    pub fn remove_volume_args(driver: &BackendDriver, name: &str) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.extend(vec!["volume".into(), "rm".into(), name.into()]);
        args
    }

    pub fn network_exists_args(driver: &BackendDriver, name: &str) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.extend(vec!["network".into(), "inspect".into(), name.into()]);
        args
    }

    pub fn volume_exists_args(driver: &BackendDriver, name: &str) -> Vec<String> {
        let mut args = Vec::new();
        if let BackendDriver::Lima { instance, .. } = driver {
            args.extend(vec!["shell".into(), instance.clone(), "nerdctl".into()]);
        }
        args.extend(vec!["volume".into(), "inspect".into(), name.into()]);
        args
    }
}

pub struct OciBackend {
    pub driver: BackendDriver,
}

impl OciBackend {
    pub fn new(driver: BackendDriver) -> Self {
        Self { driver }
    }

    async fn exec_raw(&self, args: &[String]) -> Result<(String, String)> {
        let output = Command::new(self.driver.bin())
            .args(args)
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
impl ContainerBackend for OciBackend {
    fn backend_name(&self) -> &'static str {
        self.driver.name()
    }

    async fn check_available(&self) -> Result<()> {
        Command::new(self.driver.bin())
            .arg("--version")
            .output()
            .await
            .map_err(ComposeError::IoError)
            .map(|_| ())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::run_args(&self.driver, spec);
        let (stdout, _) = self.exec_raw(&args).await?;
        Ok(ContainerHandle { id: stdout.trim().to_string(), name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::create_args(&self.driver, spec);
        let (stdout, _) = self.exec_raw(&args).await?;
        Ok(ContainerHandle { id: stdout.trim().to_string(), name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> {
        let args = OciCommandBuilder::start_args(&self.driver, id);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let args = OciCommandBuilder::stop_args(&self.driver, id, timeout);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let args = OciCommandBuilder::remove_args(&self.driver, id, force);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let args = OciCommandBuilder::list_args(&self.driver, all);
        let (stdout, _) = self.exec_raw(&args).await?;
        let entries: Vec<DockerListEntry> = stdout.lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        Ok(entries.into_iter().map(|e| ContainerInfo {
            id: e.id,
            name: e.names.first().cloned().unwrap_or_default(),
            image: e.image,
            status: e.status,
            ports: e.ports,
            created: e.created,
        }).collect())
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let args = OciCommandBuilder::inspect_args(&self.driver, id);
        let (stdout, _) = self.exec_raw(&args).await?;
        let entries: Vec<DockerInspectOutput> = serde_json::from_str(&stdout)?;
        let e = entries.into_iter().next().ok_or_else(|| ComposeError::NotFound("Inspect output empty".into()))?;
        Ok(ContainerInfo {
            id: e.id,
            name: e.name,
            image: e.config.image,
            status: e.state.status,
            ports: vec![],
            created: e.created,
        })
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let args = OciCommandBuilder::logs_args(&self.driver, id, tail);
        let (stdout, stderr) = self.exec_raw(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let args = OciCommandBuilder::exec_args(&self.driver, id, cmd, env, workdir);
        let (stdout, stderr) = self.exec_raw(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let args = OciCommandBuilder::pull_image_args(&self.driver, reference);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let args = OciCommandBuilder::list_images_args(&self.driver);
        let (stdout, _) = self.exec_raw(&args).await?;
        let entries: Vec<DockerImageEntry> = stdout.lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        Ok(entries.into_iter().map(|e| ImageInfo {
            id: e.id,
            repository: e.repository,
            tag: e.tag,
            size: e.size,
            created: e.created,
        }).collect())
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let args = OciCommandBuilder::remove_image_args(&self.driver, reference, force);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> {
        let args = OciCommandBuilder::create_network_args(&self.driver, name, config);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let args = OciCommandBuilder::remove_network_args(&self.driver, name);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        let args = OciCommandBuilder::create_volume_args(&self.driver, name, config);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let args = OciCommandBuilder::remove_volume_args(&self.driver, name);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn network_exists(&self, name: &str) -> Result<bool> {
        let args = OciCommandBuilder::network_exists_args(&self.driver, name);
        match self.exec_raw(&args).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn volume_exists(&self, name: &str) -> Result<bool> {
        let args = OciCommandBuilder::volume_exists_args(&self.driver, name);
        match self.exec_raw(&args).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[derive(Debug, Deserialize)]
struct DockerListEntry {
    #[serde(rename = "ID", alias = "Id", default)]
    id: String,
    #[serde(rename = "Names", default)]
    names: Vec<String>,
    #[serde(rename = "Image", default)]
    image: String,
    #[serde(rename = "Status", alias = "State", default)]
    status: String,
    #[serde(rename = "Ports", default)]
    ports: Vec<String>,
    #[serde(rename = "Created", alias = "CreatedAt", default)]
    created: String,
}

#[derive(Debug, Deserialize)]
struct DockerInspectOutput {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Config")]
    config: DockerInspectConfig,
    #[serde(rename = "State")]
    state: DockerInspectState,
    #[serde(rename = "Created")]
    created: String,
}

#[derive(Debug, Deserialize)]
struct DockerInspectConfig {
    #[serde(rename = "Image")]
    image: String,
}

#[derive(Debug, Deserialize)]
struct DockerInspectState {
    #[serde(rename = "Status")]
    status: String,
}

#[derive(Debug, Deserialize)]
struct DockerImageEntry {
    #[serde(rename = "ID", alias = "Id", default)]
    id: String,
    #[serde(rename = "Repositories", alias = "Repository", default)]
    repository: String,
    #[serde(rename = "Tag", default)]
    tag: String,
    #[serde(rename = "Size", default)]
    size: u64,
    #[serde(rename = "Created", alias = "CreatedAt", default)]
    created: String,
}

pub async fn detect_backend() -> std::result::Result<OciBackend, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await
            .map_err(|reason| vec![BackendProbeResult { name: name.clone(), available: false, reason }]);
    }

    let candidates = platform_candidates();
    let mut results = Vec::new();

    for candidate in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason }),
            Err(_) => results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason: "probe timed out".into() }),
        }
    }

    Err(results)
}

fn platform_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"]
    } else {
        &["podman", "nerdctl", "docker"]
    }
}

async fn probe_candidate(name: &str) -> std::result::Result<OciBackend, String> {
    let which_bin = |name: &str| -> std::result::Result<PathBuf, String> {
        which::which(name).map_err(|_| format!("{} not found", name))
    };

    match name {
        "apple/container" => {
            let bin = which_bin("container")?;
            Ok(OciBackend::new(BackendDriver::AppleContainer { bin }))
        }
        "podman" => {
            let bin = which_bin("podman")?;
            if cfg!(target_os = "macos") {
                let out = Command::new(&bin).args(&["machine", "list", "--format", "json"]).output().await.map_err(|_| "podman machine list failed")?;
                let json: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| "invalid podman output")?;
                if !json.as_array().map(|a| a.iter().any(|m| m["Running"].as_bool().unwrap_or(false))).unwrap_or(false) {
                    return Err("no podman machine running".into());
                }
            }
            Ok(OciBackend::new(BackendDriver::Podman { bin }))
        }
        "orbstack" => {
            let bin = which_bin("orb").or_else(|_| which_bin("docker")).map_err(|_| "orbstack not found")?;
            // design Req 1.3: OR socket ~/.orbstack/run/docker.sock exists & connectable
            let socket_path = shellexpand::tilde("~/.orbstack/run/docker.sock").to_string();
            if !bin.exists() && !Path::new(&socket_path).exists() {
                return Err("orbstack binary and socket not found".into());
            }
            Ok(OciBackend::new(BackendDriver::OrbStack { bin }))
        }
        "colima" => {
            let bin = which_bin("colima")?;
            let out = Command::new(&bin).arg("status").output().await.map_err(|_| "colima status failed")?;
            if !String::from_utf8_lossy(&out.stdout).contains("running") {
                return Err("colima not running".into());
            }
            let dbin = which_bin("docker").map_err(|_| "docker cli not found for colima")?;
            Ok(OciBackend::new(BackendDriver::Colima { bin: dbin }))
        }
        "rancher-desktop" => {
            let bin = which_bin("nerdctl").map_err(|_| "nerdctl not found for rancher-desktop")?;
            let socket_path = shellexpand::tilde("~/.rd/run/containerd-shim.sock").to_string();
            if !Path::new(&socket_path).exists() {
                return Err("rancher-desktop socket not found".into());
            }
            Ok(OciBackend::new(BackendDriver::RancherDesktop { bin }))
        }
        "lima" => {
            let bin = which_bin("limactl")?;
            let out = Command::new(&bin).args(&["list", "--json"]).output().await.map_err(|_| "limactl list failed")?;
            let instance = String::from_utf8_lossy(&out.stdout).lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| v["status"] == "Running")
                .and_then(|v| v["name"].as_str().map(|s| s.to_string()))
                .ok_or("no running lima instance")?;
            Ok(OciBackend::new(BackendDriver::Lima { bin, instance }))
        }
        "nerdctl" => {
            let bin = which_bin("nerdctl")?;
            Ok(OciBackend::new(BackendDriver::Nerdctl { bin }))
        }
        "docker" => {
            let bin = which_bin("docker")?;
            Ok(OciBackend::new(BackendDriver::Docker { bin }))
        }
        _ => Err("unknown backend".into()),
    }
}
