use std::{env, path::{Path, PathBuf}};

use anyhow::{Context, Result, bail};
use clap::Parser;
use codex_backend_core::HttpArgs;

#[derive(Debug, Clone, Parser)]
pub struct BridgeArgs {
    #[command(flatten)]
    pub http: HttpArgs,

    #[arg(long, env = "ROBDEX_BRIDGE_APP_SERVER_URL", default_value = "ws://127.0.0.1:4200")]
    pub app_server_url: String,

    #[arg(long, env = "ROBDEX_BRIDGE_PROJECT_PATH", default_value_os_t = default_project_path())]
    pub project_path: PathBuf,

    #[arg(long, env = "ROBDEX_BRIDGE_CWD", default_value_os_t = default_project_path())]
    pub cwd: PathBuf,

    #[arg(long, env = "ROBDEX_BRIDGE_STATE_ROOT", default_value_os_t = default_state_root())]
    pub state_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct BridgeSettings {
    pub http: HttpArgs,
    pub app_server_url: String,
    pub project_path: PathBuf,
    pub cwd: PathBuf,
    pub paths: BridgePaths,
}

#[derive(Debug, Clone)]
pub struct BridgePaths {
    pub state_root: PathBuf,
    pub state_json: PathBuf,
    pub sqlite_db: PathBuf,
}

impl BridgeArgs {
    pub fn settings(&self) -> Result<BridgeSettings> {
        let project_path = resolve_existing_dir_or_fallback("project path", &self.project_path)
            .with_context(|| format!("failed to resolve {}", self.project_path.display()))?;
        let cwd = resolve_existing_dir_or_fallback("cwd", &self.cwd)
            .with_context(|| format!("failed to resolve {}", self.cwd.display()))?;
        let state_root = canonical_or_original(&self.state_root)
            .or_else(|_| Ok::<PathBuf, anyhow::Error>(self.state_root.clone()))?;

        let paths = BridgePaths::new(state_root);
        Ok(BridgeSettings {
            http: self.http.clone(),
            app_server_url: self.app_server_url.clone(),
            project_path,
            cwd,
            paths,
        })
    }
}

impl BridgePaths {
    pub fn new(state_root: PathBuf) -> Self {
        Self {
            state_json: state_root.join("robdex.json"),
            sqlite_db: state_root.join("robdex.sqlite"),
            state_root,
        }
    }

    pub fn ensure_parent_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.state_root)
            .with_context(|| format!("failed to create {}", self.state_root.display()))?;
        Ok(())
    }
}

fn canonical_or_original(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .or_else(|_| Ok::<PathBuf, anyhow::Error>(path.to_path_buf()))
}

fn resolve_existing_dir_or_fallback(label: &str, path: &Path) -> Result<PathBuf> {
    let resolved = canonical_or_original(path)?;
    if resolved.is_dir() {
        return Ok(resolved);
    }

    if let Some(fallback) = fallback_existing_dir(&resolved) {
        eprintln!(
            "robdex bridge warning: configured {label} does not exist: {}; using {}",
            resolved.display(),
            fallback.display()
        );
        return Ok(fallback);
    }

    bail!("{label} is not a directory: {}", resolved.display())
}

fn fallback_existing_dir(original: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(value) = env::var_os("ROBDEX_HOME") {
        candidates.push(PathBuf::from(value));
    }
    if let Ok(value) = env::current_dir() {
        candidates.push(value);
    }
    if let Some(value) = home_dir() {
        candidates.push(value);
    }

    candidates.into_iter().find_map(|candidate| {
        let resolved = canonical_or_original(&candidate).ok()?;
        (resolved.is_dir() && resolved != original).then_some(resolved)
    })
}

fn default_project_path() -> PathBuf {
    env::var_os("ROBDEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| home_dir().unwrap_or_else(|| PathBuf::from(".")))
}

fn default_state_root() -> PathBuf {
    if let Some(state_home) = env::var_os("ROBDEX_STATE_HOME") {
        return PathBuf::from(state_home);
    }
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".codex"))
        .join("robdex")
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;
    use tempfile::TempDir;

    #[test]
    fn bridge_paths_derive_expected_files() {
        let root = PathBuf::from("/tmp/robdex-state");
        let paths = BridgePaths::new(root.clone());
        assert_eq!(paths.state_root, root);
        assert_eq!(paths.state_json, PathBuf::from("/tmp/robdex-state/robdex.json"));
        assert_eq!(paths.sqlite_db, PathBuf::from("/tmp/robdex-state/robdex.sqlite"));
    }

    #[test]
    fn settings_accept_temp_paths() {
        let temp = TempDir::new().expect("tempdir");
        let args = BridgeArgs {
            http: HttpArgs {
                host: "127.0.0.1".parse::<IpAddr>().expect("ip"),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            state_root: temp.path().join("state"),
        };

        let settings = args.settings().expect("settings");
        assert_eq!(settings.paths.state_json, temp.path().join("state/robdex.json"));
        assert_eq!(settings.paths.sqlite_db, temp.path().join("state/robdex.sqlite"));
    }

    #[test]
    fn settings_fall_back_when_configured_project_paths_disappear() {
        let temp = TempDir::new().expect("tempdir");
        let fallback = temp.path().join("fallback");
        std::fs::create_dir_all(&fallback).expect("fallback dir");
        let missing = temp.path().join("deleted-project");
        let previous = env::var_os("ROBDEX_HOME");
        unsafe {
            env::set_var("ROBDEX_HOME", &fallback);
        }

        let args = BridgeArgs {
            http: HttpArgs {
                host: "127.0.0.1".parse::<IpAddr>().expect("ip"),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: missing.clone(),
            cwd: missing,
            state_root: temp.path().join("state"),
        };

        let settings = args.settings().expect("settings");
        assert_eq!(settings.project_path, fallback.canonicalize().expect("canonical fallback"));
        assert_eq!(settings.cwd, settings.project_path);

        unsafe {
            match previous {
                Some(value) => env::set_var("ROBDEX_HOME", value),
                None => env::remove_var("ROBDEX_HOME"),
            }
        }
    }

    #[test]
    fn default_state_root_honors_robdex_state_home() {
        let temp = TempDir::new().expect("tempdir");
        let previous = env::var_os("ROBDEX_STATE_HOME");
        unsafe {
            env::set_var("ROBDEX_STATE_HOME", temp.path().join("custom-state"));
        }

        assert_eq!(default_state_root(), temp.path().join("custom-state"));

        unsafe {
            match previous {
                Some(value) => env::set_var("ROBDEX_STATE_HOME", value),
                None => env::remove_var("ROBDEX_STATE_HOME"),
            }
        }
    }

    #[test]
    fn default_project_path_honors_robdex_home() {
        let temp = TempDir::new().expect("tempdir");
        let previous = env::var_os("ROBDEX_HOME");
        unsafe {
            env::set_var("ROBDEX_HOME", temp.path());
        }

        assert_eq!(default_project_path(), temp.path());

        unsafe {
            match previous {
                Some(value) => env::set_var("ROBDEX_HOME", value),
                None => env::remove_var("ROBDEX_HOME"),
            }
        }
    }
}
