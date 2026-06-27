use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{approvals, command_registry, compaction, db, model_input, routing};
use crate::lifecycle::{self, TerminalStatus};
use crate::model::codex_adapter::{bounded_raw_response, concise_response_summary, CodexBackedModelClient};
use crate::model::{ModelClient, ModelFinalTurn, ModelHistoryItem, ModelInitialTurn, RuntimeInputMessage};
use crate::policy::PolicyEngine;
use crate::starlark_host::{ExecutionRoot, execute_code};

async fn append_transcript_item(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    source_table: Option<&str>,
    source_id: Option<Uuid>,
    item_type: &str,
    role: &str,
    content: &str,
    payload: Value,
) -> Result<()> {
    let stable_key = match (source_table, source_id) {
        (Some(table), Some(id)) => format!("{table}:{id}:{item_type}"),
        _ => format!("{item_type}:{role}:{content}"),
    };
    sqlx::query(
        r#"
        INSERT INTO current_turn_transcript_items (
            id, session_id, turn_id, source_table, source_id, item_type, role, content, payload, stable_key
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
        ON CONFLICT (turn_id, stable_key) WHERE stable_key <> '' DO UPDATE SET
            content = EXCLUDED.content,
            payload = EXCLUDED.payload
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(turn_id)
    .bind(source_table)
    .bind(source_id)
    .bind(item_type)
    .bind(role)
    .bind(content)
    .bind(payload)
    .bind(stable_key)
    .execute(pool)
    .await?;
    Ok(())
}

async fn output_artifact_summaries(pool: &PgPool, source_column: &str, source_id: Uuid) -> Result<Value> {
    let sql = match source_column {
        "script_run_id" => "SELECT id, stream, byte_count, line_count FROM execution_output_artifacts WHERE script_run_id=$1 AND stream IN ('stdout','stderr') ORDER BY stream ASC, created_at ASC",
        "command_run_id" => "SELECT id, stream, byte_count, line_count FROM execution_output_artifacts WHERE command_run_id=$1 AND stream IN ('stdout','stderr') ORDER BY stream ASC, created_at ASC",
        "process_id" => "SELECT id, stream, byte_count, line_count FROM execution_output_artifacts WHERE process_id=$1 AND stream IN ('stdout','stderr') ORDER BY stream ASC, created_at ASC",
        _ => anyhow::bail!("unsupported artifact summary source column: {source_column}"),
    };
    let rows = sqlx::query(sql).bind(source_id).fetch_all(pool).await?;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    for row in rows {
        let summary = json!({
            "artifactId": row.get::<Uuid, _>("id"),
            "byteCount": row.get::<i64, _>("byte_count"),
            "lineCount": row.get::<i64, _>("line_count"),
            "retrieval": "Use outputs.head/tail/slice/search/stats with this artifact id inside the owning session.",
        });
        match row.get::<String, _>("stream").as_str() {
            "stdout" => stdout.push(summary),
            "stderr" => stderr.push(summary),
            _ => {}
        }
    }
    Ok(json!({"stdout": stdout, "stderr": stderr}))
}

async fn output_artifact_summaries_by_ids(pool: &PgPool, stdout_id: Option<Uuid>, stderr_id: Option<Uuid>) -> Result<Value> {
    let ids = [stdout_id, stderr_id].into_iter().flatten().collect::<Vec<_>>();
    if ids.is_empty() {
        return Ok(json!({"stdout": [], "stderr": []}));
    }
    let rows = sqlx::query("SELECT id, stream, byte_count, line_count FROM execution_output_artifacts WHERE id = ANY($1) AND stream IN ('stdout','stderr') ORDER BY stream ASC, created_at ASC")
        .bind(&ids)
        .fetch_all(pool)
        .await?;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    for row in rows {
        let summary = json!({
            "artifactId": row.get::<Uuid, _>("id"),
            "byteCount": row.get::<i64, _>("byte_count"),
            "lineCount": row.get::<i64, _>("line_count"),
            "retrieval": "Use outputs.head/tail/slice/search/stats with this artifact id inside the owning session.",
        });
        match row.get::<String, _>("stream").as_str() {
            "stdout" => stdout.push(summary),
            "stderr" => stderr.push(summary),
            _ => {}
        }
    }
    Ok(json!({"stdout": stdout, "stderr": stderr}))
}

fn tool_result_summary(result_json: &Value) -> Value {
    json!({
        "ok": result_json.get("ok").and_then(Value::as_bool),
        "status": result_json.get("status").and_then(Value::as_str),
        "scriptRunId": result_json.get("scriptRunId"),
        "output": {
            "message": result_json.pointer("/output/message").and_then(Value::as_str),
            "stdoutArtifact": result_json.pointer("/output/stdoutArtifact").cloned(),
            "stderrArtifact": result_json.pointer("/output/stderrArtifact").cloned(),
        },
    })
}

fn explicit_output_preview(result_json: &Value) -> Option<String> {
    result_json
        .pointer("/output/stdoutArtifact/preview")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            result_json
                .pointer("/output/stdoutArtifact/tail")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
        })
        .map(|value| {
            let limit = 2_000;
            if value.len() > limit {
                format!("{}…", value.chars().take(limit).collect::<String>())
            } else {
                value.to_string()
            }
        })
}

fn artifact_handle_text(artifacts: &Value) -> String {
    let mut parts = Vec::new();
    for stream in ["stdout", "stderr"] {
        if let Some(items) = artifacts.get(stream).and_then(Value::as_array) {
            for item in items {
                if let Some(id) = item.get("artifactId").and_then(Value::as_str) {
                    let bytes = item.get("byteCount").and_then(Value::as_i64).unwrap_or_default();
                    let lines = item.get("lineCount").and_then(Value::as_i64).unwrap_or_default();
                    parts.push(format!("{stream} artifact {id} ({bytes} bytes, {lines} lines)"));
                }
            }
        }
    }
    if parts.is_empty() {
        "no stdout/stderr artifacts recorded".to_string()
    } else {
        parts.join("; ")
    }
}

pub(crate) async fn persist_tool_boundary_transcript(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    tool_call_id: Uuid,
    user_message: &str,
    assistant_summary: &str,
    result_json: &Value,
) -> Result<ModelHistoryItem> {
    append_transcript_item(pool, session_id, turn_id, Some("turns"), Some(turn_id), "initial_user_input", "user", user_message, json!({})).await?;
    append_transcript_item(pool, session_id, turn_id, Some("model_events"), None, "assistant_intermediate", "assistant", assistant_summary, json!({})).await?;
    append_transcript_item(pool, session_id, turn_id, Some("tool_calls"), Some(tool_call_id), "tool_call", "tool", "execute_code", json!({"toolCallId": tool_call_id})).await?;
    let tool_summary = tool_result_summary(result_json);
    let tool_result_content = match explicit_output_preview(result_json) {
        Some(preview) => format!("execute_code completed; explicit bounded output: {preview}"),
        None => "execute_code completed; bounded explicit output and stdout/stderr artifact metadata are recorded separately.".to_string(),
    };
    append_transcript_item(
        pool,
        session_id,
        turn_id,
        Some("tool_calls"),
        Some(tool_call_id),
        "tool_result",
        "tool",
        &tool_result_content,
        json!({"toolCallId": tool_call_id, "summary": tool_summary}),
    )
    .await?;
    let command_rows = sqlx::query(
        r#"
        SELECT cr.id, cr.binary_name, cr.argv, cr.cwd, cr.status, cr.exit_status, cr.duration_ms,
               octet_length(cr.stdout) AS stdout_bytes, octet_length(cr.stderr) AS stderr_bytes
        FROM command_runs cr
        JOIN host_api_calls ha ON ha.id = cr.host_api_call_id
        JOIN script_runs sr ON sr.id = ha.script_run_id
        WHERE sr.tool_call_id = $1
        ORDER BY cr.started_at ASC, cr.id ASC
        "#,
    )
    .bind(tool_call_id)
    .fetch_all(pool)
    .await?;
    for row in command_rows {
        let id: Uuid = row.get("id");
        let binary: String = row.get("binary_name");
        let status: String = row.get("status");
        let artifacts = output_artifact_summaries(pool, "command_run_id", id).await?;
        append_transcript_item(
            pool,
            session_id,
            turn_id,
            Some("command_runs"),
            Some(id),
            "command_registry_process",
            "tool",
            &format!("command {binary} status={status}; stdout/stderr are available only through artifact handles: {}", artifact_handle_text(&artifacts)),
            json!({
                "commandRunId": id,
                "binary": binary,
                "argv": row.get::<Value, _>("argv"),
                "cwd": row.get::<String, _>("cwd"),
                "status": status,
                "exitStatus": row.get::<Option<i32>, _>("exit_status"),
                "durationMs": row.get::<Option<i64>, _>("duration_ms"),
                "byteCounts": {"stdout": row.get::<Option<i32>, _>("stdout_bytes"), "stderr": row.get::<Option<i32>, _>("stderr_bytes")},
                "artifacts": artifacts,
            }),
        )
        .await?;
    }
    let shell_rows = sqlx::query(
        r#"
        SELECT id, left(script_source, 120) AS script_preview, cwd, status, process_id, exit_status, duration_ms,
               stdout_artifact_id, stderr_artifact_id
        FROM shell_runs
        WHERE tool_call_id = $1
        ORDER BY started_at ASC, id ASC
        "#,
    )
    .bind(tool_call_id)
    .fetch_all(pool)
    .await?;
    for row in shell_rows {
        let id: Uuid = row.get("id");
        let status: String = row.get("status");
        let artifacts = output_artifact_summaries_by_ids(
            pool,
            row.get::<Option<Uuid>, _>("stdout_artifact_id"),
            row.get::<Option<Uuid>, _>("stderr_artifact_id"),
        ).await?;
        append_transcript_item(
            pool,
            session_id,
            turn_id,
            Some("shell_runs"),
            Some(id),
            "god_mode_shell_process",
            "tool",
            &format!("God Mode shell status={status}; stdout/stderr are available only through artifact handles: {}", artifact_handle_text(&artifacts)),
            json!({
                "shellRunId": id,
                "processId": row.get::<Option<Uuid>, _>("process_id"),
                "cwd": row.get::<String, _>("cwd"),
                "status": status,
                "exitStatus": row.get::<Option<i32>, _>("exit_status"),
                "durationMs": row.get::<Option<i64>, _>("duration_ms"),
                "scriptPreview": row.get::<String, _>("script_preview"),
                "artifacts": artifacts,
            }),
        )
        .await?;
    }
    let process_rows = sqlx::query(
        r#"
        SELECT id, handle, binary_name, status, metadata
        FROM managed_processes
        WHERE starting_turn_id = $1
        ORDER BY start_time ASC, id ASC
        "#,
    )
    .bind(turn_id)
    .fetch_all(pool)
    .await?;
    for row in process_rows {
        let id: Uuid = row.get("id");
        let handle: String = row.get("handle");
        let binary: String = row.get("binary_name");
        let status: String = row.get("status");
        let artifacts = output_artifact_summaries(pool, "process_id", id).await?;
        append_transcript_item(
            pool,
            session_id,
            turn_id,
            Some("managed_processes"),
            Some(id),
            "managed_async_process",
            "tool",
            &format!("managed process {handle} {binary} status={status}; stdout/stderr are available only through artifact handles: {}", artifact_handle_text(&artifacts)),
            json!({"processId": id, "handle": handle, "status": status, "artifacts": artifacts, "metadata": row.get::<Value, _>("metadata")}),
        )
        .await?;
    }
    let rows = sqlx::query(
        "SELECT item_type, role, content FROM current_turn_transcript_items WHERE turn_id=$1 ORDER BY ordering_key ASC",
    )
    .bind(turn_id)
    .fetch_all(pool)
    .await?;
    let transcript_text = rows
        .iter()
        .map(|row| format!("{} [{}]: {}", row.get::<String, _>("role"), row.get::<String, _>("item_type"), row.get::<String, _>("content")))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(ModelHistoryItem {
        session_id: session_id.to_string(),
        turn_id: turn_id.to_string(),
        user: user_message.to_string(),
        assistant: Some(transcript_text),
        started_at: Utc::now().to_rfc3339(),
        source: "current_turn_transcript".to_string(),
        checkpoint_id: None,
    })
}

pub(crate) async fn continue_pending_steering_after_operation_boundary(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    model: &(impl ModelClient + Sync + ?Sized),
) -> Result<bool> {
    let Some(pending) = db::next_accepted_submitted_input_for_turn(pool, session_id, turn_id).await? else {
        return Ok(false);
    };
    let transcript_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM current_turn_transcript_items WHERE turn_id=$1")
        .bind(turn_id)
        .fetch_one(pool)
        .await?;
    if transcript_count == 0 {
        let user_message: String = sqlx::query_scalar("SELECT input_text FROM turns WHERE id=$1")
            .bind(turn_id)
            .fetch_one(pool)
            .await?;
        if let Some(tool_call_id) = sqlx::query_scalar::<_, Uuid>("SELECT id FROM tool_calls WHERE turn_id=$1 ORDER BY started_at ASC, id ASC LIMIT 1")
            .bind(turn_id)
            .fetch_optional(pool)
            .await?
        {
            let result_json: Value = sqlx::query_scalar("SELECT COALESCE(result, '{}'::jsonb) FROM tool_calls WHERE id=$1")
                .bind(tool_call_id)
                .fetch_one(pool)
                .await?;
            persist_tool_boundary_transcript(pool, session_id, turn_id, tool_call_id, &user_message, "operation boundary completed", &result_json).await?;
        } else {
            append_transcript_item(pool, session_id, turn_id, Some("turns"), Some(turn_id), "initial_user_input", "user", &user_message, json!({})).await?;
        }
    }
    db::mark_submitted_input_applied(pool, pending.id, turn_id).await?;
    append_transcript_item(pool, session_id, turn_id, Some("submitted_inputs"), Some(pending.id), "applied_steering", "user", &pending.content, json!({"submittedInputId": pending.id})).await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "submitted_input",
        Some(pending.id),
        "submitted_input.applied",
        Some("applied"),
        json!({"submittedInputId": pending.id, "placementTurnId": turn_id, "sameTurnContinuation": true, "boundary": "operation_completed"}),
    )
    .await?;
    let role = db::session_role_snapshot(pool, session_id).await?;
    let rows = sqlx::query("SELECT role, item_type, content FROM current_turn_transcript_items WHERE turn_id=$1 ORDER BY ordering_key ASC")
        .bind(turn_id)
        .fetch_all(pool)
        .await?;
    let user_message: String = sqlx::query_scalar("SELECT input_text FROM turns WHERE id=$1")
        .bind(turn_id)
        .fetch_one(pool)
        .await?;
    let transcript = rows
        .iter()
        .map(|row| format!("{} [{}]: {}", row.get::<String, _>("role"), row.get::<String, _>("item_type"), row.get::<String, _>("content")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut history = model_input::model_history_from_items(&db::reconstructed_history(pool, session_id).await?);
    history.push(ModelHistoryItem {
        session_id: session_id.to_string(),
        turn_id: turn_id.to_string(),
        user: user_message,
        assistant: Some(transcript),
        started_at: Utc::now().to_rfc3339(),
        source: "current_turn_transcript".to_string(),
        checkpoint_id: None,
    });
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "model",
        None,
        "model.same_turn_continuation",
        Some("running"),
        json!({"submittedInputId": pending.id, "boundary": "operation_completed", "history": {"items": history.len(), "source": "completed_history_plus_current_turn_transcript"}}),
    )
    .await?;
    let session = db::session_record(pool, session_id).await?;
    let live_commands = command_registry::live_visible_commands(pool, &role, session.project_key.as_deref()).await?;
    let previous_command_context = latest_command_context_evidence(pool, session_id).await?;
    let command_context = command_registry::runtime_command_context_message(&live_commands, previous_command_context.as_ref());
    let runtime_messages = vec![RuntimeInputMessage {
        text: command_context.text,
        metadata: command_context.metadata,
    }];
    let execute_code_contract = command_registry::stable_execute_code_contract_with_god_mode_shell(crate::god_mode::active_grant(pool, session_id).await?.is_some());
    let request_registry_contract = command_registry::request_tool_contract();
    let _ = model.request_tool_call(&role, &history, &runtime_messages, &execute_code_contract, &request_registry_contract, &pending.content).await?;
    Ok(true)
}


async fn latest_command_context_evidence(pool: &PgPool, session_id: Uuid) -> Result<Option<command_registry::CommandContextEvidence>> {
    let row: Option<Value> = sqlx::query_scalar(
        r#"
        SELECT payload->'commandContext'
        FROM model_events
        WHERE session_id=$1
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
    let session = db::ensure_session_not_archived(pool, session_id).await?;
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
    if let Some(requirements_message) = crate::requirements::hook_defined_requirements_runtime_message(pool, session_id).await? {
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
    if context_snapshot.event_kind != "unchanged" {
        sqlx::query("UPDATE session_context_snapshots SET turn_id=$1 WHERE session_id=$2 AND context_epoch=$3")
            .bind(turn_id)
            .bind(session_id)
            .bind(context_snapshot.context_epoch)
            .execute(pool)
            .await?;
        sqlx::query("UPDATE session_context_events SET turn_id=$1 WHERE session_id=$2 AND context_epoch=$3 AND turn_id IS NULL")
            .bind(turn_id)
            .bind(session_id)
            .bind(context_snapshot.context_epoch)
            .execute(pool)
            .await?;
    }
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
                &runtime_command_context.evidence,
                &runtime_messages,
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
    let mut pending_direct_final = None;
    for _ in 0..20 {
        if let Some(pending) = db::next_accepted_submitted_input_for_turn(pool, session_id, turn_id).await? {
            pending_direct_final = Some(pending);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    if let Some(pending) = pending_direct_final {
        db::mark_submitted_input_applied(pool, pending.id, turn_id).await?;
        db::append_event(
            pool,
            session_id,
            Some(turn_id),
            "submitted_input",
            Some(pending.id),
            "submitted_input.applied",
            Some("applied"),
            json!({
                "submittedInputId": pending.id,
                "placementTurnId": turn_id,
                "disposition": pending.disposition,
                "sameTurnContinuation": true,
                "boundary": "tool_completed",
            }),
        )
        .await?;
        let mut continuation_history = model_history.clone();
        continuation_history.push(
            persist_tool_boundary_transcript(
                pool,
                session_id,
                turn_id,
                tool_call_id,
                message,
                &plan.assistant_summary,
                &result_json,
            )
            .await?,
        );
        append_transcript_item(pool, session_id, turn_id, Some("submitted_inputs"), Some(pending.id), "applied_steering", "user", &pending.content, json!({"submittedInputId": pending.id})).await?;
        db::append_event(
            pool,
            session_id,
            Some(turn_id),
            "model",
            Some(model_event_id),
            "model.same_turn_continuation",
            Some("running"),
            json!({
                "submittedInputId": pending.id,
                "boundary": "tool_completed",
                "history": {"items": continuation_history.len(), "source": "completed_history_plus_current_turn_transcript"},
            }),
        )
        .await?;
        match model
            .request_tool_call(
                &model_role_snapshot,
                &continuation_history,
                &runtime_messages,
                &execute_code_contract,
                &request_registry_contract,
                &pending.content,
            )
            .await?
        {
            ModelInitialTurn::FinalResponse(continued) => {
                let continued_event_id = Uuid::new_v4();
                sqlx::query(
                    r#"
                    INSERT INTO model_events (id, session_id, turn_id, event_type, payload)
                    VALUES ($1, $2, $3, 'final_response', $4)
                    "#,
                )
                .bind(continued_event_id)
                .bind(session_id)
                .bind(turn_id)
                .bind(json!({
                    "summary": continued.final_text,
                    "provider": continued.provider,
                    "model": continued.model,
                    "raw": bounded_raw_response(&continued.raw_response),
                    "sameTurnContinuation": true,
                    "submittedInputId": pending.id,
                    "boundary": "tool_completed",
                }))
                .execute(pool)
                .await?;
                db::append_event(
                    pool,
                    session_id,
                    Some(turn_id),
                    "model",
                    Some(continued_event_id),
                    "model.final_response",
                    Some("completed"),
                    json!({
                        "finalText": continued.final_text,
                        "sameTurnContinuation": true,
                        "submittedInputId": pending.id,
                        "boundary": "tool_completed",
                        "request": {
                            "history": {"items": continuation_history.len(), "source": "completed_history_plus_current_turn_transcript"},
                        },
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
                    json!({"sameTurnContinuation": true}),
                )
                .await?;
                classify_requirements_final_response(pool, session_id, turn_id, &continued.final_text, model, budget).await?;
                println!("turn {turn_id} {}", status.as_str());
                return Ok(turn_id);
            }
            ModelInitialTurn::ToolCall(next_plan) => {
                let continued_event_id = Uuid::new_v4();
                sqlx::query(
                    r#"
                    INSERT INTO model_events (id, session_id, turn_id, event_type, payload)
                    VALUES ($1, $2, $3, 'assistant_message', $4)
                    "#,
                )
                .bind(continued_event_id)
                .bind(session_id)
                .bind(turn_id)
                .bind(json!({
                    "summary": next_plan.assistant_summary,
                    "tool": next_plan.tool_call.tool_name,
                    "sameTurnContinuation": true,
                    "submittedInputId": pending.id,
                    "boundary": "tool_completed",
                    "request": {"history": {"items": continuation_history.len(), "source": "completed_history_plus_current_turn_transcript"}},
                }))
                .execute(pool)
                .await?;
                db::append_event(
                    pool,
                    session_id,
                    Some(turn_id),
                    "model",
                    Some(continued_event_id),
                    "model.tool_call",
                    Some("planned"),
                    json!({"tool": next_plan.tool_call.tool_name, "sameTurnContinuation": true, "submittedInputId": pending.id, "boundary": "tool_completed"}),
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
                    json!({"sameTurnContinuation": true, "continuedToolPlanned": next_plan.tool_call.tool_name}),
                )
                .await?;
                println!("turn {turn_id} {}", status.as_str());
                return Ok(turn_id);
            }
        }
    }
    let final_session = db::session_record(pool, session_id).await?;
    let final_role_snapshot = db::session_role_snapshot(pool, session_id).await?;
    let mut final_model_role_snapshot = final_role_snapshot.clone();
    final_model_role_snapshot.instruction_text =
        runtime_model_role_instructions(&final_role_snapshot.instruction_text);
    let final_model_role_instructions = final_model_role_snapshot.instruction_text.clone();
    let final_live_commands = command_registry::live_visible_commands(
        pool,
        &final_role_snapshot,
        final_session.project_key.as_deref(),
    )
    .await?;
    let final_previous_command_context = latest_command_context_evidence(pool, session_id).await?;
    let final_runtime_command_context =
        command_registry::runtime_command_context_message(&final_live_commands, final_previous_command_context.as_ref());
    let final_context_snapshot = model_input::persist_context_snapshot(
        pool,
        &final_session,
        &final_model_role_snapshot,
        &final_runtime_command_context.evidence,
        Some(turn_id),
    )
    .await?;
    let final_runtime_messages =
        model_input::runtime_developer_messages(&final_context_snapshot, &final_runtime_command_context);
    let final_execute_code_contract =
        command_registry::stable_execute_code_contract_with_god_mode_shell(crate::god_mode::active_grant(pool, session_id).await?.is_some());
    let final_request_registry_contract = command_registry::request_tool_contract();

    let final_response = match model
        .submit_tool_result(
            &final_model_role_snapshot,
            &model_history,
            &final_runtime_messages,
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
        "model.final_output_committed",
        Some("committed"),
        json!({"finalText": final_response.final_text, "mode": "tool_result_final"}),
    )
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
                    "bytes": final_model_role_instructions.len(),
                    "prefix": final_model_role_instructions.chars().take(80).collect::<String>(),
                },
                "commandContext": serde_json::to_value(&final_runtime_command_context.evidence).unwrap_or(Value::Null),
                "runtimeInputMessages": final_runtime_messages.iter().map(|message| message.metadata.clone()).collect::<Vec<_>>(),
                "executeCodeContract": final_execute_code_contract,
                "requestCommandRegistryChangeContract": final_request_registry_contract,
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
    command_context: &command_registry::CommandContextEvidence,
    runtime_messages: &[RuntimeInputMessage],
    final_response: ModelFinalTurn,
    model: &(impl ModelClient + Sync + ?Sized),
    budget: compaction::CompactionBudget,
) -> Result<()> {
    let final_model_event_id = Uuid::new_v4();
    let request_evidence = model_request_evidence(&final_response.request_shape, command_context, runtime_messages);
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
        "request": request_evidence,
        "raw": bounded_raw_response(&final_response.raw_response),
        "commandContext": serde_json::to_value(command_context).unwrap_or(Value::Null),
    }))
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "model",
        Some(final_model_event_id),
        "model.final_output_committed",
        Some("committed"),
        json!({"finalText": final_response.final_text, "mode": "direct_final"}),
    )
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

    if let Some(pending) = db::next_accepted_submitted_input_for_turn(pool, session_id, turn_id).await? {
        db::mark_submitted_input_applied(pool, pending.id, turn_id).await?;
        db::append_event(
            pool,
            session_id,
            Some(turn_id),
            "submitted_input",
            Some(pending.id),
            "submitted_input.applied",
            Some("applied"),
            json!({
                "submittedInputId": pending.id,
                "placementTurnId": turn_id,
                "disposition": pending.disposition,
                "sameTurnContinuation": true,
            }),
        )
        .await?;
        let mut continuation_history = model_input::model_history_from_items(prior_history);
        let initial_user: String = sqlx::query_scalar("SELECT input_text FROM turns WHERE id=$1")
            .bind(turn_id)
            .fetch_one(pool)
            .await?;
        append_transcript_item(pool, session_id, turn_id, Some("turns"), Some(turn_id), "initial_user_input", "user", &initial_user, json!({})).await?;
        append_transcript_item(pool, session_id, turn_id, Some("model_events"), Some(final_model_event_id), "assistant_final_text", "assistant", &final_response.final_text, json!({"sameTurnContinuation": true})).await?;
        append_transcript_item(pool, session_id, turn_id, Some("submitted_inputs"), Some(pending.id), "applied_steering", "user", &pending.content, json!({"submittedInputId": pending.id})).await?;
        continuation_history.push(ModelHistoryItem {
            session_id: session_id.to_string(),
            turn_id: turn_id.to_string(),
            user: initial_user,
            assistant: Some(final_response.final_text.clone()),
            started_at: Utc::now().to_rfc3339(),
            source: "current_turn_transcript".to_string(),
            checkpoint_id: None,
        });
        db::append_event(
            pool,
            session_id,
            Some(turn_id),
            "model",
            Some(final_model_event_id),
            "model.direct_final_continuation",
            Some("running"),
            json!({
                "submittedInputId": pending.id,
                "history": {"items": continuation_history.len(), "source": "completed_history_plus_current_turn_transcript"},
            }),
        )
        .await?;
        let continuation_role = db::session_role_snapshot(pool, session_id).await?;
        let continuation_session = db::session_record(pool, session_id).await?;
        let continuation_commands = command_registry::live_visible_commands(pool, &continuation_role, continuation_session.project_key.as_deref()).await?;
        let continuation_previous_context = latest_command_context_evidence(pool, session_id).await?;
        let continuation_command_context = command_registry::runtime_command_context_message(&continuation_commands, continuation_previous_context.as_ref());
        let continuation_runtime_messages = vec![RuntimeInputMessage {
            text: continuation_command_context.text,
            metadata: continuation_command_context.metadata,
        }];
        let continuation_execute_contract = command_registry::stable_execute_code_contract_with_god_mode_shell(crate::god_mode::active_grant(pool, session_id).await?.is_some());
        let continuation_request_contract = command_registry::request_tool_contract();
        match model
            .request_tool_call(
                &continuation_role,
                &continuation_history,
                &continuation_runtime_messages,
                &continuation_execute_contract,
                &continuation_request_contract,
                &pending.content,
            )
            .await?
        {
            ModelInitialTurn::FinalResponse(continued) => {
                let continued_event_id = Uuid::new_v4();
                sqlx::query(
                    r#"
                    INSERT INTO model_events (id, session_id, turn_id, event_type, payload)
                    VALUES ($1, $2, $3, 'final_response', $4)
                    "#,
                )
                .bind(continued_event_id)
                .bind(session_id)
                .bind(turn_id)
                .bind(json!({
                    "summary": continued.final_text,
                    "provider": continued.provider,
                    "model": continued.model,
                    "raw": bounded_raw_response(&continued.raw_response),
                    "sameTurnContinuation": true,
                    "submittedInputId": pending.id,
                }))
                .execute(pool)
                .await?;
                db::append_event(
                    pool,
                    session_id,
                    Some(turn_id),
                    "model",
                    Some(continued_event_id),
                    "model.final_response",
                    Some("completed"),
                    json!({
                        "finalText": continued.final_text,
                        "sameTurnContinuation": true,
                        "submittedInputId": pending.id,
                        "request": {
                            "history": {"items": continuation_history.len(), "source": "completed_history_plus_current_turn_transcript"},
                        },
                    }),
                )
                .await?;
            }
            ModelInitialTurn::ToolCall(plan) => {
                let continued_event_id = Uuid::new_v4();
                sqlx::query(
                    r#"
                    INSERT INTO model_events (id, session_id, turn_id, event_type, payload)
                    VALUES ($1, $2, $3, 'assistant_message', $4)
                    "#,
                )
                .bind(continued_event_id)
                .bind(session_id)
                .bind(turn_id)
                .bind(json!({
                    "summary": plan.assistant_summary,
                    "tool": plan.tool_call.tool_name,
                    "sameTurnContinuation": true,
                    "submittedInputId": pending.id,
                    "request": {"history": {"items": continuation_history.len(), "source": "completed_history_plus_current_turn_transcript"}},
                }))
                .execute(pool)
                .await?;
                db::append_event(
                    pool,
                    session_id,
                    Some(turn_id),
                    "model",
                    Some(continued_event_id),
                    "model.tool_call",
                    Some("planned"),
                    json!({"tool": plan.tool_call.tool_name, "sameTurnContinuation": true, "submittedInputId": pending.id}),
                )
                .await?;
            }
        }
    }

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
        let _ = crate::requirements::record_requirements_verdict_packet(pool, session_id, turn_id, final_text).await?;
    } else {
        if let Some(record) = crate::requirements::record_requirements_claim_packet(pool, session_id, turn_id, final_text).await?
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
        VALUES ($1, $2, $3, 'runtime_error', $4)
        "#,
    )
    .bind(model_event_id)
    .bind(session_id)
    .bind(turn_id)
    .bind(json!({
        "summary": runtime_failure_message(boundary),
        "provider": "runtime",
        "model": "runtime-validation",
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
        "runtime.validation_failed",
        Some("failed"),
        json!({
            "finalText": runtime_failure_message(boundary),
            "provider": "runtime",
            "model": "runtime-validation",
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

fn runtime_failure_message(boundary: &str) -> String {
    match boundary {
        "routing" => "Runtime could not route this message. Check the role recipient settings and try again.".to_string(),
        _ => "Runtime could not start the model request. Check the role settings and try again.".to_string(),
    }
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
