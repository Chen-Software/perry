//! Container backend abstraction — `ContainerBackend` trait, `BackendDriver` enum,
//! `OciBackend`, and `detect_backend()`.

use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{
    ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo, ComposeNetwork, ComposeVolume,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

// ─────────────────────────────────────────────────────────────────────────────
// 4.1  ContainerBackend trait
// ─────────────────────────────────────────────────────────────────────────────

/// Runtime-agnostic async interface for container operations.
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
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.2 BackendDriver
// ─────────────────────────────────────────────────────────────────────────────

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

    /// Returns true if this driver accepts Docker-compatible CLI flags.
    pub fn is_docker_compatible(&self) -> bool {
        !matches!(self, Self::AppleContainer { .. } | Self::Lima { .. })
    }

    pub fn binary_path(&self) -> &Path {
        match self {
            Self::AppleContainer { bin } => bin,
            Self::Podman { bin } => bin,
            Self::OrbStack { bin } => bin,
            Self::Colima { bin } => bin,
            Self::RancherDesktop { bin } => bin,
            Self::Lima { bin, .. } => bin,
            Self::Nerdctl { bin } => bin,
            Self::Docker { bin } => bin,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.3 OciCommandBuilder
// ─────────────────────────────────────────────────────────────────────────────

pub struct OciCommandBuilder;

impl OciCommandBuilder {
    pub fn run_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        match driver {
            BackendDriver::AppleContainer { .. } => Self::apple_run_args(spec),
            BackendDriver::Lima { instance, .. } => Self::lima_args(instance, &Self::docker_run_args(spec)),
            _ => Self::docker_run_args(spec),
        }
    }

    fn docker_run_args(spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string(), "--detach".to_string()];
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        if let Some(network) = &spec.network { args.extend(["--network".into(), network.clone()]); }
        if let Some(ports) = &spec.ports {
            for p in ports { args.extend(["-p".into(), p.clone()]); }
        }
        if let Some(volumes) = &spec.volumes {
            for v in volumes { args.extend(["-v".into(), v.clone()]); }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env { args.extend(["-e".into(), format!("{}={}", k, v)]); }
        }
        if let Some(entrypoint) = &spec.entrypoint {
            args.extend(["--entrypoint".into(), entrypoint.join(" ")]);
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }

    fn apple_run_args(spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string()];
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }

    fn lima_args(instance: &str, inner_args: &[String]) -> Vec<String> {
        let mut args = vec!["shell".to_string(), instance.to_string(), "nerdctl".to_string()];
        args.extend(inner_args.iter().cloned());
        args
    }

    // ... other arg builders would go here following the same pattern
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.4 OciBackend
// ─────────────────────────────────────────────────────────────────────────────

pub struct OciBackend {
    driver: BackendDriver,
}

impl OciBackend {
    pub fn new(driver: BackendDriver) -> Self { Self { driver } }

    async fn exec_cli(&self, args: &[String]) -> Result<ContainerLogs> {
        let mut cmd = Command::new(self.driver.binary_path());
        cmd.args(args);
        let output = cmd.output().await.map_err(ComposeError::IoError)?;
        if output.status.success() {
            Ok(ContainerLogs {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            })
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            })
        }
    }
}

#[async_trait]
impl ContainerBackend for OciBackend {
    fn backend_name(&self) -> &'static str { self.driver.name() }

    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(self.driver.binary_path());
        cmd.arg("--version");
        let output = cmd.output().await.map_err(ComposeError::IoError)?;
        if output.status.success() { Ok(()) } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: "backend not available".into()
            })
        }
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::run_args(&self.driver, spec);
        let output = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: output.stdout.trim().to_string(), name: spec.name.clone() })
    }

    async fn create(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> {
        // Implementation for create
        todo!()
    }

    async fn start(&self, id: &str) -> Result<()> {
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &["start".into(), id.into()]),
            _ => vec!["start".into(), id.into()],
        };
        self.exec_cli(&args).await?;
        Ok(())
    }

    async fn stop(&self, id: &str, timeout_secs: Option<u32>) -> Result<()> {
        let mut inner = vec!["stop".into()];
        if let Some(t) = timeout_secs { inner.extend(["--time".into(), t.to_string()]); }
        inner.push(id.into());
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        self.exec_cli(&args).await?;
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let mut inner = vec!["rm".into()];
        if force { inner.push("-f".into()); }
        inner.push(id.into());
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        self.exec_cli(&args).await?;
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let mut inner = vec!["ps".into(), "--format".into(), "json".into()];
        if all { inner.push("--all".into()); }
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        let output = self.exec_cli(&args).await?;
        // Basic parsing of JSON output
        let containers: Vec<ContainerInfo> = serde_json::from_str(&output.stdout).unwrap_or_default();
        Ok(containers)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let inner = vec!["inspect".into(), "--format".into(), "json".into(), id.into()];
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        let output = self.exec_cli(&args).await?;
        let infos: Vec<ContainerInfo> = serde_json::from_str(&output.stdout).map_err(ComposeError::JsonError)?;
        infos.into_iter().next().ok_or_else(|| ComposeError::NotFound(id.to_string()))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut inner = vec!["logs".into()];
        if let Some(n) = tail { inner.extend(["--tail".into(), n.to_string()]); }
        inner.push(id.into());
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        self.exec_cli(&args).await
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        let mut inner = vec!["exec".into()];
        if let Some(w) = workdir { inner.extend(["--workdir".into(), w.into()]); }
        if let Some(e) = env {
            for (k, v) in e { inner.extend(["-e".into(), format!("{}={}", k, v)]); }
        }
        inner.push(id.into());
        inner.extend(cmd.iter().cloned());
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        self.exec_cli(&args).await
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let inner = vec!["pull".into(), reference.into()];
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        self.exec_cli(&args).await?;
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let inner = vec!["images".into(), "--format".into(), "json".into()];
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        let output = self.exec_cli(&args).await?;
        let images: Vec<ImageInfo> = serde_json::from_str(&output.stdout).unwrap_or_default();
        Ok(images)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let mut inner = vec!["rmi".into()];
        if force { inner.push("-f".into()); }
        inner.push(reference.into());
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        self.exec_cli(&args).await?;
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> {
        let mut inner = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver { inner.extend(["--driver".into(), d.clone()]); }
        inner.push(name.into());
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        self.exec_cli(&args).await?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let inner = vec!["network".into(), "rm".into(), name.into()];
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        self.exec_cli(&args).await?;
        Ok(())
    }

    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        let mut inner = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver { inner.extend(["--driver".into(), d.clone()]); }
        inner.push(name.into());
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        self.exec_cli(&args).await?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let inner = vec!["volume".into(), "rm".into(), name.into()];
        let args = match &self.driver {
            BackendDriver::Lima { instance, .. } => OciCommandBuilder::lima_args(instance, &inner),
            _ => inner,
        };
        self.exec_cli(&args).await?;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.5 Runtime Detection
// ─────────────────────────────────────────────────────────────────────────────

static CACHED_BACKEND: OnceLock<Arc<dyn ContainerBackend>> = OnceLock::new();

pub async fn detect_backend() -> Result<Arc<dyn ContainerBackend>> {
    if let Some(backend) = CACHED_BACKEND.get() {
        return Ok(Arc::clone(backend));
    }

    // 1. Check PERRY_CONTAINER_BACKEND override
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return match probe_candidate(&name).await {
            Ok(backend) => {
                let arc = Arc::from(backend);
                let _ = CACHED_BACKEND.set(Arc::clone(&arc));
                Ok(arc)
            }
            Err(reason) => Err(ComposeError::BackendNotAvailable { name, reason }),
        };
    }

    // 2. Platform-specific candidate list
    let candidates: &[&str] = match std::env::consts::OS {
        "macos" | "ios" => &["apple/container", "orbstack", "colima",
                            "rancher-desktop", "podman", "lima", "docker"],
        "linux" => &["podman", "nerdctl", "docker"],
        _ => &["podman", "nerdctl", "docker"],
    };

    // 3. Probe each candidate
    let mut results = Vec::new();
    for &name in candidates {
        let res = timeout(Duration::from_secs(2), probe_candidate(name)).await;
        let probe_res = match res {
            Ok(Ok(backend)) => {
                let arc = Arc::from(backend);
                let _ = CACHED_BACKEND.set(Arc::clone(&arc));
                return Ok(arc);
            }
            Ok(Err(reason)) => BackendProbeResult { name: name.to_string(), available: false, reason },
            Err(_) => BackendProbeResult { name: name.to_string(), available: false, reason: "probe timed out after 2s".into() },
        };
        results.push(probe_res);
    }

    Err(ComposeError::NoBackendFound { probed: results })
}

async fn probe_candidate(name: &str) -> std::result::Result<Box<dyn ContainerBackend>, String> {
    match name {
        "apple/container" => {
            let bin = which::which("container").map_err(|_| "not found")?;
            Ok(Box::new(OciBackend::new(BackendDriver::AppleContainer { bin })))
        }
        "podman" => {
            let bin = which::which("podman").map_err(|_| "not found")?;
            if std::env::consts::OS == "macos" {
                let out = Command::new(&bin).args(["machine", "list", "--format", "json"]).output().await.map_err(|e| e.to_string())?;
                let json: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
                let running = json.as_array().map(|a| a.iter().any(|m| m["Running"].as_bool().unwrap_or(false))).unwrap_or(false);
                if !running { return Err("no running podman machine".into()); }
            }
            Ok(Box::new(OciBackend::new(BackendDriver::Podman { bin })))
        }
        "colima" => {
            let bin = which::which("colima").map_err(|_| "not found")?;
            let out = Command::new(&bin).arg("status").output().await.map_err(|e| e.to_string())?;
            if !String::from_utf8_lossy(&out.stdout).contains("running") { return Err("not running".into()); }
            let dbin = which::which("docker").map_err(|_| "docker CLI not found")?;
            Ok(Box::new(OciBackend::new(BackendDriver::Colima { bin: dbin })))
        }
        "lima" => {
            let bin = which::which("limactl").map_err(|_| "not found")?;
            let out = Command::new(&bin).args(["list", "--json"]).output().await.map_err(|e| e.to_string())?;
            let instance = String::from_utf8_lossy(&out.stdout).lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| v["status"] == "Running")
                .and_then(|v| v["name"].as_str().map(|s| s.to_string()))
                .ok_or("no running instance")?;
            Ok(Box::new(OciBackend::new(BackendDriver::Lima { bin, instance })))
        }
        "docker" => {
            let bin = which::which("docker").map_err(|_| "not found")?;
            Ok(Box::new(OciBackend::new(BackendDriver::Docker { bin })))
        }
        "orbstack" => {
            let bin = which::which("orb").map_err(|_| "not found")?;
            Ok(Box::new(OciBackend::new(BackendDriver::OrbStack { bin: which::which("docker").unwrap_or(bin) })))
        }
        "nerdctl" => {
            let bin = which::which("nerdctl").map_err(|_| "not found")?;
            Ok(Box::new(OciBackend::new(BackendDriver::Nerdctl { bin })))
        }
        "rancher-desktop" => {
            let bin = which::which("nerdctl").map_err(|_| "not found")?;
            Ok(Box::new(OciBackend::new(BackendDriver::RancherDesktop { bin })))
        }
        _ => Err("unknown backend".into()),
    }
}

pub fn get_backend() -> Result<Arc<dyn ContainerBackend>> {
    CACHED_BACKEND.get().cloned().ok_or_else(|| ComposeError::BackendNotAvailable {
        name: "unknown".into(),
        reason: "backend not detected, call detect_backend() first".into(),
    })
}
