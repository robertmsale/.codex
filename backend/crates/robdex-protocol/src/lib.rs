use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RobdexAssistantFinalResponse {
    pub response: String,
    pub image_paths: Vec<String>,
}

pub fn robdex_assistant_final_response_schema() -> Value {
    let schema = schema_for!(RobdexAssistantFinalResponse);
    let mut schema =
        serde_json::to_value(schema.schema).expect("RobdexAssistantFinalResponse schema serializes");
    strip_schema_titles(&mut schema);
    schema
}

pub fn strict_robdex_assistant_final_response_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "response": {
                "type": "string"
            },
            "image_paths": {
                "type": "array",
                "items": {
                    "type": "string"
                }
            }
        },
        "required": [
            "image_paths",
            "response"
        ],
        "additionalProperties": false
    })
}

fn strip_schema_titles(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("title");
            for value in object.values_mut() {
                strip_schema_titles(value);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_schema_titles(item);
            }
        }
        _ => {}
    }
}

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
    pub service_tier: Option<String>,
    pub effective_sandbox_mode: Option<String>,
    pub effective_network_access: Option<bool>,
    pub effective_approval_policy: Option<String>,
    pub effective_model: Option<String>,
    pub effective_reasoning_effort: Option<String>,
    pub effective_service_tier: Option<String>,
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
    pub designer_default_model: Option<String>,
    pub designer_default_reasoning_effort: Option<String>,
    pub requirements_reviewer_default_model: Option<String>,
    pub requirements_reviewer_default_reasoning_effort: Option<String>,
    pub orchestrator_developer_instructions: Option<String>,
    pub worker_developer_instructions: Option<String>,
    pub qa_developer_instructions: Option<String>,
    pub designer_developer_instructions: Option<String>,
    pub operator_developer_instructions: Option<String>,
    pub hidden_developer_instructions: Option<String>,
    pub permanent_requirement_composables: Vec<String>,
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
    pub requirement_review: Option<UiRequirementReviewSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiRequirementReviewSummary {
    pub active_requirement_count: usize,
    pub stored_requirement_count: usize,
    pub requirement_set_active: bool,
    pub status: Option<String>,
    pub reviewer_thread_id: Option<String>,
    pub parent_thread_id: Option<String>,
    pub requirement_set_id: Option<String>,
    pub latest_claim_packet: Option<Value>,
    pub latest_verdict_packet: Option<Value>,
    pub passed_count: u32,
    pub failed_count: u32,
    pub blocked_count: u32,
    pub waiver_required_count: u32,
    pub unknown_count: u32,
    pub updated_at: Option<u64>,
    pub requirements: Vec<UiRequirementReviewRequirement>,
    pub verdicts: Vec<UiRequirementVerdictSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiRequirementReviewRequirement {
    pub key: String,
    pub statement: String,
    pub severity: String,
    pub verification_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiRequirementVerdictSummary {
    pub key: String,
    pub verdict: Option<String>,
    pub reason: Option<String>,
    pub evidence_assessment: Option<String>,
    pub required_correction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiChatEntry {
    pub id: String,
    pub author: String,
    pub display_label: String,
    pub timestamp: Option<u64>,
    pub body: String,
    pub subtitle: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub process_id: Option<String>,
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
pub struct UiLiveProcessItem {
    pub process_id: String,
    pub pid: Option<i64>,
    pub process_group_id: Option<i64>,
    pub command: String,
    pub cwd: Option<String>,
    pub started_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchViewData {
    pub projects: Vec<UiProjectItem>,
    pub selection: UiWorkspaceSelection,
    pub threads: Vec<UiThreadItem>,
    pub available_models: Vec<UiModelItem>,
    pub thread_groups: Vec<UiThreadGroupItem>,
    pub live_processes: Vec<UiLiveProcessItem>,
    pub chat_entries: Vec<UiChatEntry>,
    pub context_window_remaining_percent: Option<u32>,
    pub workspace_files: Vec<UiWorkspaceFile>,
    pub inspector_facts: Vec<UiInspectorFact>,
    pub pending_approvals: Vec<UiPendingApprovalItem>,
    pub worker_metadata: Option<UiWorkerMetadata>,
    pub requirement_review: Option<UiRequirementReviewSummary>,
    pub status_headline: String,
    pub status_detail: String,
    pub composer_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HookFailureNotice {
    pub project_id: String,
    pub project_name: String,
    pub thread_id: Option<String>,
    pub agent_name: String,
    pub role: String,
    pub event: String,
    pub status: String,
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use super::{
        RobdexAssistantFinalResponse, robdex_assistant_final_response_schema,
        strict_robdex_assistant_final_response_schema,
    };

    #[test]
    fn assistant_final_response_schema_matches_strict_openai_subset() {
        assert_eq!(
            robdex_assistant_final_response_schema(),
            strict_robdex_assistant_final_response_schema()
        );
    }

    #[test]
    fn writes_schema_validation_fixture_files() {
        let fixture_dir = std::path::PathBuf::from("/tmp/robdex-schema-validation");
        std::fs::create_dir_all(&fixture_dir).expect("create schema validation fixture dir");
        let schema_path = fixture_dir.join("schema.json");
        let instructions_path = fixture_dir.join("say_hi.md");

        let schema = robdex_assistant_final_response_schema();
        std::fs::write(
            &schema_path,
            serde_json::to_string_pretty(&schema).expect("serialize schema"),
        )
        .expect("write schema fixture");
        std::fs::write(&instructions_path, "say hi\n").expect("write minimal instructions fixture");

        let decoded: RobdexAssistantFinalResponse =
            serde_json::from_str(r#"{"response":"hi","image_paths":[]}"#)
                .expect("sample response decodes");
        assert_eq!(decoded.response, "hi");
        assert!(decoded.image_paths.is_empty());
        assert!(schema_path.exists());
        assert!(instructions_path.exists());
    }
}
