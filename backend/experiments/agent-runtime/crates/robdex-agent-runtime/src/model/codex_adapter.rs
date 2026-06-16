use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use codex_api::ResponsesApiRequest;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::model::{ModelClient, ModelFinalTurn, ModelHistoryItem, ModelToolTurn, RuntimeInputMessage, ToolCallRequest};

const CHATGPT_CODEX_RESPONSES_URL: &str =
    "https://chatgpt.com/backend-api/codex/responses?client_version=0.124.0&source=robdex-agent-runtime";
const RAW_MODEL_RESPONSE_LIMIT: usize = 24_000;

pub struct CodexBackedModelClient {
    model: String,
    http: reqwest::Client,
    auth: CodexAuthMaterial,
}

#[derive(Debug, Clone)]
struct CodexAuthMaterial {
    bearer: String,
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthJson {
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
    tokens: Option<AuthTokens>,
}

#[derive(Debug, Deserialize)]
struct AuthTokens {
    access_token: String,
    account_id: Option<String>,
}

impl CodexBackedModelClient {
    pub fn new() -> Result<Self> {
        let model = std::env::var("ROBDEX_AGENT_RUNTIME_MODEL")
            .unwrap_or_else(|_| "gpt-5.5".to_string());
        Self::new_with_model(model)
    }

    pub fn new_with_model(model: String) -> Result<Self> {
        Ok(Self {
            model,
            http: reqwest::Client::new(),
            auth: read_codex_auth()?,
        })
    }

    pub fn execute_code_tool_schema(execute_code_contract: &str) -> Value {
        json!({
            "type": "function",
            "name": "execute_code",
            "description": execute_code_contract,
            "parameters": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Starlark source to evaluate in the experimental runtime. Use cmd.describe() and per-command .describe() inside Starlark for live command details."
                    }
                },
                "required": ["source"]
            },
            "strict": true
        })
    }

    pub fn request_command_registry_change_tool_schema(request_contract: &str) -> Value {
        json!({
            "type": "function",
            "name": "request_command_registry_change",
            "description": request_contract,
            "parameters": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "operation": {"type":"string","enum":["add","update","disable","enable"]},
                    "proposedCommand": {
                        "type":"object",
                        "additionalProperties": false,
                        "properties": {
                            "actionId":{"type":"string"},
                            "binaryName":{"type":"string"},
                            "candidatePaths":{"type":"array","items":{"type":"string"}},
                            "starlarkObject":{"type":"string"},
                            "starlarkMethod":{"type":"string"},
                            "argvPrefix":{"type":"array","items":{"type":"string"}},
                            "defaultCwd":{"type":"string"},
                            "cwdPolicy":{"type":"string"},
                            "envPolicy":{"type":"string"},
                            "syncAllowed":{"type":"boolean"},
                            "asyncAllowed":{"type":"boolean"},
                            "maxRuntimeMs":{"anyOf":[{"type":"integer"},{"type":"null"}]},
                            "endOfTurnBehavior":{"type":"string","enum":["terminate","continue"]},
                            "stdinPolicy":{"type":"string","enum":["forbid","allow"]},
                            "minAwaitMs":{"type":"integer"},
                            "maxAwaitMs":{"type":"integer"},
                            "outputBufferBytes":{"type":"integer"},
                            "terminateGraceMs":{"type":"integer"},
                            "outputLimitBytes":{"type":"integer"},
                            "mutationClass":{"type":"string"},
                            "modelDescription":{"type":"string"},
                            "allowCwdArg":{"type":"boolean"},
                            "allowArgsArg":{"type":"boolean"},
                            "forbiddenArgs":{"type":"array","items":{"type":"string"}}
                        },
                        "required":["actionId","binaryName","candidatePaths","starlarkObject","starlarkMethod","argvPrefix","defaultCwd","cwdPolicy","envPolicy","syncAllowed","asyncAllowed","maxRuntimeMs","endOfTurnBehavior","stdinPolicy","minAwaitMs","maxAwaitMs","outputBufferBytes","terminateGraceMs","outputLimitBytes","mutationClass","modelDescription","allowCwdArg","allowArgsArg","forbiddenArgs"]
                    },
                    "rationale":{"type":"string"},
                    "intendedUse":{"type":"string"},
                    "currentBlockerOrNeed":{"type":"string"},
                    "requesterContext":{
                        "type":"object",
                        "additionalProperties": false,
                        "properties": {
                            "sourceRole":{"type":"string"},
                            "sourceTask":{"type":"string"},
                            "observedError":{"type":"string"},
                            "neededFor":{"type":"string"}
                        },
                        "required":["sourceRole","sourceTask","observedError","neededFor"]
                    }
                },
                "required":["operation","proposedCommand","rationale","intendedUse","currentBlockerOrNeed","requesterContext"]
            },
            "strict": true
        })
    }

    pub fn request_tool_call_request_shape(
        model: &str,
        role_instructions: &str,
        history: &[ModelHistoryItem],
        runtime_messages: &[RuntimeInputMessage],
        execute_code_contract: &str,
        request_registry_contract: &str,
        message: &str,
    ) -> Value {
        let tool = Self::execute_code_tool_schema(execute_code_contract);
        let request_tool = Self::request_command_registry_change_tool_schema(request_registry_contract);
        let instructions = format!(
            "{role_instructions}\n\nChoose exactly one native tool for this turn. Call execute_code when the permanent Starlark interface can satisfy the user. Inspect live registered commands with cmd.describe(), cmd[\"object\"].describe(), or cmd[\"object\"].method.describe() inside execute_code when command details are needed. Full command/process output is stored as output artifacts; use outputs.head/tail/slice/search/stats for bounded retrieval instead of dumping large logs. Call request_command_registry_change when progress is blocked by a missing or outdated command registry entry."
        );
        json!({
            "model": model,
            "instructions": instructions,
            "input": responses_input_from_history(history, runtime_messages, Some(message)),
            "tools": [tool, request_tool],
            "tool_choice": "auto",
            "parallel_tool_calls": false,
            "store": false,
            "stream": true,
            "prompt_cache_key": "robdex-agent-runtime-kernel-v1",
        })
    }

    fn request_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.auth.bearer))
                .context("invalid bearer token header")?,
        );
        if let Some(account_id) = &self.auth.account_id {
            headers.insert(
                "chatgpt-account-id",
                HeaderValue::from_str(account_id).context("invalid chatgpt account id header")?,
            );
        }
        headers.insert(
            "x-codex-client",
            HeaderValue::from_static("robdex-agent-runtime-experiment"),
        );
        Ok(headers)
    }

    async fn post_responses(&self, body: &Value) -> Result<Value> {
        let response = self
            .http
            .post(CHATGPT_CODEX_RESPONSES_URL)
            .headers(self.request_headers()?)
            .json(body)
            .send()
            .await
            .context("Responses request failed")?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("Responses request failed with HTTP {status}: {text}");
        }
        parse_responses_body(&text).with_context(|| format!("invalid Responses response body: {text}"))
    }
}

pub fn concise_response_summary(response: &Value) -> Value {
    json!({
        "id": response.get("id").and_then(Value::as_str),
        "model": response.get("model").and_then(Value::as_str),
        "status": response.get("status").and_then(Value::as_str),
        "outputTypes": response
            .get("output")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(|item| item.get("type").and_then(Value::as_str)).collect::<Vec<_>>())
            .unwrap_or_default(),
        "usage": response.get("usage").cloned().unwrap_or(Value::Null),
    })
}

pub fn bounded_raw_response(response: &Value) -> Value {
    let raw = serde_json::to_string(response).unwrap_or_default();
    let (body, truncated) = truncate_string(&raw, RAW_MODEL_RESPONSE_LIMIT);
    json!({
        "rawResponseJson": body,
        "truncation": {
            "rawResponseTruncated": truncated,
            "limitBytes": RAW_MODEL_RESPONSE_LIMIT,
        }
    })
}

#[async_trait]
impl ModelClient for CodexBackedModelClient {
    async fn request_tool_call(&self, role_instructions: &str, history: &[ModelHistoryItem], runtime_messages: &[RuntimeInputMessage], execute_code_contract: &str, request_registry_contract: &str, message: &str) -> Result<ModelToolTurn> {
        let body = Self::request_tool_call_request_shape(
            &self.model,
            role_instructions,
            history,
            runtime_messages,
            execute_code_contract,
            request_registry_contract,
            message,
        );
        let request_shape = body.clone();
        let tool = Self::execute_code_tool_schema(execute_code_contract);
        let request_tool = Self::request_command_registry_change_tool_schema(request_registry_contract);
        let request_for_shape = ResponsesApiRequest {
            model: self.model.clone(),
            instructions: body.get("instructions").and_then(Value::as_str).unwrap_or_default().to_string(),
            input: Vec::new(),
            tools: vec![tool.clone(), request_tool.clone()],
            tool_choice: "auto".to_string(),
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: true,
            include: Vec::new(),
            service_tier: None,
            prompt_cache_key: Some("robdex-agent-runtime-kernel-v1".to_string()),
            text: None,
            client_metadata: Some(HashMap::from([(
                "runtime".to_string(),
                "robdex-agent-runtime".to_string(),
            )])),
        };
        let _ = request_for_shape;
        let raw_response = self.post_responses(&body).await?;
        let (call_id, tool_name, arguments) = extract_native_tool_call(&raw_response)?;
        Ok(ModelToolTurn {
            provider: "chatgpt-codex-responses".to_string(),
            model: self.model.clone(),
            assistant_summary: "Live Responses model called execute_code.".to_string(),
            tool_call: ToolCallRequest {
                call_identity: call_id,
                tool_name,
                arguments,
            },
            request_shape,
            raw_response,
        })
    }

    async fn submit_tool_result(
        &self,
        role_instructions: &str,
        history: &[ModelHistoryItem],
        tool_call_response: &Value,
        call_id: &str,
        tool_result: &Value,
    ) -> Result<ModelFinalTurn> {
        let result_text = serde_json::to_string(tool_result)?;
        let function_call_item = find_native_tool_item(tool_call_response, call_id)?;
        let mut input = responses_input_from_history(history, &[], None);
        input.push(function_call_item);
        input.push(json!({
            "type": "function_call_output",
            "call_id": call_id,
            "output": result_text
        }));
        let instructions = format!(
            "{role_instructions}\n\nSummarize the tool result concisely using the structured prior messages in the request input."
        );
        let body = json!({
            "model": self.model,
            "instructions": instructions,
            "input": input,
            "store": false,
            "stream": true,
            "prompt_cache_key": "robdex-agent-runtime-kernel-v1",
        });
        let raw_response = self.post_responses(&body).await?;
        Ok(ModelFinalTurn {
            provider: "chatgpt-codex-responses".to_string(),
            model: self.model.clone(),
            final_text: extract_output_text(&raw_response).unwrap_or_default(),
            request_shape: body,
            raw_response,
        })
    }
}

pub fn responses_input_from_history(history: &[ModelHistoryItem], runtime_messages: &[RuntimeInputMessage], current_message: Option<&str>) -> Vec<Value> {
    let mut input = Vec::new();
    for item in history {
        input.push(json!({
            "role": "user",
            "content": [{"type": "input_text", "text": item.user}],
            "metadata": {
                "sessionId": item.session_id,
                "turnId": item.turn_id,
                "startedAt": item.started_at,
                "source": item.source,
                "checkpointId": item.checkpoint_id,
            }
        }));
        if let Some(assistant) = &item.assistant
            && !assistant.trim().is_empty()
        {
            input.push(json!({
                "role": "assistant",
                "content": [{"type": "output_text", "text": assistant}],
                "metadata": {
                    "sessionId": item.session_id,
                    "turnId": item.turn_id,
                    "startedAt": item.started_at,
                    "source": item.source,
                    "checkpointId": item.checkpoint_id,
                }
            }));
        }
    }
    for runtime_message in runtime_messages {
        input.push(json!({
            "role": "user",
            "content": [{"type": "input_text", "text": runtime_message.text}],
            "metadata": runtime_message.metadata,
        }));
    }
    if let Some(message) = current_message {
        input.push(json!({
            "role": "user",
            "content": [{"type": "input_text", "text": message}]
        }));
    }
    input
}

fn read_codex_auth() -> Result<CodexAuthMaterial> {
    if let Ok(api_key) = std::env::var("OPENAI_API_KEY")
        && !api_key.trim().is_empty()
    {
        return Ok(CodexAuthMaterial {
            bearer: api_key,
            account_id: None,
        });
    }
    let auth_path = PathBuf::from(
        std::env::var("CODEX_HOME").unwrap_or_else(|_| "/Users/robertsale/.codex".to_string()),
    )
    .join("auth.json");
    let raw = fs::read_to_string(&auth_path)
        .with_context(|| format!("failed to read Codex auth file {}", auth_path.display()))?;
    let auth: AuthJson = serde_json::from_str(&raw).context("failed to parse Codex auth JSON")?;
    if let Some(api_key) = auth.openai_api_key
        && !api_key.trim().is_empty()
    {
        return Ok(CodexAuthMaterial {
            bearer: api_key,
            account_id: None,
        });
    }
    let tokens = auth.tokens.context("Codex auth JSON has no token data")?;
    Ok(CodexAuthMaterial {
        bearer: tokens.access_token,
        account_id: tokens.account_id,
    })
}

fn find_native_tool_item(response: &Value, call_id: &str) -> Result<Value> {
    for item in response
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if item.get("type").and_then(Value::as_str) == Some("function_call")
            && item.get("call_id").or_else(|| item.get("id")).and_then(Value::as_str) == Some(call_id)
        {
            return Ok(item.clone());
        }
    }
    bail!("model response did not include native tool item for call_id {call_id}")
}

fn extract_native_tool_call(response: &Value) -> Result<(String, String, Value)> {
    let mut calls = Vec::new();
    for item in response
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if item.get("type").and_then(Value::as_str) == Some("function_call") {
            let Some(name) = item.get("name").and_then(Value::as_str) else { continue; };
            if !matches!(name, "execute_code" | "request_command_registry_change") {
                continue;
            }
            let call_id = item
                .get("call_id")
                .or_else(|| item.get("id"))
                .and_then(Value::as_str)
                .context("native function_call missing call_id")?
                .to_string();
            let arguments = item
                .get("arguments")
                .and_then(Value::as_str)
                .context("native function_call missing arguments")?;
            let parsed: Value = serde_json::from_str(arguments)
                .context("native function_call arguments are not JSON")?;
            calls.push((call_id, name.to_string(), parsed));
        }
    }
    if calls.len() != 1 {
        bail!("model response must include exactly one native tool call, got {}: {response}", calls.len());
    }
    Ok(calls.remove(0))
}

fn parse_responses_body(text: &str) -> Result<Value> {
    if let Ok(json) = serde_json::from_str::<Value>(text) {
        return Ok(json);
    }
    let mut completed = None;
    let mut output_items = Vec::new();
    for line in text.lines() {
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        if data == "[DONE]" {
            break;
        }
        let event: Value = serde_json::from_str(data)?;
        if event.get("type").and_then(Value::as_str) == Some("response.output_item.done")
            && let Some(item) = event.get("item")
        {
            output_items.push(item.clone());
        }
        if event.get("type").and_then(Value::as_str) == Some("response.completed")
            && let Some(response) = event.get("response")
        {
            completed = Some(response.clone());
        }
    }
    let mut response = completed.context("Responses stream did not contain response.completed")?;
    if response
        .get("output")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
        && !output_items.is_empty()
    {
        response["output"] = Value::Array(output_items);
    }
    Ok(response)
}

fn extract_output_text(response: &Value) -> Option<String> {
    let mut parts = Vec::new();
    for item in response.get("output")?.as_array()? {
        if item.get("type").and_then(Value::as_str) == Some("message")
            && let Some(content) = item.get("content").and_then(Value::as_array)
        {
            for part in content {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    parts.push(text.to_string());
                }
            }
        }
    }
    Some(parts.join("\n"))
}

fn truncate_string(input: &str, limit: usize) -> (String, bool) {
    if input.len() <= limit {
        return (input.to_string(), false);
    }
    let mut end = limit;
    while !input.is_char_boundary(end) {
        end -= 1;
    }
    (input[..end].to_string(), true)
}

#[cfg(test)]
mod cache_stable_tests {
    use super::*;

    fn command(action: &str) -> crate::command_registry::CommandVersion {
        crate::command_registry::CommandVersion {
            version_id: uuid::Uuid::new_v4(),
            definition_id: uuid::Uuid::new_v4(),
            scope_type: "global".to_string(),
            project_key: None,
            action_id: action.to_string(),
            binary_name: "echo".to_string(),
            candidate_paths: vec![std::path::PathBuf::from("/bin/echo")],
            starlark_object: action.replace('.', "_"),
            starlark_method: "run".to_string(),
            argv_prefix: vec![],
            default_cwd: ".".to_string(),
            cwd_policy: "underExecutionRoot".to_string(),
            env_policy: "empty".to_string(),
            max_runtime: None,
            output_limit: 12000,
            mutation_class: "readOnly".to_string(),
            model_description: "test command".to_string(),
            allow_cwd_arg: true,
            allow_args_arg: true,
            forbidden_args: vec![],
            execution_policy: "allow".to_string(),
            sync_allowed: true,
            async_allowed: false,
            end_of_turn_behavior: "terminate".to_string(),
            stdin_policy: "forbid".to_string(),
            min_await_ms: 0,
            max_await_ms: 60000,
            output_buffer_bytes: 64000,
            terminate_grace_ms: 1000,
        }
    }


    #[test]
    fn runtime_command_context_input_has_metadata_and_is_not_history() {
        let history = vec![ModelHistoryItem {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            user: "ordinary user".to_string(),
            assistant: Some("ordinary assistant".to_string()),
            started_at: "2026-01-01T00:00:00Z".to_string(),
            source: "reconstructed_session_history".to_string(),
            checkpoint_id: None,
        }];
        let runtime = vec![RuntimeInputMessage {
            text: "runtime command context".to_string(),
            metadata: json!({"source":"runtime_command_context", "commandContextId":"cmdctx-test"}),
        }];
        let input = responses_input_from_history(&history, &runtime, Some("current user"));
        assert_eq!(input.len(), 4);
        assert_eq!(input[0]["metadata"]["source"], "reconstructed_session_history");
        assert_eq!(input[1]["metadata"]["source"], "reconstructed_session_history");
        assert_eq!(input[2]["metadata"]["source"], "runtime_command_context");
        assert_eq!(input[3]["role"], "user");
        assert!(input[3].get("metadata").is_none());
    }

    #[test]
    fn complete_execute_code_tool_schema_is_identical_across_command_contexts() {
        let commands_a = vec![command("cmd.schema.one")];
        let commands_b = vec![command("cmd.schema.one"), command("cmd.schema.two")];
        let context_a = crate::command_registry::runtime_command_context_message(&commands_a, None);
        let context_b = crate::command_registry::runtime_command_context_message(&commands_b, Some(&context_a.evidence));
        let contract = crate::command_registry::stable_execute_code_contract();
        let schema_a = CodexBackedModelClient::execute_code_tool_schema(&contract);
        let schema_b = CodexBackedModelClient::execute_code_tool_schema(&contract);
        assert_eq!(schema_a, schema_b);
        assert_ne!(context_a.evidence.id, context_b.evidence.id);
        assert_ne!(context_a.text, context_b.text);
        assert_eq!(schema_a["description"], contract);
        let source_description = schema_a["parameters"]["properties"]["source"]["description"].as_str().unwrap();
        assert!(source_description.contains("Starlark source"));
        assert!(!source_description.contains("Registry commands available now"));
        assert!(!source_description.contains("cmd[\""));
    }
}
