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
        FileUpdateChange, McpServerElicitationRequestParams, PatchChangeKind, PermissionsRequestApprovalParams,
        RequestId, ServerNotification, ServerRequest, ToolRequestUserInputParams,
    },
    pinned_codex_version_label,
};
use serde_json::{Value, json};
use tokio::{
    sync::{Mutex, RwLock, broadcast, mpsc, oneshot},
    task::JoinHandle,
};

use crate::{
    config::BridgeSettings,
    models::{
        BridgeApprovalResult, BridgeEvent, BridgeInfo, BridgeSnapshot, BridgeToolQuestion, EventReplayResponse,
        MAX_EVENT_HISTORY, MAX_TRANSPORT_MESSAGES_PER_THREAD, PROTOCOL_VERSION, PendingApproval,
        PendingApprovalFileChange, PendingApprovalFileChangeKind, PendingApprovalKind, SERVER_NAME, SERVER_VERSION, SequencedEvent,
        ThreadCachePayload, ThreadMessagesResponse,
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
    disconnect_running_state_clear_delay: Duration,
    disconnect_clear_task: Mutex<Option<JoinHandle<()>>>,
    pending_thread_cache_flush_ids: Mutex<BTreeSet<String>>,
    thread_cache_flush_delay: Duration,
    thread_cache_flush_task: Mutex<Option<JoinHandle<()>>>,
    state_mutation_lock: Mutex<()>,
    next_transport_request_id: AtomicU64,
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
            disconnect_running_state_clear_delay,
            disconnect_clear_task: Mutex::new(None),
            pending_thread_cache_flush_ids: Mutex::new(BTreeSet::new()),
            thread_cache_flush_delay: Duration::from_millis(THREAD_CACHE_FLUSH_DEBOUNCE_MS),
            thread_cache_flush_task: Mutex::new(None),
            state_mutation_lock: Mutex::new(()),
            next_transport_request_id: AtomicU64::new(10_000),
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
        let running_thread_ids = thread_cache.running_thread_ids.clone();
        let pending_approvals = self.pending_approvals().await;
        json!({
            "state": state,
            "threadCache": {
                "runningThreadIDs": running_thread_ids,
                "contextWindowStatusByThreadID": thread_cache.context_window_status_by_thread_id,
            },
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

    async fn handle_upstream_event(&self, event: UpstreamRuntimeEvent) -> Result<()> {
        match event {
            UpstreamRuntimeEvent::ConnectionStatus(status) => {
                self.handle_connection_status_update(&status).await;
                self.set_connection_status(status).await;
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
                let turn_completed = match &notification {
                    ServerNotification::TurnCompleted(payload) => {
                        Some((payload.thread_id.clone(), payload.turn.id.clone()))
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
                if let Some((thread_id, turn_id)) = turn_completed {
                    self.maybe_auto_route_reply_to_orchestrator(&thread_id, &turn_id)
                        .await;
                }
            }
        }
        Ok(())
    }

    async fn handle_server_request(&self, request: ServerRequest) -> Result<()> {
        let pending = self.pending_approval_from_request(request).await;
        if let Some(approval) = pending {
            self.pending_approvals
                .write()
                .await
                .insert(approval.id.clone(), approval.clone());
            self.maybe_route_approval_to_orchestrator(&approval).await;
            let state = self.state_document.read().await.clone();
            self.push_event(BridgeEvent::AppStateSnapshot { state }).await;
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
        let state = self.state_document.read().await;
        let thread_ids = tracked_thread_ids_from_state(&state);
        for thread_id in thread_ids {
            let cwd = tracked_cwd_for_thread_value(&state, &thread_id);
            let approval_policy = tracked_approval_policy_for_thread_value(&state, &thread_id);
            let model = tracked_model_for_thread_value(&state, &thread_id);
            let effort = tracked_reasoning_for_thread_value(&state, &thread_id);
            let sandbox_mode = tracked_sandbox_mode_for_thread_value(&state, &thread_id);
            let network_access = tracked_network_access_for_thread_value(&state, &thread_id);
            let sandbox_policy = sandbox_policy_for_resume_value(
                sandbox_mode.as_deref(),
                network_access,
                cwd.as_deref(),
            );
            let base_instructions = tracked_base_instructions_for_thread_value(&state, &thread_id);
            let developer_instructions =
                tracked_developer_instructions_for_thread_value(&state, &thread_id);
            if let Err(error) = self
                .request_app_server_json(
                    "thread/resume",
                    json!({
                        "threadId": thread_id,
                        "cwd": cwd,
                        "approvalPolicy": approval_policy,
                        "sandbox": sandbox_mode,
                        "sandboxPolicy": sandbox_policy,
                        "model": model,
                        "effort": effort,
                        "baseInstructions": base_instructions,
                        "developerInstructions": developer_instructions,
                        "persistExtendedHistory": true,
                    }),
                )
                .await
            {
                tracing::warn!("resume tracked thread failed: {thread_id}: {error}");
            }
        }
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
        if tracked_role_for_thread(&state, thread_id).as_deref() == Some("hidden") {
            return;
        }
        let Some(orchestrator_thread_id) = project.orchestrator_thread_id.filter(|id| id != thread_id) else {
            return;
        };
        let Some(assistant_text) = self.latest_assistant_text_for_thread(thread_id).await else {
            return;
        };

        let routed_text = compose_auto_routed_reply(
            &assistant_text,
            &sender_label_for_thread(&state, thread_id),
        );
        if routed_text.trim().is_empty() {
            return;
        }

        if self
            .request_app_server_json(
                "turn/start",
                json!({
                    "threadId": orchestrator_thread_id,
                    "input": [{"type":"text","text": routed_text}],
                    "cwd": tracked_cwd_for_thread_value(&state, &orchestrator_thread_id)
                        .or(project.cwd)
                        .or(project.project_root),
                    "approvalPolicy": tracked_approval_policy_for_thread_value(&state, &orchestrator_thread_id),
                    "sandboxPolicy": tracked_sandbox_policy_for_thread_value(&state, &orchestrator_thread_id),
                    "model": tracked_model_for_thread_value(&state, &orchestrator_thread_id),
                    "effort": tracked_reasoning_for_thread_value(&state, &orchestrator_thread_id),
                }),
            )
            .await
            .is_ok()
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
        if tracked_role_for_thread(&state, &approval.thread_id).as_deref() == Some("hidden") {
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

        if self
            .request_app_server_json(
                "turn/start",
                json!({
                    "threadId": orchestrator_thread_id,
                    "input": [{"type":"text","text": routed_text}],
                    "cwd": tracked_cwd_for_thread_value(&state, &orchestrator_thread_id)
                        .or(project.cwd)
                        .or(project.project_root),
                    "approvalPolicy": tracked_approval_policy_for_thread_value(&state, &orchestrator_thread_id),
                    "sandboxPolicy": tracked_sandbox_policy_for_thread_value(&state, &orchestrator_thread_id),
                    "model": tracked_model_for_thread_value(&state, &orchestrator_thread_id),
                    "effort": tracked_reasoning_for_thread_value(&state, &orchestrator_thread_id),
                }),
            )
            .await
            .is_ok()
        {
            self.auto_routed_approval_keys
                .write()
                .await
                .insert(approval.id.clone());
        }
    }

    async fn latest_assistant_text_for_thread(&self, thread_id: &str) -> Option<String> {
        let thread_cache = self.thread_cache.read().await;
        thread_cache
            .message_cache_by_thread_id
            .get(thread_id)
            .and_then(|messages| {
                messages
                    .iter()
                    .rev()
                    .find(|message| message.role == "assistant" && !message.text.trim().is_empty())
                    .map(|message| message.text.trim().to_string())
            })
    }
}

fn file_change_cache_key(thread_id: &str, item_id: &str) -> String {
    format!("{thread_id}|{item_id}")
}

fn approval_id_for_request(instance_id: &str, request_id: &RequestId) -> String {
    format!("{instance_id}:{}", request_id_display(request_id))
}

fn request_id_display(request_id: &RequestId) -> String {
    match request_id {
        RequestId::Integer(value) => value.to_string(),
        RequestId::String(value) => value.clone(),
    }
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
    PendingApproval {
        id: approval_id_for_request(&instance_id, &request_id),
        instance_id,
        request_id,
        thread_id: params.thread_id,
        turn_id: params.turn_id.unwrap_or_else(|| "__global__".to_string()),
        item_id: params.server_name.clone(),
        kind: PendingApprovalKind::ToolUserInput,
        title: format!("MCP elicitation: {}", params.server_name),
        detail: None,
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
    serde_json::from_str(&raw)
        .with_context(|| format!("invalid JSON in {}", settings.paths.state_json.display()))
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
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

fn role_defaults_key_for_thread(state: &Value, thread_id: &str) -> &'static str {
    match tracked_role_for_thread(state, thread_id).as_deref() {
        Some("qa") => "qa",
        Some("orchestrator") => "orchestrator",
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
    match sandbox_mode {
        Some("danger-full-access") => Some(json!({ "type": "dangerFullAccess" })),
        Some("read-only") => Some(json!({
            "type": "readOnly",
            "access": { "type": "fullAccess" },
            "networkAccess": network_access.unwrap_or(false),
        })),
        Some("workspace-write") => Some(json!({
            "type": "workspaceWrite",
            "writableRoots": cwd.map(|value| vec![value]).unwrap_or_default(),
            "readOnlyAccess": { "type": "fullAccess" },
            "networkAccess": network_access.unwrap_or(true),
            "excludeTmpdirEnvVar": false,
            "excludeSlashTmp": false,
        })),
        Some("external-sandbox") => Some(json!({
            "type": "externalSandbox",
            "networkAccess": if network_access.unwrap_or(true) { "enabled" } else { "restricted" },
        })),
        _ => None,
    }
}

fn tracked_developer_instructions_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
    let role = tracked_role_for_thread(state, thread_id)?;
    if matches!(role.as_str(), "hidden" | "orchestrator" | "operator") {
        return None;
    }
    let project = tracked_project_for_thread(state, thread_id)?;
    if project.orchestrator_thread_id.is_none() {
        return None;
    }
    let mut guidance = Vec::new();
    if project.auto_route_replies {
        guidance.push("Final assistant replies are auto-forwarded to this project's orchestrator. Mid-turn messages and coordination are fine, but do not manually send a redundant final handoff when your turn ends unless you need to add distinct information.");
    } else {
        guidance.push("Final assistant replies are not auto-forwarded. If the orchestrator needs your final status, use $robdex-orchestrator to send it manually.");
    }
    if project.route_approval_requests {
        guidance.push("Command and file-change approval requests are forwarded to this project's orchestrator so they can guide approval decisions in real time.");
    }
    Some(guidance.join(" "))
}

fn tracked_base_instructions_for_thread_value(state: &Value, thread_id: &str) -> Option<String> {
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

fn sender_label_for_thread(state: &Value, thread_id: &str) -> String {
    tracked_agent_value(state, thread_id)
        .and_then(|agent| agent.get("displayName"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| thread_id.to_string())
}

fn compose_auto_routed_reply(text: &str, sender_label: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let prefixed = if trimmed.starts_with('[') && trimmed.contains("]: ") {
        trimmed.to_string()
    } else {
        format!("[{sender_label}]: {trimmed}")
    };
    let body = if prefixed.starts_with("[End of Turn] ") {
        prefixed
    } else {
        format!("[End of Turn] {prefixed}")
    };
    format!(
        "{body}\n\n**CRITICAL**: This agent is STOPPED. You must use `robdex send-message --name \"{sender_label}\" --text-stdin` following the $robdex-orchestrator instructions if you need to respond to this agent."
    )
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
                lines.push(format!("Command: {command}"));
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
    use crate::transport::run_transport_loop;
    use crate::upstream::UpstreamRuntimeEvent;
    use codex_app_server_adapter::app_server_protocol::{
        AgentMessageDeltaNotification, RequestId, ServerNotification, ServerRequest, ThreadClosedNotification,
        ThreadStartedNotification, ThreadStatus, ThreadStatusChangedNotification, ThreadTokenUsage,
        ThreadTokenUsageUpdatedNotification, TokenUsageBreakdown, ToolRequestUserInputParams,
        ToolRequestUserInputQuestion, Turn, TurnCompletedNotification, TurnStartedNotification, TurnStatus,
    };
    use codex_backend_core::HttpArgs;
    use futures_util::{SinkExt, StreamExt};
    use std::{net::{IpAddr, Ipv4Addr}, path::PathBuf};
    use tempfile::TempDir;
    use tokio::net::TcpListener;
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

    fn sample_turn(id: &str, status: TurnStatus) -> Turn {
        Turn {
            id: id.to_string(),
            items: Vec::new(),
            status,
            error: None,
        }
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
                skill_metadata: None,
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
        let routed = compose_auto_routed_reply("hi", "Bridge Agent Smoke");
        assert!(routed.starts_with("[End of Turn] [Bridge Agent Smoke]: hi"));
        assert!(routed.contains("robdex send-message --name \"Bridge Agent Smoke\" --text-stdin"));
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
}
