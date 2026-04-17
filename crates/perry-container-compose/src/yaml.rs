use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::error::{ComposeError, Result};
use crate::types::ComposeSpec;

pub fn interpolate_yaml(yaml: &str, env: &HashMap<String, String>) -> String {
    // Regex for ${VAR}, ${VAR:-default}, ${VAR:+value}
    let re = Regex::new(r"\$\{(?P<var>[A-Z0-9_]+)(?::(?P<op>[-+])(?P<val>.*?))?\}").unwrap();
    re.replace_all(yaml, |caps: &regex::Captures| {
        let var_name = &caps["var"];
        let op = caps.name("op").map(|m| m.as_str());
        let val = caps.name("val").map(|m| m.as_str()).unwrap_or("");

        match op {
            Some("-") => {
                // ${VAR:-default}: use default if VAR is unset or empty
                let v = env.get(var_name).map(|s| s.as_str()).unwrap_or("");
                if v.is_empty() { val.to_string() } else { v.to_string() }
            }
            Some("+") => {
                // ${VAR:+value}: use value if VAR is set and not empty, otherwise empty
                let v = env.get(var_name).map(|s| s.as_str()).unwrap_or("");
                if !v.is_empty() { val.to_string() } else { "".to_string() }
            }
            _ => {
                // ${VAR}: use VAR or empty
                env.get(var_name).cloned().unwrap_or_default()
            }
        }
    }).to_string()
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

pub fn parse_dotenv(content: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }

        // Split at first '=' that isn't inside quotes
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim().to_string();
            let mut val = line[pos+1..].trim();

            // Handle quotes
            if (val.starts_with('"') && val.ends_with('"')) || (val.starts_with('\'') && val.ends_with('\'')) {
                val = &val[1..val.len()-1];
            }

            // Handle trailing comments
            let val = val.splitn(2, '#').next().unwrap().trim();

            results.push((key, val.to_string()));
        }
    }
    results
}

pub fn parse_compose_yaml(yaml: &str, env: &HashMap<String, String>) -> Result<ComposeSpec> {
    let interpolated = interpolate_yaml(yaml, env);
    serde_yaml::from_str(&interpolated).map_err(ComposeError::ParseError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation() {
        let mut env = HashMap::new();
        env.insert("VAR".to_string(), "val".to_string());

        assert_eq!(interpolate_yaml("hello ${VAR}", &env), "hello val");
        assert_eq!(interpolate_yaml("hello ${MISSING:-default}", &env), "hello default");
        assert_eq!(interpolate_yaml("hello ${VAR:-default}", &env), "hello val");
        assert_eq!(interpolate_yaml("hello ${VAR:+set}", &env), "hello set");
        assert_eq!(interpolate_yaml("hello ${MISSING:+set}", &env), "hello ");
    }

    #[test]
    fn test_dotenv_parsing() {
        let content = "
            # Comment
            KEY=VAL
            QUOTED=\"quoted val\"
            COMMENTED=val # trailing
            SPACED = val
        ";
        let parsed = parse_dotenv(content);
        assert_eq!(parsed.len(), 4);
        assert_eq!(parsed[0], ("KEY".to_string(), "VAL".to_string()));
        assert_eq!(parsed[1], ("QUOTED".to_string(), "quoted val".to_string()));
        assert_eq!(parsed[2], ("COMMENTED".to_string(), "val".to_string()));
        assert_eq!(parsed[3], ("SPACED".to_string(), "val".to_string()));
    }
}

pub fn parse_and_merge_files(files: &[PathBuf], env: &HashMap<String, String>) -> Result<ComposeSpec> {
    if files.is_empty() {
        return Err(ComposeError::ValidationError { message: "No compose files specified".into() });
    }

    let mut root_spec: Option<ComposeSpec> = None;

    for path in files {
        let content = std::fs::read_to_string(path).map_err(|_| ComposeError::FileNotFound { path: path.to_string_lossy().to_string() })?;
        let spec = parse_compose_yaml(&content, env)?;

        if let Some(mut root) = root_spec.take() {
            root.merge(spec);
            root_spec = Some(root);
        } else {
            root_spec = Some(spec);
        }
    }

    root_spec.ok_or_else(|| ComposeError::ValidationError { message: "No compose files specified".into() })
}
