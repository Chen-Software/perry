//! Project management — compose file loading, merging, project name resolution.

use crate::entities::compose::Compose;
use crate::error::{ComposeError, Result};
use crate::orchestrate::env::{interpolate, parse_dotenv};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Default compose file names to search for (in priority order)
pub const DEFAULT_COMPOSE_FILES: &[&str] = &[
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

/// A loaded and resolved project
pub struct Project {
    /// Project name
    pub name: String,
    /// Working directory (directory of the primary compose file)
    pub working_dir: PathBuf,
    /// Merged and interpolated compose spec
    pub compose: Compose,
    /// Resolved environment variables (from .env + process env)
    pub env: HashMap<String, String>,
}

impl Project {
    /// Load a project from one or more compose files.
    ///
    /// If `files` is empty, searches the current directory for a default file.
    pub fn load(
        files: &[PathBuf],
        project_name: Option<&str>,
        env_files: &[PathBuf],
    ) -> Result<Self> {
        // Resolve compose file paths
        let resolved_files = if files.is_empty() {
            let cwd = std::env::current_dir()?;
            vec![find_default_compose_file(&cwd)?]
        } else {
            files.to_vec()
        };

        let working_dir = resolved_files[0]
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();

        // Load .env files
        let mut env = std::env::vars().collect::<HashMap<_, _>>();

        // Default .env in working dir
        let default_env = working_dir.join(".env");
        if default_env.exists() {
            let content = std::fs::read_to_string(&default_env)?;
            let file_env = parse_dotenv(&content);
            // .env values do NOT override existing process environment
            for (k, v) in file_env {
                env.entry(k).or_insert(v);
            }
        }

        // Explicit --env-file flags (override earlier values)
        for ef in env_files {
            let content = std::fs::read_to_string(ef)?;
            let file_env = parse_dotenv(&content);
            for (k, v) in file_env {
                env.insert(k, v);
            }
        }

        // Read COMPOSE_PROJECT_NAME from env if present
        let name_from_env = env.get("COMPOSE_PROJECT_NAME").cloned();

        // Parse and merge compose files
        let mut merged: Option<Compose> = None;
        for file_path in &resolved_files {
            let content = std::fs::read_to_string(file_path).map_err(|_| {
                ComposeError::FileNotFound {
                    path: file_path.display().to_string(),
                }
            })?;
            // Interpolate environment variables in YAML before parsing
            let interpolated = interpolate(&content, &env);
            let compose = Compose::parse_str(&interpolated)?;

            match &mut merged {
                None => merged = Some(compose),
                Some(base) => base.merge(compose),
            }
        }

        let compose = merged.unwrap_or_default();

        // Determine project name (priority: CLI flag > env > working dir name)
        let name = project_name
            .map(String::from)
            .or(name_from_env)
            .unwrap_or_else(|| {
                working_dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });

        Ok(Project {
            name,
            working_dir,
            compose,
            env,
        })
    }
}

fn find_default_compose_file(dir: &Path) -> Result<PathBuf> {
    for name in DEFAULT_COMPOSE_FILES {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(ComposeError::FileNotFound {
        path: format!(
            "No compose file found in {} (tried: {})",
            dir.display(),
            DEFAULT_COMPOSE_FILES.join(", ")
        ),
    })
}
