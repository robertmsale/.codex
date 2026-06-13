use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{approvals, command_registry, db, routing};
use crate::lifecycle::{self, TerminalStatus};
use crate::model::codex_adapter::{bounded_raw_response, concise_response_summary, CodexBackedModelClient};
use crate::model::{ModelClient, ModelHistoryItem};
use crate::policy::PolicyEngine;
use crate::starlark_host::{ExecutionRoot, execute_code};

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
    let execute_code_contract = command_registry::execute_code_contract(&live_commands);
    let request_registry_contract = command_registry::request_tool_contract();
    let model = CodexBackedModelClient::new_with_model(role_snapshot.model_defaults.model.clone())?;
    let plan = model.request_tool_call(&role_snapshot.instruction_text, &model_history, &execute_code_contract, &request_registry_contract, message).await?;
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
        "requestShape": plan.request_shape.clone(),
        "raw": bounded_raw_response(&plan.raw_response),
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
