use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{approvals, command_registry, db, routing};
use crate::lifecycle::{self, TerminalStatus};
use crate::model::codex_adapter::{bounded_raw_response, concise_response_summary, CodexBackedModelClient};
use crate::model::{ModelClient, ModelHistoryItem, RuntimeInputMessage};
use crate::policy::PolicyEngine;
use crate::starlark_host::{ExecutionRoot, execute_code};


async fn latest_command_context_evidence(pool: &PgPool, session_id: Uuid) -> Result<Option<command_registry::CommandContextEvidence>> {
    let row: Option<Value> = sqlx::query_scalar(
        r#"
        SELECT payload->'commandContext'
        FROM model_events
        WHERE session_id=$1
          AND event_type='assistant_message'
          AND payload ? 'commandContext'
        ORDER BY created_at DESC, ordinal DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    row.map(serde_json::from_value).transpose().map_err(anyhow::Error::from)
}

fn tool_request_summaries(request_shape: &Value) -> Value {
    let tools = request_shape
        .get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .map(|tool| {
                    json!({
                        "type": tool.get("type").and_then(Value::as_str),
                        "name": tool.get("name").and_then(Value::as_str),
                        "strict": tool.get("strict").and_then(Value::as_bool),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!(tools)
}

fn model_request_evidence(
    request_shape: &Value,
    command_context: &command_registry::CommandContextEvidence,
    runtime_messages: &[RuntimeInputMessage],
) -> Value {
    json!({
        "model": request_shape.get("model").cloned(),
        "toolChoice": request_shape.get("tool_choice").cloned(),
        "toolCount": request_shape.get("tools").and_then(Value::as_array).map(Vec::len).unwrap_or_default(),
        "tools": tool_request_summaries(request_shape),
        "inputItems": request_shape.get("input").and_then(Value::as_array).map(Vec::len).unwrap_or_default(),
        "runtimeInputMessages": runtime_messages
            .iter()
            .map(|message| json!({"metadata": message.metadata}))
            .collect::<Vec<_>>(),
        "commandContext": serde_json::to_value(command_context).unwrap_or(Value::Null),
    })
}

pub async fn send(pool: &PgPool, session_id: Uuid, message: &str) -> Result<Uuid> {
    let session = db::ensure_session_open(pool, session_id).await?;
    let workdir = session.workdir.clone();
    let role_snapshot = db::session_role_snapshot(pool, session_id).await?;
    let project_key = session.project_key.clone();
    let prior_history = db::reconstructed_history(pool, session_id).await?;
    let model_history: Vec<ModelHistoryItem> = prior_history
        .iter()
        .map(|item| ModelHistoryItem {
            session_id: item.session_id.to_string(),
            turn_id: item.turn_id.to_string(),
            user: item.user.clone(),
            assistant: item.assistant.clone(),
            started_at: item.started_at.to_rfc3339(),
        })
        .collect();

    let turn_id = Uuid::new_v4();
    let turn_started = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO turns (id, session_id, role, input_text, status, started_at)
        VALUES ($1, $2, 'user', $3, 'running', $4)
        "#,
    )
    .bind(turn_id)
    .bind(session_id)
    .bind(message)
    .bind(turn_started)
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "turn",
        Some(turn_id),
        "turn.started",
        Some("running"),
        json!({"input": message}),
    )
    .await?;
    let _route = routing::decide_route(pool, session_id, Some(turn_id), &role_snapshot).await?;

    let live_commands = command_registry::live_visible_commands(pool, &role_snapshot, project_key.as_deref()).await?;
    let previous_command_context = latest_command_context_evidence(pool, session_id).await?;
    let runtime_command_context = command_registry::runtime_command_context_message(&live_commands, previous_command_context.as_ref());
    let runtime_messages = vec![RuntimeInputMessage {
        text: runtime_command_context.text.clone(),
        metadata: runtime_command_context.metadata.clone(),
    }];
    let execute_code_contract = command_registry::stable_execute_code_contract();
    let request_registry_contract = command_registry::request_tool_contract();
    let model = CodexBackedModelClient::new_with_model(role_snapshot.model_defaults.model.clone())?;
    let plan = model.request_tool_call(&role_snapshot.instruction_text, &model_history, &runtime_messages, &execute_code_contract, &request_registry_contract, message).await?;
    let request_evidence = model_request_evidence(&plan.request_shape, &runtime_command_context.evidence, &runtime_messages);
    let model_event_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO model_events (id, session_id, turn_id, event_type, payload)
        VALUES ($1, $2, $3, 'assistant_message', $4)
        "#,
    )
    .bind(model_event_id)
    .bind(session_id)
    .bind(turn_id)
    .bind(json!({
        "provider": plan.provider,
        "model": plan.model,
        "summary": plan.assistant_summary,
        "tool": plan.tool_call.tool_name,
        "request": request_evidence,
        "raw": bounded_raw_response(&plan.raw_response),
        "commandContext": serde_json::to_value(&runtime_command_context.evidence).unwrap_or(Value::Null),
    }))
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "model",
        Some(model_event_id),
        "model.tool_call",
        Some("completed"),
        json!({
            "provider": plan.provider,
            "model": plan.model,
            "summary": plan.assistant_summary,
            "tool": plan.tool_call.tool_name,
            "request": {
                "model": plan.request_shape.get("model").cloned(),
                "roleInstructions": {
                    "source": "session.role_snapshot.instruction_text",
                    "bytes": role_snapshot.instruction_text.len(),
                    "prefix": role_snapshot.instruction_text.chars().take(80).collect::<String>(),
                },
                "toolChoice": plan.request_shape.get("tool_choice").cloned(),
                "tools": plan.request_shape.get("tools").and_then(serde_json::Value::as_array).map(Vec::len),
                "executeCodeContract": execute_code_contract,
                "requestCommandRegistryChangeContract": request_registry_contract,
                "history": {"items": prior_history.len(), "source": "reconstructed_session_history"},
                "commandContext": serde_json::to_value(&runtime_command_context.evidence).unwrap_or(Value::Null),
                "runtimeInputMessages": [{"source":"runtime_command_context", "metadata": runtime_command_context.metadata.clone()}],
            },
            "response": concise_response_summary(&plan.raw_response),
        }),
    )
    .await?;

    let tool_call_id = Uuid::new_v4();
    let tool_action = format!("tool.{}", plan.tool_call.tool_name);
    let tool_policy = PolicyEngine::decide(
        &role_snapshot,
            &tool_action,
            json!({"tool": plan.tool_call.tool_name, "identity": plan.tool_call.call_identity}),
    );
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "policy",
        None,
        "policy.decision",
        Some(tool_policy.decision.as_str()),
        tool_policy.to_event_payload(),
    )
    .await?;

    if !tool_policy.decision.can_execute() {
        let approval_request_id = if tool_policy.decision.as_str() == "approvalRequired" {
            Some(approvals::request_approval(pool, session_id, Some(turn_id), &tool_policy, &role_snapshot).await?)
        } else {
            None
        };
        let result_json = json!({
            "ok": false,
            "blocked": true,
            "action": format!("tool.{}", plan.tool_call.tool_name),
            "decision": tool_policy.decision.as_str(),
            "reason": tool_policy.reason,
            "approvalRequestId": approval_request_id,
        });
        lifecycle::complete_turn(pool, turn_id, TerminalStatus::Failed, Utc::now()).await?;
        db::append_event(
            pool,
            session_id,
            Some(turn_id),
            "turn",
            Some(turn_id),
            "turn.completed",
            Some("failed"),
            json!({"result": result_json}),
        )
        .await?;
        println!("turn {turn_id} blocked");
        return Ok(turn_id);
    }

    sqlx::query(
        r#"
        INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status, started_at)
        VALUES ($1, $2, $3, $4, $5, $6, 'running', $7)
        "#,
    )
    .bind(tool_call_id)
    .bind(session_id)
    .bind(turn_id)
    .bind(&plan.tool_call.tool_name)
    .bind(&plan.tool_call.call_identity)
    .bind(plan.tool_call.arguments.clone())
    .bind(Utc::now())
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "tool",
        Some(tool_call_id),
        "tool.started",
        Some("running"),
        json!({"tool": plan.tool_call.tool_name, "identity": plan.tool_call.call_identity}),
    )
    .await?;

    let result = match plan.tool_call.tool_name.as_str() {
        "execute_code" => {
            let source = plan.tool_call.arguments.get("source").and_then(serde_json::Value::as_str).context("execute_code missing source")?;
            let root = ExecutionRoot::new(&workdir).context("invalid execution workdir")?;
            execute_code(pool, session_id, turn_id, tool_call_id, source, &root, &role_snapshot)
                .await
                .map(|packet| serde_json::to_value(packet).unwrap_or_else(|error| json!({"ok": false, "error": error.to_string()})))
        }
        "request_command_registry_change" => {
            let input: command_registry::NativeRegistryChangeRequest = serde_json::from_value(plan.tool_call.arguments.clone())?;
            command_registry::create_native_model_request(pool, session_id, turn_id, input, &role_snapshot, project_key.as_deref())
                .await
                .map(|request_id| json!({"ok": true, "requestId": request_id, "status": "pending"}))
        }
        other => Err(anyhow::anyhow!("unsupported native tool: {other}")),
    };

    let (status, result_json) = match result {
        Ok(packet) => (TerminalStatus::Completed, packet),
        Err(error) => (TerminalStatus::Failed, json!({"ok": false, "error": error.to_string()})),
    };

    lifecycle::complete_tool_call(pool, tool_call_id, status, &result_json, Utc::now()).await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "tool",
        Some(tool_call_id),
        "tool.completed",
        Some(status.as_str()),
        json!({"result": result_json.clone()}),
    )
    .await?;
    let final_response = model
        .submit_tool_result(
            &role_snapshot.instruction_text,
            &model_history,
            &plan.raw_response,
            &plan.tool_call.call_identity,
            &result_json,
        )
        .await?;
    let final_model_event_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO model_events (id, session_id, turn_id, event_type, payload)
        VALUES ($1, $2, $3, 'final_response', $4)
        "#,
    )
    .bind(final_model_event_id)
    .bind(session_id)
    .bind(turn_id)
    .bind(json!({
        "summary": final_response.final_text,
        "provider": final_response.provider,
        "model": final_response.model,
        "raw": bounded_raw_response(&final_response.raw_response),
    }))
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "model",
        Some(final_model_event_id),
        "model.final_response",
        Some(status.as_str()),
        json!({
            "afterToolResult": result_json.clone(),
            "finalText": final_response.final_text,
            "request": {
                "model": final_response.request_shape.get("model").cloned(),
                "roleInstructions": {
                    "source": "session.role_snapshot.instruction_text",
                    "bytes": role_snapshot.instruction_text.len(),
                    "prefix": role_snapshot.instruction_text.chars().take(80).collect::<String>(),
                },
                "inputItems": final_response.request_shape.get("input").and_then(serde_json::Value::as_array).map(Vec::len),
                "history": {"items": prior_history.len(), "source": "reconstructed_session_history"},
            },
            "response": concise_response_summary(&final_response.raw_response),
        }),
    )
    .await?;

    lifecycle::complete_turn(pool, turn_id, status, Utc::now()).await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "turn",
        Some(turn_id),
        "turn.completed",
        Some(status.as_str()),
        json!({}),
    )
    .await?;

    println!("turn {turn_id} {}", status.as_str());
    Ok(turn_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_request_evidence_excludes_synthetic_catalog_text() {
        let catalog_text = "Runtime command context cmdctx-test\nVisible commands:\n- cmd.secret.catalog: large synthetic catalog text";
        let request_shape = json!({
            "model": "gpt-test",
            "input": [
                {"role":"user","content":[{"type":"input_text","text": catalog_text}],"metadata":{"source":"runtime_command_context","commandContextId":"cmdctx-test"}},
                {"role":"user","content":[{"type":"input_text","text":"ordinary prompt"}]}
            ],
            "tools": [
                {"type":"function","name":"execute_code","description":"stable execute contract","strict":true},
                {"type":"function","name":"request_command_registry_change","description":"stable registry contract","strict":true}
            ],
            "tool_choice": "auto"
        });
        let context = command_registry::CommandContextEvidence {
            id: "cmdctx-test".to_string(),
            catalog_included: true,
            visible_count: 1,
            added_count: 1,
            removed_count: 0,
            changed_count: 0,
            summaries: vec![],
        };
        let runtime_messages = vec![RuntimeInputMessage {
            text: catalog_text.to_string(),
            metadata: json!({"source":"runtime_command_context","commandContextId":"cmdctx-test"}),
        }];
        let evidence = model_request_evidence(&request_shape, &context, &runtime_messages);
        let evidence_text = serde_json::to_string(&evidence).expect("evidence json");
        assert!(!evidence_text.contains("large synthetic catalog text"));
        assert!(!evidence_text.contains("ordinary prompt"));
        assert_eq!(evidence["model"], "gpt-test");
        assert_eq!(evidence["toolCount"], 2);
        assert_eq!(evidence["tools"][0]["name"], "execute_code");
        assert_eq!(evidence["runtimeInputMessages"][0]["metadata"]["source"], "runtime_command_context");
        assert_eq!(evidence["commandContext"]["id"], "cmdctx-test");
        assert_eq!(evidence["commandContext"]["catalogIncluded"], true);
    }
}
