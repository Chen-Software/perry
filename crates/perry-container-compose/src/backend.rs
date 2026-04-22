//! Container backend abstraction and implementation.
//!
//! Structured in four layers:
//! 1. `ContainerBackend` trait (Abstract operations)
//! 2. `CliProtocol` trait (Argument building + Output parsing)
//! 3. `CliBackend` struct (CLI executor, implements Layer 1 via Layer 2)
//! 4. `detect_backend()` (Multi-candidate platform probe)

use crate::error::{ComposeError, Result, BackendProbeResult};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo,
    ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

// ============ Layer 1: Abstract Operations ============

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Backend binary name for display (e.g. "container", "podman", "docker")
    fn backend_name(&self) -> &str;

    /// Check whether the backend is available and functional.
    async fn check_available(&self) -> Result<()>;

    /// Run a container (create + start).
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Create a container without starting it.
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Start an existing stopped container.
    async fn start(&self, id: &str) -> Result<()>;

    /// Stop a running container.
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;

    /// Remove a container.
    async fn remove(&self, id: &str, force: bool) -> Result<()>;

    /// List all containers.
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;

    /// Inspect a container for metadata.
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;

    /// Fetch logs from a container.
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs>;

    /// Execute a command inside a running container.
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs>;

    /// Pull an image from a registry.
    async fn pull_image(&self, reference: &str) -> Result<()>;

    /// List locally-available images.
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;

    /// Remove an image.
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;

    /// Create an OCI network.
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;

    /// Remove an OCI network.
    async fn remove_network(&self, name: &str) -> Result<()>;

    /// Create an OCI volume.
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;

    /// Remove an OCI volume.
    async fn remove_volume(&self, name: &str) -> Result<()>;

    /// Build an image from a BuildSpec.
    async fn build(&self, image_tag: &str, spec: &crate::types::ComposeServiceBuild) -> Result<()>;
}

// ============ Layer 2: CLI Protocol ============

/// Translates abstract container operations into CLI arguments.
pub trait CliProtocol: Send + Sync {
    fn subcommand_prefix(&self) -> Option<&str> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn start_args(&self, id: &str) -> Vec<String>;
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String>;
    fn remove_args(&self, id: &str, force: bool) -> Vec<String>;
    fn list_args(&self, all: bool) -> Vec<String>;
    fn inspect_args(&self, id: &str) -> Vec<String>;
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String>;
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String>;
    fn pull_image_args(&self, reference: &str) -> Vec<String>;
    fn list_images_args(&self) -> Vec<String>;
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String>;
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String>;
    fn remove_network_args(&self, name: &str) -> Vec<String>;
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String>;
    fn remove_volume_args(&self, name: &str) -> Vec<String>;
    fn build_args(&self, image_tag: &str, spec: &crate::types::ComposeServiceBuild) -> Vec<String>;

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>>;
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo>;
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>>;
    fn parse_container_id(&self, stdout: &str) -> Result<String>;
}

/// Docker-compatible CLI protocol (podman, nerdctl, orbstack, docker, colima).
pub struct DockerProtocol;

impl CliProtocol for DockerProtocol {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into(), "--detach".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        for port in spec.ports.iter().flatten() { args.extend(["-p".into(), port.clone()]); }
        for vol in spec.volumes.iter().flatten() { args.extend(["-v".into(), vol.clone()]); }
        for (k, v) in spec.env.iter().flatten() { args.extend(["-e".into(), format!("{k}={v}")]); }
        if let Some(net) = &spec.network { args.extend(["--network".into(), net.clone()]); }
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        if let Some(ep) = &spec.entrypoint {
            args.push("--entrypoint".into());
            args.push(ep.join(" "));
        }
        args.push(spec.image.clone());
        args.extend(spec.cmd.iter().flatten().cloned());
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        for port in spec.ports.iter().flatten() { args.extend(["-p".into(), port.clone()]); }
        for vol in spec.volumes.iter().flatten() { args.extend(["-v".into(), vol.clone()]); }
        for (k, v) in spec.env.iter().flatten() { args.extend(["-e".into(), format!("{k}={v}")]); }
        if let Some(net) = &spec.network { args.extend(["--network".into(), net.clone()]); }
        if let Some(ep) = &spec.entrypoint {
            args.push("--entrypoint".into());
            args.push(ep.join(" "));
        }
        args.push(spec.image.clone());
        args.extend(spec.cmd.iter().flatten().cloned());
        args
    }

    fn start_args(&self, id: &str) -> Vec<String> { vec!["start".into(), id.into()] }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout { args.extend(["--time".into(), t.to_string()]); }
        args.push(id.into());
        args
    }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".into()];
        if force { args.push("--force".into()); }
        args.push(id.into());
        args
    }
    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all { args.push("--all".into()); }
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
        if let Some(d) = workdir { args.extend(["--workdir".into(), d.into()]); }
        if let Some(env_map) = env {
            for (k, v) in env_map {
                args.extend(["-e".into(), format!("{k}={v}")]);
            }
        }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        args
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> { vec!["pull".into(), reference.into()] }
    fn list_images_args(&self) -> Vec<String> { vec!["images".into(), "--format".into(), "json".into()] }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".into()];
        if force { args.push("--force".into()); }
        args.push(reference.into());
        args
    }
    fn create_network_args(&self, name: &str, _config: &ComposeNetwork) -> Vec<String> { vec!["network".into(), "create".into(), name.into()] }
    fn remove_network_args(&self, name: &str) -> Vec<String> { vec!["network".into(), "rm".into(), name.into()] }
    fn create_volume_args(&self, name: &str, _config: &ComposeVolume) -> Vec<String> { vec!["volume".into(), "create".into(), name.into()] }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".into(), "rm".into(), name.into()] }
    fn build_args(&self, image_tag: &str, spec: &crate::types::ComposeServiceBuild) -> Vec<String> {
        let mut args = vec!["build".into(), "-t".into(), image_tag.into()];
        if let Some(ctx) = &spec.context {
            args.push(ctx.clone());
        } else {
            args.push(".".into());
        }
        if let Some(df) = &spec.dockerfile {
            args.extend(["-f".into(), df.clone()]);
        }
        if let Some(args_ld) = &spec.args {
            for (k, v) in args_ld.to_map() {
                args.extend(["--build-arg".into(), format!("{}={}", k, v)]);
            }
        }
        args
    }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        let mut containers = Vec::new();
        for line in stdout.lines() {
            if let Ok(info) = serde_json::from_str::<serde_json::Value>(line) {
                containers.push(ContainerInfo {
                    id: info["ID"].as_str().unwrap_or_default().into(),
                    name: info["Names"].as_str().unwrap_or_default().into(),
                    image: info["Image"].as_str().unwrap_or_default().into(),
                    status: info["Status"].as_str().unwrap_or_default().into(),
                    ports: vec![info["Ports"].as_str().unwrap_or_default().into()],
                    created: info["CreatedAt"].as_str().unwrap_or_default().into(),
                });
            }
        }
        Ok(containers)
    }

    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        let val: serde_json::Value = serde_json::from_str(stdout)?;
        let info = if val.is_array() { &val[0] } else { &val };
        Ok(ContainerInfo {
            id: info["Id"].as_str().unwrap_or_default().into(),
            name: info["Name"].as_str().unwrap_or_default().strip_prefix("/").unwrap_or_default().into(),
            image: info["Config"]["Image"].as_str().unwrap_or_default().into(),
            status: info["State"]["Status"].as_str().unwrap_or_default().into(),
            ports: Vec::new(), // TODO: Parse ports from NetworkSettings
            created: info["Created"].as_str().unwrap_or_default().into(),
        })
    }

    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        let mut images = Vec::new();
        for line in stdout.lines() {
            if let Ok(info) = serde_json::from_str::<serde_json::Value>(line) {
                images.push(ImageInfo {
                    id: info["ID"].as_str().unwrap_or_default().into(),
                    repository: info["Repository"].as_str().unwrap_or_default().into(),
                    tag: info["Tag"].as_str().unwrap_or_default().into(),
                    size: 0, // TODO: Parse size
                    created: info["CreatedAt"].as_str().unwrap_or_default().into(),
                });
            }
        }
        Ok(images)
    }

    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        Ok(stdout.trim().to_string())
    }
}

/// Apple Container CLI protocol.
pub struct AppleContainerProtocol;

impl CliProtocol for AppleContainerProtocol {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into()];
        if let Some(name) = &spec.name { args.extend(["--name".into(), name.clone()]); }
        for port in spec.ports.iter().flatten() { args.extend(["-p".into(), port.clone()]); }
        for vol in spec.volumes.iter().flatten() { args.extend(["-v".into(), vol.clone()]); }
        for (k, v) in spec.env.iter().flatten() { args.extend(["-e".into(), format!("{k}={v}")]); }
        if let Some(net) = &spec.network { args.extend(["--net".into(), net.clone()]); }
        // Apple Container might not support --detach in 'run', so we might use create+start or check flags
        args.push(spec.image.clone());
        args.extend(spec.cmd.iter().flatten().cloned());
        args
    }
    // Most other methods same as Docker
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> { DockerProtocol.create_args(spec) }
    fn start_args(&self, id: &str) -> Vec<String> { DockerProtocol.start_args(id) }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> { DockerProtocol.stop_args(id, timeout) }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> { DockerProtocol.remove_args(id, force) }
    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["list".into(), "--json".into()];
        if all { args.push("--all".into()); }
        args
    }
    fn inspect_args(&self, id: &str) -> Vec<String> { vec!["inspect".into(), "--json".into(), id.into()] }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> { DockerProtocol.logs_args(id, tail) }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> { DockerProtocol.exec_args(id, cmd, env, workdir) }
    fn pull_image_args(&self, reference: &str) -> Vec<String> { DockerProtocol.pull_image_args(reference) }
    fn list_images_args(&self) -> Vec<String> { vec!["images".into(), "--json".into()] }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> { DockerProtocol.remove_image_args(reference, force) }
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> { DockerProtocol.create_network_args(name, config) }
    fn remove_network_args(&self, name: &str) -> Vec<String> { DockerProtocol.remove_network_args(name) }
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> { DockerProtocol.create_volume_args(name, config) }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { DockerProtocol.remove_volume_args(name) }
    fn build_args(&self, image_tag: &str, spec: &crate::types::ComposeServiceBuild) -> Vec<String> {
        let mut args = vec!["build".into(), "--tag".into(), image_tag.into()];
        if let Some(ctx) = &spec.context {
            args.push(ctx.clone());
        } else {
            args.push(".".into());
        }
        if let Some(df) = &spec.dockerfile {
            args.extend(["--file".into(), df.clone()]);
        }
        args
    }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        // Apple Container might return a JSON array with lowercase keys
        if let Ok(list) = serde_json::from_str::<Vec<serde_json::Value>>(stdout) {
            return Ok(list.into_iter().map(|v| ContainerInfo {
                id: v["id"].as_str().or_else(|| v["ID"].as_str()).unwrap_or_default().to_string(),
                name: v["name"].as_str().or_else(|| v["Names"].as_str()).unwrap_or_default().to_string(),
                image: v["image"].as_str().or_else(|| v["Image"].as_str()).unwrap_or_default().to_string(),
                status: v["status"].as_str().or_else(|| v["Status"].as_str()).unwrap_or_default().to_string(),
                ports: v["ports"].as_array().map(|a| a.iter().filter_map(|p| p.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                created: v["createdAt"].as_str().or_else(|| v["CreatedAt"].as_str()).unwrap_or_default().to_string(),
            }).collect());
        }
        DockerProtocol.parse_list_output(stdout)
    }
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(stdout) {
            return Ok(ContainerInfo {
                id: v["id"].as_str().or_else(|| v["Id"].as_str()).unwrap_or_default().to_string(),
                name: v["name"].as_str().or_else(|| v["Name"].as_str()).unwrap_or_default().to_string(),
                image: v["image"].as_str().or_else(|| v["Config"]["Image"].as_str()).unwrap_or_default().to_string(),
                status: v["status"].as_str().or_else(|| v["State"]["Status"].as_str()).unwrap_or_default().to_string(),
                ports: v["ports"].as_array().map(|a| a.iter().filter_map(|p| p.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                created: v["createdAt"].as_str().or_else(|| v["Created"].as_str()).unwrap_or_default().to_string(),
            });
        }
        DockerProtocol.parse_inspect_output(stdout)
    }
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        if let Ok(list) = serde_json::from_str::<Vec<serde_json::Value>>(stdout) {
            return Ok(list.into_iter().map(|v| ImageInfo {
                id: v["id"].as_str().or_else(|| v["ID"].as_str()).unwrap_or_default().to_string(),
                repository: v["repository"].as_str().or_else(|| v["Repository"].as_str()).unwrap_or_default().to_string(),
                tag: v["tag"].as_str().or_else(|| v["Tag"].as_str()).unwrap_or_default().to_string(),
                size: v["size"].as_u64().unwrap_or_default(),
                created: v["createdAt"].as_str().or_else(|| v["CreatedAt"].as_str()).unwrap_or_default().to_string(),
            }).collect());
        }
        DockerProtocol.parse_list_images_output(stdout)
    }
    fn parse_container_id(&self, stdout: &str) -> Result<String> { DockerProtocol.parse_container_id(stdout) }
}

/// Lima CLI protocol.
pub struct LimaProtocol { pub instance: String }

impl CliProtocol for LimaProtocol {
    fn subcommand_prefix(&self) -> Option<&str> { Some("shell") }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.run_args(spec));
        args
    }
    // Wrap all other methods similarly
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.create_args(spec));
        args
    }
    fn start_args(&self, id: &str) -> Vec<String> {
        vec!["shell".into(), self.instance.clone(), "nerdctl".into(), "start".into(), id.into()]
    }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.stop_args(id, timeout));
        args
    }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_args(id, force));
        args
    }
    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.list_args(all));
        args
    }
    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["shell".into(), self.instance.clone(), "nerdctl".into(), "inspect".into(), "--format".into(), "json".into(), id.into()]
    }
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.logs_args(id, tail));
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.exec_args(id, cmd, env, workdir));
        args
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> {
        vec!["shell".into(), self.instance.clone(), "nerdctl".into(), "pull".into(), reference.into()]
    }
    fn list_images_args(&self) -> Vec<String> {
        vec!["shell".into(), self.instance.clone(), "nerdctl".into(), "images".into(), "--format".into(), "json".into()]
    }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_image_args(reference, force));
        args
    }
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.create_network_args(name, config));
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> {
        vec!["shell".into(), self.instance.clone(), "nerdctl".into(), "network".into(), "rm".into(), name.into()]
    }
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.create_volume_args(name, config));
        args
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        vec!["shell".into(), self.instance.clone(), "nerdctl".into(), "volume".into(), "rm".into(), name.into()]
    }
    fn build_args(&self, image_tag: &str, spec: &crate::types::ComposeServiceBuild) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.build_args(image_tag, spec));
        args
    }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> { DockerProtocol.parse_list_output(stdout) }
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> { DockerProtocol.parse_inspect_output(stdout) }
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> { DockerProtocol.parse_list_images_output(stdout) }
    fn parse_container_id(&self, stdout: &str) -> Result<String> { DockerProtocol.parse_container_id(stdout) }
}

// ============ Layer 3: CLI Executor ============

pub struct CliBackend {
    pub bin: PathBuf,
    pub protocol: Box<dyn CliProtocol>,
}

impl CliBackend {
    pub fn new(bin: PathBuf, protocol: Box<dyn CliProtocol>) -> Self {
        Self { bin, protocol }
    }

    async fn exec(&self, args: &[String]) -> Result<String> {
        let output = Command::new(&self.bin)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

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
        self.bin.file_name().and_then(|n| n.to_str()).unwrap_or("unknown")
    }

    async fn check_available(&self) -> Result<()> {
        let output = Command::new(&self.bin).arg("--version").output().await?;
        if output.status.success() { Ok(()) } else { Err(ComposeError::validation("Backend not available")) }
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec(&self.protocol.run_args(spec)).await?;
        Ok(ContainerHandle { id: self.protocol.parse_container_id(&stdout)?, name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec(&self.protocol.create_args(spec)).await?;
        Ok(ContainerHandle { id: self.protocol.parse_container_id(&stdout)?, name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> { self.exec(&self.protocol.start_args(id)).await?; Ok(()) }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> { self.exec(&self.protocol.stop_args(id, timeout)).await?; Ok(()) }
    async fn remove(&self, id: &str, force: bool) -> Result<()> { self.exec(&self.protocol.remove_args(id, force)).await?; Ok(()) }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let stdout = self.exec(&self.protocol.list_args(all)).await?;
        self.protocol.parse_list_output(&stdout)
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let stdout = self.exec(&self.protocol.inspect_args(id)).await?;
        self.protocol.parse_inspect_output(&stdout)
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut cmd = Command::new(&self.bin);
        cmd.args(self.protocol.logs_args(id, tail));
        let output = cmd.output().await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let mut command = Command::new(&self.bin);
        command.args(self.protocol.exec_args(id, cmd, env, workdir));
        let output = command.output().await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
    async fn pull_image(&self, reference: &str) -> Result<()> { self.exec(&self.protocol.pull_image_args(reference)).await?; Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let stdout = self.exec(&self.protocol.list_images_args()).await?;
        self.protocol.parse_list_images_output(&stdout)
    }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> { self.exec(&self.protocol.remove_image_args(reference, force)).await?; Ok(()) }
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> { self.exec(&self.protocol.create_network_args(name, config)).await?; Ok(()) }
    async fn remove_network(&self, name: &str) -> Result<()> { self.exec(&self.protocol.remove_network_args(name)).await?; Ok(()) }
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> { self.exec(&self.protocol.create_volume_args(name, config)).await?; Ok(()) }
    async fn remove_volume(&self, name: &str) -> Result<()> { self.exec(&self.protocol.remove_volume_args(name)).await?; Ok(()) }
    async fn build(&self, image_tag: &str, spec: &crate::types::ComposeServiceBuild) -> Result<()> {
        self.exec(&self.protocol.build_args(image_tag, spec)).await?;
        Ok(())
    }
}

// ============ Layer 4: Detection ============

pub async fn detect_backend() -> Result<CliBackend> {
    if let Ok(override_name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&override_name).await
            .map_err(|reason| ComposeError::BackendNotAvailable { name: override_name, reason });
    }

    let candidates = platform_candidates();
    let mut probed = Vec::new();

    for name in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(name)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => probed.push(BackendProbeResult { name: name.to_string(), available: false, reason }),
            Err(_) => probed.push(BackendProbeResult { name: name.to_string(), available: false, reason: "timeout".into() }),
        }
    }

    Err(ComposeError::NoBackendFound { probed })
}

fn platform_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &[
            "apple/container",
            "orbstack",
            "colima",
            "rancher-desktop",
            "podman",
            "lima",
            "docker",
        ]
    } else if cfg!(target_os = "linux") {
        &["podman", "nerdctl", "docker"]
    } else {
        &["podman", "nerdctl", "docker"]
    }
}

async fn probe_candidate(name: &str) -> std::result::Result<CliBackend, String> {
    match name {
        "apple/container" | "container" => {
            let bin = which("container").ok_or("binary not found")?;
            Ok(CliBackend::new(bin, Box::new(AppleContainerProtocol)))
        }
        "podman" => {
            let bin = which("podman").ok_or("binary not found")?;
            if cfg!(target_os = "macos") {
                let output = Command::new(&bin)
                    .args(["machine", "list", "--format", "json"])
                    .output()
                    .await
                    .map_err(|e| e.to_string())?;
                let val: serde_json::Value =
                    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
                let running = val
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .any(|m| m["Running"].as_bool().unwrap_or(false))
                    })
                    .unwrap_or(false);
                if !running {
                    return Err("podman machine not running".into());
                }
            }
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "orbstack" => {
            let bin = which("orb").or_else(|| which("docker")).ok_or("orbstack not found")?;
            let socket_path = format!("{}/.orbstack/run/docker.sock", std::env::var("HOME").unwrap_or_default());
            if !Path::new(&socket_path).exists() {
                 // Try version check if socket missing
                 let output = Command::new(&bin).arg("--version").output().await.map_err(|e| e.to_string())?;
                 if !output.status.success() { return Err("orbstack socket and version check failed".into()); }
            }
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "colima" => {
            let bin = which("colima").ok_or("colima binary not found")?;
            let output = Command::new(&bin)
                .arg("status")
                .output()
                .await
                .map_err(|e| e.to_string())?;
            if !String::from_utf8_lossy(&output.stdout).contains("running") {
                return Err("colima not running".into());
            }
            let docker_bin =
                which("docker").ok_or("docker binary not found (required for colima)")?;
            Ok(CliBackend::new(docker_bin, Box::new(DockerProtocol)))
        }
        "rancher-desktop" => {
            let bin = which("nerdctl").ok_or("nerdctl not found")?;
            let socket_path = format!("{}/.rd/run/containerd-shim.sock", std::env::var("HOME").unwrap_or_default());
            if !Path::new(&socket_path).exists() {
                return Err("rancher-desktop socket not found".into());
            }
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "lima" => {
            let bin = which("limactl").ok_or("limactl not found")?;
            let output = Command::new(&bin)
                .args(["list", "--json"])
                .output()
                .await
                .map_err(|e| e.to_string())?;
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let val: serde_json::Value =
                    serde_json::from_str(line).map_err(|e| e.to_string())?;
                if val["status"].as_str() == Some("Running") {
                    let instance = val["name"].as_str().unwrap_or("default").to_string();
                    return Ok(CliBackend::new(bin, Box::new(LimaProtocol { instance })));
                }
            }
            Err("no running lima instance found".into())
        }
        "docker" => {
            let bin = which("docker").ok_or("docker binary not found")?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "nerdctl" => {
            let bin = which("nerdctl").ok_or("nerdctl binary not found")?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        _ => Err("unknown candidate".into()),
    }
}

fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .filter_map(|dir| {
                let full_path = dir.join(bin);
                if full_path.is_file() { Some(full_path) } else { None }
            })
            .next()
    })
}
