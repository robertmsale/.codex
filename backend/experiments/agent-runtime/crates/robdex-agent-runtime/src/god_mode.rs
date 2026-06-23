use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::db;
use crate::errors::RuntimeDomainError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GodModeGrant {
    pub id: Uuid,
    pub session_id: Uuid,
    pub granted_by: String,
    pub granted_by_kind: String,
    pub reason: String,
    pub status: String,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by: Option<String>,
    pub revoked_reason: Option<String>,
}

fn row_to_grant(row: sqlx::postgres::PgRow) -> GodModeGrant {
    GodModeGrant {
        id: row.get("id"),
        session_id: row.get("session_id"),
        granted_by: row.get("granted_by"),
        granted_by_kind: row.get("granted_by_kind"),
        reason: row.get("reason"),
        status: row.get("status"),
        granted_at: row.get("granted_at"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
        revoked_by: row.get("revoked_by"),
        revoked_reason: row.get("revoked_reason"),
    }
}

pub async fn active_grant(pool: &PgPool, session_id: Uuid) -> Result<Option<GodModeGrant>> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, granted_by, granted_by_kind, reason, status, granted_at, expires_at, revoked_at, revoked_by, revoked_reason
        FROM god_mode_grants
        WHERE session_id = $1
          AND status = 'active'
          AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > now())
        ORDER BY granted_at DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_grant))
}

pub async fn require_active_grant(pool: &PgPool, session_id: Uuid) -> Result<GodModeGrant> {
    active_grant(pool, session_id)
        .await?
        .ok_or_else(|| RuntimeDomainError::conflict("God Mode required: shell(...) disabled").into())
}

pub async fn grant_session(pool: &PgPool, session_id: Uuid, actor: &str, reason: &str, expires_at: Option<DateTime<Utc>>) -> Result<GodModeGrant> {
    let session = db::session_record(pool, session_id).await?;
    if session.status != "open" || session.archived_at.is_some() {
        return Err(RuntimeDomainError::conflict(format!("God Mode grant blocked: session is not open: {session_id}")).into());
    }
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(RuntimeDomainError::bad_request("God Mode grant requires a reason").into());
    }
    revoke_active(pool, session_id, actor, "superseded by a new God Mode grant").await?;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO god_mode_grants (id, session_id, granted_by, granted_by_kind, reason, expires_at, status)
        VALUES ($1, $2, $3, 'operator', $4, $5, 'active')
        "#,
    )
    .bind(id)
    .bind(session_id)
    .bind(actor)
    .bind(reason)
    .bind(expires_at)
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        None,
        "session",
        Some(session_id),
        "godMode.granted",
        Some("active"),
        json!({"grantId": id, "grantedBy": actor, "reason": reason, "expiresAt": expires_at}),
    )
    .await?;
    Ok(active_grant(pool, session_id).await?.expect("inserted grant is active"))
}

pub async fn revoke_active(pool: &PgPool, session_id: Uuid, actor: &str, reason: &str) -> Result<u64> {
    let reason = if reason.trim().is_empty() { "revoked" } else { reason.trim() };
    let result = sqlx::query(
        r#"
        UPDATE god_mode_grants
        SET status = 'revoked',
            revoked_at = COALESCE(revoked_at, now()),
            revoked_by = $2,
            revoked_reason = $3
        WHERE session_id = $1
          AND status = 'active'
          AND revoked_at IS NULL
        "#,
    )
    .bind(session_id)
    .bind(actor)
    .bind(reason)
    .execute(pool)
    .await?;
    if result.rows_affected() > 0 {
        db::append_event(
            pool,
            session_id,
            None,
            "session",
            Some(session_id),
            "godMode.revoked",
            Some("revoked"),
            json!({"revokedBy": actor, "reason": reason, "count": result.rows_affected()}),
        )
        .await?;
    }
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::{
        LifecycleAuthorityMetadata, ManifestDecision, ModelDefaults, RoleSnapshot, RoutingMetadata,
        VisibilityMetadata,
    };
    use sqlx::postgres::PgPoolOptions;
    use std::collections::BTreeMap;

    fn database_url() -> String {
        std::env::var("ROBDEX_AGENT_RUNTIME_TEST_DATABASE_URL")
            .or_else(|_| std::env::var("ROBDEX_AGENT_RUNTIME_DATABASE_URL"))
            .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime".to_string())
    }

    async fn test_pool() -> PgPool {
        let pool = PgPoolOptions::new().max_connections(5).connect(&database_url()).await.expect("test database must be available");
        crate::db::init(&pool).await.expect("test database initializes");
        pool
    }

    fn test_role() -> RoleSnapshot {
        RoleSnapshot {
            id: "runtime-allow".to_string(),
            version: "1.0.0".to_string(),
            display_name: "Runtime Allow".to_string(),
            role_version_id: Uuid::new_v4(),
            instruction_text: "test role".to_string(),
            model_defaults: ModelDefaults {
                model: "gpt-5-mini".to_string(),
                reasoning_effort: "medium".to_string(),
            },
            capabilities: vec!["tool.execute_code".to_string()],
            policy: BTreeMap::from([("tool.execute_code".to_string(), ManifestDecision::Allow)]),
            routing: RoutingMetadata {
                mode: "direct".to_string(),
                default_recipient: Some("assistant".to_string()),
                allowed_recipients: vec!["assistant".to_string()],
                reserved_actions: vec![],
            },
            visibility: VisibilityMetadata {
                listed: true,
                owner_visible: true,
            },
            lifecycle_authority: LifecycleAuthorityMetadata {
                can_spawn_agents: true,
                can_archive_agents: true,
                reserved_actions: vec![],
            },
            manifest: json!({"roleId":"runtime-allow","version":"1.0.0"}),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn active_god_mode_grant_survives_new_pool_and_close_archive_revoke() {
        let pool = test_pool().await;
        let role = test_role();
        let session = crate::db::new_session(&pool, &role, None, ".", None, None, None).await.expect("session");
        let grant = grant_session(&pool, session, "test-operator", "restart persistence test", None).await.expect("grant");
        assert_eq!(grant.session_id, session);
        let restarted_pool = PgPoolOptions::new().max_connections(5).connect(&database_url()).await.expect("new pool");
        let after_restart = active_grant(&restarted_pool, session).await.expect("active lookup").expect("active grant");
        assert_eq!(after_restart.id, grant.id);
        crate::db::close_session(&restarted_pool, session, "test close", 0).await.expect("close revokes");
        assert!(active_grant(&restarted_pool, session).await.expect("active lookup after close").is_none());

        let archived_session = crate::db::new_session(&restarted_pool, &role, None, ".", None, None, None).await.expect("archive session");
        grant_session(&restarted_pool, archived_session, "test-operator", "archive revoke test", None).await.expect("grant archive");
        crate::db::archive_session(&restarted_pool, archived_session).await.expect("archive revokes");
        assert!(active_grant(&restarted_pool, archived_session).await.expect("active lookup after archive").is_none());
    }
}
