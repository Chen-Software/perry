//! YAML parsing, environment variable interpolation, `.env` loading,
//! and multi-file merge.

use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use regex::Regex;

pub fn interpolate_yaml(yaml: &str, env: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\$\{([^}:]+)(?::([-+])([^}]*))?\}").unwrap();
    re.replace_all(yaml, |caps: &regex::Captures| {
        let name = &caps[1];
        let op = caps.get(2).map(|m| m.as_str());
        let arg = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        let val = env.get(name).cloned().or_else(|| std::env::var(name).ok()).unwrap_or_default();
        match op {
            Some("-") => if val.is_empty() { arg.to_string() } else { val },
            Some("+") => if !val.is_empty() { arg.to_string() } else { "".to_string() },
            _ => val,
        }
    }).to_string()
}

pub fn interpolate(input: &str, env: &HashMap<String, String>) -> String { interpolate_yaml(input, env) }

pub fn parse_dotenv(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim().to_owned();
            let v = v.trim();
            let v = if (v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')) { &v[1..v.len()-1] } else { v.split_once(" #").map(|(v,_)| v).unwrap_or(v).trim() };
            map.insert(k, v.to_string());
        }
    }
    map
}

pub fn load_env(project_dir: &Path, extra_env_files: &[PathBuf]) -> HashMap<String, String> {
    let mut env = HashMap::new();
    if let Ok(c) = std::fs::read_to_string(project_dir.join(".env")) { env.extend(parse_dotenv(&c)); }
    for f in extra_env_files { if let Ok(c) = std::fs::read_to_string(f) { env.extend(parse_dotenv(&c)); } }
    for (k, v) in std::env::vars() { env.insert(k, v); }
    env
}

pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate_yaml(yaml, env);
    serde_yaml::from_str(&interpolated).map_err(ComposeError::ParseError)
}

pub fn parse_and_merge_files(files: &[PathBuf], env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let mut merged: Option<ComposeSpec> = None;
    for f in files {
        let c = std::fs::read_to_string(f).map_err(|_| ComposeError::FileNotFound { path: f.display().to_string() })?;
        let s = parse_compose_yaml(&c, env)?;
        match &mut merged { Some(m) => m.merge(s), None => merged = Some(s) }
    }
    Ok(merged.unwrap_or_default())
}
