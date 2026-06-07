use std::sync::Arc;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use robdex_client_core::{bridge::BridgeEndpoint, LiveSessionEvent, LiveSessionHandle, WorkbenchClient, start_live_session};
use robdex_protocol::{UiChatEntry, WorkbenchViewData};
use rinf::{DartSignal, RustSignal};
use tokio::select;
use tokio::sync::mpsc;
use tokio_with_wasm::alias as tokio;

use crate::signals::{
    ArchiveThreadGroupSignal, ArchiveThreadSignal, BridgeTaskResultSignal, ClearProjectHookLogsSignal,
    CreateProjectSignal, CreateThreadGroupSignal, CreateThreadSignal, DecideApprovalSignal,
    DeleteProjectSignal, DeleteThreadGroupSignal,
    FetchThreadHistorySignal, InitializeWorkbenchSignal, MoveSelectedThreadToGroupSignal, ReloadWorkbenchSignal,
    LoadProjectHookLogsSignal, LoadRequirementComposablesSignal, LoadThreadStatsSignal,
    RenameThreadGroupSignal, RenameThreadSignal, SelectProjectSignal, SelectThreadSignal,
    SendThreadMessageSignal, SetProjectOrchestratorSignal, SetThreadRunningStateSignal,
    SetThreadRequirementsSignal, SpawnAgentSignal, TerminalCloseAllSignal, TerminalCloseSignal, TerminalInputSignal,
    TerminalEventSignal, TerminalOpenSignal, TerminalResizeSignal, TerminateCommandExecutionSignal,
    ThreadCompactSignal, UpdateGlobalSettingsSignal, UpdateProjectSignal, UpdateThreadSettingsSignal,
    UpdateWorkerMetadataSignal, InterruptThreadSignal, ThreadHistoryStateSignal, HookToastSignal,
    UploadImageBytesSignal,
    WarmHandoffSignal, WorkbenchStateSignal,
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
}

pub async fn run() {
    let (tx, mut rx) = mpsc::unbounded_channel::<Action>();
    spawn_receivers(tx.clone());

    let mut client: Option<WorkbenchClient> = None;
    let mut current_view: Option<WorkbenchViewData> = None;
    let mut live_session: Option<LiveSessionHandle> = None;
    let mut live_event_rx: Option<mpsc::UnboundedReceiver<LiveSessionEvent>> = None;
    let mut initialized = false;
    let mut terminals = TerminalRegistry::new();

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
                apply_optimistic_action(&mut current_view, &action);
                let show_loading = current_view.is_none();
                emit_state(current_view.as_ref(), show_loading, "");
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
                    }
                    Err(error) => {
                        emit_state(current_view.as_ref(), false, &error.to_string());
                    }
                }
            }
            maybe_live_event = recv_live_event(&mut live_event_rx), if initialized && live_event_rx.is_some() => {
                match maybe_live_event {
                    Some(LiveSessionEvent::View(next_view)) => {
                        current_view = Some(next_view);
                        if let (Some(client), Some(view)) = (client.as_mut(), current_view.as_ref()) {
                            client.sync_view(view);
                        }
                        emit_state(current_view.as_ref(), false, "");
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
    spawn_unit::<TerminalCloseAllSignal, _>(tx, || Action::TerminalCloseAll);
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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
