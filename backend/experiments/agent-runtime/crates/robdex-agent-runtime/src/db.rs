use anyhow::{Result, bail};
use serde::{Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use uuid::Uuid;

use crate::errors::RuntimeDomainError;

use crate::roles::{ImportedRoleVersion, RoleSnapshot, snapshot_from_value, snapshot_to_value};

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
    .execute(pool)
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
    .execute(pool)
    .await?;

    sqlx::query("UPDATE roles SET current_version_id = $2, updated_at = now() WHERE id = $1")
    .bind(&snapshot.id)
    .bind(snapshot.role_version_id)
    .execute(pool)
    .await?;
    append_admin_event(pool, "role", Some(snapshot.role_version_id), "role.imported", Some("active"), json!({"roleId": snapshot.id, "version": snapshot.version, "roleVersionId": snapshot.role_version_id, "actor": actor})).await?;
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

pub async fn activate_role_version(pool: &PgPool, role_id: &str, version_id: Uuid) -> Result<()> {
    let row = sqlx::query("SELECT role_id FROM role_versions WHERE id=$1")
        .bind(version_id)
        .fetch_one(pool)
        .await?;
    let actual: String = row.get("role_id");
    if actual != role_id {
        bail!("role version {version_id} belongs to {actual}, not {role_id}");
    }
    let updated = sqlx::query("UPDATE roles SET current_version_id=$2, status='active', archived_at=NULL, unarchived_at=now(), updated_at=now() WHERE id=$1")
        .bind(role_id)
        .bind(version_id)
        .execute(pool)
        .await?;
    if updated.rows_affected() != 1 {
        bail!("role not found: {role_id}");
    }
    append_admin_event(pool, "role", Some(version_id), "role.activated", Some("active"), json!({"roleId": role_id, "roleVersionId": version_id})).await?;
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
    sqlx::query(
        r#"
        INSERT INTO sessions (id, status, role_id, role_version, role_snapshot, project_key, workdir, worktree_root, title, name, tracked, root_session_id, fork_depth)
        VALUES ($1, 'open', $2, $3, $4, $5, $6, $7, $8, $9, true, $1, 0)
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


#[derive(Debug, Clone, Serialize)]
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
    pub closed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
    pub forked_from_session_id: Option<Uuid>,
    pub forked_from_turn_id: Option<Uuid>,
    pub root_session_id: Option<Uuid>,
    pub fork_depth: i32,
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

pub async fn session_record(pool: &PgPool, session_id: Uuid) -> Result<SessionSummary> {
    let row = sqlx::query(
        r#"
        SELECT id, status, tracked, role_id, role_version, project_key, workdir, worktree_root, title, name, created_at,
               closed_at, archived_at, forked_from_session_id, forked_from_turn_id, root_session_id, fork_depth
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
        closed_at: row.get("closed_at"),
        archived_at: row.get("archived_at"),
        forked_from_session_id: row.get("forked_from_session_id"),
        forked_from_turn_id: row.get("forked_from_turn_id"),
        root_session_id: row.get("root_session_id"),
        fork_depth: row.get("fork_depth"),
    })
}

pub async fn ensure_session_open(pool: &PgPool, session_id: Uuid) -> Result<SessionSummary> {
    let session = session_record(pool, session_id).await?;
    if session.status != "open" {
        return Err(RuntimeDomainError::conflict(format!("session {session_id} is not open: {}", session.status)).into());
    }
    Ok(session)
}

pub async fn list_sessions(pool: &PgPool, include_all: bool) -> Result<Vec<SessionSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT id, status, tracked, role_id, role_version, project_key, workdir, worktree_root, title, name, created_at,
               closed_at, archived_at, forked_from_session_id, forked_from_turn_id, root_session_id, fork_depth
        FROM sessions
        WHERE ($1::bool OR tracked = true)
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
        closed_at: row.get("closed_at"),
        archived_at: row.get("archived_at"),
        forked_from_session_id: row.get("forked_from_session_id"),
        forked_from_turn_id: row.get("forked_from_turn_id"),
        root_session_id: row.get("root_session_id"),
        fork_depth: row.get("fork_depth"),
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
    Ok(json!({
        "session": session,
        "role": {"id": role.id, "version": role.version, "displayName": role.display_name},
        "lifecycle": {"status": session.status, "tracked": session.tracked, "closedAt": session.closed_at, "archivedAt": session.archived_at},
        "pendingApprovals": pending_approvals,
        "pausedActions": paused_actions,
        "managedProcesses": managed_processes,
        "turns": turns,
    }))
}

pub async fn archive_session(pool: &PgPool, session_id: Uuid) -> Result<()> {
    let result = sqlx::query("UPDATE sessions SET tracked = false, archived_at = COALESCE(archived_at, now()), updated_at = now() WHERE id = $1")
        .bind(session_id).execute(pool).await?;
    if result.rows_affected() != 1 { return Err(RuntimeDomainError::not_found("session", session_id).into()); }
    append_event(pool, session_id, None, "session", Some(session_id), "session.archived", Some("archived"), json!({"tracked": false})).await?;
    Ok(())
}

pub async fn close_session(pool: &PgPool, session_id: Uuid, reason: &str, live_terminated: usize) -> Result<()> {
    let session = session_record(pool, session_id).await?;
    if session.status != "open" {
        return Err(RuntimeDomainError::conflict(format!("session close blocked: session missing or not open: {session_id}")).into());
    }
    let running = sqlx::query(
        "SELECT id, handle, end_of_session_behavior FROM managed_processes WHERE session_id = $1 AND status = 'running' ORDER BY start_time ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    let blocked: Vec<Value> = running
        .iter()
        .filter_map(|row| {
            let behavior: String = row.get("end_of_session_behavior");
            if behavior != "terminate" {
                Some(json!({"processId": row.get::<Uuid, _>("id"), "handle": row.get::<String, _>("handle"), "endOfSessionBehavior": behavior, "reason": "policy blocks session close while process is running"}))
            } else {
                None
            }
        })
        .collect();
    let terminable = running.len() - blocked.len();
    if !blocked.is_empty() || live_terminated < terminable {
        let unowned: Vec<Value> = running
            .iter()
            .filter_map(|row| {
                let behavior: String = row.get("end_of_session_behavior");
                if behavior == "terminate" {
                    Some(json!({"processId": row.get::<Uuid, _>("id"), "handle": row.get::<String, _>("handle"), "endOfSessionBehavior": behavior, "reason": "process was not owned and terminated by this runtime close operation"}))
                } else {
                    None
                }
            })
            .collect();
        append_event(pool, session_id, None, "session", Some(session_id), "session.closeBlocked", Some("blocked"), json!({"reason": "running managed processes block session close", "blocked": blocked, "unownedTerminable": unowned, "liveTerminated": live_terminated, "terminableRows": terminable})).await?;
        return Err(RuntimeDomainError::conflict(format!("session close blocked by running managed processes: {session_id}")).into());
    }
    let db_processes = sqlx::query(
        "UPDATE managed_processes SET status = 'sessionClosed', end_time = COALESCE(end_time, now()), termination_reason = 'sessionClosed' WHERE session_id = $1 AND status = 'running' AND end_of_session_behavior = 'terminate'",
    )
    .bind(session_id)
    .execute(pool)
    .await?
    .rows_affected();
    let result = sqlx::query(
        "UPDATE sessions SET status = 'closed', closed_at = COALESCE(closed_at, now()), close_reason = $2, updated_at = now() WHERE id = $1 AND status = 'open'",
    )
    .bind(session_id)
    .bind(reason)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 { return Err(RuntimeDomainError::conflict(format!("session close blocked: session missing or not open: {session_id}")).into()); }
    append_event(pool, session_id, None, "session", Some(session_id), "session.closed", Some("closed"), json!({"reason": reason, "liveProcessesTerminated": live_terminated, "processRowsMarked": db_processes})).await?;
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
        VALUES ($1, 'open', $2, $3, $4, $5, $6, $7, $8, $9, true, $10, $11, $12, $13, $14)
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
    append_event(pool, id, None, "session", Some(id), "session.created", Some("open"), json!({"role": {"id": role_snapshot.id, "version": role_snapshot.version}, "projectKey": source.project_key, "workdir": source.workdir, "worktreeRoot": source.worktree_root, "title": source.title, "name": source.name, "fork": true})).await?;
    append_event(pool, id, Some(fork_turn_id), "session", Some(id), "session.forked", Some("open"), json!({"sourceSessionId": source_session_id, "sourceTurnId": fork_turn_id, "rootSessionId": root, "forkDepth": depth})).await?;
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
