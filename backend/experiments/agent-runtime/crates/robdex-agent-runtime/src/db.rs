use std::collections::BTreeSet;

use anyhow::{Result, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use uuid::Uuid;

use crate::errors::RuntimeDomainError;

use crate::roles::{ImportedRoleVersion, ManifestDecision, RoleSnapshot, snapshot_from_value, snapshot_to_value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub project_key: String,
    pub display_name: String,
    pub default_workdir: String,
    pub default_worktree_root: String,
    pub default_role_id: Option<String>,
    pub default_model: String,
    pub archived: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

fn validate_project_key(project_key: &str) -> Result<()> {
    let valid = !project_key.is_empty()
        && project_key.len() <= 96
        && project_key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));
    if !valid {
        bail!("project key must use 1-96 ASCII letters, numbers, dash, underscore, or dot");
    }
    Ok(())
}

fn project_row(row: sqlx::postgres::PgRow) -> ProjectRow {
    ProjectRow {
        project_key: row.get("project_key"),
        display_name: row.get("display_name"),
        default_workdir: row.get("default_workdir"),
        default_worktree_root: row.get("default_worktree_root"),
        default_role_id: row.get("default_role_id"),
        default_model: row.get("default_model"),
        archived: row.get("archived"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub async fn connect(database_url: &str) -> Result<PgPool> {
    Ok(PgPoolOptions::new().max_connections(5).connect(database_url).await?)
}

pub async fn apply_schema(pool: &PgPool) -> Result<()> {
    sqlx::raw_sql(include_str!("../../../migrations/001_initial.sql"))
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn init(pool: &PgPool) -> Result<()> {
    apply_schema(pool).await?;
    let _ = reconcile_running_runtime_rows(pool, "runtimeRestart").await?;
    let _ = reconcile_managed_processes(pool, "runtimeRestart").await?;
    ensure_active_turn_constraint(pool).await?;
    crate::command_registry::bootstrap_seed_defaults(pool).await?;
    Ok(())
}

pub async fn list_projects(pool: &PgPool, include_archived: bool) -> Result<Vec<ProjectRow>> {
    let rows = sqlx::query(
        r#"
        SELECT project_key, display_name, default_workdir, default_worktree_root,
               default_role_id, default_model, archived, created_at, updated_at
        FROM projects
        WHERE ($1::bool OR archived = false)
        ORDER BY lower(display_name), project_key
        "#,
    )
    .bind(include_archived)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(project_row).collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn create_project(
    pool: &PgPool,
    project_key: &str,
    display_name: &str,
    default_workdir: &str,
    default_worktree_root: &str,
    default_role_id: Option<&str>,
    default_model: &str,
) -> Result<ProjectRow> {
    validate_project_key(project_key)?;
    if display_name.trim().is_empty()
        || default_workdir.trim().is_empty()
        || default_worktree_root.trim().is_empty()
        || default_model.trim().is_empty()
    {
        bail!("display name, default workdir, default worktree root, and default model are required");
    }
    let row = sqlx::query(
        r#"
        INSERT INTO projects (
            project_key, display_name, default_workdir, default_worktree_root,
            default_role_id, default_model, archived
        )
        VALUES ($1,$2,$3,$4,$5,$6,false)
        RETURNING project_key, display_name, default_workdir, default_worktree_root,
                  default_role_id, default_model, archived, created_at, updated_at
        "#,
    )
    .bind(project_key)
    .bind(display_name.trim())
    .bind(default_workdir.trim())
    .bind(default_worktree_root.trim())
    .bind(default_role_id.filter(|value| !value.trim().is_empty()))
    .bind(default_model.trim())
    .fetch_one(pool)
    .await?;
    append_admin_event(pool, "project", None, "project.created", Some("created"), json!({"projectKey": project_key})).await?;
    Ok(project_row(row))
}

#[allow(clippy::too_many_arguments)]
pub async fn update_project(
    pool: &PgPool,
    project_key: &str,
    display_name: &str,
    default_workdir: &str,
    default_worktree_root: &str,
    default_role_id: Option<&str>,
    default_model: &str,
) -> Result<ProjectRow> {
    validate_project_key(project_key)?;
    if display_name.trim().is_empty()
        || default_workdir.trim().is_empty()
        || default_worktree_root.trim().is_empty()
        || default_model.trim().is_empty()
    {
        bail!("display name, default workdir, default worktree root, and default model are required");
    }
    let row = sqlx::query(
        r#"
        UPDATE projects
        SET display_name=$2, default_workdir=$3, default_worktree_root=$4,
            default_role_id=$5, default_model=$6,
            updated_at=now()
        WHERE project_key=$1
        RETURNING project_key, display_name, default_workdir, default_worktree_root,
                  default_role_id, default_model, archived, created_at, updated_at
        "#,
    )
    .bind(project_key)
    .bind(display_name.trim())
    .bind(default_workdir.trim())
    .bind(default_worktree_root.trim())
    .bind(default_role_id.filter(|value| !value.trim().is_empty()))
    .bind(default_model.trim())
    .fetch_one(pool)
    .await?;
    append_admin_event(pool, "project", None, "project.updated", Some("updated"), json!({"projectKey": project_key})).await?;
    Ok(project_row(row))
}

pub async fn set_project_archived(pool: &PgPool, project_key: &str, archived: bool) -> Result<ProjectRow> {
    validate_project_key(project_key)?;
    let row = sqlx::query(
        r#"
        UPDATE projects
        SET archived=$2, updated_at=now()
        WHERE project_key=$1
        RETURNING project_key, display_name, default_workdir, default_worktree_root,
                  default_role_id, default_model, archived, created_at, updated_at
        "#,
    )
    .bind(project_key)
    .bind(archived)
    .fetch_one(pool)
    .await?;
    append_admin_event(
        pool,
        "project",
        None,
        if archived { "project.archived" } else { "project.unarchived" },
        Some(if archived { "archived" } else { "active" }),
        json!({"projectKey": project_key}),
    )
    .await?;
    Ok(project_row(row))
}

pub async fn ensure_active_turn_constraint(pool: &PgPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS turns_one_running_per_session_idx
            ON turns(session_id)
            WHERE status = 'running'
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessReconciliationSummary {
    pub lost_turns: u64,
    pub lost_tool_calls: u64,
    pub lost_script_runs: u64,
    pub lost_host_api_calls: u64,
    pub lost_command_runs: u64,
    pub lost_processes: u64,
    pub process_events: u64,
    pub session_events: u64,
}

pub async fn reconcile_running_runtime_rows(pool: &PgPool, reason: &str) -> Result<ProcessReconciliationSummary> {
    let mut summary = ProcessReconciliationSummary::default();
    let turns = sqlx::query("UPDATE turns SET status='lost', completed_at=now() WHERE status='running' RETURNING id, session_id")
        .fetch_all(pool)
        .await?;
    for row in turns {
        summary.lost_turns += 1;
        let id: Uuid = row.get("id");
        let session_id: Uuid = row.get("session_id");
        append_event(pool, session_id, Some(id), "turn", Some(id), "turn.lost", Some("lost"), json!({"reason": reason})).await?;
        append_event(pool, session_id, None, "session", Some(session_id), "session.recovered", Some("lost"), json!({"reason": reason, "entity":"turn", "entityId": id})).await?;
        summary.session_events += 1;
    }

    let tools = sqlx::query("UPDATE tool_calls SET status='lost', completed_at=now() WHERE status='running' RETURNING id, session_id, turn_id")
        .fetch_all(pool)
        .await?;
    for row in tools {
        summary.lost_tool_calls += 1;
        let id: Uuid = row.get("id");
        let session_id: Uuid = row.get("session_id");
        let turn_id: Uuid = row.get("turn_id");
        append_event(pool, session_id, Some(turn_id), "tool", Some(id), "tool.lost", Some("lost"), json!({"reason": reason})).await?;
    }

    let scripts = sqlx::query(
        r#"
        UPDATE script_runs sr SET status='lost', completed_at=now()
        FROM tool_calls tc
        WHERE sr.status='running' AND sr.tool_call_id=tc.id
        RETURNING sr.id, tc.session_id, tc.turn_id
        "#,
    )
    .fetch_all(pool)
    .await?;
    for row in scripts {
        summary.lost_script_runs += 1;
        append_event(pool, row.get("session_id"), Some(row.get("turn_id")), "script", Some(row.get("id")), "script.lost", Some("lost"), json!({"reason": reason})).await?;
    }

    let host_calls = sqlx::query(
        r#"
        UPDATE host_api_calls hc SET status='lost', completed_at=now()
        FROM script_runs sr JOIN tool_calls tc ON tc.id=sr.tool_call_id
        WHERE hc.status='running' AND hc.script_run_id=sr.id
        RETURNING hc.id, tc.session_id, tc.turn_id
        "#,
    )
    .fetch_all(pool)
    .await?;
    for row in host_calls {
        summary.lost_host_api_calls += 1;
        append_event(pool, row.get("session_id"), Some(row.get("turn_id")), "host_api", Some(row.get("id")), "host_api.lost", Some("lost"), json!({"reason": reason})).await?;
    }

    let command_runs = sqlx::query(
        r#"
        UPDATE command_runs cr SET status='lost', completed_at=now()
        FROM host_api_calls hc JOIN script_runs sr ON sr.id=hc.script_run_id JOIN tool_calls tc ON tc.id=sr.tool_call_id
        WHERE cr.status='running' AND cr.host_api_call_id=hc.id
        RETURNING cr.id, tc.session_id, tc.turn_id
        "#,
    )
    .fetch_all(pool)
    .await?;
    for row in command_runs {
        summary.lost_command_runs += 1;
        append_event(pool, row.get("session_id"), Some(row.get("turn_id")), "command", Some(row.get("id")), "command.lost", Some("lost"), json!({"reason": reason})).await?;
    }
    Ok(summary)
}

pub async fn reconcile_managed_processes(pool: &PgPool, reason: &str) -> Result<ProcessReconciliationSummary> {
    let rows = sqlx::query(
        r#"
        UPDATE managed_processes
        SET status = 'lost',
            end_time = now(),
            termination_reason = $1
        WHERE status = 'running'
        RETURNING id, session_id, starting_turn_id, handle, command_version_id
        "#,
    )
    .bind(reason)
    .fetch_all(pool)
    .await?;
    let mut summary = ProcessReconciliationSummary::default();
    for row in rows {
        summary.lost_processes += 1;
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
            json!({
                "handle": handle,
                "commandVersionId": command_version_id,
                "reason": reason,
                "explanation": "session-only process is no longer attached to this runtime instance",
            }),
        )
        .await?;
        summary.process_events += 1;
        append_event(
            pool,
            session_id,
            None,
            "session",
            Some(session_id),
            "session.recoveryDegraded",
            Some("degraded"),
            json!({
                "reason": reason,
                "explanation": "reconciliation found a running session-only process with no live runtime owner",
                "processId": process_id,
                "handle": handle,
                "commandVersionId": command_version_id,
            }),
        )
        .await?;
        summary.session_events += 1;
    }
    Ok(summary)
}

pub async fn import_role_version(pool: &PgPool, imported: &ImportedRoleVersion) -> Result<()> {
    import_role_version_with_actor(pool, imported, "seed-import").await
}

pub async fn role_exists(pool: &PgPool, role_id: &str) -> Result<bool> {
    Ok(sqlx::query("SELECT EXISTS (SELECT 1 FROM roles WHERE id=$1)")
        .bind(role_id)
        .fetch_one(pool)
        .await?
        .get::<bool, _>(0))
}

pub async fn import_role_version_with_actor(pool: &PgPool, imported: &ImportedRoleVersion, actor: &str) -> Result<()> {
    let snapshot = &imported.snapshot;
    let snapshot_value = snapshot_to_value(snapshot)?;
    let mut tx = pool.begin().await?;
    if actor == "seed-import" {
        let existing_current: Option<String> = sqlx::query_scalar(
            r#"
            SELECT rv.created_by
            FROM roles r
            JOIN role_versions rv ON rv.id = r.current_version_id
            WHERE r.id=$1
            "#,
        )
        .bind(&snapshot.id)
        .fetch_optional(&mut *tx)
        .await?;
        if existing_current
            .as_deref()
            .is_some_and(|created_by| created_by != "seed-import")
        {
            tx.commit().await?;
            return Ok(());
        }
    }
    sqlx::query(
        r#"
        INSERT INTO roles (id, display_name, current_version_id, status, metadata, created_at, updated_at)
        VALUES ($1, $2, NULL, 'active', '{}'::jsonb, now(), now())
        ON CONFLICT (id) DO UPDATE
        SET display_name = EXCLUDED.display_name,
            status = 'active',
            archived_at = NULL,
            unarchived_at = now(),
            updated_at = now()
        "#,
    )
    .bind(&snapshot.id)
    .bind(&snapshot.display_name)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO role_versions (
            id, role_id, version, display_name, instruction_text, manifest, model_defaults,
            policy, routing, visibility, lifecycle_authority, snapshot, created_at, created_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
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
    .bind(actor)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE roles SET current_version_id = $2, updated_at = now() WHERE id = $1")
    .bind(&snapshot.id)
    .bind(snapshot.role_version_id)
    .execute(&mut *tx)
    .await?;
    let propagated = propagate_role_snapshot_to_non_archived_sessions(&mut tx, &snapshot.id, snapshot, actor).await?;
    sqlx::query(
        "INSERT INTO event_stream (session_id, turn_id, entity_type, entity_id, event_type, status, payload) VALUES (NULL, NULL, $1, $2, $3, $4, $5)",
    )
    .bind("role")
    .bind(snapshot.role_version_id)
    .bind("role.imported")
    .bind("active")
    .bind(json!({"roleId": snapshot.id, "version": snapshot.version, "roleVersionId": snapshot.role_version_id, "actor": actor, "propagatedSessionCount": propagated}))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
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

pub async fn list_role_records(pool: &PgPool) -> Result<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT r.id, r.display_name, r.current_version_id, r.status, r.archived_at, rv.snapshot
        FROM roles r
        LEFT JOIN role_versions rv ON rv.id = r.current_version_id
        ORDER BY r.id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(|row| {
        let snapshot: Option<Value> = row.get("snapshot");
        Ok(json!({
            "id": row.get::<String,_>("id"),
            "displayName": row.get::<String,_>("display_name"),
            "currentVersionId": row.get::<Option<Uuid>,_>("current_version_id"),
            "status": row.get::<String,_>("status"),
            "archivedAt": row.get::<Option<chrono::DateTime<chrono::Utc>>,_>("archived_at"),
            "current": snapshot,
        }))
    }).collect()
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

pub async fn role_version_snapshot(pool: &PgPool, version_id: Uuid) -> Result<RoleSnapshot> {
    let row = sqlx::query("SELECT snapshot FROM role_versions WHERE id=$1")
        .bind(version_id)
        .fetch_one(pool)
        .await?;
    snapshot_from_value(row.get("snapshot"))
}

pub async fn role_versions(pool: &PgPool, role_id: &str) -> Result<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT rv.id, rv.role_id, rv.version, rv.display_name, rv.created_at, rv.created_by, r.current_version_id
        FROM role_versions rv
        JOIN roles r ON r.id = rv.role_id
        WHERE rv.role_id=$1
        ORDER BY rv.created_at ASC
        "#,
    )
    .bind(role_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| {
        let id: Uuid = row.get("id");
        let current: Option<Uuid> = row.get("current_version_id");
        json!({
            "roleVersionId": id,
            "roleId": row.get::<String,_>("role_id"),
            "version": row.get::<String,_>("version"),
            "displayName": row.get::<String,_>("display_name"),
            "createdAt": row.get::<chrono::DateTime<chrono::Utc>,_>("created_at"),
            "createdBy": row.get::<String,_>("created_by"),
            "current": current == Some(id),
        })
    }).collect())
}

fn manifest_decision_label(decision: &ManifestDecision) -> &'static str {
    match decision {
        ManifestDecision::Allow => "allow",
        ManifestDecision::Deny => "deny",
        ManifestDecision::OwnerApproval => "ownerApproval",
        ManifestDecision::OrchestratorApproval => "orchestratorApproval",
    }
}

fn role_policy_hash(snapshot: &RoleSnapshot) -> Result<String> {
    let value = serde_json::to_value(&snapshot.policy)?;
    Ok(format!("{:x}", Sha256::digest(value.to_string().as_bytes())))
}

fn changed_action_summary(previous: &RoleSnapshot, new: &RoleSnapshot) -> Value {
    const LIMIT: usize = 24;
    let previous_keys: BTreeSet<String> = previous.policy.keys().cloned().collect();
    let new_keys: BTreeSet<String> = new.policy.keys().cloned().collect();
    let added: Vec<String> = new_keys.difference(&previous_keys).take(LIMIT).cloned().collect();
    let removed: Vec<String> = previous_keys.difference(&new_keys).take(LIMIT).cloned().collect();
    let changed: Vec<Value> = previous_keys
        .intersection(&new_keys)
        .filter_map(|action| {
            let before = previous.policy.get(action)?;
            let after = new.policy.get(action)?;
            (before != after).then(|| {
                json!({
                    "action": action,
                    "previousDecision": manifest_decision_label(before),
                    "newDecision": manifest_decision_label(after),
                })
            })
        })
        .take(LIMIT)
        .collect();
    json!({
        "addedActions": added,
        "removedActions": removed,
        "changedDecisions": changed,
        "truncated": previous_keys.len().saturating_add(new_keys.len()) > LIMIT * 2,
    })
}

fn changed_capability_summary(previous: &RoleSnapshot, new: &RoleSnapshot) -> Value {
    const LIMIT: usize = 24;
    let previous_keys: BTreeSet<String> = previous.capabilities.iter().cloned().collect();
    let new_keys: BTreeSet<String> = new.capabilities.iter().cloned().collect();
    let added: Vec<String> = new_keys.difference(&previous_keys).take(LIMIT).cloned().collect();
    let removed: Vec<String> = previous_keys.difference(&new_keys).take(LIMIT).cloned().collect();
    json!({
        "addedCapabilities": added,
        "removedCapabilities": removed,
        "truncated": previous_keys.len().saturating_add(new_keys.len()) > LIMIT * 2,
    })
}

async fn latest_session_context_epoch_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_id: Uuid,
) -> Result<Option<i64>> {
    let epoch: Option<i64> = sqlx::query_scalar(
        "SELECT context_epoch FROM session_context_snapshots WHERE session_id=$1 ORDER BY context_epoch DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(epoch)
}

async fn append_session_context_event_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_id: Uuid,
    event_kind: &str,
    role_epoch: &str,
    payload: Value,
) -> Result<i64> {
    let previous_context_epoch = latest_session_context_epoch_tx(tx, session_id).await?;
    let context_epoch = previous_context_epoch.unwrap_or(0) + 1;
    let sequence: i64 = sqlx::query_scalar(
        "INSERT INTO session_context_events (session_id, turn_id, event_kind, role_epoch, context_epoch, previous_context_epoch, payload) VALUES ($1,NULL,$2,$3,$4,$5,$6) RETURNING sequence",
    )
    .bind(session_id)
    .bind(event_kind)
    .bind(role_epoch)
    .bind(context_epoch)
    .bind(previous_context_epoch)
    .bind(payload)
    .fetch_one(&mut **tx)
    .await?;
    Ok(sequence)
}

pub async fn append_session_context_event(
    pool: &PgPool,
    session_id: Uuid,
    event_kind: &str,
    role_snapshot: &RoleSnapshot,
    payload: Value,
) -> Result<i64> {
    let mut tx = pool.begin().await?;
    let role_epoch = format!("{}:{}:{}", role_snapshot.id, role_snapshot.version, role_snapshot.role_version_id);
    let sequence = append_session_context_event_tx(&mut tx, session_id, event_kind, &role_epoch, payload).await?;
    tx.commit().await?;
    Ok(sequence)
}

async fn propagate_role_snapshot_to_non_archived_sessions(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    role_id: &str,
    new_snapshot: &RoleSnapshot,
    actor: &str,
) -> Result<u64> {
    let new_snapshot_value = snapshot_to_value(new_snapshot)?;
    let new_policy_hash = role_policy_hash(new_snapshot)?;
    let rows = sqlx::query(
        r#"
        SELECT id, role_snapshot
        FROM sessions
        WHERE role_id=$1 AND archived_at IS NULL
        ORDER BY created_at ASC, id ASC
        FOR UPDATE
        "#,
    )
    .bind(role_id)
    .fetch_all(&mut **tx)
    .await?;
    let mut affected = 0u64;
    for row in rows {
        let session_id: Uuid = row.get("id");
        let previous_snapshot = snapshot_from_value(row.get::<Value, _>("role_snapshot"))?;
        if previous_snapshot.role_version_id == new_snapshot.role_version_id
            && previous_snapshot.policy == new_snapshot.policy
            && previous_snapshot.capabilities == new_snapshot.capabilities
        {
            continue;
        }
        let previous_policy_hash = role_policy_hash(&previous_snapshot)?;
        let changed = changed_action_summary(&previous_snapshot, new_snapshot);
        let changed_capabilities = changed_capability_summary(&previous_snapshot, new_snapshot);
        let timestamp = Utc::now();
        sqlx::query(
            r#"
            UPDATE sessions
            SET role_version=$2, role_snapshot=$3, updated_at=now()
            WHERE id=$1
            "#,
        )
        .bind(session_id)
        .bind(&new_snapshot.version)
        .bind(&new_snapshot_value)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO event_stream (session_id, turn_id, entity_type, entity_id, event_type, status, payload)
            VALUES ($1, NULL, 'role', $2, 'role_authority.changed', 'updated', $3)
            "#,
        )
        .bind(session_id)
        .bind(new_snapshot.role_version_id)
        .bind(json!({
            "actor": actor,
            "source": actor,
            "timestamp": timestamp,
            "roleId": role_id,
            "previousRoleVersionId": previous_snapshot.role_version_id,
            "newRoleVersionId": new_snapshot.role_version_id,
            "previousRoleVersion": previous_snapshot.version,
            "newRoleVersion": new_snapshot.version,
            "previousPolicyHash": previous_policy_hash,
            "newPolicyHash": new_policy_hash,
            "changedActionSummary": changed.clone(),
            "changedCapabilitySummary": changed_capabilities.clone(),
        }))
        .execute(&mut **tx)
        .await?;
        append_session_context_event_tx(
            tx,
            session_id,
            "role_authority_changed",
            &format!("{}:{}:{}", new_snapshot.id, new_snapshot.version, new_snapshot.role_version_id),
            json!({
                "actor": actor,
                "source": actor,
                "timestamp": timestamp,
                "roleId": role_id,
                "previousRoleVersionId": previous_snapshot.role_version_id,
                "newRoleVersionId": new_snapshot.role_version_id,
                "previousRoleVersion": previous_snapshot.version,
                "newRoleVersion": new_snapshot.version,
                "changedActionSummary": changed,
                "changedCapabilitySummary": changed_capabilities,
            }),
        )
        .await?;
        affected += 1;
    }
    Ok(affected)
}

pub async fn activate_role_version(pool: &PgPool, role_id: &str, version_id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query("SELECT role_id FROM role_versions WHERE id=$1")
        .bind(version_id)
        .fetch_one(&mut *tx)
        .await?;
    let actual: String = row.get("role_id");
    if actual != role_id {
        bail!("role version {version_id} belongs to {actual}, not {role_id}");
    }
    let updated = sqlx::query("UPDATE roles SET current_version_id=$2, status='active', archived_at=NULL, unarchived_at=now(), updated_at=now() WHERE id=$1")
        .bind(role_id)
        .bind(version_id)
        .execute(&mut *tx)
        .await?;
    if updated.rows_affected() != 1 {
        bail!("role not found: {role_id}");
    }
    let snapshot_value: Value = sqlx::query_scalar("SELECT snapshot FROM role_versions WHERE id=$1")
        .bind(version_id)
        .fetch_one(&mut *tx)
        .await?;
    let snapshot = snapshot_from_value(snapshot_value)?;
    let propagated = propagate_role_snapshot_to_non_archived_sessions(&mut tx, role_id, &snapshot, "role-version-activate").await?;
    sqlx::query(
        "INSERT INTO event_stream (session_id, turn_id, entity_type, entity_id, event_type, status, payload) VALUES (NULL, NULL, $1, $2, $3, $4, $5)",
    )
    .bind("role")
    .bind(version_id)
    .bind("role.activated")
    .bind("active")
    .bind(json!({"roleId": role_id, "roleVersionId": version_id, "propagatedSessionCount": propagated}))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn archive_role(pool: &PgPool, role_id: &str) -> Result<()> {
    let current: Option<Uuid> = sqlx::query("SELECT current_version_id FROM roles WHERE id=$1")
        .bind(role_id)
        .fetch_one(pool)
        .await?
        .get("current_version_id");
    let updated = sqlx::query("UPDATE roles SET status='archived', archived_at=now(), updated_at=now() WHERE id=$1")
        .bind(role_id)
        .execute(pool)
        .await?;
    if updated.rows_affected() != 1 {
        bail!("role not found: {role_id}");
    }
    append_admin_event(pool, "role", current, "role.archived", Some("archived"), json!({"roleId": role_id, "currentVersionId": current})).await?;
    Ok(())
}

pub async fn unarchive_role(pool: &PgPool, role_id: &str) -> Result<()> {
    let current: Option<Uuid> = sqlx::query("SELECT current_version_id FROM roles WHERE id=$1")
        .bind(role_id)
        .fetch_one(pool)
        .await?
        .get("current_version_id");
    let updated = sqlx::query("UPDATE roles SET status='active', archived_at=NULL, unarchived_at=now(), updated_at=now() WHERE id=$1 AND current_version_id IS NOT NULL")
        .bind(role_id)
        .execute(pool)
        .await?;
    if updated.rows_affected() != 1 {
        bail!("role not found or has no current version: {role_id}");
    }
    append_admin_event(pool, "role", current, "role.unarchived", Some("active"), json!({"roleId": role_id, "currentVersionId": current})).await?;
    Ok(())
}

pub async fn export_role(pool: &PgPool, role_id: &str) -> Result<Value> {
    let snapshot = current_role_snapshot(pool, role_id).await?;
    append_admin_event(pool, "role", Some(snapshot.role_version_id), "role.exported", Some("success"), json!({"roleId": role_id, "roleVersionId": snapshot.role_version_id})).await?;
    Ok(json!({
        "format": "robdex-agent-runtime-role-export-v1",
        "roleId": snapshot.id,
        "version": snapshot.version,
        "roleVersionId": snapshot.role_version_id,
        "instructionText": snapshot.instruction_text,
        "manifest": snapshot.manifest,
        "modelDefaults": snapshot.model_defaults,
        "policy": snapshot.policy,
        "routing": snapshot.routing,
        "visibility": snapshot.visibility,
        "lifecycleAuthority": snapshot.lifecycle_authority,
    }))
}

pub async fn new_session(pool: &PgPool, role_snapshot: &RoleSnapshot, project_key: Option<&str>, workdir: &str, worktree_root: Option<&str>, title: Option<&str>, name: Option<&str>) -> Result<Uuid> {
    let id = Uuid::new_v4();
    let snapshot_value = snapshot_to_value(role_snapshot)?;
    let active_runtime = if let Some(project_key) = project_key {
        sqlx::query(
            r#"
            SELECT v.id,
                   COALESCE(jsonb_object_agg(h.lifecycle_hook, h.id) FILTER (WHERE h.id IS NOT NULL), '{}'::jsonb) AS bindings
            FROM project_runtime_config_versions v
            LEFT JOIN project_runtime_hook_bindings h ON h.config_version_id=v.id AND h.status='active'
            WHERE v.project_key=$1 AND v.activation_status='active'
            GROUP BY v.id, v.activated_at, v.created_at
            ORDER BY v.activated_at DESC NULLS LAST, v.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(project_key)
        .fetch_optional(pool)
        .await?
        .map(|row| (row.get::<Uuid, _>("id"), row.get::<Value, _>("bindings")))
    } else {
        None
    };
    let active_runtime_version_id = active_runtime.as_ref().map(|(id, _)| *id);
    let active_hook_bindings = active_runtime.as_ref().map(|(_, bindings)| bindings.clone()).unwrap_or_else(|| json!({}));
    let visible_tools = crate::roles::visible_tool_bundle_for_role(pool, &role_snapshot.id, project_key).await?;
    let active_tool_bundle_version_ids = json!({
        "roleId": role_snapshot.id,
        "bundleVersion": "starter-kit-1",
        "projectKey": project_key,
        "tools": visible_tools,
    });
    sqlx::query(
        r#"
        INSERT INTO sessions (id, status, role_id, role_version, role_snapshot, project_key, workdir, worktree_root, title, name, tracked, root_session_id, fork_depth, active_project_runtime_version_id, active_hook_bindings, active_tool_bundle_version_ids)
        VALUES ($1, 'stopped', $2, $3, $4, $5, $6, $7, $8, $9, true, $1, 0, $10, $11, $12)
        "#,
    )
        .bind(id)
        .bind(&role_snapshot.id)
        .bind(&role_snapshot.version)
        .bind(&snapshot_value)
        .bind(project_key)
        .bind(workdir)
        .bind(worktree_root)
        .bind(title)
        .bind(name)
        .bind(active_runtime_version_id)
        .bind(&active_hook_bindings)
        .bind(&active_tool_bundle_version_ids)
        .execute(pool)
        .await?;
    append_event(
        pool,
        id,
        None,
        "session",
        Some(id),
        "session.created",
        Some("stopped"),
        json!({
            "role": {
                "id": role_snapshot.id,
                "version": role_snapshot.version,
                "snapshot": snapshot_value,
            },
            "projectKey": project_key,
            "workdir": workdir,
            "worktreeRoot": worktree_root,
            "title": title,
            "name": name,
            "activeProjectRuntimeVersionId": active_runtime_version_id,
            "activeHookBindings": active_hook_bindings,
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


#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: Uuid,
    pub status: String,
    pub tracked: bool,
    pub role_id: Option<String>,
    pub role_version: Option<String>,
    pub project_key: Option<String>,
    pub workdir: String,
    pub worktree_root: Option<String>,
    pub title: Option<String>,
    pub name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
    pub forked_from_session_id: Option<Uuid>,
    pub forked_from_turn_id: Option<Uuid>,
    pub root_session_id: Option<Uuid>,
    pub fork_depth: i32,
    pub parent_session_id: Option<Uuid>,
    pub session_kind: String,
    pub hidden: bool,
}

fn owner_session_lifecycle_status(status: &str, archived_at: Option<chrono::DateTime<chrono::Utc>>) -> &'static str {
    if archived_at.is_some() {
        "Archived"
    } else if status == "stopped" {
        "Idle"
    } else if status == "running" {
        "Running"
    } else {
        "Open"
    }
}

impl Serialize for SessionSummary {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("SessionSummary", 20)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("status", owner_session_lifecycle_status(&self.status, self.archived_at))?;
        state.serialize_field("executionStatus", &self.status)?;
        state.serialize_field("tracked", &self.tracked)?;
        state.serialize_field("role_id", &self.role_id)?;
        state.serialize_field("role_version", &self.role_version)?;
        state.serialize_field("project_key", &self.project_key)?;
        state.serialize_field("workdir", &self.workdir)?;
        state.serialize_field("worktree_root", &self.worktree_root)?;
        state.serialize_field("title", &self.title)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.serialize_field("archived_at", &self.archived_at)?;
        state.serialize_field("forked_from_session_id", &self.forked_from_session_id)?;
        state.serialize_field("forked_from_turn_id", &self.forked_from_turn_id)?;
        state.serialize_field("root_session_id", &self.root_session_id)?;
        state.serialize_field("fork_depth", &self.fork_depth)?;
        state.serialize_field("parent_session_id", &self.parent_session_id)?;
        state.serialize_field("session_kind", &self.session_kind)?;
        state.serialize_field("hidden", &self.hidden)?;
        state.end()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryItem {
    pub session_id: Uuid,
    pub turn_id: Uuid,
    pub user: String,
    pub assistant: Option<String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub source: String,
    pub checkpoint_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubmittedInputRecord {
    pub id: Uuid,
    pub session_id: Uuid,
    pub target_turn_id: Option<Uuid>,
    pub source_parent_session_id: Option<Uuid>,
    pub actor: String,
    pub source: String,
    pub role: String,
    pub content: String,
    pub payload: Value,
    pub disposition: String,
    pub status: String,
    pub ordering_key: i64,
    pub observed_lifecycle_state: String,
    pub placement_turn_id: Option<Uuid>,
    pub failure_metadata: Value,
}

pub async fn session_record(pool: &PgPool, session_id: Uuid) -> Result<SessionSummary> {
    let row = sqlx::query(
        r#"
        SELECT id, status, tracked, role_id, role_version, project_key, workdir, worktree_root, title, name, created_at,
               archived_at, forked_from_session_id, forked_from_turn_id, root_session_id, fork_depth,
               parent_session_id, session_kind, hidden
        FROM sessions WHERE id = $1
        "#,
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::RowNotFound => RuntimeDomainError::not_found("session", session_id).into(),
        other => anyhow::Error::from(other),
    })?;
    Ok(SessionSummary {
        id: row.get("id"),
        status: row.get("status"),
        tracked: row.get("tracked"),
        role_id: row.get("role_id"),
        role_version: row.get("role_version"),
        project_key: row.get("project_key"),
        workdir: row.get("workdir"),
        worktree_root: row.get("worktree_root"),
        title: row.get("title"),
        name: row.get("name"),
        created_at: row.get("created_at"),
        archived_at: row.get("archived_at"),
        forked_from_session_id: row.get("forked_from_session_id"),
        forked_from_turn_id: row.get("forked_from_turn_id"),
        root_session_id: row.get("root_session_id"),
        fork_depth: row.get("fork_depth"),
        parent_session_id: row.get("parent_session_id"),
        session_kind: row.get("session_kind"),
        hidden: row.get("hidden"),
    })
}

pub async fn ensure_session_not_archived(pool: &PgPool, session_id: Uuid) -> Result<SessionSummary> {
    let session = session_record(pool, session_id).await?;
    if session.archived_at.is_some() || (!session.tracked && session.session_kind != "requirementsReviewer") {
        return Err(RuntimeDomainError::conflict(format!("session {session_id} is archived")).into());
    }
    Ok(session)
}

fn submitted_input_record(row: sqlx::postgres::PgRow) -> SubmittedInputRecord {
    SubmittedInputRecord {
        id: row.get("id"),
        session_id: row.get("session_id"),
        target_turn_id: row.get("target_turn_id"),
        source_parent_session_id: row.get("source_parent_session_id"),
        actor: row.get("actor"),
        source: row.get("source"),
        role: row.get("role"),
        content: row.get("content"),
        payload: row.get("payload"),
        disposition: row.get("disposition"),
        status: row.get("status"),
        ordering_key: row.get("ordering_key"),
        observed_lifecycle_state: row.get("observed_lifecycle_state"),
        placement_turn_id: row.get("placement_turn_id"),
        failure_metadata: row.get("failure_metadata"),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn record_accepted_submitted_input(
    pool: &PgPool,
    session: &SessionSummary,
    target_turn_id: Option<Uuid>,
    actor: &str,
    source: &str,
    role: &str,
    content: &str,
    disposition: &str,
) -> Result<SubmittedInputRecord> {
    let id = Uuid::new_v4();
    let row = sqlx::query(
        r#"
        INSERT INTO submitted_inputs (
            id, session_id, target_turn_id, source_parent_session_id, actor, source, role,
            content, payload, disposition, status, observed_lifecycle_state, accepted_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'accepted',$11,now())
        RETURNING id, session_id, target_turn_id, source_parent_session_id, actor, source, role,
                  content, payload, disposition, status, ordering_key, observed_lifecycle_state,
                  placement_turn_id, failure_metadata
        "#,
    )
    .bind(id)
    .bind(session.id)
    .bind(target_turn_id)
    .bind(session.parent_session_id)
    .bind(actor)
    .bind(source)
    .bind(role)
    .bind(content)
    .bind(json!({"content": content}))
    .bind(disposition)
    .bind(if session.archived_at.is_some() { "archived" } else { session.status.as_str() })
    .fetch_one(pool)
    .await?;
    append_event(
        pool,
        session.id,
        target_turn_id,
        "submitted_input",
        Some(id),
        "submitted_input.accepted",
        Some(disposition),
        json!({"submittedInputId": id, "disposition": disposition, "status": "accepted", "sourceParentSessionId": session.parent_session_id}),
    )
    .await?;
    Ok(submitted_input_record(row))
}

#[allow(clippy::too_many_arguments)]
pub async fn record_accepted_submitted_input_atomic(
    pool: &PgPool,
    session_id: Uuid,
    compaction_active: bool,
    already_active: bool,
    actor: &str,
    source: &str,
    role: &str,
    content: &str,
) -> Result<SubmittedInputRecord> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(session_id.to_string())
        .execute(&mut *tx)
        .await?;
    let session_row = sqlx::query(
        r#"
        SELECT id, status, tracked, parent_session_id, session_kind, archived_at
        FROM sessions
        WHERE id=$1
        FOR UPDATE
        "#,
    )
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| RuntimeDomainError::not_found("session", session_id))?;
    let status: String = session_row.get("status");
    let tracked: bool = session_row.get("tracked");
    let session_kind: String = session_row.get("session_kind");
    let archived_at: Option<chrono::DateTime<chrono::Utc>> = session_row.get("archived_at");
    if archived_at.is_some() || (!tracked && session_kind != "requirementsReviewer") {
        return Err(RuntimeDomainError::conflict(format!("session {session_id} is archived")).into());
    }
    let target_turn_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM turns WHERE session_id=$1 AND status='running' ORDER BY started_at DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?;
    let final_output_committed = if let Some(turn_id) = target_turn_id {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM model_events WHERE session_id=$1 AND turn_id=$2 AND event_type='final_response')",
        )
        .bind(session_id)
        .bind(turn_id)
        .fetch_one(&mut *tx)
        .await?
    } else {
        false
    };
    let disposition = if compaction_active {
        "queued_continuation_after_compaction"
    } else if final_output_committed {
        "queued_next_turn_after_final_output"
    } else if target_turn_id.is_some() {
        "active_turn_steering"
    } else if already_active {
        "queued_next_turn_after_final_output"
    } else {
        "idle_turn_start"
    };
    let id = Uuid::new_v4();
    let row = sqlx::query(
        r#"
        INSERT INTO submitted_inputs (
            id, session_id, target_turn_id, source_parent_session_id, actor, source, role,
            content, payload, disposition, status, observed_lifecycle_state, accepted_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'accepted',$11,now())
        RETURNING id, session_id, target_turn_id, source_parent_session_id, actor, source, role,
                  content, payload, disposition, status, ordering_key, observed_lifecycle_state,
                  placement_turn_id, failure_metadata
        "#,
    )
    .bind(id)
    .bind(session_id)
    .bind(target_turn_id)
    .bind(session_row.get::<Option<Uuid>, _>("parent_session_id"))
    .bind(actor)
    .bind(source)
    .bind(role)
    .bind(content)
    .bind(json!({"content": content}))
    .bind(disposition)
    .bind(status.as_str())
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO event_stream (session_id, turn_id, entity_type, entity_id, event_type, status, payload)
        VALUES ($1,$2,'submitted_input',$3,'submitted_input.accepted',$4,$5)
        "#,
    )
    .bind(session_id)
    .bind(target_turn_id)
    .bind(id)
    .bind(disposition)
    .bind(json!({"submittedInputId": id, "disposition": disposition, "status": "accepted", "sourceParentSessionId": session_row.get::<Option<Uuid>, _>("parent_session_id")}))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(submitted_input_record(row))
}

pub async fn record_rejected_submitted_input(
    pool: &PgPool,
    session_id: Uuid,
    actor: &str,
    source: &str,
    role: &str,
    content: &str,
    observed_lifecycle_state: &str,
    reason: &str,
) -> Result<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO submitted_inputs (
            id, session_id, actor, source, role, content, payload, disposition, status,
            observed_lifecycle_state, failure_metadata, rejected_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,'rejected_terminal','rejected',$8,$9,now())
        "#,
    )
    .bind(id)
    .bind(session_id)
    .bind(actor)
    .bind(source)
    .bind(role)
    .bind(content)
    .bind(json!({"content": content}))
    .bind(observed_lifecycle_state)
    .bind(json!({"reason": reason}))
    .execute(pool)
    .await?;
    append_event(
        pool,
        session_id,
        None,
        "submitted_input",
        Some(id),
        "submitted_input.rejected",
        Some("rejected"),
        json!({"submittedInputId": id, "reason": reason, "observedLifecycleState": observed_lifecycle_state}),
    )
    .await?;
    Ok(id)
}

pub async fn next_accepted_submitted_input(pool: &PgPool, session_id: Uuid) -> Result<Option<SubmittedInputRecord>> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, target_turn_id, source_parent_session_id, actor, source, role,
               content, payload, disposition, status, ordering_key, observed_lifecycle_state,
               placement_turn_id, failure_metadata
        FROM submitted_inputs
        WHERE session_id=$1 AND status='accepted'
        ORDER BY ordering_key ASC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(submitted_input_record))
}

pub async fn next_accepted_submitted_input_for_turn(pool: &PgPool, session_id: Uuid, turn_id: Uuid) -> Result<Option<SubmittedInputRecord>> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, target_turn_id, source_parent_session_id, actor, source, role,
               content, payload, disposition, status, ordering_key, observed_lifecycle_state,
               placement_turn_id, failure_metadata
        FROM submitted_inputs
        WHERE session_id=$1 AND target_turn_id=$2 AND status='accepted'
        ORDER BY ordering_key ASC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .bind(turn_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(submitted_input_record))
}

pub async fn sessions_with_accepted_submitted_inputs(pool: &PgPool) -> Result<Vec<Uuid>> {
    let rows = sqlx::query_scalar(
        r#"
        SELECT DISTINCT si.session_id
        FROM submitted_inputs si
        JOIN sessions s ON s.id = si.session_id
        WHERE si.status='accepted'
          AND s.archived_at IS NULL
          AND (s.tracked=true OR s.session_kind='requirementsReviewer')
        ORDER BY si.session_id
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn submitted_input_counts(pool: &PgPool, session_id: Uuid) -> Result<(i64, i64, Option<String>, Option<String>)> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status='accepted') AS queued,
            COUNT(*) FILTER (WHERE status='applied') AS applied,
            (array_agg(disposition ORDER BY ordering_key DESC) FILTER (WHERE status IN ('accepted','applied','failed','rejected')))[1] AS disposition,
            (array_agg(status ORDER BY ordering_key DESC) FILTER (WHERE status IN ('accepted','applied','failed','rejected')))[1] AS status
        FROM submitted_inputs
        WHERE session_id=$1
        "#,
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    Ok((
        row.get::<i64, _>("queued"),
        row.get::<i64, _>("applied"),
        row.get::<Option<String>, _>("disposition"),
        row.get::<Option<String>, _>("status"),
    ))
}

pub async fn latest_rejected_submitted_input(pool: &PgPool, session_id: Uuid) -> Result<Option<Value>> {
    let row = sqlx::query(
        r#"
        SELECT id, failure_metadata, observed_lifecycle_state, rejected_at
        FROM submitted_inputs
        WHERE session_id=$1 AND status='rejected'
        ORDER BY ordering_key DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| {
        json!({
            "submittedInputId": row.get::<Uuid, _>("id"),
            "observedLifecycleState": row.get::<String, _>("observed_lifecycle_state"),
            "failureMetadata": row.get::<Value, _>("failure_metadata"),
            "rejectedAt": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("rejected_at").map(|time| time.to_rfc3339()),
        })
    }))
}

pub async fn active_turn_id(pool: &PgPool, session_id: Uuid) -> Result<Option<Uuid>> {
    Ok(sqlx::query_scalar("SELECT id FROM turns WHERE session_id=$1 AND status='running' ORDER BY started_at DESC LIMIT 1")
        .bind(session_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn mark_submitted_input_applied(pool: &PgPool, id: Uuid, placement_turn_id: Uuid) -> Result<()> {
    sqlx::query("UPDATE submitted_inputs SET status='applied', placement_turn_id=$2, applied_at=now(), updated_at=now() WHERE id=$1")
        .bind(id)
        .bind(placement_turn_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_submitted_input_failed(pool: &PgPool, id: Uuid, reason: &str) -> Result<()> {
    sqlx::query("UPDATE submitted_inputs SET status='failed', failure_metadata=$2, updated_at=now() WHERE id=$1")
        .bind(id)
        .bind(json!({"reason": reason}))
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn abandon_unapplied_submitted_inputs(pool: &PgPool, session_id: Uuid, reason: &str) -> Result<u64> {
    let result = sqlx::query(
        "UPDATE submitted_inputs SET status='abandoned', failure_metadata=$2, abandoned_at=now(), updated_at=now() WHERE session_id=$1 AND status='accepted'",
    )
    .bind(session_id)
    .bind(json!({"reason": reason}))
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn list_sessions(pool: &PgPool, include_all: bool) -> Result<Vec<SessionSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT id, status, tracked, role_id, role_version, project_key, workdir, worktree_root, title, name, created_at,
               archived_at, forked_from_session_id, forked_from_turn_id, root_session_id, fork_depth,
               parent_session_id, session_kind, hidden
        FROM sessions
        WHERE ($1::bool OR tracked = true) AND hidden = false
        ORDER BY updated_at DESC, created_at DESC
        "#,
    )
    .bind(include_all)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| SessionSummary {
        id: row.get("id"),
        status: row.get("status"),
        tracked: row.get("tracked"),
        role_id: row.get("role_id"),
        role_version: row.get("role_version"),
        project_key: row.get("project_key"),
        workdir: row.get("workdir"),
        worktree_root: row.get("worktree_root"),
        title: row.get("title"),
        name: row.get("name"),
        created_at: row.get("created_at"),
        archived_at: row.get("archived_at"),
        forked_from_session_id: row.get("forked_from_session_id"),
        forked_from_turn_id: row.get("forked_from_turn_id"),
        root_session_id: row.get("root_session_id"),
        fork_depth: row.get("fork_depth"),
        parent_session_id: row.get("parent_session_id"),
        session_kind: row.get("session_kind"),
        hidden: row.get("hidden"),
    }).collect())
}

pub async fn show_session(pool: &PgPool, session_id: Uuid) -> Result<Value> {
    let session = session_record(pool, session_id).await?;
    let role = session_role_snapshot(pool, session_id).await?;
    let pending_approvals: i64 = sqlx::query("SELECT COUNT(*) AS count FROM approval_requests WHERE session_id = $1 AND status = 'pending'")
        .bind(session_id).fetch_one(pool).await?.get("count");
    let paused_actions: i64 = sqlx::query("SELECT COUNT(*) AS count FROM paused_actions WHERE session_id = $1 AND status IN ('pendingApproval', 'approved', 'resuming')")
        .bind(session_id).fetch_one(pool).await?.get("count");
    let managed_processes: i64 = sqlx::query("SELECT COUNT(*) AS count FROM managed_processes WHERE session_id = $1")
        .bind(session_id).fetch_one(pool).await?.get("count");
    let turns: i64 = sqlx::query("SELECT COUNT(*) AS count FROM turns WHERE session_id = $1")
        .bind(session_id).fetch_one(pool).await?.get("count");
    let runtime_attribution: (Option<Uuid>, Value) = sqlx::query_as("SELECT active_project_runtime_version_id, active_hook_bindings FROM sessions WHERE id=$1")
        .bind(session_id)
        .fetch_one(pool)
        .await?;
    let active_contracts: i64 = sqlx::query("SELECT COUNT(*) AS count FROM generic_contracts WHERE session_id=$1 AND status='active'")
        .bind(session_id).fetch_one(pool).await?.get("count");
    let resource_leases: i64 = sqlx::query("SELECT COUNT(*) AS count FROM resource_leases WHERE owning_session_id=$1 AND status IN ('reserved','assigned')")
        .bind(session_id).fetch_one(pool).await?.get("count");
    let recent_hook_failures: i64 = sqlx::query("SELECT COUNT(*) AS count FROM hook_evaluations WHERE session_id=$1 AND validation_status='invalid' AND created_at > now() - interval '1 day'")
        .bind(session_id).fetch_one(pool).await?.get("count");
    let subagent_projection = crate::lifecycle_hooks::parent_subagent_projection(pool, session_id).await?;
    Ok(json!({
        "session": session,
        "role": {"id": role.id, "version": role.version, "displayName": role.display_name},
        "lifecycle": {"status": owner_session_lifecycle_status(&session.status, session.archived_at), "executionStatus": session.status, "tracked": session.tracked, "archivedAt": session.archived_at},
        "projectRuntime": {"activeVersionId": runtime_attribution.0, "activeHookBindings": runtime_attribution.1},
        "workflow": {"activeContracts": active_contracts, "activeResourceLeases": resource_leases, "recentHookFailures": recent_hook_failures},
        "subagents": subagent_projection,
        "pendingApprovals": pending_approvals,
        "pausedActions": paused_actions,
        "managedProcesses": managed_processes,
        "turns": turns,
    }))
}

pub async fn archive_session(pool: &PgPool, session_id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(session_id.to_string())
        .execute(&mut *tx)
        .await?;
    let result = sqlx::query("UPDATE sessions SET tracked = false, archived_at = COALESCE(archived_at, now()), updated_at = now() WHERE id = $1")
        .bind(session_id).execute(&mut *tx).await?;
    if result.rows_affected() != 1 { return Err(RuntimeDomainError::not_found("session", session_id).into()); }
    let abandoned = sqlx::query(
        "UPDATE submitted_inputs SET status='abandoned', failure_metadata=$2, abandoned_at=now(), updated_at=now() WHERE session_id=$1 AND status='accepted'",
    )
    .bind(session_id)
    .bind(json!({"reason": "session archived"}))
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let starter_servers = sqlx::query(
        "UPDATE starter_managed_servers SET status='archived', updated_at=now() WHERE session_id=$1 AND status='running'",
    )
    .bind(session_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let starter_ports = sqlx::query(
        "UPDATE starter_port_leases SET status='released', released_at=COALESCE(released_at, now()), release_reason='session.archive' WHERE session_id=$1 AND status='active'",
    )
    .bind(session_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    let role_snapshot = session_role_snapshot(pool, session_id).await?;
    append_session_context_event(
        pool,
        session_id,
        "session_lifecycle_changed",
        &role_snapshot,
        json!({
            "source": "session.archive",
            "actor": "runtime",
            "timestamp": Utc::now(),
            "previousLifecycle": "open",
            "newLifecycle": "archived",
        }),
    )
    .await?;
    let lifecycle_cleanup = crate::lifecycle_hooks::cleanup_session_lifecycle_resources(pool, session_id, "session archived").await?;
    crate::god_mode::revoke_active(pool, session_id, "runtime", "session archived").await?;
    crate::requirements::deactivate_nested_reviewers(pool, session_id).await?;
    append_event(pool, session_id, None, "session", Some(session_id), "session.archived", Some("archived"), json!({"tracked": false, "lifecycleCleanup": lifecycle_cleanup, "starterServersReleased": starter_servers, "starterPortLeasesReleased": starter_ports})).await?;
    if abandoned > 0 {
        append_event(pool, session_id, None, "submitted_input", None, "submitted_input.abandoned", Some("abandoned"), json!({"count": abandoned, "reason": "session archived"})).await?;
    }
    Ok(())
}

pub async fn fork_session(pool: &PgPool, source_session_id: Uuid, fork_turn_id: Uuid) -> Result<Uuid> {
    let source = session_record(pool, source_session_id).await?;
    let turn_row = sqlx::query("SELECT status FROM turns WHERE id = $1 AND session_id = $2")
        .bind(fork_turn_id)
        .bind(source_session_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| RuntimeDomainError::not_found("turn", fork_turn_id))?;
    let status: String = turn_row.get("status");
    if status != "completed" { return Err(RuntimeDomainError::conflict(format!("fork source turn must be completed: {fork_turn_id} status={status}")).into()); }
    let role_snapshot = session_role_snapshot(pool, source_session_id).await?;
    let snapshot_value = snapshot_to_value(&role_snapshot)?;
    let id = Uuid::new_v4();
    let depth = source.fork_depth + 1;
    let root = source.root_session_id.unwrap_or(source.id);
    sqlx::query(
        r#"
        INSERT INTO sessions (
            id, status, role_id, role_version, role_snapshot, project_key, workdir, worktree_root, title, name, tracked,
            forked_from_session_id, forked_from_turn_id, root_session_id, fork_depth, lineage
        )
        VALUES ($1, 'stopped', $2, $3, $4, $5, $6, $7, $8, $9, true, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(id)
    .bind(&role_snapshot.id)
    .bind(&role_snapshot.version)
    .bind(&snapshot_value)
    .bind(source.project_key.as_deref())
    .bind(&source.workdir)
    .bind(source.worktree_root.as_deref())
    .bind(source.title.as_deref())
    .bind(source.name.as_deref())
    .bind(source_session_id)
    .bind(fork_turn_id)
    .bind(root)
    .bind(depth)
    .bind(json!({"forkedFromSessionId": source_session_id, "forkedFromTurnId": fork_turn_id, "rootSessionId": root, "forkDepth": depth}))
    .execute(pool)
    .await?;
    append_event(pool, id, None, "session", Some(id), "session.created", Some("stopped"), json!({"role": {"id": role_snapshot.id, "version": role_snapshot.version}, "projectKey": source.project_key, "workdir": source.workdir, "worktreeRoot": source.worktree_root, "title": source.title, "name": source.name, "fork": true})).await?;
    append_event(pool, id, Some(fork_turn_id), "session", Some(id), "session.forked", Some("stopped"), json!({"sourceSessionId": source_session_id, "sourceTurnId": fork_turn_id, "rootSessionId": root, "forkDepth": depth})).await?;
    Ok(id)
}

pub async fn reconstructed_history(pool: &PgPool, session_id: Uuid) -> Result<Vec<HistoryItem>> {
    crate::compaction::reconstructed_history_after_checkpoint(pool, session_id).await
}

pub async fn history_json(pool: &PgPool, session_id: Uuid) -> Result<Value> {
    let session = session_record(pool, session_id).await?;
    let history = reconstructed_history(pool, session_id).await?;
    Ok(json!({"session": session, "history": history}))
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
    sqlx::query("UPDATE sessions SET updated_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn append_admin_event(
    pool: &PgPool,
    entity_type: &str,
    entity_id: Option<Uuid>,
    event_type: &str,
    status: Option<&str>,
    payload: Value,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO event_stream (session_id, turn_id, entity_type, entity_id, event_type, status, payload)
        VALUES (NULL, NULL, $1, $2, $3, $4, $5)
        "#,
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(event_type)
    .bind(status)
    .bind(payload)
    .execute(pool)
    .await?;
    Ok(())
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
