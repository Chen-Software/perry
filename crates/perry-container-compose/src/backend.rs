//! Container backend abstraction — `ContainerBackend` trait, `CliProtocol` trait,
//! protocol implementations (`DockerProtocol`, `AppleContainerProtocol`, `LimaProtocol`),
//! generic `CliBackend<P>`, and `detect_backend()`.

use crate::error::{ComposeError, Result};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

// ─────────────────────────────────────────────────────────────────────────────
// 4.8  BackendProbeResult — defined in error.rs, re-exported here
// ─────────────────────────────────────────────────────────────────────────────
pub use crate::error::BackendProbeResult;

// ─────────────────────────────────────────────────────────────────────────────
// 4.1  NetworkConfig and VolumeConfig — lean config structs
// ─────────────────────────────────────────────────────────────────────────────

/// Lean network configuration decoupled from compose-spec types.
#[derive(Debug, Clone, Default)]
pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
    pub internal: bool,
    pub enable_ipv6: bool,
}

/// Lean volume configuration decoupled from compose-spec types.
#[derive(Debug, Clone, Default)]
pub struct VolumeConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversions from compose-spec types to lean config types
// ─────────────────────────────────────────────────────────────────────────────

impl From<&ComposeNetwork> for NetworkConfig {
    fn from(n: &ComposeNetwork) -> Self {
        NetworkConfig {
            driver: n.driver.clone(),
            labels: n.labels.as_ref().map(|l| l.to_map()).unwrap_or_default(),
            internal: n.internal.unwrap_or(false),
            enable_ipv6: n.enable_ipv6.unwrap_or(false),
        }
    }
}

impl From<&ComposeVolume> for VolumeConfig {
    fn from(v: &ComposeVolume) -> Self {
        VolumeConfig {
            driver: v.driver.clone(),
            labels: v.labels.as_ref().map(|l| l.to_map()).unwrap_or_default(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.1  ContainerBackend trait
// ─────────────────────────────────────────────────────────────────────────────

/// Runtime-agnostic async interface for container operations.
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
    async fn wait(&self, id: &str) -> Result<()>;
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

// ─────────────────────────────────────────────────────────────────────────────
// Shared JSON deserialization helpers (Docker-compatible output format)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DockerListEntry {
    #[serde(rename = "ID", alias = "Id", default)]
    id: String,
    #[serde(rename = "Names", alias = "names", default)]
    names: serde_json::Value,
    #[serde(rename = "Image", alias = "image", default)]
    image: String,
    #[serde(rename = "Status", alias = "status", default)]
    status: String,
    #[serde(rename = "Ports", alias = "ports", default)]
    ports: serde_json::Value,
    #[serde(rename = "Created", alias = "created", default)]
    created: serde_json::Value,
}

impl DockerListEntry {
    fn into_container_info(self) -> ContainerInfo {
        let name = match &self.names {
            serde_json::Value::Array(arr) => arr
                .first()
                .and_then(|v| v.as_str())
                .map(|s| s.trim_start_matches('/').to_string())
                .unwrap_or_default(),
            serde_json::Value::String(s) => s.trim_start_matches('/').to_string(),
            _ => String::new(),
        };
        let ports = match &self.ports {
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            serde_json::Value::String(s) if !s.is_empty() => vec![s.clone()],
            _ => vec![],
        };
        let created = match &self.created {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        };
        ContainerInfo {
            id: self.id,
            name,
            image: self.image,
            status: self.status,
            ports,
            created,
        }
    }
}

#[derive(Debug, Deserialize)]
struct DockerInspectEntry {
    #[serde(rename = "Id", alias = "ID", default)]
    id: String,
    #[serde(rename = "Name", alias = "name", default)]
    name: String,
    #[serde(rename = "Image", alias = "image", default)]
    image: String,
    #[serde(rename = "State", alias = "state")]
    state: Option<DockerInspectState>,
    #[serde(rename = "Created", alias = "created", default)]
    created: String,
}

#[derive(Debug, Deserialize)]
struct DockerInspectState {
    #[serde(rename = "Running", alias = "running", default)]
    running: bool,
    #[serde(rename = "Status", alias = "status", default)]
    status: String,
}

#[derive(Debug, Deserialize)]
struct DockerImageEntry {
    #[serde(rename = "ID", alias = "Id", default)]
    id: String,
    #[serde(rename = "Repository", alias = "repository", default)]
    repository: String,
    #[serde(rename = "Tag", alias = "tag", default)]
    tag: String,
    #[serde(rename = "Size", alias = "size", default)]
    size: serde_json::Value,
    #[serde(rename = "Created", alias = "created", default)]
    created: String,
}

fn parse_size(v: &serde_json::Value) -> u64 {
    match v {
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0),
        serde_json::Value::String(s) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

fn is_not_found(stderr: &str) -> bool {
    let s = stderr.to_lowercase();
    s.contains("not found")
        || s.contains("no such")
        || s.contains("does not exist")
        || s.contains("unknown container")
}

/// Build the common Docker-compatible `run`/`create` flags from a `ContainerSpec`.
/// When `include_detach` is true, `--detach` is added (Docker/podman/nerdctl).
/// When false (apple/container), it is omitted.
pub fn docker_run_flags(spec: &ContainerSpec, include_detach: bool) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if spec.rm.unwrap_or(false) {
        args.push("--rm".into());
    }
    if include_detach {
        args.push("--detach".into());
    }
    if let Some(name) = &spec.name {
        args.push("--name".into());
        args.push(name.clone());
    }
    if let Some(network) = &spec.network {
        args.push("--network".into());
        args.push(network.clone());
    }
    if let Some(ports) = &spec.ports {
        for p in ports {
            args.push("-p".into());
            args.push(p.clone());
        }
    }
    if let Some(vols) = &spec.volumes {
        for v in vols {
            args.push("-v".into());
            args.push(v.clone());
        }
    }
    if let Some(envs) = &spec.env {
        let mut pairs: Vec<(&String, &String)> = envs.iter().collect();
        pairs.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in pairs {
            args.push("-e".into());
            args.push(format!("{}={}", k, v));
        }
    }
    if let Some(ep) = &spec.entrypoint {
        args.push("--entrypoint".into());
        args.push(ep.join(" "));
    }
    args
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.2  CliProtocol trait with Docker-compatible defaults
// ─────────────────────────────────────────────────────────────────────────────

/// Translates abstract container operations into CLI arguments for a specific
/// runtime family, and parses the CLI's JSON output back into typed structs.
///
/// Every method has a Docker-compatible default. Only `protocol_name()` is
/// required. New protocols override only what differs.
pub trait CliProtocol: Send + Sync {
    /// Human-readable protocol name (e.g. `"docker-compatible"`, `"apple/container"`).
    fn protocol_name(&self) -> &str;

    /// Optional prefix inserted before every subcommand.
    /// `LimaProtocol` returns `Some(["shell", "<instance>", "nerdctl"])`.
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        None
    }

    // ── Argument builders (Docker-compatible defaults) ─────────────────────

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into()];
        args.extend(docker_run_flags(spec, true));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.iter().cloned());
        }
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".into()];
        args.extend(docker_run_flags(spec, false));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.iter().cloned());
        }
        args
    }

    fn start_args(&self, id: &str) -> Vec<String> {
        vec!["start".into(), id.into()]
    }

    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout {
            args.push("-t".into());
            args.push(t.to_string());
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
            args.push("--all".into());
        }
        args
    }

    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".into(), "--format".into(), "json".into(), id.into()]
    }

    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if let Some(t) = tail {
            args.push("--tail".into());
            args.push(t.to_string());
        }
        args.push(id.into());
        args
    }

    fn wait_args(&self, id: &str) -> Vec<String> {
        vec!["wait".into(), id.into()]
    }

    fn exec_args(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(wd) = workdir {
            args.push("--workdir".into());
            args.push(wd.into());
        }
        if let Some(envs) = env {
            let mut pairs: Vec<(&String, &String)> = envs.iter().collect();
            pairs.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in pairs {
                args.push("-e".into());
                args.push(format!("{}={}", k, v));
            }
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
            args.push("--driver".into());
            args.push(d.clone());
        }
        let mut pairs: Vec<(&String, &String)> = config.labels.iter().collect();
        pairs.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in pairs {
            args.push("--label".into());
            args.push(format!("{}={}", k, v));
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
            args.push("--driver".into());
            args.push(d.clone());
        }
        let mut pairs: Vec<(&String, &String)> = config.labels.iter().collect();
        pairs.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in pairs {
            args.push("--label".into());
            args.push(format!("{}={}", k, v));
        }
        args.push(name.into());
        args
    }

    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        vec!["volume".into(), "rm".into(), name.into()]
    }

    // ── Output parsers (Docker JSON defaults) ─────────────────────────────

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        let trimmed = stdout.trim();
        if trimmed.starts_with('[') {
            serde_json::from_str::<Vec<DockerListEntry>>(trimmed)
                .unwrap_or_default()
                .into_iter()
                .map(|e| e.into_container_info())
                .collect()
        } else {
            trimmed
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<DockerListEntry>(l).ok())
                .map(|e| e.into_container_info())
                .collect()
        }
    }

    fn parse_inspect_output(&self, id: &str, stdout: &str) -> Option<ContainerInfo> {
        let trimmed = stdout.trim();
        let entry: Option<DockerInspectEntry> = if trimmed.starts_with('[') {
            serde_json::from_str::<Vec<DockerInspectEntry>>(trimmed)
                .ok()
                .and_then(|v| v.into_iter().next())
        } else {
            serde_json::from_str::<DockerInspectEntry>(trimmed).ok()
        };
        entry.map(|e| {
            let running = e.state.as_ref().map(|s| s.running).unwrap_or(false);
            let status = e
                .state
                .as_ref()
                .map(|s| s.status.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| if running { "running" } else { "stopped" }.into());
            ContainerInfo {
                id: if e.id.is_empty() { id.to_string() } else { e.id },
                name: e.name.trim_start_matches('/').to_string(),
                image: e.image,
                status,
                ports: vec![],
                created: e.created,
            }
        })
    }

    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> {
        let trimmed = stdout.trim();
        let entries: Vec<DockerImageEntry> = if trimmed.starts_with('[') {
            serde_json::from_str(trimmed).unwrap_or_default()
        } else {
            trimmed
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        };
        entries
            .into_iter()
            .map(|e| ImageInfo {
                id: e.id,
                repository: e.repository,
                tag: e.tag,
                size: parse_size(&e.size),
                created: e.created,
            })
            .collect()
    }

    fn parse_container_id(&self, stdout: &str) -> String {
        stdout.trim().to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.3  DockerProtocol
// ─────────────────────────────────────────────────────────────────────────────

/// `CliProtocol` for Docker-compatible runtimes: docker, podman, nerdctl,
/// orbstack, colima. All methods use the trait defaults.
pub struct DockerProtocol;

impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &str {
        "docker-compatible"
    }
    // All other methods inherit Docker-compatible defaults from the trait.
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.4  AppleContainerProtocol
// ─────────────────────────────────────────────────────────────────────────────

/// `CliProtocol` for the `apple/container` CLI on macOS/iOS.
///
/// The only difference from Docker: `run` does not support `--detach`.
pub struct AppleContainerProtocol;

impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str {
        "apple/container"
    }

    /// `apple/container run` does not accept `--detach`; omit it.
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into()];
        args.extend(docker_run_flags(spec, false));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.iter().cloned());
        }
        args
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.5  LimaProtocol
// ─────────────────────────────────────────────────────────────────────────────

/// `CliProtocol` for Lima. Wraps every command with `limactl shell <instance> nerdctl`.
pub struct LimaProtocol {
    pub instance: String,
}

impl LimaProtocol {
    pub fn new(instance: impl Into<String>) -> Self {
        LimaProtocol {
            instance: instance.into(),
        }
    }
}

impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &str {
        "lima"
    }

    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec![
            "shell".into(),
            self.instance.clone(),
            "nerdctl".into(),
        ])
    }
    // All other methods inherit Docker-compatible defaults from the trait.
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.6  Generic CliBackend<P>
// ─────────────────────────────────────────────────────────────────────────────

/// Concrete `ContainerBackend` that executes CLI commands via
/// `tokio::process::Command`. Generic over `P: CliProtocol` — zero vtable
/// overhead, monomorphised at compile time.
pub struct CliBackend<P: CliProtocol> {
    pub bin: PathBuf,
    pub protocol: P,
}

/// Type aliases for the common backends.
pub type DockerBackend = CliBackend<DockerProtocol>;
pub type AppleBackend = CliBackend<AppleContainerProtocol>;
pub type LimaBackend = CliBackend<LimaProtocol>;

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P) -> Self {
        CliBackend { bin, protocol }
    }

    /// Build the full argument list, prepending the protocol's subcommand
    /// prefix (e.g. `["shell", "default", "nerdctl"]` for Lima) when present.
    pub fn full_args(&self, subcommand_args: Vec<String>) -> Vec<String> {
        match self.protocol.subcommand_prefix() {
            Some(prefix) => {
                let mut full = prefix;
                full.extend(subcommand_args);
                full
            }
            None => subcommand_args,
        }
    }

    /// Execute the binary with the given arguments and return the raw output.
    async fn exec_raw(&self, args: Vec<String>) -> Result<std::process::Output> {
        let full = self.full_args(args);
        let output = Command::new(&self.bin)
            .args(&full)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(ComposeError::IoError)?;
        Ok(output)
    }

    /// Execute and return stdout as a `String`, mapping non-zero exit codes to
    /// `ComposeError::BackendError`.
    async fn exec_ok(&self, args: Vec<String>) -> Result<String> {
        let output = self.exec_raw(args).await?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            })
        }
    }
}

#[async_trait]
impl<P: CliProtocol + Send + Sync> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str {
        self.bin
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    }

    async fn check_available(&self) -> Result<()> {
        let output = Command::new(&self.bin)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(ComposeError::IoError)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: format!(
                    "'{}' not available: {}",
                    self.backend_name(),
                    String::from_utf8_lossy(&output.stderr)
                ),
            })
        }
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.run_args(spec);
        let stdout = self.exec_ok(args).await?;
        let id = self.protocol.parse_container_id(&stdout);
        let name = spec.name.clone().or_else(|| Some(id.clone()));
        Ok(ContainerHandle { id, name })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.create_args(spec);
        let stdout = self.exec_ok(args).await?;
        let id = self.protocol.parse_container_id(&stdout);
        let name = spec.name.clone().or_else(|| Some(id.clone()));
        Ok(ContainerHandle { id, name })
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
        let output = self.exec_raw(self.protocol.inspect_args(id)).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if is_not_found(&stderr) {
                return Err(ComposeError::NotFound(id.to_string()));
            }
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        self.protocol
            .parse_inspect_output(id, &stdout)
            .ok_or_else(|| ComposeError::NotFound(id.to_string()))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let output = self.exec_raw(self.protocol.logs_args(id, tail)).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn wait(&self, id: &str) -> Result<()> {
        self.exec_ok(self.protocol.wait_args(id)).await?;
        Ok(())
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        let output = self
            .exec_raw(self.protocol.exec_args(id, cmd, env, workdir))
            .await?;
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
        self.exec_ok(self.protocol.remove_image_args(reference, force))
            .await?;
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        self.exec_ok(self.protocol.create_network_args(name, config))
            .await?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let output = self
            .exec_raw(self.protocol.remove_network_args(name))
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if is_not_found(&stderr) {
                return Ok(());
            }
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            });
        }
        Ok(())
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        self.exec_ok(self.protocol.create_volume_args(name, config))
            .await?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let output = self
            .exec_raw(self.protocol.remove_volume_args(name))
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if is_not_found(&stderr) {
                return Ok(());
            }
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            });
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.7  detect_backend() and probe_candidate()
// ─────────────────────────────────────────────────────────────────────────────

const PROBE_TIMEOUT_SECS: u64 = 2;

/// Platform-ordered list of candidate runtime names to probe.
fn platform_candidates() -> &'static [&'static str] {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        &[
            "apple/container",
            "orbstack",
            "colima",
            "rancher-desktop",
            "podman",
            "lima",
            "docker",
        ]
    }
    #[cfg(target_os = "linux")]
    {
        &["podman", "nerdctl", "docker"]
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux")))]
    {
        &["podman", "nerdctl", "docker"]
    }
}

/// Run a quick probe command with a timeout and return its stdout.
async fn probe_run(bin: &str, args: &[&str]) -> std::result::Result<String, String> {
    use tokio::time::{timeout, Duration};
    let fut = Command::new(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match timeout(Duration::from_secs(PROBE_TIMEOUT_SECS), fut).await {
        Ok(Ok(out)) => {
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).to_string())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).to_string())
            }
        }
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err(format!("probe timed out after {}s", PROBE_TIMEOUT_SECS)),
    }
}

/// Probe a single named runtime and return a type-erased `Box<dyn ContainerBackend>`
/// if it is available, or a human-readable reason string if it is not.
pub async fn probe_candidate(
    name: &str,
) -> std::result::Result<Box<dyn ContainerBackend>, String> {
    match name {
        // ── apple/container ──────────────────────────────────────────────
        "apple/container" => {
            let bin = which::which("container")
                .map_err(|_| "container binary not found on PATH".to_string())?;
            probe_run(bin.to_str().unwrap_or("container"), &["--version"])
                .await
                .map_err(|e| format!("apple/container --version failed: {}", e))?;
            Ok(Box::new(CliBackend::new(bin, AppleContainerProtocol)))
        }

        // ── orbstack ─────────────────────────────────────────────────────
        "orbstack" => {
            let orb_ok = which::which("orb")
                .ok()
                .map(|b| {
                    let b_str = b.to_string_lossy().to_string();
                    async move { probe_run(&b_str, &["--version"]).await.is_ok() }
                });
            let sock_ok = std::path::Path::new(
                &shellexpand::tilde("~/.orbstack/run/docker.sock").to_string(),
            )
            .exists();
            let orb_available = match orb_ok {
                Some(fut) => fut.await,
                None => false,
            };
            if orb_available || sock_ok {
                let bin = which::which("docker")
                    .or_else(|_| which::which("orb"))
                    .map_err(|_| "orbstack: neither docker nor orb found".to_string())?;
                Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
            } else {
                Err("orbstack: neither `orb --version` succeeded nor socket found".into())
            }
        }

        // ── colima ───────────────────────────────────────────────────────
        "colima" => {
            let bin = which::which("colima")
                .map_err(|_| "colima not found".to_string())?;
            let status = probe_run(bin.to_str().unwrap_or("colima"), &["status"])
                .await
                .map_err(|e| format!("colima status failed: {}", e))?;
            if !status.to_lowercase().contains("running") {
                return Err("colima is installed but not running".into());
            }
            let docker_bin = which::which("docker")
                .map_err(|_| "docker CLI not found (needed for colima)".to_string())?;
            Ok(Box::new(CliBackend::new(docker_bin, DockerProtocol)))
        }

        // ── rancher-desktop ──────────────────────────────────────────────
        "rancher-desktop" => {
            let bin = which::which("nerdctl")
                .map_err(|_| "nerdctl not found".to_string())?;
            probe_run(bin.to_str().unwrap_or("nerdctl"), &["--version"])
                .await
                .map_err(|e| format!("nerdctl --version failed: {}", e))?;
            let sock = std::path::Path::new(
                &shellexpand::tilde("~/.rd/run/containerd-shim.sock").to_string(),
            )
            .exists();
            if sock {
                Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
            } else {
                Err("rancher-desktop: nerdctl found but containerd socket missing".into())
            }
        }

        // ── podman ───────────────────────────────────────────────────────
        "podman" => {
            let bin = which::which("podman")
                .map_err(|_| "podman not found".to_string())?;
            probe_run(bin.to_str().unwrap_or("podman"), &["--version"])
                .await
                .map_err(|e| format!("podman --version failed: {}", e))?;

            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                let machines = probe_run(
                    bin.to_str().unwrap_or("podman"),
                    &["machine", "list", "--format", "json"],
                )
                .await
                .unwrap_or_default();
                let has_running = serde_json::from_str::<Vec<serde_json::Value>>(&machines)
                    .unwrap_or_default()
                    .iter()
                    .any(|m| m.get("Running").and_then(|v| v.as_bool()).unwrap_or(false));
                if !has_running {
                    return Err(
                        "podman: no running machine found (run `podman machine start`)".into(),
                    );
                }
            }

            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }

        // ── lima ─────────────────────────────────────────────────────────
        "lima" => {
            let bin = which::which("limactl")
                .map_err(|_| "limactl not found".to_string())?;
            let list_out = probe_run(bin.to_str().unwrap_or("limactl"), &["list", "--json"])
                .await
                .map_err(|e| format!("limactl list --json failed: {}", e))?;
            let instance = list_out
                .lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| {
                    v.get("status")
                        .and_then(|s| s.as_str())
                        .map(|s| s.eq_ignore_ascii_case("running"))
                        .unwrap_or(false)
                })
                .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .ok_or_else(|| "limactl: no running Lima instance found".to_string())?;
            Ok(Box::new(CliBackend::new(bin, LimaProtocol::new(instance))))
        }

        // ── nerdctl (standalone) ─────────────────────────────────────────
        "nerdctl" => {
            let bin = which::which("nerdctl")
                .map_err(|_| "nerdctl not found".to_string())?;
            probe_run(bin.to_str().unwrap_or("nerdctl"), &["--version"])
                .await
                .map_err(|e| format!("nerdctl --version failed: {}", e))?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }

        // ── docker ───────────────────────────────────────────────────────
        "docker" => {
            let bin = which::which("docker")
                .map_err(|_| "docker not found".to_string())?;
            probe_run(bin.to_str().unwrap_or("docker"), &["--version"])
                .await
                .map_err(|e| format!("docker --version failed: {}", e))?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }

        other => Err(format!("unknown runtime '{}'", other)),
    }
}

/// Detect the best available container backend for the current platform.
///
/// 1. If `PERRY_CONTAINER_BACKEND` is set, use that backend directly.
/// 2. Otherwise, probe `platform_candidates()` in order with a 2s timeout each.
/// 3. If no candidate is available, returns `Err(NoBackendFound { probed })`.
pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend>, ComposeError> {
    use std::time::Duration;

    // ── Override via env var ──────────────────────────────────────────────
    if let Ok(override_name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        let name = override_name.trim().to_string();
        debug!("PERRY_CONTAINER_BACKEND={}, probing directly", name);
        return probe_candidate(&name).await.map_err(|reason| {
            ComposeError::BackendNotAvailable {
                name: name.clone(),
                reason,
            }
        });
    }

    // ── Platform probe sequence ───────────────────────────────────────────
    let mut probed: Vec<BackendProbeResult> = Vec::new();

    for &candidate in platform_candidates() {
        debug!("probing container backend: {}", candidate);
        match tokio::time::timeout(
            Duration::from_secs(PROBE_TIMEOUT_SECS),
            probe_candidate(candidate),
        )
        .await
        {
            Ok(Ok(backend)) => {
                debug!("selected container backend: {}", candidate);
                return Ok(backend);
            }
            Ok(Err(reason)) => {
                debug!("backend '{}' not available: {}", candidate, reason);
                probed.push(BackendProbeResult {
                    name: candidate.to_string(),
                    available: false,
                    reason,
                });
            }
            Err(_) => {
                debug!("backend '{}' probe timed out", candidate);
                probed.push(BackendProbeResult {
                    name: candidate.to_string(),
                    available: false,
                    reason: format!("probe timed out after {}s", PROBE_TIMEOUT_SECS),
                });
            }
        }
    }

    Err(ComposeError::NoBackendFound { probed })
}

// ─────────────────────────────────────────────────────────────────────────────
// Legacy compatibility shims
// ─────────────────────────────────────────────────────────────────────────────

/// Legacy container status enum kept for backward compatibility with `compose.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerStatus {
    Running,
    Stopped,
    NotFound,
}

impl ContainerStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, ContainerStatus::Running)
    }
    pub fn exists(&self) -> bool {
        !matches!(self, ContainerStatus::NotFound)
    }
}

/// Legacy exec result kept for backward compatibility.
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Legacy `Backend` trait kept for backward compatibility with `compose.rs`.
/// New code should use `ContainerBackend` + `CliBackend` instead.
#[async_trait]
pub trait Backend: Send + Sync {
    fn name(&self) -> &'static str;

    async fn build(
        &self,
        context: &str,
        dockerfile: Option<&str>,
        tag: &str,
        args: Option<&HashMap<String, String>>,
        target: Option<&str>,
        network: Option<&str>,
    ) -> Result<()>;

    async fn run(
        &self,
        image: &str,
        name: &str,
        ports: Option<&[String]>,
        env: Option<&HashMap<String, String>>,
        volumes: Option<&[String]>,
        labels: Option<&HashMap<String, String>>,
        cmd: Option<&[String]>,
        detach: bool,
    ) -> Result<()>;

    async fn start(&self, name: &str) -> Result<()>;
    async fn stop(&self, name: &str) -> Result<()>;
    async fn remove(&self, name: &str, force: bool) -> Result<()>;
    async fn inspect(&self, name: &str) -> Result<ContainerStatus>;
    async fn list(&self, label_filter: Option<&str>) -> Result<Vec<ContainerInfo>>;
    async fn logs(&self, name: &str, tail: Option<u32>, follow: bool) -> Result<String>;
    async fn exec(
        &self,
        name: &str,
        cmd: &[String],
        user: Option<&str>,
        workdir: Option<&str>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<ExecResult>;
    async fn create_network(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&HashMap<String, String>>,
    ) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&HashMap<String, String>>,
    ) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

/// Synchronous best-effort backend selector for legacy callers.
/// Prefer `detect_backend().await` in async contexts.
pub fn get_backend() -> Result<Box<dyn Backend>> {
    Err(ComposeError::BackendNotAvailable {
        name: "legacy".into(),
        reason: "use detect_backend() instead".into(),
    })
}

/// Synchronous best-effort `ContainerBackend` selector for legacy callers.
/// Prefer `detect_backend().await` in async contexts.
pub fn get_container_backend() -> Result<Box<dyn ContainerBackend>> {
    Err(ComposeError::BackendNotAvailable {
        name: "legacy".into(),
        reason: "use detect_backend() instead".into(),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_spec(name: Option<&str>) -> ContainerSpec {
        ContainerSpec {
            image: "alpine:latest".into(),
            name: name.map(String::from),
            ports: Some(vec!["8080:80".into()]),
            volumes: Some(vec!["/tmp:/data".into()]),
            env: Some({
                let mut m = HashMap::new();
                m.insert("FOO".into(), "bar".into());
                m
            }),
            cmd: Some(vec!["sh".into(), "-c".into(), "echo hi".into()]),
            entrypoint: None,
            network: Some("mynet".into()),
            rm: Some(true),
        }
    }

    // ── DockerProtocol ────────────────────────────────────────────────────

    #[test]
    fn docker_run_args_contains_expected_flags() {
        let p = DockerProtocol;
        let spec = dummy_spec(Some("mycontainer"));
        let args = p.run_args(&spec);
        assert!(args.contains(&"run".into()));
        assert!(args.contains(&"--rm".into()));
        assert!(args.contains(&"--detach".into()));
        assert!(args.contains(&"--name".into()));
        assert!(args.contains(&"mycontainer".into()));
        assert!(args.contains(&"-p".into()));
        assert!(args.contains(&"8080:80".into()));
        assert!(args.contains(&"-v".into()));
        assert!(args.contains(&"/tmp:/data".into()));
        assert!(args.contains(&"-e".into()));
        assert!(args.contains(&"FOO=bar".into()));
        assert!(args.contains(&"--network".into()));
        assert!(args.contains(&"mynet".into()));
        assert!(args.contains(&"alpine:latest".into()));
    }

    #[test]
    fn docker_stop_args_with_timeout() {
        let p = DockerProtocol;
        let args = p.stop_args("abc123", Some(10));
        assert_eq!(args, vec!["stop", "-t", "10", "abc123"]);
    }

    #[test]
    fn docker_stop_args_no_timeout() {
        let p = DockerProtocol;
        let args = p.stop_args("abc123", None);
        assert_eq!(args, vec!["stop", "abc123"]);
    }

    #[test]
    fn docker_remove_args_force() {
        let p = DockerProtocol;
        assert_eq!(p.remove_args("c1", true), vec!["rm", "-f", "c1"]);
        assert_eq!(p.remove_args("c1", false), vec!["rm", "c1"]);
    }

    #[test]
    fn docker_list_args() {
        let p = DockerProtocol;
        assert!(p.list_args(true).contains(&"--all".into()));
        assert!(!p.list_args(false).contains(&"--all".into()));
    }

    #[test]
    fn docker_parse_list_output_array() {
        let p = DockerProtocol;
        let json = r#"[{"ID":"abc","Names":["/myapp"],"Image":"nginx","Status":"running","Ports":["80/tcp"],"Created":"2024-01-01"}]"#;
        let infos = p.parse_list_output(json);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].id, "abc");
        assert_eq!(infos[0].name, "myapp");
    }

    #[test]
    fn docker_parse_list_output_ndjson() {
        let p = DockerProtocol;
        let json = "{\"ID\":\"abc\",\"Names\":[\"/myapp\"],\"Image\":\"nginx\",\"Status\":\"running\",\"Ports\":[],\"Created\":\"2024-01-01\"}\n{\"ID\":\"def\",\"Names\":[\"/other\"],\"Image\":\"redis\",\"Status\":\"stopped\",\"Ports\":[],\"Created\":\"2024-01-02\"}";
        let infos = p.parse_list_output(json);
        assert_eq!(infos.len(), 2);
    }

    #[test]
    fn docker_parse_inspect_output() {
        let p = DockerProtocol;
        let json = r#"[{"Id":"abc123","Name":"/myapp","Image":"nginx","State":{"Running":true,"Status":"running"},"Created":"2024-01-01"}]"#;
        let info = p.parse_inspect_output("abc123", json).unwrap();
        assert_eq!(info.status, "running");
        assert_eq!(info.name, "myapp");
    }

    #[test]
    fn docker_parse_images_output() {
        let p = DockerProtocol;
        let json = r#"[{"ID":"sha256:abc","Repository":"nginx","Tag":"latest","Size":50000000,"Created":"2024-01-01"}]"#;
        let images = p.parse_list_images_output(json);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].repository, "nginx");
        assert_eq!(images[0].size, 50_000_000);
    }

    // ── NetworkConfig / VolumeConfig args ─────────────────────────────────

    #[test]
    fn create_network_args_with_config() {
        let p = DockerProtocol;
        let mut labels = HashMap::new();
        labels.insert("env".into(), "prod".into());
        let config = NetworkConfig {
            driver: Some("bridge".into()),
            labels,
            internal: true,
            enable_ipv6: false,
        };
        let args = p.create_network_args("mynet", &config);
        assert!(args.contains(&"network".into()));
        assert!(args.contains(&"create".into()));
        assert!(args.contains(&"--driver".into()));
        assert!(args.contains(&"bridge".into()));
        assert!(args.contains(&"--label".into()));
        assert!(args.contains(&"env=prod".into()));
        assert!(args.contains(&"--internal".into()));
        assert!(!args.contains(&"--ipv6".into()));
        assert!(args.last() == Some(&"mynet".into()));
    }

    #[test]
    fn create_volume_args_with_config() {
        let p = DockerProtocol;
        let config = VolumeConfig {
            driver: Some("local".into()),
            labels: HashMap::new(),
        };
        let args = p.create_volume_args("myvol", &config);
        assert!(args.contains(&"volume".into()));
        assert!(args.contains(&"create".into()));
        assert!(args.contains(&"--driver".into()));
        assert!(args.contains(&"local".into()));
        assert!(args.last() == Some(&"myvol".into()));
    }

    // ── From conversions ──────────────────────────────────────────────────

    #[test]
    fn network_config_from_compose_network() {
        use crate::types::ListOrDict;
        let mut cn = ComposeNetwork::default();
        cn.driver = Some("overlay".into());
        cn.internal = Some(true);
        cn.enable_ipv6 = Some(true);
        cn.labels = Some(ListOrDict::List(vec!["foo=bar".into()]));
        let nc = NetworkConfig::from(&cn);
        assert_eq!(nc.driver, Some("overlay".into()));
        assert!(nc.internal);
        assert!(nc.enable_ipv6);
        assert_eq!(nc.labels.get("foo"), Some(&"bar".into()));
    }

    #[test]
    fn volume_config_from_compose_volume() {
        use crate::types::ListOrDict;
        let mut cv = ComposeVolume::default();
        cv.driver = Some("nfs".into());
        cv.labels = Some(ListOrDict::List(vec!["tier=data".into()]));
        let vc = VolumeConfig::from(&cv);
        assert_eq!(vc.driver, Some("nfs".into()));
        assert_eq!(vc.labels.get("tier"), Some(&"data".into()));
    }

    // ── AppleContainerProtocol ────────────────────────────────────────────

    #[test]
    fn apple_run_args_no_detach() {
        let p = AppleContainerProtocol;
        let spec = dummy_spec(Some("mycontainer"));
        let args = p.run_args(&spec);
        assert!(!args.contains(&"--detach".into()));
        assert!(args.contains(&"--rm".into()));
        assert!(args.contains(&"--name".into()));
    }

    #[test]
    fn apple_protocol_name() {
        let p = AppleContainerProtocol;
        assert_eq!(p.protocol_name(), "apple/container");
    }

    // ── LimaProtocol ─────────────────────────────────────────────────────

    #[test]
    fn lima_subcommand_prefix() {
        let p = LimaProtocol::new("default");
        let prefix = p.subcommand_prefix().unwrap();
        assert_eq!(prefix, vec!["shell", "default", "nerdctl"]);
    }

    #[test]
    fn lima_run_args_delegates_to_docker_defaults() {
        let lima = LimaProtocol::new("default");
        let docker = DockerProtocol;
        let spec = dummy_spec(None);
        assert_eq!(lima.run_args(&spec), docker.run_args(&spec));
    }

    #[test]
    fn lima_protocol_name() {
        let p = LimaProtocol::new("myvm");
        assert_eq!(p.protocol_name(), "lima");
    }

    // ── CliBackend<P> full_args ───────────────────────────────────────────

    #[test]
    fn cli_backend_full_args_no_prefix() {
        let backend = CliBackend::new(PathBuf::from("docker"), DockerProtocol);
        let result = backend.full_args(vec!["ps".into(), "--all".into()]);
        assert_eq!(result, vec!["ps", "--all"]);
    }

    #[test]
    fn cli_backend_full_args_with_lima_prefix() {
        let backend = CliBackend::new(PathBuf::from("limactl"), LimaProtocol::new("default"));
        let result = backend.full_args(vec!["ps".into(), "--all".into()]);
        assert_eq!(result, vec!["shell", "default", "nerdctl", "ps", "--all"]);
    }

    #[test]
    fn backend_name_from_path() {
        let backend = CliBackend::new(PathBuf::from("/usr/bin/podman"), DockerProtocol);
        assert_eq!(backend.backend_name(), "podman");
    }

    // ── Type aliases ──────────────────────────────────────────────────────

    #[test]
    fn type_aliases_compile() {
        let _: DockerBackend = CliBackend::new(PathBuf::from("docker"), DockerProtocol);
        let _: AppleBackend = CliBackend::new(PathBuf::from("container"), AppleContainerProtocol);
        let _: LimaBackend =
            CliBackend::new(PathBuf::from("limactl"), LimaProtocol::new("default"));
    }

    // ── BackendProbeResult serialization ─────────────────────────────────

    #[test]
    fn probe_result_round_trip() {
        let r = BackendProbeResult {
            name: "podman".into(),
            available: false,
            reason: "not found".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let r2: BackendProbeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r2.name, "podman");
        assert!(!r2.available);
    }
}
