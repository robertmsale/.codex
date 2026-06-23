use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{approvals, command_registry, compaction, db, model_input, routing};
use crate::lifecycle::{self, TerminalStatus};
use crate::model::codex_adapter::{bounded_raw_response, concise_response_summary, CodexBackedModelClient};
use crate::model::{ModelClient, ModelFinalTurn, ModelInitialTurn, RuntimeInputMessage};
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
        "roleEpoch": runtime_messages.iter().find_map(|message| message.metadata.get("roleEpoch").cloned()),
        "contextEpoch": runtime_messages.iter().find_map(|message| message.metadata.get("contextEpoch").cloned()),
        "contextEventWatermark": runtime_messages.iter().find_map(|message| message.metadata.get("contextEventWatermark").cloned()),
        "promptCacheKey": request_shape.get("prompt_cache_key").cloned(),
        "compactedStateIncluded": request_shape.get("input").and_then(Value::as_array).is_some_and(|items| {
            items.iter().any(|item| item.get("type").and_then(Value::as_str) == Some("compaction"))
        }),
    })
}

fn runtime_model_role_instructions(role_instructions: &str) -> String {
    let forced_prefix = ["Choose exactly one native", " tool per turn:"].concat();
    role_instructions
        .split(". ")
        .map(|sentence| {
            if sentence.trim_start().starts_with(&forced_prefix) {
                "Reply directly when no runtime work is needed. Use native tools only when the user's request requires runtime work".to_string()
            } else {
                sentence.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(". ")
}

fn model_tool_request_shape(
    role_snapshot: &crate::roles::RoleSnapshot,
    history: &[db::HistoryItem],
    runtime_messages: &[RuntimeInputMessage],
    execute_code_contract: &str,
    request_registry_contract: &str,
    message: &str,
    model: &str,
) -> Value {
    CodexBackedModelClient::request_tool_call_request_shape(
        model,
        role_snapshot,
        &model_input::model_history_from_items(history),
        runtime_messages,
        execute_code_contract,
        request_registry_contract,
        message,
    )
}

pub async fn send(pool: &PgPool, session_id: Uuid, message: &str) -> Result<Uuid> {
    let role_snapshot = db::session_role_snapshot(pool, session_id).await?;
    let model = CodexBackedModelClient::new_with_model(role_snapshot.model_defaults.model.clone())?;
    send_with_model_client(pool, session_id, message, &model, compaction::CompactionBudget::from_env()).await
}

pub async fn send_with_model_client<M: ModelClient + Sync + ?Sized>(
    pool: &PgPool,
    session_id: Uuid,
    message: &str,
    model: &M,
    budget: compaction::CompactionBudget,
) -> Result<Uuid> {
    let session = db::ensure_session_open(pool, session_id).await?;
    let workdir = session.workdir.clone();
    let role_snapshot = db::session_role_snapshot(pool, session_id).await?;
    let mut model_role_snapshot = role_snapshot.clone();
    model_role_snapshot.instruction_text = runtime_model_role_instructions(&role_snapshot.instruction_text);
    let model_role_instructions = model_role_snapshot.instruction_text.clone();
    let project_key = session.project_key.clone();
    let live_commands = command_registry::live_visible_commands(pool, &role_snapshot, project_key.as_deref()).await?;
    let previous_command_context = latest_command_context_evidence(pool, session_id).await?;
    let runtime_command_context = command_registry::runtime_command_context_message(&live_commands, previous_command_context.as_ref());
    let context_snapshot = model_input::persist_context_snapshot(pool, &session, &model_role_snapshot, &runtime_command_context.evidence, None).await?;
    let mut runtime_messages = model_input::runtime_developer_messages(&context_snapshot, &runtime_command_context);
    if let Some(requirements_message) = crate::requirements::requirements_runtime_message(pool, session_id).await? {
        runtime_messages.push(requirements_message);
    }
    let god_mode_shell_active = crate::god_mode::active_grant(pool, session_id).await?.is_some();
    let execute_code_contract = command_registry::stable_execute_code_contract_with_god_mode_shell(god_mode_shell_active);
    let request_registry_contract = command_registry::request_tool_contract();
    let prior_history_before_compaction = db::reconstructed_history(pool, session_id).await?;
    let pre_send_request_shape = model_tool_request_shape(
        &model_role_snapshot,
        &prior_history_before_compaction,
        &runtime_messages,
        &execute_code_contract,
        &request_registry_contract,
        message,
        role_snapshot.model_defaults.model.as_str(),
    );
    let pre_send_estimate = compaction::estimate_model_surfaces(&pre_send_request_shape, budget);
    if pre_send_estimate.total_estimated_tokens > budget.pre_send_threshold {
        compaction::compact_session_through_latest_completed_turn(pool, session_id, budget).await?;
        let rebuilt_history = db::reconstructed_history(pool, session_id).await?;
        let rebuilt_request_shape = model_tool_request_shape(
            &model_role_snapshot,
            &rebuilt_history,
            &runtime_messages,
            &execute_code_contract,
            &request_registry_contract,
            message,
            role_snapshot.model_defaults.model.as_str(),
        );
        let rebuilt_estimate = compaction::estimate_model_surfaces(&rebuilt_request_shape, budget);
        if rebuilt_estimate.total_estimated_tokens > budget.fail_closed_threshold {
            anyhow::bail!(
                "rebuilt model request estimate {} exceeds fail-closed threshold {}",
                rebuilt_estimate.total_estimated_tokens,
                budget.fail_closed_threshold
            );
        }
    }
    let prior_history = db::reconstructed_history(pool, session_id).await?;
    let model_history = model_input::model_history_from_items(&prior_history);

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
    sqlx::query("UPDATE session_context_snapshots SET turn_id=$1 WHERE session_id=$2 AND context_epoch=$3")
        .bind(turn_id)
        .bind(session_id)
        .bind(context_snapshot.context_epoch)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE session_context_events SET turn_id=$1 WHERE session_id=$2 AND context_epoch=$3")
        .bind(turn_id)
        .bind(session_id)
        .bind(context_snapshot.context_epoch)
        .execute(pool)
        .await?;
    let _route = match routing::decide_route(pool, session_id, Some(turn_id), &role_snapshot).await {
        Ok(route) => route,
        Err(error) => {
            finalize_failed_started_turn(pool, session_id, turn_id, "routing", &error.to_string()).await?;
            return Err(anyhow::anyhow!("routing failed after turn start: {error}"));
        }
    };

    let initial_turn = match model.request_tool_call(&model_role_snapshot, &model_history, &runtime_messages, &execute_code_contract, &request_registry_contract, message).await {
        Ok(turn) => turn,
        Err(error) => {
            finalize_failed_started_turn(pool, session_id, turn_id, "model_dispatch", &error.to_string()).await?;
            return Err(anyhow::anyhow!("model dispatch failed after turn start: {error}"));
        }
    };
    let plan = match initial_turn {
        ModelInitialTurn::ToolCall(plan) => plan,
        ModelInitialTurn::FinalResponse(final_response) => {
            complete_direct_final_response(
                pool,
                session_id,
                turn_id,
                &prior_history,
                &model_role_instructions,
                final_response,
                model,
                budget,
            )
            .await?;
            println!("turn {turn_id} completed");
            return Ok(turn_id);
        }
    };
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
                    "source": "session.role_snapshot.instruction_text.normalized_for_model",
                    "bytes": model_role_instructions.len(),
                    "prefix": model_role_instructions.chars().take(80).collect::<String>(),
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
            match (
                plan.tool_call.arguments.get("source").and_then(serde_json::Value::as_str),
                ExecutionRoot::new(&workdir),
            ) {
                (Some(source), Ok(root)) => execute_code(pool, session_id, turn_id, tool_call_id, source, &root, &role_snapshot)
                    .await
                    .map(|packet| serde_json::to_value(packet).unwrap_or_else(|error| json!({"ok": false, "error": error.to_string()}))),
                (None, _) => Err(anyhow::anyhow!("execute_code missing source")),
                (_, Err(error)) => Err(anyhow::anyhow!("invalid execution workdir: {error}")),
            }
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
    let final_response = match model
        .submit_tool_result(
            &model_role_snapshot,
            &model_history,
            &runtime_messages,
            &plan.raw_response,
            &plan.tool_call.call_identity,
            &result_json,
        )
        .await
    {
        Ok(final_response) => final_response,
        Err(error) => {
            finalize_failed_started_turn(pool, session_id, turn_id, "model_final_response", &error.to_string()).await?;
            return Err(anyhow::anyhow!("model final response failed after tool execution: {error}"));
        }
    };
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
                    "source": "session.role_snapshot.instruction_text.normalized_for_model",
                    "bytes": model_role_instructions.len(),
                    "prefix": model_role_instructions.chars().take(80).collect::<String>(),
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

    classify_requirements_final_response(pool, session_id, turn_id, &final_response.final_text, model, budget).await?;

    println!("turn {turn_id} {}", status.as_str());
    Ok(turn_id)
}

async fn complete_direct_final_response(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    prior_history: &[db::HistoryItem],
    role_instructions: &str,
    final_response: ModelFinalTurn,
    model: &(impl ModelClient + Sync + ?Sized),
    budget: compaction::CompactionBudget,
) -> Result<()> {
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
        Some("completed"),
        json!({
            "finalText": final_response.final_text,
            "request": {
                "model": final_response.request_shape.get("model").cloned(),
                "roleInstructions": {
                    "source": "session.role_snapshot.instruction_text.normalized_for_model",
                    "bytes": role_instructions.len(),
                    "prefix": role_instructions.chars().take(80).collect::<String>(),
                },
                "inputItems": final_response.request_shape.get("input").and_then(serde_json::Value::as_array).map(Vec::len),
                "history": {"items": prior_history.len(), "source": "reconstructed_session_history"},
            },
            "response": concise_response_summary(&final_response.raw_response),
        }),
    )
    .await?;

    lifecycle::complete_turn(pool, turn_id, TerminalStatus::Completed, Utc::now()).await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "turn",
        Some(turn_id),
        "turn.completed",
        Some("completed"),
        json!({"directAssistantResponse": true}),
    )
    .await?;
    classify_requirements_final_response(pool, session_id, turn_id, &final_response.final_text, model, budget).await?;
    Ok(())
}

async fn classify_requirements_final_response(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    final_text: &str,
    model: &(impl ModelClient + Sync + ?Sized),
    budget: compaction::CompactionBudget,
) -> Result<()> {
    let session = db::session_record(pool, session_id).await?;
    if session.session_kind == "requirementsReviewer" {
        let _ = crate::requirements::record_reviewer_verdict(pool, session_id, turn_id, final_text).await?;
    } else {
        if let Some(record) = crate::requirements::record_source_final_response(pool, session_id, turn_id, final_text).await?
            && record.outcome == crate::requirements::SourcePacketOutcome::Reviewable
            && let Some(reviewer_session_id) = record.reviewer_session_id
        {
            db::append_event(pool, session_id, Some(turn_id), "requirements", Some(record.packet_id), "requirements.reviewerDispatchQueued", Some("queued"), json!({"reviewerSessionId": reviewer_session_id, "requirementSetId": record.requirement_set_id})).await?;
            let status = crate::requirements::status(pool, session_id).await?;
            let packet_id_text = record.packet_id.to_string();
            let claim_packet = crate::requirements::packet_history(pool, session_id)
                .await?
                .into_iter()
                .find(|packet| packet["id"].as_str() == Some(packet_id_text.as_str()))
                .unwrap_or_else(|| json!({"id": record.packet_id}));
            let prompt = format!(
                "Review source Requirements claim packet for RequirementSet {set_id}.\n<source_claim_packet>{claim}</source_claim_packet>\n<requirement_progress>{progress}</requirement_progress>\nUse the canonical Requirements Review schema and return a verdict packet.",
                set_id = record.requirement_set_id,
                claim = claim_packet,
                progress = serde_json::to_string(&status.progress).unwrap_or_else(|_| "[]".to_string()),
            );
            let _ = Box::pin(send_with_model_client(pool, reviewer_session_id, &prompt, model, budget)).await?;
        }
    }
    Ok(())
}

async fn finalize_failed_started_turn(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    boundary: &str,
    error: &str,
) -> Result<()> {
    let model_event_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO model_events (id, session_id, turn_id, event_type, payload)
        VALUES ($1, $2, $3, 'final_response', $4)
        "#,
    )
    .bind(model_event_id)
    .bind(session_id)
    .bind(turn_id)
    .bind(json!({
        "summary": format!("Model request failed at {boundary}: {error}"),
        "provider": "runtime",
        "model": "real-model-adapter",
        "raw": {"error": error, "boundary": boundary},
    }))
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "model",
        Some(model_event_id),
        "model.final_response",
        Some("failed"),
        json!({
            "finalText": format!("Model request failed at {boundary}: {error}"),
            "provider": "runtime",
            "model": "real-model-adapter",
            "boundary": boundary,
        }),
    )
    .await?;
    lifecycle::complete_turn(pool, turn_id, TerminalStatus::Failed, Utc::now()).await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "turn",
        Some(turn_id),
        "turn.completed",
        Some("failed"),
        json!({"boundary": boundary, "error": error}),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::{LifecycleAuthorityMetadata, ManifestDecision, ModelDefaults, RoleSnapshot, RoutingMetadata, VisibilityMetadata};
    use std::collections::BTreeMap;

    fn role_snapshot(instruction_text: &str) -> RoleSnapshot {
        RoleSnapshot {
            id: "test-role".to_string(),
            version: "1.0.0".to_string(),
            display_name: "Test Role".to_string(),
            role_version_id: Uuid::new_v4(),
            instruction_text: instruction_text.to_string(),
            model_defaults: ModelDefaults { model: "model-proof".to_string(), reasoning_effort: "medium".to_string() },
            capabilities: vec!["tool.execute_code".to_string()],
            policy: BTreeMap::from([("tool.execute_code".to_string(), ManifestDecision::Allow)]),
            routing: RoutingMetadata { mode: "direct".to_string(), default_recipient: None, allowed_recipients: vec![], reserved_actions: vec![] },
            visibility: VisibilityMetadata { listed: true, owner_visible: true },
            lifecycle_authority: LifecycleAuthorityMetadata { can_spawn_agents: false, can_archive_agents: false, reserved_actions: vec![] },
            manifest: json!({}),
            created_at: Utc::now(),
        }
    }

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

    #[test]
    fn outbound_model_request_shape_uses_selected_session_model() {
        let request_shape = model_tool_request_shape(
            &role_snapshot("role instructions"),
            &[],
            &[],
            "execute contract",
            "registry contract",
            "send path model proof",
            "non-default-model-proof",
        );
        assert_eq!(request_shape["model"], "non-default-model-proof");
        println!("selected_model_send_request_model={}", request_shape["model"]);
    }

    #[test]
    fn outbound_model_request_does_not_force_a_tool_call() {
        let legacy_role = format!(
            "You are test. {} execute_code for available Starlark work, or request_command_registry_change when a registry command must be added or changed.",
            ["Choose exactly one native", " tool per turn:"].concat()
        );
        let request_shape = model_tool_request_shape(
            &role_snapshot(&runtime_model_role_instructions(&legacy_role)),
            &[],
            &[],
            "execute contract",
            "registry contract",
            "Hi",
            "model-proof",
        );
        assert_eq!(request_shape["tool_choice"], "auto");
        let input_text = serde_json::to_string(&request_shape["input"]).expect("input json");
        assert!(input_text.contains("Reply directly when no tool is needed"));
        assert!(!input_text.contains(&["Choose exactly one native", " tool"].concat()));
        assert!(!request_shape.as_object().expect("object").contains_key("instructions"));
    }

    #[test]
    fn prompt_cache_key_includes_role_and_context_epoch() {
        let role = role_snapshot("role instructions");
        let runtime_messages = vec![RuntimeInputMessage {
            text: "<runtime_context epoch=\"42\"></runtime_context>".to_string(),
            metadata: json!({"source":"runtime_context","contextEpoch":42}),
        }];
        let request_shape = model_tool_request_shape(
            &role,
            &[],
            &runtime_messages,
            "execute contract",
            "registry contract",
            "cache key proof",
            "model-proof",
        );
        let cache_key = request_shape["prompt_cache_key"].as_str().expect("cache key");
        assert!(cache_key.len() <= 64);
        assert!(cache_key.starts_with("rar2:"));
        assert!(cache_key.ends_with(":c42"));

        let changed_role = role_snapshot("changed role instructions");
        let changed_request_shape = model_tool_request_shape(
            &changed_role,
            &[],
            &runtime_messages,
            "execute contract",
            "registry contract",
            "cache key proof",
            "model-proof",
        );
        let changed_role_cache_key = changed_request_shape["prompt_cache_key"].as_str().expect("cache key");
        assert_ne!(cache_key, changed_role_cache_key);

        let changed_runtime_messages = vec![RuntimeInputMessage {
            text: "<runtime_context epoch=\"43\"></runtime_context>".to_string(),
            metadata: json!({"source":"runtime_context","contextEpoch":43}),
        }];
        let changed_context_shape = model_tool_request_shape(
            &role,
            &[],
            &changed_runtime_messages,
            "execute contract",
            "registry contract",
            "cache key proof",
            "model-proof",
        );
        let changed_context_cache_key = changed_context_shape["prompt_cache_key"].as_str().expect("cache key");
        assert_ne!(cache_key, changed_context_cache_key);
        assert!(changed_context_cache_key.ends_with(":c43"));
    }
}
