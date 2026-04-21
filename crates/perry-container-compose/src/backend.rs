use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use serde_json::Value;
pub use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo};
use std::sync::Arc;

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

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn backend_name(&self) -> String;
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
    async fn wait(&self, id: &str) -> Result<i32>;
}

pub trait CliProtocol: Send + Sync {
    fn protocol_name(&self) -> &'static str;
    fn subcommand_prefix(&self) -> Option<Vec<String>> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".to_string(), "-d".to_string()];
        args.extend(self.common_run_flags(spec));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
        args
    }

    fn common_run_flags(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        if let Some(ports) = &spec.ports { for p in ports { args.extend(["-p".into(), p.clone()]); } }
        if let Some(volumes) = &spec.volumes { for v in volumes { args.extend(["-v".into(), v.clone()]); } }
        if let Some(env) = &spec.env { for (k, v) in env { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
        if let Some(network) = &spec.network { args.extend(["--network".into(), network.clone()]); }
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        if spec.read_only.unwrap_or(false) { args.push("--read-only".into()); }
        if let Some(entrypoint) = &spec.entrypoint { args.extend(["--entrypoint".into(), entrypoint.join(" ")]); }
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".into()];
        args.extend(self.common_run_flags(spec));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
        args
    }

    fn start_args(&self, id: &str) -> Vec<String> { vec!["start".into(), id.into()] }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout { args.extend(["--time".into(), t.to_string()]); }
        args.push(id.into());
        args
    }
    fn rm_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".into()];
        if force { args.push("-f".into()); }
        args.push(id.into());
        args
    }
    fn ps_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all { args.push("-a".into()); }
        args
    }
    fn inspect_args(&self, id: &str) -> Vec<String> { vec!["inspect".into(), "--format".into(), "json".into(), id.into()] }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if let Some(n) = tail { args.extend(["--tail".into(), n.to_string()]); }
        args.push(id.into());
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(e) = env { for (k, v) in e { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
        if let Some(w) = workdir { args.extend(["-w".into(), w.into()]); }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        args
    }
    fn pull_args(&self, reference: &str) -> Vec<String> { vec!["pull".into(), reference.into()] }
    fn images_args(&self) -> Vec<String> { vec!["images".into(), "--format".into(), "json".into()] }
    fn rmi_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".into()];
        if force { args.push("-f".into()); }
        args.push(reference.into());
        args
    }
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(driver) = &config.driver { args.extend(["--driver".into(), driver.clone()]); }
        if let Some(labels) = &config.labels { for (k, v) in labels { args.extend(["--label".into(), format!("{}={}", k, v)]); } }
        if config.internal { args.push("--internal".into()); }
        args.push(name.into());
        args
    }
    fn rm_network_args(&self, name: &str) -> Vec<String> { vec!["network".into(), "rm".into(), name.into()] }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(driver) = &config.driver { args.extend(["--driver".into(), driver.clone()]); }
        if let Some(labels) = &config.labels { for (k, v) in labels { args.extend(["--label".into(), format!("{}={}", k, v)]); } }
        args.push(name.into());
        args
    }
    fn rm_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".into(), "rm".into(), name.into()] }
    fn wait_args(&self, id: &str) -> Vec<String> { vec!["wait".into(), id.into()] }
    fn build_args(&self, context: &str, dockerfile: Option<&str>, tags: &[String]) -> Vec<String> {
        let mut args = vec!["build".into()];
        if let Some(df) = dockerfile { args.extend(["-f".into(), df.into()]); }
        for t in tags { args.extend(["-t".into(), t.clone()]); }
        args.push(context.into());
        args
    }

    fn parse_container_info(&self, json: &Value) -> Result<ContainerInfo> {
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

    fn parse_image_info(&self, json: &Value) -> Result<ImageInfo> {
        let id = json["Id"].as_str().or(json["ID"].as_str()).unwrap_or("").to_string();
        Ok(ImageInfo { id, repository: json["Repository"].as_str().unwrap_or("").to_string(), tag: json["Tag"].as_str().unwrap_or("").to_string(), size: json["Size"].as_u64().unwrap_or(0), created: json["Created"].as_str().unwrap_or("").to_string() })
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
        args.extend(self.common_run_flags(spec));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
        args
    }
    fn build_args(&self, context: &str, dockerfile: Option<&str>, tags: &[String]) -> Vec<String> {
        let mut args = vec!["build".into(), "--cpus".into(), "2".into(), "--memory".into(), "2048MB".into(), "--arch".into(), "arm64".into(), "--os".into(), "linux".into()];
        if let Some(df) = dockerfile { args.extend(["-f".into(), df.into()]); }
        for t in tags { args.extend(["-t".into(), t.clone()]); }
        args.push(context.into());
        args
    }
    fn parse_container_info(&self, json: &Value) -> Result<ContainerInfo> {
        let config = &json["configuration"];
        let id = config["id"].as_str().unwrap_or("").to_string();
        let name = json["labels"]["com.apple.container.name"].as_str()
            .or(config["labels"]["com.apple.container.name"].as_str())
            .unwrap_or(&id).to_string();
        let image = config["image"]["reference"].as_str().unwrap_or("").to_string();
        let status = json["status"].as_str().unwrap_or("").to_string();
        Ok(ContainerInfo { id, name, image, status, ports: Vec::new(), created: "".to_string() })
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
    pub protocol: P,
}

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P) -> Self { Self { bin, protocol } }
    async fn exec_raw(&self, args: &[String]) -> Result<std::process::Output> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() {
            cmd.args(prefix);
        }
        cmd.args(args);
        cmd.output().await.map_err(ComposeError::IoError)
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
    fn backend_name(&self) -> String { self.bin.file_name().unwrap_or_default().to_string_lossy().to_string() }
    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("--version");
        let _ = timeout(Duration::from_secs(2), cmd.output()).await
            .map_err(|_| ComposeError::BackendError { code: 125, message: "check_available timed out".into() })?
            .map_err(ComposeError::IoError)?;
        Ok(())
    }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let output = self.exec_ok(&self.protocol.run_args(spec)).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string(), name: spec.name.clone() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let output = self.exec_ok(&self.protocol.create_args(spec)).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string(), name: spec.name.clone() })
    }
    async fn start(&self, id: &str) -> Result<()> { self.exec_ok(&self.protocol.start_args(id)).await?; Ok(()) }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> { self.exec_ok(&self.protocol.stop_args(id, timeout)).await?; Ok(()) }
    async fn remove(&self, id: &str, force: bool) -> Result<()> { self.exec_ok(&self.protocol.rm_args(id, force)).await?; Ok(()) }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let output = self.exec_ok(&self.protocol.ps_args(all)).await?;
        let v: Value = serde_json::from_slice(&output.stdout)?;
        let mut result = Vec::new();
        if let Some(arr) = v.as_array() {
            for c in arr { result.push(self.protocol.parse_container_info(c)?); }
        }
        Ok(result)
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let output = self.exec_ok(&self.protocol.inspect_args(id)).await?;
        let v: Value = serde_json::from_slice(&output.stdout)?;
        let first = v.as_array().and_then(|a| a.first()).ok_or_else(|| ComposeError::NotFound(id.into()))?;
        self.protocol.parse_container_info(first)
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let output = self.exec_ok(&self.protocol.logs_args(id, tail)).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&output.stdout).into(), stderr: String::from_utf8_lossy(&output.stderr).into() })
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let output = self.exec_ok(&self.protocol.exec_args(id, cmd, env, workdir)).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&output.stdout).into(), stderr: String::from_utf8_lossy(&output.stderr).into() })
    }
    async fn pull_image(&self, reference: &str) -> Result<()> { self.exec_ok(&self.protocol.pull_args(reference)).await?; Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let output = self.exec_ok(&self.protocol.images_args()).await?;
        let v: Value = serde_json::from_slice(&output.stdout)?;
        let mut result = Vec::new();
        if let Some(arr) = v.as_array() {
            for i in arr { result.push(self.protocol.parse_image_info(i)?); }
        }
        Ok(result)
    }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> { self.exec_ok(&self.protocol.rmi_args(reference, force)).await?; Ok(()) }
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> { self.exec_ok(&self.protocol.create_network_args(name, config)).await?; Ok(()) }
    async fn remove_network(&self, name: &str) -> Result<()> { self.exec_ok(&self.protocol.rm_network_args(name)).await?; Ok(()) }
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> { self.exec_ok(&self.protocol.create_volume_args(name, config)).await?; Ok(()) }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec_ok(&self.protocol.rm_volume_args(name)).await?; Ok(()) }
    async fn wait(&self, id: &str) -> Result<i32> {
        let output = self.exec_ok(&self.protocol.wait_args(id)).await?;
        let code = String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(0);
        Ok(code)
    }
}

pub type DockerBackend = CliBackend<DockerProtocol>;
pub type AppleBackend = CliBackend<AppleContainerProtocol>;
pub type LimaBackend = CliBackend<LimaProtocol>;

pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend>, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        let res = probe_candidate(&name).await;
        if res.available { return Ok(make_backend(&name, PathBuf::from(res.reason))); }
        return Err(vec![res]);
    }
    let candidates: &[&str] = match std::env::consts::OS {
        "macos" | "ios" => &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"],
        "linux" => &["podman", "nerdctl", "docker"],
        _ => &["podman", "nerdctl", "docker"],
    };
    let mut results = Vec::new();
    for &name in candidates {
        let result = probe_candidate(name).await;
        if result.available { return Ok(make_backend(name, PathBuf::from(result.reason))); }
        results.push(result);
    }
    Err(results)
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
    let bin = match which::which(check.0) {
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
            let socket = home::home_dir().map(|h| h.join(".orbstack/run/docker.sock"));
            if socket.map_or(false, |s| s.exists()) {
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
             let socket = home::home_dir().map(|h| h.join(".rd/run/containerd-shim.sock"));
             if socket.map_or(false, |s| s.exists()) {
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

fn make_backend(name: &str, bin: PathBuf) -> Box<dyn ContainerBackend> {
    match name {
        "apple/container" => Box::new(CliBackend::new(bin, AppleContainerProtocol)),
        "lima" => Box::new(CliBackend::new(bin, LimaProtocol { instance: "default".into() })),
        _ => Box::new(CliBackend::new(bin, DockerProtocol)),
    }
}

pub struct MockBackend;
#[async_trait]
impl ContainerBackend for MockBackend {
    fn backend_name(&self) -> String { "mock".into() }
    async fn check_available(&self) -> Result<()> { Ok(()) }
    async fn run(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> { Ok(ContainerHandle { id: "mock".into(), name: None }) }
    async fn create(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> { Ok(ContainerHandle { id: "mock".into(), name: None }) }
    async fn start(&self, _id: &str) -> Result<()> { Ok(()) }
    async fn stop(&self, _id: &str, _timeout: Option<u32>) -> Result<()> { Ok(()) }
    async fn remove(&self, _id: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> { Ok(vec![]) }
    async fn inspect(&self, _id: &str) -> Result<ContainerInfo> { Err(ComposeError::NotFound("mock".into())) }
    async fn logs(&self, _id: &str, _tail: Option<u32>) -> Result<ContainerLogs> { Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }) }
    async fn exec(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> { Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }) }
    async fn pull_image(&self, _reference: &str) -> Result<()> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(vec![]) }
    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn create_network(&self, _name: &str, _config: &NetworkConfig) -> Result<()> { Ok(()) }
    async fn remove_network(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn create_volume(&self, _name: &str, _config: &VolumeConfig) -> Result<()> { Ok(()) }
    async fn remove_volume(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn wait(&self, _id: &str) -> Result<i32> { Ok(0) }
}
