use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{approvals, command_registry, db, routing};
use crate::lifecycle::{self, TerminalStatus};
use crate::model::codex_adapter::{bounded_raw_response, concise_response_summary, CodexBackedModelClient};
use crate::model::ModelClient;
use crate::policy::PolicyEngine;
use crate::starlark_host::{ExecutionRoot, execute_code};

pub async fn send(pool: &PgPool, session_id: Uuid, message: &str, workdir: &str) -> Result<()> {
    if !db::session_exists(pool, session_id).await? {
        bail!("session is not tracked: {session_id}");
    }
    let role_snapshot = db::session_role_snapshot(pool, session_id).await?;

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

    let live_commands = command_registry::live_visible_commands(pool, &role_snapshot).await?;
    let execute_code_contract = command_registry::execute_code_contract(&live_commands);
    let model = CodexBackedModelClient::new_with_model(role_snapshot.model_defaults.model.clone())?;
    let plan = model.request_tool_call(&role_snapshot.instruction_text, &execute_code_contract, message).await?;
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
            },
            "response": concise_response_summary(&plan.raw_response),
        }),
    )
    .await?;

    let tool_call_id = Uuid::new_v4();
    let tool_policy = PolicyEngine::decide(
        &role_snapshot,
        "tool.execute_code",
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
            "action": "tool.execute_code",
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
        return Ok(());
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
    .bind(json!({"source": plan.tool_call.source}))
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

    let root = ExecutionRoot::new(workdir).context("invalid execution workdir")?;
    let result = execute_code(
        pool,
        session_id,
        turn_id,
        tool_call_id,
        &plan.tool_call.source,
        &root,
        &role_snapshot,
    )
    .await;

    let (status, result_json) = match result {
        Ok(packet) => (TerminalStatus::Completed, serde_json::to_value(packet)?),
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
    Ok(())
}
