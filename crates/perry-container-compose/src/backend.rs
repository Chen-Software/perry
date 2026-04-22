use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{PathBuf};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use serde_json::Value;
pub use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo};

#[derive(Debug, Clone, Default)]
pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: Option<crate::types::ListOrDict>,
    pub internal: bool,
    pub enable_ipv6: bool,
}

#[derive(Debug, Clone, Default)]
pub struct VolumeConfig {
    pub driver: Option<String>,
    pub labels: Option<crate::types::ListOrDict>,
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
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs>;
    async fn pull_image(&self, reference: &str) -> Result<()>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

pub trait CliProtocol: Send + Sync {
    fn protocol_name(&self) -> &'static str;
    fn subcommand_prefix(&self) -> Option<Vec<String>> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string(), "-d".to_string()];
        self.docker_run_flags(spec, &mut args);
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".to_string()];
        self.docker_run_flags(spec, &mut args);
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
        args
    }

    fn docker_run_flags(&self, spec: &ContainerSpec, args: &mut Vec<String>) {
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
    }

    fn parse_list_output(&self, stdout: &[u8]) -> Result<Vec<ContainerInfo>> {
        let v: Value = serde_json::from_slice(stdout)?;
        let mut result = Vec::new();
        if let Some(arr) = v.as_array() {
            for c in arr { result.push(self.parse_container_info_json(c)?); }
        }
        Ok(result)
    }

    fn parse_inspect_output(&self, stdout: &[u8]) -> Result<ContainerInfo> {
        let v: Value = serde_json::from_slice(stdout)?;
        let first = v.as_array().and_then(|a| a.first()).ok_or_else(|| ComposeError::NotFound("inspect failed".into()))?;
        self.parse_container_info_json(first)
    }

    fn parse_container_info_json(&self, json: &Value) -> Result<ContainerInfo> {
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

    fn parse_list_images_output(&self, stdout: &[u8]) -> Result<Vec<ImageInfo>> {
        let v: Value = serde_json::from_slice(stdout)?;
        let mut result = Vec::new();
        if let Some(arr) = v.as_array() {
            for i in arr { result.push(self.parse_image_info_json(i)?); }
        }
        Ok(result)
    }

    fn parse_image_info_json(&self, json: &Value) -> Result<ImageInfo> {
        let id = json["Id"].as_str().or(json["ID"].as_str()).unwrap_or("").to_string();
        Ok(ImageInfo { id, repository: json["Repository"].as_str().unwrap_or("").to_string(), tag: json["Tag"].as_str().unwrap_or("").to_string(), size: json["Size"].as_u64().unwrap_or(0), created: json["Created"].as_str().unwrap_or("").to_string() })
    }

    fn parse_image_inspect_output(&self, stdout: &[u8]) -> Result<ImageInfo> {
        let v: Value = serde_json::from_slice(stdout)?;
        let first = v.as_array().and_then(|a| a.first()).ok_or_else(|| ComposeError::NotFound("image inspect failed".into()))?;
        self.parse_image_info_json(first)
    }

    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(driver) = &config.driver { args.extend(["--driver".into(), driver.clone()]); }
        if config.internal { args.push("--internal".into()); }
        if config.enable_ipv6 { args.push("--ipv6".into()); }
        args.push(name.into());
        args
    }

    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(driver) = &config.driver { args.extend(["--driver".into(), driver.clone()]); }
        args.push(name.into());
        args
    }
}

pub struct DockerProtocol;
impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &'static str { "docker" }
}

pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &'static str { "apple/container" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string()];
        self.docker_run_flags(spec, &mut args);
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.clone());
        }
        args
    }
}

pub struct LimaProtocol { pub instance: String }
impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &'static str { "lima" }
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec!["shell".into(), self.instance.clone(), "nerdctl".into()])
    }
}

pub struct CliBackend<P: CliProtocol> {
    pub bin: PathBuf,
    pub name: &'static str,
    pub protocol: P,
}

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, name: &'static str, protocol: P) -> Self { Self { bin, name, protocol } }

    async fn exec_raw(&self, args: &[String]) -> Result<std::process::Output> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() {
            cmd.args(prefix);
        }
        cmd.args(args);
        let output = cmd.output().await.map_err(ComposeError::IoError)?;
        Ok(output)
    }

    async fn exec_ok(&self, args: &[String]) -> Result<std::process::Output> {
        let output = self.exec_raw(args).await?;
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
    fn backend_name(&self) -> &'static str {
        self.name
    }

    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("--version");
        let _ = timeout(Duration::from_secs(2), cmd.output()).await
            .map_err(|_| ComposeError::BackendNotAvailable { name: self.backend_name().into(), reason: "timeout".into() })?
            .map_err(ComposeError::IoError)?;
        Ok(())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.run_args(spec);
        let output = self.exec_ok(&args).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string(), name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.create_args(spec);
        let output = self.exec_ok(&args).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string(), name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> { self.exec_ok(&["start".into(), id.into()]).await?; Ok(()) }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout { args.extend(["--time".into(), t.to_string()]); }
        args.push(id.into());
        self.exec_ok(&args).await?;
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let mut args = vec!["rm".into()];
        if force { args.push("-f".into()); }
        args.push(id.into());
        self.exec_ok(&args).await?;
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all { args.push("-a".into()); }
        let output = self.exec_ok(&args).await?;
        self.protocol.parse_list_output(&output.stdout)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let output = self.exec_ok(&["inspect".into(), "--format".into(), "json".into(), id.into()]).await?;
        self.protocol.parse_inspect_output(&output.stdout)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut args = vec!["logs".into()];
        if let Some(n) = tail { args.extend(["--tail".into(), n.to_string()]); }
        args.push(id.into());
        let output = self.exec_ok(&args).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&output.stdout).into(), stderr: String::from_utf8_lossy(&output.stderr).into() })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let mut args = vec!["exec".into()];
        if let Some(e) = env { for (k, v) in e { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
        if let Some(w) = workdir { args.extend(["-w".into(), w.into()]); }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        let output = self.exec_ok(&args).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&output.stdout).into(), stderr: String::from_utf8_lossy(&output.stderr).into() })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> { self.exec_ok(&["pull".into(), reference.into()]).await?; Ok(()) }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let output = self.exec_ok(&["images".into(), "--format".into(), "json".into()]).await?;
        self.protocol.parse_list_images_output(&output.stdout)
    }

    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo> {
        let output = self.exec_ok(&["image".into(), "inspect".into(), "--format".into(), "json".into(), reference.into()]).await?;
        self.protocol.parse_image_inspect_output(&output.stdout)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let mut args = vec!["rmi".into()];
        if force { args.push("-f".into()); }
        args.push(reference.into());
        self.exec_ok(&args).await?;
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        let args = self.protocol.create_network_args(name, config);
        self.exec_ok(&args).await?; Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> { self.exec_ok(&["network".into(), "rm".into(), name.into()]).await?; Ok(()) }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        let args = self.protocol.create_volume_args(name, config);
        self.exec_ok(&args).await?; Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec_ok(&["volume".into(), "rm".into(), name.into()]).await?; Ok(()) }
}

pub type DockerBackend = CliBackend<DockerProtocol>;
pub type AppleBackend = CliBackend<AppleContainerProtocol>;
pub type LimaBackend = CliBackend<LimaProtocol>;

pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        match probe_specific(&name).await {
            Ok(b) => return Ok(b),
            Err(reason) => return Err(vec![BackendProbeResult { name, available: false, reason }]),
        }
    }
    let candidates: &[&str] = match std::env::consts::OS {
        "macos" | "ios" => &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"],
        "linux" => &["podman", "nerdctl", "docker"],
        _ => &["podman", "nerdctl", "docker"],
    };
    let mut results = Vec::new();
    for &name in candidates {
        match probe_candidate(name).await {
            Ok(b) => return Ok(b),
            Err(res) => results.push(res),
        }
    }
    Err(results)
}

async fn probe_specific(name: &str) -> std::result::Result<Box<dyn ContainerBackend>, String> {
    match probe_candidate(name).await {
        Ok(b) => Ok(b),
        Err(res) => Err(res.reason),
    }
}

async fn probe_candidate(name: &str) -> std::result::Result<Box<dyn ContainerBackend>, BackendProbeResult> {
    let check = match name {
        "apple/container" => ("container", vec!["--version"]),
        "orbstack" => ("orb", vec!["--version"]),
        "colima" => ("colima", vec!["--version"]),
        "rancher-desktop" => ("nerdctl", vec!["--version"]),
        "podman" => ("podman", vec!["--version"]),
        "lima" => ("limactl", vec!["--version"]),
        "nerdctl" => ("nerdctl", vec!["--version"]),
        "docker" => ("docker", vec!["--version"]),
        _ => return Err(BackendProbeResult { name: name.into(), available: false, reason: "unknown candidate".into() }),
    };
    let bin = match which::which(check.0) {
        Ok(p) => p,
        Err(_) => return Err(BackendProbeResult { name: name.into(), available: false, reason: format!("{} not found", check.0) }),
    };

    let mut cmd = Command::new(&bin);
    cmd.args(check.1);
    let probe_res = timeout(Duration::from_secs(2), cmd.output()).await;
    if !matches!(probe_res, Ok(Ok(ref output)) if output.status.success()) {
         return Err(BackendProbeResult { name: name.into(), available: false, reason: "CLI check failed".into() });
    }

    match name {
        "orbstack" => {
            let socket: Option<PathBuf> = home::home_dir().map(|h| h.join(".orbstack/run/docker.sock"));
            if !socket.map_or(false, |s: PathBuf| s.exists()) {
                return Err(BackendProbeResult { name: name.into(), available: false, reason: "OrbStack socket not found".into() });
            }
        }
        "colima" => {
            let mut cmd = Command::new(&bin);
            cmd.arg("status");
            let res = timeout(Duration::from_secs(2), cmd.output()).await;
            if !matches!(res, Ok(Ok(ref output)) if output.status.success() && String::from_utf8_lossy(&output.stdout).contains("running")) {
                return Err(BackendProbeResult { name: name.into(), available: false, reason: "colima status not running".into() });
            }
        }
        "rancher-desktop" => {
             let socket: Option<PathBuf> = home::home_dir().map(|h| h.join(".rd/run/containerd-shim.sock"));
             if !socket.map_or(false, |s: PathBuf| s.exists()) {
                 return Err(BackendProbeResult { name: name.into(), available: false, reason: "Rancher Desktop socket not found".into() });
             }
        }
        "podman" if std::env::consts::OS == "macos" => {
             let mut cmd = Command::new(&bin);
             cmd.args(["machine", "list", "--format", "json"]);
             let res = timeout(Duration::from_secs(2), cmd.output()).await;
             let running = if let Ok(Ok(output)) = res {
                 if let Ok(val) = serde_json::from_slice::<Value>(&output.stdout) {
                     val.as_array().map_or(false, |a| a.iter().any(|m| m["Running"].as_bool() == Some(true)))
                 } else { false }
             } else { false };
             if !running {
                 return Err(BackendProbeResult { name: name.into(), available: false, reason: "no running podman machine".into() });
             }
        }
        "lima" => {
             let mut cmd = Command::new(&bin);
             cmd.args(["list", "--json"]);
             let res = timeout(Duration::from_secs(2), cmd.output()).await;
             let running = if let Ok(Ok(output)) = res {
                 String::from_utf8_lossy(&output.stdout).lines().any(|line| {
                     if let Ok(val) = serde_json::from_str::<Value>(line) {
                         val["status"].as_str() == Some("Running")
                     } else { false }
                 })
             } else { false };
             if !running {
                 return Err(BackendProbeResult { name: name.into(), available: false, reason: "no running lima instance".into() });
             }
        }
        _ => {}
    }

    Ok(make_backend(name, bin))
}

fn make_backend(name: &str, bin: PathBuf) -> Box<dyn ContainerBackend> {
    match name {
        "apple/container" => Box::new(AppleBackend::new(bin, "apple/container", AppleContainerProtocol)),
        "lima" => Box::new(LimaBackend::new(bin, "lima", LimaProtocol { instance: "default".into() })),
        "orbstack" => Box::new(DockerBackend::new(bin, "orbstack", DockerProtocol)),
        "colima" => Box::new(DockerBackend::new(bin, "colima", DockerProtocol)),
        "rancher-desktop" => Box::new(DockerBackend::new(bin, "rancher-desktop", DockerProtocol)),
        "podman" => Box::new(DockerBackend::new(bin, "podman", DockerProtocol)),
        "nerdctl" => Box::new(DockerBackend::new(bin, "nerdctl", DockerProtocol)),
        "docker" => Box::new(DockerBackend::new(bin, "docker", DockerProtocol)),
        _ => Box::new(DockerBackend::new(bin, "docker", DockerProtocol)),
    }
}
