use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::models::DeviceType;

const DEFAULT_CONFIG_DIR: &str = "/Users/robertsale/.qa-harness/config/projects";
const DEFAULT_STATE_ROOT: &str = "/Users/robertsale/.qa-harness";

#[derive(Debug, Clone, Parser)]
pub struct HarnessArgs {
    #[arg(long, env = "CODEX_QA_HARNESS_BIND", default_value = "127.0.0.1")]
    pub host: std::net::IpAddr,

    #[arg(long, env = "CODEX_QA_HARNESS_PORT", default_value_t = 8775)]
    pub port: u16,

    #[arg(long, env = "CODEX_QA_HARNESS_CONFIG_DIR", default_value = DEFAULT_CONFIG_DIR)]
    pub config_dir: PathBuf,

    #[arg(long, env = "CODEX_QA_HARNESS_STATE_ROOT", default_value = DEFAULT_STATE_ROOT)]
    pub state_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    pub projects: BTreeMap<String, ProjectConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub id: String,
    pub display_name: String,
    pub repo_root: PathBuf,
    pub runtime_root: PathBuf,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceConfig>,
    pub hooks: HooksConfig,
    #[serde(default)]
    pub timeouts: TimeoutsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    #[serde(rename = "type")]
    pub device_type: DeviceType,
    pub device_id: String,
    pub name: String,
    pub runtime_subdir: String,
    #[serde(default = "default_boot_policy")]
    pub boot_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    pub prepare_source: PathBuf,
    pub start_dependencies: Option<PathBuf>,
    pub start_runtime: PathBuf,
    pub check_readiness: PathBuf,
    pub teardown: PathBuf,
    pub command: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutsConfig {
    #[serde(default = "default_boot_simulator_sec")]
    pub boot_simulator_sec: u64,
    #[serde(default = "default_prepare_source_sec")]
    pub prepare_source_sec: u64,
    #[serde(default = "default_start_dependencies_sec")]
    pub start_dependencies_sec: u64,
    #[serde(default = "default_start_runtime_sec")]
    pub start_runtime_sec: u64,
    #[serde(default = "default_readiness_sec")]
    pub readiness_sec: u64,
    #[serde(default = "default_command_sec")]
    pub command_sec: u64,
    #[serde(default = "default_teardown_sec")]
    pub teardown_sec: u64,
}

impl Default for TimeoutsConfig {
    fn default() -> Self {
        Self {
            boot_simulator_sec: default_boot_simulator_sec(),
            prepare_source_sec: default_prepare_source_sec(),
            start_dependencies_sec: default_start_dependencies_sec(),
            start_runtime_sec: default_start_runtime_sec(),
            readiness_sec: default_readiness_sec(),
            command_sec: default_command_sec(),
            teardown_sec: default_teardown_sec(),
        }
    }
}

pub fn load_harness_config(config_dir: &Path) -> Result<HarnessConfig> {
    let mut projects = BTreeMap::new();
    if !config_dir.exists() {
        return Ok(HarnessConfig { projects });
    }

    for entry in fs::read_dir(config_dir)
        .with_context(|| format!("read config directory {}", config_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read config file {}", path.display()))?;
        let mut project: ProjectConfig = toml::from_str(&raw)
            .with_context(|| format!("parse config file {}", path.display()))?;
        normalize_project_config(&mut project, &path)?;
        if projects.insert(project.id.clone(), project).is_some() {
            bail!("duplicate project id in {}", path.display());
        }
    }

    Ok(HarnessConfig { projects })
}

fn normalize_project_config(project: &mut ProjectConfig, config_path: &Path) -> Result<()> {
    if project.id.trim().is_empty() {
        bail!("project id missing in {}", config_path.display());
    }
    if project.devices.is_empty() {
        bail!("project {} has no devices", project.id);
    }

    let config_dir = config_path
        .parent()
        .ok_or_else(|| anyhow!("missing config dir for {}", config_path.display()))?;
    project.hooks.prepare_source = resolve_hook_path(config_dir, &project.hooks.prepare_source);
    project.hooks.start_dependencies = project
        .hooks
        .start_dependencies
        .as_ref()
        .map(|path| resolve_hook_path(config_dir, path));
    project.hooks.start_runtime = resolve_hook_path(config_dir, &project.hooks.start_runtime);
    project.hooks.check_readiness = resolve_hook_path(config_dir, &project.hooks.check_readiness);
    project.hooks.teardown = resolve_hook_path(config_dir, &project.hooks.teardown);
    project.hooks.command = resolve_hook_path(config_dir, &project.hooks.command);
    Ok(())
}

fn resolve_hook_path(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn default_boot_policy() -> String {
    "lazy".to_string()
}

fn default_boot_simulator_sec() -> u64 {
    45
}

fn default_prepare_source_sec() -> u64 {
    180
}

fn default_start_dependencies_sec() -> u64 {
    300
}

fn default_start_runtime_sec() -> u64 {
    240
}

fn default_readiness_sec() -> u64 {
    180
}

fn default_command_sec() -> u64 {
    60
}

fn default_teardown_sec() -> u64 {
    60
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn load_harness_config_resolves_relative_hooks_and_defaults() {
        let temp = tempdir().expect("tempdir");
        let config_dir = temp.path().join("projects");
        fs::create_dir_all(config_dir.join("hooks")).expect("config dir");
        fs::write(
            config_dir.join("ezra.toml"),
            r#"
id = "ezra"
display_name = "Ezra QA"
repo_root = "/tmp/repo"
runtime_root = "/tmp/runtime"

[devices.primary]
type = "ios_sim"
device_id = "SIM-123"
name = "Primary"
runtime_subdir = "sim-primary"

[hooks]
prepare_source = "./hooks/prepare_source.sh"
start_runtime = "./hooks/start_runtime.sh"
check_readiness = "./hooks/check_readiness.sh"
teardown = "./hooks/teardown.sh"
command = "./hooks/command.sh"
"#,
        )
        .expect("write config");

        let config = load_harness_config(&config_dir).expect("load config");
        let project = config.projects.get("ezra").expect("project");

        assert_eq!(project.timeouts.command_sec, 60);
        assert_eq!(project.devices["primary"].boot_policy, "lazy");
        assert_eq!(
            project.hooks.prepare_source,
            config_dir.join("./hooks/prepare_source.sh")
        );
        assert_eq!(
            project.hooks.command,
            config_dir.join("./hooks/command.sh")
        );
    }

    #[test]
    fn load_harness_config_rejects_project_without_devices() {
        let temp = tempdir().expect("tempdir");
        let config_dir = temp.path().join("projects");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(
            config_dir.join("broken.toml"),
            r#"
id = "broken"
display_name = "Broken"
repo_root = "/tmp/repo"
runtime_root = "/tmp/runtime"

[hooks]
prepare_source = "./prepare.sh"
start_runtime = "./run.sh"
check_readiness = "./ready.sh"
teardown = "./teardown.sh"
command = "./command.sh"
"#,
        )
        .expect("write config");

        let error = load_harness_config(&config_dir).expect_err("missing devices should fail");
        assert!(error.to_string().contains("has no devices"));
    }

    #[test]
    fn load_harness_config_allows_missing_directory() {
        let temp = tempdir().expect("tempdir");
        let config = load_harness_config(&temp.path().join("missing")).expect("empty config");
        assert!(config.projects.is_empty());
    }
}
