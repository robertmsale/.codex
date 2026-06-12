use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use codex_api::ResponsesApiRequest;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::model::{ModelClient, ModelFinalTurn, ModelToolTurn, ToolCallRequest};

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

    fn execute_code_tool_schema() -> Value {
        json!({
            "type": "function",
            "name": "execute_code",
            "description": "Evaluate Starlark in the experimental host runtime. Complete interface: output(value) emits final tool output; host calls return script values but are not implicit final output. APIs: fs.read(path), fs.write(path, content), patch.apply(unified_diff), cmd[\"rg\"].run(args=[...], cwd=\".\"), cmd[\"git\"].status(), cmd[\"git\"].diff(args=[...]), cmd[\"cargo\"].check(args=[...]). No raw shell or unregistered binaries.",
            "parameters": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Starlark source using only: output(value); fs.read(path); fs.write(path, content); patch.apply(unified_diff); cmd[\"rg\"].run(args=[...], cwd=\".\"); cmd[\"git\"].status(); cmd[\"git\"].diff(args=[...]); cmd[\"cargo\"].check(args=[...]). Assign host-call return values and pass the desired final value to output(value)."
                    }
                },
                "required": ["source"]
            },
            "strict": true
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
    async fn request_tool_call(&self, role_instructions: &str, message: &str) -> Result<ModelToolTurn> {
        let tool = Self::execute_code_tool_schema();
        let instructions = format!(
            "{role_instructions}\n\nCall execute_code exactly once. Use output(value) in the Starlark source for final tool output; host calls only return script values."
        );
        let request_for_shape = ResponsesApiRequest {
            model: self.model.clone(),
            instructions,
            input: Vec::new(),
            tools: vec![tool.clone()],
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
        let request_shape = serde_json::to_value(&request_for_shape)?;
        let body = json!({
            "model": self.model,
            "instructions": request_for_shape.instructions,
            "input": [{
                "role": "user",
                "content": [{"type": "input_text", "text": message}]
            }],
            "tools": [tool],
            "tool_choice": {"type": "function", "name": "execute_code"},
            "parallel_tool_calls": false,
            "store": false,
            "stream": true,
            "prompt_cache_key": "robdex-agent-runtime-kernel-v1",
        });
        let raw_response = self.post_responses(&body).await?;
        let (call_id, source) = extract_execute_code_call(&raw_response)?;
        Ok(ModelToolTurn {
            provider: "chatgpt-codex-responses".to_string(),
            model: self.model.clone(),
            assistant_summary: "Live Responses model called execute_code.".to_string(),
            tool_call: ToolCallRequest {
                call_identity: call_id,
                tool_name: "execute_code".to_string(),
                source,
            },
            request_shape,
            raw_response,
        })
    }

    async fn submit_tool_result(
        &self,
        role_instructions: &str,
        tool_call_response: &Value,
        call_id: &str,
        tool_result: &Value,
    ) -> Result<ModelFinalTurn> {
        let result_text = serde_json::to_string(tool_result)?;
        let function_call_item = find_execute_code_item(tool_call_response)?;
        let instructions = format!(
            "{role_instructions}\n\nSummarize the execute_code tool result concisely."
        );
        let body = json!({
            "model": self.model,
            "instructions": instructions,
            "input": [
                function_call_item,
                {
                "type": "function_call_output",
                "call_id": call_id,
                "output": result_text
                }
            ],
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

fn find_execute_code_item(response: &Value) -> Result<Value> {
    for item in response
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if item.get("type").and_then(Value::as_str) == Some("function_call")
            && item.get("name").and_then(Value::as_str) == Some("execute_code")
        {
            return Ok(item.clone());
        }
    }
    bail!("model response did not include execute_code item")
}

fn extract_execute_code_call(response: &Value) -> Result<(String, String)> {
    for item in response
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if item.get("type").and_then(Value::as_str) == Some("function_call")
            && item.get("name").and_then(Value::as_str) == Some("execute_code")
        {
            let call_id = item
                .get("call_id")
                .or_else(|| item.get("id"))
                .and_then(Value::as_str)
                .context("execute_code function_call missing call_id")?
                .to_string();
            let arguments = item
                .get("arguments")
                .and_then(Value::as_str)
                .context("execute_code function_call missing arguments")?;
            let parsed: Value = serde_json::from_str(arguments)
                .context("execute_code function_call arguments are not JSON")?;
            let source = parsed
                .get("source")
                .and_then(Value::as_str)
                .context("execute_code function_call arguments missing source")?
                .to_string();
            return Ok((call_id, source));
        }
    }
    bail!("model response did not include execute_code function_call: {response}");
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
