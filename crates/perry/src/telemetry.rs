//! Anonymous usage statistics for Perry CLI
//!
//! Opt-in telemetry via Chirp API. On first interactive run, the user is asked
//! once if stats collection is OK (default: yes). All telemetry is fire-and-forget
//! on background threads — never slows down the CLI.

use serde::{Deserialize, Serialize};

use crate::commands::publish::{load_config, save_config};

const CHIRP_URL: &str = "https://api.chirp247.com/api/v1/event";
const CHIRP_KEY: &str = "testkey123";
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TelemetryConfig {
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) client_id: String,
}

/// Returns true if telemetry should be skipped entirely.
fn should_skip_telemetry() -> bool {
    if std::env::var("PERRY_NO_TELEMETRY").map_or(false, |v| v == "1" || v == "true") {
        return true;
    }
    if std::env::var("CI").map_or(false, |v| v == "true" || v == "1") {
        return true;
    }
    if !atty::is(atty::Stream::Stderr) {
        return true;
    }
    false
}

/// Load telemetry config from ~/.perry/config.toml.
/// Returns None if no [telemetry] section exists (= never asked).
fn load_telemetry_config() -> Option<TelemetryConfig> {
    let config = load_config();
    config.telemetry
}

/// Save telemetry config, preserving all other config sections.
fn save_telemetry_config(telemetry: &TelemetryConfig) {
    let mut config = load_config();
    config.telemetry = Some(telemetry.clone());
    let _ = save_config(&config);
}

/// Generate a random client ID (UUID-like hex string).
fn generate_client_id() -> String {
    let mut bytes = [0u8; 16];

    // Try /dev/urandom first (Unix) — must use Read trait, not fs::read (infinite device)
    let got_random = {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .is_ok()
    };

    if !got_random {
        // Fallback: time-based
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let nanos = t.as_nanos();
        for i in 0..16 {
            bytes[i] = ((nanos >> (i * 4)) & 0xFF) as u8;
        }
    }

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

/// Prompt the user for telemetry consent. Returns true if they opt in.
/// Only prompts on interactive TTY. Non-interactive sessions get false without saving.
fn prompt_consent() -> bool {
    if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stdout) {
        return false;
    }

    let consent = dialoguer::Confirm::new()
        .with_prompt("Help improve Perry by sending anonymous usage statistics?")
        .default(true)
        .interact()
        .unwrap_or(false);

    let config = TelemetryConfig {
        enabled: consent,
        client_id: generate_client_id(),
    };
    save_telemetry_config(&config);

    consent
}

/// Check skip conditions, load config, prompt if needed.
/// Returns true if telemetry is active for this session.
pub(crate) fn init_and_check_consent() -> bool {
    if should_skip_telemetry() {
        return false;
    }

    match load_telemetry_config() {
        Some(config) => config.enabled,
        None => prompt_consent(),
    }
}

/// Fire-and-forget: send an event on a background thread.
/// All errors are silently ignored.
pub(crate) fn send_event(event: &str, dims: &[(&str, &str)]) {
    let config = match load_telemetry_config() {
        Some(c) if c.enabled => c,
        _ => return,
    };

    let event = event.to_string();
    let dims: Vec<(String, String)> = dims.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let client_id = config.client_id.clone();

    std::thread::spawn(move || {
        send_event_blocking(&event, &dims, &client_id);
    });
}

/// Actual HTTP POST to Chirp API.
/// Chirp expects `dims` object with known keys (platform, target, version, status, etc.).
fn send_event_blocking(event: &str, dims: &[(String, String)], client_id: &str) {
    let client = match reqwest::blocking::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let mut dims_obj = serde_json::Map::new();
    for (k, v) in dims.iter().take(4) {
        dims_obj.insert(k.clone(), serde_json::Value::String(v.clone()));
    }

    let body = serde_json::json!({
        "event": event,
        "dims": dims_obj,
    });

    let _ = client
        .post(CHIRP_URL)
        .header("Content-Type", "application/json")
        .header("X-Chirp-Key", CHIRP_KEY)
        .header("X-Chirp-Client", client_id)
        .json(&body)
        .send();
}
