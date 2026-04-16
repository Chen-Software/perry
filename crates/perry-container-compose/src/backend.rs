//! Container backend abstraction and CLI implementations.

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

/// Layer 1: Abstract operations over any container backend.
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
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
}

/// Layer 2: CLI Protocol trait translates abstract ops into CLI args + parses output.
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

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>>;
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo>;
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>>;
    fn parse_container_id(&self, stdout: &str) -> Result<String>;
}

/// Docker-compatible protocol (podman, nerdctl, orbstack, docker, colima).
pub struct DockerProtocol;

impl CliProtocol for DockerProtocol {
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into(), "--detach".into()];
        if let Some(name) = &spec.name {
            args.extend(["--name".into(), name.clone()]);
        }
        if let Some(ports) = &spec.ports {
            for p in ports {
                args.extend(["-p".into(), p.clone()]);
            }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols {
                args.extend(["-v".into(), v.clone()]);
            }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env {
                args.extend(["-e".into(), format!("{}={}", k, v)]);
            }
        }
        if let Some(net) = &spec.network {
            args.extend(["--network".into(), net.clone()]);
        }
        if spec.rm.unwrap_or(false) {
            args.push("--rm".into());
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
            for p in ports {
                args.extend(["-p".into(), p.clone()]);
            }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols {
                args.extend(["-v".into(), v.clone()]);
            }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env {
                args.extend(["-e".into(), format!("{}={}", k, v)]);
            }
        }
        if let Some(net) = &spec.network {
            args.extend(["--network".into(), net.clone()]);
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
            args.push("--force".into());
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
        if let Some(e) = env {
            for (k, v) in e {
                args.extend(["-e".into(), format!("{}={}", k, v)]);
            }
        }
        if let Some(w) = workdir {
            args.extend(["--workdir".into(), w.into()]);
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
            args.push("--force".into());
        }
        args.push(reference.into());
        args
    }

    fn create_network_args(&self, name: &str, config: &ComposeNetwork) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver {
            args.extend(["--driver".into(), d.clone()]);
        }
        if let Some(labels) = &config.labels {
            for (k, v) in labels.to_map() {
                args.extend(["--label".into(), format!("{}={}", k, v)]);
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
            args.extend(["--driver".into(), d.clone()]);
        }
        if let Some(labels) = &config.labels {
            for (k, v) in labels.to_map() {
                args.extend(["--label".into(), format!("{}={}", k, v)]);
            }
        }
        args.push(name.into());
        args
    }

    fn remove_volume_args(&self, name: &str) -> Vec<String> {
        vec!["volume".into(), "rm".into(), name.into()]
    }

    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>> {
        #[derive(Deserialize)]
        struct DockerPs {
            #[serde(rename = "ID")]
            id: String,
            #[serde(rename = "Names")]
            names: String,
            #[serde(rename = "Image")]
            image: String,
            #[serde(rename = "Status")]
            status: String,
            #[serde(rename = "Ports")]
            ports: String,
            #[serde(rename = "CreatedAt")]
            created: String,
        }
        let stdout = stdout.trim();
        if stdout.is_empty() {
            return Ok(Vec::new());
        }
        let entries: Vec<DockerPs> = if stdout.starts_with('[') {
            serde_json::from_str(stdout)?
        } else {
            stdout
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        };
        Ok(entries
            .into_iter()
            .map(|e| ContainerInfo {
                id: e.id,
                name: e.names.split(',').next().unwrap_or("").to_string(),
                image: e.image,
                status: e.status,
                ports: e.ports.split(',').map(|s| s.trim().to_string()).collect(),
                created: e.created,
            })
            .collect())
    }

    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        #[derive(Deserialize)]
        #[allow(non_snake_case)]
        struct DockerInspect {
            Id: String,
            Name: String,
            Config: DockerConfig,
            State: DockerState,
            Created: String,
            NetworkSettings: DockerNetworkSettings,
        }
        #[derive(Deserialize)]
        #[allow(non_snake_case)]
        struct DockerConfig {
            Image: String,
        }
        #[derive(Deserialize)]
        #[allow(non_snake_case)]
        struct DockerState {
            Status: String,
        }
        #[derive(Deserialize)]
        #[allow(non_snake_case)]
        struct DockerNetworkSettings {
            Ports: HashMap<String, Option<serde_json::Value>>,
        }

        let entries: Vec<DockerInspect> = serde_json::from_str(stdout)?;
        let e = entries
            .into_iter()
            .next()
            .ok_or_else(|| ComposeError::NotFound("inspect output empty".into()))?;
        Ok(ContainerInfo {
            id: e.Id,
            name: e.Name.strip_prefix('/').unwrap_or(&e.Name).to_string(),
            image: e.Config.Image,
            status: e.State.Status,
            ports: e.NetworkSettings.Ports.keys().cloned().collect(),
            created: e.Created,
        })
    }

    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        #[derive(Deserialize)]
        struct DockerImage {
            #[serde(rename = "ID")]
            id: String,
            #[serde(rename = "Repository")]
            repository: String,
            #[serde(rename = "Tag")]
            tag: String,
            #[serde(rename = "Size")]
            size: String,
            #[serde(rename = "CreatedAt")]
            created: String,
        }
        let stdout = stdout.trim();
        if stdout.is_empty() {
            return Ok(Vec::new());
        }
        let entries: Vec<DockerImage> = if stdout.starts_with('[') {
            serde_json::from_str(stdout)?
        } else {
            stdout
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        };
        Ok(entries
            .into_iter()
            .map(|e| ImageInfo {
                id: e.id,
                repository: e.repository,
                tag: e.tag,
                size: e.size.parse().unwrap_or(0),
                created: e.created,
            })
            .collect())
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
        if let Some(name) = &spec.name {
            args.extend(["--name".into(), name.clone()]);
        }
        if let Some(ports) = &spec.ports {
            for p in ports {
                args.extend(["-p".into(), p.clone()]);
            }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols {
                args.extend(["-v".into(), v.clone()]);
            }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env {
                args.extend(["-e".into(), format!("{}={}", k, v)]);
            }
        }
        if let Some(net) = &spec.network {
            args.extend(["--network".into(), net.clone()]);
        }
        if spec.rm.unwrap_or(false) {
            args.push("--rm".into());
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
        args.push(spec.image.clone());
        args
    }

    fn start_args(&self, id: &str) -> Vec<String> {
        vec!["start".into(), id.into()]
    }
    fn stop_args(&self, id: &str, _timeout: Option<u32>) -> Vec<String> {
        vec!["stop".into(), id.into()]
    }
    fn remove_args(&self, id: &str, _force: bool) -> Vec<String> {
        vec!["rm".into(), id.into()]
    }
    fn list_args(&self, _all: bool) -> Vec<String> {
        vec!["ps".into(), "--format".into(), "json".into()]
    }
    fn inspect_args(&self, id: &str) -> Vec<String> {
        vec!["inspect".into(), "--format".into(), "json".into(), id.into()]
    }
    fn logs_args(&self, id: &str, _tail: Option<u32>) -> Vec<String> {
        vec!["logs".into(), id.into()]
    }
    fn exec_args(
        &self,
        id: &str,
        cmd: &[String],
        _env: Option<&HashMap<String, String>>,
        _workdir: Option<&str>,
    ) -> Vec<String> {
        let mut args = vec!["exec".into(), id.into()];
        args.extend(cmd.iter().cloned());
        args
    }
    fn pull_image_args(&self, reference: &str) -> Vec<String> {
        vec!["pull".into(), reference.into()]
    }
    fn list_images_args(&self) -> Vec<String> {
        vec!["images".into(), "--format".into(), "json".into()]
    }
    fn remove_image_args(&self, reference: &str, _force: bool) -> Vec<String> {
        vec!["rmi".into(), reference.into()]
    }
    fn create_network_args(&self, name: &str, _config: &ComposeNetwork) -> Vec<String> {
        vec!["network".into(), "create".into(), name.into()]
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
        DockerProtocol.parse_list_output(stdout)
    }
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo> {
        DockerProtocol.parse_inspect_output(stdout)
    }
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        DockerProtocol.parse_list_images_output(stdout)
    }
    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        DockerProtocol.parse_container_id(stdout)
    }
}

/// Lima protocol wraps nerdctl commands in `limactl shell <instance> nerdctl`.
pub struct LimaProtocol {
    pub instance: String,
}

impl CliProtocol for LimaProtocol {
    fn subcommand_prefix(&self) -> Option<&str> {
        None
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
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>> {
        DockerProtocol.parse_list_images_output(stdout)
    }
    fn parse_container_id(&self, stdout: &str) -> Result<String> {
        DockerProtocol.parse_container_id(stdout)
    }
}

/// Layer 3: Binary executor, implements ContainerBackend.
pub struct CliBackend {
    pub bin: PathBuf,
    pub protocol: Box<dyn CliProtocol>,
}

impl CliBackend {
    pub fn new(bin: PathBuf, protocol: Box<dyn CliProtocol>) -> Self {
        Self { bin, protocol }
    }

    async fn exec_raw(&self, args: &[String]) -> Result<(String, String)> {
        let output = Command::new(&self.bin)
            .args(args)
            .output()
            .await
            .map_err(ComposeError::IoError)?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if output.status.success() {
            Ok((stdout, stderr))
        } else {
            Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr,
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
        self.exec_raw(&["--version".into()]).await.map(|_| ())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.run_args(spec);
        let (stdout, _) = self.exec_raw(&args).await?;
        let id = self.protocol.parse_container_id(&stdout)?;
        Ok(ContainerHandle {
            id,
            name: spec.name.clone(),
        })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let args = self.protocol.create_args(spec);
        let (stdout, _) = self.exec_raw(&args).await?;
        let id = self.protocol.parse_container_id(&stdout)?;
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
        let (stdout, _) = self.exec_raw(&args).await?;
        self.protocol.parse_list_output(&stdout)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let args = self.protocol.inspect_args(id);
        let (stdout, _) = self.exec_raw(&args).await?;
        self.protocol.parse_inspect_output(&stdout)
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let args = self.protocol.logs_args(id, tail);
        let (stdout, stderr) = self.exec_raw(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
    ) -> Result<ContainerLogs> {
        let args = self.protocol.exec_args(id, cmd, env, workdir);
        let (stdout, stderr) = self.exec_raw(&args).await?;
        Ok(ContainerLogs { stdout, stderr })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let args = self.protocol.pull_image_args(reference);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let args = self.protocol.list_images_args();
        let (stdout, _) = self.exec_raw(&args).await?;
        self.protocol.parse_list_images_output(&stdout)
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
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        let args = self.protocol.create_volume_args(name, config);
        self.exec_raw(&args).await.map(|_| ())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let args = self.protocol.remove_volume_args(name);
        self.exec_raw(&args).await.map(|_| ())
    }
}

/// Result of a backend candidate probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendProbeResult {
    pub name: String,
    pub available: bool,
    pub reason: String,
}

/// Probes for available container runtimes and returns the first available one.
pub async fn detect_backend() -> Result<CliBackend, Vec<BackendProbeResult>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(&name).await.map_err(|reason| {
            vec![BackendProbeResult {
                name: name.clone(),
                available: false,
                reason,
            }]
        });
    }

    let candidates = platform_candidates();
    let mut results = Vec::new();

    for candidate in candidates {
        match tokio::time::timeout(Duration::from_secs(2), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => {
                tracing::debug!(backend = candidate, "container backend detected");
                return Ok(backend);
            }
            Ok(Err(reason)) => {
                tracing::debug!(backend = candidate, reason = %reason, "backend probe failed");
                results.push(BackendProbeResult {
                    name: candidate.to_string(),
                    available: false,
                    reason,
                });
            }
            Err(_) => {
                results.push(BackendProbeResult {
                    name: candidate.to_string(),
                    available: false,
                    reason: "probe timed out after 2s".to_string(),
                });
            }
        }
    }

    Err(results)
}

fn platform_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
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

async fn probe_candidate(name: &str) -> Result<CliBackend, String> {
    match name {
        "apple/container" => {
            let bin = which("container").map_err(|_| "container binary not found on PATH".to_string())?;
            run_version_check(&bin).await?;
            Ok(CliBackend::new(bin, Box::new(AppleContainerProtocol)))
        }
        "podman" => {
            let bin = which("podman").map_err(|_| "podman binary not found on PATH".to_string())?;
            run_version_check(&bin).await?;
            if cfg!(target_os = "macos") {
                check_podman_machine_running(&bin).await?;
            }
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "orbstack" => {
            let bin = which("orb").or_else(|_| which("docker"))
                .map_err(|_| "orbstack not found".to_string())?;
            check_orbstack_socket_or_version(&bin).await?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "colima" => {
            let bin = which("colima").map_err(|_| "colima binary not found on PATH".to_string())?;
            check_colima_running(&bin).await?;
            let docker_bin = which("docker").map_err(|_| "docker CLI not found (needed for colima)".to_string())?;
            Ok(CliBackend::new(docker_bin, Box::new(DockerProtocol)))
        }
        "rancher-desktop" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl binary not found on PATH".to_string())?;
            run_version_check(&bin).await?;
            check_rancher_socket().await?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "lima" => {
            let bin = which("limactl").map_err(|_| "limactl binary not found on PATH".to_string())?;
            let instance = check_lima_running_instance(&bin).await?;
            Ok(CliBackend::new(bin, Box::new(LimaProtocol { instance })))
        }
        "nerdctl" => {
            let bin = which("nerdctl").map_err(|_| "nerdctl binary not found on PATH".to_string())?;
            run_version_check(&bin).await?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        "docker" => {
            let bin = which("docker").map_err(|_| "docker binary not found on PATH".to_string())?;
            run_version_check(&bin).await?;
            Ok(CliBackend::new(bin, Box::new(DockerProtocol)))
        }
        other => Err(format!("unknown backend: {other}")),
    }
}

fn which(name: &str) -> std::io::Result<PathBuf> {
    which::which(name).map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))
}

async fn run_version_check(bin: &Path) -> Result<(), String> {
    let output = Command::new(bin)
        .arg("--version")
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("version check failed: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

async fn check_podman_machine_running(bin: &Path) -> Result<(), String> {
    let output = Command::new(bin)
        .args(["machine", "list", "--format", "json"])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("podman machine list failed".to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("\"Running\":true") || stdout.contains("\"Running\": true") {
        Ok(())
    } else {
        Err("no running podman machine found".to_string())
    }
}

async fn check_orbstack_socket_or_version(bin: &Path) -> Result<(), String> {
    if bin.file_name().and_then(|n| n.to_str()) == Some("orb") {
        run_version_check(bin).await
    } else {
        let socket = home::home_dir()
            .ok_or_else(|| "could not find home dir".to_string())?
            .join(".orbstack/run/docker.sock");
        if socket.exists() {
            Ok(())
        } else {
            Err("orbstack socket not found".to_string())
        }
    }
}

async fn check_colima_running(bin: &Path) -> Result<(), String> {
    let output = Command::new(bin)
        .arg("status")
        .output()
        .await
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("running") {
        Ok(())
    } else {
        Err("colima is not running".to_string())
    }
}

async fn check_rancher_socket() -> Result<(), String> {
    let socket = home::home_dir()
        .ok_or_else(|| "could not find home dir".to_string())?
        .join(".rd/run/containerd-shim.sock");
    if socket.exists() {
        Ok(())
    } else {
        Err("rancher desktop socket not found".to_string())
    }
}

async fn check_lima_running_instance(bin: &Path) -> Result<String, String> {
    let output = Command::new(bin)
        .args(["list", "--json"])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
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
