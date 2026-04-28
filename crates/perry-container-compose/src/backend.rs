//! Container backend abstraction

use crate::error::{ComposeError, Result};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerInfo, ContainerLogs, ContainerSpec,
    ImageInfo, ComposeServiceBuild,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use std::sync::Arc;

pub use crate::error::BackendProbeResult;

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
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs>;
    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<()>;
    async fn pull_image(&self, reference: &str) -> Result<()>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;
    async fn inspect_image(&self, reference: &str) -> Result<ImageInfo>;
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn inspect_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
    async fn inspect_volume(&self, name: &str) -> Result<()>;
    async fn wait(&self, id: &str) -> Result<i32>;
    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs>;
    async fn manifest_inspect(&self, reference: &str) -> Result<serde_json::Value>;
}

pub trait CliProtocol: Send + Sync {
    fn protocol_name(&self) -> &str;
    fn subcommand_prefix(&self) -> Option<Vec<String>> { None }
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
    fn inspect_image_args(&self, reference: &str) -> Vec<String>;
    fn manifest_inspect_args(&self, reference: &str) -> Vec<String>;
    fn build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String>;
    fn create_network_args(&self, name: &str, config: &NetworkConfig) -> Vec<String>;
    fn remove_network_args(&self, name: &str) -> Vec<String>;
    fn inspect_network_args(&self, name: &str) -> Vec<String>;
    fn create_volume_args(&self, name: &str, config: &VolumeConfig) -> Vec<String>;
    fn remove_volume_args(&self, name: &str) -> Vec<String>;
    fn inspect_volume_args(&self, name: &str) -> Vec<String>;
    fn wait_args(&self, id: &str) -> Vec<String>;
    fn parse_list_output(&self, stdout: &str) -> Result<Vec<ContainerInfo>>;
    fn parse_inspect_output(&self, stdout: &str) -> Result<ContainerInfo>;
    fn parse_list_images_output(&self, stdout: &str) -> Result<Vec<ImageInfo>>;
    fn parse_inspect_image_output(&self, stdout: &str) -> Result<ImageInfo>;
    fn parse_container_id(&self, stdout: &str) -> Result<String>;
}

pub struct CliBackend<P: CliProtocol> { pub bin: PathBuf, pub protocol: P }
impl<P: CliProtocol> CliBackend<P> {
    pub fn new(bin: PathBuf, protocol: P) -> Self { Self { bin, protocol } }
    async fn exec_ok(&self, args: Vec<String>) -> Result<String> {
        let mut full = self.protocol.subcommand_prefix().unwrap_or_default();
        full.extend(args);
        let out = Command::new(&self.bin).args(&full).output().await.map_err(ComposeError::IoError)?;
        if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).to_string()) }
        else { Err(ComposeError::BackendError { code: out.status.code().unwrap_or(-1), message: String::from_utf8_lossy(&out.stderr).to_string() }) }
    }
}

#[async_trait]
impl<P: CliProtocol + Send + Sync> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str { self.protocol.protocol_name() }
    async fn check_available(&self) -> Result<()> { Ok(()) }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let id = self.protocol.parse_container_id(&self.exec_ok(self.protocol.run_args(spec)).await?)?;
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let id = self.protocol.parse_container_id(&self.exec_ok(self.protocol.create_args(spec)).await?)?;
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }
    async fn start(&self, id: &str) -> Result<()> { self.exec_ok(self.protocol.start_args(id)).await?; Ok(()) }
    async fn stop(&self, id: &str, t: Option<u32>) -> Result<()> { self.exec_ok(self.protocol.stop_args(id, t)).await?; Ok(()) }
    async fn remove(&self, id: &str, f: bool) -> Result<()> { self.exec_ok(self.protocol.remove_args(id, f)).await?; Ok(()) }
    async fn list(&self, a: bool) -> Result<Vec<ContainerInfo>> { self.protocol.parse_list_output(&self.exec_ok(self.protocol.list_args(a)).await?) }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> { self.protocol.parse_inspect_output(&self.exec_ok(self.protocol.inspect_args(id)).await?) }
    async fn logs(&self, id: &str, t: Option<u32>) -> Result<ContainerLogs> {
        let mut full = self.protocol.subcommand_prefix().unwrap_or_default();
        full.extend(self.protocol.logs_args(id, t));
        let out = Command::new(&self.bin).args(&full).output().await.map_err(ComposeError::IoError)?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&out.stdout).to_string(), stderr: String::from_utf8_lossy(&out.stderr).to_string() })
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, wd: Option<&str>) -> Result<ContainerLogs> {
        let mut full = self.protocol.subcommand_prefix().unwrap_or_default();
        full.extend(self.protocol.exec_args(id, cmd, env, wd));
        let out = Command::new(&self.bin).args(&full).output().await.map_err(ComposeError::IoError)?;
        Ok(ContainerLogs { stdout: String::from_utf8_lossy(&out.stdout).to_string(), stderr: String::from_utf8_lossy(&out.stderr).to_string() })
    }
    async fn build(&self, s: &ComposeServiceBuild, i: &str) -> Result<()> { self.exec_ok(self.protocol.build_args(s, i)).await?; Ok(()) }
    async fn pull_image(&self, r: &str) -> Result<()> { self.exec_ok(self.protocol.pull_image_args(r)).await?; Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { self.protocol.parse_list_images_output(&self.exec_ok(self.protocol.list_images_args()).await?) }
    async fn remove_image(&self, r: &str, f: bool) -> Result<()> { self.exec_ok(self.protocol.remove_image_args(r, f)).await?; Ok(()) }
    async fn inspect_image(&self, r: &str) -> Result<ImageInfo> { self.protocol.parse_inspect_image_output(&self.exec_ok(self.protocol.inspect_image_args(r)).await?) }
    async fn create_network(&self, n: &str, c: &NetworkConfig) -> Result<()> { self.exec_ok(self.protocol.create_network_args(n, c)).await?; Ok(()) }
    async fn remove_network(&self, n: &str) -> Result<()> { self.exec_ok(self.protocol.remove_network_args(n)).await?; Ok(()) }
    async fn inspect_network(&self, n: &str) -> Result<()> { self.exec_ok(self.protocol.inspect_network_args(n)).await?; Ok(()) }
    async fn create_volume(&self, n: &str, c: &VolumeConfig) -> Result<()> { self.exec_ok(self.protocol.create_volume_args(n, c)).await?; Ok(()) }
    async fn remove_volume(&self, n: &str) -> Result<()> { self.exec_ok(self.protocol.remove_volume_args(n)).await?; Ok(()) }
    async fn inspect_volume(&self, n: &str) -> Result<()> { self.exec_ok(self.protocol.inspect_volume_args(n)).await?; Ok(()) }
    async fn wait(&self, id: &str) -> Result<i32> { self.exec_ok(self.protocol.wait_args(id)).await?.trim().parse().map_err(|_| ComposeError::BackendError { code: -1, message: "Invalid wait output".into() }) }
    async fn wait_and_logs(&self, id: &str) -> Result<ContainerLogs> { self.wait(id).await?; self.logs(id, None).await }
    async fn manifest_inspect(&self, r: &str) -> Result<serde_json::Value> { serde_json::from_str(&self.exec_ok(self.protocol.manifest_inspect_args(r)).await?).map_err(ComposeError::JsonError) }
}

pub struct DockerProtocol;
impl CliProtocol for DockerProtocol {
    fn protocol_name(&self) -> &str { "docker" }
    fn run_args(&self, s: &ContainerSpec) -> Vec<String> { let mut a = vec!["run".into(), "--detach".into()]; a.extend(self.common(s)); a.push(s.image.clone()); if let Some(c) = &s.cmd { a.extend(c.iter().cloned()); } a }
    fn create_args(&self, s: &ContainerSpec) -> Vec<String> { let mut a = vec!["create".into()]; a.extend(self.common(s)); a.push(s.image.clone()); if let Some(c) = &s.cmd { a.extend(c.iter().cloned()); } a }
    fn start_args(&self, id: &str) -> Vec<String> { vec!["start".into(), id.into()] }
    fn stop_args(&self, id: &str, t: Option<u32>) -> Vec<String> { let mut a = vec!["stop".into()]; if let Some(v) = t { a.extend(["-t".into(), v.to_string()]); } a.push(id.into()); a }
    fn remove_args(&self, id: &str, f: bool) -> Vec<String> { let mut a = vec!["rm".into()]; if f { a.push("-f".into()); } a.push(id.into()); a }
    fn list_args(&self, a: bool) -> Vec<String> { let mut v = vec!["ps".into(), "--format".into(), "json".into()]; if a { v.push("--all".into()); } v }
    fn inspect_args(&self, id: &str) -> Vec<String> { vec!["inspect".into(), "--format".into(), "json".into(), id.into()] }
    fn logs_args(&self, id: &str, t: Option<u32>) -> Vec<String> { let mut a = vec!["logs".into()]; if let Some(v) = t { a.extend(["--tail".into(), v.to_string()]); } a.push(id.into()); a }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, wd: Option<&str>) -> Vec<String> {
        let mut a = vec!["exec".into()]; if let Some(w) = wd { a.extend(["--workdir".into(), w.into()]); }
        if let Some(e) = env { let mut ks: Vec<_> = e.keys().collect(); ks.sort(); for k in ks { a.extend(["-e".into(), format!("{}={}", k, e[k])]); } }
        a.push(id.into()); a.extend(cmd.iter().cloned()); a
    }
    fn pull_image_args(&self, r: &str) -> Vec<String> { vec!["pull".into(), r.into()] }
    fn list_images_args(&self) -> Vec<String> { vec!["images".into(), "--format".into(), "json".into()] }
    fn remove_image_args(&self, r: &str, f: bool) -> Vec<String> { let mut a = vec!["rmi".into()]; if f { a.push("-f".into()); } a.push(r.into()); a }
    fn inspect_image_args(&self, r: &str) -> Vec<String> { vec!["image".into(), "inspect".into(), "--format".into(), "json".into(), r.into()] }
    fn manifest_inspect_args(&self, r: &str) -> Vec<String> { vec!["manifest".into(), "inspect".into(), r.into()] }
    fn build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String> {
        let mut f = vec!["build".into(), "-t".into(), image_name.into()];
        if let Some(d) = &spec.dockerfile { f.extend(["-f".into(), d.into()]); }
        if let Some(args) = &spec.args {
            let m = args.to_map();
            let mut ks: Vec<_> = m.keys().collect();
            ks.sort();
            for k in ks { f.extend(["--build-arg".into(), format!("{}={}", k, m[k])]); }
        }
        f.push(spec.context.as_deref().unwrap_or(".").into());
        f
    }
    fn create_network_args(&self, n: &str, c: &NetworkConfig) -> Vec<String> {
        let mut a = vec!["network".into(), "create".into()]; if let Some(d) = &c.driver { a.extend(["--driver".into(), d.clone()]); }
        let mut ls: Vec<_> = c.labels.keys().collect(); ls.sort(); for k in ls { a.extend(["--label".into(), format!("{}={}", k, c.labels[k])]); }
        if c.internal { a.push("--internal".into()); } if c.enable_ipv6 { a.push("--ipv6".into()); }
        a.push(n.into()); a
    }
    fn remove_network_args(&self, n: &str) -> Vec<String> { vec!["network".into(), "rm".into(), n.into()] }
    fn inspect_network_args(&self, n: &str) -> Vec<String> { vec!["network".into(), "inspect".into(), n.into()] }
    fn create_volume_args(&self, n: &str, c: &VolumeConfig) -> Vec<String> {
        let mut a = vec!["volume".into(), "create".into()]; if let Some(d) = &c.driver { a.extend(["--driver".into(), d.clone()]); }
        let mut ls: Vec<_> = c.labels.keys().collect(); ls.sort(); for k in ls { a.extend(["--label".into(), format!("{}={}", k, c.labels[k])]); }
        a.push(n.into()); a
    }
    fn remove_volume_args(&self, n: &str) -> Vec<String> { vec!["volume".into(), "rm".into(), n.into()] }
    fn inspect_volume_args(&self, n: &str) -> Vec<String> { vec!["volume".into(), "inspect".into(), n.into()] }
    fn wait_args(&self, id: &str) -> Vec<String> { vec!["wait".into(), id.into()] }
    fn parse_list_output(&self, s: &str) -> Result<Vec<ContainerInfo>> {
        let v: Vec<serde_json::Value> = serde_json::from_str(s.trim()).unwrap_or_default();
        Ok(v.into_iter().map(|e| ContainerInfo {
            id: e["ID"].as_str().or(e["Id"].as_str()).unwrap_or_default().to_string(),
            name: e["Names"].as_str().or(e["names"].as_str()).unwrap_or_default().trim_start_matches('/').to_string(),
            image: e["Image"].as_str().unwrap_or_default().to_string(),
            status: e["Status"].as_str().unwrap_or_default().to_string(),
            ports: vec![], labels: HashMap::new(), created: "".into()
        }).collect())
    }
    fn parse_inspect_output(&self, s: &str) -> Result<ContainerInfo> {
        let v: serde_json::Value = serde_json::from_str(s.trim()).unwrap_or_default();
        let e = if v.is_array() { &v[0] } else { &v };
        Ok(ContainerInfo {
            id: e["Id"].as_str().unwrap_or_default().to_string(),
            name: e["Name"].as_str().unwrap_or_default().trim_start_matches('/').to_string(),
            image: e["Config"]["Image"].as_str().unwrap_or_default().to_string(),
            status: e["State"]["Status"].as_str().unwrap_or_default().to_string(),
            ports: vec![], labels: HashMap::new(), created: e["Created"].as_str().unwrap_or_default().to_string()
        })
    }
    fn parse_list_images_output(&self, s: &str) -> Result<Vec<ImageInfo>> {
        let v: Vec<serde_json::Value> = serde_json::from_str(s.trim()).unwrap_or_default();
        Ok(v.into_iter().map(|e| ImageInfo {
            id: e["ID"].as_str().unwrap_or_default().to_string(),
            repository: e["Repository"].as_str().unwrap_or_default().to_string(),
            tag: e["Tag"].as_str().unwrap_or_default().to_string(),
            size: e["Size"].as_u64().unwrap_or(0),
            created: e["CreatedSince"].as_str().unwrap_or_default().to_string()
        }).collect())
    }
    fn parse_inspect_image_output(&self, s: &str) -> Result<ImageInfo> {
        let v: serde_json::Value = serde_json::from_str(s.trim()).unwrap_or_default();
        let e = if v.is_array() { &v[0] } else { &v };
        Ok(ImageInfo {
            id: e["Id"].as_str().unwrap_or_default().to_string(),
            repository: "".into(), tag: "".into(),
            size: e["Size"].as_u64().unwrap_or(0),
            created: e["Created"].as_str().unwrap_or_default().to_string()
        })
    }
    fn parse_container_id(&self, s: &str) -> Result<String> { Ok(s.trim().to_string()) }
}

impl DockerProtocol {
    fn common(&self, s: &ContainerSpec) -> Vec<String> {
        let mut a = Vec::new();
        if s.rm.unwrap_or(false) { a.push("--rm".into()); }
        if let Some(n) = &s.name { a.extend(["--name".into(), n.clone()]); }
        if let Some(net) = &s.network { a.extend(["--network".into(), net.clone()]); }
        if let Some(ps) = &s.ports { for p in ps { a.extend(["-p".into(), p.clone()]); } }
        if let Some(vs) = &s.volumes { for v in vs { a.extend(["-v".into(), v.clone()]); } }
        if let Some(e) = &s.env {
            let mut keys: Vec<_> = e.keys().collect();
            keys.sort();
            for k in keys { a.extend(["-e".into(), format!("{}={}", k, e[k])]); }
        }
        if let Some(ep) = &s.entrypoint { a.extend(["--entrypoint".into(), ep.join(" ")]); }
        a
    }
}

pub struct AppleContainerProtocol;
impl CliProtocol for AppleContainerProtocol {
    fn protocol_name(&self) -> &str { "apple/container" }
    fn run_args(&self, s: &ContainerSpec) -> Vec<String> { let mut a = vec!["run".into()]; if s.rm.unwrap_or(false) { a.push("--rm".into()); } if let Some(n) = &s.name { a.extend(["--name".into(), n.clone()]); } a.push(s.image.clone()); if let Some(c) = &s.cmd { a.extend(c.iter().cloned()); } a }
    fn create_args(&self, s: &ContainerSpec) -> Vec<String> { DockerProtocol.create_args(s) }
    fn start_args(&self, id: &str) -> Vec<String> { DockerProtocol.start_args(id) }
    fn stop_args(&self, id: &str, t: Option<u32>) -> Vec<String> { DockerProtocol.stop_args(id, t) }
    fn remove_args(&self, id: &str, f: bool) -> Vec<String> { DockerProtocol.remove_args(id, f) }
    fn list_args(&self, a: bool) -> Vec<String> { DockerProtocol.list_args(a) }
    fn inspect_args(&self, id: &str) -> Vec<String> { DockerProtocol.inspect_args(id) }
    fn logs_args(&self, id: &str, t: Option<u32>) -> Vec<String> { DockerProtocol.logs_args(id, t) }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, wd: Option<&str>) -> Vec<String> { DockerProtocol.exec_args(id, cmd, env, wd) }
    fn pull_image_args(&self, r: &str) -> Vec<String> { DockerProtocol.pull_image_args(r) }
    fn list_images_args(&self) -> Vec<String> { DockerProtocol.list_images_args() }
    fn remove_image_args(&self, r: &str, f: bool) -> Vec<String> { DockerProtocol.remove_image_args(r, f) }
    fn inspect_image_args(&self, r: &str) -> Vec<String> { DockerProtocol.inspect_image_args(r) }
    fn manifest_inspect_args(&self, r: &str) -> Vec<String> { DockerProtocol.manifest_inspect_args(r) }
    fn build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String> { DockerProtocol.build_args(spec, image_name) }
    fn create_network_args(&self, n: &str, c: &NetworkConfig) -> Vec<String> { DockerProtocol.create_network_args(n, c) }
    fn remove_network_args(&self, n: &str) -> Vec<String> { DockerProtocol.remove_network_args(n) }
    fn inspect_network_args(&self, n: &str) -> Vec<String> { DockerProtocol.inspect_network_args(n) }
    fn create_volume_args(&self, n: &str, c: &VolumeConfig) -> Vec<String> { DockerProtocol.create_volume_args(n, c) }
    fn remove_volume_args(&self, n: &str) -> Vec<String> { DockerProtocol.remove_volume_args(n) }
    fn inspect_volume_args(&self, n: &str) -> Vec<String> { DockerProtocol.inspect_volume_args(n) }
    fn wait_args(&self, id: &str) -> Vec<String> { DockerProtocol.wait_args(id) }
    fn parse_list_output(&self, s: &str) -> Result<Vec<ContainerInfo>> { DockerProtocol.parse_list_output(s) }
    fn parse_inspect_output(&self, s: &str) -> Result<ContainerInfo> { DockerProtocol.parse_inspect_output(s) }
    fn parse_list_images_output(&self, s: &str) -> Result<Vec<ImageInfo>> { DockerProtocol.parse_list_images_output(s) }
    fn parse_inspect_image_output(&self, s: &str) -> Result<ImageInfo> { DockerProtocol.parse_inspect_image_output(s) }
    fn parse_container_id(&self, s: &str) -> Result<String> { DockerProtocol.parse_container_id(s) }
}

pub struct LimaProtocol { pub instance: String }
impl CliProtocol for LimaProtocol {
    fn protocol_name(&self) -> &str { "lima" }
    fn subcommand_prefix(&self) -> Option<Vec<String>> { Some(vec!["shell".into(), self.instance.clone(), "nerdctl".into()]) }
    fn run_args(&self, s: &ContainerSpec) -> Vec<String> { DockerProtocol.run_args(s) }
    fn create_args(&self, s: &ContainerSpec) -> Vec<String> { DockerProtocol.create_args(s) }
    fn start_args(&self, id: &str) -> Vec<String> { DockerProtocol.start_args(id) }
    fn stop_args(&self, id: &str, t: Option<u32>) -> Vec<String> { DockerProtocol.stop_args(id, t) }
    fn remove_args(&self, id: &str, f: bool) -> Vec<String> { DockerProtocol.remove_args(id, f) }
    fn list_args(&self, a: bool) -> Vec<String> { DockerProtocol.list_args(a) }
    fn inspect_args(&self, id: &str) -> Vec<String> { DockerProtocol.inspect_args(id) }
    fn logs_args(&self, id: &str, t: Option<u32>) -> Vec<String> { DockerProtocol.logs_args(id, t) }
    fn exec_args(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, wd: Option<&str>) -> Vec<String> { DockerProtocol.exec_args(id, cmd, env, wd) }
    fn pull_image_args(&self, r: &str) -> Vec<String> { DockerProtocol.pull_image_args(r) }
    fn list_images_args(&self) -> Vec<String> { DockerProtocol.list_images_args() }
    fn remove_image_args(&self, r: &str, f: bool) -> Vec<String> { DockerProtocol.remove_image_args(r, f) }
    fn inspect_image_args(&self, r: &str) -> Vec<String> { DockerProtocol.inspect_image_args(r) }
    fn manifest_inspect_args(&self, r: &str) -> Vec<String> { DockerProtocol.manifest_inspect_args(r) }
    fn build_args(&self, spec: &ComposeServiceBuild, image_name: &str) -> Vec<String> { DockerProtocol.build_args(spec, image_name) }
    fn create_network_args(&self, n: &str, c: &NetworkConfig) -> Vec<String> { DockerProtocol.create_network_args(n, c) }
    fn remove_network_args(&self, n: &str) -> Vec<String> { DockerProtocol.remove_network_args(n) }
    fn inspect_network_args(&self, n: &str) -> Vec<String> { DockerProtocol.inspect_network_args(n) }
    fn create_volume_args(&self, n: &str, c: &VolumeConfig) -> Vec<String> { DockerProtocol.create_volume_args(n, c) }
    fn remove_volume_args(&self, n: &str) -> Vec<String> { DockerProtocol.remove_volume_args(n) }
    fn inspect_volume_args(&self, n: &str) -> Vec<String> { DockerProtocol.inspect_volume_args(n) }
    fn wait_args(&self, id: &str) -> Vec<String> { DockerProtocol.wait_args(id) }
    fn parse_list_output(&self, s: &str) -> Result<Vec<ContainerInfo>> { DockerProtocol.parse_list_output(s) }
    fn parse_inspect_output(&self, s: &str) -> Result<ContainerInfo> { DockerProtocol.parse_inspect_output(s) }
    fn parse_list_images_output(&self, s: &str) -> Result<Vec<ImageInfo>> { DockerProtocol.parse_list_images_output(s) }
    fn parse_inspect_image_output(&self, s: &str) -> Result<ImageInfo> { DockerProtocol.parse_inspect_image_output(s) }
    fn parse_container_id(&self, s: &str) -> Result<String> { DockerProtocol.parse_container_id(s) }
}

pub async fn detect_backend() -> Result<Arc<dyn ContainerBackend>> {
    if let Ok(name) = std::env::var("PERRY_CONTAINER_BACKEND") { return probe_candidate(name.trim()).await; }
    for name in platform_candidates() {
        if let Ok(Ok(backend)) = tokio::time::timeout(std::time::Duration::from_secs(2), probe_candidate(name)).await { return Ok(backend); }
    }
    Err(ComposeError::NoBackendFound { probed: vec![] })
}

fn platform_candidates() -> &'static [&'static str] {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    { &["apple/container", "orbstack", "colima", "rancher-desktop", "lima", "podman", "docker"] }
    #[cfg(target_os = "linux")]
    { &["podman", "nerdctl", "docker"] }
    #[cfg(target_os = "windows")]
    { &["podman", "docker"] }
    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux", target_os = "windows")))]
    { &["podman", "docker"] }
}

async fn probe_candidate(name: &str) -> Result<Arc<dyn ContainerBackend>> {
    match name {
        "apple/container" => {
            let bin = which::which("container").map_err(|_| ComposeError::NotFound("container not found".into()))?;
            Ok(Arc::new(CliBackend::new(bin, AppleContainerProtocol)))
        }
        "orbstack" => {
            let bin = which::which("orb").or_else(|_| which::which("docker")).map_err(|_| ComposeError::NotFound("orbstack not found".into()))?;
            Ok(Arc::new(CliBackend::new(bin, DockerProtocol)))
        }
        "docker" => {
            let bin = which::which("docker").map_err(|_| ComposeError::NotFound("docker not found".into()))?;
            Ok(Arc::new(CliBackend::new(bin, DockerProtocol)))
        }
        "podman" => {
            let bin = which::which("podman").map_err(|_| ComposeError::NotFound("podman not found".into()))?;
            Ok(Arc::new(CliBackend::new(bin, DockerProtocol)))
        }
        "lima" => {
            let bin = which::which("limactl").map_err(|_| ComposeError::NotFound("limactl not found".into()))?;
            Ok(Arc::new(CliBackend::new(bin, LimaProtocol { instance: "default".into() })))
        }
        _ => Err(ComposeError::NotFound(format!("Unknown backend: {}", name))),
    }
}
