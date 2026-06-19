use robdex_agent_runtime_projection::{
    ApiErrorPacket, CommandRegistryRequestSummary, GuiConnectionState, GuiControllerState,
    GuiOperationOutcome, GuiOperationRequest, GuiOperationResult, RuntimeProjection,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::gui_sync::{RuntimeStateStream, RuntimeSyncClient, RuntimeSyncConfig, SyncError, SyncOutcome};

pub struct GuiBackendController {
    http: reqwest::Client,
    base_url: Option<String>,
    sync_client: Option<RuntimeSyncClient>,
    stream: Option<RuntimeStateStream>,
    stream_handle: Option<GuiStreamControllerHandle>,
    projection: Option<RuntimeProjection>,
    controller_state: GuiControllerState,
    next_operation_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiStreamControllerHandle {
    pub url: String,
    pub after: i64,
    pub selected_session_id: Option<String>,
    pub connected: bool,
}

impl Default for GuiBackendController {
    fn default() -> Self {
        Self::new()
    }
}

impl GuiBackendController {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: None,
            sync_client: None,
            stream: None,
            stream_handle: None,
            projection: None,
            controller_state: GuiControllerState::default(),
            next_operation_id: 1,
        }
    }

    pub fn projection(&self) -> Option<&RuntimeProjection> {
        self.projection.as_ref()
    }

    pub fn controller_state(&self) -> &GuiControllerState {
        &self.controller_state
    }

    pub fn controller_state_mut(&mut self) -> &mut GuiControllerState {
        &mut self.controller_state
    }

    pub fn sync_client(&self) -> Option<&RuntimeSyncClient> {
        self.sync_client.as_ref()
    }

    pub fn stream_handle(&self) -> Option<&GuiStreamControllerHandle> {
        self.stream_handle.as_ref()
    }

    pub async fn dispatch(&mut self, request: GuiOperationRequest) -> GuiOperationResult {
        let operation_id = self.allocate_operation_id();
        let operation = request.name();
        let expectation = request.expected_projection_effect();
        match self.dispatch_inner(&request).await {
            Ok(outcome) => GuiOperationResult {
                operation_id,
                operation,
                expectation,
                outcome,
            },
            Err(error) => {
                self.controller_state.transient_errors.push(error.clone());
                GuiOperationResult {
                    operation_id,
                    operation,
                    expectation,
                    outcome: GuiOperationOutcome::Error { error },
                }
            }
        }
    }

    pub async fn next_stream_outcome(&mut self) -> Result<SyncOutcome, ApiErrorPacket> {
        let mut stream = self.stream.take().ok_or_else(|| {
            api_error("conflict", "GUI controller stream is not connected", json!({"operation":"nextStreamOutcome"}))
        })?;
        let (outcome, projection, controller_state) = {
            let sync = self.sync_client.as_mut().ok_or_else(|| {
                api_error("conflict", "GUI controller is not connected", json!({"operation":"nextStreamOutcome"}))
            })?;
            let outcome = stream.next_outcome(sync).await.map_err(sync_error_packet)?;
            (outcome, sync.projection().cloned(), sync.controller_state().clone())
        };
        self.projection = projection;
        self.replace_controller_state_from_sync(controller_state);
        match outcome {
            SyncOutcome::StreamClosed | SyncOutcome::ServerShutdown => {
                self.stream = None;
                if let Some(handle) = self.stream_handle.as_mut() {
                    handle.connected = false;
                }
            }
            _ => {
                self.stream = Some(stream);
            }
        }
        Ok(outcome)
    }

    async fn dispatch_inner(&mut self, request: &GuiOperationRequest) -> Result<GuiOperationOutcome, ApiErrorPacket> {
        match request {
            GuiOperationRequest::Connect { base_url, selected_session_id } => {
                self.base_url = Some(base_url.clone());
                self.replace_sync_client(base_url, selected_session_id.as_deref())?;
                self.hydrate_current().await
            }
            GuiOperationRequest::Hydrate { selected_session_id } => {
                let base_url = self.required_base_url()?;
                self.replace_sync_client(&base_url, selected_session_id.as_deref())?;
                self.hydrate_current().await
            }
            GuiOperationRequest::Rehydrate { selected_session_id } => {
                let base_url = self.required_base_url()?;
                self.replace_sync_client(&base_url, selected_session_id.as_deref())?;
                self.hydrate_current().await
            }
            GuiOperationRequest::Disconnect => {
                self.stream = None;
                self.stream_handle = None;
                self.sync_client = None;
                self.projection = None;
                self.controller_state.connection_state = GuiConnectionState::Disconnected;
                Ok(GuiOperationOutcome::Accepted { entity_id: None })
            }
            GuiOperationRequest::SelectSession { session_id } => {
                let base_url = self.required_base_url()?;
                self.controller_state.select_session(session_id.clone());
                self.replace_sync_client(&base_url, session_id.as_deref())?;
                self.hydrate_current().await
            }
            GuiOperationRequest::SelectWorkflowMemory { memory_id } => {
                self.controller_state.select_workflow_memory(memory_id.clone());
                Ok(GuiOperationOutcome::Accepted { entity_id: memory_id.clone() })
            }
            GuiOperationRequest::UpdateRuntimeSettings { base_url, selected_project_id } => {
                if base_url.trim().is_empty() {
                    return Err(api_error("validation_failed", "runtime settings require a non-empty base URL", json!({"field":"baseUrl"})));
                }
                self.base_url = Some(base_url.trim().to_string());
                self.controller_state.select_project(selected_project_id.clone());
                Ok(GuiOperationOutcome::Accepted { entity_id: selected_project_id.clone() })
            }
            GuiOperationRequest::CreateSession { .. }
            | GuiOperationRequest::ListProjects
            | GuiOperationRequest::CreateProject { .. }
            | GuiOperationRequest::UpdateProject { .. }
            | GuiOperationRequest::ArchiveProject { .. }
            | GuiOperationRequest::UnarchiveProject { .. }
            | GuiOperationRequest::UpdateSessionSettings { .. }
            | GuiOperationRequest::SendMessage { .. }
            | GuiOperationRequest::TerminateProcess { .. }
            | GuiOperationRequest::InputProcess { .. }
            | GuiOperationRequest::FlushProcess { .. }
            | GuiOperationRequest::CloseSession { .. }
            | GuiOperationRequest::ArchiveSession { .. }
            | GuiOperationRequest::ForkSession { .. }
            | GuiOperationRequest::DecideApproval { .. }
            | GuiOperationRequest::ResumeApproval { .. }
            | GuiOperationRequest::ListCommandRegistry { .. }
            | GuiOperationRequest::ShowCommand { .. }
            | GuiOperationRequest::ListCommandRegistryRequests
            | GuiOperationRequest::ShowCommandRegistryRequest { .. }
            | GuiOperationRequest::PreviewCommandRegistryRequest { .. }
            | GuiOperationRequest::DecideCommandRegistryRequest { .. }
            | GuiOperationRequest::ApplyCommandRegistryRequest { .. }
            | GuiOperationRequest::WorkflowMemoryFeedback { .. }
            | GuiOperationRequest::RoleEditorOptions
            | GuiOperationRequest::ValidateRoleDraft { .. }
            | GuiOperationRequest::CreateRoleFromDraft { .. }
            | GuiOperationRequest::UpdateRoleFromDraft { .. }
            | GuiOperationRequest::ShowRoleDetail { .. }
            | GuiOperationRequest::ListRoleVersions { .. }
            | GuiOperationRequest::ShowRoleVersion { .. }
            | GuiOperationRequest::ExportRole { .. }
            | GuiOperationRequest::ActivateRoleVersion { .. }
            | GuiOperationRequest::ArchiveRole { .. }
            | GuiOperationRequest::UnarchiveRole { .. } => self.dispatch_server_operation(request).await,
        }
    }

    async fn hydrate_current(&mut self) -> Result<GuiOperationOutcome, ApiErrorPacket> {
        let (snapshot, stream_url, selected_session_id) = {
            let sync = self.sync_client.as_mut().ok_or_else(|| {
                api_error("conflict", "GUI controller is not connected", json!({"operation":"hydrate"}))
            })?;
            let snapshot = sync.hydrate().await.map_err(sync_error_packet)?.clone();
            let watermark = snapshot.watermark;
            let stream_url = sync.config().websocket_url(Some(watermark)).map_err(sync_error_packet)?;
            let selected_session_id = sync.config().selected_session_id.map(|id| id.to_string());
            (snapshot, stream_url, selected_session_id)
        };
        let watermark = snapshot.watermark;
        self.projection = Some(snapshot);
        self.reconnect_stream(watermark, stream_url, selected_session_id).await?;
        if let Some(sync) = self.sync_client.as_ref() {
            self.replace_controller_state_from_sync(sync.controller_state().clone());
        }
        Ok(GuiOperationOutcome::ProjectionUpdated { watermark })
    }

    fn replace_controller_state_from_sync(&mut self, mut next: GuiControllerState) {
        next.selected_project_id = self.controller_state.selected_project_id.clone();
        self.controller_state = next;
    }

    async fn reconnect_stream(
        &mut self,
        after: i64,
        stream_url: String,
        selected_session_id: Option<String>,
    ) -> Result<(), ApiErrorPacket> {
        self.stream = None;
        self.stream_handle = Some(GuiStreamControllerHandle {
            url: stream_url,
            after,
            selected_session_id,
            connected: false,
        });
        let sync = self.sync_client.as_ref().ok_or_else(|| {
            api_error("conflict", "GUI controller is not connected", json!({"operation":"connectStream"}))
        })?;
        let stream = sync.connect_after(Some(after)).await.map_err(sync_error_packet)?;
        self.stream = Some(stream);
        if let Some(handle) = self.stream_handle.as_mut() {
            handle.connected = true;
        }
        Ok(())
    }

    async fn dispatch_server_operation(&mut self, request: &GuiOperationRequest) -> Result<GuiOperationOutcome, ApiErrorPacket> {
        let base_url = self.required_base_url()?;
        let mapping = request.api_mapping();
        let method = mapping.method.as_deref().ok_or_else(|| {
            api_error("bad_request", "operation has no server route", json!({"operation": format!("{:?}", request.name())}))
        })?;
        let route = self.route_for_request(request)?;
        let url = format!("{}{}", base_url.trim_end_matches('/'), route);
        let mut builder = match method {
            "GET" => self.http.get(url),
            "POST" => self.http.post(url),
            other => return Err(api_error("internal_error", "unsupported GUI operation HTTP method", json!({"method": other}))),
        };
        if method == "POST" {
            builder = builder.json(&request.to_server_request_json().unwrap_or_else(|| json!({})));
        }
        let value = self.send_json(builder).await?;
        match request {
            GuiOperationRequest::ListCommandRegistryRequests => {
                let rows = value.as_array().cloned().unwrap_or_default();
                let requests = rows
                    .iter()
                    .map(CommandRegistryRequestSummary::from_server_value)
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| api_error("internal_error", "invalid command registry request summary response", json!({})))?;
                Ok(GuiOperationOutcome::CommandRegistryRequests { requests })
            }
            GuiOperationRequest::ListProjects
            | GuiOperationRequest::CreateProject { .. }
            | GuiOperationRequest::UpdateProject { .. }
            | GuiOperationRequest::ArchiveProject { .. }
            | GuiOperationRequest::UnarchiveProject { .. } => self.hydrate_current().await,
            GuiOperationRequest::ListCommandRegistry { .. }
            | GuiOperationRequest::ShowCommand { .. }
            | GuiOperationRequest::ShowCommandRegistryRequest { .. }
            | GuiOperationRequest::PreviewCommandRegistryRequest { .. }
            | GuiOperationRequest::RoleEditorOptions
            | GuiOperationRequest::ValidateRoleDraft { .. }
            | GuiOperationRequest::ShowRoleDetail { .. }
            | GuiOperationRequest::ListRoleVersions { .. }
            | GuiOperationRequest::ShowRoleVersion { .. }
            | GuiOperationRequest::ExportRole { .. } => Ok(GuiOperationOutcome::DirectValue { value }),
            GuiOperationRequest::CreateSession { .. } => {
                let entity_id = value.get("sessionId").and_then(Value::as_str).map(str::to_string);
                if let Some(id) = entity_id.as_deref() {
                    let base_url = self.required_base_url()?;
                    self.controller_state.select_session(Some(id.to_string()));
                    self.replace_sync_client(&base_url, Some(id))?;
                    let _ = self.hydrate_current().await?;
                }
                Ok(GuiOperationOutcome::Accepted { entity_id })
            }
            GuiOperationRequest::UpdateSessionSettings { session_id, .. } => {
                let base_url = self.required_base_url()?;
                self.replace_sync_client(&base_url, Some(session_id))?;
                let _ = self.hydrate_current().await?;
                Ok(GuiOperationOutcome::Accepted {
                    entity_id: Some(session_id.clone()),
                })
            }
            GuiOperationRequest::SendMessage { .. } => Ok(GuiOperationOutcome::Accepted {
                entity_id: {
                    let id = value.get("turnId").and_then(Value::as_str).map(str::to_string);
                    let _ = self.hydrate_current().await?;
                    id
                },
            }),
            GuiOperationRequest::TerminateProcess { handle, .. }
            | GuiOperationRequest::InputProcess { handle, .. }
            | GuiOperationRequest::FlushProcess { handle, .. } => Ok(GuiOperationOutcome::Accepted {
                entity_id: Some(handle.clone()),
            }),
            GuiOperationRequest::ForkSession { .. } => {
                let entity_id = value.get("sessionId").and_then(Value::as_str).map(str::to_string);
                if let Some(id) = entity_id.as_deref() {
                    let base_url = self.required_base_url()?;
                    self.controller_state.select_session(Some(id.to_string()));
                    self.replace_sync_client(&base_url, Some(id))?;
                    let _ = self.hydrate_current().await?;
                }
                Ok(GuiOperationOutcome::Accepted { entity_id })
            }
            GuiOperationRequest::DecideApproval { approval_id, .. }
            | GuiOperationRequest::ResumeApproval { approval_id } => Ok(GuiOperationOutcome::Accepted {
                entity_id: Some(approval_id.clone()),
            }),
            GuiOperationRequest::CloseSession { session_id, .. }
            | GuiOperationRequest::ArchiveSession { session_id } => {
                let base_url = self.required_base_url()?;
                self.replace_sync_client(&base_url, Some(session_id))?;
                let _ = self.hydrate_current().await?;
                Ok(GuiOperationOutcome::Accepted {
                    entity_id: Some(session_id.clone()),
                })
            }
            GuiOperationRequest::WorkflowMemoryFeedback { memory_id: session_id, .. }
            | GuiOperationRequest::DecideCommandRegistryRequest { request_id: session_id, .. }
            | GuiOperationRequest::ApplyCommandRegistryRequest { request_id: session_id, .. } => Ok(GuiOperationOutcome::Accepted {
                entity_id: Some(session_id.clone()),
            }),
            GuiOperationRequest::CreateRoleFromDraft { .. }
            | GuiOperationRequest::UpdateRoleFromDraft { .. }
            | GuiOperationRequest::ActivateRoleVersion { .. }
            | GuiOperationRequest::ArchiveRole { .. }
            | GuiOperationRequest::UnarchiveRole { .. } => self.hydrate_current().await,
            GuiOperationRequest::Connect { .. }
            | GuiOperationRequest::Hydrate { .. }
            | GuiOperationRequest::Rehydrate { .. }
            | GuiOperationRequest::Disconnect
            | GuiOperationRequest::SelectSession { .. }
            | GuiOperationRequest::SelectWorkflowMemory { .. }
            | GuiOperationRequest::UpdateRuntimeSettings { .. } => unreachable!("local operations handled before server dispatch"),
        }
    }

    fn replace_sync_client(&mut self, base_url: &str, selected_session_id: Option<&str>) -> Result<(), ApiErrorPacket> {
        let mut config = RuntimeSyncConfig::new(base_url);
        if let Some(session_id) = selected_session_id {
            let parsed = Uuid::parse_str(session_id).map_err(|error| {
                api_error("bad_request", "selected session id must be a UUID", json!({"id": session_id, "error": error.to_string()}))
            })?;
            config = config.with_selected_session(parsed);
        }
        self.sync_client = Some(RuntimeSyncClient::new(config));
        self.stream = None;
        self.stream_handle = None;
        Ok(())
    }

    fn required_base_url(&self) -> Result<String, ApiErrorPacket> {
        self.base_url.clone().ok_or_else(|| {
            api_error("conflict", "GUI controller is not connected", json!({"operation":"connect"}))
        })
    }

    fn route_for_request(&self, request: &GuiOperationRequest) -> Result<String, ApiErrorPacket> {
        Ok(match request {
            GuiOperationRequest::CreateSession { .. } => "/sessions".to_string(),
            GuiOperationRequest::ListProjects => "/projects".to_string(),
            GuiOperationRequest::CreateProject { .. } => "/projects".to_string(),
            GuiOperationRequest::UpdateProject { project_key, .. } => format!("/projects/{project_key}"),
            GuiOperationRequest::ArchiveProject { project_key } => format!("/projects/{project_key}/archive"),
            GuiOperationRequest::UnarchiveProject { project_key } => format!("/projects/{project_key}/unarchive"),
            GuiOperationRequest::UpdateSessionSettings { session_id, .. } => format!("/sessions/{session_id}/settings"),
            GuiOperationRequest::SendMessage { session_id, .. } => format!("/sessions/{session_id}/send"),
            GuiOperationRequest::TerminateProcess { session_id, handle } => format!("/sessions/{session_id}/processes/{handle}/terminate"),
            GuiOperationRequest::InputProcess { session_id, handle, .. } => format!("/sessions/{session_id}/processes/{handle}/input"),
            GuiOperationRequest::FlushProcess { session_id, handle } => format!("/sessions/{session_id}/processes/{handle}/flush"),
            GuiOperationRequest::CloseSession { session_id, .. } => format!("/sessions/{session_id}/close"),
            GuiOperationRequest::ArchiveSession { session_id } => format!("/sessions/{session_id}/archive"),
            GuiOperationRequest::ForkSession { session_id, .. } => format!("/sessions/{session_id}/fork"),
            GuiOperationRequest::DecideApproval { approval_id, .. } => format!("/approvals/{approval_id}/decide"),
            GuiOperationRequest::ResumeApproval { approval_id } => format!("/approvals/{approval_id}/resume"),
            GuiOperationRequest::ListCommandRegistry { session_id, project_key } => {
                let mut params = Vec::new();
                if let Some(session_id) = session_id {
                    params.push(format!("sessionId={session_id}"));
                }
                if let Some(project_key) = project_key {
                    params.push(format!("project={project_key}"));
                }
                if params.is_empty() {
                    "/command-registry".to_string()
                } else {
                    format!("/command-registry?{}", params.join("&"))
                }
            }
            GuiOperationRequest::ShowCommand { action_id, session_id, project_key } => {
                let mut params = Vec::new();
                if let Some(session_id) = session_id {
                    params.push(format!("sessionId={session_id}"));
                }
                if let Some(project_key) = project_key {
                    params.push(format!("project={project_key}"));
                }
                if params.is_empty() {
                    format!("/command-registry/{action_id}")
                } else {
                    format!("/command-registry/{action_id}?{}", params.join("&"))
                }
            }
            GuiOperationRequest::ListCommandRegistryRequests => "/command-registry/requests".to_string(),
            GuiOperationRequest::ShowCommandRegistryRequest { request_id } => format!("/command-registry/requests/{request_id}"),
            GuiOperationRequest::PreviewCommandRegistryRequest { request_id, .. } => format!("/command-registry/requests/{request_id}/preview-decision"),
            GuiOperationRequest::DecideCommandRegistryRequest { request_id, .. } => format!("/command-registry/requests/{request_id}/decide"),
            GuiOperationRequest::ApplyCommandRegistryRequest { request_id, .. } => format!("/command-registry/requests/{request_id}/apply"),
            GuiOperationRequest::WorkflowMemoryFeedback { memory_id, .. } => format!("/workflow-memories/{memory_id}/feedback"),
            GuiOperationRequest::RoleEditorOptions => "/roles/editor/options".to_string(),
            GuiOperationRequest::ValidateRoleDraft { .. } => "/roles/editor/validate".to_string(),
            GuiOperationRequest::CreateRoleFromDraft { .. } => "/roles".to_string(),
            GuiOperationRequest::UpdateRoleFromDraft { role_id, .. } => format!("/roles/{role_id}/versions"),
            GuiOperationRequest::ShowRoleDetail { role_id } => format!("/roles/{role_id}"),
            GuiOperationRequest::ListRoleVersions { role_id } => format!("/roles/{role_id}/versions"),
            GuiOperationRequest::ShowRoleVersion { version_id } => format!("/roles/versions/{version_id}"),
            GuiOperationRequest::ExportRole { role_id } => format!("/roles/{role_id}/export"),
            GuiOperationRequest::ActivateRoleVersion { role_id, .. } => format!("/roles/{role_id}/activate"),
            GuiOperationRequest::ArchiveRole { role_id } => format!("/roles/{role_id}/archive"),
            GuiOperationRequest::UnarchiveRole { role_id } => format!("/roles/{role_id}/unarchive"),
            GuiOperationRequest::Connect { .. }
            | GuiOperationRequest::Hydrate { .. }
            | GuiOperationRequest::Rehydrate { .. }
            | GuiOperationRequest::Disconnect
            | GuiOperationRequest::SelectSession { .. }
            | GuiOperationRequest::SelectWorkflowMemory { .. }
            | GuiOperationRequest::UpdateRuntimeSettings { .. } => {
                return Err(api_error("bad_request", "local GUI operation has no server route", json!({"operation": format!("{:?}", request.name())})));
            }
        })
    }

    async fn send_json(&self, builder: reqwest::RequestBuilder) -> Result<Value, ApiErrorPacket> {
        let response = builder.send().await.map_err(|error| {
            api_error("unavailable", "runtime server unavailable", json!({"source":"reqwest", "message": error.to_string()}))
        })?;
        let status = response.status();
        let text = response.text().await.map_err(|error| {
            api_error("unavailable", "failed to read runtime server response", json!({"source":"reqwest", "message": error.to_string()}))
        })?;
        let value = if text.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).map_err(|error| {
                api_error("internal_error", "runtime server returned invalid JSON", json!({"message": error.to_string()}))
            })?
        };
        if status.is_success() {
            Ok(value)
        } else if let Ok(packet) = serde_json::from_value::<ApiErrorPacket>(value.clone()) {
            Err(packet)
        } else {
            Err(api_error("internal_error", "runtime server returned an untyped error", json!({"status": status.as_u16()})))
        }
    }

    fn allocate_operation_id(&mut self) -> String {
        let id = self.next_operation_id;
        self.next_operation_id += 1;
        format!("gui-op-{id}")
    }
}

fn sync_error_packet(error: SyncError) -> ApiErrorPacket {
    match error {
        SyncError::Http(error) => api_error("unavailable", "runtime server HTTP sync failed", json!({"message": error.to_string()})),
        SyncError::WebSocket(error) => api_error("unavailable", "runtime server WebSocket sync failed", json!({"message": error.to_string()})),
        SyncError::Json(error) => api_error("internal_error", "runtime server sync JSON decode failed", json!({"message": error.to_string()})),
        SyncError::Protocol(message) => api_error("internal_error", "runtime server sync protocol error", json!({"message": message})),
    }
}

fn api_error(code: impl Into<String>, message: impl Into<String>, details: Value) -> ApiErrorPacket {
    ApiErrorPacket::new(code, message, details)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::ws::{Message, WebSocketUpgrade};
    use axum::response::Response;
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use futures_util::SinkExt;
    use robdex_agent_runtime_projection::{
        ApplyOutcome, CommandRegistryDecisionInput, GuiCommandSeed, GuiFinalExecutionPolicy,
        GuiOperationExpectation, GuiRegistryScope, RoleEditorDraft, RoleEditorLifecycleAuthorityMetadata,
        RoleEditorModelDefaults, RoleEditorRoutingMetadata, RoleEditorVisibilityMetadata, RoleSummary,
        RoleVersionSummary, RuntimeDelta, RuntimeDeltaKind, ServerStatusProjection, SessionListItem,
    };
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};

    async fn test_ws(ws: WebSocketUpgrade) -> Response {
        ws.on_upgrade(|_socket| async move {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        })
    }

    async fn start_test_server() -> String {
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
            .route("/state/ws", get(test_ws))
            .route("/sessions", post(Json(json!({"sessionId":"00000000-0000-0000-0000-00000000c001"}))))
            .route("/sessions/session-1/send", post(Json(json!({"sessionId":"session-1","turnId":"turn-1","status":"completed"}))))
            .route("/command-registry/requests", get(Json(json!([{
                "id":"request-1",
                "operation":"add",
                "proposedCommand":{"actionId":"cmd.rg.audit"},
                "approvalStatus":"approved",
                "applicationStatus":"pending",
                "finalScope":{"scopeType":"global"},
                "finalExecutionPolicy":{"decision":"allow"}
            }]))))
            .route("/command-registry/requests/request-1/preview-decision", post(Json(json!({"ok":true}))))
            .route("/error", get(Json(json!({"error":{"code":"not_found","message":"missing","details":{"entity":"test"}}}))));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test router");
        });
        format!("http://{addr}")
    }

    async fn start_delta_stream_test_server() -> String {
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
                                id: "session-from-websocket".to_string(),
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
                    let message = json!({"type":"delta", "delta": serde_json::to_value(delta).expect("delta json")}).to_string();
                    socket.send(Message::Text(message.into())).await.expect("send delta");
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                })
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve delta stream router");
        });
        format!("http://{addr}")
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
            instruction_text: "inline instructions".to_string(),
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

    async fn start_role_mutation_test_server() -> String {
        let mutated = Arc::new(Mutex::new(false));
        let snapshot_mutated = mutated.clone();
        let post_mutated = mutated.clone();
        let app = Router::new()
            .route("/state/snapshot", get(move || {
                let snapshot_mutated = snapshot_mutated.clone();
                async move {
                    let is_mutated = *snapshot_mutated.lock().expect("snapshot state");
                    Json(RuntimeProjection {
                        watermark: if is_mutated { 2 } else { 1 },
                        roles: if is_mutated {
                            vec![RoleSummary {
                                id: "gui-role".to_string(),
                                display_name: "GUI Role".to_string(),
                                current_version_id: Some("role-version-1".to_string()),
                                status: "active".to_string(),
                                model: Some("gpt-5.4-mini".to_string()),
                                reasoning_effort: Some("medium".to_string()),
                                archived_at: None,
                                version: Some("1.0.0".to_string()),
                                instruction_text: Some("inline instructions".to_string()),
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
                            }]
                        } else {
                            Vec::new()
                        },
                        ..RuntimeProjection::default()
                    })
                }
            }))
            .route("/state/ws", get(test_ws))
            .route("/roles", post(move || {
                let post_mutated = post_mutated.clone();
                async move {
                    *post_mutated.lock().expect("post state") = true;
                    Json(json!({"roleId":"gui-role","versionId":"role-version-1","status":"created"}))
                }
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve role mutation test router");
        });
        format!("http://{addr}")
    }

    fn command_seed() -> GuiCommandSeed {
        GuiCommandSeed {
            action_id: "cmd.rg.audit".to_string(),
            binary_name: "rg".to_string(),
            candidate_paths: vec!["/usr/bin/rg".to_string()],
            starlark_object: "rg".to_string(),
            starlark_method: "run".to_string(),
            argv_prefix: Vec::new(),
            default_cwd: ".".to_string(),
            cwd_policy: "underExecutionRoot".to_string(),
            env_policy: "empty".to_string(),
            sync_allowed: true,
            async_allowed: true,
            max_runtime_ms: None,
            end_of_turn_behavior: "terminate".to_string(),
            stdin_policy: "forbid".to_string(),
            min_await_ms: 0,
            max_await_ms: 60000,
            output_buffer_bytes: 64000,
            terminate_grace_ms: 1000,
            output_limit_bytes: 12000,
            mutation_class: "readOnly".to_string(),
            model_description: "audit".to_string(),
            allow_cwd_arg: true,
            allow_args_arg: true,
            forbidden_args: Vec::new(),
            execution_policy: "allow".to_string(),
        }
    }

    #[tokio::test]
    async fn controller_hydrate_connect_disconnect_and_select_session_state_transitions() {
        let base_url = start_test_server().await;
        let mut controller = GuiBackendController::new();
        let connect = controller.dispatch(GuiOperationRequest::Connect {
            base_url: base_url.clone(),
            selected_session_id: None,
        }).await;
        assert!(matches!(connect.outcome, GuiOperationOutcome::ProjectionUpdated { watermark: 1 }));
        assert_eq!(controller.controller_state().connection_state, GuiConnectionState::Streaming);
        assert_eq!(controller.projection().map(|projection| projection.watermark), Some(1));
        let handle = controller.stream_handle().expect("stream handle after connect");
        assert!(handle.connected);
        assert_eq!(handle.after, 1);
        assert!(handle.url.contains("/state/ws?after=1"));
        assert!(handle.selected_session_id.is_none());

        let selected = Uuid::new_v4().to_string();
        let select = controller.dispatch(GuiOperationRequest::SelectSession {
            session_id: Some(selected.clone()),
        }).await;
        assert!(matches!(select.outcome, GuiOperationOutcome::ProjectionUpdated { watermark: 1 }));
        assert_eq!(controller.controller_state().selected_session_id.as_deref(), Some(selected.as_str()));
        let selected_handle = controller.stream_handle().expect("stream handle after selected reconnect");
        assert!(selected_handle.connected);
        assert_eq!(selected_handle.after, 1);
        assert_eq!(selected_handle.selected_session_id.as_deref(), Some(selected.as_str()));
        assert!(selected_handle.url.contains("after=1"));
        assert!(selected_handle.url.contains(&format!("selectedSessionId={selected}")));

        let disconnect = controller.dispatch(GuiOperationRequest::Disconnect).await;
        assert!(matches!(disconnect.outcome, GuiOperationOutcome::Accepted { .. }));
        assert_eq!(controller.controller_state().connection_state, GuiConnectionState::Disconnected);
        assert!(controller.projection().is_none());
        assert!(controller.stream_handle().is_none());
    }

    #[tokio::test]
    async fn controller_dispatches_server_payloads_and_direct_command_request_summaries() {
        let base_url = start_test_server().await;
        let mut controller = GuiBackendController::new();
        controller.dispatch(GuiOperationRequest::Connect {
            base_url,
            selected_session_id: None,
        }).await;

        let create = controller.dispatch(GuiOperationRequest::CreateSession {
            role: "runtime-allow".to_string(),
            project: Some("__unassigned__".to_string()),
            model: Some("gpt-5.4-mini".to_string()),
            workdir: Some(".".to_string()),
            worktree_root: Some(".".to_string()),
            title: Some("Transport created session".to_string()),
            name: Some("transport-created-session".to_string()),
        }).await;
        assert!(
            matches!(create.outcome, GuiOperationOutcome::Accepted { entity_id: Some(ref id) } if id == "00000000-0000-0000-0000-00000000c001"),
            "unexpected create outcome: {:?}",
            create.outcome
        );

        let send = controller.dispatch(GuiOperationRequest::SendMessage {
            session_id: "session-1".to_string(),
            message: "hello".to_string(),
        }).await;
        assert!(matches!(send.outcome, GuiOperationOutcome::Accepted { entity_id: Some(id) } if id == "turn-1"));

        let list = controller.dispatch(GuiOperationRequest::ListCommandRegistryRequests).await;
        match list.outcome {
            GuiOperationOutcome::CommandRegistryRequests { requests } => {
                assert_eq!(requests.len(), 1);
                assert!(requests[0].can_apply);
                assert_eq!(requests[0].action_id, "cmd.rg.audit");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[tokio::test]
    async fn role_mutations_wait_for_projection_refresh_evidence() {
        let base_url = start_role_mutation_test_server().await;
        let mut controller = GuiBackendController::new();
        let connect = controller.dispatch(GuiOperationRequest::Connect {
            base_url,
            selected_session_id: None,
        }).await;
        assert!(matches!(connect.outcome, GuiOperationOutcome::ProjectionUpdated { watermark: 1 }));
        assert!(controller.projection().expect("initial projection").roles.is_empty());

        let create = controller.dispatch(GuiOperationRequest::CreateRoleFromDraft { draft: role_draft() }).await;
        assert!(matches!(create.outcome, GuiOperationOutcome::ProjectionUpdated { watermark: 2 }));
        let projection = controller.projection().expect("post role mutation projection");
        assert!(projection.roles.iter().any(|role| {
            role.id == "gui-role"
                && role.instruction_text.as_deref() == Some("inline instructions")
                && role.versions.iter().any(|version| version.status == "current")
        }));
    }

    #[tokio::test]
    async fn controller_maps_sync_and_server_errors_to_typed_packets() {
        let mut controller = GuiBackendController::new();
        let missing = controller.dispatch(GuiOperationRequest::CreateSession {
            role: "runtime-allow".to_string(),
            project: None,
            model: None,
            workdir: None,
            worktree_root: None,
            title: None,
            name: None,
        }).await;
        assert!(matches!(missing.outcome, GuiOperationOutcome::Error { ref error } if error.error.code == "conflict"));
        assert!(!controller.controller_state().transient_errors.is_empty());
    }

    #[tokio::test]
    async fn controller_reads_actual_owned_websocket_stream_message() {
        let base_url = start_delta_stream_test_server().await;
        let mut controller = GuiBackendController::new();
        let connect = controller.dispatch(GuiOperationRequest::Connect {
            base_url,
            selected_session_id: None,
        }).await;
        assert!(matches!(connect.outcome, GuiOperationOutcome::ProjectionUpdated { watermark: 1 }));
        let handle = controller.stream_handle().expect("stream handle");
        assert!(handle.connected);
        assert_eq!(handle.after, 1);

        let outcome = controller.next_stream_outcome().await.expect("next stream outcome");
        assert!(matches!(outcome, SyncOutcome::DeltaApplied { apply_outcome: ApplyOutcome::Applied, .. }));
        let projection = controller.projection().expect("projection after stream delta");
        assert_eq!(projection.watermark, 2);
        assert!(projection.sessions.iter().any(|session| session.id == "session-from-websocket"));
        assert_eq!(controller.controller_state().connection_state, GuiConnectionState::Streaming);
    }

    #[test]
    fn registry_decision_payload_uses_server_shape() {
        let request = GuiOperationRequest::PreviewCommandRegistryRequest {
            request_id: "request-1".to_string(),
            decision: CommandRegistryDecisionInput {
                session_id: Some("session-1".to_string()),
                status: "approved".to_string(),
                final_scope: Some(GuiRegistryScope { scope_type: "global".to_string(), project_key: None }),
                final_execution_policy: Some(GuiFinalExecutionPolicy { decision: "allow".to_string(), reason: Some("ok".to_string()) }),
                final_command: Some(command_seed()),
            },
        };
        let payload = request.to_server_request_json().expect("payload");
        assert_eq!(payload["finalScope"]["scopeType"], "global");
        assert_eq!(payload["finalExecutionPolicy"]["decision"], "allow");
        assert_eq!(payload["finalCommand"]["actionId"], "cmd.rg.audit");
        assert_eq!(request.expected_projection_effect(), GuiOperationExpectation::DirectResult);
    }
}
