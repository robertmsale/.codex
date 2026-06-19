use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use robdex_agent_runtime::rinf_transport::{
    AgentRuntimeWorkbenchViewModel as InternalWorkbenchViewModel,
    AgentRuntimeDiscoveryView as InternalDiscoveryView,
    AgentRuntimeRoleAdminView as InternalRoleAdminView,
    AgentRuntimeWorkflowMemoryView as InternalWorkflowMemoryView,
    GuiStreamOutcomePacket, GuiTransportHandle, GuiTransportOutput, GuiTransportOutputPacket,
    GuiTransportRequest, GuiTransportRequestPacket,
};
use robdex_agent_runtime_projection::{
    ApiErrorPacket, CommandRegistryDecisionInput, GuiCommandSeed, GuiFinalExecutionPolicy,
    GuiOperationOutcome, GuiOperationRequest, GuiRegistryScope, RoleEditorDraft,
    RoleEditorLifecycleAuthorityMetadata, RoleEditorModelDefaults, RoleEditorRoutingMetadata,
    RoleEditorVisibilityMetadata,
};
use robdex_client_core::{bridge::BridgeEndpoint, LiveSessionEvent, LiveSessionHandle, WorkbenchClient, start_live_session};
use robdex_protocol::{UiChatEntry, WorkbenchViewData};
use rinf::{DartSignal, RustSignal};
use tokio::select;
use tokio::sync::mpsc;
use tokio_with_wasm::alias as tokio;

use crate::signals::{
    AgentRuntimeOutputSignal, AgentRuntimeRequestSignal, AgentRuntimeRequest,
    AgentRuntimeGuiOperation, AgentRuntimeOutput, AgentRuntimeProjectionSnapshot,
    AgentRuntimeControllerState, AgentRuntimeOperationResult, AgentRuntimeApiError,
    AgentRuntimeFact, AgentRuntimeStreamOutcome, AgentRuntimeWorkbenchViewModel,
    AgentRuntimeModelOption,
    AgentRuntimeDiscoveryView, AgentRuntimeSessionRow, AgentRuntimeTimelineRow,
    AgentRuntimeActionRow, AgentRuntimeBadge, AgentRuntimeRoleAdminView, AgentRuntimeRoleRow,
    AgentRuntimeRoleDetail, AgentRuntimeRolePolicyRow, AgentRuntimeRoleVersionRow,
    AgentRuntimeRoleEditorDraftView, AgentRuntimeRoleEditorOptionsView, AgentRuntimeWorkflowMemoryView,
    AgentRuntimeWorkflowMemoryRow, AgentRuntimeWorkflowMemoryDetail,
    AgentRuntimeWorkflowMemoryEvent, AgentRuntimeConversationShellViewModel, AgentRuntimeOperationSurface,
    AgentRuntimeShellProjectRow, AgentRuntimeShellRolePresentation, AgentRuntimeChatEntry,
    ArchiveThreadGroupSignal, ArchiveThreadSignal, BridgeTaskResultSignal, ClearProjectHookLogsSignal,
    CreateProjectSignal, CreateThreadGroupSignal, CreateThreadSignal, DecideApprovalSignal,
    DeleteProjectSignal, DeleteThreadGroupSignal,
    FetchThreadHistorySignal, InitializeWorkbenchSignal, MoveSelectedThreadToGroupSignal, ReloadWorkbenchSignal,
    LoadPeriodStatsSignal, LoadProjectHookLogsSignal, LoadRequirementComposablesSignal, LoadThreadStatsSignal,
    RenameThreadGroupSignal, RenameThreadSignal, SelectProjectSignal, SelectThreadSignal,
    SendThreadMessageSignal, SetProjectOrchestratorSignal, SetThreadRunningStateSignal,
    SetThreadRequirementsSignal, SpawnAgentSignal, TerminalCloseAllSignal, TerminalCloseSignal, TerminalInputSignal,
    TerminalEventSignal, TerminalOpenSignal, TerminalResizeSignal, TerminateCommandExecutionSignal,
    ThreadCompactSignal, UpdateGlobalSettingsSignal, UpdateProjectSignal, UpdateThreadSettingsSignal,
    UpdateWorkerMetadataSignal, InterruptThreadSignal, ThreadHistoryStateSignal, HookToastSignal,
    UploadImageBytesSignal, LoadImageBytesSignal,
    WarmHandoffSignal, WorkbenchStateSignal, WorkbenchSelectedChatDeltaSignal, WorkbenchDiagnosticsSignal,
};
use crate::terminal::TerminalRegistry;

enum Action {
    Initialize { host: String, port: u16 },
    Reload,
    SelectThread(String),
    FetchThreadHistory,
    ThreadCompact,
    TerminateCommandExecution(String),
    LoadThreadStats { request_id: String, thread_id: String },
    LoadPeriodStats {
        request_id: String,
        start_ms: u64,
        end_ms: u64,
        label: String,
        quota_reset_at_ms: Option<u64>,
        quota_remaining_percent: Option<f64>,
    },
    LoadProjectHookLogs { request_id: String, project_id: String },
    ClearProjectHookLogs { request_id: String, project_id: String },
    LoadRequirementComposables {
        request_id: String,
        sender_thread_id: Option<String>,
        recipient_thread_id: Option<String>,
        project_path: Option<String>,
    },
    SetThreadRequirements {
        request_id: String,
        sender_thread_id: Option<String>,
        recipient_thread_id: String,
        project_path: Option<String>,
        requirement_set_json: Option<String>,
    },
    UploadImageBytes {
        request_id: String,
        filename: String,
        content_type: String,
        bytes: Vec<u8>,
    },
    LoadImageBytes {
        request_id: String,
        path: String,
    },
    CreateProject {
        name: String,
        root_path: String,
        default_cwd: String,
    },
    SelectProject(Option<String>),
    DeleteProject(String),
    UpdateGlobalSettings {
        approval_policy: Option<String>,
        sandbox_mode: Option<String>,
        network_access: Option<bool>,
    },
    UpdateProject {
        project_id: String,
        name: String,
        default_cwd: String,
        auto_route_replies: bool,
        route_approval_requests: bool,
        preferred_model_provider: Option<String>,
        default_model_id: Option<String>,
        default_reasoning_effort: Option<String>,
        default_sandbox_mode: Option<String>,
        default_approval_policy: Option<String>,
        default_network_access: Option<bool>,
        role_runtime_defaults: serde_json::Value,
        orchestrator_model_id: Option<String>,
        orchestrator_reasoning_effort: Option<String>,
        worker_model_id: Option<String>,
        worker_reasoning_effort: Option<String>,
        qa_model_id: Option<String>,
        qa_reasoning_effort: Option<String>,
        designer_model_id: Option<String>,
        designer_reasoning_effort: Option<String>,
        planner_model_id: Option<String>,
        planner_reasoning_effort: Option<String>,
        requirements_reviewer_model_id: Option<String>,
        requirements_reviewer_reasoning_effort: Option<String>,
        orchestrator_developer_instructions: Option<String>,
        worker_developer_instructions: Option<String>,
        qa_developer_instructions: Option<String>,
        designer_developer_instructions: Option<String>,
        operator_developer_instructions: Option<String>,
        hidden_developer_instructions: Option<String>,
        permanent_requirement_composables: Vec<String>,
    },
    CreateThread {
        project_id: String,
        title: String,
        initial_prompt: String,
        role: String,
        approval_policy: Option<String>,
        sandbox_mode: Option<String>,
        network_access: Option<bool>,
        model_id: Option<String>,
        reasoning_effort: Option<String>,
        requirement_set_json: Option<String>,
    },
    SpawnAgent {
        name: String,
        role: String,
        prompt: String,
        requirement_set_json: Option<String>,
    },
    SetProjectOrchestrator {
        project_id: String,
        project_path: String,
        thread_id: String,
    },
    CreateThreadGroup(String),
    RenameThreadGroup {
        group_id: String,
        title: String,
    },
    DeleteThreadGroup(String),
    ArchiveThreadGroup(String),
    MoveSelectedThreadToGroup(Option<String>),
    UpdateWorkerMetadata {
        issue_number: Option<u64>,
        pull_request_number: Option<u64>,
        blocked_reason: Option<String>,
        unblock_when: Option<String>,
        clear_blocked: bool,
    },
    SendMessage {
        text: String,
        local_image_paths: Vec<String>,
        requirement_set_json: Option<String>,
    },
    InterruptThread,
    DecideApproval {
        approval_id: String,
        decision: String,
        message: Option<String>,
    },
    UpdateThreadSettings {
        role: Option<String>,
        approval_policy: Option<String>,
        sandbox_mode: Option<String>,
        network_access: Option<bool>,
        model_id: Option<String>,
        reasoning_effort: Option<String>,
        service_tier: Option<String>,
    },
    SetThreadRunningState(bool),
    RenameThread(String),
    ArchiveThread,
    WarmHandoff(String),
    TerminalOpen {
        request_id: String,
        host: String,
        username: String,
        cols: u32,
        rows: u32,
    },
    TerminalInput {
        session_id: String,
        data: String,
    },
    TerminalResize {
        session_id: String,
        cols: u32,
        rows: u32,
    },
    TerminalClose(String),
    TerminalCloseAll,
    AgentRuntimeRequest {
        request_id: String,
        request: AgentRuntimeRequest,
    },
}

#[derive(Debug, Default, Clone)]
struct StreamingDiagnostics {
    websocket_event_counts: BTreeMap<String, u64>,
    websocket_payload_bytes: BTreeMap<String, u64>,
    native_signal_count: u64,
    serialized_payload_bytes: u64,
    full_snapshot_decode_count: u64,
    dart_selected_chat_delta_apply_count: u64,
    coalesced_stream_update_count: u64,
    dropped_intermediate_stream_update_count: u64,
    selected_timeline_entry_count: u32,
}

impl StreamingDiagnostics {
    fn record_snapshot(&mut self, bytes: usize, entries: usize) {
        *self.websocket_event_counts.entry("snapshot".to_string()).or_default() += 1;
        *self.websocket_payload_bytes.entry("snapshot".to_string()).or_default() += bytes as u64;
        self.native_signal_count += 1;
        self.serialized_payload_bytes += bytes as u64;
        self.full_snapshot_decode_count += 1;
        self.selected_timeline_entry_count = entries.min(50) as u32;
    }

    fn record_delta(&mut self, bytes: usize, entries: usize) {
        *self.websocket_event_counts.entry("selectedChatDelta".to_string()).or_default() += 1;
        *self.websocket_payload_bytes.entry("selectedChatDelta".to_string()).or_default() += bytes as u64;
        self.native_signal_count += 1;
        self.serialized_payload_bytes += bytes as u64;
        self.selected_timeline_entry_count = entries.min(50) as u32;
    }

    fn signal(&self) -> WorkbenchDiagnosticsSignal {
        WorkbenchDiagnosticsSignal {
            websocket_event_counts_json: serde_json::to_string(&self.websocket_event_counts).unwrap_or_default(),
            websocket_payload_bytes_json: serde_json::to_string(&self.websocket_payload_bytes).unwrap_or_default(),
            native_signal_count: self.native_signal_count,
            serialized_payload_bytes: self.serialized_payload_bytes,
            full_snapshot_decode_count: self.full_snapshot_decode_count,
            dart_selected_chat_delta_apply_count: self.dart_selected_chat_delta_apply_count,
            coalesced_stream_update_count: self.coalesced_stream_update_count,
            dropped_intermediate_stream_update_count: self.dropped_intermediate_stream_update_count,
            selected_timeline_entry_count: self.selected_timeline_entry_count,
        }
    }
}

#[derive(Debug, Clone)]
struct StreamCoalescer {
    max_non_final_per_second: u32,
    emitted_non_final: u32,
    coalesced: u32,
    dropped: u32,
    pending: Option<WorkbenchSelectedChatDeltaSignal>,
}

impl StreamCoalescer {
    fn new(max_non_final_per_second: u32) -> Self {
        Self {
            max_non_final_per_second,
            emitted_non_final: 0,
            coalesced: 0,
            dropped: 0,
            pending: None,
        }
    }

    fn push(&mut self, mut delta: WorkbenchSelectedChatDeltaSignal) -> Option<WorkbenchSelectedChatDeltaSignal> {
        if delta.is_final {
            if self.pending.take().is_some() {
                self.dropped += 1;
            }
            delta.coalesced_stream_update_count = self.coalesced;
            delta.dropped_intermediate_stream_update_count = self.dropped;
            return Some(delta);
        }
        if self.emitted_non_final < self.max_non_final_per_second {
            self.emitted_non_final += 1;
            self.coalesced += 1;
            delta.coalesced_stream_update_count = self.coalesced;
            delta.dropped_intermediate_stream_update_count = self.dropped;
            Some(delta)
        } else {
            if self.pending.replace(delta).is_some() {
                self.dropped += 1;
            }
            None
        }
    }
}

enum SelectedChatStreamEmission {
    Delta(WorkbenchSelectedChatDeltaSignal),
    Dropped,
}

fn selected_chat_stream_emission(
    coalescer: &mut StreamCoalescer,
    delta: WorkbenchSelectedChatDeltaSignal,
) -> SelectedChatStreamEmission {
    coalescer
        .push(delta)
        .map(SelectedChatStreamEmission::Delta)
        .unwrap_or(SelectedChatStreamEmission::Dropped)
}

pub async fn run() {
    let (tx, mut rx) = mpsc::unbounded_channel::<Action>();
    spawn_receivers(tx.clone());
    let agent_runtime_transport = GuiTransportHandle::spawn();

    let mut client: Option<WorkbenchClient> = None;
    let mut current_view: Option<WorkbenchViewData> = None;
    let mut live_session: Option<LiveSessionHandle> = None;
    let mut live_event_rx: Option<mpsc::UnboundedReceiver<LiveSessionEvent>> = None;
    let mut initialized = false;
    let mut terminals = TerminalRegistry::new();
    let mut diagnostics = StreamingDiagnostics::default();
    let mut stream_coalescer = StreamCoalescer::new(10);

    loop {
        terminals.reap_finished();
        select! {
            maybe_action = rx.recv() => {
                let Some(action) = maybe_action else {
                    break;
                };
                if matches!(action, Action::Initialize { .. }) {
                    initialized = true;
                }
                if let Action::AgentRuntimeRequest { request_id, request } = action {
                    let agent_runtime_transport = agent_runtime_transport.clone();
                    tokio::spawn(async move {
                        handle_agent_runtime_request(&agent_runtime_transport, request_id, request).await;
                    });
                    continue;
                }
                apply_optimistic_action(&mut current_view, &action);
                let show_loading = current_view.is_none();
                emit_state(current_view.as_ref(), show_loading, "");
                record_and_emit_snapshot_diagnostics(&mut diagnostics, current_view.as_ref());
                let result = handle_action(&mut client, &current_view, &mut terminals, action).await;
                match result {
                    Ok(next_view) => {
                        current_view = Some(next_view);
                        if let (Some(client), Some(view)) = (client.as_mut(), current_view.as_ref()) {
                            client.sync_view(view);
                        }
                        if initialized && live_session.is_none() {
                            let (session, event_rx) = start_live_session(
                                current_view.clone().expect("view should exist after initialize"),
                                client
                                    .as_ref()
                                    .map(|client| client.endpoint().clone())
                                    .expect("endpoint should exist after initialize"),
                            );
                            live_session = Some(session);
                            live_event_rx = Some(event_rx);
                        }
                        if let (Some(session), Some(view)) = (live_session.as_ref(), current_view.as_ref()) {
                            session.sync_view(view.clone());
                        }
                        emit_state(current_view.as_ref(), false, "");
                        record_and_emit_snapshot_diagnostics(&mut diagnostics, current_view.as_ref());
                    }
                    Err(error) => {
                        emit_state(current_view.as_ref(), false, &error.to_string());
                        record_and_emit_snapshot_diagnostics(&mut diagnostics, current_view.as_ref());
                    }
                }
            }
            maybe_live_event = recv_live_event(&mut live_event_rx), if initialized && live_event_rx.is_some() => {
                match maybe_live_event {
                    Some(LiveSessionEvent::View(next_view)) => {
                        let delta = selected_chat_delta(current_view.as_ref(), &next_view);
                        current_view = Some(next_view);
                        if let (Some(client), Some(view)) = (client.as_mut(), current_view.as_ref()) {
                            client.sync_view(view);
                        }
                        if let Some(delta) = delta {
                            let delta = match selected_chat_stream_emission(&mut stream_coalescer, delta) {
                                SelectedChatStreamEmission::Delta(delta) => delta,
                                SelectedChatStreamEmission::Dropped => {
                                    diagnostics.coalesced_stream_update_count = stream_coalescer.coalesced as u64;
                                    diagnostics.dropped_intermediate_stream_update_count = stream_coalescer.dropped as u64;
                                    diagnostics.signal().send_signal_to_dart();
                                    terminals.reap_finished();
                                    continue;
                                }
                            };
                            let bytes = delta.appended_text.len()
                                + delta.replacement_text.len()
                                + delta.metadata_json.len()
                                + delta.thread_id.len()
                                + delta.message_id.len();
                            diagnostics.coalesced_stream_update_count = delta.coalesced_stream_update_count as u64;
                            diagnostics.dropped_intermediate_stream_update_count = delta.dropped_intermediate_stream_update_count as u64;
                            diagnostics.record_delta(bytes, delta.selected_entry_count as usize);
                            delta.send_signal_to_dart();
                            diagnostics.signal().send_signal_to_dart();
                        } else {
                            emit_state(current_view.as_ref(), false, "");
                            record_and_emit_snapshot_diagnostics(&mut diagnostics, current_view.as_ref());
                        }
                    }
                    Some(LiveSessionEvent::HookFailure(notice)) => {
                        HookToastSignal {
                            message: format!(
                                "{} hook {} {}",
                                notice.role.to_uppercase(),
                                notice.event,
                                notice.status.replace('_', " ")
                            ),
                            detail: notice.detail.clone(),
                            copy_text: format!(
                                "[{}] {} / {} / {}: {}",
                                notice.project_name,
                                notice.agent_name,
                                notice.role,
                                notice.event,
                                notice.detail
                            ),
                            duration_ms: 5000,
                        }
                        .send_signal_to_dart();
                    }
                    Some(LiveSessionEvent::Error(error)) => {
                        emit_state(current_view.as_ref(), false, &error);
                        record_and_emit_snapshot_diagnostics(&mut diagnostics, current_view.as_ref());
                    }
                    None => {
                        live_event_rx = None;
                        live_session = None;
                    }
                }
                terminals.reap_finished();
            }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                terminals.reap_finished();
            }
        }
    }
    terminals.close_all();
}

fn apply_optimistic_action(current_view: &mut Option<WorkbenchViewData>, action: &Action) {
    let Some(view) = current_view.as_mut() else {
        return;
    };
    let Action::SendMessage {
        text,
        local_image_paths,
        ..
    } = action else {
        return;
    };
    if view.selection.thread_id.is_none() {
        return;
    }
    let mut lines = Vec::new();
    let body = text.trim();
    if !body.is_empty() {
        lines.push(body.to_string());
    }
    for path in local_image_paths {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            continue;
        }
        let label = std::path::Path::new(trimmed)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(trimmed);
        lines.push(format!("[local-image] {label}"));
    }
    if lines.is_empty() {
        return;
    }
    view.chat_entries.push(UiChatEntry {
        id: format!("pending-user-{}", unix_now_millis()),
        author: "User".to_string(),
        display_label: "User".to_string(),
        timestamp: Some(unix_now_seconds()),
        body: lines.join("\n"),
        subtitle: Some("Sending...".to_string()),
        kind: None,
        status: Some("pending".to_string()),
        process_id: None,
        command: None,
        output: None,
        image_preview_base64: None,
        image_preview_content_type: None,
        image_preview_error: None,
        delivery_state: Some("pending".to_string()),
        semantic_card: None,
        is_streaming: false,
        is_tool: false,
    });
}

fn current_view_clone(current_view: &Option<WorkbenchViewData>) -> Result<WorkbenchViewData> {
    current_view
        .clone()
        .ok_or_else(|| anyhow!("No current view"))
}

fn select_thread_from_current_view(
    current_view: &Option<WorkbenchViewData>,
    thread_id: &str,
    chat_entries: Vec<UiChatEntry>,
) -> Result<WorkbenchViewData> {
    let mut view = current_view_clone(current_view)?;
    let thread = view
        .threads
        .iter()
        .find(|thread| thread.id == thread_id)
        .cloned()
        .ok_or_else(|| anyhow!("Thread is not present in the current workbench state"))?;

    view.selection.project_id = Some(thread.project_id.clone()).filter(|value| !value.is_empty());
    view.selection.project_root_path =
        Some(thread.project_root_path.clone()).filter(|value| !value.is_empty());
    view.selection.project_orchestrator_thread_id = thread.project_orchestrator_thread_id.clone();
    view.selection.project_orchestrator_name = thread.project_orchestrator_name.clone();
    view.selection.thread_id = Some(thread.id.clone());
    view.selection.thread_role = Some(thread.role.clone());
    view.selection.project_name = thread.project_name.clone();
    view.selection.thread_name = thread.title.clone();
    view.selection.sandbox_mode = thread.sandbox_mode.clone();
    view.selection.network_access = thread.network_access;
    view.selection.approval_policy = thread.approval_policy.clone();
    view.selection.model = thread.model.clone();
    view.selection.reasoning_effort = thread.reasoning_effort.clone();
    view.selection.service_tier = thread.service_tier.clone();
    view.selection.effective_sandbox_mode = thread.effective_sandbox_mode.clone();
    view.selection.effective_network_access = thread.effective_network_access;
    view.selection.effective_approval_policy = thread.effective_approval_policy.clone();
    view.selection.effective_model = thread.effective_model.clone();
    view.selection.effective_reasoning_effort = thread.effective_reasoning_effort.clone();
    view.selection.effective_service_tier = thread.effective_service_tier.clone();
    view.selection.is_running = thread.is_running;

    view.chat_entries = chat_entries;
    view.context_window_remaining_percent = None;
    view.live_processes.clear();
    view.requirement_review = thread.requirement_review.clone();
    view.worker_metadata = None;
    view.workspace_files.clear();
    view.inspector_facts = vec![
        robdex_protocol::UiInspectorFact {
            label: "Role".to_string(),
            value: thread.role.clone(),
        },
        robdex_protocol::UiInspectorFact {
            label: "Model".to_string(),
            value: thread.model.clone().unwrap_or_else(|| "default".to_string()),
        },
        robdex_protocol::UiInspectorFact {
            label: "Sandbox".to_string(),
            value: thread
                .sandbox_mode
                .clone()
                .unwrap_or_else(|| "default".to_string()),
        },
        robdex_protocol::UiInspectorFact {
            label: "Network".to_string(),
            value: match thread.network_access {
                Some(true) => "enabled".to_string(),
                Some(false) => "disabled".to_string(),
                None => "default".to_string(),
            },
        },
        robdex_protocol::UiInspectorFact {
            label: "Project".to_string(),
            value: thread.project_name,
        },
    ];
    Ok(view)
}

fn selected_chat_delta(
    previous: Option<&WorkbenchViewData>,
    next: &WorkbenchViewData,
) -> Option<WorkbenchSelectedChatDeltaSignal> {
    let thread_id = next.selection.thread_id.clone()?;
    if previous.and_then(|view| view.selection.thread_id.as_deref()) != Some(thread_id.as_str()) {
        return None;
    }
    let previous_entries = previous.map(|view| view.chat_entries.as_slice()).unwrap_or(&[]);
    let next_entries = next.chat_entries.as_slice();
    let latest = next_entries.last()?;
    if latest.author.eq_ignore_ascii_case("user") {
        return None;
    }
    let previous_entry = previous_entries.iter().find(|entry| entry.id == latest.id);
    let (appended_text, replacement_text) = if let Some(previous_entry) = previous_entry {
        if latest.body == previous_entry.body {
            return None;
        }
        if latest.body.starts_with(&previous_entry.body) {
            (latest.body[previous_entry.body.len()..].to_string(), String::new())
        } else {
            (String::new(), latest.body.clone())
        }
    } else {
        (String::new(), latest.body.clone())
    };
    Some(WorkbenchSelectedChatDeltaSignal {
        thread_id,
        message_id: latest.id.clone(),
        appended_text,
        replacement_text,
        delivery_state: latest.delivery_state.clone().or_else(|| latest.status.clone()).unwrap_or_else(|| "streaming".to_string()),
        is_final: !latest.is_streaming,
        sequence: unix_now_millis() as u64,
        metadata_json: serde_json::json!({
            "author": latest.author,
            "isTool": latest.is_tool,
        })
        .to_string(),
        selected_entry_count: next_entries.len().min(50) as u32,
        coalesced_stream_update_count: 0,
        dropped_intermediate_stream_update_count: 0,
    })
}

fn record_and_emit_snapshot_diagnostics(
    diagnostics: &mut StreamingDiagnostics,
    view: Option<&WorkbenchViewData>,
) {
    let Some(view) = view else {
        return;
    };
    let bytes = serde_json::to_string(view).map(|value| value.len()).unwrap_or_default();
    diagnostics.record_snapshot(bytes, view.chat_entries.len());
    diagnostics.signal().send_signal_to_dart();
}

fn unix_now_millis() -> u128 {
    #[cfg(target_arch = "wasm32")]
    {
        return js_sys::Date::now() as u128;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default()
    }
}

fn unix_now_seconds() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        return (js_sys::Date::now() / 1000.0) as u64;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or_default()
    }
}

async fn recv_live_event(
    live_event_rx: &mut Option<mpsc::UnboundedReceiver<LiveSessionEvent>>,
) -> Option<LiveSessionEvent> {
    match live_event_rx {
        Some(receiver) => receiver.recv().await,
        None => None,
    }
}

fn spawn_receivers(tx: mpsc::UnboundedSender<Action>) {
    spawn_map::<InitializeWorkbenchSignal, _>(tx.clone(), |signal| Action::Initialize {
        host: signal.message.host,
        port: signal.message.port as u16,
    });
    spawn_unit::<ReloadWorkbenchSignal, _>(tx.clone(), || Action::Reload);
    spawn_map::<SelectThreadSignal, _>(tx.clone(), |signal| {
        Action::SelectThread(signal.message.thread_id)
    });
    spawn_unit::<FetchThreadHistorySignal, _>(tx.clone(), || Action::FetchThreadHistory);
    spawn_unit::<ThreadCompactSignal, _>(tx.clone(), || Action::ThreadCompact);
    spawn_map::<TerminateCommandExecutionSignal, _>(tx.clone(), |signal| {
        Action::TerminateCommandExecution(signal.message.process_id)
    });
    spawn_map::<LoadThreadStatsSignal, _>(tx.clone(), |signal| Action::LoadThreadStats {
        request_id: signal.message.request_id,
        thread_id: signal.message.thread_id,
    });
    spawn_map::<LoadPeriodStatsSignal, _>(tx.clone(), |signal| Action::LoadPeriodStats {
        request_id: signal.message.request_id,
        start_ms: signal.message.start_ms,
        end_ms: signal.message.end_ms,
        label: signal.message.label,
        quota_reset_at_ms: signal.message.has_quota.then_some(signal.message.quota_reset_at_ms),
        quota_remaining_percent: signal.message.has_quota.then_some(signal.message.quota_remaining_percent),
    });
    spawn_map::<LoadProjectHookLogsSignal, _>(tx.clone(), |signal| Action::LoadProjectHookLogs {
        request_id: signal.message.request_id,
        project_id: signal.message.project_id,
    });
    spawn_map::<ClearProjectHookLogsSignal, _>(tx.clone(), |signal| Action::ClearProjectHookLogs {
        request_id: signal.message.request_id,
        project_id: signal.message.project_id,
    });
    spawn_map::<LoadRequirementComposablesSignal, _>(tx.clone(), |signal| {
        Action::LoadRequirementComposables {
            request_id: signal.message.request_id,
            sender_thread_id: non_empty(signal.message.sender_thread_id),
            recipient_thread_id: non_empty(signal.message.recipient_thread_id),
            project_path: non_empty(signal.message.project_path),
        }
    });
    spawn_map::<SetThreadRequirementsSignal, _>(tx.clone(), |signal| Action::SetThreadRequirements {
        request_id: signal.message.request_id,
        sender_thread_id: non_empty(signal.message.sender_thread_id),
        recipient_thread_id: signal.message.recipient_thread_id,
        project_path: non_empty(signal.message.project_path),
        requirement_set_json: Some(signal.message.requirement_set_json),
    });
    spawn_map::<UploadImageBytesSignal, _>(tx.clone(), |signal| Action::UploadImageBytes {
        request_id: signal.message.request_id,
        filename: signal.message.filename,
        content_type: signal.message.content_type,
        bytes: signal.message.bytes,
    });
    spawn_map::<LoadImageBytesSignal, _>(tx.clone(), |signal| Action::LoadImageBytes {
        request_id: signal.message.request_id,
        path: signal.message.path,
    });
    spawn_map::<CreateProjectSignal, _>(tx.clone(), |signal| Action::CreateProject {
        name: signal.message.name,
        root_path: signal.message.root_path,
        default_cwd: signal.message.default_cwd,
    });
    spawn_map::<SelectProjectSignal, _>(tx.clone(), |signal| {
        let project_id = if signal.message.project_id.is_empty() {
            None
        } else {
            Some(signal.message.project_id)
        };
        Action::SelectProject(project_id)
    });
    spawn_map::<DeleteProjectSignal, _>(tx.clone(), |signal| {
        Action::DeleteProject(signal.message.project_id)
    });
    spawn_map::<UpdateGlobalSettingsSignal, _>(tx.clone(), |signal| {
        Action::UpdateGlobalSettings {
            approval_policy: if signal.message.approval_policy.is_empty() {
                None
            } else {
                Some(signal.message.approval_policy)
            },
            sandbox_mode: if signal.message.sandbox_mode.is_empty() {
                None
            } else {
                Some(signal.message.sandbox_mode)
            },
            network_access: match signal.message.network_access_mode.as_str() {
                "enabled" => Some(true),
                "disabled" => Some(false),
                _ => None,
            },
        }
    });
    spawn_map::<UpdateProjectSignal, _>(tx.clone(), |signal| Action::UpdateProject {
        project_id: signal.message.project_id,
        name: signal.message.name,
        default_cwd: signal.message.default_cwd,
        auto_route_replies: signal.message.auto_route_replies,
        route_approval_requests: signal.message.route_approval_requests,
        preferred_model_provider: if signal.message.preferred_model_provider.is_empty() {
            None
        } else {
            Some(signal.message.preferred_model_provider)
        },
        default_model_id: if signal.message.default_model_id.is_empty() {
            None
        } else {
            Some(signal.message.default_model_id)
        },
        default_reasoning_effort: if signal.message.default_reasoning_effort.is_empty() {
            None
        } else {
            Some(signal.message.default_reasoning_effort)
        },
        default_sandbox_mode: if signal.message.default_sandbox_mode.is_empty() {
            None
        } else {
            Some(signal.message.default_sandbox_mode)
        },
        default_approval_policy: if signal.message.default_approval_policy.is_empty() {
            None
        } else {
            Some(signal.message.default_approval_policy)
        },
        default_network_access: match signal.message.default_network_access_mode.as_str() {
            "enabled" => Some(true),
            "disabled" => Some(false),
            _ => None,
        },
        role_runtime_defaults: serde_json::from_str(&signal.message.role_runtime_defaults_json)
            .unwrap_or(serde_json::Value::Null),
        orchestrator_model_id: if signal.message.orchestrator_model_id.is_empty() {
            None
        } else {
            Some(signal.message.orchestrator_model_id)
        },
        orchestrator_reasoning_effort: if signal.message.orchestrator_reasoning_effort.is_empty() {
            None
        } else {
            Some(signal.message.orchestrator_reasoning_effort)
        },
        worker_model_id: if signal.message.worker_model_id.is_empty() {
            None
        } else {
            Some(signal.message.worker_model_id)
        },
        worker_reasoning_effort: if signal.message.worker_reasoning_effort.is_empty() {
            None
        } else {
            Some(signal.message.worker_reasoning_effort)
        },
        qa_model_id: if signal.message.qa_model_id.is_empty() {
            None
        } else {
            Some(signal.message.qa_model_id)
        },
        qa_reasoning_effort: if signal.message.qa_reasoning_effort.is_empty() {
            None
        } else {
            Some(signal.message.qa_reasoning_effort)
        },
        designer_model_id: if signal.message.designer_model_id.is_empty() {
            None
        } else {
            Some(signal.message.designer_model_id)
        },
        designer_reasoning_effort: if signal.message.designer_reasoning_effort.is_empty() {
            None
        } else {
            Some(signal.message.designer_reasoning_effort)
        },
        planner_model_id: if signal.message.planner_model_id.is_empty() {
            None
        } else {
            Some(signal.message.planner_model_id)
        },
        planner_reasoning_effort: if signal.message.planner_reasoning_effort.is_empty() {
            None
        } else {
            Some(signal.message.planner_reasoning_effort)
        },
        requirements_reviewer_model_id: if signal.message.requirements_reviewer_model_id.is_empty() {
            None
        } else {
            Some(signal.message.requirements_reviewer_model_id)
        },
        requirements_reviewer_reasoning_effort: if signal.message.requirements_reviewer_reasoning_effort.is_empty() {
            None
        } else {
            Some(signal.message.requirements_reviewer_reasoning_effort)
        },
        orchestrator_developer_instructions: if signal.message.orchestrator_developer_instructions.is_empty() {
            None
        } else {
            Some(signal.message.orchestrator_developer_instructions)
        },
        worker_developer_instructions: if signal.message.worker_developer_instructions.is_empty() {
            None
        } else {
            Some(signal.message.worker_developer_instructions)
        },
        qa_developer_instructions: if signal.message.qa_developer_instructions.is_empty() {
            None
        } else {
            Some(signal.message.qa_developer_instructions)
        },
        designer_developer_instructions: if signal.message.designer_developer_instructions.is_empty() {
            None
        } else {
            Some(signal.message.designer_developer_instructions)
        },
        operator_developer_instructions: if signal.message.operator_developer_instructions.is_empty() {
            None
        } else {
            Some(signal.message.operator_developer_instructions)
        },
        hidden_developer_instructions: if signal.message.hidden_developer_instructions.is_empty() {
            None
        } else {
            Some(signal.message.hidden_developer_instructions)
        },
        permanent_requirement_composables: signal.message.permanent_requirement_composables,
    });
    spawn_map::<CreateThreadSignal, _>(tx.clone(), |signal| Action::CreateThread {
        project_id: signal.message.project_id,
        title: signal.message.title,
        initial_prompt: signal.message.initial_prompt,
        role: signal.message.role,
        approval_policy: if signal.message.approval_policy.is_empty() {
            None
        } else {
            Some(signal.message.approval_policy)
        },
        sandbox_mode: if signal.message.sandbox_mode.is_empty() {
            None
        } else {
            Some(signal.message.sandbox_mode)
        },
        network_access: match signal.message.network_access_mode.as_str() {
            "enabled" => Some(true),
            "disabled" => Some(false),
            _ => None,
        },
        model_id: if signal.message.model_id.is_empty() {
            None
        } else {
            Some(signal.message.model_id)
        },
        reasoning_effort: if signal.message.reasoning_effort.is_empty() {
            None
        } else {
            Some(signal.message.reasoning_effort)
        },
        requirement_set_json: if signal.message.requirement_set_json.trim().is_empty() {
            None
        } else {
            Some(signal.message.requirement_set_json)
        },
    });
    spawn_map::<SpawnAgentSignal, _>(tx.clone(), |signal| Action::SpawnAgent {
        name: signal.message.name,
        role: signal.message.role,
        prompt: signal.message.prompt,
        requirement_set_json: if signal.message.requirement_set_json.trim().is_empty() {
            None
        } else {
            Some(signal.message.requirement_set_json)
        },
    });
    spawn_map::<SetProjectOrchestratorSignal, _>(tx.clone(), |signal| {
        Action::SetProjectOrchestrator {
            project_id: signal.message.project_id,
            project_path: signal.message.project_path,
            thread_id: signal.message.thread_id,
        }
    });
    spawn_map::<CreateThreadGroupSignal, _>(tx.clone(), |signal| {
        Action::CreateThreadGroup(signal.message.title)
    });
    spawn_map::<RenameThreadGroupSignal, _>(tx.clone(), |signal| {
        Action::RenameThreadGroup {
            group_id: signal.message.group_id,
            title: signal.message.title,
        }
    });
    spawn_map::<DeleteThreadGroupSignal, _>(tx.clone(), |signal| {
        Action::DeleteThreadGroup(signal.message.group_id)
    });
    spawn_map::<ArchiveThreadGroupSignal, _>(tx.clone(), |signal| {
        Action::ArchiveThreadGroup(signal.message.group_id)
    });
    spawn_map::<MoveSelectedThreadToGroupSignal, _>(tx.clone(), |signal| {
        let group_id = if signal.message.group_id.is_empty() {
            None
        } else {
            Some(signal.message.group_id)
        };
        Action::MoveSelectedThreadToGroup(group_id)
    });
    spawn_map::<UpdateWorkerMetadataSignal, _>(tx.clone(), |signal| {
        let issue_number = signal.message.issue_number.trim().parse::<u64>().ok();
        let pull_request_number = signal.message.pull_request_number.trim().parse::<u64>().ok();
        let blocked_reason = if signal.message.blocked_reason.trim().is_empty() {
            None
        } else {
            Some(signal.message.blocked_reason)
        };
        let unblock_when = if signal.message.unblock_when.trim().is_empty() {
            None
        } else {
            Some(signal.message.unblock_when)
        };
        Action::UpdateWorkerMetadata {
            issue_number,
            pull_request_number,
            blocked_reason,
            unblock_when,
            clear_blocked: signal.message.clear_blocked,
        }
    });
    spawn_map::<SendThreadMessageSignal, _>(tx.clone(), |signal| Action::SendMessage {
        text: signal.message.text,
        local_image_paths: signal.message.local_image_paths,
        requirement_set_json: if signal.message.requirement_set_json.trim().is_empty() {
            None
        } else {
            Some(signal.message.requirement_set_json)
        },
    });
    spawn_unit::<InterruptThreadSignal, _>(tx.clone(), || Action::InterruptThread);
    spawn_map::<DecideApprovalSignal, _>(tx.clone(), |signal| Action::DecideApproval {
        approval_id: signal.message.approval_id,
        decision: signal.message.decision,
        message: if signal.message.message.is_empty() {
            None
        } else {
            Some(signal.message.message)
        },
    });
    spawn_map::<UpdateThreadSettingsSignal, _>(tx.clone(), |signal| {
        let role = if signal.message.role.is_empty() {
            None
        } else {
            Some(signal.message.role)
        };
        let approval_policy = if signal.message.approval_policy.is_empty() {
            None
        } else {
            Some(signal.message.approval_policy)
        };
        let sandbox_mode = if signal.message.sandbox_mode.is_empty() {
            None
        } else {
            Some(signal.message.sandbox_mode)
        };
        let network_access = match signal.message.network_access_mode.as_str() {
            "enabled" => Some(true),
            "disabled" => Some(false),
            _ => None,
        };
        let model_id = if signal.message.model_id.is_empty() {
            None
        } else {
            Some(signal.message.model_id)
        };
        let reasoning_effort = if signal.message.reasoning_effort.is_empty() {
            None
        } else {
            Some(signal.message.reasoning_effort)
        };
        let service_tier = if signal.message.service_tier.is_empty() {
            None
        } else {
            Some(signal.message.service_tier)
        };
        Action::UpdateThreadSettings {
            role,
            approval_policy,
            sandbox_mode,
            network_access,
            model_id,
            reasoning_effort,
            service_tier,
        }
    });
    spawn_map::<SetThreadRunningStateSignal, _>(tx.clone(), |signal| {
        Action::SetThreadRunningState(signal.message.running)
    });
    spawn_map::<RenameThreadSignal, _>(tx.clone(), |signal| {
        Action::RenameThread(signal.message.name)
    });
    spawn_unit::<ArchiveThreadSignal, _>(tx.clone(), || Action::ArchiveThread);
    spawn_map::<WarmHandoffSignal, _>(tx.clone(), |signal| Action::WarmHandoff(signal.message.prompt));
    spawn_map::<TerminalOpenSignal, _>(tx.clone(), |signal| Action::TerminalOpen {
        request_id: signal.message.request_id,
        host: signal.message.host,
        username: signal.message.username,
        cols: signal.message.cols,
        rows: signal.message.rows,
    });
    spawn_map::<TerminalInputSignal, _>(tx.clone(), |signal| Action::TerminalInput {
        session_id: signal.message.session_id,
        data: signal.message.data,
    });
    spawn_map::<TerminalResizeSignal, _>(tx.clone(), |signal| Action::TerminalResize {
        session_id: signal.message.session_id,
        cols: signal.message.cols,
        rows: signal.message.rows,
    });
    spawn_map::<TerminalCloseSignal, _>(tx.clone(), |signal| {
        Action::TerminalClose(signal.message.session_id)
    });
    spawn_unit::<TerminalCloseAllSignal, _>(tx.clone(), || Action::TerminalCloseAll);
    spawn_map::<AgentRuntimeRequestSignal, _>(tx, |signal| Action::AgentRuntimeRequest {
        request_id: signal.message.request_id,
        request: signal.message.request,
    });
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn required_non_empty(value: String, field: &str) -> std::result::Result<String, ApiErrorPacket> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ApiErrorPacket::new(
            "invalid_request",
            format!("{field} is required"),
            serde_json::json!({ "field": field }),
        ))
    } else {
        Ok(trimmed.to_string())
    }
}

fn typed_agent_runtime_request_packet(
    request_id: &str,
    request: AgentRuntimeRequest,
) -> std::result::Result<GuiTransportRequestPacket, ApiErrorPacket> {
    Ok(GuiTransportRequestPacket {
        packet_id: request_id.to_string(),
        intent: match request {
            AgentRuntimeRequest::RefreshDiscovery { discovery_path } => GuiTransportRequest::RefreshDiscovery {
                discovery_path: non_empty(discovery_path),
            },
            AgentRuntimeRequest::RefreshIcloudRemoteDiscovery { profile_path } => GuiTransportRequest::RefreshIcloudRemoteDiscovery {
                profile_path: non_empty(profile_path),
            },
            AgentRuntimeRequest::ImportRemoteProfileDocument { profile_path } => GuiTransportRequest::ImportRemoteProfileDocument {
                profile_path: non_empty(profile_path),
            },
            AgentRuntimeRequest::RefreshImportedRemoteProfile => GuiTransportRequest::RefreshImportedRemoteProfile,
            AgentRuntimeRequest::ConnectDiscoveredRuntime { discovery_path, selected_session_id } => GuiTransportRequest::ConnectDiscoveredRuntime {
                discovery_path: non_empty(discovery_path),
                selected_session_id: non_empty(selected_session_id),
            },
            AgentRuntimeRequest::ConnectIcloudRemoteRuntime { profile_path, selected_session_id } => GuiTransportRequest::ConnectIcloudRemoteRuntime {
                profile_path: non_empty(profile_path),
                selected_session_id: non_empty(selected_session_id),
            },
            AgentRuntimeRequest::ConnectImportedRemoteRuntime { selected_session_id } => GuiTransportRequest::ConnectImportedRemoteRuntime {
                selected_session_id: non_empty(selected_session_id),
            },
            AgentRuntimeRequest::Connect { base_url, selected_session_id } => GuiTransportRequest::Connect {
                base_url,
                selected_session_id: non_empty(selected_session_id),
            },
            AgentRuntimeRequest::SelectProject { project_id } => GuiTransportRequest::SelectProject {
                project_id,
            },
            AgentRuntimeRequest::Hydrate { selected_session_id } => GuiTransportRequest::Hydrate {
                selected_session_id: non_empty(selected_session_id),
            },
            AgentRuntimeRequest::Rehydrate { selected_session_id } => GuiTransportRequest::Rehydrate {
                selected_session_id: non_empty(selected_session_id),
            },
            AgentRuntimeRequest::Disconnect => GuiTransportRequest::Disconnect,
            AgentRuntimeRequest::DispatchOperation { operation } => GuiTransportRequest::DispatchOperation {
                operation: typed_gui_operation(operation)?,
            },
            AgentRuntimeRequest::ConsumeStreamOnce => GuiTransportRequest::ConsumeStreamOnce,
        },
    })
}

fn typed_gui_operation(operation: AgentRuntimeGuiOperation) -> std::result::Result<GuiOperationRequest, ApiErrorPacket> {
    Ok(match operation {
        AgentRuntimeGuiOperation::SelectSession { session_id } => GuiOperationRequest::SelectSession { session_id: non_empty(session_id) },
        AgentRuntimeGuiOperation::SelectWorkflowMemory { memory_id } => GuiOperationRequest::SelectWorkflowMemory { memory_id: non_empty(memory_id) },
        AgentRuntimeGuiOperation::CreateSession { role, project, model, workdir, worktree_root, title, name } => GuiOperationRequest::CreateSession {
            role,
            project: non_empty(project),
            model: non_empty(model),
            workdir: non_empty(workdir),
            worktree_root: non_empty(worktree_root),
            title: non_empty(title),
            name: non_empty(name),
        },
        AgentRuntimeGuiOperation::ListProjects => GuiOperationRequest::ListProjects,
        AgentRuntimeGuiOperation::CreateProject { project_key, display_name, default_workdir, default_worktree_root, default_role_id, default_model, tracked, listed } => GuiOperationRequest::CreateProject {
            project_key,
            display_name,
            default_workdir,
            default_worktree_root,
            default_role_id: non_empty(default_role_id),
            default_model,
            tracked,
            listed,
        },
        AgentRuntimeGuiOperation::UpdateProject { project_key, display_name, default_workdir, default_worktree_root, default_role_id, default_model, tracked, listed } => GuiOperationRequest::UpdateProject {
            project_key,
            display_name,
            default_workdir,
            default_worktree_root,
            default_role_id: non_empty(default_role_id),
            default_model,
            tracked,
            listed,
        },
        AgentRuntimeGuiOperation::ArchiveProject { project_key } => GuiOperationRequest::ArchiveProject { project_key },
        AgentRuntimeGuiOperation::UnarchiveProject { project_key } => GuiOperationRequest::UnarchiveProject { project_key },
        AgentRuntimeGuiOperation::UpdateRuntimeSettings { base_url, selected_project_id } => GuiOperationRequest::UpdateRuntimeSettings {
            base_url,
            selected_project_id: non_empty(selected_project_id),
        },
        AgentRuntimeGuiOperation::UpdateSessionSettings { session_id, project, role, model, workdir, worktree_root, title, name, tracked } => GuiOperationRequest::UpdateSessionSettings {
            session_id,
            project,
            role,
            model,
            workdir,
            worktree_root,
            title,
            name,
            tracked,
        },
        AgentRuntimeGuiOperation::SendMessage { session_id, message } => GuiOperationRequest::SendMessage { session_id, message },
        AgentRuntimeGuiOperation::TerminateProcess { session_id, handle } => GuiOperationRequest::TerminateProcess { session_id, handle },
        AgentRuntimeGuiOperation::InputProcess { session_id, handle, text } => GuiOperationRequest::InputProcess { session_id, handle, text },
        AgentRuntimeGuiOperation::FlushProcess { session_id, handle } => GuiOperationRequest::FlushProcess { session_id, handle },
        AgentRuntimeGuiOperation::CompactSession { session_id, through_turn } => GuiOperationRequest::CompactSession {
            session_id,
            through_turn: non_empty(through_turn),
        },
        AgentRuntimeGuiOperation::CloseSession { session_id, reason } => GuiOperationRequest::CloseSession { session_id, reason: non_empty(reason) },
        AgentRuntimeGuiOperation::ArchiveSession { session_id } => GuiOperationRequest::ArchiveSession { session_id },
        AgentRuntimeGuiOperation::ForkSession { session_id, at_turn } => GuiOperationRequest::ForkSession { session_id, at_turn },
        AgentRuntimeGuiOperation::DecideApproval { approval_id, decision, reason } => GuiOperationRequest::DecideApproval {
            approval_id,
            decision,
            reason: required_non_empty(reason, "approval reason")?,
        },
        AgentRuntimeGuiOperation::ResumeApproval { approval_id } => GuiOperationRequest::ResumeApproval { approval_id },
        AgentRuntimeGuiOperation::ListCommandRegistry { session_id, project_key } => GuiOperationRequest::ListCommandRegistry {
            session_id: non_empty(session_id),
            project_key: non_empty(project_key),
        },
        AgentRuntimeGuiOperation::ShowCommand { action_id, session_id, project_key } => GuiOperationRequest::ShowCommand {
            action_id,
            session_id: non_empty(session_id),
            project_key: non_empty(project_key),
        },
        AgentRuntimeGuiOperation::ListCommandRegistryRequests => GuiOperationRequest::ListCommandRegistryRequests,
        AgentRuntimeGuiOperation::ShowCommandRegistryRequest { request_id } => GuiOperationRequest::ShowCommandRegistryRequest { request_id },
        AgentRuntimeGuiOperation::PreviewCommandRegistryRequest { request_id, decision } => GuiOperationRequest::PreviewCommandRegistryRequest {
            request_id,
            decision: typed_registry_decision(decision),
        },
        AgentRuntimeGuiOperation::DecideCommandRegistryRequest { request_id, decision } => GuiOperationRequest::DecideCommandRegistryRequest {
            request_id,
            decision: typed_registry_decision(decision),
        },
        AgentRuntimeGuiOperation::ApplyCommandRegistryRequest { request_id, session_id } => GuiOperationRequest::ApplyCommandRegistryRequest { request_id, session_id },
        AgentRuntimeGuiOperation::WorkflowMemoryFeedback { memory_id, session_id, feedback, payload } => GuiOperationRequest::WorkflowMemoryFeedback {
            memory_id,
            session_id,
            feedback,
            payload: serde_json::json!({
                "source": payload.source,
                "reason": payload.reason,
                "variant": if payload.has_variant { Some(payload.variant) } else { None },
            }),
        },
        AgentRuntimeGuiOperation::RoleEditorOptions => GuiOperationRequest::RoleEditorOptions,
        AgentRuntimeGuiOperation::ValidateRoleDraft { draft } => GuiOperationRequest::ValidateRoleDraft { draft: typed_role_draft(draft) },
        AgentRuntimeGuiOperation::CreateRoleFromDraft { draft } => GuiOperationRequest::CreateRoleFromDraft { draft: typed_role_draft(draft) },
        AgentRuntimeGuiOperation::UpdateRoleFromDraft { role_id, draft } => GuiOperationRequest::UpdateRoleFromDraft { role_id, draft: typed_role_draft(draft) },
        AgentRuntimeGuiOperation::ShowRoleDetail { role_id } => GuiOperationRequest::ShowRoleDetail { role_id },
        AgentRuntimeGuiOperation::ListRoleVersions { role_id } => GuiOperationRequest::ListRoleVersions { role_id },
        AgentRuntimeGuiOperation::ShowRoleVersion { version_id } => GuiOperationRequest::ShowRoleVersion { version_id },
        AgentRuntimeGuiOperation::ExportRole { role_id } => GuiOperationRequest::ExportRole { role_id },
        AgentRuntimeGuiOperation::ActivateRoleVersion { role_id, version_id } => GuiOperationRequest::ActivateRoleVersion { role_id, version_id },
        AgentRuntimeGuiOperation::ArchiveRole { role_id } => GuiOperationRequest::ArchiveRole { role_id },
        AgentRuntimeGuiOperation::UnarchiveRole { role_id } => GuiOperationRequest::UnarchiveRole { role_id },
    })
}

fn typed_registry_decision(input: crate::signals::AgentRuntimeCommandRegistryDecisionInput) -> CommandRegistryDecisionInput {
    CommandRegistryDecisionInput {
        session_id: non_empty(input.session_id),
        status: input.status,
        final_scope: input.has_final_scope.then(|| GuiRegistryScope {
            scope_type: input.final_scope.scope_type,
            project_key: non_empty(input.final_scope.project_key),
        }),
        final_execution_policy: input.has_final_execution_policy.then(|| GuiFinalExecutionPolicy {
            decision: input.final_execution_policy.decision,
            reason: non_empty(input.final_execution_policy.reason),
        }),
        final_command: input.has_final_command.then(|| GuiCommandSeed {
            action_id: input.final_command.action_id,
            binary_name: input.final_command.binary_name,
            candidate_paths: input.final_command.candidate_paths,
            starlark_object: input.final_command.starlark_object,
            starlark_method: input.final_command.starlark_method,
            argv_prefix: input.final_command.argv_prefix,
            default_cwd: input.final_command.default_cwd,
            cwd_policy: input.final_command.cwd_policy,
            env_policy: input.final_command.env_policy,
            sync_allowed: input.final_command.sync_allowed,
            async_allowed: input.final_command.async_allowed,
            max_runtime_ms: input.final_command.has_max_runtime_ms.then_some(input.final_command.max_runtime_ms),
            end_of_turn_behavior: input.final_command.end_of_turn_behavior,
            end_of_session_behavior: input.final_command.end_of_session_behavior,
            stdin_policy: input.final_command.stdin_policy,
            min_await_ms: input.final_command.min_await_ms,
            max_await_ms: input.final_command.max_await_ms,
            output_buffer_bytes: input.final_command.output_buffer_bytes,
            terminate_grace_ms: input.final_command.terminate_grace_ms,
            output_limit_bytes: input.final_command.output_limit_bytes,
            mutation_class: input.final_command.mutation_class,
            model_description: input.final_command.model_description,
            allow_cwd_arg: input.final_command.allow_cwd_arg,
            allow_args_arg: input.final_command.allow_args_arg,
            forbidden_args: input.final_command.forbidden_args,
            execution_policy: input.final_command.execution_policy,
        }),
    }
}

fn typed_role_draft(input: crate::signals::AgentRuntimeRoleEditorDraft) -> RoleEditorDraft {
    RoleEditorDraft {
        id: input.id,
        version: input.version,
        display_name: input.display_name,
        model_defaults: RoleEditorModelDefaults {
            model: input.model_defaults.model,
            reasoning_effort: input.model_defaults.reasoning_effort,
        },
        instruction_text: input.instruction_text,
        capabilities: input.capabilities,
        policy: input.policy_entries.into_iter().map(|entry| (entry.key, entry.value)).collect(),
        routing: RoleEditorRoutingMetadata {
            mode: input.routing.mode,
            default_recipient: input.routing.has_default_recipient.then_some(input.routing.default_recipient),
            allowed_recipients: input.routing.allowed_recipients,
            reserved_actions: input.routing.reserved_actions,
        },
        visibility: RoleEditorVisibilityMetadata {
            listed: input.visibility.listed,
            owner_visible: input.visibility.owner_visible,
        },
        lifecycle_authority: RoleEditorLifecycleAuthorityMetadata {
            can_spawn_agents: input.lifecycle_authority.can_spawn_agents,
            can_archive_agents: input.lifecycle_authority.can_archive_agents,
            reserved_actions: input.lifecycle_authority.reserved_actions,
        },
    }
}

fn typed_agent_runtime_output(output: GuiTransportOutputPacket) -> AgentRuntimeOutput {
    match output.output {
        GuiTransportOutput::ProjectionSnapshot { projection } => AgentRuntimeOutput::ProjectionSnapshot {
            projection: projection_snapshot_from_value(&projection),
        },
        GuiTransportOutput::ControllerState { controller_state } => AgentRuntimeOutput::ControllerState {
            controller_state: controller_state_from_value(&controller_state),
        },
        GuiTransportOutput::OperationResult { result } => AgentRuntimeOutput::OperationResult {
            result: typed_operation_result(result),
        },
        GuiTransportOutput::StreamOutcome { outcome, projection, controller_state } => AgentRuntimeOutput::StreamOutcome {
            outcome: typed_stream_outcome(outcome),
            projection: projection.as_ref().map(projection_snapshot_from_value).unwrap_or_default(),
            has_projection: projection.is_some(),
            controller_state: controller_state_from_value(&controller_state),
        },
        GuiTransportOutput::Error { error } => AgentRuntimeOutput::Error { error: typed_api_error(error) },
        GuiTransportOutput::WorkbenchView { view_model } => AgentRuntimeOutput::WorkbenchView {
            view_model: typed_workbench_view(view_model),
        },
    }
}

fn projection_snapshot_from_value(value: &serde_json::Value) -> AgentRuntimeProjectionSnapshot {
    AgentRuntimeProjectionSnapshot {
        watermark: value.get("watermark").and_then(|value| value.as_i64()).unwrap_or_default(),
        session_count: value.get("sessions").and_then(|value| value.as_array()).map(|items| items.len() as i64).unwrap_or_default(),
        timeline_count: value.get("timeline").and_then(|value| value.as_array()).map(|items| items.len() as i64).unwrap_or_default(),
        action_count: value.get("pendingApprovals").and_then(|value| value.as_array()).map(|items| items.len() as i64).unwrap_or_default(),
        role_count: value.get("roles").and_then(|value| value.as_array()).map(|items| items.len() as i64).unwrap_or_default(),
        workflow_memory_count: value.get("workflowMemories").and_then(|value| value.as_array()).map(|items| items.len() as i64).unwrap_or_default(),
        selected_chat_entries: value
            .get("selectedChatEntries")
            .and_then(|value| value.as_array())
            .map(|items| items.iter().map(agent_runtime_chat_entry_from_value).collect())
            .unwrap_or_default(),
    }
}

fn agent_runtime_chat_entry_from_value(value: &serde_json::Value) -> AgentRuntimeChatEntry {
    let string = |key: &str| value.get(key).and_then(|value| value.as_str()).unwrap_or_default().to_string();
    let timestamp = string("timestamp");
    let process_id = string("processId");
    AgentRuntimeChatEntry {
        id: string("id"),
        author: string("author"),
        display_label: string("displayLabel"),
        has_timestamp: !timestamp.is_empty(),
        timestamp,
        body: string("body"),
        subtitle: string("subtitle"),
        kind: string("kind"),
        status: string("status"),
        has_process_id: !process_id.is_empty(),
        process_id,
        command: string("command"),
        output: string("output"),
        delivery_state: string("deliveryState"),
        is_streaming: value.get("isStreaming").and_then(|value| value.as_bool()).unwrap_or_default(),
        is_tool: value.get("isTool").and_then(|value| value.as_bool()).unwrap_or_default(),
    }
}

fn controller_state_from_value(value: &serde_json::Value) -> AgentRuntimeControllerState {
    let selected_session_id = value.get("selectedSessionId").and_then(|value| value.as_str()).unwrap_or_default().to_string();
    let last_error = value
        .get("transientErrors")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|value| value.get("error"))
        .and_then(|value| value.get("message"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    AgentRuntimeControllerState {
        connection_state: value.get("connectionState").and_then(|value| value.as_str()).unwrap_or_default().to_string(),
        has_selected_session_id: !selected_session_id.is_empty(),
        selected_session_id,
        base_url: String::new(),
        has_last_error: !last_error.is_empty(),
        last_error,
    }
}

fn typed_operation_result(result: robdex_agent_runtime_projection::GuiOperationResult) -> AgentRuntimeOperationResult {
    let mut error_message = None;
    let outcome = match result.outcome {
        GuiOperationOutcome::Accepted { .. } => "accepted",
        GuiOperationOutcome::ProjectionUpdated { .. } => "projectionUpdated",
        GuiOperationOutcome::DirectValue { .. } => "directValue",
        GuiOperationOutcome::CommandRegistryRequests { .. } => "commandRegistryRequests",
        GuiOperationOutcome::Error { error } => {
            error_message = Some(error.error.message);
            "error"
        }
    };
    AgentRuntimeOperationResult {
        operation: format!("{:?}", result.operation),
        outcome: outcome.to_string(),
        message: error_message.unwrap_or_else(|| format!("{:?}", result.expectation)),
    }
}

fn typed_stream_outcome(outcome: GuiStreamOutcomePacket) -> AgentRuntimeStreamOutcome {
    match outcome {
        GuiStreamOutcomePacket::Hello { watermark, runtime_identity } => {
            let runtime_identity = runtime_identity.unwrap_or_default();
            AgentRuntimeStreamOutcome::Hello {
                watermark,
                has_runtime_identity: !runtime_identity.is_empty(),
                runtime_identity,
            }
        }
        GuiStreamOutcomePacket::DeltaApplied { apply_outcome, .. } => AgentRuntimeStreamOutcome::DeltaApplied { apply_outcome },
        GuiStreamOutcomePacket::ResyncRequired { reason } => {
            let reason = reason.unwrap_or_default();
            AgentRuntimeStreamOutcome::ResyncRequired {
                has_reason: !reason.is_empty(),
                reason,
            }
        }
        GuiStreamOutcomePacket::ServerShutdown => AgentRuntimeStreamOutcome::ServerShutdown,
        GuiStreamOutcomePacket::StreamClosed => AgentRuntimeStreamOutcome::StreamClosed,
    }
}

fn typed_api_error(error: ApiErrorPacket) -> AgentRuntimeApiError {
    let details = error
        .error
        .details
        .as_object()
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| AgentRuntimeFact {
                    label: key.clone(),
                    value: value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string()),
                })
                .collect()
        })
        .unwrap_or_default();
    AgentRuntimeApiError {
        code: error.error.code,
        message: error.error.message,
        details,
    }
}

fn typed_workbench_view(view: InternalWorkbenchViewModel) -> AgentRuntimeWorkbenchViewModel {
    let error_message = view.error_message.unwrap_or_default();
    AgentRuntimeWorkbenchViewModel {
        discovery: typed_discovery_view(view.discovery),
        remote_discovery: typed_discovery_view(view.remote_discovery),
        imported_remote_discovery: typed_discovery_view(view.imported_remote_discovery),
        connection_state: view.connection_state,
        connection_tone: view.connection_tone,
        base_url: view.base_url,
        status_label: view.status_label,
        watermark_label: view.watermark_label,
        status_badges: view.status_badges.into_iter().map(|badge| AgentRuntimeBadge { label: badge.label, value: badge.value, tone: badge.tone }).collect(),
        model_options: view.model_options.into_iter().map(|option| AgentRuntimeModelOption {
            id: option.id,
            display_label: option.display_label,
            source: option.source,
            is_default: option.is_default,
        }).collect(),
        selected_session_label: view.selected_session_label,
        sessions_title: view.sessions_title,
        sessions_subtitle: view.sessions_subtitle,
        timeline_title: view.timeline_title,
        timeline_subtitle: view.timeline_subtitle,
        actions_title: view.actions_title,
        actions_subtitle: view.actions_subtitle,
        detail_title: view.detail_title,
        detail_subtitle: view.detail_subtitle,
        sessions_empty_title: view.sessions_empty_title,
        sessions_empty_text: view.sessions_empty_text,
        timeline_empty_title: view.timeline_empty_title,
        timeline_empty_text: view.timeline_empty_text,
        actions_empty_title: view.actions_empty_title,
        actions_empty_text: view.actions_empty_text,
        sessions: view.sessions.into_iter().map(|row| AgentRuntimeSessionRow { id: row.id, title: row.title, status: row.status, subtitle: row.subtitle, group_label: row.group_label, tone: row.tone }).collect(),
        timeline: view.timeline.into_iter().map(|row| AgentRuntimeTimelineRow { id: row.id, title: row.title, subtitle: row.subtitle, status: row.status, tone: row.tone }).collect(),
        actions: view.actions.into_iter().map(typed_action_row).collect(),
        role_admin: typed_role_admin_view(view.role_admin),
        workflow_memory: typed_workflow_memory_view(view.workflow_memory),
        controller_facts: view.controller_facts.into_iter().map(|fact| AgentRuntimeFact { label: fact.label, value: fact.value }).collect(),
        output_log: view.output_log,
        pending_request_count: view.pending_request_count as i64,
        has_error_message: !error_message.is_empty(),
        error_message,
        shell: typed_conversation_shell_view(view.shell),
    }
}

fn typed_conversation_shell_view(view: robdex_agent_runtime::rinf_transport::AgentRuntimeConversationShellViewModel) -> AgentRuntimeConversationShellViewModel {
    let selected_session_id = view.selected_session_id.unwrap_or_default();
    AgentRuntimeConversationShellViewModel {
        projects: view.projects.into_iter().map(|project| AgentRuntimeShellProjectRow {
            id: project.id,
            title: project.title,
            subtitle: project.subtitle,
            selectable: project.selectable,
            default_workdir: project.default_workdir,
            default_worktree_root: project.default_worktree_root,
            default_role_id: project.default_role_id.unwrap_or_default(),
            default_model: project.default_model,
            tracked: project.tracked,
            listed: project.listed,
            archived: project.archived,
        }).collect(),
        sessions: view.sessions.into_iter().map(|row| AgentRuntimeSessionRow { id: row.id, title: row.title, status: row.status, subtitle: row.subtitle, group_label: row.group_label, tone: row.tone }).collect(),
        has_selected_session_id: !selected_session_id.is_empty(),
        selected_session_id,
        selected_conversation: view.selected_conversation.into_iter().map(|entry| {
            let timestamp = entry.timestamp.unwrap_or_default();
            let process_id = entry.process_id.unwrap_or_default();
            AgentRuntimeChatEntry {
                id: entry.id,
                author: entry.author,
                display_label: entry.display_label,
                has_timestamp: !timestamp.is_empty(),
                timestamp,
                body: entry.body,
                subtitle: entry.subtitle,
                kind: entry.kind,
                status: entry.status,
                has_process_id: !process_id.is_empty(),
                process_id,
                command: entry.command,
                output: entry.output,
                delivery_state: entry.delivery_state,
                is_streaming: entry.is_streaming,
                is_tool: entry.is_tool,
            }
        }).collect(),
        dynamic_roles: view.dynamic_roles.into_iter().map(|role| AgentRuntimeShellRolePresentation {
            role_id: role.role_id,
            display_label: role.display_label,
            short_label: role.short_label,
            tone: role.tone,
            description: role.description,
        }).collect(),
        actions: view.actions.into_iter().map(typed_action_row).collect(),
        settings: view.settings.into_iter().map(|fact| AgentRuntimeFact { label: fact.label, value: fact.value }).collect(),
        role_management: typed_role_admin_view(view.role_management),
        workflow_memory: typed_workflow_memory_view(view.workflow_memory),
        command_registry_requests: view.command_registry_requests.into_iter().map(typed_action_row).collect(),
        approvals: view.approvals.into_iter().map(typed_action_row).collect(),
        diagnostics: view.diagnostics.into_iter().map(|fact| AgentRuntimeFact { label: fact.label, value: fact.value }).collect(),
        operation_surfaces: view.operation_surfaces.into_iter().map(|surface| AgentRuntimeOperationSurface {
            surface_id: surface.surface_id,
            title: surface.title,
            subtitle: surface.subtitle,
            rows: surface.rows.into_iter().map(|fact| AgentRuntimeFact { label: fact.label, value: fact.value }).collect(),
            actions: surface.actions.into_iter().map(typed_action_row).collect(),
        }).collect(),
    }
}

fn typed_discovery_view(view: InternalDiscoveryView) -> AgentRuntimeDiscoveryView {
    let base_url = view.base_url.unwrap_or_default();
    let health_url = view.health_url.unwrap_or_default();
    let web_socket_url = view.web_socket_url.unwrap_or_default();
    let runtime_identity = view.runtime_identity.unwrap_or_default();
    let last_imported_at = view.last_imported_at.unwrap_or_default();
    let service_state = view.service_state.unwrap_or_default();
    AgentRuntimeDiscoveryView {
        source_type: view.source_type,
        source_path: view.source_path,
        state: view.state,
        tone: view.tone,
        title: view.title,
        message: view.message,
        has_base_url: !base_url.is_empty(),
        base_url,
        has_health_url: !health_url.is_empty(),
        health_url,
        has_web_socket_url: !web_socket_url.is_empty(),
        web_socket_url,
        has_runtime_identity: !runtime_identity.is_empty(),
        runtime_identity,
        discovery_path: view.discovery_path,
        has_last_imported_at: !last_imported_at.is_empty(),
        last_imported_at,
        has_service_state: !service_state.is_empty(),
        service_state,
        connectable: view.connectable,
        diagnostics: view.diagnostics,
    }
}

fn typed_action_row(row: robdex_agent_runtime::rinf_transport::AgentRuntimeWorkbenchActionRow) -> AgentRuntimeActionRow {
    AgentRuntimeActionRow {
        id: row.id,
        title: row.title,
        subtitle: row.subtitle,
        kind: row.kind,
        state_text: row.state_text,
        tone: row.tone,
    }
}

fn typed_role_admin_view(view: InternalRoleAdminView) -> AgentRuntimeRoleAdminView {
    AgentRuntimeRoleAdminView {
        title: view.title,
        subtitle: view.subtitle,
        empty_title: view.empty_title,
        empty_text: view.empty_text,
        rows: view.rows.into_iter().map(|row| AgentRuntimeRoleRow {
            id: row.id,
            title: row.title,
            subtitle: row.subtitle,
            status: row.status,
            tone: row.tone,
            current_version: row.current_version_id.unwrap_or_default(),
        }).collect(),
        has_selected_detail: view.selected_detail.is_some(),
        selected_detail: view.selected_detail.map(|detail| AgentRuntimeRoleDetail {
            id: detail.id,
            title: detail.display_name.clone(),
            display_name: detail.display_name,
            version: detail.version,
            status: detail.status,
            instructions_preview: detail.instruction_text,
            model_label: detail.model,
            routing_label: detail.routing.iter().map(|fact| format!("{} {}", fact.label, fact.value)).collect::<Vec<_>>().join(" · "),
            visibility_label: detail.visibility.iter().map(|fact| format!("{} {}", fact.label, fact.value)).collect::<Vec<_>>().join(" · "),
            lifecycle_label: detail.lifecycle_authority.iter().map(|fact| format!("{} {}", fact.label, fact.value)).collect::<Vec<_>>().join(" · "),
            policy_rows: detail.policy.into_iter().map(|row| AgentRuntimeRolePolicyRow { label: row.action, value: row.decision }).collect(),
        }).unwrap_or_default(),
        version_rows: view.version_rows.into_iter().map(|row| AgentRuntimeRoleVersionRow {
            can_activate: row.status != "current",
            is_current: row.status == "current",
            version_id: row.version_id,
            version: row.version,
            status: row.status,
            created_at: row.created_at.unwrap_or_default(),
        }).collect(),
        has_editor_draft: view.editor_draft.is_some(),
        editor_draft: view.editor_draft.map(|draft| AgentRuntimeRoleEditorDraftView {
            role_id: draft.role_id,
            version: draft.version,
            display_name: draft.display_name,
            model: draft.model,
            reasoning_effort: draft.reasoning_effort,
            instruction_text: draft.instruction_text,
            capabilities: draft.capabilities,
            policy_rows: draft.policy.into_iter().map(|row| AgentRuntimeRolePolicyRow { label: row.action, value: row.decision }).collect(),
            routing_mode: draft.routing_mode,
            default_recipient: draft.default_recipient.unwrap_or_default(),
            allowed_recipients: draft.allowed_recipients,
            listed: draft.listed,
            owner_visible: draft.owner_visible,
            can_spawn_agents: draft.can_spawn_agents,
            can_archive_agents: draft.can_archive_agents,
            can_validate: true,
            can_create: true,
            can_update: true,
        }).unwrap_or_default(),
        validation_errors: view.validation_errors,
        action_states: view.action_states.into_iter().map(typed_action_row).collect(),
        editor_options: AgentRuntimeRoleEditorOptionsView {
            models: view.editor_options.models,
            reasoning_efforts: view.editor_options.reasoning_efforts,
            capabilities: view.editor_options.capabilities,
            policy_actions: view.editor_options.policy_actions,
            policy_decisions: view.editor_options.policy_decisions,
            routing_modes: view.editor_options.routing_modes,
            recipients: view.editor_options.recipients,
            reserved_actions: view.editor_options.reserved_actions,
        },
    }
}

fn typed_workflow_memory_view(view: InternalWorkflowMemoryView) -> AgentRuntimeWorkflowMemoryView {
    AgentRuntimeWorkflowMemoryView {
        title: view.title,
        subtitle: view.subtitle,
        empty_title: view.empty_title,
        empty_text: view.empty_text,
        rows: view.rows.into_iter().map(|row| {
            let has_project_key = row.project_key.is_some();
            let project_key = row.project_key.unwrap_or_default();
            let has_promoted_at = row.promoted_at.is_some();
            let promoted_at = row.promoted_at.unwrap_or_default();
            AgentRuntimeWorkflowMemoryRow {
            id: row.id,
            title: row.title,
            scope_label: row.scope_type,
            project_key,
            has_project_key,
            helpful_score: format!("{:.2}", row.helpful_score),
            promoted_at,
            has_promoted_at,
            source_session_id: row.source_session_id,
            reason: row.subtitle,
            tone: row.tone,
            is_selected: row.selected,
        }}).collect(),
        has_selected_detail: view.selected_detail.is_some(),
        selected_detail: view.selected_detail.map(|detail| {
            let has_source_script_run_id = detail.source_script_run_id.is_some();
            let source_script_run_id = detail.source_script_run_id.unwrap_or_default();
            let has_feedback_session_id = detail.feedback_session_id.is_some();
            let feedback_session_id = detail.feedback_session_id.unwrap_or_default();
            AgentRuntimeWorkflowMemoryDetail {
            id: detail.id,
            title: detail.title,
            reason: detail.reason,
            summary: detail.summary,
            source_session_id: detail.source_session_id,
            source_script_run_id,
            has_source_script_run_id,
            source_preview: if detail.source_starlark.is_empty() { detail.source_preview } else { detail.source_starlark },
            provider: detail.provider.unwrap_or_default(),
            model: detail.model.unwrap_or_default(),
            dimensions: detail.dimensions.map(|value| value.to_string()).unwrap_or_default(),
            storage_label: detail.storage_type.unwrap_or_default(),
            source_hash: detail.source_hash.unwrap_or_default(),
            command_fingerprint: detail.command_fingerprint.unwrap_or_default(),
            score: format!("{:.2}", detail.helpful_score),
            scope_label: detail.scope_label,
            feedback_enabled: detail.feedback_enabled,
            feedback_session_id,
            has_feedback_session_id,
            events: view.recent_events.into_iter().map(|event| AgentRuntimeWorkflowMemoryEvent {
                id: event.id,
                title: event.title,
                subtitle: event.subtitle,
                created_at: event.created_at.unwrap_or_default(),
                tone: event.tone,
            }).collect(),
        }}).unwrap_or_default(),
        action_states: view.feedback_actions.into_iter().map(typed_action_row).collect(),
    }
}

async fn handle_agent_runtime_request(
    handle: &GuiTransportHandle,
    request_id: String,
    request: AgentRuntimeRequest,
) {
    let packet = match typed_agent_runtime_request_packet(&request_id, request) {
        Ok(packet) => packet,
        Err(error) => {
            AgentRuntimeOutputSignal {
                request_id: request_id.clone(),
                output: AgentRuntimeOutput::Error { error: typed_api_error(error) },
            }
            .send_signal_to_dart();
            return;
        }
    };
    for output in handle.send(packet).await {
        emit_agent_runtime_output(output);
    }
}

fn emit_agent_runtime_output(output: GuiTransportOutputPacket) {
    AgentRuntimeOutputSignal {
        request_id: output.request_id.clone(),
        output: typed_agent_runtime_output(output),
    }
    .send_signal_to_dart();
}

fn spawn_unit<TSignal, F>(tx: mpsc::UnboundedSender<Action>, map: F)
where
    TSignal: DartSignal + Send + 'static,
    F: Fn() -> Action + Send + Sync + 'static,
{
    let map = Arc::new(map);
    tokio::spawn(async move {
        let receiver = TSignal::get_dart_signal_receiver();
        while let Some(_signal) = receiver.recv().await {
            let _ = tx.send(map());
        }
    });
}

fn spawn_map<TSignal, F>(tx: mpsc::UnboundedSender<Action>, map: F)
where
    TSignal: DartSignal + Send + 'static,
    F: Fn(rinf::DartSignalPack<TSignal>) -> Action + Send + Sync + 'static,
{
    let map = Arc::new(map);
    tokio::spawn(async move {
        let receiver = TSignal::get_dart_signal_receiver();
        while let Some(signal) = receiver.recv().await {
            let _ = tx.send(map(signal));
        }
    });
}

async fn handle_action(
    client: &mut Option<WorkbenchClient>,
    current_view: &Option<WorkbenchViewData>,
    terminals: &mut TerminalRegistry,
    action: Action,
) -> Result<WorkbenchViewData> {
    match action {
        Action::Initialize { host, port } => {
            let mut next_client = WorkbenchClient::new(BridgeEndpoint::new(&host, port));
            let view = next_client.load_initial_view().await?;
            *client = Some(next_client);
            Ok(view)
        }
        Action::Reload => client.as_mut().ok_or_else(|| anyhow!("Not connected"))?.load_initial_view().await,
        Action::SelectThread(thread_id) => {
            let client_ref = client.as_mut().ok_or_else(|| anyhow!("Not connected"))?;
            let entries = client_ref
                .fetch_thread_history(&thread_id)
                .await?;
            if current_view
                .as_ref()
                .is_some_and(|view| view.threads.iter().any(|thread| thread.id == thread_id))
            {
                select_thread_from_current_view(current_view, &thread_id, entries)
            } else {
                let view = client_ref
                    .refresh_thread_with_preserved_messages(thread_id.clone(), entries)
                    .await?;
                if view.selection.thread_id.as_deref() != Some(thread_id.as_str()) {
                    return Err(anyhow!(
                        "Thread is not present in the current workbench state"
                    ));
                }
                Ok(view)
            }
        }
        Action::FetchThreadHistory => {
            let thread_id = current_view
                .as_ref()
                .and_then(|view| view.selection.thread_id.clone())
                .ok_or_else(|| anyhow!("No thread selected"))?;
            emit_thread_history_state(None, true, "");
            let history_result = client
                .as_mut()
                .ok_or_else(|| anyhow!("Not connected"))?
                .fetch_thread_history(&thread_id)
                .await;
            match history_result {
                Ok(entries) => emit_thread_history_state(Some(entries), false, ""),
                Err(error) => emit_thread_history_state(None, false, &error.to_string()),
            }
            current_view.clone().ok_or_else(|| anyhow!("No current view"))
        }
        Action::ThreadCompact => {
            let thread_id = current_view
                .as_ref()
                .and_then(|view| view.selection.thread_id.clone())
                .ok_or_else(|| anyhow!("No thread selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .thread_compact(&thread_id)
                .await?;
            current_view_clone(current_view)
        }
        Action::TerminateCommandExecution(process_id) => {
            let thread_id = current_view
                .as_ref()
                .and_then(|view| view.selection.thread_id.clone())
                .ok_or_else(|| anyhow!("No thread selected"))?;
            client
                .as_ref()
                .ok_or_else(|| anyhow!("Not connected"))?
                .terminate_command_execution(&thread_id, &process_id)
                .await?;
            current_view_clone(current_view)
        }
        Action::LoadThreadStats { request_id, thread_id } => {
            let task = "threadStats";
            let result = client
                .as_ref()
                .ok_or_else(|| anyhow!("Not connected"))?
                .fetch_thread_stats_json(&thread_id)
                .await;
            match result {
                Ok(payload) => emit_bridge_task_result(request_id, task, payload),
                Err(error) => emit_bridge_task_error(request_id, task, &error),
            }
            current_view_clone(current_view)
        }
        Action::LoadPeriodStats {
            request_id,
            start_ms,
            end_ms,
            label,
            quota_reset_at_ms,
            quota_remaining_percent,
        } => {
            let task = "periodStats";
            let result = client
                .as_ref()
                .ok_or_else(|| anyhow!("Not connected"))?
                .fetch_period_stats_json(
                    start_ms,
                    end_ms,
                    &label,
                    quota_reset_at_ms,
                    quota_remaining_percent,
                )
                .await;
            match result {
                Ok(payload) => emit_bridge_task_result(request_id, task, payload),
                Err(error) => emit_bridge_task_error(request_id, task, &error),
            }
            current_view_clone(current_view)
        }
        Action::LoadProjectHookLogs { request_id, project_id } => {
            let task = "projectHookLogs";
            let result = client
                .as_ref()
                .ok_or_else(|| anyhow!("Not connected"))?
                .fetch_project_hook_logs_json(&project_id)
                .await;
            match result {
                Ok(payload) => emit_bridge_task_result(request_id, task, payload),
                Err(error) => emit_bridge_task_error(request_id, task, &error),
            }
            current_view_clone(current_view)
        }
        Action::ClearProjectHookLogs { request_id, project_id } => {
            let task = "clearProjectHookLogs";
            let result = client
                .as_ref()
                .ok_or_else(|| anyhow!("Not connected"))?
                .clear_project_hook_logs(&project_id)
                .await;
            match result {
                Ok(()) => emit_bridge_task_result(request_id, task, serde_json::json!({"ok": true})),
                Err(error) => emit_bridge_task_error(request_id, task, &error),
            }
            current_view_clone(current_view)
        }
        Action::LoadRequirementComposables {
            request_id,
            sender_thread_id,
            recipient_thread_id,
            project_path,
        } => {
            let task = "requirementComposables";
            let result = client
                .as_ref()
                .ok_or_else(|| anyhow!("Not connected"))?
                .fetch_requirement_composables_json(
                    sender_thread_id.as_deref(),
                    recipient_thread_id.as_deref(),
                    project_path.as_deref(),
                )
                .await;
            match result {
                Ok(payload) => emit_bridge_task_result(request_id, task, payload),
                Err(error) => emit_bridge_task_error(request_id, task, &error),
            }
            current_view_clone(current_view)
        }
        Action::SetThreadRequirements {
            request_id,
            sender_thread_id,
            recipient_thread_id,
            project_path,
            requirement_set_json,
        } => {
            let task = "setThreadRequirements";
            let requirement_set_json = requirement_set_json.filter(|value| !value.trim().is_empty());
            let result = client
                .as_ref()
                .ok_or_else(|| anyhow!("Not connected"))?
                .set_thread_requirements_json(
                    sender_thread_id.as_deref(),
                    &recipient_thread_id,
                    project_path.as_deref(),
                    requirement_set_json,
                )
                .await;
            match result {
                Ok(payload) => emit_bridge_task_result(request_id, task, payload),
                Err(error) => emit_bridge_task_error(request_id, task, &error),
            }
            current_view_clone(current_view)
        }
        Action::UploadImageBytes {
            request_id,
            filename,
            content_type,
            bytes,
        } => {
            let task = "uploadImageBytes";
            let result = client
                .as_ref()
                .ok_or_else(|| anyhow!("Not connected"))?
                .upload_image_bytes(&filename, &content_type, bytes)
                .await;
            match result {
                Ok(path) => emit_bridge_task_result(request_id, task, serde_json::json!({"path": path})),
                Err(error) => emit_bridge_task_error(request_id, task, &error),
            }
            current_view_clone(current_view)
        }
        Action::LoadImageBytes { request_id, path } => {
            let task = "loadImageBytes";
            let result = client
                .as_ref()
                .ok_or_else(|| anyhow!("Not connected"))?
                .load_image_bytes(&path)
                .await;
            match result {
                Ok((bytes_base64, content_type)) => emit_bridge_task_result(
                    request_id,
                    task,
                    serde_json::json!({
                        "path": path,
                        "bytesBase64": bytes_base64,
                        "contentType": content_type,
                    }),
                ),
                Err(error) => emit_bridge_task_error(request_id, task, &error),
            }
            current_view_clone(current_view)
        }
        Action::CreateProject {
            name,
            root_path,
            default_cwd,
        } => {
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .create_project(name, root_path, default_cwd)
                .await?;
            current_view_clone(current_view)
        }
        Action::SelectProject(project_id) => {
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .select_project(project_id)
                .await?;
            current_view_clone(current_view)
        }
        Action::DeleteProject(project_id) => {
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .delete_project(project_id)
                .await?;
            current_view_clone(current_view)
        }
        Action::UpdateGlobalSettings {
            approval_policy,
            sandbox_mode,
            network_access,
        } => {
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .update_global_settings(approval_policy, sandbox_mode, network_access)
                .await?;
            current_view_clone(current_view)
        }
        Action::UpdateProject {
            project_id,
            name,
            default_cwd,
            auto_route_replies,
            route_approval_requests,
            preferred_model_provider,
            default_model_id,
            default_reasoning_effort,
            default_sandbox_mode,
            default_approval_policy,
            default_network_access,
            role_runtime_defaults,
            orchestrator_model_id,
            orchestrator_reasoning_effort,
            worker_model_id,
            worker_reasoning_effort,
            qa_model_id,
            qa_reasoning_effort,
            designer_model_id,
            designer_reasoning_effort,
            planner_model_id,
            planner_reasoning_effort,
            requirements_reviewer_model_id,
            requirements_reviewer_reasoning_effort,
            orchestrator_developer_instructions,
            worker_developer_instructions,
            qa_developer_instructions,
            designer_developer_instructions,
            operator_developer_instructions,
            hidden_developer_instructions,
            permanent_requirement_composables,
        } => {
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .update_project(
                    project_id,
                    name,
                    default_cwd,
                    auto_route_replies,
                    route_approval_requests,
                    preferred_model_provider,
                    default_model_id,
                    default_reasoning_effort,
                    default_sandbox_mode,
                    default_approval_policy,
                    default_network_access,
                    role_runtime_defaults,
                    orchestrator_model_id,
                    orchestrator_reasoning_effort,
                    worker_model_id,
                    worker_reasoning_effort,
                    qa_model_id,
                    qa_reasoning_effort,
                    designer_model_id,
                    designer_reasoning_effort,
                    planner_model_id,
                    planner_reasoning_effort,
                    requirements_reviewer_model_id,
                    requirements_reviewer_reasoning_effort,
                    orchestrator_developer_instructions,
                    worker_developer_instructions,
                    qa_developer_instructions,
                    designer_developer_instructions,
                    operator_developer_instructions,
                    hidden_developer_instructions,
                    permanent_requirement_composables,
                )
                .await?;
            current_view_clone(current_view)
        }
        Action::CreateThread {
            project_id,
            title,
            initial_prompt,
            role,
            approval_policy,
            sandbox_mode,
            network_access,
            model_id,
            reasoning_effort,
            requirement_set_json,
        } => {
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .create_thread(
                    project_id,
                    title,
                    initial_prompt,
                    role,
                    approval_policy,
                    sandbox_mode,
                    network_access,
                    model_id,
                    reasoning_effort,
                    requirement_set_json,
                )
                .await?;
            current_view_clone(current_view)
        }
        Action::SpawnAgent { name, role, prompt, requirement_set_json } => {
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .spawn_agent(name, role, prompt, requirement_set_json)
                .await?;
            current_view_clone(current_view)
        }
        Action::SetProjectOrchestrator {
            project_id,
            project_path,
            thread_id,
        } => {
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .set_project_orchestrator(&project_id, &project_path, &thread_id)
                .await?;
            current_view_clone(current_view)
        }
        Action::CreateThreadGroup(title) => {
            let view = current_view.as_ref().ok_or_else(|| anyhow!("No current view"))?;
            let sender_thread_id = view
                .selection
                .project_orchestrator_thread_id
                .clone()
                .ok_or_else(|| anyhow!("No project orchestrator configured"))?;
            let project_path = view
                .selection
                .project_root_path
                .clone()
                .ok_or_else(|| anyhow!("No project selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .create_thread_group(
                    &sender_thread_id,
                    &project_path,
                    &title,
                    view.selection.thread_id.as_deref(),
                )
                .await?;
            current_view_clone(current_view)
        }
        Action::RenameThreadGroup { group_id, title } => {
            let view = current_view.as_ref().ok_or_else(|| anyhow!("No current view"))?;
            let sender_thread_id = view
                .selection
                .project_orchestrator_thread_id
                .clone()
                .ok_or_else(|| anyhow!("No project orchestrator configured"))?;
            let project_path = view
                .selection
                .project_root_path
                .clone()
                .ok_or_else(|| anyhow!("No project selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .update_thread_group(
                    &sender_thread_id,
                    &project_path,
                    &group_id,
                    Some(title.as_str()),
                    None,
                )
                .await?;
            current_view_clone(current_view)
        }
        Action::DeleteThreadGroup(group_id) => {
            let view = current_view.as_ref().ok_or_else(|| anyhow!("No current view"))?;
            let sender_thread_id = view
                .selection
                .project_orchestrator_thread_id
                .clone()
                .ok_or_else(|| anyhow!("No project orchestrator configured"))?;
            let project_path = view
                .selection
                .project_root_path
                .clone()
                .ok_or_else(|| anyhow!("No project selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .delete_thread_group(&sender_thread_id, &project_path, &group_id)
                .await?;
            current_view_clone(current_view)
        }
        Action::ArchiveThreadGroup(group_id) => {
            let view = current_view.as_ref().ok_or_else(|| anyhow!("No current view"))?;
            let sender_thread_id = view
                .selection
                .project_orchestrator_thread_id
                .clone()
                .ok_or_else(|| anyhow!("No project orchestrator configured"))?;
            let project_path = view
                .selection
                .project_root_path
                .clone()
                .ok_or_else(|| anyhow!("No project selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .archive_thread_group(&sender_thread_id, &project_path, &group_id)
                .await?;
            current_view_clone(current_view)
        }
        Action::MoveSelectedThreadToGroup(group_id) => {
            let view = current_view.as_ref().ok_or_else(|| anyhow!("No current view"))?;
            let sender_thread_id = view
                .selection
                .project_orchestrator_thread_id
                .clone()
                .ok_or_else(|| anyhow!("No project orchestrator configured"))?;
            let project_path = view
                .selection
                .project_root_path
                .clone()
                .ok_or_else(|| anyhow!("No project selected"))?;
            let thread_id = view
                .selection
                .thread_id
                .clone()
                .ok_or_else(|| anyhow!("No thread selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .move_thread_to_group(
                    &sender_thread_id,
                    &project_path,
                    &thread_id,
                    group_id.as_deref(),
                )
                .await?;
            current_view_clone(current_view)
        }
        Action::UpdateWorkerMetadata {
            issue_number,
            pull_request_number,
            blocked_reason,
            unblock_when,
            clear_blocked,
        } => {
            let view = current_view.as_ref().ok_or_else(|| anyhow!("No current view"))?;
            let sender_thread_id = view
                .selection
                .project_orchestrator_thread_id
                .clone()
                .ok_or_else(|| anyhow!("No project orchestrator configured"))?;
            let project_path = view
                .selection
                .project_root_path
                .clone()
                .ok_or_else(|| anyhow!("No project selected"))?;
            let recipient_thread_id = view
                .selection
                .thread_id
                .clone()
                .ok_or_else(|| anyhow!("No thread selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .update_worker_metadata(
                    &sender_thread_id,
                    &recipient_thread_id,
                    &project_path,
                    issue_number,
                    pull_request_number,
                    blocked_reason.as_deref(),
                    unblock_when.as_deref(),
                    clear_blocked,
                )
                .await?;
            current_view_clone(current_view)
        }
        Action::SendMessage {
            text,
            local_image_paths,
            requirement_set_json,
        } => {
            let thread_id = current_view
                .as_ref()
                .and_then(|view| view.selection.thread_id.clone())
                .ok_or_else(|| anyhow!("No thread selected"))?;
            client
                .as_mut()
                .ok_or_else(|| anyhow!("Not connected"))?
                .send_message(&thread_id, &text, &local_image_paths, requirement_set_json)
                .await?;
            current_view_clone(current_view)
        }
        Action::InterruptThread => {
            let thread_id = current_view
                .as_ref()
                .and_then(|view| view.selection.thread_id.clone())
                .ok_or_else(|| anyhow!("No thread selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .interrupt_thread(&thread_id)
                .await?;
            current_view_clone(current_view)
        }
        Action::DecideApproval {
            approval_id,
            decision,
            message,
        } => {
            let sender_thread_id = current_view
                .as_ref()
                .and_then(|view| view.selection.thread_id.clone())
                .ok_or_else(|| anyhow!("No orchestrator thread selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .decide_approval(
                    &sender_thread_id,
                    &approval_id,
                    &decision,
                    message.as_deref(),
                )
                .await?;
            current_view_clone(current_view)
        }
        Action::UpdateThreadSettings {
            role,
            approval_policy,
            sandbox_mode,
            network_access,
            model_id,
            reasoning_effort,
            service_tier,
        } => {
            let thread_id = current_view
                .as_ref()
                .and_then(|view| view.selection.thread_id.clone())
                .ok_or_else(|| anyhow!("No thread selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?
                .update_thread_metadata(
                    &thread_id,
                    role.as_deref(),
                    approval_policy.as_deref(),
                    sandbox_mode.as_deref(),
                    network_access,
                    model_id.as_deref(),
                    reasoning_effort.as_deref(),
                    service_tier.as_deref(),
                )
                .await?;
            current_view_clone(current_view)
        }
        Action::SetThreadRunningState(running) => {
            let thread_id = current_view
                .as_ref()
                .and_then(|view| view.selection.thread_id.clone())
                .ok_or_else(|| anyhow!("No thread selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?.set_thread_running_state(&thread_id, running).await?;
            current_view_clone(current_view)
        }
        Action::RenameThread(name) => {
            let thread_id = current_view
                .as_ref()
                .and_then(|view| view.selection.thread_id.clone())
                .ok_or_else(|| anyhow!("No thread selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?.rename_thread(&thread_id, &name).await?;
            current_view_clone(current_view)
        }
        Action::ArchiveThread => {
            let thread_id = current_view
                .as_ref()
                .and_then(|view| view.selection.thread_id.clone())
                .ok_or_else(|| anyhow!("No thread selected"))?;
            client.as_mut().ok_or_else(|| anyhow!("Not connected"))?.archive_thread(&thread_id).await?;
            current_view_clone(current_view)
        }
        Action::WarmHandoff(prompt) => {
            let view = current_view
                .as_ref()
                .ok_or_else(|| anyhow!("Not connected"))?;
            let recipient_thread_id = view
                .selection
                .thread_id
                .clone()
                .ok_or_else(|| anyhow!("No thread selected"))?;
            let can_self_handoff = matches!(
                view.selection.thread_role.as_deref(),
                Some("orchestrator" | "operator" | "hidden" | "designer")
            );
            let sender_thread_id = if can_self_handoff {
                recipient_thread_id.clone()
            } else {
                view.selection
                    .project_orchestrator_thread_id
                    .clone()
                    .ok_or_else(|| anyhow!("No project orchestrator configured"))?
            };
            let project_path = view
                .selection
                .project_root_path
                .clone()
                .ok_or_else(|| anyhow!("No project path available"))?;
            client
                .as_mut()
                .ok_or_else(|| anyhow!("Not connected"))?
                .warm_handoff(&sender_thread_id, &recipient_thread_id, &project_path, &prompt)
                .await
        }
        Action::TerminalOpen {
            request_id,
            host,
            username,
            cols,
            rows,
        } => {
            if let Err(error) = terminals.open(request_id.clone(), host.clone(), username.clone(), cols, rows) {
                TerminalEventSignal {
                    request_id,
                    session_id: String::new(),
                    kind: "error".to_string(),
                    data: error.to_string(),
                    host,
                    username,
                }
                .send_signal_to_dart();
            }
            current_view_clone(current_view)
        }
        Action::TerminalInput { session_id, data } => {
            terminals.input(&session_id, &data)?;
            current_view_clone(current_view)
        }
        Action::TerminalResize {
            session_id,
            cols,
            rows,
        } => {
            terminals.resize(&session_id, cols, rows)?;
            current_view_clone(current_view)
        }
        Action::TerminalClose(session_id) => {
            terminals.close(&session_id);
            current_view_clone(current_view)
        }
        Action::TerminalCloseAll => {
            terminals.close_all();
            current_view_clone(current_view)
        }
        Action::AgentRuntimeRequest { .. } => current_view_clone(current_view),
    }
}

fn emit_state(view: Option<&WorkbenchViewData>, is_loading: bool, error_message: &str) {
    let view_json = view
        .map(serde_json::to_string)
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_default();
    WorkbenchStateSignal {
        view_json,
        is_loading,
        error_message: error_message.to_string(),
    }
    .send_signal_to_dart();
}

fn emit_thread_history_state(
    entries: Option<Vec<UiChatEntry>>,
    is_loading: bool,
    error_message: &str,
) {
    let entries_json = entries
        .map(|entries| serde_json::to_string(&entries))
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_default();
    ThreadHistoryStateSignal {
        entries_json,
        is_loading,
        error_message: error_message.to_string(),
    }
    .send_signal_to_dart();
}

fn emit_bridge_task_result(request_id: String, task: &str, payload: serde_json::Value) {
    BridgeTaskResultSignal {
        request_id,
        task: task.to_string(),
        payload_json: serde_json::to_string(&payload).unwrap_or_else(|_| "null".to_string()),
        error_message: String::new(),
    }
    .send_signal_to_dart();
}

fn emit_bridge_task_error(request_id: String, task: &str, error: &anyhow::Error) {
    BridgeTaskResultSignal {
        request_id,
        task: task.to_string(),
        payload_json: String::new(),
        error_message: error.to_string(),
    }
    .send_signal_to_dart();
}

#[cfg(test)]
mod agent_runtime_typed_mapping_tests {
    use super::*;

    #[test]
    fn typed_request_maps_connect_and_discovery_without_json_packet() {
        let connect = typed_agent_runtime_request_packet(
            "connect-1",
            AgentRuntimeRequest::Connect {
                base_url: "127.0.0.1:8765".to_string(),
                selected_session_id: String::new(),
            },
        )
        .expect("typed connect maps");
        assert_eq!(connect.packet_id, "connect-1");
        assert_eq!(
            connect.intent,
            GuiTransportRequest::Connect {
                base_url: "127.0.0.1:8765".to_string(),
                selected_session_id: None,
            }
        );

        let refresh = typed_agent_runtime_request_packet(
            "discover-1",
            AgentRuntimeRequest::RefreshDiscovery {
                discovery_path: String::new(),
            },
        )
        .expect("typed discovery maps");
        assert_eq!(
            refresh.intent,
            GuiTransportRequest::RefreshDiscovery {
                discovery_path: None,
            }
        );

        let project = typed_agent_runtime_request_packet(
            "project-1",
            AgentRuntimeRequest::SelectProject {
                project_id: "runtime".to_string(),
            },
        )
        .expect("typed project selection maps");
        assert_eq!(
            project.intent,
            GuiTransportRequest::SelectProject {
                project_id: "runtime".to_string(),
            }
        );

        let stream = typed_agent_runtime_request_packet(
            "stream-1",
            AgentRuntimeRequest::ConsumeStreamOnce,
        )
        .expect("typed stream consumption maps");
        assert_eq!(stream.intent, GuiTransportRequest::ConsumeStreamOnce);
    }

    #[test]
    fn typed_request_maps_role_and_workflow_operations_without_json_envelope() {
        let role = typed_agent_runtime_request_packet(
            "role-activate-1",
            AgentRuntimeRequest::DispatchOperation {
                operation: AgentRuntimeGuiOperation::ActivateRoleVersion {
                    role_id: "runtime-allow".to_string(),
                    version_id: "role-version-1".to_string(),
                },
            },
        )
        .expect("typed role operation maps");
        assert_eq!(
            role.intent,
            GuiTransportRequest::DispatchOperation {
                operation: GuiOperationRequest::ActivateRoleVersion {
                    role_id: "runtime-allow".to_string(),
                    version_id: "role-version-1".to_string(),
                },
            }
        );

        let feedback = typed_agent_runtime_request_packet(
            "memory-feedback-1",
            AgentRuntimeRequest::DispatchOperation {
                operation: AgentRuntimeGuiOperation::WorkflowMemoryFeedback {
                    memory_id: "memory-1".to_string(),
                    session_id: "session-1".to_string(),
                    feedback: "helpful".to_string(),
                    payload: crate::signals::AgentRuntimeWorkflowMemoryFeedbackPayload {
                        source: "gui.workbench".to_string(),
                        reason: String::new(),
                        variant: false,
                        has_variant: false,
                    },
                },
            },
        )
        .expect("typed feedback maps");
        match feedback.intent {
            GuiTransportRequest::DispatchOperation {
                operation: GuiOperationRequest::WorkflowMemoryFeedback { memory_id, session_id, feedback, .. },
            } => {
                assert_eq!(memory_id, "memory-1");
                assert_eq!(session_id, "session-1");
                assert_eq!(feedback, "helpful");
            }
            other => panic!("unexpected mapped operation: {other:?}"),
        }
    }

    #[test]
    fn typed_request_rejects_empty_approval_reason() {
        let result = typed_agent_runtime_request_packet(
            "approval-empty-reason-1",
            AgentRuntimeRequest::DispatchOperation {
                operation: AgentRuntimeGuiOperation::DecideApproval {
                    approval_id: "approval-1".to_string(),
                    decision: "approved".to_string(),
                    reason: "   ".to_string(),
                },
            },
        );
        let error = result.expect_err("empty approval reason must be rejected before dispatch");
        assert_eq!(error.error.code, "invalid_request");
        assert!(error.error.message.contains("approval reason is required"));
    }

    #[test]
    fn typed_output_maps_error_and_workbench_view_without_generic_string() {
        let error = typed_agent_runtime_output(GuiTransportOutputPacket {
            request_id: "bad-1".to_string(),
            output: GuiTransportOutput::Error {
                error: ApiErrorPacket::new(
                    "bad_request",
                    "typed failure",
                    serde_json::json!({"field":"baseUrl"}),
                ),
            },
        });
        match error {
            AgentRuntimeOutput::Error { error } => {
                assert_eq!(error.code, "bad_request");
                assert_eq!(error.message, "typed failure");
            }
            other => panic!("unexpected typed output: {other:?}"),
        }

        let controller_state = robdex_agent_runtime_projection::GuiControllerState::default();
        let view_model = InternalWorkbenchViewModel::from_runtime_state(
            "http://127.0.0.1:8765",
            None,
            &controller_state,
            &[],
            0,
            None,
            &InternalDiscoveryView::default(),
            &InternalDiscoveryView::default(),
            &InternalDiscoveryView::default(),
            &[],
        );
        let view = typed_agent_runtime_output(GuiTransportOutputPacket {
            request_id: "view-1".to_string(),
            output: GuiTransportOutput::WorkbenchView { view_model },
        });
        match view {
            AgentRuntimeOutput::WorkbenchView { view_model } => {
                assert_eq!(view_model.base_url, "http://127.0.0.1:8765");
                assert_eq!(view_model.connection_state, "disconnected");
            }
            other => panic!("unexpected typed output: {other:?}"),
        }
    }

    fn delta_for_test(index: u32, is_final: bool) -> WorkbenchSelectedChatDeltaSignal {
        let replacement_text = if is_final {
            (0..1000).map(|token| format!("token-{token} ")).collect::<String>()
        } else {
            String::new()
        };
        WorkbenchSelectedChatDeltaSignal {
            thread_id: "thread-1".to_string(),
            message_id: "assistant-1".to_string(),
            appended_text: format!("token-{index} "),
            replacement_text,
            delivery_state: if is_final { "complete" } else { "streaming" }.to_string(),
            is_final,
            sequence: index as u64,
            metadata_json: "{}".to_string(),
            selected_entry_count: 50,
            coalesced_stream_update_count: 0,
            dropped_intermediate_stream_update_count: 0,
        }
    }

    #[test]
    fn robdex_streaming_coalescer_limits_burst_to_ten_non_final_plus_final() {
        let mut coalescer = StreamCoalescer::new(10);
        let mut emitted = Vec::new();
        for index in 0..1000 {
            match selected_chat_stream_emission(&mut coalescer, delta_for_test(index, false)) {
                SelectedChatStreamEmission::Delta(delta) => emitted.push(delta),
                SelectedChatStreamEmission::Dropped => {}
            }
        }
        match selected_chat_stream_emission(&mut coalescer, delta_for_test(1000, true)) {
            SelectedChatStreamEmission::Delta(delta) => emitted.push(delta),
            SelectedChatStreamEmission::Dropped => panic!("final delta must not be dropped"),
        }
        let mut assistant_text = String::new();
        for delta in &emitted {
            assert!(delta.selected_entry_count <= 50, "selected timeline exceeded cap after delta {}", delta.sequence);
            if !delta.replacement_text.is_empty() {
                assistant_text = delta.replacement_text.clone();
            } else {
                assistant_text.push_str(&delta.appended_text);
            }
        }
        let non_final = emitted.iter().filter(|delta| !delta.is_final).count();
        let final_count = emitted.iter().filter(|delta| delta.is_final).count();
        assert!(non_final <= 10, "non-final emissions exceeded budget: {non_final}");
        assert_eq!(final_count, 1);
        let complete = (0..1000).map(|token| format!("token-{token} ")).collect::<String>();
        assert_eq!(assistant_text, complete);
        assert!(emitted.last().is_some_and(|delta| delta.is_final && delta.replacement_text == complete));
        assert!(emitted.last().unwrap().dropped_intermediate_stream_update_count > 0);
    }

    #[test]
    fn robdex_streaming_diagnostics_increment_for_snapshot_delta_burst_and_final() {
        let mut diagnostics = StreamingDiagnostics::default();
        diagnostics.record_snapshot(2048, 75);
        assert_eq!(diagnostics.websocket_event_counts.get("snapshot"), Some(&1));
        assert_eq!(diagnostics.websocket_payload_bytes.get("snapshot"), Some(&2048));
        assert_eq!(diagnostics.native_signal_count, 1);
        assert_eq!(diagnostics.serialized_payload_bytes, 2048);
        assert_eq!(diagnostics.full_snapshot_decode_count, 1);
        assert_eq!(diagnostics.selected_timeline_entry_count, 50);

        diagnostics.record_delta(128, 50);
        diagnostics.dart_selected_chat_delta_apply_count += 1;
        assert_eq!(diagnostics.websocket_event_counts.get("selectedChatDelta"), Some(&1));
        assert_eq!(diagnostics.websocket_payload_bytes.get("selectedChatDelta"), Some(&128));
        assert_eq!(diagnostics.native_signal_count, 2);
        assert_eq!(diagnostics.serialized_payload_bytes, 2176);
        assert_eq!(diagnostics.dart_selected_chat_delta_apply_count, 1);

        let mut coalescer = StreamCoalescer::new(10);
        let mut emitted = 0;
        for index in 0..1000 {
            if let SelectedChatStreamEmission::Delta(delta) = selected_chat_stream_emission(&mut coalescer, delta_for_test(index, false)) {
                emitted += 1;
                diagnostics.coalesced_stream_update_count = delta.coalesced_stream_update_count as u64;
                diagnostics.dropped_intermediate_stream_update_count = delta.dropped_intermediate_stream_update_count as u64;
                diagnostics.record_delta(64, delta.selected_entry_count as usize);
                diagnostics.dart_selected_chat_delta_apply_count += 1;
                assert!(diagnostics.selected_timeline_entry_count <= 50);
            }
        }
        assert!(emitted <= 10);
        let final_delta = match selected_chat_stream_emission(&mut coalescer, delta_for_test(1000, true)) {
            SelectedChatStreamEmission::Delta(delta) => delta,
            SelectedChatStreamEmission::Dropped => panic!("final message delivery must emit"),
        };
        diagnostics.coalesced_stream_update_count = final_delta.coalesced_stream_update_count as u64;
        diagnostics.dropped_intermediate_stream_update_count = final_delta.dropped_intermediate_stream_update_count as u64;
        diagnostics.record_delta(4096, final_delta.selected_entry_count as usize);
        diagnostics.dart_selected_chat_delta_apply_count += 1;

        let signal = diagnostics.signal();
        assert!(signal.websocket_event_counts_json.contains("snapshot"));
        assert!(signal.websocket_event_counts_json.contains("selectedChatDelta"));
        assert!(signal.websocket_payload_bytes_json.contains("selectedChatDelta"));
        assert!(signal.native_signal_count >= 3);
        assert!(signal.serialized_payload_bytes > 2048);
        assert_eq!(signal.full_snapshot_decode_count, 1);
        assert_eq!(signal.selected_timeline_entry_count, 50);
        assert!(signal.coalesced_stream_update_count >= 10);
        assert!(signal.dropped_intermediate_stream_update_count > 0);
        assert!(signal.dart_selected_chat_delta_apply_count >= 2);
        let payload_values = diagnostics.websocket_payload_bytes.values().copied().collect::<Vec<_>>();
        let payload_event_total = diagnostics.websocket_event_counts.values().sum::<u64>().max(1);
        let average_payload_bytes = diagnostics.serialized_payload_bytes / payload_event_total;
        let max_payload_bytes = payload_values.into_iter().max().unwrap_or_default();
        println!(
            "streaming_diagnostics full_snapshot_count={} delta_count={} average_payload_bytes={} max_payload_bytes={} selected_chat_entry_count={} coalesced_updates={} dropped_updates={} native_signals={} serialized_payload_bytes={}",
            signal.full_snapshot_decode_count,
            signal.dart_selected_chat_delta_apply_count,
            average_payload_bytes,
            max_payload_bytes,
            signal.selected_timeline_entry_count,
            signal.coalesced_stream_update_count,
            signal.dropped_intermediate_stream_update_count,
            signal.native_signal_count,
            signal.serialized_payload_bytes,
        );
    }

    #[test]
    fn robdex_streaming_hot_path_drops_intermediate_without_snapshot_emission() {
        let mut coalescer = StreamCoalescer::new(1);
        let first = selected_chat_stream_emission(&mut coalescer, delta_for_test(0, false));
        assert!(matches!(first, SelectedChatStreamEmission::Delta(_)));
        let dropped = selected_chat_stream_emission(&mut coalescer, delta_for_test(1, false));
        assert!(matches!(dropped, SelectedChatStreamEmission::Dropped));
        assert_eq!(coalescer.coalesced, 1);
        assert_eq!(coalescer.dropped, 0);
        let dropped_again = selected_chat_stream_emission(&mut coalescer, delta_for_test(2, false));
        assert!(matches!(dropped_again, SelectedChatStreamEmission::Dropped));
        assert_eq!(coalescer.dropped, 1);
        let final_delta = selected_chat_stream_emission(&mut coalescer, delta_for_test(3, true));
        match final_delta {
            SelectedChatStreamEmission::Delta(delta) => {
                assert!(delta.is_final);
                assert!(delta.replacement_text.contains("token-999 "));
                assert_eq!(delta.coalesced_stream_update_count, 1);
                assert_eq!(delta.dropped_intermediate_stream_update_count, 2);
            }
            SelectedChatStreamEmission::Dropped => panic!("final delta must be emitted"),
        }
    }

}
