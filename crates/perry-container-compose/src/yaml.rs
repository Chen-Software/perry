use regex::{Regex, Captures};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;

/// Interpolate ${VAR}, ${VAR:-default}, ${VAR:+value} in a YAML string.
pub fn interpolate_yaml(yaml: &str, env: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\$\{(?P<var>[A-Z0-9_]+)(?:(?P<mod>:-|:\+)(?P<val>[^}]*))?\}").unwrap();

    re.replace_all(yaml, |caps: &Captures| {
        let var_name = &caps["var"];
        let modifier = caps.name("mod").map(|m| m.as_str());
        let mod_val = caps.name("val").map(|v| v.as_str()).unwrap_or("");

        match (env.get(var_name), modifier) {
            (Some(_v), Some(":+")) => mod_val.to_string(),
            (Some(v), _) if !v.is_empty() => v.clone(),
            (_, Some(":-")) => mod_val.to_string(),
            _ => String::new(),
        }
    }).to_string()
}

/// Load a .env file and merge with process environment.
pub fn load_env(project_dir: &Path, extra_env_files: &[PathBuf]) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();

    // Default .env in project directory
    let default_env = project_dir.join(".env");
    if default_env.exists() {
        if let Ok(content) = std::fs::read_to_string(&default_env) {
            for (k, v) in parse_dotenv(&content) {
                env.entry(k).or_insert(v);
            }
        }
    }

    // Explicit --env-file flags override earlier values
    for ef in extra_env_files {
        if let Ok(content) = std::fs::read_to_string(ef) {
            for (k, v) in parse_dotenv(&content) {
                env.insert(k, v);
            }
        }
    }

    env
}

fn parse_dotenv(content: &str) -> Vec<(String, String)> {
    content.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .filter_map(|l| {
            let mut parts = l.splitn(2, '=');
            let k = parts.next()?.trim().to_string();
            let v = parts.next().unwrap_or("").trim().trim_matches('"').trim_matches('\'').to_string();
            Some((k, v))
        })
        .collect()
}

/// Parse a compose YAML string into a ComposeSpec after interpolation.
pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate_yaml(yaml, env);
    serde_yaml::from_str(&interpolated).map_err(ComposeError::ParseError)
}

pub fn parse_and_merge_files(files: &[PathBuf], env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let mut merged = ComposeSpec::default();
    for f in files {
        let content = std::fs::read_to_string(f).map_err(|_| ComposeError::FileNotFound { path: f.to_string_lossy().to_string() })?;
        let spec = parse_compose_yaml(&content, env)?;
        merged.merge(spec);
    }
    Ok(merged)
}
