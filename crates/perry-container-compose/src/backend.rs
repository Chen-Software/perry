use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use serde_json::Value;
use crate::error::{ComposeError, Result};
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
    async fn wait(&self, id: &str) -> Result<i32>;
}

async fn run_command(cmd: &mut Command) -> Result<std::process::Output> {
    let output = cmd.output().await.map_err(ComposeError::IoError)?;
    if !output.status.success() {
        return Err(ComposeError::BackendError {
            code: output.status.code().unwrap_or(1),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(output)
}

pub struct PodmanBackend {
    pub binary_path: String,
}

impl PodmanBackend {
    pub fn new(binary_path: String) -> Self { Self { binary_path } }

    fn cmd(&self) -> Command {
        Command::new(&self.binary_path)
    }
}

#[async_trait]
impl ContainerBackend for PodmanBackend {
    fn name(&self) -> &'static str { "podman" }

    async fn check_available(&self) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("--version");
        run_command(&mut cmd).await?;
        Ok(())
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut cmd = self.cmd();
        cmd.arg("run").arg("-d");

        if let Some(name) = &spec.name {
            cmd.arg("--name").arg(name);
        }

        if let Some(ports) = &spec.ports {
            for port in ports {
                cmd.arg("-p").arg(port);
            }
        }

        if let Some(volumes) = &spec.volumes {
            for vol in volumes {
                cmd.arg("-v").arg(vol);
            }
        }

        if let Some(env) = &spec.env {
            for (k, v) in env {
                cmd.arg("-e").arg(format!("{}={}", k, v));
            }
        }

        if let Some(network) = &spec.network {
            cmd.arg("--network").arg(network);
        }

        if spec.rm.unwrap_or(false) {
            cmd.arg("--rm");
        }

        if spec.read_only.unwrap_or(false) {
            cmd.arg("--read-only");
        }

        if let Some(entrypoint) = &spec.entrypoint {
            cmd.arg("--entrypoint").arg(entrypoint.join(" "));
        }

        cmd.arg(&spec.image);

        if let Some(args) = &spec.cmd {
            cmd.args(args);
        }

        let output = run_command(&mut cmd).await?;
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut cmd = self.cmd();
        cmd.arg("create");

        if let Some(name) = &spec.name {
            cmd.arg("--name").arg(name);
        }

        if let Some(ports) = &spec.ports {
            for port in ports { cmd.arg("-p").arg(port); }
        }
        if let Some(volumes) = &spec.volumes {
            for vol in volumes { cmd.arg("-v").arg(vol); }
        }
        if let Some(env) = &spec.env {
            for (k, v) in env { cmd.arg("-e").arg(format!("{}={}", k, v)); }
        }
        if let Some(network) = &spec.network {
            cmd.arg("--network").arg(network);
        }
        if spec.read_only.unwrap_or(false) {
            cmd.arg("--read-only");
        }

        cmd.arg(&spec.image);

        if let Some(args) = &spec.cmd {
            cmd.args(args);
        }

        let output = run_command(&mut cmd).await?;
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        Ok(ContainerHandle { id, name: spec.name.clone() })
    }

    async fn start(&self, id: &str) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("start").arg(id);
        run_command(&mut cmd).await?;
        Ok(())
    }

    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("stop");
        if let Some(t) = timeout {
            cmd.arg("-t").arg(t.to_string());
        }
        cmd.arg(id);
        run_command(&mut cmd).await?;
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("rm");
        if force {
            cmd.arg("-f");
        }
        cmd.arg(id);
        run_command(&mut cmd).await?;
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let mut cmd = self.cmd();
        cmd.arg("ps").arg("--format").arg("json");
        if all {
            cmd.arg("-a");
        }
        let output = run_command(&mut cmd).await?;
        let containers: Vec<Value> = serde_json::from_slice(&output.stdout).map_err(ComposeError::JsonError)?;

        let mut result = Vec::new();
        for c in containers {
            result.push(ContainerInfo {
                id: c["Id"].as_str().unwrap_or_default().to_string(),
                name: c["Names"].as_array().and_then(|a| a[0].as_str()).unwrap_or_default().to_string(),
                image: c["Image"].as_str().unwrap_or_default().to_string(),
                status: c["Status"].as_str().unwrap_or_default().to_string(),
                ports: c["Ports"].as_array().map(|a| a.iter().filter_map(|p| p["hostPort"].as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                created: c["Created"].as_str().unwrap_or_default().to_string(),
            });
        }
        Ok(result)
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let mut cmd = self.cmd();
        cmd.arg("inspect").arg("--format").arg("json").arg(id);
        let output = run_command(&mut cmd).await?;
        let inspect_res: Vec<Value> = serde_json::from_slice(&output.stdout).map_err(ComposeError::JsonError)?;
        let c = inspect_res.get(0).ok_or_else(|| ComposeError::NotFound(id.to_string()))?;

        Ok(ContainerInfo {
            id: c["Id"].as_str().unwrap_or_default().to_string(),
            name: c["Name"].as_str().unwrap_or_default().trim_start_matches('/').to_string(),
            image: c["Config"]["Image"].as_str().unwrap_or_default().to_string(),
            status: c["State"]["Status"].as_str().unwrap_or_default().to_string(),
            ports: Vec::new(),
            created: c["Created"].as_str().unwrap_or_default().to_string(),
        })
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut cmd = self.cmd();
        cmd.arg("logs");
        if let Some(n) = tail {
            cmd.arg("--tail").arg(n.to_string());
        }
        cmd.arg(id);
        let output = run_command(&mut cmd).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let mut exec_cmd = self.cmd();
        exec_cmd.arg("exec");
        if let Some(e) = env {
            for (k, v) in e {
                exec_cmd.arg("-e").arg(format!("{}={}", k, v));
            }
        }
        if let Some(w) = workdir {
            exec_cmd.arg("-w").arg(w);
        }
        exec_cmd.arg(id).args(cmd);
        let output = run_command(&mut exec_cmd).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("pull").arg(reference);
        run_command(&mut cmd).await?;
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let mut cmd = self.cmd();
        cmd.arg("images").arg("--format").arg("json");
        let output = run_command(&mut cmd).await?;
        let images: Vec<Value> = serde_json::from_slice(&output.stdout).map_err(ComposeError::JsonError)?;

        let mut result = Vec::new();
        for i in images {
            result.push(ImageInfo {
                id: i["Id"].as_str().unwrap_or_default().to_string(),
                repository: i["Repository"].as_str().unwrap_or_default().to_string(),
                tag: i["Tag"].as_str().unwrap_or_default().to_string(),
                size: i["Size"].as_u64().unwrap_or(0),
                created: i["Created"].as_str().unwrap_or_default().to_string(),
            });
        }
        Ok(result)
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("rmi");
        if force { cmd.arg("-f"); }
        cmd.arg(reference);
        run_command(&mut cmd).await?;
        Ok(())
    }

    async fn create_network(&self, name: &str, _config: &ComposeNetwork) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("network").arg("create").arg(name);
        run_command(&mut cmd).await?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("network").arg("rm").arg(name);
        run_command(&mut cmd).await?;
        Ok(())
    }

    async fn create_volume(&self, name: &str, _config: &ComposeVolume) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("volume").arg("create").arg(name);
        run_command(&mut cmd).await?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("volume").arg("rm").arg(name);
        run_command(&mut cmd).await?;
        Ok(())
    }

    async fn wait(&self, id: &str) -> Result<i32> {
        let mut cmd = self.cmd();
        cmd.arg("wait").arg(id);
        let output = run_command(&mut cmd).await?;
        let code_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let code = code_str.parse::<i32>().unwrap_or(0);
        Ok(code)
    }
}

pub struct DockerBackend {
    pub binary_path: String,
}

impl DockerBackend {
    pub fn new(binary_path: String) -> Self { Self { binary_path } }
    fn cmd(&self) -> Command { Command::new(&self.binary_path) }
}

#[async_trait]
impl ContainerBackend for DockerBackend {
    fn name(&self) -> &'static str { "docker" }
    async fn check_available(&self) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("--version");
        run_command(&mut cmd).await?;
        Ok(())
    }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.run(spec).await
    }
    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.create(spec).await
    }
    async fn start(&self, id: &str) -> Result<()> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.start(id).await
    }
    async fn stop(&self, id: &str, timeout: Option<u32>) -> Result<()> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.stop(id, timeout).await
    }
    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.remove(id, force).await
    }
    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.list(all).await
    }
    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.inspect(id).await
    }
    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.logs(id, tail).await
    }
    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.exec(id, cmd, env, workdir).await
    }
    async fn pull_image(&self, reference: &str) -> Result<()> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.pull_image(reference).await
    }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.list_images().await
    }
    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.remove_image(reference, force).await
    }
    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.create_network(name, config).await
    }
    async fn remove_network(&self, name: &str) -> Result<()> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.remove_network(name).await
    }
    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.create_volume(name, config).await
    }
    async fn remove_volume(&self, name: &str) -> Result<()> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.remove_volume(name).await
    }
    async fn wait(&self, id: &str) -> Result<i32> {
        let p = PodmanBackend { binary_path: self.binary_path.clone() };
        p.wait(id).await
    }
}

pub struct AppleContainerBackend {
    pub binary_path: String,
}

impl AppleContainerBackend {
    pub fn new(binary_path: String) -> Self { Self { binary_path } }
    fn cmd(&self) -> Command { Command::new(&self.binary_path) }
}

#[async_trait]
impl ContainerBackend for AppleContainerBackend {
    fn name(&self) -> &'static str { "apple/container" }
    async fn check_available(&self) -> Result<()> {
        let mut cmd = self.cmd();
        cmd.arg("--version");
        run_command(&mut cmd).await?;
        Ok(())
    }
    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut cmd = self.cmd();
        cmd.arg("run");
        if let Some(name) = &spec.name { cmd.arg("--name").arg(name); }
        cmd.arg(&spec.image);
        let output = run_command(&mut cmd).await?;
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(ContainerHandle { id, name: spec.name.clone() })
    }
    async fn create(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> { unimplemented!() }
    async fn start(&self, _id: &str) -> Result<()> { unimplemented!() }
    async fn stop(&self, _id: &str, _timeout: Option<u32>) -> Result<()> { unimplemented!() }
    async fn remove(&self, _id: &str, _force: bool) -> Result<()> { unimplemented!() }
    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> { unimplemented!() }
    async fn inspect(&self, _id: &str) -> Result<ContainerInfo> { unimplemented!() }
    async fn logs(&self, _id: &str, _tail: Option<u32>) -> Result<ContainerLogs> { unimplemented!() }
    async fn exec(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> { unimplemented!() }
    async fn pull_image(&self, _reference: &str) -> Result<()> { unimplemented!() }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { unimplemented!() }
    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<()> { unimplemented!() }
    async fn create_network(&self, _name: &str, _config: &ComposeNetwork) -> Result<()> { unimplemented!() }
    async fn remove_network(&self, _name: &str) -> Result<()> { unimplemented!() }
    async fn create_volume(&self, _name: &str, _config: &ComposeVolume) -> Result<()> { unimplemented!() }
    async fn remove_volume(&self, _name: &str) -> Result<()> { unimplemented!() }
    async fn wait(&self, _id: &str) -> Result<i32> { unimplemented!() }
}

pub fn get_backend() -> Result<Arc<dyn ContainerBackend + Send + Sync>> {
    let os = std::env::consts::OS;

    if os == "macos" || os == "ios" {
        if let Ok(p) = which::which("container") {
            return Ok(Arc::new(AppleContainerBackend::new(p.to_string_lossy().to_string())));
        }
    }

    if let Ok(p) = which::which("podman") {
        return Ok(Arc::new(PodmanBackend::new(p.to_string_lossy().to_string())));
    }

    if let Ok(p) = which::which("docker") {
        return Ok(Arc::new(DockerBackend::new(p.to_string_lossy().to_string())));
    }

    Err(ComposeError::BackendError {
        code: 125,
        message: "No container backend (container, podman, or docker) found on PATH".to_string(),
    })
}

pub struct MockBackend;

#[async_trait]
impl ContainerBackend for MockBackend {
    fn name(&self) -> &'static str { "mock" }
    async fn check_available(&self) -> Result<()> { Ok(()) }
    async fn run(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> { Ok(ContainerHandle { id: "mock".into(), name: None }) }
    async fn create(&self, _spec: &ContainerSpec) -> Result<ContainerHandle> { Ok(ContainerHandle { id: "mock".into(), name: None }) }
    async fn start(&self, _id: &str) -> Result<()> { Ok(()) }
    async fn stop(&self, _id: &str, _timeout: Option<u32>) -> Result<()> { Ok(()) }
    async fn remove(&self, _id: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn list(&self, _all: bool) -> Result<Vec<ContainerInfo>> { Ok(vec![]) }
    async fn inspect(&self, _id: &str) -> Result<ContainerInfo> { Err(ComposeError::NotFound("mock".into())) }
    async fn logs(&self, _id: &str, _tail: Option<u32>) -> Result<ContainerLogs> { Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }) }
    async fn exec(&self, _id: &str, _cmd: &[String], _env: Option<&HashMap<String, String>>, _workdir: Option<&str>) -> Result<ContainerLogs> { Ok(ContainerLogs { stdout: "".into(), stderr: "".into() }) }
    async fn pull_image(&self, _reference: &str) -> Result<()> { Ok(()) }
    async fn list_images(&self) -> Result<Vec<ImageInfo>> { Ok(vec![]) }
    async fn remove_image(&self, _reference: &str, _force: bool) -> Result<()> { Ok(()) }
    async fn create_network(&self, _name: &str, _config: &ComposeNetwork) -> Result<()> { Ok(()) }
    async fn remove_network(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn create_volume(&self, _name: &str, _config: &ComposeVolume) -> Result<()> { Ok(()) }
    async fn remove_volume(&self, _name: &str) -> Result<()> { Ok(()) }
    async fn wait(&self, _id: &str) -> Result<i32> { Ok(0) }
}
