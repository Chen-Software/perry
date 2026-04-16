//! YAML parsing, environment interpolation, and file merging.

use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Interpolate ${VAR}, ${VAR:-default}, ${VAR:+value} in a YAML string.
pub fn interpolate_yaml(yaml: &str, env: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\$\{(?P<var>[A-Z0-9_]+)(?::(?P<op>[-+])(?P<val>[^}]*))?\}").unwrap();
    re.replace_all(yaml, |caps: &Captures| {
        let var = caps.name("var").unwrap().as_str();
        let op = caps.name("op").map(|m| m.as_str());
        let val = caps.name("val").map(|m| m.as_str()).unwrap_or("");

        match (env.get(var), op) {
            (Some(_v), Some("+")) => val.to_string(),
            (Some(v), _) => v.clone(),
            (None, Some("-")) => val.to_string(),
            (None, _) => String::new(),
        }
    })
    .into_owned()
}

/// Load .env file and merge with process environment.
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

fn parse_dotenv(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"').trim_matches('\'');
            map.insert(k.to_string(), v.to_string());
        }
    }
    map
}

/// Parse, interpolate, and deserialize a YAML string.
pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate_yaml(yaml, env);
    serde_yaml::from_str(&interpolated).map_err(ComposeError::ParseError)
}

/// Read, parse, interpolate and merge multiple compose files.
pub fn parse_and_merge_files(files: &[PathBuf], env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let mut root_spec = ComposeSpec::default();
    for f in files {
        let content = std::fs::read_to_string(f).map_err(|_| ComposeError::FileNotFound {
            path: f.to_string_lossy().to_string(),
        })?;
        let spec = parse_compose_yaml(&content, env)?;
        root_spec.merge(spec);
    }
    Ok(root_spec)
}

impl ComposeSpec {
    /// Merges another spec into this one (last-writer-wins for maps).
    pub fn merge(&mut self, other: ComposeSpec) {
        for (name, service) in other.services {
            self.services.insert(name, service);
        }
        if let Some(nets) = other.networks {
            let existing = self.networks.get_or_insert_with(indexmap::IndexMap::new);
            for (name, net) in nets {
                existing.insert(name, net);
            }
        }
        if let Some(vols) = other.volumes {
            let existing = self.volumes.get_or_insert_with(indexmap::IndexMap::new);
            for (name, vol) in vols {
                existing.insert(name, vol);
            }
        }
        if let Some(secs) = other.secrets {
            let existing = self.secrets.get_or_insert_with(indexmap::IndexMap::new);
            for (name, sec) in secs {
                existing.insert(name, sec);
            }
        }
        if let Some(cfgs) = other.configs {
            let existing = self.configs.get_or_insert_with(indexmap::IndexMap::new);
            for (name, cfg) in cfgs {
                existing.insert(name, cfg);
            }
        }
        if other.name.is_some() {
            self.name = other.name;
        }
        for (k, v) in other.extensions {
            self.extensions.insert(k, v);
        }
    }
}
