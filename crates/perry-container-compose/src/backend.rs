use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use which::which;

pub use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo};

/// Layer 1: Abstract Operations
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
    async fn inspect_volume(&self, name: &str) -> Result<()>;
}

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

/// Layer 2: CLI Protocol
pub trait CliProtocol: Send + Sync {
    fn protocol_name(&self) -> &str;

    fn subcommand_prefix(&self) -> Option<Vec<String>> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        docker_run_flags(spec, true)
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        docker_run_flags(spec, false)
    }

    fn start_args(&self, id: &str) -> Vec<String> {
        vec!["start".into(), id.into()]
    }

    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout {
            args.extend(["--time".into(), t.to_string()]);
        }
        args.push(id.into());
        args
    }

    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".into()];
        if force {
            args.push("-f".into());
        }
        args.push(id.into());
        args
    }

    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all {
            args.push("-a".into());
        }
        args
    }

    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".into(), "--format".into(), "json".into(), id.into()]
    }

    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if let Some(n) = tail {
            args.extend(["--tail".into(), n.to_string()]);
        }
        args.push(id.into());
        args
    }

    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(e) = env {
            for (k, v) in e {
                args.extend(["-e".into(), format!("{}={}", k, v)]);
            }
        }
        if let Some(w) = workdir {
            args.extend(["-w".into(), w.into()]);
        }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        args
    }

    fn pull_image_args(&self, reference: &str) -> Vec<String> {
        vec!["pull".into(), reference.into()]
    }

    fn list_images_args(&self) -> Vec<String> {
        vec!["images".into(), "--format".into(), "json".into()]
    }

    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".into()];
        if force {
            args.push("-f".into());
        }
        args.push(reference.into());
        args
    }

    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver {
            args.extend(["--driver".into(), d.clone()]);
        }
        for (k, v) in &config.labels {
            args.extend(["--label".into(), format!("{}={}", k, v)]);
        }
        if config.internal {
            args.push("--internal".into());
        }
        if config.enable_ipv6 {
            args.push("--ipv6".into());
        }
        args.push(name.into());
        args
    }

    fn remove_network_args(&self, name: &str) -> Vec<String> {
        vec!["network".into(), "rm".into(), name.into()]
    }

    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver {
            args.extend(["--driver".into(), d.clone()]);
        }
        for (k, v) in &config.labels {
            args.extend(["--label".into(), format!("{}={}", k, v)]);
        }
        args.push(name.into());
        args
    }

    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        vec!["volume".into(), "rm".into(), name.into()]
    }

    fn inspect_volume_args(&self, name: &str) -> Vec<String> {
        vec!["volume".into(), "inspect".into(), name.into()]
    }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        serde_json::from_str(stdout).unwrap_or_else(|_| {
            stdout.lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .filter_map(|v| parse_container_info_from_json(&v).ok())
                .collect()
        })
    }

    fn parse_inspect_output(&self, _id: &str, stdout: &str) -> Option<ContainerInfo> {
        let v: serde_json::Value = serde_json::from_str(stdout).ok()?;
        if let Some(arr) = v.as_array() {
            arr.first().and_then(|v| parse_container_info_from_json(v).ok())
        } else {
            parse_container_info_from_json(&v).ok()
        }
    }

    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> {
        stdout.lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .filter_map(|v| parse_image_info_from_json(&v).ok())
            .collect()
    }

    fn parse_container_id(&self, stdout: &str) -> String {
        stdout.trim().to_string()
    }
}

pub fn docker_run_flags(spec: &ContainerSpec, include_detach: bool) -> Vec<String> {
    let mut args = vec!["run".to_string()];
    if include_detach {
        args.push("-d".to_string());
    }
    if let Some(name) = &spec.name {
        args.extend(["--name".into(), name.clone()]);
    }
    if let Some(ports) = &spec.ports {
        for p in ports {
            args.extend(["-p".into(), p.clone()]);
        }
    }
    if let Some(volumes) = &spec.volumes {
        for v in volumes {
            args.extend(["-v".into(), v.clone()]);
        }
    }
    if let Some(env) = &spec.env {
        for (k, v) in env {
            args.extend(["-e".into(), format!("{}={}", k, v)]);
        }
    }
    if let Some(network) = &spec.network {
        args.extend(["--network".into(), network.clone()]);
    }
    if spec.rm.unwrap_or(false) {
        args.push("--rm".into());
    }
    if let Some(entrypoint) = &spec.entrypoint {
        args.extend(["--entrypoint".into(), entrypoint.join(" ")]);
    }
    args.push(spec.image.clone());
    if let Some(cmd) = &spec.cmd {
        args.extend(cmd.clone());
    }
    args
}

fn parse_container_info_from_json(json: &serde_json::Value) -> Result<ContainerInfo> {
    let id = json["Id"].as_str().or(json["ID"].as_str()).unwrap_or("").to_string();
    let name = json["Names"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str())
        .or(json["Name"].as_str())
        .unwrap_or("")
        .trim_start_matches('/')
        .to_string();
    let image = json["Image"].as_str().unwrap_or("").to_string();
    let status = json["Status"].as_str().or_else(|| json["State"].get("Status").and_then(|v| v.as_str())).unwrap_or("").to_string();
    Ok(ContainerInfo { id, name, image, status, ports: Vec::new(), created: json["Created"].as_str().unwrap_or("").to_string() })
}

fn parse_image_info_from_json(json: &serde_json::Value) -> Result<ImageInfo> {
    let id = json["Id"].as_str().or(json["ID"].as_str()).unwrap_or("").to_string();
    Ok(ImageInfo { id, repository: json["Repository"].as_str().unwrap_or("").to_string(), tag: json["Tag"].as_str().unwrap_or("").to_string(), size: json["Size"].as_u64().unwrap_or(0), created: json["Created"].as_str().unwrap_or("").to_string() })
}

/// Layer 2 impl: Docker-compatible
pub struct DockerProtocol;
impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &str { "docker-compatible" }
}

/// Layer 2 impl: Apple Container CLI
pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str { "apple/container" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        docker_run_flags(spec, false)
    }
}

/// Layer 2 impl: Lima
pub struct LimaProtocol {
    pub instance: String,
}
impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &str { "lima" }
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec!["shell".into(), self.instance.clone(), "nerdctl".into()])
    }
}

/// Layer 3: Generic CLI Executor
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
        cmd.output().await.map_err(ComposeError::IoError)
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

pub type DockerBackend = CliBackend<DockerProtocol>;
pub type AppleBackend = CliBackend<AppleContainerProtocol>;
pub type LimaBackend = CliBackend<LimaProtocol>;

#[async_trait]
impl<P: CliProtocol + Send + Sync + 'static> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str {
        self.bin.file_name().and_then(|n| n.to_str()).unwrap_or("unknown")
    }

    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("--version");
        let _ = timeout(Duration::from_secs(2), cmd.output()).await
            .map_err(|_| ComposeError::BackendError { code: 125, message: "check_available timed out".into() })?
            .map_err(ComposeError::IoError)?;
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
            .ok_or_else(|| ComposeError::NotFound(id.into()))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let output = self.exec_raw(self.protocol.logs_args(id, tail)).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let output = self.exec_raw(self.protocol.exec_args(id, cmd, env, workdir)).await?;
        if !output.status.success() {
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(1),
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
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

    async fn build(&self, spec: &crate::types::ComposeServiceBuild, image_name: &str) -> Result<()> {
        let mut args = vec!["build".into(), "--tag".into(), image_name.into()];
        if let Some(ctx) = &spec.context {
            args.push(ctx.clone());
        }
        self.exec_ok(args).await?;
        Ok(())
    }

    async fn inspect_network(&self, name: &str) -> Result<()> {
        let args = vec!["network".into(), "inspect".into(), name.into()];
        self.exec_ok(args).await?;
        Ok(())
    }

    async fn inspect_volume(&self, name: &str) -> Result<()> {
        let args = self.protocol.inspect_volume_args(name);
        self.exec_ok(args).await?;
        Ok(())
    }
}

/// Identifies the detected container runtime and its resolved CLI binary path.
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
}

pub struct OciBackend {
    inner: Box<dyn ContainerBackend>,
    driver: BackendDriver,
}

impl OciBackend {
    pub fn new(driver: BackendDriver) -> Self {
        let inner: Box<dyn ContainerBackend> = match &driver {
            BackendDriver::AppleContainer { bin } => Box::new(CliBackend::new(bin.clone(), AppleContainerProtocol)),
            BackendDriver::Lima { bin, instance } => Box::new(CliBackend::new(bin.clone(), LimaProtocol { instance: instance.clone() })),
            BackendDriver::Podman { bin } | BackendDriver::OrbStack { bin } | BackendDriver::Colima { bin } |
            BackendDriver::RancherDesktop { bin } | BackendDriver::Nerdctl { bin } | BackendDriver::Docker { bin } =>
                Box::new(CliBackend::new(bin.clone(), DockerProtocol)),
        };
        Self { inner, driver }
    }

    pub fn driver(&self) -> &BackendDriver {
        &self.driver
    }
}

#[async_trait]
impl ContainerBackend for OciBackend {
    fn backend_name(&self) -> &str {
        self.driver.name()
    }

    async fn check_available(&self) -> Result<()> {
        self.inner.check_available().await
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.inner.run(spec).await
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        self.inner.create(spec).await
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.inner.start(id).await
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        self.inner.stop(id, timeout).await
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        self.inner.remove(id, force).await
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        self.inner.list(all).await
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        self.inner.inspect(id).await
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        self.inner.logs(id, tail).await
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        self.inner.exec(id, cmd, env, workdir).await
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        self.inner.pull_image(reference).await
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        self.inner.list_images().await
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        self.inner.remove_image(reference, force).await
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        self.inner.create_network(name, config).await
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        self.inner.remove_network(name).await
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        self.inner.create_volume(name, config).await
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        self.inner.remove_volume(name).await
    }

    async fn build(&self, spec: &crate::types::ComposeServiceBuild, image_name: &str) -> Result<()> {
        self.inner.build(spec, image_name).await
    }

    async fn inspect_network(&self, name: &str) -> Result<()> {
        self.inner.inspect_network(name).await
    }

    async fn inspect_volume(&self, name: &str) -> Result<()> {
        self.inner.inspect_volume(name).await
    }
}

pub struct OciCommandBuilder;
impl OciCommandBuilder {
    pub fn run_args(driver: &BackendDriver, spec: &ContainerSpec) -> Vec<String> {
        match driver {
            BackendDriver::AppleContainer { .. } => AppleContainerProtocol.run_args(spec),
            BackendDriver::Lima { instance, .. } => LimaProtocol { instance: instance.clone() }.run_args(spec),
            _ => DockerProtocol.run_args(spec),
        }
    }
}
static GLOBAL_BACKEND: tokio::sync::OnceCell<std::sync::Arc<dyn ContainerBackend + Send + Sync>> = tokio::sync::OnceCell::const_new();

pub async fn get_global_backend_instance() -> Result<std::sync::Arc<dyn ContainerBackend + Send + Sync>> {
    GLOBAL_BACKEND.get_or_try_init(|| async {
        detect_backend().await.map_err(|e| ComposeError::NoBackendFound { probed: e })
    }).await.cloned()
}

/// Layer 4: Runtime Detection
pub async fn detect_backend() -> std::result::Result<std::sync::Arc<dyn ContainerBackend + Send + Sync>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await
            .map(|(b, _)| b)
            .map_err(|reason| vec![BackendProbeResult {
                name: name.clone(), available: false, reason: Some(reason), version: None
            }]);
    }

    let mode = std::env::var("PERRY_CONTAINER_MODE").unwrap_or_else(|_| "local-first".to_string());

    let candidates: &[&str] = if mode == "server-first" {
        &["docker", "podman", "nerdctl"] // Remote candidates typically use standard names
    } else {
        match std::env::consts::OS {
            "macos" | "ios" => &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"],
            _ => &["podman", "nerdctl", "docker"],
        }
    };

    let mut results = Vec::new();

    for &candidate in candidates {
        match timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok((backend, _version))) => {
                return Ok(backend);
            }
            Ok(Err(reason)) => {
                results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason: Some(reason), version: None });
            }
            Err(_) => {
                results.push(BackendProbeResult {
                    name: candidate.to_string(), available: false,
                    reason: Some("probe timed out after 2s".to_string()),
                    version: None,
                });
            }
        }
    }

    Err(results)
}

pub async fn probe_all_backends() -> Vec<BackendProbeResult> {
    let candidates: &[&str] = match std::env::consts::OS {
        "macos" | "ios" => &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"],
        _ => &["podman", "nerdctl", "docker"],
    };

    let mut results = Vec::new();
    for &candidate in candidates {
        match timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok((_, version))) => {
                results.push(BackendProbeResult {
                    name: candidate.to_string(),
                    available: true,
                    reason: None,
                    version: Some(version),
                });
            }
            Ok(Err(reason)) => {
                results.push(BackendProbeResult { name: candidate.to_string(), available: false, reason: Some(reason), version: None });
            }
            Err(_) => {
                results.push(BackendProbeResult {
                    name: candidate.to_string(), available: false,
                    reason: Some("probe timed out after 2s".to_string()),
                    version: None,
                });
            }
        }
    }
    results
}

async fn probe_candidate(name: &str) -> std::result::Result<(std::sync::Arc<dyn ContainerBackend + Send + Sync>, String), String> {
    let (bin, ver, driver) = match name {
        "apple/container" => {
            let bin = which("container").map_err(|_| "container binary not found on PATH".to_string())?;
            let ver = run_version_check(&bin).await?;
            (bin, ver, BackendDriver::AppleContainer { bin: PathBuf::new() }) // bin will be replaced below
        }
        "podman" => {
            let bin = which("podman").map_err(|_| "podman binary not found on PATH".to_string())?;
            let ver = run_version_check(&bin).await?;
            if std::env::consts::OS == "macos" {
                check_podman_machine_running(&bin).await?;
            }
            (bin, ver, BackendDriver::Podman { bin: PathBuf::new() })
        }
        "orbstack" => {
            let bin = which("orb").or_else(|_| which("docker"))
                .map_err(|_| "orbstack not found".to_string())?;
            let ver = check_orbstack_socket_or_version(&bin).await?;
            (bin, ver, BackendDriver::OrbStack { bin: PathBuf::new() })
        }
        "colima" => {
            let bin = which("colima").map_err(|_| "colima binary not found on PATH".to_string())?;
            check_colima_running(&bin).await?;
            let docker_bin = which("docker").map_err(|_| "docker CLI not found (needed for colima)".to_string())?;
            let ver = run_version_check(&docker_bin).await?;
            (docker_bin, ver, BackendDriver::Colima { bin: PathBuf::new() })
        }
        "rancher-desktop" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl binary not found on PATH".to_string())?;
            let ver = run_version_check(&bin).await?;
            check_rancher_socket().await?;
            (bin, ver, BackendDriver::RancherDesktop { bin: PathBuf::new() })
        }
        "lima" => {
            let bin = which("limactl").map_err(|_| "limactl binary not found on PATH".to_string())?;
            let instance = check_lima_running_instance(&bin).await?;
            let ver = run_version_check(&bin).await?;
            (bin, ver, BackendDriver::Lima { bin: PathBuf::new(), instance })
        }
        "nerdctl" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl binary not found on PATH".to_string())?;
            let ver = run_version_check(&bin).await?;
            (bin, ver, BackendDriver::Nerdctl { bin: PathBuf::new() })
        }
        "docker" => {
            let bin = which("docker").map_err(|_| "docker binary not found on PATH".to_string())?;
            let ver = run_version_check(&bin).await?;
            (bin, ver, BackendDriver::Docker { bin: PathBuf::new() })
        }
        other => return Err(format!("unknown backend: {other}")),
    };

    let final_driver = match driver {
        BackendDriver::AppleContainer { .. } => BackendDriver::AppleContainer { bin: bin.clone() },
        BackendDriver::Podman { .. } => BackendDriver::Podman { bin: bin.clone() },
        BackendDriver::OrbStack { .. } => BackendDriver::OrbStack { bin: bin.clone() },
        BackendDriver::Colima { .. } => BackendDriver::Colima { bin: bin.clone() },
        BackendDriver::RancherDesktop { .. } => BackendDriver::RancherDesktop { bin: bin.clone() },
        BackendDriver::Lima { instance, .. } => BackendDriver::Lima { bin: bin.clone(), instance },
        BackendDriver::Nerdctl { .. } => BackendDriver::Nerdctl { bin: bin.clone() },
        BackendDriver::Docker { .. } => BackendDriver::Docker { bin: bin.clone() },
    };

    Ok((std::sync::Arc::new(OciBackend::new(final_driver)), ver))
}

async fn run_version_check(bin: &Path) -> std::result::Result<String, String> {
    let output = Command::new(bin).arg("--version").output().await
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(format!("{} --version failed", bin.display()))
    }
}

async fn check_podman_machine_running(bin: &Path) -> std::result::Result<(), String> {
    let output = Command::new(bin).args(["machine", "list", "--format", "json"]).output().await
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        let val: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
        if val.as_array().map_or(false, |a| a.iter().any(|m| m["Running"].as_bool() == Some(true))) {
            Ok(())
        } else {
            Err("no running podman machine".to_string())
        }
    } else {
        Err("podman machine list failed".to_string())
    }
}

async fn check_orbstack_socket_or_version(bin: &Path) -> std::result::Result<String, String> {
    let socket = home::home_dir().map(|h| h.join(".orbstack/run/docker.sock"));
    if socket.map_or(false, |s| s.exists()) {
        run_version_check(bin).await
    } else {
        run_version_check(bin).await
    }
}

async fn check_colima_running(bin: &Path) -> std::result::Result<(), String> {
    let output = Command::new(bin).arg("status").output().await
        .map_err(|e| e.to_string())?;
    if output.status.success() && String::from_utf8_lossy(&output.stdout).contains("running") {
        Ok(())
    } else {
        Err("colima not running".to_string())
    }
}

async fn check_rancher_socket() -> std::result::Result<(), String> {
    let socket = home::home_dir().map(|h| h.join(".rd/run/containerd-shim.sock"));
    if socket.map_or(false, |s| s.exists()) {
        Ok(())
    } else {
        Err("Rancher Desktop socket not found".to_string())
    }
}

async fn check_lima_running_instance(bin: &Path) -> std::result::Result<String, String> {
    let output = Command::new(bin).args(["list", "--json"]).output().await
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                if val["status"].as_str() == Some("Running") {
                    return Ok(val["name"].as_str().unwrap_or("default").to_string());
                }
            }
        }
        Err("no running lima instance".to_string())
    } else {
        Err("limactl list failed".to_string())
    }
}
