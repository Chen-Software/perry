//! Run command - compile and launch a TypeScript file in one step

use anyhow::{anyhow, Result};
use clap::Args;
use std::path::PathBuf;
use std::process::Command;

use super::compile::{CompileArgs, CompileResult};
use crate::OutputFormat;

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Input TypeScript file
    pub input: Option<PathBuf>,

    /// Run on macOS (default on macOS host)
    #[arg(long)]
    pub macos: bool,

    /// Run on iOS (simulator or device)
    #[arg(long)]
    pub ios: bool,

    /// Run on web (opens in browser)
    #[arg(long)]
    pub web: bool,

    /// Run on Android
    #[arg(long)]
    pub android: bool,

    /// Specific iOS simulator UDID to target
    #[arg(long)]
    pub simulator: Option<String>,

    /// Specific iOS device UDID to target
    #[arg(long)]
    pub device: Option<String>,

    /// Enable V8 JavaScript runtime
    #[arg(long)]
    pub enable_js_runtime: bool,

    /// Enable type checking via tsgo
    #[arg(long)]
    pub type_check: bool,

    /// Arguments passed to the compiled program
    #[arg(last = true)]
    pub program_args: Vec<String>,
}

/// A detected simulator or device
struct DeviceInfo {
    udid: String,
    name: String,
}

pub fn run(args: RunArgs, format: OutputFormat, use_color: bool, verbose: u8) -> Result<()> {
    // 1. Resolve entry file
    let input = resolve_entry_file(args.input.as_deref(), &args)?;

    // 2. Resolve target and device
    let (target, device_udid) = resolve_target(&args)?;

    // 3. Build CompileArgs
    let compile_args = CompileArgs {
        input: input.clone(),
        output: None,
        keep_intermediates: false,
        print_hir: false,
        no_link: false,
        enable_js_runtime: args.enable_js_runtime,
        target: target.clone(),
        app_bundle_id: None,
        output_type: "executable".to_string(),
        bundle_extensions: None,
        type_check: args.type_check,
    };

    // 4. Compile
    let result = super::compile::run(compile_args, format, use_color, verbose)?;

    // 5. Launch
    launch(&result, device_udid.as_deref(), &args.program_args, format)
}

/// Resolve the entry TypeScript file
fn resolve_entry_file(input: Option<&std::path::Path>, _args: &RunArgs) -> Result<PathBuf> {
    if let Some(path) = input {
        if path.exists() {
            return Ok(path.to_path_buf());
        }
        return Err(anyhow!("File not found: {}", path.display()));
    }

    // Try perry.toml
    if let Some(entry) = read_perry_toml_entry() {
        if entry.exists() {
            return Ok(entry);
        }
    }

    // Fallback: src/main.ts, then main.ts
    for candidate in &["src/main.ts", "main.ts"] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(anyhow!(
        "No input file specified and no main.ts found.\n\
         Usage: perry run <file.ts>\n\
         Or create src/main.ts or main.ts, or set entry in perry.toml"
    ))
}

/// Read entry point from perry.toml if present
fn read_perry_toml_entry() -> Option<PathBuf> {
    let toml_str = std::fs::read_to_string("perry.toml").ok()?;
    // Simple TOML parsing: look for entry = "..."
    for line in toml_str.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("entry") {
            if let Some(eq_pos) = trimmed.find('=') {
                let value = trimmed[eq_pos + 1..].trim().trim_matches('"');
                return Some(PathBuf::from(value));
            }
        }
    }
    None
}

/// Resolve the compilation target and optional device UDID
fn resolve_target(args: &RunArgs) -> Result<(Option<String>, Option<String>)> {
    if args.web {
        return Ok((Some("web".to_string()), None));
    }

    if args.android {
        let devices = detect_android_devices()?;
        if devices.is_empty() {
            return Err(anyhow!(
                "No Android devices found. Connect a device or start an emulator, then try again."
            ));
        }
        let serial = if devices.len() == 1 {
            devices[0].udid.clone()
        } else {
            pick_device(&devices, "Android device")?
        };
        return Ok((Some("android".to_string()), Some(serial)));
    }

    if args.ios {
        if let Some(ref udid) = args.simulator {
            return Ok((Some("ios-simulator".to_string()), Some(udid.clone())));
        }
        if let Some(ref udid) = args.device {
            return Ok((Some("ios".to_string()), Some(udid.clone())));
        }

        // Auto-detect: booted simulators + connected devices
        let simulators = detect_booted_simulators().unwrap_or_default();
        let devices = detect_ios_devices().unwrap_or_default();

        let mut all: Vec<(DeviceInfo, &str)> = Vec::new();
        for s in simulators {
            all.push((s, "ios-simulator"));
        }
        for d in devices {
            all.push((d, "ios"));
        }

        if all.is_empty() {
            return Err(anyhow!(
                "No iOS simulators or devices found.\n\
                 Boot a simulator:  xcrun simctl boot <UDID>\n\
                 Or specify one:    perry run --ios --simulator <UDID>"
            ));
        }

        if all.len() == 1 {
            let (dev, target) = all.remove(0);
            return Ok((Some(target.to_string()), Some(dev.udid)));
        }

        // Multiple options: prompt
        let names: Vec<String> = all.iter().map(|(d, t)| format!("{} ({})", d.name, t)).collect();
        let selection = pick_from_list(&names, "Select iOS target")?;
        let (dev, target) = all.remove(selection);
        return Ok((Some(target.to_string()), Some(dev.udid)));
    }

    if args.macos {
        return Ok((None, None));
    }

    // Default: native (no target flag)
    Ok((None, None))
}

/// Launch the compiled output based on target
fn launch(
    result: &CompileResult,
    device_udid: Option<&str>,
    program_args: &[String],
    format: OutputFormat,
) -> Result<()> {
    match result.target.as_str() {
        "web" => launch_web(&result.output_path, format),
        "ios-simulator" => {
            let udid = device_udid
                .ok_or_else(|| anyhow!("No simulator UDID — use --simulator <UDID>"))?;
            let bundle_id = result.bundle_id.as_deref()
                .ok_or_else(|| anyhow!("No bundle ID found for iOS app"))?;
            launch_ios_simulator(&result.output_path, bundle_id, udid, format)
        }
        "ios" => {
            let udid = device_udid
                .ok_or_else(|| anyhow!("No device UDID — use --device <UDID>"))?;
            let bundle_id = result.bundle_id.as_deref()
                .ok_or_else(|| anyhow!("No bundle ID found for iOS app"))?;
            launch_ios_device(&result.output_path, bundle_id, udid, format)
        }
        "android" => {
            if let OutputFormat::Text = format {
                println!();
                println!("Android .so compiled. Perry produces native libraries, not APKs.");
                println!("To test, integrate the .so into an Android project.");
            }
            Ok(())
        }
        _ => launch_native(&result.output_path, program_args, format),
    }
}

/// Launch a native executable
fn launch_native(exe_path: &std::path::Path, program_args: &[String], format: OutputFormat) -> Result<()> {
    let exe = if exe_path.is_absolute() {
        exe_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(exe_path)
    };

    if !exe.exists() {
        return Err(anyhow!("Compiled executable not found: {}", exe.display()));
    }

    if let OutputFormat::Text = format {
        println!();
        println!("Running {}...", exe_path.display());
        println!();
    }

    let status = Command::new(&exe)
        .args(program_args)
        .status()
        .map_err(|e| anyhow!("Failed to launch {}: {}", exe.display(), e))?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

/// Launch on iOS Simulator: install + launch
fn launch_ios_simulator(
    app_dir: &std::path::Path,
    bundle_id: &str,
    udid: &str,
    format: OutputFormat,
) -> Result<()> {
    if let OutputFormat::Text = format {
        println!();
        println!("Installing on simulator {}...", udid);
    }

    let install = Command::new("xcrun")
        .args(["simctl", "install", udid])
        .arg(app_dir)
        .status()
        .map_err(|e| anyhow!("Failed to run xcrun simctl install: {}", e))?;

    if !install.success() {
        return Err(anyhow!("Failed to install app on simulator {}", udid));
    }

    if let OutputFormat::Text = format {
        println!("Launching {}...", bundle_id);
        println!();
    }

    let launch = Command::new("xcrun")
        .args(["simctl", "launch", "--console-pty", udid, bundle_id])
        .status()
        .map_err(|e| anyhow!("Failed to run xcrun simctl launch: {}", e))?;

    if !launch.success() {
        return Err(anyhow!("App exited with error on simulator"));
    }
    Ok(())
}

/// Launch on a physical iOS device via devicectl (Xcode 15+)
fn launch_ios_device(
    app_dir: &std::path::Path,
    bundle_id: &str,
    udid: &str,
    format: OutputFormat,
) -> Result<()> {
    if let OutputFormat::Text = format {
        println!();
        println!("Installing on device {}...", udid);
    }

    let install = Command::new("xcrun")
        .args(["devicectl", "device", "install", "app", "--device", udid])
        .arg(app_dir)
        .status()
        .map_err(|e| anyhow!("Failed to run xcrun devicectl install: {}", e))?;

    if !install.success() {
        return Err(anyhow!("Failed to install app on device {}", udid));
    }

    if let OutputFormat::Text = format {
        println!("Launching {}...", bundle_id);
        println!();
    }

    let launch = Command::new("xcrun")
        .args([
            "devicectl", "device", "process", "launch",
            "--device", udid, bundle_id,
        ])
        .status()
        .map_err(|e| anyhow!("Failed to run xcrun devicectl launch: {}", e))?;

    if !launch.success() {
        return Err(anyhow!("App exited with error on device"));
    }
    Ok(())
}

/// Launch a web build: open HTML in browser
fn launch_web(html_path: &std::path::Path, format: OutputFormat) -> Result<()> {
    if let OutputFormat::Text = format {
        println!();
        println!("Opening {} in browser...", html_path.display());
    }

    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "start"
    } else {
        "xdg-open"
    };

    Command::new(cmd)
        .arg(html_path)
        .status()
        .map_err(|e| anyhow!("Failed to open browser: {}", e))?;

    Ok(())
}

// --- Device detection ---

/// Detect booted iOS simulators via `xcrun simctl list`
fn detect_booted_simulators() -> Result<Vec<DeviceInfo>> {
    let output = Command::new("xcrun")
        .args(["simctl", "list", "devices", "booted", "--json"])
        .output()
        .map_err(|e| anyhow!("Failed to run xcrun simctl: {}", e))?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .unwrap_or(serde_json::Value::Null);

    let mut devices = Vec::new();
    if let Some(device_map) = json.get("devices").and_then(|d| d.as_object()) {
        for (_runtime, device_list) in device_map {
            if let Some(arr) = device_list.as_array() {
                for dev in arr {
                    let state = dev.get("state").and_then(|s| s.as_str()).unwrap_or("");
                    if state == "Booted" {
                        if let (Some(udid), Some(name)) = (
                            dev.get("udid").and_then(|s| s.as_str()),
                            dev.get("name").and_then(|s| s.as_str()),
                        ) {
                            devices.push(DeviceInfo {
                                udid: udid.to_string(),
                                name: name.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(devices)
}

/// Detect connected iOS devices via `xcrun devicectl` (Xcode 15+)
fn detect_ios_devices() -> Result<Vec<DeviceInfo>> {
    let output = Command::new("xcrun")
        .args(["devicectl", "list", "devices", "--json-output", "-"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Ok(Vec::new()), // devicectl not available or failed
    };

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .unwrap_or(serde_json::Value::Null);

    let mut devices = Vec::new();
    // devicectl JSON structure: { "result": { "devices": [...] } }
    if let Some(arr) = json
        .get("result")
        .and_then(|r| r.get("devices"))
        .and_then(|d| d.as_array())
    {
        for dev in arr {
            let connected = dev
                .get("connectionProperties")
                .and_then(|c| c.get("transportType"))
                .and_then(|t| t.as_str())
                .is_some();
            if connected {
                if let Some(udid) = dev.get("identifier").and_then(|s| s.as_str()) {
                    let name = dev
                        .get("deviceProperties")
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("iOS Device");
                    devices.push(DeviceInfo {
                        udid: udid.to_string(),
                        name: name.to_string(),
                    });
                }
            }
        }
    }

    Ok(devices)
}

/// Detect connected Android devices via `adb devices`
fn detect_android_devices() -> Result<Vec<DeviceInfo>> {
    let output = Command::new("adb")
        .args(["devices", "-l"])
        .output()
        .map_err(|_| anyhow!("adb not found. Install Android SDK platform-tools."))?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('*') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == "device" {
            let serial = parts[0].to_string();
            // Try to extract model name from the line
            let name = parts.iter()
                .find(|p| p.starts_with("model:"))
                .map(|p| p.trim_start_matches("model:").to_string())
                .unwrap_or_else(|| serial.clone());
            devices.push(DeviceInfo {
                udid: serial,
                name,
            });
        }
    }

    Ok(devices)
}

// --- Interactive prompts ---

/// Pick a device from a list using dialoguer, or auto-select if non-interactive
fn pick_device(devices: &[DeviceInfo], label: &str) -> Result<String> {
    let names: Vec<String> = devices.iter().map(|d| format!("{} ({})", d.name, d.udid)).collect();
    let idx = pick_from_list(&names, &format!("Select {}", label))?;
    Ok(devices[idx].udid.clone())
}

/// Interactive selection from a list of options
fn pick_from_list(items: &[String], prompt: &str) -> Result<usize> {
    if items.is_empty() {
        return Err(anyhow!("No options available"));
    }

    // Non-interactive: pick first
    if !atty::is(atty::Stream::Stdin) {
        eprintln!("Non-interactive terminal, selecting: {}", items[0]);
        return Ok(0);
    }

    let selection = dialoguer::Select::new()
        .with_prompt(prompt)
        .items(items)
        .default(0)
        .interact()
        .map_err(|e| anyhow!("Selection cancelled: {}", e))?;

    Ok(selection)
}
