use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::extract::rejection::JsonRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use robdex_agent_runtime_projection::{RuntimeDelta, RuntimeDeltaKind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::{Mutex, watch};
use tokio::time::{Duration, interval};
use uuid::Uuid;

use crate::roles::DEFAULT_ROLE_ID;
use crate::errors::{RuntimeDomainError, RuntimeErrorKind};
use crate::{approvals, command_registry, db, projection, routing, runtime, starlark_host, workflow_memory};

#[derive(Clone)]
pub struct ServerState {
    pub pool: PgPool,
    pub runtime_identity: String,
    pub active_sends: Arc<Mutex<HashSet<Uuid>>>,
    pub shutdown_tx: watch::Sender<bool>,
    pub shutdown: watch::Receiver<bool>,
}

impl ServerState {
    pub fn new(pool: PgPool) -> Self {
        Self::new_with_identity(pool, format!("robdex-agent-runtime/{}", env!("CARGO_PKG_VERSION")))
    }

    pub fn new_with_identity(pool: PgPool, runtime_identity: String) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self::new_with_shutdown(pool, runtime_identity, shutdown_tx, shutdown_rx)
    }

    pub fn new_with_shutdown(pool: PgPool, runtime_identity: String, shutdown_tx: watch::Sender<bool>, shutdown: watch::Receiver<bool>) -> Self {
        Self {
            pool,
            runtime_identity,
            active_sends: Arc::new(Mutex::new(HashSet::new())),
            shutdown_tx,
            shutdown,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub database: String,
    pub runtime_identity: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    details: Value,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self::from(RuntimeDomainError::bad_request(message))
    }
    fn not_found(entity: &'static str, id: impl ToString) -> Self {
        Self::from(RuntimeDomainError::not_found(entity, id))
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::from(RuntimeDomainError::conflict(message))
    }
    fn unavailable(message: impl Into<String>) -> Self {
        Self::from(RuntimeDomainError::unavailable(message))
    }

    fn new(status: StatusCode, code: &'static str, message: impl Into<String>, details: Value) -> Self {
        Self { status, code, message: message.into(), details }
    }

    fn from_anyhow(error: anyhow::Error) -> Self {
        if let Some(domain) = error.downcast_ref::<RuntimeDomainError>() {
            return Self::from(domain.clone());
        }
        if error.chain().any(is_unavailable_dependency_error) {
            return Self::from(RuntimeDomainError::unavailable("runtime dependency unavailable"));
        }
        Self::from(RuntimeDomainError::internal_safe())
    }
}

impl From<RuntimeDomainError> for ApiError {
    fn from(error: RuntimeDomainError) -> Self {
        let status = match error.kind {
            RuntimeErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            RuntimeErrorKind::NotFound => StatusCode::NOT_FOUND,
            RuntimeErrorKind::Forbidden => StatusCode::FORBIDDEN,
            RuntimeErrorKind::Conflict => StatusCode::CONFLICT,
            RuntimeErrorKind::ValidationFailed => StatusCode::UNPROCESSABLE_ENTITY,
            RuntimeErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            RuntimeErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(status, error.kind.code(), error.message, error.details)
    }
}


fn is_unavailable_dependency_error(error: &(dyn std::error::Error + 'static)) -> bool {
    if let Some(sqlx_error) = error.downcast_ref::<sqlx::Error>() {
        return matches!(
            sqlx_error,
            sqlx::Error::PoolClosed
                | sqlx::Error::PoolTimedOut
                | sqlx::Error::Io(_)
                | sqlx::Error::Tls(_)
                | sqlx::Error::Configuration(_)
        );
    }
    error.downcast_ref::<reqwest::Error>().is_some()
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "error": {
                    "code": self.code,
                    "message": self.message,
                    "details": self.details,
                }
            })),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self::from_anyhow(error)
    }
}


fn is_row_not_found(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| matches!(cause.downcast_ref::<sqlx::Error>(), Some(sqlx::Error::RowNotFound)))
}

fn map_missing_entity(error: anyhow::Error, entity: &'static str, id: impl ToString) -> ApiError {
    if is_row_not_found(&error) {
        ApiError::not_found(entity, id)
    } else {
        ApiError::from(error)
    }
}

fn parse_json<T>(payload: std::result::Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    payload
        .map(|Json(value)| value)
        .map_err(|rejection| ApiError::bad_request(rejection.body_text()))
}

pub fn app(state: ServerState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/state/snapshot", get(snapshot))
        .route("/state/ws", get(state_ws))
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/sessions/{session_id}", get(show_session))
        .route("/sessions/{session_id}/history", get(session_history))
        .route("/sessions/{session_id}/send", post(send_message))
        .route("/sessions/{session_id}/close", post(close_session))
        .route("/sessions/{session_id}/archive", post(archive_session))
        .route("/sessions/{session_id}/fork", post(fork_session))
        .route("/approvals", get(list_approvals))
        .route("/approvals/{approval_id}", get(show_approval))
        .route("/approvals/{approval_id}/decide", post(decide_approval))
        .route("/approvals/{approval_id}/resume", post(resume_approval))
        .route("/roles", get(list_roles))
        .route("/roles/{role_id}", get(show_role))
        .route("/roles/{role_id}/versions", get(role_versions))
        .route("/roles/versions/{version_id}", get(show_role_version))
        .route("/roles/{role_id}/activate", post(activate_role))
        .route("/roles/{role_id}/archive", post(archive_role))
        .route("/roles/{role_id}/unarchive", post(unarchive_role))
        .route("/roles/{role_id}/export", get(export_role))
        .route("/command-registry", get(list_commands))
        .route("/command-registry/{action_id}", get(show_command))
        .route("/command-registry/requests", get(list_registry_requests))
        .route("/command-registry/requests/{request_id}", get(show_registry_request))
        .route("/command-registry/requests/{request_id}/review", get(review_registry_request))
        .route("/command-registry/requests/{request_id}/final-template", get(final_template_registry_request))
        .route("/command-registry/requests/{request_id}/preview-decision", post(preview_registry_decision))
        .route("/command-registry/requests/{request_id}/decide", post(decide_registry_request))
        .route("/command-registry/requests/{request_id}/apply", post(apply_registry_request))
        .route("/workflow-memories", get(list_workflow_memories))
        .route("/workflow-memories/{memory_id}", get(show_workflow_memory))
        .route("/workflow-memories/{memory_id}/events", get(list_workflow_memory_events))
        .route("/workflow-memories/{memory_id}/feedback", post(record_workflow_memory_feedback))
        .with_state(state)
}

pub async fn serve(pool: PgPool, host: &str, port: u16) -> Result<()> {
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app(ServerState::new(pool))).await?;
    Ok(())
}

pub async fn serve_with_shutdown<F>(
    pool: PgPool,
    host: &str,
    port: u16,
    runtime_identity: String,
    shutdown_signal: F,
) -> Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let state = ServerState::new_with_shutdown(pool, runtime_identity, shutdown_tx.clone(), shutdown_rx);
    axum::serve(listener, app(state))
        .with_graceful_shutdown(async move {
            shutdown_signal.await;
            let _ = shutdown_tx.send(true);
        })
        .await?;
    Ok(())
}

async fn health(State(state): State<ServerState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("SELECT 1").execute(&state.pool).await.map_err(|error| {
        ApiError::unavailable(error.to_string())
    })?;
    Ok(Json(HealthResponse {
        status: "ok".to_string(),
        database: "connected".to_string(),
        runtime_identity: state.runtime_identity,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotQuery {
    selected_session_id: Option<Uuid>,
}

async fn snapshot(
    State(state): State<ServerState>,
    Query(query): Query<SnapshotQuery>,
) -> Result<Json<Value>, ApiError> {
    let projection = projection::build_runtime_projection_snapshot(&state.pool, query.selected_session_id).await?;
    Ok(Json(serde_json::to_value(projection).map_err(anyhow::Error::from)?))
}

async fn list_sessions(State(state): State<ServerState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(serde_json::to_value(db::list_sessions(&state.pool, false).await?).map_err(anyhow::Error::from)?))
}

async fn show_session(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(db::show_session(&state.pool, session_id).await.map_err(|error| map_missing_entity(error, "session", session_id))?))
}

async fn session_history(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(db::history_json(&state.pool, session_id).await.map_err(|error| map_missing_entity(error, "session", session_id))?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSessionRequest {
    role: Option<String>,
    project: Option<String>,
    workdir: Option<String>,
    worktree_root: Option<String>,
    title: Option<String>,
    name: Option<String>,
}

async fn create_session(
    State(state): State<ServerState>,
    payload: std::result::Result<Json<CreateSessionRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let role_id = request.role.as_deref().unwrap_or(DEFAULT_ROLE_ID);
    let workdir = request.workdir.as_deref().unwrap_or(".");
    let role = db::current_role_snapshot(&state.pool, role_id).await.map_err(|error| map_missing_entity(error, "role", role_id))?;
    let id = db::new_session(
        &state.pool,
        &role,
        request.project.as_deref(),
        workdir,
        request.worktree_root.as_deref(),
        request.title.as_deref(),
        request.name.as_deref(),
    )
    .await?;
    Ok(Json(json!({"sessionId": id})))
}

#[derive(Debug, Deserialize)]
struct SendRequest {
    message: String,
}

async fn send_message(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
    payload: std::result::Result<Json<SendRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    if request.message.trim().is_empty() {
        return Err(ApiError::bad_request("message must not be empty"));
    }
    {
        let mut active = state.active_sends.lock().await;
        if !active.insert(session_id) {
            return Err(ApiError::conflict("session already has an active send"));
        }
    }
    let result = runtime::send(&state.pool, session_id, &request.message).await;
    state.active_sends.lock().await.remove(&session_id);
    let turn_id = result?;
    Ok(Json(json!({"sessionId": session_id, "turnId": turn_id, "status": "completed"})))
}

#[derive(Debug, Deserialize)]
struct CloseRequest {
    reason: Option<String>,
}

async fn close_session(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
    payload: std::result::Result<Json<CloseRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let live_terminated = starlark_host::terminate_session_processes_for_close(session_id);
    db::close_session(
        &state.pool,
        session_id,
        request.reason.as_deref().unwrap_or("closed by server"),
        live_terminated,
    )
    .await?;
    Ok(Json(json!({"sessionId": session_id, "status": "closed"})))
}

async fn archive_session(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    db::archive_session(&state.pool, session_id).await?;
    Ok(Json(json!({"sessionId": session_id, "tracked": false})))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForkRequest {
    at_turn: Uuid,
}

async fn fork_session(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
    payload: std::result::Result<Json<ForkRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let forked = db::fork_session(&state.pool, session_id, request.at_turn).await?;
    Ok(Json(json!({"sessionId": forked, "forkedFromSessionId": session_id, "forkedFromTurnId": request.at_turn})))
}

async fn list_approvals(State(state): State<ServerState>) -> Result<Json<Value>, ApiError> {
    let approvals = approvals::list(&state.pool).await?;
    Ok(Json(Value::Array(approvals.into_iter().filter(|value| value.get("status").and_then(Value::as_str) == Some("pending")).collect())))
}

async fn show_approval(State(state): State<ServerState>, Path(approval_id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    Ok(Json(approvals::show(&state.pool, approval_id).await.map_err(|error| map_missing_entity(error, "approval", approval_id))?))
}

#[derive(Debug, Deserialize)]
struct DecideApprovalRequest {
    decision: String,
    reason: String,
}

async fn decide_approval(State(state): State<ServerState>, Path(approval_id): Path<Uuid>, payload: std::result::Result<Json<DecideApprovalRequest>, JsonRejection>) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let decision = approvals::ApprovalDecision::try_from(request.decision.as_str())
        .map_err(|error| ApiError::from(RuntimeDomainError::validation_failed(error.to_string())))?;
    approvals::decide(&state.pool, approval_id, decision, &request.reason)
        .await
        .map_err(|error| map_missing_entity(error, "approval", approval_id))?;
    Ok(Json(json!({"approvalId": approval_id, "decision": decision.as_str()})))
}

async fn resume_approval(State(state): State<ServerState>, Path(approval_id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    approvals::resume(&state.pool, approval_id)
        .await
        .map_err(|error| map_missing_entity(error, "approval", approval_id))?;
    Ok(Json(json!({"approvalId": approval_id, "status": "resumed"})))
}

async fn list_roles(State(state): State<ServerState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(Value::Array(db::list_role_records(&state.pool).await?)))
}

async fn show_role(State(state): State<ServerState>, Path(role_id): Path<String>) -> Result<Json<Value>, ApiError> {
    Ok(Json(serde_json::to_value(db::current_role_snapshot(&state.pool, &role_id).await.map_err(|error| map_missing_entity(error, "role", &role_id))?).map_err(anyhow::Error::from)?))
}

async fn role_versions(State(state): State<ServerState>, Path(role_id): Path<String>) -> Result<Json<Value>, ApiError> {
    let versions = db::role_versions(&state.pool, &role_id).await?;
    if versions.is_empty() && db::current_role_snapshot(&state.pool, &role_id).await.is_err() {
        return Err(ApiError::not_found("role", &role_id));
    }
    Ok(Json(serde_json::to_value(versions).map_err(anyhow::Error::from)?))
}

async fn show_role_version(State(state): State<ServerState>, Path(version_id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    Ok(Json(serde_json::to_value(db::role_version_snapshot(&state.pool, version_id).await.map_err(|error| map_missing_entity(error, "role_version", version_id))?).map_err(anyhow::Error::from)?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivateRoleRequest {
    version_id: Uuid,
}

async fn activate_role(State(state): State<ServerState>, Path(role_id): Path<String>, payload: std::result::Result<Json<ActivateRoleRequest>, JsonRejection>) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let snapshot = db::role_version_snapshot(&state.pool, request.version_id).await.map_err(|error| map_missing_entity(error, "role_version", request.version_id))?;
    if snapshot.id != role_id {
        return Err(ApiError::from(RuntimeDomainError::validation_failed(format!("role version {} belongs to {}, not {}", request.version_id, snapshot.id, role_id))));
    }
    routing::validate_snapshot_routing_against_db(&state.pool, &snapshot).await.map_err(|error| ApiError::from(RuntimeDomainError::validation_failed(error.to_string())))?;
    command_registry::validate_policy_actions_exist(&state.pool, snapshot.policy.keys().cloned()).await.map_err(|error| ApiError::from(RuntimeDomainError::validation_failed(error.to_string())))?;
    db::activate_role_version(&state.pool, &role_id, request.version_id).await?;
    Ok(Json(json!({"roleId": role_id, "versionId": request.version_id, "status": "active"})))
}

async fn archive_role(State(state): State<ServerState>, Path(role_id): Path<String>) -> Result<Json<Value>, ApiError> {
    db::current_role_snapshot(&state.pool, &role_id).await.map_err(|error| map_missing_entity(error, "role", &role_id))?;
    db::archive_role(&state.pool, &role_id).await.map_err(|error| map_missing_entity(error, "role", &role_id))?;
    Ok(Json(json!({"roleId": role_id, "status": "archived"})))
}

async fn unarchive_role(State(state): State<ServerState>, Path(role_id): Path<String>) -> Result<Json<Value>, ApiError> {
    let role_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM roles WHERE id=$1)")
        .bind(&role_id)
        .fetch_one(&state.pool)
        .await
        .map_err(anyhow::Error::from)?;
    if !role_exists {
        return Err(ApiError::not_found("role", &role_id));
    }
    db::unarchive_role(&state.pool, &role_id).await.map_err(|error| {
        if is_row_not_found(&error) {
            ApiError::not_found("role", &role_id)
        } else {
            ApiError::conflict(error.to_string())
        }
    })?;
    Ok(Json(json!({"roleId": role_id, "status": "active"})))
}

async fn export_role(State(state): State<ServerState>, Path(role_id): Path<String>) -> Result<Json<Value>, ApiError> {
    Ok(Json(db::export_role(&state.pool, &role_id).await.map_err(|error| map_missing_entity(error, "role", &role_id))?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandRegistryQuery {
    session_id: Option<Uuid>,
    project: Option<String>,
}

async fn command_registry_project_key(pool: &PgPool, query: &CommandRegistryQuery) -> Result<Option<String>, ApiError> {
    if let Some(session_id) = query.session_id {
        return Ok(db::session_project_key(pool, session_id).await.map_err(|error| map_missing_entity(error, "session", session_id))?);
    }
    Ok(query.project.clone())
}

async fn list_commands(State(state): State<ServerState>, Query(query): Query<CommandRegistryQuery>) -> Result<Json<Value>, ApiError> {
    if query.session_id.is_some() || query.project.is_some() {
        let project_key = command_registry_project_key(&state.pool, &query).await?;
        Ok(Json(serde_json::to_value(command_registry::list_visible(&state.pool, project_key.as_deref()).await?).map_err(anyhow::Error::from)?))
    } else {
        Ok(Json(serde_json::to_value(command_registry::list(&state.pool).await?).map_err(anyhow::Error::from)?))
    }
}

async fn show_command(State(state): State<ServerState>, Query(query): Query<CommandRegistryQuery>, Path(action_id): Path<String>) -> Result<Json<Value>, ApiError> {
    if query.session_id.is_some() || query.project.is_some() {
        let project_key = command_registry_project_key(&state.pool, &query).await?;
        Ok(Json(command_registry::show_visible(&state.pool, &action_id, project_key.as_deref()).await.map_err(|error| map_missing_entity(error, "command", &action_id))?))
    } else {
        Ok(Json(command_registry::show(&state.pool, &action_id).await.map_err(|error| map_missing_entity(error, "command", &action_id))?))
    }
}

async fn list_registry_requests(State(state): State<ServerState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(serde_json::to_value(command_registry::list_requests(&state.pool).await?).map_err(anyhow::Error::from)?))
}

async fn show_registry_request(State(state): State<ServerState>, Path(request_id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    Ok(Json(command_registry::show_request(&state.pool, request_id).await.map_err(|error| map_missing_entity(error, "command_registry_request", request_id))?))
}

async fn review_registry_request(State(state): State<ServerState>, Path(request_id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    Ok(Json(command_registry::review_request(&state.pool, request_id).await.map_err(|error| map_missing_entity(error, "command_registry_request", request_id))?))
}

async fn final_template_registry_request(State(state): State<ServerState>, Path(request_id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    Ok(Json(command_registry::final_template(&state.pool, request_id).await.map_err(|error| map_missing_entity(error, "command_registry_request", request_id))?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryDecisionRequest {
    session_id: Option<Uuid>,
    status: String,
    final_scope: Option<command_registry::RegistryScope>,
    final_execution_policy: Option<command_registry::FinalExecutionPolicy>,
    final_command: Option<command_registry::CommandSeed>,
}

async fn preview_registry_decision(State(state): State<ServerState>, Path(request_id): Path<Uuid>, payload: std::result::Result<Json<RegistryDecisionRequest>, JsonRejection>) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    Ok(Json(command_registry::preview_decision(&state.pool, request_id, &request.status, request.final_scope, request.final_execution_policy, request.final_command).await.map_err(|error| map_missing_entity(error, "command_registry_request", request_id))?))
}

async fn decide_registry_request(State(state): State<ServerState>, Path(request_id): Path<Uuid>, payload: std::result::Result<Json<RegistryDecisionRequest>, JsonRejection>) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let session_id = request.session_id.ok_or_else(|| ApiError::bad_request("sessionId is required"))?;
    command_registry::decide_request(&state.pool, session_id, request_id, &request.status, request.final_scope, request.final_execution_policy, request.final_command).await.map_err(|error| map_missing_entity(error, "command_registry_request", request_id))?;
    Ok(Json(json!({"requestId": request_id, "status": request.status})))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryApplyRequest {
    session_id: Uuid,
}

async fn apply_registry_request(State(state): State<ServerState>, Path(request_id): Path<Uuid>, payload: std::result::Result<Json<RegistryApplyRequest>, JsonRejection>) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    command_registry::apply_request(&state.pool, request.session_id, request_id).await.map_err(|error| map_missing_entity(error, "command_registry_request", request_id))?;
    Ok(Json(json!({"requestId": request_id, "status": "applied"})))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowMemoryQuery {
    session_id: Uuid,
}

async fn list_workflow_memories(State(state): State<ServerState>, Query(query): Query<WorkflowMemoryQuery>) -> Result<Json<Value>, ApiError> {
    Ok(Json(Value::Array(workflow_memory::list_visible_memories(&state.pool, query.session_id).await.map_err(|error| map_missing_entity(error, "session", query.session_id))?)))
}

async fn show_workflow_memory(State(state): State<ServerState>, Query(query): Query<WorkflowMemoryQuery>, Path(memory_id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    if !workflow_memory::memory_exists(&state.pool, memory_id).await? {
        return Err(ApiError::not_found("workflow_memory", memory_id));
    }
    Ok(Json(workflow_memory::show_visible_memory(&state.pool, query.session_id, memory_id).await?))
}

async fn list_workflow_memory_events(State(state): State<ServerState>, Query(query): Query<WorkflowMemoryQuery>, Path(memory_id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    if !workflow_memory::memory_exists(&state.pool, memory_id).await? {
        return Err(ApiError::not_found("workflow_memory", memory_id));
    }
    Ok(Json(Value::Array(workflow_memory::list_memory_events(&state.pool, query.session_id, memory_id).await?)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowMemoryFeedbackRequest {
    session_id: Uuid,
    feedback: String,
    #[serde(default)]
    payload: Value,
}

async fn record_workflow_memory_feedback(State(state): State<ServerState>, Path(memory_id): Path<Uuid>, payload: std::result::Result<Json<WorkflowMemoryFeedbackRequest>, JsonRejection>) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    if !workflow_memory::memory_exists(&state.pool, memory_id).await? {
        return Err(ApiError::not_found("workflow_memory", memory_id));
    }
    workflow_memory::record_visible_feedback(&state.pool, request.session_id, memory_id, &request.feedback, request.payload).await?;
    Ok(Json(json!({"memoryId": memory_id, "feedback": request.feedback, "status": "recorded"})))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WsQuery {
    after: Option<i64>,
    selected_session_id: Option<Uuid>,
}

async fn state_ws(
    State(state): State<ServerState>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| state_ws_loop(state, query, socket))
}

async fn state_ws_loop(state: ServerState, query: WsQuery, socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let mut receiver_shutdown = state.shutdown.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = receiver_shutdown.changed() => break,
                next = receiver.next() => {
                    if next.is_none() {
                        break;
                    }
                }
            }
        }
    });
    let mut sender_shutdown = state.shutdown.clone();
    let current = projection::build_runtime_projection_snapshot(&state.pool, query.selected_session_id)
        .await
        .map(|snapshot| snapshot.watermark)
        .unwrap_or(0);
    if sender
        .send(Message::Text(
            json!({"type": "hello", "watermark": current, "runtimeIdentity": state.runtime_identity})
                .to_string()
                .into(),
        ))
        .await
        .is_err()
    {
        return;
    }
    let Some(mut last) = query.after else {
        let delta = RuntimeDelta {
            watermark: current,
            previous_watermark: None,
            kind: RuntimeDeltaKind::ResyncRequired {
                reason: "missing after watermark; hydrate a snapshot before opening the delta stream".to_string(),
            },
        };
        let _ = sender
            .send(Message::Text(json!({"type": "resyncRequired", "delta": delta}).to_string().into()))
            .await;
        return;
    };
    match projection::event_stream_can_continue_after(&state.pool, last, query.selected_session_id).await {
        Ok(true) => {}
        _ => {
            let delta = RuntimeDelta {
                watermark: current,
                previous_watermark: None,
                kind: RuntimeDeltaKind::ResyncRequired {
                    reason: "requested after watermark is stale, missing, or cannot be continued safely".to_string(),
                },
            };
            let _ = sender
                .send(Message::Text(json!({"type": "resyncRequired", "delta": delta}).to_string().into()))
                .await;
            return;
        }
    }
    let mut tick = interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            _ = sender_shutdown.changed() => {
                let _ = sender.send(Message::Text(json!({"type":"serverShutdown","runtimeIdentity": state.runtime_identity}).to_string().into())).await;
                let _ = sender.send(Message::Close(None)).await;
                return;
            }
            _ = tick.tick() => {}
        }
        let deltas = match projection::build_projection_deltas_after(&state.pool, last, query.selected_session_id, 100).await {
            Ok(deltas) => deltas,
            Err(error) => {
                let delta = RuntimeDelta {
                    watermark: last,
                    previous_watermark: None,
                    kind: RuntimeDeltaKind::ResyncRequired {
                        reason: format!("delta query failed: {error}"),
                    },
                };
                let _ = sender
                    .send(Message::Text(json!({"type": "resyncRequired", "delta": delta}).to_string().into()))
                    .await;
                return;
            }
        };
        if std::env::var("ROBDEX_AGENT_RUNTIME_SERVER_TRACE_WS").ok().as_deref() == Some("1") && !deltas.is_empty() {
            eprintln!("[server-ws] after={last} selected={:?} deltas={}", query.selected_session_id, deltas.len());
        }
        for delta in deltas {
            last = delta.watermark;
            if sender
                .send(Message::Text(json!({"type": "delta", "delta": delta}).to_string().into()))
                .await
                .is_err()
            {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use crate::gui_sync::{RuntimeSyncClient, RuntimeSyncConfig, SyncOutcome};
    use robdex_agent_runtime_projection::{RuntimeDeltaKind, RuntimeProjection};
    use tokio_tungstenite::connect_async;
    use tower::ServiceExt;

    struct TestDb {
        pool: PgPool,
        admin: PgPool,
        name: String,
    }

    impl TestDb {
        async fn cleanup(self) {
            self.pool.close().await;
            sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, self.name))
                .execute(&self.admin)
                .await
                .expect("drop validation db");
            self.admin.close().await;
        }
    }

    async fn validation_db() -> TestDb {
        let admin_url = std::env::var("ROBDEX_AGENT_RUNTIME_VALIDATION_ADMIN_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres@127.0.0.1:5432/postgres".to_string());
        let name = format!(
            "robdex_agent_runtime_validation_{}",
            Uuid::new_v4().to_string().replace('-', "")
        );
        let admin = db::connect(&admin_url).await.expect("admin postgres");
        sqlx::query(&format!(r#"CREATE DATABASE "{name}""#)).execute(&admin).await.expect("create db");
        let url = format!("{}/{}", admin_url.rsplit_once('/').map(|(base, _)| base).unwrap_or(&admin_url), name);
        let pool = db::connect(&url).await.expect("runtime db");
        db::init(&pool).await.expect("init db");
        let registry = crate::roles::RoleRegistry::default_for_workspace().expect("registry");
        for path in registry.manifest_paths().expect("role paths") {
            let imported = registry.load_for_import(&path).expect("load role");
            db::import_role_version(&pool, &imported).await.expect("import role");
        }
        TestDb { pool, admin, name }
    }

    async fn request_json(app: Router, method: Method, path: &str, body: Value) -> (StatusCode, Value) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
        let value = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("json response parse failed: status={status}, error={error}, body={}", String::from_utf8_lossy(&bytes))) };
        (status, value)
    }

    async fn request_raw(app: Router, method: Method, path: &str, body: &str) -> (StatusCode, Value) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
        let value = serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("json response parse failed: status={status}, error={error}, body={}", String::from_utf8_lossy(&bytes)));
        (status, value)
    }

    fn assert_api_error(value: &Value, code: &str) {
        assert_eq!(value["error"]["code"], code);
        assert!(value["error"]["message"].as_str().is_some());
        assert!(value["error"]["details"].is_object());
    }

    async fn next_delta(ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>) -> RuntimeDelta {
        loop {
            let message = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next())
                .await
                .expect("timed out waiting for websocket delta")
                .expect("ws next")
                .expect("ws message");
            let text = message.into_text().expect("text");
            let value: Value = serde_json::from_str(&text).expect("json");
            if value["type"] == "delta" {
                return serde_json::from_value(value["delta"].clone()).expect("runtime delta");
            }
            if value["type"] == "resyncRequired" {
                panic!("websocket returned resyncRequired while waiting for delta: {value}");
            }
        }
    }

    async fn apply_until<F>(
        ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        projection: &mut RuntimeProjection,
        mut predicate: F,
    ) where
        F: FnMut(&RuntimeDelta, &RuntimeProjection) -> bool,
    {
        for _ in 0..40 {
            let delta = next_delta(ws).await;
            projection.apply_delta(delta.clone());
            if predicate(&delta, projection) {
                return;
            }
        }
        panic!("predicate was not satisfied by websocket deltas");
    }

    #[tokio::test]
    async fn deterministic_http_health_snapshot_session_lifecycle_and_conflict() {
        let test_db = validation_db().await;
        let state = ServerState::new(test_db.pool.clone());
        let router = app(state.clone());

        let response = router
            .clone()
            .oneshot(Request::builder().uri("/health").body(Body::empty()).expect("request"))
            .await
            .expect("health");
        assert_eq!(response.status(), StatusCode::OK);

        let (_, created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"server-validation","workdir":"."}),
        )
        .await;
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");

        let response = router
            .clone()
            .oneshot(Request::builder().uri(format!("/state/snapshot?selectedSessionId={session_id}")).body(Body::empty()).expect("request"))
            .await
            .expect("snapshot");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("snapshot body");
        let snapshot: Value = serde_json::from_slice(&bytes).expect("snapshot json");
        assert_eq!(snapshot["selectedSession"]["id"], session_id.to_string());

        let show_response = router
            .clone()
            .oneshot(Request::builder().uri(format!("/sessions/{session_id}")).body(Body::empty()).expect("request"))
            .await
            .expect("show");
        assert_eq!(show_response.status(), StatusCode::OK);

        let turn_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO turns (id, session_id, role, input_text, status, completed_at)
            VALUES ($1, $2, 'user', 'deterministic history', 'completed', now())
            "#,
        )
        .bind(turn_id)
        .bind(session_id)
        .execute(&state.pool)
        .await
        .expect("insert completed turn");
        db::append_event(
            &state.pool,
            session_id,
            Some(turn_id),
            "turn",
            Some(turn_id),
            "turn.completed",
            Some("completed"),
            json!({}),
        )
        .await
        .expect("append completed turn event");

        let history_response = router
            .clone()
            .oneshot(Request::builder().uri(format!("/sessions/{session_id}/history")).body(Body::empty()).expect("request"))
            .await
            .expect("history");
        assert_eq!(history_response.status(), StatusCode::OK);

        let (status, forked) = request_json(
            router.clone(),
            Method::POST,
            &format!("/sessions/{session_id}/fork"),
            json!({"atTurn": turn_id}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_ne!(forked["sessionId"], session_id.to_string());

        state.active_sends.lock().await.insert(session_id);
        let (status, conflict) = request_json(
            router.clone(),
            Method::POST,
            &format!("/sessions/{session_id}/send"),
            json!({"message":"conflict"}),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_api_error(&conflict, "conflict");
        assert!(conflict["error"]["message"].as_str().unwrap_or_default().contains("active send"));
        state.active_sends.lock().await.remove(&session_id);

        let (status, archived) = request_json(
            router.clone(),
            Method::POST,
            &format!("/sessions/{session_id}/archive"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(archived["tracked"], false);

        let (status, closed) = request_json(
            router,
            Method::POST,
            &format!("/sessions/{session_id}/close"),
            json!({"reason":"server validation"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(closed["status"], "closed");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn deterministic_websocket_streams_event_stream_delta_after_database_change() {
        let test_db = validation_db().await;
        let state = ServerState::new(test_db.pool.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let after = projection::build_runtime_projection_snapshot(&test_db.pool, None)
            .await
            .expect("snapshot")
            .watermark;

        let (mut ws, _) = connect_async(format!("ws://{addr}/state/ws?after={after}"))
            .await
            .expect("connect ws");
        let hello = ws.next().await.expect("hello").expect("hello message");
        let hello_text = hello.into_text().expect("hello text");
        assert_eq!(serde_json::from_str::<Value>(&hello_text).expect("hello json")["type"], "hello");

        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg")
            .await
            .expect("role");
        let session_id = db::new_session(
            &test_db.pool,
            &role,
            Some("ws-validation"),
            ".",
            Some("."),
            Some("WebSocket validation"),
            Some("ws-validation"),
        )
        .await
        .expect("new session");

        let mut saw_created = false;
        for _ in 0..20 {
            let Some(message) = ws.next().await else { break; };
            let text = message.expect("ws message").into_text().expect("text");
            let value: Value = serde_json::from_str(&text).expect("delta json");
            if value["type"] == "delta"
                && value["delta"]["type"] == "timelineAppend"
                && value["delta"]["item"]["eventType"] == "session.created"
                && value["delta"]["item"]["sessionId"] == session_id.to_string()
            {
                assert!(value["delta"]["watermark"].as_i64().unwrap_or_default() > after);
                saw_created = true;
                break;
            }
        }
        assert!(saw_created, "websocket did not stream session.created event_stream delta");
        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn deterministic_websocket_semantic_session_deltas_update_snapshot_projection() {
        let test_db = validation_db().await;
        let mut client_projection = projection::build_runtime_projection_snapshot(&test_db.pool, None)
            .await
            .expect("initial snapshot");
        let after = client_projection.watermark;
        let state = ServerState::new(test_db.pool.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let (mut ws, _) = connect_async(format!("ws://{addr}/state/ws?after={after}"))
            .await
            .expect("connect ws");
        let _hello = ws.next().await.expect("hello").expect("hello message");

        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg")
            .await
            .expect("role");
        let session_id = db::new_session(
            &test_db.pool,
            &role,
            Some("semantic-session"),
            ".",
            Some("."),
            Some("Semantic session"),
            Some("semantic-session"),
        )
        .await
        .expect("new session");

        let mut saw_timeline_for_create = false;
        apply_until(&mut ws, &mut client_projection, |delta, projection| {
            saw_timeline_for_create |= matches!(&delta.kind, RuntimeDeltaKind::TimelineAppend { item } if item.event_type == "session.created" && item.session_id.as_deref() == Some(&session_id.to_string()));
            projection.sessions.iter().any(|session| session.id == session_id.to_string())
        })
        .await;
        assert!(saw_timeline_for_create);

        db::archive_session(&test_db.pool, session_id).await.expect("archive");
        apply_until(&mut ws, &mut client_projection, |_delta, projection| {
            projection.sessions.iter().all(|session| session.id != session_id.to_string())
        })
        .await;

        db::close_session(&test_db.pool, session_id, "semantic close", 0)
            .await
            .expect("close");
        apply_until(&mut ws, &mut client_projection, |_delta, projection| {
            projection
                .sessions
                .iter()
                .any(|session| session.id == session_id.to_string() && session.status == "closed")
        })
        .await;

        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn deterministic_websocket_semantic_approval_deltas_update_snapshot_projection() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg")
            .await
            .expect("role");
        let session_id = db::new_session(
            &test_db.pool,
            &role,
            Some("semantic-approval"),
            ".",
            Some("."),
            Some("Semantic approval"),
            Some("semantic-approval"),
        )
        .await
        .expect("new session");
        let mut client_projection = projection::build_runtime_projection_snapshot(&test_db.pool, Some(session_id))
            .await
            .expect("initial snapshot");
        let after = client_projection.watermark;
        let state = ServerState::new(test_db.pool.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let (mut ws, _) = connect_async(format!("ws://{addr}/state/ws?after={after}&selectedSessionId={session_id}"))
            .await
            .expect("connect ws");
        let _hello = ws.next().await.expect("hello").expect("hello message");

        let approval_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO approval_requests (
                id, session_id, turn_id, action_name, requested_by_role, input_context,
                required_approver_kind, status
            ) VALUES ($1, $2, NULL, 'fs.write', '{}'::jsonb, '{"action":"fs.write"}'::jsonb, 'owner', 'pending')
            "#,
        )
        .bind(approval_id)
        .bind(session_id)
        .execute(&test_db.pool)
        .await
        .expect("insert approval");
        db::append_event(
            &test_db.pool,
            session_id,
            None,
            "approval",
            Some(approval_id),
            "approval.requested",
            Some("pending"),
            json!({"requestId": approval_id, "action": "fs.write"}),
        )
        .await
        .expect("approval requested event");

        let mut saw_timeline_for_approval = false;
        apply_until(&mut ws, &mut client_projection, |delta, projection| {
            saw_timeline_for_approval |= matches!(&delta.kind, RuntimeDeltaKind::TimelineAppend { item } if item.event_type == "approval.requested");
            projection.pending_approvals.iter().any(|approval| approval.id == approval_id.to_string())
        })
        .await;
        assert!(saw_timeline_for_approval);

        sqlx::query("UPDATE approval_requests SET status = 'denied', completed_at = now() WHERE id = $1")
            .bind(approval_id)
            .execute(&test_db.pool)
            .await
            .expect("deny approval");
        db::append_event(
            &test_db.pool,
            session_id,
            None,
            "approval",
            Some(approval_id),
            "approval.decided",
            Some("denied"),
            json!({"requestId": approval_id, "decision": "denied"}),
        )
        .await
        .expect("approval decided event");
        apply_until(&mut ws, &mut client_projection, |_delta, projection| {
            projection.pending_approvals.iter().all(|approval| approval.id != approval_id.to_string())
        })
        .await;

        server.abort();
        test_db.cleanup().await;
    }


    #[tokio::test]
    async fn gui_sync_client_hydrates_streams_non_selected_deltas_and_converges_with_snapshot() {
        let test_db = validation_db().await;
        let state = ServerState::new(test_db.pool.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let base_url = format!("http://{addr}");
        let mut sync = RuntimeSyncClient::new(RuntimeSyncConfig::new(base_url.clone()));
        let initial_watermark = sync.hydrate().await.expect("hydrate").watermark;
        let mut stream = sync.connect_after(Some(initial_watermark)).await.expect("connect after snapshot");
        assert!(matches!(stream.next_outcome(&mut sync).await.expect("hello"), SyncOutcome::Hello { .. }));

        let http = reqwest::Client::new();
        let created: Value = http
            .post(format!("{base_url}/sessions"))
            .json(&json!({"role":"runtime-no-rg","project":"gui-sync","workdir":"."}))
            .send()
            .await
            .expect("create session response")
            .error_for_status()
            .expect("create session ok")
            .json()
            .await
            .expect("create session json");
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");

        let mut saw_session_upsert = false;
        let mut saw_timeline_append = false;
        for _ in 0..80 {
            match stream.next_outcome(&mut sync).await.expect("delta outcome") {
                SyncOutcome::DeltaApplied { delta, .. } => {
                    saw_session_upsert |= matches!(&delta.kind, RuntimeDeltaKind::SessionUpsert { session } if session.id == session_id.to_string());
                    saw_timeline_append |= matches!(&delta.kind, RuntimeDeltaKind::TimelineAppend { item } if item.event_type == "session.created" && item.session_id.as_deref() == Some(&session_id.to_string()));
                    if saw_session_upsert && saw_timeline_append {
                        break;
                    }
                }
                other => panic!("unexpected sync outcome while waiting for session create deltas: {other:?}"),
            }
        }
        assert!(saw_session_upsert, "GUI sync client did not apply session upsert delta");
        assert!(saw_timeline_append, "GUI sync client did not receive timeline append delta");
        assert!(sync.projection().expect("projection").sessions.iter().any(|session| session.id == session_id.to_string()));

        let fresh: RuntimeProjection = http
            .get(format!("{base_url}/state/snapshot"))
            .send()
            .await
            .expect("fresh snapshot response")
            .error_for_status()
            .expect("fresh snapshot ok")
            .json()
            .await
            .expect("fresh snapshot json");
        assert_eq!(
            sync.projection().expect("projection").sessions.iter().any(|session| session.id == session_id.to_string()),
            fresh.sessions.iter().any(|session| session.id == session_id.to_string())
        );
        assert_eq!(
            sync.projection().expect("projection").timeline.iter().any(|item| item.event_type == "session.created" && item.session_id.as_deref() == Some(&session_id.to_string())),
            fresh.timeline.iter().any(|item| item.event_type == "session.created" && item.session_id.as_deref() == Some(&session_id.to_string()))
        );

        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn gui_sync_client_preserves_selected_detail_and_applies_selected_timeline_delta() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg")
            .await
            .expect("role");
        let session_id = db::new_session(
            &test_db.pool,
            &role,
            Some("gui-selected"),
            ".",
            Some("."),
            Some("GUI selected"),
            Some("gui-selected"),
        )
        .await
        .expect("new session");
        let state = ServerState::new(test_db.pool.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let base_url = format!("http://{addr}");
        let mut sync = RuntimeSyncClient::new(RuntimeSyncConfig::new(base_url.clone()).with_selected_session(session_id));
        let initial = sync.hydrate().await.expect("hydrate selected").watermark;
        assert_eq!(sync.projection().expect("projection").selected_session.as_ref().map(|session| session.id.as_str()), Some(session_id.to_string().as_str()));
        let mut stream = sync.connect_after(Some(initial)).await.expect("connect selected");
        assert!(matches!(stream.next_outcome(&mut sync).await.expect("hello"), SyncOutcome::Hello { .. }));

        let http = reqwest::Client::new();
        http.post(format!("{base_url}/sessions/{session_id}/archive"))
            .json(&json!({}))
            .send()
            .await
            .expect("archive response")
            .error_for_status()
            .expect("archive ok");

        let mut saw_archive_timeline = false;
        let mut saw_archive_semantic = false;
        for _ in 0..80 {
            match stream.next_outcome(&mut sync).await.expect("selected delta outcome") {
                SyncOutcome::DeltaApplied { delta, .. } => {
                    saw_archive_timeline |= matches!(&delta.kind, RuntimeDeltaKind::TimelineAppend { item } if item.event_type == "session.archived" && item.session_id.as_deref() == Some(&session_id.to_string()));
                    saw_archive_semantic |= matches!(&delta.kind, RuntimeDeltaKind::SessionArchive { session_id: delta_session_id, .. } if delta_session_id == &session_id.to_string());
                    if saw_archive_timeline && saw_archive_semantic {
                        break;
                    }
                }
                other => panic!("unexpected selected sync outcome: {other:?}"),
            }
        }
        assert!(saw_archive_timeline, "selected stream did not deliver timeline append");
        assert!(saw_archive_semantic, "selected stream did not deliver semantic archive delta");
        let projection = sync.projection().expect("projection");
        assert_eq!(projection.selected_session.as_ref().map(|session| session.id.as_str()), Some(session_id.to_string().as_str()));
        assert!(projection.timeline.iter().any(|item| item.event_type == "session.archived" && item.session_id.as_deref() == Some(&session_id.to_string())));

        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn gui_sync_client_marks_resync_rehydrates_and_handles_shutdown_message() {
        let test_db = validation_db().await;
        let state = ServerState::new(test_db.pool.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let base_url = format!("http://{addr}");
        let mut sync = RuntimeSyncClient::new(RuntimeSyncConfig::new(base_url.clone()));
        sync.hydrate().await.expect("hydrate");

        let mut omitted_after_stream = sync.connect_after(None).await.expect("connect without after");
        assert!(matches!(omitted_after_stream.next_outcome(&mut sync).await.expect("hello"), SyncOutcome::Hello { .. }));
        assert!(matches!(omitted_after_stream.next_outcome(&mut sync).await.expect("resync"), SyncOutcome::ResyncRequired { .. }));
        assert!(sync.resync_required());
        assert!(sync.projection().expect("projection").resync_required.is_some());
        let rehydrated_watermark = sync.rehydrate().await.expect("rehydrate after resync").watermark;
        assert!(!sync.resync_required());
        assert!(sync.projection().expect("projection").resync_required.is_none());

        let mut valid_stream = sync.connect_after(Some(rehydrated_watermark)).await.expect("connect after rehydrate");
        assert!(matches!(valid_stream.next_outcome(&mut sync).await.expect("hello"), SyncOutcome::Hello { .. }));
        let http = reqwest::Client::new();
        let created: Value = http
            .post(format!("{base_url}/sessions"))
            .json(&json!({"role":"runtime-no-rg","project":"gui-sync-resync","workdir":"."}))
            .send()
            .await
            .expect("create response")
            .error_for_status()
            .expect("create ok")
            .json()
            .await
            .expect("create json");
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");
        for _ in 0..80 {
            match valid_stream.next_outcome(&mut sync).await.expect("post rehydrate outcome") {
                SyncOutcome::DeltaApplied { delta, .. } if matches!(&delta.kind, RuntimeDeltaKind::SessionUpsert { session } if session.id == session_id.to_string()) => break,
                SyncOutcome::DeltaApplied { .. } => {}
                other => panic!("unexpected post-rehydrate sync outcome: {other:?}"),
            }
        }
        assert!(sync.projection().expect("projection").sessions.iter().any(|session| session.id == session_id.to_string()));

        let before_shutdown_projection = sync.projection().expect("projection").clone();
        let outcome = sync
            .handle_server_message_value(json!({"type":"serverShutdown","runtimeIdentity":"test-runtime"}))
            .expect("shutdown message");
        assert!(matches!(outcome, SyncOutcome::ServerShutdown));
        assert!(sync.server_shutdown_seen());
        assert_eq!(sync.projection().expect("projection"), &before_shutdown_projection);

        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn deterministic_admin_approval_routes_preserve_db_events() {
        let test_db = validation_db().await;
        let router = app(ServerState::new(test_db.pool.clone()));
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("admin-approval"), ".", Some("."), None, None).await.expect("session");
        let approval_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO approval_requests (
                id, session_id, action_name, requested_by_role, input_context, required_approver_kind, status
            ) VALUES ($1, $2, 'fs.write', '{}'::jsonb, '{"action":"fs.write"}'::jsonb, 'owner', 'pending')
            "#,
        )
        .bind(approval_id)
        .bind(session_id)
        .execute(&test_db.pool)
        .await
        .expect("insert approval");
        db::append_event(&test_db.pool, session_id, None, "approval", Some(approval_id), "approval.requested", Some("pending"), json!({"requestId": approval_id})).await.expect("event");

        let response = router.clone().oneshot(Request::builder().uri("/approvals").body(Body::empty()).expect("request")).await.expect("list approvals");
        assert_eq!(response.status(), StatusCode::OK);
        let response = router.clone().oneshot(Request::builder().uri(format!("/approvals/{approval_id}")).body(Body::empty()).expect("request")).await.expect("show approval");
        assert_eq!(response.status(), StatusCode::OK);
        let (status, decided) = request_json(router, Method::POST, &format!("/approvals/{approval_id}/decide"), json!({"decision":"denied","reason":"deterministic admin validation"})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(decided["decision"], "denied");
        let decided_events: i64 = sqlx::query_scalar("SELECT count(*) FROM event_stream WHERE entity_id=$1 AND event_type='approval.decided' AND status='denied'")
            .bind(approval_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("decided events");
        assert_eq!(decided_events, 1);

        let resume_approval_id = Uuid::new_v4();
        let turn_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status) VALUES ($1,$2,'user','resume','completed')")
            .bind(turn_id)
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("resume turn");
        let tool_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status) VALUES ($1,$2,$3,'execute_code','resume-call','{}'::jsonb,'completed')")
            .bind(tool_id)
            .bind(session_id)
            .bind(turn_id)
            .execute(&test_db.pool)
            .await
            .expect("resume tool");
        let script_id = Uuid::new_v4();
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status) VALUES ($1,$2,'fs.write(\"resume.txt\", \"ok\")','completed')")
            .bind(script_id)
            .bind(tool_id)
            .execute(&test_db.pool)
            .await
            .expect("resume script");
        sqlx::query(
            r#"
            INSERT INTO approval_requests (
                id, session_id, turn_id, action_name, requested_by_role, input_context, required_approver_kind, status
            ) VALUES ($1, $2, $3, 'fs.write', '{}'::jsonb, '{"action":"fs.write"}'::jsonb, 'owner', 'pending')
            "#,
        )
        .bind(resume_approval_id)
        .bind(session_id)
        .bind(turn_id)
        .execute(&test_db.pool)
        .await
        .expect("insert resumable approval");
        let temp_root = std::env::temp_dir().join(format!("robdex-agent-runtime-resume-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).expect("resume temp root");
        let paused_id = approvals::create_paused_action(
            &test_db.pool,
            resume_approval_id,
            session_id,
            Some(turn_id),
            Some(tool_id),
            Some(script_id),
            "fs.write",
            json!({"path":"resume.txt","content":"resumed","executionRoot": temp_root.display().to_string()}),
            &role,
        )
        .await
        .expect("paused action");
        let (status, _) = request_json(
            app(ServerState::new(test_db.pool.clone())),
            Method::POST,
            &format!("/approvals/{resume_approval_id}/decide"),
            json!({"decision":"approved","reason":"deterministic resume validation"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = request_json(
            app(ServerState::new(test_db.pool.clone())),
            Method::POST,
            &format!("/approvals/{resume_approval_id}/resume"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let paused_status: String = sqlx::query_scalar("SELECT status FROM paused_actions WHERE id=$1")
            .bind(paused_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("paused status");
        assert_eq!(paused_status, "completed");
        assert_eq!(std::fs::read_to_string(temp_root.join("resume.txt")).expect("resumed file"), "resumed");
        let resume_events: i64 = sqlx::query_scalar("SELECT count(*) FROM event_stream WHERE entity_id=$1 AND event_type IN ('approval.resume.started','approval.resume.completed')")
            .bind(resume_approval_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("resume events");
        assert_eq!(resume_events, 2);
        std::fs::remove_dir_all(&temp_root).expect("cleanup resume temp root");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn deterministic_admin_role_routes_preserve_role_semantics() {
        let test_db = validation_db().await;
        let router = app(ServerState::new(test_db.pool.clone()));
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role snapshot");
        let session_id = db::new_session(&test_db.pool, &role, Some("role-admin"), ".", Some("."), None, None).await.expect("session");
        let before_snapshot = db::session_role_snapshot(&test_db.pool, session_id).await.expect("session role before");
        let response = router.clone().oneshot(Request::builder().uri("/roles").body(Body::empty()).expect("request")).await.expect("roles");
        assert_eq!(response.status(), StatusCode::OK);
        let response = router.clone().oneshot(Request::builder().uri("/roles/runtime-no-rg").body(Body::empty()).expect("request")).await.expect("role");
        assert_eq!(response.status(), StatusCode::OK);
        let response = router.clone().oneshot(Request::builder().uri("/roles/runtime-no-rg/versions").body(Body::empty()).expect("request")).await.expect("versions");
        assert_eq!(response.status(), StatusCode::OK);
        let version_id: Uuid = sqlx::query_scalar("SELECT current_version_id FROM roles WHERE id='runtime-no-rg'").fetch_one(&test_db.pool).await.expect("version");
        let response = router.clone().oneshot(Request::builder().uri(format!("/roles/versions/{version_id}")).body(Body::empty()).expect("request")).await.expect("version");
        assert_eq!(response.status(), StatusCode::OK);
        let response = router.clone().oneshot(Request::builder().uri("/roles/runtime-no-rg/export").body(Body::empty()).expect("request")).await.expect("export");
        assert_eq!(response.status(), StatusCode::OK);
        let (status, _) = request_json(router.clone(), Method::POST, "/roles/runtime-no-rg/archive", json!({})).await;
        assert_eq!(status, StatusCode::OK);
        let archived: String = sqlx::query_scalar("SELECT status FROM roles WHERE id='runtime-no-rg'").fetch_one(&test_db.pool).await.expect("archived");
        assert_eq!(archived, "archived");
        let (status, _) = request_json(router.clone(), Method::POST, "/roles/runtime-no-rg/unarchive", json!({})).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = request_json(router, Method::POST, "/roles/runtime-no-rg/activate", json!({"versionId": version_id})).await;
        assert_eq!(status, StatusCode::OK);
        let after_snapshot = db::session_role_snapshot(&test_db.pool, session_id).await.expect("session role after");
        assert_eq!(before_snapshot.role_version_id, after_snapshot.role_version_id);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn deterministic_admin_websocket_mutations_update_projection_without_rehydrate() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("admin-ws"), ".", Some("."), None, None).await.expect("session");
        let approval_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO approval_requests (
                id, session_id, action_name, requested_by_role, input_context, required_approver_kind, status
            ) VALUES ($1, $2, 'fs.write', '{}'::jsonb, '{"action":"fs.write"}'::jsonb, 'owner', 'pending')
            "#,
        )
        .bind(approval_id)
        .bind(session_id)
        .execute(&test_db.pool)
        .await
        .expect("insert approval");
        db::append_event(&test_db.pool, session_id, None, "approval", Some(approval_id), "approval.requested", Some("pending"), json!({"requestId": approval_id})).await.expect("approval requested event");

        let mut client_projection = projection::build_runtime_projection_snapshot(&test_db.pool, None)
            .await
            .expect("snapshot");
        let after = client_projection.watermark;
        assert!(client_projection.roles.iter().any(|role| role.id == "runtime-no-rg" && role.status != "archived"));
        assert!(client_projection.pending_approvals.iter().any(|approval| approval.id == approval_id.to_string()));
        let state = ServerState::new(test_db.pool.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let (mut ws, _) = connect_async(format!("ws://{addr}/state/ws?after={after}"))
            .await
            .expect("connect ws");
        let _hello = ws.next().await.expect("hello").expect("hello message");

        let router = app(ServerState::new(test_db.pool.clone()));
        let (status, _) = request_json(
            router.clone(),
            Method::POST,
            &format!("/approvals/{approval_id}/decide"),
            json!({"decision":"denied","reason":"deterministic admin ws validation"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let direct_deltas = projection::build_projection_deltas_after(&test_db.pool, after, None, 100).await.expect("direct deltas after approval decide");
        assert!(direct_deltas.iter().any(|delta| matches!(&delta.kind, RuntimeDeltaKind::TimelineAppend { item } if item.event_type == "approval.decided")));
        let mut saw_timeline_for_approval_decide = false;
        apply_until(&mut ws, &mut client_projection, |delta, projection| {
            saw_timeline_for_approval_decide |= matches!(&delta.kind, RuntimeDeltaKind::TimelineAppend { item } if item.event_type == "approval.decided");
            projection.pending_approvals.iter().all(|approval| approval.id != approval_id.to_string())
        })
        .await;
        assert!(saw_timeline_for_approval_decide);

        let (status, _) = request_json(router.clone(), Method::POST, "/roles/runtime-no-rg/archive", json!({})).await;
        assert_eq!(status, StatusCode::OK);

        let mut saw_timeline_for_role_archive = false;
        apply_until(&mut ws, &mut client_projection, |delta, projection| {
            saw_timeline_for_role_archive |= matches!(&delta.kind, RuntimeDeltaKind::TimelineAppend { item } if item.event_type == "role.archived");
            projection.roles.iter().any(|role| role.id == "runtime-no-rg" && role.status == "archived")
        })
        .await;
        assert!(saw_timeline_for_role_archive);

        let seed: command_registry::CommandSeed = serde_json::from_value(admin_command_seed("cmd.admin.ws")).expect("ws seed");
        let request_id = command_registry::create_request(&test_db.pool, session_id, command_registry::ChangeRequestInput {
            operation: "add".to_string(),
            command: seed.clone(),
            rationale: "deterministic websocket validation".to_string(),
            recommended_policy: "operator reviewed".to_string(),
            requester: "server-test".to_string(),
        })
        .await
        .expect("ws registry request");
        let decision = json!({
            "sessionId": session_id,
            "status": "approved",
            "finalScope": {"scopeType":"project","projectKey":"admin-ws"},
            "finalExecutionPolicy": {"decision":"allow","reason":"deterministic"},
            "finalCommand": seed
        });
        let (status, _) = request_json(router.clone(), Method::POST, &format!("/command-registry/requests/{request_id}/decide"), decision).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = request_json(router.clone(), Method::POST, &format!("/command-registry/requests/{request_id}/apply"), json!({"sessionId": session_id})).await;
        assert_eq!(status, StatusCode::OK);
        let mut saw_timeline_for_command_apply = false;
        apply_until(&mut ws, &mut client_projection, |delta, projection| {
            saw_timeline_for_command_apply |= matches!(&delta.kind, RuntimeDeltaKind::TimelineAppend { item } if item.event_type == "command_registry.applied");
            saw_timeline_for_command_apply && projection.command_registry.iter().any(|command| command.action_id == "cmd.admin.ws" && command.enabled)
        })
        .await;
        assert!(saw_timeline_for_command_apply);

        let turn_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status) VALUES ($1,$2,'user','memory','completed')")
            .bind(turn_id)
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("memory turn");
        let tool_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status) VALUES ($1,$2,$3,'execute_code','memory-call','{}'::jsonb,'completed')")
            .bind(tool_id)
            .bind(session_id)
            .bind(turn_id)
            .execute(&test_db.pool)
            .await
            .expect("memory tool");
        let script_id = Uuid::new_v4();
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status) VALUES ($1,$2,'output(\"memory\")','completed')")
            .bind(script_id)
            .bind(tool_id)
            .execute(&test_db.pool)
            .await
            .expect("memory script");
        let memory_id = Uuid::new_v4();
        let vector = format!("[{}]", vec!["0"; workflow_memory::DEFAULT_DIMENSIONS].join(","));
        sqlx::query(
            r#"
            INSERT INTO workflow_memories (
                id, script_run_id, session_id, scope_type, project_key, title, reason, summary,
                provider, model, dimensions, storage_type, source_hash, command_fingerprint, embedding
            ) VALUES ($1,$2,$3,'project','admin-ws','WS Memory','Reason','Summary','deterministic','test',$4,'halfvec','ws-hash','plain',$5::halfvec)
            "#,
        )
        .bind(memory_id)
        .bind(script_id)
        .bind(session_id)
        .bind(workflow_memory::DEFAULT_DIMENSIONS as i32)
        .bind(vector)
        .execute(&test_db.pool)
        .await
        .expect("ws memory");
        let (status, _) = request_json(
            router,
            Method::POST,
            &format!("/workflow-memories/{memory_id}/feedback"),
            json!({"sessionId": session_id, "feedback":"attempted", "payload":{"variant":true}}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let mut saw_timeline_for_memory_feedback = false;
        apply_until(&mut ws, &mut client_projection, |delta, projection| {
            saw_timeline_for_memory_feedback |= matches!(&delta.kind, RuntimeDeltaKind::TimelineAppend { item } if item.event_type == "workflow_memory.mark_attempted");
            projection.workflow_memories.iter().any(|memory| memory.id == memory_id.to_string())
        })
        .await;
        assert!(saw_timeline_for_memory_feedback);

        server.abort();
        test_db.cleanup().await;
    }

    fn admin_command_seed(action_id: &str) -> Value {
        json!({
            "actionId": action_id,
            "binaryName": "echo",
            "candidatePaths": ["/bin/echo", "/usr/bin/echo"],
            "starlarkObject": "admin_echo",
            "starlarkMethod": "run",
            "argvPrefix": [],
            "defaultCwd": ".",
            "cwdPolicy": "underExecutionRoot",
            "envPolicy": "empty",
            "syncAllowed": true,
            "asyncAllowed": false,
            "maxRuntimeMs": null,
            "endOfTurnBehavior": "terminate",
            "stdinPolicy": "forbid",
            "minAwaitMs": 0,
            "maxAwaitMs": 60000,
            "outputBufferBytes": 64000,
            "terminateGraceMs": 1000,
            "outputLimitBytes": 12000,
            "mutationClass": "readOnly",
            "modelDescription": "deterministic admin validation echo command",
            "allowCwdArg": false,
            "allowArgsArg": true,
            "forbiddenArgs": [],
            "executionPolicy": "allow"
        })
    }


    #[tokio::test]
    async fn typed_provider_connection_errors_map_to_unavailable() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind unused provider port");
        let addr = listener.local_addr().expect("provider addr");
        drop(listener);
        let error = reqwest::Client::new()
            .post(format!("http://{addr}/v1/embeddings"))
            .json(&json!({"model":"unavailable", "input":"deterministic"}))
            .send()
            .await
            .expect_err("provider connection should fail after listener is dropped");
        let api_error = ApiError::from(anyhow::Error::new(error));
        assert_eq!(api_error.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(api_error.code, "unavailable");
    }

    #[test]
    fn untyped_anyhow_messages_do_not_drive_api_error_status() {
        for message in [
            "workflow memory is not visible to session: 00000000-0000-0000-0000-000000000000",
            "role not found: runtime-x",
            "session already has an active send",
            "unsupported command registry operation: explode",
            "database provider model embedding connection failed",
        ] {
            let api_error = ApiError::from(anyhow::anyhow!(message));
            assert_eq!(api_error.status, StatusCode::INTERNAL_SERVER_ERROR, "untyped message was classified: {message}");
            assert_eq!(api_error.code, "internal_error");
            assert_eq!(api_error.message, "unexpected server error");
        }
        let typed = ApiError::from(anyhow::Error::new(RuntimeDomainError::forbidden(
            "workflow memory is not visible to session: memory-1",
            json!({"entity":"workflow_memory","id":"memory-1"}),
        )));
        assert_eq!(typed.status, StatusCode::FORBIDDEN);
        assert_eq!(typed.code, "forbidden");
    }

    #[tokio::test]
    async fn deterministic_api_errors_are_structured_and_domain_mapped() {
        let test_db = validation_db().await;
        let router = app(ServerState::new(test_db.pool.clone()));
        let missing_id = Uuid::new_v4();

        let response = router.clone().oneshot(Request::builder().uri(format!("/sessions/{missing_id}")).body(Body::empty()).expect("request")).await.expect("missing session");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("missing session body");
        let missing_session: Value = serde_json::from_slice(&bytes).expect("missing session json");
        assert_api_error(&missing_session, "not_found");
        assert_eq!(missing_session["error"]["details"]["entity"], "session");

        let response = router.clone().oneshot(Request::builder().uri("/roles/runtime-x").body(Body::empty()).expect("request")).await.expect("missing role");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("missing role body");
        let missing_role: Value = serde_json::from_slice(&bytes).expect("missing role json");
        assert_api_error(&missing_role, "not_found");
        assert_eq!(missing_role["error"]["details"]["entity"], "role");

        let response = router.clone().oneshot(Request::builder().uri(format!("/approvals/{missing_id}")).body(Body::empty()).expect("request")).await.expect("missing approval");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("missing approval body");
        let missing_approval: Value = serde_json::from_slice(&bytes).expect("missing approval json");
        assert_api_error(&missing_approval, "not_found");
        assert_eq!(missing_approval["error"]["details"]["entity"], "approval");

        let response = router.clone().oneshot(Request::builder().uri("/command-registry/cmd.missing.test").body(Body::empty()).expect("request")).await.expect("missing command");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("missing command body");
        let missing_command: Value = serde_json::from_slice(&bytes).expect("missing command json");
        assert_api_error(&missing_command, "not_found");
        assert_eq!(missing_command["error"]["details"]["entity"], "command");

        let response = router.clone().oneshot(Request::builder().uri(format!("/command-registry/requests/{missing_id}")).body(Body::empty()).expect("request")).await.expect("missing registry request");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("missing registry request body");
        let missing_request: Value = serde_json::from_slice(&bytes).expect("missing registry request json");
        assert_api_error(&missing_request, "not_found");
        assert_eq!(missing_request["error"]["details"]["entity"], "command_registry_request");

        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("error-api"), ".", Some("."), None, None).await.expect("session");
        let response = router.clone().oneshot(Request::builder().uri(format!("/workflow-memories/{missing_id}?sessionId={session_id}")).body(Body::empty()).expect("request")).await.expect("missing memory");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("missing memory body");
        let missing_memory: Value = serde_json::from_slice(&bytes).expect("missing memory json");
        assert_api_error(&missing_memory, "not_found");
        assert_eq!(missing_memory["error"]["details"]["entity"], "workflow_memory");

        let (status, bad_json) = request_raw(router.clone(), Method::POST, "/sessions", "{").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_api_error(&bad_json, "bad_request");

        let approval_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO approval_requests (
                id, session_id, action_name, requested_by_role, input_context, required_approver_kind, status
            ) VALUES ($1, $2, 'fs.write', '{}'::jsonb, '{"action":"fs.write"}'::jsonb, 'owner', 'pending')
            "#,
        )
        .bind(approval_id)
        .bind(session_id)
        .execute(&test_db.pool)
        .await
        .expect("insert approval");
        let (status, _) = request_json(router.clone(), Method::POST, &format!("/approvals/{approval_id}/decide"), json!({"decision":"denied","reason":"first"})).await;
        assert_eq!(status, StatusCode::OK);
        let (status, conflict) = request_json(router.clone(), Method::POST, &format!("/approvals/{approval_id}/decide"), json!({"decision":"denied","reason":"second"})).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_api_error(&conflict, "conflict");

        let other_session_id = db::new_session(&test_db.pool, &role, Some("other-error-api"), ".", Some("."), None, None).await.expect("other session");
        let turn_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status) VALUES ($1,$2,'user','memory','completed')")
            .bind(turn_id)
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("memory turn");
        let tool_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status) VALUES ($1,$2,$3,'execute_code','memory-call','{}'::jsonb,'completed')")
            .bind(tool_id)
            .bind(session_id)
            .bind(turn_id)
            .execute(&test_db.pool)
            .await
            .expect("memory tool");
        let script_id = Uuid::new_v4();
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status) VALUES ($1,$2,'output(\"memory\")','completed')")
            .bind(script_id)
            .bind(tool_id)
            .execute(&test_db.pool)
            .await
            .expect("memory script");
        let memory_id = Uuid::new_v4();
        let vector = format!("[{}]", vec!["0"; workflow_memory::DEFAULT_DIMENSIONS].join(","));
        sqlx::query(
            r#"
            INSERT INTO workflow_memories (
                id, script_run_id, session_id, scope_type, project_key, title, reason, summary,
                provider, model, dimensions, storage_type, source_hash, command_fingerprint, embedding
            ) VALUES ($1,$2,$3,'project','error-api','Memory','Reason','Summary','deterministic','test',$4,'halfvec','error-hash','plain',$5::halfvec)
            "#,
        )
        .bind(memory_id)
        .bind(script_id)
        .bind(session_id)
        .bind(workflow_memory::DEFAULT_DIMENSIONS as i32)
        .bind(vector)
        .execute(&test_db.pool)
        .await
        .expect("memory");
        let (status, forbidden) = request_json(router.clone(), Method::POST, &format!("/workflow-memories/{memory_id}/feedback"), json!({"sessionId": other_session_id, "feedback":"helpful", "payload":{}})).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_api_error(&forbidden, "forbidden");

        let seed: command_registry::CommandSeed = serde_json::from_value(admin_command_seed("cmd.error.validation")).expect("seed");
        let request_id = command_registry::create_request(&test_db.pool, session_id, command_registry::ChangeRequestInput {
            operation: "add".to_string(),
            command: seed,
            rationale: "deterministic error validation".to_string(),
            recommended_policy: "operator reviewed".to_string(),
            requester: "server-test".to_string(),
        })
        .await
        .expect("registry request");
        let (status, validation) = request_json(router.clone(), Method::POST, &format!("/command-registry/requests/{request_id}/preview-decision"), json!({"status":"maybe"})).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_api_error(&validation, "validation_failed");

        let (status, missing_decide) = request_json(router.clone(), Method::POST, &format!("/command-registry/requests/{missing_id}/decide"), json!({"sessionId": session_id, "status":"denied"})).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_api_error(&missing_decide, "not_found");
        assert_eq!(missing_decide["error"]["details"]["entity"], "command_registry_request");
        let (status, missing_apply) = request_json(router.clone(), Method::POST, &format!("/command-registry/requests/{missing_id}/apply"), json!({"sessionId": session_id})).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_api_error(&missing_apply, "not_found");
        assert_eq!(missing_apply["error"]["details"]["entity"], "command_registry_request");
        let (status, _) = request_json(router.clone(), Method::POST, &format!("/command-registry/requests/{request_id}/decide"), json!({"sessionId": session_id, "status":"denied"})).await;
        assert_eq!(status, StatusCode::OK);
        let (status, registry_conflict) = request_json(router.clone(), Method::POST, &format!("/command-registry/requests/{request_id}/decide"), json!({"sessionId": session_id, "status":"denied"})).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_api_error(&registry_conflict, "conflict");

        test_db.pool.close().await;
        let response = router.clone().oneshot(Request::builder().uri("/health").body(Body::empty()).expect("request")).await.expect("unavailable health");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("unavailable body");
        let unavailable: Value = serde_json::from_slice(&bytes).expect("unavailable json");
        assert_api_error(&unavailable, "unavailable");
        let response = router.oneshot(Request::builder().uri(format!("/sessions/{missing_id}")).body(Body::empty()).expect("request")).await.expect("closed-pool session show");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("closed-pool body");
        let unavailable_non_health: Value = serde_json::from_slice(&bytes).expect("closed-pool json");
        assert_api_error(&unavailable_non_health, "unavailable");

        test_db.cleanup().await;
    }


    async fn insert_turn_and_tool(pool: &PgPool, session_id: Uuid, source: &str) -> (Uuid, Uuid) {
        let turn_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status) VALUES ($1,$2,'user',$3,'running')")
            .bind(turn_id)
            .bind(session_id)
            .bind(source)
            .execute(pool)
            .await
            .expect("insert execute_code turn");
        let tool_call_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status) VALUES ($1,$2,$3,'execute_code','test-call',$4,'running')")
            .bind(tool_call_id)
            .bind(session_id)
            .bind(turn_id)
            .bind(json!({"source": source}))
            .execute(pool)
            .await
            .expect("insert execute_code tool call");
        (turn_id, tool_call_id)
    }

    fn scoped_command_seed(action_id: &str, object: &str) -> command_registry::CommandSeed {
        let mut value = admin_command_seed(action_id);
        value["starlarkObject"] = json!(object);
        serde_json::from_value(value).expect("scoped command seed")
    }

    async fn apply_registry_seed(pool: &PgPool, session_id: Uuid, seed: command_registry::CommandSeed, scope: command_registry::RegistryScope) {
        let request_id = command_registry::create_request(pool, session_id, command_registry::ChangeRequestInput {
            operation: "add".to_string(),
            command: seed.clone(),
            rationale: "deterministic cache-stable discovery test".to_string(),
            recommended_policy: "operator reviewed".to_string(),
            requester: "server-test".to_string(),
        }).await.expect("create command request");
        command_registry::decide_request(pool, session_id, request_id, "approved", Some(scope), Some(command_registry::FinalExecutionPolicy { decision: "allow".to_string(), reason: Some("deterministic".to_string()) }), Some(seed)).await.expect("decide command request");
        command_registry::apply_request(pool, session_id, request_id).await.expect("apply command request");
    }

    #[tokio::test]
    async fn db_backed_command_discovery_updates_next_execute_code_and_enforces_visibility() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let alpha = db::new_session(&test_db.pool, &role, Some("cache-alpha"), ".", Some("."), None, None).await.expect("alpha session");
        let beta = db::new_session(&test_db.pool, &role, Some("cache-beta"), ".", Some("."), None, None).await.expect("beta session");
        let root = starlark_host::ExecutionRoot::new(".").expect("root");
        let project_source = "output(cmd[\"project_cache\"].run.describe())";
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, alpha, project_source).await;
        let before = starlark_host::execute_code(&test_db.pool, alpha, turn_id, tool_call_id, project_source, &root, &role).await;
        assert!(before.unwrap_err().to_string().contains("project_cache"));

        let project_seed = scoped_command_seed("cmd.cache.project", "project_cache");
        apply_registry_seed(&test_db.pool, alpha, project_seed, command_registry::RegistryScope { scope_type: "project".to_string(), project_key: Some("cache-alpha".to_string()) }).await;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, alpha, project_source).await;
        let after = starlark_host::execute_code(&test_db.pool, alpha, turn_id, tool_call_id, project_source, &root, &role).await.expect("execute after project command");
        let after_value = serde_json::to_value(after).expect("after packet");
        assert_eq!(after_value["status"], "completed");
        assert!(after_value["output"].as_str().unwrap_or_default().contains("cmd.cache.project"));

        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, beta, project_source).await;
        let non_visible = starlark_host::execute_code(&test_db.pool, beta, turn_id, tool_call_id, project_source, &root, &role).await;
        assert!(non_visible.unwrap_err().to_string().contains("project_cache"));

        let global_source = "output(cmd[\"global_cache\"].run.describe())";
        let global_seed = scoped_command_seed("cmd.cache.global", "global_cache");
        apply_registry_seed(&test_db.pool, alpha, global_seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, beta, global_source).await;
        let global = starlark_host::execute_code(&test_db.pool, beta, turn_id, tool_call_id, global_source, &root, &role).await.expect("execute global command");
        let global_value = serde_json::to_value(global).expect("global packet");
        assert_eq!(global_value["status"], "completed");
        assert!(global_value["output"].as_str().unwrap_or_default().contains("cmd.cache.global"));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn reconstructed_history_excludes_synthetic_runtime_command_context_messages() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("history-cache"), ".", Some("."), None, None).await.expect("session");
        let turn_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, completed_at) VALUES ($1,$2,'user','ordinary user prompt','completed',now())")
            .bind(turn_id)
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("turn");
        sqlx::query("INSERT INTO model_events (id, session_id, turn_id, event_type, payload) VALUES ($1,$2,$3,'assistant_message',$4)")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .bind(turn_id)
            .bind(json!({"request":{"runtimeInputMessages":[{"metadata":{"source":"runtime_command_context","commandContextId":"cmdctx-test"}}]},"commandContext":{"id":"cmdctx-test"}}))
            .execute(&test_db.pool)
            .await
            .expect("assistant event");
        sqlx::query("INSERT INTO model_events (id, session_id, turn_id, event_type, payload) VALUES ($1,$2,$3,'final_response',$4)")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .bind(turn_id)
            .bind(json!({"summary":"ordinary assistant summary"}))
            .execute(&test_db.pool)
            .await
            .expect("final response");
        let history = db::reconstructed_history(&test_db.pool, session_id).await.expect("history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].user, "ordinary user prompt");
        assert_eq!(history[0].assistant.as_deref(), Some("ordinary assistant summary"));
        assert!(!history.iter().any(|item| item.user.contains("runtime command context") || item.assistant.as_deref().unwrap_or_default().contains("runtime command context")));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn deterministic_admin_command_registry_routes_preserve_request_apply_semantics() {
        let test_db = validation_db().await;
        let router = app(ServerState::new(test_db.pool.clone()));
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("admin-command"), ".", Some("."), None, None).await.expect("session");
        let seed: command_registry::CommandSeed = serde_json::from_value(admin_command_seed("cmd.admin.echo")).expect("seed");
        let request_id = command_registry::create_request(&test_db.pool, session_id, command_registry::ChangeRequestInput {
            operation: "add".to_string(),
            command: seed.clone(),
            rationale: "deterministic admin validation".to_string(),
            recommended_policy: "operator reviewed".to_string(),
            requester: "server-test".to_string(),
        }).await.expect("registry request");

        for path in [
            "/command-registry".to_string(),
            "/command-registry/requests".to_string(),
            format!("/command-registry/requests/{request_id}"),
            format!("/command-registry/requests/{request_id}/review"),
            format!("/command-registry/requests/{request_id}/final-template"),
        ] {
            let response = router.clone().oneshot(Request::builder().uri(path).body(Body::empty()).expect("request")).await.expect("response");
            assert_eq!(response.status(), StatusCode::OK);
        }
        let decision = json!({
            "sessionId": session_id,
            "status": "approved",
            "finalScope": {"scopeType":"project","projectKey":"admin-command"},
            "finalExecutionPolicy": {"decision":"allow","reason":"deterministic"},
            "finalCommand": seed
        });
        let (status, _) = request_json(router.clone(), Method::POST, &format!("/command-registry/requests/{request_id}/preview-decision"), decision.clone()).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = request_json(router.clone(), Method::POST, &format!("/command-registry/requests/{request_id}/decide"), decision).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = request_json(router.clone(), Method::POST, &format!("/command-registry/requests/{request_id}/apply"), json!({"sessionId": session_id})).await;
        assert_eq!(status, StatusCode::OK);
        let response = router.clone().oneshot(Request::builder().uri("/command-registry/cmd.admin.echo").body(Body::empty()).expect("request")).await.expect("show");
        assert_eq!(response.status(), StatusCode::OK);
        let response = router.clone().oneshot(Request::builder().uri("/command-registry?project=admin-command").body(Body::empty()).expect("request")).await.expect("scoped list");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("scoped list body");
        let scoped_list: Value = serde_json::from_slice(&bytes).expect("scoped list json");
        assert!(scoped_list.as_array().expect("scoped list array").iter().any(|item| item["actionId"] == "cmd.admin.echo" && item["scope"]["projectKey"] == "admin-command"));
        let response = router.clone().oneshot(Request::builder().uri("/command-registry?project=other-command").body(Body::empty()).expect("request")).await.expect("other scoped list");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("other scoped list body");
        let other_scoped_list: Value = serde_json::from_slice(&bytes).expect("other scoped list json");
        assert!(other_scoped_list.as_array().expect("other scoped list array").iter().all(|item| item["actionId"] != "cmd.admin.echo"));
        let response = router.clone().oneshot(Request::builder().uri("/command-registry/cmd.admin.echo?project=admin-command").body(Body::empty()).expect("request")).await.expect("scoped show");
        assert_eq!(response.status(), StatusCode::OK);
        let response = router.clone().oneshot(Request::builder().uri("/command-registry/cmd.admin.echo?project=other-command").body(Body::empty()).expect("request")).await.expect("other scoped show");
        assert_ne!(response.status(), StatusCode::OK);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn deterministic_admin_workflow_memory_routes_validate_visibility_and_feedback() {
        let test_db = validation_db().await;
        let router = app(ServerState::new(test_db.pool.clone()));
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("memory-project"), ".", Some("."), None, None).await.expect("session");
        let other_session_id = db::new_session(&test_db.pool, &role, Some("other-project"), ".", Some("."), None, None).await.expect("other session");
        let turn_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status) VALUES ($1,$2,'user','memory','completed')")
            .bind(turn_id).bind(session_id).execute(&test_db.pool).await.expect("turn");
        let tool_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status) VALUES ($1,$2,$3,'execute_code','call','{}'::jsonb,'completed')")
            .bind(tool_id).bind(session_id).bind(turn_id).execute(&test_db.pool).await.expect("tool");
        let script_id = Uuid::new_v4();
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status) VALUES ($1,$2,'output(\"ok\")','completed')")
            .bind(script_id).bind(tool_id).execute(&test_db.pool).await.expect("script");
        let memory_id = Uuid::new_v4();
        let vector = format!("[{}]", vec!["0"; workflow_memory::DEFAULT_DIMENSIONS].join(","));
        sqlx::query(
            r#"
            INSERT INTO workflow_memories (
                id, script_run_id, session_id, scope_type, project_key, title, reason, summary,
                provider, model, dimensions, storage_type, source_hash, command_fingerprint, embedding
            ) VALUES ($1,$2,$3,'project','memory-project','Memory','Reason','Summary','deterministic','test',$4,'halfvec','hash','plain',$5::halfvec)
            "#,
        )
        .bind(memory_id).bind(script_id).bind(session_id).bind(workflow_memory::DEFAULT_DIMENSIONS as i32).bind(vector).execute(&test_db.pool).await.expect("memory");

        let response = router.clone().oneshot(Request::builder().uri(format!("/workflow-memories?sessionId={session_id}")).body(Body::empty()).expect("request")).await.expect("list");
        assert_eq!(response.status(), StatusCode::OK);
        let response = router.clone().oneshot(Request::builder().uri(format!("/workflow-memories/{memory_id}?sessionId={session_id}")).body(Body::empty()).expect("request")).await.expect("show");
        assert_eq!(response.status(), StatusCode::OK);
        let (status, _) = request_json(router.clone(), Method::POST, &format!("/workflow-memories/{memory_id}/feedback"), json!({"sessionId": session_id, "feedback":"attempted", "payload":{"variant":true}})).await;
        assert_eq!(status, StatusCode::OK);
        let response = router.clone().oneshot(Request::builder().uri(format!("/workflow-memories/{memory_id}/events?sessionId={session_id}")).body(Body::empty()).expect("request")).await.expect("events");
        assert_eq!(response.status(), StatusCode::OK);
        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/workflow-memories/{memory_id}/feedback"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"sessionId": other_session_id, "feedback":"helpful", "payload":{}}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("invisible feedback response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn deterministic_websocket_reports_resync_required_for_uncontinuable_after() {
        let test_db = validation_db().await;
        let state = ServerState::new(test_db.pool.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });

        let (mut ws, _) = connect_async(format!("ws://{addr}/state/ws?after=999999999"))
            .await
            .expect("connect ws");
        let _hello = ws.next().await.expect("hello").expect("hello message");
        let resync = ws.next().await.expect("resync").expect("resync message");
        let value: Value = serde_json::from_str(&resync.into_text().expect("text")).expect("json");
        assert_eq!(value["type"], "resyncRequired");
        assert_eq!(value["delta"]["type"], "resyncRequired");
        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn deterministic_websocket_reports_resync_required_when_after_is_omitted() {
        let test_db = validation_db().await;
        let state = ServerState::new(test_db.pool.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });

        let (mut ws, _) = connect_async(format!("ws://{addr}/state/ws"))
            .await
            .expect("connect ws");
        let _hello = ws.next().await.expect("hello").expect("hello message");
        let resync = ws.next().await.expect("resync").expect("resync message");
        let value: Value = serde_json::from_str(&resync.into_text().expect("text")).expect("json");
        assert_eq!(value["type"], "resyncRequired");
        assert_eq!(value["delta"]["type"], "resyncRequired");
        assert!(
            value["delta"]["reason"]
                .as_str()
                .unwrap_or_default()
                .contains("missing after watermark")
        );
        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn deterministic_websocket_reports_resync_required_for_missing_positive_after() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg")
            .await
            .expect("role");
        let first = db::new_session(&test_db.pool, &role, Some("missing-after"), ".", Some("."), None, None)
            .await
            .expect("first session");
        let missing_after: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence), 0)::bigint FROM event_stream WHERE session_id = $1",
        )
        .bind(first)
        .fetch_one(&test_db.pool)
        .await
        .expect("first sequence");
        db::new_session(&test_db.pool, &role, Some("missing-after"), ".", Some("."), None, None)
            .await
            .expect("second session");
        sqlx::query("DELETE FROM event_stream WHERE sequence = $1")
            .bind(missing_after)
            .execute(&test_db.pool)
            .await
            .expect("delete middle watermark");

        let state = ServerState::new(test_db.pool.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });

        let (mut ws, _) = connect_async(format!("ws://{addr}/state/ws?after={missing_after}"))
            .await
            .expect("connect ws");
        let _hello = ws.next().await.expect("hello").expect("hello message");
        let resync = ws.next().await.expect("resync").expect("resync message");
        let value: Value = serde_json::from_str(&resync.into_text().expect("text")).expect("json");
        assert_eq!(value["type"], "resyncRequired");
        assert_eq!(value["delta"]["type"], "resyncRequired");
        assert!(
            value["delta"]["reason"]
                .as_str()
                .unwrap_or_default()
                .contains("stale, missing")
        );
        server.abort();
        test_db.cleanup().await;
    }
}
