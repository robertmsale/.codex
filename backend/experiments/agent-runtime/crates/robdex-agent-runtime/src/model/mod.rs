pub mod codex_adapter;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::roles::RoleSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub call_identity: String,
    pub tool_name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelToolTurn {
    pub provider: String,
    pub model: String,
    pub assistant_summary: String,
    pub tool_call: ToolCallRequest,
    pub request_shape: Value,
    pub raw_response: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelInitialTurn {
    ToolCall(ModelToolTurn),
    FinalResponse(ModelFinalTurn),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFinalTurn {
    pub provider: String,
    pub model: String,
    pub final_text: String,
    pub request_shape: Value,
    pub raw_response: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInputMessage {
    pub text: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHistoryItem {
    pub session_id: String,
    pub turn_id: String,
    pub user: String,
    pub assistant: Option<String>,
    pub started_at: String,
    pub source: String,
    pub checkpoint_id: Option<String>,
}

#[async_trait]
pub trait ModelClient {
    async fn request_tool_call(&self, role: &RoleSnapshot, history: &[ModelHistoryItem], runtime_messages: &[RuntimeInputMessage], execute_code_contract: &str, request_registry_contract: &str, message: &str) -> Result<ModelInitialTurn>;
    async fn submit_tool_result(
        &self,
        role: &RoleSnapshot,
        history: &[ModelHistoryItem],
        runtime_messages: &[RuntimeInputMessage],
        tool_call_response: &Value,
        call_id: &str,
        tool_result: &Value,
    ) -> Result<ModelFinalTurn>;
}
