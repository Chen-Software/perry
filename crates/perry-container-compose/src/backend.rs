use crate::error::{ComposeError, Result};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

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
    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub enum BackendDriver {
    AppleContainer { bin: PathBuf },
    Podman { bin: PathBuf },
    OrbStack { bin: PathBuf },
    Colima { bin: PathBuf },
    RancherDesktop { bin: PathBuf },
    Lima { bin: PathBuf },
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

    pub fn is_docker_compatible(&self) -> bool {
        !matches!(self, Self::AppleContainer { .. } | Self::Lima { .. })
    }
}

pub struct OciCommandBuilder;

impl OciCommandBuilder {
    pub fn run_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        let mut args = if driver.is_docker_compatible() {
            vec!["run".into(), "-d".into()]
        } else {
            vec!["run".into()]
        };

        if let Some(name) = &spec.name {
            args.extend(["--name".into(), name.clone()]);
        }
        if let Some(ports) = &spec.ports {
            for port in ports {
                args.extend(["-p".into(), port.clone()]);
            }
        }
        if let Some(volumes) = &spec.volumes {
            for vol in volumes {
                args.extend(["-v".into(), vol.clone()]);
            }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env {
                args.extend(["-e".into(), format!("{}={}", k, v)]);
            }
        }
        if spec.rm.unwrap_or(false) {
            args.push("--rm".into());
        }
        if spec.read_only.unwrap_or(false) {
            args.push("--read-only".into());
        }
        if let Some(seccomp) = &spec.seccomp {
            args.extend(["--security-opt".into(), format!("seccomp={}", seccomp)]);
        }
        if let Some(labels) = &spec.labels {
            for (k, v) in labels {
                args.extend(["-l".into(), format!("{}={}", k, v)]);
            }
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.clone());
        }
        args
    }

    pub fn create_args(_driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".into()];
        if let Some(name) = &spec.name {
            args.extend(["--name".into(), name.clone()]);
        }
        args.push(spec.image.clone());
        args
    }

    pub fn stop_args(_driver: &BackendDriver, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout {
            args.extend(["--time".into(), t.to_string()]);
        }
        args.push(id.to_string());
        args
    }

    pub fn remove_args(_driver: &BackendDriver, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".into()];
        if force {
            args.push("-f".into());
        }
        args.push(id.to_string());
        args
    }

    pub fn list_args(_driver: &BackendDriver, all: bool) -> Vec<String> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all {
            args.push("-a".into());
        }
        args
    }

    pub fn exec_args(
        _driver: &BackendDriver,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(e) = env {
            for (k, v) in e {
                args.extend(["-e".into(), format!("{}={}", k, v)]);
            }
        }
        if let Some(w) = workdir {
            args.extend(["-w".into(), w.to_string()]);
        }
        args.push(id.to_string());
        args.extend(cmd.iter().cloned());
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

    async fn exec_cli(&self, args: Vec<String>) -> Result<CliOutput> {
        let mut cmd = Command::new(self.driver.bin());
        cmd.args(args);

        let output = cmd.output().await.map_err(ComposeError::IoError)?;
        if output.status.success() {
            Ok(CliOutput {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(1),
                message: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }
}

struct CliOutput {
    stdout: String,
    stderr: String,
}

#[async_trait]
impl ContainerBackend for OciBackend {
    fn backend_name(&self) -> &'static str {
        self.driver.name()
    }

    async fn check_available(&self) -> Result<()> {
        self.exec_cli(vec!["--version".into()]).await.map(|_| ())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::run_args(&self.driver, spec);
        let output = self.exec_cli(args).await?;
        Ok(ContainerHandle {
            id: output.stdout.trim().to_string(),
            name: spec.name.clone(),
        })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = OciCommandBuilder::create_args(&self.driver, spec);
        let output = self.exec_cli(args).await?;
        Ok(ContainerHandle {
            id: output.stdout.trim().to_string(),
            name: spec.name.clone(),
        })
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.exec_cli(vec!["start".into(), id.to_string()])
            .await
            .map(|_| ())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let args = OciCommandBuilder::stop_args(&self.driver, id, timeout);
        self.exec_cli(args).await.map(|_| ())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let args = OciCommandBuilder::remove_args(&self.driver, id, force);
        self.exec_cli(args).await.map(|_| ())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let args = OciCommandBuilder::list_args(&self.driver, all);
        let output = self.exec_cli(args).await?;
        let list: Vec<ContainerInfo> = serde_json::from_str(&output.stdout).unwrap_or_default();
        Ok(list)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let output = self
            .exec_cli(vec![
                "inspect".into(),
                "--format".into(),
                "json".into(),
                id.to_string(),
            ])
            .await?;
        let list: Vec<ContainerInfo> = serde_json::from_str(&output.stdout).unwrap_or_default();
        list.into_iter()
            .next()
            .ok_or_else(|| ComposeError::NotFound(id.to_string()))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut args = vec!["logs".into()];
        if let Some(t) = tail {
            args.extend(["--tail".into(), t.to_string()]);
        }
        args.push(id.to_string());
        let output = self.exec_cli(args).await?;
        Ok(ContainerLogs {
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        let args = OciCommandBuilder::exec_args(&self.driver, id, cmd, env, workdir);
        let output = self.exec_cli(args).await?;
        Ok(ContainerLogs {
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.exec_cli(vec!["pull".into(), reference.to_string()])
            .await
            .map(|_| ())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let output = self
            .exec_cli(vec!["images".into(), "--format".into(), "json".into()])
            .await?;
        let list: Vec<ImageInfo> = serde_json::from_str(&output.stdout).unwrap_or_default();
        Ok(list)
    }

    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo> {
        let output = self
            .exec_cli(vec![
                "inspect".into(),
                "--type".into(),
                "image".into(),
                "--format".into(),
                "json".into(),
                reference.to_string(),
            ])
            .await?;
        let list: Vec<ImageInfo> = serde_json::from_str(&output.stdout).unwrap_or_default();
        list.into_iter()
            .next()
            .ok_or_else(|| ComposeError::NotFound(reference.to_string()))
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let mut args = vec!["rmi".into()];
        if force {
            args.push("-f".into());
        }
        args.push(reference.to_string());
        self.exec_cli(args).await.map(|_| ())
    }

    async fn create_network(&self, name: &str, _config: &ComposeNetwork) -> Result<()> {
        self.exec_cli(vec!["network".into(), "create".into(), name.to_string()])
            .await
            .map(|_| ())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.exec_cli(vec!["network".into(), "rm".into(), name.to_string()])
            .await
            .map(|_| ())
    }

    async fn create_volume(&self, name: &str, _config: &ComposeVolume) -> Result<()> {
        self.exec_cli(vec!["volume".into(), "create".into(), name.to_string()])
            .await
            .map(|_| ())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.exec_cli(vec!["volume".into(), "rm".into(), name.to_string()])
            .await
            .map(|_| ())
    }
}

pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend + Send + Sync>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        match probe_candidate(&name).await {
            Ok(driver) => return Ok(Box::new(OciBackend::new(driver))),
            Err(res) => return Err(vec![res]),
        }
    }

    let candidates = match std::env::consts::OS {
        "macos" | "ios" => vec![
            "apple/container",
            "orbstack",
            "colima",
            "rancher-desktop",
            "podman",
            "lima",
            "docker",
        ],
        _ => vec!["podman", "nerdctl", "docker"],
    };

    let mut results = Vec::new();
    for name in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(name)).await {
            Ok(Ok(driver)) => {
                tracing::debug!(backend = name, available = true, "backend probe success");
                return Ok(Box::new(OciBackend::new(driver)));
            }
            Ok(Err(res)) => {
                tracing::debug!(
                    backend = name,
                    available = false,
                    reason = %res.reason,
                    "backend probe failed"
                );
                results.push(res);
            }
            Err(_) => {
                tracing::debug!(backend = name, available = false, "backend probe timed out");
                results.push(BackendProbeResult {
                    name: name.to_string(),
                    available: false,
                    reason: "probe timed out".into(),
                });
            }
        }
    }

    Err(results)
}

async fn probe_candidate(name: &str) -> std::result::Result<BackendDriver, BackendProbeResult> {
    let bin_name = match name {
        "apple/container" => "container",
        "podman" => "podman",
        "orbstack" => "orb",
        "colima" => "colima",
        "rancher-desktop" => "nerdctl",
        "lima" => "limactl",
        "nerdctl" => "nerdctl",
        "docker" => "docker",
        _ => {
            return Err(BackendProbeResult {
                name: name.to_string(),
                available: false,
                reason: "unknown backend".into(),
            })
        }
    };

    if let Some(path) = find_binary(bin_name) {
        let driver = match name {
            "apple/container" => BackendDriver::AppleContainer { bin: path.clone() },
            "podman" => BackendDriver::Podman { bin: path.clone() },
            "orbstack" => BackendDriver::OrbStack { bin: path.clone() },
            "colima" => BackendDriver::Colima { bin: path.clone() },
            "rancher-desktop" => BackendDriver::RancherDesktop { bin: path.clone() },
            "lima" => BackendDriver::Lima { bin: path.clone() },
            "nerdctl" => BackendDriver::Nerdctl { bin: path.clone() },
            "docker" => BackendDriver::Docker { bin: path.clone() },
            _ => unreachable!(),
        };

        let mut cmd = Command::new(&path);
        cmd.arg("--version");
        match cmd.output().await {
            Ok(output) if output.status.success() => {
                if name == "colima" {
                    let status = Command::new(&path).arg("status").output().await;
                    if let Ok(s) = status {
                        if !String::from_utf8_lossy(&s.stdout).contains("running") {
                            return Err(BackendProbeResult {
                                name: name.to_string(),
                                available: false,
                                reason: "colima is not running".into(),
                            });
                        }
                    }
                } else if name == "podman" && std::env::consts::OS == "macos" {
                    let status = Command::new(&path)
                        .args(["machine", "list", "--format", "json"])
                        .output()
                        .await;
                    if let Ok(s) = status {
                        let v: serde_json::Value =
                            serde_json::from_slice(&s.stdout).unwrap_or_default();
                        let running = v.as_array().map_or(false, |a| {
                            a.iter().any(|m| m["Running"].as_bool().unwrap_or(false))
                        });
                        if !running {
                            return Err(BackendProbeResult {
                                name: name.to_string(),
                                available: false,
                                reason: "no podman machine is running".into(),
                            });
                        }
                    }
                } else if name == "lima" {
                    let status = Command::new(&path).args(["list", "--json"]).output().await;
                    if let Ok(s) = status {
                        // lima output is multiple JSON objects, one per line
                        let running = String::from_utf8_lossy(&s.stdout)
                            .lines()
                            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                            .any(|m| m["status"].as_str() == Some("Running"));
                        if !running {
                            return Err(BackendProbeResult {
                                name: name.to_string(),
                                available: false,
                                reason: "no lima instance is running".into(),
                            });
                        }
                    }
                } else if name == "rancher-desktop" {
                    // Req 16.9: nerdctl binary AND containerd socket
                    let socket = dirs::home_dir()
                        .map(|h| h.join(".rd/run/containerd-shim.sock"))
                        .unwrap_or_default();
                    if !socket.exists() {
                        return Err(BackendProbeResult {
                            name: name.to_string(),
                            available: false,
                            reason: "Rancher Desktop containerd socket not found".into(),
                        });
                    }
                }
                Ok(driver)
            }
            Ok(output) => Err(BackendProbeResult {
                name: name.to_string(),
                available: false,
                reason: format!(
                    "CLI returned error: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            }),
            Err(e) => Err(BackendProbeResult {
                name: name.to_string(),
                available: false,
                reason: e.to_string(),
            }),
        }
    } else {
        Err(BackendProbeResult {
            name: name.to_string(),
            available: false,
            reason: format!("binary '{}' not found in PATH", bin_name),
        })
    }
}

fn find_binary(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .filter_map(|p| {
                let full_path = p.join(name);
                if full_path.is_file() {
                    Some(full_path)
                } else {
                    None
                }
            })
            .next()
    })
}
