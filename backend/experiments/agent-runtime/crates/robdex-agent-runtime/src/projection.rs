use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use robdex_agent_runtime_projection::{
    timeline_by_sequence, timeline_item_id, AgentRuntimeChatEntry, CommandRegistryRequestSummary,
    CommandRegistrySummary, ManagedProcessSummary, PendingApprovalSummary, ProjectSummary, RequirementsPacketSummary,
    RequirementsReviewSummary, RoleSummary, RuntimeDelta, RuntimeStatistics, RuntimeDeltaKind,
    RuntimeProjection, SelectedSessionDetail, ServerStatusProjection, SessionListItem, TimelineItem,
    WorkflowMemoryEventSummary, WorkflowMemorySummary,
};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

fn optional_uuid(value: Option<Uuid>) -> Option<String> {
    value.map(|id| id.to_string())
}

fn optional_time(value: Option<chrono::DateTime<chrono::Utc>>) -> Option<String> {
    value.map(|time| time.to_rfc3339())
}

fn time(value: chrono::DateTime<chrono::Utc>) -> String {
    value.to_rfc3339()
}

pub async fn build_runtime_projection_snapshot(
    pool: &PgPool,
    selected_session_id: Option<Uuid>,
) -> Result<RuntimeProjection> {
    let watermark = current_watermark(pool).await?;
    let mut projection = RuntimeProjection {
        watermark,
        server_status: ServerStatusProjection {
            status: "ok".to_string(),
            database: "connected".to_string(),
            message: None,
        },
        projects: project_summaries(pool).await?,
        sessions: session_list_items(pool).await?,
        selected_session: selected_session_detail(pool, selected_session_id).await?,
        timeline: timeline_items(pool, selected_session_id).await?,
        selected_chat_entries: selected_chat_entries(pool, selected_session_id).await?,
        pending_approvals: pending_approval_summaries(pool).await?,
        roles: role_summaries(pool).await?,
        command_registry: command_registry_summaries(pool).await?,
        command_registry_requests: command_registry_request_summaries(pool).await?,
        workflow_memories: workflow_memory_summaries(pool, selected_session_id).await?,
        statistics: runtime_statistics(pool, selected_session_id).await?,
        resync_required: None,
    };
    projection.timeline = timeline_by_sequence(projection.timeline);
    Ok(projection)
}

async fn project_summaries(pool: &PgPool) -> Result<Vec<ProjectSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT project_key, display_name, default_workdir, default_worktree_root,
               default_role_id, default_model, archived, created_at, updated_at
        FROM projects
        ORDER BY lower(display_name), project_key
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ProjectSummary {
            project_key: row.get("project_key"),
            display_name: row.get("display_name"),
            default_workdir: row.get("default_workdir"),
            default_worktree_root: row.get("default_worktree_root"),
            default_role_id: row.get("default_role_id"),
            default_model: row.get("default_model"),
            archived: row.get("archived"),
            created_at: optional_time(row.get("created_at")),
            updated_at: optional_time(row.get("updated_at")),
        })
        .collect())
}

async fn runtime_statistics(pool: &PgPool, selected_session_id: Option<Uuid>) -> Result<RuntimeStatistics> {
    async fn count(pool: &PgPool, sql: &str) -> Result<u64> {
        let value: i64 = sqlx::query_scalar(sql).fetch_one(pool).await?;
        Ok(value.max(0) as u64)
    }
    async fn scoped_count(pool: &PgPool, global_sql: &str, scoped_sql: &str, selected_session_id: Option<Uuid>) -> Result<u64> {
        if let Some(session_id) = selected_session_id {
            let value: i64 = sqlx::query_scalar(scoped_sql).bind(session_id).fetch_one(pool).await?;
            Ok(value.max(0) as u64)
        } else {
            count(pool, global_sql).await
        }
    }
    Ok(RuntimeStatistics {
        sessions: scoped_count(pool, "SELECT COUNT(*) FROM sessions WHERE hidden = false", "SELECT COUNT(*) FROM sessions WHERE id=$1 AND hidden = false", selected_session_id).await?,
        open_sessions: scoped_count(pool, "SELECT COUNT(*) FROM sessions WHERE hidden = false AND closed_at IS NULL AND archived_at IS NULL", "SELECT COUNT(*) FROM sessions WHERE id=$1 AND hidden = false AND closed_at IS NULL AND archived_at IS NULL", selected_session_id).await?,
        closed_sessions: scoped_count(pool, "SELECT COUNT(*) FROM sessions WHERE hidden = false AND closed_at IS NOT NULL", "SELECT COUNT(*) FROM sessions WHERE id=$1 AND hidden = false AND closed_at IS NOT NULL", selected_session_id).await?,
        archived_sessions: scoped_count(pool, "SELECT COUNT(*) FROM sessions WHERE hidden = false AND archived_at IS NOT NULL", "SELECT COUNT(*) FROM sessions WHERE id=$1 AND hidden = false AND archived_at IS NOT NULL", selected_session_id).await?,
        turns: scoped_count(pool, "SELECT COUNT(*) FROM turns", "SELECT COUNT(*) FROM turns WHERE session_id=$1", selected_session_id).await?,
        running_turns: scoped_count(pool, "SELECT COUNT(*) FROM turns WHERE status = 'running'", "SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status = 'running'", selected_session_id).await?,
        failed_turns: scoped_count(pool, "SELECT COUNT(*) FROM turns WHERE status = 'failed'", "SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status = 'failed'", selected_session_id).await?,
        model_events: scoped_count(pool, "SELECT COUNT(*) FROM model_events", "SELECT COUNT(*) FROM model_events WHERE session_id=$1", selected_session_id).await?,
        tool_calls: scoped_count(pool, "SELECT COUNT(*) FROM tool_calls", "SELECT COUNT(*) FROM tool_calls WHERE session_id=$1", selected_session_id).await?,
        script_runs: scoped_count(pool, "SELECT COUNT(*) FROM script_runs", "SELECT COUNT(*) FROM script_runs sr JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1", selected_session_id).await?,
        host_api_calls: scoped_count(pool, "SELECT COUNT(*) FROM host_api_calls", "SELECT COUNT(*) FROM host_api_calls h JOIN script_runs sr ON sr.id=h.script_run_id JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1", selected_session_id).await?,
        command_runs: scoped_count(pool, "SELECT COUNT(*) FROM command_runs", "SELECT COUNT(*) FROM command_runs cr JOIN host_api_calls h ON h.id=cr.host_api_call_id JOIN script_runs sr ON sr.id=h.script_run_id JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1", selected_session_id).await?,
        managed_processes: scoped_count(pool, "SELECT COUNT(*) FROM managed_processes", "SELECT COUNT(*) FROM managed_processes WHERE session_id=$1", selected_session_id).await?,
        output_artifacts: scoped_count(pool, "SELECT COUNT(*) FROM execution_output_artifacts", "SELECT COUNT(*) FROM execution_output_artifacts WHERE session_id=$1", selected_session_id).await?,
        compaction_checkpoints: scoped_count(pool, "SELECT COUNT(*) FROM compaction_checkpoints", "SELECT COUNT(*) FROM compaction_checkpoints WHERE session_id=$1", selected_session_id).await?,
        approval_requests: scoped_count(pool, "SELECT COUNT(*) FROM approval_requests", "SELECT COUNT(*) FROM approval_requests WHERE session_id=$1", selected_session_id).await?,
        command_registry_requests: scoped_count(pool, "SELECT COUNT(*) FROM command_registry_requests", "SELECT COUNT(*) FROM command_registry_requests WHERE session_id=$1", selected_session_id).await?,
        workflow_memories: scoped_count(pool, "SELECT COUNT(*) FROM workflow_memories", "SELECT COUNT(*) FROM workflow_memories WHERE session_id=$1", selected_session_id).await?,
        failed_rows: scoped_count(pool, "SELECT COUNT(*) FROM turns WHERE status = 'failed'", "SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status = 'failed'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM tool_calls WHERE status = 'failed'", "SELECT COUNT(*) FROM tool_calls WHERE session_id=$1 AND status = 'failed'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM script_runs WHERE status = 'failed'", "SELECT COUNT(*) FROM script_runs sr JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1 AND sr.status = 'failed'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM host_api_calls WHERE status = 'failed'", "SELECT COUNT(*) FROM host_api_calls h JOIN script_runs sr ON sr.id=h.script_run_id JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1 AND h.status = 'failed'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM command_runs WHERE status = 'failed'", "SELECT COUNT(*) FROM command_runs cr JOIN host_api_calls h ON h.id=cr.host_api_call_id JOIN script_runs sr ON sr.id=h.script_run_id JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1 AND cr.status = 'failed'", selected_session_id).await?,
        running_rows: scoped_count(pool, "SELECT COUNT(*) FROM turns WHERE status = 'running'", "SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status = 'running'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM tool_calls WHERE status = 'running'", "SELECT COUNT(*) FROM tool_calls WHERE session_id=$1 AND status = 'running'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM script_runs WHERE status = 'running'", "SELECT COUNT(*) FROM script_runs sr JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1 AND sr.status = 'running'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM host_api_calls WHERE status = 'running'", "SELECT COUNT(*) FROM host_api_calls h JOIN script_runs sr ON sr.id=h.script_run_id JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1 AND h.status = 'running'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM command_runs WHERE status = 'running'", "SELECT COUNT(*) FROM command_runs cr JOIN host_api_calls h ON h.id=cr.host_api_call_id JOIN script_runs sr ON sr.id=h.script_run_id JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1 AND cr.status = 'running'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM managed_processes WHERE status = 'running'", "SELECT COUNT(*) FROM managed_processes WHERE session_id=$1 AND status = 'running'", selected_session_id).await?,
        lost_rows: scoped_count(pool, "SELECT COUNT(*) FROM turns WHERE status = 'lost'", "SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status = 'lost'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM tool_calls WHERE status = 'lost'", "SELECT COUNT(*) FROM tool_calls WHERE session_id=$1 AND status = 'lost'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM script_runs WHERE status = 'lost'", "SELECT COUNT(*) FROM script_runs sr JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1 AND sr.status = 'lost'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM host_api_calls WHERE status = 'lost'", "SELECT COUNT(*) FROM host_api_calls h JOIN script_runs sr ON sr.id=h.script_run_id JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1 AND h.status = 'lost'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM command_runs WHERE status = 'lost'", "SELECT COUNT(*) FROM command_runs cr JOIN host_api_calls h ON h.id=cr.host_api_call_id JOIN script_runs sr ON sr.id=h.script_run_id JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1 AND cr.status = 'lost'", selected_session_id).await?
            + scoped_count(pool, "SELECT COUNT(*) FROM managed_processes WHERE status = 'lost'", "SELECT COUNT(*) FROM managed_processes WHERE session_id=$1 AND status = 'lost'", selected_session_id).await?,
    })
}

pub async fn build_projection_deltas_after(
    pool: &PgPool,
    after: i64,
    selected_session_id: Option<Uuid>,
    limit: i64,
) -> Result<Vec<RuntimeDelta>> {
    let rows = sqlx::query(
        r#"
        SELECT sequence, session_id, turn_id, entity_type, entity_id, event_type, status, payload, created_at
        FROM event_stream
        WHERE sequence > $1 AND ($2::uuid IS NULL OR session_id = $2)
        ORDER BY sequence ASC
        LIMIT $3
        "#,
    )
    .bind(after)
    .bind(selected_session_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let mut deltas = Vec::new();
    let mut previous = after;
    for row in rows {
        let mut row_deltas = projection_deltas_from_event_row(pool, &row, previous).await?;
        if let Some(last) = row_deltas.last() {
            previous = last.watermark;
        }
        deltas.append(&mut row_deltas);
    }
    Ok(deltas)
}

async fn projection_deltas_from_event_row(
    pool: &PgPool,
    row: &sqlx::postgres::PgRow,
    previous: i64,
) -> Result<Vec<RuntimeDelta>> {
    let item = timeline_item_from_row(row);
    let watermark = item.sequence;
    let entity_id = item.entity_id.clone();
    let event_type = item.event_type.clone();
    let status = item.status.clone();
    let payload = item.payload.clone();
    let mut deltas = vec![RuntimeDelta {
        watermark,
        previous_watermark: Some(previous),
        kind: RuntimeDeltaKind::TimelineAppend { item: item.clone() },
    }];

    match event_type.as_str() {
        "session.created" => {
            if let Some(id) = uuid_from_option(entity_id.as_deref())
                && let Some(session) = session_list_item(pool, id).await?
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::SessionUpsert { session }));
            }
        }
        "session.archived" => {
            if let Some(id) = uuid_from_option(entity_id.as_deref()) {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::SessionArchive {
                    session_id: id.to_string(),
                    archived_at: None,
                }));
            }
        }
        "session.closed" => {
            if let Some(id) = uuid_from_option(entity_id.as_deref()) {
                if let Some(session) = session_list_item(pool, id).await? {
                    deltas.push(same_row_delta(watermark, RuntimeDeltaKind::SessionUpsert { session }));
                }
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::SessionClose {
                    session_id: id.to_string(),
                    closed_at: None,
                }));
            }
        }
        "turn.started" | "turn.completed" => {
            if let Some(id) = uuid_from_option(entity_id.as_deref())
                && let Some(status) = status
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::TurnStatusChanged {
                    turn_id: id.to_string(),
                    status,
                }));
                if let Some(entry) = turn_chat_entry(pool, id).await? {
                    deltas.push(same_row_delta(watermark, RuntimeDeltaKind::SelectedChatUpdate { entry }));
                }
            }
        }
        "tool.started" | "tool.completed" => {
            if let Some(id) = uuid_from_option(entity_id.as_deref())
                && let Some(status) = status
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::ToolStatusChanged {
                    tool_call_id: id.to_string(),
                    status,
                }));
                if let Some(entry) = tool_chat_entry_for_tool(pool, id).await? {
                    deltas.push(same_row_delta(watermark, RuntimeDeltaKind::SelectedChatUpdate { entry }));
                }
            }
        }
        "script.started" | "script.completed" => {
            if let Some(id) = uuid_from_option(entity_id.as_deref())
                && let Some(status) = status
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::ScriptStatusChanged {
                    script_run_id: id.to_string(),
                    status,
                }));
            }
        }
        event if event.starts_with("process.") => {
            if let Some(id) = uuid_from_option(entity_id.as_deref())
                && let Some(status) = status
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::ProcessStatusChanged {
                    process_id: id.to_string(),
                    status,
                }));
                for entry in tool_chat_entries_for_process(pool, id).await? {
                    deltas.push(same_row_delta(watermark, RuntimeDeltaKind::SelectedChatUpdate { entry }));
                }
            }
        }
        "model.final_response" => {
            if let Some(turn_id) = uuid_from_option(item.turn_id.as_deref())
                && let Some(session_id) = uuid_from_option(item.session_id.as_deref())
                && let Some(entry) = assistant_chat_entry(pool, session_id, turn_id).await?
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::SelectedChatUpdate { entry }));
            }
        }
        "runtime.validation_failed" => {
            if let Some(turn_id) = uuid_from_option(item.turn_id.as_deref())
                && let Some(session_id) = uuid_from_option(item.session_id.as_deref())
                && let Some(entry) = runtime_error_chat_entry(pool, session_id, turn_id).await?
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::SelectedChatUpdate { entry }));
            }
        }
        "approval.requested" => {
            if let Some(id) = uuid_from_option(entity_id.as_deref())
                && let Some(approval) = pending_approval_summary(pool, id).await?
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::ApprovalUpsert { approval }));
            }
        }
        "approval.decided" | "approval.resume.started" | "approval.resume.completed" | "approval.resume.failed" => {
            if let Some(id) = uuid_from_option(entity_id.as_deref()) {
                match pending_approval_summary(pool, id).await? {
                    Some(approval) => deltas.push(same_row_delta(watermark, RuntimeDeltaKind::ApprovalUpsert { approval })),
                    None => deltas.push(same_row_delta(watermark, RuntimeDeltaKind::ApprovalRemove { approval_id: id.to_string() })),
                }
            }
        }
        "role.created" | "role.updated" | "role.imported" | "role.activated" | "role.unarchived" => {
            if let Some(role_id) = payload.get("roleId").and_then(Value::as_str)
                && let Some(role) = role_summary(pool, role_id).await?
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::RoleUpsert { role }));
            }
        }
        "role.archived" => {
            if let Some(role_id) = payload.get("roleId").and_then(Value::as_str) {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::RoleArchive {
                    role_id: role_id.to_string(),
                    archived_at: None,
                }));
            }
        }
        "command_registry.applied" | "command_registry.decided" | "command_registry.requested" => {
            if let Some(request_id) = command_registry_request_id_for_event(&item, &payload)
                && let Some(request) = command_registry_request_summary(pool, request_id).await?
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::CommandRegistryRequestUpsert { request }));
            } else if let Some(request_id) = command_registry_request_id_for_event(&item, &payload) {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::CommandRegistryRequestRemove { request_id: request_id.to_string() }));
            }
            if let Some(command) = command_summary_for_event(pool, &payload).await? {
                if command.enabled {
                    deltas.push(same_row_delta(watermark, RuntimeDeltaKind::CommandRegistryUpsert { command }));
                } else {
                    deltas.push(same_row_delta(watermark, RuntimeDeltaKind::CommandRegistryDisable { command_id: command.id }));
                }
            }
        }
        event if event.starts_with("workflow_memory.") => {
            if let Some(id) = uuid_from_option(entity_id.as_deref())
                && let Some(memory) = workflow_memory_summary(pool, id).await?
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::WorkflowMemoryUpsert { memory }));
            }
            deltas.push(same_row_delta(watermark, RuntimeDeltaKind::WorkflowMemoryEvent { item }));
        }
        event if event.starts_with("requirements.") => {
            if let Some(session_id) = uuid_from_option(item.session_id.as_deref())
                && let Some(summary) = requirements_review_summary(pool, session_id).await?
            {
                deltas.push(same_row_delta(watermark, RuntimeDeltaKind::RequirementsReviewUpdate {
                    session_id: session_id.to_string(),
                    summary,
                }));
            }
        }
        _ => {}
    }
    Ok(deltas)
}

fn same_row_delta(watermark: i64, kind: RuntimeDeltaKind) -> RuntimeDelta {
    RuntimeDelta {
        watermark,
        previous_watermark: None,
        kind,
    }
}

fn uuid_from_option(value: Option<&str>) -> Option<Uuid> {
    value.and_then(|value| Uuid::parse_str(value).ok())
}

pub async fn event_stream_can_continue_after(
    pool: &PgPool,
    after: i64,
    selected_session_id: Option<Uuid>,
) -> Result<bool> {
    if after <= 0 {
        return Ok(false);
    }
    let current = current_watermark(pool).await?;
    if after > current {
        return Ok(false);
    }
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM event_stream
            WHERE sequence = $1
        )
        "#,
    )
    .bind(after)
    .fetch_one(pool)
    .await?;
    let _ = selected_session_id;
    Ok(exists)
}

async fn current_watermark(pool: &PgPool) -> Result<i64> {
    let row = sqlx::query("SELECT COALESCE(MAX(sequence), 0)::bigint AS watermark FROM event_stream")
        .fetch_one(pool)
        .await?;
    Ok(row.get("watermark"))
}

async fn session_list_items(pool: &PgPool) -> Result<Vec<SessionListItem>> {
    let rows = sqlx::query(
        r#"
        SELECT id, status, role_id, role_version, project_key, workdir, title, name, tracked,
               archived_at, closed_at, updated_at
        FROM sessions
        WHERE archived_at IS NULL AND hidden = false AND (tracked = true OR status <> 'open')
        ORDER BY updated_at DESC, created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| SessionListItem {
            id: row.get::<Uuid, _>("id").to_string(),
            status: row.get("status"),
            role_id: row.get("role_id"),
            role_version: row.get("role_version"),
            project_key: row.get("project_key"),
            title: row.get("title"),
            name: row.get("name"),
            workdir: row.get("workdir"),
            tracked: row.get("tracked"),
            archived_at: optional_time(row.get("archived_at")),
            closed_at: optional_time(row.get("closed_at")),
            updated_at: optional_time(Some(row.get("updated_at"))),
        })
        .collect())
}

async fn session_list_item(pool: &PgPool, session_id: Uuid) -> Result<Option<SessionListItem>> {
    let row = sqlx::query(
        r#"
        SELECT id, status, role_id, role_version, project_key, workdir, title, name, tracked,
               archived_at, closed_at, updated_at
        FROM sessions
        WHERE id = $1 AND archived_at IS NULL AND hidden = false AND (tracked = true OR status <> 'open')
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| SessionListItem {
        id: row.get::<Uuid, _>("id").to_string(),
        status: row.get("status"),
        role_id: row.get("role_id"),
        role_version: row.get("role_version"),
        project_key: row.get("project_key"),
        title: row.get("title"),
        name: row.get("name"),
        workdir: row.get("workdir"),
        tracked: row.get("tracked"),
        archived_at: optional_time(row.get("archived_at")),
        closed_at: optional_time(row.get("closed_at")),
        updated_at: optional_time(Some(row.get("updated_at"))),
    }))
}

async fn selected_session_detail(
    pool: &PgPool,
    selected_session_id: Option<Uuid>,
) -> Result<Option<SelectedSessionDetail>> {
    let Some(session_id) = selected_session_id else {
        return Ok(None);
    };
    let row = sqlx::query(
        r#"
        SELECT id, status, role_id, role_version, project_key, workdir, worktree_root, title, name, metadata, role_snapshot,
               active_project_runtime_version_id, active_hook_bindings, active_tool_bundle_version_ids
        FROM sessions
        WHERE id = $1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let pending_approval_count: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM approval_requests WHERE session_id = $1 AND status = 'pending'",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?
    .get("count");
    let managed_process_count: i64 = sqlx::query(
        "SELECT COUNT(*) AS count FROM managed_processes WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?
    .get("count");
    let god_mode = crate::god_mode::active_grant(pool, session_id).await?;
    let mut metadata: serde_json::Value = row.get("metadata");
    if let Some(model) = row
        .get::<serde_json::Value, _>("role_snapshot")
        .get("modelDefaults")
        .and_then(|value| value.get("model"))
        .and_then(serde_json::Value::as_str)
        && let Some(map) = metadata.as_object_mut()
    {
        map.insert("model".to_string(), serde_json::Value::String(model.to_string()));
    }
    if let Some(map) = metadata.as_object_mut() {
        if let Some(grant) = god_mode {
            map.insert("godMode".to_string(), serde_json::json!({
                "active": true,
                "grantId": grant.id,
                "reason": grant.reason,
                "grantedBy": grant.granted_by,
                "grantedAt": grant.granted_at,
                "expiresAt": grant.expires_at,
            }));
        } else {
            map.insert("godMode".to_string(), serde_json::json!({"active": false}));
        }
        let requirements = crate::requirements::status(pool, session_id).await?;
        map.insert("requirementsReview".to_string(), serde_json::to_value(&requirements).unwrap_or(serde_json::Value::Null));
    }
    let active_model = metadata.get("model").and_then(Value::as_str).map(str::to_string);
    let requirements_review = requirements_review_summary(pool, session_id).await?;
    let managed_processes = selected_managed_processes(pool, session_id).await?;
    let active_turn_id = crate::db::active_turn_id(pool, session_id).await?.map(|id| id.to_string());
    let (queued_submitted_input_count, applied_steering_count, submit_disposition, submit_status) =
        crate::db::submitted_input_counts(pool, session_id).await?;
    let terminal_submission_rejection = crate::db::latest_rejected_submitted_input(pool, session_id).await?;
    let active_project_runtime_version_id: Option<Uuid> = row.get("active_project_runtime_version_id");
    let active_hook_bindings: Value = row.get("active_hook_bindings");
    let active_tool_bundle_version_ids: Value = row.get("active_tool_bundle_version_ids");
    let project_runtime = json!({
        "activeVersionId": active_project_runtime_version_id,
        "hookBindingCount": active_hook_bindings.as_object().map(|value| value.len()).unwrap_or_default(),
        "activeToolBundleVersionIds": active_tool_bundle_version_ids,
    });
    let hook_overrides = active_hook_bindings.clone();
    let subagents = crate::lifecycle_hooks::parent_subagent_projection(pool, session_id).await?;
    let contract_rows = sqlx::query("SELECT id, contract_type, status, active_version FROM generic_contracts WHERE session_id=$1 ORDER BY created_at DESC LIMIT 20")
        .bind(session_id)
        .fetch_all(pool)
        .await?;
    let contracts = contract_rows
        .into_iter()
        .map(|row| json!({
            "contractId": row.get::<Uuid, _>("id"),
            "contractType": row.get::<String, _>("contract_type"),
            "status": row.get::<String, _>("status"),
            "activeVersion": row.get::<String, _>("active_version"),
        }))
        .collect::<Vec<_>>();
    let lease_rows = sqlx::query("SELECT id, resource_type, resource_id, handle, status FROM resource_leases WHERE owning_session_id=$1 ORDER BY created_at DESC LIMIT 20")
        .bind(session_id)
        .fetch_all(pool)
        .await?;
    let resource_leases = lease_rows
        .into_iter()
        .map(|row| json!({
            "leaseId": row.get::<Uuid, _>("id"),
            "resourceType": row.get::<String, _>("resource_type"),
            "resourceId": row.get::<Option<String>, _>("resource_id"),
            "handle": row.get::<Option<String>, _>("handle"),
            "status": row.get::<String, _>("status"),
        }))
        .collect::<Vec<_>>();
    let failure_rows = sqlx::query("SELECT id, lifecycle_event_id, errors, created_at FROM hook_evaluations WHERE session_id=$1 AND validation_status='invalid' ORDER BY created_at DESC LIMIT 10")
        .bind(session_id)
        .fetch_all(pool)
        .await?;
    let recent_hook_failures = failure_rows
        .into_iter()
        .map(|row| json!({
            "evaluationId": row.get::<Uuid, _>("id"),
            "lifecycleEventId": row.get::<Uuid, _>("lifecycle_event_id"),
            "errors": row.get::<Value, _>("errors"),
            "createdAt": optional_time(Some(row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"))),
        }))
        .collect::<Vec<_>>();
    let server_rows = sqlx::query("SELECT handle, status, url, port, readiness_config FROM starter_managed_servers WHERE session_id=$1 ORDER BY created_at DESC LIMIT 20")
        .bind(session_id)
        .fetch_all(pool)
        .await?;
    let running_servers = server_rows
        .into_iter()
        .map(|row| json!({
            "handle": row.get::<String, _>("handle"),
            "status": row.get::<String, _>("status"),
            "url": row.get::<String, _>("url"),
            "port": row.get::<i32, _>("port"),
            "readiness": row.get::<Value, _>("readiness_config"),
            "actions": ["status", "logs", "stop"],
        }))
        .collect::<Vec<_>>();
    let tooling_rows = sqlx::query("SELECT id, title, urgency, status, route FROM starter_tooling_requests WHERE session_id=$1 ORDER BY created_at DESC LIMIT 20")
        .bind(session_id)
        .fetch_all(pool)
        .await?;
    let tooling_requests = tooling_rows
        .into_iter()
        .map(|row| json!({
            "packetId": row.get::<Uuid, _>("id"),
            "title": row.get::<String, _>("title"),
            "urgency": row.get::<String, _>("urgency"),
            "status": row.get::<String, _>("status"),
            "route": row.get::<Value, _>("route"),
        }))
        .collect::<Vec<_>>();
    Ok(Some(SelectedSessionDetail {
        id: row.get::<Uuid, _>("id").to_string(),
        role_id: row.get("role_id"),
        role_version: row.get("role_version"),
        project_key: row.get("project_key"),
        active_model,
        workdir: row.get("workdir"),
        worktree_root: row.get("worktree_root"),
        title: row.get("title"),
        name: row.get("name"),
        status: row.get("status"),
        pending_approval_count: pending_approval_count.max(0) as u64,
        managed_process_count: managed_process_count.max(0) as u64,
        active_turn_id,
        queued_submitted_input_count: queued_submitted_input_count.max(0) as u64,
        applied_steering_count: applied_steering_count.max(0) as u64,
        submit_disposition,
        submit_status,
        terminal_submission_rejection,
        metadata,
        project_runtime,
        hook_overrides,
        subagents,
        contracts,
        resource_leases,
        recent_hook_failures,
        running_servers,
        tooling_requests,
        requirements_review,
        managed_processes,
    }))
}

async fn selected_managed_processes(pool: &PgPool, session_id: Uuid) -> Result<Vec<ManagedProcessSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT id, handle, starting_turn_id, binary_name, argv, cwd, os_pid, status, start_time, end_time,
               end_of_turn_behavior, end_of_session_behavior, metadata
        FROM managed_processes
        WHERE session_id = $1
        ORDER BY
            CASE status WHEN 'running' THEN 0 WHEN 'starting' THEN 1 WHEN 'lost' THEN 2 ELSE 3 END,
            start_time DESC,
            id ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    let mut processes = Vec::new();
    for row in rows {
        let id: Uuid = row.get("id");
        let argv_value: Value = row.get("argv");
        let argv = argv_value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let binary_name: String = row.get("binary_name");
        let command_label = std::iter::once(binary_name.clone())
            .chain(argv.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        let artifact_rows = sqlx::query(
            "SELECT id, stream, byte_count, line_count, created_at FROM execution_output_artifacts WHERE process_id=$1 ORDER BY created_at DESC LIMIT 4",
        )
        .bind(id)
        .fetch_all(pool)
        .await?;
        let mut latest_output_summary = None;
        let output_artifacts = artifact_rows
            .into_iter()
            .map(|artifact| {
                let stream: String = artifact.get("stream");
                let byte_count: i64 = artifact.get("byte_count");
                let line_count: i64 = artifact.get("line_count");
                if latest_output_summary.is_none() {
                    latest_output_summary = Some(format!("{stream}: {line_count} lines, {byte_count} bytes"));
                }
                json!({
                    "artifactId": artifact.get::<Uuid, _>("id"),
                    "stream": stream,
                    "byteCount": byte_count,
                    "lineCount": line_count,
                    "createdAt": optional_time(Some(artifact.get::<chrono::DateTime<chrono::Utc>, _>("created_at"))),
                })
            })
            .collect::<Vec<_>>();
        let status: String = row.get("status");
        let metadata: Value = row.get("metadata");
        let stdin_policy = metadata
            .get("stdinPolicy")
            .and_then(Value::as_str)
            .unwrap_or_else(|| metadata.get("stdin_policy").and_then(Value::as_str).unwrap_or("none"))
            .to_string();
        let running = matches!(status.as_str(), "running" | "starting");
        let can_input = running && stdin_policy != "none";
        processes.push(ManagedProcessSummary {
            id: id.to_string(),
            handle: row.get("handle"),
            turn_id: row.get::<Option<Uuid>, _>("starting_turn_id").map(|id| id.to_string()),
            binary_name,
            argv,
            command_label,
            cwd: row.get("cwd"),
            status,
            started_at: optional_time(Some(row.get::<chrono::DateTime<chrono::Utc>, _>("start_time"))),
            ended_at: optional_time(row.get("end_time")),
            os_pid: row.get("os_pid"),
            stdin_policy,
            end_of_turn_behavior: row.get("end_of_turn_behavior"),
            end_of_session_behavior: row.get("end_of_session_behavior"),
            output_artifacts,
            latest_output_summary,
            can_terminate: running,
            can_flush: true,
            can_input,
            metadata,
        });
    }
    Ok(processes)
}

pub async fn requirements_review_summary(pool: &PgPool, session_id: Uuid) -> Result<Option<RequirementsReviewSummary>> {
    let status = crate::requirements::status(pool, session_id).await?;
    let packets = crate::requirements::packet_history(pool, session_id)
        .await?
        .into_iter()
        .map(|packet| RequirementsPacketSummary {
            id: packet["id"].as_str().unwrap_or_default().to_string(),
            requirement_set_id: packet["requirementSetId"].as_str().unwrap_or_default().to_string(),
            packet_kind: packet["packetKind"].as_str().unwrap_or_default().to_string(),
            status: packet["status"].as_str().unwrap_or_default().to_string(),
            reviewer_session_id: packet["reviewerSessionId"].as_str().map(str::to_string),
            turn_id: packet["turnId"].as_str().map(str::to_string),
        })
        .collect();
    Ok(Some(RequirementsReviewSummary {
        active: status.active,
        active_set_id: status.active_set_id.map(|id| id.to_string()),
        total: status.total,
        unresolved: status.unresolved,
        passed: status.passed,
        blocked: status.blocked,
        waived: status.waived,
        reviewer_session_id: status.reviewer_session_id.map(|id| id.to_string()),
        review_status: status.review_status,
        latest_claim_packet_id: status.latest_claim_packet_id.map(|id| id.to_string()),
        latest_verdict_packet_id: status.latest_verdict_packet_id.map(|id| id.to_string()),
        packets,
        progress: status.progress,
        owner_action: status.owner_action,
    }))
}

async fn timeline_items(
    pool: &PgPool,
    selected_session_id: Option<Uuid>,
) -> Result<Vec<TimelineItem>> {
    let rows = sqlx::query(
        r#"
        SELECT sequence, session_id, turn_id, entity_type, entity_id, event_type, status, payload, created_at
        FROM event_stream
        WHERE $1::uuid IS NULL OR session_id = $1
        ORDER BY sequence ASC
        LIMIT 500
        "#,
    )
    .bind(selected_session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            timeline_item_from_row(&row)
        })
        .collect())
}

fn timeline_item_from_row(row: &sqlx::postgres::PgRow) -> TimelineItem {
    let sequence: i64 = row.get("sequence");
    let payload: Value = row.get("payload");
    TimelineItem {
        id: timeline_item_id(sequence),
        sequence,
        session_id: optional_uuid(row.get("session_id")),
        turn_id: optional_uuid(row.get("turn_id")),
        entity_type: row.get("entity_type"),
        entity_id: optional_uuid(row.get("entity_id")),
        event_type: row.get("event_type"),
        status: row.get("status"),
        summary: event_summary(&payload),
        payload,
        created_at: Some(time(row.get("created_at"))),
    }
}

async fn selected_chat_entries(
    pool: &PgPool,
    selected_session_id: Option<Uuid>,
) -> Result<Vec<AgentRuntimeChatEntry>> {
    let Some(session_id) = selected_session_id else {
        return Ok(Vec::new());
    };

    let mut entries = Vec::new();
    let turn_rows = sqlx::query(
        r#"
        SELECT id, role, input_text, status, started_at, completed_at
        FROM turns
        WHERE session_id = $1
        ORDER BY started_at ASC, id ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    for row in turn_rows {
        let turn_id = row.get::<Uuid, _>("id");
        let role: String = row.get("role");
        let author = match role.as_str() {
            "operator" => "Operator",
            _ => "User",
        }
        .to_string();
        let started_at = time(row.get("started_at"));
        let status: String = row.get("status");
        entries.push(AgentRuntimeChatEntry {
            id: format!("turn:{turn_id}:user"),
            author: author.clone(),
            display_label: author,
            timestamp: Some(started_at),
            body: row.get("input_text"),
            subtitle: status.clone(),
            kind: "message".to_string(),
            status: status.clone(),
            process_id: None,
                tool_call_id: None,
                script_run_id: None,
                stdout_artifact_id: None,
                stderr_artifact_id: None,
            command: String::new(),
            output: String::new(),
            image_preview_base64: None,
            image_preview_content_type: None,
            image_preview_error: None,
            delivery_state: if status == "completed" { "delivered" } else { "sending" }.to_string(),
            is_streaming: status == "running",
            is_tool: false,
        });

        for submitted in submitted_input_chat_entries_for_turn(pool, turn_id).await? {
            entries.push(submitted);
        }

        for tool in tool_chat_entries(pool, session_id, turn_id).await? {
            entries.push(tool);
        }

        if let Some(runtime_error) = runtime_error_chat_entry(pool, session_id, turn_id).await? {
            entries.push(runtime_error);
        }

        if let Some(assistant) = assistant_chat_entry(pool, session_id, turn_id).await? {
            entries.push(assistant);
        }
    }
    entries.extend(image_artifact_chat_entries(pool, session_id).await?);
    entries.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then(left.id.cmp(&right.id))
    });
    Ok(entries)
}

async fn image_artifact_chat_entries(pool: &PgPool, session_id: Uuid) -> Result<Vec<AgentRuntimeChatEntry>> {
    let rows = sqlx::query(
        r#"
        SELECT id, source_type, source_path, mime_type, byte_count, width, height,
               retrieval_metadata, binary_content, created_at
        FROM starter_image_artifacts
        WHERE session_id = $1
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let id = row.get::<Uuid, _>("id");
            let mime_type: String = row.get("mime_type");
            let byte_count: i64 = row.get("byte_count");
            let width: Option<i32> = row.get("width");
            let height: Option<i32> = row.get("height");
            let source_type: String = row.get("source_type");
            let source_path: Option<String> = row.get("source_path");
            let retrieval_metadata: Value = row.get("retrieval_metadata");
            let bytes: Vec<u8> = row.get("binary_content");
            let dimensions = match (width, height) {
                (Some(width), Some(height)) => format!("{width} × {height}"),
                _ => "dimensions unavailable".to_string(),
            };
            let source = source_path
                .filter(|path| !path.trim().is_empty())
                .unwrap_or(source_type);
            let caption = retrieval_metadata
                .get("description")
                .and_then(Value::as_str)
                .or_else(|| retrieval_metadata.get("captureDescription").and_then(Value::as_str))
                .or_else(|| retrieval_metadata.get("sourceDescription").and_then(Value::as_str))
                .unwrap_or("Image artifact");
            AgentRuntimeChatEntry {
                id: format!("imageArtifact:{id}"),
                author: "Runtime".to_string(),
                display_label: "Image artifact".to_string(),
                timestamp: Some(time(row.get("created_at"))),
                body: caption.to_string(),
                subtitle: format!("{mime_type} · {dimensions} · {byte_count} bytes"),
                kind: "imageView".to_string(),
                status: "stored".to_string(),
                process_id: None,
                tool_call_id: None,
                script_run_id: None,
                stdout_artifact_id: None,
                stderr_artifact_id: None,
                command: source,
                output: format!("agent-runtime-image://{session_id}/{id}"),
                image_preview_base64: Some(BASE64_STANDARD.encode(bytes)),
                image_preview_content_type: Some(mime_type),
                image_preview_error: None,
                delivery_state: "delivered".to_string(),
                is_streaming: false,
                is_tool: false,
            }
        })
        .collect())
}

async fn submitted_input_chat_entries_for_turn(pool: &PgPool, turn_id: Uuid) -> Result<Vec<AgentRuntimeChatEntry>> {
    let rows = sqlx::query(
        r#"
        SELECT id, actor, role, content, disposition, status, ordering_key, accepted_at, applied_at
        FROM submitted_inputs
        WHERE target_turn_id = $1 AND status IN ('accepted','applied')
        ORDER BY ordering_key ASC
        "#,
    )
    .bind(turn_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let id = row.get::<Uuid, _>("id");
            let role: String = row.get("role");
            let actor: String = row.get("actor");
            let status: String = row.get("status");
            let disposition: String = row.get("disposition");
            let ordering_key: i64 = row.get("ordering_key");
            let accepted_at: Option<chrono::DateTime<chrono::Utc>> = row.get("accepted_at");
            let applied_at: Option<chrono::DateTime<chrono::Utc>> = row.get("applied_at");
            let author = if role == "operator" || actor == "operator" { "Operator" } else { "User" }.to_string();
            let subtitle = match (status.as_str(), disposition.as_str()) {
                ("applied", "active_turn_steering") => "added to live turn",
                ("accepted", "queued_continuation_after_compaction") => "waiting for compaction",
                ("accepted", "queued_next_turn_after_final_output") => "queued for next turn",
                ("accepted", _) => "queued",
                ("applied", _) => "delivered",
                _ => status.as_str(),
            }
            .to_string();
            AgentRuntimeChatEntry {
                id: format!("submitted:{id}:{}", ordering_key),
                author: author.clone(),
                display_label: author,
                timestamp: accepted_at.or(applied_at).map(time),
                body: row.get("content"),
                subtitle: subtitle.clone(),
                kind: "message".to_string(),
                status: subtitle,
                process_id: None,
                tool_call_id: None,
                script_run_id: None,
                stdout_artifact_id: None,
                stderr_artifact_id: None,
                command: String::new(),
                output: String::new(),
                image_preview_base64: None,
                image_preview_content_type: None,
                image_preview_error: None,
                delivery_state: if status == "applied" { "delivered" } else { "queued" }.to_string(),
                is_streaming: false,
                is_tool: false,
            }
        })
        .collect())
}

async fn turn_chat_entry(pool: &PgPool, turn_id: Uuid) -> Result<Option<AgentRuntimeChatEntry>> {
    let row = sqlx::query(
        r#"
        SELECT id, role, input_text, status, started_at
        FROM turns
        WHERE id = $1
        "#,
    )
    .bind(turn_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| {
        let role: String = row.get("role");
        let author = match role.as_str() {
            "operator" => "Operator",
            _ => "User",
        }
        .to_string();
        let status: String = row.get("status");
        AgentRuntimeChatEntry {
            id: format!("turn:{turn_id}:user"),
            author: author.clone(),
            display_label: author,
            timestamp: Some(time(row.get("started_at"))),
            body: row.get("input_text"),
            subtitle: status.clone(),
            kind: "message".to_string(),
            status: status.clone(),
            process_id: None,
                tool_call_id: None,
                script_run_id: None,
                stdout_artifact_id: None,
                stderr_artifact_id: None,
            command: String::new(),
            output: String::new(),
            image_preview_base64: None,
            image_preview_content_type: None,
            image_preview_error: None,
            delivery_state: if status == "completed" { "delivered" } else { "sending" }.to_string(),
            is_streaming: status == "running",
            is_tool: false,
        }
    }))
}

async fn runtime_error_chat_entry(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
) -> Result<Option<AgentRuntimeChatEntry>> {
    let row = sqlx::query(
        r#"
        SELECT id, payload, created_at
        FROM model_events
        WHERE session_id = $1 AND turn_id = $2 AND event_type = 'runtime_error'
        ORDER BY ordinal DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .bind(turn_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| {
        let id = row.get::<Uuid, _>("id");
        let payload: Value = row.get("payload");
        let body = payload
            .get("finalText")
            .and_then(Value::as_str)
            .or_else(|| payload.get("summary").and_then(Value::as_str))
            .unwrap_or("Runtime could not start the model request. Check the role settings and try again.")
            .to_string();
        AgentRuntimeChatEntry {
            id: format!("runtime:{id}:error"),
            author: "Runtime".to_string(),
            display_label: "Runtime".to_string(),
            timestamp: Some(time(row.get("created_at"))),
            body,
            subtitle: "failed".to_string(),
            kind: "system_error".to_string(),
            status: "failed".to_string(),
            process_id: None,
                tool_call_id: None,
                script_run_id: None,
                stdout_artifact_id: None,
                stderr_artifact_id: None,
            command: String::new(),
            output: String::new(),
            image_preview_base64: None,
            image_preview_content_type: None,
            image_preview_error: None,
            delivery_state: "failed".to_string(),
            is_streaming: false,
            is_tool: false,
        }
    }))
}

async fn assistant_chat_entry(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
) -> Result<Option<AgentRuntimeChatEntry>> {
    let row = sqlx::query(
        r#"
        SELECT id, payload, created_at
        FROM model_events
        WHERE session_id = $1 AND turn_id = $2 AND event_type = 'final_response'
        ORDER BY ordinal DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .bind(turn_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| {
        let id = row.get::<Uuid, _>("id");
        let payload: Value = row.get("payload");
        let body = payload
            .get("finalText")
            .and_then(Value::as_str)
            .or_else(|| payload.get("summary").and_then(Value::as_str))
            .unwrap_or("")
            .to_string();
        AgentRuntimeChatEntry {
            id: format!("model:{id}:assistant"),
            author: "Assistant".to_string(),
            display_label: "Assistant".to_string(),
            timestamp: Some(time(row.get("created_at"))),
            body,
            subtitle: "completed".to_string(),
            kind: "message".to_string(),
            status: "completed".to_string(),
            process_id: None,
                tool_call_id: None,
                script_run_id: None,
                stdout_artifact_id: None,
                stderr_artifact_id: None,
            command: String::new(),
            output: String::new(),
            image_preview_base64: None,
            image_preview_content_type: None,
            image_preview_error: None,
            delivery_state: "delivered".to_string(),
            is_streaming: false,
            is_tool: false,
        }
    }))
}

async fn tool_chat_entries(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
) -> Result<Vec<AgentRuntimeChatEntry>> {
    let rows = sqlx::query(
        r#"
        SELECT tc.id AS tool_id, tc.tool_name, tc.status AS tool_status, tc.result, tc.started_at,
               sr.id AS script_id, sr.source, sr.status AS script_status, sr.final_output,
               COALESCE(sr.final_output, '') AS script_output, COALESCE(sr.stderr, '') AS script_stderr,
               stdout_artifact.id AS stdout_artifact_id, stderr_artifact.id AS stderr_artifact_id,
               mp.id AS process_id, mp.handle AS process_handle, mp.binary_name, mp.argv, mp.status AS process_status,
               (
                   SELECT string_agg(
                       artifact.stream || ' artifact ' || artifact.id::text || ' (' || artifact.byte_count::text || ' bytes, ' || artifact.line_count::text || ' lines)',
                       E'\n'
                       ORDER BY artifact.stream ASC, artifact.created_at ASC
                   )
                   FROM execution_output_artifacts artifact
                   WHERE artifact.tool_call_id = tc.id OR artifact.script_run_id = sr.id OR artifact.process_id = mp.id
                     AND artifact.stream IN ('stdout', 'stderr')
               ) AS artifact_output
        FROM tool_calls tc
        LEFT JOIN script_runs sr ON sr.tool_call_id = tc.id
        LEFT JOIN managed_processes mp ON mp.starting_turn_id = tc.turn_id
        LEFT JOIN LATERAL (
            SELECT id FROM execution_output_artifacts
            WHERE (tool_call_id = tc.id OR script_run_id = sr.id) AND stream = 'stdout'
            ORDER BY created_at DESC LIMIT 1
        ) stdout_artifact ON true
        LEFT JOIN LATERAL (
            SELECT id FROM execution_output_artifacts
            WHERE (tool_call_id = tc.id OR script_run_id = sr.id) AND stream = 'stderr'
            ORDER BY created_at DESC LIMIT 1
        ) stderr_artifact ON true
        WHERE tc.session_id = $1 AND tc.turn_id = $2
        ORDER BY tc.started_at ASC
        "#,
    )
    .bind(session_id)
    .bind(turn_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let tool_id = row.get::<Uuid, _>("tool_id");
            let tool_name: String = row.get("tool_name");
            let script_id: Option<Uuid> = row.get("script_id");
            let source: Option<String> = row.get("source");
            let process_id: Option<Uuid> = row.get("process_id");
            let process_handle: Option<String> = row.get("process_handle");
            let binary_name: Option<String> = row.get("binary_name");
            let argv: Option<Value> = row.get("argv");
            let artifact_output: Option<String> = row.get("artifact_output");
            let script_output: Option<String> = row.get("script_output");
            let script_stderr: Option<String> = row.get("script_stderr");
            let stdout_artifact_id: Option<Uuid> = row.get("stdout_artifact_id");
            let stderr_artifact_id: Option<Uuid> = row.get("stderr_artifact_id");
            let status = row
                .get::<Option<String>, _>("script_status")
                .or_else(|| row.get::<Option<String>, _>("process_status"))
                .unwrap_or_else(|| row.get("tool_status"));
            let command = source
                .or_else(|| {
                    binary_name.map(|binary| {
                        let args = argv
                            .and_then(|value| value.as_array().cloned())
                            .unwrap_or_default()
                            .into_iter()
                            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                            .collect::<Vec<_>>()
                            .join(" ");
                        if args.is_empty() { binary } else { format!("{binary} {args}") }
                    })
                })
                .unwrap_or_else(|| tool_name.clone());
            let output = match (
                script_output.filter(|value| !value.is_empty()),
                script_stderr.filter(|value| !value.is_empty()),
                artifact_output.filter(|value| !value.is_empty()),
            ) {
                (Some(explicit), Some(stderr), Some(artifacts)) => format!("{explicit}\nstderr:\n{stderr}\n{artifacts}"),
                (Some(explicit), Some(stderr), None) => format!("{explicit}\nstderr:\n{stderr}"),
                (Some(explicit), None, Some(artifacts)) => format!("{explicit}\n{artifacts}"),
                (Some(explicit), None, None) => explicit,
                (None, Some(stderr), Some(artifacts)) => format!("stderr:\n{stderr}\n{artifacts}"),
                (None, Some(stderr), None) => format!("stderr:\n{stderr}"),
                (None, None, Some(artifacts)) => artifacts,
                (None, None, None) => String::new(),
            };
            let process = process_id.map(|id| id.to_string());
            AgentRuntimeChatEntry {
                id: format!("tool:{tool_id}:{}", script_id.map(|id| id.to_string()).unwrap_or_else(|| "call".to_string())),
                author: "Tool".to_string(),
                display_label: "Tool".to_string(),
                timestamp: Some(time(row.get("started_at"))),
                body: String::new(),
                subtitle: process_handle.unwrap_or_else(|| tool_name.clone()),
                kind: tool_name,
                status: status.clone(),
                process_id: process,
                tool_call_id: Some(tool_id.to_string()),
                script_run_id: script_id.map(|id| id.to_string()),
                stdout_artifact_id: stdout_artifact_id.map(|id| id.to_string()),
                stderr_artifact_id: stderr_artifact_id.map(|id| id.to_string()),
                command,
                output,
                image_preview_base64: None,
                image_preview_content_type: None,
                image_preview_error: None,
                delivery_state: if status == "completed" { "delivered" } else { "running" }.to_string(),
                is_streaming: status == "running",
                is_tool: true,
            }
        })
        .collect())
}

async fn tool_chat_entry_for_tool(pool: &PgPool, tool_call_id: Uuid) -> Result<Option<AgentRuntimeChatEntry>> {
    let row = sqlx::query("SELECT session_id, turn_id FROM tool_calls WHERE id=$1")
        .bind(tool_call_id)
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let session_id: Uuid = row.get("session_id");
    let turn_id: Uuid = row.get("turn_id");
    Ok(tool_chat_entries(pool, session_id, turn_id)
        .await?
        .into_iter()
        .find(|entry| entry.id.starts_with(&format!("tool:{tool_call_id}:"))))
}

async fn tool_chat_entries_for_process(pool: &PgPool, process_id: Uuid) -> Result<Vec<AgentRuntimeChatEntry>> {
    let rows = sqlx::query("SELECT session_id, starting_turn_id FROM managed_processes WHERE id=$1")
        .bind(process_id)
        .fetch_all(pool)
        .await?;
    let mut entries = Vec::new();
    for row in rows {
        let session_id: Uuid = row.get("session_id");
        let Some(turn_id) = row.get::<Option<Uuid>, _>("starting_turn_id") else {
            continue;
        };
        entries.extend(
            tool_chat_entries(pool, session_id, turn_id)
                .await?
                .into_iter()
                .filter(|entry| entry.process_id.as_deref() == Some(&process_id.to_string())),
        );
    }
    Ok(entries)
}

fn event_summary(payload: &Value) -> Option<String> {
    payload
        .get("finalText")
        .or_else(|| payload.get("finalOutput"))
        .or_else(|| payload.get("reason"))
        .or_else(|| payload.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

async fn pending_approval_summaries(pool: &PgPool) -> Result<Vec<PendingApprovalSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT ar.id, ar.session_id, ar.turn_id, ar.action_name, ar.required_approver_kind, ar.status, ar.input_context, ar.created_at,
               ad.created_at AS decision_at,
               ad.reason AS decision_reason,
               pa.status AS resumable_action_status,
               COALESCE(pa.status IN ('approved', 'pendingApproval'), false) AS has_resumable_action
        FROM approval_requests ar
        LEFT JOIN LATERAL (
            SELECT created_at, reason
            FROM approval_decisions
            WHERE request_id = ar.id
            ORDER BY created_at DESC
            LIMIT 1
        ) ad ON true
        LEFT JOIN LATERAL (
            SELECT status
            FROM paused_actions
            WHERE approval_request_id = ar.id
            ORDER BY created_at DESC
            LIMIT 1
        ) pa ON true
        ORDER BY ar.created_at DESC
        LIMIT 50
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| pending_approval_summary_from_row(row))
        .collect())
}

fn pending_approval_summary_from_row(row: sqlx::postgres::PgRow) -> PendingApprovalSummary {
    let status: String = row.get("status");
    let has_resumable_action: bool = row.get("has_resumable_action");
    PendingApprovalSummary {
            id: row.get::<Uuid, _>("id").to_string(),
            session_id: row.get::<Uuid, _>("session_id").to_string(),
            turn_id: optional_uuid(row.get("turn_id")),
            action_name: row.get("action_name"),
            required_approver_kind: row.get("required_approver_kind"),
            status: status.clone(),
            can_decide: status == "pending",
            can_resume: status == "approved" && has_resumable_action,
            input_context: row.get("input_context"),
            created_at: Some(time(row.get("created_at"))),
            decision_at: optional_time(row.get("decision_at")),
            decision_reason: row.get("decision_reason"),
            resumable_action_status: row.get("resumable_action_status"),
    }
}

async fn pending_approval_summary(
    pool: &PgPool,
    approval_id: Uuid,
) -> Result<Option<PendingApprovalSummary>> {
    let row = sqlx::query(
        r#"
        SELECT ar.id, ar.session_id, ar.turn_id, ar.action_name, ar.required_approver_kind, ar.status, ar.input_context, ar.created_at,
               ad.created_at AS decision_at,
               ad.reason AS decision_reason,
               pa.status AS resumable_action_status,
               COALESCE(pa.status IN ('approved', 'pendingApproval'), false) AS has_resumable_action
        FROM approval_requests ar
        LEFT JOIN LATERAL (
            SELECT created_at, reason
            FROM approval_decisions
            WHERE request_id = ar.id
            ORDER BY created_at DESC
            LIMIT 1
        ) ad ON true
        LEFT JOIN LATERAL (
            SELECT status
            FROM paused_actions
            WHERE approval_request_id = ar.id
            ORDER BY created_at DESC
            LIMIT 1
        ) pa ON true
        WHERE ar.id = $1
        "#,
    )
    .bind(approval_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(pending_approval_summary_from_row))
}

async fn role_summaries(pool: &PgPool) -> Result<Vec<RoleSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT r.id, r.display_name, r.current_version_id, r.status, r.archived_at, rv.snapshot, rv.model_defaults,
               COALESCE((
                   SELECT jsonb_agg(jsonb_build_object(
                       'versionId', rv2.id::text,
                       'version', rv2.version,
                       'status', CASE WHEN rv2.id = r.current_version_id THEN 'current' ELSE 'available' END,
                       'createdAt', rv2.created_at::text
                   ) ORDER BY rv2.created_at DESC)
                   FROM role_versions rv2
                   WHERE rv2.role_id = r.id
               ), '[]'::jsonb) AS versions
        FROM roles r
        LEFT JOIN role_versions rv ON rv.id = r.current_version_id
        ORDER BY r.id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let model_defaults: Option<Value> = row.get("model_defaults");
            let snapshot: Option<Value> = row.get("snapshot");
            RoleSummary {
                id: row.get("id"),
                display_name: row.get("display_name"),
                current_version_id: optional_uuid(row.get("current_version_id")),
                status: row.get("status"),
                model: model_defaults
                    .and_then(|value| value.get("model").and_then(Value::as_str).map(str::to_string)),
                reasoning_effort: snapshot
                    .as_ref()
                    .and_then(|value| value.get("modelDefaults").and_then(|defaults| defaults.get("reasoningEffort")).and_then(Value::as_str).map(str::to_string)),
                archived_at: optional_time(row.get("archived_at")),
                version: snapshot.as_ref().and_then(|value| value.get("version").and_then(Value::as_str).map(str::to_string)),
                instruction_text: snapshot.as_ref().and_then(|value| value.get("instructionText").and_then(Value::as_str).map(str::to_string)),
                capabilities: snapshot
                    .as_ref()
                    .and_then(|value| value.get("capabilities").and_then(Value::as_array))
                    .map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect())
                    .unwrap_or_default(),
                policy: snapshot
                    .as_ref()
                    .and_then(|value| value.get("policy").and_then(Value::as_object))
                    .map(|policy| {
                        policy
                            .iter()
                            .filter_map(|(action, decision)| decision.as_str().map(|decision| (action.clone(), decision.to_string())))
                            .collect()
                    })
                    .unwrap_or_default(),
                routing: snapshot.as_ref().and_then(|value| value.get("routing").cloned()).unwrap_or(Value::Null),
                visibility: snapshot.as_ref().and_then(|value| value.get("visibility").cloned()).unwrap_or(Value::Null),
                lifecycle_authority: snapshot.as_ref().and_then(|value| value.get("lifecycleAuthority").cloned()).unwrap_or(Value::Null),
                versions: serde_json::from_value(row.get::<Value, _>("versions")).unwrap_or_default(),
            }
        })
        .collect())
}

async fn role_summary(pool: &PgPool, role_id: &str) -> Result<Option<RoleSummary>> {
    let row = sqlx::query(
        r#"
        SELECT r.id, r.display_name, r.current_version_id, r.status, r.archived_at, rv.snapshot, rv.model_defaults,
               COALESCE((
                   SELECT jsonb_agg(jsonb_build_object(
                       'versionId', rv2.id::text,
                       'version', rv2.version,
                       'status', CASE WHEN rv2.id = r.current_version_id THEN 'current' ELSE 'available' END,
                       'createdAt', rv2.created_at::text
                   ) ORDER BY rv2.created_at DESC)
                   FROM role_versions rv2
                   WHERE rv2.role_id = r.id
               ), '[]'::jsonb) AS versions
        FROM roles r
        LEFT JOIN role_versions rv ON rv.id = r.current_version_id
        WHERE r.id = $1
        "#,
    )
    .bind(role_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| {
        let model_defaults: Option<Value> = row.get("model_defaults");
        let snapshot: Option<Value> = row.get("snapshot");
        RoleSummary {
            id: row.get("id"),
            display_name: row.get("display_name"),
            current_version_id: optional_uuid(row.get("current_version_id")),
            status: row.get("status"),
            model: model_defaults
                .and_then(|value| value.get("model").and_then(Value::as_str).map(str::to_string)),
            reasoning_effort: snapshot
                .as_ref()
                .and_then(|value| value.get("modelDefaults").and_then(|defaults| defaults.get("reasoningEffort")).and_then(Value::as_str).map(str::to_string)),
            archived_at: optional_time(row.get("archived_at")),
            version: snapshot.as_ref().and_then(|value| value.get("version").and_then(Value::as_str).map(str::to_string)),
            instruction_text: snapshot.as_ref().and_then(|value| value.get("instructionText").and_then(Value::as_str).map(str::to_string)),
            capabilities: snapshot
                .as_ref()
                .and_then(|value| value.get("capabilities").and_then(Value::as_array))
                .map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
            policy: snapshot
                .as_ref()
                .and_then(|value| value.get("policy").and_then(Value::as_object))
                .map(|policy| {
                    policy
                        .iter()
                        .filter_map(|(action, decision)| decision.as_str().map(|decision| (action.clone(), decision.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
            routing: snapshot.as_ref().and_then(|value| value.get("routing").cloned()).unwrap_or(Value::Null),
            visibility: snapshot.as_ref().and_then(|value| value.get("visibility").cloned()).unwrap_or(Value::Null),
            lifecycle_authority: snapshot.as_ref().and_then(|value| value.get("lifecycleAuthority").cloned()).unwrap_or(Value::Null),
            versions: serde_json::from_value(row.get::<Value, _>("versions")).unwrap_or_default(),
        }
    }))
}

fn command_config_string(config: &Option<Value>, key: &str) -> Option<String> {
    config.as_ref()?.get(key)?.as_str().map(str::to_string)
}

fn command_config_bool(config: &Option<Value>, key: &str) -> Option<bool> {
    config.as_ref()?.get(key)?.as_bool()
}

fn command_config_i64(config: &Option<Value>, key: &str) -> Option<i64> {
    config.as_ref()?.get(key)?.as_i64()
}

fn command_config_string_list(config: &Option<Value>, key: &str) -> Vec<String> {
    config.as_ref().and_then(|value| value.get(key)).and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

async fn command_registry_summaries(pool: &PgPool) -> Result<Vec<CommandRegistrySummary>> {
    let rows = sqlx::query(
        r#"
        SELECT cd.id, cd.action_id, cd.scope_type, cd.project_key, cd.enabled, cd.current_version_id,
               cd.updated_at, cv.version_number, cv.binary_name, cv.starlark_object, cv.starlark_method, cv.config, cv.model_description
        FROM command_definitions cd
        LEFT JOIN command_versions cv ON cv.id = cd.current_version_id
        ORDER BY cd.scope_type ASC, cd.action_id ASC, cd.project_key ASC NULLS FIRST
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| CommandRegistrySummary {
            id: row.get::<Uuid, _>("id").to_string(),
            action_id: row.get("action_id"),
            scope_type: row.get("scope_type"),
            project_key: row.get("project_key"),
            enabled: row.get("enabled"),
            current_version_id: optional_uuid(row.get("current_version_id")),
            command_version: row.get::<Option<i64>, _>("version_number"),
            binary_name: row.get("binary_name"),
            starlark_object: row.get("starlark_object"),
            starlark_method: row.get("starlark_method"),
            argv_template: command_config_string_list(&row.get::<Option<Value>, _>("config"), "argvPrefix"),
            default_cwd: command_config_string(&row.get::<Option<Value>, _>("config"), "defaultCwd"),
            cwd_policy: command_config_string(&row.get::<Option<Value>, _>("config"), "cwdPolicy"),
            env_policy: command_config_string(&row.get::<Option<Value>, _>("config"), "envPolicy"),
            stdin_policy: command_config_string(&row.get::<Option<Value>, _>("config"), "stdinPolicy"),
            sync_allowed: command_config_bool(&row.get::<Option<Value>, _>("config"), "syncAllowed"),
            async_allowed: command_config_bool(&row.get::<Option<Value>, _>("config"), "asyncAllowed"),
            max_runtime_ms: command_config_i64(&row.get::<Option<Value>, _>("config"), "maxRuntimeMs"),
            end_of_turn_behavior: command_config_string(&row.get::<Option<Value>, _>("config"), "endOfTurnBehavior"),
            end_of_session_behavior: command_config_string(&row.get::<Option<Value>, _>("config"), "endOfSessionBehavior"),
            mutation_class: command_config_string(&row.get::<Option<Value>, _>("config"), "mutationClass"),
            model_description: row.get::<Option<String>, _>("model_description"),
            allow_cwd_arg: command_config_bool(&row.get::<Option<Value>, _>("config"), "allowCwdArg"),
            allow_args_arg: command_config_bool(&row.get::<Option<Value>, _>("config"), "allowArgsArg"),
            forbidden_args: command_config_string_list(&row.get::<Option<Value>, _>("config"), "forbiddenArgs"),
            execution_policy: command_config_string(&row.get::<Option<Value>, _>("config"), "executionPolicy"),
            updated_at: optional_time(Some(row.get("updated_at"))),
        })
        .collect())
}

async fn command_summary_for_event(
    pool: &PgPool,
    payload: &Value,
) -> Result<Option<CommandRegistrySummary>> {
    let definition_id = payload
        .get("definitionId")
        .or_else(|| payload.get("commandDefinitionId"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    if let Some(definition_id) = definition_id {
        return command_registry_summary_by_definition_id(pool, definition_id).await;
    }
    let action_id = payload
        .get("actionId")
        .or_else(|| payload.pointer("/finalCommand/actionId"))
        .or_else(|| payload.pointer("/proposedCommand/actionId"))
        .and_then(Value::as_str);
    if let Some(action_id) = action_id {
        return command_registry_summary_by_action_id(pool, action_id).await;
    }
    Ok(None)
}

async fn command_registry_summary_by_definition_id(
    pool: &PgPool,
    definition_id: Uuid,
) -> Result<Option<CommandRegistrySummary>> {
    let row = sqlx::query(
        r#"
        SELECT cd.id, cd.action_id, cd.scope_type, cd.project_key, cd.enabled, cd.current_version_id,
               cd.updated_at, cv.version_number, cv.binary_name, cv.starlark_object, cv.starlark_method, cv.config, cv.model_description
        FROM command_definitions cd
        LEFT JOIN command_versions cv ON cv.id = cd.current_version_id
        WHERE cd.id = $1
        "#,
    )
    .bind(definition_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(command_registry_summary_from_row))
}

async fn command_registry_summary_by_action_id(
    pool: &PgPool,
    action_id: &str,
) -> Result<Option<CommandRegistrySummary>> {
    let row = sqlx::query(
        r#"
        SELECT cd.id, cd.action_id, cd.scope_type, cd.project_key, cd.enabled, cd.current_version_id,
               cd.updated_at, cv.version_number, cv.binary_name, cv.starlark_object, cv.starlark_method, cv.config, cv.model_description
        FROM command_definitions cd
        LEFT JOIN command_versions cv ON cv.id = cd.current_version_id
        WHERE cd.action_id = $1
        ORDER BY cd.scope_type ASC, cd.project_key ASC NULLS FIRST
        LIMIT 1
        "#,
    )
    .bind(action_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(command_registry_summary_from_row))
}

fn command_registry_summary_from_row(row: sqlx::postgres::PgRow) -> CommandRegistrySummary {
    CommandRegistrySummary {
        id: row.get::<Uuid, _>("id").to_string(),
        action_id: row.get("action_id"),
        scope_type: row.get("scope_type"),
        project_key: row.get("project_key"),
        enabled: row.get("enabled"),
        current_version_id: optional_uuid(row.get("current_version_id")),
        command_version: row.get::<Option<i64>, _>("version_number"),
        binary_name: row.get("binary_name"),
        starlark_object: row.get("starlark_object"),
        starlark_method: row.get("starlark_method"),
        argv_template: command_config_string_list(&row.get::<Option<Value>, _>("config"), "argvPrefix"),
        default_cwd: command_config_string(&row.get::<Option<Value>, _>("config"), "defaultCwd"),
        cwd_policy: command_config_string(&row.get::<Option<Value>, _>("config"), "cwdPolicy"),
        env_policy: command_config_string(&row.get::<Option<Value>, _>("config"), "envPolicy"),
        stdin_policy: command_config_string(&row.get::<Option<Value>, _>("config"), "stdinPolicy"),
        sync_allowed: command_config_bool(&row.get::<Option<Value>, _>("config"), "syncAllowed"),
        async_allowed: command_config_bool(&row.get::<Option<Value>, _>("config"), "asyncAllowed"),
        max_runtime_ms: command_config_i64(&row.get::<Option<Value>, _>("config"), "maxRuntimeMs"),
        end_of_turn_behavior: command_config_string(&row.get::<Option<Value>, _>("config"), "endOfTurnBehavior"),
        end_of_session_behavior: command_config_string(&row.get::<Option<Value>, _>("config"), "endOfSessionBehavior"),
        mutation_class: command_config_string(&row.get::<Option<Value>, _>("config"), "mutationClass"),
        model_description: row.get::<Option<String>, _>("model_description"),
        allow_cwd_arg: command_config_bool(&row.get::<Option<Value>, _>("config"), "allowCwdArg"),
        allow_args_arg: command_config_bool(&row.get::<Option<Value>, _>("config"), "allowArgsArg"),
        forbidden_args: command_config_string_list(&row.get::<Option<Value>, _>("config"), "forbiddenArgs"),
        execution_policy: command_config_string(&row.get::<Option<Value>, _>("config"), "executionPolicy"),
        updated_at: optional_time(Some(row.get("updated_at"))),
    }
}

async fn command_registry_request_summaries(pool: &PgPool) -> Result<Vec<CommandRegistryRequestSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT id, operation, proposed_command, approval_status, application_status,
               final_scope, final_execution_policy
        FROM command_registry_requests
        WHERE approval_status = 'pending'
           OR (approval_status = 'approved' AND application_status = 'pending')
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(command_registry_request_summary_from_row)
        .collect())
}

async fn command_registry_request_summary(
    pool: &PgPool,
    request_id: Uuid,
) -> Result<Option<CommandRegistryRequestSummary>> {
    let row = sqlx::query(
        r#"
        SELECT id, operation, proposed_command, approval_status, application_status,
               final_scope, final_execution_policy
        FROM command_registry_requests
        WHERE id = $1
          AND (
              approval_status = 'pending'
              OR (approval_status = 'approved' AND application_status = 'pending')
          )
        "#,
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(command_registry_request_summary_from_row))
}

fn command_registry_request_summary_from_row(row: sqlx::postgres::PgRow) -> Option<CommandRegistryRequestSummary> {
    CommandRegistryRequestSummary::from_server_value(&serde_json::json!({
        "id": row.get::<Uuid, _>("id"),
        "operation": row.get::<String, _>("operation"),
        "proposedCommand": row.get::<Value, _>("proposed_command"),
        "approvalStatus": row.get::<String, _>("approval_status"),
        "applicationStatus": row.get::<String, _>("application_status"),
        "finalScope": row.get::<Option<Value>, _>("final_scope"),
        "finalExecutionPolicy": row.get::<Option<Value>, _>("final_execution_policy"),
    }))
}

fn command_registry_request_id_for_event(item: &TimelineItem, payload: &Value) -> Option<Uuid> {
    item.entity_id
        .as_deref()
        .and_then(|id| Uuid::parse_str(id).ok())
        .or_else(|| {
            payload
                .get("requestId")
                .and_then(Value::as_str)
                .and_then(|id| Uuid::parse_str(id).ok())
        })
}

async fn workflow_memory_summaries(
    pool: &PgPool,
    selected_session_id: Option<Uuid>,
) -> Result<Vec<WorkflowMemorySummary>> {
    let Some(session_id) = selected_session_id else {
        return Ok(Vec::new());
    };
    let project_key = crate::db::session_project_key(pool, session_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT wm.id, wm.session_id, wm.script_run_id, wm.scope_type, wm.project_key, wm.title, wm.reason,
               wm.summary, wm.helpful_score, wm.promoted_at, wm.provider, wm.model, wm.dimensions,
               wm.storage_type, wm.source_hash, wm.command_fingerprint, sr.source
        FROM workflow_memories wm
        LEFT JOIN script_runs sr ON sr.id = wm.script_run_id
        WHERE wm.scope_type='global'
           OR (wm.scope_type='project' AND COALESCE(wm.project_key,'')=COALESCE($1,''))
        ORDER BY wm.promoted_at DESC
        LIMIT 200
        "#,
    )
    .bind(project_key.as_deref())
    .fetch_all(pool)
    .await?;
    let mut memories = Vec::new();
    for row in rows {
        memories.push(workflow_memory_summary_from_row(pool, row).await?);
    }
    Ok(memories)
}

async fn workflow_memory_summary(
    pool: &PgPool,
    memory_id: Uuid,
) -> Result<Option<WorkflowMemorySummary>> {
    let row = sqlx::query(
        r#"
        SELECT wm.id, wm.session_id, wm.script_run_id, wm.scope_type, wm.project_key, wm.title, wm.reason,
               wm.summary, wm.helpful_score, wm.promoted_at, wm.provider, wm.model, wm.dimensions,
               wm.storage_type, wm.source_hash, wm.command_fingerprint, sr.source
        FROM workflow_memories wm
        LEFT JOIN script_runs sr ON sr.id = wm.script_run_id
        WHERE wm.id = $1
        "#,
    )
    .bind(memory_id)
    .fetch_optional(pool)
    .await?;
    match row {
        Some(row) => Ok(Some(workflow_memory_summary_from_row(pool, row).await?)),
        None => Ok(None),
    }
}

async fn workflow_memory_summary_from_row(pool: &PgPool, row: sqlx::postgres::PgRow) -> Result<WorkflowMemorySummary> {
    let memory_id: Uuid = row.get("id");
    let source: Option<String> = row.get("source");
    let recent_events = workflow_memory_recent_events(pool, memory_id).await?;
    Ok(WorkflowMemorySummary {
        id: memory_id.to_string(),
        session_id: row.get::<Uuid, _>("session_id").to_string(),
        source_script_run_id: Some(row.get::<Uuid, _>("script_run_id").to_string()),
        scope_type: row.get("scope_type"),
        project_key: row.get("project_key"),
        title: row.get("title"),
        reason: row.get("reason"),
        summary: row.get("summary"),
        helpful_score: row.get("helpful_score"),
        promoted_at: optional_time(Some(row.get("promoted_at"))),
        source_preview: bounded_source_preview(source.as_deref().unwrap_or_default()),
        source_starlark: source,
        provider: Some(row.get("provider")),
        model: Some(row.get("model")),
        dimensions: Some(row.get("dimensions")),
        storage_type: Some(row.get("storage_type")),
        source_hash: Some(row.get("source_hash")),
        command_fingerprint: Some(row.get("command_fingerprint")),
        recent_events,
    })
}

async fn workflow_memory_recent_events(pool: &PgPool, memory_id: Uuid) -> Result<Vec<WorkflowMemoryEventSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT id, event_type, payload, created_at
        FROM workflow_memory_events
        WHERE memory_id=$1
        ORDER BY created_at DESC
        LIMIT 8
        "#,
    )
    .bind(memory_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| WorkflowMemoryEventSummary {
            id: row.get::<Uuid, _>("id").to_string(),
            event_type: row.get("event_type"),
            created_at: optional_time(Some(row.get("created_at"))),
            payload_summary: bounded_source_preview(&row.get::<Value, _>("payload").to_string()),
        })
        .collect())
}

fn bounded_source_preview(source: &str) -> String {
    let compact = source.trim();
    if compact.len() <= 900 {
        compact.to_string()
    } else {
        format!("{}…", compact.chars().take(900).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command_registry, db, roles::RoleRegistry};
    use serde_json::json;
    use sqlx::PgPool;

    async fn create_validation_database(suffix: &str) -> (String, String, PgPool) {
        let admin_url = std::env::var("ROBDEX_AGENT_RUNTIME_VALIDATION_ADMIN_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres@127.0.0.1:5432/postgres".to_string());
        let database_name = format!(
            "robdex_agent_runtime_validation_{}_{}_{}",
            suffix,
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default().abs()
        );
        assert!(database_name.starts_with("robdex_agent_runtime_validation_"));
        let admin_pool = db::connect(&admin_url).await.expect("connect validation admin Postgres");
        sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
            .execute(&admin_pool)
            .await
            .expect("create validation database");
        let runtime_url = format!("{}/{}", admin_url.rsplit_once('/').map(|(base, _)| base).unwrap_or(&admin_url), database_name);
        (admin_url, database_name, db::connect(&runtime_url).await.expect("connect runtime validation database"))
    }

    async fn drop_validation_database(admin_url: &str, database_name: &str) {
        let admin_pool = db::connect(admin_url).await.expect("connect validation admin for cleanup");
        sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{database_name}" WITH (FORCE)"#))
            .execute(&admin_pool)
            .await
            .expect("drop validation database");
    }

    async fn seed_durable_selected_chat(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid, Uuid, Uuid, Uuid, String) {
        db::init(pool).await.expect("init schema");
        let session_id = Uuid::new_v4();
        let turn_id = Uuid::new_v4();
        let tool_id = Uuid::new_v4();
        let script_id = Uuid::new_v4();
        let process_id = Uuid::new_v4();
        let artifact_id = Uuid::new_v4();
        let model_event_id = Uuid::new_v4();
        let final_text = "## Distinctive final\n\n- exact **markdown** response\n- no placeholder".to_string();
        sqlx::query("INSERT INTO sessions (id, status, role_id, project_key, workdir, title, tracked) VALUES ($1,'open','runtime-allow','project-a','/tmp/project-a','Durable selected chat',true)")
            .bind(session_id)
            .execute(pool)
            .await
            .expect("insert session");
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, completed_at) VALUES ($1,$2,'user',$3,'completed',now())")
            .bind(turn_id)
            .bind(session_id)
            .bind("Exact submitted composer text")
            .execute(pool)
            .await
            .expect("insert turn");
        for (event_type, entity_type, entity_id, status, payload) in [
            ("role.imported", "role", Uuid::new_v4(), Some("completed"), json!({"roleId":"runtime-allow"})),
            ("turn.started", "turn", turn_id, Some("running"), json!({"input":"raw event input must not be chat"})),
            ("policy.decision", "policy", Uuid::new_v4(), Some("completed"), json!({"decision":"allow"})),
            ("tool.completed", "tool", tool_id, Some("completed"), json!({"summary":"raw tool event must stay history"})),
            ("model.final_response", "turn", turn_id, Some("completed"), json!({"finalText":"raw final event must not fabricate selected chat"})),
            ("turn.completed", "turn", turn_id, Some("completed"), json!({"status":"completed"})),
        ] {
            sqlx::query("INSERT INTO event_stream (session_id, turn_id, entity_type, entity_id, event_type, status, payload) VALUES ($1,$2,$3,$4,$5,$6,$7)")
                .bind(session_id)
                .bind(turn_id)
                .bind(entity_type)
                .bind(entity_id)
                .bind(event_type)
                .bind(status)
                .bind(payload)
                .execute(pool)
                .await
                .expect("insert runtime event");
        }
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status, result, completed_at) VALUES ($1,$2,$3,'execute_code','call-1',$4,'completed',$5,now())")
            .bind(tool_id)
            .bind(session_id)
            .bind(turn_id)
            .bind(json!({"source":"print('canonical tool output')"}))
            .bind(json!({"ok":true}))
            .execute(pool)
            .await
            .expect("insert tool call");
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status, final_output, stdout, stderr, completed_at) VALUES ($1,$2,$3,'completed',$4,$5,'',now())")
            .bind(script_id)
            .bind(tool_id)
            .bind("print('canonical tool output')")
            .bind("canonical final output")
            .bind("canonical stdout preview")
            .execute(pool)
            .await
            .expect("insert script run");
        sqlx::query("INSERT INTO managed_processes (id, handle, session_id, starting_turn_id, binary_name, argv, cwd, status, end_of_turn_behavior, end_of_session_behavior, metadata) VALUES ($1,'proc-chat-1',$2,$3,'python',$4,'/tmp/project-a','completed','terminate','terminate',$5)")
            .bind(process_id)
            .bind(session_id)
            .bind(turn_id)
            .bind(json!(["-c", "print('canonical')"]))
            .bind(json!({"label":"process proof"}))
            .execute(pool)
            .await
            .expect("insert managed process");
        sqlx::query("INSERT INTO execution_output_artifacts (id, session_id, turn_id, tool_call_id, script_run_id, process_id, source_type, stream, content, byte_count, line_count, metadata) VALUES ($1,$2,$3,$4,$5,$6,'script','stdout',$7,64,2,$8)")
            .bind(artifact_id)
            .bind(session_id)
            .bind(turn_id)
            .bind(tool_id)
            .bind(script_id)
            .bind(process_id)
            .bind("artifact bounded output preview hidden-full-body-sentinel")
            .bind(json!({"artifactIdentifier":"stdout-artifact"}))
            .execute(pool)
            .await
            .expect("insert output artifact");
        sqlx::query("INSERT INTO model_events (id, session_id, turn_id, event_type, payload) VALUES ($1,$2,$3,'final_response',$4)")
            .bind(model_event_id)
            .bind(session_id)
            .bind(turn_id)
            .bind(json!({"finalText": final_text, "summary":"summary fallback must not win"}))
            .execute(pool)
            .await
            .expect("insert model event");
        (session_id, turn_id, tool_id, script_id, process_id, artifact_id, model_event_id, final_text)
    }

    #[tokio::test]
    async fn selected_conversation_is_built_from_durable_chat_sources_not_event_stream_labels() {
        let (admin_url, database_name, pool) = create_validation_database("selected_chat_sources").await;
        let (session_id, _turn_id, tool_id, script_id, _process_id, artifact_id, _model_event_id, final_text) = seed_durable_selected_chat(&pool).await;
        let snapshot = build_runtime_projection_snapshot(&pool, Some(session_id)).await.expect("projection");
        drop(pool);
        drop_validation_database(&admin_url, &database_name).await;

        assert_eq!(snapshot.selected_chat_entries.len(), 3);
        assert_eq!(snapshot.selected_chat_entries[0].author, "User");
        assert_eq!(snapshot.selected_chat_entries[0].body, "Exact submitted composer text");
        assert_eq!(snapshot.selected_chat_entries[1].author, "Tool");
        assert!(snapshot.selected_chat_entries[1].is_tool);
        assert_eq!(snapshot.selected_chat_entries[1].tool_call_id, Some(tool_id.to_string()));
        assert_eq!(snapshot.selected_chat_entries[1].script_run_id, Some(script_id.to_string()));
        assert_eq!(snapshot.selected_chat_entries[1].stdout_artifact_id, Some(artifact_id.to_string()));
        assert_eq!(snapshot.selected_chat_entries[2].author, "Assistant");
        assert_eq!(snapshot.selected_chat_entries[2].body, final_text);
        let forbidden = [
            "role.imported",
            "turn.started",
            "policy.decision",
            "tool.completed",
            "model.final_response",
            "turn.completed",
        ];
        for entry in &snapshot.selected_chat_entries {
            for field in [
                entry.id.as_str(),
                entry.author.as_str(),
                entry.display_label.as_str(),
                entry.body.as_str(),
                entry.subtitle.as_str(),
                entry.kind.as_str(),
                entry.status.as_str(),
                entry.command.as_str(),
                entry.output.as_str(),
            ] {
                assert!(!forbidden.iter().any(|raw| field.contains(raw)), "raw event {field} leaked into selected chat");
            }
        }
        assert!(snapshot.timeline.iter().any(|item| item.event_type == "role.imported"));
        assert!(snapshot.timeline.iter().any(|item| item.event_type == "tool.completed"));
    }

    #[tokio::test]
    async fn selected_conversation_projects_stored_image_artifacts_as_chat_image_entries() {
        let (admin_url, database_name, pool) = create_validation_database("selected_image_chat").await;
        let (session_id, turn_id, tool_id, script_id, process_id, _artifact_id, _model_event_id, _final_text) = seed_durable_selected_chat(&pool).await;
        let image_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO starter_image_artifacts (
                id, session_id, turn_id, tool_call_id, script_run_id, process_id,
                source_type, source_path, mime_type, byte_count, width, height,
                retrieval_metadata, binary_content
            ) VALUES ($1,$2,$3,$4,$5,$6,'screenshot','/tmp/evidence.png','image/png',8,1,1,$7,decode('89504e470d0a1a0a','hex'))
            "#,
        )
        .bind(image_id)
        .bind(session_id)
        .bind(turn_id)
        .bind(tool_id)
        .bind(script_id)
        .bind(process_id)
        .bind(json!({"description":"Screenshot evidence"}))
        .execute(&pool)
        .await
        .expect("insert image artifact");

        let snapshot = build_runtime_projection_snapshot(&pool, Some(session_id)).await.expect("projection");
        drop(pool);
        drop_validation_database(&admin_url, &database_name).await;

        let image = snapshot
            .selected_chat_entries
            .iter()
            .find(|entry| entry.id == format!("imageArtifact:{image_id}"))
            .expect("image artifact chat entry");
        assert_eq!(image.author, "Runtime");
        assert_eq!(image.display_label, "Image artifact");
        assert_eq!(image.kind, "imageView");
        assert_eq!(image.status, "stored");
        assert_eq!(image.body, "Screenshot evidence");
        assert_eq!(image.image_preview_content_type.as_deref(), Some("image/png"));
        assert_eq!(image.image_preview_base64.as_deref(), Some("iVBORw0KGgo="));
        assert_eq!(image.output, format!("agent-runtime-image://{session_id}/{image_id}"));
        assert!(image.subtitle.contains("image/png"));
        assert!(image.subtitle.contains("1 × 1"));
    }

    #[tokio::test]
    async fn selected_conversation_renders_every_stored_image_artifact_without_cap() {
        let (admin_url, database_name, pool) = create_validation_database("all_selected_images").await;
        let (session_id, turn_id, tool_id, script_id, process_id, _artifact_id, _model_event_id, _final_text) = seed_durable_selected_chat(&pool).await;
        let image_count = 55;
        for index in 0..image_count {
            let image_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO starter_image_artifacts (
                    id, session_id, turn_id, tool_call_id, script_run_id, process_id,
                    source_type, source_path, mime_type, byte_count, width, height,
                    retrieval_metadata, binary_content, created_at
                ) VALUES ($1,$2,$3,$4,$5,$6,'screenshot',NULL,'image/png',8,1,1,$7,decode('89504e470d0a1a0a','hex'), now() + ($8::int * interval '1 second'))
                "#,
            )
            .bind(image_id)
            .bind(session_id)
            .bind(turn_id)
            .bind(tool_id)
            .bind(script_id)
            .bind(process_id)
            .bind(json!({"description": format!("Screenshot evidence {index}")}))
            .bind(index)
            .execute(&pool)
            .await
            .expect("insert image artifact");
        }

        let snapshot = build_runtime_projection_snapshot(&pool, Some(session_id)).await.expect("projection");
        drop(pool);
        drop_validation_database(&admin_url, &database_name).await;

        let rendered_images = snapshot
            .selected_chat_entries
            .iter()
            .filter(|entry| entry.kind == "imageView")
            .count();
        assert_eq!(rendered_images, image_count as usize);
    }

    #[tokio::test]
    async fn assistant_final_response_text_is_preserved_exactly_in_selected_conversation() {
        let (admin_url, database_name, pool) = create_validation_database("assistant_final_exact").await;
        let (session_id, _turn_id, _tool_id, _script_id, _process_id, _artifact_id, _model_event_id, final_text) = seed_durable_selected_chat(&pool).await;
        let snapshot = build_runtime_projection_snapshot(&pool, Some(session_id)).await.expect("projection");
        drop(pool);
        drop_validation_database(&admin_url, &database_name).await;

        let assistant = snapshot
            .selected_chat_entries
            .iter()
            .find(|entry| entry.author == "Assistant")
            .expect("assistant selected chat entry");
        assert_eq!(assistant.body, final_text);
        assert_ne!(assistant.body, "Response completed");
        assert_ne!(assistant.body, "Output details are available in History");
        assert!(!assistant.body.contains("read History"));
    }

    #[tokio::test]
    async fn selected_tool_rows_carry_canonical_chat_timeline_fields() {
        let (admin_url, database_name, pool) = create_validation_database("tool_row_fields").await;
        let (session_id, _turn_id, _tool_id, _script_id, process_id, artifact_id, _model_event_id, _final_text) = seed_durable_selected_chat(&pool).await;
        let after: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(sequence), 0) FROM event_stream")
            .fetch_one(&pool)
            .await
            .expect("watermark");
        sqlx::query("INSERT INTO event_stream (session_id, entity_type, entity_id, event_type, status, payload) VALUES ($1,'process',$2,'process.output','completed',$3)")
            .bind(session_id)
            .bind(process_id)
            .bind(json!({"reason":"projection safety delta"}))
            .execute(&pool)
            .await
            .expect("process output event");
        let snapshot = build_runtime_projection_snapshot(&pool, Some(session_id)).await.expect("projection");
        let deltas = build_projection_deltas_after(&pool, after, Some(session_id), 100).await.expect("projection deltas");

        let tool = snapshot
            .selected_chat_entries
            .iter()
            .find(|entry| entry.is_tool)
            .expect("tool selected chat entry");
        assert_eq!(tool.author, "Tool");
        assert_eq!(tool.display_label, "Tool");
        assert_eq!(tool.kind, "execute_code");
        assert_eq!(tool.status, "completed");
        let expected_process_id = process_id.to_string();
        assert_eq!(tool.process_id.as_deref(), Some(expected_process_id.as_str()));
        assert!(tool.command.contains("print('canonical tool output')"));
        assert!(!tool.output.contains("artifact bounded output preview hidden-full-body-sentinel"));
        assert!(tool.output.contains(&artifact_id.to_string()));
        assert!(tool.output.contains("stdout artifact"));
        assert!(tool.output.contains("64 bytes"));
        assert!(!tool.output.contains("combined"));
        assert!(tool.subtitle.contains("proc-chat-1") || tool.subtitle.contains("execute_code"));
        assert_eq!(tool.delivery_state, "delivered");
        let delta_text = serde_json::to_string(&deltas).expect("delta json");
        assert!(delta_text.contains(&artifact_id.to_string()));
        assert!(delta_text.contains("stdout artifact"));
        assert!(!delta_text.contains("artifact bounded output preview hidden-full-body-sentinel"));
        assert!(!delta_text.contains("combined"));
        drop(pool);
        drop_validation_database(&admin_url, &database_name).await;
    }

    #[tokio::test]
    async fn submitted_steering_inputs_render_inside_the_owning_turn_in_timeline_order() {
        let (admin_url, database_name, pool) = create_validation_database("submitted_input_chat_order").await;
        let (session_id, turn_id, _tool_id, _script_id, _process_id, _artifact_id, _model_event_id, final_text) =
            seed_durable_selected_chat(&pool).await;
        let submitted_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO submitted_inputs (
                id, session_id, target_turn_id, actor, source, role, content, payload,
                disposition, status, observed_lifecycle_state, accepted_at, applied_at
            )
            VALUES ($1,$2,$3,'operator','gui','operator',$4,$5,
                'active_turn_steering','applied','open',now(),now())
            "#,
        )
        .bind(submitted_id)
        .bind(session_id)
        .bind(turn_id)
        .bind("Apply this steering after the tool output is durable.")
        .bind(json!({"content":"Apply this steering after the tool output is durable."}))
        .execute(&pool)
        .await
        .expect("insert submitted steering");

        let snapshot = build_runtime_projection_snapshot(&pool, Some(session_id)).await.expect("projection");
        drop(pool);
        drop_validation_database(&admin_url, &database_name).await;

        let ids = snapshot
            .selected_chat_entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>();
        let user_index = ids.iter().position(|id| id.starts_with(&format!("turn:{turn_id}:user"))).expect("turn user entry");
        let steering_index = ids.iter().position(|id| id.starts_with(&format!("submitted:{submitted_id}:"))).expect("submitted steering entry");
        let tool_index = ids.iter().position(|id| id.starts_with("tool:")).expect("tool entry");
        let assistant_index = ids.iter().position(|id| id.starts_with("model:")).expect("assistant entry");
        assert!(user_index < steering_index, "steering must render after initial user input");
        assert!(steering_index < tool_index, "steering must render inside the owning turn before durable tool output");
        assert!(tool_index < assistant_index, "tool output remains before final assistant output");
        let steering = &snapshot.selected_chat_entries[steering_index];
        assert_eq!(steering.body, "Apply this steering after the tool output is durable.");
        assert_eq!(steering.delivery_state, "delivered");
        assert_eq!(snapshot.selected_chat_entries[assistant_index].body, final_text);
    }

    #[tokio::test]
    async fn post_final_and_post_compaction_submitted_inputs_render_as_placed_user_turns_with_audit_rows() {
        let (admin_url, database_name, pool) = create_validation_database("submitted_input_placed_turns").await;
        db::init(&pool).await.expect("init schema");
        let session_id = Uuid::new_v4();
        let final_turn_id = Uuid::new_v4();
        let post_final_turn_id = Uuid::new_v4();
        let post_compaction_turn_id = Uuid::new_v4();
        let post_final_submitted_id = Uuid::new_v4();
        let post_compaction_submitted_id = Uuid::new_v4();
        let checkpoint_id = Uuid::new_v4();
        sqlx::query("INSERT INTO sessions (id, status, role_id, project_key, workdir, title, tracked) VALUES ($1,'open','runtime-allow','project-a','/tmp/project-a','Placed submitted chat',true)")
            .bind(session_id)
            .execute(&pool)
            .await
            .expect("insert session");
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at, completed_at) VALUES ($1,$2,'user','initial before committed final','completed',now() - interval '4 minutes',now() - interval '4 minutes')")
            .bind(final_turn_id)
            .bind(session_id)
            .execute(&pool)
            .await
            .expect("insert final turn");
        sqlx::query("INSERT INTO model_events (id, session_id, turn_id, event_type, payload, created_at) VALUES ($1,$2,$3,'final_response',$4,now() - interval '4 minutes')")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .bind(final_turn_id)
            .bind(json!({"finalText":"committed final stays before next input"}))
            .execute(&pool)
            .await
            .expect("insert final response");
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at, completed_at) VALUES ($1,$2,'user','first input after committed final','completed',now() - interval '3 minutes',now() - interval '3 minutes')")
            .bind(post_final_turn_id)
            .bind(session_id)
            .execute(&pool)
            .await
            .expect("post-final turn");
        sqlx::query("INSERT INTO submitted_inputs (id, session_id, actor, source, role, content, payload, disposition, status, observed_lifecycle_state, placement_turn_id, accepted_at, applied_at) VALUES ($1,$2,'operator','gui','user','first input after committed final',$3,'queued_next_turn_after_final_output','applied','open',$4,now() - interval '3 minutes',now() - interval '3 minutes')")
            .bind(post_final_submitted_id)
            .bind(session_id)
            .bind(json!({"content":"first input after committed final"}))
            .bind(post_final_turn_id)
            .execute(&pool)
            .await
            .expect("post-final submitted audit");
        sqlx::query("INSERT INTO compaction_checkpoints (id, session_id, status, compacted_through_turn_id, replacement_context, summary, completed_at) VALUES ($1,$2,'completed',$3,'compacted before queued input',$4,now() - interval '2 minutes')")
            .bind(checkpoint_id)
            .bind(session_id)
            .bind(post_final_turn_id)
            .bind(json!({"summary":"compaction completed before queued input placement"}))
            .execute(&pool)
            .await
            .expect("checkpoint");
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at, completed_at) VALUES ($1,$2,'user','first input after compaction','completed',now() - interval '60 seconds',now() - interval '60 seconds')")
            .bind(post_compaction_turn_id)
            .bind(session_id)
            .execute(&pool)
            .await
            .expect("post-compaction turn");
        sqlx::query("INSERT INTO submitted_inputs (id, session_id, actor, source, role, content, payload, disposition, status, observed_lifecycle_state, placement_turn_id, accepted_at, applied_at) VALUES ($1,$2,'operator','gui','user','first input after compaction',$3,'queued_continuation_after_compaction','applied','open',$4,now() - interval '90 seconds',now() - interval '60 seconds')")
            .bind(post_compaction_submitted_id)
            .bind(session_id)
            .bind(json!({"content":"first input after compaction"}))
            .bind(post_compaction_turn_id)
            .execute(&pool)
            .await
            .expect("post-compaction submitted audit");

        let snapshot = build_runtime_projection_snapshot(&pool, Some(session_id)).await.expect("projection");
        let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE id IN ($1,$2) AND status='applied' AND placement_turn_id IS NOT NULL")
            .bind(post_final_submitted_id)
            .bind(post_compaction_submitted_id)
            .fetch_one(&pool)
            .await
            .expect("audit count");
        drop(pool);
        drop_validation_database(&admin_url, &database_name).await;

        assert_eq!(audit_count, 2, "visible placement must preserve submitted-input audit rows");
        let bodies = snapshot
            .selected_chat_entries
            .iter()
            .map(|entry| entry.body.as_str())
            .collect::<Vec<_>>();
        let final_index = bodies.iter().position(|body| *body == "committed final stays before next input").expect("final response");
        let post_final_index = bodies.iter().position(|body| *body == "first input after committed final").expect("post-final input");
        let post_compaction_index = bodies.iter().position(|body| *body == "first input after compaction").expect("post-compaction input");
        assert!(final_index < post_final_index, "post-final submitted input must render after the committed final response");
        assert!(post_final_index < post_compaction_index, "compaction-queued input must render after prior placed turn");
    }

    #[tokio::test]
    async fn runtime_statistics_count_authoritative_tables_without_event_stream_rows() {
        let (admin_url, database_name, pool) = create_validation_database("authoritative_stats_no_events").await;
        db::init(&pool).await.expect("init schema");
        let session_id = Uuid::new_v4();
        let turn_id = Uuid::new_v4();
        let model_event_id = Uuid::new_v4();
        let tool_id = Uuid::new_v4();
        let script_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let command_id = Uuid::new_v4();
        let process_id = Uuid::new_v4();
        let artifact_id = Uuid::new_v4();
        let checkpoint_id = Uuid::new_v4();
        let approval_id = Uuid::new_v4();
        let registry_request_id = Uuid::new_v4();
        let other_session_id = Uuid::new_v4();
        let other_turn_id = Uuid::new_v4();
        sqlx::query("INSERT INTO sessions (id, status, role_id, project_key, workdir, title, tracked) VALUES ($1,'open','runtime-allow','stats-project','/tmp/stats','Stats seed',true)")
            .bind(session_id)
            .execute(&pool)
            .await
            .expect("session");
        sqlx::query("INSERT INTO sessions (id, status, role_id, project_key, workdir, title, tracked) VALUES ($1,'open','runtime-allow','stats-project','/tmp/other','Stats other',true)")
            .bind(other_session_id)
            .execute(&pool)
            .await
            .expect("other session");
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, completed_at) VALUES ($1,$2,'user','stats turn','failed',now())")
            .bind(turn_id).bind(session_id).execute(&pool).await.expect("turn");
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, completed_at) VALUES ($1,$2,'user','other turn','completed',now())")
            .bind(other_turn_id).bind(other_session_id).execute(&pool).await.expect("other turn");
        sqlx::query("INSERT INTO model_events (id, session_id, turn_id, event_type, payload) VALUES ($1,$2,$3,'final_response',$4)")
            .bind(model_event_id).bind(session_id).bind(turn_id).bind(json!({"finalText":"stats final"})).execute(&pool).await.expect("model");
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status, completed_at) VALUES ($1,$2,$3,'execute_code','stats-call','{}'::jsonb,'lost',now())")
            .bind(tool_id).bind(session_id).bind(turn_id).execute(&pool).await.expect("tool");
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status, stdout, stderr, completed_at) VALUES ($1,$2,'print(1)','running','','',NULL)")
            .bind(script_id).bind(tool_id).execute(&pool).await.expect("script");
        sqlx::query("INSERT INTO host_api_calls (id, script_run_id, api_name, input, status) VALUES ($1,$2,'fs.read','{}'::jsonb,'completed')")
            .bind(host_id).bind(script_id).execute(&pool).await.expect("host");
        sqlx::query("INSERT INTO command_runs (id, host_api_call_id, binary_name, argv, cwd, status) VALUES ($1,$2,'echo','[]'::jsonb,'.','completed')")
            .bind(command_id).bind(host_id).execute(&pool).await.expect("command");
        sqlx::query("INSERT INTO managed_processes (id, handle, session_id, starting_turn_id, binary_name, argv, cwd, status, end_of_turn_behavior, end_of_session_behavior, metadata) VALUES ($1,'stats-process',$2,$3,'sleep','[]'::jsonb,'.','running','terminate','terminate','{}'::jsonb)")
            .bind(process_id).bind(session_id).bind(turn_id).execute(&pool).await.expect("process");
        sqlx::query("INSERT INTO execution_output_artifacts (id, session_id, turn_id, tool_call_id, script_run_id, command_run_id, process_id, source_type, stream, content, byte_count, line_count, metadata) VALUES ($1,$2,$3,$4,$5,$6,$7,'script','stdout','stats artifact',14,1,'{}'::jsonb)")
            .bind(artifact_id).bind(session_id).bind(turn_id).bind(tool_id).bind(script_id).bind(command_id).bind(process_id).execute(&pool).await.expect("artifact");
        sqlx::query("INSERT INTO compaction_checkpoints (id, session_id, status, replacement_context, summary, completed_at) VALUES ($1,$2,'completed','stats','{}'::jsonb,now())")
            .bind(checkpoint_id).bind(session_id).execute(&pool).await.expect("checkpoint");
        sqlx::query("INSERT INTO approval_requests (id, session_id, turn_id, action_name, requested_by_role, input_context, required_approver_kind, status) VALUES ($1,$2,$3,'tool.execute','{}'::jsonb,'{}'::jsonb,'owner','pending')")
            .bind(approval_id).bind(session_id).bind(turn_id).execute(&pool).await.expect("approval");
        sqlx::query("INSERT INTO command_registry_requests (id, session_id, operation, proposed_command, rationale, recommended_policy, requester, approval_status, application_status) VALUES ($1,$2,'add',$3,'stats','allow','test','pending','pending')")
            .bind(registry_request_id)
            .bind(session_id)
            .bind(json!({"actionId":"cmd.stats","binaryName":"echo"}))
            .execute(&pool)
            .await
            .expect("registry request");
        let event_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1").bind(session_id).fetch_one(&pool).await.expect("event count");
        assert_eq!(event_rows, 0);
        let snapshot = build_runtime_projection_snapshot(&pool, Some(session_id)).await.expect("selected projection");
        let global_snapshot = build_runtime_projection_snapshot(&pool, None).await.expect("global projection");
        drop(pool);
        drop_validation_database(&admin_url, &database_name).await;

        assert_eq!(snapshot.statistics.sessions, 1);
        assert_eq!(snapshot.statistics.open_sessions, 1);
        assert_eq!(snapshot.statistics.closed_sessions, 0);
        assert_eq!(snapshot.statistics.archived_sessions, 0);
        assert_eq!(snapshot.statistics.turns, 1);
        assert_eq!(snapshot.statistics.running_turns, 0);
        assert_eq!(snapshot.statistics.failed_turns, 1);
        assert_eq!(snapshot.statistics.model_events, 1);
        assert_eq!(snapshot.statistics.tool_calls, 1);
        assert_eq!(snapshot.statistics.script_runs, 1);
        assert_eq!(snapshot.statistics.host_api_calls, 1);
        assert_eq!(snapshot.statistics.command_runs, 1);
        assert_eq!(snapshot.statistics.managed_processes, 1);
        assert_eq!(snapshot.statistics.output_artifacts, 1);
        assert_eq!(snapshot.statistics.compaction_checkpoints, 1);
        assert_eq!(snapshot.statistics.approval_requests, 1);
        assert_eq!(snapshot.statistics.command_registry_requests, 1);
        assert_eq!(snapshot.statistics.workflow_memories, 0);
        assert_eq!(snapshot.statistics.failed_rows, 1);
        assert_eq!(snapshot.statistics.running_rows, 2);
        assert_eq!(snapshot.statistics.lost_rows, 1);
        let selected = snapshot.selected_session.as_ref().expect("selected session detail");
        assert_eq!(selected.managed_process_count, 1);
        assert_eq!(selected.managed_processes.len(), 1);
        let process = &selected.managed_processes[0];
        assert_eq!(process.handle, "stats-process");
        assert_eq!(process.status, "running");
        assert_eq!(process.binary_name, "sleep");
        assert_eq!(process.cwd, ".");
        assert_eq!(process.end_of_turn_behavior, "terminate");
        assert_eq!(process.end_of_session_behavior, "terminate");
        assert!(process.can_terminate);
        assert!(process.can_flush);
        assert!(!process.can_input);
        assert_eq!(process.latest_output_summary.as_deref(), Some("stdout: 1 lines, 14 bytes"));
        assert!(process.output_artifacts.iter().any(|artifact| artifact["artifactId"] == artifact_id.to_string()));
        assert_eq!(global_snapshot.statistics.sessions, 2);
        assert_eq!(global_snapshot.statistics.open_sessions, 2);
        assert_eq!(global_snapshot.statistics.turns, 2);
        assert_eq!(global_snapshot.statistics.failed_turns, 1);
        assert_eq!(global_snapshot.statistics.model_events, 1);
        assert_eq!(global_snapshot.statistics.managed_processes, 1);
    }

    #[tokio::test]
    async fn selected_session_managed_processes_are_projected_from_current_rows_not_timeline_events() {
        let (admin_url, database_name, pool) = create_validation_database("managed_process_rows").await;
        db::init(&pool).await.expect("init schema");
        let session_id = Uuid::new_v4();
        let turn_id = Uuid::new_v4();
        let running_id = Uuid::new_v4();
        let completed_id = Uuid::new_v4();
        let lost_id = Uuid::new_v4();
        let timeline_only_id = Uuid::new_v4();
        sqlx::query("INSERT INTO sessions (id, status, role_id, project_key, workdir, title, tracked) VALUES ($1,'open','runtime-allow','project-a','/tmp/processes','Process row proof',true)")
            .bind(session_id)
            .execute(&pool)
            .await
            .expect("session");
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, completed_at) VALUES ($1,$2,'user','start processes','completed',now())")
            .bind(turn_id)
            .bind(session_id)
            .execute(&pool)
            .await
            .expect("turn");
        for (id, handle, status, end_time, stdin_policy, binary, argv) in [
            (running_id, "row-running", "running", false, "allow", "python", json!(["-m", "http.server"])),
            (completed_id, "row-completed", "completed", true, "none", "echo", json!(["done"])),
            (lost_id, "row-lost", "lost", true, "forbid", "tail", json!(["-f", "app.log"])),
        ] {
            sqlx::query(
                r#"
                INSERT INTO managed_processes (
                    id, handle, session_id, starting_turn_id, binary_name, argv, cwd, os_pid,
                    status, start_time, end_time, end_of_turn_behavior, end_of_session_behavior, metadata
                )
                VALUES ($1,$2,$3,$4,$5,$6,'/tmp/processes',4242,$7,now() - interval '5 minutes',
                    CASE WHEN $8 THEN now() - interval '1 minute' ELSE NULL END,
                    'continue','terminate',$9)
                "#,
            )
            .bind(id)
            .bind(handle)
            .bind(session_id)
            .bind(turn_id)
            .bind(binary)
            .bind(argv)
            .bind(status)
            .bind(end_time)
            .bind(json!({"stdinPolicy": stdin_policy}))
            .execute(&pool)
            .await
            .expect("managed process");
        }
        sqlx::query("INSERT INTO event_stream (session_id, turn_id, entity_type, entity_id, event_type, status, payload) VALUES ($1,$2,'process',$3,'process.started','lost',$4)")
            .bind(session_id)
            .bind(turn_id)
            .bind(running_id)
            .bind(json!({"handle":"row-running","binary":"timeline-binary","status":"lost"}))
            .execute(&pool)
            .await
            .expect("conflicting process event");
        sqlx::query("INSERT INTO event_stream (session_id, turn_id, entity_type, entity_id, event_type, status, payload) VALUES ($1,$2,'process',$3,'process.started','running',$4)")
            .bind(session_id)
            .bind(turn_id)
            .bind(timeline_only_id)
            .bind(json!({"handle":"timeline-only","binary":"timeline","argv":["fake"],"cwd":"/tmp/event-only"}))
            .execute(&pool)
            .await
            .expect("timeline-only process event");

        let snapshot = build_runtime_projection_snapshot(&pool, Some(session_id)).await.expect("projection");
        let event_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1")
            .bind(session_id)
            .fetch_one(&pool)
            .await
            .expect("event rows");
        drop(pool);
        drop_validation_database(&admin_url, &database_name).await;

        assert_eq!(event_rows, 2);
        let selected = snapshot.selected_session.as_ref().expect("selected session");
        assert_eq!(selected.managed_process_count, 3);
        assert_eq!(selected.managed_processes.len(), 3);
        let by_handle = selected
            .managed_processes
            .iter()
            .map(|process| (process.handle.as_str(), process))
            .collect::<std::collections::BTreeMap<_, _>>();
        let running = by_handle.get("row-running").expect("running row");
        assert_eq!(running.status, "running");
        assert!(running.command_label.contains("python"));
        assert!(running.started_at.is_some());
        assert!(running.ended_at.is_none());
        assert!(running.can_terminate);
        assert!(running.can_flush);
        assert!(running.can_input);
        let completed = by_handle.get("row-completed").expect("completed row");
        assert_eq!(completed.status, "completed");
        assert!(completed.started_at.is_some());
        assert!(completed.ended_at.is_some());
        assert!(!completed.can_terminate);
        assert!(completed.can_flush);
        assert!(!completed.can_input);
        let lost = by_handle.get("row-lost").expect("lost row");
        assert_eq!(lost.status, "lost");
        assert!(lost.started_at.is_some());
        assert!(lost.ended_at.is_some());
        assert!(!lost.can_terminate);
        assert!(lost.can_flush);
        assert!(!lost.can_input);
        assert!(!by_handle.contains_key("timeline-only"), "timeline-only process event must not create a selected managed-process row");
        assert!(
            selected
                .managed_processes
                .iter()
                .all(|process| !process.command_label.contains("timeline-binary")),
            "event-stream process payload must not override authoritative managed_processes rows"
        );
    }

    #[tokio::test]
    async fn postgres_backed_snapshot_builds_from_current_schema() {
        let admin_url = std::env::var("ROBDEX_AGENT_RUNTIME_VALIDATION_ADMIN_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres@127.0.0.1:5432/postgres".to_string());
        let database_name = format!(
            "robdex_agent_runtime_validation_projection_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default().abs()
        );
        assert!(
            database_name.starts_with("robdex_agent_runtime_validation_"),
            "validation database name must use protected prefix"
        );
        let admin_pool = db::connect(&admin_url)
            .await
            .expect("connect validation admin Postgres");
        sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
            .execute(&admin_pool)
            .await
            .expect("create validation database");

        let runtime_url = format!("{}/{}", admin_url.rsplit_once('/').map(|(base, _)| base).unwrap_or(&admin_url), database_name);
        let validation = async {
            let pool = db::connect(&runtime_url).await?;
            db::init(&pool).await?;
            let registry = RoleRegistry::default_for_workspace()?;
            for path in registry.manifest_paths()? {
                let imported = registry.load_for_import(&path)?;
                db::import_role_version(&pool, &imported).await?;
            }
            let role = db::current_role_snapshot(&pool, "runtime-allow").await?;
            let session_id = db::new_session(
                &pool,
                &role,
                Some("projection-validation"),
                ".",
                Some("."),
                Some("Projection validation"),
                Some("projection-validation"),
            )
            .await?;
            let approval_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO approval_requests (
                    id, session_id, turn_id, action_name, requested_by_role, input_context,
                    required_approver_kind, status, created_at, completed_at
                ) VALUES ($1, $2, NULL, 'fs.write', '{}'::jsonb, '{"decision":"approvalRequired"}'::jsonb,
                    'owner', 'approved', now(), now())
                "#,
            )
            .bind(approval_id)
            .bind(session_id)
            .execute(&pool)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO approval_decisions (id, request_id, decision, reason, decided_by, created_at)
                VALUES ($1, $2, 'approved', 'projection approval proof', '{"kind":"test"}'::jsonb, now())
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(approval_id)
            .execute(&pool)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO paused_actions (
                    id, approval_request_id, session_id, turn_id, tool_call_id, script_run_id,
                    action_name, action_input, role_snapshot, status, created_at, updated_at
                ) VALUES ($1, $2, $3, NULL, NULL, NULL, 'fs.write', $4, $5, 'pendingApproval', now(), now())
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(approval_id)
            .bind(session_id)
            .bind(json!({"path":"proof.txt","content":"proof"}))
            .bind(json!({"id": role.id, "version": role.version, "roleVersionId": role.role_version_id}))
            .execute(&pool)
            .await?;
            let registry_request_id = Uuid::new_v4();
            let proposed_command = serde_json::to_value(command_registry::CommandSeed {
                action_id: "cmd.projection.request".to_string(),
                binary_name: "rg".to_string(),
                candidate_paths: vec!["/usr/bin/rg".to_string()],
                starlark_object: "rg".to_string(),
                starlark_method: "projection".to_string(),
                argv_prefix: Vec::new(),
                default_cwd: ".".to_string(),
                cwd_policy: "underExecutionRoot".to_string(),
                env_policy: "empty".to_string(),
                sync_allowed: true,
                async_allowed: false,
                max_runtime_ms: Some(5000),
                end_of_turn_behavior: "terminate".to_string(),
                end_of_session_behavior: "terminate".to_string(),
                stdin_policy: "forbid".to_string(),
                min_await_ms: 0,
                max_await_ms: 60000,
                output_buffer_bytes: 64000,
                terminate_grace_ms: 1000,
                output_limit_bytes: 12000,
                mutation_class: "readOnly".to_string(),
                model_description: "projection request proof".to_string(),
                allow_cwd_arg: true,
                allow_args_arg: true,
                forbidden_args: Vec::new(),
                execution_policy: "allow".to_string(),
            })?;
            sqlx::query(
                r#"
                INSERT INTO command_registry_requests (
                    id, session_id, operation, proposed_command, requester_context,
                    rationale, recommended_policy, requester, requested_by_role,
                    approval_status, application_status
                ) VALUES ($1, $2, 'add', $3, '{}'::jsonb, 'projection proof',
                    'approver selects final policy', 'test', '{}'::jsonb, 'pending', 'pending')
                "#,
            )
            .bind(registry_request_id)
            .bind(session_id)
            .bind(proposed_command)
            .execute(&pool)
            .await?;
            let snapshot = build_runtime_projection_snapshot(&pool, Some(session_id)).await?;
            anyhow::Ok(snapshot)
        }
        .await;

        let drop_result = sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{database_name}" WITH (FORCE)"#))
            .execute(&admin_pool)
            .await;
        drop_result.expect("drop validation database");

        let snapshot = validation.expect("build projection snapshot from migrated validation database");
        assert_eq!(snapshot.server_status.status, "ok");
        assert!(snapshot.watermark >= 1);
        assert_eq!(snapshot.sessions.len(), 1);
        assert!(snapshot.selected_session.is_some());
        assert!(!snapshot.roles.is_empty());
        assert!(!snapshot.command_registry.is_empty());
        let registry_request = snapshot
            .command_registry_requests
            .iter()
            .find(|request| request.action_id == "cmd.projection.request")
            .expect("pending command-registry request is projected");
        assert!(registry_request.can_preview);
        assert!(registry_request.can_decide);
        assert!(!registry_request.can_apply);
        assert_eq!(registry_request.state_text, "Needs registry decision");
        assert_eq!(snapshot.timeline.len(), 1);
        let resumable = snapshot
            .pending_approvals
            .iter()
            .find(|approval| approval.status == "approved" && approval.action_name == "fs.write")
            .expect("approved resumable approval is projected");
        assert!(!resumable.can_decide);
        assert!(resumable.can_resume);
        assert!(resumable.decision_at.is_some());
        assert_eq!(resumable.decision_reason.as_deref(), Some("projection approval proof"));
        assert_eq!(resumable.resumable_action_status.as_deref(), Some("pendingApproval"));
    }
}
