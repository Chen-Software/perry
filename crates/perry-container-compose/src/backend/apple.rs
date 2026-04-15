//! Apple Container backend implementation.
//!
//! Shells out to the `container` CLI (provided by Apple's native container
//! framework on macOS).  Each method maps to one or more `container <cmd>`
//! invocations and parses their output.

use crate::backend::{Backend, ContainerInfo, ExecResult};
use crate::commands::ContainerStatus;
use crate::error::{BackendError, ComposeError, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;

/// The Apple Container CLI binary name
const CONTAINER_BIN: &str = "container";

/// Apple Container backend — wraps the `container` CLI
pub struct AppleContainerBackend {
    /// Override the binary path (useful in tests)
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

// ============ Helper ============

async fn run_cmd(bin: &str, args: &[&str]) -> Result<std::process::Output> {
    let output = Command::new(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| ComposeError::IoError(e))?;
    Ok(output)
}

fn check_output(output: std::process::Output) -> Result<String> {
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(BackendError::CommandFailed {
            code: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
        .into())
    }
}

// ============ Inspect JSON types ============

#[derive(Debug, Deserialize)]
struct InspectOutput {
    #[serde(rename = "Status")]
    #[allow(dead_code)]
    status: Option<String>,
    #[serde(rename = "State")]
    state: Option<InspectState>,
}

#[derive(Debug, Deserialize)]
struct InspectState {
    #[serde(rename = "Status")]
    status: Option<String>,
    #[serde(rename = "Running")]
    running: Option<bool>,
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

// ============ Backend impl ============

#[async_trait]
impl Backend for AppleContainerBackend {
    fn name(&self) -> &'static str {
        "apple-container"
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
        let mut cmd_args: Vec<&str> = vec!["build", "-t", tag, context];

        let dockerfile_owned;
        if let Some(df) = dockerfile {
            cmd_args.push("-f");
            dockerfile_owned = df.to_owned();
            cmd_args.push(&dockerfile_owned);
        }

        let mut build_arg_strs: Vec<String> = Vec::new();
        if let Some(build_args) = args {
            for (k, v) in build_args {
                build_arg_strs.push(format!("{}={}", k, v));
            }
        }
        for ba in &build_arg_strs {
            cmd_args.push("--build-arg");
            cmd_args.push(ba.as_str());
        }

        let target_owned;
        if let Some(t) = target {
            cmd_args.push("--target");
            target_owned = t.to_owned();
            cmd_args.push(&target_owned);
        }

        let network_owned;
        if let Some(n) = network {
            cmd_args.push("--network");
            network_owned = n.to_owned();
            cmd_args.push(&network_owned);
        }

        let output = run_cmd(self.bin, &cmd_args).await?;
        check_output(output)?;
        Ok(())
    }

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
    ) -> Result<()> {
        let mut args: Vec<String> = vec!["run".into(), "--name".into(), name.into()];

        if detach {
            args.push("-d".into());
        }

        if let Some(ps) = ports {
            for p in ps {
                args.push("-p".into());
                args.push(p.clone());
            }
        }

        if let Some(envs) = env {
            for (k, v) in envs {
                args.push("-e".into());
                args.push(format!("{}={}", k, v));
            }
        }

        if let Some(vols) = volumes {
            for v in vols {
                args.push("-v".into());
                args.push(v.clone());
            }
        }

        if let Some(lbls) = labels {
            for (k, v) in lbls {
                args.push("--label".into());
                args.push(format!("{}={}", k, v));
            }
        }

        args.push(image.into());

        if let Some(extra_cmd) = cmd {
            args.extend(extra_cmd.iter().cloned());
        }

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
        if force {
            args.push("-f");
        }
        args.push(name);
        let output = run_cmd(self.bin, &args).await?;
        check_output(output)?;
        Ok(())
    }

    async fn inspect(&self, name: &str) -> Result<ContainerStatus> {
        let output = run_cmd(self.bin, &["inspect", "--format", "json", name]).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // "not found" / "no such container" → NotFound
            if stderr.contains("not found")
                || stderr.contains("no such")
                || stderr.contains("does not exist")
            {
                return Ok(ContainerStatus::NotFound);
            }
            return Err(BackendError::CommandFailed {
                code: output.status.code().unwrap_or(-1),
                stderr: stderr.to_string(),
            }
            .into());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // The output can be a JSON object or array
        let json_str = stdout.trim();

        // Try array first (docker-compatible format), fall back to object
        let parsed: Option<InspectOutput> = if json_str.starts_with('[') {
            serde_json::from_str::<Vec<InspectOutput>>(json_str)
                .ok()
                .and_then(|v| v.into_iter().next())
        } else {
            serde_json::from_str::<InspectOutput>(json_str).ok()
        };

        match parsed {
            Some(info) => {
                let running = info
                    .state
                    .as_ref()
                    .and_then(|s| s.running)
                    .unwrap_or_else(|| {
                        info.state
                            .as_ref()
                            .and_then(|s| s.status.as_deref())
                            .map(|s| s == "running")
                            .unwrap_or(false)
                    });

                if running {
                    Ok(ContainerStatus::Running)
                } else {
                    Ok(ContainerStatus::Stopped)
                }
            }
            None => {
                // Fallback: if we got output but can't parse, assume exists/stopped
                Ok(ContainerStatus::Stopped)
            }
        }
    }

    async fn list(&self, label_filter: Option<&str>) -> Result<Vec<ContainerInfo>> {
        let mut args = vec!["ps", "--format", "json", "--all"];
        let filter_str;
        if let Some(lf) = label_filter {
            args.push("--filter");
            filter_str = format!("label={}", lf);
            args.push(&filter_str);
        }

        let output = run_cmd(self.bin, &args).await?;
        let stdout = check_output(output)?;

        let entries: Vec<ListEntry> = serde_json::from_str(&stdout).unwrap_or_default();
        let infos = entries
            .into_iter()
            .map(|e| ContainerInfo {
                id: e.id,
                name: e.names.into_iter().next().unwrap_or_default(),
                image: e.image,
                status: e.status,
                ports: e.ports,
                created: e.created,
            })
            .collect();

        Ok(infos)
    }

    async fn logs(&self, name: &str, tail: Option<u32>, _follow: bool) -> Result<String> {
        let mut args = vec!["logs".to_owned()];
        if let Some(t) = tail {
            args.push("--tail".into());
            args.push(t.to_string());
        }
        args.push(name.to_owned());

        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run_cmd(self.bin, &arg_refs).await?;
        let stdout = check_output(output)?;
        Ok(stdout)
    }

    async fn exec(
        &self,
        name: &str,
        cmd: &[String],
        user: Option<&str>,
        workdir: Option<&str>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<ExecResult> {
        let mut args: Vec<String> = vec!["exec".into()];

        if let Some(u) = user {
            args.push("--user".into());
            args.push(u.into());
        }

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

        args.push(name.into());
        args.extend(cmd.iter().cloned());

        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = run_cmd(self.bin, &arg_refs).await?;

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    // ── Network operations ──

    async fn create_network(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        let mut args: Vec<String> = vec!["network".into(), "create".into()];

        if let Some(d) = driver {
            args.push("--driver".into());
            args.push(d.into());
        }

        if let Some(lbls) = labels {
            for (k, v) in lbls {
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
            // Idempotent: "not found" errors are OK during teardown
            if stderr.contains("not found")
                || stderr.contains("no such")
                || stderr.contains("does not exist")
            {
                return Ok(());
            }
            return Err(BackendError::CommandFailed {
                code: output.status.code().unwrap_or(-1),
                stderr: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    // ── Volume operations ──

    async fn create_volume(
        &self,
        name: &str,
        driver: Option<&str>,
        labels: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        let mut args: Vec<String> = vec!["volume".into(), "create".into()];

        if let Some(d) = driver {
            args.push("--driver".into());
            args.push(d.into());
        }

        if let Some(lbls) = labels {
            for (k, v) in lbls {
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
            // Idempotent: "not found" errors are OK during teardown
            if stderr.contains("not found")
                || stderr.contains("no such")
                || stderr.contains("does not exist")
            {
                return Ok(());
            }
            return Err(BackendError::CommandFailed {
                code: output.status.code().unwrap_or(-1),
                stderr: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }
}
