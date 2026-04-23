use std::process::Command;
use std::path::{Path, PathBuf};

pub struct E2eResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn compile_e2e(ts_path: &Path) -> PathBuf {
    let mut bin_path = ts_path.to_path_buf();
    bin_path.set_extension("bin");

    let perry_bin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/perry");

    let status = Command::new(perry_bin)
        .arg("build")
        .arg(ts_path)
        .arg("-o")
        .arg(&bin_path)
        .status()
        .expect("Failed to run perry build");

    assert!(status.success(), "perry build failed for {:?}", ts_path);
    bin_path
}

pub fn run_e2e(binary: &Path) -> E2eResult {
    let output = Command::new(binary)
        .output()
        .expect("Failed to run e2e binary");

    E2eResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}
