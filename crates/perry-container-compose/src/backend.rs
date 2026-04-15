//! Container backend abstraction and implementation.
//!
//! Separates the `ContainerBackend` async trait from the `CliProtocol` trait,
//! allowing different container runtimes (podman, docker, apple-container, etc.)
//! to be supported by the same generic `CliBackend` executor.

use crate::error::{BackendProbeResult, ComposeError, Result};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Layer 1: The public contract — what operations exist, completely runtime-agnostic.
#[async_trait]
pub trait ContainerBackend: Send + Sync {
    /// Backend name for display (e.g. "apple/container", "podman", "docker")
    fn backend_name(&self) -> &str;

    /// Check whether the backend binary is available and functional.
    async fn check_available(&self) -> Result<()>;

    /// Build an image from a context.
    async fn build(
        &self,
        context: &str,
        dockerfile: Option<&str>,
        tag: &str,
        args: Option<&HashMap<String, String>>,
        target: Option<&str>,
        network: Option<&str>,
    ) -> Result<()>;

    /// Run a container (create + start). Returns a handle.
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Create a container (without starting it).
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle>;

    /// Start an existing stopped container.
    async fn start(&self, id: &str) -> Result<()>;

    /// Stop a running container.
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;

    /// Remove a container.
    async fn remove(&self, id: &str, force: bool) -> Result<()>;

    /// List all containers.
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;

    /// Inspect a container.
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;

    /// Inspect an image.
    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo>;

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

    /// Pull an image.
    async fn pull_image(&self, reference: &str) -> Result<()>;

    /// List images.
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;

    /// Remove an image.
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;

    /// Create a network.
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;

    /// Remove a network.
    async fn remove_network(&self, name: &str) -> Result<()>;

    /// Create a volume.
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;

    /// Remove a volume.
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

/// Layer 2: CLI Protocol trait.
/// Separates *command building* from *command execution*.
pub trait CliProtocol: Send + Sync {
    fn subcommand_prefix(&self) -> Option<&str> {
        None
    }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn create_args(&self, spec: &ContainerSpec) -> Vec<String>;
    fn start_args(&self, id: &str) -> Vec<String>;
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String>;
    fn remove_args(&self, id: &str, force: bool) -> Vec<String>;
    fn list_args(&self, all: bool) -> Vec<String>;
    fn inspect_args(&self, id: &str) -> Vec<String>;
    fn inspect_image_args(&self, reference: &str) -> Vec<String>;
    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String>;
    fn exec_args(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String>;
    fn pull_image_args(&self, reference: &str) -> Vec<String>;
    fn build_args(
        &self,
        context: &str,
        dockerfile: Option<&str>,
        tag: &str,
        args: Option<&HashMap<String, String>>,
        target: Option<&str>,
        network: Option<&str>,
    ) -> Vec<String>;
    fn list_images_args(&self) -> Vec<String>;
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String>;
    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String>;
    fn remove_network_args(&self, name: &str) -> Vec<String>;
    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String>;
    fn remove_volume_args(&self, name: &str) -> Vec<String>;

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>>;
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo>;
    fn parse_inspect_image_output(&self, stdout: &str) -> Result<ImageInfo>;
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>>;
    fn parse_container_id(&self, stdout: &str) -> Result<String>;
}

/// Docker-compatible CLI protocol implementation.
pub struct DockerProtocol;

impl CliProtocol for DockerProtocol {
    fn build_args(
        &self,
        context: &str,
        dockerfile: Option<&str>,
        tag: &str,
        args: Option<&HashMap<String, String>>,
        target: Option<&str>,
        network: Option<&str>,
    ) -> Vec<String> {
        let mut cmd_args = vec!["build".into(), "-t".into(), tag.into()];
        if let Some(df) = dockerfile {
            cmd_args.extend(["-f".into(), df.into()]);
        }
        if let Some(ba) = args {
            for (k, v) in ba {
                cmd_args.extend(["--build-arg".into(), format!("{}={}", k, v)]);
            }
        }
        if let Some(t) = target {
            cmd_args.extend(["--target".into(), t.into()]);
        }
        if let Some(n) = network {
            cmd_args.extend(["--network".into(), n.into()]);
        }
        cmd_args.push(context.into());
        cmd_args
    }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into(), "--detach".into()];
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
                args.extend(["-e".into(), format!("{k}={v}")]);
            }
        }
        if let Some(net) = &spec.network {
            args.extend(["--network".into(), net.clone()]);
        }
        if spec.rm.unwrap_or(false) {
            args.push("--rm".into());
        }
        if spec.read_only.unwrap_or(false) {
            args.push("--read-only".into());
        }
        if let Some(ep) = &spec.entrypoint {
            args.extend(["--entrypoint".into(), ep.join(" ")]);
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.iter().cloned());
        }
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".into()];
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
                args.extend(["-e".into(), format!("{k}={v}")]);
            }
        }
        if let Some(net) = &spec.network {
            args.extend(["--network".into(), net.clone()]);
        }
        if spec.read_only.unwrap_or(false) {
            args.push("--read-only".into());
        }
        if let Some(ep) = &spec.entrypoint {
            args.extend(["--entrypoint".into(), ep.join(" ")]);
        }
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
            args.push("--all".into());
        }
        args
    }

    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".into(), "--format".into(), "json".into(), id.into()]
    }

    fn inspect_image_args(&self, reference: &str) -> Vec<String> {
        vec![
            "image".into(),
            "inspect".into(),
            "--format".into(),
            "json".into(),
            reference.into(),
        ]
    }

    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if let Some(t) = tail {
            args.extend(["--tail".into(), t.to_string()]);
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
        if let Some(envs) = env {
            for (k, v) in envs {
                args.extend(["-e".into(), format!("{k}={v}")]);
            }
        }
        if let Some(wd) = workdir {
            args.extend(["--workdir".into(), wd.into()]);
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
        if let Some(driver) = &config.driver {
            args.extend(["--driver".into(), driver.clone()]);
        }
        args.push(name.into());
        args
    }

    fn remove_network_args(&self, name: &str) -> Vec<String> {
        vec!["network".into(), "rm".into(), name.into()]
    }

    fn create_volume_args(&self, name: &str, _config: &ComposeVolume) -> Vec<String> {
        vec!["volume".into(), "create".into(), name.into()]
    }

    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        vec!["volume".into(), "rm".into(), name.into()]
    }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        let entries: Vec<serde_json::Value> = stdout
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        Ok(entries
            .into_iter()
            .map(|e| ContainerInfo {
                id: e["ID"].as_str().unwrap_or_default().to_string(),
                name: e["Names"]
                    .as_str()
                    .or_else(|| e["Names"].as_array().and_then(|a| a[0].as_str()))
                    .unwrap_or_default()
                    .to_string(),
                image: e["Image"].as_str().unwrap_or_default().to_string(),
                status: e["Status"].as_str().unwrap_or_default().to_string(),
                ports: vec![e["Ports"].as_str().unwrap_or_default().to_string()],
                created: e["CreatedAt"].as_str().unwrap_or_default().to_string(),
            })
            .collect())
    }

    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        let val: serde_json::Value = serde_json::from_str(stdout).map_err(ComposeError::JsonError)?;
        let e = if val.is_array() { &val[0] } else { &val };

        Ok(ContainerInfo {
            id: e["Id"].as_str().unwrap_or_default().to_string(),
            name: e["Name"].as_str().unwrap_or_default().trim_start_matches('/').to_string(),
            image: e["Config"]["Image"].as_str().unwrap_or_default().to_string(),
            status: e["State"]["Status"].as_str().unwrap_or_default().to_string(),
            ports: vec![],
            created: e["Created"].as_str().unwrap_or_default().to_string(),
        })
    }

    fn parse_inspect_image_output(&self, stdout: &str) -> Result<ImageInfo> {
        let val: serde_json::Value = serde_json::from_str(stdout).map_err(ComposeError::JsonError)?;
        let e = if val.is_array() { &val[0] } else { &val };

        Ok(ImageInfo {
            id: e["Id"].as_str().unwrap_or_default().to_string(),
            repository: String::new(),
            tag: String::new(),
            size: e["Size"].as_u64().unwrap_or(0),
            created: e["Created"].as_str().unwrap_or_default().to_string(),
        })
    }

    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        let entries: Vec<serde_json::Value> = stdout
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        Ok(entries
            .into_iter()
            .map(|e| ImageInfo {
                id: e["ID"].as_str().unwrap_or_default().to_string(),
                repository: e["Repository"].as_str().unwrap_or_default().to_string(),
                tag: e["Tag"].as_str().unwrap_or_default().to_string(),
                size: 0, // Not always easy to parse from common JSON formats
                created: e["CreatedAt"].as_str().unwrap_or_default().to_string(),
            })
            .collect())
    }

    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        Ok(stdout.trim().to_string())
    }
}

/// Apple Container CLI protocol implementation.
pub struct AppleContainerProtocol;

impl CliProtocol for AppleContainerProtocol {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        // Apple Container is mostly docker-compatible but we can override here if needed
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

    fn inspect_image_args(&self, reference: &str) -> Vec<String> {
        DockerProtocol.inspect_image_args(reference)
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

    fn build_args(
        &self,
        context: &str,
        dockerfile: Option<&str>,
        tag: &str,
        args: Option<&HashMap<String, String>>,
        target: Option<&str>,
        network: Option<&str>,
    ) -> Vec<String> {
        DockerProtocol.build_args(context, dockerfile, tag, args, target, network)
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

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        DockerProtocol.parse_list_output(stdout)
    }

    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        DockerProtocol.parse_inspect_output(stdout)
    }

    fn parse_inspect_image_output(&self, stdout: &str) -> Result<ImageInfo> {
        DockerProtocol.parse_inspect_image_output(stdout)
    }

    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        DockerProtocol.parse_list_images_output(stdout)
    }

    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        DockerProtocol.parse_container_id(stdout)
    }
}

/// Lima CLI protocol implementation.
pub struct LimaProtocol {
    pub instance: String,
}

impl CliProtocol for LimaProtocol {
    fn build_args(
        &self,
        context: &str,
        dockerfile: Option<&str>,
        tag: &str,
        args: Option<&HashMap<String, String>>,
        target: Option<&str>,
        network: Option<&str>,
    ) -> Vec<String> {
        let mut cmd_args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        cmd_args.extend(DockerProtocol.build_args(context, dockerfile, tag, args, target, network));
        cmd_args
    }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.run_args(spec));
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.create_args(spec));
        args
    }

    fn start_args(&self, id: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.start_args(id));
        args
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
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.inspect_args(id));
        args
    }

    fn inspect_image_args(&self, reference: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.inspect_image_args(reference));
        args
    }

    fn logs_args(&self, id: &str, tail: Option<u32>) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.logs_args(id, tail));
        args
    }

    fn exec_args(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.exec_args(id, cmd, env, workdir));
        args
    }

    fn pull_image_args(&self, reference: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.pull_image_args(reference));
        args
    }

    fn list_images_args(&self) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.list_images_args());
        args
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
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_network_args(name));
        args
    }

    fn create_volume_args(&self, name: &str, config: &ComposeVolume) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.create_volume_args(name, config));
        args
    }

    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        let mut args = vec!["shell".into(), self.instance.clone(), "nerdctl".into()];
        args.extend(DockerProtocol.remove_volume_args(name));
        args
    }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        DockerProtocol.parse_list_output(stdout)
    }

    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        DockerProtocol.parse_inspect_output(stdout)
    }

    fn parse_inspect_image_output(&self, stdout: &str) -> Result<ImageInfo> {
        DockerProtocol.parse_inspect_image_output(stdout)
    }

    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        DockerProtocol.parse_list_images_output(stdout)
    }

    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        DockerProtocol.parse_container_id(stdout)
    }
}

/// Generic CLI backend implementation.
pub struct CliBackend {
    pub bin: PathBuf,
    pub protocol: Box<dyn CliProtocol>,
}

impl CliBackend {
    pub fn new(bin: PathBuf, protocol: Box<dyn CliProtocol>) -> Self {
        Self { bin, protocol }
    }

    async fn exec_raw(&self, args: &[String]) -> Result<CliOutput> {
        let output = tokio::process::Command::new(&self.bin)
            .args(args)
            .output()
            .await
            .map_err(ComposeError::IoError)?;

        if output.status.success() {
            Ok(CliOutput {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
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
impl ContainerBackend for CliBackend {
    fn backend_name(&self) -> &str {
        self.bin.file_name().and_then(|n| n.to_str()).unwrap_or("unknown")
    }

    async fn check_available(&self) -> Result<()> {
        let args = vec!["--version".to_string()];
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn build(
        &self,
        context: &str,
        dockerfile: Option<&str>,
        tag: &str,
        args: Option<&HashMap<String, String>>,
        target: Option<&str>,
        network: Option<&str>,
    ) -> Result<()> {
        let args = self.protocol.build_args(context, dockerfile, tag, args, target, network);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.run_args(spec);
        let out = self.exec_raw(&args).await?;
        let id = self.protocol.parse_container_id(&out.stdout)?;
        Ok(ContainerHandle {
            id,
            name: spec.name.clone(),
        })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.create_args(spec);
        let out = self.exec_raw(&args).await?;
        let id = self.protocol.parse_container_id(&out.stdout)?;
        Ok(ContainerHandle {
            id,
            name: spec.name.clone(),
        })
    }

    async fn start(&self, id: &str) -> Result<()> {
        let args = self.protocol.start_args(id);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let args = self.protocol.stop_args(id, timeout);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_args(id, force);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let args = self.protocol.list_args(all);
        let out = self.exec_raw(&args).await?;
        self.protocol.parse_list_output(&out.stdout)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let args = self.protocol.inspect_args(id);
        let out = self.exec_raw(&args).await?;
        self.protocol.parse_inspect_output(&out.stdout)
    }

    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo> {
        let args = self.protocol.inspect_image_args(reference);
        let out = self.exec_raw(&args).await?;
        self.protocol.parse_inspect_image_output(&out.stdout)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let args = self.protocol.logs_args(id, tail);
        let out = self.exec_raw(&args).await?;
        Ok(ContainerLogs {
            stdout: out.stdout,
            stderr: out.stderr,
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
        let out = self.exec_raw(&args).await?;
        Ok(ContainerLogs {
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let args = self.protocol.pull_image_args(reference);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let args = self.protocol.list_images_args();
        let out = self.exec_raw(&args).await?;
        self.protocol.parse_list_images_output(&out.stdout)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let args = self.protocol.remove_image_args(reference, force);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> {
        let args = self.protocol.create_network_args(name, config);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_network_args(name);
        match self.exec_raw(&args).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.to_string().contains("not found") {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        let args = self.protocol.create_volume_args(name, config);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_volume_args(name);
        match self.exec_raw(&args).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.to_string().contains("not found") {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }
}

/// Detect the available container backend.
pub async fn detect_backend() -> std::result::Result<CliBackend, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await.map_err(|reason| {
            vec![BackendProbeResult {
                name,
                available: false,
                reason,
            }]
        });
    }

    let candidates = platform_candidates();
    let mut results = Vec::new();

    for candidate in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => results.push(BackendProbeResult {
                name: candidate.to_string(),
                available: false,
                reason,
            }),
            Err(_) => results.push(BackendProbeResult {
                name: candidate.to_string(),
                available: false,
                reason: "probe timed out".to_string(),
            }),
        }
    }

    Err(results)
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
        "apple/container" => {
            let bin = which::which("container").map_err(|_| "binary not found".to_string())?;
            let backend = CliBackend::new(bin, Box::new(AppleContainerProtocol));
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(backend)
        }
        "podman" => {
            let bin = which::which("podman").map_err(|_| "binary not found".to_string())?;
            if cfg!(target_os = "macos") {
                check_podman_machine_running(&bin).await?;
            }
            let backend = CliBackend::new(bin, Box::new(DockerProtocol));
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(backend)
        }
        "docker" => {
            let bin = which::which("docker").map_err(|_| "binary not found".to_string())?;
            let backend = CliBackend::new(bin, Box::new(DockerProtocol));
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(backend)
        }
        "orbstack" => {
            let bin = which::which("orb")
                .or_else(|_| which::which("docker"))
                .map_err(|_| "binary not found".to_string())?;
            check_orbstack_socket_or_version(&bin).await?;
            let backend = CliBackend::new(bin, Box::new(DockerProtocol));
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(backend)
        }
        "nerdctl" => {
            let bin = which::which("nerdctl").map_err(|_| "binary not found".to_string())?;
            let backend = CliBackend::new(bin, Box::new(DockerProtocol));
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(backend)
        }
        "lima" => {
            let bin = which::which("limactl").map_err(|_| "binary not found".to_string())?;
            let instance = check_lima_running_instance(&bin).await?;
            let backend = CliBackend::new(bin, Box::new(LimaProtocol { instance }));
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(backend)
        }
        "colima" => {
            let bin = which::which("colima").map_err(|_| "binary not found".to_string())?;
            check_colima_running(&bin).await?;
            let docker_bin = which::which("docker").map_err(|_| "docker binary not found".to_string())?;
            let backend = CliBackend::new(docker_bin, Box::new(DockerProtocol));
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(backend)
        }
        "rancher-desktop" => {
            let bin = which::which("nerdctl").map_err(|_| "nerdctl binary not found".to_string())?;
            check_rancher_socket().await?;
            let backend = CliBackend::new(bin, Box::new(DockerProtocol));
            backend.check_available().await.map_err(|e| e.to_string())?;
            Ok(backend)
        }
        _ => Err("unknown backend".into()),
    }
}

async fn check_podman_machine_running(bin: &Path) -> std::result::Result<(), String> {
    let out = tokio::process::Command::new(bin)
        .args(["machine", "list", "--format", "json"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.contains("\"Running\":true") || stdout.contains("\"Running\": true") {
        Ok(())
    } else {
        Err("no running podman machine found".to_string())
    }
}

async fn check_orbstack_socket_or_version(bin: &Path) -> std::result::Result<(), String> {
    let out = tokio::process::Command::new(bin)
        .arg("--version")
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if out.status.success() {
        Ok(())
    } else {
        Err("orbstack not functional".to_string())
    }
}

async fn check_lima_running_instance(bin: &Path) -> std::result::Result<String, String> {
    let out = tokio::process::Command::new(bin)
        .args(["list", "--json"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if val["status"] == "Running" {
                if let Some(name) = val["name"].as_str() {
                    return Ok(name.to_string());
                }
            }
        }
    }
    Err("no running lima instance found".to_string())
}

async fn check_colima_running(bin: &Path) -> std::result::Result<(), String> {
    let out = tokio::process::Command::new(bin)
        .arg("status")
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.contains("running") {
        Ok(())
    } else {
        Err("colima not running".to_string())
    }
}

async fn check_rancher_socket() -> std::result::Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let socket = PathBuf::from(home).join(".rd/run/containerd-shim.sock");
    if socket.exists() {
        Ok(())
    } else {
        Err("rancher desktop socket not found".to_string())
    }
}
