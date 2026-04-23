use std::process::Command;
use std::path::PathBuf;

#[test]
#[ignore] // Requires perry binary to be built and in PATH
fn test_perry_check_container_imports() {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop();
    root.pop();

    let ts_file = root.join("tests/smoke/container_import.ts");
    let perry_bin = root.join("target/debug/perry");

    if !perry_bin.exists() {
        return;
    }

    let output = Command::new(perry_bin)
        .arg("check")
        .arg(ts_file)
        .output()
        .expect("failed to execute perry check");

    assert!(output.status.success(), "perry check failed: {}", String::from_utf8_lossy(&output.stderr));
}
