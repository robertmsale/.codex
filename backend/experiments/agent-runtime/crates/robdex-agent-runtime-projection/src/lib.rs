use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub type Watermark = i64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorPacket {
    pub error: ApiErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Value,
}

impl ApiErrorPacket {
    pub fn new(code: impl Into<String>, message: impl Into<String>, details: Value) -> Self {
        Self {
            error: ApiErrorBody {
                code: code.into(),
                message: message.into(),
                details,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatusProjection {
    pub status: String,
    pub database: String,
    pub message: Option<String>,
}

impl Default for ServerStatusProjection {
    fn default() -> Self {
        Self {
            status: "unknown".to_string(),
            database: "unknown".to_string(),
            message: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionListItem {
    pub id: String,
    pub status: String,
    pub role_id: Option<String>,
    pub role_version: Option<String>,
    pub project_key: Option<String>,
    pub title: Option<String>,
    pub name: Option<String>,
    pub workdir: String,
    pub tracked: bool,
    pub archived_at: Option<String>,
    pub closed_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub project_key: String,
    pub display_name: String,
    pub default_workdir: String,
    pub default_worktree_root: String,
    pub default_role_id: Option<String>,
    pub default_model: String,
    pub archived: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SelectedSessionDetail {
    pub id: String,
    pub role_id: Option<String>,
    pub role_version: Option<String>,
    pub project_key: Option<String>,
    pub workdir: String,
    pub worktree_root: Option<String>,
    pub title: Option<String>,
    pub name: Option<String>,
    pub status: String,
    pub pending_approval_count: u64,
    pub managed_process_count: u64,
    pub active_turn_id: Option<String>,
    pub queued_submitted_input_count: u64,
    pub applied_steering_count: u64,
    pub submit_disposition: Option<String>,
    pub submit_status: Option<String>,
    pub terminal_submission_rejection: Option<Value>,
    pub metadata: Value,
    #[serde(default)]
    pub requirements_review: Option<RequirementsReviewSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RequirementsReviewSummary {
    pub active: bool,
    pub active_set_id: Option<String>,
    pub total: usize,
    pub unresolved: usize,
    pub passed: usize,
    pub blocked: usize,
    pub waived: usize,
    pub reviewer_session_id: Option<String>,
    pub review_status: Option<String>,
    pub latest_claim_packet_id: Option<String>,
    pub latest_verdict_packet_id: Option<String>,
    #[serde(default)]
    pub packets: Vec<RequirementsPacketSummary>,
    #[serde(default)]
    pub progress: Vec<Value>,
    #[serde(default)]
    pub owner_action: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RequirementsPacketSummary {
    pub id: String,
    pub requirement_set_id: String,
    pub packet_kind: String,
    pub status: String,
    pub reviewer_session_id: Option<String>,
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineItem {
    pub id: String,
    pub sequence: Watermark,
    pub session_id: Option<String>,
    pub turn_id: Option<String>,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub event_type: String,
    pub status: Option<String>,
    pub summary: Option<String>,
    pub payload: Value,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeChatEntry {
    pub id: String,
    pub author: String,
    pub display_label: String,
    pub timestamp: Option<String>,
    pub body: String,
    pub subtitle: String,
    pub kind: String,
    pub status: String,
    pub process_id: Option<String>,
    pub command: String,
    pub output: String,
    pub delivery_state: String,
    pub is_streaming: bool,
    pub is_tool: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PendingApprovalSummary {
    pub id: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub action_name: String,
    pub required_approver_kind: String,
    pub status: String,
    pub can_decide: bool,
    pub can_resume: bool,
    pub input_context: Value,
    pub created_at: Option<String>,
    #[serde(default)]
    pub decision_at: Option<String>,
    #[serde(default)]
    pub decision_reason: Option<String>,
    #[serde(default)]
    pub resumable_action_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleSummary {
    pub id: String,
    pub display_name: String,
    pub current_version_id: Option<String>,
    pub status: String,
    pub model: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    pub archived_at: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub instruction_text: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub policy: BTreeMap<String, String>,
    #[serde(default)]
    pub routing: Value,
    #[serde(default)]
    pub visibility: Value,
    #[serde(default)]
    pub lifecycle_authority: Value,
    #[serde(default)]
    pub versions: Vec<RoleVersionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleVersionSummary {
    pub version_id: String,
    pub version: String,
    pub status: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleEditorModelDefaults {
    pub model: String,
    pub reasoning_effort: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleEditorRoutingMetadata {
    pub mode: String,
    pub default_recipient: Option<String>,
    #[serde(default)]
    pub allowed_recipients: Vec<String>,
    #[serde(default)]
    pub reserved_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleEditorVisibilityMetadata {
    pub listed: bool,
    pub owner_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleEditorLifecycleAuthorityMetadata {
    pub can_spawn_agents: bool,
    pub can_archive_agents: bool,
    #[serde(default)]
    pub reserved_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleEditorDraft {
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub model_defaults: RoleEditorModelDefaults,
    pub instruction_text: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub policy: BTreeMap<String, String>,
    pub routing: RoleEditorRoutingMetadata,
    pub visibility: RoleEditorVisibilityMetadata,
    pub lifecycle_authority: RoleEditorLifecycleAuthorityMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleEditorValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub role_id: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleEditorOptions {
    pub policy_decisions: Vec<String>,
    pub routing_modes: Vec<String>,
    pub default_recipients: Vec<String>,
    pub known_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandRegistrySummary {
    pub id: String,
    pub action_id: String,
    pub scope_type: String,
    pub project_key: Option<String>,
    pub enabled: bool,
    pub current_version_id: Option<String>,
    pub command_version: Option<i64>,
    pub binary_name: Option<String>,
    pub starlark_object: Option<String>,
    pub starlark_method: Option<String>,
    #[serde(default)]
    pub argv_template: Vec<String>,
    pub default_cwd: Option<String>,
    pub cwd_policy: Option<String>,
    pub env_policy: Option<String>,
    pub stdin_policy: Option<String>,
    pub sync_allowed: Option<bool>,
    pub async_allowed: Option<bool>,
    pub max_runtime_ms: Option<i64>,
    pub end_of_turn_behavior: Option<String>,
    pub end_of_session_behavior: Option<String>,
    pub mutation_class: Option<String>,
    pub model_description: Option<String>,
    pub allow_cwd_arg: Option<bool>,
    pub allow_args_arg: Option<bool>,
    #[serde(default)]
    pub forbidden_args: Vec<String>,
    pub execution_policy: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowMemorySummary {
    pub id: String,
    pub session_id: String,
    #[serde(default)]
    pub source_script_run_id: Option<String>,
    pub scope_type: String,
    pub project_key: Option<String>,
    pub title: String,
    pub reason: String,
    #[serde(default)]
    pub summary: String,
    pub helpful_score: f64,
    pub promoted_at: Option<String>,
    #[serde(default)]
    pub source_preview: String,
    #[serde(default)]
    pub source_starlark: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub dimensions: Option<i32>,
    #[serde(default)]
    pub storage_type: Option<String>,
    #[serde(default)]
    pub source_hash: Option<String>,
    #[serde(default)]
    pub command_fingerprint: Option<String>,
    #[serde(default)]
    pub recent_events: Vec<WorkflowMemoryEventSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowMemoryEventSummary {
    pub id: String,
    pub event_type: String,
    pub created_at: Option<String>,
    #[serde(default)]
    pub payload_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResyncRequiredState {
    pub required: bool,
    pub reason: String,
    pub expected_watermark: Option<Watermark>,
    pub received_watermark: Option<Watermark>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProjection {
    pub watermark: Watermark,
    pub server_status: ServerStatusProjection,
    #[serde(default)]
    pub projects: Vec<ProjectSummary>,
    pub sessions: Vec<SessionListItem>,
    pub selected_session: Option<SelectedSessionDetail>,
    pub timeline: Vec<TimelineItem>,
    #[serde(default)]
    pub selected_chat_entries: Vec<AgentRuntimeChatEntry>,
    pub pending_approvals: Vec<PendingApprovalSummary>,
    pub roles: Vec<RoleSummary>,
    pub command_registry: Vec<CommandRegistrySummary>,
    #[serde(default)]
    pub command_registry_requests: Vec<CommandRegistryRequestSummary>,
    pub workflow_memories: Vec<WorkflowMemorySummary>,
    #[serde(default)]
    pub statistics: RuntimeStatistics,
    pub resync_required: Option<ResyncRequiredState>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatistics {
    #[serde(default)]
    pub sessions: u64,
    #[serde(default)]
    pub open_sessions: u64,
    #[serde(default)]
    pub closed_sessions: u64,
    #[serde(default)]
    pub archived_sessions: u64,
    pub turns: u64,
    #[serde(default)]
    pub running_turns: u64,
    #[serde(default)]
    pub failed_turns: u64,
    pub model_events: u64,
    pub tool_calls: u64,
    pub script_runs: u64,
    pub host_api_calls: u64,
    pub command_runs: u64,
    pub managed_processes: u64,
    pub output_artifacts: u64,
    pub compaction_checkpoints: u64,
    pub approval_requests: u64,
    pub command_registry_requests: u64,
    #[serde(default)]
    pub workflow_memories: u64,
    pub failed_rows: u64,
    pub running_rows: u64,
    pub lost_rows: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeChatTransportDiagnostics {
    pub full_snapshot_count: u64,
    pub delta_count: u64,
    pub total_payload_bytes: u64,
    pub max_payload_bytes: u64,
    pub selected_chat_entry_count: usize,
    pub coalesced_payload_count: u64,
    pub dropped_intermediate_payload_count: u64,
    pub unrelated_modal_rebuild_count: u64,
    pub unrelated_rail_rebuild_count: u64,
}

impl AgentRuntimeChatTransportDiagnostics {
    pub fn record_snapshot(&mut self, payload_bytes: usize, selected_chat_entry_count: usize) {
        self.full_snapshot_count += 1;
        self.record_payload(payload_bytes);
        self.selected_chat_entry_count = selected_chat_entry_count.min(50);
    }

    pub fn record_delta(&mut self, payload_bytes: usize, selected_chat_entry_count: usize, coalesced: bool) {
        self.delta_count += 1;
        if coalesced {
            self.coalesced_payload_count += 1;
        }
        self.record_payload(payload_bytes);
        self.selected_chat_entry_count = selected_chat_entry_count.min(50);
    }

    pub fn record_dropped_intermediate(&mut self) {
        self.dropped_intermediate_payload_count += 1;
    }

    pub fn average_payload_bytes(&self) -> u64 {
        let count = self.full_snapshot_count + self.delta_count;
        if count == 0 { 0 } else { self.total_payload_bytes / count }
    }

    fn record_payload(&mut self, payload_bytes: usize) {
        let bytes = payload_bytes as u64;
        self.total_payload_bytes += bytes;
        self.max_payload_bytes = self.max_payload_bytes.max(bytes);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GuiConnectionState {
    Disconnected,
    Connecting,
    Hydrating,
    Streaming,
    Reconnecting,
    ShuttingDown,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GuiSelectedView {
    Sessions,
    SessionDetail,
    Approvals,
    Roles,
    CommandRegistry,
    WorkflowMemory,
    Operations,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GuiOperationState {
    pub operation_id: String,
    pub operation: GuiOperationName,
    pub status: GuiOperationStatus,
    pub target_id: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<ApiErrorPacket>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GuiControllerState {
    pub connection_state: GuiConnectionState,
    pub selected_session_id: Option<String>,
    pub selected_project_id: Option<String>,
    pub selected_workflow_memory_id: Option<String>,
    pub selected_view: GuiSelectedView,
    pub active_operations: Vec<GuiOperationState>,
    pub transient_errors: Vec<ApiErrorPacket>,
    pub resync_required: Option<ResyncRequiredState>,
    pub pending_rehydrate: bool,
    pub pending_reconnect: bool,
    pub draft_inputs: BTreeMap<String, String>,
}

impl Default for GuiControllerState {
    fn default() -> Self {
        Self {
            connection_state: GuiConnectionState::Disconnected,
            selected_session_id: None,
            selected_project_id: None,
            selected_workflow_memory_id: None,
            selected_view: GuiSelectedView::Sessions,
            active_operations: Vec::new(),
            transient_errors: Vec::new(),
            resync_required: None,
            pending_rehydrate: false,
            pending_reconnect: false,
            draft_inputs: BTreeMap::new(),
        }
    }
}

impl GuiControllerState {
    pub fn record_resync_required(&mut self, reason: impl Into<String>, expected_watermark: Option<Watermark>, received_watermark: Option<Watermark>) {
        self.resync_required = Some(ResyncRequiredState {
            required: true,
            reason: reason.into(),
            expected_watermark,
            received_watermark,
        });
        self.pending_rehydrate = true;
        self.pending_reconnect = true;
        self.connection_state = GuiConnectionState::Reconnecting;
    }

    pub fn select_session(&mut self, session_id: Option<String>) -> GuiOperationExpectation {
        self.selected_session_id = session_id;
        self.selected_workflow_memory_id = None;
        self.pending_rehydrate = true;
        self.pending_reconnect = true;
        GuiOperationExpectation::RehydrateAndReconnect
    }

    pub fn select_workflow_memory(&mut self, memory_id: Option<String>) -> GuiOperationExpectation {
        self.selected_workflow_memory_id = memory_id;
        GuiOperationExpectation::UpdateLocalState
    }

    pub fn select_project(&mut self, project_id: Option<String>) -> GuiOperationExpectation {
        self.selected_project_id = project_id;
        self.selected_session_id = None;
        self.selected_workflow_memory_id = None;
        self.pending_rehydrate = true;
        GuiOperationExpectation::Rehydrate
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GuiOperationStatus {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GuiOperationExpectation {
    WaitForDelta,
    Rehydrate,
    RehydrateAndReconnect,
    UpdateLocalState,
    DirectResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GuiOperationName {
    Connect,
    Hydrate,
    Rehydrate,
    Disconnect,
    SelectSession,
    SelectWorkflowMemory,
    CreateSession,
    ListProjects,
    CreateProject,
    UpdateProject,
    ArchiveProject,
    UnarchiveProject,
    UpdateRuntimeSettings,
    UpdateSessionSettings,
    SendMessage,
    TerminateProcess,
    InputProcess,
    FlushProcess,
    CompactSession,
    GrantGodMode,
    RevokeGodMode,
    CloseSession,
    ArchiveSession,
    ForkSession,
    DecideApproval,
    ResumeApproval,
    ListCommandRegistry,
    ShowCommand,
    ListCommandRegistryRequests,
    ShowCommandRegistryRequest,
    PreviewCommandRegistryRequest,
    DecideCommandRegistryRequest,
    ApplyCommandRegistryRequest,
    WorkflowMemoryFeedback,
    RoleEditorOptions,
    ValidateRoleDraft,
    CreateRoleFromDraft,
    UpdateRoleFromDraft,
    ShowRoleDetail,
    ListRoleVersions,
    ShowRoleVersion,
    ExportRole,
    ActivateRoleVersion,
    ArchiveRole,
    UnarchiveRole,
    SetRequirements,
    ClearRequirements,
    ShowRequirementsStatus,
    ListRequirementsPackets,
    SubmitRequirementsReviewerInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", content = "request", rename_all = "camelCase")]
pub enum GuiOperationRequest {
    Connect { base_url: String, selected_session_id: Option<String> },
    Hydrate { selected_session_id: Option<String> },
    Rehydrate { selected_session_id: Option<String> },
    Disconnect,
    SelectSession { session_id: Option<String> },
    SelectWorkflowMemory { memory_id: Option<String> },
    CreateSession { role: String, project: Option<String>, model: Option<String>, workdir: Option<String>, worktree_root: Option<String>, title: Option<String>, name: Option<String> },
    ListProjects,
    CreateProject { project_key: String, display_name: String, default_workdir: String, default_worktree_root: String, default_role_id: Option<String>, default_model: String },
    UpdateProject { project_key: String, display_name: String, default_workdir: String, default_worktree_root: String, default_role_id: Option<String>, default_model: String },
    ArchiveProject { project_key: String },
    UnarchiveProject { project_key: String },
    UpdateRuntimeSettings { base_url: String, selected_project_id: Option<String> },
    UpdateSessionSettings { session_id: String, project: String, role: String, model: String, workdir: String, worktree_root: String, title: String, name: String, tracked: bool },
    SendMessage { session_id: String, message: String },
    TerminateProcess { session_id: String, handle: String },
    InputProcess { session_id: String, handle: String, text: String },
    FlushProcess { session_id: String, handle: String },
    CompactSession { session_id: String, through_turn: Option<String> },
    GrantGodMode { session_id: String, reason: String },
    RevokeGodMode { session_id: String, reason: String },
    CloseSession { session_id: String, reason: Option<String> },
    ArchiveSession { session_id: String },
    ForkSession { session_id: String, at_turn: String },
    DecideApproval { approval_id: String, decision: String, reason: String },
    ResumeApproval { approval_id: String },
    ListCommandRegistry { session_id: Option<String>, project_key: Option<String> },
    ShowCommand { action_id: String, session_id: Option<String>, project_key: Option<String> },
    ListCommandRegistryRequests,
    ShowCommandRegistryRequest { request_id: String },
    PreviewCommandRegistryRequest { request_id: String, decision: CommandRegistryDecisionInput },
    DecideCommandRegistryRequest { request_id: String, decision: CommandRegistryDecisionInput },
    ApplyCommandRegistryRequest { request_id: String, session_id: String },
    WorkflowMemoryFeedback { memory_id: String, session_id: String, feedback: String, payload: Value },
    RoleEditorOptions,
    ValidateRoleDraft { draft: RoleEditorDraft },
    CreateRoleFromDraft { draft: RoleEditorDraft },
    UpdateRoleFromDraft { role_id: String, draft: RoleEditorDraft },
    ShowRoleDetail { role_id: String },
    ListRoleVersions { role_id: String },
    ShowRoleVersion { version_id: String },
    ExportRole { role_id: String },
    ActivateRoleVersion { role_id: String, version_id: String },
    ArchiveRole { role_id: String },
    UnarchiveRole { role_id: String },
    SetRequirements { session_id: String, title: Option<String>, requirements: Vec<Value> },
    ClearRequirements { session_id: String },
    ShowRequirementsStatus { session_id: String },
    ListRequirementsPackets { session_id: String },
    SubmitRequirementsReviewerInput { source_session_id: String, message: String },
}

impl GuiOperationRequest {
    pub fn name(&self) -> GuiOperationName {
        match self {
            Self::Connect { .. } => GuiOperationName::Connect,
            Self::Hydrate { .. } => GuiOperationName::Hydrate,
            Self::Rehydrate { .. } => GuiOperationName::Rehydrate,
            Self::Disconnect => GuiOperationName::Disconnect,
            Self::SelectSession { .. } => GuiOperationName::SelectSession,
            Self::SelectWorkflowMemory { .. } => GuiOperationName::SelectWorkflowMemory,
            Self::CreateSession { .. } => GuiOperationName::CreateSession,
            Self::ListProjects => GuiOperationName::ListProjects,
            Self::CreateProject { .. } => GuiOperationName::CreateProject,
            Self::UpdateProject { .. } => GuiOperationName::UpdateProject,
            Self::ArchiveProject { .. } => GuiOperationName::ArchiveProject,
            Self::UnarchiveProject { .. } => GuiOperationName::UnarchiveProject,
            Self::UpdateRuntimeSettings { .. } => GuiOperationName::UpdateRuntimeSettings,
            Self::UpdateSessionSettings { .. } => GuiOperationName::UpdateSessionSettings,
            Self::SendMessage { .. } => GuiOperationName::SendMessage,
            Self::TerminateProcess { .. } => GuiOperationName::TerminateProcess,
            Self::InputProcess { .. } => GuiOperationName::InputProcess,
            Self::FlushProcess { .. } => GuiOperationName::FlushProcess,
            Self::CompactSession { .. } => GuiOperationName::CompactSession,
            Self::GrantGodMode { .. } => GuiOperationName::GrantGodMode,
            Self::RevokeGodMode { .. } => GuiOperationName::RevokeGodMode,
            Self::CloseSession { .. } => GuiOperationName::CloseSession,
            Self::ArchiveSession { .. } => GuiOperationName::ArchiveSession,
            Self::ForkSession { .. } => GuiOperationName::ForkSession,
            Self::DecideApproval { .. } => GuiOperationName::DecideApproval,
            Self::ResumeApproval { .. } => GuiOperationName::ResumeApproval,
            Self::ListCommandRegistry { .. } => GuiOperationName::ListCommandRegistry,
            Self::ShowCommand { .. } => GuiOperationName::ShowCommand,
            Self::ListCommandRegistryRequests => GuiOperationName::ListCommandRegistryRequests,
            Self::ShowCommandRegistryRequest { .. } => GuiOperationName::ShowCommandRegistryRequest,
            Self::PreviewCommandRegistryRequest { .. } => GuiOperationName::PreviewCommandRegistryRequest,
            Self::DecideCommandRegistryRequest { .. } => GuiOperationName::DecideCommandRegistryRequest,
            Self::ApplyCommandRegistryRequest { .. } => GuiOperationName::ApplyCommandRegistryRequest,
            Self::WorkflowMemoryFeedback { .. } => GuiOperationName::WorkflowMemoryFeedback,
            Self::RoleEditorOptions => GuiOperationName::RoleEditorOptions,
            Self::ValidateRoleDraft { .. } => GuiOperationName::ValidateRoleDraft,
            Self::CreateRoleFromDraft { .. } => GuiOperationName::CreateRoleFromDraft,
            Self::UpdateRoleFromDraft { .. } => GuiOperationName::UpdateRoleFromDraft,
            Self::ShowRoleDetail { .. } => GuiOperationName::ShowRoleDetail,
            Self::ListRoleVersions { .. } => GuiOperationName::ListRoleVersions,
            Self::ShowRoleVersion { .. } => GuiOperationName::ShowRoleVersion,
            Self::ExportRole { .. } => GuiOperationName::ExportRole,
            Self::ActivateRoleVersion { .. } => GuiOperationName::ActivateRoleVersion,
            Self::ArchiveRole { .. } => GuiOperationName::ArchiveRole,
            Self::UnarchiveRole { .. } => GuiOperationName::UnarchiveRole,
            Self::SetRequirements { .. } => GuiOperationName::SetRequirements,
            Self::ClearRequirements { .. } => GuiOperationName::ClearRequirements,
            Self::ShowRequirementsStatus { .. } => GuiOperationName::ShowRequirementsStatus,
            Self::ListRequirementsPackets { .. } => GuiOperationName::ListRequirementsPackets,
            Self::SubmitRequirementsReviewerInput { .. } => GuiOperationName::SubmitRequirementsReviewerInput,
        }
    }

    pub fn expected_projection_effect(&self) -> GuiOperationExpectation {
        match self {
            Self::Connect { .. } | Self::Hydrate { .. } | Self::Rehydrate { .. } => GuiOperationExpectation::Rehydrate,
            Self::Disconnect | Self::SelectWorkflowMemory { .. } | Self::UpdateRuntimeSettings { .. } => GuiOperationExpectation::UpdateLocalState,
            Self::SelectSession { .. } => GuiOperationExpectation::RehydrateAndReconnect,
            Self::CreateSession { .. }
            | Self::CreateProject { .. }
            | Self::UpdateProject { .. }
            | Self::ArchiveProject { .. }
            | Self::UnarchiveProject { .. }
            | Self::UpdateSessionSettings { .. }
            | Self::SendMessage { .. }
            | Self::TerminateProcess { .. }
            | Self::InputProcess { .. }
            | Self::FlushProcess { .. }
            | Self::CompactSession { .. }
            | Self::GrantGodMode { .. }
            | Self::RevokeGodMode { .. }
            | Self::CloseSession { .. }
            | Self::ArchiveSession { .. }
            | Self::ForkSession { .. }
            | Self::DecideApproval { .. }
            | Self::ResumeApproval { .. }
            | Self::DecideCommandRegistryRequest { .. }
            | Self::ApplyCommandRegistryRequest { .. }
            | Self::WorkflowMemoryFeedback { .. }
            | Self::CreateRoleFromDraft { .. }
            | Self::UpdateRoleFromDraft { .. }
            | Self::ActivateRoleVersion { .. }
            | Self::ArchiveRole { .. }
            | Self::UnarchiveRole { .. }
            | Self::SetRequirements { .. }
            | Self::ClearRequirements { .. }
            | Self::SubmitRequirementsReviewerInput { .. } => GuiOperationExpectation::WaitForDelta,
            Self::ListProjects
            | Self::ListCommandRegistry { .. }
            | Self::ShowCommand { .. }
            | Self::ListCommandRegistryRequests
            | Self::ShowCommandRegistryRequest { .. }
            | Self::PreviewCommandRegistryRequest { .. }
            | Self::RoleEditorOptions
            | Self::ValidateRoleDraft { .. }
            | Self::ShowRoleDetail { .. }
            | Self::ListRoleVersions { .. }
            | Self::ShowRoleVersion { .. }
            | Self::ExportRole { .. }
            | Self::ShowRequirementsStatus { .. }
            | Self::ListRequirementsPackets { .. } => GuiOperationExpectation::DirectResult,
        }
    }

    pub fn api_mapping(&self) -> GuiOperationApiMapping {
        match self {
            Self::Connect { .. } => local_mapping(self.name(), "RuntimeSyncClient::new + hydrate/connect_after", GuiOperationExpectation::Rehydrate),
            Self::Hydrate { .. } => http_mapping(self.name(), "GET", "/state/snapshot?selectedSessionId=<optional>", "none", "RuntimeProjection", GuiOperationExpectation::Rehydrate),
            Self::Rehydrate { .. } => http_mapping(self.name(), "GET", "/state/snapshot?selectedSessionId=<optional>", "none", "RuntimeProjection", GuiOperationExpectation::Rehydrate),
            Self::Disconnect => local_mapping(self.name(), "close local WebSocket stream and mark disconnected", GuiOperationExpectation::UpdateLocalState),
            Self::SelectSession { .. } => local_mapping(self.name(), "set selectedSessionId, then GET /state/snapshot and reconnect /state/ws with selectedSessionId", GuiOperationExpectation::RehydrateAndReconnect),
            Self::SelectWorkflowMemory { .. } => local_mapping(self.name(), "set selectedWorkflowMemoryId; Workbench view model deterministically falls back when unavailable", GuiOperationExpectation::UpdateLocalState),
            Self::CreateSession { .. } => http_mapping(self.name(), "POST", "/sessions", r#"{"role","project","model","workdir","worktreeRoot","title","name"}"#, r#"{"sessionId"}"#, GuiOperationExpectation::WaitForDelta),
            Self::ListProjects => http_mapping(self.name(), "GET", "/projects", "none", r#"{"projects"}"#, GuiOperationExpectation::DirectResult),
            Self::CreateProject { .. } => http_mapping(self.name(), "POST", "/projects", r#"{"projectKey","displayName","defaultWorkdir","defaultWorktreeRoot","defaultRoleId","defaultModel"}"#, r#"{"project"}"#, GuiOperationExpectation::WaitForDelta),
            Self::UpdateProject { .. } => http_mapping(self.name(), "POST", "/projects/{projectKey}", r#"{"displayName","defaultWorkdir","defaultWorktreeRoot","defaultRoleId","defaultModel"}"#, r#"{"project"}"#, GuiOperationExpectation::WaitForDelta),
            Self::ArchiveProject { .. } => http_mapping(self.name(), "POST", "/projects/{projectKey}/archive", "{}", r#"{"project"}"#, GuiOperationExpectation::WaitForDelta),
            Self::UnarchiveProject { .. } => http_mapping(self.name(), "POST", "/projects/{projectKey}/unarchive", "{}", r#"{"project"}"#, GuiOperationExpectation::WaitForDelta),
            Self::UpdateRuntimeSettings { .. } => local_mapping(self.name(), "validate runtime GUI settings and update Rust-owned controller settings", GuiOperationExpectation::UpdateLocalState),
            Self::UpdateSessionSettings { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/settings", r#"{"project","role","model","workdir","worktreeRoot","title","name","tracked"}"#, r#"{"sessionId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::SendMessage { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/send", r#"{"message"}"#, r#"{"sessionId","turnId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::TerminateProcess { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/processes/{handle}/terminate", "{}", r#"{"handle","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::InputProcess { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/processes/{handle}/input", r#"{"text"}"#, r#"{"handle","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::FlushProcess { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/processes/{handle}/flush", "{}", r#"{"handle","status","artifact"}"#, GuiOperationExpectation::WaitForDelta),
            Self::CompactSession { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/compact", r#"{"throughTurn"?}"#, r#"{"sessionId","checkpointId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::GrantGodMode { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/god-mode/grant", r#"{"reason"}"#, r#"{"sessionId","grantId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::RevokeGodMode { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/god-mode/revoke", r#"{"reason"}"#, r#"{"sessionId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::CloseSession { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/close", r#"{"reason"?}"#, r#"{"sessionId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::ArchiveSession { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/archive", "{}", r#"{"sessionId","tracked"}"#, GuiOperationExpectation::WaitForDelta),
            Self::ForkSession { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/fork", r#"{"atTurn"}"#, r#"{"sessionId","forkedFromSessionId","forkedFromTurnId"}"#, GuiOperationExpectation::WaitForDelta),
            Self::DecideApproval { .. } => http_mapping(self.name(), "POST", "/approvals/{approvalId}/decide", r#"{"decision","reason"}"#, r#"{"approvalId","decision"}"#, GuiOperationExpectation::WaitForDelta),
            Self::ResumeApproval { .. } => http_mapping(self.name(), "POST", "/approvals/{approvalId}/resume", "{}", r#"{"approvalId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::ListCommandRegistry { .. } => http_mapping(self.name(), "GET", "/command-registry?sessionId=<optional>&project=<optional>", "none", "Vec<CommandRegistrySummary/raw command detail>", GuiOperationExpectation::DirectResult),
            Self::ShowCommand { .. } => http_mapping(self.name(), "GET", "/command-registry/{actionId}?sessionId=<optional>&project=<optional>", "none", "command detail JSON", GuiOperationExpectation::DirectResult),
            Self::ListCommandRegistryRequests => http_mapping(self.name(), "GET", "/command-registry/requests", "none", "Vec<CommandRegistryRequestSummary> via from_server_value", GuiOperationExpectation::DirectResult),
            Self::ShowCommandRegistryRequest { .. } => http_mapping(self.name(), "GET", "/command-registry/requests/{requestId}", "none", "command-registry request detail JSON", GuiOperationExpectation::DirectResult),
            Self::PreviewCommandRegistryRequest { .. } => http_mapping(self.name(), "POST", "/command-registry/requests/{requestId}/preview-decision", "RegistryDecisionInput server JSON", "preview packet JSON", GuiOperationExpectation::DirectResult),
            Self::DecideCommandRegistryRequest { .. } => http_mapping(self.name(), "POST", "/command-registry/requests/{requestId}/decide", "RegistryDecisionInput server JSON", r#"{"requestId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::ApplyCommandRegistryRequest { .. } => http_mapping(self.name(), "POST", "/command-registry/requests/{requestId}/apply", r#"{"sessionId"}"#, r#"{"requestId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::WorkflowMemoryFeedback { .. } => http_mapping(self.name(), "POST", "/workflow-memories/{memoryId}/feedback", r#"{"sessionId","feedback","payload"}"#, r#"{"memoryId","feedback","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::RoleEditorOptions => http_mapping(self.name(), "GET", "/roles/editor/options", "none", "RoleEditorOptions", GuiOperationExpectation::DirectResult),
            Self::ValidateRoleDraft { .. } => http_mapping(self.name(), "POST", "/roles/editor/validate", "RoleEditorDraft", "RoleEditorValidationResult", GuiOperationExpectation::DirectResult),
            Self::CreateRoleFromDraft { .. } => http_mapping(self.name(), "POST", "/roles", "RoleEditorDraft", r#"{"roleId","versionId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::UpdateRoleFromDraft { .. } => http_mapping(self.name(), "POST", "/roles/{roleId}/versions", "RoleEditorDraft", r#"{"roleId","versionId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::ShowRoleDetail { .. } => http_mapping(self.name(), "GET", "/roles/{roleId}", "none", "RoleSnapshot", GuiOperationExpectation::DirectResult),
            Self::ListRoleVersions { .. } => http_mapping(self.name(), "GET", "/roles/{roleId}/versions", "none", "Vec<RoleVersion>", GuiOperationExpectation::DirectResult),
            Self::ShowRoleVersion { .. } => http_mapping(self.name(), "GET", "/roles/versions/{versionId}", "none", "RoleSnapshot", GuiOperationExpectation::DirectResult),
            Self::ExportRole { .. } => http_mapping(self.name(), "GET", "/roles/{roleId}/export", "none", "Role export manifest", GuiOperationExpectation::DirectResult),
            Self::ActivateRoleVersion { .. } => http_mapping(self.name(), "POST", "/roles/{roleId}/activate", r#"{"versionId"}"#, r#"{"roleId","versionId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::ArchiveRole { .. } => http_mapping(self.name(), "POST", "/roles/{roleId}/archive", "{}", r#"{"roleId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::UnarchiveRole { .. } => http_mapping(self.name(), "POST", "/roles/{roleId}/unarchive", "{}", r#"{"roleId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::SetRequirements { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/requirements", r#"{"title"?,"requirements":[RequirementInput]}"#, r#"{"requirementSetId"}"#, GuiOperationExpectation::WaitForDelta),
            Self::ClearRequirements { .. } => http_mapping(self.name(), "POST", "/sessions/{sessionId}/requirements/clear", "{}", r#"{"sessionId","status"}"#, GuiOperationExpectation::WaitForDelta),
            Self::ShowRequirementsStatus { .. } => http_mapping(self.name(), "GET", "/sessions/{sessionId}/requirements", "none", "RequirementStatus", GuiOperationExpectation::DirectResult),
            Self::ListRequirementsPackets { .. } => http_mapping(self.name(), "GET", "/sessions/{sessionId}/requirements/packets", "none", "Vec<RequirementPacket>", GuiOperationExpectation::DirectResult),
            Self::SubmitRequirementsReviewerInput { .. } => http_mapping(self.name(), "POST", "/sessions/{sourceSessionId}/requirements/reviewer/send", r#"{"message"}"#, r#"{"sessionId","submittedInputId","disposition","status"}"#, GuiOperationExpectation::WaitForDelta),
        }
    }

    pub fn to_server_request_json(&self) -> Option<Value> {
        match self {
            Self::Connect { .. }
            | Self::Hydrate { .. }
            | Self::Rehydrate { .. }
            | Self::Disconnect
            | Self::SelectSession { .. }
            | Self::SelectWorkflowMemory { .. }
            | Self::UpdateRuntimeSettings { .. }
            | Self::ListProjects
            | Self::ListCommandRegistry { .. }
            | Self::ShowCommand { .. }
            | Self::ListCommandRegistryRequests
            | Self::ShowCommandRegistryRequest { .. }
            | Self::RoleEditorOptions
            | Self::ShowRoleDetail { .. }
            | Self::ListRoleVersions { .. }
            | Self::ShowRoleVersion { .. }
            | Self::ExportRole { .. }
            | Self::ShowRequirementsStatus { .. }
            | Self::ListRequirementsPackets { .. } => None,
            Self::CreateSession { role, project, model, workdir, worktree_root, title, name } => Some(json!({
                "role": role,
                "project": project,
                "model": model,
                "workdir": workdir,
                "worktreeRoot": worktree_root,
                "title": title,
                "name": name,
            })),
            Self::CreateProject { project_key, display_name, default_workdir, default_worktree_root, default_role_id, default_model } => Some(json!({
                "projectKey": project_key,
                "displayName": display_name,
                "defaultWorkdir": default_workdir,
                "defaultWorktreeRoot": default_worktree_root,
                "defaultRoleId": default_role_id,
                "defaultModel": default_model,
            })),
            Self::UpdateProject { display_name, default_workdir, default_worktree_root, default_role_id, default_model, .. } => Some(json!({
                "displayName": display_name,
                "defaultWorkdir": default_workdir,
                "defaultWorktreeRoot": default_worktree_root,
                "defaultRoleId": default_role_id,
                "defaultModel": default_model,
            })),
            Self::ArchiveProject { .. } | Self::UnarchiveProject { .. } => Some(json!({})),
            Self::UpdateSessionSettings { project, role, model, workdir, worktree_root, title, name, tracked, .. } => Some(json!({
                "project": project,
                "role": role,
                "model": model,
                "workdir": workdir,
                "worktreeRoot": worktree_root,
                "title": title,
                "name": name,
                "tracked": tracked,
            })),
            Self::SendMessage { message, .. } => Some(json!({"message": message})),
            Self::TerminateProcess { .. } | Self::FlushProcess { .. } => Some(json!({})),
            Self::CompactSession { through_turn, .. } => Some(json!({"throughTurn": through_turn})),
            Self::GrantGodMode { reason, .. } | Self::RevokeGodMode { reason, .. } => Some(json!({"reason": reason})),
            Self::InputProcess { text, .. } => Some(json!({"text": text})),
            Self::CloseSession { reason, .. } => Some(json!({"reason": reason})),
            Self::ArchiveSession { .. } | Self::ResumeApproval { .. } => Some(json!({})),
            Self::ForkSession { at_turn, .. } => Some(json!({"atTurn": at_turn})),
            Self::DecideApproval { decision, reason, .. } => Some(json!({"decision": decision, "reason": reason})),
            Self::PreviewCommandRegistryRequest { decision, .. }
            | Self::DecideCommandRegistryRequest { decision, .. } => Some(serde_json::to_value(decision).expect("registry decision serializes")),
            Self::ApplyCommandRegistryRequest { session_id, .. } => Some(json!({"sessionId": session_id})),
            Self::WorkflowMemoryFeedback { session_id, feedback, payload, .. } => Some(json!({"sessionId": session_id, "feedback": feedback, "payload": payload})),
            Self::ValidateRoleDraft { draft } | Self::CreateRoleFromDraft { draft } | Self::UpdateRoleFromDraft { draft, .. } => Some(serde_json::to_value(draft).expect("role draft serializes")),
            Self::ActivateRoleVersion { version_id, .. } => Some(json!({"versionId": version_id})),
            Self::ArchiveRole { .. } | Self::UnarchiveRole { .. } => Some(json!({})),
            Self::SetRequirements { title, requirements, .. } => Some(json!({"title": title, "requirements": requirements})),
            Self::ClearRequirements { .. } => Some(json!({})),
            Self::SubmitRequirementsReviewerInput { message, .. } => Some(json!({"message": message})),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GuiOperationApiMapping {
    pub operation: GuiOperationName,
    pub local_only: bool,
    pub method: Option<String>,
    pub route_or_action: String,
    pub request_shape: String,
    pub response_shape: String,
    pub error_shape: String,
    pub expected_projection_effect: GuiOperationExpectation,
}

fn http_mapping(operation: GuiOperationName, method: &str, route: &str, request_shape: &str, response_shape: &str, expected_projection_effect: GuiOperationExpectation) -> GuiOperationApiMapping {
    GuiOperationApiMapping {
        operation,
        local_only: false,
        method: Some(method.to_string()),
        route_or_action: route.to_string(),
        request_shape: request_shape.to_string(),
        response_shape: response_shape.to_string(),
        error_shape: r#"{"error":{"code","message","details"}}"#.to_string(),
        expected_projection_effect,
    }
}

fn local_mapping(operation: GuiOperationName, action: &str, expected_projection_effect: GuiOperationExpectation) -> GuiOperationApiMapping {
    GuiOperationApiMapping {
        operation,
        local_only: true,
        method: None,
        route_or_action: action.to_string(),
        request_shape: "local controller state".to_string(),
        response_shape: "GuiControllerState/SyncOutcome".to_string(),
        error_shape: "ApiErrorPacket or SyncError mapped by controller".to_string(),
        expected_projection_effect,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandRegistryDecisionInput {
    pub session_id: Option<String>,
    pub status: String,
    pub final_scope: Option<GuiRegistryScope>,
    pub final_execution_policy: Option<GuiFinalExecutionPolicy>,
    pub final_command: Option<GuiCommandSeed>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GuiRegistryScope {
    pub scope_type: String,
    pub project_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GuiFinalExecutionPolicy {
    pub decision: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GuiCommandSeed {
    pub action_id: String,
    pub binary_name: String,
    pub candidate_paths: Vec<String>,
    pub starlark_object: String,
    pub starlark_method: String,
    pub argv_prefix: Vec<String>,
    pub default_cwd: String,
    pub cwd_policy: String,
    pub env_policy: String,
    pub sync_allowed: bool,
    pub async_allowed: bool,
    pub max_runtime_ms: Option<i64>,
    pub end_of_turn_behavior: String,
    pub end_of_session_behavior: String,
    pub stdin_policy: String,
    pub min_await_ms: i64,
    pub max_await_ms: i64,
    pub output_buffer_bytes: i64,
    pub terminate_grace_ms: i64,
    pub output_limit_bytes: i64,
    pub mutation_class: String,
    pub model_description: String,
    pub allow_cwd_arg: bool,
    pub allow_args_arg: bool,
    pub forbidden_args: Vec<String>,
    pub execution_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandRegistryRequestSummary {
    pub id: String,
    pub operation: String,
    pub action_id: String,
    pub action_label: String,
    pub status: String,
    pub state_text: String,
    pub apply_status: String,
    pub final_scope_type: Option<String>,
    pub final_project_key: Option<String>,
    pub scope_summary: Option<String>,
    pub final_policy: Option<String>,
    pub policy_summary: Option<String>,
    pub can_preview: bool,
    pub preview_label: String,
    pub can_decide: bool,
    pub decide_label: String,
    pub can_apply: bool,
    pub apply_label: String,
}

impl CommandRegistryRequestSummary {
    pub fn from_server_value(value: &Value) -> Option<Self> {
        let status = value.get("approvalStatus").and_then(Value::as_str)?.to_string();
        let apply_status = value.get("applicationStatus").and_then(Value::as_str)?.to_string();
        let proposed = value.get("proposedCommand")?;
        let final_scope = value.get("finalScope").and_then(|value| if value.is_null() { None } else { Some(value) });
        let final_policy = value.get("finalExecutionPolicy").and_then(|value| if value.is_null() { None } else { Some(value) });
        let action_id = proposed.get("actionId").and_then(Value::as_str)?.to_string();
        let final_scope_type = final_scope.and_then(|scope| scope.get("scopeType").or_else(|| scope.get("type"))).and_then(Value::as_str).map(str::to_string);
        let final_project_key = final_scope.and_then(|scope| scope.get("projectKey")).and_then(Value::as_str).map(str::to_string);
        let final_policy = final_policy.and_then(|policy| policy.get("decision")).and_then(Value::as_str).map(str::to_string);
        let can_preview = status == "pending";
        let can_decide = status == "pending";
        let can_apply = status == "approved" && apply_status == "pending";
        Some(Self {
            id: value.get("id").and_then(Value::as_str).map(str::to_string)
                .or_else(|| value.get("id").map(|id| id.to_string().trim_matches('"').to_string()))?,
            operation: value.get("operation").and_then(Value::as_str)?.to_string(),
            action_label: action_label(&action_id),
            action_id,
            status: status.clone(),
            state_text: registry_request_state_text(&status, &apply_status),
            apply_status: apply_status.clone(),
            scope_summary: scope_summary(final_scope_type.as_deref(), final_project_key.as_deref()),
            final_scope_type,
            final_project_key,
            policy_summary: final_policy.clone(),
            final_policy,
            can_preview,
            preview_label: if can_preview { "Preview decision".to_string() } else { "Preview unavailable".to_string() },
            can_decide,
            decide_label: if can_decide { "Decide request".to_string() } else { "Decision unavailable".to_string() },
            can_apply,
            apply_label: if can_apply { "Apply approved request".to_string() } else { "Apply unavailable".to_string() },
        })
    }
}

fn action_label(action_id: &str) -> String {
    action_id
        .strip_prefix("cmd.")
        .unwrap_or(action_id)
        .replace('.', " · ")
}

fn registry_request_state_text(status: &str, apply_status: &str) -> String {
    if status == "pending" {
        "Needs registry decision".to_string()
    } else if status == "approved" && apply_status == "pending" {
        "Approved · ready to apply".to_string()
    } else {
        format!("{status} · {apply_status}")
    }
}

fn scope_summary(scope_type: Option<&str>, project_key: Option<&str>) -> Option<String> {
    scope_type.map(|scope| match (scope, project_key) {
        ("project", Some(project)) => format!("project:{project}"),
        (other, _) => other.to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GuiOperationResult {
    pub operation_id: String,
    pub operation: GuiOperationName,
    pub expectation: GuiOperationExpectation,
    pub outcome: GuiOperationOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", content = "payload", rename_all = "camelCase")]
pub enum GuiOperationOutcome {
    Accepted { entity_id: Option<String> },
    ProjectionUpdated { watermark: Watermark },
    DirectValue { value: Value },
    CommandRegistryRequests { requests: Vec<CommandRegistryRequestSummary> },
    Error { error: ApiErrorPacket },
}

pub const DART_FORBIDDEN_RESPONSIBILITIES: &[&str] = &[
    "sessionLifecycleStatus",
    "approvalAvailability",
    "commandVisibility",
    "commandPolicy",
    "roleStatus",
    "processStatus",
    "timelineInterpretation",
    "semanticOperationSuccess",
];

pub const DART_ALLOWED_EPHEMERAL_RESPONSIBILITIES: &[&str] = &[
    "textFields",
    "focus",
    "scrollPosition",
    "hoverPressState",
    "animations",
    "localLayout",
];

pub const GUI_OPERATION_VARIANT_COUNT: usize = 50;

impl Default for RuntimeProjection {
    fn default() -> Self {
        Self {
            watermark: 0,
            server_status: ServerStatusProjection::default(),
            projects: Vec::new(),
            sessions: Vec::new(),
            selected_session: None,
            timeline: Vec::new(),
            selected_chat_entries: Vec::new(),
            pending_approvals: Vec::new(),
            roles: Vec::new(),
            command_registry: Vec::new(),
            command_registry_requests: Vec::new(),
            workflow_memories: Vec::new(),
            statistics: RuntimeStatistics::default(),
            resync_required: None,
        }
    }
}

impl RuntimeProjection {
    pub fn apply_delta(&mut self, delta: RuntimeDelta) -> ApplyOutcome {
        apply_delta(self, delta)
    }

    pub fn apply_deltas<I>(&mut self, deltas: I) -> ApplyOutcome
    where
        I: IntoIterator<Item = RuntimeDelta>,
    {
        let mut outcome = ApplyOutcome::Unchanged;
        for delta in deltas {
            let next = self.apply_delta(delta);
            if matches!(next, ApplyOutcome::ResyncRequired) {
                return next;
            }
            if matches!(next, ApplyOutcome::Applied) {
                outcome = ApplyOutcome::Applied;
            }
        }
        outcome
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    Unchanged,
    Stale,
    ResyncRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuntimeDeltaKind {
    ServerStatus { status: ServerStatusProjection },
    SessionUpsert { session: SessionListItem },
    SessionArchive { session_id: String, archived_at: Option<String> },
    SessionClose { session_id: String, closed_at: Option<String> },
    SelectedSessionReplace { session: Option<SelectedSessionDetail> },
    SelectedSessionPatch { session: SelectedSessionDetail },
    SelectedChatAppend { entry: AgentRuntimeChatEntry },
    SelectedChatUpdate { entry: AgentRuntimeChatEntry },
    SelectedChatFinalize { entry_id: String, delivery_state: String, status: String },
    TimelineAppend { item: TimelineItem },
    TurnStatusChanged { turn_id: String, status: String },
    ToolStatusChanged { tool_call_id: String, status: String },
    ScriptStatusChanged { script_run_id: String, status: String },
    ProcessStatusChanged { process_id: String, status: String },
    ApprovalUpsert { approval: PendingApprovalSummary },
    ApprovalRemove { approval_id: String },
    RoleUpsert { role: RoleSummary },
    RoleArchive { role_id: String, archived_at: Option<String> },
    CommandRegistryUpsert { command: CommandRegistrySummary },
    CommandRegistryDisable { command_id: String },
    CommandRegistryRequestUpsert { request: CommandRegistryRequestSummary },
    CommandRegistryRequestRemove { request_id: String },
    WorkflowMemoryUpsert { memory: WorkflowMemorySummary },
    WorkflowMemoryEvent { item: TimelineItem },
    RequirementsReviewUpdate { session_id: String, summary: RequirementsReviewSummary },
    ResyncRequired { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDelta {
    pub watermark: Watermark,
    #[serde(default)]
    pub previous_watermark: Option<Watermark>,
    #[serde(flatten)]
    pub kind: RuntimeDeltaKind,
}

pub fn apply_delta(projection: &mut RuntimeProjection, delta: RuntimeDelta) -> ApplyOutcome {
    if delta.watermark < projection.watermark {
        return ApplyOutcome::Stale;
    }
    if let Some(previous) = delta.previous_watermark {
        if previous != projection.watermark && delta.watermark != projection.watermark {
            mark_resync_required(
                projection,
                "delta watermark gap detected",
                Some(projection.watermark),
                Some(delta.watermark),
            );
            return ApplyOutcome::ResyncRequired;
        }
    } else if delta.watermark > projection.watermark + 1 && projection.watermark != 0 {
        mark_resync_required(
            projection,
            "delta watermark gap detected",
            Some(projection.watermark + 1),
            Some(delta.watermark),
        );
        return ApplyOutcome::ResyncRequired;
    }

    let changed = apply_delta_kind(projection, delta.kind);
    if delta.watermark > projection.watermark {
        projection.watermark = delta.watermark;
        return ApplyOutcome::Applied;
    }
    if changed {
        ApplyOutcome::Applied
    } else {
        ApplyOutcome::Unchanged
    }
}

fn mark_resync_required(
    projection: &mut RuntimeProjection,
    reason: &str,
    expected_watermark: Option<Watermark>,
    received_watermark: Option<Watermark>,
) {
    projection.resync_required = Some(ResyncRequiredState {
        required: true,
        reason: reason.to_string(),
        expected_watermark,
        received_watermark,
    });
}

fn apply_delta_kind(projection: &mut RuntimeProjection, kind: RuntimeDeltaKind) -> bool {
    match kind {
        RuntimeDeltaKind::ServerStatus { status } => replace_if_changed(&mut projection.server_status, status),
        RuntimeDeltaKind::SessionUpsert { session } => upsert_by(&mut projection.sessions, session, |item| item.id.as_str()),
        RuntimeDeltaKind::SessionArchive { session_id, archived_at } => {
            if let Some(index) = projection.sessions.iter().position(|item| item.id == session_id) {
                if projection.sessions[index].status == "open" {
                    projection.sessions.remove(index);
                    true
                } else {
                    let session = &mut projection.sessions[index];
                    let mut changed = false;
                    changed |= replace_if_changed(&mut session.tracked, false);
                    changed |= replace_if_changed(&mut session.archived_at, archived_at);
                    changed
                }
            } else {
                false
            }
        }
        RuntimeDeltaKind::SessionClose { session_id, closed_at } => {
            if let Some(session) = projection.sessions.iter_mut().find(|item| item.id == session_id) {
                let mut changed = false;
                changed |= replace_if_changed(&mut session.status, "closed".to_string());
                changed |= replace_if_changed(&mut session.closed_at, closed_at);
                changed
            } else {
                false
            }
        }
        RuntimeDeltaKind::SelectedSessionReplace { session } => replace_if_changed(&mut projection.selected_session, session),
        RuntimeDeltaKind::SelectedSessionPatch { session } => replace_if_changed(&mut projection.selected_session, Some(session)),
        RuntimeDeltaKind::SelectedChatAppend { entry } | RuntimeDeltaKind::SelectedChatUpdate { entry } => {
            let changed = upsert_by(&mut projection.selected_chat_entries, entry, |item| item.id.as_str());
            cap_selected_chat(projection);
            changed
        }
        RuntimeDeltaKind::SelectedChatFinalize { entry_id, delivery_state, status } => {
            if let Some(entry) = projection.selected_chat_entries.iter_mut().find(|entry| entry.id == entry_id) {
                let mut changed = false;
                changed |= replace_if_changed(&mut entry.delivery_state, delivery_state);
                changed |= replace_if_changed(&mut entry.status, status);
                changed |= replace_if_changed(&mut entry.is_streaming, false);
                changed
            } else {
                false
            }
        }
        RuntimeDeltaKind::TimelineAppend { item } | RuntimeDeltaKind::WorkflowMemoryEvent { item } => append_timeline(projection, item),
        RuntimeDeltaKind::TurnStatusChanged { turn_id, status } => update_timeline_status(projection, "turn", &turn_id, status),
        RuntimeDeltaKind::ToolStatusChanged { tool_call_id, status } => update_timeline_status(projection, "tool", &tool_call_id, status),
        RuntimeDeltaKind::ScriptStatusChanged { script_run_id, status } => update_timeline_status(projection, "script", &script_run_id, status),
        RuntimeDeltaKind::ProcessStatusChanged { process_id, status } => update_timeline_status(projection, "process", &process_id, status),
        RuntimeDeltaKind::ApprovalUpsert { approval } => upsert_by(&mut projection.pending_approvals, approval, |item| item.id.as_str()),
        RuntimeDeltaKind::ApprovalRemove { approval_id } => remove_by(&mut projection.pending_approvals, |item| item.id == approval_id),
        RuntimeDeltaKind::RoleUpsert { role } => upsert_by(&mut projection.roles, role, |item| item.id.as_str()),
        RuntimeDeltaKind::RoleArchive { role_id, archived_at } => {
            if let Some(role) = projection.roles.iter_mut().find(|item| item.id == role_id) {
                let mut changed = false;
                changed |= replace_if_changed(&mut role.status, "archived".to_string());
                changed |= replace_if_changed(&mut role.archived_at, archived_at);
                changed
            } else {
                false
            }
        }
        RuntimeDeltaKind::CommandRegistryUpsert { command } => upsert_by(&mut projection.command_registry, command, |item| item.id.as_str()),
        RuntimeDeltaKind::CommandRegistryDisable { command_id } => {
            if let Some(command) = projection.command_registry.iter_mut().find(|item| item.id == command_id) {
                replace_if_changed(&mut command.enabled, false)
            } else {
                false
            }
        }
        RuntimeDeltaKind::CommandRegistryRequestUpsert { request } => upsert_by(&mut projection.command_registry_requests, request, |item| item.id.as_str()),
        RuntimeDeltaKind::CommandRegistryRequestRemove { request_id } => remove_by(&mut projection.command_registry_requests, |item| item.id == request_id),
        RuntimeDeltaKind::WorkflowMemoryUpsert { memory } => upsert_by(&mut projection.workflow_memories, memory, |item| item.id.as_str()),
        RuntimeDeltaKind::RequirementsReviewUpdate { session_id, summary } => {
            if let Some(session) = projection.selected_session.as_mut().filter(|session| session.id == session_id) {
                replace_if_changed(&mut session.requirements_review, Some(summary))
            } else {
                false
            }
        }
        RuntimeDeltaKind::ResyncRequired { reason } => {
            mark_resync_required(projection, &reason, None, None);
            true
        }
    }
}

fn append_timeline(projection: &mut RuntimeProjection, item: TimelineItem) -> bool {
    if projection.timeline.iter().any(|existing| existing.id == item.id || existing.sequence == item.sequence) {
        return false;
    }
    let insert_at = projection
        .timeline
        .binary_search_by_key(&item.sequence, |existing| existing.sequence)
        .unwrap_or_else(|index| index);
    projection.timeline.insert(insert_at, item);
    true
}

fn cap_selected_chat(projection: &mut RuntimeProjection) {
    if projection.selected_chat_entries.len() > 50 {
        let drop_count = projection.selected_chat_entries.len() - 50;
        projection.selected_chat_entries.drain(0..drop_count);
    }
}

fn update_timeline_status(
    projection: &mut RuntimeProjection,
    entity_type: &str,
    entity_id: &str,
    status: String,
) -> bool {
    let mut changed = false;
    for item in &mut projection.timeline {
        if item.entity_type == entity_type && item.entity_id.as_deref() == Some(entity_id) {
            changed |= replace_if_changed(&mut item.status, Some(status.clone()));
        }
    }
    changed
}

fn upsert_by<T, F>(items: &mut Vec<T>, incoming: T, id: F) -> bool
where
    T: PartialEq,
    F: Fn(&T) -> &str,
{
    let incoming_id = id(&incoming).to_string();
    if let Some(existing) = items.iter_mut().find(|item| id(item) == incoming_id) {
        return replace_if_changed(existing, incoming);
    }
    items.push(incoming);
    true
}

fn remove_by<T, F>(items: &mut Vec<T>, predicate: F) -> bool
where
    F: Fn(&T) -> bool,
{
    let before = items.len();
    items.retain(|item| !predicate(item));
    items.len() != before
}

fn replace_if_changed<T: PartialEq>(target: &mut T, value: T) -> bool {
    if *target == value {
        return false;
    }
    *target = value;
    true
}

pub fn timeline_item_id(sequence: Watermark) -> String {
    format!("event-{sequence}")
}

pub fn timeline_by_sequence(items: Vec<TimelineItem>) -> Vec<TimelineItem> {
    let mut by_sequence: BTreeMap<Watermark, TimelineItem> = BTreeMap::new();
    for item in items {
        by_sequence.entry(item.sequence).or_insert(item);
    }
    by_sequence.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn session(id: &str) -> SessionListItem {
        SessionListItem {
            id: id.to_string(),
            status: "open".to_string(),
            role_id: Some("role".to_string()),
            role_version: Some("1".to_string()),
            project_key: Some("project".to_string()),
            title: None,
            name: None,
            workdir: ".".to_string(),
            tracked: true,
            archived_at: None,
            closed_at: None,
            updated_at: None,
        }
    }

    fn event(sequence: Watermark, entity_type: &str, entity_id: &str, event_type: &str) -> TimelineItem {
        TimelineItem {
            id: timeline_item_id(sequence),
            sequence,
            session_id: Some("session-1".to_string()),
            turn_id: None,
            entity_type: entity_type.to_string(),
            entity_id: Some(entity_id.to_string()),
            event_type: event_type.to_string(),
            status: Some("running".to_string()),
            summary: None,
            payload: Value::Null,
            created_at: None,
        }
    }

    fn delta(watermark: Watermark, kind: RuntimeDeltaKind) -> RuntimeDelta {
        RuntimeDelta {
            watermark,
            previous_watermark: None,
            kind,
        }
    }

    fn registry_request(id: &str, status: &str, apply_status: &str) -> CommandRegistryRequestSummary {
        CommandRegistryRequestSummary::from_server_value(&json!({
            "id": id,
            "operation": "add",
            "proposedCommand": {"actionId": "cmd.rg.audit"},
            "approvalStatus": status,
            "applicationStatus": apply_status,
            "finalScope": {"scopeType": "project", "projectKey": "alpha"},
            "finalExecutionPolicy": {"decision": "ownerApproval"}
        }))
        .expect("registry request summary")
    }

    fn command_seed() -> GuiCommandSeed {
        GuiCommandSeed {
            action_id: "cmd.rg.audit".to_string(),
            binary_name: "rg".to_string(),
            candidate_paths: vec!["/usr/bin/rg".to_string()],
            starlark_object: "rg_audit".to_string(),
            starlark_method: "run".to_string(),
            argv_prefix: vec!["--files".to_string()],
            default_cwd: ".".to_string(),
            cwd_policy: "underExecutionRoot".to_string(),
            env_policy: "empty".to_string(),
            sync_allowed: true,
            async_allowed: true,
            max_runtime_ms: Some(5000),
            end_of_turn_behavior: "terminate".to_string(),
            end_of_session_behavior: "terminate".to_string(),
            stdin_policy: "forbid".to_string(),
            min_await_ms: 0,
            max_await_ms: 60000,
            output_buffer_bytes: 64000,
            terminate_grace_ms: 1000,
            output_limit_bytes: 12000,
            mutation_class: "readOnly".to_string(),
            model_description: "audit command".to_string(),
            allow_cwd_arg: true,
            allow_args_arg: true,
            forbidden_args: Vec::new(),
            execution_policy: "allow".to_string(),
        }
    }

    fn role_draft(id: &str, version: &str) -> RoleEditorDraft {
        RoleEditorDraft {
            id: id.to_string(),
            version: version.to_string(),
            display_name: "GUI Role".to_string(),
            model_defaults: RoleEditorModelDefaults {
                model: "gpt-5.4-mini".to_string(),
                reasoning_effort: "medium".to_string(),
            },
            instruction_text: "Inline GUI-authored role instructions.".to_string(),
            capabilities: vec!["tool.execute_code".to_string()],
            policy: BTreeMap::from([("tool.execute_code".to_string(), "allow".to_string())]),
            routing: RoleEditorRoutingMetadata {
                mode: "direct".to_string(),
                default_recipient: Some("owner".to_string()),
                allowed_recipients: vec!["owner".to_string()],
                reserved_actions: vec!["message.send".to_string()],
            },
            visibility: RoleEditorVisibilityMetadata {
                listed: true,
                owner_visible: true,
            },
            lifecycle_authority: RoleEditorLifecycleAuthorityMetadata {
                can_spawn_agents: false,
                can_archive_agents: false,
                reserved_actions: vec!["agent.archive".to_string()],
            },
        }
    }

    fn operation_samples() -> Vec<GuiOperationRequest> {
        vec![
            GuiOperationRequest::Connect { base_url: "http://127.0.0.1:8765".to_string(), selected_session_id: None },
            GuiOperationRequest::Hydrate { selected_session_id: None },
            GuiOperationRequest::Rehydrate { selected_session_id: Some("session-1".to_string()) },
            GuiOperationRequest::Disconnect,
            GuiOperationRequest::SelectSession { session_id: Some("session-1".to_string()) },
            GuiOperationRequest::SelectWorkflowMemory { memory_id: Some("memory-1".to_string()) },
            GuiOperationRequest::CreateSession { role: "runtime-allow".to_string(), project: Some("project".to_string()), model: Some("gpt-5.4-mini".to_string()), workdir: Some(".".to_string()), worktree_root: None, title: None, name: None },
            GuiOperationRequest::ListProjects,
            GuiOperationRequest::CreateProject { project_key: "project".to_string(), display_name: "Project".to_string(), default_workdir: ".".to_string(), default_worktree_root: ".".to_string(), default_role_id: Some("runtime-allow".to_string()), default_model: "gpt-5.4-mini".to_string() },
            GuiOperationRequest::UpdateProject { project_key: "project".to_string(), display_name: "Project Updated".to_string(), default_workdir: ".".to_string(), default_worktree_root: ".".to_string(), default_role_id: Some("runtime-no-rg".to_string()), default_model: "gpt-5.5".to_string() },
            GuiOperationRequest::ArchiveProject { project_key: "project".to_string() },
            GuiOperationRequest::UnarchiveProject { project_key: "project".to_string() },
            GuiOperationRequest::UpdateRuntimeSettings { base_url: "http://127.0.0.1:8765".to_string(), selected_project_id: Some("project".to_string()) },
            GuiOperationRequest::UpdateSessionSettings { session_id: "session-1".to_string(), project: "project".to_string(), role: "runtime-allow".to_string(), model: "gpt-5.5".to_string(), workdir: ".".to_string(), worktree_root: ".".to_string(), title: "title".to_string(), name: "name".to_string(), tracked: true },
            GuiOperationRequest::SendMessage { session_id: "session-1".to_string(), message: "hello".to_string() },
            GuiOperationRequest::TerminateProcess { session_id: "session-1".to_string(), handle: "proc_1".to_string() },
            GuiOperationRequest::InputProcess { session_id: "session-1".to_string(), handle: "proc_1".to_string(), text: "hello".to_string() },
            GuiOperationRequest::FlushProcess { session_id: "session-1".to_string(), handle: "proc_1".to_string() },
            GuiOperationRequest::CompactSession { session_id: "session-1".to_string(), through_turn: None },
            GuiOperationRequest::GrantGodMode { session_id: "session-1".to_string(), reason: "break-glass host shell needed".to_string() },
            GuiOperationRequest::RevokeGodMode { session_id: "session-1".to_string(), reason: "break-glass complete".to_string() },
            GuiOperationRequest::CloseSession { session_id: "session-1".to_string(), reason: Some("done".to_string()) },
            GuiOperationRequest::ArchiveSession { session_id: "session-1".to_string() },
            GuiOperationRequest::ForkSession { session_id: "session-1".to_string(), at_turn: "turn-1".to_string() },
            GuiOperationRequest::DecideApproval { approval_id: "approval-1".to_string(), decision: "approved".to_string(), reason: "operator approved".to_string() },
            GuiOperationRequest::ResumeApproval { approval_id: "approval-1".to_string() },
            GuiOperationRequest::ListCommandRegistry { session_id: Some("session-1".to_string()), project_key: None },
            GuiOperationRequest::ShowCommand { action_id: "cmd.rg.audit".to_string(), session_id: None, project_key: Some("project".to_string()) },
            GuiOperationRequest::ListCommandRegistryRequests,
            GuiOperationRequest::ShowCommandRegistryRequest { request_id: "request-1".to_string() },
            GuiOperationRequest::PreviewCommandRegistryRequest { request_id: "request-1".to_string(), decision: registry_decision() },
            GuiOperationRequest::DecideCommandRegistryRequest { request_id: "request-1".to_string(), decision: registry_decision() },
            GuiOperationRequest::ApplyCommandRegistryRequest { request_id: "request-1".to_string(), session_id: "session-1".to_string() },
            GuiOperationRequest::WorkflowMemoryFeedback { memory_id: "memory-1".to_string(), session_id: "session-1".to_string(), feedback: "attempted".to_string(), payload: json!({"variant": true}) },
            GuiOperationRequest::RoleEditorOptions,
            GuiOperationRequest::ValidateRoleDraft { draft: role_draft("gui-role", "1.0.0") },
            GuiOperationRequest::CreateRoleFromDraft { draft: role_draft("gui-role", "1.0.0") },
            GuiOperationRequest::UpdateRoleFromDraft { role_id: "gui-role".to_string(), draft: role_draft("gui-role", "1.0.1") },
            GuiOperationRequest::ShowRoleDetail { role_id: "gui-role".to_string() },
            GuiOperationRequest::ListRoleVersions { role_id: "gui-role".to_string() },
            GuiOperationRequest::ShowRoleVersion { version_id: "00000000-0000-0000-0000-000000000001".to_string() },
            GuiOperationRequest::ExportRole { role_id: "gui-role".to_string() },
            GuiOperationRequest::ActivateRoleVersion { role_id: "gui-role".to_string(), version_id: "00000000-0000-0000-0000-000000000001".to_string() },
            GuiOperationRequest::ArchiveRole { role_id: "gui-role".to_string() },
            GuiOperationRequest::UnarchiveRole { role_id: "gui-role".to_string() },
            GuiOperationRequest::SetRequirements { session_id: "session-1".to_string(), title: Some("contract".to_string()), requirements: vec![json!({"key":"prove_it","statement":"Prove it.","severity":"must","verificationMethod":{"method":"review"}})] },
            GuiOperationRequest::ClearRequirements { session_id: "session-1".to_string() },
            GuiOperationRequest::ShowRequirementsStatus { session_id: "session-1".to_string() },
            GuiOperationRequest::ListRequirementsPackets { session_id: "session-1".to_string() },
            GuiOperationRequest::SubmitRequirementsReviewerInput { source_session_id: "session-1".to_string(), message: "I accept the waiver; continue the review.".to_string() },
        ]
    }

    fn registry_decision() -> CommandRegistryDecisionInput {
        CommandRegistryDecisionInput {
            session_id: Some("session-1".to_string()),
            status: "approved".to_string(),
            final_scope: Some(GuiRegistryScope { scope_type: "global".to_string(), project_key: None }),
            final_execution_policy: Some(GuiFinalExecutionPolicy { decision: "allow".to_string(), reason: Some("approved for audit".to_string()) }),
            final_command: Some(command_seed()),
        }
    }

    #[test]
    fn session_upsert_is_idempotent() {
        let mut projection = RuntimeProjection::default();
        let s = session("session-1");
        assert_eq!(projection.apply_delta(delta(1, RuntimeDeltaKind::SessionUpsert { session: s.clone() })), ApplyOutcome::Applied);
        assert_eq!(projection.apply_delta(delta(1, RuntimeDeltaKind::SessionUpsert { session: s })), ApplyOutcome::Unchanged);
        assert_eq!(projection.sessions.len(), 1);
    }

    #[test]
    fn session_archive_removes_open_session_from_list_without_clearing_selected_detail() {
        let mut projection = RuntimeProjection::default();
        projection.selected_session = Some(SelectedSessionDetail {
            id: "session-1".to_string(),
            role_id: Some("role".to_string()),
            role_version: Some("1".to_string()),
            project_key: None,
            workdir: ".".to_string(),
            worktree_root: None,
            title: Some("selected".to_string()),
            name: None,
            status: "open".to_string(),
            pending_approval_count: 0,
            managed_process_count: 0,
            active_turn_id: None,
            queued_submitted_input_count: 0,
            applied_steering_count: 0,
            submit_disposition: None,
            submit_status: None,
            terminal_submission_rejection: None,
            metadata: Value::Null,
            requirements_review: None,
        });
        projection.apply_delta(delta(1, RuntimeDeltaKind::SessionUpsert { session: session("session-1") }));
        projection.apply_delta(delta(2, RuntimeDeltaKind::SessionArchive {
            session_id: "session-1".to_string(),
            archived_at: Some("archived".to_string()),
        }));
        assert!(projection.sessions.is_empty());
        assert_eq!(projection.selected_session.as_ref().map(|session| session.id.as_str()), Some("session-1"));
    }

    #[test]
    fn session_archive_keeps_non_open_session_consistent_with_snapshot_filter() {
        let mut projection = RuntimeProjection::default();
        let mut closed = session("session-1");
        closed.status = "closed".to_string();
        projection.apply_delta(delta(1, RuntimeDeltaKind::SessionUpsert { session: closed }));
        projection.apply_delta(delta(2, RuntimeDeltaKind::SessionArchive {
            session_id: "session-1".to_string(),
            archived_at: Some("archived".to_string()),
        }));
        assert!(!projection.sessions[0].tracked);
        assert_eq!(projection.sessions[0].archived_at.as_deref(), Some("archived"));
    }

    #[test]
    fn selected_session_update_replaces_detail() {
        let mut projection = RuntimeProjection::default();
        let detail = SelectedSessionDetail {
            id: "session-1".to_string(),
            role_id: Some("role".to_string()),
            role_version: Some("1".to_string()),
            project_key: None,
            workdir: ".".to_string(),
            worktree_root: None,
            title: Some("selected".to_string()),
            name: None,
            status: "open".to_string(),
            pending_approval_count: 0,
            managed_process_count: 0,
            active_turn_id: None,
            queued_submitted_input_count: 0,
            applied_steering_count: 0,
            submit_disposition: None,
            submit_status: None,
            terminal_submission_rejection: None,
            metadata: Value::Null,
            requirements_review: None,
        };
        projection.apply_delta(delta(1, RuntimeDeltaKind::SelectedSessionReplace { session: Some(detail.clone()) }));
        assert_eq!(projection.selected_session, Some(detail));
    }

    #[test]
    fn requirements_review_delta_updates_only_selected_source_detail() {
        let mut projection = RuntimeProjection::default();
        projection.selected_session = Some(SelectedSessionDetail {
            id: "session-1".to_string(),
            role_id: Some("role".to_string()),
            role_version: Some("1".to_string()),
            project_key: None,
            workdir: ".".to_string(),
            worktree_root: None,
            title: Some("selected".to_string()),
            name: None,
            status: "open".to_string(),
            pending_approval_count: 0,
            managed_process_count: 0,
            active_turn_id: None,
            queued_submitted_input_count: 0,
            applied_steering_count: 0,
            submit_disposition: None,
            submit_status: None,
            terminal_submission_rejection: None,
            metadata: Value::Null,
            requirements_review: None,
        });
        let summary = RequirementsReviewSummary {
            active: true,
            active_set_id: Some("set-1".to_string()),
            total: 2,
            unresolved: 1,
            passed: 1,
            blocked: 0,
            waived: 0,
            reviewer_session_id: Some("reviewer-1".to_string()),
            review_status: Some("inReview".to_string()),
            latest_claim_packet_id: Some("claim-1".to_string()),
            latest_verdict_packet_id: None,
            packets: vec![RequirementsPacketSummary {
                id: "claim-1".to_string(),
                requirement_set_id: "set-1".to_string(),
                packet_kind: "claim".to_string(),
                status: "reviewable".to_string(),
                reviewer_session_id: Some("reviewer-1".to_string()),
                turn_id: Some("turn-1".to_string()),
            }],
            progress: vec![json!({"requirementKey":"a","status":"passed"})],
            owner_action: None,
        };
        assert_eq!(
            projection.apply_delta(delta(1, RuntimeDeltaKind::RequirementsReviewUpdate {
                session_id: "session-1".to_string(),
                summary: summary.clone(),
            })),
            ApplyOutcome::Applied
        );
        assert_eq!(projection.selected_session.as_ref().and_then(|session| session.requirements_review.clone()), Some(summary));
        assert_eq!(
            projection.apply_delta(delta(2, RuntimeDeltaKind::RequirementsReviewUpdate {
                session_id: "other-session".to_string(),
                summary: RequirementsReviewSummary { active: false, ..projection.selected_session.as_ref().unwrap().requirements_review.clone().unwrap() },
            })),
            ApplyOutcome::Applied
        );
        assert_eq!(projection.selected_session.as_ref().unwrap().requirements_review.as_ref().unwrap().active_set_id.as_deref(), Some("set-1"));
    }

    #[test]
    fn timeline_append_preserves_sequence_order() {
        let mut projection = RuntimeProjection::default();
        projection.apply_delta(delta(1, RuntimeDeltaKind::TimelineAppend { item: event(10, "turn", "turn-1", "turn.completed") }));
        projection.apply_delta(delta(2, RuntimeDeltaKind::TimelineAppend { item: event(8, "turn", "turn-1", "turn.started") }));
        assert_eq!(projection.timeline.iter().map(|item| item.sequence).collect::<Vec<_>>(), vec![8, 10]);
    }

    #[test]
    fn duplicate_event_is_idempotent() {
        let mut projection = RuntimeProjection::default();
        let item = event(8, "turn", "turn-1", "turn.started");
        projection.apply_delta(delta(1, RuntimeDeltaKind::TimelineAppend { item: item.clone() }));
        projection.apply_delta(delta(1, RuntimeDeltaKind::TimelineAppend { item }));
        assert_eq!(projection.timeline.len(), 1);
    }

    #[test]
    fn approval_update_and_removal() {
        let mut projection = RuntimeProjection::default();
        let approval = PendingApprovalSummary {
            id: "approval-1".to_string(),
            session_id: "session-1".to_string(),
            turn_id: None,
            action_name: "fs.write".to_string(),
            required_approver_kind: "owner".to_string(),
            status: "pending".to_string(),
            can_decide: true,
            can_resume: false,
            input_context: Value::Null,
            created_at: None,
            decision_at: None,
            decision_reason: None,
            resumable_action_status: None,
        };
        projection.apply_delta(delta(1, RuntimeDeltaKind::ApprovalUpsert { approval }));
        assert_eq!(projection.pending_approvals.len(), 1);
        projection.apply_delta(delta(2, RuntimeDeltaKind::ApprovalRemove { approval_id: "approval-1".to_string() }));
        assert!(projection.pending_approvals.is_empty());
    }

    #[test]
    fn role_update_and_archive() {
        let mut projection = RuntimeProjection::default();
        projection.apply_delta(delta(1, RuntimeDeltaKind::RoleUpsert { role: RoleSummary {
            id: "role-1".to_string(),
            display_name: "Role".to_string(),
            current_version_id: Some("version-1".to_string()),
            status: "active".to_string(),
            model: Some("gpt".to_string()),
            reasoning_effort: Some("medium".to_string()),
            archived_at: None,
            version: Some("1.0.0".to_string()),
            instruction_text: Some("instructions".to_string()),
            capabilities: vec!["tool.execute_code".to_string()],
            policy: BTreeMap::from([("tool.execute_code".to_string(), "allow".to_string())]),
            routing: json!({"mode":"direct","defaultRecipient":"owner","allowedRecipients":["owner"],"reservedActions":[]}),
            visibility: json!({"listed":true,"ownerVisible":true}),
            lifecycle_authority: json!({"canSpawnAgents":false,"canArchiveAgents":false,"reservedActions":[]}),
            versions: vec![RoleVersionSummary {
                version_id: "version-1".to_string(),
                version: "1.0.0".to_string(),
                status: "current".to_string(),
                created_at: None,
            }],
        }}));
        projection.apply_delta(delta(2, RuntimeDeltaKind::RoleArchive { role_id: "role-1".to_string(), archived_at: Some("now".to_string()) }));
        assert_eq!(projection.roles[0].status, "archived");
    }

    #[test]
    fn command_registry_update_and_disable() {
        let mut projection = RuntimeProjection::default();
        projection.apply_delta(delta(1, RuntimeDeltaKind::CommandRegistryUpsert { command: CommandRegistrySummary {
            id: "command-1".to_string(),
            action_id: "cmd.rg.run".to_string(),
            scope_type: "global".to_string(),
            project_key: None,
            enabled: true,
            current_version_id: Some("version-1".to_string()),
            command_version: Some(1),
            binary_name: Some("rg".to_string()),
            starlark_object: Some("rg".to_string()),
            starlark_method: Some("run".to_string()),
            argv_template: vec!["--files".to_string()],
            default_cwd: Some(".".to_string()),
            cwd_policy: Some("project".to_string()),
            env_policy: Some("inherit".to_string()),
            stdin_policy: Some("deny".to_string()),
            sync_allowed: Some(true),
            async_allowed: Some(false),
            max_runtime_ms: Some(30000),
            end_of_turn_behavior: Some("wait".to_string()),
            end_of_session_behavior: Some("terminate".to_string()),
            mutation_class: Some("readOnly".to_string()),
            model_description: Some("Search files".to_string()),
            allow_cwd_arg: Some(false),
            allow_args_arg: Some(true),
            forbidden_args: vec!["--pcre2".to_string()],
            execution_policy: Some("allow".to_string()),
            updated_at: None,
        }}));
        projection.apply_delta(delta(2, RuntimeDeltaKind::CommandRegistryDisable { command_id: "command-1".to_string() }));
        assert!(!projection.command_registry[0].enabled);
    }

    #[test]
    fn workflow_memory_update_and_event() {
        let mut projection = RuntimeProjection::default();
        projection.apply_delta(delta(1, RuntimeDeltaKind::WorkflowMemoryUpsert { memory: WorkflowMemorySummary {
            id: "memory-1".to_string(),
            session_id: "session-1".to_string(),
            source_script_run_id: Some("script-1".to_string()),
            scope_type: "project".to_string(),
            project_key: Some("project".to_string()),
            title: "Memory".to_string(),
            reason: "Useful".to_string(),
            summary: "Summary".to_string(),
            helpful_score: 1.0,
            promoted_at: None,
            source_preview: "output(\"hello\")".to_string(),
            source_starlark: Some("output(\"hello\")".to_string()),
            provider: Some("provider".to_string()),
            model: Some("model".to_string()),
            dimensions: Some(2560),
            storage_type: Some("halfvec".to_string()),
            source_hash: Some("hash".to_string()),
            command_fingerprint: Some("fingerprint".to_string()),
            recent_events: Vec::new(),
        }}));
        projection.apply_delta(delta(2, RuntimeDeltaKind::WorkflowMemoryEvent { item: event(2, "workflow_memory", "memory-1", "workflow_memory.promoted") }));
        assert_eq!(projection.workflow_memories.len(), 1);
        assert_eq!(projection.timeline.len(), 1);
    }

    #[test]
    fn turn_tool_script_and_process_status_changes_update_timeline() {
        let mut projection = RuntimeProjection::default();
        projection.apply_deltas(vec![
            delta(1, RuntimeDeltaKind::TimelineAppend { item: event(1, "turn", "turn-1", "turn.started") }),
            delta(2, RuntimeDeltaKind::TimelineAppend { item: event(2, "tool", "tool-1", "tool.started") }),
            delta(3, RuntimeDeltaKind::TimelineAppend { item: event(3, "script", "script-1", "script.started") }),
            delta(4, RuntimeDeltaKind::TimelineAppend { item: event(4, "process", "process-1", "process.started") }),
            delta(5, RuntimeDeltaKind::TurnStatusChanged { turn_id: "turn-1".to_string(), status: "completed".to_string() }),
            delta(6, RuntimeDeltaKind::ToolStatusChanged { tool_call_id: "tool-1".to_string(), status: "completed".to_string() }),
            delta(7, RuntimeDeltaKind::ScriptStatusChanged { script_run_id: "script-1".to_string(), status: "completed".to_string() }),
            delta(8, RuntimeDeltaKind::ProcessStatusChanged { process_id: "process-1".to_string(), status: "lost".to_string() }),
        ]);
        let statuses = projection
            .timeline
            .iter()
            .map(|item| item.status.as_deref().unwrap_or(""))
            .collect::<Vec<_>>();
        assert_eq!(statuses, vec!["completed", "completed", "completed", "lost"]);
    }

    #[test]
    fn stale_watermark_is_rejected() {
        let mut projection = RuntimeProjection::default();
        projection.apply_delta(delta(10, RuntimeDeltaKind::SessionUpsert { session: session("session-1") }));
        let before = projection.clone();
        assert_eq!(projection.apply_delta(delta(9, RuntimeDeltaKind::SessionUpsert { session: session("session-2") })), ApplyOutcome::Stale);
        assert_eq!(projection, before);
    }

    #[test]
    fn resync_required_on_gap() {
        let mut projection = RuntimeProjection::default();
        projection.apply_delta(delta(1, RuntimeDeltaKind::SessionUpsert { session: session("session-1") }));
        assert_eq!(projection.apply_delta(delta(3, RuntimeDeltaKind::SessionUpsert { session: session("session-2") })), ApplyOutcome::ResyncRequired);
        assert!(projection.resync_required.as_ref().is_some_and(|state| state.required));
        assert_eq!(projection.sessions.len(), 1);
    }

    #[test]
    fn many_deltas_apply_deterministically() {
        let mut left = RuntimeProjection::default();
        let mut right = RuntimeProjection::default();
        let deltas = vec![
            delta(1, RuntimeDeltaKind::SessionUpsert { session: session("session-1") }),
            delta(2, RuntimeDeltaKind::TimelineAppend { item: event(2, "turn", "turn-1", "turn.started") }),
        ];
        left.apply_deltas(deltas.clone());
        for delta in deltas {
            right.apply_delta(delta);
        }
        assert_eq!(left, right);
    }

    #[test]
    fn gui_controller_state_is_local_only_and_separate_from_projection() {
        let mut projection = RuntimeProjection::default();
        projection.apply_delta(delta(1, RuntimeDeltaKind::SessionUpsert { session: session("session-1") }));
        let mut local = GuiControllerState::default();
        local.draft_inputs.insert("session-1".to_string(), "draft message".to_string());
        local.select_workflow_memory(Some("memory-1".to_string()));
        local.active_operations.push(GuiOperationState {
            operation_id: "op-1".to_string(),
            operation: GuiOperationName::SendMessage,
            status: GuiOperationStatus::Pending,
            target_id: Some("session-1".to_string()),
            started_at: None,
            completed_at: None,
            error: None,
        });
        let projection_json = serde_json::to_value(&projection).expect("projection json");
        let local_json = serde_json::to_value(&local).expect("local json");
        assert!(projection_json.get("draftInputs").is_none());
        assert!(projection_json.get("selectedWorkflowMemoryId").is_none());
        assert!(projection_json.get("activeOperations").is_none());
        assert_eq!(local_json["draftInputs"]["session-1"], "draft message");
        assert_eq!(local_json["selectedWorkflowMemoryId"], "memory-1");
        assert_eq!(projection.sessions[0].id, "session-1");
    }

    #[test]
    fn selected_session_switch_requires_rehydrate_and_reconnect() {
        let mut local = GuiControllerState::default();
        let effect = local.select_session(Some("session-2".to_string()));
        assert_eq!(effect, GuiOperationExpectation::RehydrateAndReconnect);
        assert_eq!(local.selected_session_id.as_deref(), Some("session-2"));
        assert!(local.pending_rehydrate);
        assert!(local.pending_reconnect);

        let request = GuiOperationRequest::SelectSession {
            session_id: Some("session-2".to_string()),
        };
        assert_eq!(request.expected_projection_effect(), GuiOperationExpectation::RehydrateAndReconnect);
    }

    #[test]
    fn selected_workflow_memory_is_local_controller_state() {
        let mut local = GuiControllerState::default();
        let effect = local.select_workflow_memory(Some("memory-2".to_string()));
        assert_eq!(effect, GuiOperationExpectation::UpdateLocalState);
        assert_eq!(local.selected_workflow_memory_id.as_deref(), Some("memory-2"));

        let request = GuiOperationRequest::SelectWorkflowMemory {
            memory_id: Some("memory-2".to_string()),
        };
        assert_eq!(request.name(), GuiOperationName::SelectWorkflowMemory);
        assert_eq!(request.expected_projection_effect(), GuiOperationExpectation::UpdateLocalState);
        assert!(request.api_mapping().local_only);
    }

    #[test]
    fn operation_contracts_serde_and_error_packets_are_typed() {
        let request = GuiOperationRequest::DecideApproval {
            approval_id: "approval-1".to_string(),
            decision: "approved".to_string(),
            reason: "ok".to_string(),
        };
        let encoded = serde_json::to_string(&request).expect("request json");
        let decoded: GuiOperationRequest = serde_json::from_str(&encoded).expect("request decode");
        assert_eq!(decoded.name(), GuiOperationName::DecideApproval);
        assert_eq!(decoded.expected_projection_effect(), GuiOperationExpectation::WaitForDelta);

        let error = ApiErrorPacket::new("conflict", "approval already decided", json!({"entity":"approval","id":"approval-1"}));
        let result = GuiOperationResult {
            operation_id: "op-approval".to_string(),
            operation: GuiOperationName::DecideApproval,
            expectation: GuiOperationExpectation::WaitForDelta,
            outcome: GuiOperationOutcome::Error { error: error.clone() },
        };
        let value = serde_json::to_value(&result).expect("result json");
        assert_eq!(value["outcome"]["status"], "error");
        assert_eq!(value["outcome"]["payload"]["error"]["error"]["code"], "conflict");
        let round_trip: GuiOperationResult = serde_json::from_value(value).expect("result decode");
        assert_eq!(round_trip, result);
    }

    #[test]
    fn resync_required_is_surfaced_to_gui_local_state() {
        let mut projection = RuntimeProjection::default();
        projection.apply_delta(delta(1, RuntimeDeltaKind::SessionUpsert { session: session("session-1") }));
        assert_eq!(projection.apply_delta(delta(3, RuntimeDeltaKind::SessionUpsert { session: session("session-2") })), ApplyOutcome::ResyncRequired);
        let resync = projection.resync_required.clone().expect("projection resync");
        let mut local = GuiControllerState::default();
        local.record_resync_required(resync.reason.clone(), resync.expected_watermark, resync.received_watermark);
        assert!(local.pending_rehydrate);
        assert!(local.pending_reconnect);
        assert_eq!(local.connection_state, GuiConnectionState::Reconnecting);
        assert_eq!(local.resync_required.as_ref().map(|state| state.reason.as_str()), Some("delta watermark gap detected"));
    }

    #[test]
    fn snapshot_plus_deltas_converges_through_reducer() {
        let mut from_snapshot = RuntimeProjection::default();
        let deltas = vec![
            delta(1, RuntimeDeltaKind::SessionUpsert { session: session("session-1") }),
            delta(2, RuntimeDeltaKind::ApprovalUpsert { approval: PendingApprovalSummary {
                id: "approval-1".to_string(),
                session_id: "session-1".to_string(),
                turn_id: None,
                action_name: "fs.write".to_string(),
                required_approver_kind: "owner".to_string(),
                status: "pending".to_string(),
                can_decide: true,
                can_resume: false,
                input_context: json!({"policy": {"decision": "approvalRequired"}}),
                created_at: None,
                decision_at: None,
                decision_reason: None,
                resumable_action_status: None,
            }}),
        ];
        assert_eq!(from_snapshot.apply_deltas(deltas), ApplyOutcome::Applied);

        let hydrated = RuntimeProjection {
            watermark: 2,
            sessions: vec![session("session-1")],
            pending_approvals: from_snapshot.pending_approvals.clone(),
            ..RuntimeProjection::default()
        };
        assert_eq!(from_snapshot, hydrated);
    }

    #[test]
    fn command_registry_request_contract_exposes_gui_enablement_without_policy_inference() {
        let summary = CommandRegistryRequestSummary {
            id: "request-1".to_string(),
            operation: "add".to_string(),
            action_id: "cmd.rg.run".to_string(),
            action_label: "rg · run".to_string(),
            status: "approved".to_string(),
            state_text: "Approved · ready to apply".to_string(),
            apply_status: "pending".to_string(),
            final_scope_type: Some("global".to_string()),
            final_project_key: None,
            scope_summary: Some("global".to_string()),
            final_policy: Some("allow".to_string()),
            policy_summary: Some("allow".to_string()),
            can_preview: false,
            preview_label: "Preview unavailable".to_string(),
            can_decide: false,
            decide_label: "Decision unavailable".to_string(),
            can_apply: true,
            apply_label: "Apply approved request".to_string(),
        };
        let result = GuiOperationResult {
            operation_id: "op-list".to_string(),
            operation: GuiOperationName::ListCommandRegistryRequests,
            expectation: GuiOperationExpectation::DirectResult,
            outcome: GuiOperationOutcome::CommandRegistryRequests { requests: vec![summary.clone()] },
        };
        let value = serde_json::to_value(&result).expect("operation result");
        assert_eq!(value["outcome"]["payload"]["requests"][0]["canApply"], true);
        assert_eq!(value["outcome"]["payload"]["requests"][0]["finalPolicy"], "allow");
        assert!(DART_FORBIDDEN_RESPONSIBILITIES.contains(&"commandPolicy"));
        assert!(DART_FORBIDDEN_RESPONSIBILITIES.contains(&"approvalAvailability"));
        assert!(DART_ALLOWED_EPHEMERAL_RESPONSIBILITIES.contains(&"textFields"));
        let decoded: GuiOperationResult = serde_json::from_value(value).expect("decode result");
        assert_eq!(decoded, result);
    }

    #[test]
    fn command_registry_request_projection_upsert_change_and_remove_is_typed() {
        let mut projection = RuntimeProjection::default();
        let pending = registry_request("request-1", "pending", "pending");
        assert_eq!(
            projection.apply_delta(delta(1, RuntimeDeltaKind::CommandRegistryRequestUpsert { request: pending.clone() })),
            ApplyOutcome::Applied
        );
        assert_eq!(projection.command_registry_requests, vec![pending.clone()]);
        assert_eq!(projection.command_registry_requests[0].state_text, "Needs registry decision");
        assert!(projection.command_registry_requests[0].can_preview);
        assert!(projection.command_registry_requests[0].can_decide);
        assert!(!projection.command_registry_requests[0].can_apply);

        let approved = registry_request("request-1", "approved", "pending");
        assert_eq!(
            projection.apply_delta(delta(2, RuntimeDeltaKind::CommandRegistryRequestUpsert { request: approved.clone() })),
            ApplyOutcome::Applied
        );
        assert_eq!(projection.command_registry_requests.len(), 1);
        assert_eq!(projection.command_registry_requests[0].state_text, "Approved · ready to apply");
        assert!(!projection.command_registry_requests[0].can_decide);
        assert!(projection.command_registry_requests[0].can_apply);
        assert_eq!(projection.command_registry_requests[0].scope_summary.as_deref(), Some("project:alpha"));
        assert_eq!(projection.command_registry_requests[0].policy_summary.as_deref(), Some("ownerApproval"));

        assert_eq!(
            projection.apply_delta(delta(3, RuntimeDeltaKind::CommandRegistryRequestRemove { request_id: "request-1".to_string() })),
            ApplyOutcome::Applied
        );
        assert!(projection.command_registry_requests.is_empty());
    }

    #[test]
    fn pending_approval_summary_exposes_control_enablement_without_raw_payload_inference() {
        let approval = PendingApprovalSummary {
            id: "approval-1".to_string(),
            session_id: "session-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            action_name: "fs.write".to_string(),
            required_approver_kind: "owner".to_string(),
            status: "pending".to_string(),
            can_decide: true,
            can_resume: false,
            input_context: json!({
                "policy": {"decision": "approvalRequired"},
                "rawDiagnosticOnly": {"nested": ["not", "a", "control", "contract"]}
            }),
            created_at: None,
            decision_at: None,
            decision_reason: None,
            resumable_action_status: None,
        };
        let mut projection = RuntimeProjection::default();
        projection.apply_delta(delta(1, RuntimeDeltaKind::ApprovalUpsert { approval: approval.clone() }));
        let value = serde_json::to_value(&projection.pending_approvals[0]).expect("approval json");
        assert_eq!(value["canDecide"], true);
        assert_eq!(value["canResume"], false);
        assert_eq!(value["requiredApproverKind"], "owner");
        assert!(value.get("inputContext").is_some(), "raw context remains available for inspection panes");
        assert!(DART_FORBIDDEN_RESPONSIBILITIES.contains(&"approvalAvailability"));

        let decoded: PendingApprovalSummary = serde_json::from_value(value).expect("approval decode");
        assert_eq!(decoded, approval);

        let resumable = PendingApprovalSummary {
            status: "approved".to_string(),
            can_decide: false,
            can_resume: true,
            ..approval
        };
        let resumable_value = serde_json::to_value(&resumable).expect("resumable approval json");
        assert_eq!(resumable_value["canDecide"], false);
        assert_eq!(resumable_value["canResume"], true);
    }

    #[test]
    fn every_gui_operation_has_api_mapping_and_expected_effect() {
        let samples = operation_samples();
        assert_eq!(samples.len(), GUI_OPERATION_VARIANT_COUNT);
        for request in samples {
            let mapping = request.api_mapping();
            assert_eq!(mapping.operation, request.name());
            assert_eq!(mapping.expected_projection_effect, request.expected_projection_effect());
            assert!(!mapping.route_or_action.is_empty());
            assert!(!mapping.request_shape.is_empty());
            assert!(!mapping.response_shape.is_empty());
            assert!(!mapping.error_shape.is_empty());
            match request {
                GuiOperationRequest::Connect { .. }
                | GuiOperationRequest::Disconnect
                | GuiOperationRequest::SelectSession { .. }
                | GuiOperationRequest::SelectWorkflowMemory { .. }
                | GuiOperationRequest::UpdateRuntimeSettings { .. } => {
                    assert!(mapping.local_only);
                    assert!(mapping.method.is_none());
                }
                _ => {
                    assert!(!mapping.local_only);
                    assert!(mapping.method.is_some());
                    assert!(mapping.error_shape.contains("error"));
                }
            }
        }
    }

    #[test]
    fn gui_operation_server_json_matches_unambiguous_server_shapes() {
        let approval = GuiOperationRequest::DecideApproval {
            approval_id: "approval-1".to_string(),
            decision: "approved".to_string(),
            reason: "explicit reason required by server".to_string(),
        };
        assert_eq!(
            approval.to_server_request_json().expect("approval server json"),
            json!({"decision":"approved","reason":"explicit reason required by server"})
        );

        let decision = GuiOperationRequest::DecideCommandRegistryRequest {
            request_id: "request-1".to_string(),
            decision: registry_decision(),
        };
        let server_json = decision.to_server_request_json().expect("registry decision server json");
        assert_eq!(server_json["sessionId"], "session-1");
        assert_eq!(server_json["status"], "approved");
        assert_eq!(server_json["finalScope"]["scopeType"], "global");
        assert_eq!(server_json["finalExecutionPolicy"]["decision"], "allow");
        assert_eq!(server_json["finalCommand"]["actionId"], "cmd.rg.audit");
        assert!(server_json.get("finalProject").is_none());
        assert!(server_json.get("finalPolicy").is_none());
    }

    #[test]
    fn command_registry_request_summary_maps_raw_server_response_to_control_enablement() {
        let raw = json!({
            "id": "request-1",
            "operation": "add",
            "proposedCommand": {"actionId": "cmd.rg.audit"},
            "approvalStatus": "approved",
            "applicationStatus": "pending",
            "finalScope": {"scopeType": "project", "projectKey": "alpha"},
            "finalExecutionPolicy": {"decision": "ownerApproval"}
        });
        let summary = CommandRegistryRequestSummary::from_server_value(&raw).expect("summary");
        assert_eq!(summary.action_id, "cmd.rg.audit");
        assert_eq!(summary.action_label, "rg · audit");
        assert_eq!(summary.state_text, "Approved · ready to apply");
        assert_eq!(summary.final_scope_type.as_deref(), Some("project"));
        assert_eq!(summary.final_project_key.as_deref(), Some("alpha"));
        assert_eq!(summary.scope_summary.as_deref(), Some("project:alpha"));
        assert_eq!(summary.final_policy.as_deref(), Some("ownerApproval"));
        assert_eq!(summary.policy_summary.as_deref(), Some("ownerApproval"));
        assert!(!summary.can_preview);
        assert!(!summary.can_decide);
        assert!(summary.can_apply);
        assert_eq!(summary.apply_label, "Apply approved request");

        let pending = json!({
            "id": "request-2",
            "operation": "disable",
            "proposedCommand": {"actionId": "cmd.rg.audit"},
            "approvalStatus": "pending",
            "applicationStatus": "pending"
        });
        let pending_summary = CommandRegistryRequestSummary::from_server_value(&pending).expect("pending summary");
        assert_eq!(pending_summary.state_text, "Needs registry decision");
        assert!(pending_summary.can_preview);
        assert_eq!(pending_summary.preview_label, "Preview decision");
        assert!(pending_summary.can_decide);
        assert_eq!(pending_summary.decide_label, "Decide request");
        assert!(!pending_summary.can_apply);
    }

    fn chat_entry(id: &str, author: &str, body: &str, is_tool: bool, status: &str) -> AgentRuntimeChatEntry {
        AgentRuntimeChatEntry {
            id: id.to_string(),
            author: author.to_string(),
            display_label: author.to_string(),
            timestamp: None,
            body: body.to_string(),
            subtitle: status.to_string(),
            kind: if is_tool { "execute_code".to_string() } else { "message".to_string() },
            status: status.to_string(),
            process_id: if is_tool { Some("proc-1".to_string()) } else { None },
            command: if is_tool { "output('delta')".to_string() } else { String::new() },
            output: if is_tool { "partial output".to_string() } else { String::new() },
            delivery_state: if status == "completed" { "delivered".to_string() } else { "streaming".to_string() },
            is_streaming: status == "running",
            is_tool,
        }
    }

    #[test]
    fn selected_chat_semantic_deltas_update_user_assistant_tool_and_final_without_full_snapshot() {
        let mut projection = RuntimeProjection::default();
        projection.selected_chat_entries = (0..50)
            .map(|index| chat_entry(&format!("old-{index}"), "Assistant", "old", false, "completed"))
            .collect();
        let mut diagnostics = AgentRuntimeChatTransportDiagnostics::default();
        let snapshot_payload = serde_json::to_vec(&projection.selected_chat_entries).expect("snapshot bytes");
        diagnostics.record_snapshot(snapshot_payload.len(), projection.selected_chat_entries.len());

        let deltas = [
            RuntimeDeltaKind::SelectedChatAppend { entry: chat_entry("turn-1-user", "User", "exact user composer text", false, "completed") },
            RuntimeDeltaKind::SelectedChatAppend { entry: chat_entry("tool-1", "Tool", "", true, "running") },
            RuntimeDeltaKind::SelectedChatUpdate { entry: chat_entry("assistant-1", "Assistant", "partial assistant", false, "running") },
            RuntimeDeltaKind::SelectedChatUpdate { entry: chat_entry("tool-1", "Tool", "", true, "completed") },
            RuntimeDeltaKind::SelectedChatUpdate { entry: chat_entry("assistant-1", "Assistant", "complete assistant final", false, "completed") },
            RuntimeDeltaKind::SelectedChatFinalize { entry_id: "assistant-1".to_string(), delivery_state: "delivered".to_string(), status: "completed".to_string() },
        ];
        let modal_generation = 7_u64;
        let rail_generation = 11_u64;
        for (index, delta_kind) in deltas.into_iter().enumerate() {
            let runtime_delta = delta((index + 1) as i64, delta_kind);
            let encoded = serde_json::to_vec(&runtime_delta).expect("delta bytes");
            assert!(!String::from_utf8_lossy(&encoded).contains("old-0"), "semantic delta must not carry latest-50 snapshot");
            diagnostics.record_delta(encoded.len(), projection.selected_chat_entries.len(), true);
            assert_eq!(projection.apply_delta(runtime_delta), ApplyOutcome::Applied);
            assert!(projection.selected_chat_entries.len() <= 50);
            assert_eq!(modal_generation, 7, "streaming delta must not rebuild unrelated modal surfaces");
            assert_eq!(rail_generation, 11, "streaming delta must not rebuild unrelated rail surfaces");
        }
        for _ in 0..990 {
            diagnostics.record_dropped_intermediate();
        }

        assert!(projection.selected_chat_entries.iter().any(|entry| entry.id == "turn-1-user" && entry.author == "User" && entry.body == "exact user composer text"));
        assert!(projection.selected_chat_entries.iter().any(|entry| entry.id == "assistant-1" && entry.author == "Assistant" && entry.body == "complete assistant final" && !entry.is_streaming));
        assert!(projection.selected_chat_entries.iter().any(|entry| entry.id == "tool-1" && entry.author == "Tool" && entry.is_tool && entry.status == "completed" && entry.command.contains("output")));
        println!(
            "agent_runtime_selected_chat_delta_counters full_snapshot_count={} delta_count={} average_payload_bytes={} max_payload_bytes={} selected_chat_entry_count={} coalesced_payload_frequency={} dropped_intermediate_payload_count={} unrelated_modal_rebuilds={} unrelated_rail_rebuilds={}",
            diagnostics.full_snapshot_count,
            diagnostics.delta_count,
            diagnostics.average_payload_bytes(),
            diagnostics.max_payload_bytes,
            projection.selected_chat_entries.len(),
            diagnostics.coalesced_payload_count,
            diagnostics.dropped_intermediate_payload_count,
            diagnostics.unrelated_modal_rebuild_count,
            diagnostics.unrelated_rail_rebuild_count,
        );
        assert_eq!(diagnostics.full_snapshot_count, 1);
        assert_eq!(diagnostics.delta_count, 6);
        assert_eq!(diagnostics.coalesced_payload_count, 6);
        assert!(diagnostics.dropped_intermediate_payload_count > 0);
        assert_eq!(diagnostics.unrelated_modal_rebuild_count, 0);
        assert_eq!(diagnostics.unrelated_rail_rebuild_count, 0);
    }
}
