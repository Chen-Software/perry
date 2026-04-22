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

/// Security profile for sandboxed OCI containers.
#[derive(Debug, Clone, Default)]
pub struct SecurityProfile {
    pub read_only_rootfs: bool,
    pub seccomp_profile: Option<String>,
    pub cap_drop: Vec<String>,
}

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
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;

    async fn inspect_network(&self, name: &str) -> Result<serde_json::Value>;
    async fn inspect_volume(&self, name: &str) -> Result<serde_json::Value>;

    async fn build_image(
        &self,
        context: &str,
        tag: &str,
        dockerfile: Option<&str>,
        args: Option<&HashMap<String, String>>,
    ) -> Result<()>;

    async fn inspect_image(&self, reference: &str) -> Result<serde_json::Value>;
    async fn manifest_inspect(&self, reference: &str) -> Result<serde_json::Value>;

    async fn run_with_security(
        &self,
        spec: &ContainerSpec,
        profile: &SecurityProfile,
    ) -> Result<ContainerHandle>;

    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs>;
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.2  CliProtocol trait
// ─────────────────────────────────────────────────────────────────────────────

pub trait CliProtocol: Send + Sync {
    fn protocol_name(&self) -> &'static str;
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        None
    }

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

    fn inspect_network_args(&self, name: &str) -> Vec<String> {
        vec!["network".into(), "inspect".into(), name.into()]
    }

    fn inspect_volume_args(&self, name: &str) -> Vec<String> {
        vec!["volume".into(), "inspect".into(), name.into()]
    }

    fn build_args(
        &self,
        context: &str,
        tag: &str,
        dockerfile: Option<&str>,
        args: Option<&HashMap<String, String>>,
    ) -> Vec<String> {
        let mut full_args = vec!["build".into(), "-t".into(), tag.into()];
        if let Some(df) = dockerfile {
            full_args.push("-f".into());
            full_args.push(df.into());
        }
        if let Some(a) = args {
            for (k, v) in a {
                full_args.push("--build-arg".into());
                full_args.push(format!("{}={}", k, v));
            }
        }
        full_args.push(context.into());
        full_args
    }

    fn inspect_image_args(&self, reference: &str) -> Vec<String> {
        vec!["image".into(), "inspect".into(), reference.into()]
    }

    fn manifest_inspect_args(&self, reference: &str) -> Vec<String> {
        vec!["manifest".into(), "inspect".into(), reference.into()]
    }

    fn run_with_security_args(
        &self,
        spec: &ContainerSpec,
        profile: &SecurityProfile,
    ) -> Vec<String> {
        let mut args = self.run_args(spec);
        let image_pos = args.iter().position(|s| s == &spec.image).unwrap_or(args.len());

        let mut security_flags = Vec::new();
        if profile.read_only_rootfs {
            security_flags.push("--read-only".into());
        }
        if let Some(p) = &profile.seccomp_profile {
            security_flags.push("--security-opt".into());
            security_flags.push(format!("seccomp={}", p));
        }
        for cap in &profile.cap_drop {
            security_flags.push("--cap-drop".into());
            security_flags.push(cap.clone());
        }

        for (i, flag) in security_flags.into_iter().enumerate() {
            args.insert(image_pos + i, flag);
        }
        args
    }

    fn wait_args(&self, id: &str) -> Vec<String> {
        vec!["wait".into(), id.into()]
    }

    // ── Output parsers ───────────────────────────────────────────────────────

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
                labels: e.config.map(|c| c.labels).unwrap_or_default(),
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

pub struct DockerProtocol;
impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &'static str {
        "docker"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.4  AppleContainerProtocol
// ─────────────────────────────────────────────────────────────────────────────

pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &'static str {
        "apple/container"
    }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into()];
        args.extend(docker_run_flags(spec, false));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd {
            args.extend(cmd.iter().cloned());
        }
        args
    }

    fn build_args(
        &self,
        context: &str,
        tag: &str,
        dockerfile: Option<&str>,
        args: Option<&HashMap<String, String>>,
    ) -> Vec<String> {
        let mut full_args = vec!["build".into(), "-t".into(), tag.into()];
        // Apple specific defaults
        full_args.extend(vec![
            "--cpus".into(), "2".into(),
            "--memory".into(), "2048MB".into(),
            "--arch".into(), "arm64".into(),
            "--os".into(), "linux".into(),
        ]);
        if let Some(df) = dockerfile {
            full_args.push("-f".into());
            full_args.push(df.into());
        }
        if let Some(a) = args {
            for (k, v) in a {
                full_args.push("--build-arg".into());
                full_args.push(format!("{}={}", k, v));
            }
        }
        full_args.push(context.into());
        full_args
    }

    fn parse_inspect_output(&self, id: &str, stdout: &str) -> Option<ContainerInfo> {
        #[derive(Debug, Deserialize)]
        struct AppleInspectEntry {
            configuration: AppleConfig,
            status: String,
        }
        #[derive(Debug, Deserialize)]
        struct AppleConfig {
            id: String,
            image: AppleImageRef,
            labels: HashMap<String, String>,
        }
        #[derive(Debug, Deserialize)]
        struct AppleImageRef {
            reference: String,
        }

        let entries: Vec<AppleInspectEntry> = serde_json::from_str(stdout).ok()?;
        let e = entries.first()?;
        Some(ContainerInfo {
            id: if e.configuration.id.is_empty() { id.to_string() } else { e.configuration.id.clone() },
            name: String::new(), // apple/container might not have separate names
            image: e.configuration.image.reference.clone(),
            status: e.status.clone(),
            ports: vec![],
            labels: e.configuration.labels.clone(),
            created: String::new(),
        })
    }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> {
        // Assume same schema as inspect but in a list
        self.parse_inspect_output("", stdout).into_iter().collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.5  LimaProtocol
// ─────────────────────────────────────────────────────────────────────────────

pub struct LimaProtocol {
    pub instance: String,
}
impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &'static str {
        "lima"
    }
    fn subcommand_prefix(&self) -> Option<Vec<String>> {
        Some(vec!["shell".into(), self.instance.clone(), "nerdctl".into()])
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4.6  CliBackend<P>
// ─────────────────────────────────────────────────────────────────────────────

pub struct CliBackend<P: CliProtocol> {
    pub bin: PathBuf,
    pub protocol: P,
}

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P) -> Self {
        Self { bin, protocol }
    }

    async fn exec_raw(&self, args: Vec<String>) -> Result<std::process::Output> {
        let mut full = self.protocol.subcommand_prefix().unwrap_or_default();
        full.extend(args);
        let output = Command::new(&self.bin)
            .args(&full)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(ComposeError::IoError)?;
        Ok(output)
    }

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
impl<P: CliProtocol> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str {
        self.bin.to_str().unwrap_or("unknown")
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
        let stdout = self.exec_ok(self.protocol.run_args(spec)).await?;
        let id = self.protocol.parse_container_id(&stdout);
        let name = spec.name.clone().or_else(|| Some(id.clone()));
        Ok(ContainerHandle { id, name })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(self.protocol.create_args(spec)).await?;
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
        self.protocol.parse_inspect_output(id, &stdout)
            .ok_or_else(|| ComposeError::NotFound(id.to_string()))
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let output = self.exec_raw(self.protocol.logs_args(id, tail)).await?;
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
        self.exec_ok(self.protocol.remove_image_args(reference, force)).await?;
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        self.exec_ok(self.protocol.create_network_args(name, config)).await?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let output = self.exec_raw(self.protocol.remove_network_args(name)).await?;
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
        self.exec_ok(self.protocol.create_volume_args(name, config)).await?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let output = self.exec_raw(self.protocol.remove_volume_args(name)).await?;
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

    async fn inspect_network(&self, name: &str) -> Result<serde_json::Value> {
        let stdout = self.exec_ok(self.protocol.inspect_network_args(name)).await?;
        serde_json::from_str(&stdout).map_err(ComposeError::JsonError)
    }

    async fn inspect_volume(&self, name: &str) -> Result<serde_json::Value> {
        let stdout = self.exec_ok(self.protocol.inspect_volume_args(name)).await?;
        serde_json::from_str(&stdout).map_err(ComposeError::JsonError)
    }

    async fn build_image(
        &self,
        context: &str,
        tag: &str,
        dockerfile: Option<&str>,
        args: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        self.exec_ok(self.protocol.build_args(context, tag, dockerfile, args)).await?;
        Ok(())
    }

    async fn inspect_image(&self, reference: &str) -> Result<serde_json::Value> {
        let stdout = self.exec_ok(self.protocol.inspect_image_args(reference)).await?;
        serde_json::from_str(&stdout).map_err(ComposeError::JsonError)
    }

    async fn manifest_inspect(&self, reference: &str) -> Result<serde_json::Value> {
        let stdout = self.exec_ok(self.protocol.manifest_inspect_args(reference)).await?;
        serde_json::from_str(&stdout).map_err(ComposeError::JsonError)
    }

    async fn run_with_security(
        &self,
        spec: &ContainerSpec,
        profile: &SecurityProfile,
    ) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(self.protocol.run_with_security_args(spec, profile)).await?;
        let id = self.protocol.parse_container_id(&stdout);
        let name = spec.name.clone().or_else(|| Some(id.clone()));
        Ok(ContainerHandle { id, name })
    }

    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs> {
        let _ = self.exec_ok(self.protocol.wait_args(id)).await?;
        self.logs(id, None).await
    }
}

pub type DockerBackend = CliBackend<DockerProtocol>;
pub type AppleBackend = CliBackend<AppleContainerProtocol>;
pub type LimaBackend = CliBackend<LimaProtocol>;

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
    #[serde(rename = "Labels", alias = "labels", default)]
    labels: serde_json::Value,
    #[serde(rename = "Created", alias = "created", default)]
    created: serde_json::Value,
}

impl DockerListEntry {
    fn into_container_info(self) -> ContainerInfo {
        let labels = match self.labels {
            serde_json::Value::Object(map) => map
                .into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                .collect(),
            serde_json::Value::String(s) if !s.is_empty() => s
                .split(',')
                .filter_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    Some((parts.next()?.to_string(), parts.next()?.to_string()))
                })
                .collect(),
            _ => HashMap::new(),
        };

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
            labels,
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
    #[serde(rename = "Config", alias = "config")]
    config: Option<DockerInspectConfig>,
    #[serde(rename = "State", alias = "state")]
    state: Option<DockerInspectState>,
    #[serde(rename = "Created", alias = "created", default)]
    created: String,
}

#[derive(Debug, Deserialize)]
struct DockerInspectConfig {
    #[serde(rename = "Labels", alias = "labels", default)]
    labels: HashMap<String, String>,
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
    if let Some(labels) = &spec.labels {
        let mut pairs: Vec<(&String, &String)> = labels.iter().collect();
        pairs.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in pairs {
            args.push("--label".into());
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
// 4.7  detect_backend() and probe_candidate()
// ─────────────────────────────────────────────────────────────────────────────

const PROBE_TIMEOUT_SECS: u64 = 2;

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

pub async fn probe_candidate(
    name: &str,
) -> std::result::Result<Box<dyn ContainerBackend>, String> {
    match name {
        "apple/container" => {
            let bin = which::which("container")
                .map_err(|_| "container binary not found on PATH".to_string())?;
            probe_run(bin.to_str().unwrap_or("container"), &["--version"]).await?;
            Ok(Box::new(AppleBackend::new(bin, AppleContainerProtocol)))
        }
        "orbstack" => {
            let orb_ok = which::which("orb").ok().is_some();
            let sock_ok = std::path::Path::new(&shellexpand::tilde("~/.orbstack/run/docker.sock").to_string()).exists();
            if orb_ok || sock_ok {
                let bin = which::which("docker").or_else(|_| which::which("orb"))
                    .map_err(|_| "orbstack: neither docker nor orb found".to_string())?;
                Ok(Box::new(DockerBackend::new(bin, DockerProtocol)))
            } else {
                Err("orbstack: neither orb command nor socket found".into())
            }
        }
        "colima" => {
            let bin = which::which("colima").map_err(|_| "colima not found".to_string())?;
            let status = probe_run(bin.to_str().unwrap_or("colima"), &["status"]).await?;
            if !status.to_lowercase().contains("running") {
                return Err("colima is not running".into());
            }
            let docker_bin = which::which("docker").map_err(|_| "docker CLI not found".to_string())?;
            Ok(Box::new(DockerBackend::new(docker_bin, DockerProtocol)))
        }
        "rancher-desktop" => {
            let bin = which::which("nerdctl").map_err(|_| "nerdctl not found".to_string())?;
            let sock = std::path::Path::new(&shellexpand::tilde("~/.rd/run/containerd-shim.sock").to_string()).exists();
            if sock {
                Ok(Box::new(DockerBackend::new(bin, DockerProtocol)))
            } else {
                Err("rancher-desktop: containerd socket missing".into())
            }
        }
        "podman" => {
            let bin = which::which("podman").map_err(|_| "podman not found".to_string())?;
            probe_run(bin.to_str().unwrap_or("podman"), &["--version"]).await?;
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                let machines = probe_run(bin.to_str().unwrap_or("podman"), &["machine", "list", "--format", "json"]).await?;
                let has_running = serde_json::from_str::<Vec<serde_json::Value>>(&machines).unwrap_or_default().iter().any(|m| m.get("Running").and_then(|v| v.as_bool()).unwrap_or(false));
                if !has_running { return Err("podman: no running machine".into()); }
            }
            Ok(Box::new(DockerBackend::new(bin, DockerProtocol)))
        }
        "lima" => {
            let bin = which::which("limactl").map_err(|_| "limactl not found".to_string())?;
            let list_out = probe_run(bin.to_str().unwrap_or("limactl"), &["list", "--json"]).await?;
            let instance = list_out.lines().filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find(|v| v.get("status").and_then(|s| s.as_str()).map(|s| s.eq_ignore_ascii_case("running")).unwrap_or(false))
                .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .ok_or_else(|| "lima: no running instance".to_string())?;
            Ok(Box::new(LimaBackend::new(bin, LimaProtocol { instance })))
        }
        "nerdctl" => {
            let bin = which::which("nerdctl").map_err(|_| "nerdctl not found".to_string())?;
            probe_run(bin.to_str().unwrap_or("nerdctl"), &["--version"]).await?;
            Ok(Box::new(DockerBackend::new(bin, DockerProtocol)))
        }
        "docker" => {
            let bin = which::which("docker").map_err(|_| "docker not found".to_string())?;
            probe_run(bin.to_str().unwrap_or("docker"), &["--version"]).await?;
            Ok(Box::new(DockerBackend::new(bin, DockerProtocol)))
        }
        other => Err(format!("unknown runtime '{}'", other)),
    }
}

pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend>, ComposeError> {
    if let Ok(override_name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        let name = override_name.trim().to_string();
        return probe_candidate(&name).await.map_err(|reason| ComposeError::BackendNotAvailable { name, reason });
    }

    let mut probed = Vec::new();
    for &candidate in platform_candidates() {
        match tokio::time::timeout(std::time::Duration::from_secs(PROBE_TIMEOUT_SECS), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => probed.push(BackendProbeResult { name: candidate.to_string(), available: false, reason }),
            Err(_) => probed.push(BackendProbeResult { name: candidate.to_string(), available: false, reason: "timed out".into() }),
        }
    }
    Err(ComposeError::NoBackendFound { probed })
}
