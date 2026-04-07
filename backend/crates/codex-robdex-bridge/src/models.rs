use std::collections::BTreeMap;

use codex_app_server_adapter::app_server_protocol::RequestId;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                Ok(value)
            } else if let Some(value) = number.as_i64() {
                Ok(value.max(0) as u64)
            } else if let Some(value) = number.as_f64() {
                Ok(value.floor().max(0.0) as u64)
            } else {
                Err(serde::de::Error::custom("unsupported numeric timestamp"))
            }
        }
        Value::String(text) => {
            let trimmed = text.trim();
            if let Ok(value) = trimmed.parse::<u64>() {
                return Ok(value);
            }
            if let Ok(value) = trimmed.parse::<f64>() {
                return Ok(value.floor().max(0.0) as u64);
            }
            Err(serde::de::Error::custom("invalid timestamp string"))
        }
        other => Err(serde::de::Error::custom(format!("invalid timestamp value: {other}"))),
    }
}

pub const SERVER_NAME: &str = "codex-robdex-bridge";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_EVENT_HISTORY: usize = 4_000;
pub const MAX_TRANSPORT_MESSAGES_PER_THREAD: usize = 50;
pub const MAX_TRANSPORT_THREAD_MESSAGES_BYTES: usize = 850_000;
pub const MAX_MESSAGE_TEXT_CHARS: usize = 24_000;
pub const MAX_TOOL_OUTPUT_CHARS: usize = 12_000;
pub const BRIDGE_TRUNCATION_MARKER: &str = "\n\n[truncated by bridge]";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeInfo {
    pub protocol_version: u32,
    pub server_name: String,
    pub server_version: String,
    pub codex_version: String,
    pub app_server_url: String,
    pub state_json_path: String,
    pub sqlite_db_path: String,
    pub connection_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeSnapshot {
    pub state: Value,
    pub thread_cache: ThreadCachePayload,
    pub connection_status: String,
    pub latest_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeAppStateSnapshot {
    pub state: Value,
    pub thread_cache: ThreadCachePayload,
    pub instances: Vec<BridgeInstanceSummary>,
    pub agents: Vec<BridgeAgentSummary>,
    pub pending_approvals: Vec<PendingApproval>,
    pub review_jobs: Vec<Value>,
    #[serde(rename = "activeInstanceID")]
    pub active_instance_id: Option<String>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeInstanceSummary {
    pub id: String,
    pub project_path: String,
    pub cwd: String,
    pub is_running: bool,
    #[serde(rename = "experimentalAPIEnabled")]
    pub experimental_api_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadMessagesResponse {
    #[serde(rename = "threadID")]
    pub thread_id: String,
    pub version: u64,
    pub messages: Vec<RobdexChatMessage>,
    pub context_window_status: Option<ThreadContextWindowStatus>,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventReplayResponse {
    pub events: Vec<SequencedEvent>,
    pub latest_sequence: u64,
    pub requires_snapshot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SequencedEvent {
    pub sequence: u64,
    pub event: BridgeEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "name", content = "data")]
pub enum BridgeEvent {
    #[serde(rename = "connectionStatus")]
    ConnectionStatus { message: String },
    #[serde(rename = "appStateSnapshot")]
    AppStateSnapshot { state: Value },
    #[serde(rename = "threadMessagesChanged")]
    ThreadMessagesChanged { payload: ThreadMessagesResponse },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeApprovalResult {
    pub follow_up_message_requested: bool,
    pub follow_up_message_sent: bool,
    pub follow_up_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingApprovalKind {
    CommandExecution,
    FileChange,
    ToolUserInput,
    DynamicToolCall,
    ChatGptAuthRefresh,
}

impl PendingApprovalKind {
    fn wire_name(&self) -> &'static str {
        match self {
            Self::CommandExecution => "commandExecution",
            Self::FileChange => "fileChange",
            Self::ToolUserInput => "toolUserInput",
            Self::DynamicToolCall => "dynamicToolCall",
            Self::ChatGptAuthRefresh => "chatGPTAuthRefresh",
        }
    }

    fn from_wire_name(value: &str) -> Option<Self> {
        match value {
            "commandExecution" => Some(Self::CommandExecution),
            "fileChange" => Some(Self::FileChange),
            "toolUserInput" => Some(Self::ToolUserInput),
            "dynamicToolCall" => Some(Self::DynamicToolCall),
            "chatGPTAuthRefresh" => Some(Self::ChatGptAuthRefresh),
            _ => None,
        }
    }
}

impl Serialize for PendingApprovalKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = BTreeMap::new();
        map.insert(self.wire_name(), serde_json::Map::<String, Value>::new());
        map.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PendingApprovalKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(text) => {
                Self::from_wire_name(text.trim()).ok_or_else(|| serde::de::Error::custom("invalid approval kind"))
            }
            Value::Object(map) => {
                if map.len() != 1 {
                    return Err(serde::de::Error::custom("invalid approval kind object"));
                }
                let key = map.keys().next().cloned().unwrap_or_default();
                Self::from_wire_name(key.trim())
                    .ok_or_else(|| serde::de::Error::custom("invalid approval kind object"))
            }
            _ => Err(serde::de::Error::custom("invalid approval kind payload")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PendingApprovalFileChangeKind {
    Create,
    Update,
    Delete,
    Rename,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PendingApprovalFileChange {
    pub path: String,
    pub kind: PendingApprovalFileChangeKind,
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PendingApproval {
    pub id: String,
    #[serde(rename = "instanceID", alias = "instanceId")]
    pub instance_id: String,
    #[serde(rename = "requestID", alias = "requestId")]
    pub request_id: RequestId,
    #[serde(rename = "threadID", alias = "threadId")]
    pub thread_id: String,
    #[serde(rename = "turnID", alias = "turnId")]
    pub turn_id: String,
    #[serde(rename = "itemID", alias = "itemId")]
    pub item_id: String,
    pub kind: PendingApprovalKind,
    pub title: String,
    pub detail: Option<String>,
    pub approval_reason: Option<String>,
    pub tool_name: Option<String>,
    pub tool_arguments: Option<Value>,
    #[serde(default)]
    pub tool_questions: Vec<BridgeToolQuestion>,
    pub auth_refresh_reason: Option<String>,
    pub command: Option<String>,
    #[serde(rename = "commandCWD", alias = "commandCwd")]
    pub command_cwd: Option<String>,
    pub file_grant_root: Option<String>,
    #[serde(default)]
    pub file_changes: Vec<PendingApprovalFileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct BridgeToolQuestion {
    pub id: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadContextWindowStatus {
    pub remaining_percent: u32,
    pub used_tokens_in_context_window: u64,
    pub model_context_window: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RobdexToolMetadata {
    pub kind: String,
    pub status: Option<String>,
    pub command: Option<String>,
    pub output: Option<String>,
    #[serde(rename = "processId", alias = "processID")]
    pub process_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RobdexChatMessage {
    pub id: String,
    #[serde(rename = "threadID", alias = "threadId")]
    pub thread_id: String,
    pub role: String,
    pub text: String,
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub created_at: u64,
    pub subtitle: Option<String>,
    pub tool_metadata: Option<RobdexToolMetadata>,
    pub delivery_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCachePayload {
    #[serde(default)]
    #[serde(rename = "messageCacheByThreadID", alias = "messageCacheByThreadId")]
    pub message_cache_by_thread_id: BTreeMap<String, Vec<RobdexChatMessage>>,
    #[serde(default)]
    #[serde(rename = "contextWindowStatusByThreadID", alias = "contextWindowStatusByThreadId")]
    pub context_window_status_by_thread_id: BTreeMap<String, ThreadContextWindowStatus>,
    #[serde(default)]
    #[serde(rename = "runningThreadIDs", alias = "runningThreadIds")]
    pub running_thread_ids: Vec<String>,
    pub updated_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScopedAgentRecord {
    pub thread_id: String,
    pub display_name: Option<String>,
    pub project_path: String,
    pub cwd: String,
    pub role: String,
    pub is_orchestrator: bool,
    pub is_running: bool,
    pub is_archived: bool,
    pub is_hidden: bool,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BridgeAgentSummary {
    pub id: String,
    pub instance_id: String,
    pub thread_id: Option<String>,
    pub parent_agent_id: Option<String>,
    pub display_name: String,
    pub role: String,
    pub status: String,
    pub project_path: String,
    pub cwd: String,
    pub last_event: Option<String>,
    pub updated_at: u64,
}

#[cfg(test)]
mod tests {
    use super::RobdexToolMetadata;
    use serde_json::json;

    #[test]
    fn tool_metadata_serializes_process_id_with_swift_key() {
        let metadata = RobdexToolMetadata {
            kind: "commandExecution".to_string(),
            status: Some("completed".to_string()),
            command: Some("echo hi".to_string()),
            output: Some("hi".to_string()),
            process_id: Some("123".to_string()),
        };

        let value = serde_json::to_value(metadata).expect("serialize tool metadata");
        assert_eq!(value.get("processId"), Some(&json!("123")));
        assert!(value.get("processID").is_none());
    }

    #[test]
    fn tool_metadata_deserializes_legacy_process_id_alias() {
        let value = json!({
            "kind": "commandExecution",
            "status": "completed",
            "command": "echo hi",
            "output": "hi",
            "processID": "123"
        });

        let metadata: RobdexToolMetadata =
            serde_json::from_value(value).expect("deserialize legacy tool metadata");
        assert_eq!(metadata.process_id.as_deref(), Some("123"));
    }
}
