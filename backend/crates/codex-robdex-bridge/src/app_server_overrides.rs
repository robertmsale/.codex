use serde_json::{Map, Value, json};

#[derive(Debug, Default, Clone)]
pub struct AppServerThreadOverrides {
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub service_tier: Option<Value>,
    pub cwd: Option<String>,
    pub approval_policy: Option<Value>,
    pub approvals_reviewer: Option<Value>,
    pub sandbox: Option<String>,
    pub config: Option<Value>,
    pub base_instructions: Option<String>,
    pub developer_instructions: Option<String>,
    pub personality: Option<Value>,
    pub service_name: Option<String>,
    pub ephemeral: Option<bool>,
    pub persist_extended_history: Option<bool>,
    pub reasoning_effort: Option<String>,
    pub dynamic_tools: Option<Value>,
    pub exclude_turns: Option<bool>,
}

#[derive(Debug, Default, Clone)]
pub struct AppServerTurnOverrides {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub summary: Option<Value>,
    pub service_tier: Option<Value>,
    pub cwd: Option<String>,
    pub approval_policy: Option<Value>,
    pub approvals_reviewer: Option<Value>,
    pub sandbox_policy: Option<Value>,
    pub personality: Option<Value>,
    pub output_schema: Option<Value>,
    pub collaboration_mode: Option<Value>,
}

impl AppServerThreadOverrides {
    pub fn thread_start_params(self) -> Value {
        self.into_params(None, None, None)
    }

    pub fn thread_resume_params(self, thread_id: impl Into<String>, history: Option<Value>, path: Option<Value>) -> Value {
        self.into_params(Some(("threadId", Value::String(thread_id.into()))), history, path)
    }

    pub fn thread_fork_params(self, thread_id: impl Into<String>, path: Option<Value>) -> Value {
        self.into_params(Some(("threadId", Value::String(thread_id.into()))), None, path)
    }

    fn into_params(self, id: Option<(&'static str, Value)>, history: Option<Value>, path: Option<Value>) -> Value {
        let mut map = Map::new();
        if let Some((key, value)) = id {
            map.insert(key.to_string(), value);
        }
        insert_opt(&mut map, "model", self.model.map(Value::String));
        insert_opt(&mut map, "modelProvider", self.model_provider.map(Value::String));
        insert_opt(&mut map, "serviceTier", self.service_tier);
        insert_opt(&mut map, "cwd", self.cwd.map(Value::String));
        insert_opt(&mut map, "approvalPolicy", self.approval_policy);
        insert_opt(&mut map, "approvalsReviewer", self.approvals_reviewer);
        insert_opt(&mut map, "sandbox", self.sandbox.map(Value::String));
        insert_opt(
            &mut map,
            "config",
            config_with_reasoning_effort(self.config, self.reasoning_effort),
        );
        insert_opt(&mut map, "baseInstructions", self.base_instructions.map(Value::String));
        insert_opt(&mut map, "developerInstructions", self.developer_instructions.map(Value::String));
        insert_opt(&mut map, "personality", self.personality);
        insert_opt(&mut map, "serviceName", self.service_name.map(Value::String));
        insert_opt(&mut map, "ephemeral", self.ephemeral.map(Value::Bool));
        insert_opt(
            &mut map,
            "persistExtendedHistory",
            self.persist_extended_history.map(Value::Bool),
        );
        insert_opt(&mut map, "dynamicTools", self.dynamic_tools);
        insert_opt(&mut map, "excludeTurns", self.exclude_turns.map(Value::Bool));
        insert_opt(&mut map, "history", history);
        insert_opt(&mut map, "path", path);
        Value::Object(map)
    }
}

impl AppServerTurnOverrides {
    pub fn turn_start_params(self, thread_id: impl Into<String>, input: Value) -> Value {
        let mut map = Map::new();
        map.insert("threadId".to_string(), Value::String(thread_id.into()));
        map.insert("input".to_string(), input);
        insert_opt(&mut map, "model", self.model.map(Value::String));
        insert_opt(&mut map, "effort", self.effort.map(Value::String));
        insert_opt(&mut map, "summary", self.summary);
        insert_opt(&mut map, "serviceTier", self.service_tier);
        insert_opt(&mut map, "cwd", self.cwd.map(Value::String));
        insert_opt(&mut map, "approvalPolicy", self.approval_policy);
        insert_opt(&mut map, "approvalsReviewer", self.approvals_reviewer);
        insert_opt(&mut map, "sandboxPolicy", self.sandbox_policy);
        insert_opt(&mut map, "personality", self.personality);
        insert_opt(&mut map, "outputSchema", self.output_schema);
        insert_opt(&mut map, "collaborationMode", self.collaboration_mode);
        Value::Object(map)
    }
}

pub fn simple_sandbox_policy(
    sandbox_mode: Option<&str>,
    network_access: Option<bool>,
    cwd: Option<&str>,
) -> Option<Value> {
    match sandbox_mode {
        Some("danger-full-access") => Some(json!({ "type": "dangerFullAccess" })),
        Some("read-only") => Some(json!({
            "type": "readOnly",
            "access": { "type": "fullAccess" },
            "networkAccess": network_access.unwrap_or(false),
        })),
        Some("workspace-write") => Some(json!({
            "type": "workspaceWrite",
            "writableRoots": cwd.map(|value| vec![value]).unwrap_or_default(),
            "readOnlyAccess": { "type": "fullAccess" },
            "networkAccess": network_access.unwrap_or(true),
            "excludeTmpdirEnvVar": false,
            "excludeSlashTmp": false,
        })),
        Some("external-sandbox") => Some(json!({
            "type": "externalSandbox",
            "networkAccess": if network_access.unwrap_or(true) { "enabled" } else { "restricted" },
        })),
        _ => None,
    }
}

pub fn config_with_reasoning_effort(config: Option<Value>, reasoning_effort: Option<String>) -> Option<Value> {
    match (config, reasoning_effort) {
        (None, None) => None,
        (Some(config), None) => Some(config),
        (None, Some(reasoning_effort)) => Some(json!({ "model_reasoning_effort": reasoning_effort })),
        (Some(Value::Object(mut object)), Some(reasoning_effort)) => {
            object.insert("model_reasoning_effort".to_string(), Value::String(reasoning_effort));
            Some(Value::Object(object))
        }
        (Some(config), Some(_)) => Some(config),
    }
}

fn insert_opt(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value.filter(|value| !value.is_null()) {
        map.insert(key.to_string(), value);
    }
}
