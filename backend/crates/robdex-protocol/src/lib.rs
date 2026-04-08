use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BridgeConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ThreadRunState {
    Idle,
    Running,
    Waiting,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub title: String,
    pub root_path: String,
    pub thread_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub role: String,
    pub run_state: ThreadRunState,
    pub unread_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadMessage {
    pub id: String,
    pub thread_id: String,
    pub role: String,
    pub text: String,
    pub created_at: u64,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSelection {
    pub thread_id: String,
    pub cwd: String,
    pub sandbox_mode: Option<String>,
    pub network_access: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub connection_status: BridgeConnectionStatus,
    pub selected_thread_id: Option<String>,
    pub selected_project_id: Option<String>,
    pub projects: Vec<ProjectSummary>,
    pub threads: Vec<ThreadSummary>,
    pub workspace: Option<WorkspaceSelection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeCommandEnvelope {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEventEnvelope {
    pub sequence: Option<u64>,
    pub name: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiWorkspaceSelection {
    pub project_id: Option<String>,
    pub project_root_path: Option<String>,
    pub project_orchestrator_thread_id: Option<String>,
    pub project_orchestrator_name: Option<String>,
    pub thread_id: Option<String>,
    pub thread_role: Option<String>,
    pub project_name: String,
    pub thread_name: String,
    pub connection_label: String,
    pub sandbox_mode: Option<String>,
    pub network_access: Option<bool>,
    pub approval_policy: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub effective_sandbox_mode: Option<String>,
    pub effective_network_access: Option<bool>,
    pub effective_approval_policy: Option<String>,
    pub effective_model: Option<String>,
    pub effective_reasoning_effort: Option<String>,
    pub is_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiProjectItem {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub default_cwd: String,
    pub auto_route_replies: bool,
    pub route_approval_requests: bool,
    pub preferred_model_provider: Option<String>,
    pub orchestrator_default_model: Option<String>,
    pub orchestrator_default_reasoning_effort: Option<String>,
    pub worker_default_model: Option<String>,
    pub worker_default_reasoning_effort: Option<String>,
    pub qa_default_model: Option<String>,
    pub qa_default_reasoning_effort: Option<String>,
    pub is_selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiThreadItem {
    pub id: String,
    pub title: String,
    pub role: String,
    pub project_name: String,
    pub preview: String,
    pub is_running: bool,
    pub unread_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiChatEntry {
    pub id: String,
    pub author: String,
    pub display_label: String,
    pub timestamp_label: String,
    pub body: String,
    pub subtitle: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub command: Option<String>,
    pub output: Option<String>,
    pub delivery_state: Option<String>,
    pub is_streaming: bool,
    pub is_tool: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiWorkspaceFile {
    pub path: String,
    pub kind: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiThreadGroupItem {
    pub id: String,
    pub title: String,
    pub thread_ids: Vec<String>,
    pub is_collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiWorkerMetadata {
    pub thread_id: String,
    pub issue_number: Option<u64>,
    pub pull_request_number: Option<u64>,
    pub blocked_reason: Option<String>,
    pub unblock_when: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiInspectorFact {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiPendingApprovalItem {
    pub id: String,
    pub thread_id: String,
    pub kind: String,
    pub title: String,
    pub detail: Option<String>,
    pub command: Option<String>,
    pub command_cwd: Option<String>,
    pub file_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiModelItem {
    pub id: String,
    pub name: Option<String>,
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchViewData {
    pub projects: Vec<UiProjectItem>,
    pub selection: UiWorkspaceSelection,
    pub threads: Vec<UiThreadItem>,
    pub available_models: Vec<UiModelItem>,
    pub thread_groups: Vec<UiThreadGroupItem>,
    pub chat_entries: Vec<UiChatEntry>,
    pub context_window_remaining_percent: Option<u32>,
    pub workspace_files: Vec<UiWorkspaceFile>,
    pub inspector_facts: Vec<UiInspectorFact>,
    pub pending_approvals: Vec<UiPendingApprovalItem>,
    pub worker_metadata: Option<UiWorkerMetadata>,
    pub status_headline: String,
    pub status_detail: String,
    pub composer_hint: String,
}
