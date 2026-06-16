use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{db, output_artifacts};

pub const DEFAULT_EFFECTIVE_CONTEXT_BUDGET: usize = 120_000;
pub const DEFAULT_MAX_OUTPUT_RESERVE: usize = 8_000;
pub const DEFAULT_PRE_SEND_THRESHOLD: usize = 90_000;
pub const DEFAULT_FAIL_CLOSED_THRESHOLD: usize = 115_000;
const FIXED_REQUEST_RESERVE_TOKENS: usize = 1_024;
const REPLACEMENT_CONTEXT_LIMIT: usize = 16_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionBudget {
    pub effective_context_budget: usize,
    pub max_output_reserve: usize,
    pub pre_send_threshold: usize,
    pub fail_closed_threshold: usize,
}

impl Default for CompactionBudget {
    fn default() -> Self {
        Self {
            effective_context_budget: DEFAULT_EFFECTIVE_CONTEXT_BUDGET,
            max_output_reserve: DEFAULT_MAX_OUTPUT_RESERVE,
            pre_send_threshold: DEFAULT_PRE_SEND_THRESHOLD,
            fail_closed_threshold: DEFAULT_FAIL_CLOSED_THRESHOLD,
        }
    }
}

impl CompactionBudget {
    pub fn from_env() -> Self {
        let defaults = Self::default();
        Self {
            effective_context_budget: env_usize("ROBDEX_AGENT_RUNTIME_CONTEXT_BUDGET", defaults.effective_context_budget),
            max_output_reserve: env_usize("ROBDEX_AGENT_RUNTIME_MAX_OUTPUT_RESERVE", defaults.max_output_reserve),
            pre_send_threshold: env_usize("ROBDEX_AGENT_RUNTIME_PRE_SEND_COMPACTION_THRESHOLD", defaults.pre_send_threshold),
            fail_closed_threshold: env_usize("ROBDEX_AGENT_RUNTIME_FAIL_CLOSED_THRESHOLD", defaults.fail_closed_threshold),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEstimate {
    pub bytes: usize,
    pub estimated_tokens: usize,
    pub fixed_reserve_tokens: usize,
    pub max_output_reserve: usize,
    pub total_estimated_tokens: usize,
    pub budget: CompactionBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionCheckpoint {
    pub id: Uuid,
    pub session_id: Uuid,
    pub status: String,
    pub source_start_turn_id: Option<Uuid>,
    pub source_end_turn_id: Option<Uuid>,
    pub compacted_through_turn_id: Option<Uuid>,
    pub compacted_through_event_sequence: Option<i64>,
    pub replacement_context: String,
    pub summary: Value,
    pub estimate_metadata: Value,
    pub model_provider_metadata: Value,
    pub failure_info: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct CompletedTurn {
    id: Uuid,
    input_text: String,
    assistant: Option<String>,
    started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
struct ReplacementContextData {
    active_task_goal: String,
    important_decisions: Vec<String>,
    touched_surfaces: Vec<String>,
}

pub fn estimate_context_value(value: &Value, budget: CompactionBudget) -> ContextEstimate {
    let bytes = serde_json::to_vec(value).map(|bytes| bytes.len()).unwrap_or_default();
    let estimated_tokens = bytes / 4;
    let total_estimated_tokens = estimated_tokens + FIXED_REQUEST_RESERVE_TOKENS + budget.max_output_reserve;
    ContextEstimate {
        bytes,
        estimated_tokens,
        fixed_reserve_tokens: FIXED_REQUEST_RESERVE_TOKENS,
        max_output_reserve: budget.max_output_reserve,
        total_estimated_tokens,
        budget,
    }
}

pub fn estimate_model_surfaces(surfaces: &Value, budget: CompactionBudget) -> ContextEstimate {
    estimate_context_value(surfaces, budget)
}

pub async fn latest_completed_checkpoint(pool: &PgPool, session_id: Uuid) -> Result<Option<CompactionCheckpoint>> {
    let row = sqlx::query(
        r#"
        SELECT id, session_id, status, source_start_turn_id, source_end_turn_id, compacted_through_turn_id,
               compacted_through_event_sequence, replacement_context, summary, estimate_metadata,
               model_provider_metadata, failure_info, created_at, completed_at
        FROM compaction_checkpoints
        WHERE session_id=$1 AND status='completed'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    row.map(checkpoint_from_row).transpose()
}

pub async fn latest_applicable_completed_checkpoint(pool: &PgPool, session_id: Uuid, stop_at_turn: Option<Uuid>) -> Result<Option<CompactionCheckpoint>> {
    let rows = sqlx::query(
        r#"
        SELECT cc.id, cc.session_id, cc.status, cc.source_start_turn_id, cc.source_end_turn_id, cc.compacted_through_turn_id,
               cc.compacted_through_event_sequence, cc.replacement_context, cc.summary, cc.estimate_metadata,
               cc.model_provider_metadata, cc.failure_info, cc.created_at, cc.completed_at
        FROM compaction_checkpoints cc
        JOIN turns compacted ON compacted.id = cc.compacted_through_turn_id
        LEFT JOIN turns stop_turn ON stop_turn.id = $2
        WHERE cc.session_id=$1
          AND cc.status='completed'
          AND ($2::uuid IS NULL OR compacted.started_at <= stop_turn.started_at)
        ORDER BY cc.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .bind(stop_at_turn)
    .fetch_optional(pool)
    .await?;
    rows.map(checkpoint_from_row).transpose()
}

pub async fn compact_session_through_latest_completed_turn(pool: &PgPool, session_id: Uuid, budget: CompactionBudget) -> Result<CompactionCheckpoint> {
    let row = sqlx::query("SELECT id FROM turns WHERE session_id=$1 AND status='completed' ORDER BY started_at DESC LIMIT 1")
        .bind(session_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        insert_failed_checkpoint(pool, session_id, None, "session has no completed turns to compact", budget).await?;
        bail!("session has no completed turns to compact");
    };
    compact_session_through_turn(pool, session_id, row.get("id"), budget).await
}

pub async fn compact_session_through_turn(pool: &PgPool, session_id: Uuid, through_turn_id: Uuid, budget: CompactionBudget) -> Result<CompactionCheckpoint> {
    let turns = match completed_turns_through(pool, session_id, through_turn_id).await {
        Ok(turns) => turns,
        Err(error) => {
            let reason = error.to_string();
            insert_failed_checkpoint(pool, session_id, Some(through_turn_id), &reason, budget).await?;
            bail!("{reason}");
        }
    };
    if turns.is_empty() {
        insert_failed_checkpoint(pool, session_id, Some(through_turn_id), "no completed turns through requested boundary", budget).await?;
        bail!("no completed turns through requested boundary");
    }
    let source_start_turn_id = turns.first().map(|turn| turn.id);
    let source_end_turn_id = turns.last().map(|turn| turn.id);
    let event_boundary: Option<i64> = sqlx::query_scalar("SELECT MAX(sequence) FROM event_stream WHERE session_id=$1 AND (turn_id IS NULL OR turn_id = ANY($2))")
        .bind(session_id)
        .bind(turns.iter().map(|turn| turn.id).collect::<Vec<_>>())
        .fetch_one(pool)
        .await?;
    let artifact_rows = sqlx::query(
        r#"
        SELECT id, stream, byte_count, line_count
        FROM execution_output_artifacts
        WHERE session_id=$1 AND (turn_id IS NULL OR turn_id = ANY($2))
        ORDER BY created_at ASC
        LIMIT 20
        "#,
    )
    .bind(session_id)
    .bind(turns.iter().map(|turn| turn.id).collect::<Vec<_>>())
    .fetch_all(pool)
    .await?;
    let artifact_refs = artifact_rows
        .iter()
        .map(|row| json!({
            "artifactId": row.get::<Uuid, _>("id"),
            "stream": row.get::<String, _>("stream"),
            "byteCount": row.get::<i64, _>("byte_count"),
            "lineCount": row.get::<i64, _>("line_count"),
        }))
        .collect::<Vec<_>>();
    let pending_approvals = pending_approval_refs(pool, session_id).await?;
    let pending_processes = pending_process_refs(pool, session_id).await?;
    let context_data = replacement_context_data(pool, session_id, &turns).await?;
    let replacement_context = bounded_replacement_context(&turns, &artifact_refs, &pending_approvals, &pending_processes, &context_data);
    let surfaces = json!({
        "replacementContext": replacement_context,
        "turnCount": turns.len(),
        "artifactRefs": artifact_refs,
    });
    let estimate = estimate_context_value(&surfaces, budget);
    let id = Uuid::new_v4();
    let summary = json!({
        "kind": "session_memory",
        "turnCount": turns.len(),
        "source": "deterministic_compaction",
        "outputArtifacts": artifact_refs,
        "pendingApprovals": pending_approvals,
        "pendingProcesses": pending_processes,
        "latestActionableState": turns.last().map(|turn| turn.assistant.clone().unwrap_or_else(|| turn.input_text.clone())).unwrap_or_default(),
        "notes": [
            "Original audit rows are preserved.",
            "Command discovery remains synthetic/runtime-managed.",
            "Output evidence is represented by artifact handles and bounded excerpts only."
        ],
    });
    sqlx::query(
        r#"
        INSERT INTO compaction_checkpoints (
            id, session_id, status, source_start_turn_id, source_end_turn_id, compacted_through_turn_id,
            compacted_through_event_sequence, replacement_context, summary, estimate_metadata,
            model_provider_metadata, completed_at
        )
        VALUES ($1,$2,'completed',$3,$4,$5,$6,$7,$8,$9,$10,now())
        "#,
    )
    .bind(id)
    .bind(session_id)
    .bind(source_start_turn_id)
    .bind(source_end_turn_id)
    .bind(through_turn_id)
    .bind(event_boundary)
    .bind(&replacement_context)
    .bind(summary)
    .bind(serde_json::to_value(&estimate)?)
    .bind(json!({"provider": Value::Null, "model": Value::Null, "summarizer": "deterministic-bounded-v1"}))
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        Some(through_turn_id),
        "compaction_checkpoint",
        Some(id),
        "compaction.completed",
        Some("completed"),
        json!({"checkpointId": id, "compactedThroughTurnId": through_turn_id, "estimate": estimate}),
    )
    .await?;
    latest_completed_checkpoint(pool, session_id).await?.ok_or_else(|| anyhow::anyhow!("completed checkpoint was not readable after insert"))
}

pub async fn reconstructed_history_after_checkpoint(pool: &PgPool, session_id: Uuid) -> Result<Vec<db::HistoryItem>> {
    let mut chain = Vec::new();
    let mut cursor = Some(session_id);
    while let Some(id) = cursor {
        let session = db::session_record(pool, id).await?;
        cursor = session.forked_from_session_id;
        chain.push(session);
    }
    chain.reverse();
    let mut history = Vec::new();
    for (idx, session) in chain.iter().enumerate() {
        let stop = if idx + 1 < chain.len() { chain[idx + 1].forked_from_turn_id } else { None };
        history.extend(reconstructed_local_segment(pool, session.id, stop).await?);
    }
    Ok(history)
}

async fn reconstructed_local_segment(pool: &PgPool, session_id: Uuid, stop_at_turn: Option<Uuid>) -> Result<Vec<db::HistoryItem>> {
    if let Some(checkpoint) = latest_applicable_completed_checkpoint(pool, session_id, stop_at_turn).await? {
        let mut out = vec![db::HistoryItem {
            session_id,
            turn_id: checkpoint.compacted_through_turn_id.unwrap_or(checkpoint.id),
            user: format!("Compaction checkpoint {} replacement context", checkpoint.id),
            assistant: Some(checkpoint.replacement_context.clone()),
            started_at: checkpoint.created_at,
            source: "compaction_checkpoint".to_string(),
            checkpoint_id: Some(checkpoint.id),
        }];
        let rows = local_completed_turn_rows_after(pool, session_id, checkpoint.compacted_through_turn_id, stop_at_turn).await?;
        out.extend(rows.into_iter().map(|turn| history_item_from_turn(session_id, turn)));
        return Ok(out);
    }
    Ok(local_completed_turn_rows_until(pool, session_id, stop_at_turn).await?.into_iter().map(|turn| history_item_from_turn(session_id, turn)).collect())
}

async fn completed_turns_through(pool: &PgPool, session_id: Uuid, through_turn_id: Uuid) -> Result<Vec<CompletedTurn>> {
    let boundary: Option<DateTime<Utc>> = sqlx::query_scalar("SELECT started_at FROM turns WHERE session_id=$1 AND id=$2 AND status='completed'")
        .bind(session_id)
        .bind(through_turn_id)
        .fetch_optional(pool)
        .await?;
    let Some(boundary) = boundary else {
        bail!("completed compaction boundary turn not found for session");
    };
    let rows = sqlx::query(
        r#"
        SELECT t.id, t.input_text, t.started_at,
               (SELECT me.payload->>'summary' FROM model_events me WHERE me.turn_id = t.id AND me.event_type = 'final_response' ORDER BY me.ordinal DESC LIMIT 1) AS assistant
        FROM turns t
        WHERE t.session_id=$1 AND t.status='completed' AND t.started_at <= $2
        ORDER BY t.started_at ASC
        "#,
    )
    .bind(session_id)
    .bind(boundary)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(turn_from_row).collect())
}

async fn local_completed_turn_rows_until(pool: &PgPool, session_id: Uuid, stop_at_turn: Option<Uuid>) -> Result<Vec<CompletedTurn>> {
    let rows = sqlx::query(
        r#"
        SELECT t.id, t.input_text, t.started_at,
               (SELECT me.payload->>'summary' FROM model_events me WHERE me.turn_id = t.id AND me.event_type = 'final_response' ORDER BY me.ordinal DESC LIMIT 1) AS assistant
        FROM turns t
        WHERE t.session_id=$1 AND t.status='completed'
        ORDER BY t.started_at ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    trim_at_stop(rows.into_iter().map(turn_from_row).collect(), stop_at_turn)
}

async fn local_completed_turn_rows_after(pool: &PgPool, session_id: Uuid, after_turn_id: Option<Uuid>, stop_at_turn: Option<Uuid>) -> Result<Vec<CompletedTurn>> {
    if after_turn_id.is_some() && after_turn_id == stop_at_turn {
        return Ok(Vec::new());
    }
    let after_started: Option<DateTime<Utc>> = if let Some(after_turn_id) = after_turn_id {
        sqlx::query_scalar("SELECT started_at FROM turns WHERE session_id=$1 AND id=$2")
            .bind(session_id)
            .bind(after_turn_id)
            .fetch_optional(pool)
            .await?
    } else {
        None
    };
    let rows = sqlx::query(
        r#"
        SELECT t.id, t.input_text, t.started_at,
               (SELECT me.payload->>'summary' FROM model_events me WHERE me.turn_id = t.id AND me.event_type = 'final_response' ORDER BY me.ordinal DESC LIMIT 1) AS assistant
        FROM turns t
        WHERE t.session_id=$1 AND t.status='completed' AND ($2::timestamptz IS NULL OR t.started_at > $2)
        ORDER BY t.started_at ASC
        "#,
    )
    .bind(session_id)
    .bind(after_started)
    .fetch_all(pool)
    .await?;
    trim_at_stop(rows.into_iter().map(turn_from_row).collect(), stop_at_turn)
}

fn trim_at_stop(turns: Vec<CompletedTurn>, stop_at_turn: Option<Uuid>) -> Result<Vec<CompletedTurn>> {
    let Some(stop) = stop_at_turn else {
        return Ok(turns);
    };
    let mut out = Vec::new();
    for turn in turns {
        let id = turn.id;
        out.push(turn);
        if id == stop {
            return Ok(out);
        }
    }
    bail!("completed history does not contain fork boundary {stop}");
}

fn turn_from_row(row: sqlx::postgres::PgRow) -> CompletedTurn {
    CompletedTurn {
        id: row.get("id"),
        input_text: row.get("input_text"),
        assistant: row.get("assistant"),
        started_at: row.get("started_at"),
    }
}

fn history_item_from_turn(session_id: Uuid, turn: CompletedTurn) -> db::HistoryItem {
    db::HistoryItem {
        session_id,
        turn_id: turn.id,
        user: turn.input_text,
        assistant: turn.assistant,
        started_at: turn.started_at,
        source: "reconstructed_session_history".to_string(),
        checkpoint_id: None,
    }
}

async fn pending_approval_refs(pool: &PgPool, session_id: Uuid) -> Result<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT id, turn_id, action_name, status FROM approval_requests WHERE session_id=$1 AND status IN ('pending','approved') ORDER BY created_at ASC LIMIT 20",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| json!({
            "approvalId": row.get::<Uuid, _>("id"),
            "turnId": row.get::<Option<Uuid>, _>("turn_id"),
            "actionName": row.get::<String, _>("action_name"),
            "status": row.get::<String, _>("status"),
        }))
        .collect())
}

async fn pending_process_refs(pool: &PgPool, session_id: Uuid) -> Result<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT id, handle, status, end_of_turn_behavior, end_of_session_behavior FROM managed_processes WHERE session_id=$1 AND status IN ('running','starting') ORDER BY start_time ASC LIMIT 20",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| json!({
            "processId": row.get::<Uuid, _>("id"),
            "handle": row.get::<String, _>("handle"),
            "status": row.get::<String, _>("status"),
            "endOfTurnBehavior": row.get::<String, _>("end_of_turn_behavior"),
            "endOfSessionBehavior": row.get::<String, _>("end_of_session_behavior"),
        }))
        .collect())
}

async fn replacement_context_data(pool: &PgPool, session_id: Uuid, turns: &[CompletedTurn]) -> Result<ReplacementContextData> {
    let session = db::session_record(pool, session_id).await?;
    let active_task_goal = session
        .title
        .or(session.name)
        .unwrap_or_else(|| turns.last().map(|turn| compact_text(&turn.input_text, 600)).unwrap_or_else(|| "Continue the current session task.".to_string()));
    let decision_rows = sqlx::query(
        r#"
        SELECT event_type, status, payload
        FROM event_stream
        WHERE session_id=$1
          AND event_type IN ('policy.decision','approval.decided','command_registry.request.decided','command_registry.request.applied','route.decision')
        ORDER BY sequence DESC
        LIMIT 12
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    let important_decisions = decision_rows
        .into_iter()
        .rev()
        .map(|row| {
            let event_type: String = row.get("event_type");
            let status: Option<String> = row.get("status");
            let payload: Value = row.get("payload");
            compact_text(&format!("{event_type} status={} payload={payload}", status.unwrap_or_else(|| "none".to_string())), 600)
        })
        .collect::<Vec<_>>();
    let script_sources = sqlx::query(
        r#"
        SELECT sr.source
        FROM script_runs sr
        JOIN tool_calls tc ON tc.id = sr.tool_call_id
        WHERE tc.session_id=$1
        ORDER BY sr.started_at DESC
        LIMIT 20
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    let mut touched_surfaces = Vec::new();
    for row in script_sources {
        let source: String = row.get("source");
        for marker in ["fs.read(", "fs.write(", "patch.apply("] {
            if source.contains(marker) {
                touched_surfaces.push(compact_text(&format!("{marker} in script: {}", source.replace('\n', " ")), 500));
            }
        }
    }
    touched_surfaces.sort();
    touched_surfaces.dedup();
    touched_surfaces.truncate(20);
    Ok(ReplacementContextData {
        active_task_goal,
        important_decisions,
        touched_surfaces,
    })
}

fn bounded_replacement_context(turns: &[CompletedTurn], artifact_refs: &[Value], pending_approvals: &[Value], pending_processes: &[Value], context_data: &ReplacementContextData) -> String {
    let mut lines = vec![
        "Runtime compaction checkpoint replacement context.".to_string(),
        "Original turns, model events, tool calls, process rows, approvals, and output artifacts remain durable audit history.".to_string(),
        "Command discovery is not persisted here; use the current synthetic runtime command context and cmd.describe() for live command details.".to_string(),
        "Owner instructions: preserve the session role snapshot instructions that are sent separately with each model request.".to_string(),
        format!("Active task goal: {}", compact_text(&context_data.active_task_goal, 800)),
        format!("Compacted completed turns: {}", turns.len()),
    ];
    lines.push("Important decisions:".to_string());
    if context_data.important_decisions.is_empty() {
        lines.push("- none recorded in compacted event window".to_string());
    } else {
        for decision in &context_data.important_decisions {
            lines.push(format!("- {decision}"));
        }
    }
    lines.push("Files/surfaces touched:".to_string());
    if context_data.touched_surfaces.is_empty() {
        lines.push("- none detected from bounded script/audit extraction".to_string());
    } else {
        for surface in &context_data.touched_surfaces {
            lines.push(format!("- {surface}"));
        }
    }
    lines.push("Blockers: pending approvals and continuing processes are listed below when present.".to_string());
    lines.push("Pending approvals by id:".to_string());
    if pending_approvals.is_empty() {
        lines.push("- none".to_string());
    } else {
        for approval in pending_approvals {
            lines.push(format!(
                "- approval={} action={} status={} turn={}",
                approval.get("approvalId").and_then(Value::as_str).unwrap_or("<unknown>"),
                approval.get("actionName").and_then(Value::as_str).unwrap_or("<unknown>"),
                approval.get("status").and_then(Value::as_str).unwrap_or("<unknown>"),
                approval.get("turnId").and_then(Value::as_str).unwrap_or("<none>"),
            ));
        }
    }
    lines.push("Pending/continuing managed processes by handle/id:".to_string());
    if pending_processes.is_empty() {
        lines.push("- none".to_string());
    } else {
        for process in pending_processes {
            lines.push(format!(
                "- handle={} process={} status={} endOfTurn={} endOfSession={}",
                process.get("handle").and_then(Value::as_str).unwrap_or("<unknown>"),
                process.get("processId").and_then(Value::as_str).unwrap_or("<unknown>"),
                process.get("status").and_then(Value::as_str).unwrap_or("<unknown>"),
                process.get("endOfTurnBehavior").and_then(Value::as_str).unwrap_or("<unknown>"),
                process.get("endOfSessionBehavior").and_then(Value::as_str).unwrap_or("<unknown>"),
            ));
        }
    }
    if !artifact_refs.is_empty() {
        lines.push("Output artifacts referenced by handle only:".to_string());
        for artifact in artifact_refs.iter().take(20) {
            lines.push(format!(
                "- artifact={} stream={} bytes={} lines={}",
                artifact.get("artifactId").and_then(Value::as_str).unwrap_or("<unknown>"),
                artifact.get("stream").and_then(Value::as_str).unwrap_or("<unknown>"),
                artifact.get("byteCount").and_then(Value::as_i64).unwrap_or_default(),
                artifact.get("lineCount").and_then(Value::as_i64).unwrap_or_default(),
            ));
        }
    }
    lines.push("Bounded findings and latest actionable state:".to_string());
    for turn in turns.iter().rev().take(12).rev() {
        lines.push(format!("- user: {}", compact_text(&turn.input_text, 400)));
        if let Some(assistant) = &turn.assistant {
            lines.push(format!("  assistant: {}", compact_text(assistant, 400)));
        }
    }
    let mut context = lines.join("\n");
    if context.len() > REPLACEMENT_CONTEXT_LIMIT {
        context.truncate(REPLACEMENT_CONTEXT_LIMIT);
        context.push_str("\n[replacement context truncated at bounded limit]");
    }
    context
}

async fn insert_failed_checkpoint(pool: &PgPool, session_id: Uuid, through_turn_id: Option<Uuid>, reason: &str, budget: CompactionBudget) -> Result<CompactionCheckpoint> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO compaction_checkpoints (id, session_id, status, compacted_through_turn_id, replacement_context, summary, estimate_metadata, model_provider_metadata, failure_info, completed_at)
        VALUES ($1,$2,'failed',$3,'',$4,$5,$6,$7,now())
        "#,
    )
    .bind(id)
    .bind(session_id)
    .bind(None::<Uuid>)
    .bind(json!({}))
    .bind(json!({"budget": budget}))
    .bind(json!({"summarizer": "deterministic-bounded-v1"}))
    .bind(json!({"reason": reason, "requestedThroughTurnId": through_turn_id}))
    .execute(pool)
    .await?;
    db::append_event(pool, session_id, through_turn_id, "compaction_checkpoint", Some(id), "compaction.failed", Some("failed"), json!({"checkpointId": id, "reason": reason})).await?;
    let row = sqlx::query(
        "SELECT id, session_id, status, source_start_turn_id, source_end_turn_id, compacted_through_turn_id, compacted_through_event_sequence, replacement_context, summary, estimate_metadata, model_provider_metadata, failure_info, created_at, completed_at FROM compaction_checkpoints WHERE id=$1",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    checkpoint_from_row(row)
}

fn checkpoint_from_row(row: sqlx::postgres::PgRow) -> Result<CompactionCheckpoint> {
    Ok(CompactionCheckpoint {
        id: row.get("id"),
        session_id: row.get("session_id"),
        status: row.get("status"),
        source_start_turn_id: row.get("source_start_turn_id"),
        source_end_turn_id: row.get("source_end_turn_id"),
        compacted_through_turn_id: row.get("compacted_through_turn_id"),
        compacted_through_event_sequence: row.get("compacted_through_event_sequence"),
        replacement_context: row.get("replacement_context"),
        summary: row.get("summary"),
        estimate_metadata: row.get("estimate_metadata"),
        model_provider_metadata: row.get("model_provider_metadata"),
        failure_info: row.get("failure_info"),
        created_at: row.get("created_at"),
        completed_at: row.get("completed_at"),
    })
}

fn compact_text(text: &str, limit: usize) -> String {
    let mut compact = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(" ");
    if compact.len() > limit {
        compact.truncate(limit);
        compact.push('…');
    }
    compact
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
}

#[allow(dead_code)]
fn _assert_output_artifact_limits_are_available() {
    let _ = output_artifacts::DEFAULT_VISIBLE_BYTE_LIMIT;
}
