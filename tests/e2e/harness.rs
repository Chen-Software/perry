use std::process::Command;
use std::path::PathBuf;

pub struct E2eHarness {
    pub perry_bin: PathBuf,
}

impl E2eHarness {
    pub fn new() -> Self {
        Self {
            perry_bin: PathBuf::from("perry"),
        }
    }

    pub fn run_test(&self, file_path: &str) -> anyhow::Result<String> {
        let output = Command::new(&self.perry_bin)
            .arg("run")
            .arg(file_path)
            .env("PERRY_E2E_TESTS", "1")
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            anyhow::bail!("E2E test failed: {}", String::from_utf8_lossy(&output.stderr))
        }
    }
}
