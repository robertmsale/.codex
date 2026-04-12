use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    IosSim,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SlotStatus {
    Idle,
    BootingSimulator,
    Preparing,
    StartingDependencies,
    StartingRuntime,
    Ready,
    Busy,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SlotPhase {
    None,
    BootSimulator,
    PrepareSource,
    StartDependencies,
    StartRuntime,
    CheckReadiness,
    ExecuteCommand,
    Teardown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandName {
    Hierarchy,
    Tap,
    Swipe,
    TypeText,
    PressKey,
    Screenshot,
    Logs,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseRecord {
    pub owner: String,
    pub reason: String,
    pub acquired_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessRecord {
    pub purpose: String,
    pub pid: Option<u32>,
    pub started_at: Option<String>,
    pub expected_cleanup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookProcessRecord {
    pub purpose: String,
    pub pid: Option<u32>,
    pub expected_cleanup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookResult {
    #[serde(default = "default_true")]
    pub ok: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub artifacts: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub processes: Vec<HookProcessRecord>,
    #[serde(default)]
    pub error: Option<ErrorRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotRuntimeState {
    pub project_id: String,
    pub device_key: String,
    pub device_id: String,
    pub runtime_dir: PathBuf,
    pub status: SlotStatus,
    pub phase: SlotPhase,
    pub lease: Option<LeaseRecord>,
    pub artifacts: BTreeMap<String, String>,
    pub processes: Vec<ProcessRecord>,
    pub last_error: Option<ErrorRecord>,
    pub last_ready_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub id: String,
    pub display_name: String,
    pub repo_root: PathBuf,
    pub runtime_root: PathBuf,
    pub device_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceSummary {
    pub project_id: String,
    pub device_key: String,
    pub device_id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub runtime_dir: PathBuf,
    pub boot_policy: String,
    pub state: SlotRuntimeState,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LeaseRequest {
    pub owner: String,
    pub reason: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartRequest {
    pub lease_owner: String,
    #[serde(default)]
    pub startup: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommandRequest {
    pub lease_owner: String,
    pub command: CommandName,
    #[serde(default)]
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub ok: bool,
    pub error: String,
}

impl ApiError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: message.into(),
        }
    }
}

fn default_true() -> bool {
    true
}
