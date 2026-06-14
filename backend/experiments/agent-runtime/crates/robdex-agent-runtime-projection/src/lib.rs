use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub metadata: Value,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleSummary {
    pub id: String,
    pub display_name: String,
    pub current_version_id: Option<String>,
    pub status: String,
    pub model: Option<String>,
    pub archived_at: Option<String>,
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
    pub binary_name: Option<String>,
    pub starlark_object: Option<String>,
    pub starlark_method: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowMemorySummary {
    pub id: String,
    pub session_id: String,
    pub scope_type: String,
    pub project_key: Option<String>,
    pub title: String,
    pub reason: String,
    pub helpful_score: f64,
    pub promoted_at: Option<String>,
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
    pub sessions: Vec<SessionListItem>,
    pub selected_session: Option<SelectedSessionDetail>,
    pub timeline: Vec<TimelineItem>,
    pub pending_approvals: Vec<PendingApprovalSummary>,
    pub roles: Vec<RoleSummary>,
    pub command_registry: Vec<CommandRegistrySummary>,
    pub workflow_memories: Vec<WorkflowMemorySummary>,
    pub resync_required: Option<ResyncRequiredState>,
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
        self.pending_rehydrate = true;
        self.pending_reconnect = true;
        GuiOperationExpectation::RehydrateAndReconnect
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
    CreateSession,
    SendMessage,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", content = "request", rename_all = "camelCase")]
pub enum GuiOperationRequest {
    Connect { base_url: String, selected_session_id: Option<String> },
    Hydrate { selected_session_id: Option<String> },
    Rehydrate { selected_session_id: Option<String> },
    Disconnect,
    SelectSession { session_id: Option<String> },
    CreateSession { role: String, project: Option<String>, workdir: Option<String>, worktree_root: Option<String>, title: Option<String>, name: Option<String> },
    SendMessage { session_id: String, message: String },
    CloseSession { session_id: String, reason: Option<String> },
    ArchiveSession { session_id: String },
    ForkSession { session_id: String, at_turn: String },
    DecideApproval { approval_id: String, decision: String, reason: Option<String> },
    ResumeApproval { approval_id: String },
    ListCommandRegistry { session_id: Option<String>, project_key: Option<String> },
    ShowCommand { action_id: String, session_id: Option<String>, project_key: Option<String> },
    ListCommandRegistryRequests,
    ShowCommandRegistryRequest { request_id: String },
    PreviewCommandRegistryRequest { request_id: String, decision: CommandRegistryDecisionInput },
    DecideCommandRegistryRequest { request_id: String, decision: CommandRegistryDecisionInput },
    ApplyCommandRegistryRequest { request_id: String, session_id: String },
    WorkflowMemoryFeedback { memory_id: String, session_id: String, feedback: String, payload: Value },
}

impl GuiOperationRequest {
    pub fn name(&self) -> GuiOperationName {
        match self {
            Self::Connect { .. } => GuiOperationName::Connect,
            Self::Hydrate { .. } => GuiOperationName::Hydrate,
            Self::Rehydrate { .. } => GuiOperationName::Rehydrate,
            Self::Disconnect => GuiOperationName::Disconnect,
            Self::SelectSession { .. } => GuiOperationName::SelectSession,
            Self::CreateSession { .. } => GuiOperationName::CreateSession,
            Self::SendMessage { .. } => GuiOperationName::SendMessage,
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
        }
    }

    pub fn expected_projection_effect(&self) -> GuiOperationExpectation {
        match self {
            Self::Connect { .. } | Self::Hydrate { .. } | Self::Rehydrate { .. } => GuiOperationExpectation::Rehydrate,
            Self::Disconnect => GuiOperationExpectation::UpdateLocalState,
            Self::SelectSession { .. } => GuiOperationExpectation::RehydrateAndReconnect,
            Self::CreateSession { .. }
            | Self::SendMessage { .. }
            | Self::CloseSession { .. }
            | Self::ArchiveSession { .. }
            | Self::ForkSession { .. }
            | Self::DecideApproval { .. }
            | Self::ResumeApproval { .. }
            | Self::DecideCommandRegistryRequest { .. }
            | Self::ApplyCommandRegistryRequest { .. }
            | Self::WorkflowMemoryFeedback { .. } => GuiOperationExpectation::WaitForDelta,
            Self::ListCommandRegistry { .. }
            | Self::ShowCommand { .. }
            | Self::ListCommandRegistryRequests
            | Self::ShowCommandRegistryRequest { .. }
            | Self::PreviewCommandRegistryRequest { .. } => GuiOperationExpectation::DirectResult,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandRegistryDecisionInput {
    pub session_id: Option<String>,
    pub status: String,
    pub final_scope: Option<String>,
    pub final_project: Option<String>,
    pub final_policy: Option<String>,
    pub final_command: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandRegistryRequestSummary {
    pub id: String,
    pub operation: String,
    pub action_id: String,
    pub status: String,
    pub apply_status: String,
    pub final_scope_type: Option<String>,
    pub final_project_key: Option<String>,
    pub final_policy: Option<String>,
    pub can_preview: bool,
    pub can_decide: bool,
    pub can_apply: bool,
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

impl Default for RuntimeProjection {
    fn default() -> Self {
        Self {
            watermark: 0,
            server_status: ServerStatusProjection::default(),
            sessions: Vec::new(),
            selected_session: None,
            timeline: Vec::new(),
            pending_approvals: Vec::new(),
            roles: Vec::new(),
            command_registry: Vec::new(),
            workflow_memories: Vec::new(),
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
    WorkflowMemoryUpsert { memory: WorkflowMemorySummary },
    WorkflowMemoryEvent { item: TimelineItem },
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
        RuntimeDeltaKind::WorkflowMemoryUpsert { memory } => upsert_by(&mut projection.workflow_memories, memory, |item| item.id.as_str()),
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
            metadata: Value::Null,
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
            metadata: Value::Null,
        };
        projection.apply_delta(delta(1, RuntimeDeltaKind::SelectedSessionReplace { session: Some(detail.clone()) }));
        assert_eq!(projection.selected_session, Some(detail));
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
            archived_at: None,
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
            binary_name: Some("rg".to_string()),
            starlark_object: Some("rg".to_string()),
            starlark_method: Some("run".to_string()),
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
            scope_type: "project".to_string(),
            project_key: Some("project".to_string()),
            title: "Memory".to_string(),
            reason: "Useful".to_string(),
            helpful_score: 1.0,
            promoted_at: None,
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
        assert!(projection_json.get("activeOperations").is_none());
        assert_eq!(local_json["draftInputs"]["session-1"], "draft message");
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
    fn operation_contracts_serde_and_error_packets_are_typed() {
        let request = GuiOperationRequest::DecideApproval {
            approval_id: "approval-1".to_string(),
            decision: "approved".to_string(),
            reason: Some("ok".to_string()),
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
            status: "approved".to_string(),
            apply_status: "pending".to_string(),
            final_scope_type: Some("global".to_string()),
            final_project_key: None,
            final_policy: Some("allow".to_string()),
            can_preview: false,
            can_decide: false,
            can_apply: true,
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
}
