use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::command_registry::CommandContextEvidence;
use crate::db::{HistoryItem, SessionSummary};
use crate::model::{ModelHistoryItem, RuntimeInputMessage};
use crate::roles::RoleSnapshot;

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
    let sandbox_policy = s.pointer("/sandbox/policy").and_then(Value::as_str).unwrap_or("unknown");
    let approval_policy = s.pointer("/sandbox/approval").and_then(Value::as_str).unwrap_or("unknown");
    let policy_summary = s.pointer("/policy/summary").and_then(Value::as_str).unwrap_or("unknown");
    let generated_at = s.get("generatedAt").and_then(Value::as_str).unwrap_or("unknown");
    format!(
        "<runtime_context epoch=\"{}\">\n  <session_id>{}</session_id>\n  <project key=\"{}\">{}</project>\n  <cwd state=\"{}\" source=\"{}\">{}</cwd>\n  <role epoch=\"{}\" key=\"{}\" version=\"{}\" />\n  <model>{}</model>\n  <sandbox policy=\"{}\" approval=\"{}\">{}</sandbox>\n  <tools command_context_id=\"{}\" visible_command_count=\"{}\" />\n  <context_event_watermark>{}</context_event_watermark>\n  <sequence>{}</sequence>\n  <generated_at>{}</generated_at>\n</runtime_context>",
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
        snapshot.context_event_watermark,
        snapshot.event_sequence,
        xml_escape(generated_at)
    )
}

pub fn context_delta_block(snapshot: &ContextSnapshotRecord) -> Option<String> {
    if matches!(snapshot.event_kind.as_str(), "initial" | "snapshot_refreshed" | "role_epoch_changed") {
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
    if snapshot.event_kind != "role_epoch_changed" {
        return None;
    }
    let previous = snapshot.previous_snapshot.as_ref().and_then(|value| value.pointer("/role/epoch")).and_then(Value::as_str).unwrap_or("unknown");
    Some(format!(
        "<role_transition_summary epoch=\"{}\" previous_epoch=\"{}\" sequence=\"{}\">\nRole authority changed. Prior role instructions and policy text are not active instructions. Use only the current role_instructions block and current runtime_context block as active authority.\n</role_transition_summary>",
        xml_escape(&snapshot.role_epoch),
        xml_escape(previous),
        snapshot.event_sequence
    ))
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
    let previous: Option<(i64, Value)> = sqlx::query_as(
        "SELECT context_epoch, snapshot FROM session_context_snapshots WHERE session_id=$1 ORDER BY context_epoch DESC LIMIT 1",
    )
    .bind(session.id)
    .fetch_optional(pool)
    .await?;
    let previous_epoch = previous.as_ref().map(|(epoch, _)| *epoch);
    let context_epoch = previous_epoch.unwrap_or(0) + 1;
    let role_epoch = role_epoch(role);
    let (project_key, project_display) = project_label(pool, session.project_key.as_deref()).await?;
    let cwd_trimmed = session.workdir.trim();
    let cwd_known = !cwd_trimmed.is_empty();
    let snapshot = json!({
        "sessionId": session.id,
        "project": {"key": project_key, "displayName": project_display, "assignment": if session.project_key.is_some() {"assigned"} else {"unassigned"}},
        "cwd": {"state": if cwd_known {"known"} else {"unavailable"}, "path": if cwd_known {cwd_trimmed} else {"unavailable"}, "source": if cwd_known {"session.workdir"} else {"unavailable"}},
        "worktreeRoot": session.worktree_root,
        "role": {"key": role.id, "version": role.version, "epoch": role_epoch, "roleVersionId": role.role_version_id},
        "model": role.model_defaults.model,
        "sandbox": {"policy": "role.policy", "approval": "role.policy"},
        "policy": {"summary": "role snapshot policy and runtime command policy are authoritative"},
        "tools": {"commandContextId": command_context.id, "visibleCommandCount": command_context.visible_count},
        "generatedAt": Utc::now().to_rfc3339(),
    });
    let event_kind = match previous.as_ref() {
        None => "initial",
        Some((_, prior)) if prior.pointer("/project/key") != snapshot.pointer("/project/key") => "project_assignment_changed",
        Some((_, prior)) if prior.pointer("/cwd/path") != snapshot.pointer("/cwd/path") => "cwd_changed",
        Some((_, prior)) if prior.pointer("/worktreeRoot") != snapshot.pointer("/worktreeRoot") => "worktree_root_changed",
        Some((_, prior)) if prior.get("model") != snapshot.get("model") => "model_changed",
        Some((_, prior)) if prior.pointer("/tools/commandContextId") != snapshot.pointer("/tools/commandContextId") => "tool_context_changed",
        Some((_, prior)) if prior.pointer("/role/epoch") != snapshot.pointer("/role/epoch") => "role_epoch_changed",
        _ => "snapshot_refreshed",
    };
    let event_sequence: i64 = sqlx::query_scalar(
        "INSERT INTO session_context_events (session_id, turn_id, event_kind, role_epoch, context_epoch, previous_context_epoch, payload) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING sequence",
    )
    .bind(session.id)
    .bind(turn_id)
    .bind(event_kind)
    .bind(&role_epoch)
    .bind(context_epoch)
    .bind(previous_epoch)
    .bind(json!({"snapshot": snapshot, "previousSnapshot": previous.as_ref().map(|(_, value)| value), "previousContextEpoch": previous_epoch}))
    .fetch_one(pool)
    .await?;
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
        event_kind: event_kind.to_string(),
        event_sequence,
        snapshot,
        previous_snapshot: previous.map(|(_, value)| value),
    })
}

pub fn runtime_developer_messages(snapshot: &ContextSnapshotRecord, command_context: &crate::command_registry::RuntimeCommandContextMessage) -> Vec<RuntimeInputMessage> {
    let mut messages = vec![
        RuntimeInputMessage {
            text: runtime_context_block(snapshot),
            metadata: json!({"source": "runtime_context", "roleEpoch": snapshot.role_epoch, "contextEpoch": snapshot.context_epoch, "contextEventWatermark": snapshot.context_event_watermark}),
        },
    ];
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
    messages.push(RuntimeInputMessage {
        text: command_context.text.clone(),
        metadata: command_context.metadata.clone(),
    });
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
        input.push(json!({
            "role": "developer",
            "content": [{"type": "input_text", "text": runtime_message.text}],
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
                "tools": {"commandContextId":"cmdctx-empty","visibleCommandCount":0},
                "generatedAt": "2026-06-19T00:00:00Z"
            }),
            previous_snapshot: None,
        };
        let block = runtime_context_block(&snapshot);
        assert!(block.contains("state=\"unavailable\""));
        assert!(block.contains(">unavailable</cwd>"));
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
            event_kind: "role_epoch_changed".to_string(),
            event_sequence: 14,
            snapshot: json!({"role": {"epoch": "operator:2.0.1:new"}}),
            previous_snapshot: Some(json!({"role": {"epoch": "operator:2.0.0:old"}})),
        };
        assert!(context_delta_block(&snapshot).is_none());
        let transition = role_transition_summary_block(&snapshot).expect("transition");
        assert!(transition.contains("<role_transition_summary"));
        assert!(transition.contains("Prior role instructions and policy text are not active instructions"));
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
}
