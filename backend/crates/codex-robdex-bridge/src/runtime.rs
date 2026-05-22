use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    env,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use codex_app_server_adapter::{
    app_server_protocol::{
        CommandExecutionRequestApprovalParams, DynamicToolCallParams, FileChangeRequestApprovalParams,
        FileUpdateChange, McpServerElicitationRequest, McpServerElicitationRequestParams, PatchChangeKind,
        PermissionsRequestApprovalParams, RequestId, ServerNotification, ServerRequest, ToolRequestUserInputParams,
        TurnStatus,
    },
    pinned_codex_version_label,
};
use codex_shell_command::parse_command::{extract_shell_command, shlex_join};
use serde_json::{Value, json};
use tokio::{
    sync::{Mutex, RwLock, broadcast, mpsc, oneshot},
    task::JoinHandle,
};

use crate::{
    app_server_overrides::{AppServerThreadOverrides, AppServerTurnOverrides, simple_sandbox_policy},
    commands::{
        active_requirements_for_thread, archive_thread, ensure_requirements_reviewer_for_thread, increment_compaction_count, mark_requirements_review_in_progress,
        mark_requirements_review_verdict, parse_state, persist_state, prune_archived_thread_locally,
        prune_missing_project_roots, record_requirement_packet, requirements_review_prompt,
        requirements_review_source_for_reviewer, requirements_review_target_for_thread,
        requirements_verdict_schema, run_compaction_hook_best_effort, send_follow_up_message,
        send_thread_input, tracked_project_identity_for_thread,
        PersistedState, RequirementPacketState,
    },
    config::BridgeSettings,
    hooks::{
        HookAction, HookEvent, approval_requested_payload, approval_resolved_payload,
        maybe_run_project_hook, stopped_payload,
    },
    models::{
        BridgeEvent, BridgeInfo, BridgeSnapshot, BridgeToolQuestion, EventReplayResponse,
        MAX_EVENT_HISTORY, MAX_TRANSPORT_MESSAGES_PER_THREAD, PROTOCOL_VERSION, PendingApproval,
        PendingApprovalFileChange, PendingApprovalFileChangeKind, PendingApprovalKind, SERVER_NAME, SERVER_VERSION, SequencedEvent,
        LiveProcessRecord, RobdexChatMessage, ThreadCachePayload, ThreadMessagesResponse,
    },
    store::RobdexBridgeStore,
    transport::{DEFAULT_RECONNECT_DELAY_MS, TransportControlMessage, run_transport_loop},
    transforms::resolve_role_instructions,
    upstream::{RunningStateReducer, UpstreamRuntimeEvent, transport_messages},
};

const DISCONNECT_RUNNING_STATE_CLEAR_DELAY_MS: u64 = 15_000;
const THREAD_CACHE_FLUSH_DEBOUNCE_MS: u64 = 200;

#[derive(Debug)]
pub struct BridgeRuntime {
    settings: BridgeSettings,
    store: RobdexBridgeStore,
    state_document: RwLock<Value>,
    thread_cache: RwLock<ThreadCachePayload>,
    live_processes_by_thread_id: RwLock<BTreeMap<String, Vec<LiveProcessRecord>>>,
    connection_status: RwLock<String>,
    event_log: RwLock<EventLog>,
    event_tx: broadcast::Sender<SequencedEvent>,
    upstream_tx: mpsc::Sender<UpstreamRuntimeEvent>,
    transport_tx: mpsc::Sender<TransportControlMessage>,
    transport_rx: Mutex<Option<mpsc::Receiver<TransportControlMessage>>>,
    running_state: RwLock<RunningStateReducer>,
    pending_approvals: RwLock<BTreeMap<String, PendingApproval>>,
    file_changes_by_item: RwLock<BTreeMap<String, Vec<PendingApprovalFileChange>>>,
    auto_routed_turn_keys: RwLock<BTreeSet<String>>,
    auto_routed_approval_keys: RwLock<BTreeSet<String>>,
    quarantined_auto_resume_thread_ids: RwLock<BTreeSet<String>>,
    disconnect_running_state_clear_delay: Duration,
    disconnect_clear_task: Mutex<Option<JoinHandle<()>>>,
    pending_thread_cache_flush_ids: Mutex<BTreeSet<String>>,
    thread_cache_flush_delay: Duration,
    thread_cache_flush_task: Mutex<Option<JoinHandle<()>>>,
    state_mutation_lock: Mutex<()>,
    next_transport_request_id: AtomicU64,
    cached_models: RwLock<Option<Value>>,
}

impl BridgeRuntime {
    pub async fn new(settings: BridgeSettings) -> Result<Arc<Self>> {
        Self::new_with_disconnect_delay(
            settings,
            Duration::from_millis(DISCONNECT_RUNNING_STATE_CLEAR_DELAY_MS),
        )
        .await
    }

    async fn new_with_disconnect_delay(
        settings: BridgeSettings,
        disconnect_running_state_clear_delay: Duration,
    ) -> Result<Arc<Self>> {
        settings.paths.ensure_parent_dirs()?;
        let store = RobdexBridgeStore::connect(&settings.paths.sqlite_db).await?;
        let state_document = load_state_json(&settings).await?;
        let thread_cache = store
            .load_thread_cache_payload(ThreadCachePayload::default())
            .await?;
        let (upstream_tx, upstream_rx) = mpsc::channel(256);
        let (transport_tx, transport_rx) = mpsc::channel(256);
        let (event_tx, _) = broadcast::channel(512);
        let runtime = Arc::new(Self {
            settings,
            store,
            state_document: RwLock::new(state_document),
            thread_cache: RwLock::new(thread_cache),
            live_processes_by_thread_id: RwLock::new(BTreeMap::new()),
            connection_status: RwLock::new("disconnected".to_string()),
            event_log: RwLock::new(EventLog::default()),
            event_tx,
            upstream_tx,
            transport_tx,
            transport_rx: Mutex::new(Some(transport_rx)),
            running_state: RwLock::new(RunningStateReducer::default()),
            pending_approvals: RwLock::new(BTreeMap::new()),
            file_changes_by_item: RwLock::new(BTreeMap::new()),
            auto_routed_turn_keys: RwLock::new(BTreeSet::new()),
            auto_routed_approval_keys: RwLock::new(BTreeSet::new()),
            quarantined_auto_resume_thread_ids: RwLock::new(BTreeSet::new()),
            disconnect_running_state_clear_delay,
            disconnect_clear_task: Mutex::new(None),
            pending_thread_cache_flush_ids: Mutex::new(BTreeSet::new()),
            thread_cache_flush_delay: Duration::from_millis(THREAD_CACHE_FLUSH_DEBOUNCE_MS),
            thread_cache_flush_task: Mutex::new(None),
            state_mutation_lock: Mutex::new(()),
            next_transport_request_id: AtomicU64::new(10_000),
            cached_models: RwLock::new(None),
        });
        runtime.clone().spawn_upstream_worker(upstream_rx);
        runtime
            .push_event(BridgeEvent::ConnectionStatus {
                message: "disconnected".to_string(),
            })
            .await;
        Ok(runtime)
    }

    pub fn settings(&self) -> &BridgeSettings {
        &self.settings
    }

    pub fn upstream_sender(&self) -> mpsc::Sender<UpstreamRuntimeEvent> {
        self.upstream_tx.clone()
    }

    pub async fn info(&self) -> BridgeInfo {
        BridgeInfo {
            protocol_version: PROTOCOL_VERSION,
            server_name: SERVER_NAME.to_string(),
            server_version: SERVER_VERSION.to_string(),
            codex_version: pinned_codex_version_label(),
            app_server_url: self.settings.app_server_url.clone(),
            state_json_path: self.settings.paths.state_json.display().to_string(),
            sqlite_db_path: self.settings.paths.sqlite_db.display().to_string(),
            connection_status: self.connection_status.read().await.clone(),
        }
    }

    pub async fn snapshot(&self) -> Result<BridgeSnapshot> {
        let thread_cache = self.thread_cache.read().await.clone();
        let state = self.state_document.read().await.clone();
        let log = self.event_log.read().await;
        Ok(BridgeSnapshot {
            state,
            thread_cache,
            connection_status: self.connection_status.read().await.clone(),
            latest_sequence: log.latest_sequence,
        })
    }

    pub async fn workbench_snapshot_value(&self) -> Value {
        let state = self.state_document.read().await.clone();
        let connection_status = self.connection_status.read().await.clone();
        let latest_sequence = self.event_log.read().await.latest_sequence;
        let thread_cache = self.thread_cache.read().await.clone();
        let live_processes_by_thread_id = self.live_processes_by_thread_id.read().await.clone();
        let running_thread_ids = thread_cache.running_thread_ids.clone();
        let pending_approvals = self.pending_approvals().await;
        json!({
            "state": state,
            "threadCache": {
                "runningThreadIDs": running_thread_ids,
                "contextWindowStatusByThreadID": thread_cache.context_window_status_by_thread_id,
            },
            "liveProcessesByThreadID": live_processes_by_thread_id,
            "pendingApprovals": pending_approvals,
            "connectionStatus": connection_status,
            "latestSequence": latest_sequence,
        })
    }

    pub async fn thread_messages(
        &self,
        thread_id: &str,
        limit: Option<usize>,
    ) -> Result<Option<ThreadMessagesResponse>> {
        let snapshot = self.thread_cache.read().await.clone();
        let messages = snapshot
            .message_cache_by_thread_id
            .get(thread_id)
            .cloned()
            .unwrap_or_default();
        if messages.is_empty() {
            return Ok(None);
        }
        let messages = match limit {
            Some(limit) => transport_messages(&messages, limit),
            None => messages,
        };
        Ok(Some(ThreadMessagesResponse {
            thread_id: thread_id.to_string(),
            version: snapshot.updated_at.unwrap_or(0),
            messages,
            context_window_status: snapshot.context_window_status_by_thread_id.get(thread_id).cloned(),
            generated_at: snapshot.updated_at.unwrap_or(0),
        }))
    }

    pub async fn append_local_user_message(
        &self,
        thread_id: &str,
        text: &str,
        local_image_paths: &[String],
    ) -> Result<()> {
        let text = text.trim();
        if text.is_empty() && local_image_paths.is_empty() {
            return Ok(());
        }

        let rendered_text = if local_image_paths.is_empty() {
            text.to_string()
        } else {
            let mut lines = Vec::new();
            if !text.is_empty() {
                lines.push(text.to_string());
            }
            for path in local_image_paths {
                if !path.trim().is_empty() {
                    let label = std::path::Path::new(path.trim())
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or(path.trim());
                    lines.push(format!("[local-image] {label}"));
                }
            }
            lines.join("\n")
        };

        let now = unix_now();
        let sequence = self.next_transport_request_id.fetch_add(1, Ordering::Relaxed);
        let message = RobdexChatMessage {
            id: format!("local-user-{now}-{sequence}"),
            thread_id: thread_id.to_string(),
            turn_id: None,
            role: "user".to_string(),
            text: rendered_text.clone(),
            phase: None,
            created_at: now,
            subtitle: None,
            tool_metadata: None,
            delivery_state: "sent".to_string(),
        };

        let payload = {
            let mut thread_cache = self.thread_cache.write().await;
            let messages = thread_cache
                .message_cache_by_thread_id
                .entry(thread_id.to_string())
                .or_default();
            if messages
                .iter()
                .rev()
                .take(20)
                .any(|existing| existing.role == "user" && existing.text.trim() == rendered_text.as_str())
            {
                return Ok(());
            }
            messages.push(message);
            thread_cache.updated_at = Some(now);
            thread_messages_changed_payload(&thread_cache, thread_id)
        };

        self.persist_thread_cache_now(&[thread_id.to_string()]).await?;
        self.push_event(BridgeEvent::ThreadMessagesChanged { payload }).await;
        Ok(())
    }

    pub async fn state_document_value(&self) -> Value {
        self.state_document.read().await.clone()
    }

    pub async fn persist_state_document(&self, value: Value) -> Result<()> {
        *self.state_document.write().await = value.clone();
        self.store.save_state_document(&value).await?;
        let encoded = serde_json::to_string_pretty(&value)?;
        tokio::fs::write(&self.settings.paths.state_json, encoded)
            .await
            .with_context(|| format!("failed to write {}", self.settings.paths.state_json.display()))?;
        Ok(())
    }

    pub async fn lock_state_mutation(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.state_mutation_lock.lock().await
    }

    pub async fn prune_thread_local(&self, thread_id: &str) -> Result<()> {
        let removed_pending = {
            let mut pending = self.pending_approvals.write().await;
            let before = pending.len();
            pending.retain(|_, approval| approval.thread_id != thread_id);
            pending.len() != before
        };

        self.file_changes_by_item
            .write()
            .await
            .retain(|key, _| !key.starts_with(&format!("{thread_id}|")));

        let payload = {
            let mut thread_cache = self.thread_cache.write().await;
            self.running_state
                .write()
                .await
                .set_thread_running_state(thread_id, false, &mut thread_cache);
            thread_cache.message_cache_by_thread_id.remove(thread_id);
            thread_cache.context_window_status_by_thread_id.remove(thread_id);
            thread_cache.updated_at = Some(unix_now());
            thread_messages_changed_payload(&thread_cache, thread_id)
        };

        self.store.delete_thread_cache(thread_id).await?;
        self.persist_thread_cache_now(&[]).await?;

        self.push_event(BridgeEvent::ThreadMessagesChanged { payload }).await;
        if removed_pending {
            let state = self.state_document.read().await.clone();
            self.push_event(BridgeEvent::AppStateSnapshot { state }).await;
        }
        Ok(())
    }

    pub async fn replay_events(&self, since: Option<u64>) -> EventReplayResponse {
        let log = self.event_log.read().await;
        log.replay(since)
    }

    pub async fn active_turn_id_for_thread(&self, thread_id: &str) -> Option<String> {
        self.running_state
            .read()
            .await
            .active_turn_id_for_thread(thread_id)
    }

    pub async fn pending_approvals(&self) -> Vec<PendingApproval> {
        self.pending_approvals
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    pub async fn clear_pending_approval(&self, approval_id: &str) -> bool {
        let removed = self.pending_approvals.write().await.remove(approval_id).is_some();
        if removed {
            let state = self.state_document.read().await.clone();
            self.push_event(BridgeEvent::AppStateSnapshot { state }).await;
        }
        removed
    }

    #[cfg(test)]
    pub async fn insert_pending_approval_for_test(&self, approval: PendingApproval) {
        self.pending_approvals
            .write()
            .await
            .insert(approval.id.clone(), approval);
    }

    pub async fn send_server_response(
        &self,
        request_id: RequestId,
        result: Value,
    ) -> Result<()> {
        let (ack_tx, ack_rx) = oneshot::channel();
        self.transport_tx
            .send(TransportControlMessage::SendServerResponse {
                request_id,
                result,
                ack: ack_tx,
            })
            .await
            .context("failed to send server response to transport")?;
        ack_rx
            .await
            .context("transport dropped approval response acknowledgement")?
    }

    pub async fn request_app_server_json(
        &self,
        method: impl Into<String>,
        params: Value,
    ) -> Result<Value> {
        let request_id = RequestId::Integer(
            self.next_transport_request_id
                .fetch_add(1, Ordering::Relaxed) as i64,
        );
        let (ack_tx, ack_rx) = oneshot::channel();
        self.transport_tx
            .send(TransportControlMessage::SendJsonRequest {
                request_id,
                method: method.into(),
                params,
                ack: ack_tx,
            })
            .await
            .context("failed to send app-server request to transport")?;
        ack_rx
            .await
            .context("transport dropped app-server request acknowledgement")?
    }

    pub async fn cached_model_list(&self) -> Result<Value> {
        if let Some(models) = self.cached_models.read().await.clone() {
            return Ok(models);
        }
        let result = self.request_app_server_json("model/list", json!({})).await?;
        let models = result.get("data").cloned().unwrap_or(result);
        *self.cached_models.write().await = Some(models.clone());
        Ok(models)
    }

    pub async fn register_live_process(&self, thread_id: &str, process: LiveProcessRecord) -> Value {
        let payload = {
            let mut processes_by_thread = self.live_processes_by_thread_id.write().await;
            let processes = processes_by_thread.entry(thread_id.to_string()).or_default();
            processes.retain(|entry| live_process_is_alive(entry) && entry.process_id != process.process_id);
            processes.push(process);
            processes.sort_by_key(|entry| entry.started_at);
            live_processes_changed_payload(thread_id, processes)
        };
        self.push_event(BridgeEvent::LiveProcessesChanged {
            payload: payload.clone(),
        })
        .await;
        payload
    }

    pub async fn complete_live_process(&self, thread_id: &str, process_id: &str) -> Option<Value> {
        let payload = {
            let mut processes_by_thread = self.live_processes_by_thread_id.write().await;
            let processes = processes_by_thread.get_mut(thread_id)?;
            processes.retain(|entry| live_process_is_alive(entry) && entry.process_id != process_id);
            let payload = live_processes_changed_payload(thread_id, processes);
            if processes.is_empty() {
                processes_by_thread.remove(thread_id);
            }
            payload
        };
        self.push_event(BridgeEvent::LiveProcessesChanged {
            payload: payload.clone(),
        })
        .await;
        Some(payload)
    }

    pub async fn live_process(&self, thread_id: &str, process_id: &str) -> Option<LiveProcessRecord> {
        let mut processes_by_thread = self.live_processes_by_thread_id.write().await;
        let processes = processes_by_thread.get_mut(thread_id)?;
        processes.retain(live_process_is_alive);
        processes
            .iter()
            .find(|entry| entry.process_id == process_id)
            .cloned()
    }

    pub async fn set_manual_thread_running_state(
        &self,
        thread_id: &str,
        is_running: bool,
    ) -> Result<()> {
        let mut thread_cache = self.thread_cache.write().await;
        let changed = self
            .running_state
            .write()
            .await
            .set_thread_running_state(thread_id, is_running, &mut thread_cache);
        if changed {
            thread_cache.updated_at = Some(unix_now());
            drop(thread_cache);
            self.persist_thread_cache_now(&[]).await?;
            let state = self.state_document.read().await.clone();
            self.push_event(BridgeEvent::AppStateSnapshot { state }).await;
        }
        Ok(())
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<SequencedEvent> {
        self.event_tx.subscribe()
    }

    pub async fn set_connection_status(&self, status: impl Into<String>) {
        let status = status.into();
        *self.connection_status.write().await = status.clone();
        self.push_event(BridgeEvent::ConnectionStatus { message: status }).await;
    }

    pub async fn push_event(&self, event: BridgeEvent) {
        let entry = self.event_log.write().await.push(event);
        let _ = self.event_tx.send(entry);
    }

    pub fn spawn_transport(self: &Arc<Self>) -> JoinHandle<()> {
        let url = self.settings.app_server_url.clone();
        let tx = self.upstream_sender();
        let self_clone = Arc::clone(self);
        tokio::spawn(async move {
            let control_rx = {
                let mut guard = self_clone.transport_rx.lock().await;
                guard.take()
            };
            let Some(control_rx) = control_rx else {
                tracing::warn!("transport already spawned");
                return;
            };
            run_transport_loop(
                url,
                tx,
                control_rx,
                std::time::Duration::from_millis(DEFAULT_RECONNECT_DELAY_MS),
            )
            .await;
        })
    }

    fn spawn_upstream_worker(self: Arc<Self>, mut rx: mpsc::Receiver<UpstreamRuntimeEvent>) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Err(error) = self.handle_upstream_event(event).await {
                    self.set_connection_status(format!("worker error: {error}")).await;
                }
            }
        })
    }

    async fn handle_upstream_event(self: &Arc<Self>, event: UpstreamRuntimeEvent) -> Result<()> {
        match event {
            UpstreamRuntimeEvent::ConnectionStatus(status) => {
                self.set_connection_status(status.clone()).await;
                let runtime = self.clone();
                tokio::spawn(async move {
                    runtime.handle_connection_status_update(&status).await;
                });
            }
            UpstreamRuntimeEvent::ClearRunningStateAfterDisconnect => {
                self.handle_disconnect_running_state_clear().await?;
            }
            UpstreamRuntimeEvent::FlushPendingThreadCacheWrites => {
                self.flush_pending_thread_cache_writes().await?;
            }
            UpstreamRuntimeEvent::ServerRequest(request) => {
                self.handle_server_request(request).await?;
            }
            UpstreamRuntimeEvent::Notification(notification) => {
                let compaction_completed_thread_id = match &notification {
                    ServerNotification::ItemCompleted(payload) => match payload.item {
                        codex_app_server_adapter::app_server_protocol::ThreadItem::ContextCompaction { .. } => {
                            Some(payload.thread_id.clone())
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let turn_completed = match &notification {
                    ServerNotification::TurnCompleted(payload) => {
                        Some((
                            payload.thread_id.clone(),
                            payload.turn.id.clone(),
                            payload.turn.status.clone(),
                        ))
                    }
                    _ => None,
                };
                self.capture_item_file_changes(&notification).await;
                let result = {
                    let mut thread_cache = self.thread_cache.write().await;
                    self.running_state
                        .write()
                        .await
                        .apply_notification(&notification, &mut thread_cache)
                };
                self.handle_request_resolved_notification(&notification).await;
                if result.thread_cache_changed {
                    {
                        let mut thread_cache = self.thread_cache.write().await;
                        thread_cache.updated_at = Some(unix_now());
                    }
                    let changed_thread_ids = result.changed_thread_ids.clone();
                    let should_debounce_flush =
                        is_streaming_notification(&notification) && !result.running_state_changed;
                    if should_debounce_flush {
                        self.mark_thread_cache_dirty(&changed_thread_ids).await;
                        self.schedule_thread_cache_flush().await;
                    } else {
                        self.persist_thread_cache_now(&changed_thread_ids).await?;
                    }
                    if result.running_state_changed {
                        let state = self.state_document.read().await.clone();
                        self.push_event(BridgeEvent::AppStateSnapshot { state }).await;
                    }
                    let thread_cache = self.thread_cache.read().await;
                    for thread_id in changed_thread_ids {
                        self.push_event(BridgeEvent::ThreadMessagesChanged {
                            payload: thread_messages_changed_payload(&thread_cache, &thread_id),
                        })
                        .await;
                    }
                }
                if let Some((thread_id, turn_id, turn_status)) = turn_completed
                    && turn_status == TurnStatus::Completed
                {
                    let runtime = self.clone();
                    tokio::spawn(async move {
                        if let Err(error) = runtime.handle_completed_turn_routes(thread_id, turn_id).await {
                            tracing::warn!("completed turn routing failed: {error}");
                        }
                    });
                }
                if let Some(thread_id) = compaction_completed_thread_id {
                    let runtime = self.clone();
                    tokio::spawn(async move {
                        if let Err(error) = runtime.handle_compaction_completed(thread_id).await {
                            tracing::warn!("compaction completion hook failed: {error}");
                        }
                    });
                }
            }
        }
        Ok(())
    }

    async fn handle_completed_turn_routes(&self, thread_id: String, turn_id: String) -> Result<()> {
        let handled_requirements_verdict =
            self.maybe_record_requirements_verdict(&thread_id, &turn_id).await;
        if handled_requirements_verdict {
            return Ok(());
        }
        if self.maybe_route_requirements_review(&thread_id, &turn_id).await {
            return Ok(());
        }
        let hook_configured = self.maybe_run_stopped_hook(&thread_id, &turn_id).await?;
        if !hook_configured {
            self.maybe_auto_route_reply_to_orchestrator(&thread_id, &turn_id)
                .await;
        }
        Ok(())
    }

    async fn handle_compaction_completed(&self, thread_id: String) -> Result<()> {
        let maybe_compaction = {
            let _guard = self.lock_state_mutation().await;
            let mut state = parse_state(&self.state_document_value().await);
            let next = increment_compaction_count(&mut state, &thread_id);
            if next.is_some() {
                persist_state(self, &state).await?;
            }
            next
        };
        if let Some(compaction) = maybe_compaction {
            run_compaction_hook_best_effort(self, &thread_id, &compaction).await?;
        }
        Ok(())
    }

    async fn handle_pending_approval_routes(&self, approval: PendingApproval) -> Result<()> {
        let hook_configured = self.maybe_run_approval_requested_hook(&approval).await?;
        let still_pending = self
            .pending_approvals
            .read()
            .await
            .contains_key(&approval.id);
        if still_pending {
            if !hook_configured {
                self.maybe_route_approval_to_orchestrator(&approval).await;
            }
            let state = self.state_document.read().await.clone();
            self.push_event(BridgeEvent::AppStateSnapshot { state }).await;
        }
        Ok(())
    }

    async fn handle_server_request(self: &Arc<Self>, request: ServerRequest) -> Result<()> {
        let pending = self.pending_approval_from_request(request).await;
        if let Some(approval) = pending {
            self.pending_approvals
                .write()
                .await
                .insert(approval.id.clone(), approval.clone());
            let state = self.state_document.read().await.clone();
            self.push_event(BridgeEvent::AppStateSnapshot { state }).await;
            let runtime = self.clone();
            tokio::spawn(async move {
                if let Err(error) = runtime.handle_pending_approval_routes(approval).await {
                    tracing::warn!("pending approval routing failed: {error}");
                }
            });
        }
        Ok(())
    }

    async fn handle_connection_status_update(&self, status: &str) {
        if status == "connected" {
            self.cancel_disconnect_running_state_clear().await;
            self.resume_tracked_threads().await;
        } else if should_schedule_disconnect_running_state_clear(status) {
            self.schedule_disconnect_running_state_clear().await;
        }
    }

    async fn resume_tracked_threads(&self) {
        let resume_requests = {
            let state = self.state_document.read().await;
            let quarantined = self.quarantined_auto_resume_thread_ids.read().await;
            let thread_ids = tracked_thread_ids_from_state(&state);
            thread_ids
                .into_iter()
                .filter(|thread_id| !quarantined.contains(thread_id))
                .map(|thread_id| {
                    let cwd = tracked_cwd_for_thread_value(&state, &thread_id);
                    let approval_policy =
                        tracked_approval_policy_for_thread_value(&state, &thread_id);
                    let model = tracked_model_for_thread_value(&state, &thread_id);
                    let model_provider = tracked_model_provider_for_thread_value(&state, &thread_id);
                    let effort = tracked_reasoning_for_thread_value(&state, &thread_id);
                    let sandbox_mode = tracked_sandbox_mode_for_thread_value(&state, &thread_id);
                    let base_instructions =
                        tracked_base_instructions_for_thread_value(&state, &thread_id);
                    let developer_instructions =
                        tracked_developer_instructions_for_thread_value(&state, &thread_id);
                    let params = AppServerThreadOverrides {
                        cwd,
                        approval_policy: approval_policy.map(Value::String),
                        sandbox: sandbox_mode,
                        model,
                        model_provider,
                        reasoning_effort: effort,
                        service_tier: tracked_service_tier_for_thread_value(&state, &thread_id),
                        approvals_reviewer: tracked_approvals_reviewer_for_thread_value(&state, &thread_id),
                        personality: tracked_personality_for_thread_value(&state, &thread_id),
                        config: tracked_config_for_thread_value(&state, &thread_id),
                        base_instructions,
                        developer_instructions,
                        persist_extended_history: tracked_persist_extended_history_for_thread_value(&state, &thread_id)
                            .or(Some(true)),
                        exclude_turns: Some(true),
                        ..Default::default()
                    }
                    .thread_resume_params(thread_id.clone(), None, None);
                    (thread_id, params)
                })
                .collect::<Vec<_>>()
        };

        for (thread_id, params) in resume_requests {
            if let Err(error) = self.request_app_server_json("thread/resume", params).await {
                if resume_error_means_missing_rollout(&error) {
                    if let Err(prune_error) = self.prune_missing_tracked_thread(&thread_id).await {
                        tracing::warn!(
                            "resume tracked thread prune failed: {thread_id}: {prune_error}"
                        );
                    }
                    continue;
                }
                if resume_error_means_transport_closed(&error) {
                    self.quarantined_auto_resume_thread_ids
                        .write()
                        .await
                        .insert(thread_id.clone());
                    tracing::warn!(
                        "resume tracked thread closed transport; quarantining automatic resume until bridge restart: {thread_id}: {error}"
                    );
                    continue;
                }
                tracing::warn!(
                    "resume tracked thread failed: {thread_id}: {error}"
                );
            }
        }
    }

    async fn prune_missing_tracked_thread(&self, thread_id: &str) -> Result<()> {
        let _guard = self.lock_state_mutation().await;
        let mut state = parse_state(&self.state_document_value().await);
        let pruned_thread_ids = prune_archived_thread_locally(&mut state, thread_id);
        if !pruned_thread_ids.is_empty() {
            persist_state(self, &state).await?;
            for pruned_thread_id in &pruned_thread_ids {
                self.prune_thread_local(pruned_thread_id).await?;
            }
            tracing::warn!(
                "pruned tracked thread(s) after missing rollout from app-server: {:?}",
                pruned_thread_ids
            );
        }
        Ok(())
    }

    async fn schedule_disconnect_running_state_clear(&self) {
        let mut task = self.disconnect_clear_task.lock().await;
        if task.is_some() {
            return;
        }
        let tx = self.upstream_sender();
        let delay = self.disconnect_running_state_clear_delay;
        *task = Some(tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            let _ = tx.send(UpstreamRuntimeEvent::ClearRunningStateAfterDisconnect).await;
        }));
    }

    async fn cancel_disconnect_running_state_clear(&self) {
        let mut task = self.disconnect_clear_task.lock().await;
        if let Some(handle) = task.take() {
            handle.abort();
        }
    }

    async fn handle_disconnect_running_state_clear(&self) -> Result<()> {
        {
            let mut task = self.disconnect_clear_task.lock().await;
            task.take();
        }

        if self.connection_status.read().await.as_str() == "connected" {
            return Ok(());
        }

        let mut thread_cache = self.thread_cache.write().await;
        let changed = self
            .running_state
            .write()
            .await
            .clear_running_state(&mut thread_cache);
        if changed {
            thread_cache.updated_at = Some(unix_now());
            drop(thread_cache);
            self.persist_thread_cache_now(&[]).await?;
            let state = self.state_document.read().await.clone();
            self.push_event(BridgeEvent::AppStateSnapshot { state }).await;
        }
        Ok(())
    }

    async fn mark_thread_cache_dirty(&self, changed_thread_ids: &[String]) {
        let mut pending = self.pending_thread_cache_flush_ids.lock().await;
        pending.extend(changed_thread_ids.iter().cloned());
    }

    async fn schedule_thread_cache_flush(&self) {
        let mut task = self.thread_cache_flush_task.lock().await;
        if task.is_some() {
            return;
        }
        let tx = self.upstream_sender();
        let delay = self.thread_cache_flush_delay;
        *task = Some(tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            let _ = tx
                .send(UpstreamRuntimeEvent::FlushPendingThreadCacheWrites)
                .await;
        }));
    }

    async fn cancel_thread_cache_flush(&self) {
        let mut task = self.thread_cache_flush_task.lock().await;
        if let Some(handle) = task.take() {
            handle.abort();
        }
    }

    async fn persist_thread_cache_now(&self, changed_thread_ids: &[String]) -> Result<()> {
        self.cancel_thread_cache_flush().await;
        let mut merged_ids = {
            let mut pending = self.pending_thread_cache_flush_ids.lock().await;
            let mut merged = std::mem::take(&mut *pending);
            merged.extend(changed_thread_ids.iter().cloned());
            merged.into_iter().collect::<Vec<_>>()
        };
        merged_ids.sort();
        let thread_cache = self.thread_cache.read().await.clone();
        self.store
            .save_thread_cache_delta(&thread_cache, &merged_ids)
            .await
    }

    async fn flush_pending_thread_cache_writes(&self) -> Result<()> {
        self.persist_thread_cache_now(&[]).await
    }

    async fn handle_request_resolved_notification(&self, notification: &ServerNotification) {
        let ServerNotification::ServerRequestResolved(payload) = notification else {
            return;
        };
        let mut pending = self.pending_approvals.write().await;
        let before_len = pending.len();
        pending.retain(|_, approval| {
            !(approval.thread_id == payload.thread_id && approval.request_id == payload.request_id)
        });
        if pending.len() != before_len {
            let state = self.state_document.read().await.clone();
            drop(pending);
            self.push_event(BridgeEvent::AppStateSnapshot { state }).await;
        }
        let prefix = format!("{}|", payload.thread_id);
        self.file_changes_by_item
            .write()
            .await
            .retain(|key, _| !key.starts_with(&prefix));
    }

    async fn capture_item_file_changes(&self, notification: &ServerNotification) {
        let ServerNotification::ItemStarted(payload) = notification else {
            return;
        };
        if let codex_app_server_adapter::app_server_protocol::ThreadItem::FileChange { id, changes, .. } = &payload.item {
            let normalized = changes
                .iter()
                .map(normalize_file_change)
                .collect::<Vec<_>>();
            if normalized.is_empty() {
                return;
            }
            self.file_changes_by_item
                .write()
                .await
                .insert(file_change_cache_key(&payload.thread_id, id), normalized);
        }
    }

    async fn pending_approval_from_request(&self, request: ServerRequest) -> Option<PendingApproval> {
        let instance_id = self.settings().project_path.display().to_string();
        match request {
            ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                Some(command_approval_from_request(instance_id, request_id, params))
            }
            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                let file_changes = self.file_changes_by_item.read().await;
                Some(file_change_approval_from_request(
                    instance_id,
                    request_id,
                    params,
                    &file_changes,
                ))
            }
            ServerRequest::ToolRequestUserInput { request_id, params } => Some(tool_user_input_from_request(
                instance_id,
                request_id,
                params,
            )),
            ServerRequest::DynamicToolCall { request_id, params } => Some(dynamic_tool_call_from_request(
                instance_id,
                request_id,
                params,
            )),
            ServerRequest::ChatgptAuthTokensRefresh { request_id, params } => {
                Some(chatgpt_refresh_from_request(
                    instance_id,
                    request_id,
                    serde_json::to_value(&params.reason)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_string))
                        .unwrap_or_else(|| "refresh requested".to_string()),
                ))
            }
            ServerRequest::PermissionsRequestApproval { request_id, params } => Some(permissions_approval_from_request(
                instance_id,
                request_id,
                params,
            )),
            ServerRequest::McpServerElicitationRequest { request_id, params } => Some(mcp_elicitation_from_request(
                instance_id,
                request_id,
                params,
            )),
            _ => None,
        }
    }

    async fn maybe_record_requirements_verdict(&self, thread_id: &str, turn_id: &str) -> bool {
        let state = self.state_document.read().await.clone();
        if !matches!(
            tracked_role_for_thread(&state, thread_id).as_deref(),
            Some("requirements-reviewer") | Some("requirementsReviewer")
        ) {
            return false;
        }
        let Some(verdict_text) = self.latest_assistant_text_for_thread(thread_id, Some(turn_id)).await else {
            return true;
        };
        let Some(source_thread_id) = self.latest_requirements_review_source_thread_id(thread_id).await else {
            return true;
        };
        let payload = serde_json::from_str(verdict_text.trim())
            .unwrap_or_else(|_| json!({ "raw": verdict_text.trim() }));
        let verdict_payload = match reviewable_requirements_verdict_payload(&payload) {
            ReviewableRequirementsVerdict::Verdict(value) => value,
            ReviewableRequirementsVerdict::NullCommentary => {
                let _ = record_requirement_packet(
                    self,
                    &source_thread_id,
                    RequirementPacketState {
                        packet_type: "verdictNull".to_string(),
                        source_thread_id: source_thread_id.clone(),
                        turn_id: Some(turn_id.to_string()),
                        target_thread_id: Some(thread_id.to_string()),
                        payload,
                        created_at: crate::commands::unix_now(),
                    },
                )
                .await;
                return true;
            }
            ReviewableRequirementsVerdict::Invalid => payload.clone(),
        };
        let _ = mark_requirements_review_verdict(self, &source_thread_id, thread_id, verdict_payload.clone()).await;
        let _ = record_requirement_packet(
            self,
            &source_thread_id,
            RequirementPacketState {
                packet_type: "verdict".to_string(),
                source_thread_id: source_thread_id.clone(),
                turn_id: Some(turn_id.to_string()),
                target_thread_id: Some(thread_id.to_string()),
                payload: verdict_payload.clone(),
                created_at: crate::commands::unix_now(),
            },
        )
        .await;
        self.maybe_route_requirements_verdict(&source_thread_id, thread_id, turn_id, &verdict_payload)
            .await;
        true
    }

    async fn maybe_route_requirements_verdict(
        &self,
        source_thread_id: &str,
        reviewer_thread_id: &str,
        turn_id: &str,
        payload: &Value,
    ) {
        let dedupe_key = format!("requirements-verdict|{reviewer_thread_id}|{turn_id}");
        {
            let routed = self.auto_routed_turn_keys.read().await;
            if routed.contains(&dedupe_key) {
                return;
            }
        }

        let overall = payload
            .get("overallVerdict")
            .and_then(Value::as_str)
            .unwrap_or("fail");
        let route_message = payload
            .get("route")
            .and_then(|route| route.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let state_value = self.state_document.read().await.clone();
        let source_label = sender_label_for_thread(&state_value, source_thread_id);
        let text = compose_requirements_verdict_route_message(overall, &source_label, route_message, payload);
        if text.trim().is_empty() {
            return;
        }

        let parsed_state = parse_state(&state_value);
        let destination_thread_id = match overall {
            "fail" | "rejectedBlocked" => source_thread_id.to_string(),
            "needsHumanWaiver" => {
                let Some(project) = tracked_project_for_thread(&state_value, source_thread_id) else {
                    return;
                };
                let Some(orchestrator_thread_id) = project
                    .orchestrator_thread_id
                    .filter(|id| id != reviewer_thread_id && id != source_thread_id)
                else {
                    return;
                };
                orchestrator_thread_id
            }
            "pass" | "acceptedBlocked" | "waiverAccepted" => {
                let Some(project) = tracked_project_for_thread(&state_value, source_thread_id) else {
                    return;
                };
                project
                    .orchestrator_thread_id
                    .filter(|id| id != reviewer_thread_id)
                    .unwrap_or_else(|| source_thread_id.to_string())
            }
            _ => source_thread_id.to_string(),
        };

        let result = if destination_thread_id == source_thread_id {
            send_thread_input(
                self,
                &parsed_state,
                &destination_thread_id,
                Some(&text),
                &[],
                None,
                None,
            )
            .await
        } else {
            let project = tracked_project_for_thread(&state_value, &destination_thread_id);
            let cwd = tracked_cwd_for_thread_value(&state_value, &destination_thread_id)
                .or_else(|| project.as_ref().and_then(|project| project.cwd.clone()))
                .or_else(|| project.as_ref().and_then(|project| project.project_root.clone()));
            let params = AppServerTurnOverrides {
                cwd,
                approval_policy: tracked_approval_policy_for_thread_value(&state_value, &destination_thread_id)
                    .map(Value::String),
                sandbox_policy: tracked_sandbox_policy_for_thread_value(&state_value, &destination_thread_id),
                model: tracked_model_for_thread_value(&state_value, &destination_thread_id),
                effort: tracked_reasoning_for_thread_value(&state_value, &destination_thread_id),
                service_tier: tracked_service_tier_for_thread_value(&state_value, &destination_thread_id),
                approvals_reviewer: tracked_approvals_reviewer_for_thread_value(&state_value, &destination_thread_id),
                personality: tracked_personality_for_thread_value(&state_value, &destination_thread_id),
                ..Default::default()
            }
            .turn_start_params(destination_thread_id.clone(), json!([{"type":"text","text": text}]));
            self.request_app_server_json("turn/start", params).await
        };
        if result.is_ok() {
            self.auto_routed_turn_keys
                .write()
                .await
                .insert(dedupe_key);
            if overall == "pass" {
                if let Err(error) = archive_thread(self, reviewer_thread_id).await {
                    tracing::warn!(
                        "failed to archive completed requirements reviewer {reviewer_thread_id}: {error}"
                    );
                }
            }
        }
    }

    async fn latest_requirements_review_source_thread_id(&self, reviewer_thread_id: &str) -> Option<String> {
        let state = parse_state(&self.state_document.read().await.clone());
        if let Some(source_thread_id) = requirements_review_source_for_reviewer(&state, reviewer_thread_id) {
            return Some(source_thread_id);
        }

        let thread_cache = self.thread_cache.read().await;
        let messages = thread_cache.message_cache_by_thread_id.get(reviewer_thread_id)?;
        messages
            .iter()
            .rev()
            .filter(|message| message.role == "user")
            .flat_map(|message| message.text.lines())
            .find_map(|line| {
                line.trim()
                    .strip_prefix("Source thread ID:")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
    }

    async fn maybe_route_requirements_review(&self, thread_id: &str, turn_id: &str) -> bool {
        let state = self.state_document.read().await.clone();
        if matches!(
            tracked_role_for_thread(&state, thread_id).as_deref(),
            Some("requirements-reviewer") | Some("requirementsReviewer")
        ) {
            return false;
        }
        let parsed_state = parse_state(&state);
        let Some(set) = active_requirements_for_thread(&parsed_state, thread_id) else {
            return false;
        };
        if requirements_review_status_for_thread_value(&state, thread_id).as_deref() == Some("waiverRequired") {
            return false;
        }
        let Some(route_content) = self.auto_route_content_for_thread(thread_id, Some(turn_id)).await else {
            return false;
        };
        let claim_text = route_content.text.trim();
        if claim_text.is_empty() {
            return false;
        }
        let claim_payload = serde_json::from_str::<Value>(claim_text).unwrap_or_else(|_| json!({ "raw": claim_text }));
        let reviewable_claim_payload = match reviewable_requirements_claim_payload(&claim_payload) {
            ReviewableRequirementsClaim::Claims(value) => value,
            ReviewableRequirementsClaim::NullCommentary => {
                let text = requirements_null_claim_prompt();
                let _ = send_thread_input(
                    self,
                    &parsed_state,
                    thread_id,
                    Some(&text),
                    &[],
                    None,
                    None,
                )
                .await;
                let _ = record_requirement_packet(
                    self,
                    thread_id,
                    RequirementPacketState {
                        packet_type: "claimNull".to_string(),
                        source_thread_id: thread_id.to_string(),
                        turn_id: Some(turn_id.to_string()),
                        target_thread_id: None,
                        payload: claim_payload,
                        created_at: crate::commands::unix_now(),
                    },
                )
                .await;
                return true;
            }
            ReviewableRequirementsClaim::Invalid => {
                let text = requirements_invalid_claim_prompt();
                let _ = send_thread_input(
                    self,
                    &parsed_state,
                    thread_id,
                    Some(&text),
                    &[],
                    None,
                    None,
                )
                .await;
                return true;
            }
        };
        let reviewer_thread_id =
            match requirements_review_target_for_thread(&parsed_state, thread_id, &set)
                .filter(|reviewer_thread_id| reviewer_thread_id != thread_id)
            {
                Some(reviewer_thread_id) => reviewer_thread_id,
                None => match ensure_requirements_reviewer_for_thread(self, thread_id).await {
                    Ok(Some(reviewer_thread_id)) if reviewer_thread_id != thread_id => reviewer_thread_id,
                    _ => return false,
                },
            };

        let project = tracked_project_for_thread(&state, thread_id);
        let cwd = tracked_cwd_for_thread_value(&state, &reviewer_thread_id)
            .or_else(|| project.as_ref().and_then(|project| project.cwd.clone()))
            .or_else(|| project.as_ref().and_then(|project| project.project_root.clone()));
        let prompt = requirements_review_prompt(
            &set,
            &sender_label_for_thread(&state, thread_id),
            thread_id,
            turn_id,
            claim_text,
        );
        let params = AppServerTurnOverrides {
            cwd,
            approval_policy: tracked_approval_policy_for_thread_value(&state, &reviewer_thread_id)
                .map(Value::String),
            sandbox_policy: tracked_sandbox_policy_for_thread_value(&state, &reviewer_thread_id),
            model: tracked_model_for_thread_value(&state, &reviewer_thread_id),
            effort: tracked_reasoning_for_thread_value(&state, &reviewer_thread_id),
            service_tier: tracked_service_tier_for_thread_value(&state, &reviewer_thread_id),
            approvals_reviewer: tracked_approvals_reviewer_for_thread_value(&state, &reviewer_thread_id),
            personality: tracked_personality_for_thread_value(&state, &reviewer_thread_id),
            output_schema: Some(requirements_verdict_schema(&set)),
            ..Default::default()
        }
        .turn_start_params(reviewer_thread_id.clone(), json!([{"type":"text","text": prompt}]));

        if self.request_app_server_json("turn/start", params).await.is_ok() {
            let _ = mark_requirements_review_in_progress(
                self,
                thread_id,
                &reviewer_thread_id,
                &set,
                reviewable_claim_payload.clone(),
            )
            .await;
            let _ = record_requirement_packet(
                self,
                thread_id,
                RequirementPacketState {
                    packet_type: "claim".to_string(),
                    source_thread_id: thread_id.to_string(),
                    turn_id: Some(turn_id.to_string()),
                    target_thread_id: Some(reviewer_thread_id),
                    payload: reviewable_claim_payload,
                    created_at: crate::commands::unix_now(),
                },
            )
            .await;
            return true;
        }
        false
    }

    async fn maybe_auto_route_reply_to_orchestrator(&self, thread_id: &str, turn_id: &str) {
        let dedupe_key = format!("{thread_id}|{turn_id}");
        {
            let routed = self.auto_routed_turn_keys.read().await;
            if routed.contains(&dedupe_key) {
                return;
            }
        }

        let state = self.state_document.read().await.clone();
        let Some(project) = tracked_project_for_thread(&state, thread_id) else {
            return;
        };
        if !project.auto_route_replies {
            return;
        }
        if matches!(
            tracked_role_for_thread(&state, thread_id).as_deref(),
            Some("hidden") | Some("designer") | Some("operator")
        ) {
            return;
        }
        let Some(orchestrator_thread_id) = project.orchestrator_thread_id.filter(|id| id != thread_id) else {
            return;
        };
        let Some(route_content) = self.auto_route_content_for_thread(thread_id, Some(turn_id)).await else {
            return;
        };

        let routed_text = compose_auto_routed_reply(
            &route_content.text,
            &sender_label_for_thread(&state, thread_id),
            &route_content.local_image_paths,
        );
        if routed_text.trim().is_empty() {
            return;
        }

        let cwd = tracked_cwd_for_thread_value(&state, &orchestrator_thread_id)
            .or(project.cwd)
            .or(project.project_root);
        let params = AppServerTurnOverrides {
            cwd,
            approval_policy: tracked_approval_policy_for_thread_value(&state, &orchestrator_thread_id)
                .map(Value::String),
            sandbox_policy: tracked_sandbox_policy_for_thread_value(&state, &orchestrator_thread_id),
            model: tracked_model_for_thread_value(&state, &orchestrator_thread_id),
            effort: tracked_reasoning_for_thread_value(&state, &orchestrator_thread_id),
            service_tier: tracked_service_tier_for_thread_value(&state, &orchestrator_thread_id),
            approvals_reviewer: tracked_approvals_reviewer_for_thread_value(&state, &orchestrator_thread_id),
            personality: tracked_personality_for_thread_value(&state, &orchestrator_thread_id),
            ..Default::default()
        }
        .turn_start_params(
            orchestrator_thread_id.clone(),
            build_auto_route_input(routed_text, &route_content.local_image_paths),
        );

        if self.request_app_server_json("turn/start", params).await.is_ok()
        {
            self.auto_routed_turn_keys
                .write()
                .await
                .insert(dedupe_key);
        }
    }

    async fn maybe_route_approval_to_orchestrator(&self, approval: &PendingApproval) {
        {
            let routed = self.auto_routed_approval_keys.read().await;
            if routed.contains(&approval.id) {
                return;
            }
        }

        let state = self.state_document.read().await.clone();
        let Some(project) = tracked_project_for_thread(&state, &approval.thread_id) else {
            return;
        };
        if !project.route_approval_requests {
            return;
        }
        if matches!(
            tracked_role_for_thread(&state, &approval.thread_id).as_deref(),
            Some("hidden") | Some("designer")
        ) {
            return;
        }
        let Some(orchestrator_thread_id) = project
            .orchestrator_thread_id
            .filter(|id| id != &approval.thread_id)
        else {
            return;
        };

        let routed_text = compose_auto_routed_approval_request(
            approval,
            &sender_label_for_thread(&state, &approval.thread_id),
        );
        if routed_text.trim().is_empty() {
            return;
        }

        let cwd = tracked_cwd_for_thread_value(&state, &orchestrator_thread_id)
            .or(project.cwd)
            .or(project.project_root);
        let params = AppServerTurnOverrides {
            cwd,
            approval_policy: tracked_approval_policy_for_thread_value(&state, &orchestrator_thread_id)
                .map(Value::String),
            sandbox_policy: tracked_sandbox_policy_for_thread_value(&state, &orchestrator_thread_id),
            model: tracked_model_for_thread_value(&state, &orchestrator_thread_id),
            effort: tracked_reasoning_for_thread_value(&state, &orchestrator_thread_id),
            service_tier: tracked_service_tier_for_thread_value(&state, &orchestrator_thread_id),
            approvals_reviewer: tracked_approvals_reviewer_for_thread_value(&state, &orchestrator_thread_id),
            personality: tracked_personality_for_thread_value(&state, &orchestrator_thread_id),
            ..Default::default()
        }
        .turn_start_params(
            orchestrator_thread_id.clone(),
            json!([{"type":"text","text": routed_text}]),
        );

        if self.request_app_server_json("turn/start", params).await.is_ok()
        {
            self.auto_routed_approval_keys
                .write()
                .await
                .insert(approval.id.clone());
        }
    }

    async fn auto_route_content_for_thread(&self, thread_id: &str, turn_id: Option<&str>) -> Option<AutoRouteContent> {
        let text = self.latest_assistant_text_for_thread(thread_id, turn_id).await.unwrap_or_default();
        let local_image_paths = self.generated_image_paths_for_thread(thread_id, turn_id).await;
        if text.trim().is_empty() && local_image_paths.is_empty() {
            return None;
        }
        Some(AutoRouteContent {
            text,
            local_image_paths,
        })
    }

    async fn latest_assistant_text_for_thread(&self, thread_id: &str, turn_id: Option<&str>) -> Option<String> {
        let thread_cache = self.thread_cache.read().await;
        let messages = thread_cache.message_cache_by_thread_id.get(thread_id)?;
        if let Some(turn_id) = turn_id {
            let final_text = messages
                .iter()
                .filter(|message| {
                    message.role == "assistant"
                        && message.turn_id.as_deref() == Some(turn_id)
                        && message.phase.as_deref() == Some("final_answer")
                        && !message.text.trim().is_empty()
                })
                .map(|message| message.text.trim())
                .collect::<Vec<_>>()
                .join("\n\n");
            if !final_text.trim().is_empty() {
                return Some(final_text);
            }

            let turn_text = messages
                .iter()
                .filter(|message| {
                    message.role == "assistant"
                        && message.turn_id.as_deref() == Some(turn_id)
                        && message.phase.as_deref() != Some("commentary")
                        && !message.text.trim().is_empty()
                })
                .map(|message| message.text.trim())
                .collect::<Vec<_>>()
                .join("\n\n");
            if !turn_text.trim().is_empty() {
                return Some(turn_text);
            }
        }

        messages
            .iter()
            .rev()
            .find(|message| {
                message.role == "assistant"
                    && message.phase.as_deref() != Some("commentary")
                    && !message.text.trim().is_empty()
            })
            .map(|message| message.text.trim().to_string())
    }

    async fn generated_image_paths_for_thread(&self, thread_id: &str, turn_id: Option<&str>) -> Vec<String> {
        let thread_cache = self.thread_cache.read().await;
        let Some(messages) = thread_cache.message_cache_by_thread_id.get(thread_id) else {
            return Vec::new();
        };
        let mut paths = Vec::new();
        for message in messages {
            if let Some(turn_id) = turn_id
                && message.turn_id.as_deref() != Some(turn_id)
            {
                continue;
            }
            let Some(tool) = &message.tool_metadata else {
                continue;
            };
            if tool.kind != "imageGeneration" {
                continue;
            }
            let Some(path) = tool.output.as_deref().map(str::trim).filter(|value| !value.is_empty()) else {
                continue;
            };
            if !paths.iter().any(|existing| existing == path) {
                paths.push(path.to_string());
            }
        }
        paths
    }

    async fn maybe_run_approval_requested_hook(&self, approval: &PendingApproval) -> Result<bool> {
        let state_value = self.state_document.read().await.clone();
        let state = parse_state(&state_value);
        let Some(project) = tracked_project_for_thread(&state_value, &approval.thread_id) else {
            return Ok(false);
        };
        let project_root = project.project_root.clone().unwrap_or_default();
        if project_root.trim().is_empty() {
            return Ok(false);
        }
        let (project_id, project_name, _) = tracked_project_identity_for_thread(&state_value, &approval.thread_id)
            .unwrap_or_else(|| (project_root.clone(), project_root.clone(), project_root.clone()));
        let agent_name = sender_label_for_thread(&state_value, &approval.thread_id);
        let role = tracked_role_for_thread(&state_value, &approval.thread_id).unwrap_or_else(|| "worker".to_string());
        let payload = approval_requested_payload(
            &approval.thread_id,
            &project_id,
            &project_name,
            &project_root,
            &agent_name,
            &role,
            tracked_cwd_for_thread_value(&state_value, &approval.thread_id).as_deref(),
            project.orchestrator_thread_id.as_deref(),
            approval_payload_value(approval),
        );
        let invocation = maybe_run_project_hook(&project_root, HookEvent::ApprovalRequested, payload).await;
        if let Some(result) = invocation.result.as_ref() {
            self.execute_hook_actions(&state, &approval.thread_id, &result.actions, Some(approval))
                .await?;
        }
        if let Some(telemetry) = invocation.telemetry.as_ref() {
            tracing::warn!(
                "project hook {} failed for thread {}: {}",
                telemetry.event,
                approval.thread_id,
                telemetry.detail.clone().unwrap_or_default()
            );
        }
        Ok(invocation.configured)
    }

    pub(crate) async fn maybe_run_approval_resolved_hook(
        &self,
        approval: &PendingApproval,
        decision: &str,
        message: Option<&str>,
        sender_thread_id: Option<&str>,
    ) -> Result<()> {
        let state_value = self.state_document.read().await.clone();
        let state = parse_state(&state_value);
        let Some(project) = tracked_project_for_thread(&state_value, &approval.thread_id) else {
            return Ok(());
        };
        let project_root = project.project_root.clone().unwrap_or_default();
        if project_root.trim().is_empty() {
            return Ok(());
        }
        let (project_id, project_name, _) = tracked_project_identity_for_thread(&state_value, &approval.thread_id)
            .unwrap_or_else(|| (project_root.clone(), project_root.clone(), project_root.clone()));
        let agent_name = sender_label_for_thread(&state_value, &approval.thread_id);
        let role = tracked_role_for_thread(&state_value, &approval.thread_id).unwrap_or_else(|| "worker".to_string());
        let event = if decision == "accept" {
            HookEvent::Approved
        } else {
            HookEvent::Denied
        };
        let payload = approval_resolved_payload(
            event,
            &approval.thread_id,
            &project_id,
            &project_name,
            &project_root,
            &agent_name,
            &role,
            tracked_cwd_for_thread_value(&state_value, &approval.thread_id).as_deref(),
            project.orchestrator_thread_id.as_deref(),
            approval_payload_value(approval),
            json!({
                "decision": decision,
                "message": message,
                "senderThreadId": sender_thread_id,
            }),
        );
        let invocation = maybe_run_project_hook(&project_root, event, payload).await;
        if let Some(result) = invocation.result.as_ref() {
            self.execute_nonapproval_hook_actions(&state, &result.actions).await?;
        }
        if let Some(telemetry) = invocation.telemetry.as_ref() {
            tracing::warn!(
                "project hook {} failed for thread {}: {}",
                telemetry.event,
                approval.thread_id,
                telemetry.detail.clone().unwrap_or_default()
            );
        }
        Ok(())
    }

    async fn execute_nonapproval_hook_actions(
        &self,
        state: &PersistedState,
        actions: &[HookAction],
    ) -> Result<()> {
        for action in actions {
            match action {
                HookAction::SendMessage {
                    recipient_thread_id,
                    text,
                } => {
                    send_thread_input(
                        self,
                        state,
                        recipient_thread_id,
                        Some(text),
                        &[],
                        None,
                        None,
                    )
                    .await?;
                }
                HookAction::DeclineApproval { .. } => {
                    anyhow::bail!("declineApproval is only valid during onApprovalRequested hooks");
                }
            }
        }
        Ok(())
    }

    async fn maybe_run_stopped_hook(&self, thread_id: &str, turn_id: &str) -> Result<bool> {
        let state_value = self.state_document.read().await.clone();
        let state = parse_state(&state_value);
        let Some(project) = tracked_project_for_thread(&state_value, thread_id) else {
            return Ok(false);
        };
        let project_root = project.project_root.clone().unwrap_or_default();
        if project_root.trim().is_empty() {
            return Ok(false);
        }
        let (project_id, project_name, _) = tracked_project_identity_for_thread(&state_value, thread_id)
            .unwrap_or_else(|| (project_root.clone(), project_root.clone(), project_root.clone()));
        let agent_name = sender_label_for_thread(&state_value, thread_id);
        let role = tracked_role_for_thread(&state_value, thread_id).unwrap_or_else(|| "worker".to_string());
        let payload = stopped_payload(
            thread_id,
            turn_id,
            &project_id,
            &project_name,
            &project_root,
            &agent_name,
            &role,
            tracked_cwd_for_thread_value(&state_value, thread_id).as_deref(),
            project.orchestrator_thread_id.as_deref(),
            &self
                .latest_assistant_text_for_thread(thread_id, Some(turn_id))
                .await
                .unwrap_or_default(),
        );
        let invocation = maybe_run_project_hook(&project_root, HookEvent::Stopped, payload).await;
        if let Some(result) = invocation.result.as_ref() {
            self.execute_hook_actions(&state, thread_id, &result.actions, None)
                .await?;
        }
        if let Some(telemetry) = invocation.telemetry.as_ref() {
            tracing::warn!(
                "project hook {} failed for thread {}: {}",
                telemetry.event,
                thread_id,
                telemetry.detail.clone().unwrap_or_default()
            );
        }
        Ok(invocation.configured)
    }

    async fn execute_hook_actions(
        &self,
        state: &PersistedState,
        sender_thread_id: &str,
        actions: &[HookAction],
        current_approval: Option<&PendingApproval>,
    ) -> Result<()> {
        for action in actions {
            match action {
                HookAction::SendMessage {
                    recipient_thread_id,
                    text,
                } => {
                    self.execute_nonapproval_hook_actions(
                        state,
                        &[HookAction::SendMessage {
                            recipient_thread_id: recipient_thread_id.clone(),
                            text: text.clone(),
                        }],
                    )
                    .await?;
                }
                HookAction::DeclineApproval { approval_id, message } => {
                    let approval = if let Some(approval_id) = approval_id.as_deref() {
                        self.pending_approvals
                            .read()
                            .await
                            .get(approval_id)
                            .cloned()
                    } else {
                        current_approval.cloned()
                    }
                    .ok_or_else(|| anyhow::anyhow!("hook declineApproval target not found"))?;
                    let follow_up = message.as_deref().map(str::trim).filter(|value| !value.is_empty());
                    if let Some(message) = follow_up {
                        send_follow_up_message(self, &approval, message).await?;
                    }
                    self.send_server_response(approval.request_id.clone(), approval_response_payload(&approval, "decline"))
                        .await?;
                    self.clear_pending_approval(&approval.id).await;
                    self.maybe_run_approval_resolved_hook(
                        &approval,
                        "decline",
                        follow_up,
                        Some(sender_thread_id),
                    )
                    .await?;
                }
            }
        }
        Ok(())
    }
}

fn resume_error_means_missing_rollout(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("no rollout found for thread id") || message.contains("\"code\": -32600")
}

fn resume_error_means_transport_closed(error: &anyhow::Error) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("websocket receive error")
        || message.contains("websocket closed")
        || message.contains("websocket ended unexpectedly")
        || message.contains("transport dropped app-server request acknowledgement")
}

fn file_change_cache_key(thread_id: &str, item_id: &str) -> String {
    format!("{thread_id}|{item_id}")
}

fn approval_id_for_request(instance_id: &str, request_id: &RequestId) -> String {
    format!("{instance_id}:{}", request_id_display(request_id))
}

fn approval_payload_value(approval: &PendingApproval) -> Value {
    json!({
        "id": approval.id,
        "requestId": approval.request_id,
        "threadId": approval.thread_id,
        "turnId": approval.turn_id,
        "itemId": approval.item_id,
        "kind": approval.kind,
        "title": approval.title,
        "detail": approval.detail,
        "approvalReason": approval.approval_reason,
        "command": approval.command,
        "commandCwd": approval.command_cwd,
        "fileGrantRoot": approval.file_grant_root,
        "fileChanges": approval.file_changes,
    })
}

fn request_id_display(request_id: &RequestId) -> String {
    match request_id {
        RequestId::Integer(value) => value.to_string(),
        RequestId::String(value) => value.clone(),
    }
}

fn approval_response_payload(approval: &PendingApproval, decision: &str) -> Value {
    if approval.kind == PendingApprovalKind::McpElicitation {
        return json!({
            "action": if decision == "cancel" { "cancel" } else { "decline" },
            "content": null,
            "_meta": null,
        });
    }

    json!({ "decision": decision })
}

fn normalize_file_change(change: &FileUpdateChange) -> PendingApprovalFileChange {
    PendingApprovalFileChange {
        path: change.path.trim().to_string(),
        kind: match &change.kind {
            PatchChangeKind::Add => PendingApprovalFileChangeKind::Create,
            PatchChangeKind::Delete => PendingApprovalFileChangeKind::Delete,
            PatchChangeKind::Update { move_path } if move_path.is_some() => {
                PendingApprovalFileChangeKind::Rename
            }
            PatchChangeKind::Update { .. } => PendingApprovalFileChangeKind::Update,
        },
        diff: compact_optional_text(Some(change.diff.as_str())),
    }
}

fn command_approval_from_request(
    instance_id: String,
    request_id: RequestId,
    params: CommandExecutionRequestApprovalParams,
) -> PendingApproval {
    PendingApproval {
        id: approval_id_for_request(&instance_id, &request_id),
        instance_id,
        request_id,
        thread_id: params.thread_id,
        turn_id: params.turn_id,
        item_id: params.item_id,
        kind: PendingApprovalKind::CommandExecution,
        title: command_approval_title(params.command.as_deref()),
        detail: compact_optional_text(params.reason.as_deref()),
        approval_reason: compact_optional_text(params.reason.as_deref()),
        tool_name: None,
        tool_arguments: None,
        tool_questions: Vec::new(),
        auth_refresh_reason: None,
        command: compact_optional_text(params.command.as_deref()),
        command_cwd: params.cwd.as_ref().map(|value| value.display().to_string()),
        file_grant_root: None,
        file_changes: Vec::new(),
    }
}

fn file_change_approval_from_request(
    instance_id: String,
    request_id: RequestId,
    params: FileChangeRequestApprovalParams,
    file_changes: &BTreeMap<String, Vec<PendingApprovalFileChange>>,
) -> PendingApproval {
    let changes = file_changes
        .get(&file_change_cache_key(&params.thread_id, &params.item_id))
        .cloned()
        .unwrap_or_default();
    let file_grant_root = params.grant_root.as_ref().map(|value| value.display().to_string());
    PendingApproval {
        id: approval_id_for_request(&instance_id, &request_id),
        instance_id,
        request_id,
        thread_id: params.thread_id,
        turn_id: params.turn_id,
        item_id: params.item_id,
        kind: PendingApprovalKind::FileChange,
        title: file_change_approval_title(&changes, file_grant_root.as_deref()),
        detail: compact_optional_text(params.reason.as_deref())
            .or_else(|| file_trace_summary(&changes))
            .or_else(|| file_grant_root.clone()),
        approval_reason: compact_optional_text(params.reason.as_deref()),
        tool_name: None,
        tool_arguments: None,
        tool_questions: Vec::new(),
        auth_refresh_reason: None,
        command: None,
        command_cwd: None,
        file_grant_root,
        file_changes: changes,
    }
}

fn tool_user_input_from_request(
    instance_id: String,
    request_id: RequestId,
    params: ToolRequestUserInputParams,
) -> PendingApproval {
    let title = params
        .questions
        .first()
        .map(|question| format!("User input requested: {}", question.header))
        .unwrap_or_else(|| "User input requested".to_string());
    let tool_questions = params
        .questions
        .into_iter()
        .map(|question| BridgeToolQuestion {
            id: question.id,
            prompt: question.question,
        })
        .collect();
    PendingApproval {
        id: approval_id_for_request(&instance_id, &request_id),
        instance_id,
        request_id,
        thread_id: params.thread_id,
        turn_id: params.turn_id,
        item_id: params.item_id,
        kind: PendingApprovalKind::ToolUserInput,
        title,
        detail: None,
        approval_reason: None,
        tool_name: None,
        tool_arguments: None,
        tool_questions,
        auth_refresh_reason: None,
        command: None,
        command_cwd: None,
        file_grant_root: None,
        file_changes: Vec::new(),
    }
}

fn dynamic_tool_call_from_request(
    instance_id: String,
    request_id: RequestId,
    params: DynamicToolCallParams,
) -> PendingApproval {
    PendingApproval {
        id: approval_id_for_request(&instance_id, &request_id),
        instance_id,
        request_id,
        thread_id: params.thread_id,
        turn_id: params.turn_id,
        item_id: params.call_id,
        kind: PendingApprovalKind::DynamicToolCall,
        title: format!("Tool call: {}", params.tool),
        detail: None,
        approval_reason: None,
        tool_name: Some(params.tool),
        tool_arguments: Some(params.arguments),
        tool_questions: Vec::new(),
        auth_refresh_reason: None,
        command: None,
        command_cwd: None,
        file_grant_root: None,
        file_changes: Vec::new(),
    }
}

fn chatgpt_refresh_from_request(
    instance_id: String,
    request_id: RequestId,
    reason: String,
) -> PendingApproval {
    PendingApproval {
        id: approval_id_for_request(&instance_id, &request_id),
        instance_id,
        request_id,
        thread_id: "__global__".to_string(),
        turn_id: "__global__".to_string(),
        item_id: "__global__".to_string(),
        kind: PendingApprovalKind::ChatGptAuthRefresh,
        title: "ChatGPT auth refresh requested".to_string(),
        detail: compact_optional_text(Some(reason.as_str())),
        approval_reason: None,
        tool_name: None,
        tool_arguments: None,
        tool_questions: Vec::new(),
        auth_refresh_reason: compact_optional_text(Some(reason.as_str())),
        command: None,
        command_cwd: None,
        file_grant_root: None,
        file_changes: Vec::new(),
    }
}

fn permissions_approval_from_request(
    instance_id: String,
    request_id: RequestId,
    params: PermissionsRequestApprovalParams,
) -> PendingApproval {
    PendingApproval {
        id: approval_id_for_request(&instance_id, &request_id),
        instance_id,
        request_id,
        thread_id: params.thread_id,
        turn_id: params.turn_id,
        item_id: params.item_id,
        kind: PendingApprovalKind::CommandExecution,
        title: "Additional permissions requested".to_string(),
        detail: compact_optional_text(params.reason.as_deref()),
        approval_reason: compact_optional_text(params.reason.as_deref()),
        tool_name: None,
        tool_arguments: serde_json::to_value(params.permissions).ok(),
        tool_questions: Vec::new(),
        auth_refresh_reason: None,
        command: None,
        command_cwd: None,
        file_grant_root: None,
        file_changes: Vec::new(),
    }
}

fn mcp_elicitation_from_request(
    instance_id: String,
    request_id: RequestId,
    params: McpServerElicitationRequestParams,
) -> PendingApproval {
    let detail = match &params.request {
        McpServerElicitationRequest::Form { message, .. } => compact_optional_text(Some(message)),
        McpServerElicitationRequest::Url {
            message,
            url,
            elicitation_id,
            ..
        } => {
            let lines = vec![message.as_str(), url.as_str(), elicitation_id.as_str()];
            compact_optional_text(Some(&lines.join("\n")))
        }
    };
    PendingApproval {
        id: approval_id_for_request(&instance_id, &request_id),
        instance_id,
        request_id,
        thread_id: params.thread_id,
        turn_id: params.turn_id.unwrap_or_else(|| "__global__".to_string()),
        item_id: params.server_name.clone(),
        kind: PendingApprovalKind::McpElicitation,
        title: format!("MCP elicitation: {}", params.server_name),
        detail,
        approval_reason: None,
        tool_name: Some(params.server_name),
        tool_arguments: serde_json::to_value(params.request).ok(),
        tool_questions: Vec::new(),
        auth_refresh_reason: None,
        command: None,
        command_cwd: None,
        file_grant_root: None,
        file_changes: Vec::new(),
    }
}

fn compact_optional_text(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        None
    } else {
        Some(
            value
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

fn command_approval_title(command: Option<&str>) -> String {
    match compact_optional_text(command) {
        Some(command) => format!("Command approval: {}", truncate_text(&command, 72)),
        None => "Command approval".to_string(),
    }
}

fn file_change_approval_title(
    file_changes: &[PendingApprovalFileChange],
    grant_root: Option<&str>,
) -> String {
    if let Some(summary) = file_trace_summary(file_changes) {
        return format!("File approval: {}", truncate_text(&summary, 72));
    }
    if let Some(root) = compact_optional_text(grant_root) {
        return format!("File approval in {}", truncate_text(&root, 60));
    }
    "File change approval".to_string()
}

fn file_trace_summary(file_changes: &[PendingApprovalFileChange]) -> Option<String> {
    if file_changes.is_empty() {
        return None;
    }
    let visible = file_changes
        .iter()
        .take(3)
        .map(|change| format!("{} {}", file_change_label(&change.kind), change.path))
        .collect::<Vec<_>>();
    let remainder = file_changes.len().saturating_sub(visible.len());
    let suffix = if remainder > 0 {
        format!(" +{remainder} more")
    } else {
        String::new()
    };
    Some(format!("{}{}", visible.join(", "), suffix))
}

fn file_change_label(kind: &PendingApprovalFileChangeKind) -> &'static str {
    match kind {
        PendingApprovalFileChangeKind::Create => "create",
        PendingApprovalFileChangeKind::Update => "modify",
        PendingApprovalFileChangeKind::Delete => "delete",
        PendingApprovalFileChangeKind::Rename => "rename",
        PendingApprovalFileChangeKind::Unknown => "change",
    }
}

fn split_command_string(command: &str) -> Vec<String> {
    let Some(parts) = shlex::split(command) else {
        return vec![command.to_string()];
    };
    match shlex::try_join(parts.iter().map(String::as_str)) {
        Ok(round_trip)
            if round_trip == command
                || (!command.contains(":\\")
                    && shlex::split(&round_trip).as_ref() == Some(&parts)) =>
        {
            parts
        }
        _ => vec![command.to_string()],
    }
}

fn command_for_orchestrator_approval(command: &str) -> String {
    let parts = split_command_string(command);
    if let Some((_, script)) = extract_shell_command(&parts) {
        return script.to_string();
    }
    shlex_join(&parts)
}

fn truncate_text(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }
    value.chars().take(max_len).collect::<String>().trim().to_string() + "..."
}

#[derive(Debug, Default)]
struct EventLog {
    latest_sequence: u64,
    events: VecDeque<SequencedEvent>,
}

impl EventLog {
    fn push(&mut self, event: BridgeEvent) -> SequencedEvent {
        self.latest_sequence += 1;
        let entry = SequencedEvent {
            sequence: self.latest_sequence,
            event,
        };
        self.events.push_back(entry.clone());
        while self.events.len() > MAX_EVENT_HISTORY {
            self.events.pop_front();
        }
        entry
    }

    fn replay(&self, since: Option<u64>) -> EventReplayResponse {
        match since {
            Some(sequence) => {
                let events = self
                    .events
                    .iter()
                    .filter(|entry| entry.sequence > sequence)
                    .cloned()
                    .collect::<Vec<_>>();
                let requires_snapshot = self
                    .events
                    .front()
                    .map(|entry| sequence < entry.sequence)
                    .unwrap_or(false);
                EventReplayResponse {
                    events,
                    latest_sequence: self.latest_sequence,
                    requires_snapshot,
                }
            }
            None => EventReplayResponse {
                events: self.events.iter().cloned().collect(),
                latest_sequence: self.latest_sequence,
                requires_snapshot: false,
            },
        }
    }
}

async fn load_state_json(settings: &BridgeSettings) -> Result<Value> {
    if !settings.paths.state_json.exists() {
        return Ok(Value::Object(Default::default()));
    }

    let raw = tokio::fs::read_to_string(&settings.paths.state_json)
        .await
        .with_context(|| format!("failed to read {}", settings.paths.state_json.display()))?;
    let mut value: Value = serde_json::from_str(&raw)
        .with_context(|| format!("invalid JSON in {}", settings.paths.state_json.display()))?;
    let mut state = parse_state(&value);
    let removed = prune_missing_project_roots(&mut state);
    if !removed.is_empty() {
        tracing::warn!(
            "pruned {} stale Robdex project(s) with missing roots during startup: {}",
            removed.len(),
            removed.join(", ")
        );
        value = serde_json::to_value(&state)?;
        let bytes = serde_json::to_vec_pretty(&value)?;
        tokio::fs::write(&settings.paths.state_json, bytes)
            .await
            .with_context(|| format!("failed to write sanitized {}", settings.paths.state_json.display()))?;
    }
    Ok(value)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn live_process_is_alive(process: &LiveProcessRecord) -> bool {
    let target = process
        .process_group_id
        .filter(|pgid| *pgid > 0)
        .map(|pgid| -(pgid as libc::pid_t))
        .unwrap_or(process.pid as libc::pid_t);
    let rc = unsafe { libc::kill(target, 0) };
    if rc == 0 {
        return true;
    }
    let error = std::io::Error::last_os_error();
    error.raw_os_error() == Some(libc::EPERM)
}

fn live_processes_changed_payload(thread_id: &str, processes: &[LiveProcessRecord]) -> Value {
    json!({
        "threadId": thread_id,
        "processes": processes,
        "generatedAt": unix_now(),
    })
}

fn tracked_thread_ids_from_state(state: &Value) -> Vec<String> {
    let mut thread_ids = std::collections::BTreeSet::new();
    let Some(projects) = state.get("projects").and_then(Value::as_object) else {
        return Vec::new();
    };

    for project in projects.values() {
        if let Some(orchestrator) = project.get("orchestratorThreadID").and_then(Value::as_str) {
            let trimmed = orchestrator.trim();
            if !trimmed.is_empty() {
                thread_ids.insert(trimmed.to_string());
            }
        }
        if let Some(orchestrator) = project.get("orchestratorThreadId").and_then(Value::as_str) {
            let trimmed = orchestrator.trim();
            if !trimmed.is_empty() {
                thread_ids.insert(trimmed.to_string());
            }
        }
        if let Some(agents) = project.get("agents").and_then(Value::as_object) {
            for thread_id in agents.keys() {
                let trimmed = thread_id.trim();
                if !trimmed.is_empty() {
                    thread_ids.insert(trimmed.to_string());
                }
            }
        }
    }

    thread_ids.into_iter().collect()
}

fn should_schedule_disconnect_running_state_clear(status: &str) -> bool {
    status.starts_with("disconnected:")
        || status.starts_with("connect failed:")
        || status.starts_with("initialize failed:")
        || status == "disconnected"
}

#[derive(Clone)]
struct RoutedProjectState {
    project_root: Option<String>,
    cwd: Option<String>,
    auto_route_replies: bool,
    route_approval_requests: bool,
    orchestrator_thread_id: Option<String>,
    role_defaults: Value,
}

fn tracked_project_for_thread(state: &Value, thread_id: &str) -> Option<RoutedProjectState> {
    let projects = state.get("projects")?.as_object()?;
    for project in projects.values() {
        let project_object = project.as_object()?;
        let agents = project_object.get("agents").and_then(Value::as_object);
        let orchestrator = project_object
            .get("orchestratorThreadID")
            .or_else(|| project_object.get("orchestratorThreadId"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if agents.map(|agents| agents.contains_key(thread_id)).unwrap_or(false)
            || orchestrator.as_deref() == Some(thread_id)
        {
            return Some(RoutedProjectState {
                project_root: project_object
                    .get("projectRoot")
                    .or_else(|| project_object.get("project_root"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                cwd: project_object.get("cwd").and_then(Value::as_str).map(str::to_string),
                auto_route_replies: project_object
                    .get("autoRouteReplies")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                route_approval_requests: project_object
                    .get("routeApprovalRequests")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                orchestrator_thread_id: orchestrator,
                role_defaults: project_object
                    .get("configs")
                    .and_then(|value| value.get("roleModelReasoningDefaults"))
                    .cloned()
                    .unwrap_or(Value::Null),
            });
        }
    }
    None
}

fn tracked_agent_value<'a>(state: &'a Value, thread_id: &str) -> Option<&'a serde_json::Map<String, Value>> {
    let projects = state.get("projects")?.as_object()?;
    for project in projects.values() {
        let project_object = project.as_object()?;
        if let Some(agent) = project_object
            .get("agents")
            .and_then(Value::as_object)
            .and_then(|agents| agents.get(thread_id))
            .and_then(Value::as_object)
        {
            return Some(agent);
        }
    }
    None
}

fn project_value_for_thread<'a>(state: &'a Value, thread_id: &str, key: &str) -> Option<&'a Value> {
    let projects = state.get("projects")?.as_object()?;
    for project in projects.values() {
        let project_object = project.as_object()?;
        let contains_thread = project_object
            .get("agents")
            .and_then(Value::as_object)
            .map(|agents| agents.contains_key(thread_id))
            .unwrap_or(false)
            || project_object
                .get("orchestratorThreadID")
                .or_else(|| project_object.get("orchestratorThreadId"))
                .and_then(Value::as_str)
                == Some(thread_id);
        if contains_thread {
            return project_object.get(key);
        }
    }
    None
}

fn tracked_role_for_thread(state: &Value, thread_id: &str) -> Option<String> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("role"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            tracked_project_for_thread(state, thread_id).and_then(|project| {
                if project.orchestrator_thread_id.as_deref() == Some(thread_id) {
                    Some("orchestrator".to_string())
                } else {
                    None
                }
            })
        })
}

fn tracked_cwd_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("cwd"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| tracked_project_for_thread(state, thread_id).and_then(|project| project.cwd.or(project.project_root)))
}

fn tracked_approval_policy_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("approvalPolicy"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            state
                .get("globalConfigs")
                .and_then(|value| value.get("approvalPolicy"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn tracked_model_provider_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("modelProvider"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            tracked_project_for_thread(state, thread_id)
                .and_then(|_project| project_value_for_thread(state, thread_id, "preferredModelProvider"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn role_defaults_key_for_thread(state: &Value, thread_id: &str) -> &'static str {
    match tracked_role_for_thread(state, thread_id).as_deref() {
        Some("designer") => "designer",
        Some("qa") => "qa",
        Some("orchestrator") => "orchestrator",
        Some("requirements-reviewer") | Some("requirementsReviewer") => "requirements-reviewer",
        Some("worker") | Some("hidden") | Some("operator") | _ => "worker",
    }
}

fn role_default_model_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    let key = role_defaults_key_for_thread(state, thread_id);
    tracked_project_for_thread(state, thread_id)
        .and_then(|project| project.role_defaults.get(key).cloned())
        .and_then(|value| value.get("modelID").cloned())
        .and_then(|value| value.as_str().map(str::to_string))
}

fn role_default_reasoning_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    let key = role_defaults_key_for_thread(state, thread_id);
    tracked_project_for_thread(state, thread_id)
        .and_then(|project| project.role_defaults.get(key).cloned())
        .and_then(|value| value.get("reasoningEffort").cloned())
        .and_then(|value| value.as_str().map(str::to_string))
}

fn tracked_sandbox_mode_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("sandboxMode"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            state
                .get("globalConfigs")
                .and_then(|value| value.get("sandboxMode"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn effective_network_access_for_sandbox_value(
    sandbox_mode: Option<&str>,
    explicit_network_access: Option<bool>,
    default_network_access: Option<bool>,
) -> Option<bool> {
    match sandbox_mode {
        Some("workspace-write") | Some("external-sandbox") => {
            if explicit_network_access == Some(true) || default_network_access == Some(true) {
                Some(true)
            } else if explicit_network_access == Some(false) || default_network_access == Some(false) {
                Some(false)
            } else {
                Some(true)
            }
        }
        _ => None,
    }
}

fn tracked_network_access_for_thread_value(state: &Value, thread_id: &str) -> Option<bool> {
    let sandbox_mode = tracked_sandbox_mode_for_thread_value(state, thread_id);
    let default_network_access = state
        .get("globalConfigs")
        .and_then(|value| value.get("networkAccess"))
        .and_then(Value::as_bool);
    effective_network_access_for_sandbox_value(
        sandbox_mode.as_deref(),
        tracked_agent_value(state, thread_id)
            .and_then(|agent| agent.get("networkAccess"))
            .and_then(Value::as_bool),
        default_network_access,
    )
}

fn tracked_sandbox_policy_for_thread_value(state: &Value, thread_id: &str) -> Option<Value> {
    let sandbox_mode = tracked_sandbox_mode_for_thread_value(state, thread_id);
    let network_access = tracked_network_access_for_thread_value(state, thread_id);
    let cwd = tracked_cwd_for_thread_value(state, thread_id);
    sandbox_policy_for_resume_value(sandbox_mode.as_deref(), network_access, cwd.as_deref())
}

fn sandbox_policy_for_resume_value(
    sandbox_mode: Option<&str>,
    network_access: Option<bool>,
    cwd: Option<&str>,
) -> Option<Value> {
    simple_sandbox_policy(sandbox_mode, network_access, cwd)
}

fn tracked_developer_instructions_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    if let Some(value) = tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("developerInstructions"))
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        return Some(value);
    }
    let role = tracked_role_for_thread(state, thread_id)?;
    if matches!(role.as_str(), "hidden" | "orchestrator" | "operator") {
        return None;
    }
    let project = tracked_project_for_thread(state, thread_id)?;
    if project.orchestrator_thread_id.is_none() {
        return None;
    }
    let mut guidance = Vec::new();
    if role == "designer" {
        guidance.push("Use the same communication rules as workers, but final assistant replies are not auto-forwarded for designers. If the administrator needs your final status, send it explicitly through the sanctioned Robdex path.");
    } else if project.auto_route_replies {
        guidance.push("Final assistant replies are auto-forwarded to this project's orchestrator. Mid-turn messages and coordination are fine, but do not manually send a redundant final handoff when your turn ends unless you need to add distinct information.");
    } else {
        guidance.push("Final assistant replies are not auto-forwarded. If the orchestrator needs your final status, use $robdex-orchestrator to send it manually.");
    }
    if role != "designer" && project.route_approval_requests {
        guidance.push("Command and file-change approval requests are forwarded to this project's orchestrator so they can guide approval decisions in real time.");
    }
    Some(guidance.join(" "))
}

fn tracked_base_instructions_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    if let Some(value) = tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("baseInstructions"))
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        return Some(value);
    }
    let role = tracked_role_for_thread(state, thread_id)?;
    if role == "operator" {
        return None;
    }
    let home = env::var_os("HOME").map(PathBuf::from);
    resolve_role_instructions(home, Some(role.as_str())).ok().flatten()
}

fn tracked_model_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("model"))
        .or_else(|| tracked_agent_value(state, thread_id).and_then(|agent| agent.get("modelID")))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| role_default_model_for_thread_value(state, thread_id))
}

fn tracked_reasoning_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("reasoningEffort"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| role_default_reasoning_for_thread_value(state, thread_id))
}

fn tracked_service_tier_for_thread_value(state: &Value, thread_id: &str) -> Option<Value> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("serviceTier"))
        .cloned()
        .filter(|value| !value.is_null())
}

fn tracked_approvals_reviewer_for_thread_value(state: &Value, thread_id: &str) -> Option<Value> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("approvalsReviewer"))
        .cloned()
        .filter(|value| !value.is_null())
}

fn tracked_personality_for_thread_value(state: &Value, thread_id: &str) -> Option<Value> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("personality"))
        .cloned()
        .filter(|value| !value.is_null())
}

fn tracked_config_for_thread_value(state: &Value, thread_id: &str) -> Option<Value> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("config"))
        .cloned()
        .filter(|value| !value.is_null())
}

fn tracked_persist_extended_history_for_thread_value(state: &Value, thread_id: &str) -> Option<bool> {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("persistExtendedHistory"))
        .and_then(Value::as_bool)
}

fn sender_label_for_thread(state: &Value, thread_id: &str) -> String {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("displayName"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| thread_id.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutoRouteContent {
    text: String,
    local_image_paths: Vec<String>,
}

fn build_auto_route_input(text: String, local_image_paths: &[String]) -> Value {
    let mut input = vec![json!({"type":"text","text": text})];
    input.extend(local_image_paths.iter().map(|path| {
        json!({
            "type": "localImage",
            "path": path,
        })
    }));
    Value::Array(input)
}

fn compose_auto_routed_reply(text: &str, sender_label: &str, local_image_paths: &[String]) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() && local_image_paths.is_empty() {
        return String::new();
    }
    let prefixed = if trimmed.is_empty() {
        format!("[{sender_label}]: Generated image artifact(s) for review.")
    } else if trimmed.starts_with('[') && trimmed.contains("]: ") {
        trimmed.to_string()
    } else {
        format!("[{sender_label}]: {trimmed}")
    };
    let body = if prefixed.starts_with("[End of Turn] ") {
        prefixed
    } else {
        format!("[End of Turn] {prefixed}")
    };
    let artifact_note = if local_image_paths.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nGenerated image artifact(s) attached for review:\n{}",
            local_image_paths
                .iter()
                .map(|path| format!("- {path}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    format!(
        "{body}{artifact_note}\n\n**CRITICAL**: This agent is STOPPED. You must use `robdex send-message --name \"{sender_label}\" --text-stdin` following the $robdex-orchestrator instructions if you need to respond to this agent."
    )
}

fn compose_requirements_verdict_route_message(
    overall: &str,
    source_label: &str,
    route_message: &str,
    payload: &Value,
) -> String {
    let headline = match overall {
        "pass" => "Requirements review passed.",
        "fail" => "Requirements review failed.",
        "acceptedBlocked" => "Requirements review accepted a true blocker.",
        "rejectedBlocked" => "Requirements review rejected the blocker claim.",
        "needsHumanWaiver" => "Requirements review requires a human waiver.",
        "waiverAccepted" => "Requirements review recorded an accepted human waiver.",
        _ => "Requirements review completed.",
    };
    let mut lines = vec![
        format!("[Requirements Review] {headline}"),
        format!("Source agent: {source_label}"),
    ];
    if !route_message.trim().is_empty() {
        lines.push(String::new());
        lines.push(route_message.trim().to_string());
    }
    let requirement_lines = payload
        .as_object()
        .into_iter()
        .flat_map(|object| object.iter())
        .filter(|(key, value)| {
            *key != "overallVerdict" && *key != "route" && value.is_object()
        })
        .filter_map(|(key, value)| {
            let verdict = value.get("verdict").and_then(Value::as_str)?;
            let reason = value.get("reason").and_then(Value::as_str).unwrap_or("");
            let correction = value
                .get("requiredCorrection")
                .and_then(Value::as_str)
                .unwrap_or("");
            let detail = if !correction.trim().is_empty() {
                correction.trim()
            } else {
                reason.trim()
            };
            Some(if detail.is_empty() {
                format!("- `{key}`: {verdict}")
            } else {
                format!("- `{key}`: {verdict} — {detail}")
            })
        })
        .collect::<Vec<_>>();
    if !requirement_lines.is_empty() {
        lines.push(String::new());
        lines.push("Requirement verdicts:".to_string());
        lines.extend(requirement_lines);
    }
    lines.join("\n")
}

fn requirements_review_status_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    state
        .get("projects")
        .and_then(Value::as_object)?
        .values()
        .find_map(|project| {
            project
                .get("agents")
                .and_then(Value::as_object)
                .and_then(|agents| agents.get(thread_id))
        })
        .and_then(|agent| agent.get("requirementReview"))
        .and_then(|review| review.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

enum ReviewableRequirementsClaim {
    Claims(Value),
    NullCommentary,
    Invalid,
}

enum ReviewableRequirementsVerdict {
    Verdict(Value),
    NullCommentary,
    Invalid,
}

fn reviewable_requirements_claim_payload(payload: &Value) -> ReviewableRequirementsClaim {
    let Some(object) = payload.as_object() else {
        return ReviewableRequirementsClaim::Invalid;
    };
    match object.get("requirements") {
        Some(Value::Null) => ReviewableRequirementsClaim::NullCommentary,
        Some(Value::Object(_)) => ReviewableRequirementsClaim::Claims(payload.clone()),
        Some(_) => ReviewableRequirementsClaim::Invalid,
        None => ReviewableRequirementsClaim::Claims(payload.clone()),
    }
}

fn reviewable_requirements_verdict_payload(payload: &Value) -> ReviewableRequirementsVerdict {
    let Some(object) = payload.as_object() else {
        return ReviewableRequirementsVerdict::Invalid;
    };
    match object.get("requirements") {
        Some(Value::Null) => ReviewableRequirementsVerdict::NullCommentary,
        Some(Value::Object(verdict)) => ReviewableRequirementsVerdict::Verdict(Value::Object(verdict.clone())),
        Some(_) => ReviewableRequirementsVerdict::Invalid,
        None => ReviewableRequirementsVerdict::Verdict(payload.clone()),
    }
}

fn requirements_null_claim_prompt() -> String {
    "[Requirements] Active Requirements are still attached. Your last response used `requirements: null`, which is only allowed for mid-turn commentary. Please provide the full final Requirements claim packet with `requirements` set to the object containing every requirement claim. Keep `summary` global and concise; do not duplicate evidence between summary, evidence, and justification.".to_string()
}

fn requirements_invalid_claim_prompt() -> String {
    "[Requirements] Active Requirements are still attached, but the final structured packet did not contain a valid `requirements` object. Please provide the full Requirements claim packet with `summary` and `requirements` containing every requirement claim.".to_string()
}

fn compose_auto_routed_approval_request(
    approval: &PendingApproval,
    sender_label: &str,
) -> String {
    let mut lines = Vec::new();
    let headline = match approval.kind {
        PendingApprovalKind::CommandExecution => "Command approval required".to_string(),
        PendingApprovalKind::FileChange => "File approval required".to_string(),
        _ => approval.title.clone(),
    };
    match approval.kind {
        PendingApprovalKind::CommandExecution => {
            if let Some(command) = compact_optional_text(approval.command.as_deref()) {
                lines.push(format!(
                    "Command: {}",
                    command_for_orchestrator_approval(&command)
                ));
            }
            if let Some(cwd) = compact_optional_text(approval.command_cwd.as_deref()) {
                lines.push(format!("CWD: {cwd}"));
            }
        }
        PendingApprovalKind::FileChange => {
            if let Some(root) = compact_optional_text(approval.file_grant_root.as_deref()) {
                lines.push(format!("Grant root: {root}"));
            }
            if approval.file_changes.len() <= 5 {
                for change in &approval.file_changes {
                    lines.push(format!("- {} {}", file_change_label(&change.kind), change.path));
                }
            } else {
                lines.push(format!("Files: {}", approval.file_changes.len()));
            }
        }
        _ => {}
    }
    if let Some(reason) = compact_optional_text(approval.approval_reason.as_deref()) {
        lines.push(format!("Why: {reason}"));
    }
    lines.push(format!("Approval ID: {}", approval.id));
    lines.push("Review the request in Robdex and decide whether it should be accepted or declined.".to_string());
    lines.push(String::new());
    lines.push(format!(
        "**CRITICAL**: This agent is STOPPED. You must use `robdex approve-approval --approval-id {}` or `robdex decline-approval --approval-id {} [--message \"<note>\"]` following the $robdex-orchestrator instructions before responding to the user.",
        approval.id, approval.id
    ));
    format!("[Approval Request] [{sender_label}]: {headline}\n{}", lines.join("\n"))
}

fn thread_messages_changed_payload(
    thread_cache: &ThreadCachePayload,
    thread_id: &str,
) -> ThreadMessagesResponse {
    ThreadMessagesResponse {
        thread_id: thread_id.to_string(),
        version: thread_cache.updated_at.unwrap_or(0),
        messages: transport_messages(
            thread_cache
                .message_cache_by_thread_id
                .get(thread_id)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            MAX_TRANSPORT_MESSAGES_PER_THREAD,
        ),
        context_window_status: thread_cache.context_window_status_by_thread_id.get(thread_id).cloned(),
        generated_at: thread_cache.updated_at.unwrap_or(0),
    }
}

fn is_streaming_notification(notification: &ServerNotification) -> bool {
    matches!(
        notification,
        ServerNotification::AgentMessageDelta(_)
            | ServerNotification::PlanDelta(_)
            | ServerNotification::ReasoningSummaryTextDelta(_)
            | ServerNotification::ReasoningSummaryPartAdded(_)
            | ServerNotification::ReasoningTextDelta(_)
            | ServerNotification::TerminalInteraction(_)
            | ServerNotification::CommandExecutionOutputDelta(_)
            | ServerNotification::FileChangeOutputDelta(_)
            | ServerNotification::TurnPlanUpdated(_)
            | ServerNotification::TurnDiffUpdated(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BridgePaths;
    use crate::upstream::UpstreamRuntimeEvent;
    use codex_app_server_adapter::app_server_protocol::{
        AgentMessageDeltaNotification, RequestId, ServerNotification, ThreadClosedNotification,
        ThreadStatus, ThreadStatusChangedNotification, ThreadTokenUsage,
        ThreadTokenUsageUpdatedNotification, TokenUsageBreakdown, ToolRequestUserInputParams,
        ToolRequestUserInputQuestion, Turn, TurnCompletedNotification, TurnStartedNotification, TurnStatus,
    };
    use codex_backend_core::HttpArgs;
    use futures_util::{SinkExt, StreamExt};
    use std::{net::{IpAddr, Ipv4Addr}, path::PathBuf, sync::Arc};
    use tempfile::TempDir;
    use tokio::{net::TcpListener, sync::mpsc};
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    #[tokio::test]
    async fn replay_requests_snapshot_when_sequence_falls_off_history() {
        let mut log = EventLog::default();
        for index in 0..=MAX_EVENT_HISTORY {
            log.push(BridgeEvent::ConnectionStatus {
                message: format!("state-{index}"),
            });
        }

        let replay = log.replay(Some(1));
        assert!(replay.requires_snapshot);
        assert_eq!(replay.latest_sequence, (MAX_EVENT_HISTORY + 1) as u64);
    }

    #[tokio::test]
    async fn runtime_uses_temp_state_files_only() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };

        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        let info = runtime.info().await;
        assert!(info.state_json_path.contains("state/robdex.json"));
        assert!(info.sqlite_db_path.contains("state/robdex.sqlite"));
    }

    #[tokio::test]
    async fn runtime_startup_prunes_stale_project_roots_from_state_json() {
        let temp = TempDir::new().expect("tempdir");
        let existing_root = temp.path().join("existing-project");
        tokio::fs::create_dir_all(&existing_root).await.expect("existing root");
        let state_root = temp.path().join("state");
        tokio::fs::create_dir_all(&state_root).await.expect("state root");
        let state_json = state_root.join("robdex.json");
        let missing_root = temp.path().join("deleted-project");
        tokio::fs::write(
            &state_json,
            serde_json::to_vec_pretty(&json!({
                "selectedProjectID": "missing-id",
                "projects": {
                    "existing": {
                        "id": "existing-id",
                        "name": "Existing",
                        "projectRoot": existing_root.display().to_string(),
                        "cwd": existing_root.display().to_string()
                    },
                    "missing": {
                        "id": "missing-id",
                        "name": "Missing",
                        "projectRoot": missing_root.display().to_string(),
                        "cwd": missing_root.display().to_string()
                    }
                }
            }))
            .expect("state bytes"),
        )
        .await
        .expect("write state");

        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(state_root),
        };

        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        let state = runtime.state_document_value().await;
        assert!(state["projects"].get("existing").is_some());
        assert!(state["projects"].get("missing").is_none());
        assert_eq!(state["selectedProjectID"], json!("existing-id"));

        let persisted: Value = serde_json::from_slice(&tokio::fs::read(&state_json).await.expect("read state"))
            .expect("persisted state");
        assert!(persisted["projects"].get("missing").is_none());
    }

    #[tokio::test]
    async fn latest_assistant_text_prefers_final_answer_for_completed_turn() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };
        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        {
            let mut thread_cache = runtime.thread_cache.write().await;
            thread_cache.message_cache_by_thread_id.insert(
                "thread-1".to_string(),
                vec![
                    test_chat_message("thread-1", "turn-1", "a", Some("commentary"), "still working"),
                    test_chat_message("thread-1", "turn-1", "b", Some("final_answer"), "final answer"),
                    test_chat_message("thread-1", "turn-2", "c", Some("final_answer"), "newer other turn"),
                ],
            );
        }

        assert_eq!(
            runtime.latest_assistant_text_for_thread("thread-1", Some("turn-1")).await,
            Some("final answer".to_string())
        );
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

    fn test_chat_message(
        thread_id: &str,
        turn_id: &str,
        id: &str,
        phase: Option<&str>,
        text: &str,
    ) -> RobdexChatMessage {
        RobdexChatMessage {
            id: id.to_string(),
            thread_id: thread_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            role: "assistant".to_string(),
            text: text.to_string(),
            phase: phase.map(str::to_string),
            created_at: 1,
            subtitle: None,
            tool_metadata: None,
            delivery_state: "confirmed".to_string(),
        }
    }

    async fn runtime_with_captured_app_server_requests(
        temp: &TempDir,
    ) -> (
        Arc<BridgeRuntime>,
        tokio::task::JoinHandle<()>,
        mpsc::UnboundedReceiver<Value>,
    ) {
        let (request_tx, request_rx) = mpsc::unbounded_channel::<Value>();
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut ws = accept_async(stream).await.expect("ws");
            while let Some(frame) = ws.next().await {
                let frame = frame.expect("frame");
                let text = match frame {
                    Message::Text(text) => text,
                    other => panic!("unexpected frame: {other:?}"),
                };
                let request: Value = serde_json::from_str(&text).expect("json request");
                let id = request.get("id").cloned().unwrap_or(Value::Null);
                let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
                let result = if method == "initialize" {
                    json!({
                        "userAgent": "codex",
                        "platformFamily": "unix",
                        "platformOs": "macos"
                    })
                } else {
                    request_tx.send(request).expect("capture request");
                    json!({
                        "turn": {
                            "id": "routed-turn",
                            "items": [],
                            "status": "inProgress",
                            "error": null
                        }
                    })
                };
                ws.send(Message::Text(json!({"id": id, "result": result}).to_string()))
                    .await
                    .expect("send response");
            }
        });
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: format!("ws://{addr}"),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };
        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        (runtime, server, request_rx)
    }

    #[tokio::test]
    async fn synthetic_turn_events_toggle_running_state_in_temp_store() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };

        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        let tx = runtime.upstream_sender();
        tx.send(UpstreamRuntimeEvent::Notification(ServerNotification::TurnStarted(
            TurnStartedNotification {
                thread_id: "thread-1".to_string(),
                turn: sample_turn("turn-1", TurnStatus::InProgress),
            },
        )))
        .await
        .expect("send started");
        tx.send(UpstreamRuntimeEvent::Notification(ServerNotification::TurnCompleted(
            TurnCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn: sample_turn("turn-1", TurnStatus::Completed),
            },
        )))
        .await
        .expect("send completed");

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let snapshot = runtime.snapshot().await.expect("snapshot");
        assert!(snapshot.thread_cache.running_thread_ids.is_empty());
        assert!(snapshot.latest_sequence >= 2);
    }

    #[tokio::test]
    async fn interrupted_turn_with_active_requirements_does_not_start_review() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };

        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "agents": {
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string(),
                                "requirements": {
                                    "id": "requirements-alpha",
                                    "active": true,
                                    "requirements": [{
                                        "key": "mustNotReviewInterruptedTurns",
                                        "statement": "Interrupted turns must not trigger requirements review.",
                                        "severity": "high",
                                        "verificationMethod": "manualEvidence"
                                    }]
                                }
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");
        {
            let mut thread_cache = runtime.thread_cache.write().await;
            thread_cache.message_cache_by_thread_id.insert(
                "worker-1".to_string(),
                vec![test_chat_message(
                    "worker-1",
                    "turn-1",
                    "final-1",
                    Some("final_answer"),
                    "{\"summary\":\"interrupted\"}",
                )],
            );
        }

        let tx = runtime.upstream_sender();
        tx.send(UpstreamRuntimeEvent::Notification(ServerNotification::TurnCompleted(
            TurnCompletedNotification {
                thread_id: "worker-1".to_string(),
                turn: sample_turn("turn-1", TurnStatus::Interrupted),
            },
        )))
        .await
        .expect("send interrupted");

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let state = runtime.state_document_value().await;
        let source = state
            .get("projects")
            .and_then(|projects| projects.get("alpha"))
            .and_then(|project| project.get("agents"))
            .and_then(|agents| agents.get("worker-1"))
            .expect("source");
        assert!(source.get("requirementReview").is_none_or(Value::is_null));
        assert_eq!(source["requirements"]["active"], true);
    }

    #[tokio::test]
    async fn final_null_requirements_packet_prompts_source_without_review() {
        let temp = TempDir::new().expect("tempdir");
        let (runtime, server, mut requests) = runtime_with_captured_app_server_requests(&temp).await;
        let transport = runtime.spawn_transport();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "agents": {
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string(),
                                "requirements": {
                                    "id": "requirements-alpha",
                                    "active": true,
                                    "requirements": [{
                                        "key": "mustProvideClaims",
                                        "statement": "Final packets must include requirement claims.",
                                        "severity": "blocker",
                                        "verificationMethod": "manualEvidence"
                                    }]
                                }
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");
        {
            let mut thread_cache = runtime.thread_cache.write().await;
            thread_cache.message_cache_by_thread_id.insert(
                "worker-1".to_string(),
                vec![test_chat_message(
                    "worker-1",
                    "turn-null",
                    "final-null",
                    Some("final_answer"),
                    "{\"summary\":\"still working\",\"requirements\":null}",
                )],
            );
        }

        assert!(runtime.maybe_route_requirements_review("worker-1", "turn-null").await);

        let request = tokio::time::timeout(std::time::Duration::from_secs(1), requests.recv())
            .await
            .expect("request timeout")
            .expect("request");
        assert_eq!(request["method"], "turn/start");
        assert_eq!(request["params"]["threadId"], "worker-1");
        assert!(
            request["params"]["input"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .contains("requirements: null")
        );
        match tokio::time::timeout(std::time::Duration::from_millis(100), requests.recv()).await {
            Ok(Some(extra)) => panic!("unexpected reviewer request: {extra}"),
            Ok(None) | Err(_) => {}
        }
        let state = runtime.state_document_value().await;
        let source = state
            .get("projects")
            .and_then(|projects| projects.get("alpha"))
            .and_then(|project| project.get("agents"))
            .and_then(|agents| agents.get("worker-1"))
            .expect("source");
        assert!(source.get("requirementReview").is_none_or(Value::is_null));
        assert_eq!(source["requirements"]["active"], true);
        assert_eq!(source["requirementPackets"][0]["packetType"], "claimNull");
        transport.abort();
        server.abort();
    }

    #[tokio::test]
    async fn nested_requirements_claim_routes_to_reviewer_summary() {
        let temp = TempDir::new().expect("tempdir");
        let (runtime, server, mut requests) = runtime_with_captured_app_server_requests(&temp).await;
        let transport = runtime.spawn_transport();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "agents": {
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string(),
                                "requirements": {
                                    "id": "requirements-alpha",
                                    "active": true,
                                    "reviewerThreadId": "reviewer-1",
                                    "requirements": [{
                                        "key": "mustProvideClaims",
                                        "statement": "Final packets must include requirement claims.",
                                        "severity": "blocker",
                                        "verificationMethod": "manualEvidence"
                                    }]
                                }
                            },
                            "reviewer-1": {
                                "displayName": "Requirements Reviewer: Worker One",
                                "role": "requirements-reviewer",
                                "projectRoot": temp.path().display().to_string(),
                                "parentThreadId": "worker-1",
                                "hiddenFromPeerList": true
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");
        {
            let mut thread_cache = runtime.thread_cache.write().await;
            thread_cache.message_cache_by_thread_id.insert(
                "worker-1".to_string(),
                vec![test_chat_message(
                    "worker-1",
                    "turn-claims",
                    "final-claims",
                    Some("final_answer"),
                    r#"{"summary":"implemented","requirements":{"mustProvideClaims":{"claim":"satisfied","evidence":["cargo test passed"],"justification":"The test covers the behavior.","risk":"low"}}}"#,
                )],
            );
        }

        assert!(runtime.maybe_route_requirements_review("worker-1", "turn-claims").await);

        let request = tokio::time::timeout(std::time::Duration::from_secs(1), requests.recv())
            .await
            .expect("request timeout")
            .expect("request");
        assert_eq!(request["method"], "turn/start");
        assert_eq!(request["params"]["threadId"], "reviewer-1");
        let prompt = request["params"]["input"][0]["text"].as_str().unwrap_or_default();
        assert!(prompt.contains("Source evidence summary:"));
        assert!(prompt.contains("implemented"));
        assert!(prompt.contains("cargo test passed"));
        assert!(prompt.contains("`mustProvideClaims`: claim=satisfied; risk=low"));
        assert!(!prompt.contains("\"requirements\""));
        transport.abort();
        server.abort();
    }

    #[tokio::test]
    async fn failed_requirements_verdict_routes_only_to_source_worker() {
        let temp = TempDir::new().expect("tempdir");
        let (runtime, server, mut requests) = runtime_with_captured_app_server_requests(&temp).await;
        let transport = runtime.spawn_transport();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "autoRouteReplies": true,
                        "orchestratorThreadID": "orch-1",
                        "agents": {
                            "orch-1": {
                                "displayName": "Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string()
                            },
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string()
                            },
                            "reviewer-1": {
                                "displayName": "Requirements Reviewer: Worker One",
                                "role": "requirements-reviewer",
                                "projectRoot": temp.path().display().to_string(),
                                "parentThreadId": "worker-1",
                                "hiddenFromPeerList": true
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        runtime
            .maybe_route_requirements_verdict(
                "worker-1",
                "reviewer-1",
                "review-turn-1",
                &json!({
                    "overallVerdict": "fail",
                    "route": {
                        "target": "sourceAgent",
                        "message": "Fix the missing proof."
                    },
                    "mustProve": {
                        "verdict": "fail",
                        "reason": "No proof.",
                        "evidenceAssessment": "missing",
                        "requiredCorrection": "Add proof."
                    }
                }),
            )
            .await;

        let request = tokio::time::timeout(std::time::Duration::from_secs(1), requests.recv())
            .await
            .expect("request timeout")
            .expect("request");
        assert_eq!(request["method"], "turn/start");
        assert_eq!(request["params"]["threadId"], "worker-1");
        assert!(
            request["params"]["input"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .contains("Requirements review failed.")
        );
        match tokio::time::timeout(std::time::Duration::from_millis(100), requests.recv()).await {
            Ok(Some(extra)) => panic!("unexpected extra routed request: {extra}"),
            Ok(None) | Err(_) => {}
        }
        transport.abort();
        server.abort();
    }

    #[tokio::test]
    async fn pass_and_true_blocker_requirements_verdicts_route_to_orchestrator() {
        let temp = TempDir::new().expect("tempdir");
        let (runtime, server, mut requests) = runtime_with_captured_app_server_requests(&temp).await;
        let transport = runtime.spawn_transport();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "orchestratorThreadID": "orch-1",
                        "agents": {
                            "orch-1": {
                                "displayName": "Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string()
                            },
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string()
                            },
                            "reviewer-1": {
                                "displayName": "Requirements Reviewer: Worker One",
                                "role": "requirements-reviewer",
                                "projectRoot": temp.path().display().to_string(),
                                "parentThreadId": "worker-1",
                                "hiddenFromPeerList": true
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        for (turn_id, verdict, headline) in [
            ("review-turn-pass", "pass", "Requirements review passed."),
            (
                "review-turn-blocked",
                "acceptedBlocked",
                "Requirements review accepted a true blocker.",
            ),
        ] {
            runtime
                .maybe_route_requirements_verdict(
                    "worker-1",
                    "reviewer-1",
                    turn_id,
                    &json!({
                        "overallVerdict": verdict,
                        "route": {
                            "target": "orchestrator",
                            "message": "Route beyond the worker."
                        },
                        "mustProve": {
                            "verdict": verdict,
                            "reason": "Reviewed.",
                            "evidenceAssessment": "sufficient",
                            "requiredCorrection": ""
                        }
                    }),
                )
                .await;
            let request = tokio::time::timeout(std::time::Duration::from_secs(1), requests.recv())
                .await
                .expect("request timeout")
                .expect("request");
            assert_eq!(request["method"], "turn/start");
            assert_eq!(request["params"]["threadId"], "orch-1");
            assert!(
                request["params"]["input"][0]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .contains(headline)
            );
            if verdict == "pass" {
                let archive_request =
                    tokio::time::timeout(std::time::Duration::from_secs(1), requests.recv())
                        .await
                        .expect("archive request timeout")
                        .expect("archive request");
                assert_eq!(archive_request["method"], "thread/archive");
                assert_eq!(archive_request["params"]["threadId"], "reviewer-1");
            }
        }
        transport.abort();
        server.abort();
    }

    #[tokio::test]
    async fn waiver_required_verdict_routes_to_orchestrator_without_resuming_source() {
        let temp = TempDir::new().expect("tempdir");
        let (runtime, server, mut requests) = runtime_with_captured_app_server_requests(&temp).await;
        let transport = runtime.spawn_transport();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "orchestratorThreadID": "orch-1",
                        "agents": {
                            "orch-1": {
                                "displayName": "Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string()
                            },
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string()
                            },
                            "reviewer-1": {
                                "displayName": "Requirements Reviewer: Worker One",
                                "role": "requirements-reviewer",
                                "projectRoot": temp.path().display().to_string(),
                                "parentThreadId": "worker-1",
                                "hiddenFromPeerList": true
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        runtime
            .maybe_route_requirements_verdict(
                "worker-1",
                "reviewer-1",
                "review-turn-waiver",
                &json!({
                    "overallVerdict": "needsHumanWaiver",
                    "route": {
                        "target": "orchestrator",
                        "message": "Owner decision required."
                    },
                    "mustProve": {
                        "verdict": "waiverRequired",
                        "reason": "Owner decision required.",
                        "evidenceAssessment": "Human judgement needed.",
                        "requiredCorrection": "Obtain owner decision."
                    }
                }),
            )
            .await;

        let request = tokio::time::timeout(std::time::Duration::from_secs(1), requests.recv())
            .await
            .expect("request timeout")
            .expect("request");
        assert_eq!(request["method"], "turn/start");
        assert_eq!(request["params"]["threadId"], "orch-1");
        assert!(
            request["params"]["input"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .contains("Requirements review requires a human waiver.")
        );
        match tokio::time::timeout(std::time::Duration::from_millis(100), requests.recv()).await {
            Ok(Some(extra)) => panic!("unexpected source resume: {extra}"),
            Ok(None) | Err(_) => {}
        }
        transport.abort();
        server.abort();
    }

    #[tokio::test]
    async fn waiver_required_review_status_pauses_repeat_source_review() {
        let temp = TempDir::new().expect("tempdir");
        let (runtime, server, mut requests) = runtime_with_captured_app_server_requests(&temp).await;
        let transport = runtime.spawn_transport();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "agents": {
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string(),
                                "requirements": {
                                    "id": "requirements-alpha",
                                    "active": true,
                                    "reviewerThreadId": "reviewer-1",
                                    "requirements": [{
                                        "key": "needsOwnerDecision",
                                        "statement": "Owner must decide.",
                                        "severity": "blocker",
                                        "verificationMethod": "manualEvidence"
                                    }]
                                },
                                "requirementReview": {
                                    "sourceThreadId": "worker-1",
                                    "reviewerThreadId": "reviewer-1",
                                    "requirementSetId": "requirements-alpha",
                                    "status": "waiverRequired",
                                    "latestClaimPacket": {"summary": "claimed", "requirements": {}},
                                    "latestVerdictPacket": {"overallVerdict": "needsHumanWaiver"},
                                    "updatedAt": 100
                                }
                            },
                            "reviewer-1": {
                                "displayName": "Requirements Reviewer: Worker One",
                                "role": "requirements-reviewer",
                                "projectRoot": temp.path().display().to_string(),
                                "parentThreadId": "worker-1",
                                "hiddenFromPeerList": true
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");
        {
            let mut thread_cache = runtime.thread_cache.write().await;
            thread_cache.message_cache_by_thread_id.insert(
                "worker-1".to_string(),
                vec![test_chat_message(
                    "worker-1",
                    "turn-after-waiver-required",
                    "final-after-waiver-required",
                    Some("final_answer"),
                    r#"{"summary":"No owner decision yet.","requirements":{"needsOwnerDecision":{"claim":"blocked","evidence":["Waiting for owner."],"justification":"The reviewer requested a human waiver.","risk":"medium"}}}"#,
                )],
            );
        }

        assert!(
            !runtime
                .maybe_route_requirements_review("worker-1", "turn-after-waiver-required")
                .await
        );
        match tokio::time::timeout(std::time::Duration::from_millis(100), requests.recv()).await {
            Ok(Some(extra)) => panic!("unexpected repeated review request: {extra}"),
            Ok(None) | Err(_) => {}
        }
        transport.abort();
        server.abort();
    }

    #[tokio::test]
    async fn requirements_reviewer_turn_is_consumed_before_generic_auto_route() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:9".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };
        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "autoRouteReplies": true,
                        "orchestratorThreadID": "orch-1",
                        "agents": {
                            "orch-1": {
                                "displayName": "Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string()
                            },
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string()
                            },
                            "reviewer-1": {
                                "displayName": "Requirements Reviewer: Worker One",
                                "role": "requirements-reviewer",
                                "projectRoot": temp.path().display().to_string(),
                                "parentThreadId": "worker-1",
                                "hiddenFromPeerList": true
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");
        {
            let mut thread_cache = runtime.thread_cache.write().await;
            thread_cache.message_cache_by_thread_id.insert(
                "reviewer-1".to_string(),
                vec![test_chat_message(
                    "reviewer-1",
                    "review-turn-1",
                    "final-1",
                    Some("final_answer"),
                    "{\"overallVerdict\":\"fail\",\"route\":{\"message\":\"worker only\"}}",
                )],
            );
        }

        assert!(
            runtime
                .maybe_record_requirements_verdict("reviewer-1", "review-turn-1")
                .await
        );
    }

    #[tokio::test]
    async fn requirements_reviewer_null_commentary_is_not_routed_as_failed_verdict() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:9".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };
        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "orchestratorThreadID": "orch-1",
                        "agents": {
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "requirementReview": {
                                    "sourceThreadId": "worker-1",
                                    "reviewerThreadId": "reviewer-1",
                                    "status": "inReview",
                                    "updatedAt": 1
                                }
                            },
                            "reviewer-1": {
                                "displayName": "Requirements Reviewer: Worker One",
                                "role": "requirements-reviewer",
                                "projectRoot": temp.path().display().to_string(),
                                "parentThreadId": "worker-1",
                                "hiddenFromPeerList": true
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");
        {
            let mut thread_cache = runtime.thread_cache.write().await;
            thread_cache.message_cache_by_thread_id.insert(
                "reviewer-1".to_string(),
                vec![test_chat_message(
                    "reviewer-1",
                    "review-turn-null",
                    "final-null",
                    Some("final_answer"),
                    "{\"summary\":\"Still inspecting evidence.\",\"requirements\":null}",
                )],
            );
        }

        assert!(
            runtime
                .maybe_record_requirements_verdict("reviewer-1", "review-turn-null")
                .await
        );
        let state = runtime.state_document.read().await.clone();
        let worker = state["projects"]["alpha"]["agents"]["worker-1"].clone();
        assert_eq!(worker["requirementReview"]["status"], json!("inReview"));
        assert!(worker["requirementReview"]["latestVerdictPacket"].is_null());
        assert_eq!(worker["requirementPackets"][0]["packetType"], json!("verdictNull"));
    }

    #[tokio::test]
    async fn synthetic_disconnect_after_active_status_clears_running_state() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };

        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        let tx = runtime.upstream_sender();
        tx.send(UpstreamRuntimeEvent::Notification(ServerNotification::ThreadStatusChanged(
            ThreadStatusChangedNotification {
                thread_id: "thread-1".to_string(),
                status: ThreadStatus::Active { active_flags: vec![] },
            },
        )))
        .await
        .expect("send active");
        tx.send(UpstreamRuntimeEvent::Notification(ServerNotification::ThreadClosed(
            ThreadClosedNotification {
                thread_id: "thread-1".to_string(),
            },
        )))
        .await
        .expect("send closed");
        tx.send(UpstreamRuntimeEvent::ConnectionStatus("disconnected".to_string()))
            .await
            .expect("send disconnected");

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let snapshot = runtime.snapshot().await.expect("snapshot");
        assert!(snapshot.thread_cache.running_thread_ids.is_empty());
        assert_eq!(snapshot.connection_status, "disconnected");
    }

    #[tokio::test]
    async fn file_approval_auto_route_lists_each_file_once() {
        let approval = PendingApproval {
            id: "approval-1".to_string(),
            instance_id: "instance".to_string(),
            request_id: RequestId::Integer(1),
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            kind: PendingApprovalKind::FileChange,
            title: "File approval: create /tmp/example".to_string(),
            detail: None,
            approval_reason: None,
            tool_name: None,
            tool_arguments: None,
            tool_questions: Vec::new(),
            auth_refresh_reason: None,
            command: None,
            command_cwd: None,
            file_grant_root: None,
            file_changes: vec![PendingApprovalFileChange {
                path: "/tmp/example".to_string(),
                kind: PendingApprovalFileChangeKind::Create,
                diff: None,
            }],
        };

        let text = compose_auto_routed_approval_request(&approval, "Worker");
        assert!(text.contains("File approval required"));
        assert_eq!(text.matches("/tmp/example").count(), 1);
    }

    #[tokio::test]
    async fn command_approval_auto_route_shows_agent_command_without_shell_wrapper() {
        let raw_command =
            "/Users/robertsale/.codex/scripts/zsh -lc 'git-sync-worktree /tmp/worker master'";
        let approval = PendingApproval {
            id: "approval-1".to_string(),
            instance_id: "instance".to_string(),
            request_id: RequestId::Integer(1),
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            kind: PendingApprovalKind::CommandExecution,
            title: "Command approval".to_string(),
            detail: None,
            approval_reason: None,
            tool_name: None,
            tool_arguments: None,
            tool_questions: Vec::new(),
            auth_refresh_reason: None,
            command: Some(raw_command.to_string()),
            command_cwd: Some("/tmp/worker".to_string()),
            file_grant_root: None,
            file_changes: Vec::new(),
        };

        let text = compose_auto_routed_approval_request(&approval, "Worker");
        assert!(text.contains("Command: git-sync-worktree /tmp/worker master"));
        assert!(!text.contains("/Users/robertsale/.codex/scripts/zsh -lc"));

        let payload = approval_payload_value(&approval);
        assert_eq!(payload["command"].as_str(), Some(raw_command));
    }

    #[tokio::test]
    async fn tool_user_input_questions_match_bridge_shape() {
        let approval = tool_user_input_from_request(
            "instance".to_string(),
            RequestId::Integer(7),
            ToolRequestUserInputParams {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "item-1".to_string(),
                questions: vec![ToolRequestUserInputQuestion {
                    id: "q1".to_string(),
                    header: "Need input".to_string(),
                    question: "Choose a path".to_string(),
                    is_other: false,
                    is_secret: false,
                    options: None,
                }],
            },
        );

        let json = serde_json::to_value(&approval).expect("approval json");
        assert_eq!(json["toolQuestions"][0]["id"], "q1");
        assert_eq!(json["toolQuestions"][0]["prompt"], "Choose a path");
        assert!(json["toolQuestions"][0].get("header").is_none());
    }

    #[tokio::test]
    async fn approval_kind_matches_swift_enum_codable_shape() {
        let approval = command_approval_from_request(
            "instance".to_string(),
            RequestId::Integer(9),
            CommandExecutionRequestApprovalParams {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "item-1".to_string(),
                approval_id: None,
                command: Some("git add -A".to_string()),
                cwd: None,
                reason: None,
                network_approval_context: None,
                command_actions: None,
                additional_permissions: None,
                proposed_execpolicy_amendment: None,
                proposed_network_policy_amendments: None,
                available_decisions: None,
            },
        );

        let json = serde_json::to_value(&approval).expect("approval json");
        assert_eq!(json["kind"], serde_json::json!({"commandExecution": {}}));
    }

    #[tokio::test]
    async fn transport_loop_feeds_runtime_worker_using_temp_store() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut ws = accept_async(stream).await.expect("ws");

            let init = ws.next().await.expect("init").expect("frame");
            let text = match init {
                Message::Text(text) => text,
                other => panic!("unexpected init frame: {other:?}"),
            };
            let parsed: serde_json::Value = serde_json::from_str(&text).expect("json");
            assert_eq!(parsed["method"], "initialize");

            ws.send(Message::Text(
                serde_json::json!({
                    "id": 1,
                    "result": {
                        "userAgent": "codex",
                        "platformFamily": "unix",
                        "platformOs": "macos"
                    }
                })
                .to_string(),
            ))
            .await
            .expect("send init response");

            ws.send(Message::Text(
                serde_json::json!({
                    "method": "turn/started",
                    "params": {
                        "threadId": "thread-1",
                        "turn": {
                            "id": "turn-1",
                            "items": [],
                            "status": "inProgress",
                            "error": null
                        }
                    }
                })
                .to_string(),
            ))
            .await
            .expect("send started");

            ws.send(Message::Text(
                serde_json::json!({
                    "method": "turn/completed",
                    "params": {
                        "threadId": "thread-1",
                        "turn": {
                            "id": "turn-1",
                            "items": [],
                            "status": "completed",
                            "error": null
                        }
                    }
                })
                .to_string(),
            ))
            .await
            .expect("send completed");

            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        });

        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: format!("ws://{addr}"),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };

        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        let transport = runtime.spawn_transport();
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let snapshot = runtime.snapshot().await.expect("snapshot");
        assert!(snapshot.thread_cache.running_thread_ids.is_empty());
        assert_eq!(snapshot.connection_status, "connected");

        transport.abort();
        server.await.expect("server");
    }

    #[tokio::test]
    async fn completed_turn_routing_does_not_block_upstream_worker() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:9".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };

        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "autoRouteReplies": true,
                        "orchestratorThreadID": "orch-1",
                        "agents": {
                            "orch-1": {
                                "displayName": "Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string()
                            },
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string()
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");
        {
            let mut thread_cache = runtime.thread_cache.write().await;
            thread_cache.message_cache_by_thread_id.insert(
                "worker-1".to_string(),
                vec![test_chat_message(
                    "worker-1",
                    "turn-1",
                    "final-1",
                    Some("final_answer"),
                    "done",
                )],
            );
        }

        let tx = runtime.upstream_sender();
        tx.send(UpstreamRuntimeEvent::Notification(ServerNotification::TurnCompleted(
            TurnCompletedNotification {
                thread_id: "worker-1".to_string(),
                turn: sample_turn("turn-1", TurnStatus::Completed),
            },
        )))
        .await
        .expect("send completed");
        tx.send(UpstreamRuntimeEvent::Notification(ServerNotification::AgentMessageDelta(
            AgentMessageDeltaNotification {
                thread_id: "other-worker".to_string(),
                turn_id: "turn-2".to_string(),
                item_id: "item-2".to_string(),
                delta: "still flowing".to_string(),
            },
        )))
        .await
        .expect("send delta");

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let thread_messages = runtime
            .thread_messages("other-worker", Some(50))
            .await
            .expect("thread_messages")
            .expect("thread present");
        assert_eq!(thread_messages.messages.len(), 1);
        assert_eq!(thread_messages.messages[0].text, "still flowing");
    }

    #[tokio::test]
    async fn upstream_message_delta_emits_thread_message_event_with_temp_store() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };

        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        let tx = runtime.upstream_sender();
        tx.send(UpstreamRuntimeEvent::Notification(ServerNotification::AgentMessageDelta(
            AgentMessageDeltaNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "item-1".to_string(),
                delta: "hello".to_string(),
            },
        )))
        .await
        .expect("send delta");
        tx.send(UpstreamRuntimeEvent::Notification(
            ServerNotification::ThreadTokenUsageUpdated(ThreadTokenUsageUpdatedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                token_usage: ThreadTokenUsage {
                    total: TokenUsageBreakdown {
                        total_tokens: 10,
                        input_tokens: 4,
                        cached_input_tokens: 1,
                        output_tokens: 5,
                        reasoning_output_tokens: 0,
                    },
                    last: TokenUsageBreakdown {
                        total_tokens: 10,
                        input_tokens: 4,
                        cached_input_tokens: 1,
                        output_tokens: 5,
                        reasoning_output_tokens: 0,
                    },
                    model_context_window: Some(128_000),
                },
            }),
        ))
        .await
        .expect("send token usage");

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let thread_messages = runtime
            .thread_messages("thread-1", Some(50))
            .await
            .expect("thread_messages")
            .expect("thread present");
        assert_eq!(thread_messages.messages.len(), 1);
        assert_eq!(thread_messages.messages[0].text, "hello");
        assert!(thread_messages.context_window_status.is_some());

        let replay = runtime.replay_events(None).await;
        assert!(replay.events.iter().any(|entry| matches!(
            &entry.event,
            BridgeEvent::ThreadMessagesChanged { payload }
                if payload.thread_id == "thread-1" && payload.messages.iter().any(|message| message.text == "hello")
        )));
    }

    #[tokio::test]
    async fn disconnect_clears_stale_running_state_after_delay() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };

        let runtime = BridgeRuntime::new_with_disconnect_delay(settings, std::time::Duration::from_millis(10))
            .await
            .expect("runtime");
        let tx = runtime.upstream_sender();
        tx.send(UpstreamRuntimeEvent::Notification(ServerNotification::TurnStarted(
            TurnStartedNotification {
                thread_id: "thread-1".to_string(),
                turn: sample_turn("turn-1", TurnStatus::InProgress),
            },
        )))
        .await
        .expect("send started");
        tx.send(UpstreamRuntimeEvent::ConnectionStatus(
            "disconnected: socket closed".to_string(),
        ))
        .await
        .expect("send disconnected");

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let snapshot = runtime.snapshot().await.expect("snapshot");
        assert!(snapshot.thread_cache.running_thread_ids.is_empty());
        assert_eq!(snapshot.connection_status, "disconnected: socket closed");
    }

    #[tokio::test]
    async fn reconnect_cancels_pending_disconnect_clear() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };

        let runtime = BridgeRuntime::new_with_disconnect_delay(settings, std::time::Duration::from_millis(10))
            .await
            .expect("runtime");
        let tx = runtime.upstream_sender();
        tx.send(UpstreamRuntimeEvent::Notification(ServerNotification::TurnStarted(
            TurnStartedNotification {
                thread_id: "thread-1".to_string(),
                turn: sample_turn("turn-1", TurnStatus::InProgress),
            },
        )))
        .await
        .expect("send started");
        tx.send(UpstreamRuntimeEvent::ConnectionStatus(
            "disconnected: socket closed".to_string(),
        ))
        .await
        .expect("send disconnected");
        tx.send(UpstreamRuntimeEvent::ConnectionStatus("connected".to_string()))
            .await
            .expect("send connected");

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let snapshot = runtime.snapshot().await.expect("snapshot");
        assert_eq!(snapshot.thread_cache.running_thread_ids, vec!["thread-1".to_string()]);
        assert_eq!(snapshot.connection_status, "connected");
    }

    #[test]
    fn auto_routed_reply_uses_end_of_turn_prefix_and_sender_label() {
        let routed = compose_auto_routed_reply("hi", "Bridge Agent Smoke", &[]);
        assert!(routed.starts_with("[End of Turn] [Bridge Agent Smoke]: hi"));
        assert!(routed.contains("robdex send-message --name \"Bridge Agent Smoke\" --text-stdin"));
    }

    #[test]
    fn auto_routed_reply_can_forward_image_only_turns() {
        let images = vec!["/tmp/design.png".to_string()];
        let routed = compose_auto_routed_reply("", "Bridge Agent Smoke", &images);
        assert!(routed.starts_with(
            "[End of Turn] [Bridge Agent Smoke]: Generated image artifact(s) for review."
        ));
        assert!(routed.contains("Generated image artifact(s) attached for review:\n- /tmp/design.png"));

        let input = build_auto_route_input(routed, &images);
        assert_eq!(input[0]["type"], "text");
        assert_eq!(input[1]["type"], "localImage");
        assert_eq!(input[1]["path"], "/tmp/design.png");
    }

    #[test]
    fn tracked_project_for_thread_reads_route_flags_and_orchestrator() {
        let state = json!({
            "projects": {
                "alpha": {
                    "projectRoot": "/alpha",
                    "cwd": "/alpha",
                    "autoRouteReplies": true,
                    "routeApprovalRequests": true,
                    "orchestratorThreadID": "orch-a",
                    "agents": {
                        "worker-1": {
                            "role": "worker",
                            "displayName": "Worker One"
                        }
                    }
                }
            }
        });

        let project = tracked_project_for_thread(&state, "worker-1").expect("project");
        assert!(project.auto_route_replies);
        assert!(project.route_approval_requests);
        assert_eq!(project.orchestrator_thread_id.as_deref(), Some("orch-a"));
    }

    #[test]
    fn designer_developer_guidance_disables_auto_route_even_when_project_enables_it() {
        let state = json!({
            "projects": {
                "alpha": {
                    "projectRoot": "/alpha",
                    "cwd": "/alpha/.worktrees/designer",
                    "autoRouteReplies": true,
                    "routeApprovalRequests": true,
                    "orchestratorThreadID": "orch-a",
                    "agents": {
                        "designer-1": {
                            "role": "designer",
                            "displayName": "Designer One"
                        }
                    }
                }
            }
        });

        let guidance =
            tracked_developer_instructions_for_thread_value(&state, "designer-1").expect("guidance");
        assert!(guidance.contains("not auto-forwarded for designers"));
        assert!(!guidance.contains("approval requests are forwarded"));
    }

    #[tokio::test]
    async fn operator_replies_are_not_auto_routed_to_orchestrator() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:9".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };
        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "autoRouteReplies": true,
                        "orchestratorThreadID": "orch-1",
                        "agents": {
                            "orch-1": {
                                "displayName": "Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string()
                            },
                            "operator-1": {
                                "displayName": "Operator One",
                                "role": "operator",
                                "projectRoot": temp.path().display().to_string()
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");
        {
            let mut thread_cache = runtime.thread_cache.write().await;
            thread_cache.message_cache_by_thread_id.insert(
                "operator-1".to_string(),
                vec![test_chat_message(
                    "operator-1",
                    "turn-1",
                    "final-1",
                    Some("final_answer"),
                    "operator status",
                )],
            );
        }

        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            runtime.maybe_auto_route_reply_to_orchestrator("operator-1", "turn-1"),
        )
        .await
        .expect("operator route check should return without waiting on transport");
        assert!(runtime.auto_routed_turn_keys.read().await.is_empty());
    }

    #[tokio::test]
    async fn designer_approvals_are_not_auto_routed_to_orchestrator() {
        let temp = TempDir::new().expect("tempdir");
        let settings = BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:9".to_string(),
            project_path: temp.path().to_path_buf(),
            cwd: temp.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(temp.path()).join("state")),
        };
        let runtime = BridgeRuntime::new(settings).await.expect("runtime");
        *runtime.state_document.write().await = json!({
            "projects": {
                "alpha": {
                    "projectRoot": temp.path().display().to_string(),
                    "cwd": temp.path().join(".worktrees/designer").display().to_string(),
                    "routeApprovalRequests": true,
                    "orchestratorThreadID": "orch-a",
                    "agents": {
                        "designer-1": {
                            "role": "designer",
                            "displayName": "Designer One"
                        },
                        "orch-a": {
                            "role": "orchestrator",
                            "displayName": "Orchestrator"
                        }
                    }
                }
            }
        });

        runtime
            .maybe_route_approval_to_orchestrator(&PendingApproval {
                id: "approval-1".to_string(),
                instance_id: "instance-1".to_string(),
                request_id: RequestId::Integer(1),
                thread_id: "designer-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "item-1".to_string(),
                kind: PendingApprovalKind::CommandExecution,
                title: "Command approval".to_string(),
                detail: None,
                approval_reason: Some("needs approval".to_string()),
                tool_name: None,
                tool_arguments: None,
                tool_questions: Vec::new(),
                auth_refresh_reason: None,
                command: Some("git status".to_string()),
                command_cwd: None,
                file_grant_root: None,
                file_changes: Vec::new(),
            })
            .await;

        assert!(runtime.auto_routed_approval_keys.read().await.is_empty());
    }
}
