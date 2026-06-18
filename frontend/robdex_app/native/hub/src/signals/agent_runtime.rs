use rinf::{DartSignal, RustSignal, SignalPiece};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, DartSignal)]
pub struct AgentRuntimeRequestSignal {
    pub request_id: String,
    pub request: AgentRuntimeRequest,
}

#[derive(Clone, Debug, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub enum AgentRuntimeRequest {
    RefreshDiscovery { discovery_path: String },
    RefreshIcloudRemoteDiscovery { profile_path: String },
    ImportRemoteProfileDocument { profile_path: String },
    RefreshImportedRemoteProfile,
    ConnectDiscoveredRuntime { discovery_path: String, selected_session_id: String },
    ConnectIcloudRemoteRuntime { profile_path: String, selected_session_id: String },
    ConnectImportedRemoteRuntime { selected_session_id: String },
    Connect { base_url: String, selected_session_id: String },
    SelectProject { project_id: String },
    Hydrate { selected_session_id: String },
    Rehydrate { selected_session_id: String },
    PollStreamOnce,
    Disconnect,
    DispatchOperation { operation: AgentRuntimeGuiOperation },
}

#[derive(Clone, Debug, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub enum AgentRuntimeGuiOperation {
    SelectSession { session_id: String },
    SelectWorkflowMemory { memory_id: String },
    CreateSession { role: String, project: String, workdir: String, worktree_root: String, title: String, name: String },
    SendMessage { session_id: String, message: String },
    TerminateProcess { session_id: String, handle: String },
    InputProcess { session_id: String, handle: String, text: String },
    FlushProcess { session_id: String, handle: String },
    CloseSession { session_id: String, reason: String },
    ArchiveSession { session_id: String },
    ForkSession { session_id: String, at_turn: String },
    DecideApproval { approval_id: String, decision: String, reason: String },
    ResumeApproval { approval_id: String },
    ListCommandRegistry { session_id: String, project_key: String },
    ShowCommand { action_id: String, session_id: String, project_key: String },
    ListCommandRegistryRequests,
    ShowCommandRegistryRequest { request_id: String },
    PreviewCommandRegistryRequest { request_id: String, decision: AgentRuntimeCommandRegistryDecisionInput },
    DecideCommandRegistryRequest { request_id: String, decision: AgentRuntimeCommandRegistryDecisionInput },
    ApplyCommandRegistryRequest { request_id: String, session_id: String },
    WorkflowMemoryFeedback { memory_id: String, session_id: String, feedback: String, payload: AgentRuntimeWorkflowMemoryFeedbackPayload },
    RoleEditorOptions,
    ValidateRoleDraft { draft: AgentRuntimeRoleEditorDraft },
    CreateRoleFromDraft { draft: AgentRuntimeRoleEditorDraft },
    UpdateRoleFromDraft { role_id: String, draft: AgentRuntimeRoleEditorDraft },
    ShowRoleDetail { role_id: String },
    ListRoleVersions { role_id: String },
    ShowRoleVersion { version_id: String },
    ExportRole { role_id: String },
    ActivateRoleVersion { role_id: String, version_id: String },
    ArchiveRole { role_id: String },
    UnarchiveRole { role_id: String },
}

#[derive(Clone, Debug, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeWorkflowMemoryFeedbackPayload {
    pub source: String,
    pub reason: String,
    pub variant: bool,
    pub has_variant: bool,
}

#[derive(Clone, Debug, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeCommandRegistryDecisionInput {
    pub session_id: String,
    pub status: String,
    pub final_scope: AgentRuntimeRegistryScope,
    pub has_final_scope: bool,
    pub final_execution_policy: AgentRuntimeFinalExecutionPolicy,
    pub has_final_execution_policy: bool,
    pub final_command: AgentRuntimeCommandSeed,
    pub has_final_command: bool,
}

#[derive(Clone, Debug, Default, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRegistryScope {
    pub scope_type: String,
    pub project_key: String,
}

#[derive(Clone, Debug, Default, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeFinalExecutionPolicy {
    pub decision: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeCommandSeed {
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
    pub max_runtime_ms: i64,
    pub has_max_runtime_ms: bool,
    pub end_of_turn_behavior: String,
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

#[derive(Clone, Debug, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleEditorDraft {
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub model_defaults: AgentRuntimeRoleEditorModelDefaults,
    pub instruction_text: String,
    pub capabilities: Vec<String>,
    pub policy_entries: Vec<AgentRuntimeRolePolicyEntry>,
    pub routing: AgentRuntimeRoleEditorRoutingMetadata,
    pub visibility: AgentRuntimeRoleEditorVisibilityMetadata,
    pub lifecycle_authority: AgentRuntimeRoleEditorLifecycleAuthorityMetadata,
}

#[derive(Clone, Debug, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRolePolicyEntry {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleEditorModelDefaults {
    pub model: String,
    pub reasoning_effort: String,
}

#[derive(Clone, Debug, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleEditorRoutingMetadata {
    pub mode: String,
    pub default_recipient: String,
    pub has_default_recipient: bool,
    pub allowed_recipients: Vec<String>,
    pub reserved_actions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleEditorVisibilityMetadata {
    pub listed: bool,
    pub owner_visible: bool,
}

#[derive(Clone, Debug, Deserialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleEditorLifecycleAuthorityMetadata {
    pub can_spawn_agents: bool,
    pub can_archive_agents: bool,
    pub reserved_actions: Vec<String>,
}

#[derive(Serialize, RustSignal)]
pub struct AgentRuntimeOutputSignal {
    pub request_id: String,
    pub output: AgentRuntimeOutput,
}

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub enum AgentRuntimeOutput {
    ProjectionSnapshot { projection: AgentRuntimeProjectionSnapshot },
    ControllerState { controller_state: AgentRuntimeControllerState },
    OperationResult { result: AgentRuntimeOperationResult },
    StreamOutcome { outcome: AgentRuntimeStreamOutcome, projection: AgentRuntimeProjectionSnapshot, has_projection: bool, controller_state: AgentRuntimeControllerState },
    Error { error: AgentRuntimeApiError },
    ControlTowerView { view_model: AgentRuntimeControlTowerViewModel },
}

#[derive(Clone, Debug, Default, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeProjectionSnapshot {
    pub watermark: i64,
    pub session_count: i64,
    pub timeline_count: i64,
    pub action_count: i64,
    pub role_count: i64,
    pub workflow_memory_count: i64,
}

#[derive(Clone, Debug, Default, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControllerState {
    pub connection_state: String,
    pub selected_session_id: String,
    pub has_selected_session_id: bool,
    pub base_url: String,
    pub last_error: String,
    pub has_last_error: bool,
}

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeOperationResult {
    pub operation: String,
    pub outcome: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub enum AgentRuntimeStreamOutcome {
    Hello { watermark: i64, runtime_identity: String, has_runtime_identity: bool },
    DeltaApplied { apply_outcome: String },
    ResyncRequired { reason: String, has_reason: bool },
    ServerShutdown,
    StreamClosed,
}

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeApiError {
    pub code: String,
    pub message: String,
    pub details: Vec<AgentRuntimeFact>,
}

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControlTowerViewModel {
    pub discovery: AgentRuntimeDiscoveryView,
    pub remote_discovery: AgentRuntimeDiscoveryView,
    pub imported_remote_discovery: AgentRuntimeDiscoveryView,
    pub connection_state: String,
    pub connection_tone: String,
    pub base_url: String,
    pub status_label: String,
    pub watermark_label: String,
    pub status_badges: Vec<AgentRuntimeBadge>,
    pub selected_session_label: String,
    pub sessions_title: String,
    pub sessions_subtitle: String,
    pub timeline_title: String,
    pub timeline_subtitle: String,
    pub actions_title: String,
    pub actions_subtitle: String,
    pub detail_title: String,
    pub detail_subtitle: String,
    pub sessions_empty_title: String,
    pub sessions_empty_text: String,
    pub timeline_empty_title: String,
    pub timeline_empty_text: String,
    pub actions_empty_title: String,
    pub actions_empty_text: String,
    pub sessions: Vec<AgentRuntimeSessionRow>,
    pub timeline: Vec<AgentRuntimeTimelineRow>,
    pub actions: Vec<AgentRuntimeActionRow>,
    pub role_admin: AgentRuntimeRoleAdminView,
    pub workflow_memory: AgentRuntimeWorkflowMemoryView,
    pub controller_facts: Vec<AgentRuntimeFact>,
    pub output_log: Vec<String>,
    pub pending_request_count: i64,
    pub error_message: String,
    pub has_error_message: bool,
    pub shell: AgentRuntimeConversationShellViewModel,
}

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeConversationShellViewModel {
    pub projects: Vec<AgentRuntimeShellProjectRow>,
    pub sessions: Vec<AgentRuntimeSessionRow>,
    pub selected_session_id: String,
    pub has_selected_session_id: bool,
    pub selected_conversation: Vec<AgentRuntimeTimelineRow>,
    pub dynamic_roles: Vec<AgentRuntimeShellRolePresentation>,
    pub actions: Vec<AgentRuntimeActionRow>,
    pub settings: Vec<AgentRuntimeFact>,
    pub role_management: AgentRuntimeRoleAdminView,
    pub workflow_memory: AgentRuntimeWorkflowMemoryView,
    pub command_registry_requests: Vec<AgentRuntimeActionRow>,
    pub approvals: Vec<AgentRuntimeActionRow>,
    pub diagnostics: Vec<AgentRuntimeFact>,
    pub operation_surfaces: Vec<AgentRuntimeOperationSurface>,
}

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeShellProjectRow { pub id: String, pub title: String, pub subtitle: String, pub selectable: bool }

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeShellRolePresentation { pub role_id: String, pub display_label: String, pub short_label: String, pub tone: String, pub description: String }

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeOperationSurface {
    pub surface_id: String,
    pub title: String,
    pub subtitle: String,
    pub rows: Vec<AgentRuntimeFact>,
    pub actions: Vec<AgentRuntimeActionRow>,
}

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeDiscoveryView {
    pub source_type: String,
    pub source_path: String,
    pub state: String,
    pub tone: String,
    pub title: String,
    pub message: String,
    pub base_url: String,
    pub has_base_url: bool,
    pub health_url: String,
    pub has_health_url: bool,
    pub web_socket_url: String,
    pub has_web_socket_url: bool,
    pub runtime_identity: String,
    pub has_runtime_identity: bool,
    pub discovery_path: String,
    pub last_imported_at: String,
    pub has_last_imported_at: bool,
    pub service_state: String,
    pub has_service_state: bool,
    pub connectable: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeSessionRow { pub id: String, pub title: String, pub status: String, pub subtitle: String, pub group_label: String, pub tone: String }
#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeTimelineRow { pub id: String, pub title: String, pub subtitle: String, pub status: String, pub tone: String }
#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeActionRow { pub id: String, pub title: String, pub subtitle: String, pub kind: String, pub state_text: String, pub tone: String }
#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeFact { pub label: String, pub value: String }
#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeBadge { pub label: String, pub value: String, pub tone: String }

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleAdminView {
    pub title: String,
    pub subtitle: String,
    pub empty_title: String,
    pub empty_text: String,
    pub rows: Vec<AgentRuntimeRoleRow>,
    pub selected_detail: AgentRuntimeRoleDetail,
    pub has_selected_detail: bool,
    pub version_rows: Vec<AgentRuntimeRoleVersionRow>,
    pub editor_draft: AgentRuntimeRoleEditorDraftView,
    pub has_editor_draft: bool,
    pub validation_errors: Vec<String>,
    pub action_states: Vec<AgentRuntimeActionRow>,
}

#[derive(Clone, Debug, Default, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleRow { pub id: String, pub title: String, pub subtitle: String, pub status: String, pub tone: String, pub current_version: String }
#[derive(Clone, Debug, Default, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleDetail { pub id: String, pub title: String, pub display_name: String, pub version: String, pub status: String, pub instructions_preview: String, pub model_label: String, pub routing_label: String, pub visibility_label: String, pub lifecycle_label: String, pub policy_rows: Vec<AgentRuntimeRolePolicyRow> }
#[derive(Clone, Debug, Default, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRolePolicyRow { pub label: String, pub value: String }
#[derive(Clone, Debug, Default, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleVersionRow { pub version_id: String, pub version: String, pub status: String, pub created_at: String, pub is_current: bool, pub can_activate: bool }
#[derive(Clone, Debug, Default, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleEditorDraftView { pub role_id: String, pub version: String, pub display_name: String, pub model: String, pub reasoning_effort: String, pub instruction_text: String, pub capabilities: Vec<String>, pub policy_rows: Vec<AgentRuntimeRolePolicyRow>, pub routing_mode: String, pub default_recipient: String, pub allowed_recipients: Vec<String>, pub listed: bool, pub owner_visible: bool, pub can_spawn_agents: bool, pub can_archive_agents: bool, pub can_validate: bool, pub can_create: bool, pub can_update: bool }

#[derive(Clone, Debug, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeWorkflowMemoryView { pub title: String, pub subtitle: String, pub empty_title: String, pub empty_text: String, pub rows: Vec<AgentRuntimeWorkflowMemoryRow>, pub selected_detail: AgentRuntimeWorkflowMemoryDetail, pub has_selected_detail: bool, pub action_states: Vec<AgentRuntimeActionRow> }
#[derive(Clone, Debug, Default, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeWorkflowMemoryRow { pub id: String, pub title: String, pub scope_label: String, pub project_key: String, pub has_project_key: bool, pub helpful_score: String, pub promoted_at: String, pub has_promoted_at: bool, pub source_session_id: String, pub reason: String, pub tone: String, pub is_selected: bool }
#[derive(Clone, Debug, Default, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeWorkflowMemoryDetail { pub id: String, pub title: String, pub reason: String, pub summary: String, pub source_session_id: String, pub source_script_run_id: String, pub has_source_script_run_id: bool, pub source_preview: String, pub provider: String, pub model: String, pub dimensions: String, pub storage_label: String, pub source_hash: String, pub command_fingerprint: String, pub score: String, pub scope_label: String, pub feedback_enabled: bool, pub feedback_session_id: String, pub has_feedback_session_id: bool, pub events: Vec<AgentRuntimeWorkflowMemoryEvent> }
#[derive(Clone, Debug, Default, Serialize, SignalPiece)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeWorkflowMemoryEvent { pub id: String, pub title: String, pub subtitle: String, pub created_at: String, pub tone: String }
