//! Experiment-local Rinf-shaped transport proof.
//!
//! This module intentionally does not depend on Rinf or Flutter. It models the
//! packet boundary a future `frontend/robdex_app/native/hub` integration can use
//! while keeping runtime state, reduction, and operation decisions inside Rust.

use robdex_agent_runtime_projection::{
    ApiErrorPacket, CommandRegistryRequestSummary, GuiConnectionState, GuiControllerState,
    GuiOperationRequest, GuiOperationResult, PendingApprovalSummary, RoleSummary, RuntimeProjection,
    SessionListItem, TimelineItem,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::sync::{mpsc, oneshot};

use crate::gui_backend::GuiBackendController;
use crate::gui_sync::SyncOutcome;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GuiTransportRequestPacket {
    pub packet_id: String,
    pub intent: GuiTransportRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum GuiTransportRequest {
    RefreshDiscovery {
        discovery_path: Option<String>,
    },
    ConnectDiscoveredRuntime {
        discovery_path: Option<String>,
        selected_session_id: Option<String>,
    },
    Connect {
        base_url: String,
        selected_session_id: Option<String>,
    },
    Hydrate {
        selected_session_id: Option<String>,
    },
    Rehydrate {
        selected_session_id: Option<String>,
    },
    DispatchOperation {
        operation: GuiOperationRequest,
    },
    PollStreamOnce,
    Disconnect,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GuiTransportOutputPacket {
    pub request_id: String,
    pub output: GuiTransportOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum GuiTransportOutput {
    ProjectionSnapshot {
        projection: Value,
    },
    ControllerState {
        controller_state: Value,
    },
    OperationResult {
        result: GuiOperationResult,
    },
    StreamOutcome {
        outcome: GuiStreamOutcomePacket,
        projection: Option<Value>,
        controller_state: Value,
    },
    Error {
        error: ApiErrorPacket,
    },
    ControlTowerView {
        view_model: AgentRuntimeControlTowerViewModel,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControlTowerViewModel {
    pub discovery: AgentRuntimeDiscoveryView,
    pub connection_state: String,
    pub connection_tone: String,
    pub base_url: String,
    pub status_label: String,
    pub watermark_label: String,
    pub status_badges: Vec<AgentRuntimeControlTowerBadge>,
    pub selected_session_label: String,
    pub sessions_title: String,
    pub sessions_subtitle: String,
    pub timeline_title: String,
    pub timeline_subtitle: String,
    pub actions_title: String,
    pub actions_subtitle: String,
    pub detail_title: String,
    pub detail_subtitle: String,
    pub sessions_empty_title: String,
    pub sessions_empty_text: String,
    pub timeline_empty_title: String,
    pub timeline_empty_text: String,
    pub actions_empty_title: String,
    pub actions_empty_text: String,
    pub sessions: Vec<AgentRuntimeControlTowerSessionRow>,
    pub timeline: Vec<AgentRuntimeControlTowerTimelineRow>,
    pub actions: Vec<AgentRuntimeControlTowerActionRow>,
    pub role_admin: AgentRuntimeRoleAdminView,
    pub controller_facts: Vec<AgentRuntimeControlTowerFact>,
    pub output_log: Vec<String>,
    pub pending_request_count: usize,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeDiscoveryView {
    pub state: String,
    pub tone: String,
    pub title: String,
    pub message: String,
    pub base_url: Option<String>,
    pub health_url: Option<String>,
    pub web_socket_url: Option<String>,
    pub runtime_identity: Option<String>,
    pub discovery_path: String,
    pub service_state: Option<String>,
    pub connectable: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControlTowerSessionRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub subtitle: String,
    pub group_label: String,
    pub tone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControlTowerTimelineRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub status: String,
    pub tone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControlTowerActionRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub kind: String,
    pub state_text: String,
    pub tone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControlTowerFact {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControlTowerBadge {
    pub label: String,
    pub value: String,
    pub tone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleAdminView {
    pub title: String,
    pub subtitle: String,
    pub empty_title: String,
    pub empty_text: String,
    pub rows: Vec<AgentRuntimeRoleRow>,
    pub selected_detail: Option<AgentRuntimeRoleDetail>,
    pub version_rows: Vec<AgentRuntimeRoleVersionRow>,
    pub editor_draft: Option<AgentRuntimeRoleEditorDraftView>,
    pub validation_errors: Vec<String>,
    pub action_states: Vec<AgentRuntimeControlTowerActionRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub status: String,
    pub tone: String,
    pub current_version_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleDetail {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub model: String,
    pub status: String,
    pub instruction_text: String,
    pub capabilities: Vec<String>,
    pub policy: Vec<AgentRuntimeRolePolicyRow>,
    pub routing: Vec<AgentRuntimeControlTowerFact>,
    pub visibility: Vec<AgentRuntimeControlTowerFact>,
    pub lifecycle_authority: Vec<AgentRuntimeControlTowerFact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRolePolicyRow {
    pub action: String,
    pub decision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleVersionRow {
    pub version_id: String,
    pub version: String,
    pub status: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeRoleEditorDraftView {
    pub role_id: String,
    pub version: String,
    pub display_name: String,
    pub model: String,
    pub reasoning_effort: String,
    pub instruction_text: String,
    pub capabilities: Vec<String>,
    pub policy: Vec<AgentRuntimeRolePolicyRow>,
    pub routing_mode: String,
    pub routing_reserved_actions: Vec<String>,
    pub default_recipient: Option<String>,
    pub allowed_recipients: Vec<String>,
    pub listed: bool,
    pub owner_visible: bool,
    pub can_spawn_agents: bool,
    pub can_archive_agents: bool,
    pub lifecycle_reserved_actions: Vec<String>,
}

impl AgentRuntimeControlTowerViewModel {
    pub fn from_runtime_state(
        base_url: impl Into<String>,
        projection: Option<&RuntimeProjection>,
        controller_state: &GuiControllerState,
        output_log: &[String],
        pending_request_count: usize,
        error_message: Option<String>,
        discovery: &AgentRuntimeDiscoveryView,
    ) -> Self {
        let base_url = base_url.into();
        let sessions = projection
            .map(|projection| projection.sessions.iter().map(session_row).collect())
            .unwrap_or_default();
        let timeline = projection
            .map(|projection| projection.timeline.iter().map(timeline_row).collect())
            .unwrap_or_default();
        let mut actions: Vec<AgentRuntimeControlTowerActionRow> = projection
            .map(|projection| {
                projection
                    .pending_approvals
                    .iter()
                    .map(approval_action_row)
                    .chain(projection.command_registry_requests.iter().map(command_request_action_row))
                    .collect()
            })
            .unwrap_or_default();
        actions.sort_by(|left, right| left.kind.cmp(&right.kind).then(left.id.cmp(&right.id)));
        let selected_session_label = selected_session_label(projection, controller_state);
        Self {
            discovery: discovery.clone(),
            connection_state: connection_state_label(&controller_state.connection_state).to_string(),
            connection_tone: connection_tone(&controller_state.connection_state).to_string(),
            base_url,
            status_label: status_label(projection),
            watermark_label: projection
                .map(|projection| projection.watermark.to_string())
                .unwrap_or_else(|| "—".to_string()),
            status_badges: status_badges(projection, controller_state, pending_request_count),
            selected_session_label: selected_session_label.clone(),
            sessions_title: projection
                .map(|projection| format!("Sessions ({})", projection.sessions.len()))
                .unwrap_or_else(|| "Sessions".to_string()),
            sessions_subtitle: "Grouped by Rust-owned operational state".to_string(),
            timeline_title: format!("Selected session stream · {selected_session_label}"),
            timeline_subtitle: "Operations event stream, not a chat transcript".to_string(),
            actions_title: format!("Action queue ({})", actions.len()),
            actions_subtitle: "Approvals, resumable work, and registry requests".to_string(),
            detail_title: "Controller detail".to_string(),
            detail_subtitle: "Rust-owned controller facts".to_string(),
            sessions_empty_title: "No sessions".to_string(),
            sessions_empty_text: "No sessions are visible in the current runtime projection.".to_string(),
            timeline_empty_title: selected_session_label,
            timeline_empty_text: "Select a session to hydrate its operations event stream.".to_string(),
            actions_empty_title: "No action required".to_string(),
            actions_empty_text: "No approvals, resumable actions, or registry requests need attention.".to_string(),
            sessions,
            timeline,
            actions,
            role_admin: role_admin_view(projection),
            controller_facts: controller_facts(controller_state),
            output_log: output_log.iter().take(8).cloned().collect(),
            pending_request_count,
            error_message,
        }
    }
}

impl Default for AgentRuntimeDiscoveryView {
    fn default() -> Self {
        let discovery_path = default_discovery_path().display().to_string();
        AgentRuntimeDiscoveryView {
            state: "notLoaded".to_string(),
            tone: "muted".to_string(),
            title: "Discovery not loaded".to_string(),
            message: "Refresh discovery to inspect the local Agent Runtime service packet.".to_string(),
            base_url: None,
            health_url: None,
            web_socket_url: None,
            runtime_identity: None,
            discovery_path,
            service_state: None,
            connectable: false,
            diagnostics: Vec::new(),
        }
    }
}

fn default_discovery_path() -> PathBuf {
    default_service_state_dir().join("discovery.json")
}

fn default_service_state_dir() -> PathBuf {
    if let Ok(state_dir) = std::env::var("ROBDEX_AGENT_RUNTIME_SERVICE_STATE_DIR") {
        return PathBuf::from(state_dir);
    }
    let home = std::env::var("HOME").ok();
    let xdg_state_home = std::env::var("XDG_STATE_HOME").ok();
    canonical_service_state_dir(home.as_deref(), xdg_state_home.as_deref(), std::env::consts::OS)
}

fn canonical_service_state_dir(home: Option<&str>, xdg_state_home: Option<&str>, os: &str) -> PathBuf {
    if os == "macos" {
        return home
            .map(|home| PathBuf::from(home).join("Library/Application Support/Robdex Agent Runtime/service"))
            .unwrap_or_else(|| PathBuf::from("Library/Application Support/Robdex Agent Runtime/service"));
    }
    xdg_state_home
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from(".local/state"))
        .join("robdex-agent-runtime/service")
}

fn read_discovery_file(path: &Path) -> AgentRuntimeDiscoveryView {
    let display = path.display().to_string();
    match std::fs::read_to_string(path) {
        Ok(contents) => match serde_json::from_str::<Value>(&contents) {
            Ok(packet) => classify_discovery_packet(Some(&packet), &display),
            Err(error) => AgentRuntimeDiscoveryView {
                state: "parseError".to_string(),
                tone: "danger".to_string(),
                title: "Discovery packet cannot be parsed".to_string(),
                message: "The local service discovery file is malformed JSON.".to_string(),
                base_url: None,
                health_url: None,
                web_socket_url: None,
                runtime_identity: None,
                discovery_path: display,
                service_state: None,
                connectable: false,
                diagnostics: vec![error.to_string()],
            },
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => AgentRuntimeDiscoveryView {
            state: "noDiscoveryFile".to_string(),
            tone: "muted".to_string(),
            title: "No local service discovery file".to_string(),
            message: "Start or refresh the local Agent Runtime service to create discovery.json.".to_string(),
            base_url: None,
            health_url: None,
            web_socket_url: None,
            runtime_identity: None,
            discovery_path: display,
            service_state: None,
            connectable: false,
            diagnostics: vec!["discovery file is missing".to_string()],
        },
        Err(error) => AgentRuntimeDiscoveryView {
            state: "unavailable".to_string(),
            tone: "danger".to_string(),
            title: "Discovery file cannot be read".to_string(),
            message: "Rust could not read the local service discovery file.".to_string(),
            base_url: None,
            health_url: None,
            web_socket_url: None,
            runtime_identity: None,
            discovery_path: display,
            service_state: None,
            connectable: false,
            diagnostics: vec![error.to_string()],
        },
    }
}

fn classify_discovery_packet(packet: Option<&Value>, discovery_path: &str) -> AgentRuntimeDiscoveryView {
    let Some(packet) = packet else {
        let mut view = AgentRuntimeDiscoveryView::default();
        view.discovery_path = discovery_path.to_string();
        return view;
    };
    let service_state = packet.get("serviceState").and_then(Value::as_str).map(str::to_string);
    let flags = packet.get("stateFlags").unwrap_or(&Value::Null);
    let health = packet.get("healthResult").unwrap_or(&Value::Null);
    let base_url = packet.get("baseUrl").and_then(Value::as_str).map(str::to_string);
    let health_url = packet.get("healthUrl").and_then(Value::as_str).map(str::to_string);
    let web_socket_url = packet.get("webSocketUrl").and_then(Value::as_str).map(str::to_string);
    let runtime_identity = packet.get("runtimeIdentity").and_then(Value::as_str).map(str::to_string);
    let flag = |name: &str| flags.get(name).and_then(Value::as_bool).unwrap_or(false);
    let health_ok = health.get("ok").and_then(Value::as_bool);
    let diagnostics = packet
        .get("diagnostics")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let code = item.get("code").and_then(Value::as_str)?;
                    let message = item.get("message").and_then(Value::as_str).unwrap_or("");
                    Some(if message.is_empty() { code.to_string() } else { format!("{code}: {message}") })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let (state, tone, title, message) = if flag("stalePid") || service_state.as_deref() == Some("stalePid") {
        ("stalePid", "danger", "Stale service pid", "The pid file exists but the process is not alive.")
    } else if flag("unhealthy") || service_state.as_deref() == Some("unhealthy") {
        ("unhealthy", "danger", "Local runtime is unhealthy", "The service process exists but /health is failing.")
    } else if flag("missingConfig") || service_state.as_deref() == Some("missingConfig") {
        ("missingConfig", "warning", "Service config is missing", "The local wrapper has not written an effective configuration snapshot.")
    } else if flag("staleDiscovery") {
        ("staleDiscovery", "warning", "Discovery needs refresh", "The discovery packet is older than service metadata.")
    } else if flag("running") || service_state.as_deref() == Some("running") {
        if health_ok == Some(true) {
            ("runningHealthy", "success", "Local runtime ready", "A healthy local Agent Runtime service was discovered.")
        } else {
            ("unhealthy", "danger", "Local runtime is unhealthy", "The service is running but health is not confirmed.")
        }
    } else if flag("stopped") || service_state.as_deref() == Some("stopped") {
        ("stopped", "muted", "Local runtime stopped", "The local Agent Runtime service is stopped.")
    } else {
        ("unknown", "muted", "Discovery state unknown", "The discovery packet did not match a known local service state.")
    };
    let connectable = state == "runningHealthy" && base_url.is_some();
    AgentRuntimeDiscoveryView {
        state: state.to_string(),
        tone: tone.to_string(),
        title: title.to_string(),
        message: message.to_string(),
        base_url,
        health_url,
        web_socket_url,
        runtime_identity,
        discovery_path: discovery_path.to_string(),
        service_state,
        connectable,
        diagnostics,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum GuiStreamOutcomePacket {
    Hello {
        watermark: i64,
        runtime_identity: Option<String>,
    },
    DeltaApplied {
        delta: Value,
        apply_outcome: String,
    },
    ResyncRequired {
        reason: Option<String>,
    },
    ServerShutdown,
    StreamClosed,
}

#[derive(Clone)]
pub struct GuiTransportHandle {
    sender: mpsc::Sender<TransportAction>,
}

struct TransportAction {
    packet: GuiTransportRequestPacket,
    reply: oneshot::Sender<Vec<GuiTransportOutputPacket>>,
}

impl GuiTransportHandle {
    pub fn spawn() -> Self {
        let (sender, mut receiver) = mpsc::channel::<TransportAction>(32);
        tokio::spawn(async move {
            let mut runner = GuiTransportRunner::new();
            while let Some(action) = receiver.recv().await {
                let outputs = runner.handle_packet(action.packet).await;
                let _ = action.reply.send(outputs);
            }
        });
        Self { sender }
    }

    pub async fn send(&self, packet: GuiTransportRequestPacket) -> Vec<GuiTransportOutputPacket> {
        let request_id = packet.packet_id.clone();
        let (reply, receiver) = oneshot::channel();
        if self.sender.send(TransportAction { packet, reply }).await.is_err() {
            return vec![error_output(
                request_id,
                ApiErrorPacket::new(
                    "unavailable",
                    "experimental GUI transport runner is unavailable",
                    json!({"source":"transportActionLoop"}),
                ),
            )];
        }
        receiver.await.unwrap_or_else(|_| {
            vec![error_output(
                request_id,
                ApiErrorPacket::new(
                    "unavailable",
                    "experimental GUI transport runner stopped before replying",
                    json!({"source":"transportActionLoop"}),
                ),
            )]
        })
    }
}

struct GuiTransportRunner {
    controller: GuiBackendController,
    base_url: String,
    output_log: Vec<String>,
    discovery: AgentRuntimeDiscoveryView,
}

impl GuiTransportRunner {
    fn new() -> Self {
        Self {
            controller: GuiBackendController::new(),
            base_url: "http://127.0.0.1:8765".to_string(),
            output_log: Vec::new(),
            discovery: AgentRuntimeDiscoveryView::default(),
        }
    }

    async fn handle_packet(&mut self, packet: GuiTransportRequestPacket) -> Vec<GuiTransportOutputPacket> {
        let request_id = packet.packet_id;
        match self.handle_intent(packet.intent).await {
            Ok(mut outputs) => {
                for output in &mut outputs {
                    output.request_id = request_id.clone();
                }
                self.record_outputs(&outputs);
                outputs.push(self.control_tower_view_output(request_id, None));
                outputs
            }
            Err(error) => {
                let mut outputs = vec![error_output(request_id.clone(), error.clone())];
                self.record_outputs(&outputs);
                outputs.push(self.control_tower_view_output(request_id, Some(&error)));
                outputs
            }
        }
    }

    async fn handle_intent(&mut self, intent: GuiTransportRequest) -> Result<Vec<GuiTransportOutputPacket>, ApiErrorPacket> {
        match intent {
            GuiTransportRequest::RefreshDiscovery { discovery_path } => {
                self.refresh_discovery(discovery_path);
                Ok(vec![])
            }
            GuiTransportRequest::ConnectDiscoveredRuntime {
                discovery_path,
                selected_session_id,
            } => {
                self.refresh_discovery(discovery_path);
                if !self.discovery.connectable {
                    return Err(ApiErrorPacket::new(
                        "conflict",
                        "local discovery target is not connectable",
                        json!({"discoveryState": self.discovery.state, "discoveryPath": self.discovery.discovery_path}),
                    ));
                }
                let base_url = self.discovery.base_url.clone().ok_or_else(|| {
                    ApiErrorPacket::new(
                        "validation_failed",
                        "connectable discovery target is missing base URL",
                        json!({"discoveryState": self.discovery.state}),
                    )
                })?;
                self.base_url = base_url.clone();
                let result = self
                    .controller
                    .dispatch(GuiOperationRequest::Connect {
                        base_url,
                        selected_session_id,
                    })
                    .await;
                Ok(self.operation_outputs(result))
            }
            GuiTransportRequest::Connect {
                base_url,
                selected_session_id,
            } => {
                self.base_url = base_url.clone();
                let result = self
                    .controller
                    .dispatch(GuiOperationRequest::Connect {
                        base_url,
                        selected_session_id,
                    })
                    .await;
                Ok(self.operation_outputs(result))
            }
            GuiTransportRequest::Hydrate { selected_session_id } => {
                let result = self
                    .controller
                    .dispatch(GuiOperationRequest::Hydrate { selected_session_id })
                    .await;
                Ok(self.operation_outputs(result))
            }
            GuiTransportRequest::Rehydrate { selected_session_id } => {
                let result = self
                    .controller
                    .dispatch(GuiOperationRequest::Rehydrate { selected_session_id })
                    .await;
                Ok(self.operation_outputs(result))
            }
            GuiTransportRequest::DispatchOperation { operation } => {
                let result = self.controller.dispatch(operation).await;
                Ok(self.operation_outputs(result))
            }
            GuiTransportRequest::PollStreamOnce => {
                let outcome = self.controller.next_stream_outcome().await?;
                Ok(vec![GuiTransportOutputPacket {
                    request_id: String::new(),
                    output: GuiTransportOutput::StreamOutcome {
                        outcome: stream_outcome_packet(outcome)?,
                        projection: optional_json(self.controller.projection())?,
                        controller_state: to_json(self.controller.controller_state())?,
                    },
                }])
            }
            GuiTransportRequest::Disconnect => {
                let result = self.controller.dispatch(GuiOperationRequest::Disconnect).await;
                Ok(self.operation_outputs(result))
            }
        }
    }

    fn operation_outputs(&self, result: GuiOperationResult) -> Vec<GuiTransportOutputPacket> {
        let mut outputs = vec![GuiTransportOutputPacket {
            request_id: String::new(),
            output: GuiTransportOutput::OperationResult { result },
        }];
        if let Ok(Some(projection)) = optional_json(self.controller.projection()) {
            outputs.push(GuiTransportOutputPacket {
                request_id: String::new(),
                output: GuiTransportOutput::ProjectionSnapshot { projection },
            });
        }
        if let Ok(controller_state) = to_json(self.controller.controller_state()) {
            outputs.push(GuiTransportOutputPacket {
                request_id: String::new(),
                output: GuiTransportOutput::ControllerState { controller_state },
            });
        }
        outputs
    }

    fn control_tower_view_output(&self, request_id: String, error: Option<&ApiErrorPacket>) -> GuiTransportOutputPacket {
        GuiTransportOutputPacket {
            request_id,
            output: GuiTransportOutput::ControlTowerView {
                view_model: AgentRuntimeControlTowerViewModel::from_runtime_state(
                    self.base_url.clone(),
                    self.controller.projection(),
                    self.controller.controller_state(),
                    &self.output_log,
                    0,
                    error.map(|error| format!("{}: {}", error.error.code, error.error.message)),
                    &self.discovery,
                ),
            },
        }
    }

    fn refresh_discovery(&mut self, discovery_path: Option<String>) {
        let path = discovery_path.map(PathBuf::from).unwrap_or_else(default_discovery_path);
        self.discovery = read_discovery_file(&path);
    }

    fn record_outputs(&mut self, outputs: &[GuiTransportOutputPacket]) {
        for output in outputs {
            self.output_log.insert(0, format!("{} · {}", output_type(&output.output), output.request_id));
        }
        self.output_log.truncate(8);
    }
}

fn output_type(output: &GuiTransportOutput) -> &'static str {
    match output {
        GuiTransportOutput::ProjectionSnapshot { .. } => "projectionSnapshot",
        GuiTransportOutput::ControllerState { .. } => "controllerState",
        GuiTransportOutput::OperationResult { .. } => "operationResult",
        GuiTransportOutput::StreamOutcome { .. } => "streamOutcome",
        GuiTransportOutput::Error { .. } => "error",
        GuiTransportOutput::ControlTowerView { .. } => "controlTowerView",
    }
}

fn session_row(session: &SessionListItem) -> AgentRuntimeControlTowerSessionRow {
    let title = session
        .title
        .as_ref()
        .or(session.name.as_ref())
        .cloned()
        .unwrap_or_else(|| session.id.clone());
    let role = session
        .role_id
        .as_ref()
        .or(session.role_version.as_ref())
        .cloned()
        .unwrap_or_else(|| "runtime role".to_string());
    let project = session.project_key.as_deref().unwrap_or("no project");
    AgentRuntimeControlTowerSessionRow {
        id: session.id.clone(),
        title,
        status: session.status.clone(),
        subtitle: format!("{role} · {project} · {}", session.workdir),
        group_label: session_group_label(session),
        tone: status_tone(&session.status).to_string(),
    }
}

fn timeline_row(item: &TimelineItem) -> AgentRuntimeControlTowerTimelineRow {
    AgentRuntimeControlTowerTimelineRow {
        id: item.id.clone(),
        title: item.event_type.clone(),
        subtitle: item
            .summary
            .as_ref()
            .or(item.entity_id.as_ref())
            .cloned()
            .unwrap_or_else(|| item.entity_type.clone()),
        status: item
            .status
            .clone()
            .unwrap_or_else(|| format!("#{}", item.sequence)),
        tone: timeline_tone(item).to_string(),
    }
}

fn approval_action_row(approval: &PendingApprovalSummary) -> AgentRuntimeControlTowerActionRow {
    AgentRuntimeControlTowerActionRow {
        id: approval.id.clone(),
        title: approval.action_name.clone(),
        subtitle: format!(
            "{} · canDecide={} · canResume={}",
            approval.status, approval.can_decide, approval.can_resume
        ),
        kind: "approval".to_string(),
        state_text: approval_state_text(approval),
        tone: approval_tone(approval).to_string(),
    }
}

fn command_request_action_row(request: &CommandRegistryRequestSummary) -> AgentRuntimeControlTowerActionRow {
    AgentRuntimeControlTowerActionRow {
        id: request.id.clone(),
        title: request.action_label.clone(),
        subtitle: format!(
            "{} · {} · {}",
            request.operation,
            request
                .scope_summary
                .as_deref()
                .unwrap_or("scope pending"),
            request
                .policy_summary
                .as_deref()
                .unwrap_or("policy pending")
        ),
        kind: "commandRegistryRequest".to_string(),
        state_text: request.state_text.clone(),
        tone: if request.can_apply {
            "success"
        } else if request.can_decide || request.can_preview {
            "warning"
        } else {
            "info"
        }
        .to_string(),
    }
}

fn role_admin_view(projection: Option<&RuntimeProjection>) -> AgentRuntimeRoleAdminView {
    let roles: Vec<RoleSummary> = projection.map(|projection| projection.roles.clone()).unwrap_or_default();
    let mut rows: Vec<_> = roles.iter().map(role_row).collect();
    rows.sort_by(|left, right| left.id.cmp(&right.id));
    let selected = roles
        .iter()
        .find(|role| role.status != "archived")
        .or_else(|| roles.first());
    let selected_detail = selected.map(role_detail);
    let editor_draft = selected.map(role_editor_draft);
    let version_rows = selected.map(role_version_rows).unwrap_or_default();
    let validation_errors = selected
        .filter(|role| role.capabilities.len() != role.policy.len())
        .map(|_| vec!["capabilities must exactly match policy keys".to_string()])
        .unwrap_or_default();
    let action_states = selected
        .map(|role| role_operation_actions(role, validation_errors.is_empty()))
        .unwrap_or_default();
    AgentRuntimeRoleAdminView {
        title: format!("Role Admin ({})", rows.len()),
        subtitle: "DB-backed immutable role versions".to_string(),
        empty_title: "No roles projected".to_string(),
        empty_text: "Hydrate the runtime projection or create a role draft through Rust-owned role operations.".to_string(),
        rows,
        selected_detail,
        version_rows,
        editor_draft,
        validation_errors,
        action_states,
    }
}

fn role_row(role: &RoleSummary) -> AgentRuntimeRoleRow {
    AgentRuntimeRoleRow {
        id: role.id.clone(),
        title: role.display_name.clone(),
        subtitle: format!(
            "{} · {}",
            role.version.as_deref().unwrap_or("version unknown"),
            role.model.as_deref().unwrap_or("model unknown")
        ),
        status: role.status.clone(),
        tone: status_tone(&role.status).to_string(),
        current_version_id: role.current_version_id.clone(),
    }
}

fn role_detail(role: &RoleSummary) -> AgentRuntimeRoleDetail {
    AgentRuntimeRoleDetail {
        id: role.id.clone(),
        display_name: role.display_name.clone(),
        version: role.version.clone().unwrap_or_else(|| "unknown".to_string()),
        model: role.model.clone().unwrap_or_else(|| "model unknown".to_string()),
        status: role.status.clone(),
        instruction_text: role.instruction_text.clone().unwrap_or_default(),
        capabilities: role.capabilities.clone(),
        policy: role
            .policy
            .iter()
            .map(|(action, decision)| AgentRuntimeRolePolicyRow {
                action: action.clone(),
                decision: decision.clone(),
            })
            .collect(),
        routing: json_object_facts(&role.routing),
        visibility: json_object_facts(&role.visibility),
        lifecycle_authority: json_object_facts(&role.lifecycle_authority),
    }
}

fn role_editor_draft(role: &RoleSummary) -> AgentRuntimeRoleEditorDraftView {
    AgentRuntimeRoleEditorDraftView {
        role_id: role.id.clone(),
        version: role.version.clone().unwrap_or_else(|| "1.0.0".to_string()),
        display_name: role.display_name.clone(),
        model: role.model.clone().unwrap_or_default(),
        reasoning_effort: role.reasoning_effort.clone().unwrap_or_else(|| "medium".to_string()),
        instruction_text: role.instruction_text.clone().unwrap_or_default(),
        capabilities: role.capabilities.clone(),
        policy: role.policy.iter().map(|(action, decision)| AgentRuntimeRolePolicyRow { action: action.clone(), decision: decision.clone() }).collect(),
        routing_mode: role.routing.get("mode").and_then(Value::as_str).unwrap_or("direct").to_string(),
        routing_reserved_actions: role
            .routing
            .get("reservedActions")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default(),
        default_recipient: role.routing.get("defaultRecipient").and_then(Value::as_str).map(str::to_string),
        allowed_recipients: role
            .routing
            .get("allowedRecipients")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default(),
        listed: role.visibility.get("listed").and_then(Value::as_bool).unwrap_or(false),
        owner_visible: role.visibility.get("ownerVisible").and_then(Value::as_bool).unwrap_or(false),
        can_spawn_agents: role.lifecycle_authority.get("canSpawnAgents").and_then(Value::as_bool).unwrap_or(false),
        can_archive_agents: role.lifecycle_authority.get("canArchiveAgents").and_then(Value::as_bool).unwrap_or(false),
        lifecycle_reserved_actions: role
            .lifecycle_authority
            .get("reservedActions")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default(),
    }
}

fn role_version_rows(role: &RoleSummary) -> Vec<AgentRuntimeRoleVersionRow> {
    role
        .versions
        .iter()
        .map(|version| AgentRuntimeRoleVersionRow {
            version_id: version.version_id.clone(),
            version: version.version.clone(),
            status: version.status.clone(),
            created_at: version.created_at.clone(),
        })
        .collect()
}

fn role_operation_actions(role: &RoleSummary, valid: bool) -> Vec<AgentRuntimeControlTowerActionRow> {
    let mut actions = vec![
        AgentRuntimeControlTowerActionRow {
            id: format!("role:{}:validate", role.id),
            title: "Validate draft".to_string(),
            subtitle: "Runs canonical role manifest, routing, and command-policy validation".to_string(),
            kind: "roleAdmin".to_string(),
            state_text: if valid { "Ready".to_string() } else { "Fix validation errors".to_string() },
            tone: if valid { "success" } else { "danger" }.to_string(),
        },
        AgentRuntimeControlTowerActionRow {
            id: format!("role:{}:export", role.id),
            title: "Export current role".to_string(),
            subtitle: "Returns the DB-backed current manifest plus inline instructions".to_string(),
            kind: "roleAdmin".to_string(),
            state_text: "Direct result".to_string(),
            tone: "info".to_string(),
        },
    ];
    actions.push(AgentRuntimeControlTowerActionRow {
        id: format!("role:{}:archive", role.id),
        title: if role.status == "archived" { "Unarchive role" } else { "Archive role" }.to_string(),
        subtitle: "Mutation waits for projection evidence".to_string(),
        kind: "roleAdmin".to_string(),
        state_text: if role.status == "archived" { "Can unarchive" } else { "Can archive" }.to_string(),
        tone: if role.status == "archived" { "warning" } else { "muted" }.to_string(),
    });
    actions
}

fn json_object_facts(value: &Value) -> Vec<AgentRuntimeControlTowerFact> {
    value
        .as_object()
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| AgentRuntimeControlTowerFact {
                    label: key.clone(),
                    value: value
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| value.to_string()),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn session_group_label(session: &SessionListItem) -> String {
    if !session.tracked {
        "Archived".to_string()
    } else if session.status == "open" {
        "Open".to_string()
    } else if session.status == "closed" {
        "Closed".to_string()
    } else {
        "Attention".to_string()
    }
}

fn status_tone(status: &str) -> &'static str {
    match status {
        "open" | "streaming" | "connected" | "ok" | "completed" => "success",
        "pending" | "connecting" | "hydrating" | "reconnecting" => "warning",
        "failed" | "error" | "lost" | "blocked" => "danger",
        "closed" | "disabled" | "archived" => "muted",
        _ => "info",
    }
}

fn timeline_tone(item: &TimelineItem) -> &'static str {
    if let Some(status) = &item.status {
        return status_tone(status);
    }
    if item.event_type.contains("error") || item.event_type.contains("failed") {
        "danger"
    } else if item.event_type.contains("approval") {
        "warning"
    } else if item.event_type.contains("completed") {
        "success"
    } else {
        "info"
    }
}

fn approval_state_text(approval: &PendingApprovalSummary) -> String {
    if approval.can_resume {
        "Ready to resume".to_string()
    } else if approval.can_decide {
        "Needs decision".to_string()
    } else {
        format!("Approval {}", approval.status)
    }
}

fn approval_tone(approval: &PendingApprovalSummary) -> &'static str {
    if approval.can_resume {
        "success"
    } else if approval.can_decide {
        "warning"
    } else {
        status_tone(&approval.status)
    }
}

fn selected_session_label(projection: Option<&RuntimeProjection>, controller_state: &GuiControllerState) -> String {
    let selected_id = controller_state.selected_session_id.as_deref();
    projection
        .and_then(|projection| {
            selected_id.and_then(|id| {
                projection
                    .sessions
                    .iter()
                    .find(|session| session.id == id)
                    .map(|session| session.title.as_deref().or(session.name.as_deref()).unwrap_or(&session.id).to_string())
            })
        })
        .or_else(|| selected_id.map(str::to_string))
        .unwrap_or_else(|| "none selected".to_string())
}

fn status_badges(
    projection: Option<&RuntimeProjection>,
    controller_state: &GuiControllerState,
    _pending_request_count: usize,
) -> Vec<AgentRuntimeControlTowerBadge> {
    let mut badges = vec![
        AgentRuntimeControlTowerBadge {
            label: "Connection".to_string(),
            value: connection_state_label(&controller_state.connection_state).to_string(),
            tone: status_tone(connection_state_label(&controller_state.connection_state)).to_string(),
        },
    ];
    if let Some(projection) = projection {
        badges.push(AgentRuntimeControlTowerBadge {
            label: "Sessions".to_string(),
            value: projection.sessions.len().to_string(),
            tone: if projection.sessions.is_empty() { "muted" } else { "info" }.to_string(),
        });
        badges.push(AgentRuntimeControlTowerBadge {
            label: "Attention".to_string(),
            value: (projection.pending_approvals.len() + projection.command_registry_requests.len()).to_string(),
            tone: if projection.pending_approvals.is_empty() && projection.command_registry_requests.is_empty() { "muted" } else { "warning" }.to_string(),
        });
        badges.push(AgentRuntimeControlTowerBadge {
            label: "Registry requests".to_string(),
            value: projection.command_registry_requests.len().to_string(),
            tone: if projection.command_registry_requests.is_empty() { "muted" } else { "warning" }.to_string(),
        });
        badges.push(AgentRuntimeControlTowerBadge {
            label: "Command inventory".to_string(),
            value: projection.command_registry.len().to_string(),
            tone: if projection.command_registry.is_empty() { "muted" } else { "info" }.to_string(),
        });
        badges.push(AgentRuntimeControlTowerBadge {
            label: "Timeline".to_string(),
            value: projection.timeline.len().to_string(),
            tone: if projection.timeline.is_empty() { "muted" } else { "info" }.to_string(),
        });
    }
    badges
}

fn controller_facts(controller_state: &GuiControllerState) -> Vec<AgentRuntimeControlTowerFact> {
    vec![
        AgentRuntimeControlTowerFact {
            label: "Controller".to_string(),
            value: connection_state_label(&controller_state.connection_state).to_string(),
        },
        AgentRuntimeControlTowerFact {
            label: "Selected session".to_string(),
            value: controller_state
                .selected_session_id
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        },
        AgentRuntimeControlTowerFact {
            label: "Pending rehydrate".to_string(),
            value: controller_state.pending_rehydrate.to_string(),
        },
        AgentRuntimeControlTowerFact {
            label: "Pending reconnect".to_string(),
            value: controller_state.pending_reconnect.to_string(),
        },
    ]
}

fn status_label(projection: Option<&RuntimeProjection>) -> String {
    projection
        .map(|projection| {
            let mut label = format!(
                "{} · {}",
                projection.server_status.status, projection.server_status.database
            );
            if let Some(message) = &projection.server_status.message {
                if !message.trim().is_empty() {
                    label.push_str(" · ");
                    label.push_str(message);
                }
            }
            label
        })
        .unwrap_or_else(|| "No projection packet".to_string())
}

fn connection_state_label(state: &GuiConnectionState) -> &'static str {
    match state {
        GuiConnectionState::Disconnected => "disconnected",
        GuiConnectionState::Connecting => "connecting",
        GuiConnectionState::Hydrating => "hydrating",
        GuiConnectionState::Streaming => "streaming",
        GuiConnectionState::Reconnecting => "reconnecting",
        GuiConnectionState::ShuttingDown => "shuttingDown",
        GuiConnectionState::Failed => "failed",
    }
}

fn connection_tone(state: &GuiConnectionState) -> &'static str {
    match state {
        GuiConnectionState::Streaming => "success",
        GuiConnectionState::Connecting | GuiConnectionState::Hydrating | GuiConnectionState::Reconnecting => "warning",
        GuiConnectionState::Failed => "danger",
        GuiConnectionState::Disconnected | GuiConnectionState::ShuttingDown => "muted",
    }
}

fn stream_outcome_packet(outcome: SyncOutcome) -> Result<GuiStreamOutcomePacket, ApiErrorPacket> {
    Ok(match outcome {
        SyncOutcome::Hello {
            watermark,
            runtime_identity,
        } => GuiStreamOutcomePacket::Hello {
            watermark,
            runtime_identity,
        },
        SyncOutcome::DeltaApplied {
            delta,
            apply_outcome,
        } => GuiStreamOutcomePacket::DeltaApplied {
            delta: to_json(&delta)?,
            apply_outcome: format!("{apply_outcome:?}"),
        },
        SyncOutcome::ResyncRequired { reason } => GuiStreamOutcomePacket::ResyncRequired { reason },
        SyncOutcome::ServerShutdown => GuiStreamOutcomePacket::ServerShutdown,
        SyncOutcome::StreamClosed => GuiStreamOutcomePacket::StreamClosed,
    })
}

fn optional_json<T: Serialize>(value: Option<&T>) -> Result<Option<Value>, ApiErrorPacket> {
    value.map(to_json).transpose()
}

fn to_json<T: Serialize>(value: &T) -> Result<Value, ApiErrorPacket> {
    serde_json::to_value(value).map_err(|error| {
        ApiErrorPacket::new(
            "internal_error",
            "failed to encode GUI transport packet payload",
            json!({"source":"serde_json", "message": error.to_string()}),
        )
    })
}

fn error_output(request_id: String, error: ApiErrorPacket) -> GuiTransportOutputPacket {
    GuiTransportOutputPacket {
        request_id,
        output: GuiTransportOutput::Error { error },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::ws::{Message, WebSocketUpgrade};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use futures_util::SinkExt;
    use robdex_agent_runtime_projection::{
        CommandRegistryRequestSummary, CommandRegistrySummary, GuiConnectionState,
        GuiOperationOutcome, PendingApprovalSummary, RoleEditorDraft,
        RoleEditorLifecycleAuthorityMetadata, RoleEditorModelDefaults, RoleEditorRoutingMetadata,
        RoleEditorVisibilityMetadata, RoleSummary, RoleVersionSummary, RuntimeDelta,
        RuntimeDeltaKind, RuntimeProjection, ServerStatusProjection, SessionListItem, TimelineItem,
    };
    use std::net::SocketAddr;

    async fn start_transport_test_server() -> String {
        let app = Router::new()
            .route("/state/snapshot", get(|| async {
                Json(RuntimeProjection {
                    watermark: 1,
                    server_status: ServerStatusProjection {
                        status: "ok".to_string(),
                        database: "connected".to_string(),
                        message: None,
                    },
                    roles: vec![test_role_summary()],
                    ..RuntimeProjection::default()
                })
            }))
            .route("/state/ws", get(|ws: WebSocketUpgrade| async move {
                ws.on_upgrade(|mut socket| async move {
                    let delta = RuntimeDelta {
                        watermark: 2,
                        previous_watermark: Some(1),
                        kind: RuntimeDeltaKind::SessionUpsert {
                            session: SessionListItem {
                                id: "transport-session-delta".to_string(),
                                status: "open".to_string(),
                                role_id: Some("runtime-allow".to_string()),
                                role_version: Some("1.0.0".to_string()),
                                project_key: None,
                                title: None,
                                name: None,
                                workdir: ".".to_string(),
                                tracked: true,
                                archived_at: None,
                                closed_at: None,
                                updated_at: None,
                            },
                        },
                    };
                    let message = json!({"type":"delta","delta": serde_json::to_value(delta).expect("delta")}).to_string();
                    socket.send(Message::Text(message.into())).await.expect("send delta");
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                })
            }))
            .route("/sessions", post(Json(json!({"sessionId":"transport-created-session"}))))
            .route("/roles/editor/options", get(Json(json!({"policyDecisions":["allow","deny"],"routingModes":["direct"],"defaultRecipients":["owner"],"knownActions":["tool.execute_code"]}))))
            .route("/roles/editor/validate", post(Json(json!({"valid":true,"errors":[],"warnings":[],"roleId":"gui-role","version":"1.0.0"}))))
            .route("/roles", post(Json(json!({"roleId":"gui-role","versionId":"role-version-1","status":"created"}))))
            .route("/roles/gui-role/versions", post(Json(json!({"roleId":"gui-role","versionId":"role-version-2","status":"updated"}))).get(Json(json!([{"roleVersionId":"role-version-1","version":"1.0.0","current":true}]))))
            .route("/roles/gui-role/export", get(Json(json!({"manifest":{"id":"gui-role"},"instructionText":"inline"}))))
            .route("/roles/gui-role/activate", post(Json(json!({"roleId":"gui-role","versionId":"role-version-1","status":"active"}))))
            .route("/roles/gui-role/archive", post(Json(json!({"roleId":"gui-role","status":"archived"}))))
            .route("/roles/gui-role/unarchive", post(Json(json!({"roleId":"gui-role","status":"active"}))));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve transport test server");
        });
        format!("http://{addr}")
    }

    fn packet(packet_id: &str, intent: GuiTransportRequest) -> GuiTransportRequestPacket {
        GuiTransportRequestPacket {
            packet_id: packet_id.to_string(),
            intent,
        }
    }

    fn test_role_summary() -> RoleSummary {
        RoleSummary {
            id: "gui-role".to_string(),
            display_name: "GUI Role".to_string(),
            current_version_id: Some("role-version-1".to_string()),
            status: "active".to_string(),
            model: Some("gpt-5.4-mini".to_string()),
            reasoning_effort: Some("medium".to_string()),
            archived_at: None,
            version: Some("1.0.0".to_string()),
            instruction_text: Some("inline".to_string()),
            capabilities: vec!["tool.execute_code".to_string()],
            policy: std::collections::BTreeMap::from([("tool.execute_code".to_string(), "allow".to_string())]),
            routing: json!({"mode":"direct","defaultRecipient":"owner","allowedRecipients":["owner"]}),
            visibility: json!({"listed":true,"ownerVisible":true}),
            lifecycle_authority: json!({"canSpawnAgents":false,"canArchiveAgents":false}),
            versions: vec![RoleVersionSummary {
                version_id: "role-version-1".to_string(),
                version: "1.0.0".to_string(),
                status: "current".to_string(),
                created_at: None,
            }],
        }
    }

    fn role_draft() -> RoleEditorDraft {
        RoleEditorDraft {
            id: "gui-role".to_string(),
            version: "1.0.0".to_string(),
            display_name: "GUI Role".to_string(),
            model_defaults: RoleEditorModelDefaults {
                model: "gpt-5.4-mini".to_string(),
                reasoning_effort: "medium".to_string(),
            },
            instruction_text: "inline".to_string(),
            capabilities: vec!["tool.execute_code".to_string()],
            policy: std::collections::BTreeMap::from([("tool.execute_code".to_string(), "allow".to_string())]),
            routing: RoleEditorRoutingMetadata {
                mode: "direct".to_string(),
                default_recipient: Some("owner".to_string()),
                allowed_recipients: vec!["owner".to_string()],
                reserved_actions: vec!["message.send".to_string()],
            },
            visibility: RoleEditorVisibilityMetadata {
                listed: true,
                owner_visible: true,
            },
            lifecycle_authority: RoleEditorLifecycleAuthorityMetadata {
                can_spawn_agents: false,
                can_archive_agents: false,
                reserved_actions: vec!["agent.archive".to_string()],
            },
        }
    }

    fn discovery_packet(service_state: &str, running: bool, health_ok: Option<bool>) -> Value {
        json!({
            "baseUrl": "http://127.0.0.1:8765",
            "healthUrl": "http://127.0.0.1:8765/health",
            "webSocketUrl": "ws://127.0.0.1:8765/state/ws",
            "runtimeIdentity": "runtime-test",
            "serviceState": service_state,
            "stateFlags": {
                "running": running,
                "stopped": service_state == "stopped",
                "stalePid": service_state == "stalePid",
                "unhealthy": service_state == "unhealthy",
                "missingConfig": service_state == "missingConfig",
                "staleDiscovery": service_state == "staleDiscovery"
            },
            "healthResult": {
                "checked": running,
                "ok": health_ok
            },
            "diagnostics": [
                {"code": service_state, "message": "diagnostic"}
            ]
        })
    }

    fn temp_discovery_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "robdex-agent-runtime-discovery-{name}-{}-{}.json",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    #[test]
    fn discovery_default_paths_are_user_scoped_and_not_repo_relative() {
        let mac_state = canonical_service_state_dir(Some("/Users/tester"), None, "macos");
        assert_eq!(
            mac_state,
            PathBuf::from("/Users/tester/Library/Application Support/Robdex Agent Runtime/service")
        );
        let linux_state = canonical_service_state_dir(Some("/home/tester"), None, "linux");
        assert_eq!(linux_state, PathBuf::from("/home/tester/.local/state/robdex-agent-runtime/service"));
        let xdg_state = canonical_service_state_dir(Some("/home/tester"), Some("/state"), "linux");
        assert_eq!(xdg_state, PathBuf::from("/state/robdex-agent-runtime/service"));

        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace path")
            .to_path_buf();
        assert!(!mac_state.starts_with(&workspace));
        assert!(!linux_state.starts_with(&workspace));
        assert!(!mac_state.to_string_lossy().contains(".runtime-service"));
    }

    #[test]
    fn discovery_packets_classify_connectability_and_view_copy() {
        let running = discovery_packet("running", true, Some(true));
        let view = classify_discovery_packet(Some(&running), "discovery.json");
        assert_eq!(view.state, "runningHealthy");
        assert_eq!(view.tone, "success");
        assert!(view.connectable);
        assert_eq!(view.base_url.as_deref(), Some("http://127.0.0.1:8765"));

        let unhealthy = discovery_packet("unhealthy", true, Some(false));
        let view = classify_discovery_packet(Some(&unhealthy), "discovery.json");
        assert_eq!(view.state, "unhealthy");
        assert_eq!(view.tone, "danger");
        assert!(!view.connectable);

        for (service_state, expected, tone) in [
            ("missingConfig", "missingConfig", "warning"),
            ("staleDiscovery", "staleDiscovery", "warning"),
            ("stalePid", "stalePid", "danger"),
        ] {
            let packet = discovery_packet(service_state, false, None);
            let view = classify_discovery_packet(Some(&packet), "discovery.json");
            assert_eq!(view.state, expected);
            assert_eq!(view.tone, tone);
            assert!(!view.connectable);
        }

        let control = AgentRuntimeControlTowerViewModel::from_runtime_state(
            "http://manual.example",
            None,
            &GuiControllerState::default(),
            &[],
            0,
            None,
            &view,
        );
        assert_eq!(control.discovery.state, "unhealthy");
        assert_eq!(control.discovery.title, "Local runtime is unhealthy");
    }

    #[tokio::test]
    async fn connect_discovered_runtime_uses_connectable_discovery_base_url() {
        let base_url = start_transport_test_server().await;
        let path = temp_discovery_path("connectable");
        let mut discovery = discovery_packet("running", true, Some(true));
        discovery["baseUrl"] = json!(base_url);
        std::fs::write(&path, serde_json::to_string(&discovery).expect("packet")).expect("write packet");
        let transport = GuiTransportHandle::spawn();
        let outputs = transport
            .send(packet(
                "connect-discovered-1",
                GuiTransportRequest::ConnectDiscoveredRuntime {
                    discovery_path: Some(path.display().to_string()),
                    selected_session_id: None,
                },
            ))
            .await;
        let _ = std::fs::remove_file(&path);
        assert!(outputs.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::OperationResult {
                result: GuiOperationResult {
                    outcome: GuiOperationOutcome::ProjectionUpdated { watermark: 1 },
                    ..
                }
            }
        )));
        assert!(outputs.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::ControlTowerView { view_model }
                if view_model.discovery.connectable && view_model.discovery.state == "runningHealthy"
        )));
    }

    #[test]
    fn discovery_file_read_path_covers_missing_malformed_stopped_unhealthy_and_running() {
        let missing_path = temp_discovery_path("missing");
        let missing = read_discovery_file(&missing_path);
        assert_eq!(missing.state, "noDiscoveryFile");
        assert!(!missing.connectable);

        let malformed_path = temp_discovery_path("malformed");
        std::fs::write(&malformed_path, "{not-json").expect("write malformed");
        let malformed = read_discovery_file(&malformed_path);
        assert_eq!(malformed.state, "parseError");
        assert!(!malformed.connectable);
        let _ = std::fs::remove_file(&malformed_path);

        for (name, packet, expected_state, connectable) in [
            ("stopped", discovery_packet("stopped", false, None), "stopped", false),
            ("unhealthy", discovery_packet("unhealthy", true, Some(false)), "unhealthy", false),
            ("running", discovery_packet("running", true, Some(true)), "runningHealthy", true),
        ] {
            let path = temp_discovery_path(name);
            std::fs::write(&path, serde_json::to_string(&packet).expect("packet")).expect("write packet");
            let view = read_discovery_file(&path);
            assert_eq!(view.state, expected_state);
            assert_eq!(view.connectable, connectable);
            let _ = std::fs::remove_file(&path);
        }
    }

    #[tokio::test]
    async fn transport_packets_serialize_with_json_backed_payloads() {
        let request = packet(
            "packet-1",
            GuiTransportRequest::DispatchOperation {
                operation: GuiOperationRequest::Disconnect,
            },
        );
        let value = serde_json::to_value(&request).expect("request json");
        assert_eq!(value["intent"]["type"], "dispatchOperation");
        assert_eq!(value["intent"]["payload"]["operation"]["operation"], "disconnect");

        let output = GuiTransportOutputPacket {
            request_id: "packet-1".to_string(),
            output: GuiTransportOutput::ProjectionSnapshot {
                projection: json!({"watermark": 7}),
            },
        };
        let value = serde_json::to_value(&output).expect("output json");
        assert_eq!(value["output"]["type"], "projectionSnapshot");
        assert_eq!(value["output"]["payload"]["projection"]["watermark"], 7);
    }

    #[test]
    fn control_tower_view_model_maps_projection_and_controller_to_constructor_ready_rows() {
        let projection = RuntimeProjection {
            watermark: 9,
            server_status: ServerStatusProjection {
                status: "ok".to_string(),
                database: "connected".to_string(),
                message: Some("runtime ready".to_string()),
            },
            sessions: vec![SessionListItem {
                id: "session-1".to_string(),
                status: "open".to_string(),
                role_id: Some("runtime-allow".to_string()),
                role_version: Some("role-version-1".to_string()),
                project_key: Some("project-a".to_string()),
                title: Some("Runtime check".to_string()),
                name: None,
                workdir: "/tmp/project-a".to_string(),
                tracked: true,
                archived_at: None,
                closed_at: None,
                updated_at: None,
            }],
            timeline: vec![TimelineItem {
                id: "event-7".to_string(),
                sequence: 7,
                session_id: Some("session-1".to_string()),
                turn_id: Some("turn-1".to_string()),
                entity_type: "tool".to_string(),
                entity_id: Some("tool-call-1".to_string()),
                event_type: "tool.completed".to_string(),
                status: Some("completed".to_string()),
                summary: Some("execute_code completed".to_string()),
                payload: json!({"bounded": true}),
                created_at: None,
            }],
            pending_approvals: vec![PendingApprovalSummary {
                id: "approval-1".to_string(),
                session_id: "session-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                action_name: "execute_code".to_string(),
                required_approver_kind: "owner".to_string(),
                status: "approved".to_string(),
                can_decide: false,
                can_resume: true,
                input_context: json!({"raw":"bounded"}),
                created_at: None,
            }],
            command_registry: vec![CommandRegistrySummary {
                id: "cmd-1".to_string(),
                action_id: "rg_project".to_string(),
                scope_type: "project".to_string(),
                project_key: Some("project-a".to_string()),
                enabled: true,
                current_version_id: Some("cmd-version-1".to_string()),
                binary_name: Some("rg".to_string()),
                starlark_object: Some("rg".to_string()),
                starlark_method: Some("project".to_string()),
                updated_at: None,
            }],
            command_registry_requests: vec![CommandRegistryRequestSummary {
                id: "request-1".to_string(),
                operation: "add".to_string(),
                action_id: "cmd.rg.audit".to_string(),
                action_label: "rg · audit".to_string(),
                status: "pending".to_string(),
                state_text: "Needs registry decision".to_string(),
                apply_status: "pending".to_string(),
                final_scope_type: None,
                final_project_key: None,
                scope_summary: None,
                final_policy: None,
                policy_summary: None,
                can_preview: true,
                preview_label: "Preview decision".to_string(),
                can_decide: true,
                decide_label: "Decide request".to_string(),
                can_apply: false,
                apply_label: "Apply unavailable".to_string(),
            }],
            roles: vec![RoleSummary {
                id: "runtime-allow".to_string(),
                display_name: "Runtime Allow".to_string(),
                current_version_id: Some("role-version-1".to_string()),
                status: "active".to_string(),
                model: Some("gpt-5.4-mini".to_string()),
                reasoning_effort: Some("medium".to_string()),
                archived_at: None,
                version: Some("1.0.0".to_string()),
                instruction_text: Some("Inline role instructions".to_string()),
                capabilities: vec!["tool.execute_code".to_string()],
                policy: std::collections::BTreeMap::from([("tool.execute_code".to_string(), "allow".to_string())]),
                routing: json!({"mode":"direct","defaultRecipient":"owner","allowedRecipients":["owner"],"reservedActions":["message.send"]}),
                visibility: json!({"listed":true,"ownerVisible":true}),
                lifecycle_authority: json!({"canSpawnAgents":false,"canArchiveAgents":false,"reservedActions":["agent.archive"]}),
                versions: vec![
                    robdex_agent_runtime_projection::RoleVersionSummary {
                        version_id: "role-version-1".to_string(),
                        version: "1.0.0".to_string(),
                        status: "current".to_string(),
                        created_at: Some("now".to_string()),
                    },
                    robdex_agent_runtime_projection::RoleVersionSummary {
                        version_id: "role-version-0".to_string(),
                        version: "0.9.0".to_string(),
                        status: "available".to_string(),
                        created_at: Some("before".to_string()),
                    },
                ],
            }],
            ..RuntimeProjection::default()
        };
        let controller = GuiControllerState {
            connection_state: GuiConnectionState::Streaming,
            selected_session_id: Some("session-1".to_string()),
            pending_rehydrate: false,
            pending_reconnect: false,
            ..GuiControllerState::default()
        };

        let view = AgentRuntimeControlTowerViewModel::from_runtime_state(
            "http://127.0.0.1:8765",
            Some(&projection),
            &controller,
            &["operationResult · request-1".to_string()],
            2,
            None,
            &AgentRuntimeDiscoveryView::default(),
        );

        assert_eq!(view.connection_state, "streaming");
        assert_eq!(view.connection_tone, "success");
        assert_eq!(view.status_label, "ok · connected · runtime ready");
        assert_eq!(view.watermark_label, "9");
        assert_eq!(view.sessions_title, "Sessions (1)");
        assert_eq!(view.sessions_subtitle, "Grouped by Rust-owned operational state");
        assert_eq!(view.timeline_title, "Selected session stream · Runtime check");
        assert_eq!(view.timeline_subtitle, "Operations event stream, not a chat transcript");
        assert_eq!(view.actions_title, "Action queue (2)");
        assert_eq!(view.actions_subtitle, "Approvals, resumable work, and registry requests");
        assert_eq!(view.detail_subtitle, "Rust-owned controller facts");
        assert_eq!(view.sessions_empty_title, "No sessions");
        assert_eq!(view.timeline_empty_title, "Runtime check");
        assert_eq!(view.actions_empty_title, "No action required");
        assert!(view.status_badges.iter().any(|badge| badge.label == "Attention" && badge.value == "2"));
        assert!(view.status_badges.iter().any(|badge| badge.label == "Registry requests" && badge.value == "1"));
        assert!(view.status_badges.iter().any(|badge| badge.label == "Command inventory" && badge.value == "1"));
        assert!(!view.status_badges.iter().any(|badge| badge.label == "Pending UI requests"));
        assert_eq!(view.sessions[0].title, "Runtime check");
        assert!(view.sessions[0].subtitle.contains("runtime-allow"));
        assert_eq!(view.sessions[0].group_label, "Open");
        assert_eq!(view.sessions[0].tone, "success");
        assert_eq!(view.timeline[0].title, "tool.completed");
        assert_eq!(view.timeline[0].subtitle, "execute_code completed");
        assert_eq!(view.timeline[0].tone, "success");
        assert!(view.actions.iter().any(|row| row.kind == "approval" && row.subtitle.contains("canResume=true")));
        assert!(view.actions.iter().any(|row| row.state_text == "Ready to resume" && row.tone == "success"));
        assert!(view.actions.iter().any(|row| row.kind == "commandRegistryRequest" && row.title == "rg · audit" && row.state_text == "Needs registry decision"));
        assert!(!view.actions.iter().any(|row| row.kind == "commandRegistry"));
        assert_eq!(view.role_admin.title, "Role Admin (1)");
        assert_eq!(view.role_admin.rows[0].id, "runtime-allow");
        assert_eq!(view.role_admin.version_rows.len(), 2);
        assert_eq!(view.role_admin.selected_detail.as_ref().map(|role| role.instruction_text.as_str()), Some("Inline role instructions"));
        assert_eq!(view.role_admin.editor_draft.as_ref().map(|draft| draft.capabilities.len()), Some(1));
        assert!(view.role_admin.action_states.iter().any(|row| row.kind == "roleAdmin" && row.title == "Validate draft"));
        assert!(view.controller_facts.iter().any(|fact| fact.label == "Selected session" && fact.value == "session-1"));
        assert_eq!(view.pending_request_count, 2);
    }

    #[tokio::test]
    async fn transport_runner_serializes_controller_access_and_covers_core_intents() {
        let base_url = start_transport_test_server().await;
        let transport = GuiTransportHandle::spawn();

        let connect = transport
            .send(packet(
                "connect-1",
                GuiTransportRequest::Connect {
                    base_url: base_url.clone(),
                    selected_session_id: None,
                },
            ))
            .await;
        assert!(connect.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::OperationResult {
                result: GuiOperationResult {
                    outcome: GuiOperationOutcome::ProjectionUpdated { watermark: 1 },
                    ..
                }
            }
        )));
        assert!(connect.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::ProjectionSnapshot { projection } if projection["watermark"] == 1
        )));
        assert!(connect.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::ControllerState { controller_state }
                if controller_state["connectionState"] == "streaming"
        )));
        assert!(connect.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::ControlTowerView { view_model }
                if view_model.connection_state == "streaming" && view_model.watermark_label == "1"
        )));

        let created = transport
            .send(packet(
                "create-1",
                GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CreateSession {
                        role: "runtime-allow".to_string(),
                        project: None,
                        workdir: Some(".".to_string()),
                        worktree_root: None,
                        title: None,
                        name: None,
                    },
                },
            ))
            .await;
        assert!(created.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::OperationResult {
                result: GuiOperationResult {
                    outcome: GuiOperationOutcome::Accepted {
                        entity_id: Some(id),
                    },
                    ..
                }
            } if id == "transport-created-session"
        )));

        let stream = transport
            .send(packet("stream-1", GuiTransportRequest::PollStreamOnce))
            .await;
        assert!(stream.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::StreamOutcome {
                outcome: GuiStreamOutcomePacket::DeltaApplied { .. },
                projection: Some(projection),
                controller_state,
            } if projection["watermark"] == 2 && controller_state["connectionState"] == "streaming"
        )));
        assert!(stream.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::ControlTowerView { view_model }
                if view_model.sessions.iter().any(|row| row.id == "transport-session-delta")
        )));

        let rehydrate = transport
            .send(packet(
                "rehydrate-1",
                GuiTransportRequest::Rehydrate {
                    selected_session_id: None,
                },
            ))
            .await;
        assert!(rehydrate.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::ProjectionSnapshot { projection } if projection["watermark"] == 1
        )));

        let disconnect = transport
            .send(packet("disconnect-1", GuiTransportRequest::Disconnect))
            .await;
        assert!(disconnect.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::ControllerState { controller_state }
                if controller_state["connectionState"] == "disconnected"
        )));
    }

    #[tokio::test]
    async fn transport_dispatches_role_editor_operations() {
        let base_url = start_transport_test_server().await;
        let transport = GuiTransportHandle::spawn();
        let _ = transport
            .send(packet(
                "connect-role",
                GuiTransportRequest::Connect {
                    base_url,
                    selected_session_id: None,
                },
            ))
            .await;

        let options = transport
            .send(packet(
                "role-options",
                GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::RoleEditorOptions,
                },
            ))
            .await;
        assert!(options.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::OperationResult {
                result: GuiOperationResult {
                    outcome: GuiOperationOutcome::DirectValue { value },
                    ..
                }
            } if value["policyDecisions"].is_array()
        )));

        let validate = transport
            .send(packet(
                "role-validate",
                GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::ValidateRoleDraft { draft: role_draft() },
                },
            ))
            .await;
        assert!(validate.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::OperationResult {
                result: GuiOperationResult {
                    outcome: GuiOperationOutcome::DirectValue { value },
                    ..
                }
            } if value["valid"] == true
        )));

        for (packet_id, operation) in [
            ("role-create", GuiOperationRequest::CreateRoleFromDraft { draft: role_draft() }),
            ("role-update", GuiOperationRequest::UpdateRoleFromDraft { role_id: "gui-role".to_string(), draft: role_draft() }),
            ("role-activate", GuiOperationRequest::ActivateRoleVersion { role_id: "gui-role".to_string(), version_id: "role-version-1".to_string() }),
            ("role-archive", GuiOperationRequest::ArchiveRole { role_id: "gui-role".to_string() }),
            ("role-unarchive", GuiOperationRequest::UnarchiveRole { role_id: "gui-role".to_string() }),
        ] {
            let outputs = transport
                .send(packet(packet_id, GuiTransportRequest::DispatchOperation { operation }))
                .await;
            assert!(outputs.iter().any(|packet| matches!(
                &packet.output,
                GuiTransportOutput::OperationResult {
                    result: GuiOperationResult {
                        outcome: GuiOperationOutcome::ProjectionUpdated { .. },
                        ..
                    }
                }
            )));
            assert!(outputs.iter().any(|packet| matches!(
                &packet.output,
                GuiTransportOutput::ControlTowerView { view_model }
                    if view_model.role_admin.rows.iter().any(|role| role.id == "gui-role")
            )));
        }

        let export = transport
            .send(packet(
                "role-export",
                GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::ExportRole { role_id: "gui-role".to_string() },
                },
            ))
            .await;
        assert!(export.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::OperationResult {
                result: GuiOperationResult {
                    outcome: GuiOperationOutcome::DirectValue { value },
                    ..
                }
            } if value["instructionText"] == "inline"
        )));
    }

    #[tokio::test]
    async fn transport_maps_controller_errors_to_typed_error_packets() {
        let transport = GuiTransportHandle::spawn();
        let outputs = transport
            .send(packet("stream-before-connect", GuiTransportRequest::PollStreamOnce))
            .await;
        assert_eq!(outputs.len(), 2);
        match &outputs[0].output {
            GuiTransportOutput::Error { error } => {
                assert_eq!(error.error.code, "conflict");
                assert_eq!(error.error.details["operation"], "nextStreamOutcome");
            }
            other => panic!("expected typed error packet, got {other:?}"),
        }
        assert!(matches!(
            &outputs[1].output,
            GuiTransportOutput::ControlTowerView { view_model }
                if view_model.error_message.as_deref().is_some_and(|message| message.contains("conflict"))
        ));
    }
}
