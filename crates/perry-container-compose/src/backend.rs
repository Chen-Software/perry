use async_trait::async_trait;
use std::collections::HashMap;
use tokio::process::Command;
use crate::types::*;
use crate::error::ComposeError;
use anyhow::{Result, anyhow};

#[async_trait]
pub trait ContainerBackend: Send + Sync {
    fn backend_name(&self) -> &str;
    async fn check_available(&self) -> Result<()>;
    async fn run(&self, spec: &ContainerSpec) -> Result<String>;
    async fn create(&self, spec: &ContainerSpec) -> Result<String>;
    async fn start(&self, id: &str) -> Result<()>;
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()>;
    async fn remove(&self, id: &str, force: bool) -> Result<()>;
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>>;
    async fn inspect(&self, id: &str) -> Result<ContainerInfo>;
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs>;
    async fn exec(&self, id: &str, cmd: &[String],
                  env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs>;
    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<()>;
    async fn pull_image(&self, reference: &str) -> Result<()>;
    async fn list_images(&self) -> Result<Vec<ImageInfo>>;
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()>;
    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()>;
    async fn remove_network(&self, name: &str) -> Result<()>;
    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()>;
    async fn remove_volume(&self, name: &str) -> Result<()>;
    async fn inspect_network(&self, name: &str) -> Result<()>;
}

#[async_trait]
pub trait CliProtocol: Send + Sync {
    fn name(&self) -> &str;
    fn binary(&self) -> &str;
    async fn check_available(&self) -> Result<()>;
    fn get_args(&self, cmd: &str) -> Vec<String>;
}

pub struct DockerProtocol { pub binary: String }
#[async_trait]
impl CliProtocol for DockerProtocol {
    fn name(&self) -> &str { "docker" }
    fn binary(&self) -> &str { &self.binary }
    async fn check_available(&self) -> Result<()> {
        let output = Command::new(&self.binary).arg("info").output().await?;
        if output.status.success() { Ok(()) } else { Err(anyhow!("Docker-compatible runtime not running")) }
    }
    fn get_args(&self, _cmd: &str) -> Vec<String> { vec![] }
}

pub struct PodmanProtocol;
#[async_trait]
impl CliProtocol for PodmanProtocol {
    fn name(&self) -> &str { "podman" }
    fn binary(&self) -> &str { "podman" }
    async fn check_available(&self) -> Result<()> {
        let output = Command::new("podman").arg("info").output().await?;
        if output.status.success() { Ok(()) } else { Err(anyhow!("Podman not available")) }
    }
    fn get_args(&self, _cmd: &str) -> Vec<String> { vec![] }
}

pub struct AppleContainerProtocol;
#[async_trait]
impl CliProtocol for AppleContainerProtocol {
    fn name(&self) -> &str { "apple/container" }
    fn binary(&self) -> &str { "container" }
    async fn check_available(&self) -> Result<()> {
        let output = Command::new("container").arg("version").output().await?;
        if output.status.success() { Ok(()) } else { Err(anyhow!("Apple Container not available")) }
    }
    fn get_args(&self, _cmd: &str) -> Vec<String> { vec![] }
}

pub struct LimaProtocol { pub instance: Option<String> }
#[async_trait]
impl CliProtocol for LimaProtocol {
    fn name(&self) -> &str { "lima" }
    fn binary(&self) -> &str { "limactl" }
    async fn check_available(&self) -> Result<()> {
        let output = Command::new("limactl").arg("ls").arg("--format").arg("json").output().await?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                    if v["status"] == "Running" { return Ok(()); }
                }
            }
        }
        Err(anyhow!("No running Lima instance found"))
    }
    fn get_args(&self, _cmd: &str) -> Vec<String> {
        let mut args = vec!["shell".to_string()];
        if let Some(inst) = &self.instance { args.push(inst.clone()); }
        args.push("docker".to_string());
        args
    }
}

pub struct CliBackend<P: CliProtocol> {
    pub protocol: P,
}

#[async_trait]
impl<P: CliProtocol> ContainerBackend for CliBackend<P> {
    fn backend_name(&self) -> &str { self.protocol.name() }

    async fn check_available(&self) -> Result<()> { self.protocol.check_available().await }

    async fn run(&self, spec: &ContainerSpec) -> Result<String> {
        let mut args = self.protocol.get_args("run");
        args.push("run".to_string());
        args.push("-d".to_string());
        if let Some(true) = spec.rm { args.push("--rm".to_string()); }
        if let Some(name) = &spec.name { args.push("--name".to_string()); args.push(name.clone()); }
        if let Some(ports) = &spec.ports {
            for p in ports { args.push("-p".to_string()); args.push(p.clone()); }
        }
        if let Some(volumes) = &spec.volumes {
            for v in volumes { args.push("-v".to_string()); args.push(v.clone()); }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env { args.push("-e".to_string()); args.push(format!("{}={}", k, v)); }
        }
        if let Some(net) = &spec.network { args.push("--network".to_string()); args.push(net.clone()); }
        if let Some(ep) = &spec.entrypoint {
            args.push("--entrypoint".to_string());
            args.push(ep.join(" "));
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }

        let output = Command::new(self.protocol.binary()).args(&args).output().await?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow!("Failed to run container: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<String> {
        let mut args = self.protocol.get_args("create");
        args.push("create".to_string());
        if let Some(name) = &spec.name { args.push("--name".to_string()); args.push(name.clone()); }
        if let Some(ports) = &spec.ports {
            for p in ports { args.push("-p".to_string()); args.push(p.clone()); }
        }
        if let Some(volumes) = &spec.volumes {
            for v in volumes { args.push("-v".to_string()); args.push(v.clone()); }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env { args.push("-e".to_string()); args.push(format!("{}={}", k, v)); }
        }
        if let Some(net) = &spec.network { args.push("--network".to_string()); args.push(net.clone()); }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.clone()); }

        let output = Command::new(self.protocol.binary()).args(&args).output().await?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow!("Failed to create container: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }

    async fn start(&self, id: &str) -> Result<()> {
        let mut args = self.protocol.get_args("start");
        args.push("start".to_string());
        args.push(id.to_string());
        let status = Command::new(self.protocol.binary()).args(&args).status().await?;
        if status.success() { Ok(()) } else { Err(anyhow!("Failed to start container")) }
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let mut args = self.protocol.get_args("stop");
        args.push("stop".to_string());
        if let Some(t) = timeout { args.push("-t".to_string()); args.push(t.to_string()); }
        args.push(id.to_string());
        let status = Command::new(self.protocol.binary()).args(&args).status().await?;
        if status.success() { Ok(()) } else { Err(anyhow!("Failed to stop container")) }
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let mut args = self.protocol.get_args("rm");
        args.push("rm".to_string());
        if force { args.push("-f".to_string()); }
        args.push(id.to_string());
        let status = Command::new(self.protocol.binary()).args(&args).status().await?;
        if status.success() { Ok(()) } else { Err(anyhow!("Failed to remove container")) }
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let mut args = self.protocol.get_args("ps");
        args.push("ps".to_string());
        if all { args.push("-a".to_string()); }
        args.push("--format".to_string());
        args.push("json".to_string());

        let output = Command::new(self.protocol.binary()).args(&args).output().await?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut containers = Vec::new();
            for line in stdout.lines() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                    containers.push(ContainerInfo {
                        id: v["ID"].as_str().unwrap_or_default().to_string(),
                        name: v["Names"].as_str().unwrap_or_default().to_string(),
                        image: v["Image"].as_str().unwrap_or_default().to_string(),
                        status: v["Status"].as_str().unwrap_or_default().to_string(),
                        state: v["State"].as_str().unwrap_or_default().to_string(),
                        ports: v["Ports"].as_str().map(|s| s.split(", ").map(|p| p.to_string()).collect()).unwrap_or_default(),
                    });
                }
            }
            Ok(containers)
        } else {
            Err(anyhow!("Failed to list containers"))
        }
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let mut args = self.protocol.get_args("inspect");
        args.push("inspect".to_string());
        args.push(id.to_string());

        let output = Command::new(self.protocol.binary()).args(&args).output().await?;
        if output.status.success() {
            let v: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            let v = if v.is_array() { &v[0] } else { &v };
            Ok(ContainerInfo {
                id: v["Id"].as_str().unwrap_or_default().to_string(),
                name: v["Name"].as_str().unwrap_or_default().strip_prefix("/").unwrap_or_default().to_string(),
                image: v["Config"]["Image"].as_str().unwrap_or_default().to_string(),
                status: v["State"]["Status"].as_str().unwrap_or_default().to_string(),
                state: v["State"]["Status"].as_str().unwrap_or_default().to_string(),
                ports: vec![],
            })
        } else {
            Err(anyhow!("Failed to inspect container"))
        }
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut args = self.protocol.get_args("logs");
        args.push("logs".to_string());
        if let Some(t) = tail { args.push("--tail".to_string()); args.push(t.to_string()); }
        args.push(id.to_string());

        let output = Command::new(self.protocol.binary()).args(&args).output().await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let mut args = self.protocol.get_args("exec");
        args.push("exec".to_string());
        if let Some(e) = env {
            for (k, v) in e { args.push("-e".to_string()); args.push(format!("{}={}", k, v)); }
        }
        if let Some(w) = workdir { args.push("-w".to_string()); args.push(w.to_string()); }
        args.push(id.to_string());
        args.extend(cmd.iter().cloned());

        let output = Command::new(self.protocol.binary()).args(&args).output().await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn build(&self, spec: &ComposeServiceBuild, image_name: &str) -> Result<()> {
        let mut args = self.protocol.get_args("build");
        args.push("build".to_string());
        args.push("-t".to_string());
        args.push(image_name.to_string());
        if let Some(f) = &spec.dockerfile { args.push("-f".to_string()); args.push(f.clone()); }
        if let Some(a) = &spec.args {
            match a {
                ListOrDict::List(l) => { for item in l { args.push("--build-arg".to_string()); args.push(item.clone()); } }
                ListOrDict::Dict(d) => { for (k, v) in d {
                    args.push("--build-arg".to_string());
                    args.push(format!("{}={}", k, v.as_deref().unwrap_or("")));
                } }
            }
        }
        args.push(spec.context.clone());

        let status = Command::new(self.protocol.binary()).args(&args).status().await?;
        if status.success() { Ok(()) } else { Err(anyhow!("Failed to build image")) }
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let mut args = self.protocol.get_args("pull");
        args.push("pull".to_string());
        args.push(reference.to_string());
        let status = Command::new(self.protocol.binary()).args(&args).status().await?;
        if status.success() { Ok(()) } else { Err(anyhow!("Failed to pull image")) }
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let mut args = self.protocol.get_args("images");
        args.push("images".to_string());
        args.push("--format".to_string());
        args.push("json".to_string());

        let output = Command::new(self.protocol.binary()).args(&args).output().await?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut images = Vec::new();
            for line in stdout.lines() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                    images.push(ImageInfo {
                        id: v["ID"].as_str().unwrap_or_default().to_string(),
                        repository: v["Repository"].as_str().unwrap_or_default().to_string(),
                        tag: v["Tag"].as_str().unwrap_or_default().to_string(),
                        size: 0,
                    });
                }
            }
            Ok(images)
        } else {
            Err(anyhow!("Failed to list images"))
        }
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let mut args = self.protocol.get_args("rmi");
        args.push("rmi".to_string());
        if force { args.push("-f".to_string()); }
        args.push(reference.to_string());
        let status = Command::new(self.protocol.binary()).args(&args).status().await?;
        if status.success() { Ok(()) } else { Err(anyhow!("Failed to remove image")) }
    }

    async fn create_network(&self, name: &str, config: &NetworkConfig) -> Result<()> {
        let mut args = self.protocol.get_args("network create");
        args.push("network".to_string());
        args.push("create".to_string());
        if let Some(d) = &config.driver { args.push("--driver".to_string()); args.push(d.clone()); }
        args.push(name.to_string());
        let status = Command::new(self.protocol.binary()).args(&args).status().await?;
        if status.success() { Ok(()) } else { Err(anyhow!("Failed to create network")) }
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let mut args = self.protocol.get_args("network rm");
        args.push("network".to_string());
        args.push("rm".to_string());
        args.push(name.to_string());
        let status = Command::new(self.protocol.binary()).args(&args).status().await?;
        if status.success() { Ok(()) } else { Err(anyhow!("Failed to remove network")) }
    }

    async fn create_volume(&self, name: &str, config: &VolumeConfig) -> Result<()> {
        let mut args = self.protocol.get_args("volume create");
        args.push("volume".to_string());
        args.push("create".to_string());
        if let Some(d) = &config.driver { args.push("--driver".to_string()); args.push(d.clone()); }
        args.push(name.to_string());
        let status = Command::new(self.protocol.binary()).args(&args).status().await?;
        if status.success() { Ok(()) } else { Err(anyhow!("Failed to create volume")) }
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let mut args = self.protocol.get_args("volume rm");
        args.push("volume".to_string());
        args.push("rm".to_string());
        args.push(name.to_string());
        let status = Command::new(self.protocol.binary()).args(&args).status().await?;
        if status.success() { Ok(()) } else { Err(anyhow!("Failed to remove volume")) }
    }

    async fn inspect_network(&self, name: &str) -> Result<()> {
        let mut args = self.protocol.get_args("network inspect");
        args.push("network".to_string());
        args.push("inspect".to_string());
        args.push(name.to_string());
        let status = Command::new(self.protocol.binary()).args(&args).status().await?;
        if status.success() { Ok(()) } else { Err(anyhow!("Network not found")) }
    }
}

pub async fn probe_all_backends() -> Vec<BackendProbeResult> {
    let mut results = Vec::new();
    let candidates = if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        vec!["apple/container", "orbstack", "colima", "rancher-desktop", "lima", "podman", "nerdctl", "docker"]
    } else {
        vec!["podman", "nerdctl", "docker"]
    };

    for name in candidates {
        let (available, error) = match name {
            "apple/container" => {
                if which::which("container").is_ok() {
                    let p = AppleContainerProtocol;
                    match p.check_available().await {
                        Ok(_) => (true, None),
                        Err(e) => (false, Some(e.to_string())),
                    }
                } else { (false, Some("binary 'container' not found".to_string())) }
            }
            "podman" => {
                if which::which("podman").is_ok() {
                    let p = PodmanProtocol;
                    match p.check_available().await {
                        Ok(_) => (true, None),
                        Err(e) => (false, Some(e.to_string())),
                    }
                } else { (false, Some("binary 'podman' not found".to_string())) }
            }
            "docker" | "orbstack" | "colima" => {
                let bin = if name == "orbstack" { "orb" } else { "docker" };
                if which::which(bin).is_ok() {
                    let p = DockerProtocol { binary: bin.to_string() };
                    match p.check_available().await {
                        Ok(_) => (true, None),
                        Err(e) => (false, Some(e.to_string())),
                    }
                } else { (false, Some(format!("binary '{}' not found", bin))) }
            }
            "lima" => {
                if which::which("limactl").is_ok() {
                    let p = LimaProtocol { instance: None };
                    match p.check_available().await {
                        Ok(_) => (true, None),
                        Err(e) => (false, Some(e.to_string())),
                    }
                } else { (false, Some("binary 'limactl' not found".to_string())) }
            }
            _ => (false, Some("Not implemented".to_string())),
        };
        results.push(BackendProbeResult { name: name.to_string(), available, error });
    }
    results
}

pub async fn detect_backend() -> Result<std::sync::Arc<dyn ContainerBackend>, ComposeError> {
    let probed = probe_all_backends().await;
    for res in &probed {
        if res.available {
            match res.name.as_str() {
                "apple/container" => return Ok(std::sync::Arc::new(CliBackend { protocol: AppleContainerProtocol })),
                "podman" => return Ok(std::sync::Arc::new(CliBackend { protocol: PodmanProtocol })),
                "docker" | "orbstack" | "colima" => {
                    let bin = if res.name == "orbstack" { "orb" } else { "docker" };
                    return Ok(std::sync::Arc::new(CliBackend { protocol: DockerProtocol { binary: bin.to_string() } }));
                },
                "lima" => return Ok(std::sync::Arc::new(CliBackend { protocol: LimaProtocol { instance: None } })),
                _ => continue,
            }
        }
    }
    Err(ComposeError::NoBackendFound { probed })
}
