use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::command_registry::CommandContextEvidence;
use crate::db::{HistoryItem, SessionSummary};
use crate::model::{ModelHistoryItem, RuntimeInputMessage};
use crate::roles::{RoleSnapshot, visible_tool_bundle_for_role};

#[derive(Debug, Clone)]
pub struct ContextSnapshotRecord {
    pub role_epoch: String,
    pub context_epoch: i64,
    pub previous_context_epoch: Option<i64>,
    pub context_event_watermark: i64,
    pub event_kind: String,
    pub event_sequence: i64,
    pub snapshot: Value,
    pub previous_snapshot: Option<Value>,
}

pub fn role_epoch(role: &RoleSnapshot) -> String {
    format!("{}:{}:{}", role.id, role.version, role.role_version_id)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn role_instructions_block(role: &RoleSnapshot) -> String {
    format!(
        "<role_instructions epoch=\"{}\" role_key=\"{}\" role_version=\"{}\">\n{}\n\nRole instructions are not additive. The latest role_instructions block supersedes all earlier role_instructions blocks.\n</role_instructions>",
        xml_escape(&role_epoch(role)),
        xml_escape(&role.id),
        xml_escape(&role.version),
        role.instruction_text
    )
}

pub fn runtime_context_block(snapshot: &ContextSnapshotRecord) -> String {
    let s = &snapshot.snapshot;
    let cwd = s.pointer("/cwd/path").and_then(Value::as_str).unwrap_or("unavailable");
    let cwd_state = s.pointer("/cwd/state").and_then(Value::as_str).unwrap_or("unavailable");
    let cwd_source = s.pointer("/cwd/source").and_then(Value::as_str).unwrap_or("unknown");
    let project = s.pointer("/project/displayName").and_then(Value::as_str).unwrap_or("Unassigned");
    let project_key = s.pointer("/project/key").and_then(Value::as_str).unwrap_or("unassigned");
    let session_id = s.get("sessionId").and_then(Value::as_str).unwrap_or("unknown");
    let model = s.get("model").and_then(Value::as_str).unwrap_or("unknown");
    let command_context_id = s.pointer("/tools/commandContextId").and_then(Value::as_str).unwrap_or("unknown");
    let visible_commands = s.pointer("/tools/visibleCommandCount").and_then(Value::as_i64).unwrap_or_default();
    let native_affordance_count = s.pointer("/tools/nativeAffordanceCount").and_then(Value::as_i64).unwrap_or_default();
    let sandbox_policy = s.pointer("/sandbox/policy").and_then(Value::as_str).unwrap_or("unknown");
    let approval_policy = s.pointer("/sandbox/approval").and_then(Value::as_str).unwrap_or("unknown");
    let policy_summary = s.pointer("/policy/summary").and_then(Value::as_str).unwrap_or("unknown");
    let generated_at = s.get("generatedAt").and_then(Value::as_str).unwrap_or("unknown");
    format!(
        "<runtime_context epoch=\"{}\">\n  <session_id>{}</session_id>\n  <project key=\"{}\">{}</project>\n  <cwd state=\"{}\" source=\"{}\">{}</cwd>\n  <role epoch=\"{}\" key=\"{}\" version=\"{}\" />\n  <model>{}</model>\n  <sandbox policy=\"{}\" approval=\"{}\">{}</sandbox>\n  <tools command_context_id=\"{}\" visible_command_count=\"{}\" native_affordance_count=\"{}\" />\n  <context_event_watermark>{}</context_event_watermark>\n  <sequence>{}</sequence>\n  <generated_at>{}</generated_at>\n</runtime_context>",
        snapshot.context_epoch,
        xml_escape(session_id),
        xml_escape(project_key),
        xml_escape(project),
        xml_escape(cwd_state),
        xml_escape(cwd_source),
        xml_escape(cwd),
        xml_escape(snapshot.role_epoch.as_str()),
        xml_escape(s.pointer("/role/key").and_then(Value::as_str).unwrap_or("unknown")),
        xml_escape(s.pointer("/role/version").and_then(Value::as_str).unwrap_or("unknown")),
        xml_escape(model),
        xml_escape(sandbox_policy),
        xml_escape(approval_policy),
        xml_escape(policy_summary),
        xml_escape(command_context_id),
        visible_commands,
        native_affordance_count,
        snapshot.context_event_watermark,
        snapshot.event_sequence,
        xml_escape(generated_at)
    )
}

pub fn context_delta_block(snapshot: &ContextSnapshotRecord) -> Option<String> {
    if matches!(snapshot.event_kind.as_str(), "initial" | "unchanged" | "snapshot_refreshed" | "role_authority_changed") {
        return None;
    }
    let details = bounded_delta_details(snapshot);
    Some(format!(
        "<context_delta epoch=\"{}\" previous_epoch=\"{}\">\n  <change kind=\"{}\" sequence=\"{}\">{}</change>\n</context_delta>",
        snapshot.context_epoch,
        snapshot.previous_context_epoch.map(|v| v.to_string()).unwrap_or_else(|| "none".to_string()),
        xml_escape(&snapshot.event_kind),
        snapshot.event_sequence,
        xml_escape(&details)
    ))
}

pub fn role_transition_summary_block(snapshot: &ContextSnapshotRecord) -> Option<String> {
    if snapshot.event_kind != "role_authority_changed" {
        return None;
    }
    let previous = snapshot.previous_snapshot.as_ref().and_then(|value| value.pointer("/role/epoch")).and_then(Value::as_str).unwrap_or("unknown");
    let authority_delta = snapshot
        .snapshot
        .pointer("/role/authorityDelta")
        .map(role_authority_delta_text)
        .unwrap_or_else(|| "Changed permissions were not summarized by the role save event.".to_string());
    Some(format!(
        "<role_transition_summary epoch=\"{}\" previous_epoch=\"{}\" sequence=\"{}\">\nRole authority changed. Prior role instructions and policy text are not active instructions. Use the current role_instructions block and this concise authority delta as active authority.\n{}\n</role_transition_summary>",
        xml_escape(&snapshot.role_epoch),
        xml_escape(previous),
        snapshot.event_sequence,
        xml_escape(&authority_delta)
    ))
}

fn role_authority_delta_text(summary: &Value) -> String {
    let action_summary = summary.get("changedActionSummary").unwrap_or(summary);
    fn list(value: Option<&Value>) -> String {
        let items = value
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .take(12)
            .collect::<Vec<_>>();
        if items.is_empty() { "none".to_string() } else { items.join(", ") }
    }
    let changed = action_summary
        .get("changedDecisions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(12)
        .filter_map(|item| {
            Some(format!(
                "{}:{}→{}",
                item.get("action")?.as_str()?,
                item.get("previousDecision")?.as_str()?,
                item.get("newDecision")?.as_str()?,
            ))
        })
        .collect::<Vec<_>>();
    let capabilities = summary
        .pointer("/changedCapabilitySummary")
        .map(|capability_summary| {
            format!(
                "added capabilities [{}]; removed capabilities [{}]",
                list(capability_summary.get("addedCapabilities")),
                list(capability_summary.get("removedCapabilities")),
            )
        })
        .unwrap_or_else(|| "capability changes not summarized".to_string());
    truncate_chars(
        &format!(
            "Changed role authority: added actions [{}]; removed actions [{}]; changed decisions [{}]; {}.",
            list(action_summary.get("addedActions")),
            list(action_summary.get("removedActions")),
            if changed.is_empty() { "none".to_string() } else { changed.join(", ") },
            capabilities,
        ),
        1200,
    )
}

fn bounded_delta_details(snapshot: &ContextSnapshotRecord) -> String {
    let old = snapshot.previous_snapshot.as_ref();
    let current = &snapshot.snapshot;
    let detail = match snapshot.event_kind.as_str() {
        "cwd_changed" => format!(
            "old_cwd={} new_cwd={} source={} sequence={}",
            old.and_then(|value| value.pointer("/cwd/path")).and_then(Value::as_str).unwrap_or("unknown"),
            current.pointer("/cwd/path").and_then(Value::as_str).unwrap_or("unknown"),
            current.pointer("/cwd/source").and_then(Value::as_str).unwrap_or("unknown"),
            snapshot.event_sequence,
        ),
        "project_assignment_changed" => format!(
            "old_project={} new_project={} sequence={}",
            old.and_then(|value| value.pointer("/project/key")).and_then(Value::as_str).unwrap_or("unknown"),
            current.pointer("/project/key").and_then(Value::as_str).unwrap_or("unknown"),
            snapshot.event_sequence,
        ),
        "worktree_root_changed" => format!(
            "old_worktree_root={} new_worktree_root={} sequence={}",
            old.and_then(|value| value.pointer("/worktreeRoot")).and_then(Value::as_str).unwrap_or("unknown"),
            current.pointer("/worktreeRoot").and_then(Value::as_str).unwrap_or("unknown"),
            snapshot.event_sequence,
        ),
        "model_changed" => format!(
            "old_model={} new_model={} sequence={}",
            old.and_then(|value| value.get("model")).and_then(Value::as_str).unwrap_or("unknown"),
            current.get("model").and_then(Value::as_str).unwrap_or("unknown"),
            snapshot.event_sequence,
        ),
        "tool_context_changed" => format!(
            "old_command_context={} new_command_context={} old_visible_count={} new_visible_count={} sequence={}",
            old.and_then(|value| value.pointer("/tools/commandContextId")).and_then(Value::as_str).unwrap_or("unknown"),
            current.pointer("/tools/commandContextId").and_then(Value::as_str).unwrap_or("unknown"),
            old.and_then(|value| value.pointer("/tools/visibleCommandCount")).and_then(Value::as_i64).unwrap_or_default(),
            current.pointer("/tools/visibleCommandCount").and_then(Value::as_i64).unwrap_or_default(),
            snapshot.event_sequence,
        ),
        "god_mode_changed" => format!(
            "old_god_mode={} new_god_mode={} sequence={}",
            old.and_then(|value| value.pointer("/godMode/state")).and_then(Value::as_str).unwrap_or("unknown"),
            current.pointer("/godMode/state").and_then(Value::as_str).unwrap_or("unknown"),
            snapshot.event_sequence,
        ),
        "session_lifecycle_changed" => format!(
            "old_lifecycle={} new_lifecycle={} sequence={}",
            old.and_then(|value| value.pointer("/lifecycle/status")).and_then(Value::as_str).unwrap_or("unknown"),
            current.pointer("/lifecycle/status").and_then(Value::as_str).unwrap_or("unknown"),
            snapshot.event_sequence,
        ),
        other => format!("kind={other} sequence={}", snapshot.event_sequence),
    };
    truncate_chars(&detail, 1200)
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut out = value.chars().take(limit.saturating_sub(1)).collect::<String>();
    out.push('…');
    out
}

pub fn prompt_cache_key(role: &RoleSnapshot, snapshot: Option<&ContextSnapshotRecord>) -> String {
    let role_epoch = role_epoch(role);
    match snapshot {
        Some(snapshot) => prompt_cache_key_from_epochs(&role_epoch, Some(&snapshot.context_epoch.to_string())),
        None => prompt_cache_key_from_epochs(&role_epoch, None),
    }
}

pub fn prompt_cache_key_from_epochs(role_epoch: &str, context_epoch: Option<&str>) -> String {
    let role_hash = format!("{:x}", Sha256::digest(role_epoch.as_bytes()));
    let context = context_epoch.unwrap_or("none");
    format!("rar2:{}:c{}", &role_hash[..24], context)
}

async fn project_label(pool: &PgPool, project_key: Option<&str>) -> Result<(String, String)> {
    let Some(project_key) = project_key else {
        return Ok(("unassigned".to_string(), "Unassigned".to_string()));
    };
    let row = sqlx::query("SELECT display_name FROM projects WHERE project_key=$1")
        .bind(project_key)
        .fetch_optional(pool)
        .await?;
    Ok((
        project_key.to_string(),
        row.map(|row| row.get::<String, _>("display_name"))
            .unwrap_or_else(|| project_key.to_string()),
    ))
}

pub async fn persist_context_snapshot(
    pool: &PgPool,
    session: &SessionSummary,
    role: &RoleSnapshot,
    command_context: &CommandContextEvidence,
    turn_id: Option<Uuid>,
) -> Result<ContextSnapshotRecord> {
    let previous: Option<(i64, i64, Value)> = sqlx::query_as(
        "SELECT context_epoch, context_event_watermark, snapshot FROM session_context_snapshots WHERE session_id=$1 ORDER BY context_epoch DESC LIMIT 1",
    )
    .bind(session.id)
    .fetch_optional(pool)
    .await?;
    let previous_epoch = previous.as_ref().map(|(epoch, _, _)| *epoch);
    let previous_watermark = previous.as_ref().map(|(_, watermark, _)| *watermark).unwrap_or(0);
    let context_epoch = previous_epoch.unwrap_or(0) + 1;
    let role_epoch = role_epoch(role);
    let (project_key, project_display) = project_label(pool, session.project_key.as_deref()).await?;
    let cwd_trimmed = session.workdir.trim();
    let cwd_known = !cwd_trimmed.is_empty();
    let native_affordances = visible_tool_bundle_for_role(pool, &role.id, session.project_key.as_deref()).await?;
    let god_mode = crate::god_mode::active_grant(pool, session.id).await?;
    let role_authority_delta: Option<Value> = sqlx::query_scalar(
        r#"
        SELECT payload
        FROM event_stream
        WHERE session_id=$1
          AND event_type='role_authority.changed'
          AND payload->>'newRoleVersionId'=$2
        ORDER BY sequence DESC
        LIMIT 1
        "#,
    )
    .bind(session.id)
    .bind(role.role_version_id.to_string())
    .fetch_optional(pool)
    .await?;
    let snapshot = json!({
        "sessionId": session.id,
        "lifecycle": {"status": session.status},
        "project": {"key": project_key, "displayName": project_display, "assignment": if session.project_key.is_some() {"assigned"} else {"unassigned"}},
        "cwd": {"state": if cwd_known {"known"} else {"unavailable"}, "path": if cwd_known {cwd_trimmed} else {"unavailable"}, "source": if cwd_known {"session.workdir"} else {"unavailable"}},
        "worktreeRoot": session.worktree_root,
        "role": {"key": role.id, "version": role.version, "epoch": role_epoch, "roleVersionId": role.role_version_id, "authorityDelta": role_authority_delta},
        "model": role.model_defaults.model,
        "sandbox": {"policy": "role.policy", "approval": "role.policy"},
        "policy": {"summary": "role snapshot policy and runtime command policy are authoritative"},
        "tools": {"commandContextId": command_context.id, "visibleCommandCount": command_context.visible_count, "nativeAffordances": native_affordances, "nativeAffordanceCount": native_affordances.len()},
        "godMode": {
            "state": if god_mode.is_some() { "active" } else { "inactive" },
            "grantId": god_mode.as_ref().map(|grant| grant.id),
            "grantedBy": god_mode.as_ref().map(|grant| grant.granted_by.as_str()),
            "grantedAt": god_mode.as_ref().map(|grant| grant.granted_at.to_rfc3339()),
        },
        "generatedAt": Utc::now().to_rfc3339(),
    });
    let structural_event_kind = match previous.as_ref() {
        None => Some("initial"),
        Some((_, _, prior)) if prior.pointer("/project/key") != snapshot.pointer("/project/key") => Some("project_assignment_changed"),
        Some((_, _, prior)) if prior.pointer("/cwd/path") != snapshot.pointer("/cwd/path") => Some("cwd_changed"),
        Some((_, _, prior)) if prior.pointer("/worktreeRoot") != snapshot.pointer("/worktreeRoot") => Some("worktree_root_changed"),
        Some((_, _, prior)) if prior.pointer("/role/epoch") != snapshot.pointer("/role/epoch") => Some("role_authority_changed"),
        Some((_, _, prior)) if prior.pointer("/godMode/state") != snapshot.pointer("/godMode/state") => Some("god_mode_changed"),
        Some((_, _, prior)) if prior.pointer("/lifecycle/status") != snapshot.pointer("/lifecycle/status") => Some("session_lifecycle_changed"),
        Some((_, _, prior)) if prior.get("model") != snapshot.get("model") => Some("model_changed"),
        Some((_, _, prior)) if prior.pointer("/tools/commandContextId") != snapshot.pointer("/tools/commandContextId") => Some("tool_context_changed"),
        _ => None,
    };
    let pending_count: i64 = if previous.is_some() {
        sqlx::query_scalar("SELECT COUNT(*) FROM session_context_events WHERE session_id=$1 AND sequence > $2")
            .bind(session.id)
            .bind(previous_watermark)
            .fetch_one(pool)
            .await?
    } else {
        0_i64
    };
    if let Some((prior_epoch, prior_watermark, prior_snapshot)) = previous.as_ref()
        && structural_event_kind.is_none()
        && pending_count == 0
    {
        return Ok(ContextSnapshotRecord {
            role_epoch,
            context_epoch: *prior_epoch,
            previous_context_epoch: Some(*prior_epoch),
            context_event_watermark: *prior_watermark,
            event_kind: "unchanged".to_string(),
            event_sequence: *prior_watermark,
            snapshot: prior_snapshot.clone(),
            previous_snapshot: Some(prior_snapshot.clone()),
        });
    }
    if previous.is_some() && structural_event_kind.is_some() && pending_count == 0 {
        sqlx::query(
            "INSERT INTO session_context_events (session_id, turn_id, event_kind, role_epoch, context_epoch, previous_context_epoch, payload) VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(session.id)
        .bind(turn_id)
        .bind(structural_event_kind.expect("checked"))
        .bind(&role_epoch)
        .bind(context_epoch)
        .bind(previous_epoch)
        .bind(json!({"snapshot": snapshot, "previousSnapshot": previous.as_ref().map(|(_, _, value)| value), "previousContextEpoch": previous_epoch}))
        .execute(pool)
        .await?;
    }
    let pending: Vec<(i64, String, Value)> = sqlx::query_as(
        "SELECT sequence, event_kind, payload FROM session_context_events WHERE session_id=$1 AND sequence > $2 ORDER BY sequence ASC",
    )
    .bind(session.id)
    .bind(previous_watermark)
    .fetch_all(pool)
    .await?;
    let (event_sequence, event_kind) = if previous.is_none() {
        let event_sequence: i64 = sqlx::query_scalar(
            "INSERT INTO session_context_events (session_id, turn_id, event_kind, role_epoch, context_epoch, previous_context_epoch, payload) VALUES ($1,$2,'initial',$3,$4,$5,$6) RETURNING sequence",
        )
        .bind(session.id)
        .bind(turn_id)
        .bind(&role_epoch)
        .bind(context_epoch)
        .bind(previous_epoch)
        .bind(json!({"snapshot": snapshot, "previousSnapshot": Value::Null, "previousContextEpoch": previous_epoch}))
        .fetch_one(pool)
        .await?;
        (event_sequence, "initial".to_string())
    } else if let Some((sequence, kind, _)) = pending.last() {
        (*sequence, kind.clone())
    } else {
        unreachable!("unchanged contexts returned early and structural changes create a pending event")
    };
    let watermark = event_sequence;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO session_context_snapshots (id, session_id, turn_id, role_epoch, context_epoch, context_event_watermark, snapshot) VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(id)
    .bind(session.id)
    .bind(turn_id)
    .bind(&role_epoch)
    .bind(context_epoch)
    .bind(watermark)
    .bind(&snapshot)
    .execute(pool)
    .await?;
    Ok(ContextSnapshotRecord {
        role_epoch,
        context_epoch,
        previous_context_epoch: previous_epoch,
        context_event_watermark: watermark,
        event_kind,
        event_sequence,
        snapshot,
        previous_snapshot: previous.map(|(_, _, value)| value),
    })
}

pub fn runtime_developer_messages(snapshot: &ContextSnapshotRecord, command_context: &crate::command_registry::RuntimeCommandContextMessage) -> Vec<RuntimeInputMessage> {
    let mut messages = Vec::new();
    if snapshot.event_kind == "initial" {
        messages.push(RuntimeInputMessage {
            text: runtime_context_block(snapshot),
            metadata: json!({"source": "runtime_context", "roleEpoch": snapshot.role_epoch, "contextEpoch": snapshot.context_epoch, "contextEventWatermark": snapshot.context_event_watermark}),
        });
    }
    if let Some(delta) = context_delta_block(snapshot) {
        messages.push(RuntimeInputMessage {
            text: delta,
            metadata: json!({"source": "context_delta", "kind": snapshot.event_kind, "roleEpoch": snapshot.role_epoch, "contextEpoch": snapshot.context_epoch, "contextEventWatermark": snapshot.context_event_watermark}),
        });
    }
    if let Some(transition) = role_transition_summary_block(snapshot) {
        messages.push(RuntimeInputMessage {
            text: transition,
            metadata: json!({"source": "role_transition_summary", "roleEpoch": snapshot.role_epoch, "contextEpoch": snapshot.context_epoch, "contextEventWatermark": snapshot.context_event_watermark}),
        });
    }
    let native_affordance_count = snapshot
        .snapshot
        .pointer("/tools/nativeAffordanceCount")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let command_context_text = format!(
        "<command_context id=\"{}\" visible_command_count=\"{}\" native_affordance_count=\"{}\">\n{}\nInspect live commands with cmd.describe() or cmd[\"name\"].method.describe() inside execute_code. The full command catalog is not included in this request.\n</command_context>",
        xml_escape(&command_context.evidence.id),
        command_context.evidence.visible_count,
        native_affordance_count,
        command_context.text
    );
    let mut metadata = command_context.metadata.clone();
    metadata["visibleCommandCount"] = json!(command_context.evidence.visible_count);
    metadata["nativeAffordanceCount"] = json!(native_affordance_count);
    metadata["describeGuidance"] = json!("cmd.describe() or cmd[\"name\"].method.describe() inside execute_code");
    messages.push(RuntimeInputMessage { text: command_context_text, metadata });
    messages
}

pub fn model_history_from_items(history: &[HistoryItem]) -> Vec<ModelHistoryItem> {
    history
        .iter()
        .map(|item| ModelHistoryItem {
            session_id: item.session_id.to_string(),
            turn_id: item.turn_id.to_string(),
            user: item.user.clone(),
            assistant: item.assistant.clone(),
            started_at: item.started_at.to_rfc3339(),
            source: item.source.clone(),
            checkpoint_id: item.checkpoint_id.map(|id| id.to_string()),
        })
        .collect()
}

pub fn responses_input(
    role: &RoleSnapshot,
    history: &[ModelHistoryItem],
    runtime_messages: &[RuntimeInputMessage],
    current_message: Option<&str>,
) -> Vec<Value> {
    let mut input = vec![json!({
        "role": "developer",
        "content": [{"type": "input_text", "text": role_instructions_block(role)}],
        "metadata": {"source": "role_instructions", "roleEpoch": role_epoch(role)}
    })];
    for runtime_message in runtime_messages {
        let mut content = vec![json!({"type": "input_text", "text": runtime_message.text})];
        if let Some(attachments) = runtime_message.metadata.get("imageArtifactAttachments").and_then(Value::as_array) {
            for attachment in attachments {
                content.push(json!({
                    "type": "input_image",
                    "artifact_id": attachment.get("imageArtifactId").cloned().unwrap_or(Value::Null),
                    "mime_type": attachment.get("mimeType").cloned().unwrap_or(Value::Null),
                    "detail": attachment.get("detail").cloned().unwrap_or_else(|| json!("auto")),
                    "source": attachment.get("source").cloned().unwrap_or_else(|| json!("artifact_store")),
                }));
            }
        }
        input.push(json!({
            "role": "developer",
            "content": content,
            "metadata": runtime_message.metadata,
        }));
    }
    input.extend(normalized_history(history));
    if let Some(message) = current_message {
        input.push(json!({"role": "user", "content": [{"type": "input_text", "text": message}]}));
    }
    input
}

fn normalized_history(history: &[ModelHistoryItem]) -> Vec<Value> {
    let mut input = Vec::new();
    for item in history {
        if !item.user.contains("<role_instructions") && !item.user.contains("<runtime_context") {
            input.push(json!({
                "role": "user",
                "content": [{"type": "input_text", "text": item.user}],
                "metadata": {"sessionId": item.session_id, "turnId": item.turn_id, "startedAt": item.started_at, "source": item.source, "checkpointId": item.checkpoint_id}
            }));
        }
        if let Some(assistant) = &item.assistant && !assistant.trim().is_empty() {
            input.push(json!({
                "role": "assistant",
                "content": [{"type": "output_text", "text": assistant}],
                "metadata": {"sessionId": item.session_id, "turnId": item.turn_id, "startedAt": item.started_at, "source": item.source, "checkpointId": item.checkpoint_id}
            }));
        }
    }
    input
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::{LifecycleAuthorityMetadata, ManifestDecision, ModelDefaults, RoutingMetadata, VisibilityMetadata};
    use std::collections::BTreeMap;

    fn role(instruction_text: &str) -> RoleSnapshot {
        RoleSnapshot {
            id: "operator".to_string(),
            version: "2.0.0".to_string(),
            display_name: "Operator".to_string(),
            role_version_id: Uuid::new_v4(),
            instruction_text: instruction_text.to_string(),
            model_defaults: ModelDefaults { model: "gpt-test".to_string(), reasoning_effort: "medium".to_string() },
            capabilities: vec!["tool.execute_code".to_string()],
            policy: BTreeMap::from([("tool.execute_code".to_string(), ManifestDecision::Allow)]),
            routing: RoutingMetadata { mode: "direct".to_string(), default_recipient: None, allowed_recipients: vec![], reserved_actions: vec![] },
            visibility: VisibilityMetadata { listed: true, owner_visible: true },
            lifecycle_authority: LifecycleAuthorityMetadata { can_spawn_agents: false, can_archive_agents: false, reserved_actions: vec![] },
            manifest: json!({}),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn role_and_runtime_context_are_developer_messages_before_user() {
        let role = role("Answer directly from runtime context when possible.");
        let runtime = RuntimeInputMessage {
            text: "<runtime_context epoch=\"1\"><cwd state=\"known\" source=\"session.workdir\">/tmp/project</cwd></runtime_context>".to_string(),
            metadata: json!({"source":"runtime_context","contextEpoch":1}),
        };
        let input = responses_input(&role, &[], &[runtime], Some("what is my cwd?"));
        assert_eq!(input[0]["role"], "developer");
        assert!(input[0]["content"][0]["text"].as_str().unwrap().contains("<role_instructions"));
        assert_eq!(input[1]["role"], "developer");
        assert!(input[1]["content"][0]["text"].as_str().unwrap().contains("<runtime_context"));
        assert_eq!(input[2]["role"], "user");
    }

    #[test]
    fn stale_visible_role_and_runtime_blocks_are_not_replayed_from_history() {
        let role = role("new role instructions");
        let history = vec![ModelHistoryItem {
            session_id: Uuid::new_v4().to_string(),
            turn_id: Uuid::new_v4().to_string(),
            user: "<role_instructions epoch=\"old\">old role</role_instructions>".to_string(),
            assistant: Some("assistant answer".to_string()),
            started_at: Utc::now().to_rfc3339(),
            source: "compaction_checkpoint".to_string(),
            checkpoint_id: None,
        }];
        let input = responses_input(&role, &history, &[], Some("continue"));
        let rendered = serde_json::to_string(&input).unwrap();
        assert!(rendered.contains("new role instructions"));
        assert!(!rendered.contains("old role"));
        assert!(rendered.contains("assistant answer"));
    }

    #[test]
    fn unavailable_cwd_context_is_explicit_and_not_guessed() {
        let snapshot = ContextSnapshotRecord {
            role_epoch: "operator:2.0.0:test".to_string(),
            context_epoch: 7,
            previous_context_epoch: Some(6),
            context_event_watermark: 9,
            event_kind: "snapshot_refreshed".to_string(),
            event_sequence: 9,
            snapshot: json!({
                "sessionId": Uuid::new_v4(),
                "project": {"key":"unassigned","displayName":"Unassigned"},
                "cwd": {"state":"unavailable","source":"unavailable","path":"unavailable"},
                "role": {"key":"operator","version":"2.0.0"},
                "model": "gpt-test",
                "sandbox": {"policy": "role.policy", "approval": "role.policy"},
                "policy": {"summary": "role snapshot policy"},
                "tools": {"commandContextId":"cmdctx-empty","visibleCommandCount":0,"nativeAffordanceCount":3},
                "generatedAt": "2026-06-19T00:00:00Z"
            }),
            previous_snapshot: None,
        };
        let block = runtime_context_block(&snapshot);
        assert!(block.contains("state=\"unavailable\""));
        assert!(block.contains(">unavailable</cwd>"));
        assert!(block.contains("native_affordance_count=\"3\""));
    }

    #[test]
    fn bounded_context_delta_reports_cwd_change_schema() {
        let snapshot = ContextSnapshotRecord {
            role_epoch: "operator:2.0.0:test".to_string(),
            context_epoch: 2,
            previous_context_epoch: Some(1),
            context_event_watermark: 12,
            event_kind: "cwd_changed".to_string(),
            event_sequence: 12,
            snapshot: json!({
                "cwd": {"path": "/tmp/new", "source": "session.workdir"},
                "tools": {"commandContextId":"a","visibleCommandCount":1}
            }),
            previous_snapshot: Some(json!({
                "cwd": {"path": "/tmp/old", "source": "session.workdir"},
                "tools": {"commandContextId":"a","visibleCommandCount":1}
            })),
        };
        let delta = context_delta_block(&snapshot).expect("delta");
        assert!(delta.contains("<context_delta epoch=\"2\" previous_epoch=\"1\">"));
        assert!(delta.contains("kind=\"cwd_changed\""));
        assert!(delta.contains("old_cwd=/tmp/old"));
        assert!(delta.contains("new_cwd=/tmp/new"));
        assert!(delta.contains("sequence=12"));
    }

    #[test]
    fn role_epoch_change_is_transition_summary_not_context_delta() {
        let snapshot = ContextSnapshotRecord {
            role_epoch: "operator:2.0.1:new".to_string(),
            context_epoch: 4,
            previous_context_epoch: Some(3),
            context_event_watermark: 14,
            event_kind: "role_authority_changed".to_string(),
            event_sequence: 14,
            snapshot: json!({"role": {"epoch": "operator:2.0.1:new"}}),
            previous_snapshot: Some(json!({"role": {"epoch": "operator:2.0.0:old"}})),
        };
        assert!(context_delta_block(&snapshot).is_none());
        let transition = role_transition_summary_block(&snapshot).expect("transition");
        assert!(transition.contains("<role_transition_summary"));
        assert!(transition.contains("Prior role instructions and policy text are not active instructions"));
        assert!(transition.contains("concise authority delta"));
    }

    #[test]
    fn context_delta_covers_project_worktree_and_model_change_kinds() {
        for (kind, expected) in [
            ("project_assignment_changed", "old_project=old new_project=new"),
            ("worktree_root_changed", "old_worktree_root=/old-root new_worktree_root=/new-root"),
            ("model_changed", "old_model=gpt-old new_model=gpt-new"),
        ] {
            let snapshot = ContextSnapshotRecord {
                role_epoch: "operator:2.0.0:test".to_string(),
                context_epoch: 3,
                previous_context_epoch: Some(2),
                context_event_watermark: 15,
                event_kind: kind.to_string(),
                event_sequence: 15,
                snapshot: json!({
                    "project": {"key": "new"},
                    "worktreeRoot": "/new-root",
                    "model": "gpt-new",
                }),
                previous_snapshot: Some(json!({
                    "project": {"key": "old"},
                    "worktreeRoot": "/old-root",
                    "model": "gpt-old",
                })),
            };
            let delta = context_delta_block(&snapshot).expect("delta");
            assert!(delta.contains(expected), "{delta}");
            assert!(delta.contains("sequence=15"), "{delta}");
        }
    }

    #[test]
    fn runtime_messages_do_not_reinsert_full_context_on_snapshot_refresh() {
        let snapshot = ContextSnapshotRecord {
            role_epoch: "operator:2.0.0:test".to_string(),
            context_epoch: 2,
            previous_context_epoch: Some(1),
            context_event_watermark: 13,
            event_kind: "unchanged".to_string(),
            event_sequence: 13,
            snapshot: json!({"role": {"epoch": "operator:2.0.0:test"}}),
            previous_snapshot: Some(json!({"role": {"epoch": "operator:2.0.0:test"}})),
        };
        let command_context = crate::command_registry::RuntimeCommandContextMessage {
            text: "Runtime command context unchanged: cmdctx-test.".to_string(),
            metadata: json!({"source": "runtime_command_context", "commandContextId": "cmdctx-test", "visibleCommandCount": 0, "catalogIncluded": false}),
            evidence: CommandContextEvidence {
                id: "cmdctx-test".to_string(),
                catalog_included: false,
                visible_count: 0,
                added_count: 0,
                removed_count: 0,
                changed_count: 0,
                summaries: vec![],
            },
        };
        let messages = runtime_developer_messages(&snapshot, &command_context);
        let rendered = serde_json::to_string(&messages).unwrap();
        assert!(!rendered.contains("<runtime_context"), "{rendered}");
        assert!(rendered.contains("<command_context id=\\\"cmdctx-test\\\" visible_command_count=\\\"0\\\" native_affordance_count=\\\"0\\\""), "{rendered}");
        assert!(rendered.contains("cmd.describe()"), "{rendered}");
        assert!(rendered.contains("cmd[\\\"name\\\"].method.describe()"), "{rendered}");
        assert!(rendered.contains("The full command catalog is not included"), "{rendered}");
        assert!(rendered.contains("nativeAffordanceCount"), "{rendered}");
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn god_mode_delta_is_concise_context_delta() {
        let snapshot = ContextSnapshotRecord {
            role_epoch: "operator:2.0.0:test".to_string(),
            context_epoch: 2,
            previous_context_epoch: Some(1),
            context_event_watermark: 14,
            event_kind: "god_mode_changed".to_string(),
            event_sequence: 14,
            snapshot: json!({"godMode": {"state": "active"}}),
            previous_snapshot: Some(json!({"godMode": {"state": "inactive"}})),
        };
        let delta = context_delta_block(&snapshot).expect("delta");
        assert!(delta.contains("kind=\"god_mode_changed\""));
        assert!(delta.contains("old_god_mode=inactive"));
        assert!(delta.contains("new_god_mode=active"));
        assert!(!delta.contains("<runtime_context"));
    }
}
