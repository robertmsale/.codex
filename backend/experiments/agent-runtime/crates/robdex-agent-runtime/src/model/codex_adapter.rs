use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_models_manager::manager::{ModelsEndpointClient, ModelsManager, OpenAiModelsManager, RefreshStrategy};
use codex_protocol::error::{CodexErr, Result as CoreResult};
use codex_protocol::openai_models::{ModelInfo, ModelVisibility};
use codex_api::ResponsesApiRequest;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::model::{ModelClient, ModelFinalTurn, ModelHistoryItem, ModelInitialTurn, ModelToolTurn, RuntimeInputMessage, ToolCallRequest};
use crate::model_input;
use crate::roles::RoleSnapshot;

const CHATGPT_CODEX_RESPONSES_URL: &str =
    "https://chatgpt.com/backend-api/codex/responses";
const CHATGPT_CODEX_MODELS_URL: &str = "https://chatgpt.com/backend-api/codex/models";
const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";
const OPENAI_MODELS_URL: &str = "https://api.openai.com/v1/models";
const RAW_MODEL_RESPONSE_LIMIT: usize = 24_000;

pub struct CodexBackedModelClient {
    model: String,
    http: reqwest::Client,
    auth_path: PathBuf,
}

#[derive(Debug, Clone)]
struct CodexAuthMaterial {
    bearer: String,
    account_id: Option<String>,
    endpoint: &'static str,
    source: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexModelOption {
    pub id: String,
    pub display_label: String,
    pub source: String,
    pub is_default: bool,
}

pub struct CodexModelOptionsProvider {
    auth_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct AuthJson {
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
    auth_mode: Option<String>,
    tokens: Option<AuthTokens>,
}

#[derive(Debug, Deserialize)]
struct AuthTokens {
    access_token: String,
    account_id: Option<String>,
    id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IdTokenClaims {
    exp: Option<i64>,
}

impl CodexBackedModelClient {
    pub fn new() -> Result<Self> {
        let model = std::env::var("ROBDEX_AGENT_RUNTIME_MODEL")
            .context("ROBDEX_AGENT_RUNTIME_MODEL must be set when constructing a model client without an explicit session model")?;
        Self::new_with_model(model)
    }

    pub fn new_with_model(model: String) -> Result<Self> {
        Ok(Self {
            model,
            http: reqwest::Client::new(),
            auth_path: codex_home().join("auth.json"),
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
        role: &RoleSnapshot,
        history: &[ModelHistoryItem],
        runtime_messages: &[RuntimeInputMessage],
        execute_code_contract: &str,
        request_registry_contract: &str,
        message: &str,
    ) -> Value {
        let tool = Self::execute_code_tool_schema(execute_code_contract);
        let request_tool = Self::request_command_registry_change_tool_schema(request_registry_contract);
        let mut runtime_messages = runtime_messages.to_vec();
        runtime_messages.push(RuntimeInputMessage {
            text: "Use a native tool only when the user's request requires runtime work. Reply directly when no tool is needed. Call execute_code when the permanent Starlark interface can satisfy runtime work. Inspect live registered commands with cmd.describe(), cmd[\"object\"].describe(), or cmd[\"object\"].method.describe() inside execute_code when command details are needed. Full command/process output is stored as output artifacts; use outputs.head/tail/slice/search/stats for bounded retrieval instead of dumping large logs. Call request_command_registry_change when progress is blocked by a missing or outdated command registry entry.".to_string(),
            metadata: json!({"source": "runtime_tool_policy"}),
        });
        let cache_key = prompt_cache_key_for_runtime(role, &runtime_messages);
        json!({
            "model": model,
            "input": model_input::responses_input(role, history, &runtime_messages, Some(message)),
            "tools": [tool, request_tool],
            "tool_choice": "auto",
            "parallel_tool_calls": false,
            "store": false,
            "stream": true,
            "prompt_cache_key": cache_key,
        })
    }

    #[cfg(test)]
    fn new_for_testing(model: String, auth_path: PathBuf) -> Self {
        Self {
            model,
            http: reqwest::Client::new(),
            auth_path,
        }
    }

    fn resolve_auth(&self) -> Result<CodexAuthMaterial> {
        read_codex_auth(&self.auth_path)
    }

    fn request_headers(auth: &CodexAuthMaterial) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", auth.bearer))
                .context("invalid bearer token header")?,
        );
        if let Some(account_id) = &auth.account_id {
            headers.insert(
                "ChatGPT-Account-ID",
                HeaderValue::from_str(account_id).context("invalid chatgpt account id header")?,
            );
        }
        headers.insert("originator", HeaderValue::from_static("codex_cli_rs"));
        headers.insert(
            "x-codex-client",
            HeaderValue::from_static("robdex-agent-runtime-experiment"),
        );
        Ok(headers)
    }

    async fn post_responses(&self, body: &Value) -> Result<Value> {
        let auth = self.resolve_auth()?;
        let response = self
            .http
            .post(auth.endpoint)
            .headers(Self::request_headers(&auth)?)
            .json(body)
            .send()
            .await
            .context("Responses request failed")?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("Responses request failed using {} auth with HTTP {status}: {text}", auth.source);
        }
        parse_responses_body(&text).with_context(|| format!("invalid Responses response body: {text}"))
    }
}

impl CodexModelOptionsProvider {
    pub fn new() -> Self {
        let home = codex_home();
        Self {
            auth_path: home.join("auth.json"),
        }
    }

    #[cfg(test)]
    fn new_for_testing(auth_path: PathBuf) -> Self {
        Self {
            auth_path,
        }
    }

    pub async fn model_options(&self, force_refresh: bool) -> Result<Vec<CodexModelOption>> {
        let auth = read_codex_auth(&self.auth_path)?;
        let codex_home = self.auth_path.parent().unwrap_or_else(|| std::path::Path::new(".")).to_path_buf();
        let endpoint = Arc::new(CodexAuthModelsEndpoint {
            http: reqwest::Client::new(),
            auth_path: self.auth_path.clone(),
        });
        let manager = OpenAiModelsManager::new(
            codex_home,
            endpoint,
            None,
            CollaborationModesConfig::default(),
        );
        let strategy = if force_refresh {
            RefreshStrategy::Online
        } else {
            RefreshStrategy::OnlineIfUncached
        };
        let catalog = manager.raw_model_catalog(strategy).await;
        let chatgpt_auth = auth.endpoint == CHATGPT_CODEX_RESPONSES_URL;
        let mut options = catalog.models
            .into_iter()
            .filter(|model| model.visibility == ModelVisibility::List)
            .filter(|model| chatgpt_auth || model.supported_in_api)
            .map(|model| CodexModelOption {
                id: model.slug,
                display_label: model.display_name,
                source: "codex-models-manager".to_string(),
                is_default: false,
            })
            .collect::<Vec<_>>();
        options.sort_by(|left, right| right.is_default.cmp(&left.is_default).then(left.display_label.to_lowercase().cmp(&right.display_label.to_lowercase())));
        options.dedup_by(|left, right| left.id == right.id);
        if options.is_empty() {
            bail!("Codex models-manager did not return selectable models")
        }
        Ok(options)
    }
}

#[derive(Debug)]
struct CodexAuthModelsEndpoint {
    http: reqwest::Client,
    auth_path: PathBuf,
}

#[async_trait]
impl ModelsEndpointClient for CodexAuthModelsEndpoint {
    fn has_command_auth(&self) -> bool {
        true
    }

    async fn uses_codex_backend(&self) -> bool {
        read_codex_auth(&self.auth_path)
            .map(|auth| auth.endpoint == CHATGPT_CODEX_RESPONSES_URL)
            .unwrap_or(false)
    }

    async fn list_models(&self, _client_version: &str) -> CoreResult<(Vec<ModelInfo>, Option<String>)> {
        let auth = read_codex_auth(&self.auth_path).map_err(|error| CodexErr::InvalidRequest(error.to_string()))?;
        let url = if auth.endpoint == CHATGPT_CODEX_RESPONSES_URL {
            CHATGPT_CODEX_MODELS_URL
        } else {
            OPENAI_MODELS_URL
        };
        let response = self
            .http
            .get(url)
            .headers(CodexBackedModelClient::request_headers(&auth).map_err(|error| CodexErr::InvalidRequest(error.to_string()))?)
            .send()
            .await
            .map_err(|error| CodexErr::InvalidRequest(format!("Codex model-options request failed for {url}: {error}")))?;
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string);
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(CodexErr::InvalidRequest(format!(
                "Codex model-options request failed using {} auth with HTTP {status}: {text}",
                auth.source
            )));
        }
        let catalog: codex_protocol::openai_models::ModelsResponse = serde_json::from_str(&text)
            .map_err(|error| CodexErr::InvalidRequest(format!("invalid Codex model-options body: {error}: {text}")))?;
        Ok((catalog.models, etag))
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

fn prompt_cache_key_for_runtime(role: &RoleSnapshot, runtime_messages: &[RuntimeInputMessage]) -> String {
    let role_epoch = model_input::role_epoch(role);
    let context_epoch = runtime_messages
        .iter()
        .find_map(|message| message.metadata.get("contextEpoch").and_then(Value::as_i64))
        .map(|epoch| epoch.to_string())
        .unwrap_or_else(|| "none".to_string());
    model_input::prompt_cache_key_from_epochs(&role_epoch, Some(&context_epoch))
}

#[async_trait]
impl ModelClient for CodexBackedModelClient {
    async fn request_tool_call(&self, role: &RoleSnapshot, history: &[ModelHistoryItem], runtime_messages: &[RuntimeInputMessage], execute_code_contract: &str, request_registry_contract: &str, message: &str) -> Result<ModelInitialTurn> {
        let body = Self::request_tool_call_request_shape(
            &self.model,
            role,
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
            instructions: String::new(),
            input: Vec::new(),
            tools: vec![tool.clone(), request_tool.clone()],
            tool_choice: "auto".to_string(),
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: true,
            include: Vec::new(),
            service_tier: None,
            prompt_cache_key: Some(model_input::prompt_cache_key(role, None)),
            text: None,
            client_metadata: Some(HashMap::from([(
                "runtime".to_string(),
                "robdex-agent-runtime".to_string(),
            )])),
        };
        let _ = request_for_shape;
        let raw_response = self.post_responses(&body).await?;
        let calls = extract_native_tool_calls(&raw_response)?;
        if calls.len() > 1 {
            bail!("model response included more than one native tool call, got {}: {raw_response}", calls.len());
        }
        if let Some((call_id, tool_name, arguments)) = calls.into_iter().next() {
            return Ok(ModelInitialTurn::ToolCall(ModelToolTurn {
            provider: "chatgpt-codex-responses".to_string(),
            model: self.model.clone(),
            assistant_summary: format!("Live Responses model called {tool_name}."),
            tool_call: ToolCallRequest {
                call_identity: call_id,
                tool_name,
                arguments,
            },
            request_shape,
            raw_response,
            }));
        }
        let final_text = extract_output_text(&raw_response).unwrap_or_default();
        if final_text.trim().is_empty() {
            bail!("model response included neither native tool call nor assistant text: {raw_response}");
        }
        Ok(ModelInitialTurn::FinalResponse(ModelFinalTurn {
            provider: "chatgpt-codex-responses".to_string(),
            model: self.model.clone(),
            final_text,
            request_shape,
            raw_response,
        }))
    }

    async fn submit_tool_result(
        &self,
        role: &RoleSnapshot,
        history: &[ModelHistoryItem],
        runtime_messages: &[RuntimeInputMessage],
        tool_call_response: &Value,
        call_id: &str,
        tool_result: &Value,
    ) -> Result<ModelFinalTurn> {
        let result_text = serde_json::to_string(tool_result)?;
        let function_call_item = find_native_tool_item(tool_call_response, call_id)?;
        let mut runtime_messages = runtime_messages.to_vec();
        runtime_messages.push(RuntimeInputMessage {
            text: "Summarize the tool result concisely using the structured prior messages in the request input.".to_string(),
            metadata: json!({"source": "runtime_tool_result_policy"}),
        });
        let mut input = model_input::responses_input(role, history, &runtime_messages, None);
        input.push(function_call_item);
        input.push(json!({
            "type": "function_call_output",
            "call_id": call_id,
            "output": result_text
        }));
        let cache_key = prompt_cache_key_for_runtime(role, &runtime_messages);
        let body = json!({
            "model": self.model,
            "input": input,
            "store": false,
            "stream": true,
            "prompt_cache_key": cache_key,
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

fn codex_home() -> PathBuf {
    PathBuf::from(std::env::var("CODEX_HOME").unwrap_or_else(|_| "/Users/robertsale/.codex".to_string()))
}

fn read_codex_auth(auth_path: &std::path::Path) -> Result<CodexAuthMaterial> {
    let auth = fs::read_to_string(auth_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<AuthJson>(&raw).ok());
    if let Some(auth) = auth {
        if auth.auth_mode.as_deref() != Some("api_key")
            && let Some(tokens) = auth.tokens
            && !tokens.access_token.trim().is_empty()
            && id_token_is_not_expired(tokens.id_token.as_deref())
        {
            return Ok(CodexAuthMaterial {
                bearer: tokens.access_token,
                account_id: tokens.account_id,
                endpoint: CHATGPT_CODEX_RESPONSES_URL,
                source: "codex-auth-json",
            });
        }
        if let Some(api_key) = auth.openai_api_key.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) {
            return Ok(CodexAuthMaterial {
                bearer: api_key,
                account_id: None,
                endpoint: OPENAI_RESPONSES_URL,
                source: "codex-auth-json-api-key",
            });
        }
    }
    if let Ok(api_key) = std::env::var("OPENAI_API_KEY")
        && !api_key.trim().is_empty()
    {
        return Ok(CodexAuthMaterial {
            bearer: api_key.trim().to_string(),
            account_id: None,
            endpoint: OPENAI_RESPONSES_URL,
            source: "openai-api-key-env",
        });
    }
    bail!("Codex auth.json does not contain a non-expired ChatGPT token and OPENAI_API_KEY fallback is not set")
}

fn id_token_is_not_expired(id_token: Option<&str>) -> bool {
    let Some(id_token) = id_token else {
        return false;
    };
    let Some(payload) = id_token.split('.').nth(1) else {
        return false;
    };
    let decoded = match base64_url_decode(payload) {
        Some(decoded) => decoded,
        None => return false,
    };
    let claims = match serde_json::from_slice::<IdTokenClaims>(&decoded) {
        Ok(claims) => claims,
        Err(_) => return false,
    };
    let Some(exp) = claims.exp else {
        return false;
    };
    let now = unix_now();
    exp > now + 60
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(i64::MAX)
}

fn base64_url_decode(input: &str) -> Option<Vec<u8>> {
    const INVALID: u8 = 255;
    fn value(byte: u8) -> u8 {
        match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => INVALID,
        }
    }
    let mut buffer = 0u32;
    let mut bits = 0u8;
    let mut output = Vec::new();
    for byte in input.bytes().filter(|byte| *byte != b'=') {
        let val = value(byte);
        if val == INVALID {
            return None;
        }
        buffer = (buffer << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    Some(output)
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

fn extract_native_tool_calls(response: &Value) -> Result<Vec<(String, String, Value)>> {
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
    Ok(calls)
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
    use std::sync::atomic::{AtomicUsize, Ordering};

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
            end_of_session_behavior: "terminate".to_string(),
            stdin_policy: "forbid".to_string(),
            min_await_ms: 0,
            max_await_ms: 60000,
            output_buffer_bytes: 64000,
            terminate_grace_ms: 1000,
        }
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

    #[test]
    fn auth_json_chatgpt_tokens_take_precedence_over_api_key_when_not_expired() {
        let dir = tempfile::tempdir().expect("temp dir");
        let auth_path = dir.path().join("auth.json");
        std::fs::write(
            &auth_path,
            r#"{
                "auth_mode": "chatgpt",
                "OPENAI_API_KEY": "sk-auth-json",
                "tokens": {
                    "access_token": "chatgpt-access",
                    "account_id": "account-123",
                    "id_token": "header.eyJleHAiOjQxMDI0NDQ4MDB9.signature",
                    "refresh_token": "refresh"
                }
            }"#,
        )
        .expect("write auth");

        let auth = read_codex_auth(&auth_path).expect("auth");

        assert_eq!(auth.source, "codex-auth-json");
        assert_eq!(auth.endpoint, CHATGPT_CODEX_RESPONSES_URL);
        assert_eq!(auth.bearer, "chatgpt-access");
        assert_eq!(auth.account_id.as_deref(), Some("account-123"));
    }

    #[test]
    fn expired_chatgpt_token_falls_back_to_auth_json_api_key() {
        let dir = tempfile::tempdir().expect("temp dir");
        let auth_path = dir.path().join("auth.json");
        std::fs::write(
            &auth_path,
            r#"{
                "auth_mode": "chatgpt",
                "OPENAI_API_KEY": "sk-auth-json",
                "tokens": {
                    "access_token": "expired-chatgpt-access",
                    "account_id": "account-123",
                    "id_token": "header.eyJleHAiOjEwMDB9.signature",
                    "refresh_token": "refresh"
                }
            }"#,
        )
        .expect("write auth");

        let auth = read_codex_auth(&auth_path).expect("auth");

        assert_eq!(auth.source, "codex-auth-json-api-key");
        assert_eq!(auth.endpoint, OPENAI_RESPONSES_URL);
        assert_eq!(auth.bearer, "sk-auth-json");
        assert!(auth.account_id.is_none());
    }

    #[test]
    fn chatgpt_auth_headers_match_codex_backend_contract() {
        let auth = CodexAuthMaterial {
            bearer: "chatgpt-access".to_string(),
            account_id: Some("account-123".to_string()),
            endpoint: CHATGPT_CODEX_RESPONSES_URL,
            source: "codex-auth-json",
        };

        let headers = CodexBackedModelClient::request_headers(&auth).expect("headers");

        assert_eq!(headers.get(AUTHORIZATION).and_then(|value| value.to_str().ok()), Some("Bearer chatgpt-access"));
        assert_eq!(headers.get("chatgpt-account-id").and_then(|value| value.to_str().ok()), Some("account-123"));
        assert_eq!(headers.get("originator").and_then(|value| value.to_str().ok()), Some("codex_cli_rs"));
    }

    #[derive(Debug)]
    struct CountingModelsEndpoint {
        calls: Arc<AtomicUsize>,
        models: Vec<ModelInfo>,
    }

    #[async_trait]
    impl ModelsEndpointClient for CountingModelsEndpoint {
        fn has_command_auth(&self) -> bool {
            true
        }

        async fn uses_codex_backend(&self) -> bool {
            true
        }

        async fn list_models(&self, _client_version: &str) -> CoreResult<(Vec<ModelInfo>, Option<String>)> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok((self.models.clone(), Some("etag-test".to_string())))
        }
    }

    #[tokio::test]
    async fn vendored_models_manager_cache_serves_model_options_without_second_fetch() {
        let dir = tempfile::tempdir().expect("temp dir");
        let model = codex_models_manager::bundled_models_response()
            .expect("bundled models")
            .models
            .into_iter()
            .next()
            .expect("at least one bundled model");
        let calls = Arc::new(AtomicUsize::new(0));
        let endpoint = Arc::new(CountingModelsEndpoint {
            calls: calls.clone(),
            models: vec![model],
        });
        let manager = OpenAiModelsManager::new(
            dir.path().to_path_buf(),
            endpoint,
            None,
            CollaborationModesConfig::default(),
        );

        let first = manager.list_models(RefreshStrategy::OnlineIfUncached).await;
        let second = manager.list_models(RefreshStrategy::OnlineIfUncached).await;

        assert!(!first.is_empty());
        assert_eq!(first, second);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(dir.path().join("models_cache.json").exists());
    }

    #[tokio::test]
    async fn unavailable_model_options_surface_auth_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let provider = CodexModelOptionsProvider::new_for_testing(dir.path().join("auth.json"));

        let error = provider.model_options(false).await.expect_err("missing auth must fail");

        assert!(error.to_string().contains("Codex auth.json does not contain a non-expired ChatGPT token"));
    }
}
