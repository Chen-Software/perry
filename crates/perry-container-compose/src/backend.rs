use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use serde_json::Value;
pub use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{ContainerSpec, ContainerHandle, ContainerInfo, ContainerLogs, ImageInfo, ComposeNetwork, ComposeVolume};

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn name(&self) -> &'static str;
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
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

pub struct AppleContainerBackend { pub bin: PathBuf }
pub struct PodmanBackend { pub bin: PathBuf }

impl AppleContainerBackend {
    pub fn new(bin: PathBuf) -> Self { Self { bin } }
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
impl ContainerBackend for AppleContainerBackend {
    fn name(&self) -> &'static str { "apple/container" }
    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("--version");
        let _ = timeout(Duration::from_secs(2), cmd.output()).await
            .map_err(|_| ComposeError::BackendError { code: 125, message: "check_available timed out".into() })?
            .map_err(ComposeError::IoError)?;
        Ok(())
    }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut args = vec!["run".to_string()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
        let output = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string(), name: spec.name.clone() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut args = vec!["create".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        args.push(spec.image.clone());
        let output = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string(), name: spec.name.clone() })
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
        let v: Value = serde_json::from_slice(&output.stdout)?;
        let mut result = Vec::new();
        if let Some(arr) = v.as_array() {
            for c in arr { result.push(parse_container_info_from_json(c)?); }
        }
        Ok(result)
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let output = self.exec_cli(&["inspect".into(), "--format".into(), "json".into(), id.into()]).await?;
        let v: Value = serde_json::from_slice(&output.stdout)?;
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
        let v: Value = serde_json::from_slice(&output.stdout)?;
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
    async fn create_network(&self, name: &str, _config: &ComposeNetwork) -> Result<()> { self.exec_cli(&["network".into(), "create".into(), name.into()]).await?; Ok(()) }
    async fn remove_network(&self, name: &str) -> Result<()> { self.exec_cli(&["network".into(), "rm".into(), name.into()]).await?; Ok(()) }
    async fn create_volume(&self, name: &str, _config: &ComposeVolume) -> Result<()> { self.exec_cli(&["volume".into(), "create".into(), name.into()]).await?; Ok(()) }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec_cli(&["volume".into(), "rm".into(), name.into()]).await?; Ok(()) }
}

impl PodmanBackend {
    pub fn new(bin: PathBuf) -> Self { Self { bin } }
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
impl ContainerBackend for PodmanBackend {
    fn name(&self) -> &'static str { "podman" }
    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("--version");
        let _ = timeout(Duration::from_secs(2), cmd.output()).await
            .map_err(|_| ComposeError::BackendError { code: 125, message: "check_available timed out".into() })?
            .map_err(ComposeError::IoError)?;
        Ok(())
    }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut args = vec!["run".to_string(), "-d".to_string()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        if let Some(ports) = &spec.ports { for p in ports { args.extend(["-p".into(), p.clone()]); } }
        if let Some(volumes) = &spec.volumes { for v in volumes { args.extend(["-v".into(), v.clone()]); } }
        if let Some(env) = &spec.env { for (k, v) in env { args.extend(["-e".into(), format!("{}={}", k, v)]); } }
        if let Some(network) = &spec.network { args.extend(["--network".into(), network.clone()]); }
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        if spec.read_only.unwrap_or(false) { args.push("--read-only".into()); }
        if let Some(entrypoint) = &spec.entrypoint { args.extend(["--entrypoint".into(), entrypoint.join(" ")]); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }
        let output = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string(), name: spec.name.clone() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut args = vec!["create".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        args.push(spec.image.clone());
        let output = self.exec_cli(&args).await?;
        Ok(ContainerHandle { id: String::from_utf8_lossy(&output.stdout).trim().to_string(), name: spec.name.clone() })
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
        let v: Value = serde_json::from_slice(&output.stdout)?;
        let mut result = Vec::new();
        if let Some(arr) = v.as_array() {
            for c in arr { result.push(parse_container_info_from_json(c)?); }
        }
        Ok(result)
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let output = self.exec_cli(&["inspect".into(), "--format".into(), "json".into(), id.into()]).await?;
        let v: Value = serde_json::from_slice(&output.stdout)?;
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
        let v: Value = serde_json::from_slice(&output.stdout)?;
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
    async fn create_network(&self, name: &str, _config: &ComposeNetwork) -> Result<()> { self.exec_cli(&["network".into(), "create".into(), name.into()]).await?; Ok(()) }
    async fn remove_network(&self, name: &str) -> Result<()> { self.exec_cli(&["network".into(), "rm".into(), name.into()]).await?; Ok(()) }
    async fn create_volume(&self, name: &str, _config: &ComposeVolume) -> Result<()> { self.exec_cli(&["volume".into(), "create".into(), name.into()]).await?; Ok(()) }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec_cli(&["volume".into(), "rm".into(), name.into()]).await?; Ok(()) }
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

pub async fn get_backend() -> std::result::Result<Box<dyn ContainerBackend>, Vec<BackendProbeResult>> {
    let os = std::env::consts::OS;
    let candidates = match os {
        "macos" | "ios" => vec!["container", "podman", "docker"],
        _ => vec!["podman", "docker"],
    };

    let mut probed = Vec::new();
    for name in candidates {
        if let Ok(bin) = which::which(name) {
            match name {
                "container" => return Ok(Box::new(AppleContainerBackend::new(bin))),
                "podman" | "docker" => return Ok(Box::new(PodmanBackend::new(bin))),
                _ => {}
            }
        } else {
            probed.push(BackendProbeResult {
                name: name.to_string(),
                available: false,
                reason: format!("{} not found in PATH", name),
            });
        }
    }
    Err(probed)
}
