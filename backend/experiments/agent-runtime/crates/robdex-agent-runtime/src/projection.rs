use anyhow::Result;
use robdex_agent_runtime_projection::{
    timeline_by_sequence, timeline_item_id, CommandRegistrySummary, PendingApprovalSummary,
    RoleSummary, RuntimeProjection, SelectedSessionDetail, ServerStatusProjection, SessionListItem,
    TimelineItem, WorkflowMemorySummary,
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
        })
        .collect())
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
