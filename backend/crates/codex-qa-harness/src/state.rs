use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use tracing::warn;

use crate::{config::ProjectConfig, models::{SlotPhase, SlotRuntimeState, SlotStatus}};

#[derive(Debug, Clone)]
pub struct StateStore {
    root: PathBuf,
}

impl StateStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn ensure_layout(&self) -> Result<()> {
        for relative in ["config", "leases", "logs", "runtimes", "state"] {
            fs::create_dir_all(self.root.join(relative))
                .with_context(|| format!("create state dir {}", self.root.join(relative).display()))?;
        }
        Ok(())
    }

    pub fn runtime_dir(&self, project: &ProjectConfig, runtime_subdir: &str) -> PathBuf {
        project.runtime_root.join(runtime_subdir)
    }

    pub fn logs_dir(&self, project_id: &str, device_key: &str) -> PathBuf {
        self.root.join("logs").join(project_id).join(device_key)
    }

    pub fn slot_state_path(&self, project_id: &str, device_key: &str) -> PathBuf {
        self.root
            .join("state")
            .join(project_id)
            .join(format!("{device_key}.json"))
    }

    pub fn load_slot_states(
        &self,
        projects: &BTreeMap<String, ProjectConfig>,
    ) -> Result<BTreeMap<(String, String), SlotRuntimeState>> {
        let mut states = BTreeMap::new();
        for (project_id, project) in projects {
            for (device_key, device) in &project.devices {
                let runtime_dir = self.runtime_dir(project, &device.runtime_subdir);
                let path = self.slot_state_path(project_id, device_key);
                let state = if path.exists() {
                    match fs::read_to_string(&path)
                        .with_context(|| format!("read slot state {}", path.display()))
                        .and_then(|raw| {
                            serde_json::from_str::<SlotRuntimeState>(&raw)
                                .with_context(|| format!("parse slot state {}", path.display()))
                        }) {
                        Ok(existing) => existing,
                        Err(error) => {
                            warn!("failed to load {}; using default: {error:#}", path.display());
                            default_slot_state(project_id, device_key, &device.device_id, runtime_dir)
                        }
                    }
                } else {
                    default_slot_state(project_id, device_key, &device.device_id, runtime_dir)
                };
                states.insert((project_id.clone(), device_key.clone()), state);
            }
        }
        Ok(states)
    }

    pub fn persist_slot_state(&self, state: &SlotRuntimeState) -> Result<()> {
        let path = self.slot_state_path(&state.project_id, &state.device_key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create slot state dir {}", parent.display()))?;
        }
        fs::write(&path, serde_json::to_vec_pretty(state)?)
            .with_context(|| format!("write slot state {}", path.display()))?;
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn default_slot_state(
    project_id: &str,
    device_key: &str,
    device_id: &str,
    runtime_dir: PathBuf,
) -> SlotRuntimeState {
    SlotRuntimeState {
        project_id: project_id.to_string(),
        device_key: device_key.to_string(),
        device_id: device_id.to_string(),
        runtime_dir,
        status: SlotStatus::Idle,
        phase: SlotPhase::None,
        lease: None,
        artifacts: BTreeMap::new(),
        processes: Vec::new(),
        last_error: None,
        last_ready_at: None,
        updated_at: iso_timestamp(),
    }
}

pub fn iso_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "0".to_string(),
    }
}

