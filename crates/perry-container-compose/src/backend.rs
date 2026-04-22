use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use serde_json::Value;
pub use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{Container, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, ComposeServiceBuild, IsolationLevel, BackendMode, BackendInfo};
use tokio::sync::Mutex;
use std::sync::{Arc, OnceLock};

pub fn which_helper(bin: &str) -> std::result::Result<PathBuf, which::Error> {
    which::which(bin)
}

pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: Option<HashMap<String, String>>,
    pub internal: bool,
    pub enable_ipv6: bool,
}

pub struct VolumeConfig {
    pub driver: Option<String>,
    pub labels: Option<HashMap<String, String>>,
}

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn backend_name(&self) -> &str;
    async fn check_available(&self) -> Result<()>;
    async fn run(&self, spec: &Container) -> Result<ContainerHandle>;
    async fn create(&self, spec: &Container) -> Result<ContainerHandle>;
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
    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<()>;
    async fn inspect_network(&self, name: &str) -> Result<()>;
    fn strategy(&self) -> ExecutionStrategy;
    fn isolation_level(&self) -> IsolationLevel;
}

#[derive(Debug, Clone)]
pub enum ExecutionStrategy {
    CliExec { bin: PathBuf },
    ApiSocket { socket: PathBuf },
    VmSpawn { config: String }, // Placeholder
}

#[derive(Debug, Clone)]
pub enum BackendDriver {
    AppleContainer { bin: PathBuf },
    Orbstack { bin: PathBuf },
    Colima { bin: PathBuf },
    RancherDesktop { bin: PathBuf },
    Lima { bin: PathBuf },
    Podman { bin: PathBuf },
    Nerdctl { bin: PathBuf },
    Docker { bin: PathBuf },
}

impl BackendDriver {
    pub fn name(&self) -> &'static str {
        match self {
            Self::AppleContainer { .. } => "apple/container",
            Self::Orbstack { .. } => "orbstack",
            Self::Colima { .. } => "colima",
            Self::RancherDesktop { .. } => "rancher-desktop",
            Self::Lima { .. } => "lima",
            Self::Podman { .. } => "podman",
            Self::Nerdctl { .. } => "nerdctl",
            Self::Docker { .. } => "docker",
        }
    }
    pub fn bin(&self) -> &Path {
        match self {
            Self::AppleContainer { bin } => bin,
            Self::Orbstack { bin } => bin,
            Self::Colima { bin } => bin,
            Self::RancherDesktop { bin } => bin,
            Self::Lima { bin } => bin,
            Self::Podman { bin } => bin,
            Self::Nerdctl { bin } => bin,
            Self::Docker { bin } => bin,
        }
    }
    pub fn is_docker_compatible(&self) -> bool {
        !matches!(self, Self::AppleContainer { .. } | Self::Lima { .. })
    }
    pub fn isolation_level(&self) -> IsolationLevel {
        match self {
             Self::Orbstack { .. } | Self::Colima { .. } | Self::Lima { .. } => IsolationLevel::MicroVm,
             _ => IsolationLevel::Container,
        }
    }
}

pub trait CliProtocol: Send + Sync + std::fmt::Debug {
    fn protocol_name(&self) -> &str;
}

#[derive(Debug)]
pub struct DockerProtocol;
impl CliProtocol for DockerProtocol { fn protocol_name(&self) -> &str { "docker" } }

#[derive(Debug)]
pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol { fn protocol_name(&self) -> &str { "apple" } }

#[derive(Debug)]
pub struct LimaProtocol { pub instance: String }
impl CliProtocol for LimaProtocol { fn protocol_name(&self) -> &str { "lima" } }

pub struct CliBackend<P: CliProtocol> {
    pub bin: PathBuf,
    pub protocol: P,
    pub driver_name: &'static str,
    pub isolation: IsolationLevel,
}

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P, driver_name: &'static str, isolation: IsolationLevel) -> Self {
        Self { bin, protocol, driver_name, isolation }
    }
    async fn exec_cli(&self, args: &[String]) -> Result<std::process::Output> {
        let mut cmd = Command::new(&self.bin);
        cmd.args(args);
        let output = cmd.output().await.map_err(ComposeError::IoError)?;
        if !output.status.success() {
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(1),
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        Ok(output)
    }
}

#[async_trait]
impl<P: CliProtocol> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str { self.driver_name }
    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("--version");
        let _ = timeout(Duration::from_secs(2), cmd.output()).await
            .map_err(|_| ComposeError::BackendError { code: 125, message: "check_available timed out".into() })?
            .map_err(ComposeError::IoError)?;
        Ok(())
    }
    async fn run(&self, spec: &Container) -> Result<ContainerHandle> {
        let mut args = vec!["run".to_string(), "-d".to_string()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        if let Some(ports) = &spec.ports { for p in ports { args.extend(["-p".into(), p.clone()]); } }
        if let Some(volumes) = &spec.volumes { for v in volumes { args.extend(["-v".into(), v.clone()]); } }
        if let Some(env) = &spec.env { for (k, v) in env { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
        if let Some(network) = &spec.network { args.extend(["--network".into(), network.clone()]); }
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
        let output = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string() })
    }
    async fn create(&self, spec: &Container) -> Result<ContainerHandle> {
        let mut args = vec!["create".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        args.push(spec.image.clone());
        let output = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string() })
    }
    async fn start(&self, id: &str) -> Result<()> { self.exec_cli(&["start".into(), id.into()]).await?; Ok(()) }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout { args.extend(["--time".into(), t.to_string()]); }
        args.push(id.into());
        self.exec_cli(&args).await?;
        Ok(())
    }
    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let mut args = vec!["rm".into()];
        if force { args.push("-f".into()); }
        args.push(id.into());
        self.exec_cli(&args).await?;
        Ok(())
    }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all { args.push("-a".into()); }
        let output = self.exec_cli(&args).await?;
        let v: Value = serde_json::from_slice(&output.stdout).map_err(|e| ComposeError::InvalidConfig(e.to_string()))?;
        let mut result = Vec::new();
        if let Some(arr) = v.as_array() {
            for c in arr { result.push(parse_container_info_from_json(c)?); }
        }
        Ok(result)
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let output = self.exec_cli(&["inspect".into(), "--format".into(), "json".into(), id.into()]).await?;
        let v: Value = serde_json::from_slice(&output.stdout).map_err(|e| ComposeError::InvalidConfig(e.to_string()))?;
        let first = v.as_array().and_then(|a| a.first()).ok_or_else(|| ComposeError::NotFound(id.into()))?;
        parse_container_info_from_json(first)
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut args = vec!["logs".into()];
        if let Some(n) = tail { args.extend(["--tail".into(), n.to_string()]); }
        args.push(id.into());
        let output = self.exec_cli(&args).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&output.stdout).into(), stderr: String::from_utf8_lossy(&output.stderr).into() })
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let mut args = vec!["exec".into()];
        if let Some(e) = env { for (k, v) in e { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
        if let Some(w) = workdir { args.extend(["-w".into(), w.into()]); }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        let output = self.exec_cli(&args).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&output.stdout).into(), stderr: String::from_utf8_lossy(&output.stderr).into() })
    }
    async fn pull_image(&self, reference: &str) -> Result<()> { self.exec_cli(&["pull".into(), reference.into()]).await?; Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let output = self.exec_cli(&["images".into(), "--format".into(), "json".into()]).await?;
        let v: Value = serde_json::from_slice(&output.stdout).map_err(|e| ComposeError::InvalidConfig(e.to_string()))?;
        let mut result = Vec::new();
        if let Some(arr) = v.as_array() {
            for i in arr { result.push(parse_image_info_from_json(i)?); }
        }
        Ok(result)
    }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let mut args = vec!["rmi".into()];
        if force { args.push("-f".into()); }
        args.push(reference.into());
        self.exec_cli(&args).await?;
        Ok(())
    }
    async fn create_network(&self, name: &str, _config: &NetworkConfig) -> Result<()> { self.exec_cli(&["network".into(), "create".into(), name.into()]).await?; Ok(()) }
    async fn remove_network(&self, name: &str) -> Result<()> { self.exec_cli(&["network".into(), "rm".into(), name.into()]).await?; Ok(()) }
    async fn create_volume(&self, name: &str, _config: &VolumeConfig) -> Result<()> { self.exec_cli(&["volume".into(), "create".into(), name.into()]).await?; Ok(()) }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec_cli(&["volume".into(), "rm".into(), name.into()]).await?; Ok(()) }
    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<()> {
        let mut args = vec!["build".into(), "-t".into(), image_name.into()];
        if let Some(ctx) = &spec.context { args.push(ctx.clone()); } else { args.push(".".into()); }
        if let Some(df) = &spec.dockerfile { args.extend(["-f".into(), df.clone()]); }
        self.exec_cli(&args).await?;
        Ok(())
    }
    async fn inspect_network(&self, name: &str) -> Result<()> {
        self.exec_cli(&["network".into(), "inspect".into(), name.into()]).await?;
        Ok(())
    }
    fn strategy(&self) -> ExecutionStrategy { ExecutionStrategy::CliExec { bin: self.bin.clone() } }
    fn isolation_level(&self) -> IsolationLevel { self.isolation }
}

fn parse_container_info_from_json(json: &Value) -> Result<ContainerInfo> {
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

fn parse_image_info_from_json(json: &Value) -> Result<ImageInfo> {
    let id = json["Id"].as_str().or(json["ID"].as_str()).unwrap_or("").to_string();
    Ok(ImageInfo { id, repository: json["Repository"].as_str().unwrap_or("").to_string(), tag: json["Tag"].as_str().unwrap_or("").to_string(), size: json["Size"].as_u64().unwrap_or(0), created: json["Created"].as_str().unwrap_or("").to_string() })
}

static GLOBAL_BACKEND: OnceLock<Arc<Mutex<Option<Arc<dyn ContainerBackend + Send + Sync>>>>> = OnceLock::new();

pub async fn get_global_backend_instance() -> Result<Arc<dyn ContainerBackend + Send + Sync>> {
    let mutex = GLOBAL_BACKEND.get_or_init(|| Arc::new(Mutex::new(None)));
    let mut lock = mutex.lock().await;
    if let Some(b) = lock.as_ref() {
        return Ok(Arc::clone(b));
    }
    let b_boxed = detect_backend().await.map_err(|probed| ComposeError::NoBackendFound { probed })?;
    let b: Arc<dyn ContainerBackend + Send + Sync> = Arc::from(b_boxed);
    *lock = Some(Arc::clone(&b));
    Ok(b)
}

pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend + Send + Sync>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        let res = probe_candidate(&name).await;
        if res.available { return Ok(make_backend(&name, PathBuf::from(res.reason))); }
        return Err(vec![res]);
    }
    let candidates = platform_candidates();
    let mut results = Vec::new();
    for &name in candidates {
        let result = probe_candidate(name).await;
        if result.available { return Ok(make_backend(name, PathBuf::from(result.reason))); }
        results.push(result);
    }
    Err(results)
}

fn platform_candidates() -> &'static [&'static str] {
    match std::env::consts::OS {
        "macos" | "ios" => &["apple/container", "orbstack", "colima", "rancher-desktop", "lima", "podman", "nerdctl", "docker"],
        "linux" => &["podman", "nerdctl", "docker"],
        _ => &["podman", "nerdctl", "docker"],
    }
}

async fn probe_candidate(name: &str) -> BackendProbeResult {
    let check = match name {
        "apple/container" => ("container", vec!["--version"]),
        "orbstack" => ("orb", vec!["--version"]),
        "colima" => ("colima", vec!["--version"]),
        "rancher-desktop" => ("nerdctl", vec!["--version"]),
        "podman" => ("podman", vec!["--version"]),
        "lima" => ("limactl", vec!["--version"]),
        "nerdctl" => ("nerdctl", vec!["--version"]),
        "docker" => ("docker", vec!["--version"]),
        _ => return BackendProbeResult { name: name.into(), available: false, reason: "unknown candidate".into() },
    };
    let bin = match which_helper(check.0) {
        Ok(p) => p,
        Err(_) => return BackendProbeResult { name: name.into(), available: false, reason: format!("{} not found", check.0) },
    };

    let mut cmd = Command::new(&bin);
    cmd.args(check.1);
    let probe_res = timeout(Duration::from_secs(2), cmd.output()).await;
    if !matches!(probe_res, Ok(Ok(ref output)) if output.status.success()) {
         return BackendProbeResult { name: name.into(), available: false, reason: "CLI check failed".into() };
    }

    match name {
        "orbstack" => {
            let socket: Option<PathBuf> = home::home_dir().map(|h| h.join(".orbstack/run/docker.sock"));
            if socket.map_or(false, |s: PathBuf| s.exists()) {
                BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() }
            } else {
                BackendProbeResult { name: name.into(), available: false, reason: "OrbStack socket not found".into() }
            }
        }
        "colima" => {
            let mut cmd = Command::new(&bin);
            cmd.arg("status");
            let res = timeout(Duration::from_secs(2), cmd.output()).await;
            if matches!(res, Ok(Ok(ref output)) if output.status.success() && String::from_utf8_lossy(&output.stdout).contains("running")) {
                BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() }
            } else {
                BackendProbeResult { name: name.into(), available: false, reason: "colima status not running".into() }
            }
        }
        "rancher-desktop" => {
             let socket: Option<PathBuf> = home::home_dir().map(|h| h.join(".rd/run/containerd-shim.sock"));
             if socket.map_or(false, |s: PathBuf| s.exists()) {
                 BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() }
             } else {
                 BackendProbeResult { name: name.into(), available: false, reason: "Rancher Desktop socket not found".into() }
             }
        }
        "podman" if std::env::consts::OS == "macos" => {
             let mut cmd = Command::new(&bin);
             cmd.args(["machine", "list", "--format", "json"]);
             let res = timeout(Duration::from_secs(2), cmd.output()).await;
             if let Ok(Ok(output)) = res {
                 if let Ok(val) = serde_json::from_slice::<Value>(&output.stdout) {
                     if val.as_array().map_or(false, |a| a.iter().any(|m| m["Running"].as_bool() == Some(true))) {
                         return BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() };
                     }
                 }
             }
             BackendProbeResult { name: name.into(), available: false, reason: "no running podman machine".into() }
        }
        "lima" => {
             let mut cmd = Command::new(&bin);
             cmd.args(["list", "--json"]);
             let res = timeout(Duration::from_secs(2), cmd.output()).await;
             if let Ok(Ok(output)) = res {
                 for line in String::from_utf8_lossy(&output.stdout).lines() {
                     if let Ok(val) = serde_json::from_str::<Value>(line) {
                         if val["status"].as_str() == Some("Running") {
                             return BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() };
                         }
                     }
                 }
             }
             BackendProbeResult { name: name.into(), available: false, reason: "no running lima instance".into() }
        }
        _ => BackendProbeResult { name: name.into(), available: true, reason: bin.to_string_lossy().into() }
    }
}

fn make_backend(name: &str, bin: PathBuf) -> Box<dyn ContainerBackend + Send + Sync> {
    match name {
        "apple/container" => Box::new(CliBackend::new(bin, AppleContainerProtocol, "apple/container", IsolationLevel::Container)),
        "orbstack" => Box::new(CliBackend::new(bin, DockerProtocol, "orbstack", IsolationLevel::MicroVm)),
        "colima" => Box::new(CliBackend::new(bin, DockerProtocol, "colima", IsolationLevel::MicroVm)),
        "rancher-desktop" => Box::new(CliBackend::new(bin, DockerProtocol, "rancher-desktop", IsolationLevel::Container)),
        "podman" => Box::new(CliBackend::new(bin, DockerProtocol, "podman", IsolationLevel::Container)),
        "nerdctl" => Box::new(CliBackend::new(bin, DockerProtocol, "nerdctl", IsolationLevel::Container)),
        "docker" => Box::new(CliBackend::new(bin, DockerProtocol, "docker", IsolationLevel::Container)),
        "lima" => Box::new(CliBackend::new(bin, LimaProtocol { instance: "default".into() }, "lima", IsolationLevel::MicroVm)),
        _ => {
             let name_static: &'static str = Box::leak(name.to_string().into_boxed_str());
             Box::new(CliBackend::new(bin, DockerProtocol, name_static, IsolationLevel::Container))
        }
    }
}
