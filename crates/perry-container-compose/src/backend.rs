//! Container backend abstraction — `ContainerBackend` trait, `CliProtocol` trait,
//! protocol implementations (`DockerProtocol`, `AppleContainerProtocol`, `LimaProtocol`),
//! generic `CliBackend<P>`, and `detect_backend()`.

use crate::error::{ComposeError, Result};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tracing::debug;

// 4.8 BackendProbeResult — defined in error.rs, re-exported here
pub use crate::error::BackendProbeResult;

#[derive(Debug, Clone, Default)]
pub struct SecurityProfile {
    pub read_only_rootfs: bool,
    pub seccomp_profile: Option<String>,
    pub cap_drop: Vec<String>,
}

// 4.1 NetworkConfig and VolumeConfig
#[derive(Debug, Clone, Default)]
pub struct NetworkConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
    pub internal: bool,
    pub enable_ipv6: bool,
}

#[derive(Debug, Clone, Default)]
pub struct VolumeConfig {
    pub driver: Option<String>,
    pub labels: HashMap<String, String>,
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

// 4.1 ContainerBackend trait
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
    async fn logs(&self, id: &str, tail: Option<u32>, follow: bool) -> Result<ContainerLogs>;
    async fn exec(
        &self,
        id: &str,
        cmd: &[String],
        env: Option<&HashMap<String, String>>,
        workdir: Option<&str>,
        user: Option<&str>,
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
    async fn wait(&self, id: &str) -> Result<()>;
    async fn inspect_image(&self, reference: &str) -> Result<serde_json::Value>;
    async fn manifest_inspect(&self, reference: &str) -> Result<serde_json::Value>;
    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle>;
    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs>;
}

// OciBackend and BackendDriver for backward compatibility (perry-stdlib)
pub type OciBackend = CliBackend<DockerProtocol>;

#[derive(Debug, Clone)]
pub enum BackendDriver {
    Docker { bin: PathBuf },
    AppleContainer { bin: PathBuf },
    Lima { bin: PathBuf, instance: String },
}

impl BackendDriver {
    pub fn name(&self) -> &str { match self { Self::Docker { .. } => "docker", Self::AppleContainer { .. } => "apple/container", Self::Lima { .. } => "lima" } }
    pub fn bin(&self) -> &PathBuf { match self { Self::Docker { bin } | Self::AppleContainer { bin } | Self::Lima { bin, .. } => bin } }
}

pub struct OciCommandBuilder;
impl OciCommandBuilder {
    pub fn run_args(_d: &BackendDriver, s: &ContainerSpec) -> Vec<String> { DockerProtocol.run_args(s) }
    pub fn parse_container_id(s: &str) -> String { DockerProtocol.parse_container_id(s) }
}

// 4.2 CliProtocol trait
pub trait CliProtocol: Send + Sync {
    fn protocol_name(&self) -> &str;
    fn subcommand_prefix(&self) -> Option<Vec<String>> { None }

    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into()];
        args.extend(self.docker_run_flags(spec, true));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }

    fn create_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["create".into()];
        args.extend(self.docker_run_flags(spec, false));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }

    fn start_args(&self, id: &str) -> Vec<String> { vec!["start".into(), id.into()] }
    fn stop_args(&self, id: &str, timeout: Option<u32>) -> Vec<String> {
        let mut args = vec!["stop".into()];
        if let Some(t) = timeout { args.extend(vec!["-t".into(), t.to_string()]); }
        args.push(id.into());
        args
    }
    fn remove_args(&self, id: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rm".into()];
        if force { args.push("-f".into()); }
        args.push(id.into());
        args
    }
    fn list_args(&self, all: bool) -> Vec<String> {
        let mut args = vec!["ps".into(), "--format".into(), "json".into()];
        if all { args.push("--all".into()); }
        args
    }
    fn inspect_args(&self, id: &str) -> Vec<String> { vec!["inspect".into(), "--format".into(), "json".into(), id.into()] }
    fn logs_args(&self, id: &str, tail: Option<u32>, follow: bool) -> Vec<String> {
        let mut args = vec!["logs".into()];
        if follow { args.push("-f".into()); }
        if let Some(t) = tail { args.extend(vec!["--tail".into(), t.to_string()]); }
        args.push(id.into());
        args
    }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>, user: Option<&str>) -> Vec<String> {
        let mut args = vec!["exec".into()];
        if let Some(u) = user { args.extend(vec!["--user".into(), u.into()]); }
        if let Some(w) = workdir { args.extend(vec!["--workdir".into(), w.into()]); }
        if let Some(e) = env {
            for (k, v) in e { args.extend(vec!["-e".into(), format!("{}={}", k, v)]); }
        }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        args
    }
    fn pull_args(&self, reference: &str) -> Vec<String> { vec!["pull".into(), reference.into()] }
    fn list_images_args(&self) -> Vec<String> { vec!["images".into(), "--format".into(), "json".into()] }
    fn remove_image_args(&self, reference: &str, force: bool) -> Vec<String> {
        let mut args = vec!["rmi".into()];
        if force { args.push("-f".into()); }
        args.push(reference.into());
        args
    }
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String> {
        let mut args = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver { args.extend(vec!["--driver".into(), d.clone()]); }
        for (k, v) in &config.labels { args.extend(vec!["--label".into(), format!("{}={}", k, v)]); }
        if config.internal { args.push("--internal".into()); }
        if config.enable_ipv6 { args.push("--ipv6".into()); }
        args.push(name.into());
        args
    }
    fn remove_network_args(&self, name: &str) -> Vec<String> { vec!["network".into(), "rm".into(), name.into()] }
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String> {
        let mut args = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver { args.extend(vec!["--driver".into(), d.clone()]); }
        for (k, v) in &config.labels { args.extend(vec!["--label".into(), format!("{}={}", k, v)]); }
        args.push(name.into());
        args
    }
    fn remove_volume_args(&self, name: &str) -> Vec<String> { vec!["volume".into(), "rm".into(), name.into()] }
    fn build_args(&self, context: &str, tag: &str, dockerfile: Option<&str>, args: Option<&HashMap<String, String>>) -> Vec<String> {
        let mut full_args = vec!["build".into(), "-t".into(), tag.into()];
        if let Some(df) = dockerfile { full_args.extend(vec!["-f".into(), df.into()]); }
        if let Some(a) = args {
            for (k, v) in a { full_args.extend(vec!["--build-arg".into(), format!("{}={}", k, v)]); }
        }
        full_args.push(context.into());
        full_args
    }
    fn wait_args(&self, id: &str) -> Vec<String> { vec!["wait".into(), id.into()] }

    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> { parse_docker_list_json(stdout) }
    fn parse_inspect_output(&self, _id: &str, stdout: &str) -> Option<ContainerInfo> { parse_docker_inspect_json(stdout) }
    fn parse_list_images_output(&self, stdout: &str) -> Vec<ImageInfo> { parse_docker_images_json(stdout) }
    fn parse_container_id(&self, stdout: &str) -> String { stdout.trim().to_string() }

    fn docker_run_flags(&self, spec: &ContainerSpec, detach: bool) -> Vec<String> {
        let mut args = Vec::new();
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        if detach { args.push("--detach".into()); }
        if let Some(n) = &spec.name { args.extend(vec!["--name".into(), n.clone()]); }
        if let Some(nw) = &spec.network { args.extend(vec!["--network".into(), nw.clone()]); }
        if let Some(ps) = &spec.ports { for p in ps { args.extend(vec!["-p".into(), p.clone()]); } }
        if let Some(vs) = &spec.volumes { for v in vs { args.extend(vec!["-v".into(), v.clone()]); } }
        if let Some(es) = &spec.env { for (k, v) in es { args.extend(vec!["-e".into(), format!("{}={}", k, v)]); } }
        if let Some(ls) = &spec.labels { for (k, v) in ls { args.extend(vec!["--label".into(), format!("{}={}", k, v)]); } }
        if let Some(ep) = &spec.entrypoint { args.extend(vec!["--entrypoint".into(), ep.join(" ")]); }
        args
    }
}

// 4.3 DockerProtocol
pub struct DockerProtocol;
impl CliProtocol for DockerProtocol { fn protocol_name(&self) -> &str { "docker" } }

// 4.4 AppleContainerProtocol
pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str { "apple/container" }
    fn run_args(&self, spec: &ContainerSpec) -> Vec<String> {
        let mut args = vec!["run".into()];
        args.extend(self.docker_run_flags(spec, false));
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        args
    }
    fn build_args(&self, context: &str, tag: &str, dockerfile: Option<&str>, args: Option<&HashMap<String, String>>) -> Vec<String> {
        let mut full_args = vec!["build".into(), "--cpus".into(), "2".into(), "--memory".into(), "2048MB".into(), "--arch".into(), "arm64".into(), "--os".into(), "linux".into(), "-t".into(), tag.into()];
        if let Some(df) = dockerfile { full_args.extend(vec!["-f".into(), df.into()]); }
        if let Some(a) = args {
            for (k, v) in a { full_args.extend(vec!["--build-arg".into(), format!("{}={}", k, v)]); }
        }
        full_args.push(context.into());
        full_args
    }
    fn parse_list_output(&self, stdout: &str) -> Vec<ContainerInfo> { parse_apple_container_json(stdout) }
    fn parse_inspect_output(&self, _id: &str, stdout: &str) -> Option<ContainerInfo> { parse_apple_container_json(stdout).into_iter().next() }
}

// 4.5 LimaProtocol
pub struct LimaProtocol { pub instance: String }
impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &str { "lima" }
    fn subcommand_prefix(&self) -> Option<Vec<String>> { Some(vec!["shell".into(), self.instance.clone(), "nerdctl".into()]) }
}

// 4.6 CliBackend<P: CliProtocol>
pub struct CliBackend<P: CliProtocol> { pub bin: PathBuf, pub protocol: P }
pub type DockerBackend = CliBackend<DockerProtocol>;
pub type AppleBackend = CliBackend<AppleContainerProtocol>;
pub type LimaBackend = CliBackend<LimaProtocol>;

impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P) -> Self { Self { bin, protocol } }
    async fn exec_raw(&self, args: Vec<String>) -> Result<std::process::Output> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() { cmd.args(prefix); }
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd.output().await.map_err(ComposeError::IoError)
    }
    async fn exec_ok(&self, args: Vec<String>) -> Result<String> {
        let out = self.exec_raw(args).await?;
        if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).to_string()) }
        else { Err(ComposeError::BackendError { code: out.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&out.stderr).to_string() }) }
    }
}

#[async_trait]
impl<P: CliProtocol> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str { self.bin.file_name().and_then(|n| n.to_str()).unwrap_or("unknown") }
    async fn check_available(&self) -> Result<()> {
        let mut cmd = Command::new(&self.bin);
        if let Some(prefix) = self.protocol.subcommand_prefix() { cmd.args(prefix); }
        let out = cmd.arg("--version").output().await.map_err(ComposeError::IoError)?;
        if out.status.success() { Ok(()) } else { Err(ComposeError::BackendError { code: out.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&out.stderr).to_string() }) }
    }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(self.protocol.run_args(spec)).await?;
        let id = self.protocol.parse_container_id(&stdout);
        Ok(ContainerHandle { id: id.clone(), name: spec.name.clone().or(Some(id)) })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let stdout = self.exec_ok(self.protocol.create_args(spec)).await?;
        let id = self.protocol.parse_container_id(&stdout);
        Ok(ContainerHandle { id: id.clone(), name: spec.name.clone().or(Some(id)) })
    }
    async fn start(&self, id: &str) -> Result<()> { self.exec_ok(self.protocol.start_args(id)).await.map(|_| ()) }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> { self.exec_ok(self.protocol.stop_args(id, timeout)).await.map(|_| ()) }
    async fn remove(&self, id: &str, force: bool) -> Result<()> { self.exec_ok(self.protocol.remove_args(id, force)).await.map(|_| ()) }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> { Ok(self.protocol.parse_list_output(&self.exec_ok(self.protocol.list_args(all)).await?)) }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let out = self.exec_raw(self.protocol.inspect_args(id)).await?;
        if out.status.success() { self.protocol.parse_inspect_output(id, &String::from_utf8_lossy(&out.stdout)).ok_or_else(|| ComposeError::NotFound(id.to_string())) }
        else { let stderr = String::from_utf8_lossy(&out.stderr); if is_not_found(&stderr) { Err(ComposeError::NotFound(id.to_string())) } else { Err(ComposeError::BackendError { code: out.status.code().unwrap_or(-1), message: stderr.to_string() }) } }
    }
    async fn logs(&self, id: &str, tail: Option<u32>, follow: bool) -> Result<ContainerLogs> {
        let out = self.exec_raw(self.protocol.logs_args(id, tail, follow)).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&out.stdout).to_string(), stderr: String::from_utf8_lossy(&out.stderr).to_string() })
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>, user: Option<&str>) -> Result<ContainerLogs> {
        let out = self.exec_raw(self.protocol.exec_args(id, cmd, env, workdir, user)).await?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&out.stdout).to_string(), stderr: String::from_utf8_lossy(&out.stderr).to_string() })
    }
    async fn pull_image(&self, reference: &str) -> Result<()> { self.exec_ok(self.protocol.pull_args(reference)).await.map(|_| ()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(self.protocol.parse_list_images_output(&self.exec_ok(self.protocol.list_images_args()).await?)) }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> { self.exec_ok(self.protocol.remove_image_args(reference, force)).await.map(|_| ()) }
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> { self.exec_ok(self.protocol.create_network_args(name, config)).await.map(|_| ()) }
    async fn remove_network(&self, name: &str) -> Result<()> {
        let out = self.exec_raw(self.protocol.remove_network_args(name)).await?;
        if out.status.success() || is_not_found(&String::from_utf8_lossy(&out.stderr)) { Ok(()) }
        else { Err(ComposeError::BackendError { code: out.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&out.stderr).to_string() }) }
    }
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> { self.exec_ok(self.protocol.create_volume_args(name, config)).await.map(|_| ()) }
    async fn remove_volume(&self, name: &str) -> Result<()> {
        let out = self.exec_raw(self.protocol.remove_volume_args(name)).await?;
        if out.status.success() || is_not_found(&String::from_utf8_lossy(&out.stderr)) { Ok(()) }
        else { Err(ComposeError::BackendError { code: out.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&out.stderr).to_string() }) }
    }
    async fn inspect_network(&self, name: &str) -> Result<serde_json::Value> { serde_json::from_str(&self.exec_ok(vec!["network".into(), "inspect".into(), name.into()]).await?).map_err(ComposeError::JsonError) }
    async fn inspect_volume(&self, name: &str) -> Result<serde_json::Value> { serde_json::from_str(&self.exec_ok(vec!["volume".into(), "inspect".into(), name.into()]).await?).map_err(ComposeError::JsonError) }
    async fn build_image(&self, context: &str, tag: &str, dockerfile: Option<&str>, args: Option<&HashMap<String, String>>) -> Result<()> { self.exec_ok(self.protocol.build_args(context, tag, dockerfile, args)).await.map(|_| ()) }
    async fn wait(&self, id: &str) -> Result<()> { self.exec_ok(self.protocol.wait_args(id)).await.map(|_| ()) }
    async fn inspect_image(&self, reference: &str) -> Result<serde_json::Value> { serde_json::from_str(&self.exec_ok(vec!["image".into(), "inspect".into(), reference.into()]).await?).map_err(ComposeError::JsonError) }
    async fn manifest_inspect(&self, reference: &str) -> Result<serde_json::Value> { serde_json::from_str(&self.exec_ok(vec!["manifest".into(), "inspect".into(), reference.into()]).await?).map_err(ComposeError::JsonError) }
    async fn run_with_security(&self, spec: &ContainerSpec, profile: &SecurityProfile) -> Result<ContainerHandle> {
        let mut args = self.protocol.run_args(spec);
        let pos = args.iter().position(|s| s == &spec.image).unwrap_or(args.len());
        if profile.read_only_rootfs { args.insert(pos, "--read-only".into()); }
        if let Some(p) = &profile.seccomp_profile { args.insert(pos, format!("--security-opt=seccomp={}", p)); }
        for c in &profile.cap_drop { args.insert(pos, format!("--cap-drop={}", c)); }
        let stdout = self.exec_ok(args).await?;
        let id = self.protocol.parse_container_id(&stdout);
        Ok(ContainerHandle { id: id.clone(), name: spec.name.clone().or(Some(id)) })
    }
    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs> { self.wait(id).await?; self.logs(id, None, false).await }
}

// 4.7 detect_backend()
const PROBE_TIMEOUT_SECS: u64 = 2;

fn platform_candidates() -> &'static [&'static str] {
    if cfg!(any(target_os = "macos", target_os = "ios")) { &["apple/container", "orbstack", "colima", "rancher-desktop", "podman", "lima", "docker"] }
    else { &["podman", "nerdctl", "docker"] }
}

pub async fn probe_candidate(name: &str) -> std::result::Result<Box<dyn ContainerBackend>, String> {
    match name {
        "apple/container" => {
            let bin = which::which("container").map_err(|_| "container not found")?;
            probe_run(bin.to_str().unwrap(), &["--version"]).await.map_err(|e| format!("apple/container failed: {}", e))?;
            Ok(Box::new(CliBackend::new(bin, AppleContainerProtocol)))
        }
        "orbstack" => {
            let orb_ok = which::which("orb").is_ok();
            let sock_ok = std::path::Path::new(&shellexpand::tilde("~/.orbstack/run/docker.sock").to_string()).exists();
            if orb_ok || sock_ok {
                let bin = which::which("docker").or_else(|_| which::which("orb")).map_err(|_| "orbstack: neither docker nor orb found")?;
                Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
            } else { Err("orbstack not found".into()) }
        }
        "colima" => {
            let bin = which::which("colima").map_err(|_| "colima not found")?;
            let status = probe_run(bin.to_str().unwrap(), &["status"]).await.map_err(|e| format!("colima status failed: {}", e))?;
            if !status.to_lowercase().contains("running") { return Err("colima not running".into()); }
            let docker_bin = which::which("docker").map_err(|_| "docker CLI not found (needed for colima)")?;
            Ok(Box::new(CliBackend::new(docker_bin, DockerProtocol)))
        }
        "rancher-desktop" => {
            let bin = which::which("nerdctl").map_err(|_| "nerdctl not found")?;
            let sock = std::path::Path::new(&shellexpand::tilde("~/.rd/run/containerd-shim.sock").to_string()).exists();
            if sock { Ok(Box::new(CliBackend::new(bin, DockerProtocol))) }
            else { Err("rancher-desktop containerd socket missing".into()) }
        }
        "podman" => {
            let bin = which::which("podman").map_err(|_| "podman not found")?;
            probe_run(bin.to_str().unwrap(), &["--version"]).await?;
            if cfg!(any(target_os = "macos", target_os = "ios")) {
                let machines = probe_run(bin.to_str().unwrap(), &["machine", "list", "--format", "json"]).await.unwrap_or_default();
                let has_running = serde_json::from_str::<Vec<serde_json::Value>>(&machines).unwrap_or_default().iter().any(|m| m.get("Running").and_then(|v| v.as_bool()).unwrap_or(false));
                if !has_running { return Err("podman: no running machine found".into()); }
            }
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        "lima" => {
            let bin = which::which("limactl").map_err(|_| "limactl not found")?;
            let list_out = probe_run(bin.to_str().unwrap(), &["list", "--json"]).await?;
            let instance = list_out.lines().filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok()).find(|v| v.get("status").and_then(|s| s.as_str()).map(|s| s.eq_ignore_ascii_case("running")).unwrap_or(false)).and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from)).ok_or_else(|| "limactl: no running instance found")?;
            Ok(Box::new(CliBackend::new(bin, LimaProtocol { instance })))
        }
        "nerdctl" | "docker" => {
            let bin = which::which(name).map_err(|_| format!("{} not found", name))?;
            probe_run(bin.to_str().unwrap(), &["--version"]).await?;
            Ok(Box::new(CliBackend::new(bin, DockerProtocol)))
        }
        _ => Err(format!("unknown runtime '{}'", name)),
    }
}

pub async fn detect_backend() -> std::result::Result<Box<dyn ContainerBackend>, ComposeError> {
    use std::time::Duration;
    if let Ok(override_name) = std::env::var("PERRY_CONTAINER_BACKEND") {
        return probe_candidate(override_name.trim()).await.map_err(|reason| ComposeError::BackendNotAvailable { name: override_name, reason });
    }
    let mut probed = Vec::new();
    for &candidate in platform_candidates() {
        match tokio::time::timeout(Duration::from_secs(PROBE_TIMEOUT_SECS), probe_candidate(candidate)).await {
            Ok(Ok(backend)) => return Ok(backend),
            Ok(Err(reason)) => probed.push(BackendProbeResult { name: candidate.to_string(), available: false, reason }),
            Err(_) => probed.push(BackendProbeResult { name: candidate.to_string(), available: false, reason: format!("probe timed out after {}s", PROBE_TIMEOUT_SECS) }),
        }
    }
    Err(ComposeError::NoBackendFound { probed })
}

async fn probe_run(bin: &str, args: &[&str]) -> std::result::Result<String, String> {
    let out = Command::new(bin).args(args).output().await.map_err(|e| e.to_string())?;
    if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).to_string()) } else { Err(String::from_utf8_lossy(&out.stderr).to_string()) }
}

fn is_not_found(stderr: &str) -> bool { let s = stderr.to_lowercase(); s.contains("not found") || s.contains("no such") || s.contains("does not exist") }

// ── JSON Parsers (Docker) ─────────────────────────────
#[derive(Deserialize)] struct DockerListEntry { #[serde(alias="Id")] id: String, #[serde(alias="Names")] names: serde_json::Value, #[serde(alias="Image")] image: String, #[serde(alias="Status")] status: String, #[serde(alias="Ports")] ports: serde_json::Value, #[serde(alias="Labels")] labels: serde_json::Value, #[serde(alias="Created")] created: serde_json::Value }
impl DockerListEntry {
    fn into_info(self) -> ContainerInfo {
        let labels = match self.labels { serde_json::Value::Object(map) => map.into_iter().filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string()))).collect(), _ => HashMap::new() };
        let name = match &self.names { serde_json::Value::Array(arr) => arr.first().and_then(|v| v.as_str()).map(|s| s.trim_start_matches('/').to_string()).unwrap_or_default(), serde_json::Value::String(s) => s.trim_start_matches('/').to_string(), _ => String::new() };
        ContainerInfo { id: self.id, name, image: self.image, status: self.status, ports: vec![], labels, created: "".into() }
    }
}
fn parse_docker_list_json(stdout: &str) -> Vec<ContainerInfo> { serde_json::from_str::<Vec<DockerListEntry>>(stdout.trim()).unwrap_or_default().into_iter().map(|e| e.into_info()).collect() }
fn parse_docker_inspect_json(stdout: &str) -> Option<ContainerInfo> { serde_json::from_str::<Vec<DockerListEntry>>(stdout.trim()).ok()?.into_iter().next().map(|e| e.into_info()) }
fn parse_docker_images_json(stdout: &str) -> Vec<ImageInfo> {
    #[derive(Deserialize)] struct Entry { #[serde(alias="Id")] id: String, #[serde(alias="Repository")] repository: String, #[serde(alias="Tag")] tag: String, #[serde(alias="Size")] size: serde_json::Value, #[serde(alias="Created")] created: String }
    serde_json::from_str::<Vec<Entry>>(stdout.trim()).unwrap_or_default().into_iter().map(|e| ImageInfo { id: e.id, repository: e.repository, tag: e.tag, size: match e.size { serde_json::Value::Number(n) => n.as_u64().unwrap_or(0), _ => 0 }, created: e.created }).collect()
}

// ── Apple JSON Parser ─────────────────────────────
fn parse_apple_container_json(stdout: &str) -> Vec<ContainerInfo> {
    #[derive(Deserialize)] struct Entry { configuration: Config, status: String }
    #[derive(Deserialize)] struct Config { id: String, image: ImageRef, labels: HashMap<String, String> }
    #[derive(Deserialize)] struct ImageRef { reference: String }
    serde_json::from_str::<Vec<Entry>>(stdout.trim()).unwrap_or_default().into_iter().map(|e| ContainerInfo { id: e.configuration.id.clone(), name: e.configuration.id, image: e.configuration.image.reference, status: e.status, ports: vec![], labels: e.configuration.labels, created: "".into() }).collect()
}
