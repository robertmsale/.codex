use std::{collections::BTreeSet, path::PathBuf};

use anyhow::{Context, Result, bail};

use crate::models::{
    BridgeAgentSummary, RobdexChatMessage, ScopedAgentRecord, ThreadCachePayload,
};

pub fn resolve_role_instructions(home_directory: Option<PathBuf>, role: Option<&str>) -> Result<Option<String>> {
    let Some(role) = role.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let Some(home_directory) = home_directory else {
        return Ok(None);
    };

    let path = home_directory.join(".codex/roles").join(format!("{role}.md"));
    let contents =
        std::fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        bail!("Empty {role} role instructions at {}.", path.display());
    }
    Ok(Some(trimmed.to_string()))
}

pub fn merge_delta_text(existing: &str, incoming: &str) -> String {
    if incoming.is_empty() {
        return existing.to_string();
    }
    if incoming.starts_with(existing) {
        return incoming.to_string();
    }
    if existing.ends_with(incoming) {
        return existing.to_string();
    }

    let incoming_boundaries = incoming
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(incoming.len()))
        .collect::<Vec<_>>();
    let max_overlap = existing.len().min(incoming.len());
    for overlap in incoming_boundaries.into_iter().rev() {
        if overlap == 0 || overlap > max_overlap {
            continue;
        }
        if existing.ends_with(&incoming[..overlap]) {
            return format!("{existing}{}", &incoming[overlap..]);
        }
    }
    format!("{existing}{incoming}")
}

pub fn prune_thread_cache_payload(mut payload: ThreadCachePayload, thread_id: &str) -> PruneResult {
    let mut changed = false;
    changed |= payload.message_cache_by_thread_id.remove(thread_id).is_some();
    changed |= payload.context_window_status_by_thread_id.remove(thread_id).is_some();

    let original_running = payload.running_thread_ids.len();
    payload.running_thread_ids.retain(|entry| entry != thread_id);
    changed |= payload.running_thread_ids.len() != original_running;

    PruneResult {
        payload,
        changed,
        pruned_thread_ids: vec![thread_id.to_string()],
    }
}

pub fn prune_orphaned_thread_cache_payload(
    mut payload: ThreadCachePayload,
    live_thread_ids: &[String],
) -> PruneResult {
    let live: BTreeSet<&str> = live_thread_ids.iter().map(String::as_str).collect();
    let mut pruned = Vec::new();

    for thread_id in payload
        .message_cache_by_thread_id
        .keys()
        .filter(|thread_id| !live.contains(thread_id.as_str()))
        .cloned()
        .collect::<Vec<_>>()
    {
        payload.message_cache_by_thread_id.remove(&thread_id);
        pruned.push(thread_id);
    }

    for thread_id in payload
        .context_window_status_by_thread_id
        .keys()
        .filter(|thread_id| !live.contains(thread_id.as_str()))
        .cloned()
        .collect::<Vec<_>>()
    {
        payload.context_window_status_by_thread_id.remove(&thread_id);
        if !pruned.iter().any(|entry| entry == &thread_id) {
            pruned.push(thread_id);
        }
    }

    let running_before = payload.running_thread_ids.len();
    payload
        .running_thread_ids
        .retain(|thread_id| live.contains(thread_id.as_str()));
    let changed = !pruned.is_empty() || payload.running_thread_ids.len() != running_before;

    PruneResult {
        payload,
        changed,
        pruned_thread_ids: pruned,
    }
}

pub fn summarize_scoped_agent_record(record: &ScopedAgentRecord, instance_id: &str) -> BridgeAgentSummary {
    let role = if record.is_hidden {
        "hidden"
    } else if record.is_orchestrator {
        "orchestrator"
    } else {
        match record.role.as_str() {
            "operator" => "operator",
            "qa" => "qa",
            "worker" => "worker",
            other => other,
        }
    };

    BridgeAgentSummary {
        id: record.thread_id.clone(),
        instance_id: instance_id.to_string(),
        thread_id: Some(record.thread_id.clone()),
        parent_agent_id: None,
        display_name: record.display_name.clone().unwrap_or_else(|| record.thread_id.clone()),
        role: role.to_string(),
        status: if record.is_running { "running" } else { "idle" }.to_string(),
        project_path: record.project_path.clone(),
        cwd: record.cwd.clone(),
        last_event: Some(if record.is_running { "Running" } else { "Idle" }.to_string()),
        updated_at: record.updated_at,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PruneResult {
    pub payload: ThreadCachePayload,
    pub changed: bool,
    pub pruned_thread_ids: Vec<String>,
}

pub fn sample_message(thread_id: &str, id: &str) -> RobdexChatMessage {
    RobdexChatMessage {
        id: id.to_string(),
        thread_id: thread_id.to_string(),
        turn_id: None,
        role: "assistant".to_string(),
        text: "hello".to_string(),
        phase: None,
        created_at: 1,
        subtitle: None,
        tool_metadata: None,
        delivery_state: "confirmed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ThreadContextWindowStatus;
    use tempfile::TempDir;

    #[test]
    fn resolve_role_instructions_returns_none_without_role() {
        let instructions = resolve_role_instructions(None, None).expect("instructions");
        assert_eq!(instructions, None);
    }

    #[test]
    fn resolve_role_instructions_reads_temp_role_file() {
        let temp = TempDir::new().expect("tempdir");
        let roles = temp.path().join(".codex/roles");
        std::fs::create_dir_all(&roles).expect("roles dir");
        std::fs::write(roles.join("hidden.md"), "hidden role instructions\n").expect("role write");

        let instructions = resolve_role_instructions(Some(temp.path().to_path_buf()), Some("hidden"))
            .expect("instructions");
        assert_eq!(instructions.as_deref(), Some("hidden role instructions"));
    }

    #[test]
    fn merge_delta_text_avoids_duplicate_overlap() {
        assert_eq!(merge_delta_text("hello wor", "world"), "hello world");
        assert_eq!(merge_delta_text("hello", "hello"), "hello");
        assert_eq!(merge_delta_text("hello", "hello world"), "hello world");
    }

    #[test]
    fn merge_delta_text_handles_unicode_boundaries() {
        assert_eq!(merge_delta_text("I", "I’m"), "I’m");
        assert_eq!(merge_delta_text("that", "’s fine"), "that’s fine");
    }

    #[test]
    fn prune_thread_cache_payload_removes_requested_thread() {
        let mut payload = ThreadCachePayload::default();
        payload
            .message_cache_by_thread_id
            .insert("thr-a".to_string(), vec![sample_message("thr-a", "msg-1")]);
        payload
            .message_cache_by_thread_id
            .insert("thr-b".to_string(), vec![sample_message("thr-b", "msg-2")]);
        payload.context_window_status_by_thread_id.insert(
            "thr-a".to_string(),
            ThreadContextWindowStatus {
                remaining_percent: 50,
                used_tokens_in_context_window: 100,
                model_context_window: Some(200),
            },
        );
        payload.running_thread_ids = vec!["thr-a".to_string(), "thr-b".to_string()];

        let result = prune_thread_cache_payload(payload, "thr-a");
        assert!(result.changed);
        assert_eq!(
            result
                .payload
                .message_cache_by_thread_id
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["thr-b".to_string()]
        );
        assert_eq!(result.payload.running_thread_ids, vec!["thr-b".to_string()]);
    }

    #[test]
    fn prune_orphaned_thread_cache_payload_removes_only_orphans() {
        let mut payload = ThreadCachePayload::default();
        payload
            .message_cache_by_thread_id
            .insert("thr-live".to_string(), vec![sample_message("thr-live", "msg-1")]);
        payload
            .message_cache_by_thread_id
            .insert("thr-orphan".to_string(), vec![sample_message("thr-orphan", "msg-2")]);
        payload.context_window_status_by_thread_id.insert(
            "thr-orphan".to_string(),
            ThreadContextWindowStatus {
                remaining_percent: 20,
                used_tokens_in_context_window: 10,
                model_context_window: Some(100),
            },
        );
        payload.running_thread_ids = vec!["thr-live".to_string(), "thr-orphan".to_string()];

        let result = prune_orphaned_thread_cache_payload(payload, &[String::from("thr-live")]);
        assert!(result.changed);
        assert_eq!(result.pruned_thread_ids, vec!["thr-orphan".to_string()]);
        assert_eq!(
            result
                .payload
                .message_cache_by_thread_id
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["thr-live".to_string()]
        );
    }

    #[test]
    fn summarize_scoped_agent_record_keeps_hidden_agents_hidden() {
        let summary = summarize_scoped_agent_record(
            &ScopedAgentRecord {
                thread_id: "thr-hidden".to_string(),
                display_name: Some("Hidden QA".to_string()),
                project_path: "/tmp/project".to_string(),
                cwd: "/tmp/project".to_string(),
                role: "qa".to_string(),
                is_orchestrator: false,
                is_running: true,
                is_archived: false,
                is_hidden: true,
                updated_at: 123,
            },
            "instance-1",
        );

        assert_eq!(summary.role, "hidden");
        assert_eq!(summary.status, "running");
        assert_eq!(summary.display_name, "Hidden QA");
    }
}
