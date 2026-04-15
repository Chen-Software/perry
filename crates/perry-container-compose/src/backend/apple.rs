//! Apple Container backend implementation.
//!
//! Shells out to the `container` CLI on macOS/iOS.

use super::{Backend, ContainerBackend, ContainerInfo, ContainerStatus, ExecResult};
use crate::error::{ComposeError, Result};
use crate::types::{
    ComposeNetwork, ComposeVolume, ContainerHandle, ContainerLogs, ContainerSpec, ImageInfo,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;

const CONTAINER_BIN: &str = "container";

pub struct AppleContainerBackend {
    bin: &'static str,
}

impl AppleContainerBackend {
    pub fn new() -> Self {
        AppleContainerBackend {
            bin: CONTAINER_BIN,
        }
    }
}

impl Default for AppleContainerBackend {
    fn default() -> Self {
        Self::new()
    }
}

// ============ Helpers ============

async fn run_cmd(bin: &str, args: &[&str]) -> Result<std::process::Output> {
    let output = Command::new(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(ComposeError::IoError)?;
    Ok(output)
}

async fn run_cmd_args(bin: &str, args: &[String]) -> Result<std::process::Output> {
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_cmd(bin, &arg_refs).await
}

fn check_output(output: std::process::Output) -> Result<String> {
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(ComposeError::BackendError { code, message: stderr })
    }
}

fn is_not_found(stderr: &str) -> bool {
    stderr.contains("not found")
        || stderr.contains("no such")
        || stderr.contains("does not exist")
}

#[derive(Debug, Deserialize)]
struct InspectOutput {
    #[serde(rename = "State")]
    state: Option<InspectState>,
}

#[derive(Debug, Deserialize)]
struct InspectState {
    #[serde(rename = "Running")]
    running: Option<bool>,
    #[serde(rename = "Status")]
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListEntry {
    #[serde(rename = "ID", default)]
    id: String,
    #[serde(rename = "Names", default)]
    names: Vec<String>,
    #[serde(rename = "Image", default)]
    image: String,
    #[serde(rename = "Status", default)]
    status: String,
    #[serde(rename = "Ports", default)]
    ports: Vec<String>,
    #[serde(rename = "Created", default)]
    created: String,
}

#[derive(Debug, Deserialize)]
struct ImageEntry {
    #[serde(rename = "ID", default)]
    id: String,
    #[serde(rename = "Repository", default)]
    repository: String,
    #[serde(rename = "Tag", default)]
    tag: String,
    #[serde(rename = "Size", default)]
    size: u64,
    #[serde(rename = "Created", default)]
    created: String,
}

// ============ ContainerBackend impl ============

#[async_trait]
impl ContainerBackend for AppleContainerBackend {
    fn name(&self) -> &'static str {
        "apple-container"
    }

    async fn check_available(&self) -> Result<()> {
        let output = Command::new(self.bin)
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
                    "'{}' binary not available: {}",
                    self.bin,
                    String::from_utf8_lossy(&output.stderr)
                ),
            })
        }
    }

    async fn run(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut args: Vec<String> = vec!["run".into()];
        if spec.rm.unwrap_or(false) { args.push("--rm".into()); }
        if let Some(name) = &spec.name {
            args.push("--name".into());
            args.push(name.clone());
        }
        if let Some(network) = &spec.network {
            args.push("--network".into());
            args.push(network.clone());
        }
        if let Some(ports) = &spec.ports {
            for p in ports { args.push("-p".into()); args.push(p.clone()); }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols { args.push("-v".into()); args.push(v.clone()); }
        }
        if let Some(envs) = &spec.env {
            for (k, v) in envs {
                args.push("-e".into());
                args.push(format!("{}={}", k, v));
            }
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        let output = run_cmd_args(self.bin, &args).await?;
        let stdout = check_output(output)?;
        let name = spec.name.clone().unwrap_or_else(|| stdout.trim().to_string());
        Ok(ContainerHandle { id: name.clone(), name: Some(name) })
    }

    async fn create(&self, spec: &ContainerSpec) -> Result<ContainerHandle> {
        let mut args: Vec<String> = vec!["create".into()];
        if let Some(name) = &spec.name {
            args.push("--name".into());
            args.push(name.clone());
        }
        if let Some(network) = &spec.network {
            args.push("--network".into());
            args.push(network.clone());
        }
        if let Some(ports) = &spec.ports {
            for p in ports { args.push("-p".into()); args.push(p.clone()); }
        }
        if let Some(vols) = &spec.volumes {
            for v in vols { args.push("-v".into()); args.push(v.clone()); }
        }
        if let Some(envs) = &spec.env {
            for (k, v) in envs {
                args.push("-e".into());
                args.push(format!("{}={}", k, v));
            }
        }
        args.push(spec.image.clone());
        if let Some(cmd) = &spec.cmd { args.extend(cmd.iter().cloned()); }
        let output = run_cmd_args(self.bin, &args).await?;
        let stdout = check_output(output)?;
        let name = spec.name.clone().unwrap_or_else(|| stdout.trim().to_string());
        Ok(ContainerHandle { id: name.clone(), name: Some(name) })
    }

    async fn start(&self, id: &str) -> Result<()> {
        let output = run_cmd(self.bin, &["start", id]).await?;
        check_output(output)?;
        Ok(())
    }

    async fn stop(&self, id: &str, _timeout: Option<u32>) -> Result<()> {
        let output = run_cmd(self.bin, &["stop", id]).await?;
        check_output(output)?;
        Ok(())
    }

    async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let mut args = vec!["rm"];
        if force { args.push("-f"); }
        args.push(id);
        let output = run_cmd(self.bin, &args).await?;
        check_output(output)?;
        Ok(())
    }

    async fn list(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let mut args = vec!["ps", "--format", "json"];
        if all { args.push("--all"); }
        let output = run_cmd(self.bin, &args).await?;
        let stdout = check_output(output)?;
        let entries: Vec<ListEntry> = serde_json::from_str(&stdout).unwrap_or_default();
        Ok(entries.into_iter().map(|e| ContainerInfo {
            id: e.id,
            name: e.names.into_iter().next().unwrap_or_default(),
            image: e.image,
            status: e.status,
            ports: e.ports,
            created: e.created,
        }).collect())
    }

    async fn inspect(&self, id: &str) -> Result<ContainerInfo> {
        let output = run_cmd(self.bin, &["inspect", "--format", "json", id]).await?;
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
        let json_str = stdout.trim();
        let parsed: Option<InspectOutput> = if json_str.starts_with('[') {
            serde_json::from_str::<Vec<InspectOutput>>(json_str).ok().and_then(|v| v.into_iter().next())
        } else {
            serde_json::from_str::<InspectOutput>(json_str).ok()
        };
        match parsed {
            Some(info) => {
                let running = info.state.as_ref().and_then(|s| s.running).unwrap_or(false);
                Ok(ContainerInfo {
                    id: id.to_string(),
                    name: id.to_string(),
                    image: String::new(),
                    status: if running { "running" } else { "stopped" }.to_string(),
                    ports: vec![], created: String::new(),
                })
            }
            None => Ok(ContainerInfo {
                id: id.to_string(), name: id.to_string(), image: String::new(),
                status: "stopped".to_string(), ports: vec![], created: String::new(),
            }),
        }
    }

    async fn logs(&self, id: &str, tail: Option<u32>) -> Result<ContainerLogs> {
        let mut args = vec!["logs".to_owned()];
        if let Some(t) = tail {
            args.push("--tail".into());
            args.push(t.to_string());
        }
        args.push(id.to_owned());
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run_cmd(self.bin, &arg_refs).await?;
        let stdout = check_output(output)?;
        Ok(ContainerLogs { stdout, stderr: String::new() })
    }

    async fn exec(&self, id: &str, cmd: &[String], env: Option<&HashMap<String, String>>, workdir: Option<&str>) -> Result<ContainerLogs> {
        let mut args: Vec<String> = vec!["exec".into()];
        if let Some(wd) = workdir {
            args.push("--workdir".into());
            args.push(wd.into());
        }
        if let Some(envs) = env {
            for (k, v) in envs {
                args.push("-e".into());
                args.push(format!("{}={}", k, v));
            }
        }
        args.push(id.into());
        args.extend(cmd.iter().cloned());
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run_cmd(self.bin, &arg_refs).await?;
        Ok(ContainerLogs {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn pull_image(&self, reference: &str) -> Result<()> {
        let output = run_cmd(self.bin, &["pull", reference]).await?;
        check_output(output)?;
        Ok(())
    }

    async fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let output = run_cmd(self.bin, &["images", "--format", "json"]).await?;
        let stdout = check_output(output)?;
        let entries: Vec<ImageEntry> = serde_json::from_str(&stdout).unwrap_or_default();
        Ok(entries.into_iter().map(|e| ImageInfo {
            id: e.id, repository: e.repository, tag: e.tag, size: e.size, created: e.created,
        }).collect())
    }

    async fn remove_image(&self, reference: &str, force: bool) -> Result<()> {
        let mut args = vec!["rmi"];
        if force { args.push("-f"); }
        args.push(reference);
        let output = run_cmd(self.bin, &args).await?;
        check_output(output)?;
        Ok(())
    }

    async fn create_network(&self, name: &str, config: &ComposeNetwork) -> Result<()> {
        let mut args: Vec<String> = vec!["network".into(), "create".into()];
        if let Some(d) = &config.driver {
            args.push("--driver".into());
            args.push(d.clone());
        }
        if let Some(lbls) = &config.labels {
            for (k, v) in lbls.to_map() {
                args.push("--label".into());
                args.push(format!("{}={}", k, v));
            }
        }
        args.push(name.into());
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run_cmd(self.bin, &arg_refs).await?;
        check_output(output)?;
        Ok(())
    }

    async fn remove_network(&self, name: &str) -> Result<()> {
        let output = run_cmd(self.bin, &["network", "rm", name]).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if is_not_found(&stderr) { return Ok(()); }
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            });
        }
        Ok(())
    }

    async fn create_volume(&self, name: &str, config: &ComposeVolume) -> Result<()> {
        let mut args: Vec<String> = vec!["volume".into(), "create".into()];
        if let Some(d) = &config.driver {
            args.push("--driver".into());
            args.push(d.clone());
        }
        if let Some(lbls) = &config.labels {
            for (k, v) in lbls.to_map() {
                args.push("--label".into());
                args.push(format!("{}={}", k, v));
            }
        }
        args.push(name.into());
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run_cmd(self.bin, &arg_refs).await?;
        check_output(output)?;
        Ok(())
    }

    async fn remove_volume(&self, name: &str) -> Result<()> {
        let output = run_cmd(self.bin, &["volume", "rm", name]).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if is_not_found(&stderr) { return Ok(()); }
            return Err(ComposeError::BackendError {
                code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            });
        }
        Ok(())
    }
}

// ============ Legacy Backend impl ============

#[async_trait]
impl Backend for AppleContainerBackend {
    fn name(&self) -> &'static str { "apple-container" }

    async fn build(&self, context: &str, dockerfile: Option<&str>, tag: &str, args: Option<&HashMap<String, String>>, target: Option<&str>, network: Option<&str>) -> Result<()> {
        let mut cmd_args: Vec<&str> = vec!["build", "-t", tag, context];
        let dockerfile_owned;
        if let Some(df) = dockerfile {
            cmd_args.push("-f");
            dockerfile_owned = df.to_owned();
            cmd_args.push(&dockerfile_owned);
        }
        let mut ba_strs: Vec<String> = Vec::new();
        if let Some(ba) = args {
            for (k, v) in ba { ba_strs.push(format!("{}={}", k, v)); }
        }
        for ba in &ba_strs { cmd_args.push("--build-arg"); cmd_args.push(ba.as_str()); }
        let t_owned;
        if let Some(t) = target { cmd_args.push("--target"); t_owned = t.to_owned(); cmd_args.push(&t_owned); }
        let n_owned;
        if let Some(n) = network { cmd_args.push("--network"); n_owned = n.to_owned(); cmd_args.push(&n_owned); }
        let output = run_cmd(self.bin, &cmd_args).await?;
        check_output(output)?;
        Ok(())
    }

    async fn run(&self, image: &str, name: &str, ports: Option<&[String]>, env: Option<&HashMap<String, String>>, volumes: Option<&[String]>, labels: Option<&HashMap<String, String>>, cmd: Option<&[String]>, detach: bool) -> Result<()> {
        let mut args: Vec<String> = vec!["run".into(), "--name".into(), name.into()];
        if detach { args.push("-d".into()); }
        if let Some(ps) = ports { for p in ps { args.push("-p".into()); args.push(p.clone()); } }
        if let Some(envs) = env { for (k, v) in envs { args.push("-e".into()); args.push(format!("{}={}", k, v)); } }
        if let Some(vols) = volumes { for v in vols { args.push("-v".into()); args.push(v.clone()); } }
        if let Some(lbls) = labels { for (k, v) in lbls { args.push("--label".into()); args.push(format!("{}={}", k, v)); } }
        args.push(image.into());
        if let Some(extra) = cmd { args.extend(extra.iter().cloned()); }
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run_cmd(self.bin, &arg_refs).await?;
        check_output(output)?;
        Ok(())
    }

    async fn start(&self, name: &str) -> Result<()> {
        let output = run_cmd(self.bin, &["start", name]).await?;
        check_output(output)?;
        Ok(())
    }

    async fn stop(&self, name: &str) -> Result<()> {
        let output = run_cmd(self.bin, &["stop", name]).await?;
        check_output(output)?;
        Ok(())
    }

    async fn remove(&self, name: &str, force: bool) -> Result<()> {
        let mut args = vec!["rm"];
        if force { args.push("-f"); }
        args.push(name);
        let output = run_cmd(self.bin, &args).await?;
        check_output(output)?;
        Ok(())
    }

    async fn inspect(&self, name: &str) -> Result<ContainerStatus> {
        let output = run_cmd(self.bin, &["inspect", "--format", "json", name]).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if is_not_found(&stderr) { return Ok(ContainerStatus::NotFound); }
            return Err(ComposeError::BackendError { code: output.status.code().unwrap_or(-1), message: stderr.to_string() });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let json_str = stdout.trim();
        let parsed: Option<InspectOutput> = if json_str.starts_with('[') {
            serde_json::from_str::<Vec<InspectOutput>>(json_str).ok().and_then(|v| v.into_iter().next())
        } else {
            serde_json::from_str::<InspectOutput>(json_str).ok()
        };
        match parsed {
            Some(info) => Ok(if info.state.as_ref().and_then(|s| s.running).unwrap_or(false) { ContainerStatus::Running } else { ContainerStatus::Stopped }),
            None => Ok(ContainerStatus::Stopped),
        }
    }

    async fn list(&self, label_filter: Option<&str>) -> Result<Vec<ContainerInfo>> {
        let mut args = vec!["ps", "--format", "json", "--all"];
        let f_str;
        if let Some(lf) = label_filter { args.push("--filter"); f_str = format!("label={}", lf); args.push(&f_str); }
        let output = run_cmd(self.bin, &args).await?;
        let stdout = check_output(output)?;
        let entries: Vec<ListEntry> = serde_json::from_str(&stdout).unwrap_or_default();
        Ok(entries.into_iter().map(|e| ContainerInfo { id: e.id, name: e.names.into_iter().next().unwrap_or_default(), image: e.image, status: e.status, ports: e.ports, created: e.created }).collect())
    }

    async fn logs(&self, name: &str, tail: Option<u32>, _follow: bool) -> Result<String> {
        let mut args = vec!["logs".to_owned()];
        if let Some(t) = tail { args.push("--tail".into()); args.push(t.to_string()); }
        args.push(name.to_owned());
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run_cmd(self.bin, &arg_refs).await?;
        check_output(output)
    }

    async fn exec(&self, name: &str, cmd: &[String], user: Option<&str>, workdir: Option<&str>, env: Option<&HashMap<String, String>>) -> Result<ExecResult> {
        let mut args: Vec<String> = vec!["exec".into()];
        if let Some(u) = user { args.push("--user".into()); args.push(u.into()); }
        if let Some(wd) = workdir { args.push("--workdir".into()); args.push(wd.into()); }
        if let Some(envs) = env { for (k, v) in envs { args.push("-e".into()); args.push(format!("{}={}", k, v)); } }
        args.push(name.into());
        args.extend(cmd.iter().cloned());
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run_cmd(self.bin, &arg_refs).await?;
        Ok(ExecResult { stdout: String::from_utf8_lossy(&output.stdout).to_string(), stderr: String::from_utf8_lossy(&output.stderr).to_string(), exit_code: output.status.code().unwrap_or(-1) })
    }

    async fn create_network(&self, name: &str, driver: Option<&str>, labels: Option<&HashMap<String, String>>) -> Result<()> {
        let config = ComposeNetwork {
            driver: driver.map(String::from),
            labels: labels.map(|l| { let mut m = indexmap::IndexMap::new(); for (k, v) in l { m.insert(k.clone(), Some(serde_yaml::Value::String(v.clone()))); } crate::types::ListOrDict::Dict(m) }),
            ..Default::default()
        };
        ContainerBackend::create_network(self, name, &config).await
    }

    async fn remove_network(&self, name: &str) -> Result<()> { ContainerBackend::remove_network(self, name).await }
    async fn create_volume(&self, name: &str, driver: Option<&str>, labels: Option<&HashMap<String, String>>) -> Result<()> {
        let config = ComposeVolume {
            driver: driver.map(String::from),
            labels: labels.map(|l| { let mut m = indexmap::IndexMap::new(); for (k, v) in l { m.insert(k.clone(), Some(serde_yaml::Value::String(v.clone()))); } crate::types::ListOrDict::Dict(m) }),
            ..Default::default()
        };
        ContainerBackend::create_volume(self, name, &config).await
    }
    async fn remove_volume(&self, name: &str) -> Result<()> { ContainerBackend::remove_volume(self, name).await }
}
