//! Container backend abstraction — `ContainerBackend` trait, `CliProtocol` trait,
//! protocol implementations (`DockerProtocol`, `AppleContainerProtocol`, `LimaProtocol`),
//! `CliBackend`, and `detect_backend()`.

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
// 4.8  BackendProbeResult — defined in error.rs, re-exported here for
//      convenience so callers can use `backend::BackendProbeResult`.
// ─────────────────────────────────────────────────────────────────────────────
pub use crate::error::BackendProbeResult;

// ─────────────────────────────────────────────────────────────────────────────
// 4.1  ContainerBackend trait
// ─────────────────────────────────────────────────────────────────────────────

/// Runtime-agnostic async interface for container operations.
#[async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Human-readable backend name (e.g. `"apple/container"`, `"podman"`).
    fn backend_name(&self) -> &str;

    /// Verify the backend binary is installed and (where applicable) running.
    async fn check_available(&self) -> Result<()>;

    /// Create **and** start a container. Returns a handle.
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Create a container without starting it.
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Start a previously created container.
    async fn start(&self, id: &str) -> Result<()>;

    /// Stop a running container. `timeout` is seconds before SIGKILL.
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;

    /// Remove a container. When `force` is true, stop it first.
    async fn remove(&self, id: &str, force: bool) -> Result<()>;

    /// List containers. When `all` is true, include stopped containers.
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;

    /// Inspect a single container by ID or name.
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;

    /// Fetch stdout/stderr logs. `tail` limits to the last N lines.
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs>;

    /// Execute a command inside a running container.
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs>;

    /// Pull an OCI image from a registry.
    async fn pull_image(&self, reference: &str) -> Result<()>;

    /// List locally available images.
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;

    /// Remove a local image. When `force` is true, remove even if in use.
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;

    /// Create a network.
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;

    /// Remove a network (idempotent — not-found is silently ignored).
    async fn remove_network(&self, name: &str) -> Result<()>;

    /// Create a named volume.
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;

    /// Remove a named volume (idempotent — not-found is silently ignored).
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.2  CliProtocol trait
// ─────────────────────────────────────────────────────────────────────────────

/// Translates abstract container operations into CLI arguments for a specific
/// runtime family, and parses the CLI's JSON output back into typed structs.
///
/// Implement this trait to add support for a new CLI syntax without touching
/// `CliBackend`.
pub trait CliProtocol: Send + Sync {
    /// Optional prefix inserted before every subcommand.
    ///
    /// `LimaProtocol` returns `Some(vec!["shell", "<instance>", "nerdctl"])`.
    /// All other protocols return `None`.
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        None
    }

    // ── Argument builders ──────────────────────────────────────────────────

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn start_args(&self, id: &str) -> Vec<String>;
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String>;
    fn remove_args(&self, id: &str, force: bool) -> Vec<String>;
    fn list_args(&self, all: bool) -> Vec<String>;
    fn inspect_args(&self, id: &str) -> Vec<String>;
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String>;
    fn exec_args(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String>;
    fn pull_image_args(&self, reference: &str) -> Vec<String>;
    fn list_images_args(&self) -> Vec<String>;
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String>;
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String>;
    fn remove_network_args(&self, name: &str) -> Vec<String>;
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String>;
    fn remove_volume_args(&self, name: &str) -> Vec<String>;

    // ── Output parsers ─────────────────────────────────────────────────────

    /// Parse the JSON output of the list command into `ContainerInfo` objects.
    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo>;

    /// Parse the JSON output of the inspect command into a `ContainerInfo`.
    fn parse_inspect_output(&self, id: &str, stdout: &str) -> Option<ContainerInfo>;

    /// Parse the JSON output of the list-images command into `ImageInfo` objects.
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo>;

    /// Extract the container ID from the stdout of a `run` or `create` command.
    fn parse_container_id(&self, stdout: &str) -> String {
        stdout.trim().to_string()
    }
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
fn docker_run_flags(spec: &ContainerSpec, include_detach: bool) -> Vec<String> {
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
// 4.3  DockerProtocol
// ─────────────────────────────────────────────────────────────────────────────

/// `CliProtocol` implementation for Docker-compatible runtimes:
/// podman, nerdctl, orbstack, docker, colima.
pub struct DockerProtocol;

impl CliProtocol for DockerProtocol {
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

    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver {
            args.push("--driver".into());
            args.push(d.clone());
        }
        if let Some(lbls) = &config.labels {
            let mut pairs: Vec<(String, String)> = lbls.to_map().into_iter().collect();
            pairs.sort_by_key(|(k, _)| k.clone());
            for (k, v) in pairs {
                args.push("--label".into());
                args.push(format!("{}={}", k, v));
            }
        }
        args.push(name.into());
        args
    }

    fn remove_network_args(&self, name: &str) -> Vec<String> {
        vec!["network".into(), "rm".into(), name.into()]
    }

    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver {
            args.push("--driver".into());
            args.push(d.clone());
        }
        if let Some(lbls) = &config.labels {
            let mut pairs: Vec<(String, String)> = lbls.to_map().into_iter().collect();
            pairs.sort_by_key(|(k, _)| k.clone());
            for (k, v) in pairs {
                args.push("--label".into());
                args.push(format!("{}={}", k, v));
            }
        }
        args.push(name.into());
        args
    }

    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        vec!["volume".into(), "rm".into(), name.into()]
    }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        // Docker outputs one JSON object per line OR a JSON array
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
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.4  AppleContainerProtocol
// ─────────────────────────────────────────────────────────────────────────────

/// `CliProtocol` implementation for the `apple/container` CLI on macOS/iOS.
///
/// The `apple/container` CLI is largely Docker-compatible but has a few
/// differences:
/// - `run` does not support `--detach`; containers run in the foreground by
///   default and the name is returned on stdout.
/// - `ps --format json` returns objects with `"ID"` (not `"Id"`).
/// - `inspect` returns a single object (not an array).
/// - `images --format json` uses `"ID"` (not `"Id"`).
///
/// We inherit all Docker-compatible implementations via delegation and only
/// override the methods that differ.
pub struct AppleContainerProtocol;

impl CliProtocol for AppleContainerProtocol {
    // `apple/container run` does not accept `--detach`; omit it.
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into()];
        // Build flags without --detach
        if spec.rm.unwrap_or(false) {
            args.push("--rm".into());
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
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.iter().cloned());
        }
        args
    }

    // Delegate everything else to DockerProtocol
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        DockerProtocol.create_args(spec)
    }
    fn start_args(&self, id: &str) -> Vec<String> {
        DockerProtocol.start_args(id)
    }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        DockerProtocol.stop_args(id, timeout)
    }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        DockerProtocol.remove_args(id, force)
    }
    fn list_args(&self, all: bool) -> Vec<String> {
        DockerProtocol.list_args(all)
    }
    fn inspect_args(&self, id: &str) -> Vec<String> {
        DockerProtocol.inspect_args(id)
    }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        DockerProtocol.logs_args(id, tail)
    }
    fn exec_args(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String> {
        DockerProtocol.exec_args(id, cmd, env, workdir)
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> {
        DockerProtocol.pull_image_args(reference)
    }
    fn list_images_args(&self) -> Vec<String> {
        DockerProtocol.list_images_args()
    }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        DockerProtocol.remove_image_args(reference, force)
    }
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> {
        DockerProtocol.create_network_args(name, config)
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> {
        DockerProtocol.remove_network_args(name)
    }
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> {
        DockerProtocol.create_volume_args(name, config)
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        DockerProtocol.remove_volume_args(name)
    }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        DockerProtocol.parse_list_output(stdout)
    }
    fn parse_inspect_output(&self, id: &str, stdout: &str) -> Option<ContainerInfo> {
        DockerProtocol.parse_inspect_output(id, stdout)
    }
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> {
        DockerProtocol.parse_list_images_output(stdout)
    }

    /// `apple/container run` prints the container name (not ID) on stdout.
    fn parse_container_id(&self, stdout: &str) -> String {
        stdout.trim().to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.5  LimaProtocol
// ─────────────────────────────────────────────────────────────────────────────

/// `CliProtocol` implementation for Lima.
///
/// All commands are wrapped as:
///   `limactl shell <instance> nerdctl <cmd>`
///
/// The inner `nerdctl` invocation uses Docker-compatible flags, so we delegate
/// all argument building to `DockerProtocol` and only override
/// `subcommand_prefix()`.
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
    /// Returns `["shell", "<instance>", "nerdctl"]` so that `CliBackend`
    /// prepends these before every subcommand.
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec![
            "shell".into(),
            self.instance.clone(),
            "nerdctl".into(),
        ])
    }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        DockerProtocol.run_args(spec)
    }
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        DockerProtocol.create_args(spec)
    }
    fn start_args(&self, id: &str) -> Vec<String> {
        DockerProtocol.start_args(id)
    }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        DockerProtocol.stop_args(id, timeout)
    }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        DockerProtocol.remove_args(id, force)
    }
    fn list_args(&self, all: bool) -> Vec<String> {
        DockerProtocol.list_args(all)
    }
    fn inspect_args(&self, id: &str) -> Vec<String> {
        DockerProtocol.inspect_args(id)
    }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        DockerProtocol.logs_args(id, tail)
    }
    fn exec_args(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String> {
        DockerProtocol.exec_args(id, cmd, env, workdir)
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> {
        DockerProtocol.pull_image_args(reference)
    }
    fn list_images_args(&self) -> Vec<String> {
        DockerProtocol.list_images_args()
    }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        DockerProtocol.remove_image_args(reference, force)
    }
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> {
        DockerProtocol.create_network_args(name, config)
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> {
        DockerProtocol.remove_network_args(name)
    }
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> {
        DockerProtocol.create_volume_args(name, config)
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        DockerProtocol.remove_volume_args(name)
    }
    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        DockerProtocol.parse_list_output(stdout)
    }
    fn parse_inspect_output(&self, id: &str, stdout: &str) -> Option<ContainerInfo> {
        DockerProtocol.parse_inspect_output(id, stdout)
    }
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> {
        DockerProtocol.parse_list_images_output(stdout)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.6  CliBackend
// ─────────────────────────────────────────────────────────────────────────────

/// Concrete `ContainerBackend` that executes CLI commands via
/// `tokio::process::Command`.
///
/// Argument building is fully delegated to the `CliProtocol` implementation,
/// so `CliBackend` itself is runtime-agnostic.
pub struct CliBackend {
    pub bin: PathBuf,
    pub protocol: Box<dyn CliProtocol>,
}

impl CliBackend {
    pub fn new(bin: PathBuf, protocol: Box<dyn CliProtocol>) -> Self {
        CliBackend { bin, protocol }
    }

    /// Build the full argument list, prepending the protocol's subcommand
    /// prefix (e.g. `["shell", "default", "nerdctl"]` for Lima) when present.
    fn full_args(&self, subcommand_args: Vec<String>) -> Vec<String> {
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
impl ContainerBackend for CliBackend {
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
        let args = self.protocol.start_args(id);
        self.exec_ok(args).await?;
        Ok(())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let args = self.protocol.stop_args(id, timeout);
        self.exec_ok(args).await?;
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_args(id, force);
        self.exec_ok(args).await?;
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let args = self.protocol.list_args(all);
        let stdout = self.exec_ok(args).await?;
        Ok(self.protocol.parse_list_output(&stdout))
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let args = self.protocol.inspect_args(id);
        let output = self.exec_raw(args).await?;
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
        let args = self.protocol.logs_args(id, tail);
        let output = self.exec_raw(args).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        let args = self.protocol.exec_args(id, cmd, env, workdir);
        let output = self.exec_raw(args).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let args = self.protocol.pull_image_args(reference);
        self.exec_ok(args).await?;
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let args = self.protocol.list_images_args();
        let stdout = self.exec_ok(args).await?;
        Ok(self.protocol.parse_list_images_output(&stdout))
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_image_args(reference, force);
        self.exec_ok(args).await?;
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> {
        let args = self.protocol.create_network_args(name, config);
        self.exec_ok(args).await?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_network_args(name);
        let output = self.exec_raw(args).await?;
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

    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        let args = self.protocol.create_volume_args(name, config);
        self.exec_ok(args).await?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_volume_args(name);
        let output = self.exec_raw(args).await?;
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

/// Resolve the binary path for a named runtime.
fn resolve_bin(name: &str) -> Option<PathBuf> {
    let bin_name = match name {
        "apple/container" => "container",
        "rancher-desktop" => "nerdctl",
        "lima" => "limactl",
        other => other,
    };
    which::which(bin_name).ok()
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

/// Probe a single named runtime and return a `CliBackend` if it is available,
/// or a human-readable reason string if it is not.
pub async fn probe_candidate(name: &str) -> std::result::Result<CliBackend, String> {
    let bin = match resolve_bin(name) {
        Some(p) => p,
        None => return Err(format!("'{}' binary not found on PATH", name)),
    };
    let bin_str = bin.to_string_lossy().to_string();

    match name {
        // ── apple/container ──────────────────────────────────────────────
        "apple/container" => {
            probe_run(&bin_str, &["--version"])
                .await
                .map_err(|e| format!("apple/container --version failed: {}", e))?;
            Ok(CliBackend::new(bin, Box::new(AppleContainerProtocol)))
        }

        // ── orbstack ─────────────────────────────────────────────────────
        "orbstack" => {
            // Accept if `orb --version` succeeds OR the Docker socket exists.
            let orb_ok = probe_run(&bin_str, &["--version"]).await.is_ok();
            let sock_ok = std::path::Path::new(
                &shellexpand::tilde("~/.orbstack/run/docker.sock").to_string(),
            )
            .exists();
            if orb_ok || sock_ok {
                Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
            } else {
                Err("orbstack: neither `orb --version` succeeded nor socket found".into())
            }
        }

        // ── colima ───────────────────────────────────────────────────────
        "colima" => {
            let status = probe_run(&bin_str, &["status"])
                .await
                .map_err(|e| format!("colima status failed: {}", e))?;
            if status.to_lowercase().contains("running") {
                Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
            } else {
                Err("colima is installed but not running".into())
            }
        }

        // ── rancher-desktop ──────────────────────────────────────────────
        "rancher-desktop" => {
            // Requires `nerdctl --version` AND the containerd socket.
            probe_run(&bin_str, &["--version"])
                .await
                .map_err(|e| format!("nerdctl --version failed: {}", e))?;
            let sock = std::path::Path::new(
                &shellexpand::tilde("~/.rd/run/containerd-shim.sock").to_string(),
            )
            .exists();
            if sock {
                // Use the resolved nerdctl binary directly.
                Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
            } else {
                Err("rancher-desktop: nerdctl found but containerd socket missing".into())
            }
        }

        // ── podman ───────────────────────────────────────────────────────
        "podman" => {
            probe_run(&bin_str, &["--version"])
                .await
                .map_err(|e| format!("podman --version failed: {}", e))?;

            // On macOS, also verify a machine is running.
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                let machines = probe_run(&bin_str, &["machine", "list", "--format", "json"])
                    .await
                    .unwrap_or_default();
                // Look for at least one entry with "Running": true
                let has_running = serde_json::from_str::<Vec<serde_json::Value>>(&machines)
                    .unwrap_or_default()
                    .iter()
                    .any(|m| m.get("Running").and_then(|v| v.as_bool()).unwrap_or(false));
                if !has_running {
                    return Err("podman: no running machine found (run `podman machine start`)".into());
                }
            }

            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }

        // ── lima ─────────────────────────────────────────────────────────
        "lima" => {
            let list_out = probe_run(&bin_str, &["list", "--json"])
                .await
                .map_err(|e| format!("limactl list --json failed: {}", e))?;
            // Find a running instance.
            let instance = list_out
                .lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| {
                    v.get("status")
                        .and_then(|s| s.as_str())
                        .map(|s| s.eq_ignore_ascii_case("running"))
                        .unwrap_or(false)
                })
                .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from));
            match instance {
                Some(inst) => Ok(CliBackend::new(bin, Box::new(LimaProtocol::new(inst)))),
                None => Err("limactl: no running Lima instance found".into()),
            }
        }

        // ── nerdctl (standalone) ─────────────────────────────────────────
        "nerdctl" => {
            probe_run(&bin_str, &["--version"])
                .await
                .map_err(|e| format!("nerdctl --version failed: {}", e))?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }

        // ── docker ───────────────────────────────────────────────────────
        "docker" => {
            probe_run(&bin_str, &["--version"])
                .await
                .map_err(|e| format!("docker --version failed: {}", e))?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }

        other => Err(format!("unknown runtime '{}'", other)),
    }
}

/// Detect the best available container backend for the current platform.
///
/// 1. If `PERRY_CONTAINER_BACKEND` is set, use that backend directly without
///    probing any others. Returns `Err(NoBackendFound)` if it is unavailable.
/// 2. Otherwise, probe `platform_candidates()` in order and return the first
///    available one.
/// 3. If no candidate is available, returns `Err(NoBackendFound { probed })`.
pub async fn detect_backend() -> std::result::Result<CliBackend, ComposeError> {
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
        match probe_candidate(candidate).await {
            Ok(backend) => {
                debug!("selected container backend: {}", candidate);
                probed.push(BackendProbeResult {
                    name: candidate.to_string(),
                    available: true,
                    reason: String::new(),
                });
                // Log remaining candidates as not-probed (skipped)
                return Ok(backend);
            }
            Err(reason) => {
                debug!("backend '{}' not available: {}", candidate, reason);
                probed.push(BackendProbeResult {
                    name: candidate.to_string(),
                    available: false,
                    reason,
                });
            }
        }
    }

    Err(ComposeError::NoBackendFound { probed })
}

// ─────────────────────────────────────────────────────────────────────────────
// Legacy compatibility shims
// ─────────────────────────────────────────────────────────────────────────────

/// Legacy `Backend` trait kept for backward compatibility with the CLI path.
/// New code should use `ContainerBackend` + `CliBackend` instead.
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

#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

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
    // Return a stub that panics on use — callers should migrate to detect_backend().
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

    // ── LimaProtocol ─────────────────────────────────────────────────────

    #[test]
    fn lima_subcommand_prefix() {
        let p = LimaProtocol::new("default");
        let prefix = p.subcommand_prefix().unwrap();
        assert_eq!(prefix, vec!["shell", "default", "nerdctl"]);
    }

    #[test]
    fn lima_run_args_delegates_to_docker() {
        let lima = LimaProtocol::new("default");
        let docker = DockerProtocol;
        let spec = dummy_spec(None);
        assert_eq!(lima.run_args(&spec), docker.run_args(&spec));
    }

    // ── CliBackend full_args ──────────────────────────────────────────────

    #[test]
    fn cli_backend_full_args_no_prefix() {
        let backend = CliBackend::new(PathBuf::from("docker"), Box::new(DockerProtocol));
        let result = backend.full_args(vec!["ps".into(), "--all".into()]);
        assert_eq!(result, vec!["ps", "--all"]);
    }

    #[test]
    fn cli_backend_full_args_with_lima_prefix() {
        let backend = CliBackend::new(
            PathBuf::from("limactl"),
            Box::new(LimaProtocol::new("default")),
        );
        let result = backend.full_args(vec!["ps".into(), "--all".into()]);
        assert_eq!(result, vec!["shell", "default", "nerdctl", "ps", "--all"]);
    }

    #[test]
    fn backend_name_from_path() {
        let backend = CliBackend::new(PathBuf::from("/usr/bin/podman"), Box::new(DockerProtocol));
        assert_eq!(backend.backend_name(), "podman");
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
