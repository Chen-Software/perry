//! Project configuration and environment variable resolution.
//!
//! Implements the priority chain for compose file discovery and project naming
//! as defined in the compose-spec and requirements 9.1–9.8.

use crate::error::{ComposeError, Result};
use std::path::{Path, PathBuf};

/// Default compose file names to search for, in priority order (req 9.6).
pub const DEFAULT_COMPOSE_FILES: &[&str] = &[
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

/// Project-level configuration holding raw CLI inputs for file paths, project name, and env files.
///
/// This is the *project-level* config struct — distinct from the compose-spec
/// `ComposeConfig` type in `types.rs` which describes a top-level `configs:` entry.
///
/// Use [`ProjectConfig::new`] to construct from CLI args, then pass to
/// [`crate::project::ComposeProject::load`] which runs the full resolution chain.
#[derive(Debug, Clone, Default)]
pub struct ProjectConfig {
    /// Compose file paths from `-f` flags (empty = use env var / default discovery).
    pub compose_files: Vec<PathBuf>,
    /// Project name from `-p` flag (`None` = use env var / directory name).
    pub project_name: Option<String>,
    /// Extra environment file paths from `--env-file` flags.
    pub env_files: Vec<PathBuf>,
}

impl ProjectConfig {
    /// Create a `ProjectConfig` from raw CLI inputs.
    ///
    /// No resolution is performed here; call [`crate::project::ComposeProject::load`]
    /// to run the full priority chain (req 9.1–9.8).
    pub fn new(
        compose_files: Vec<PathBuf>,
        project_name: Option<String>,
        env_files: Vec<PathBuf>,
    ) -> Self {
        ProjectConfig {
            compose_files,
            project_name,
            env_files,
        }
    }
}

/// Resolve the project name.
///
/// Priority (req 9.3, 9.4, 9.7):
/// 1. CLI `-p` / `--project-name` flag
/// 2. `COMPOSE_PROJECT_NAME` environment variable
/// 3. Directory name of the directory containing the primary compose file
pub fn resolve_project_name(cli_name: Option<&str>, project_dir: &Path) -> String {
    if let Some(name) = cli_name {
        if !name.is_empty() {
            return name.to_string();
        }
    }

    if let Ok(name) = std::env::var("COMPOSE_PROJECT_NAME") {
        if !name.is_empty() {
            return name;
        }
    }

    // Fall back to the directory name (req 9.7).
    project_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".to_string())
}

/// Resolve compose file paths.
///
/// Priority (req 9.1, 9.5, 9.6):
/// 1. CLI `-f` / `--file` flags — returned as-is; missing files produce an error (req 9.8)
/// 2. `COMPOSE_FILE` environment variable — colon-separated list of paths; missing files error
/// 3. Default file search in CWD: `compose.yaml`, `compose.yml`, `docker-compose.yaml`,
///    `docker-compose.yml` (in that order)
pub fn resolve_compose_files(cli_files: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if !cli_files.is_empty() {
        // Validate every explicitly-specified file exists (req 9.8).
        for path in cli_files {
            if !path.exists() {
                return Err(ComposeError::FileNotFound {
                    path: path.display().to_string(),
                });
            }
        }
        return Ok(cli_files.to_vec());
    }

    if let Ok(compose_file_env) = std::env::var("COMPOSE_FILE") {
        if !compose_file_env.is_empty() {
            // The compose-spec uses `:` on POSIX and `;` on Windows (req 9.5).
            #[cfg(target_os = "windows")]
            let separator = ";";
            #[cfg(not(target_os = "windows"))]
            let separator = ":";

            let paths: Vec<PathBuf> = compose_file_env
                .split(separator)
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect();

            // Validate every path from the env var (req 9.8).
            for path in &paths {
                if !path.exists() {
                    return Err(ComposeError::FileNotFound {
                        path: path.display().to_string(),
                    });
                }
            }

            if !paths.is_empty() {
                return Ok(paths);
            }
        }
    }

    // Fall back to searching CWD for a default compose file (req 9.6).
    let cwd = std::env::current_dir()?;
    find_default_compose_file(&cwd)
}

/// Search `dir` for the first default compose file that exists (req 9.6).
///
/// Returns `Err(ComposeError::FileNotFound)` if none are found.
pub fn find_default_compose_file(dir: &Path) -> Result<Vec<PathBuf>> {
    for name in DEFAULT_COMPOSE_FILES {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Ok(vec![candidate]);
        }
    }
    Err(ComposeError::FileNotFound {
        path: format!(
            "No compose file found in '{}' (tried: {})",
            dir.display(),
            DEFAULT_COMPOSE_FILES.join(", ")
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    // Use a global lock for tests that manipulate environment variables to avoid race conditions
    // when running tests in parallel (req 9.3, 9.4).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn make_temp_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("perry-config-test-{suffix}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    // ── resolve_project_name ──────────────────────────────────────────────────

    #[test]
    fn test_project_name_cli_takes_priority() {
        let dir = make_temp_dir("cli-priority");
        let name = resolve_project_name(Some("explicit-name"), &dir);
        assert_eq!(name, "explicit-name");
    }

    #[test]
    fn test_project_name_env_var_fallback() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = make_temp_dir("env-fallback");
        // Temporarily set the env var; restore afterwards.
        std::env::set_var("COMPOSE_PROJECT_NAME", "env-project");
        let name = resolve_project_name(None, &dir);
        std::env::remove_var("COMPOSE_PROJECT_NAME");
        assert_eq!(name, "env-project");
    }

    #[test]
    fn test_project_name_dir_fallback() {
        let _lock = ENV_LOCK.lock().unwrap();
        // Ensure env var is not set for this test.
        std::env::remove_var("COMPOSE_PROJECT_NAME");
        let dir = make_temp_dir("dir-fallback");
        let name = resolve_project_name(None, &dir);
        assert_eq!(name, "perry-config-test-dir-fallback");
    }

    #[test]
    fn test_project_name_empty_cli_falls_through_to_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = make_temp_dir("empty-cli");
        std::env::set_var("COMPOSE_PROJECT_NAME", "from-env");
        let name = resolve_project_name(Some(""), &dir);
        std::env::remove_var("COMPOSE_PROJECT_NAME");
        assert_eq!(name, "from-env");
    }

    // ── resolve_compose_files ─────────────────────────────────────────────────

    #[test]
    fn test_cli_files_returned_directly() {
        let dir = make_temp_dir("cli-files");
        let file = dir.join("compose.yaml");
        fs::write(&file, "services: {}").unwrap();

        let result = resolve_compose_files(&[file.clone()]).unwrap();
        assert_eq!(result, vec![file]);
    }

    #[test]
    fn test_cli_file_missing_returns_error() {
        let missing = PathBuf::from("/nonexistent/path/compose.yaml");
        let err = resolve_compose_files(&[missing.clone()]).unwrap_err();
        match err {
            ComposeError::FileNotFound { path } => {
                assert!(path.contains("nonexistent"));
            }
            other => panic!("expected FileNotFound, got {other:?}"),
        }
    }

    #[test]
    fn test_default_file_discovery_compose_yaml() {
        let dir = make_temp_dir("default-discovery");
        let file = dir.join("compose.yaml");
        fs::write(&file, "services: {}").unwrap();

        // Use find_default_compose_file directly to avoid set_current_dir races.
        let result = find_default_compose_file(&dir).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name().unwrap(), "compose.yaml");
    }

    #[test]
    fn test_default_file_discovery_docker_compose_yml_fallback() {
        let dir = make_temp_dir("docker-compose-fallback");
        let file = dir.join("docker-compose.yml");
        fs::write(&file, "services: {}").unwrap();

        let result = find_default_compose_file(&dir).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name().unwrap(), "docker-compose.yml");
    }

    #[test]
    fn test_no_compose_file_returns_error() {
        let dir = make_temp_dir("no-file");
        let result = find_default_compose_file(&dir);
        assert!(matches!(result, Err(ComposeError::FileNotFound { .. })));
    }

    // ── ProjectConfig::new ────────────────────────────────────────────────────

    #[test]
    fn test_project_config_new_stores_raw_inputs() {
        let dir = make_temp_dir("project-config");
        let file = dir.join("compose.yaml");
        fs::write(&file, "services: {}").unwrap();

        let cfg = ProjectConfig::new(vec![file.clone()], Some("my-project".into()), vec![]);
        assert_eq!(cfg.project_name, Some("my-project".to_string()));
        assert_eq!(cfg.compose_files, vec![file]);
        assert!(cfg.env_files.is_empty());
    }
}
