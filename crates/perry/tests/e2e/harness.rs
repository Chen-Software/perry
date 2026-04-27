use std::process::Command;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};

pub struct E2eResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn compile_e2e(path: &str) -> Result<PathBuf> {
    let mut root = std::env::current_dir().unwrap();
    if root.ends_with("crates/perry") {
        root = root.parent().unwrap().parent().unwrap().to_path_buf();
    }

    let test_path = root.join(path);
    let output_path = test_path.with_extension("bin");
    let perry_bin = root.join("target/debug/perry");

    // Check if perry exists
    if !perry_bin.exists() {
        return Err(anyhow!("perry binary not found at {:?}", perry_bin));
    }

    let status = Command::new(&perry_bin)
        .args([test_path.to_str().unwrap(), "-o", output_path.to_str().unwrap()])
        .status()?;

    if status.success() {
        Ok(output_path)
    } else {
        Err(anyhow!("Compilation failed for {:?} using {:?}", test_path, perry_bin))
    }
}

pub fn run_e2e(binary: &Path) -> E2eResult {
    let output = Command::new(binary)
        .output()
        .expect("Failed to execute e2e binary");

    E2eResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

pub fn assert_e2e_pass(result: &E2eResult) {
    assert_eq!(result.exit_code, 0, "E2E test failed with exit code {}. Stderr: {}", result.exit_code, result.stderr);
    assert!(result.stdout.contains("[e2e] PASS"), "E2E test output missing '[e2e] PASS'. Output: {}", result.stdout);
}

pub fn assert_e2e_output(result: &E2eResult, pattern: &str) {
    assert!(result.stdout.contains(pattern), "E2E test output missing pattern '{}'. Output: {}", pattern, result.stdout);
}
