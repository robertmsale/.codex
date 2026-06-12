use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{db, starlark_host};
use crate::policy::PolicyResult;
use crate::roles::RoleSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApproverKind {
    Owner,
    Orchestrator,
}

impl ApproverKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Orchestrator => "orchestrator",
        }
    }
}

impl TryFrom<&str> for ApproverKind {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> Result<Self> {
        match value {
            "owner" => Ok(Self::Owner),
            "orchestrator" => Ok(Self::Orchestrator),
            other => bail!("unsupported approver kind: {other}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    Denied,
}

impl ApprovalDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Denied => "denied",
        }
    }
}

impl TryFrom<&str> for ApprovalDecision {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> Result<Self> {
        match value {
            "approved" => Ok(Self::Approved),
            "denied" => Ok(Self::Denied),
            other => bail!("approval decision must be approved or denied, got: {other}"),
        }
    }
}

pub async fn request_approval(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    policy: &PolicyResult,
    role: &RoleSnapshot,
) -> Result<Uuid> {
    let required = policy.required_approver_kind.ok_or_else(|| anyhow::anyhow!("approval policy missing required approver kind"))?;
    let id = Uuid::new_v4();
    let role_identity = serde_json::json!({
        "id": role.id,
        "version": role.version,
        "roleVersionId": role.role_version_id,
    });
    sqlx::query(
        r#"
        INSERT INTO approval_requests (
            id, session_id, turn_id, action_name, requested_by_role, input_context,
            required_approver_kind, status, created_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', $8)
        "#,
    )
    .bind(id)
    .bind(session_id)
    .bind(turn_id)
    .bind(&policy.action)
    .bind(&role_identity)
    .bind(policy.to_event_payload())
    .bind(required.as_str())
    .bind(Utc::now())
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        turn_id,
        "approval",
        Some(id),
        "approval.requested",
        Some("pending"),
        serde_json::json!({
            "requestId": id,
            "action": policy.action,
            "requiredApproverKind": required.as_str(),
            "policy": policy.to_event_payload(),
        }),
    )
    .await?;
    Ok(id)
}

pub async fn create_paused_action(
    pool: &PgPool,
    approval_request_id: Uuid,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    tool_call_id: Option<Uuid>,
    script_run_id: Option<Uuid>,
    action_name: &str,
    action_input: Value,
    role: &RoleSnapshot,
) -> Result<Uuid> {
    let id = Uuid::new_v4();
    let role_snapshot = serde_json::json!({
        "id": role.id,
        "version": role.version,
        "roleVersionId": role.role_version_id,
        "snapshot": role,
    });
    sqlx::query(
        r#"
        INSERT INTO paused_actions (
            id, approval_request_id, session_id, turn_id, tool_call_id, script_run_id,
            action_name, action_input, role_snapshot, status, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pendingApproval', $10, $10)
        "#,
    )
    .bind(id)
    .bind(approval_request_id)
    .bind(session_id)
    .bind(turn_id)
    .bind(tool_call_id)
    .bind(script_run_id)
    .bind(action_name)
    .bind(&action_input)
    .bind(&role_snapshot)
    .bind(Utc::now())
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn list(pool: &PgPool) -> Result<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, turn_id, action_name, requested_by_role, input_context,
               required_approver_kind, status, created_at, completed_at
        FROM approval_requests
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_json).collect())
}

pub async fn show(pool: &PgPool, id: Uuid) -> Result<Value> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, turn_id, action_name, requested_by_role, input_context,
               required_approver_kind, status, created_at, completed_at
        FROM approval_requests
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    let mut value = row_to_json(row);
    let decisions = sqlx::query(
        r#"
        SELECT id, decision, reason, decided_by, created_at
        FROM approval_decisions
        WHERE request_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    value["decisions"] = Value::Array(decisions.into_iter().map(decision_row_to_json).collect());
    let paused = sqlx::query(
        r#"
        SELECT id, approval_request_id, session_id, turn_id, tool_call_id, script_run_id,
               action_name, action_input, role_snapshot, status, result, error,
               created_at, updated_at, completed_at
        FROM paused_actions
        WHERE approval_request_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    value["pausedActions"] = Value::Array(paused.into_iter().map(paused_row_to_json).collect());
    Ok(value)
}

pub async fn decide(pool: &PgPool, id: Uuid, decision: ApprovalDecision, reason: &str) -> Result<()> {
    let request = sqlx::query("SELECT session_id, turn_id, status FROM approval_requests WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let status: String = request.get("status");
    validate_decide_status(&status).map_err(|error| anyhow::anyhow!("{error}: {id} status={status}"))?;
    let decision_id = Uuid::new_v4();
    let decided_by = serde_json::json!({"kind": "operator-placeholder", "principal": "local-cli"});
    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO approval_decisions (id, request_id, decision, reason, decided_by, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(decision_id)
    .bind(id)
    .bind(decision.as_str())
    .bind(reason)
    .bind(&decided_by)
    .bind(now)
    .execute(pool)
    .await?;
    let updated = sqlx::query("UPDATE approval_requests SET status = $2, completed_at = $3 WHERE id = $1 AND status = 'pending'")
        .bind(id)
        .bind(decision.as_str())
        .bind(now)
        .execute(pool)
        .await?;
    if updated.rows_affected() != 1 {
        bail!("approval request terminal update failed for {id}: expected one pending row, updated {}", updated.rows_affected());
    }
    let session_id: Uuid = request.get("session_id");
    let turn_id: Option<Uuid> = request.get("turn_id");
    db::append_event(
        pool,
        session_id,
        turn_id,
        "approval",
        Some(id),
        "approval.decided",
        Some(decision.as_str()),
        serde_json::json!({
            "requestId": id,
            "decisionId": decision_id,
            "decision": decision.as_str(),
            "reason": reason,
            "decidedBy": decided_by,
        }),
    )
    .await?;
    Ok(())
}

pub async fn resume(pool: &PgPool, approval_id: Uuid) -> Result<()> {
    let request = sqlx::query(
        "SELECT id, session_id, turn_id, status FROM approval_requests WHERE id = $1",
    )
    .bind(approval_id)
    .fetch_one(pool)
    .await?;
    let request_status: String = request.get("status");
    validate_resume_request_status(&request_status)
        .map_err(|error| anyhow::anyhow!("{error}: {approval_id} status={request_status}"))?;
    let paused = sqlx::query(
        r#"
        SELECT id, session_id, turn_id, tool_call_id, script_run_id, action_name, action_input, role_snapshot, status
        FROM paused_actions
        WHERE approval_request_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(approval_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("approval request has no linked paused action: {approval_id}"))?;
    let paused_id: Uuid = paused.get("id");
    let paused_status: String = paused.get("status");
    validate_resume_paused_status(&paused_status)
        .map_err(|error| anyhow::anyhow!("{error}: {paused_id} status={paused_status}"))?;
    let action_name: String = paused.get("action_name");
    if !matches!(
        action_name.as_str(),
        "cmd.rg.run" | "fs.write" | "patch.apply" | "cmd.git.status" | "cmd.git.diff" | "cmd.cargo.check"
    ) {
        bail!("resume does not support action in this phase: {action_name}");
    }
    let session_id: Uuid = paused.get("session_id");
    let turn_id: Option<Uuid> = paused.get("turn_id");
    let script_run_id: Option<Uuid> = paused.get("script_run_id");
    let script_run_id = script_run_id.ok_or_else(|| anyhow::anyhow!("paused action missing script_run_id"))?;
    let action_input: Value = paused.get("action_input");
    let role_snapshot: Value = paused.get("role_snapshot");
    db::append_event(
        pool,
        session_id,
        turn_id,
        "approval",
        Some(approval_id),
        "approval.resume.started",
        Some("resuming"),
        serde_json::json!({"approvalRequestId": approval_id, "pausedActionId": paused_id, "action": action_name}),
    )
    .await?;
    let updated = sqlx::query(
        "UPDATE paused_actions SET status = 'resuming', updated_at = $2 WHERE id = $1 AND status IN ('pendingApproval', 'approved')",
    )
    .bind(paused_id)
    .bind(Utc::now())
    .execute(pool)
    .await?;
    if updated.rows_affected() != 1 {
        bail!("paused action resume transition failed for {paused_id}: expected one ready row, updated {}", updated.rows_affected());
    }
    let policy_decision = serde_json::json!({
        "action": action_name,
        "decision": "allow",
        "reason": "approved approval request authorizes stored paused action",
        "approvalRequestId": approval_id,
        "pausedActionId": paused_id,
        "role": role_snapshot.get("id").cloned().unwrap_or(Value::Null),
    });
    db::append_event(
        pool,
        session_id,
        turn_id,
        "policy",
        None,
        "policy.resumeDecision",
        Some("allow"),
        policy_decision.clone(),
    )
    .await?;
    match starlark_host::execute_resumed_action(pool, session_id, turn_id, script_run_id, &action_name, &action_input, policy_decision).await {
        Ok(result) => {
            sqlx::query(
                "UPDATE paused_actions SET status = 'completed', result = $2, updated_at = $3, completed_at = $3 WHERE id = $1 AND status = 'resuming'",
            )
            .bind(paused_id)
            .bind(&result)
            .bind(Utc::now())
            .execute(pool)
            .await?;
            db::append_event(
                pool,
                session_id,
                turn_id,
                "approval",
                Some(approval_id),
                "approval.resume.completed",
                Some("completed"),
                serde_json::json!({"approvalRequestId": approval_id, "pausedActionId": paused_id, "result": result}),
            )
            .await?;
            Ok(())
        }
        Err(error) => {
            let error_json = serde_json::json!({"error": error.to_string()});
            sqlx::query(
                "UPDATE paused_actions SET status = 'failed', error = $2, updated_at = $3, completed_at = $3 WHERE id = $1 AND status = 'resuming'",
            )
            .bind(paused_id)
            .bind(&error_json)
            .bind(Utc::now())
            .execute(pool)
            .await?;
            db::append_event(
                pool,
                session_id,
                turn_id,
                "approval",
                Some(approval_id),
                "approval.resume.failed",
                Some("failed"),
                serde_json::json!({"approvalRequestId": approval_id, "pausedActionId": paused_id, "error": error_json}),
            )
            .await?;
            bail!("approval resume failed: {}", error_json);
        }
    }
}

pub fn validate_decide_status(status: &str) -> std::result::Result<(), &'static str> {
    if status == "pending" {
        Ok(())
    } else {
        Err("approval request is not pending")
    }
}

pub fn validate_resume_request_status(status: &str) -> std::result::Result<(), &'static str> {
    if status == "approved" {
        Ok(())
    } else {
        Err("approval request is not approved")
    }
}

pub fn validate_resume_paused_status(status: &str) -> std::result::Result<(), &'static str> {
    if status == "pendingApproval" || status == "approved" {
        Ok(())
    } else {
        Err("paused action is not resume-ready")
    }
}

fn row_to_json(row: sqlx::postgres::PgRow) -> Value {
    let id: Uuid = row.get("id");
    let session_id: Uuid = row.get("session_id");
    let turn_id: Option<Uuid> = row.get("turn_id");
    let created_at: DateTime<Utc> = row.get("created_at");
    let completed_at: Option<DateTime<Utc>> = row.get("completed_at");
    serde_json::json!({
        "id": id,
        "sessionId": session_id,
        "turnId": turn_id,
        "actionName": row.get::<String, _>("action_name"),
        "requestedByRole": row.get::<Value, _>("requested_by_role"),
        "inputContext": row.get::<Value, _>("input_context"),
        "requiredApproverKind": row.get::<String, _>("required_approver_kind"),
        "status": row.get::<String, _>("status"),
        "createdAt": created_at,
        "completedAt": completed_at,
    })
}

fn decision_row_to_json(row: sqlx::postgres::PgRow) -> Value {
    let id: Uuid = row.get("id");
    let created_at: DateTime<Utc> = row.get("created_at");
    serde_json::json!({
        "id": id,
        "decision": row.get::<String, _>("decision"),
        "reason": row.get::<String, _>("reason"),
        "decidedBy": row.get::<Value, _>("decided_by"),
        "createdAt": created_at,
    })
}

fn paused_row_to_json(row: sqlx::postgres::PgRow) -> Value {
    let id: Uuid = row.get("id");
    let approval_request_id: Uuid = row.get("approval_request_id");
    let session_id: Uuid = row.get("session_id");
    let turn_id: Option<Uuid> = row.get("turn_id");
    let tool_call_id: Option<Uuid> = row.get("tool_call_id");
    let script_run_id: Option<Uuid> = row.get("script_run_id");
    let created_at: DateTime<Utc> = row.get("created_at");
    let updated_at: DateTime<Utc> = row.get("updated_at");
    let completed_at: Option<DateTime<Utc>> = row.get("completed_at");
    serde_json::json!({
        "id": id,
        "approvalRequestId": approval_request_id,
        "sessionId": session_id,
        "turnId": turn_id,
        "toolCallId": tool_call_id,
        "scriptRunId": script_run_id,
        "actionName": row.get::<String, _>("action_name"),
        "actionInput": row.get::<Value, _>("action_input"),
        "roleSnapshot": row.get::<Value, _>("role_snapshot"),
        "status": row.get::<String, _>("status"),
        "result": row.get::<Option<Value>, _>("result"),
        "error": row.get::<Option<Value>, _>("error"),
        "createdAt": created_at,
        "updatedAt": updated_at,
        "completedAt": completed_at,
    })
}

#[cfg(test)]
mod tests {
    use super::{validate_decide_status, validate_resume_paused_status, validate_resume_request_status};

    #[test]
    fn approval_decision_rejects_non_pending_statuses() {
        assert!(validate_decide_status("pending").is_ok());
        for status in ["approved", "denied", "expired", "cancelled"] {
            assert_eq!(validate_decide_status(status), Err("approval request is not pending"));
        }
    }

    #[test]
    fn resume_rejects_pending_denied_and_completed_states() {
        assert!(validate_resume_request_status("approved").is_ok());
        for status in ["pending", "denied", "expired", "cancelled"] {
            assert_eq!(validate_resume_request_status(status), Err("approval request is not approved"));
        }
        assert!(validate_resume_paused_status("pendingApproval").is_ok());
        assert!(validate_resume_paused_status("approved").is_ok());
        for status in ["resuming", "completed", "failed", "cancelled"] {
            assert_eq!(validate_resume_paused_status(status), Err("paused action is not resume-ready"));
        }
    }
}
