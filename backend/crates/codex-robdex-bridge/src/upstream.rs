use std::collections::{BTreeMap, BTreeSet};

use base64::{Engine as _, engine::general_purpose};
use codex_app_server_adapter::app_server_protocol::{
    AgentMessageDeltaNotification, CommandExecutionOutputDeltaNotification, CommandExecutionStatus,
    FileChangeOutputDeltaNotification, FileUpdateChange, ItemCompletedNotification,
    ItemStartedNotification, McpToolCallStatus, ModelReroutedNotification, PatchApplyStatus,
    PlanDeltaNotification, RawResponseItemCompletedNotification, ReasoningSummaryPartAddedNotification,
    ReasoningSummaryTextDeltaNotification, ReasoningTextDeltaNotification, ServerNotification, ServerRequest,
    TerminalInteractionNotification, Thread, ThreadActiveFlag, ThreadClosedNotification, ThreadItem,
    ThreadStartedNotification, ThreadStatus, ThreadStatusChangedNotification, ThreadTokenUsageUpdatedNotification,
    Turn, TurnCompletedNotification, TurnDiffUpdatedNotification, TurnPlanStepStatus, TurnPlanUpdatedNotification,
    TurnStartedNotification, TurnStatus,
};
use codex_app_server_adapter::protocol::models::{ContentItem, MessagePhase, ResponseItem};
use serde_json::Value;

use crate::{
    models::{
        BRIDGE_TRUNCATION_MARKER, MAX_TOOL_OUTPUT_CHARS, MAX_TRANSPORT_THREAD_MESSAGES_BYTES,
        RobdexChatMessage, RobdexToolMetadata, ThreadCachePayload, ThreadContextWindowStatus,
    },
    transforms::merge_delta_text,
};

const CONTEXT_WINDOW_BASELINE_TOKENS: i64 = 12_000;
const MAX_MODEL_VISIBLE_COMMENTARY_CHARS: usize = 4_000;
const MAX_REQUIREMENTS_SUMMARY_CHARS: usize = 1_000;

#[derive(Debug, Clone)]
pub enum UpstreamRuntimeEvent {
    ConnectionStatus(String),
    Notification(ServerNotification),
    ServerRequest(ServerRequest),
    ClearRunningStateAfterDisconnect,
    FlushPendingThreadCacheWrites,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamApplyResult {
    pub thread_cache_changed: bool,
    pub changed_thread_ids: Vec<String>,
    pub running_state_changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpsertMode {
    Merge,
    Replace,
}

#[derive(Debug, Default)]
pub struct RunningStateReducer {
    active_turn_ids_by_thread: BTreeMap<String, BTreeSet<String>>,
}

impl RunningStateReducer {
    pub fn active_turn_id_for_thread(&self, thread_id: &str) -> Option<String> {
        self.active_turn_ids_by_thread
            .get(thread_id)
            .and_then(|turns| turns.iter().next().cloned())
    }

    pub fn set_thread_running_state(
        &mut self,
        thread_id: &str,
        is_running: bool,
        thread_cache: &mut ThreadCachePayload,
    ) -> bool {
        if !is_running {
            self.active_turn_ids_by_thread.remove(thread_id);
        }
        self.set_running_state(thread_id, is_running, thread_cache)
    }

    pub fn set_thread_active_turn_state(
        &mut self,
        thread_id: &str,
        active_turn_id: Option<String>,
        thread_cache: &mut ThreadCachePayload,
    ) -> bool {
        match active_turn_id {
            Some(turn_id) => {
                self.active_turn_ids_by_thread
                    .entry(thread_id.to_string())
                    .or_default()
                    .insert(turn_id);
                self.set_running_state(thread_id, true, thread_cache)
            }
            None => self.set_thread_running_state(thread_id, false, thread_cache),
        }
    }

    pub fn clear_running_state(&mut self, thread_cache: &mut ThreadCachePayload) -> bool {
        let had_running = !thread_cache.running_thread_ids.is_empty();
        self.active_turn_ids_by_thread.clear();
        thread_cache.running_thread_ids.clear();
        had_running
    }

    pub fn apply_notification(
        &mut self,
        notification: &ServerNotification,
        thread_cache: &mut ThreadCachePayload,
    ) -> UpstreamApplyResult {
        let mut changed_thread_ids = BTreeSet::new();
        let mut changed = false;
        let running_changed = self.apply_running_notification(notification, thread_cache);
        changed |= running_changed;
        changed |= self.apply_message_notification(notification, thread_cache, &mut changed_thread_ids);
        changed |= self.apply_context_window_notification(notification, thread_cache, &mut changed_thread_ids);

        UpstreamApplyResult {
            thread_cache_changed: changed,
            changed_thread_ids: changed_thread_ids.into_iter().collect(),
            running_state_changed: running_changed,
        }
    }

    fn apply_running_notification(
        &mut self,
        notification: &ServerNotification,
        thread_cache: &mut ThreadCachePayload,
    ) -> bool {
        match notification {
            ServerNotification::ThreadStarted(payload) => self.apply_thread_started(payload, thread_cache),
            ServerNotification::ThreadStatusChanged(payload) => {
                self.apply_thread_status_changed(payload, thread_cache)
            }
            ServerNotification::TurnStarted(payload) => self.apply_turn_started(payload, thread_cache),
            ServerNotification::TurnCompleted(payload) => self.apply_turn_completed(payload, thread_cache),
            ServerNotification::ThreadClosed(payload) => self.apply_thread_closed(payload, thread_cache),
            _ => false,
        }
    }

    fn apply_message_notification(
        &mut self,
        notification: &ServerNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        match notification {
            ServerNotification::TurnStarted(payload) => {
                self.apply_turn_items(
                    &payload.thread_id,
                    &payload.turn.id,
                    &payload.turn.items,
                    UpsertMode::Merge,
                    thread_cache,
                    changed_thread_ids,
                )
            }
            ServerNotification::TurnCompleted(payload) => {
                self.apply_turn_items(
                    &payload.thread_id,
                    &payload.turn.id,
                    &payload.turn.items,
                    UpsertMode::Replace,
                    thread_cache,
                    changed_thread_ids,
                )
            }
            ServerNotification::ItemStarted(payload) => {
                self.apply_item_notification(payload, UpsertMode::Merge, thread_cache, changed_thread_ids)
            }
            ServerNotification::ItemCompleted(payload) => {
                self.apply_item_notification(payload, UpsertMode::Replace, thread_cache, changed_thread_ids)
            }
            ServerNotification::AgentMessageDelta(payload) => {
                self.apply_agent_message_delta(payload, thread_cache, changed_thread_ids)
            }
            ServerNotification::RawResponseItemCompleted(payload) => {
                self.apply_raw_response_item_completed(payload, thread_cache, changed_thread_ids)
            }
            ServerNotification::PlanDelta(payload) => {
                self.apply_plan_delta(payload, thread_cache, changed_thread_ids)
            }
            ServerNotification::ReasoningSummaryTextDelta(payload) => {
                self.apply_reasoning_summary_text_delta(payload, thread_cache, changed_thread_ids)
            }
            ServerNotification::ReasoningSummaryPartAdded(payload) => {
                self.apply_reasoning_summary_part_added(payload, thread_cache, changed_thread_ids)
            }
            ServerNotification::ReasoningTextDelta(payload) => {
                self.apply_reasoning_text_delta(payload, thread_cache, changed_thread_ids)
            }
            ServerNotification::TerminalInteraction(payload) => {
                self.apply_terminal_interaction(payload, thread_cache, changed_thread_ids)
            }
            ServerNotification::CommandExecutionOutputDelta(payload) => {
                self.apply_command_execution_output_delta(payload, thread_cache, changed_thread_ids)
            }
            ServerNotification::FileChangeOutputDelta(payload) => {
                self.apply_file_change_output_delta(payload, thread_cache, changed_thread_ids)
            }
            ServerNotification::TurnPlanUpdated(payload) => {
                self.apply_turn_plan_updated(payload, thread_cache, changed_thread_ids)
            }
            ServerNotification::TurnDiffUpdated(payload) => {
                self.apply_turn_diff_updated(payload, thread_cache, changed_thread_ids)
            }
            ServerNotification::ContextCompacted(_payload) => false,
            ServerNotification::ModelRerouted(payload) => {
                self.apply_model_rerouted(payload, thread_cache, changed_thread_ids)
            }
            _ => false,
        }
    }

    fn apply_turn_items(
        &self,
        thread_id: &str,
        turn_id: &str,
        items: &[ThreadItem],
        mode: UpsertMode,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        let mut changed = false;
        for item in items {
            let Some(message) = message_from_item(item, thread_id, Some(turn_id), mode) else {
                continue;
            };
            changed |= upsert_message(thread_cache, thread_id, message, mode, changed_thread_ids);
        }
        changed
    }

    fn apply_context_window_notification(
        &mut self,
        notification: &ServerNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        let ServerNotification::ThreadTokenUsageUpdated(payload) = notification else {
            return false;
        };

        let fallback_window = thread_cache
            .context_window_status_by_thread_id
            .get(&payload.thread_id)
            .and_then(|status| status.model_context_window);
        let Some(next_status) = context_window_status_from_token_usage(payload, fallback_window) else {
            return false;
        };

        let changed = thread_cache
            .context_window_status_by_thread_id
            .get(&payload.thread_id)
            .map(|existing| existing != &next_status)
            .unwrap_or(true);
        if changed {
            thread_cache
                .context_window_status_by_thread_id
                .insert(payload.thread_id.clone(), next_status);
            changed_thread_ids.insert(payload.thread_id.clone());
        }
        changed
    }

    fn apply_thread_started(
        &mut self,
        payload: &ThreadStartedNotification,
        thread_cache: &mut ThreadCachePayload,
    ) -> bool {
        self.set_running_state(
            &payload.thread.id,
            thread_is_running(&payload.thread),
            thread_cache,
        )
    }

    fn apply_thread_status_changed(
        &mut self,
        payload: &ThreadStatusChangedNotification,
        thread_cache: &mut ThreadCachePayload,
    ) -> bool {
        let has_active_turns = self
            .active_turn_ids_by_thread
            .get(&payload.thread_id)
            .map(|turns| !turns.is_empty())
            .unwrap_or(false);
        let should_run = has_active_turns || thread_status_is_running(&payload.status);
        self.set_running_state(&payload.thread_id, should_run, thread_cache)
    }

    fn apply_turn_started(
        &mut self,
        payload: &TurnStartedNotification,
        thread_cache: &mut ThreadCachePayload,
    ) -> bool {
        self.active_turn_ids_by_thread
            .entry(payload.thread_id.clone())
            .or_default()
            .insert(payload.turn.id.clone());
        self.set_running_state(&payload.thread_id, true, thread_cache)
    }

    fn apply_turn_completed(
        &mut self,
        payload: &TurnCompletedNotification,
        thread_cache: &mut ThreadCachePayload,
    ) -> bool {
        let should_run = match self.active_turn_ids_by_thread.get_mut(&payload.thread_id) {
            Some(turn_ids) => {
                turn_ids.remove(&payload.turn.id);
                !turn_ids.is_empty()
            }
            None => false,
        };
        if !should_run {
            self.active_turn_ids_by_thread.remove(&payload.thread_id);
        }
        let should_run = should_run || thread_status_from_turn(&payload.turn) == Some(TurnStatus::InProgress);
        self.set_running_state(&payload.thread_id, should_run, thread_cache)
    }

    fn apply_thread_closed(
        &mut self,
        payload: &ThreadClosedNotification,
        thread_cache: &mut ThreadCachePayload,
    ) -> bool {
        self.active_turn_ids_by_thread.remove(&payload.thread_id);
        self.set_running_state(&payload.thread_id, false, thread_cache)
    }

    fn apply_item_notification(
        &self,
        payload: &impl ItemNotification,
        mode: UpsertMode,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        let Some(message) = message_from_item(payload.item(), payload.thread_id(), Some(payload.turn_id()), mode) else {
            return false;
        };
        if mode == UpsertMode::Replace && matches!(payload.item(), ThreadItem::AgentMessage { .. }) {
            remove_superseded_agent_fragments(thread_cache, payload.thread_id(), payload.turn_id(), &message);
        }
        upsert_message(
            thread_cache,
            payload.thread_id(),
            message,
            mode,
            changed_thread_ids,
        )
    }

    fn apply_agent_message_delta(
        &self,
        payload: &AgentMessageDeltaNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        let message_id = assistant_message_id(&payload.item_id, &payload.turn_id);
        let existing = find_message(thread_cache, &payload.thread_id, &message_id);
        let mut message = make_message_with_context(
            message_id,
            payload.thread_id.clone(),
            Some(payload.turn_id.clone()),
            "assistant",
            merge_delta_text(existing.map(|message| message.text.as_str()).unwrap_or(""), &payload.delta),
            existing.and_then(|message| message.phase.clone()),
            existing.and_then(|message| message.subtitle.clone()),
            existing.and_then(|message| message.tool_metadata.clone()),
            existing.map(|message| message.created_at),
        );
        message.delivery_state = "streaming".to_string();
        upsert_message(
            thread_cache,
            &payload.thread_id,
            message,
            UpsertMode::Merge,
            changed_thread_ids,
        )
    }

    fn apply_raw_response_item_completed(
        &self,
        payload: &RawResponseItemCompletedNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
    let Some(message) = message_from_raw_response_item(&payload.item, &payload.thread_id, &payload.turn_id) else {
            return false;
        };
        remove_superseded_agent_fragments(thread_cache, &payload.thread_id, &payload.turn_id, &message);
        upsert_message(
            thread_cache,
            &payload.thread_id,
            message,
            UpsertMode::Replace,
            changed_thread_ids,
        )
    }

    fn apply_plan_delta(
        &self,
        payload: &PlanDeltaNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        let existing = find_message(thread_cache, &payload.thread_id, &payload.item_id);
        let message = make_message(
            payload.item_id.clone(),
            payload.thread_id.clone(),
            "system",
            merge_delta_text(existing.map(|message| message.text.as_str()).unwrap_or(""), &payload.delta),
            Some(existing.and_then(|message| message.subtitle.clone()).unwrap_or_else(|| "plan (draft)".to_string())),
            existing.and_then(|message| message.tool_metadata.clone()),
            existing.map(|message| message.created_at),
        );
        upsert_message(
            thread_cache,
            &payload.thread_id,
            message,
            UpsertMode::Merge,
            changed_thread_ids,
        )
    }

    fn apply_reasoning_summary_text_delta(
        &self,
        payload: &ReasoningSummaryTextDeltaNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        let message_id = format!("{}-summary-{}", payload.item_id, payload.summary_index);
        let existing = find_message(thread_cache, &payload.thread_id, &message_id);
        let message = make_message(
            message_id,
            payload.thread_id.clone(),
            "system",
            merge_delta_text(existing.map(|message| message.text.as_str()).unwrap_or(""), &payload.delta),
            Some(
                existing
                    .and_then(|message| message.subtitle.clone())
                    .unwrap_or_else(|| "reasoning summary".to_string()),
            ),
            existing.and_then(|message| message.tool_metadata.clone()),
            existing.map(|message| message.created_at),
        );
        upsert_message(
            thread_cache,
            &payload.thread_id,
            message,
            UpsertMode::Merge,
            changed_thread_ids,
        )
    }

    fn apply_reasoning_summary_part_added(
        &self,
        payload: &ReasoningSummaryPartAddedNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        let message_id = format!("{}-summary-{}", payload.item_id, payload.summary_index);
        let Some(existing) = find_message(thread_cache, &payload.thread_id, &message_id) else {
            return false;
        };
        if existing.text.is_empty() || existing.text.ends_with("\n\n") {
            return false;
        }
        let message = make_message(
            message_id,
            payload.thread_id.clone(),
            "system",
            format!("{}\n\n", existing.text),
            Some(
                existing
                    .subtitle
                    .clone()
                    .unwrap_or_else(|| "reasoning summary".to_string()),
            ),
            existing.tool_metadata.clone(),
            Some(existing.created_at),
        );
        upsert_message(
            thread_cache,
            &payload.thread_id,
            message,
            UpsertMode::Replace,
            changed_thread_ids,
        )
    }

    fn apply_reasoning_text_delta(
        &self,
        payload: &ReasoningTextDeltaNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        let message_id = format!("{}-content-{}", payload.item_id, payload.content_index);
        let existing = find_message(thread_cache, &payload.thread_id, &message_id);
        let message = make_message(
            message_id,
            payload.thread_id.clone(),
            "system",
            merge_delta_text(existing.map(|message| message.text.as_str()).unwrap_or(""), &payload.delta),
            Some(
                existing
                    .and_then(|message| message.subtitle.clone())
                    .unwrap_or_else(|| "reasoning".to_string()),
            ),
            existing.and_then(|message| message.tool_metadata.clone()),
            existing.map(|message| message.created_at),
        );
        upsert_message(
            thread_cache,
            &payload.thread_id,
            message,
            UpsertMode::Merge,
            changed_thread_ids,
        )
    }

    fn apply_terminal_interaction(
        &self,
        payload: &TerminalInteractionNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        self.apply_tool_output_delta(
            &payload.thread_id,
            &payload.item_id,
            "commandExecution",
            Some(payload.process_id.clone()),
            &payload.stdin,
            thread_cache,
            changed_thread_ids,
        )
    }

    fn apply_command_execution_output_delta(
        &self,
        payload: &CommandExecutionOutputDeltaNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        self.apply_tool_output_delta(
            &payload.thread_id,
            &payload.item_id,
            "commandExecution",
            None,
            &payload.delta,
            thread_cache,
            changed_thread_ids,
        )
    }

    fn apply_file_change_output_delta(
        &self,
        payload: &FileChangeOutputDeltaNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        self.apply_tool_output_delta(
            &payload.thread_id,
            &payload.item_id,
            "fileChange",
            None,
            &payload.delta,
            thread_cache,
            changed_thread_ids,
        )
    }

    fn apply_tool_output_delta(
        &self,
        thread_id: &str,
        item_id: &str,
        kind: &str,
        process_id: Option<String>,
        delta: &str,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        if delta.is_empty() {
            return false;
        }

        let existing = find_message(thread_cache, thread_id, item_id);
        let existing_output = existing
            .and_then(|message| message.tool_metadata.as_ref())
            .and_then(|metadata| metadata.output.as_deref())
            .unwrap_or("");
        let subtitle_default = match kind {
            "fileChange" => "fileChange (in_progress)",
            _ => "commandExecution (in_progress)",
        };
        let body_default = match kind {
            "fileChange" => "Proposed file changes",
            _ => "Shell command",
        };
        let tool_metadata = RobdexToolMetadata {
            kind: kind.to_string(),
            status: Some("in_progress".to_string()),
            command: existing
                .and_then(|message| message.tool_metadata.as_ref())
                .and_then(|metadata| metadata.command.clone()),
            output: Some(format!("{existing_output}{delta}")),
            process_id: process_id.or_else(|| {
                existing
                    .and_then(|message| message.tool_metadata.as_ref())
                    .and_then(|metadata| metadata.process_id.clone())
            }),
        };
        let message = make_message(
            item_id.to_string(),
            thread_id.to_string(),
            "tool",
            existing
                .map(|message| message.text.clone())
                .unwrap_or_else(|| body_default.to_string()),
            Some(
                existing
                    .and_then(|message| message.subtitle.clone())
                    .unwrap_or_else(|| subtitle_default.to_string()),
            ),
            Some(tool_metadata),
            existing.map(|message| message.created_at),
        );
        upsert_message(
            thread_cache,
            thread_id,
            message,
            UpsertMode::Merge,
            changed_thread_ids,
        )
    }

    fn apply_turn_plan_updated(
        &self,
        payload: &TurnPlanUpdatedNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        let step_lines = payload
            .plan
            .iter()
            .map(|step| format!("[{}] {}", turn_plan_status_label(step.status), step.step))
            .collect::<Vec<_>>();
        let body = [payload.explanation.as_deref().unwrap_or("").trim(), &step_lines.join("\n")]
            .into_iter()
            .filter(|entry| !entry.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        if body.is_empty() {
            return false;
        }
        let message = make_message(
            format!("turn-plan-{}", payload.turn_id),
            payload.thread_id.clone(),
            "system",
            body,
            Some("turn plan".to_string()),
            None,
            None,
        );
        upsert_message(
            thread_cache,
            &payload.thread_id,
            message,
            UpsertMode::Replace,
            changed_thread_ids,
        )
    }

    fn apply_turn_diff_updated(
        &self,
        payload: &TurnDiffUpdatedNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        let message = make_message(
            format!("turn-diff-{}", payload.turn_id),
            payload.thread_id.clone(),
            "tool",
            "Turn diff updated".to_string(),
            Some("git diff".to_string()),
            Some(RobdexToolMetadata {
                kind: "fileChange".to_string(),
                status: Some("in_progress".to_string()),
                command: None,
                output: Some(payload.diff.clone()),
                process_id: None,
            }),
            None,
        );
        upsert_message(
            thread_cache,
            &payload.thread_id,
            message,
            UpsertMode::Replace,
            changed_thread_ids,
        )
    }
    fn apply_model_rerouted(
        &self,
        payload: &ModelReroutedNotification,
        thread_cache: &mut ThreadCachePayload,
        changed_thread_ids: &mut BTreeSet<String>,
    ) -> bool {
        let message = make_message(
            format!("model-rerouted-{}", payload.turn_id),
            payload.thread_id.clone(),
            "system",
            format!(
                "Model rerouted from {} to {} ({}).",
                payload.from_model,
                payload.to_model,
                format!("{:?}", payload.reason).to_ascii_lowercase()
            ),
            Some("model reroute".to_string()),
            None,
            None,
        );
        upsert_message(
            thread_cache,
            &payload.thread_id,
            message,
            UpsertMode::Replace,
            changed_thread_ids,
        )
    }

    fn set_running_state(
        &self,
        thread_id: &str,
        is_running: bool,
        thread_cache: &mut ThreadCachePayload,
    ) -> bool {
        let mut running = thread_cache.running_thread_ids.iter().cloned().collect::<BTreeSet<_>>();
        let changed = if is_running {
            running.insert(thread_id.to_string())
        } else {
            running.remove(thread_id)
        };
        if changed {
            thread_cache.running_thread_ids = running.into_iter().collect();
        }
        changed
    }
}

trait ItemNotification {
    fn thread_id(&self) -> &str;
    fn turn_id(&self) -> &str;
    fn item(&self) -> &ThreadItem;
}

impl ItemNotification for ItemStartedNotification {
    fn thread_id(&self) -> &str {
        &self.thread_id
    }

    fn turn_id(&self) -> &str {
        &self.turn_id
    }

    fn item(&self) -> &ThreadItem {
        &self.item
    }
}

impl ItemNotification for ItemCompletedNotification {
    fn thread_id(&self) -> &str {
        &self.thread_id
    }

    fn turn_id(&self) -> &str {
        &self.turn_id
    }

    fn item(&self) -> &ThreadItem {
        &self.item
    }
}

fn upsert_message(
    thread_cache: &mut ThreadCachePayload,
    thread_id: &str,
    mut message: RobdexChatMessage,
    mode: UpsertMode,
    changed_thread_ids: &mut BTreeSet<String>,
) -> bool {
    bound_model_visible_message_text(&mut message);
    if !is_renderable_message(&message) && find_message(thread_cache, thread_id, &message.id).is_none() {
        return false;
    }
    let messages = thread_cache
        .message_cache_by_thread_id
        .entry(thread_id.to_string())
        .or_default();
    if message.role == "user"
        && let Some(index) = messages.iter().position(|entry| {
            entry.id.starts_with("local-user-")
                && entry.role == "user"
                && entry.text.trim() == message.text.trim()
        })
    {
        let next_message = replace_message(messages[index].clone(), message);
        if messages[index] == next_message {
            return false;
        }
        messages[index] = next_message;
        changed_thread_ids.insert(thread_id.to_string());
        return true;
    }
    let next = match messages.iter().position(|entry| entry.id == message.id) {
        Some(index) => {
            let next_message = match mode {
                UpsertMode::Merge => merge_chat_messages(messages[index].clone(), message),
                UpsertMode::Replace => replace_message(messages[index].clone(), message),
            };
            if messages[index] == next_message {
                return false;
            }
            messages[index] = next_message;
            true
        }
        None => {
            messages.push(message);
            true
        }
    };
    if next {
        changed_thread_ids.insert(thread_id.to_string());
    }
    next
}

fn bound_model_visible_message_text(message: &mut RobdexChatMessage) {
    if message.role != "assistant" {
        return;
    }
    if message.phase.as_deref() == Some("commentary") {
        message.text = truncate_chars(&message.text, MAX_MODEL_VISIBLE_COMMENTARY_CHARS);
        return;
    }
    if let Ok(mut value) = serde_json::from_str::<Value>(message.text.trim())
        && let Some(object) = value.as_object_mut()
        && let Some(summary) = object.get_mut("summary")
        && let Some(text) = summary.as_str()
        && text.chars().count() > MAX_REQUIREMENTS_SUMMARY_CHARS
    {
        *summary = Value::String(truncate_chars(text, MAX_REQUIREMENTS_SUMMARY_CHARS));
        if let Ok(next) = serde_json::to_string(&value) {
            message.text = next;
        }
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut truncated = text.chars().take(max_chars).collect::<String>();
    truncated.push_str(BRIDGE_TRUNCATION_MARKER);
    truncated
}

fn is_renderable_message(message: &RobdexChatMessage) -> bool {
    !message.text.trim().is_empty()
        || message.subtitle.as_deref().is_some_and(|value| !value.trim().is_empty())
        || message.tool_metadata.is_some()
}

fn remove_superseded_agent_fragments(
    thread_cache: &mut ThreadCachePayload,
    thread_id: &str,
    turn_id: &str,
    incoming: &RobdexChatMessage,
) -> bool {
    let Some(messages) = thread_cache.message_cache_by_thread_id.get_mut(thread_id) else {
        return false;
    };
    let before = messages.len();
    messages.retain(|message| {
        if message.id == incoming.id {
            return true;
        }
        if message.role != "assistant" || message.turn_id.as_deref() != Some(turn_id) {
            return true;
        }
        if incoming.phase.as_deref() == Some("commentary")
            && message.phase.as_deref() == Some("commentary")
        {
            return true;
        }
        !(message.phase.is_none() || message.phase == incoming.phase)
    });
    messages.len() != before
}

fn find_message<'a>(
    thread_cache: &'a ThreadCachePayload,
    thread_id: &str,
    message_id: &str,
) -> Option<&'a RobdexChatMessage> {
    thread_cache
        .message_cache_by_thread_id
        .get(thread_id)
        .and_then(|messages| messages.iter().find(|message| message.id == message_id))
}

fn replace_message(existing: RobdexChatMessage, mut incoming: RobdexChatMessage) -> RobdexChatMessage {
    incoming.created_at = existing.created_at;
    if incoming.turn_id.is_none() {
        incoming.turn_id = existing.turn_id;
    }
    if incoming.phase.is_none() {
        incoming.phase = existing.phase;
    }
    incoming
}

fn merge_chat_messages(existing: RobdexChatMessage, incoming: RobdexChatMessage) -> RobdexChatMessage {
    let merged_tool = match (existing.tool_metadata.as_ref(), incoming.tool_metadata.as_ref()) {
        (None, None) => None,
        (existing_tool, incoming_tool) => Some(RobdexToolMetadata {
            kind: incoming_tool
                .and_then(|metadata| Some(metadata.kind.clone()))
                .or_else(|| existing_tool.map(|metadata| metadata.kind.clone()))
                .unwrap_or_else(|| "other".to_string()),
            status: incoming_tool
                .and_then(|metadata| metadata.status.clone())
                .or_else(|| existing_tool.and_then(|metadata| metadata.status.clone())),
            command: prefer_merged_optional_text(
                existing_tool.and_then(|metadata| metadata.command.as_deref()),
                incoming_tool.and_then(|metadata| metadata.command.as_deref()),
            ),
            output: prefer_merged_optional_text(
                existing_tool.and_then(|metadata| metadata.output.as_deref()),
                incoming_tool.and_then(|metadata| metadata.output.as_deref()),
            ),
            process_id: incoming_tool
                .and_then(|metadata| metadata.process_id.clone())
                .or_else(|| existing_tool.and_then(|metadata| metadata.process_id.clone())),
        }),
    };

    RobdexChatMessage {
        id: existing.id,
        thread_id: existing.thread_id,
        turn_id: incoming.turn_id.or(existing.turn_id),
        role: incoming.role,
        text: prefer_merged_text(&existing.text, &incoming.text),
        phase: incoming.phase.or(existing.phase),
        created_at: existing.created_at,
        subtitle: incoming.subtitle.or(existing.subtitle),
        tool_metadata: merged_tool,
        delivery_state: incoming.delivery_state,
    }
}

fn prefer_merged_optional_text(existing: Option<&str>, incoming: Option<&str>) -> Option<String> {
    let merged = prefer_merged_text(existing.unwrap_or(""), incoming.unwrap_or(""));
    (!merged.trim().is_empty()).then_some(merged)
}

fn prefer_merged_text(existing: &str, incoming: &str) -> String {
    if incoming.is_empty() {
        return existing.to_string();
    }
    if existing.is_empty() {
        return incoming.to_string();
    }
    if existing == incoming {
        return existing.to_string();
    }
    if incoming.starts_with(existing) {
        return incoming.to_string();
    }
    if existing.starts_with(incoming) || existing.contains(incoming) {
        return existing.to_string();
    }

    let merged = merge_delta_text(existing, incoming);
    if merged.len() > existing.len().max(incoming.len()) {
        return merged;
    }

    if incoming.len() >= existing.len() {
        incoming.to_string()
    } else {
        existing.to_string()
    }
}

fn message_from_item(
    item: &ThreadItem,
    thread_id: &str,
    turn_id: Option<&str>,
    mode: UpsertMode,
) -> Option<RobdexChatMessage> {
    match item {
        ThreadItem::UserMessage { id, content } => {
            let text = content
                .iter()
                .map(render_user_input)
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if text.trim().is_empty() {
                return None;
            }
            Some(make_message(
                id.clone(),
                thread_id.to_string(),
                "user",
                text,
                None,
                None,
                None,
            ))
        }
        ThreadItem::AgentMessage {
            id, text, phase, ..
        } => {
            let mut message = make_message_with_context(
                assistant_message_id(id, turn_id.unwrap_or("")),
                thread_id.to_string(),
                turn_id.map(str::to_string),
                "assistant",
                text.clone(),
                agent_phase_label(phase.as_ref(), mode),
                None,
                None,
                None,
            );
            if mode == UpsertMode::Merge {
                message.delivery_state = "streaming".to_string();
            }
            Some(message)
        }
        ThreadItem::Plan { id, text } => Some(make_message(
            id.clone(),
            thread_id.to_string(),
            "system",
            text.clone(),
            Some("plan (draft)".to_string()),
            None,
            None,
        )),
        ThreadItem::Reasoning { id, summary, content } => {
            let mut body = Vec::new();
            if !summary.is_empty() {
                body.push(summary.join("\n\n"));
            }
            if !content.is_empty() {
                body.push(content.join("\n\n"));
            }
            if body.is_empty() {
                return None;
            }
            Some(make_message(
                id.clone(),
                thread_id.to_string(),
                "system",
                body.join("\n\n"),
                Some("reasoning".to_string()),
                None,
                None,
            ))
        }
        ThreadItem::CommandExecution {
            id,
            command,
            process_id,
            status,
            aggregated_output,
            ..
        } => Some(make_message(
            id.clone(),
            thread_id.to_string(),
            "tool",
            command.clone(),
            Some(format!("commandExecution ({})", command_execution_status_label(status))),
            Some(RobdexToolMetadata {
                kind: "commandExecution".to_string(),
                status: Some(command_execution_status_label(status).to_string()),
                command: Some(command.clone()),
                output: aggregated_output.clone(),
                process_id: process_id.clone(),
            }),
            None,
        )),
        ThreadItem::FileChange {
            id,
            status,
            changes,
            ..
        } => Some(make_message(
            id.clone(),
            thread_id.to_string(),
            "tool",
            "Proposed file changes".to_string(),
            Some(format!("fileChange ({})", patch_apply_status_label(status))),
            Some(RobdexToolMetadata {
                kind: "fileChange".to_string(),
                status: Some(patch_apply_status_label(status).to_string()),
                command: summarize_file_change_paths(changes),
                output: summarize_file_change_diffs(changes),
                process_id: None,
            }),
            None,
        )),
        ThreadItem::McpToolCall {
            id,
            server,
            tool,
            status,
            arguments,
            result,
            error,
            ..
        } => Some(make_message(
            id.clone(),
            thread_id.to_string(),
            "tool",
            format!("{server}.{tool}"),
            Some(format!("{server}.{tool} ({})", mcp_tool_status_label(status))),
            Some(RobdexToolMetadata {
                kind: "mcpToolCall".to_string(),
                status: Some(mcp_tool_status_label(status).to_string()),
                command: serde_json::to_string_pretty(arguments).ok(),
                output: result
                    .as_ref()
                    .and_then(|result| serde_json::to_string_pretty(result).ok())
                    .or_else(|| error.as_ref().and_then(|error| serde_json::to_string_pretty(error).ok())),
                process_id: None,
            }),
                None,
            )),
        ThreadItem::ImageView { id, path } => {
            let path = path.display().to_string();
            Some(make_message_with_context(
                id.clone(),
                thread_id.to_string(),
                turn_id.map(str::to_string),
                "tool",
                format!("Viewed image: {path}"),
                None,
                Some("imageView".to_string()),
                Some(RobdexToolMetadata {
                    kind: "imageView".to_string(),
                    status: Some("viewed".to_string()),
                    command: Some("view_image".to_string()),
                    output: Some(path),
                    process_id: None,
                }),
                None,
            ))
        }
        ThreadItem::ImageGeneration {
            id,
            status,
            revised_prompt,
            saved_path,
            ..
        } => {
            let path = saved_path.as_ref().map(|path| path.display().to_string());
            let body = match (status.trim(), path.as_deref()) {
                ("", Some(path)) => format!("Generated image saved to {path}"),
                ("", None) => "Image generation started.".to_string(),
                (status, Some(path)) => format!("Image generation {status}: {path}"),
                (status, None) => format!("Image generation {status}."),
            };
            Some(make_message_with_context(
                id.clone(),
                thread_id.to_string(),
                turn_id.map(str::to_string),
                "tool",
                body,
                None,
                Some(format!(
                    "imageGeneration ({})",
                    if status.trim().is_empty() { "running" } else { status.trim() }
                )),
                Some(RobdexToolMetadata {
                    kind: "imageGeneration".to_string(),
                    status: Some(if status.trim().is_empty() {
                        "running".to_string()
                    } else {
                        status.clone()
                    }),
                    command: revised_prompt.clone(),
                    output: path,
                    process_id: None,
                }),
                None,
            ))
        }
        ThreadItem::ContextCompaction { id } => {
            let (text, subtitle) = match mode {
                UpsertMode::Merge => (
                    "Context compaction started for this thread.".to_string(),
                    "context (in progress)".to_string(),
                ),
                UpsertMode::Replace => (
                    "Context compaction completed for this thread.".to_string(),
                    "context (completed)".to_string(),
                ),
            };
            Some(make_message(
                id.clone(),
                thread_id.to_string(),
                "system",
                text,
                Some(subtitle),
                None,
                None,
            ))
        }
        _ => None,
    }
}

fn message_from_raw_response_item(
    item: &ResponseItem,
    thread_id: &str,
    turn_id: &str,
) -> Option<RobdexChatMessage> {
    match item {
        ResponseItem::Message {
            id,
            role,
            content,
            phase,
            ..
        } => {
            if role != "assistant" {
                return None;
            }
            let text = content
                .iter()
                .filter_map(|entry| match entry {
                    ContentItem::OutputText { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>();
            if text.trim().is_empty() {
                return None;
            }
            Some(make_message_with_context(
                assistant_message_id(
                    &id.clone().unwrap_or_else(|| format!("raw-agent-message-{turn_id}")),
                    turn_id,
                ),
                thread_id.to_string(),
                Some(turn_id.to_string()),
                "assistant",
                text,
                agent_phase_label(phase.as_ref(), UpsertMode::Replace),
                None,
                None,
                None,
            ))
        }
        ResponseItem::ImageGenerationCall {
            id,
            status,
            revised_prompt,
            result,
            ..
        } => {
            let saved_path = save_raw_image_generation_result(thread_id, id, result);
            let body = match (status.trim(), saved_path.as_deref()) {
                ("", Some(path)) => format!("Image generation completed: {path}"),
                ("", None) => "Image generation completed.".to_string(),
                (status, Some(path)) => format!("Image generation {status}: {path}"),
                (status, None) => format!("Image generation {status}."),
            };
            Some(make_message_with_context(
                id.clone(),
                thread_id.to_string(),
                Some(turn_id.to_string()),
                "tool",
                body,
                None,
                Some(format!(
                    "imageGeneration ({})",
                    if status.trim().is_empty() { "completed" } else { status.trim() }
                )),
                Some(RobdexToolMetadata {
                    kind: "imageGeneration".to_string(),
                    status: Some(if status.trim().is_empty() {
                        "completed".to_string()
                    } else {
                        status.clone()
                    }),
                    command: revised_prompt.clone(),
                    output: saved_path,
                    process_id: None,
                }),
                None,
            ))
        }
        _ => None,
    }
}

fn save_raw_image_generation_result(thread_id: &str, image_id: &str, result: &str) -> Option<String> {
    if result.trim().is_empty() {
        return None;
    }
    let bytes = general_purpose::STANDARD.decode(result.trim()).ok()?;
    let codex_home = std::env::var_os("CODEX_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".codex")))?;
    let output_dir = codex_home.join("generated_images").join(sanitize_image_path_component(thread_id));
    std::fs::create_dir_all(&output_dir).ok()?;
    let path = output_dir.join(format!("{}.png", sanitize_image_path_component(image_id)));
    std::fs::write(&path, bytes).ok()?;
    Some(path.display().to_string())
}

fn sanitize_image_path_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => ch,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.trim_matches('_').is_empty() {
        "image".to_string()
    } else {
        sanitized
    }
}

fn render_user_input(input: &codex_app_server_adapter::app_server_protocol::UserInput) -> String {
    match input {
        codex_app_server_adapter::app_server_protocol::UserInput::Text { text, .. } => text.clone(),
        codex_app_server_adapter::app_server_protocol::UserInput::Image { url } => format!("[image] {url}"),
        codex_app_server_adapter::app_server_protocol::UserInput::LocalImage { path } => {
            let label = path
                .file_name()
                .and_then(|value| value.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            format!("[local-image] {label}")
        }
        codex_app_server_adapter::app_server_protocol::UserInput::Skill { name, .. } => format!("${name}"),
        codex_app_server_adapter::app_server_protocol::UserInput::Mention { name, .. } => format!("@{name}"),
    }
}

fn make_message(
    id: String,
    thread_id: String,
    role: &str,
    text: String,
    subtitle: Option<String>,
    tool_metadata: Option<RobdexToolMetadata>,
    created_at: Option<u64>,
) -> RobdexChatMessage {
    make_message_with_context(
        id,
        thread_id,
        None,
        role,
        text,
        None,
        subtitle,
        tool_metadata,
        created_at,
    )
}

fn assistant_message_id(item_id: &str, turn_id: &str) -> String {
    if turn_id.trim().is_empty() {
        return item_id.to_string();
    }
    format!("{turn_id}:{item_id}")
}

fn make_message_with_context(
    id: String,
    thread_id: String,
    turn_id: Option<String>,
    role: &str,
    text: String,
    phase: Option<String>,
    subtitle: Option<String>,
    tool_metadata: Option<RobdexToolMetadata>,
    created_at: Option<u64>,
) -> RobdexChatMessage {
    RobdexChatMessage {
        id,
        thread_id,
        turn_id,
        role: role.to_string(),
        text,
        phase,
        created_at: created_at.unwrap_or_else(unix_now),
        subtitle,
        tool_metadata: sanitize_tool_metadata(tool_metadata),
        delivery_state: "confirmed".to_string(),
    }
}

fn message_phase_label(phase: &MessagePhase) -> String {
    match phase {
        MessagePhase::Commentary => "commentary",
        MessagePhase::FinalAnswer => "final_answer",
    }
    .to_string()
}

fn agent_phase_label(phase: Option<&MessagePhase>, mode: UpsertMode) -> Option<String> {
    match phase {
        Some(phase) => Some(message_phase_label(phase)),
        None if mode == UpsertMode::Replace => Some("final_answer".to_string()),
        None => None,
    }
}

fn sanitize_tool_metadata(tool_metadata: Option<RobdexToolMetadata>) -> Option<RobdexToolMetadata> {
    tool_metadata.map(|metadata| RobdexToolMetadata {
        kind: metadata.kind,
        status: metadata.status,
        command: metadata.command,
        output: metadata
            .output
            .map(|value| truncate_bridge_text(&value, MAX_TOOL_OUTPUT_CHARS)),
        process_id: metadata.process_id,
    })
}

fn summarize_file_change_paths(changes: &[FileUpdateChange]) -> Option<String> {
    let mut paths = changes
        .iter()
        .map(|change| change.path.trim())
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    paths.sort_unstable();
    paths.dedup();
    if paths.is_empty() {
        return None;
    }
    Some(paths.join("\n"))
}

fn summarize_file_change_diffs(changes: &[FileUpdateChange]) -> Option<String> {
    let combined = changes
        .iter()
        .filter_map(|change| {
            let path = change.path.trim();
            let diff = change.diff.trim();
            if diff.is_empty() {
                return None;
            }
            Some(if path.is_empty() {
                diff.to_string()
            } else {
                format!("{path}\n{diff}")
            })
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let trimmed = combined.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn truncate_bridge_text(value: &str, hard_limit: usize) -> String {
    if value.chars().count() <= hard_limit {
        return value.to_string();
    }
    let allowed = hard_limit.saturating_sub(BRIDGE_TRUNCATION_MARKER.chars().count());
    let prefix = value.chars().take(allowed).collect::<String>();
    format!("{prefix}{BRIDGE_TRUNCATION_MARKER}")
}

pub fn transport_messages(
    messages: &[RobdexChatMessage],
    limit: usize,
) -> Vec<RobdexChatMessage> {
    let mut trimmed = if messages.len() <= limit {
        messages.to_vec()
    } else {
        messages[messages.len() - limit..].to_vec()
    };

    while trimmed.len() > 1
        && serde_json::to_vec(&trimmed)
            .map(|encoded| encoded.len() > MAX_TRANSPORT_THREAD_MESSAGES_BYTES)
            .unwrap_or(false)
    {
        trimmed.remove(0);
    }

    trimmed
}

fn context_window_status_from_token_usage(
    payload: &ThreadTokenUsageUpdatedNotification,
    fallback_model_context_window: Option<u64>,
) -> Option<ThreadContextWindowStatus> {
    let tokens_in_window = payload.token_usage.last.total_tokens.max(0) as u64;
    let model_context_window = payload
        .token_usage
        .model_context_window
        .map(|value| value.max(0) as u64)
        .or(fallback_model_context_window);

    let remaining_percent = match model_context_window {
        None => 100,
        Some(window) if window <= CONTEXT_WINDOW_BASELINE_TOKENS as u64 => 0,
        Some(window) => {
            let effective_window = window - CONTEXT_WINDOW_BASELINE_TOKENS as u64;
            let used = tokens_in_window.saturating_sub(CONTEXT_WINDOW_BASELINE_TOKENS as u64);
            let remaining = effective_window.saturating_sub(used);
            ((remaining as f64 / effective_window as f64) * 100.0)
                .round()
                .clamp(0.0, 100.0) as u32
        }
    };

    Some(ThreadContextWindowStatus {
        remaining_percent,
        used_tokens_in_context_window: tokens_in_window,
        model_context_window,
    })
}

fn thread_is_running(thread: &Thread) -> bool {
    thread_status_is_running(&thread.status)
}

fn thread_status_is_running(status: &ThreadStatus) -> bool {
    matches!(
        status,
        ThreadStatus::Active { active_flags }
            if active_flags.is_empty()
                || active_flags.contains(&ThreadActiveFlag::WaitingOnApproval)
                || active_flags.contains(&ThreadActiveFlag::WaitingOnUserInput)
    )
}

fn thread_status_from_turn(turn: &Turn) -> Option<TurnStatus> {
    Some(turn.status.clone())
}

fn turn_plan_status_label(status: TurnPlanStepStatus) -> &'static str {
    match status {
        TurnPlanStepStatus::Pending => "pending",
        TurnPlanStepStatus::InProgress => "in_progress",
        TurnPlanStepStatus::Completed => "completed",
    }
}

fn command_execution_status_label(status: &CommandExecutionStatus) -> &'static str {
    match status {
        CommandExecutionStatus::InProgress => "in_progress",
        CommandExecutionStatus::Completed => "completed",
        CommandExecutionStatus::Failed => "failed",
        CommandExecutionStatus::Declined => "declined",
    }
}

fn patch_apply_status_label(status: &PatchApplyStatus) -> &'static str {
    match status {
        PatchApplyStatus::InProgress => "in_progress",
        PatchApplyStatus::Completed => "completed",
        PatchApplyStatus::Failed => "failed",
        PatchApplyStatus::Declined => "declined",
    }
}

fn mcp_tool_status_label(status: &McpToolCallStatus) -> &'static str {
    match status {
        McpToolCallStatus::InProgress => "in_progress",
        McpToolCallStatus::Completed => "completed",
        McpToolCallStatus::Failed => "failed",
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MAX_TRANSPORT_MESSAGES_PER_THREAD;
    use codex_app_server_adapter::app_server_protocol::{
        CommandExecutionSource, GitInfo, ItemCompletedNotification, ItemStartedNotification,
        ServerNotification, SessionSource, Thread, ThreadClosedNotification, ThreadStartedNotification,
        ThreadStatus, ThreadStatusChangedNotification, ThreadTokenUsage,
        ThreadTokenUsageUpdatedNotification, TokenUsageBreakdown, Turn, TurnCompletedNotification,
        TurnStartedNotification, TurnStatus,
    };
    use std::path::PathBuf;

    fn sample_thread(status: ThreadStatus) -> Thread {
        Thread {
            id: "thread-1".to_string(),
            preview: "demo".to_string(),
            ephemeral: false,
            model_provider: "openai".to_string(),
            created_at: 1,
            updated_at: 1,
            status,
            forked_from_id: None,
            path: None,
            cwd: PathBuf::from("/tmp").try_into().expect("absolute cwd"),
            cli_version: "0.125.0".to_string(),
            source: SessionSource::AppServer,
            agent_nickname: None,
            agent_role: None,
            git_info: None::<GitInfo>,
            name: Some("demo".to_string()),
            turns: Vec::new(),
        }
    }

    fn sample_turn(id: &str, status: TurnStatus) -> Turn {
        Turn {
            id: id.to_string(),
            items: Vec::new(),
            status,
            started_at: None,
            completed_at: None,
            duration_ms: None,
            error: None,
        }
    }

    #[test]
    fn turn_started_marks_thread_running() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        let changed = reducer.apply_notification(
            &ServerNotification::TurnStarted(TurnStartedNotification {
                thread_id: "thread-1".to_string(),
                turn: sample_turn("turn-1", TurnStatus::InProgress),
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        assert!(changed.changed_thread_ids.is_empty());
        assert_eq!(cache.running_thread_ids, vec!["thread-1".to_string()]);
    }

    #[test]
    fn turn_completed_clears_last_active_turn() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        reducer.apply_notification(
            &ServerNotification::TurnStarted(TurnStartedNotification {
                thread_id: "thread-1".to_string(),
                turn: sample_turn("turn-1", TurnStatus::InProgress),
            }),
            &mut cache,
        );

        let changed = reducer.apply_notification(
            &ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn: sample_turn("turn-1", TurnStatus::Completed),
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        assert!(cache.running_thread_ids.is_empty());
    }

    #[test]
    fn turn_completed_caches_embedded_final_agent_message() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        let changed = reducer.apply_notification(
            &ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn: Turn {
                    id: "turn-1".to_string(),
                    items: vec![ThreadItem::AgentMessage {
                        id: "agent-final-1".to_string(),
                        text: "final status from completed turn".to_string(),
                        phase: None,
                        memory_citation: None,
                    }],
                    status: TurnStatus::Completed,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                    error: None,
                },
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        assert_eq!(changed.changed_thread_ids, vec!["thread-1".to_string()]);
        let message = &cache.message_cache_by_thread_id["thread-1"][0];
        assert_eq!(message.role, "assistant");
        assert_eq!(message.text, "final status from completed turn");
    }

    #[test]
    fn turn_completed_preserves_large_agent_message_without_bridge_truncation() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();
        let large_text = format!(
            "{{\"overallVerdict\":\"fail\",\"route\":{{\"message\":\"{}\"}}}}",
            "review detail ".repeat(3_000)
        );

        let changed = reducer.apply_notification(
            &ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn: Turn {
                    id: "turn-1".to_string(),
                    items: vec![ThreadItem::AgentMessage {
                        id: "agent-final-1".to_string(),
                        text: large_text.clone(),
                        phase: Some(MessagePhase::FinalAnswer),
                        memory_citation: None,
                    }],
                    status: TurnStatus::Completed,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                    error: None,
                },
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        let message = &cache.message_cache_by_thread_id["thread-1"][0];
        assert_eq!(message.text, large_text);
        assert!(!message.text.contains(BRIDGE_TRUNCATION_MARKER));
    }

    #[test]
    fn thread_cache_bounds_oversized_requirements_summary_and_commentary() {
        let mut cache = ThreadCachePayload::default();
        let mut changed = BTreeSet::new();
        let oversized_summary = "summary ".repeat(1_000);
        upsert_message(
            &mut cache,
            "thread-1",
            RobdexChatMessage {
                id: "final-1".to_string(),
                thread_id: "thread-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                role: "assistant".to_string(),
                text: serde_json::json!({"summary": oversized_summary, "requirements": null}).to_string(),
                phase: Some("final_answer".to_string()),
                created_at: 1,
                subtitle: None,
                tool_metadata: None,
                delivery_state: "completed".to_string(),
            },
            UpsertMode::Merge,
            &mut changed,
        );
        let value: Value = serde_json::from_str(&cache.message_cache_by_thread_id["thread-1"][0].text)
            .expect("json final message");
        let summary = value["summary"].as_str().expect("summary");
        assert!(summary.len() < "summary ".repeat(1_000).len());
        assert!(summary.contains(BRIDGE_TRUNCATION_MARKER));

        upsert_message(
            &mut cache,
            "thread-1",
            RobdexChatMessage {
                id: "commentary-1".to_string(),
                thread_id: "thread-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                role: "assistant".to_string(),
                text: "commentary ".repeat(1_000),
                phase: Some("commentary".to_string()),
                created_at: 2,
                subtitle: None,
                tool_metadata: None,
                delivery_state: "completed".to_string(),
            },
            UpsertMode::Merge,
            &mut changed,
        );
        let commentary = &cache.message_cache_by_thread_id["thread-1"][1].text;
        assert!(commentary.len() < "commentary ".repeat(1_000).len());
        assert!(commentary.contains(BRIDGE_TRUNCATION_MARKER));
    }

    #[test]
    fn turn_completed_preserves_agent_message_phase_and_turn_id() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        let changed = reducer.apply_notification(
            &ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn: Turn {
                    id: "turn-1".to_string(),
                    items: vec![ThreadItem::AgentMessage {
                        id: "agent-final-1".to_string(),
                        text: "final status".to_string(),
                        phase: Some(MessagePhase::FinalAnswer),
                        memory_citation: None,
                    }],
                    status: TurnStatus::Completed,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                    error: None,
                },
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        let message = &cache.message_cache_by_thread_id["thread-1"][0];
        assert_eq!(message.turn_id.as_deref(), Some("turn-1"));
        assert_eq!(message.phase.as_deref(), Some("final_answer"));
    }

    #[test]
    fn turn_completed_caches_embedded_command_message() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        let changed = reducer.apply_notification(
            &ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn: Turn {
                    id: "turn-1".to_string(),
                    items: vec![ThreadItem::CommandExecution {
                        id: "cmd-1".to_string(),
                        command: "echo ok".to_string(),
                        cwd: PathBuf::from("/tmp").try_into().expect("absolute cwd"),
                        process_id: Some("123".to_string()),
                        source: CommandExecutionSource::Agent,
                        status: CommandExecutionStatus::Completed,
                        command_actions: Vec::new(),
                        aggregated_output: Some("ok\n".to_string()),
                        exit_code: Some(0),
                        duration_ms: Some(10),
                    }],
                    status: TurnStatus::Completed,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                    error: None,
                },
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        assert_eq!(changed.changed_thread_ids, vec!["thread-1".to_string()]);
        let message = &cache.message_cache_by_thread_id["thread-1"][0];
        assert_eq!(message.role, "tool");
        assert_eq!(message.subtitle.as_deref(), Some("commandExecution (completed)"));
        assert_eq!(
            message
                .tool_metadata
                .as_ref()
                .and_then(|metadata| metadata.output.as_deref()),
            Some("ok\n")
        );
    }

    #[test]
    fn active_thread_status_marks_running_without_turn_tracking() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        let changed = reducer.apply_notification(
            &ServerNotification::ThreadStatusChanged(ThreadStatusChangedNotification {
                thread_id: "thread-1".to_string(),
                status: ThreadStatus::Active {
                    active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
                },
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        assert_eq!(cache.running_thread_ids, vec!["thread-1".to_string()]);
    }

    #[test]
    fn thread_closed_clears_running_state() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();
        cache.running_thread_ids = vec!["thread-1".to_string()];

        let changed = reducer.apply_notification(
            &ServerNotification::ThreadClosed(ThreadClosedNotification {
                thread_id: "thread-1".to_string(),
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        assert!(cache.running_thread_ids.is_empty());
    }

    #[test]
    fn thread_started_uses_upstream_status() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        reducer.apply_notification(
            &ServerNotification::ThreadStarted(ThreadStartedNotification {
                thread: sample_thread(ThreadStatus::Active { active_flags: vec![] }),
            }),
            &mut cache,
        );

        assert_eq!(cache.running_thread_ids, vec!["thread-1".to_string()]);
    }

    #[test]
    fn agent_message_delta_accumulates_text() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        reducer.apply_notification(
            &ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "item-1".to_string(),
                delta: "hello wor".to_string(),
            }),
            &mut cache,
        );
        let changed = reducer.apply_notification(
            &ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "item-1".to_string(),
                delta: "world".to_string(),
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        assert_eq!(changed.changed_thread_ids, vec!["thread-1".to_string()]);
        assert_eq!(
            cache.message_cache_by_thread_id["thread-1"][0].text,
            "hello world"
        );
        assert_eq!(
            cache.message_cache_by_thread_id["thread-1"][0].delivery_state,
            "streaming"
        );
    }

    #[test]
    fn multiple_commentary_messages_in_one_turn_remain_distinct() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        reducer.apply_notification(
            &ServerNotification::ItemCompleted(ItemCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item: ThreadItem::AgentMessage {
                    id: "commentary-1".to_string(),
                    text: "First commentary.".to_string(),
                    phase: Some(MessagePhase::Commentary),
                    memory_citation: None,
                },
            }),
            &mut cache,
        );
        reducer.apply_notification(
            &ServerNotification::ItemCompleted(ItemCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item: ThreadItem::AgentMessage {
                    id: "commentary-2".to_string(),
                    text: "Second commentary.".to_string(),
                    phase: Some(MessagePhase::Commentary),
                    memory_citation: None,
                },
            }),
            &mut cache,
        );
        reducer.apply_notification(
            &ServerNotification::ItemCompleted(ItemCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item: ThreadItem::AgentMessage {
                    id: "final-1".to_string(),
                    text: "Final answer.".to_string(),
                    phase: Some(MessagePhase::FinalAnswer),
                    memory_citation: None,
                },
            }),
            &mut cache,
        );

        let messages = &cache.message_cache_by_thread_id["thread-1"];
        assert_eq!(messages.len(), 3);
        assert_eq!(
            messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["turn-1:commentary-1", "turn-1:commentary-2", "turn-1:final-1"]
        );
    }

    #[test]
    fn reused_agent_item_ids_across_turns_remain_distinct() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        reducer.apply_notification(
            &ServerNotification::ItemCompleted(ItemCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item: ThreadItem::AgentMessage {
                    id: "item-1".to_string(),
                    text: "First assistant response.".to_string(),
                    phase: Some(MessagePhase::FinalAnswer),
                    memory_citation: None,
                },
            }),
            &mut cache,
        );
        reducer.apply_notification(
            &ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-2".to_string(),
                item_id: "item-1".to_string(),
                delta: "Second assistant".to_string(),
            }),
            &mut cache,
        );
        reducer.apply_notification(
            &ServerNotification::ItemCompleted(ItemCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-2".to_string(),
                item: ThreadItem::AgentMessage {
                    id: "item-1".to_string(),
                    text: "Second assistant response.".to_string(),
                    phase: Some(MessagePhase::FinalAnswer),
                    memory_citation: None,
                },
            }),
            &mut cache,
        );

        let messages = &cache.message_cache_by_thread_id["thread-1"];
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["turn-1:item-1", "turn-2:item-1"]
        );
        assert_eq!(
            messages.iter().map(|message| message.text.as_str()).collect::<Vec<_>>(),
            vec!["First assistant response.", "Second assistant response."]
        );
    }

    #[test]
    fn item_completed_agent_message_replaces_partial_delta_with_different_id() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        reducer.apply_notification(
            &ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "streamed-agent-1".to_string(),
                delta: "refix only".to_string(),
            }),
            &mut cache,
        );
        let changed = reducer.apply_notification(
            &ServerNotification::ItemCompleted(ItemCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item: ThreadItem::AgentMessage {
                    id: "completed-agent-1".to_string(),
                    text: "Full prefix only".to_string(),
                    phase: Some(MessagePhase::FinalAnswer),
                    memory_citation: None,
                },
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        let messages = &cache.message_cache_by_thread_id["thread-1"];
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "turn-1:completed-agent-1");
        assert_eq!(messages[0].text, "Full prefix only");
        assert_eq!(messages[0].phase.as_deref(), Some("final_answer"));
    }

    #[test]
    fn raw_response_item_completed_replaces_partial_agent_delta() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        reducer.apply_notification(
            &ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "streamed-agent-1".to_string(),
                delta: "uffix-only final".to_string(),
            }),
            &mut cache,
        );
        let changed = reducer.apply_notification(
            &ServerNotification::RawResponseItemCompleted(RawResponseItemCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item: ResponseItem::Message {
                    id: Some("response-agent-1".to_string()),
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "Full suffix-only final".to_string(),
                    }],
                    end_turn: None,
                    phase: Some(MessagePhase::FinalAnswer),
                },
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        let messages = &cache.message_cache_by_thread_id["thread-1"];
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "turn-1:response-agent-1");
        assert_eq!(messages[0].text, "Full suffix-only final");
        assert_eq!(messages[0].phase.as_deref(), Some("final_answer"));
    }

    #[test]
    fn item_completed_replaces_streamed_tool_payload() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        reducer.apply_notification(
            &ServerNotification::CommandExecutionOutputDelta(CommandExecutionOutputDeltaNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "cmd-1".to_string(),
                delta: "stdout line\n".to_string(),
            }),
            &mut cache,
        );

        let changed = reducer.apply_notification(
            &ServerNotification::ItemCompleted(ItemCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item: ThreadItem::CommandExecution {
                    id: "cmd-1".to_string(),
                    command: "cargo test".to_string(),
                    cwd: PathBuf::from("/tmp").try_into().expect("absolute cwd"),
                    process_id: Some("123".to_string()),
                    source: CommandExecutionSource::Agent,
                    status: CommandExecutionStatus::Completed,
                    command_actions: Vec::new(),
                    aggregated_output: Some("stdout line\ncompleted".to_string()),
                    exit_code: Some(0),
                    duration_ms: Some(10),
                },
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        let message = &cache.message_cache_by_thread_id["thread-1"][0];
        assert_eq!(message.subtitle.as_deref(), Some("commandExecution (completed)"));
        assert_eq!(
            message
                .tool_metadata
                .as_ref()
                .and_then(|metadata| metadata.output.as_deref()),
            Some("stdout line\ncompleted")
        );
    }

    #[test]
    fn context_compaction_items_show_started_and_completed_states() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        let started = reducer.apply_notification(
            &ServerNotification::ItemStarted(ItemStartedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item: ThreadItem::ContextCompaction {
                    id: "compact-1".to_string(),
                },
            }),
            &mut cache,
        );
        assert!(started.thread_cache_changed);
        let message = &cache.message_cache_by_thread_id["thread-1"][0];
        assert_eq!(message.text, "Context compaction started for this thread.");
        assert_eq!(message.subtitle.as_deref(), Some("context (in progress)"));

        let completed = reducer.apply_notification(
            &ServerNotification::ItemCompleted(ItemCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item: ThreadItem::ContextCompaction {
                    id: "compact-1".to_string(),
                },
            }),
            &mut cache,
        );
        assert!(completed.thread_cache_changed);
        let message = &cache.message_cache_by_thread_id["thread-1"][0];
        assert_eq!(message.text, "Context compaction completed for this thread.");
        assert_eq!(message.subtitle.as_deref(), Some("context (completed)"));
    }

    #[test]
    fn token_usage_notification_updates_context_window() {
        let mut reducer = RunningStateReducer::default();
        let mut cache = ThreadCachePayload::default();

        let changed = reducer.apply_notification(
            &ServerNotification::ThreadTokenUsageUpdated(ThreadTokenUsageUpdatedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                token_usage: ThreadTokenUsage {
                    total: TokenUsageBreakdown {
                        total_tokens: 30_000,
                        input_tokens: 0,
                        cached_input_tokens: 0,
                        output_tokens: 0,
                        reasoning_output_tokens: 0,
                    },
                    last: TokenUsageBreakdown {
                        total_tokens: 30_000,
                        input_tokens: 0,
                        cached_input_tokens: 0,
                        output_tokens: 0,
                        reasoning_output_tokens: 0,
                    },
                    model_context_window: Some(128_000),
                },
            }),
            &mut cache,
        );

        assert!(changed.thread_cache_changed);
        assert_eq!(changed.changed_thread_ids, vec!["thread-1".to_string()]);
        assert_eq!(
            cache.context_window_status_by_thread_id["thread-1"].used_tokens_in_context_window,
            30_000
        );
    }

    #[test]
    fn transport_messages_enforces_payload_budget() {
        let messages = (0..60)
            .map(|index| make_message(
                format!("msg-{index}"),
                "thread-1".to_string(),
                "assistant",
                format!("message-{index}"),
                None,
                None,
                Some(index),
            ))
            .collect::<Vec<_>>();

        let transported = transport_messages(&messages, MAX_TRANSPORT_MESSAGES_PER_THREAD);
        assert_eq!(transported.len(), MAX_TRANSPORT_MESSAGES_PER_THREAD);
        assert_eq!(transported.first().map(|message| message.id.as_str()), Some("msg-10"));
    }
}
