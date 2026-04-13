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

    #[arg(long, env = "ROBDEX_BRIDGE_QA_HARNESS_URL", default_value = "http://127.0.0.1:8775")]
    pub qa_harness_url: String,

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
    pub qa_harness_url: String,
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
        let project_path = canonical_or_original(&self.project_path)
            .with_context(|| format!("failed to resolve {}", self.project_path.display()))?;
        let cwd =
            canonical_or_original(&self.cwd).with_context(|| format!("failed to resolve {}", self.cwd.display()))?;
        let state_root = canonical_or_original(&self.state_root)
            .or_else(|_| Ok::<PathBuf, anyhow::Error>(self.state_root.clone()))?;

        if !project_path.is_dir() {
            bail!("project path is not a directory: {}", project_path.display());
        }
        if !cwd.is_dir() {
            bail!("cwd is not a directory: {}", cwd.display());
        }

        let paths = BridgePaths::new(state_root);
        Ok(BridgeSettings {
            http: self.http.clone(),
            app_server_url: self.app_server_url.clone(),
            qa_harness_url: self.qa_harness_url.clone(),
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

fn default_project_path() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/Users/robertsale"))
        .join("Code/robdex")
}

fn default_state_root() -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/Users/robertsale"))
                .join(".codex")
        })
        .join("robdex")
}

#[cfg(test)]
mod tests {
    use super::*;
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
            qa_harness_url: "http://127.0.0.1:8775".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            state_root: temp.path().join("state"),
        };

        let settings = args.settings().expect("settings");
        assert_eq!(settings.paths.state_json, temp.path().join("state/robdex.json"));
        assert_eq!(settings.paths.sqlite_db, temp.path().join("state/robdex.sqlite"));
    }
}
