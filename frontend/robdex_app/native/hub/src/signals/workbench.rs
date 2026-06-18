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
pub struct LoadThreadStatsSignal {
    pub request_id: String,
    pub thread_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct LoadPeriodStatsSignal {
    pub request_id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub label: String,
    pub quota_reset_at_ms: u64,
    pub quota_remaining_percent: f64,
    pub has_quota: bool,
}

#[derive(Deserialize, DartSignal)]
pub struct LoadProjectHookLogsSignal {
    pub request_id: String,
    pub project_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct ClearProjectHookLogsSignal {
    pub request_id: String,
    pub project_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct LoadRequirementComposablesSignal {
    pub request_id: String,
    pub sender_thread_id: String,
    pub recipient_thread_id: String,
    pub project_path: String,
}

#[derive(Deserialize, DartSignal)]
pub struct SetThreadRequirementsSignal {
    pub request_id: String,
    pub sender_thread_id: String,
    pub recipient_thread_id: String,
    pub project_path: String,
    pub requirement_set_json: String,
}

#[derive(Deserialize, DartSignal)]
pub struct UploadImageBytesSignal {
    pub request_id: String,
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}


#[derive(Deserialize, DartSignal)]
pub struct LoadImageBytesSignal {
    pub request_id: String,
    pub path: String,
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
pub struct UpdateGlobalSettingsSignal {
    pub approval_policy: String,
    pub sandbox_mode: String,
    pub network_access_mode: String,
}

#[derive(Deserialize, DartSignal)]
pub struct UpdateProjectSignal {
    pub project_id: String,
    pub name: String,
    pub default_cwd: String,
    pub auto_route_replies: bool,
    pub route_approval_requests: bool,
    pub preferred_model_provider: String,
    pub default_model_id: String,
    pub default_reasoning_effort: String,
    pub default_sandbox_mode: String,
    pub default_approval_policy: String,
    pub default_network_access_mode: String,
    pub role_runtime_defaults_json: String,
    pub orchestrator_model_id: String,
    pub orchestrator_reasoning_effort: String,
    pub worker_model_id: String,
    pub worker_reasoning_effort: String,
    pub qa_model_id: String,
    pub qa_reasoning_effort: String,
    pub designer_model_id: String,
    pub designer_reasoning_effort: String,
    pub planner_model_id: String,
    pub planner_reasoning_effort: String,
    pub requirements_reviewer_model_id: String,
    pub requirements_reviewer_reasoning_effort: String,
    pub orchestrator_developer_instructions: String,
    pub worker_developer_instructions: String,
    pub qa_developer_instructions: String,
    pub designer_developer_instructions: String,
    pub operator_developer_instructions: String,
    pub hidden_developer_instructions: String,
    pub permanent_requirement_composables: Vec<String>,
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
    pub requirement_set_json: String,
}

#[derive(Deserialize, DartSignal)]
pub struct SpawnAgentSignal {
    pub name: String,
    pub role: String,
    pub prompt: String,
    pub requirement_set_json: String,
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
    pub requirement_set_json: String,
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

#[derive(Deserialize, DartSignal)]
pub struct TerminalOpenSignal {
    pub request_id: String,
    pub host: String,
    pub username: String,
    pub cols: u32,
    pub rows: u32,
}

#[derive(Deserialize, DartSignal)]
pub struct TerminalInputSignal {
    pub session_id: String,
    pub data: String,
}

#[derive(Deserialize, DartSignal)]
pub struct TerminalResizeSignal {
    pub session_id: String,
    pub cols: u32,
    pub rows: u32,
}

#[derive(Deserialize, DartSignal)]
pub struct TerminalCloseSignal {
    pub session_id: String,
}

#[derive(Deserialize, DartSignal)]
pub struct TerminalCloseAllSignal;

#[derive(Serialize, RustSignal)]
pub struct WorkbenchStateSignal {
    pub view_json: String,
    pub is_loading: bool,
    pub error_message: String,
}

#[derive(Serialize, RustSignal, Clone, Debug)]
pub struct WorkbenchSelectedChatDeltaSignal {
    pub thread_id: String,
    pub message_id: String,
    pub appended_text: String,
    pub replacement_text: String,
    pub delivery_state: String,
    pub is_final: bool,
    pub sequence: u64,
    pub metadata_json: String,
    pub selected_entry_count: u32,
    pub coalesced_stream_update_count: u32,
    pub dropped_intermediate_stream_update_count: u32,
}

#[derive(Serialize, RustSignal, Clone, Debug)]
pub struct WorkbenchDiagnosticsSignal {
    pub websocket_event_counts_json: String,
    pub websocket_payload_bytes_json: String,
    pub native_signal_count: u64,
    pub serialized_payload_bytes: u64,
    pub dart_full_snapshot_decode_micros: u64,
    pub dart_selected_chat_delta_apply_count: u64,
    pub coalesced_stream_update_count: u64,
    pub dropped_intermediate_stream_update_count: u64,
    pub selected_timeline_entry_count: u32,
}

#[derive(Serialize, RustSignal)]
pub struct ThreadHistoryStateSignal {
    pub entries_json: String,
    pub is_loading: bool,
    pub error_message: String,
}

#[derive(Serialize, RustSignal)]
pub struct BridgeTaskResultSignal {
    pub request_id: String,
    pub task: String,
    pub payload_json: String,
    pub error_message: String,
}

#[derive(Serialize, RustSignal)]
pub struct HookToastSignal {
    pub message: String,
    pub detail: String,
    pub copy_text: String,
    pub duration_ms: u32,
}

#[derive(Serialize, RustSignal)]
pub struct TerminalEventSignal {
    pub request_id: String,
    pub session_id: String,
    pub kind: String,
    pub data: String,
    pub host: String,
    pub username: String,
}
