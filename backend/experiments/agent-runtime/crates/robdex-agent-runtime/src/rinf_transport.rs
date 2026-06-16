//! Experiment-local Rinf-shaped transport proof.
//!
//! This module intentionally does not depend on Rinf or Flutter. It models the
//! packet boundary a future `frontend/robdex_app/native/hub` integration can use
//! while keeping runtime state, reduction, and operation decisions inside Rust.

use robdex_agent_runtime_projection::{
    ApiErrorPacket, CommandRegistrySummary, GuiConnectionState, GuiControllerState,
    GuiOperationRequest, GuiOperationResult, PendingApprovalSummary, RuntimeProjection,
    SessionListItem, TimelineItem,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
    pub connection_state: String,
    pub base_url: String,
    pub status_label: String,
    pub watermark_label: String,
    pub sessions: Vec<AgentRuntimeControlTowerSessionRow>,
    pub timeline: Vec<AgentRuntimeControlTowerTimelineRow>,
    pub actions: Vec<AgentRuntimeControlTowerActionRow>,
    pub controller_facts: Vec<AgentRuntimeControlTowerFact>,
    pub output_log: Vec<String>,
    pub pending_request_count: usize,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControlTowerSessionRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub subtitle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControlTowerTimelineRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControlTowerActionRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeControlTowerFact {
    pub label: String,
    pub value: String,
}

impl AgentRuntimeControlTowerViewModel {
    pub fn from_runtime_state(
        base_url: impl Into<String>,
        projection: Option<&RuntimeProjection>,
        controller_state: &GuiControllerState,
        output_log: &[String],
        pending_request_count: usize,
        error_message: Option<String>,
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
                    .chain(projection.command_registry.iter().map(command_action_row))
                    .collect()
            })
            .unwrap_or_default();
        actions.sort_by(|left, right| left.kind.cmp(&right.kind).then(left.id.cmp(&right.id)));
        Self {
            connection_state: connection_state_label(&controller_state.connection_state).to_string(),
            base_url,
            status_label: status_label(projection),
            watermark_label: projection
                .map(|projection| projection.watermark.to_string())
                .unwrap_or_else(|| "—".to_string()),
            sessions,
            timeline,
            actions,
            controller_facts: controller_facts(controller_state),
            output_log: output_log.iter().take(8).cloned().collect(),
            pending_request_count,
            error_message,
        }
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
}

impl GuiTransportRunner {
    fn new() -> Self {
        Self {
            controller: GuiBackendController::new(),
            base_url: "http://127.0.0.1:8765".to_string(),
            output_log: Vec::new(),
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
                ),
            },
        }
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
    }
}

fn command_action_row(command: &CommandRegistrySummary) -> AgentRuntimeControlTowerActionRow {
    let name = command
        .starlark_object
        .as_ref()
        .zip(command.starlark_method.as_ref())
        .map(|(object, method)| format!("cmd.{object}.{method}"))
        .unwrap_or_else(|| command.action_id.clone());
    AgentRuntimeControlTowerActionRow {
        id: command.id.clone(),
        title: name,
        subtitle: format!(
            "{}{} · {}",
            command.scope_type,
            command
                .project_key
                .as_ref()
                .map(|project| format!(":{project}"))
                .unwrap_or_default(),
            if command.enabled { "enabled" } else { "disabled" }
        ),
        kind: "commandRegistry".to_string(),
    }
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
        CommandRegistrySummary, GuiConnectionState, GuiOperationOutcome, PendingApprovalSummary,
        RuntimeDelta, RuntimeDeltaKind, RuntimeProjection, ServerStatusProjection, SessionListItem,
        TimelineItem,
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
            .route("/sessions", post(Json(json!({"sessionId":"transport-created-session"}))));
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
        );

        assert_eq!(view.connection_state, "streaming");
        assert_eq!(view.status_label, "ok · connected · runtime ready");
        assert_eq!(view.watermark_label, "9");
        assert_eq!(view.sessions[0].title, "Runtime check");
        assert!(view.sessions[0].subtitle.contains("runtime-allow"));
        assert_eq!(view.timeline[0].title, "tool.completed");
        assert_eq!(view.timeline[0].subtitle, "execute_code completed");
        assert!(view.actions.iter().any(|row| row.kind == "approval" && row.subtitle.contains("canResume=true")));
        assert!(view.actions.iter().any(|row| row.kind == "commandRegistry" && row.title == "cmd.rg.project"));
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
