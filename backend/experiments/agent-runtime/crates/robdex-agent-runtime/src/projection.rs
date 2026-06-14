use anyhow::Result;
use robdex_agent_runtime_projection::{
    timeline_by_sequence, timeline_item_id, CommandRegistrySummary, PendingApprovalSummary,
    RoleSummary, RuntimeDelta, RuntimeDeltaKind, RuntimeProjection, SelectedSessionDetail,
    ServerStatusProjection, SessionListItem, TimelineItem, WorkflowMemorySummary,
};
use serde_json::Value;
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
        sessions: session_list_items(pool).await?,
        selected_session: selected_session_detail(pool, selected_session_id).await?,
        timeline: timeline_items(pool, selected_session_id).await?,
        pending_approvals: pending_approval_summaries(pool).await?,
        roles: role_summaries(pool).await?,
        command_registry: command_registry_summaries(pool).await?,
        workflow_memories: workflow_memory_summaries(pool, selected_session_id).await?,
        resync_required: None,
    };
    projection.timeline = timeline_by_sequence(projection.timeline);
    Ok(projection)
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
        WHERE tracked = true OR status <> 'open'
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
        WHERE id = $1 AND (tracked = true OR status <> 'open')
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
        SELECT id, status, role_id, role_version, project_key, workdir, worktree_root, title, name, metadata
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
    Ok(Some(SelectedSessionDetail {
        id: row.get::<Uuid, _>("id").to_string(),
        role_id: row.get("role_id"),
        role_version: row.get("role_version"),
        project_key: row.get("project_key"),
        workdir: row.get("workdir"),
        worktree_root: row.get("worktree_root"),
        title: row.get("title"),
        name: row.get("name"),
        status: row.get("status"),
        pending_approval_count: pending_approval_count.max(0) as u64,
        managed_process_count: managed_process_count.max(0) as u64,
        metadata: row.get("metadata"),
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
        SELECT id, session_id, turn_id, action_name, required_approver_kind, status, input_context, created_at
        FROM approval_requests
        WHERE status = 'pending'
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| PendingApprovalSummary {
            id: row.get::<Uuid, _>("id").to_string(),
            session_id: row.get::<Uuid, _>("session_id").to_string(),
            turn_id: optional_uuid(row.get("turn_id")),
            action_name: row.get("action_name"),
            required_approver_kind: row.get("required_approver_kind"),
            status: row.get("status"),
            input_context: row.get("input_context"),
            created_at: Some(time(row.get("created_at"))),
        })
        .collect())
}

async fn pending_approval_summary(
    pool: &PgPool,
    approval_id: Uuid,
) -> Result<Option<PendingApprovalSummary>> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, turn_id, action_name, required_approver_kind, status, input_context, created_at
        FROM approval_requests
        WHERE id = $1 AND status = 'pending'
        "#,
    )
    .bind(approval_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| PendingApprovalSummary {
        id: row.get::<Uuid, _>("id").to_string(),
        session_id: row.get::<Uuid, _>("session_id").to_string(),
        turn_id: optional_uuid(row.get("turn_id")),
        action_name: row.get("action_name"),
        required_approver_kind: row.get("required_approver_kind"),
        status: row.get("status"),
        input_context: row.get("input_context"),
        created_at: Some(time(row.get("created_at"))),
    }))
}

async fn role_summaries(pool: &PgPool) -> Result<Vec<RoleSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT r.id, r.display_name, r.current_version_id, r.status, r.archived_at, rv.model_defaults
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
            RoleSummary {
                id: row.get("id"),
                display_name: row.get("display_name"),
                current_version_id: optional_uuid(row.get("current_version_id")),
                status: row.get("status"),
                model: model_defaults
                    .and_then(|value| value.get("model").and_then(Value::as_str).map(str::to_string)),
                archived_at: optional_time(row.get("archived_at")),
            }
        })
        .collect())
}

async fn role_summary(pool: &PgPool, role_id: &str) -> Result<Option<RoleSummary>> {
    let row = sqlx::query(
        r#"
        SELECT r.id, r.display_name, r.current_version_id, r.status, r.archived_at, rv.model_defaults
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
        RoleSummary {
            id: row.get("id"),
            display_name: row.get("display_name"),
            current_version_id: optional_uuid(row.get("current_version_id")),
            status: row.get("status"),
            model: model_defaults
                .and_then(|value| value.get("model").and_then(Value::as_str).map(str::to_string)),
            archived_at: optional_time(row.get("archived_at")),
        }
    }))
}

async fn command_registry_summaries(pool: &PgPool) -> Result<Vec<CommandRegistrySummary>> {
    let rows = sqlx::query(
        r#"
        SELECT cd.id, cd.action_id, cd.scope_type, cd.project_key, cd.enabled, cd.current_version_id,
               cd.updated_at, cv.binary_name, cv.starlark_object, cv.starlark_method
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
            binary_name: row.get("binary_name"),
            starlark_object: row.get("starlark_object"),
            starlark_method: row.get("starlark_method"),
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
               cd.updated_at, cv.binary_name, cv.starlark_object, cv.starlark_method
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
               cd.updated_at, cv.binary_name, cv.starlark_object, cv.starlark_method
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
        binary_name: row.get("binary_name"),
        starlark_object: row.get("starlark_object"),
        starlark_method: row.get("starlark_method"),
        updated_at: optional_time(Some(row.get("updated_at"))),
    }
}

async fn workflow_memory_summaries(
    pool: &PgPool,
    selected_session_id: Option<Uuid>,
) -> Result<Vec<WorkflowMemorySummary>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, scope_type, project_key, title, reason, helpful_score, promoted_at
        FROM workflow_memories
        WHERE $1::uuid IS NULL OR session_id = $1
        ORDER BY promoted_at DESC
        LIMIT 200
        "#,
    )
    .bind(selected_session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| WorkflowMemorySummary {
            id: row.get::<Uuid, _>("id").to_string(),
            session_id: row.get::<Uuid, _>("session_id").to_string(),
            scope_type: row.get("scope_type"),
            project_key: row.get("project_key"),
            title: row.get("title"),
            reason: row.get("reason"),
            helpful_score: row.get("helpful_score"),
            promoted_at: optional_time(Some(row.get("promoted_at"))),
        })
        .collect())
}

async fn workflow_memory_summary(
    pool: &PgPool,
    memory_id: Uuid,
) -> Result<Option<WorkflowMemorySummary>> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, scope_type, project_key, title, reason, helpful_score, promoted_at
        FROM workflow_memories
        WHERE id = $1
        "#,
    )
    .bind(memory_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| WorkflowMemorySummary {
        id: row.get::<Uuid, _>("id").to_string(),
        session_id: row.get::<Uuid, _>("session_id").to_string(),
        scope_type: row.get("scope_type"),
        project_key: row.get("project_key"),
        title: row.get("title"),
        reason: row.get("reason"),
        helpful_score: row.get("helpful_score"),
        promoted_at: optional_time(Some(row.get("promoted_at"))),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, roles::RoleRegistry};

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
        assert_eq!(snapshot.timeline.len(), 1);
    }
}
