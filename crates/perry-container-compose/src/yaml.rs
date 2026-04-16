use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Interpolate ${VAR}, ${VAR:-default}, ${VAR:+value} in a YAML string.
pub fn interpolate(yaml: &str, env: &HashMap<String, String>) -> String {
    let mut result = String::new();
    let mut i = 0;
    let bytes = yaml.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            if let Some(end) = yaml[i..].find('}') {
                let end = i + end;
                let var_spec = &yaml[i + 2..end];

                let val = if let Some(idx) = var_spec.find(":-") {
                    let var_name = &var_spec[..idx];
                    let default = &var_spec[idx + 2..];
                    env.get(var_name).cloned().unwrap_or_else(|| default.to_string())
                } else if let Some(idx) = var_spec.find(":+") {
                    let var_name = &var_spec[..idx];
                    let value = &var_spec[idx + 2..];
                    if env.contains_key(var_name) { value.to_string() } else { String::new() }
                } else {
                    env.get(var_spec).cloned().unwrap_or_default()
                };

                result.push_str(&val);
                i = end + 1;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

pub fn load_env(project_dir: &Path, extra_env_files: &[PathBuf]) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();

    let default_env = project_dir.join(".env");
    if default_env.exists() {
        if let Ok(content) = std::fs::read_to_string(&default_env) {
            for (k, v) in parse_dotenv(&content) {
                env.entry(k).or_insert(v);
            }
        }
    }

    for ef in extra_env_files {
        if let Ok(content) = std::fs::read_to_string(ef) {
            for (k, v) in parse_dotenv(&content) {
                env.insert(k, v);
            }
        }
    }
    env
}

pub fn parse_dotenv(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim().trim_matches('"').trim_matches('\'');
            map.insert(k.trim().to_string(), v.to_string());
        }
    }
    map
}

pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate(yaml, env);
    serde_yaml::from_str(&interpolated).map_err(ComposeError::ParseError)
}

impl ComposeSpec {
    pub fn parse_str(yaml: &str) -> Result<Self> {
        parse_compose_yaml(yaml, &std::collections::HashMap::new())
    }
}
impl ComposeSpec {
    pub fn merge(&mut self, other: ComposeSpec) {
        for (name, service) in other.services {
            self.services.insert(name, service);
        }
        if let Some(nets) = other.networks {
            let existing = self.networks.get_or_insert_with(Default::default);
            for (name, net) in nets { existing.insert(name, net); }
        }
        if let Some(vols) = other.volumes {
            let existing = self.volumes.get_or_insert_with(Default::default);
            for (name, vol) in vols { existing.insert(name, vol); }
        }
        if let Some(secs) = other.secrets {
            let existing = self.secrets.get_or_insert_with(Default::default);
            for (name, sec) in secs { existing.insert(name, sec); }
        }
        if let Some(cfgs) = other.configs {
            let existing = self.configs.get_or_insert_with(Default::default);
            for (name, cfg) in cfgs { existing.insert(name, cfg); }
        }
        if other.name.is_some() { self.name = other.name; }
        if other.version.is_some() { self.version = other.version; }
        for (k, v) in other.extensions { self.extensions.insert(k, v); }
    }
}
