use rinf::{DartSignal, RustSignal};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, DartSignal)]
pub struct InitializeWorkbenchSignal {
    pub host: String,
    pub port: u32,
}

#[derive(Deserialize, DartSignal)]
pub struct ReloadWorkbenchSignal;

#[derive(Deserialize, DartSignal)]
pub struct SelectThreadSignal {
    pub thread_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct FetchThreadHistorySignal;

#[derive(Deserialize, DartSignal)]
pub struct ThreadCompactSignal;

#[derive(Deserialize, DartSignal)]
pub struct TerminateCommandExecutionSignal {
    pub process_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct CreateProjectSignal {
    pub name: String,
    pub root_path: String,
    pub default_cwd: String,
}

#[derive(Deserialize, DartSignal)]
pub struct SelectProjectSignal {
    pub project_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct DeleteProjectSignal {
    pub project_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct UpdateProjectSignal {
    pub project_id: String,
    pub name: String,
    pub default_cwd: String,
    pub auto_route_replies: bool,
    pub route_approval_requests: bool,
    pub preferred_model_provider: String,
    pub orchestrator_model_id: String,
    pub orchestrator_reasoning_effort: String,
    pub worker_model_id: String,
    pub worker_reasoning_effort: String,
    pub qa_model_id: String,
    pub qa_reasoning_effort: String,
    pub designer_model_id: String,
    pub designer_reasoning_effort: String,
    pub orchestrator_developer_instructions: String,
    pub worker_developer_instructions: String,
    pub qa_developer_instructions: String,
    pub designer_developer_instructions: String,
    pub operator_developer_instructions: String,
    pub hidden_developer_instructions: String,
}

#[derive(Deserialize, DartSignal)]
pub struct CreateThreadSignal {
    pub project_id: String,
    pub title: String,
    pub initial_prompt: String,
    pub role: String,
    pub approval_policy: String,
    pub sandbox_mode: String,
    pub network_access_mode: String,
    pub model_id: String,
    pub reasoning_effort: String,
}

#[derive(Deserialize, DartSignal)]
pub struct SpawnAgentSignal {
    pub name: String,
    pub role: String,
    pub prompt: String,
}

#[derive(Deserialize, DartSignal)]
pub struct SetProjectOrchestratorSignal {
    pub project_id: String,
    pub project_path: String,
    pub thread_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct CreateThreadGroupSignal {
    pub title: String,
}

#[derive(Deserialize, DartSignal)]
pub struct RenameThreadGroupSignal {
    pub group_id: String,
    pub title: String,
}

#[derive(Deserialize, DartSignal)]
pub struct DeleteThreadGroupSignal {
    pub group_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct ArchiveThreadGroupSignal {
    pub group_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct MoveSelectedThreadToGroupSignal {
    pub group_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct UpdateWorkerMetadataSignal {
    pub issue_number: String,
    pub pull_request_number: String,
    pub blocked_reason: String,
    pub unblock_when: String,
    pub clear_blocked: bool,
}

#[derive(Deserialize, DartSignal)]
pub struct SendThreadMessageSignal {
    pub text: String,
    pub local_image_paths: Vec<String>,
}

#[derive(Deserialize, DartSignal)]
pub struct InterruptThreadSignal;

#[derive(Deserialize, DartSignal)]
pub struct DecideApprovalSignal {
    pub approval_id: String,
    pub decision: String,
    pub message: String,
}

#[derive(Deserialize, DartSignal)]
pub struct UpdateThreadSettingsSignal {
    pub role: String,
    pub approval_policy: String,
    pub sandbox_mode: String,
    pub network_access_mode: String,
    pub model_id: String,
    pub reasoning_effort: String,
    pub service_tier: String,
}

#[derive(Deserialize, DartSignal)]
pub struct SetThreadRunningStateSignal {
    pub running: bool,
}

#[derive(Deserialize, DartSignal)]
pub struct RenameThreadSignal {
    pub name: String,
}

#[derive(Deserialize, DartSignal)]
pub struct ArchiveThreadSignal;

#[derive(Deserialize, DartSignal)]
pub struct WarmHandoffSignal {
    pub prompt: String,
}

#[derive(Serialize, RustSignal)]
pub struct WorkbenchStateSignal {
    pub view_json: String,
    pub is_loading: bool,
    pub error_message: String,
}

#[derive(Serialize, RustSignal)]
pub struct ThreadHistoryStateSignal {
    pub entries_json: String,
    pub is_loading: bool,
    pub error_message: String,
}

#[derive(Serialize, RustSignal)]
pub struct HookToastSignal {
    pub message: String,
    pub detail: String,
    pub copy_text: String,
    pub duration_ms: u32,
}
