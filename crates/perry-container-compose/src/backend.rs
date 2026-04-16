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
    /// Backend name for display (e.g. "apple/container", "podman")
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

    /// Inspect an image.
    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo>;

    /// Check if an image exists.
    async fn image_exists(&self, reference: &str) -> Result<bool>;

    /// Create network.
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;

    /// Remove network.
    async fn remove_network(&self, name: &str) -> Result<()>;

    /// Create volume.
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;

    /// Remove volume.
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

/// Identifies the detected container runtime and its resolved CLI binary path.
#[derive(Debug, Clone)]
pub enum BackendDriver {
    AppleContainer { bin: PathBuf },
    Podman { bin: PathBuf },
    OrbStack { bin: PathBuf },
    Colima { bin: PathBuf },
    RancherDesktop { bin: PathBuf }, // uses nerdctl
    Lima { bin: PathBuf },           // uses limactl
    Nerdctl { bin: PathBuf },
    Docker { bin: PathBuf },
}

impl BackendDriver {
    /// Returns the human-readable name.
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

    /// Returns the binary path.
    pub fn bin(&self) -> &Path {
        match self {
            Self::AppleContainer { bin }
            | Self::Podman { bin }
            | Self::OrbStack { bin }
            | Self::Colima { bin }
            | Self::RancherDesktop { bin }
            | Self::Lima { bin }
            | Self::Nerdctl { bin }
            | Self::Docker { bin } => bin,
        }
    }

    /// Returns true if this driver accepts Docker-compatible CLI flags.
    pub fn is_docker_compatible(&self) -> bool {
        !matches!(self, Self::AppleContainer { .. } | Self::Lima { .. })
    }
}

pub struct OciBackend {
    driver: BackendDriver,
}

impl OciBackend {
    pub fn new(driver: BackendDriver) -> Self {
        Self { driver }
    }

    async fn exec_cli(&self, args: &[String]) -> Result<(String, String)> {
        let output = Command::new(self.driver.bin())
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
impl ContainerBackend for OciBackend {
    fn backend_name(&self) -> &str {
        self.driver.name()
    }

    async fn check_available(&self) -> Result<()> {
        let _ = self.exec_cli(&["--version".to_string()]).await?;
        Ok(())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::run_args(&self.driver, spec);
        let (stdout, _) = self.exec_cli(&args).await?;
        Ok(ContainerHandle {
            id: stdout.trim().to_string(),
            name: spec.name.clone(),
        })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::create_args(&self.driver, spec);
        let (stdout, _) = self.exec_cli(&args).await?;
        Ok(ContainerHandle {
            id: stdout.trim().to_string(),
            name: spec.name.clone(),
        })
    }

    async fn start(&self, id: &str) -> Result<()> {
        let args = OciCommandBuilder::start_args(&self.driver, id);
        let _ = self.exec_cli(&args).await?;
        Ok(())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let args = OciCommandBuilder::stop_args(&self.driver, id, timeout);
        let _ = self.exec_cli(&args).await?;
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let args = OciCommandBuilder::remove_args(&self.driver, id, force);
        let _ = self.exec_cli(&args).await?;
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let args = OciCommandBuilder::list_args(&self.driver, all);
        let (stdout, _) = self.exec_cli(&args).await?;
        OciCommandBuilder::parse_list_output(&self.driver, &stdout)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let args = OciCommandBuilder::inspect_args(&self.driver, id);
        let (stdout, _) = self.exec_cli(&args).await?;
        OciCommandBuilder::parse_inspect_output(&self.driver, &stdout)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let args = OciCommandBuilder::logs_args(&self.driver, id, tail);
        let (stdout, stderr) = self.exec_cli(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        let args = OciCommandBuilder::exec_args(&self.driver, id, cmd, env, workdir);
        let (stdout, stderr) = self.exec_cli(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let args = OciCommandBuilder::pull_image_args(&self.driver, reference);
        let _ = self.exec_cli(&args).await?;
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let args = OciCommandBuilder::list_images_args(&self.driver);
        let (stdout, _) = self.exec_cli(&args).await?;
        OciCommandBuilder::parse_list_images_output(&self.driver, &stdout)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let args = OciCommandBuilder::remove_image_args(&self.driver, reference, force);
        let _ = self.exec_cli(&args).await?;
        Ok(())
    }

    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo> {
        let args = OciCommandBuilder::inspect_image_args(&self.driver, reference);
        let (stdout, _) = self.exec_cli(&args).await?;
        OciCommandBuilder::parse_inspect_image_output(&self.driver, &stdout)
    }

    async fn image_exists(&self, reference: &str) -> Result<bool> {
        match self.inspect_image(reference).await {
            Ok(_) => Ok(true),
            Err(ComposeError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> {
        let args = OciCommandBuilder::create_network_args(&self.driver, name, config);
        let _ = self.exec_cli(&args).await?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let args = OciCommandBuilder::remove_network_args(&self.driver, name);
        let res = self.exec_cli(&args).await;
        match res {
            Ok(_) => Ok(()),
            Err(ComposeError::BackendError { message, .. })
                if message.contains("not found") || message.contains("No such") =>
            {
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        let args = OciCommandBuilder::create_volume_args(&self.driver, name, config);
        let _ = self.exec_cli(&args).await?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let args = OciCommandBuilder::remove_volume_args(&self.driver, name);
        let res = self.exec_cli(&args).await;
        match res {
            Ok(_) => Ok(()),
            Err(ComposeError::BackendError { message, .. })
                if message.contains("not found") || message.contains("No such") =>
            {
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

pub struct OciCommandBuilder;

impl OciCommandBuilder {
    pub fn run_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        if driver.is_docker_compatible() {
            Self::docker_run_args(spec)
        } else {
            match driver {
                BackendDriver::AppleContainer { .. } => Self::apple_run_args(spec),
                BackendDriver::Lima { .. } => Self::lima_run_args(spec),
                _ => unreachable!(),
            }
        }
    }

    fn docker_run_args(spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string(), "-d".to_string()];
        if let Some(name) = &spec.name {
            args.extend(["--name".to_string(), name.clone()]);
        }
        if let Some(ports) = &spec.ports {
            for p in ports {
                args.extend(["-p".to_string(), p.clone()]);
            }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols {
                args.extend(["-v".to_string(), v.clone()]);
            }
        }
        if let Some(envs) = &spec.env {
            for (k, v) in envs {
                args.extend(["-e".to_string(), format!("{}={}", k, v)]);
            }
        }
        if let Some(net) = &spec.network {
            args.extend(["--network".to_string(), net.clone()]);
        }
        if spec.rm.unwrap_or(false) {
            args.push("--rm".to_string());
        }
        if let Some(ep) = &spec.entrypoint {
            args.extend(["--entrypoint".to_string(), ep.join(" ")]);
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.iter().cloned());
        }
        args
    }

    fn apple_run_args(spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string()];
        if spec.rm.unwrap_or(false) {
            args.push("--rm".to_string());
        }
        if let Some(name) = &spec.name {
            args.extend(["--name".to_string(), name.clone()]);
        }
        if let Some(ports) = &spec.ports {
            for p in ports {
                args.extend(["-p".to_string(), p.clone()]);
            }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols {
                args.extend(["-v".to_string(), v.clone()]);
            }
        }
        if let Some(envs) = &spec.env {
            for (k, v) in envs {
                args.extend(["-e".to_string(), format!("{}={}", k, v)]);
            }
        }
        if let Some(net) = &spec.network {
            args.extend(["--network".to_string(), net.clone()]);
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.iter().cloned());
        }
        args
    }

    fn lima_run_args(spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["shell".to_string(), "default".to_string(), "nerdctl".to_string()];
        args.extend(Self::docker_run_args(spec));
        args
    }

    pub fn create_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        if driver.is_docker_compatible() {
            let mut args = vec!["create".to_string()];
            if let Some(name) = &spec.name {
                args.extend(["--name".to_string(), name.clone()]);
            }
            if let Some(ports) = &spec.ports {
                for p in ports {
                    args.extend(["-p".to_string(), p.clone()]);
                }
            }
            if let Some(vols) = &spec.volumes {
                for v in vols {
                    args.extend(["-v".to_string(), v.clone()]);
                }
            }
            if let Some(envs) = &spec.env {
                for (k, v) in envs {
                    args.extend(["-e".to_string(), format!("{}={}", k, v)]);
                }
            }
            if let Some(net) = &spec.network {
                args.extend(["--network".to_string(), net.clone()]);
            }
            if let Some(ep) = &spec.entrypoint {
                args.extend(["--entrypoint".to_string(), ep.join(" ")]);
            }
            args.push(spec.image.clone());
            if let Some(cmd) = &spec.cmd {
                args.extend(cmd.iter().cloned());
            }
            args
        } else {
            match driver {
                BackendDriver::AppleContainer { .. } => {
                    let mut args = vec!["create".to_string()];
                    if let Some(name) = &spec.name {
                        args.extend(["--name".to_string(), name.clone()]);
                    }
                    if let Some(ports) = &spec.ports {
                        for p in ports {
                            args.extend(["-p".to_string(), p.clone()]);
                        }
                    }
                    if let Some(vols) = &spec.volumes {
                        for v in vols {
                            args.extend(["-v".to_string(), v.clone()]);
                        }
                    }
                    if let Some(envs) = &spec.env {
                        for (k, v) in envs {
                            args.extend(["-e".to_string(), format!("{}={}", k, v)]);
                        }
                    }
                    if let Some(net) = &spec.network {
                        args.extend(["--network".to_string(), net.clone()]);
                    }
                    args.push(spec.image.clone());
                    if let Some(cmd) = &spec.cmd {
                        args.extend(cmd.iter().cloned());
                    }
                    args
                }
                BackendDriver::Lima { .. } => {
                    vec![
                        "shell".to_string(),
                        "default".to_string(),
                        "nerdctl".to_string(),
                        "create".to_string(),
                    ]
                    // Simplified: reuse docker-like flags for nerdctl
                }
                _ => unreachable!(),
            }
        }
    }

    pub fn start_args(_driver: &BackendDriver, id: &str) -> Vec<String> {
        vec!["start".to_string(), id.to_string()]
    }

    pub fn stop_args(_driver: &BackendDriver, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".to_string()];
        if let Some(t) = timeout {
            args.extend(["--time".to_string(), t.to_string()]);
        }
        args.push(id.to_string());
        args
    }

    pub fn remove_args(_driver: &BackendDriver, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".to_string()];
        if force {
            args.push("-f".to_string());
        }
        args.push(id.to_string());
        args
    }

    pub fn list_args(_driver: &BackendDriver, all: bool) -> Vec<String> {
        let mut args = vec!["ps".to_string(), "--format".to_string(), "json".to_string()];
        if all {
            args.push("--all".to_string());
        }
        args
    }

    pub fn inspect_args(_driver: &BackendDriver, id: &str) -> Vec<String> {
        vec![
            "inspect".to_string(),
            "--format".to_string(),
            "json".to_string(),
            id.to_string(),
        ]
    }

    pub fn logs_args(_driver: &BackendDriver, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".to_string()];
        if let Some(t) = tail {
            args.extend(["--tail".to_string(), t.to_string()]);
        }
        args.push(id.to_string());
        args
    }

    pub fn exec_args(
        _driver: &BackendDriver,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String> {
        let mut args = vec!["exec".to_string()];
        if let Some(envs) = env {
            for (k, v) in envs {
                args.extend(["-e".to_string(), format!("{}={}", k, v)]);
            }
        }
        if let Some(wd) = workdir {
            args.extend(["--workdir".to_string(), wd.to_string()]);
        }
        args.push(id.to_string());
        args.extend(cmd.iter().cloned());
        args
    }

    pub fn pull_image_args(_driver: &BackendDriver, reference: &str) -> Vec<String> {
        vec!["pull".to_string(), reference.to_string()]
    }

    pub fn list_images_args(_driver: &BackendDriver) -> Vec<String> {
        vec![
            "images".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ]
    }

    pub fn remove_image_args(_driver: &BackendDriver, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".to_string()];
        if force {
            args.push("-f".to_string());
        }
        args.push(reference.to_string());
        args
    }

    pub fn inspect_image_args(_driver: &BackendDriver, reference: &str) -> Vec<String> {
        vec![
            "inspect".to_string(),
            "--type=image".to_string(),
            "--format".to_string(),
            "json".to_string(),
            reference.to_string(),
        ]
    }

    pub fn create_network_args(_driver: &BackendDriver, name: &str, config: &ComposeNetwork) -> Vec<String> {
        let mut args = vec!["network".to_string(), "create".to_string()];
        if let Some(d) = &config.driver {
            args.extend(["--driver".to_string(), d.clone()]);
        }
        if let Some(lbls) = &config.labels {
            for (k, v) in lbls.to_map() {
                args.extend(["--label".to_string(), format!("{}={}", k, v)]);
            }
        }
        args.push(name.to_string());
        args
    }

    pub fn remove_network_args(_driver: &BackendDriver, name: &str) -> Vec<String> {
        vec!["network".to_string(), "rm".to_string(), name.to_string()]
    }

    pub fn create_volume_args(_driver: &BackendDriver, name: &str, config: &ComposeVolume) -> Vec<String> {
        let mut args = vec!["volume".to_string(), "create".to_string()];
        if let Some(d) = &config.driver {
            args.extend(["--driver".to_string(), d.clone()]);
        }
        if let Some(lbls) = &config.labels {
            for (k, v) in lbls.to_map() {
                args.extend(["--label".to_string(), format!("{}={}", k, v)]);
            }
        }
        args.push(name.to_string());
        args
    }

    pub fn remove_volume_args(_driver: &BackendDriver, name: &str) -> Vec<String> {
        vec!["volume".to_string(), "rm".to_string(), name.to_string()]
    }

    pub fn parse_list_output(driver: &BackendDriver, stdout: &str) -> Result<Vec<ContainerInfo>> {
        if matches!(driver, BackendDriver::AppleContainer { .. }) {
            #[allow(non_snake_case)]
            #[derive(serde::Deserialize)]
            struct ApplePs {
                ID: String,
                Names: Vec<String>,
                Image: String,
                Status: String,
                Ports: Vec<String>,
                Created: String,
            }
            let entries: Vec<ApplePs> = serde_json::from_str(stdout).unwrap_or_default();
            Ok(entries
                .into_iter()
                .map(|e| ContainerInfo {
                    id: e.ID,
                    name: e.Names.into_iter().next().unwrap_or_default(),
                    image: e.Image,
                    status: e.Status,
                    ports: e.Ports,
                    created: e.Created,
                })
                .collect())
        } else {
            #[allow(non_snake_case)]
            #[derive(serde::Deserialize)]
            struct DockerPs {
                ID: String,
                Names: String,
                Image: String,
                Status: String,
                Ports: String,
                CreatedAt: String,
            }
            let entries: Vec<DockerPs> = serde_json::from_str(stdout).unwrap_or_default();
            Ok(entries
                .into_iter()
                .map(|e| ContainerInfo {
                    id: e.ID,
                    name: e.Names,
                    image: e.Image,
                    status: e.Status,
                    ports: vec![e.Ports],
                    created: e.CreatedAt,
                })
                .collect())
        }
    }

    pub fn parse_inspect_output(driver: &BackendDriver, stdout: &str) -> Result<ContainerInfo> {
        if matches!(driver, BackendDriver::AppleContainer { .. }) {
            #[allow(non_snake_case)]
            #[derive(serde::Deserialize)]
            struct AppleInspect {
                State: AppleState,
            }
            #[allow(non_snake_case)]
            #[derive(serde::Deserialize)]
            struct AppleState {
                Running: bool,
            }
            let info: AppleInspect = serde_json::from_str(stdout).map_err(ComposeError::JsonError)?;
            Ok(ContainerInfo {
                id: String::new(),
                name: String::new(),
                image: String::new(),
                status: if info.State.Running {
                    "running".to_string()
                } else {
                    "stopped".to_string()
                },
                ports: vec![],
                created: String::new(),
            })
        } else {
            #[allow(non_snake_case)]
            #[derive(serde::Deserialize)]
            struct DockerInspect {
                Id: String,
                Name: String,
                Config: DockerConfig,
                State: DockerState,
                Created: String,
            }
            #[allow(non_snake_case)]
            #[derive(serde::Deserialize)]
            struct DockerConfig {
                Image: String,
            }
            #[allow(non_snake_case)]
            #[derive(serde::Deserialize)]
            struct DockerState {
                Status: String,
            }
            let entries: Vec<DockerInspect> =
                serde_json::from_str(stdout).map_err(ComposeError::JsonError)?;
            let e = entries.into_iter().next().ok_or_else(|| {
                ComposeError::NotFound("inspect output empty".to_string())
            })?;
            Ok(ContainerInfo {
                id: e.Id,
                name: e.Name.trim_start_matches('/').to_string(),
                image: e.Config.Image,
                status: e.State.Status,
                ports: vec![],
                created: e.Created,
            })
        }
    }

    pub fn parse_inspect_image_output(driver: &BackendDriver, stdout: &str) -> Result<ImageInfo> {
        let images = Self::parse_list_images_output(driver, stdout)?;
        images
            .into_iter()
            .next()
            .ok_or_else(|| ComposeError::NotFound("image not found".to_string()))
    }

    pub fn parse_list_images_output(driver: &BackendDriver, stdout: &str) -> Result<Vec<ImageInfo>> {
        if matches!(driver, BackendDriver::AppleContainer { .. }) {
            #[allow(non_snake_case)]
            #[derive(serde::Deserialize)]
            struct AppleImage {
                ID: String,
                Repository: String,
                Tag: String,
                Size: u64,
                Created: String,
            }
            let entries: Vec<AppleImage> = serde_json::from_str(stdout).unwrap_or_default();
            Ok(entries
                .into_iter()
                .map(|e| ImageInfo {
                    id: e.ID,
                    repository: e.Repository,
                    tag: e.Tag,
                    size: e.Size,
                    created: e.Created,
                })
                .collect())
        } else {
            #[allow(non_snake_case)]
            #[derive(serde::Deserialize)]
            struct DockerImage {
                ID: String,
                Repository: String,
                Tag: String,
                #[allow(dead_code)]
                Size: String,
                CreatedAt: String,
            }
            let entries: Vec<DockerImage> = serde_json::from_str(stdout).unwrap_or_default();
            Ok(entries
                .into_iter()
                .map(|e| ImageInfo {
                    id: e.ID,
                    repository: e.Repository,
                    tag: e.Tag,
                    size: 0,
                    created: e.CreatedAt,
                })
                .collect())
        }
    }
}

pub async fn detect_backend() -> Result<OciBackend> {
    // 1. Check PERRY_CONTAINER_BACKEND override
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await
            .map(OciBackend::new)
            .map_err(|reason| ComposeError::NoBackendFound {
                probed: vec![BackendProbeResult { name, available: false, reason }],
            });
    }

    // 2. Platform-specific candidate list
    let candidates: &[&str] = if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"]
    } else {
        &["podman", "nerdctl", "docker"]
    };

    // 3. Probe each candidate; return first available
    let mut results = Vec::new();
    for &name in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(name)).await {
            Ok(Ok(driver)) => {
                tracing::debug!(backend = name, "backend detected");
                return Ok(OciBackend::new(driver));
            }
            Ok(Err(reason)) => {
                tracing::debug!(backend = name, reason = %reason, "backend probe failed");
                results.push(BackendProbeResult {
                    name: name.to_string(),
                    available: false,
                    reason,
                });
            }
            Err(_) => {
                tracing::debug!(backend = name, "backend probe timed out");
                results.push(BackendProbeResult {
                    name: name.to_string(),
                    available: false,
                    reason: "probe timed out after 2s".to_string(),
                });
            }
        }
    }

    Err(ComposeError::NoBackendFound { probed: results })
}

async fn probe_candidate(name: &str) -> std::result::Result<BackendDriver, String> {
    match name {
        "apple/container" => {
            let bin = which::which("container").map_err(|_| "container binary not found")?;
            run_version_check(&bin).await?;
            Ok(BackendDriver::AppleContainer { bin })
        }
        "podman" => {
            let bin = which::which("podman").map_err(|_| "podman binary not found")?;
            run_version_check(&bin).await?;
            if cfg!(target_os = "macos") {
                check_podman_machine_running(&bin).await?;
            }
            Ok(BackendDriver::Podman { bin })
        }
        "orbstack" => {
            // Check orb binary or docker socket
            let bin = which::which("orb").or_else(|_| which::which("docker"))
                .map_err(|_| "orb or docker binary not found")?;

            let mut available = run_version_check(&bin).await.is_ok();
            if !available {
                let socket = dirs::home_dir().map(|h| h.join(".orbstack/run/docker.sock"));
                if let Some(s) = socket {
                    if s.exists() { available = true; }
                }
            }
            if !available { return Err("orbstack not available".to_string()); }
            Ok(BackendDriver::OrbStack { bin })
        }
        "colima" => {
            let colima_bin = which::which("colima").map_err(|_| "colima binary not found")?;
            let output = Command::new(&colima_bin).arg("status").output().await.map_err(|e| e.to_string())?;
            if !String::from_utf8_lossy(&output.stdout).contains("running") {
                return Err("colima is not running".to_string());
            }
            let bin = which::which("docker").map_err(|_| "docker binary not found (needed for colima)")?;
            Ok(BackendDriver::Colima { bin })
        }
        "rancher-desktop" => {
            let bin = which::which("nerdctl").map_err(|_| "nerdctl binary not found")?;
            let socket = dirs::home_dir().map(|h| h.join(".rd/run/containerd-shim.sock"));
            if let Some(s) = socket {
                if !s.exists() { return Err("rancher-desktop socket not found".to_string()); }
            }
            Ok(BackendDriver::RancherDesktop { bin })
        }
        "lima" => {
            let bin = which::which("limactl").map_err(|_| "limactl binary not found")?;
            let output = Command::new(&bin).arg("list").arg("--json").output().await.map_err(|e| e.to_string())?;
            if !String::from_utf8_lossy(&output.stdout).contains("Running") {
                return Err("no running lima instance".to_string());
            }
            Ok(BackendDriver::Lima { bin })
        }
        "nerdctl" => {
            let bin = which::which("nerdctl").map_err(|_| "nerdctl binary not found")?;
            run_version_check(&bin).await?;
            Ok(BackendDriver::Nerdctl { bin })
        }
        "docker" => {
            let bin = which::which("docker").map_err(|_| "docker binary not found")?;
            run_version_check(&bin).await?;
            Ok(BackendDriver::Docker { bin })
        }
        _ => Err("unknown backend".to_string()),
    }
}

async fn run_version_check(bin: &Path) -> std::result::Result<(), String> {
    let output = Command::new(bin).arg("--version").output().await.map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("version check failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

async fn check_podman_machine_running(bin: &Path) -> std::result::Result<(), String> {
    let output = Command::new(bin).args(["machine", "list", "--format", "json"]).output().await.map_err(|e| e.to_string())?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("\"Running\":true") || stdout.contains("\"Running\": true") {
            Ok(())
        } else {
            Err("no podman machine running".to_string())
        }
    } else {
        Err("failed to list podman machines".to_string())
    }
}
