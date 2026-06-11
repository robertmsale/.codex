use anyhow::{Context, Result};
use serde_json::Value;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use uuid::Uuid;

use crate::roles::{RoleSnapshot, snapshot_from_value};

pub async fn connect(database_url: &str) -> Result<PgPool> {
    Ok(PgPoolOptions::new().max_connections(5).connect(database_url).await?)
}

pub async fn init(pool: &PgPool) -> Result<()> {
    sqlx::raw_sql(include_str!("../../../migrations/001_initial.sql"))
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn new_session(pool: &PgPool, role_snapshot: &RoleSnapshot) -> Result<Uuid> {
    let id = Uuid::new_v4();
    let snapshot_value = serde_json::to_value(role_snapshot).context("role snapshot is not serializable")?;
    sqlx::query(
        r#"
        INSERT INTO sessions (id, status, role_id, role_version, role_snapshot)
        VALUES ($1, 'open', $2, $3, $4)
        "#,
    )
        .bind(id)
        .bind(&role_snapshot.id)
        .bind(&role_snapshot.version)
        .bind(&snapshot_value)
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
            }
        }),
    )
    .await?;
    Ok(id)
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
