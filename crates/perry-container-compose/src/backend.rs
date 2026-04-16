//! Container backend system.
//!
//! Structured as five layers:
//! 1. `ContainerBackend` trait: Abstract operations (runtime-agnostic)
//! 2. `Helper Config Types`: NetworkConfig, VolumeConfig, SecurityProfile
//! 3. `BackendDriver` enum: Identified runtime CLI path and type
//! 4. `OciCommandBuilder`: Translates operations into CLI arguments
//! 5. `OciBackend` struct: Implementation of ContainerBackend using BackendDriver
//! 6. `detect_backend()`: Runtime detection and auto-configuration

use crate::error::{BackendProbeResult, ComposeError, Result};
use crate::types::{
    ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use which::which;

// ============ Helper Config Types ============

#[derive(Debug, Clone, Default)]
pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: Option<HashMap<String, String>>,
    pub internal: bool,
    pub enable_ipv6: bool,
}

#[derive(Debug, Clone, Default)]
pub struct VolumeConfig {
    pub driver: Option<String>,
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Default)]
pub struct SecurityProfile {
    pub read_only_rootfs: bool,
    pub seccomp_profile: Option<String>,
    pub no_new_privileges: bool,
}

// ============ Layer 1: Abstract Operations ============

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn backend_name(&self) -> &'static str;
    async fn check_available(&self) -> Result<()>;
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;
    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle>;
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;
    async fn start(&self, id: &str) -> Result<()>;
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;
    async fn remove(&self, id: &str, force: bool) -> Result<()>;
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs>;
    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs>;
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

// ============ Layer 2: Backend Driver ============

#[derive(Debug, Clone)]
pub enum BackendDriver {
    AppleContainer { bin: PathBuf },
    Podman         { bin: PathBuf },
    OrbStack       { bin: PathBuf },
    Colima         { bin: PathBuf },
    RancherDesktop { bin: PathBuf },
    Lima           { bin: PathBuf, instance: String },
    Nerdctl        { bin: PathBuf },
    Docker         { bin: PathBuf },
}

impl BackendDriver {
    pub fn name(&self) -> &'static str {
        match self {
            Self::AppleContainer { .. } => "apple/container",
            Self::Podman { .. }         => "podman",
            Self::OrbStack { .. }       => "orbstack",
            Self::Colima { .. }         => "colima",
            Self::RancherDesktop { .. } => "rancher-desktop",
            Self::Lima { .. }           => "lima",
            Self::Nerdctl { .. }        => "nerdctl",
            Self::Docker { .. }         => "docker",
        }
    }

    pub fn bin(&self) -> &Path {
        match self {
            Self::AppleContainer { bin } => bin,
            Self::Podman { bin }         => bin,
            Self::OrbStack { bin }       => bin,
            Self::Colima { bin }         => bin,
            Self::RancherDesktop { bin } => bin,
            Self::Lima { bin, .. }       => bin,
            Self::Nerdctl { bin }        => bin,
            Self::Docker { bin }         => bin,
        }
    }

    pub fn is_docker_compatible(&self) -> bool {
        !matches!(self, Self::AppleContainer { .. } | Self::Lima { .. })
    }
}

// ============ Layer 3: Command Builder ============

pub struct OciCommandBuilder;

impl OciCommandBuilder {
    pub fn run_args(driver: &BackendDriver, spec: &ContainerSpec, profile: Option<&SecurityProfile>) -> Vec<String> {
        let mut args = match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "run".into()],
            _ => vec!["run".into()],
        };

        if !matches!(driver, BackendDriver::AppleContainer { .. }) {
            args.push("--detach".into());
        }

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
            if driver.is_docker_compatible() {
                args.extend(["--entrypoint".into(), ep.join(" ")]);
            }
        }

        if let Some(p) = profile {
            if p.read_only_rootfs {
                args.push("--read-only".into());
            }
            if let Some(seccomp) = &p.seccomp_profile {
                args.extend(["--security-opt".into(), format!("seccomp={}", seccomp)]);
            }
            if p.no_new_privileges {
                if !matches!(driver, BackendDriver::AppleContainer { .. }) {
                    args.extend(["--security-opt".into(), "no-new-privileges".into()]);
                }
            }
        }

        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }

        args
    }

    pub fn create_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        let mut args = match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "create".into()],
            _ => vec!["create".into()],
        };

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

    pub fn start_args(driver: &BackendDriver, id: &str) -> Vec<String> {
        match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "start".into(), id.into()],
            _ => vec!["start".into(), id.into()],
        }
    }

    pub fn stop_args(driver: &BackendDriver, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "stop".into()],
            _ => vec!["stop".into()],
        };
        if let Some(t) = timeout {
            args.extend(["--time".into(), t.to_string()]);
        }
        args.push(id.into());
        args
    }

    pub fn remove_args(driver: &BackendDriver, id: &str, force: bool) -> Vec<String> {
        let mut args = match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "rm".into()],
            _ => vec!["rm".into()],
        };
        if force { args.push("-f".into()); }
        args.push(id.into());
        args
    }

    pub fn list_args(driver: &BackendDriver, all: bool) -> Vec<String> {
        let mut args = match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "ps".into(), "--format".into(), "json".into()],
            _ => vec!["ps".into(), "--format".into(), "json".into()],
        };
        if all { args.push("--all".into()); }
        args
    }

    pub fn inspect_args(driver: &BackendDriver, id: &str) -> Vec<String> {
        match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "inspect".into(), "--format".into(), "json".into(), id.into()],
            _ => vec!["inspect".into(), "--format".into(), "json".into(), id.into()],
        }
    }

    pub fn logs_args(driver: &BackendDriver, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "logs".into()],
            _ => vec!["logs".into()],
        };
        if let Some(n) = tail { args.extend(["--tail".into(), n.to_string()]); }
        args.push(id.into());
        args
    }

    pub fn wait_args(driver: &BackendDriver, id: &str) -> Vec<String> {
        match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "wait".into(), id.into()],
            _ => vec!["wait".into(), id.into()],
        }
    }

    pub fn exec_args(driver: &BackendDriver, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "exec".into()],
            _ => vec!["exec".into()],
        };
        if let Some(envs) = env {
            for (k, v) in envs { args.extend(["-e".into(), format!("{}={}", k, v)]); }
        }
        if let Some(w) = workdir { args.extend(["-w".into(), w.into()]); }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        args
    }

    pub fn pull_args(driver: &BackendDriver, reference: &str) -> Vec<String> {
        match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "pull".into(), reference.into()],
            _ => vec!["pull".into(), reference.into()],
        }
    }

    pub fn list_images_args(driver: &BackendDriver) -> Vec<String> {
        match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "images".into(), "--format".into(), "json".into()],
            _ => vec!["images".into(), "--format".into(), "json".into()],
        }
    }

    pub fn remove_image_args(driver: &BackendDriver, reference: &str, force: bool) -> Vec<String> {
        let mut args = match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "rmi".into()],
            _ => vec!["rmi".into()],
        };
        if force { args.push("-f".into()); }
        args.push(reference.into());
        args
    }

    pub fn create_network_args(driver: &BackendDriver, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "network".into(), "create".into()],
            _ => vec!["network".into(), "create".into()],
        };
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        if config.internal { args.push("--internal".into()); }
        if config.enable_ipv6 { args.push("--ipv6".into()); }
        if let Some(labels) = &config.labels {
            for (k, v) in labels { args.extend(["--label".into(), format!("{}={}", k, v)]); }
        }
        args.push(name.into());
        args
    }

    pub fn remove_network_args(driver: &BackendDriver, name: &str) -> Vec<String> {
        match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "network".into(), "rm".into(), name.into()],
            _ => vec!["network".into(), "rm".into(), name.into()],
        }
    }

    pub fn create_volume_args(driver: &BackendDriver, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "volume".into(), "create".into()],
            _ => vec!["volume".into(), "create".into()],
        };
        if let Some(d) = &config.driver { args.extend(["--driver".into(), d.clone()]); }
        if let Some(labels) = &config.labels {
            for (k, v) in labels { args.extend(["--label".into(), format!("{}={}", k, v)]); }
        }
        args.push(name.into());
        args
    }

    pub fn remove_volume_args(driver: &BackendDriver, name: &str) -> Vec<String> {
        match driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "volume".into(), "rm".into(), name.into()],
            _ => vec!["volume".into(), "rm".into(), name.into()],
        }
    }

    pub fn parse_container_id(stdout: &str) -> Result<String> {
        let id = stdout.trim().to_string();
        if id.is_empty() {
            Err(ComposeError::BackendError { code: -1, message: "Empty output from run/create command".into() })
        } else {
            Ok(id)
        }
    }

    pub fn parse_list_output(stdout: &str) -> Result<Vec<ContainerInfo>> {
        #[derive(serde::Deserialize)]
        struct DockerContainer {
            #[serde(rename = "ID")] id: String,
            #[serde(rename = "Names")] names: String,
            #[serde(rename = "Image")] image: String,
            #[serde(rename = "Status")] status: String,
            #[serde(rename = "Ports")] ports: String,
            #[serde(rename = "CreatedAt")] created_at: String,
        }
        let mut infos = Vec::new();
        for line in stdout.lines() {
            if let Ok(c) = serde_json::from_str::<DockerContainer>(line) {
                infos.push(ContainerInfo {
                    id: c.id, name: c.names, image: c.image, status: c.status,
                    ports: c.ports.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
                    created: c.created_at,
                });
            }
        }
        Ok(infos)
    }

    pub fn parse_inspect_output(stdout: &str) -> Result<ContainerInfo> {
        #[derive(serde::Deserialize)]
        struct DockerInspect {
            #[serde(rename = "Id")] id: String,
            #[serde(rename = "Name")] name: String,
            #[serde(rename = "Config")] config: DockerInspectConfig,
            #[serde(rename = "State")] state: DockerInspectState,
            #[serde(rename = "Created")] created: String,
        }
        #[derive(serde::Deserialize)]
        struct DockerInspectConfig { #[serde(rename = "Image")] image: String }
        #[derive(serde::Deserialize)]
        struct DockerInspectState { #[serde(rename = "Status")] status: String }

        let list: Vec<DockerInspect> = serde_json::from_str(stdout)?;
        let c = list.first().ok_or_else(|| ComposeError::NotFound("inspect output empty".into()))?;
        Ok(ContainerInfo {
            id: c.id.clone(), name: c.name.clone().strip_prefix('/').unwrap_or(&c.name).to_string(),
            image: c.config.image.clone(), status: c.state.status.clone(),
            ports: vec![],
            created: c.created.clone(),
        })
    }

    pub fn parse_list_images_output(stdout: &str) -> Result<Vec<ImageInfo>> {
        #[derive(serde::Deserialize)]
        struct DockerImage {
            #[serde(rename = "ID")] id: String,
            #[serde(rename = "Repository")] repository: String,
            #[serde(rename = "Tag")] tag: String,
            #[serde(rename = "CreatedAt")] created_at: String,
        }
        let mut infos = Vec::new();
        for line in stdout.lines() {
            if let Ok(img) = serde_json::from_str::<DockerImage>(line) {
                infos.push(ImageInfo {
                    id: img.id, repository: img.repository, tag: img.tag,
                    size: 0,
                    created: img.created_at,
                });
            }
        }
        Ok(infos)
    }
}

// ============ Layer 4: OCI Backend ============

pub struct OciBackend {
    driver: BackendDriver,
}

impl OciBackend {
    pub fn new(driver: BackendDriver) -> Self { Self { driver } }

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
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr,
            })
        }
    }
}

#[async_trait]
impl ContainerBackend for OciBackend {
    fn backend_name(&self) -> &'static str { self.driver.name() }

    async fn check_available(&self) -> Result<()> {
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => vec!["shell".into(), instance.clone(), "nerdctl".into(), "--version".into()],
            _ => vec!["--version".into()],
        };
        self.exec_cli(&args).await.map(|_| ())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::run_args(&self.driver, spec, None);
        let (stdout, _) = self.exec_cli(&args).await?;
        let id = OciCommandBuilder::parse_container_id(&stdout)?;
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::run_args(&self.driver, spec, Some(profile));
        let (stdout, _) = self.exec_cli(&args).await?;
        let id = OciCommandBuilder::parse_container_id(&stdout)?;
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::create_args(&self.driver, spec);
        let (stdout, _) = self.exec_cli(&args).await?;
        let id = OciCommandBuilder::parse_container_id(&stdout)?;
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> {
        let args = OciCommandBuilder::start_args(&self.driver, id);
        self.exec_cli(&args).await.map(|_| ())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let args = OciCommandBuilder::stop_args(&self.driver, id, timeout);
        self.exec_cli(&args).await.map(|_| ())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let args = OciCommandBuilder::remove_args(&self.driver, id, force);
        self.exec_cli(&args).await.map(|_| ())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let args = OciCommandBuilder::list_args(&self.driver, all);
        let (stdout, _) = self.exec_cli(&args).await?;
        OciCommandBuilder::parse_list_output(&stdout)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let args = OciCommandBuilder::inspect_args(&self.driver, id);
        let (stdout, _) = self.exec_cli(&args).await?;
        OciCommandBuilder::parse_inspect_output(&stdout)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let args = OciCommandBuilder::logs_args(&self.driver, id, tail);
        let (stdout, stderr) = self.exec_cli(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs> {
        let wait_args = OciCommandBuilder::wait_args(&self.driver, id);
        let _ = self.exec_cli(&wait_args).await?;
        self.logs(id, None).await
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let args = OciCommandBuilder::exec_args(&self.driver, id, cmd, env, workdir);
        let (stdout, stderr) = self.exec_cli(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let args = OciCommandBuilder::pull_args(&self.driver, reference);
        self.exec_cli(&args).await.map(|_| ())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let args = OciCommandBuilder::list_images_args(&self.driver);
        let (stdout, _) = self.exec_cli(&args).await?;
        OciCommandBuilder::parse_list_images_output(&stdout)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let args = OciCommandBuilder::remove_image_args(&self.driver, reference, force);
        self.exec_cli(&args).await.map(|_| ())
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        let args = OciCommandBuilder::create_network_args(&self.driver, name, config);
        self.exec_cli(&args).await.map(|_| ())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let args = OciCommandBuilder::remove_network_args(&self.driver, name);
        self.exec_cli(&args).await.map(|_| ())
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        let args = OciCommandBuilder::create_volume_args(&self.driver, name, config);
        self.exec_cli(&args).await.map(|_| ())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let args = OciCommandBuilder::remove_volume_args(&self.driver, name);
        self.exec_cli(&args).await.map(|_| ())
    }
}

// ============ Layer 5: Runtime Detection ============

pub async fn detect_backend() -> Result<Box<dyn ContainerBackend + Send + Sync>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await
            .map_err(|reason| ComposeError::BackendNotAvailable { name, reason });
    }

    let candidates = platform_candidates();
    let mut results = Vec::new();

    for &candidate in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => {
                tracing::debug!(backend = candidate, "container backend detected");
                return Ok(backend);
            }
            Ok(Err(reason)) => {
                tracing::debug!(backend = candidate, reason = %reason, "backend probe failed");
                results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason });
            }
            Err(_) => {
                results.push(BackendProbeResult {
                    name: candidate.to_string(), available: false,
                    reason: "probe timed out after 2s".to_string(),
                });
            }
        }
    }

    Err(ComposeError::NoBackendFound { probed: results })
}

async fn probe_candidate(name: &str) -> std::result::Result<Box<dyn ContainerBackend + Send + Sync>, String> {
    match name {
        "apple/container" => {
            let bin = which("container").map_err(|_| "container binary not found".to_string())?;
            run_version_check(&bin).await?;
            Ok(Box::new(OciBackend::new(BackendDriver::AppleContainer { bin })))
        }
        "podman" => {
            let bin = which("podman").map_err(|_| "podman binary not found".to_string())?;
            run_version_check(&bin).await?;
            if cfg!(target_os = "macos") {
                check_podman_machine_running(&bin).await?;
            }
            Ok(Box::new(OciBackend::new(BackendDriver::Podman { bin })))
        }
        "orbstack" => {
            let bin = which("orb").or_else(|_| which("docker"))
                .map_err(|_| "orbstack not found".to_string())?;
            check_orbstack_available(&bin).await?;
            Ok(Box::new(OciBackend::new(BackendDriver::OrbStack { bin })))
        }
        "colima" => {
            let bin = which("colima").map_err(|_| "colima binary not found".to_string())?;
            check_colima_running(&bin).await?;
            let docker_bin = which("docker").map_err(|_| "docker CLI not found (needed for colima)".to_string())?;
            Ok(Box::new(OciBackend::new(BackendDriver::Colima { bin: docker_bin })))
        }
        "rancher-desktop" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl binary not found".to_string())?;
            run_version_check(&bin).await?;
            check_rancher_available().await?;
            Ok(Box::new(OciBackend::new(BackendDriver::RancherDesktop { bin })))
        }
        "lima" => {
            let bin = which("limactl").map_err(|_| "limactl binary not found".to_string())?;
            let instance = check_lima_running_instance(&bin).await?;
            Ok(Box::new(OciBackend::new(BackendDriver::Lima { bin, instance })))
        }
        "nerdctl" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl binary not found".to_string())?;
            run_version_check(&bin).await?;
            Ok(Box::new(OciBackend::new(BackendDriver::Nerdctl { bin })))
        }
        "docker" => {
            let bin = which("docker").map_err(|_| "docker binary not found".to_string())?;
            run_version_check(&bin).await?;
            Ok(Box::new(OciBackend::new(BackendDriver::Docker { bin })))
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

async fn run_version_check(bin: &Path) -> std::result::Result<(), String> {
    let output = Command::new(bin).arg("--version").output().await
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("version check failed with exit code {}", output.status.code().unwrap_or(-1)))
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
