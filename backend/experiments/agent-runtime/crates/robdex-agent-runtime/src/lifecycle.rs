use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalStatus {
    Completed,
    Failed,
    Timeout,
}

impl TerminalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TerminalStatus::Completed => "completed",
            TerminalStatus::Failed => "failed",
            TerminalStatus::Timeout => "timeout",
        }
    }
}

impl TryFrom<&str> for TerminalStatus {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "timeout" => Ok(Self::Timeout),
            other => bail!("unsupported terminal status: {other}"),
        }
    }
}

pub async fn complete_turn(
    pool: &PgPool,
    id: Uuid,
    status: TerminalStatus,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE turns
        SET status = $2, completed_at = $3, duration_ms = GREATEST(0, (EXTRACT(EPOCH FROM ($3 - started_at)) * 1000)::bigint)
        WHERE id = $1 AND status = 'running'
        "#,
    )
    .bind(id)
    .bind(status.as_str())
    .bind(completed_at)
    .execute(pool)
    .await?;
    ensure_one_updated("turn", id, result.rows_affected())
}

pub async fn complete_tool_call(
    pool: &PgPool,
    id: Uuid,
    status: TerminalStatus,
    result_json: &Value,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE tool_calls
        SET status = $2, result = $3, completed_at = $4, duration_ms = GREATEST(0, (EXTRACT(EPOCH FROM ($4 - started_at)) * 1000)::bigint)
        WHERE id = $1 AND status = 'running'
        "#,
    )
    .bind(id)
    .bind(status.as_str())
    .bind(result_json)
    .bind(completed_at)
    .execute(pool)
    .await?;
    ensure_one_updated("tool_call", id, result.rows_affected())
}

pub async fn complete_script_run(
    pool: &PgPool,
    id: Uuid,
    status: TerminalStatus,
    final_output: &str,
    stderr: &str,
    truncation: &Value,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE script_runs
        SET status = $2,
            completed_at = $3,
            duration_ms = GREATEST(0, (EXTRACT(EPOCH FROM ($3 - started_at)) * 1000)::bigint),
            final_output = $4,
            stderr = $5,
            truncation = $6
        WHERE id = $1 AND status = 'running'
        "#,
    )
    .bind(id)
    .bind(status.as_str())
    .bind(completed_at)
    .bind(final_output)
    .bind(stderr)
    .bind(truncation)
    .execute(pool)
    .await?;
    ensure_one_updated("script_run", id, result.rows_affected())
}

pub async fn complete_host_api_call(
    pool: &PgPool,
    id: Uuid,
    status: TerminalStatus,
    output: &Value,
    duration_ms: i64,
    truncation: &Value,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE host_api_calls
        SET status = $2,
            completed_at = $3,
            duration_ms = $4,
            output = $5,
            truncation = $6
        WHERE id = $1 AND status = 'running'
        "#,
    )
    .bind(id)
    .bind(status.as_str())
    .bind(completed_at)
    .bind(duration_ms)
    .bind(output)
    .bind(truncation)
    .execute(pool)
    .await?;
    ensure_one_updated("host_api_call", id, result.rows_affected())
}

pub async fn complete_command_run(
    pool: &PgPool,
    id: Uuid,
    status: TerminalStatus,
    stdout: &str,
    stderr: &str,
    exit_status: Option<i32>,
    duration_ms: i64,
    policy_decision: &Value,
    truncation: &Value,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE command_runs
        SET status = $2,
            completed_at = $3,
            duration_ms = $4,
            stdout = $5,
            stderr = $6,
            exit_status = $7,
            policy_decision = $8,
            truncation = $9
        WHERE id = $1 AND status = 'running'
        "#,
    )
    .bind(id)
    .bind(status.as_str())
    .bind(completed_at)
    .bind(duration_ms)
    .bind(stdout)
    .bind(stderr)
    .bind(exit_status)
    .bind(policy_decision)
    .bind(truncation)
    .execute(pool)
    .await?;
    ensure_one_updated("command_run", id, result.rows_affected())
}

pub fn ensure_one_updated(entity: &str, id: Uuid, rows_affected: u64) -> Result<()> {
    if rows_affected == 1 {
        Ok(())
    } else {
        bail!(
            "terminal status update failed for {entity} {id}: expected one running row, updated {rows_affected}"
        )
    }
}
