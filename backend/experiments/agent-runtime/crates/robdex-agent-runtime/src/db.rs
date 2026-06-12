use anyhow::Result;
use serde_json::Value;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use uuid::Uuid;

use crate::roles::{ImportedRoleVersion, RoleSnapshot, snapshot_from_value, snapshot_to_value};

pub async fn connect(database_url: &str) -> Result<PgPool> {
    Ok(PgPoolOptions::new().max_connections(5).connect(database_url).await?)
}

pub async fn init(pool: &PgPool) -> Result<()> {
    sqlx::raw_sql(include_str!("../../../migrations/001_initial.sql"))
        .execute(pool)
        .await?;
    reconcile_managed_processes(pool).await?;
    crate::command_registry::bootstrap_seed_defaults(pool).await?;
    Ok(())
}

pub async fn reconcile_managed_processes(pool: &PgPool) -> Result<()> {
    let rows = sqlx::query(
        r#"
        UPDATE managed_processes
        SET status = 'lost',
            end_time = now(),
            termination_reason = 'runtimeRestart'
        WHERE status = 'running'
        RETURNING id, session_id, starting_turn_id, handle, command_version_id
        "#,
    )
    .fetch_all(pool)
    .await?;
    for row in rows {
        let process_id: Uuid = row.get("id");
        let session_id: Uuid = row.get("session_id");
        let turn_id: Option<Uuid> = row.get("starting_turn_id");
        let handle: String = row.get("handle");
        let command_version_id: Option<Uuid> = row.get("command_version_id");
        append_event(
            pool,
            session_id,
            turn_id,
            "process",
            Some(process_id),
            "process.lost",
            Some("lost"),
            serde_json::json!({
                "handle": handle,
                "commandVersionId": command_version_id,
                "reason": "session-only process is no longer attached after runtime startup",
            }),
        )
        .await?;
    }
    Ok(())
}

pub async fn import_role_version(pool: &PgPool, imported: &ImportedRoleVersion) -> Result<()> {
    let snapshot = &imported.snapshot;
    let snapshot_value = snapshot_to_value(snapshot)?;
    sqlx::query(
        r#"
        INSERT INTO roles (id, display_name, current_version_id, status, metadata, created_at, updated_at)
        VALUES ($1, $2, NULL, 'active', '{}'::jsonb, now(), now())
        ON CONFLICT (id) DO UPDATE
        SET display_name = EXCLUDED.display_name,
            status = 'active',
            updated_at = now()
        "#,
    )
    .bind(&snapshot.id)
    .bind(&snapshot.display_name)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO role_versions (
            id, role_id, version, display_name, instruction_text, manifest, model_defaults,
            policy, routing, visibility, lifecycle_authority, snapshot, created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(snapshot.role_version_id)
    .bind(&snapshot.id)
    .bind(&snapshot.version)
    .bind(&snapshot.display_name)
    .bind(&snapshot.instruction_text)
    .bind(&imported.manifest_json)
    .bind(serde_json::to_value(&snapshot.model_defaults)?)
    .bind(serde_json::to_value(&snapshot.policy)?)
    .bind(serde_json::to_value(&snapshot.routing)?)
    .bind(serde_json::to_value(&snapshot.visibility)?)
    .bind(serde_json::to_value(&snapshot.lifecycle_authority)?)
    .bind(&snapshot_value)
    .bind(snapshot.created_at)
    .execute(pool)
    .await?;

    sqlx::query("UPDATE roles SET current_version_id = $2, updated_at = now() WHERE id = $1")
    .bind(&snapshot.id)
    .bind(snapshot.role_version_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_roles(pool: &PgPool) -> Result<Vec<RoleSnapshot>> {
    let rows = sqlx::query(
        r#"
        SELECT rv.snapshot
        FROM roles r
        JOIN role_versions rv ON rv.id = r.current_version_id
        ORDER BY r.id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| snapshot_from_value(row.get("snapshot")))
        .collect()
}

pub async fn current_role_snapshot(pool: &PgPool, role_id: &str) -> Result<RoleSnapshot> {
    let row = sqlx::query(
        r#"
        SELECT rv.snapshot
        FROM roles r
        JOIN role_versions rv ON rv.id = r.current_version_id
        WHERE r.id = $1 AND r.status = 'active'
        "#,
    )
    .bind(role_id)
    .fetch_one(pool)
    .await?;
    snapshot_from_value(row.get("snapshot"))
}

pub async fn new_session(pool: &PgPool, role_snapshot: &RoleSnapshot, project_key: Option<&str>) -> Result<Uuid> {
    let id = Uuid::new_v4();
    let snapshot_value = snapshot_to_value(role_snapshot)?;
    sqlx::query(
        r#"
        INSERT INTO sessions (id, status, role_id, role_version, role_snapshot, project_key)
        VALUES ($1, 'open', $2, $3, $4, $5)
        "#,
    )
        .bind(id)
        .bind(&role_snapshot.id)
        .bind(&role_snapshot.version)
        .bind(&snapshot_value)
        .bind(project_key)
        .execute(pool)
        .await?;
    append_event(
        pool,
        id,
        None,
        "session",
        Some(id),
        "session.created",
        Some("open"),
        serde_json::json!({
            "role": {
                "id": role_snapshot.id,
                "version": role_snapshot.version,
                "snapshot": snapshot_value,
            },
            "projectKey": project_key,
        }),
    )
    .await?;
    Ok(id)
}

pub async fn session_project_key(pool: &PgPool, session_id: Uuid) -> Result<Option<String>> {
    let row = sqlx::query("SELECT project_key FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_one(pool)
        .await?;
    Ok(row.get("project_key"))
}

pub async fn session_process_handles(pool: &PgPool, session_id: Uuid) -> Result<Vec<String>> {
    let rows = sqlx::query(
        "SELECT handle FROM managed_processes WHERE session_id = $1 ORDER BY start_time ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.get("handle")).collect())
}

pub async fn session_role_snapshot(pool: &PgPool, session_id: Uuid) -> Result<RoleSnapshot> {
    let row = sqlx::query("SELECT role_snapshot FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_one(pool)
        .await?;
    let value: Value = row.get("role_snapshot");
    snapshot_from_value(value)
}

pub async fn append_event(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    entity_type: &str,
    entity_id: Option<Uuid>,
    event_type: &str,
    status: Option<&str>,
    payload: Value,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO event_stream (session_id, turn_id, entity_type, entity_id, event_type, status, payload)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(session_id)
    .bind(turn_id)
    .bind(entity_type)
    .bind(entity_id)
    .bind(event_type)
    .bind(status)
    .bind(payload)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn session_exists(pool: &PgPool, session_id: Uuid) -> Result<bool> {
    let row = sqlx::query("SELECT EXISTS (SELECT 1 FROM sessions WHERE id = $1)")
        .bind(session_id)
        .fetch_one(pool)
        .await?;
    Ok(row.get::<bool, _>(0))
}

pub async fn print_events(pool: &PgPool, session_id: Uuid) -> Result<()> {
    let rows = sqlx::query(
        r#"
        SELECT sequence, created_at, entity_type, COALESCE(entity_id::text, '') AS entity_id, event_type, COALESCE(status, '') AS status, payload
        FROM event_stream
        WHERE session_id = $1
        ORDER BY sequence ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let payload: serde_json::Value = row.get("payload");
        println!(
            "{} #{} {} {} {} {} {}",
            row.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
            row.get::<i64, _>("sequence"),
            row.get::<String, _>("entity_type"),
            row.get::<String, _>("entity_id"),
            row.get::<String, _>("event_type"),
            row.get::<String, _>("status"),
            serde_json::to_string(&payload)?
        );
    }
    Ok(())
}
