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
use robdex_agent_runtime_projection::{RoleEditorDraft, RuntimeDelta, RuntimeDeltaKind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::{Mutex, watch};
use tokio::time::{Duration, interval};
use uuid::Uuid;

use crate::errors::{RuntimeDomainError, RuntimeErrorKind};
use crate::model::ModelClient;
use crate::{approvals, command_registry, compaction, db, projection, requirements, routing, runtime, starlark_host, workflow_memory};

#[derive(Clone)]
pub struct ServerState {
    pub pool: PgPool,
    pub runtime_identity: String,
    pub active_sends: Arc<Mutex<HashSet<Uuid>>>,
    pub shutdown_tx: watch::Sender<bool>,
    pub shutdown: watch::Receiver<bool>,
    pub model_client: Option<Arc<dyn ModelClient + Send + Sync>>,
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
            model_client: None,
        }
    }

    #[cfg(test)]
    pub fn new_with_model_client(pool: PgPool, runtime_identity: String, model_client: Arc<dyn ModelClient + Send + Sync>) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            pool,
            runtime_identity,
            active_sends: Arc::new(Mutex::new(HashSet::new())),
            shutdown_tx,
            shutdown: shutdown_rx,
            model_client: Some(model_client),
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
        .route("/projects", get(list_projects).post(create_project))
        .route("/projects/{project_key}", post(update_project))
        .route("/projects/{project_key}/archive", post(archive_project))
        .route("/projects/{project_key}/unarchive", post(unarchive_project))
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/sessions/{session_id}", get(show_session))
        .route("/sessions/{session_id}/history", get(session_history))
        .route("/sessions/{session_id}/send", post(send_message))
        .route("/sessions/{session_id}/compact", post(compact_session))
        .route("/sessions/{session_id}/god-mode/grant", post(grant_god_mode))
        .route("/sessions/{session_id}/god-mode/revoke", post(revoke_god_mode))
        .route("/sessions/{session_id}/requirements", get(requirements_status).post(set_requirements))
        .route("/sessions/{session_id}/requirements/clear", post(clear_requirements))
        .route("/sessions/{session_id}/requirements/packets", get(requirements_packets))
        .route("/sessions/{session_id}/settings", post(update_session_settings))
        .route("/sessions/{session_id}/close", post(close_session))
        .route("/sessions/{session_id}/archive", post(archive_session))
        .route("/sessions/{session_id}/fork", post(fork_session))
        .route("/sessions/{session_id}/processes/{handle}/terminate", post(terminate_process))
        .route("/sessions/{session_id}/processes/{handle}/input", post(input_process))
        .route("/sessions/{session_id}/processes/{handle}/flush", post(flush_process))
        .route("/approvals", get(list_approvals))
        .route("/approvals/{approval_id}", get(show_approval))
        .route("/approvals/{approval_id}/decide", post(decide_approval))
        .route("/approvals/{approval_id}/resume", post(resume_approval))
        .route("/roles", get(list_roles).post(create_role_from_draft))
        .route("/roles/editor/options", get(role_editor_options))
        .route("/roles/editor/validate", post(validate_role_draft))
        .route("/roles/{role_id}", get(show_role))
        .route("/roles/{role_id}/versions", get(role_versions).post(update_role_from_draft))
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
    let mut value = serde_json::to_value(&projection).map_err(anyhow::Error::from)?;
    if let Some(object) = value.as_object_mut() {
        let selected_project_id = projection
            .selected_session
            .as_ref()
            .and_then(|session| session.project_key.clone());
        let role_options = projection
            .roles
            .iter()
            .map(|role| json!({
                "id": role.id,
                "displayLabel": role.display_name,
                "status": role.status,
                "model": role.model,
            }))
            .collect::<Vec<_>>();
        let (model_options, model_options_error) = match crate::model::codex_adapter::CodexModelOptionsProvider::new().model_options(false).await {
            Ok(options) => (
                options
                    .into_iter()
                    .map(|model| json!({"id": model.id, "displayLabel": model.display_label, "source": model.source, "isDefault": model.is_default}))
                    .collect::<Vec<_>>(),
                None,
            ),
            Err(error) => (Vec::new(), Some(format!("Model options unavailable: {error}"))),
        };
        let mut project_options = vec![
            json!({"id": "__all__", "displayLabel": "All", "selected": selected_project_id.is_none()}),
            json!({"id": "__unassigned__", "displayLabel": "Unassigned", "selected": selected_project_id.as_deref() == Some("__unassigned__")}),
        ];
        project_options.extend(projection.projects.iter().filter(|project| !project.archived).map(|project| {
            json!({
                "id": project.project_key,
                "displayLabel": project.display_name,
                "selected": selected_project_id.as_deref() == Some(project.project_key.as_str()),
                "defaultWorkdir": project.default_workdir,
                "defaultWorktreeRoot": project.default_worktree_root,
                "defaultRoleId": project.default_role_id,
                "defaultModel": project.default_model,
            })
        }));
        let modal_surface_summaries = vec![
            json!({"surfaceId":"session","title":"Session","rowCount": projection.selected_session.iter().count(), "actionCount": 3}),
            json!({"surfaceId":"history","title":"History","rowCount": projection.timeline.len(), "actionCount": 0}),
            json!({"surfaceId":"diagnostics","title":"Diagnostics","rowCount": 7, "actionCount": 0}),
            json!({"surfaceId":"compaction","title":"Compaction","rowCount": projection.statistics.compaction_checkpoints, "actionCount": 1}),
            json!({"surfaceId":"statistics","title":"Statistics","rowCount": 14, "actionCount": 0}),
            json!({"surfaceId":"processManager","title":"Process Manager","rowCount": projection.statistics.managed_processes, "actionCount": 4}),
            json!({"surfaceId":"settings","title":"Settings","rowCount": 8, "actionCount": 1}),
            json!({"surfaceId":"roleAdmin","title":"Role Admin","rowCount": projection.roles.len(), "actionCount": 6}),
            json!({"surfaceId":"workflowMemory","title":"Workflow Memory","rowCount": projection.workflow_memories.len(), "actionCount": 3}),
            json!({"surfaceId":"approvals","title":"Approvals","rowCount": projection.pending_approvals.len(), "actionCount": 2}),
            json!({"surfaceId":"commandRegistry","title":"Command Registry","rowCount": projection.command_registry.len(), "actionCount": 3}),
        ];
        let pending_actions = projection
            .pending_approvals
            .iter()
            .map(|approval| json!({"kind":"approval","id": approval.id, "status": approval.status}))
            .chain(projection.command_registry_requests.iter().map(|request| json!({"kind":"commandRegistry","id": request.id, "status": request.status})))
            .collect::<Vec<_>>();
        object.insert("selectedSessionIdentity".to_string(), json!(projection.selected_session.as_ref().map(|session| json!({"id": session.id, "title": session.title, "status": session.status}))));
        object.insert("selectedProjectIdentity".to_string(), json!(selected_project_id.as_ref().map(|project| json!({"id": project, "displayLabel": project}))));
        object.insert("modalSurfaceSummaries".to_string(), json!(modal_surface_summaries));
        object.insert("pendingActions".to_string(), json!(pending_actions));
        object.insert("sessionList".to_string(), json!(projection.sessions));
        object.insert("roleOptions".to_string(), json!(role_options));
        object.insert("modelOptions".to_string(), json!(model_options));
        object.insert("modelOptionsError".to_string(), json!(model_options_error));
        object.insert("projectOptions".to_string(), json!(project_options));
        object.insert("watermarkDeltaMetadata".to_string(), json!({
            "watermark": projection.watermark,
            "initialHydrateEntryCap": 50,
            "selectedChatEntryCount": projection.selected_chat_entries.len(),
            "deltaContract": {
                "semanticSelectedChatDeltas": true,
                "fullSnapshotAllowedFor": ["hydrate", "selection", "resync", "recovery"],
            },
        }));
    }
    Ok(Json(value))
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
    model: Option<String>,
    workdir: Option<String>,
    worktree_root: Option<String>,
    title: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
struct ProjectSettingsRequest {
    display_name: String,
    default_workdir: String,
    default_worktree_root: String,
    default_role_id: Option<String>,
    default_model: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
struct CreateProjectRequest {
    project_key: String,
    display_name: String,
    default_workdir: String,
    default_worktree_root: String,
    default_role_id: Option<String>,
    default_model: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateSessionSettingsRequest {
    project: String,
    role: String,
    model: String,
    workdir: String,
    worktree_root: String,
    title: String,
    name: String,
    tracked: bool,
}

async fn create_session(
    State(state): State<ServerState>,
    payload: std::result::Result<Json<CreateSessionRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let role_id = required_create_session_field(request.role.as_deref(), "role")?;
    let project_intent = required_create_session_field(request.project.as_deref(), "project")?;
    let model = required_create_session_field(request.model.as_deref(), "model")?;
    let workdir = required_create_session_field(request.workdir.as_deref(), "workdir")?;
    let worktree_root = required_create_session_field(request.worktree_root.as_deref(), "worktreeRoot")?;
    let title = required_create_session_field(request.title.as_deref(), "title")?;
    let name = required_create_session_field(request.name.as_deref(), "name")?;
    let mut role = db::current_role_snapshot(&state.pool, role_id).await.map_err(|error| map_missing_entity(error, "role", role_id))?;
    role.model_defaults.model = model.to_string();
    let project = project_from_intent(&state.pool, project_intent).await?;
    let id = db::new_session(
        &state.pool,
        &role,
        project.as_deref(),
        workdir,
        Some(worktree_root),
        Some(title),
        Some(name),
    )
    .await?;
    Ok(Json(json!({"sessionId": id})))
}

async fn list_projects(State(state): State<ServerState>) -> Result<Json<Value>, ApiError> {
    let projects = db::list_projects(&state.pool, true).await?;
    Ok(Json(json!({"projects": projects.into_iter().map(project_json).collect::<Vec<_>>() })))
}

async fn create_project(
    State(state): State<ServerState>,
    payload: std::result::Result<Json<CreateProjectRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let project = db::create_project(
        &state.pool,
        request.project_key.trim(),
        request.display_name.trim(),
        request.default_workdir.trim(),
        request.default_worktree_root.trim(),
        request.default_role_id.as_deref(),
        request.default_model.trim(),
    )
    .await?;
    Ok(Json(json!({"project": project_json(project)})))
}

async fn update_project(
    State(state): State<ServerState>,
    Path(project_key): Path<String>,
    payload: std::result::Result<Json<ProjectSettingsRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let project = db::update_project(
        &state.pool,
        project_key.trim(),
        request.display_name.trim(),
        request.default_workdir.trim(),
        request.default_worktree_root.trim(),
        request.default_role_id.as_deref(),
        request.default_model.trim(),
    )
    .await
    .map_err(|error| map_missing_entity(error, "project", project_key.trim()))?;
    Ok(Json(json!({"project": project_json(project)})))
}

async fn archive_project(State(state): State<ServerState>, Path(project_key): Path<String>) -> Result<Json<Value>, ApiError> {
    let project = db::set_project_archived(&state.pool, project_key.trim(), true)
        .await
        .map_err(|error| map_missing_entity(error, "project", project_key.trim()))?;
    Ok(Json(json!({"project": project_json(project)})))
}

async fn unarchive_project(State(state): State<ServerState>, Path(project_key): Path<String>) -> Result<Json<Value>, ApiError> {
    let project = db::set_project_archived(&state.pool, project_key.trim(), false)
        .await
        .map_err(|error| map_missing_entity(error, "project", project_key.trim()))?;
    Ok(Json(json!({"project": project_json(project)})))
}

async fn project_from_intent(pool: &PgPool, project_intent: &str) -> Result<Option<String>, ApiError> {
    if matches!(project_intent, "__unassigned__" | "unassigned") {
        return Ok(None);
    }
    let active_exists = db::list_projects(pool, true)
        .await?
        .into_iter()
        .find(|project| project.project_key == project_intent)
        .map(|project| !project.archived)
        .unwrap_or(false);
    if !active_exists {
        db::create_project(
            pool,
            project_intent,
            &project_intent
                .split(['-', '_'])
                .filter(|part| !part.is_empty())
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
            ".",
            ".",
            None,
            "gpt-5.4-mini",
        )
        .await?;
    }
    Ok(Some(project_intent.to_string()))
}

fn project_json(project: db::ProjectRow) -> Value {
    json!({
        "projectKey": project.project_key,
        "displayName": project.display_name,
        "defaultWorkdir": project.default_workdir,
        "defaultWorktreeRoot": project.default_worktree_root,
        "defaultRoleId": project.default_role_id,
        "defaultModel": project.default_model,
        "archived": project.archived,
        "createdAt": project.created_at.to_rfc3339(),
        "updatedAt": project.updated_at.to_rfc3339(),
    })
}

fn required_create_session_field<'a>(value: Option<&'a str>, name: &str) -> Result<&'a str, ApiError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request(format!("create session requires {name}")))
}

async fn update_session_settings(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
    payload: std::result::Result<Json<UpdateSessionSettingsRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let project_intent = required_create_session_field(Some(&request.project), "project")?;
    let role_id = required_create_session_field(Some(&request.role), "role")?;
    let model = required_create_session_field(Some(&request.model), "model")?;
    let workdir = required_create_session_field(Some(&request.workdir), "workdir")?;
    let worktree_root = required_create_session_field(Some(&request.worktree_root), "worktreeRoot")?;
    let title = required_create_session_field(Some(&request.title), "title")?;
    let name = required_create_session_field(Some(&request.name), "name")?;
    let mut role = db::current_role_snapshot(&state.pool, role_id).await.map_err(|error| map_missing_entity(error, "role", role_id))?;
    role.model_defaults.model = model.to_string();
    let role_snapshot = crate::roles::snapshot_to_value(&role).map_err(anyhow::Error::from)?;
    let project = project_from_intent(&state.pool, project_intent).await?;
    let result = sqlx::query(
        r#"
        UPDATE sessions
        SET project_key=$2,
            role_id=$3,
            role_version=$4,
            role_snapshot=$5,
            workdir=$6,
            worktree_root=$7,
            title=$8,
            name=$9,
            tracked=$10,
            updated_at=now()
        WHERE id=$1
        "#,
    )
    .bind(session_id)
    .bind(project.as_deref())
    .bind(role_id)
    .bind(&role.version)
    .bind(role_snapshot)
    .bind(workdir)
    .bind(worktree_root)
    .bind(title)
    .bind(name)
    .bind(request.tracked)
    .execute(&state.pool)
    .await
    .map_err(anyhow::Error::from)?;
    if result.rows_affected() != 1 {
        return Err(ApiError::not_found("session", session_id));
    }
    db::append_event(
        &state.pool,
        session_id,
        None,
        "session",
        Some(session_id),
        "session.settingsUpdated",
        Some("updated"),
        json!({"project": project, "role": role_id, "model": model, "workdir": workdir, "worktreeRoot": worktree_root, "title": title, "name": name, "tracked": request.tracked}),
    )
    .await?;
    Ok(Json(json!({"sessionId": session_id, "status": "updated"})))
}

#[derive(Debug, Deserialize)]
struct SendRequest {
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactSessionRequest {
    through_turn: Option<Uuid>,
}

async fn compact_session(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
    payload: std::result::Result<Json<CompactSessionRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let checkpoint = if let Some(through_turn) = request.through_turn {
        compaction::compact_session_through_turn(
            &state.pool,
            session_id,
            through_turn,
            compaction::CompactionBudget::from_env(),
        )
        .await?
    } else {
        compaction::compact_session_through_latest_completed_turn(
            &state.pool,
            session_id,
            compaction::CompactionBudget::from_env(),
        )
        .await?
    };
    Ok(Json(json!({
        "sessionId": session_id,
        "checkpointId": checkpoint.id,
        "status": checkpoint.status,
        "compactedThroughTurnId": checkpoint.compacted_through_turn_id,
    })))
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
    let pool = state.pool.clone();
    let active_sends = state.active_sends.clone();
    let model_client = state.model_client.clone();
    let message = request.message.clone();
    let message_for_query = message.clone();
    let send_task = tokio::spawn(async move {
        let result = if let Some(model) = model_client {
            runtime::send_with_model_client(
                &pool,
                session_id,
                &message,
                model.as_ref(),
                compaction::CompactionBudget::from_env(),
            )
            .await
        } else {
            runtime::send(&pool, session_id, &message).await
        };
        active_sends.lock().await.remove(&session_id);
        result
    });

    for _ in 0..100 {
        if let Some(turn_id) = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM turns WHERE session_id=$1 AND input_text=$2 ORDER BY started_at DESC LIMIT 1",
        )
        .bind(session_id)
        .bind(&message_for_query)
        .fetch_optional(&state.pool)
        .await
        .map_err(anyhow::Error::from)?
        {
            return Ok(Json(json!({"sessionId": session_id, "turnId": turn_id, "status": "running"})));
        }
        if send_task.is_finished() {
            let result = send_task
                .await
                .map_err(|error| ApiError::from_anyhow(anyhow::anyhow!("send worker failed: {error}")))?;
            return match result {
                Ok(turn_id) => Ok(Json(json!({"sessionId": session_id, "turnId": turn_id, "status": "completed"}))),
                Err(error) => {
                    if let Some(turn_id) = sqlx::query_scalar::<_, Uuid>(
                        "SELECT id FROM turns WHERE session_id=$1 AND input_text=$2 ORDER BY started_at DESC LIMIT 1",
                    )
                    .bind(session_id)
                    .bind(&message_for_query)
                    .fetch_optional(&state.pool)
                    .await
                    .map_err(anyhow::Error::from)?
                    {
                        Ok(Json(json!({"sessionId": session_id, "turnId": turn_id, "status": "failed", "error": error.to_string()})))
                    } else {
                        Err(ApiError::from_anyhow(error))
                    }
                }
            };
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Ok(Json(json!({"sessionId": session_id, "turnId": null, "status": "queued"})))
}

#[derive(Debug, Deserialize)]
struct CloseRequest {
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GodModeRequest {
    reason: String,
}

async fn grant_god_mode(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
    payload: std::result::Result<Json<GodModeRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let grant = crate::god_mode::grant_session(&state.pool, session_id, "gui", &request.reason, None).await?;
    Ok(Json(json!({"sessionId": session_id, "grantId": grant.id, "status": grant.status})))
}

async fn revoke_god_mode(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
    payload: std::result::Result<Json<GodModeRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    crate::god_mode::revoke_active(&state.pool, session_id, "gui", &request.reason).await?;
    Ok(Json(json!({"sessionId": session_id, "status": "revoked"})))
}

async fn set_requirements(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
    payload: std::result::Result<Json<requirements::RequirementSetInput>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let set_id = requirements::set_active_requirements(&state.pool, session_id, request).await?;
    let status = requirements::status(&state.pool, session_id).await?;
    Ok(Json(json!({"sessionId": session_id, "requirementSetId": set_id, "status": status})))
}

async fn requirements_status(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let status = requirements::status(&state.pool, session_id).await?;
    Ok(Json(json!({"sessionId": session_id, "requirements": status})))
}

async fn requirements_packets(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let packets = requirements::packet_history(&state.pool, session_id).await?;
    Ok(Json(json!({"sessionId": session_id, "packets": packets})))
}

async fn clear_requirements(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    requirements::deactivate(&state.pool, session_id, "cleared").await?;
    Ok(Json(json!({"sessionId": session_id, "status": "inactive"})))
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

async fn terminate_process(
    State(state): State<ServerState>,
    Path((session_id, handle)): Path<(Uuid, String)>,
) -> Result<Json<Value>, ApiError> {
    let value = starlark_host::terminate_managed_process(&state.pool, session_id, &handle).await?;
    Ok(Json(value))
}

#[derive(Debug, Deserialize)]
struct ProcessInputRequest {
    text: String,
}

async fn input_process(
    State(state): State<ServerState>,
    Path((session_id, handle)): Path<(Uuid, String)>,
    payload: std::result::Result<Json<ProcessInputRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let value = starlark_host::input_managed_process(&state.pool, session_id, &handle, &request.text).await?;
    Ok(Json(value))
}

async fn flush_process(
    State(state): State<ServerState>,
    Path((session_id, handle)): Path<(Uuid, String)>,
) -> Result<Json<Value>, ApiError> {
    let value = starlark_host::flush_managed_process(&state.pool, session_id, &handle).await?;
    Ok(Json(value))
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

async fn role_editor_options() -> Result<Json<Value>, ApiError> {
    Ok(Json(serde_json::to_value(crate::roles::editor_options()).map_err(anyhow::Error::from)?))
}

async fn validate_role_draft(State(state): State<ServerState>, payload: std::result::Result<Json<RoleEditorDraft>, JsonRejection>) -> Result<Json<Value>, ApiError> {
    let draft = parse_json(payload)?;
    let result = validate_role_draft_for_server(&state.pool, &draft).await;
    Ok(Json(serde_json::to_value(result).map_err(anyhow::Error::from)?))
}

async fn create_role_from_draft(State(state): State<ServerState>, payload: std::result::Result<Json<RoleEditorDraft>, JsonRejection>) -> Result<Json<Value>, ApiError> {
    let draft = parse_json(payload)?;
    if db::role_exists(&state.pool, &draft.id).await? {
        return Err(ApiError::conflict(format!("role already exists: {}", draft.id)));
    }
    let imported = imported_validated_role_from_draft(&state.pool, &draft).await?;
    let role_id = imported.snapshot.id.clone();
    let version_id = imported.snapshot.role_version_id;
    db::import_role_version_with_actor(&state.pool, &imported, "gui-role-editor").await?;
    Ok(Json(json!({"roleId": role_id, "versionId": version_id, "status": "created"})))
}

async fn update_role_from_draft(State(state): State<ServerState>, Path(role_id): Path<String>, payload: std::result::Result<Json<RoleEditorDraft>, JsonRejection>) -> Result<Json<Value>, ApiError> {
    let draft = parse_json(payload)?;
    if draft.id != role_id {
        return Err(ApiError::from(RuntimeDomainError::validation_failed(format!("role draft id {} does not match route role id {role_id}", draft.id))));
    }
    if !db::role_exists(&state.pool, &role_id).await? {
        return Err(ApiError::not_found("role", &role_id));
    }
    let imported = imported_validated_role_from_draft(&state.pool, &draft).await?;
    let version_id = imported.snapshot.role_version_id;
    db::import_role_version_with_actor(&state.pool, &imported, "gui-role-editor").await?;
    Ok(Json(json!({"roleId": role_id, "versionId": version_id, "status": "updated"})))
}

async fn imported_validated_role_from_draft(pool: &PgPool, draft: &RoleEditorDraft) -> Result<crate::roles::ImportedRoleVersion, ApiError> {
    let imported = crate::roles::imported_role_from_editor_draft(draft)
        .map_err(|error| ApiError::from(RuntimeDomainError::validation_failed(error.to_string())))?;
    routing::validate_snapshot_routing_against_db(pool, &imported.snapshot)
        .await
        .map_err(|error| ApiError::from(RuntimeDomainError::validation_failed(error.to_string())))?;
    command_registry::validate_policy_actions_exist(pool, imported.snapshot.policy.keys().cloned())
        .await
        .map_err(|error| ApiError::from(RuntimeDomainError::validation_failed(error.to_string())))?;
    Ok(imported)
}

async fn validate_role_draft_for_server(pool: &PgPool, draft: &RoleEditorDraft) -> robdex_agent_runtime_projection::RoleEditorValidationResult {
    let mut result = crate::roles::validation_result_for_editor_draft(draft);
    if let Ok(imported) = crate::roles::imported_role_from_editor_draft(draft) {
        if let Err(error) = routing::validate_snapshot_routing_against_db(pool, &imported.snapshot).await {
            result.errors.push(error.to_string());
        }
        if let Err(error) = command_registry::validate_policy_actions_exist(pool, imported.snapshot.policy.keys().cloned()).await {
            result.errors.push(error.to_string());
        }
    }
    result.valid = result.errors.is_empty();
    result
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
    use crate::compaction;
    use crate::model::{ModelClient, ModelFinalTurn, ModelHistoryItem, ModelInitialTurn, ModelToolTurn, RuntimeInputMessage, ToolCallRequest};
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use crate::gui_sync::{RuntimeSyncClient, RuntimeSyncConfig, SyncOutcome};
    use crate::rinf_transport::{
        GuiTransportHandle, GuiTransportOutput, GuiTransportOutputPacket, GuiTransportRequest,
        GuiTransportRequestPacket,
    };
    use robdex_agent_runtime_projection::{
        CommandRegistryDecisionInput, GuiConnectionState, GuiControllerState, GuiOperationOutcome,
        GuiOperationRequest, RuntimeDeltaKind, RuntimeProjection,
    };
    use tokio_tungstenite::connect_async;
    use tower::ServiceExt;
    use sqlx::Row;
    use std::sync::{Arc, Mutex as StdMutex};

    #[derive(Default, Clone)]
    struct FakeModelClient {
        observed_history: Arc<StdMutex<Vec<Vec<ModelHistoryItem>>>>,
        observed_request_shapes: Arc<StdMutex<Vec<Value>>>,
        request_error: Option<&'static str>,
        final_error: Option<&'static str>,
        direct_final_text: Option<&'static str>,
        reviewer_final_text: Option<&'static str>,
        tool_name: Option<&'static str>,
        tool_arguments: Option<Value>,
        model_name: Option<&'static str>,
        request_delay_ms: Option<u64>,
    }

    fn assert_gui_operation_error(outputs: &[GuiTransportOutputPacket], context: &str) {
        assert!(
            outputs.iter().any(|packet| matches!(
                &packet.output,
                GuiTransportOutput::OperationResult { result }
                    if matches!(&result.outcome, GuiOperationOutcome::Error { error } if !error.error.code.is_empty() && !error.error.message.is_empty())
            )),
            "{context} must surface a typed OperationResult error with actionable message: {outputs:?}"
        );
    }

    #[async_trait]
    impl ModelClient for FakeModelClient {
        async fn request_tool_call(&self, role: &crate::roles::RoleSnapshot, history: &[ModelHistoryItem], runtime_messages: &[RuntimeInputMessage], execute_code_contract: &str, request_registry_contract: &str, message: &str) -> anyhow::Result<ModelInitialTurn> {
            self.observed_history.lock().expect("history lock").push(history.to_vec());
            if let Some(message) = self.request_error {
                anyhow::bail!("{message}");
            }
            if let Some(delay) = self.request_delay_ms {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            let model_name = self.model_name.unwrap_or("fake-model");
            let request_shape = crate::model::codex_adapter::CodexBackedModelClient::request_tool_call_request_shape(
                model_name,
                role,
                history,
                runtime_messages,
                execute_code_contract,
                request_registry_contract,
                message,
            );
            self.observed_request_shapes.lock().expect("request shapes").push(request_shape.clone());
            if let Some(final_text) = if role.id == "requirements-reviewer" { self.reviewer_final_text.or(self.direct_final_text) } else { self.direct_final_text } {
                return Ok(ModelInitialTurn::FinalResponse(ModelFinalTurn {
                    provider: "fake".to_string(),
                    model: model_name.to_string(),
                    final_text: final_text.to_string(),
                    request_shape,
                    raw_response: json!({"output":[{"type":"message","content":[{"type":"output_text","text":final_text}]}]}),
                }));
            }
            Ok(ModelInitialTurn::ToolCall(ModelToolTurn {
                provider: "fake".to_string(),
                model: model_name.to_string(),
                assistant_summary: "fake tool call".to_string(),
                tool_call: ToolCallRequest {
                    call_identity: "fake-call".to_string(),
                    tool_name: self.tool_name.unwrap_or("execute_code").to_string(),
                    arguments: self.tool_arguments.clone().unwrap_or_else(|| json!({"source": "output(\"fake-ok\")"})),
                },
                request_shape,
                raw_response: json!({"output":[{"type":"function_call","name":"execute_code","call_id":"fake-call","arguments":"{\"source\":\"output(\\\"fake-ok\\\")\"}"}]}),
            }))
        }

        async fn submit_tool_result(&self, role: &crate::roles::RoleSnapshot, history: &[ModelHistoryItem], runtime_messages: &[RuntimeInputMessage], _tool_call_response: &Value, call_id: &str, tool_result: &Value) -> anyhow::Result<ModelFinalTurn> {
            if let Some(message) = self.final_error {
                anyhow::bail!("{message}");
            }
            let model_name = self.model_name.unwrap_or("fake-model");
            Ok(ModelFinalTurn {
                provider: "fake".to_string(),
                model: model_name.to_string(),
                final_text: format!("fake final {call_id} {}", tool_result.get("status").and_then(Value::as_str).unwrap_or("unknown")),
                request_shape: json!({"model":model_name,"input": crate::model_input::responses_input(role, history, runtime_messages, None), "toolResult":tool_result}),
                raw_response: json!({"output":[{"type":"message","content":[{"type":"output_text","text":"fake final"}]}]}),
            })
        }
    }

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

    async fn assert_turn_terminal(pool: &PgPool, session_id: Uuid, input: &str, expected_status: &str, expected_event_status: &str) {
        let row = sqlx::query("SELECT id, status FROM turns WHERE session_id=$1 AND input_text=$2 ORDER BY started_at DESC LIMIT 1")
            .bind(session_id)
            .bind(input)
            .fetch_one(pool)
            .await
            .expect("terminal turn row");
        let turn_id: Uuid = row.get("id");
        let status: String = row.get("status");
        assert_eq!(status, expected_status);
        assert_ne!(status, "running");
        let completed_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND turn_id=$2 AND event_type='turn.completed' AND status=$3")
            .bind(session_id)
            .bind(turn_id)
            .bind(expected_event_status)
            .fetch_one(pool)
            .await
            .expect("terminal event count");
        assert!(completed_events >= 1, "terminal turn event missing for {turn_id}");
    }

    #[tokio::test]
    async fn forced_routing_failure_leaves_terminal_turn() {
        let test_db = validation_db().await;
        let mut role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("lifecycle-route"), ".", Some("."), None, None).await.expect("session");
        role.routing.default_recipient = Some("missing-routing-recipient".to_string());
        role.routing.allowed_recipients = vec!["missing-routing-recipient".to_string()];
        sqlx::query("UPDATE sessions SET role_snapshot=$2 WHERE id=$1")
            .bind(session_id)
            .bind(crate::roles::snapshot_to_value(&role).expect("snapshot value"))
            .execute(&test_db.pool)
            .await
            .expect("corrupt route snapshot");
        let model = FakeModelClient::default();
        let result = crate::runtime::send_with_model_client(&test_db.pool, session_id, "routing forced failure", &model, compaction::CompactionBudget::default()).await;
        assert!(result.is_err());
        assert_turn_terminal(&test_db.pool, session_id, "routing forced failure", "failed", "failed").await;
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn forced_model_dispatch_failure_leaves_terminal_turn() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("lifecycle-model"), ".", Some("."), None, None).await.expect("session");
        let model = FakeModelClient { request_error: Some("forced model dispatch failure"), ..Default::default() };
        let result = crate::runtime::send_with_model_client(&test_db.pool, session_id, "model forced failure", &model, compaction::CompactionBudget::default()).await;
        assert!(result.is_err());
        assert_turn_terminal(&test_db.pool, session_id, "model forced failure", "failed", "failed").await;
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn send_persists_context_snapshot_and_dispatches_developer_role_context_without_instructions() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("context-proof"), "/tmp/context-proof", Some("/tmp/context-proof"), None, None).await.expect("session");
        let model = FakeModelClient { direct_final_text: Some("The current CWD is /tmp/context-proof."), ..Default::default() };
        let turn_id = crate::runtime::send_with_model_client(&test_db.pool, session_id, "what is my CWD?", &model, compaction::CompactionBudget::default()).await.expect("send");
        let shape = model.observed_request_shapes.lock().expect("request shapes").first().cloned().expect("shape");
        assert!(shape.get("instructions").is_none(), "role/runtime context must not use Responses instructions: {shape}");
        assert!(shape.get("previous_response_id").is_none(), "production stateless request must not use previous_response_id: {shape}");
        let input = shape["input"].as_array().expect("input array");
        assert!(input.iter().any(|item| item["role"] == "developer" && item["content"][0]["text"].as_str().unwrap_or_default().contains("<role_instructions")));
        assert!(input.iter().any(|item| item["role"] == "developer" && item["content"][0]["text"].as_str().unwrap_or_default().contains("<runtime_context")));
        assert!(serde_json::to_string(&shape).unwrap().contains("/tmp/context-proof"), "known CWD must be model-visible without a tool call");
        let snapshot: Value = sqlx::query_scalar("SELECT snapshot FROM session_context_snapshots WHERE session_id=$1 AND turn_id=$2 ORDER BY context_epoch DESC LIMIT 1")
            .bind(session_id)
            .bind(turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("context snapshot");
        assert_eq!(snapshot.pointer("/cwd/path").and_then(Value::as_str), Some("/tmp/context-proof"));
        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM session_context_events WHERE session_id=$1 AND turn_id=$2")
            .bind(session_id)
            .bind(turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("context events");
        assert!(event_count >= 1);
        let projection = crate::projection::build_runtime_projection_snapshot(&test_db.pool, Some(session_id)).await.expect("projection");
        let visible_chat = serde_json::to_string(&projection.selected_chat_entries).expect("chat json");
        assert!(!visible_chat.contains("<role_instructions"), "developer role context must stay out of visible chat timeline");
        assert!(!visible_chat.contains("<runtime_context"), "developer runtime context must stay out of visible chat timeline");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn cwd_change_persists_context_event_and_next_request_contains_bounded_delta() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("context-cwd-delta"), "/tmp/old-cwd", Some("/tmp/old-cwd"), None, None).await.expect("session");
        let model = FakeModelClient { direct_final_text: Some("ok"), ..Default::default() };
        crate::runtime::send_with_model_client(&test_db.pool, session_id, "first", &model, compaction::CompactionBudget::default()).await.expect("first send");
        sqlx::query("UPDATE sessions SET workdir=$2, worktree_root=$2, updated_at=now() WHERE id=$1")
            .bind(session_id)
            .bind("/tmp/new-cwd")
            .execute(&test_db.pool)
            .await
            .expect("update cwd");
        crate::runtime::send_with_model_client(&test_db.pool, session_id, "what changed?", &model, compaction::CompactionBudget::default()).await.expect("second send");
        let shape = model.observed_request_shapes.lock().expect("request shapes").last().cloned().expect("shape");
        let rendered = serde_json::to_string(&shape).expect("shape json");
        assert!(rendered.contains("<context_delta epoch=\\\"2\\\" previous_epoch=\\\"1\\\">"), "{rendered}");
        assert!(rendered.contains("kind=\\\"cwd_changed\\\""), "{rendered}");
        assert!(rendered.contains("old_cwd=/tmp/old-cwd"), "{rendered}");
        assert!(rendered.contains("new_cwd=/tmp/new-cwd"), "{rendered}");
        let event: (String, Value) = sqlx::query_as("SELECT event_kind, payload FROM session_context_events WHERE session_id=$1 ORDER BY sequence DESC LIMIT 1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("event");
        assert_eq!(event.0, "cwd_changed");
        assert_eq!(event.1.pointer("/snapshot/cwd/path").and_then(Value::as_str), Some("/tmp/new-cwd"));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn role_epoch_change_emits_new_role_block_and_transition_summary_not_context_delta() {
        let test_db = validation_db().await;
        let mut role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("context-role-epoch"), ".", Some("."), None, None).await.expect("session");
        let model = FakeModelClient { direct_final_text: Some("ok"), ..Default::default() };
        crate::runtime::send_with_model_client(&test_db.pool, session_id, "first", &model, compaction::CompactionBudget::default()).await.expect("first send");
        role.version = "epoch-test-2".to_string();
        role.role_version_id = Uuid::new_v4();
        role.instruction_text = "new role epoch instructions".to_string();
        let snapshot = crate::roles::snapshot_to_value(&role).expect("snapshot");
        sqlx::query("UPDATE sessions SET role_version=$2, role_snapshot=$3, updated_at=now() WHERE id=$1")
            .bind(session_id)
            .bind(&role.version)
            .bind(snapshot)
            .execute(&test_db.pool)
            .await
            .expect("role update");
        crate::runtime::send_with_model_client(&test_db.pool, session_id, "second", &model, compaction::CompactionBudget::default()).await.expect("second send");
        let shape = model.observed_request_shapes.lock().expect("request shapes").last().cloned().expect("shape");
        let rendered = serde_json::to_string(&shape).expect("shape json");
        assert!(rendered.contains("new role epoch instructions"), "{rendered}");
        assert!(rendered.contains("<role_transition_summary"), "{rendered}");
        assert!(!rendered.contains("kind=\\\"role_epoch_changed\\\""), "role change must not be represented as ordinary context_delta: {rendered}");
        let event_kind: String = sqlx::query_scalar("SELECT event_kind FROM session_context_events WHERE session_id=$1 ORDER BY sequence DESC LIMIT 1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("event");
        assert_eq!(event_kind, "role_epoch_changed");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn tool_command_registry_change_persists_event_and_next_request_contains_delta() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("context-tool-delta"), ".", Some("."), None, None).await.expect("session");
        let model = FakeModelClient { direct_final_text: Some("ok"), ..Default::default() };
        crate::runtime::send_with_model_client(&test_db.pool, session_id, "first", &model, compaction::CompactionBudget::default()).await.expect("first send");
        let seed = scoped_command_seed("cmd.context.delta", "context_delta");
        apply_registry_seed(&test_db.pool, session_id, seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;
        crate::runtime::send_with_model_client(&test_db.pool, session_id, "second", &model, compaction::CompactionBudget::default()).await.expect("second send");
        let shape = model.observed_request_shapes.lock().expect("request shapes").last().cloned().expect("shape");
        let rendered = serde_json::to_string(&shape).expect("shape json");
        assert!(rendered.contains("<context_delta"), "{rendered}");
        assert!(rendered.contains("tool_context_changed"), "{rendered}");
        assert!(rendered.contains("new_command_context=cmdctx-"), "{rendered}");
        let event_kind: String = sqlx::query_scalar("SELECT event_kind FROM session_context_events WHERE session_id=$1 ORDER BY sequence DESC LIMIT 1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("event");
        assert_eq!(event_kind, "tool_context_changed");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn forced_tool_execution_failure_leaves_terminal_turn() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("lifecycle-tool"), ".", Some("."), None, None).await.expect("session");
        let model = FakeModelClient { tool_arguments: Some(json!({})), ..Default::default() };
        let turn_id = crate::runtime::send_with_model_client(&test_db.pool, session_id, "tool forced failure", &model, compaction::CompactionBudget::default()).await.expect("tool failure send returns terminal turn");
        assert_turn_terminal(&test_db.pool, session_id, "tool forced failure", "failed", "failed").await;
        let tool_status: String = sqlx::query_scalar("SELECT status FROM tool_calls WHERE turn_id=$1")
            .bind(turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("tool status");
        assert_eq!(tool_status, "failed");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn direct_assistant_response_completes_without_tool_call() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-allow").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("direct-final"), ".", Some("."), None, None).await.expect("session");
        let model = FakeModelClient {
            direct_final_text: Some("Hi! How can I help?"),
            ..Default::default()
        };
        let turn_id = crate::runtime::send_with_model_client(&test_db.pool, session_id, "Hi", &model, compaction::CompactionBudget::default())
            .await
            .expect("direct assistant send succeeds");

        assert_turn_terminal(&test_db.pool, session_id, "Hi", "completed", "completed").await;
        let tool_call_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tool_calls WHERE turn_id=$1")
            .bind(turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("tool call count");
        assert_eq!(tool_call_count, 0, "direct assistant response must not synthesize a tool call");
        let final_text: String = sqlx::query_scalar("SELECT payload->>'finalText' FROM event_stream WHERE turn_id=$1 AND event_type='model.final_response'")
            .bind(turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("final response event");
        assert_eq!(final_text, "Hi! How can I help?");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn forced_cancellation_boundary_leaves_terminal_turn() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("lifecycle-cancel"), ".", Some("."), None, None).await.expect("session");
        let model = FakeModelClient { final_error: Some("request cancellation forced by test fixture"), ..Default::default() };
        let result = crate::runtime::send_with_model_client(&test_db.pool, session_id, "cancellation forced failure", &model, compaction::CompactionBudget::default()).await;
        assert!(result.is_err());
        assert_turn_terminal(&test_db.pool, session_id, "cancellation forced failure", "failed", "failed").await;
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn startup_reconciliation_marks_seeded_running_rows_lost_and_appends_recovery_events() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("reconcile-project"), ".", Some("."), None, None).await.expect("session");
        sqlx::query("UPDATE sessions SET updated_at = now() - interval '1 hour' WHERE id=$1")
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("age session");
        let before_updated_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT updated_at FROM sessions WHERE id=$1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("before updated_at");
        let turn_id = Uuid::new_v4();
        let tool_id = Uuid::new_v4();
        let script_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let command_id = Uuid::new_v4();
        let process_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user','stale running turn','running',now() - interval '10 minutes')")
            .bind(turn_id).bind(session_id).execute(&test_db.pool).await.expect("turn");
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status, started_at) VALUES ($1,$2,$3,'execute_code','stale-tool','{}'::jsonb,'running',now() - interval '10 minutes')")
            .bind(tool_id).bind(session_id).bind(turn_id).execute(&test_db.pool).await.expect("tool");
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status, started_at) VALUES ($1,$2,'output(1)','running',now() - interval '10 minutes')")
            .bind(script_id).bind(tool_id).execute(&test_db.pool).await.expect("script");
        sqlx::query("INSERT INTO host_api_calls (id, script_run_id, api_name, input, status, started_at) VALUES ($1,$2,'fs.read','{}'::jsonb,'running',now() - interval '10 minutes')")
            .bind(host_id).bind(script_id).execute(&test_db.pool).await.expect("host");
        sqlx::query("INSERT INTO command_runs (id, host_api_call_id, binary_name, argv, cwd, status, started_at) VALUES ($1,$2,'echo','[]'::jsonb,'.','running',now() - interval '10 minutes')")
            .bind(command_id).bind(host_id).execute(&test_db.pool).await.expect("command");
        sqlx::query("INSERT INTO managed_processes (id, handle, session_id, starting_turn_id, binary_name, argv, cwd, status, end_of_turn_behavior, end_of_session_behavior, metadata, start_time) VALUES ($1,'stale-process',$2,$3,'sleep','[]'::jsonb,'.','running','terminate','terminate','{}'::jsonb,now() - interval '10 minutes')")
            .bind(process_id).bind(session_id).bind(turn_id).execute(&test_db.pool).await.expect("process");

        let runtime_summary = db::reconcile_running_runtime_rows(&test_db.pool, "startupTest").await.expect("runtime reconcile");
        let process_summary = db::reconcile_managed_processes(&test_db.pool, "startupTest").await.expect("process reconcile");
        assert_eq!(runtime_summary.lost_turns, 1);
        assert_eq!(runtime_summary.lost_tool_calls, 1);
        assert_eq!(runtime_summary.lost_script_runs, 1);
        assert_eq!(runtime_summary.lost_host_api_calls, 1);
        assert_eq!(runtime_summary.lost_command_runs, 1);
        assert_eq!(process_summary.lost_processes, 1);
        for table in ["turns", "tool_calls", "script_runs", "host_api_calls", "command_runs", "managed_processes"] {
            let lost: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE status='lost'"))
                .fetch_one(&test_db.pool)
                .await
                .expect("lost count");
            assert_eq!(lost, 1, "{table} lost count");
        }
        let events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type IN ('turn.lost','tool.lost','script.lost','host_api.lost','command.lost','process.lost','session.recovered','session.recoveryDegraded')")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("recovery events");
        assert!(events >= 8);
        let after_updated_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT updated_at FROM sessions WHERE id=$1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("after updated_at");
        assert!(after_updated_at > before_updated_at);
        db::ensure_active_turn_constraint(&test_db.pool).await.expect("active turn constraint");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn database_rejects_more_than_one_running_turn_per_open_session() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("active-turn-constraint"), ".", Some("."), None, None).await.expect("session");
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status) VALUES ($1,$2,'user','first active','running')")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("first running turn");
        let duplicate = sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status) VALUES ($1,$2,'user','second active','running')")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .execute(&test_db.pool)
            .await;
        assert!(duplicate.is_err(), "unique partial index must reject a second running turn for the same session");
        test_db.cleanup().await;
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
            json!({"role":"runtime-no-rg","project":"server-validation","model":"fake-model","workdir":".","worktreeRoot":".","title":"Server validation","name":"server-validation"}),
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
    async fn gui_project_crud_projection_and_unassigned_filter_validation() {
        let test_db = validation_db().await;
        let state = ServerState::new_with_identity(test_db.pool.clone(), "project-crud-validation".to_string());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let base_url = format!("http://{addr}");
        let transport = GuiTransportHandle::spawn();

        let _ = transport
            .send(GuiTransportRequestPacket {
                packet_id: "project-connect".to_string(),
                intent: GuiTransportRequest::Connect { base_url: base_url.clone(), selected_session_id: None },
            })
            .await;

        let create = transport
            .send(GuiTransportRequestPacket {
                packet_id: "project-create".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CreateProject {
                        project_key: "zeta-project".to_string(),
                        display_name: "Zeta Project".to_string(),
                        default_workdir: "/tmp/zeta".to_string(),
                        default_worktree_root: "/tmp/zeta".to_string(),
                        default_role_id: Some("runtime-no-rg".to_string()),
                        default_model: "gpt-5.4-mini".to_string(),
                    },
                },
            })
            .await;
        assert!(create.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model } if view_model.shell.projects.iter().any(|row| row.id == "zeta-project" && row.title == "Zeta Project"))), "project create must update Workbench projection: {create:?}");

        let update = transport
            .send(GuiTransportRequestPacket {
                packet_id: "project-update".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::UpdateProject {
                        project_key: "zeta-project".to_string(),
                        display_name: "Zeta Updated".to_string(),
                        default_workdir: "/tmp/zeta-updated".to_string(),
                        default_worktree_root: "/tmp/zeta-root".to_string(),
                        default_role_id: Some("runtime-allow".to_string()),
                        default_model: "gpt-5.5".to_string(),
                    },
                },
            })
            .await;
        assert!(update.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model } if view_model.shell.projects.iter().any(|row| row.id == "zeta-project" && row.title == "Zeta Updated"))), "project update must update rendered rail data: {update:?}");

        let row: (String, String, String, Option<String>, String, bool) = sqlx::query_as(
            "SELECT display_name, default_workdir, default_worktree_root, default_role_id, default_model, archived FROM projects WHERE project_key='zeta-project'",
        )
        .fetch_one(&test_db.pool)
        .await
        .expect("project row");
        assert_eq!(row.0, "Zeta Updated");
        assert_eq!(row.1, "/tmp/zeta-updated");
        assert_eq!(row.2, "/tmp/zeta-root");
        assert_eq!(row.3.as_deref(), Some("runtime-allow"));
        assert_eq!(row.4, "gpt-5.5");
        assert!(!row.5);

        let archived = transport
            .send(GuiTransportRequestPacket {
                packet_id: "project-archive".to_string(),
                intent: GuiTransportRequest::DispatchOperation { operation: GuiOperationRequest::ArchiveProject { project_key: "zeta-project".to_string() } },
            })
            .await;
        assert!(archived.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model } if !view_model.shell.projects.iter().any(|row| row.id == "zeta-project"))), "archived project must leave normal rail: {archived:?}");
        let archived_flag: bool = sqlx::query_scalar("SELECT archived FROM projects WHERE project_key='zeta-project'")
            .fetch_one(&test_db.pool)
            .await
            .expect("archived flag");
        assert!(archived_flag, "archive action must persist archived=true");

        let unarchived = transport
            .send(GuiTransportRequestPacket {
                packet_id: "project-unarchive".to_string(),
                intent: GuiTransportRequest::DispatchOperation { operation: GuiOperationRequest::UnarchiveProject { project_key: "zeta-project".to_string() } },
            })
            .await;
        assert!(unarchived.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model } if view_model.shell.projects.iter().any(|row| row.id == "zeta-project"))), "unarchived project must return to rail: {unarchived:?}");
        let unarchived_flag: bool = sqlx::query_scalar("SELECT archived FROM projects WHERE project_key='zeta-project'")
            .fetch_one(&test_db.pool)
            .await
            .expect("unarchived flag");
        assert!(!unarchived_flag, "unarchive action must persist archived=false");

        let unassigned_session = transport
            .send(GuiTransportRequestPacket {
                packet_id: "unassigned-create".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CreateSession {
                        role: "runtime-no-rg".to_string(),
                        project: Some("__unassigned__".to_string()),
                        model: Some("gpt-5.4-mini".to_string()),
                        workdir: Some(".".to_string()),
                        worktree_root: Some(".".to_string()),
                        title: Some("Unassigned proof".to_string()),
                        name: Some("unassigned-proof".to_string()),
                    },
                },
            })
            .await;
        let session_id = unassigned_session
            .iter()
            .find_map(|packet| match &packet.output {
                GuiTransportOutput::OperationResult { result } => match &result.outcome {
                    GuiOperationOutcome::Accepted { entity_id: Some(id) } => Some(id.clone()),
                    _ => None,
                },
                _ => None,
            })
            .expect("unassigned session id");
        let project_key: Option<String> = sqlx::query_scalar("SELECT project_key FROM sessions WHERE id=$1")
            .bind(Uuid::parse_str(&session_id).expect("session uuid"))
            .fetch_one(&test_db.pool)
            .await
            .expect("session project key");
        assert!(project_key.is_none());

        let _ = transport
            .send(GuiTransportRequestPacket {
                packet_id: "select-unassigned-filter".to_string(),
                intent: GuiTransportRequest::SelectProject { project_id: "__unassigned__".to_string() },
            })
            .await;
        let unassigned_view = transport
            .send(GuiTransportRequestPacket {
                packet_id: "unassigned-rehydrate".to_string(),
                intent: GuiTransportRequest::Rehydrate { selected_session_id: None },
            })
            .await;
        assert!(unassigned_view.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model } if view_model.shell.sessions.iter().any(|row| row.id == session_id) && view_model.shell.projects[0].id == "__all__" && view_model.shell.projects[1].id == "__unassigned__")), "Unassigned filter must show projectless sessions and stable project row order: {unassigned_view:?}");

        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn project_create_update_reject_obsolete_tracked_listed_fields() {
        let test_db = validation_db().await;
        let state = ServerState::new_with_identity(test_db.pool.clone(), "project-obsolete-fields-validation".to_string());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let base_url = format!("http://{addr}");
        let http = reqwest::Client::new();

        let create = http
            .post(format!("{base_url}/projects"))
            .json(&json!({
                "projectKey": "obsolete-project",
                "displayName": "Obsolete Project",
                "defaultWorkdir": ".",
                "defaultWorktreeRoot": ".",
                "defaultRoleId": "runtime-no-rg",
                "defaultModel": "gpt-5.4-mini",
                "tracked": true,
                "listed": true
            }))
            .send()
            .await
            .expect("create response");
        assert_eq!(create.status(), axum::http::StatusCode::BAD_REQUEST);

        let created: Value = http
            .post(format!("{base_url}/projects"))
            .json(&json!({
                "projectKey": "obsolete-project",
                "displayName": "Obsolete Project",
                "defaultWorkdir": ".",
                "defaultWorktreeRoot": ".",
                "defaultRoleId": "runtime-no-rg",
                "defaultModel": "gpt-5.4-mini"
            }))
            .send()
            .await
            .expect("create response")
            .error_for_status()
            .expect("create ok")
            .json()
            .await
            .expect("create json");
        assert_eq!(created["project"]["projectKey"], "obsolete-project");
        assert!(created["project"].get("tracked").is_none());
        assert!(created["project"].get("listed").is_none());

        let update = http
            .post(format!("{base_url}/projects/obsolete-project"))
            .json(&json!({
                "displayName": "Obsolete Updated",
                "defaultWorkdir": ".",
                "defaultWorktreeRoot": ".",
                "defaultRoleId": "runtime-allow",
                "defaultModel": "gpt-5.5",
                "tracked": false,
                "listed": false
            }))
            .send()
            .await
            .expect("update response");
        assert_eq!(update.status(), axum::http::StatusCode::BAD_REQUEST);

        let row: (String, bool) = sqlx::query_as("SELECT display_name, archived FROM projects WHERE project_key='obsolete-project'")
            .fetch_one(&test_db.pool)
            .await
            .expect("project row");
        assert_eq!(row.0, "Obsolete Project");
        assert!(!row.1);

        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn gui_disconnect_returns_disconnected_surface_without_mutating_sessions_or_projects() {
        let test_db = validation_db().await;
        let state = ServerState::new_with_identity(test_db.pool.clone(), "disconnect-validation".to_string());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let base_url = format!("http://{addr}");
        let transport = GuiTransportHandle::spawn();

        let _ = db::create_project(
            &test_db.pool,
            "disconnect-project",
            "Disconnect Project",
            "/tmp/disconnect",
            "/tmp/disconnect",
            Some("runtime-no-rg"),
            "gpt-5.4-mini",
        )
        .await
        .expect("project");
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg")
            .await
            .expect("role");
        let _session_id = db::new_session(
            &test_db.pool,
            &role,
            Some("disconnect-project"),
            "/tmp/disconnect",
            Some("/tmp/disconnect"),
            Some("Disconnect proof"),
            Some("disconnect-proof"),
        )
        .await
        .expect("session");

        let before_sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(&test_db.pool)
            .await
            .expect("session count");
        let before_projects: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(&test_db.pool)
            .await
            .expect("project count");

        let connect = transport
            .send(GuiTransportRequestPacket {
                packet_id: "disconnect-connect".to_string(),
                intent: GuiTransportRequest::Connect { base_url, selected_session_id: None },
            })
            .await;
        assert!(connect.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::WorkbenchView { view_model } if view_model.connection_state == "connected" || view_model.connection_state == "streaming"
        )), "connect must render connected Workbench view: {connect:?}");

        let disconnect = transport
            .send(GuiTransportRequestPacket {
                packet_id: "disconnect-click".to_string(),
                intent: GuiTransportRequest::Disconnect,
            })
            .await;
        assert!(disconnect.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::OperationResult { result } if matches!(result.outcome, GuiOperationOutcome::Accepted { .. })
        )), "disconnect must dispatch typed Rust operation: {disconnect:?}");
        assert!(disconnect.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::ControllerState { controller_state }
                if controller_state["connectionState"] == "disconnected"
        )), "disconnect must close Rust-owned sync state: {disconnect:?}");
        assert!(disconnect.iter().any(|packet| matches!(
            &packet.output,
            GuiTransportOutput::WorkbenchView { view_model }
                if view_model.connection_state == "disconnected" && view_model.discovery.title.contains("Discovery")
        )), "disconnect must render disconnected discovery surface: {disconnect:?}");

        let after_sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(&test_db.pool)
            .await
            .expect("session count after");
        let after_projects: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(&test_db.pool)
            .await
            .expect("project count after");
        assert_eq!(before_sessions, after_sessions, "disconnect must not mutate sessions");
        assert_eq!(before_projects, after_projects, "disconnect must not mutate projects");

        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn unassigned_migration_and_project_filter_semantics_preserve_selected_filter() {
        let test_db = validation_db().await;
        let state = ServerState::new_with_identity(test_db.pool.clone(), "unassigned-filter-validation".to_string());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let base_url = format!("http://{addr}");
        let transport = GuiTransportHandle::spawn();

        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg")
            .await
            .expect("role");
        let _ = db::create_project(&test_db.pool, "alpha-project", "Alpha Project", "/tmp/alpha", "/tmp/alpha", Some("runtime-no-rg"), "gpt-5.4-mini")
            .await
            .expect("alpha project");
        let _ = db::create_project(&test_db.pool, "zeta-project", "Zeta Project", "/tmp/zeta", "/tmp/zeta", Some("runtime-no-rg"), "gpt-5.4-mini")
            .await
            .expect("zeta project");
        let historical_unassigned = db::new_session(&test_db.pool, &role, None, "/tmp/unassigned", Some("/tmp/unassigned"), Some("Historical unassigned"), Some("historical-unassigned"))
            .await
            .expect("historical unassigned");
        let zeta_session = db::new_session(&test_db.pool, &role, Some("zeta-project"), "/tmp/zeta", Some("/tmp/zeta"), Some("Zeta session"), Some("zeta-session"))
            .await
            .expect("zeta session");

        let _ = transport
            .send(GuiTransportRequestPacket {
                packet_id: "filter-connect".to_string(),
                intent: GuiTransportRequest::Connect { base_url, selected_session_id: None },
            })
            .await;

        let unassigned = transport
            .send(GuiTransportRequestPacket {
                packet_id: "filter-unassigned".to_string(),
                intent: GuiTransportRequest::SelectProject { project_id: "__unassigned__".to_string() },
            })
            .await;
        assert!(unassigned.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
            if view_model.shell.projects.iter().map(|row| row.id.as_str()).take(4).collect::<Vec<_>>() == vec!["__all__", "__unassigned__", "alpha-project", "zeta-project"]
                && view_model.shell.sessions.iter().any(|row| row.id == historical_unassigned.to_string())
                && !view_model.shell.sessions.iter().any(|row| row.id == zeta_session.to_string())
        )), "historical projectless session must appear under Unassigned only, with sorted project rows: {unassigned:?}");

        let zeta_filter = transport
            .send(GuiTransportRequestPacket {
                packet_id: "filter-zeta".to_string(),
                intent: GuiTransportRequest::SelectProject { project_id: "zeta-project".to_string() },
            })
            .await;
        assert!(zeta_filter.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
            if view_model.shell.projects.iter().any(|row| row.id == "zeta-project" && row.subtitle.contains("Selected"))
                && view_model.shell.sessions.iter().any(|row| row.id == zeta_session.to_string())
                && !view_model.shell.sessions.iter().any(|row| row.id == historical_unassigned.to_string())
        )), "selecting a non-first project filter must filter sessions and preserve row selection: {zeta_filter:?}");
        let zeta_view = transport
            .send(GuiTransportRequestPacket {
                packet_id: "filter-zeta-select-session".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::SelectSession { session_id: Some(zeta_session.to_string()) },
                },
            })
            .await;
        assert!(zeta_view.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
            if view_model.shell.projects.iter().any(|row| row.id == "zeta-project" && row.subtitle.contains("Selected"))
                && view_model.shell.sessions.iter().any(|row| row.id == zeta_session.to_string())
        )), "selecting a session inside a non-first project must preserve the active project filter: {zeta_view:?}");

        let outside_filter_selection = transport
            .send(GuiTransportRequestPacket {
                packet_id: "filter-zeta-select-outside-session".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::SelectSession { session_id: Some(historical_unassigned.to_string()) },
                },
            })
            .await;
        assert!(outside_filter_selection.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::Error { error }
            if error.error.code == "project_filter_mismatch"
                && error.error.details["selectedProjectId"] == "zeta-project"
                && error.error.details["sessionId"] == historical_unassigned.to_string()
        )), "selecting a session outside the active project filter must return a typed visible error: {outside_filter_selection:?}");
        assert!(outside_filter_selection.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
            if view_model.shell.projects.iter().any(|row| row.id == "zeta-project" && row.subtitle.contains("Selected"))
                && view_model.shell.selected_session_id == Some(zeta_session.to_string())
                && !view_model.shell.sessions.iter().any(|row| row.id == historical_unassigned.to_string())
        )), "outside-filter rejection must preserve selected project filter and not show the rejected session: {outside_filter_selection:?}");

        let move_to_project = transport
            .send(GuiTransportRequestPacket {
                packet_id: "unassigned-to-project".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::UpdateSessionSettings {
                        session_id: historical_unassigned.to_string(),
                        project: "alpha-project".to_string(),
                        role: "runtime-no-rg".to_string(),
                        model: "gpt-5.4-mini".to_string(),
                        workdir: "/tmp/alpha".to_string(),
                        worktree_root: "/tmp/alpha".to_string(),
                        title: "Moved to alpha".to_string(),
                        name: "moved-to-alpha".to_string(),
                        tracked: true,
                    },
                },
            })
            .await;
        assert!(move_to_project.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::OperationResult { result } if matches!(result.outcome, GuiOperationOutcome::Accepted { .. }))));
        let moved_key: Option<String> = sqlx::query_scalar("SELECT project_key FROM sessions WHERE id=$1")
            .bind(historical_unassigned)
            .fetch_one(&test_db.pool)
            .await
            .expect("moved key");
        assert_eq!(moved_key.as_deref(), Some("alpha-project"));

        let move_to_unassigned = transport
            .send(GuiTransportRequestPacket {
                packet_id: "project-to-unassigned".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::UpdateSessionSettings {
                        session_id: historical_unassigned.to_string(),
                        project: "__unassigned__".to_string(),
                        role: "runtime-no-rg".to_string(),
                        model: "gpt-5.4-mini".to_string(),
                        workdir: "/tmp/unassigned".to_string(),
                        worktree_root: "/tmp/unassigned".to_string(),
                        title: "Moved back unassigned".to_string(),
                        name: "moved-back-unassigned".to_string(),
                        tracked: true,
                    },
                },
            })
            .await;
        assert!(move_to_unassigned.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::OperationResult { result } if matches!(result.outcome, GuiOperationOutcome::Accepted { .. }))));
        let unassigned_key: Option<String> = sqlx::query_scalar("SELECT project_key FROM sessions WHERE id=$1")
            .bind(historical_unassigned)
            .fetch_one(&test_db.pool)
            .await
            .expect("unassigned key");
        assert!(unassigned_key.is_none());

        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn gui_session_creation_settings_and_lifecycle_persist_and_reproject() {
        let test_db = validation_db().await;
        let state = ServerState::new_with_identity(test_db.pool.clone(), "session-settings-lifecycle-validation".to_string());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let base_url = format!("http://{addr}");
        let transport = GuiTransportHandle::spawn();

        let _ = db::create_project(&test_db.pool, "alpha-project", "Alpha Project", "/tmp/alpha", "/tmp/alpha-root", Some("runtime-no-rg"), "gpt-5.4-mini")
            .await
            .expect("alpha project");
        let _ = db::create_project(&test_db.pool, "zeta-project", "Zeta Project", "/tmp/zeta", "/tmp/zeta-root", Some("runtime-no-rg"), "gpt-5.4-mini")
            .await
            .expect("zeta project");

        let _ = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-connect".to_string(),
                intent: GuiTransportRequest::Connect { base_url, selected_session_id: None },
            })
            .await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg")
            .await
            .expect("role");
        let blocker_session = db::new_session(
            &test_db.pool,
            &role,
            Some("zeta-project"),
            "/tmp/zeta/blocker",
            Some("/tmp/zeta-root"),
            Some("Process blocker"),
            Some("process-blocker"),
        )
        .await
        .expect("blocker session");
        let blocker_process = Uuid::new_v4();
        sqlx::query("INSERT INTO managed_processes (id, handle, session_id, binary_name, argv, cwd, status, end_of_turn_behavior, end_of_session_behavior, metadata, start_time) VALUES ($1,'blocker',$2,'sleep','[]'::jsonb,'/tmp','running','continue','block','{}'::jsonb,now())")
            .bind(blocker_process)
            .bind(blocker_session)
            .execute(&test_db.pool)
            .await
            .expect("blocking managed process");
        let blocked_close = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-close-blocked-by-process".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CloseSession {
                        session_id: blocker_session.to_string(),
                        reason: Some("blocked process policy proof".to_string()),
                    },
                },
            })
            .await;
        assert!(blocked_close.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::OperationResult { result } if matches!(&result.outcome, GuiOperationOutcome::Error { error } if error.error.code == "conflict" && error.error.message.contains("managed processes")))), "close must surface typed process-policy failure: {blocked_close:?}");
        let blocked_event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='session.closeBlocked'")
            .bind(blocker_session)
            .fetch_one(&test_db.pool)
            .await
            .expect("close blocked event count");
        assert_eq!(blocked_event_count, 1, "process-policy close failure must append a recovery/audit event");

        let terminable_session = db::new_session(
            &test_db.pool,
            &role,
            Some("zeta-project"),
            "/tmp/zeta/terminable",
            Some("/tmp/zeta-root"),
            Some("Terminable process session"),
            Some("terminable-process-session"),
        )
        .await
        .expect("terminable session");
        let (terminable_process, terminable_handle) =
            starlark_host::register_test_terminable_process(terminable_session)
                .expect("live terminable process");
        sqlx::query("INSERT INTO managed_processes (id, handle, session_id, binary_name, argv, cwd, status, end_of_turn_behavior, end_of_session_behavior, metadata, start_time) VALUES ($1,$2,$3,'sleep','[\"30\"]'::jsonb,'.','running','continue','terminate','{}'::jsonb,now())")
            .bind(terminable_process)
            .bind(&terminable_handle)
            .bind(terminable_session)
            .execute(&test_db.pool)
            .await
            .expect("terminable managed process row");
        let terminable_close = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-close-terminates-process".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CloseSession {
                        session_id: terminable_session.to_string(),
                        reason: Some("terminates live process policy proof".to_string()),
                    },
                },
            })
            .await;
        assert!(terminable_close.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
            if view_model.shell.sessions.iter().any(|row| row.id == terminable_session.to_string() && row.status == "closed")
        )), "Close must terminate session-ending managed processes and refresh the connected projection: {terminable_close:?}");
        let terminated_process_row: (String, Option<String>, bool) = sqlx::query_as(
            "SELECT status, termination_reason, end_time IS NOT NULL FROM managed_processes WHERE id=$1",
        )
        .bind(terminable_process)
        .fetch_one(&test_db.pool)
        .await
        .expect("terminated process row");
        assert_eq!(terminated_process_row.0, "sessionClosed");
        assert_eq!(terminated_process_row.1.as_deref(), Some("sessionClosed"));
        assert!(terminated_process_row.2);
        let terminable_session_row: (String, Option<String>) =
            sqlx::query_as("SELECT status, close_reason FROM sessions WHERE id=$1")
                .bind(terminable_session)
                .fetch_one(&test_db.pool)
                .await
                .expect("terminable closed session row");
        assert_eq!(terminable_session_row.0, "closed");
        assert_eq!(
            terminable_session_row.1.as_deref(),
            Some("terminates live process policy proof")
        );
        let _ = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-select-zeta".to_string(),
                intent: GuiTransportRequest::SelectProject { project_id: "zeta-project".to_string() },
            })
            .await;

        let create = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-create-session".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CreateSession {
                        role: "runtime-no-rg".to_string(),
                        project: Some("zeta-project".to_string()),
                        model: Some("gpt-5.4-mini".to_string()),
                        workdir: Some("/tmp/zeta/work".to_string()),
                        worktree_root: Some("/tmp/zeta-root".to_string()),
                        title: Some("Zeta modal session".to_string()),
                        name: Some("zeta-modal-session".to_string()),
                    },
                },
            })
            .await;
        let session_id = create
            .iter()
            .find_map(|packet| match &packet.output {
                GuiTransportOutput::OperationResult { result } => match &result.outcome {
                    GuiOperationOutcome::Accepted { entity_id: Some(id) } => Some(id.clone()),
                    _ => None,
                },
                _ => None,
            })
            .expect("created session id");
        assert!(create.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
            if view_model.shell.selected_session_id == Some(session_id.clone())
                && view_model.shell.sessions.iter().any(|row| row.id == session_id && row.title == "Zeta modal session")
        )), "Create Session modal path must select and render the new zeta session under the active filter: {create:?}");

        let created_uuid = Uuid::parse_str(&session_id).expect("created session uuid");
        let created_row: (Option<String>, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>, bool, String) = sqlx::query_as(
            "SELECT project_key, title, name, workdir, worktree_root, role_id, role_snapshot #>> '{modelDefaults,model}', tracked, status FROM sessions WHERE id=$1",
        )
        .bind(created_uuid)
        .fetch_one(&test_db.pool)
        .await
        .expect("created session row");
        assert_eq!(created_row.0.as_deref(), Some("zeta-project"));
        assert_eq!(created_row.1.as_deref(), Some("Zeta modal session"));
        assert_eq!(created_row.2.as_deref(), Some("zeta-modal-session"));
        assert_eq!(created_row.3, "/tmp/zeta/work");
        assert_eq!(created_row.4.as_deref(), Some("/tmp/zeta-root"));
        assert_eq!(created_row.5.as_deref(), Some("runtime-no-rg"));
        assert_eq!(created_row.6.as_deref(), Some("gpt-5.4-mini"));
        assert!(created_row.7);
        assert_eq!(created_row.8, "open");

        let update = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-update-session".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::UpdateSessionSettings {
                        session_id: session_id.clone(),
                        project: "alpha-project".to_string(),
                        role: "runtime-allow".to_string(),
                        model: "gpt-5.5".to_string(),
                        workdir: "/tmp/alpha/updated-work".to_string(),
                        worktree_root: "/tmp/alpha-root-updated".to_string(),
                        title: "Alpha updated title".to_string(),
                        name: "alpha-updated-name".to_string(),
                        tracked: true,
                    },
                },
            })
            .await;
        assert!(update.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::OperationResult { result } if matches!(result.outcome, GuiOperationOutcome::Accepted { .. }))), "session settings save must dispatch typed operation: {update:?}");
        let updated_row: (Option<String>, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>, bool) = sqlx::query_as(
            "SELECT project_key, title, name, workdir, worktree_root, role_id, role_snapshot #>> '{modelDefaults,model}', tracked FROM sessions WHERE id=$1",
        )
        .bind(created_uuid)
        .fetch_one(&test_db.pool)
        .await
        .expect("updated session row");
        assert_eq!(updated_row.0.as_deref(), Some("alpha-project"));
        assert_eq!(updated_row.1.as_deref(), Some("Alpha updated title"));
        assert_eq!(updated_row.2.as_deref(), Some("alpha-updated-name"));
        assert_eq!(updated_row.3, "/tmp/alpha/updated-work");
        assert_eq!(updated_row.4.as_deref(), Some("/tmp/alpha-root-updated"));
        assert_eq!(updated_row.5.as_deref(), Some("runtime-allow"));
        assert_eq!(updated_row.6.as_deref(), Some("gpt-5.5"));
        assert!(updated_row.7);

        let _ = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-select-alpha".to_string(),
                intent: GuiTransportRequest::SelectProject { project_id: "alpha-project".to_string() },
            })
            .await;
        let alpha_view = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-alpha-rehydrate".to_string(),
                intent: GuiTransportRequest::Rehydrate { selected_session_id: Some(session_id.clone()) },
            })
            .await;
        assert!(alpha_view.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
            if view_model.shell.selected_session_id == Some(session_id.clone())
                && view_model.shell.sessions.iter().any(|row| row.id == session_id && row.subtitle.contains("alpha-project") && row.subtitle.contains("runtime-allow") && row.title == "Alpha updated title")
        )), "session settings save must update projection and rendered left rail under the new project: {alpha_view:?}");

        let fork_turn = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at, completed_at) VALUES ($1,$2,'user','fork from completed turn','completed',now(),now())")
            .bind(fork_turn)
            .bind(created_uuid)
            .execute(&test_db.pool)
            .await
            .expect("completed fork turn");
        let fork = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-fork-session".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::ForkSession {
                        session_id: session_id.clone(),
                        at_turn: fork_turn.to_string(),
                    },
                },
            })
            .await;
        let forked_session_id = fork
            .iter()
            .find_map(|packet| match &packet.output {
                GuiTransportOutput::OperationResult { result } => match &result.outcome {
                    GuiOperationOutcome::Accepted { entity_id: Some(id) } => Some(id.clone()),
                    _ => None,
                },
                _ => None,
            })
            .expect("forked session id");
        assert!(fork.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
            if view_model.shell.selected_session_id == Some(forked_session_id.clone())
                && view_model.shell.sessions.iter().any(|row| row.id == forked_session_id)
        )), "Fork must create and select the forked session in the connected GUI: {fork:?}");
        let forked_uuid = Uuid::parse_str(&forked_session_id).expect("forked session uuid");
        let fork_linkage: (Option<Uuid>, Option<Uuid>, i32, Option<String>) = sqlx::query_as(
            "SELECT forked_from_session_id, forked_from_turn_id, fork_depth, project_key FROM sessions WHERE id=$1",
        )
        .bind(forked_uuid)
        .fetch_one(&test_db.pool)
        .await
        .expect("fork linkage");
        assert_eq!(fork_linkage.0, Some(created_uuid));
        assert_eq!(fork_linkage.1, Some(fork_turn));
        assert_eq!(fork_linkage.2, 1);
        assert_eq!(fork_linkage.3.as_deref(), Some("alpha-project"));

        let close = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-close-session".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CloseSession {
                        session_id: forked_session_id.clone(),
                        reason: Some("settings modal close proof".to_string()),
                    },
                },
            })
            .await;
        assert!(close.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
            if view_model.shell.sessions.iter().any(|row| row.id == forked_session_id && row.status == "closed")
        )), "Close must update connected GUI state immediately: {close:?}");
        let closed_row: (String, Option<String>) = sqlx::query_as("SELECT status, close_reason FROM sessions WHERE id=$1")
            .bind(forked_uuid)
            .fetch_one(&test_db.pool)
            .await
            .expect("closed row");
        assert_eq!(closed_row.0, "closed");
        assert_eq!(closed_row.1.as_deref(), Some("settings modal close proof"));

        let archive = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-archive-session".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::ArchiveSession { session_id: forked_session_id.clone() },
                },
            })
            .await;
        assert!(archive.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
            if !view_model.shell.sessions.iter().any(|row| row.id == forked_session_id)
        )), "Archive must remove the session from normal tracked lists: {archive:?}");
        let archived_row: (bool, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as("SELECT tracked, archived_at FROM sessions WHERE id=$1")
            .bind(forked_uuid)
            .fetch_one(&test_db.pool)
            .await
            .expect("archived session row");
        assert!(!archived_row.0);
        assert!(archived_row.1.is_some());

        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn nonblocking_send_streams_selected_chat_deltas_and_terminal_state() {
        let test_db = validation_db().await;
        let model = FakeModelClient {
            request_delay_ms: Some(250),
            model_name: Some("nonblocking-test-model"),
            ..Default::default()
        };
        let state = ServerState::new_with_model_client(
            test_db.pool.clone(),
            "nonblocking-stream-validation".to_string(),
            Arc::new(model),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let role = db::current_role_snapshot(&test_db.pool, "runtime-allow")
            .await
            .expect("role");
        let session_id = db::new_session(
            &test_db.pool,
            &role,
            Some("stream-project"),
            ".",
            Some("."),
            Some("Nonblocking stream"),
            Some("nonblocking-stream"),
        )
        .await
        .expect("session");
        let base_url = format!("http://{addr}");
        let mut sync = RuntimeSyncClient::new(RuntimeSyncConfig::new(base_url.clone()).with_selected_session(session_id));
        let snapshot = sync.hydrate().await.expect("hydrate").clone();
        let mut stream = sync.connect_after(Some(snapshot.watermark)).await.expect("connect stream");

        let started = std::time::Instant::now();
        let response = reqwest::Client::new()
            .post(format!("{base_url}/sessions/{session_id}/send"))
            .json(&json!({"message":"nonblocking exact composer text"}))
            .send()
            .await
            .expect("send request");
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = response.json().await.expect("send body");
        assert_eq!(body["status"], "running", "send must return while model work continues");
        let turn_id = Uuid::parse_str(body["turnId"].as_str().expect("turn id")).expect("turn uuid");
        assert!(started.elapsed() < Duration::from_millis(220), "send response must not wait for delayed model completion");

        let mut saw_user_delta = false;
        let mut saw_tool_delta = false;
        let mut saw_assistant_delta = false;
        for _ in 0..80 {
            let outcome = stream.next_outcome(&mut sync).await.expect("stream outcome");
            if let SyncOutcome::DeltaApplied { delta, .. } = outcome {
                match &delta.kind {
                    RuntimeDeltaKind::SelectedChatUpdate { entry }
                        if entry.author == "User"
                            && entry.body == "nonblocking exact composer text"
                            && entry.is_streaming =>
                    {
                        saw_user_delta = true;
                    }
                    RuntimeDeltaKind::SelectedChatUpdate { entry }
                        if entry.author == "Tool" && entry.is_tool && !entry.command.is_empty() =>
                    {
                        saw_tool_delta = true;
                    }
                    RuntimeDeltaKind::SelectedChatUpdate { entry }
                        if entry.author == "Assistant" && entry.body.contains("fake final fake-call completed") =>
                    {
                        saw_assistant_delta = true;
                    }
                    _ => {}
                }
                if saw_user_delta && saw_tool_delta && saw_assistant_delta {
                    break;
                }
            }
        }
        assert!(saw_user_delta, "stream must deliver selected-chat User delta before final completion");
        assert!(saw_tool_delta, "stream must deliver selected-chat Tool delta");
        assert!(saw_assistant_delta, "stream must deliver selected-chat Assistant final delta");

        let mut turn_status = String::new();
        for _ in 0..50 {
            turn_status = sqlx::query_scalar("SELECT status FROM turns WHERE id=$1")
                .bind(turn_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("turn status");
            if turn_status == "completed" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(turn_status, "completed");
        let assistant_text: String = sqlx::query_scalar("SELECT payload->>'finalText' FROM event_stream WHERE turn_id=$1 AND event_type='model.final_response'")
            .bind(turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("assistant final text");
        assert!(assistant_text.contains("fake final fake-call completed"));
        let running_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status='running'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("running turn count");
        assert_eq!(running_count, 0);

        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn gui_transport_surfaces_typed_errors_for_required_interactions() {
        let test_db = validation_db().await;
        let state = ServerState::new_with_identity(test_db.pool.clone(), "typed-errors-validation".to_string());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let transport = GuiTransportHandle::spawn();

        let bad_connect = transport
            .send(GuiTransportRequestPacket {
                packet_id: "typed-error-connect".to_string(),
                intent: GuiTransportRequest::Connect { base_url: "not-a-url".to_string(), selected_session_id: None },
            })
            .await;
        assert!(bad_connect.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::OperationResult { result } if matches!(&result.outcome, GuiOperationOutcome::Error { error } if error.error.code == "unavailable" || error.error.code == "bad_request"))), "connect failure must surface typed error: {bad_connect:?}");

        let base_url = format!("http://{addr}");
        let _ = transport
            .send(GuiTransportRequestPacket {
                packet_id: "typed-error-connect-ok".to_string(),
                intent: GuiTransportRequest::Connect { base_url, selected_session_id: None },
            })
            .await;

        let bad_create = transport
            .send(GuiTransportRequestPacket {
                packet_id: "typed-error-create".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CreateSession {
                        role: String::new(),
                        project: Some("typed-error-project".to_string()),
                        model: Some("gpt-5.4-mini".to_string()),
                        workdir: Some(".".to_string()),
                        worktree_root: Some(".".to_string()),
                        title: Some("Bad create".to_string()),
                        name: Some("bad-create".to_string()),
                    },
                },
            })
            .await;
        assert!(bad_create.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::OperationResult { result } if matches!(&result.outcome, GuiOperationOutcome::Error { error } if error.error.code == "bad_request" && error.error.message.contains("role")))), "create validation failure must surface typed modal error: {bad_create:?}");

        let _ = transport
            .send(GuiTransportRequestPacket {
                packet_id: "typed-error-create-project-ok".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CreateProject {
                        project_key: "typed-error-project".to_string(),
                        display_name: "Typed Error Project".to_string(),
                        default_workdir: ".".to_string(),
                        default_worktree_root: ".".to_string(),
                        default_role_id: Some("runtime-no-rg".to_string()),
                        default_model: "gpt-5.4-mini".to_string(),
                    },
                },
            })
            .await;
        let duplicate_project = transport
            .send(GuiTransportRequestPacket {
                packet_id: "typed-error-create-project-duplicate".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CreateProject {
                        project_key: "typed-error-project".to_string(),
                        display_name: "Duplicate".to_string(),
                        default_workdir: ".".to_string(),
                        default_worktree_root: ".".to_string(),
                        default_role_id: Some("runtime-no-rg".to_string()),
                        default_model: "gpt-5.4-mini".to_string(),
                    },
                },
            })
            .await;
        assert_gui_operation_error(&duplicate_project, "duplicate project create");

        let bad_project_update = transport
            .send(GuiTransportRequestPacket {
                packet_id: "typed-error-project-update".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::UpdateProject {
                        project_key: "missing-project".to_string(),
                        display_name: "Missing".to_string(),
                        default_workdir: ".".to_string(),
                        default_worktree_root: ".".to_string(),
                        default_role_id: Some("runtime-no-rg".to_string()),
                        default_model: "gpt-5.4-mini".to_string(),
                    },
                },
            })
            .await;
        assert!(bad_project_update.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::OperationResult { result } if matches!(&result.outcome, GuiOperationOutcome::Error { error } if error.error.code == "not_found"))), "project update failure must surface typed error: {bad_project_update:?}");

        for (packet_id, operation) in [
            (
                "typed-error-runtime-settings",
                GuiOperationRequest::UpdateRuntimeSettings {
                    base_url: String::new(),
                    selected_project_id: Some("typed-error-project".to_string()),
                },
            ),
            (
                "typed-error-session-settings",
                GuiOperationRequest::UpdateSessionSettings {
                    session_id: Uuid::nil().to_string(),
                    project: "typed-error-project".to_string(),
                    role: "runtime-no-rg".to_string(),
                    model: "gpt-5.4-mini".to_string(),
                    workdir: ".".to_string(),
                    worktree_root: ".".to_string(),
                    title: "Missing session".to_string(),
                    name: "missing-session".to_string(),
                    tracked: true,
                },
            ),
            (
                "typed-error-close",
                GuiOperationRequest::CloseSession {
                    session_id: Uuid::nil().to_string(),
                    reason: Some("missing session".to_string()),
                },
            ),
            (
                "typed-error-archive",
                GuiOperationRequest::ArchiveSession {
                    session_id: Uuid::nil().to_string(),
                },
            ),
            (
                "typed-error-fork",
                GuiOperationRequest::ForkSession {
                    session_id: Uuid::nil().to_string(),
                    at_turn: Uuid::nil().to_string(),
                },
            ),
            (
                "typed-error-archive-project",
                GuiOperationRequest::ArchiveProject {
                    project_key: "missing-project".to_string(),
                },
            ),
            (
                "typed-error-unarchive-project",
                GuiOperationRequest::UnarchiveProject {
                    project_key: "missing-project".to_string(),
                },
            ),
            (
                "typed-error-role-admin",
                GuiOperationRequest::ShowRoleDetail {
                    role_id: "missing-role".to_string(),
                },
            ),
            (
                "typed-error-role-activate",
                GuiOperationRequest::ActivateRoleVersion {
                    role_id: "missing-role".to_string(),
                    version_id: Uuid::nil().to_string(),
                },
            ),
            (
                "typed-error-approval-decide",
                GuiOperationRequest::DecideApproval {
                    approval_id: Uuid::nil().to_string(),
                    decision: "approve".to_string(),
                    reason: "missing approval".to_string(),
                },
            ),
            (
                "typed-error-approval-resume",
                GuiOperationRequest::ResumeApproval {
                    approval_id: Uuid::nil().to_string(),
                },
            ),
            (
                "typed-error-command-show",
                GuiOperationRequest::ShowCommand {
                    action_id: "missing.command".to_string(),
                    session_id: None,
                    project_key: None,
                },
            ),
            (
                "typed-error-command-preview",
                GuiOperationRequest::PreviewCommandRegistryRequest {
                    request_id: Uuid::nil().to_string(),
                    decision: CommandRegistryDecisionInput {
                        session_id: None,
                        status: "approved".to_string(),
                        final_scope: None,
                        final_execution_policy: None,
                        final_command: None,
                    },
                },
            ),
            (
                "typed-error-command-apply",
                GuiOperationRequest::ApplyCommandRegistryRequest {
                    request_id: Uuid::nil().to_string(),
                    session_id: Uuid::nil().to_string(),
                },
            ),
            (
                "typed-error-process-flush",
                GuiOperationRequest::FlushProcess {
                    session_id: Uuid::nil().to_string(),
                    handle: "missing".to_string(),
                },
            ),
            (
                "typed-error-process-input",
                GuiOperationRequest::InputProcess {
                    session_id: Uuid::nil().to_string(),
                    handle: "missing".to_string(),
                    text: "hello".to_string(),
                },
            ),
            (
                "typed-error-process-terminate",
                GuiOperationRequest::TerminateProcess {
                    session_id: Uuid::nil().to_string(),
                    handle: "missing".to_string(),
                },
            ),
            (
                "typed-error-workflow-memory",
                GuiOperationRequest::WorkflowMemoryFeedback {
                    memory_id: Uuid::nil().to_string(),
                    session_id: Uuid::nil().to_string(),
                    feedback: "helpful".to_string(),
                    payload: json!({"source":"typed-error-validation"}),
                },
            ),
        ] {
            let outputs = transport
                .send(GuiTransportRequestPacket {
                    packet_id: packet_id.to_string(),
                    intent: GuiTransportRequest::DispatchOperation { operation },
                })
                .await;
            assert_gui_operation_error(&outputs, packet_id);
        }

        let bad_send = transport
            .send(GuiTransportRequestPacket {
                packet_id: "typed-error-send".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::SendMessage {
                        session_id: "00000000-0000-0000-0000-000000000000".to_string(),
                        message: String::new(),
                    },
                },
            })
            .await;
        assert!(bad_send.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::OperationResult { result } if matches!(&result.outcome, GuiOperationOutcome::Error { error } if error.error.code == "bad_request" || error.error.code == "validation_failed"))), "send failure must surface typed composer error: {bad_send:?}");

        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn live_ui_validation_send_response_projects_into_shared_chat_timeline_shape() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg")
            .await
            .expect("role");
        let session_id = db::new_session(
            &test_db.pool,
            &role,
            Some("live-ui-validation"),
            ".",
            Some("."),
            Some("Live UI validation"),
            Some("live-ui-validation"),
        )
        .await
        .expect("session");
        let fake = FakeModelClient::default();
        let turn_id = crate::runtime::send_with_model_client(
            &test_db.pool,
            session_id,
            "Check the runtime health and answer briefly.",
            &fake,
            compaction::CompactionBudget::default(),
        )
        .await
        .expect("send");
        let projection = projection::build_runtime_projection_snapshot(&test_db.pool, Some(session_id))
            .await
            .expect("projection");
        let controller_state = GuiControllerState {
            connection_state: GuiConnectionState::Streaming,
            selected_session_id: Some(session_id.to_string()),
            ..GuiControllerState::default()
        };
        let view = crate::rinf_transport::AgentRuntimeWorkbenchViewModel::from_runtime_state(
            "http://127.0.0.1:8765",
            Some(&projection),
            &controller_state,
            &[],
            0,
            None,
            &crate::rinf_transport::AgentRuntimeDiscoveryView::default(),
            &crate::rinf_transport::AgentRuntimeDiscoveryView::default(),
            &crate::rinf_transport::AgentRuntimeDiscoveryView::default(),
            &[],
        );
        assert_eq!(view.shell.selected_session_id.as_deref(), Some(session_id.to_string().as_str()));
        assert!(
            view.shell
                .selected_conversation
                .iter()
                .any(|row| row.author == "Assistant" && row.body.contains("fake final fake-call completed")),
            "response row must be available for shared ChatTimeline rendering: {:?}",
            view.shell.selected_conversation
        );
        println!(
            "live_ui_validation session_id={session_id} turn_id={turn_id} chat_timeline_response=\"fake final fake-call completed\" rows={}",
            view.shell.selected_conversation.len()
        );
        let _ = std::fs::create_dir_all("/tmp/agent-runtime-shell-proof");
        std::fs::write(
            "/tmp/agent-runtime-shell-proof/live-ui-validation-response.txt",
            format!(
                "session_id={session_id}\nturn_id={turn_id}\nchat_timeline_response=fake final fake-call completed\nrows={}\n",
                view.shell.selected_conversation.len()
            ),
        )
        .expect("write live ui validation evidence");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn live_rinf_transport_ui_validation_connect_create_send_projects_chat_timeline_response() {
        let test_db = validation_db().await;
        let state = ServerState::new_with_model_client(
            test_db.pool.clone(),
            "live-rinf-ui-validation".to_string(),
            Arc::new(FakeModelClient { model_name: Some("deterministic-test-model"), ..Default::default() }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app(state)).await.expect("serve");
        });
        let base_url = format!("http://{addr}");
        let transport = GuiTransportHandle::spawn();

        let connect = transport
            .send(GuiTransportRequestPacket {
                packet_id: "live-validation-connect".to_string(),
                intent: GuiTransportRequest::Connect {
                    base_url: base_url.clone(),
                    selected_session_id: None,
                },
            })
            .await;
        assert!(
            connect.iter().any(|packet| matches!(
                &packet.output,
                GuiTransportOutput::OperationResult {
                    result
                } if matches!(result.outcome, GuiOperationOutcome::ProjectionUpdated { .. })
            )),
            "connect must hydrate through the live transport path: {connect:?}"
        );

        let created = transport
            .send(GuiTransportRequestPacket {
                packet_id: "live-validation-create".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CreateSession {
                        role: "runtime-no-rg".to_string(),
                        project: Some("live-rinf-ui-validation".to_string()),
                        model: Some("deterministic-test-model".to_string()),
                        workdir: Some(".".to_string()),
                        worktree_root: Some(".".to_string()),
                        title: Some("Live Rinf UI validation".to_string()),
                        name: Some("live-rinf-ui-validation".to_string()),
                    },
                },
            })
            .await;
        let session_id = created
            .iter()
            .find_map(|packet| match &packet.output {
                GuiTransportOutput::OperationResult { result } => match &result.outcome {
                    GuiOperationOutcome::Accepted { entity_id: Some(id) } => Some(id.clone()),
                    _ => None,
                },
                _ => None,
            })
            .expect("created session id");

        let _ = transport
            .send(GuiTransportRequestPacket {
                packet_id: "live-validation-project".to_string(),
                intent: GuiTransportRequest::SelectProject {
                    project_id: "live-rinf-ui-validation".to_string(),
                },
            })
            .await;
        let project_filtered = transport
            .send(GuiTransportRequestPacket {
                packet_id: "live-validation-project-refresh".to_string(),
                intent: GuiTransportRequest::Rehydrate {
                    selected_session_id: Some(session_id.clone()),
                },
            })
            .await;
        assert!(
            project_filtered.iter().any(|packet| matches!(
                &packet.output,
                GuiTransportOutput::WorkbenchView { view_model }
                    if view_model.shell.sessions.iter().any(|row| row.id == session_id && row.subtitle.contains("live-rinf-ui-validation") && row.group_label == "runtime-no-rg")
            )),
            "created GUI session must refresh under selected project with product-facing role/project labels: {project_filtered:?}"
        );

        let selected = transport
            .send(GuiTransportRequestPacket {
                packet_id: "live-validation-select".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::SelectSession {
                        session_id: Some(session_id.clone()),
                    },
                },
            })
            .await;
        assert!(
            selected.iter().any(|packet| matches!(
                &packet.output,
                GuiTransportOutput::WorkbenchView { view_model }
                    if view_model.shell.selected_session_id.as_deref() == Some(session_id.as_str())
            )),
            "selection must flow through the live transport shell view: {selected:?}"
        );

        let sent = transport
            .send(GuiTransportRequestPacket {
                packet_id: "live-validation-send".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::SendMessage {
                        session_id: session_id.clone(),
                        message: "Check the runtime health and answer briefly.".to_string(),
                    },
                },
            })
            .await;
        let turn_id = sent
            .iter()
            .find_map(|packet| match &packet.output {
                GuiTransportOutput::OperationResult { result } => match &result.outcome {
                    GuiOperationOutcome::Accepted { entity_id: Some(id) } => Some(id.clone()),
                    _ => None,
                },
                _ => None,
            })
            .expect("send turn id");

        let rehydrated = transport
            .send(GuiTransportRequestPacket {
                packet_id: "live-validation-rehydrate".to_string(),
                intent: GuiTransportRequest::Rehydrate {
                    selected_session_id: Some(session_id.clone()),
                },
            })
            .await;
        let view = rehydrated
            .iter()
            .find_map(|packet| match &packet.output {
                GuiTransportOutput::WorkbenchView { view_model } => Some(view_model),
                _ => None,
            })
            .expect("Workbench view after send");
        assert_eq!(view.shell.selected_session_id.as_deref(), Some(session_id.as_str()));
        assert!(
            view.shell
                .selected_conversation
                .iter()
                .any(|row| row.author == "Assistant" && row.body.contains("fake final fake-call completed")),
            "live transport response must render through shared ChatTimeline rows: {:?}",
            view.shell.selected_conversation
        );
        let persisted_turn_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id = $1 AND id = $2")
            .bind(Uuid::parse_str(&session_id).expect("session uuid"))
            .bind(Uuid::parse_str(&turn_id).expect("turn uuid"))
            .fetch_one(&test_db.pool)
            .await
            .expect("turn count");
        assert_eq!(persisted_turn_count, 1, "live GUI transport validation must persist the sent turn");
        let requested_model: String = sqlx::query_scalar("SELECT payload->'request'->>'model' FROM model_events WHERE turn_id=$1 AND event_type='assistant_message'")
            .bind(Uuid::parse_str(&turn_id).expect("turn uuid"))
            .fetch_one(&test_db.pool)
            .await
            .expect("requested model");
        assert_eq!(requested_model, "deterministic-test-model");
        println!(
            "live_rinf_ui_validation base_url={base_url} session_id={session_id} turn_id={turn_id} persisted_turn_count={persisted_turn_count} requested_model={requested_model} chat_timeline_response=\"fake final fake-call completed\" rows={}",
            view.shell.selected_conversation.len()
        );
        let _ = std::fs::create_dir_all("/tmp/agent-runtime-shell-proof");
        std::fs::write(
            "/tmp/agent-runtime-shell-proof/live-rinf-ui-validation-response.txt",
            format!(
                "base_url={base_url}\nsession_id={session_id}\nturn_id={turn_id}\npersisted_turn_count={persisted_turn_count}\nchat_timeline_response=fake final fake-call completed\nrows={}\n",
                view.shell.selected_conversation.len()
            ),
        )
        .expect("write live rinf ui validation evidence");
        server.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    #[ignore]
    async fn live_real_model_gui_e2e_without_fake_model_behavior() {
        let base_url = std::env::var("ROBDEX_AGENT_RUNTIME_SERVER_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8765".to_string());
        let database_url = std::env::var("ROBDEX_AGENT_RUNTIME_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres@127.0.0.1:5432/robdex_agent_runtime".to_string());
        let pool = db::connect(&database_url).await.expect("connect live runtime database");
        let transport = GuiTransportHandle::spawn();
        let unique = format!("live-e2e-{}", chrono::Utc::now().timestamp_millis());
        let project_key = format!("agent-runtime-{unique}");
        let title = format!("Agent Runtime {unique}");
        let name = format!("agent-runtime-{unique}");

        let connect = transport
            .send(GuiTransportRequestPacket {
                packet_id: "live-real-connect".to_string(),
                intent: GuiTransportRequest::Connect {
                    base_url: base_url.clone(),
                    selected_session_id: None,
                },
            })
            .await;
        assert!(
            connect.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model } if view_model.connection_state == "streaming")),
            "live GUI connect must render streaming Workbench state: {connect:?}"
        );

        let created_project = transport
            .send(GuiTransportRequestPacket {
                packet_id: "live-real-project".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CreateProject {
                        project_key: project_key.clone(),
                        display_name: title.clone(),
                        default_workdir: ".".to_string(),
                        default_worktree_root: ".".to_string(),
                        default_role_id: Some("runtime-no-rg".to_string()),
                        default_model: "gpt-5.4-mini".to_string(),
                    },
                },
            })
            .await;
        assert!(
            created_project.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
                if view_model.shell.projects.iter().any(|project| project.id == project_key)
            )),
            "live GUI project creation must appear in the DB-backed rail: {created_project:?}"
        );

        let created = transport
            .send(GuiTransportRequestPacket {
                packet_id: "live-real-session".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::CreateSession {
                        role: "runtime-no-rg".to_string(),
                        project: Some(project_key.clone()),
                        model: Some("gpt-5.4-mini".to_string()),
                        workdir: Some(".".to_string()),
                        worktree_root: Some(".".to_string()),
                        title: Some(title.clone()),
                        name: Some(name.clone()),
                    },
                },
            })
            .await;
        let session_id = created
            .iter()
            .find_map(|packet| match &packet.output {
                GuiTransportOutput::OperationResult { result } => match &result.outcome {
                    GuiOperationOutcome::Accepted { entity_id: Some(id) } => Some(id.clone()),
                    _ => None,
                },
                _ => None,
            })
            .expect("live GUI session id");
        let session_uuid = Uuid::parse_str(&session_id).expect("session uuid");
        assert!(
            created.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
                if view_model.shell.selected_session_id == Some(session_id.clone())
            )),
            "live GUI session creation must select the new session: {created:?}"
        );

        let send = transport
            .send(GuiTransportRequestPacket {
                packet_id: "live-real-send".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::SendMessage {
                        session_id: session_id.clone(),
                        message: "Use execute_code with exactly this harmless read-only Starlark: output({\"validation\":\"ok\",\"source\":\"live-real-gui-e2e\"})".to_string(),
                    },
                },
            })
            .await;
        assert!(
            send.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::OperationResult { result }
                if matches!(result.outcome, GuiOperationOutcome::Accepted { .. })
            )),
            "live GUI send must dispatch through typed operation path: {send:?}"
        );

        let mut saw_user_delta = false;
        let mut saw_assistant_delta = false;
        for index in 0..120 {
            let packet = GuiTransportRequestPacket {
                packet_id: format!("live-real-stream-{index}"),
                intent: GuiTransportRequest::ConsumeStreamOnce,
            };
            let outputs = match tokio::time::timeout(Duration::from_secs(2), transport.send(packet)).await {
                Ok(outputs) => outputs,
                Err(_) => Vec::new(),
            };
            for packet in &outputs {
                if let GuiTransportOutput::StreamOutcome { projection: Some(projection), .. } = &packet.output {
                    let entries = projection["selectedChatEntries"].as_array().cloned().unwrap_or_default();
                    saw_user_delta |= entries.iter().any(|entry| entry["author"] == "User" && entry["body"].as_str().unwrap_or_default().contains("live-real-gui-e2e"));
                    saw_assistant_delta |= entries.iter().any(|entry| entry["author"] == "Assistant" && !entry["body"].as_str().unwrap_or_default().trim().is_empty());
                }
            }
            let terminal: Option<String> = sqlx::query_scalar("SELECT status FROM turns WHERE session_id=$1 ORDER BY started_at DESC LIMIT 1")
                .bind(session_uuid)
                .fetch_optional(&pool)
                .await
                .expect("turn status");
            if matches!(terminal.as_deref(), Some("completed" | "failed" | "cancelled" | "lost")) && saw_user_delta && saw_assistant_delta {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        let turn_row: (Uuid, String, String) = sqlx::query_as("SELECT id, input_text, status FROM turns WHERE session_id=$1 ORDER BY started_at DESC LIMIT 1")
            .bind(session_uuid)
            .fetch_one(&pool)
            .await
            .expect("live turn row");
        let turn_id = turn_row.0;
        let model_event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_events WHERE turn_id=$1")
            .bind(turn_id)
            .fetch_one(&pool)
            .await
            .expect("model event count");
        let tool_call_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tool_calls WHERE turn_id=$1")
            .bind(turn_id)
            .fetch_one(&pool)
            .await
            .expect("tool call count");
        let artifact_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_output_artifacts WHERE turn_id=$1")
            .bind(turn_id)
            .fetch_one(&pool)
            .await
            .expect("artifact count");
        let running_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status='running'")
            .bind(session_uuid)
            .fetch_one(&pool)
            .await
            .expect("running count");
        let stored_project: String = sqlx::query_scalar("SELECT project_key FROM sessions WHERE id=$1")
            .bind(session_uuid)
            .fetch_one(&pool)
            .await
            .expect("stored project");
        assert_eq!(stored_project, project_key);
        assert!(saw_user_delta, "live GUI E2E must observe selected-chat user delta");
        assert!(saw_assistant_delta, "live GUI E2E must observe selected-chat assistant delta");
        assert_eq!(turn_row.1.contains("live-real-gui-e2e"), true);
        assert!(matches!(turn_row.2.as_str(), "completed" | "failed"), "turn must reach terminal status, got {}", turn_row.2);
        assert!(model_event_count > 0, "live GUI E2E must persist model events");
        if turn_row.2 == "completed" {
            assert!(tool_call_count > 0, "completed live GUI E2E must persist tool calls");
            assert!(artifact_count > 0, "completed live GUI E2E must persist output artifacts");
        }
        assert_eq!(running_count, 0, "live GUI E2E must leave no orphan running turn");
        println!(
            "live_real_model_gui_e2e base_url={base_url} project_key={project_key} session_id={session_id} turn_id={turn_id} terminal_status={} model_events={model_event_count} tool_calls={tool_call_count} output_artifacts={artifact_count} saw_user_delta={saw_user_delta} saw_assistant_delta={saw_assistant_delta}",
            turn_row.2
        );
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

        let close_session_id = db::new_session(
            &test_db.pool,
            &role,
            Some("semantic-session"),
            ".",
            Some("."),
            Some("Semantic close session"),
            Some("semantic-close-session"),
        )
        .await
        .expect("new close session");
        apply_until(&mut ws, &mut client_projection, |_delta, projection| {
            projection.sessions.iter().any(|session| session.id == close_session_id.to_string())
        })
        .await;
        db::close_session(&test_db.pool, close_session_id, "semantic close", 0)
            .await
            .expect("close");
        apply_until(&mut ws, &mut client_projection, |_delta, projection| {
            projection
                .sessions
                .iter()
                .any(|session| session.id == close_session_id.to_string() && session.status == "closed")
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
            projection.pending_approvals.iter().any(|approval| {
                approval.id == approval_id.to_string()
                    && approval.status == "denied"
                    && !approval.can_decide
                    && !approval.can_resume
            })
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
            .json(&json!({"role":"runtime-no-rg","project":"gui-sync","model":"fake-model","workdir":".","worktreeRoot":".","title":"GUI sync","name":"gui-sync"}))
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
            .json(&json!({"role":"runtime-no-rg","project":"gui-sync-resync","model":"fake-model","workdir":".","worktreeRoot":".","title":"GUI sync resync","name":"gui-sync-resync"}))
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

    fn role_editor_draft_json(role_id: &str, version: &str, instruction: &str) -> Value {
        json!({
            "id": role_id,
            "version": version,
            "displayName": "GUI Runtime Role",
            "modelDefaults": {"model": "gpt-5.4-mini", "reasoningEffort": "medium"},
            "instructionText": instruction,
            "capabilities": ["tool.execute_code"],
            "policy": {"tool.execute_code": "allow"},
            "routing": {"mode": "direct", "defaultRecipient": "owner", "allowedRecipients": ["owner"], "reservedActions": ["message.send"]},
            "visibility": {"listed": true, "ownerVisible": true},
            "lifecycleAuthority": {"canSpawnAgents": false, "canArchiveAgents": false, "reservedActions": ["agent.archive"]}
        })
    }

    #[tokio::test]
    async fn deterministic_role_editor_api_uses_inline_db_versions_and_canonical_validation() {
        let test_db = validation_db().await;
        let router = app(ServerState::new(test_db.pool.clone()));
        let (status, options) = request_json(router.clone(), Method::GET, "/roles/editor/options", Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert!(options["policyDecisions"].as_array().expect("decisions").iter().any(|value| value == "allow"));

        let draft_v1 = role_editor_draft_json("gui-role", "1.0.0", "inline gui role instructions v1");
        let (status, validation) = request_json(router.clone(), Method::POST, "/roles/editor/validate", draft_v1.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(validation["valid"], true);
        let (status, created) = request_json(router.clone(), Method::POST, "/roles", draft_v1).await;
        assert_eq!(status, StatusCode::OK);
        let version_v1 = created["versionId"].as_str().expect("created version").to_string();
        let stored_v1: String = sqlx::query_scalar("SELECT instruction_text FROM role_versions WHERE id=$1")
            .bind(Uuid::parse_str(&version_v1).expect("version uuid"))
            .fetch_one(&test_db.pool)
            .await
            .expect("stored v1 instruction");
        assert_eq!(stored_v1, "inline gui role instructions v1");

        let draft_v2 = role_editor_draft_json("gui-role", "1.0.1", "inline gui role instructions v2");
        let (status, updated) = request_json(router.clone(), Method::POST, "/roles/gui-role/versions", draft_v2).await;
        assert_eq!(status, StatusCode::OK);
        let version_v2 = updated["versionId"].as_str().expect("updated version").to_string();
        assert_ne!(version_v1, version_v2);
        let version_count: i64 = sqlx::query_scalar("SELECT count(*) FROM role_versions WHERE role_id='gui-role'")
            .fetch_one(&test_db.pool)
            .await
            .expect("version count");
        assert_eq!(version_count, 2);
        let current_instruction: String = sqlx::query_scalar("SELECT rv.instruction_text FROM roles r JOIN role_versions rv ON rv.id=r.current_version_id WHERE r.id='gui-role'")
            .fetch_one(&test_db.pool)
            .await
            .expect("current instruction");
        assert_eq!(current_instruction, "inline gui role instructions v2");
        let (status, _) = request_json(router.clone(), Method::POST, "/roles/gui-role/activate", json!({"versionId": version_v1})).await;
        assert_eq!(status, StatusCode::OK);
        let current_version: Uuid = sqlx::query_scalar("SELECT current_version_id FROM roles WHERE id='gui-role'")
            .fetch_one(&test_db.pool)
            .await
            .expect("current version after rollback activation");
        assert_eq!(current_version.to_string(), version_v1);
        let (status, _) = request_json(router.clone(), Method::POST, "/roles/gui-role/archive", json!({})).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = request_json(router.clone(), Method::POST, "/roles/gui-role/unarchive", json!({})).await;
        assert_eq!(status, StatusCode::OK);
        let mut invalid = role_editor_draft_json("bad-role", "1.0.0", "instructions");
        invalid["capabilities"] = json!(["cmd.rg.run"]);
        invalid["policy"] = json!({"cmd.rg.run": "allow"});
        let (status, validation) = request_json(router.clone(), Method::POST, "/roles/editor/validate", invalid.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(validation["valid"], false);
        assert!(validation["errors"].to_string().contains("concrete command actions"));
        let (status, error) = request_json(router, Method::POST, "/roles", invalid).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_api_error(&error, "validation_failed");
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
            projection.pending_approvals.iter().any(|approval| {
                approval.id == approval_id.to_string()
                    && approval.status == "denied"
                    && approval.decision_reason.as_deref() == Some("deterministic admin ws validation")
            })
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
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, completed_at) VALUES ($1,$2,'user',$3,'completed',now())")
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

    async fn insert_completed_turn(pool: &PgPool, session_id: Uuid, input: &str, assistant: &str) -> Uuid {
        let turn_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, completed_at) VALUES ($1,$2,'user',$3,'completed',now())")
            .bind(turn_id)
            .bind(session_id)
            .bind(input)
            .execute(pool)
            .await
            .expect("insert completed turn");
        sqlx::query("INSERT INTO model_events (id, session_id, turn_id, event_type, payload) VALUES ($1,$2,$3,'final_response',$4)")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .bind(turn_id)
            .bind(json!({"summary": assistant}))
            .execute(pool)
            .await
            .expect("insert final response");
        db::append_event(pool, session_id, Some(turn_id), "turn", Some(turn_id), "turn.completed", Some("completed"), json!({"test": true})).await.expect("turn event");
        turn_id
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
        let before = starlark_host::execute_code(&test_db.pool, alpha, turn_id, tool_call_id, project_source, &root, &role).await.expect("failed packet before project command");
        let before_value = serde_json::to_value(before).expect("before packet");
        assert_eq!(before_value["status"], "failed");
        assert!(before_value["output"]["stderrArtifact"]["preview"].as_str().unwrap_or_default().contains("project_cache") || before_value["output"]["stderrArtifact"]["tail"].as_str().unwrap_or_default().contains("project_cache"));

        let project_seed = scoped_command_seed("cmd.cache.project", "project_cache");
        apply_registry_seed(&test_db.pool, alpha, project_seed, command_registry::RegistryScope { scope_type: "project".to_string(), project_key: Some("cache-alpha".to_string()) }).await;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, alpha, project_source).await;
        let after = starlark_host::execute_code(&test_db.pool, alpha, turn_id, tool_call_id, project_source, &root, &role).await.expect("execute after project command");
        let after_value = serde_json::to_value(after).expect("after packet");
        assert_eq!(after_value["status"], "completed");
        assert!(after_value["output"]["artifact"]["preview"].as_str().unwrap_or_default().contains("cmd.cache.project") || after_value["output"]["artifact"]["tail"].as_str().unwrap_or_default().contains("cmd.cache.project"));

        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, beta, project_source).await;
        let non_visible = starlark_host::execute_code(&test_db.pool, beta, turn_id, tool_call_id, project_source, &root, &role).await.expect("non-visible failed packet");
        let non_visible_value = serde_json::to_value(non_visible).expect("non-visible packet");
        assert_eq!(non_visible_value["status"], "failed");
        assert!(non_visible_value["output"]["stderrArtifact"]["preview"].as_str().unwrap_or_default().contains("project_cache") || non_visible_value["output"]["stderrArtifact"]["tail"].as_str().unwrap_or_default().contains("project_cache"));

        let global_source = "output(cmd[\"global_cache\"].run.describe())";
        let global_seed = scoped_command_seed("cmd.cache.global", "global_cache");
        apply_registry_seed(&test_db.pool, alpha, global_seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, beta, global_source).await;
        let global = starlark_host::execute_code(&test_db.pool, beta, turn_id, tool_call_id, global_source, &root, &role).await.expect("execute global command");
        let global_value = serde_json::to_value(global).expect("global packet");
        assert_eq!(global_value["status"], "completed");
        assert!(global_value["output"]["artifact"]["preview"].as_str().unwrap_or_default().contains("cmd.cache.global") || global_value["output"]["artifact"]["tail"].as_str().unwrap_or_default().contains("cmd.cache.global"));
        test_db.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn output_artifacts_store_full_output_and_retrieve_bounded_views() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("output-artifacts"), ".", Some("."), None, None).await.expect("session");
        let root = starlark_host::ExecutionRoot::new(".").expect("root");
        let large = (0..900)
            .map(|i| format!("line-{i:04}-{}", if i == 777 { "needle-output-artifact" } else { "payload" }))
            .collect::<Vec<_>>()
            .join("\n");
        let source = format!("output({})", serde_json::to_string(&large).expect("source string"));
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, &source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, &source, &root, &role).await.expect("execute large output");
        let value = serde_json::to_value(packet).expect("packet");
        let artifact_id = Uuid::parse_str(value["output"]["artifact"]["artifactId"].as_str().expect("artifact id")).expect("artifact uuid");
        assert!(value["output"]["artifact"]["truncated"].as_bool().unwrap_or(false));
        assert!(!value.to_string().contains(&large));

        let row = sqlx::query("SELECT content, byte_count, line_count FROM execution_output_artifacts WHERE id=$1")
            .bind(artifact_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("artifact row");
        assert_eq!(row.get::<String, _>("content"), large);
        assert_eq!(row.get::<i64, _>("byte_count") as usize, large.len());
        assert_eq!(row.get::<i64, _>("line_count"), 900);

        let retrieval_source = r#"
artifact = outputs.last()
output(outputs.head(artifact, lines=3))
output(outputs.tail(artifact, lines=4))
output(outputs.slice(artifact, start_line=500, end_line=650))
output(outputs.search(artifact, "needle-output-artifact", context=2))
output(outputs.stats(artifact))
"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, retrieval_source).await;
        let retrieval = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, retrieval_source, &root, &role).await.expect("retrieve artifact");
        let retrieval_value = serde_json::to_value(retrieval).expect("retrieval packet");
        let retrieval_artifact_id = Uuid::parse_str(retrieval_value["output"]["artifact"]["artifactId"].as_str().expect("retrieval artifact")).expect("retrieval uuid");
        let retrieved_text: String = sqlx::query_scalar("SELECT content FROM execution_output_artifacts WHERE id=$1")
            .bind(retrieval_artifact_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("retrieved output artifact");
        let packets: Vec<Value> = retrieved_text
            .lines()
            .map(|line| serde_json::from_str(line).expect("retrieval packet json"))
            .collect();
        assert_eq!(packets.len(), 5);
        let head_content = "line-0000-payload\nline-0001-payload\nline-0002-payload";
        assert_eq!(packets[0]["mode"], "head");
        assert_eq!(packets[0]["returnedLines"], 3);
        assert_eq!(packets[0]["returnedBytes"], head_content.len() as u64);
        assert_eq!(packets[0]["omittedBytes"], (large.len() - head_content.len()) as u64);
        assert_eq!(packets[0]["omittedLines"], 897);
        assert_eq!(packets[0]["truncated"], true);
        assert_eq!(packets[0]["content"], head_content);
        let tail_content = "line-0896-payload\nline-0897-payload\nline-0898-payload\nline-0899-payload";
        assert_eq!(packets[1]["mode"], "tail");
        assert_eq!(packets[1]["returnedLines"], 4);
        assert_eq!(packets[1]["returnedBytes"], tail_content.len() as u64);
        assert_eq!(packets[1]["omittedBytes"], (large.len() - tail_content.len()) as u64);
        assert_eq!(packets[1]["omittedLines"], 896);
        assert_eq!(packets[1]["truncated"], true);
        assert_eq!(packets[1]["content"], tail_content);
        let slice_content = (499..650).map(|i| format!("line-{i:04}-payload")).collect::<Vec<_>>().join("\n");
        assert_eq!(packets[2]["mode"], "slice");
        assert_eq!(packets[2]["startLine"], 500);
        assert_eq!(packets[2]["returnedLines"], 151);
        assert_eq!(packets[2]["returnedBytes"], slice_content.len() as u64);
        assert_eq!(packets[2]["omittedBytes"], (large.len() - slice_content.len()) as u64);
        assert_eq!(packets[2]["omittedLines"], 749);
        assert_eq!(packets[2]["truncated"], true);
        assert_eq!(packets[2]["content"], slice_content);
        let search_content = (775..780)
            .map(|i| format!("line-{i:04}-{}", if i == 777 { "needle-output-artifact" } else { "payload" }))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(packets[3]["mode"], "search");
        assert_eq!(packets[3]["matches"], 1);
        assert_eq!(packets[3]["returnedLines"], 5);
        assert_eq!(packets[3]["returnedBytes"], search_content.len() as u64);
        assert_eq!(packets[3]["omittedBytes"], (large.len() - search_content.len()) as u64);
        assert_eq!(packets[3]["omittedLines"], 895);
        assert_eq!(packets[3]["truncated"], true);
        assert_eq!(packets[3]["content"], search_content);
        assert_eq!(packets[4]["mode"], "stats");
        assert_eq!(packets[4]["byteCount"].as_u64().unwrap() as usize, large.len());
        assert_eq!(packets[4]["lineCount"], 900);
        assert_eq!(packets[4]["estimatedTokens"], (large.len() / 4) as u64);
        assert_eq!(packets[4]["returnedBytes"], 0);
        assert_eq!(packets[4]["returnedLines"], 0);
        assert_eq!(packets[4]["omittedBytes"], large.len() as u64);
        assert_eq!(packets[4]["omittedLines"], 900);
        assert_eq!(packets[4]["truncated"], false);
        assert_eq!(packets[4]["content"], "");
        assert!(!retrieval_value.to_string().contains(&large));

        let fail_source = format!("output({})\nmissing_symbol", serde_json::to_string(&large).expect("failure source"));
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, &fail_source).await;
        let failed = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, &fail_source, &root, &role).await.expect("failed execute packet");
        let failed_value = serde_json::to_value(failed).expect("failed value");
        assert_eq!(failed_value["ok"], false);
        assert_eq!(failed_value["status"], "failed");
        assert!(failed_value["output"]["artifact"]["artifactId"].is_string());
        assert!(failed_value["output"]["stderrArtifact"]["artifactId"].is_string());
        assert!(!failed_value.to_string().contains(&large));

        let mut sh_seed = admin_command_seed("cmd.output_artifacts.sh");
        sh_seed["binaryName"] = json!("sh");
        sh_seed["candidatePaths"] = json!(["/bin/sh"]);
        sh_seed["starlarkObject"] = json!("output_sh");
        sh_seed["argvPrefix"] = json!(["-c"]);
        let sh_seed: command_registry::CommandSeed = serde_json::from_value(sh_seed).expect("sh seed");
        apply_registry_seed(&test_db.pool, session_id, sh_seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;
        let command_source = r#"output(cmd["output_sh"].run(args=["printf stdout-artifact; printf stderr-artifact >&2"]).sync())"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, command_source).await;
        let command_packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, command_source, &root, &role).await.expect("command output packet");
        let command_value = serde_json::to_value(command_packet).expect("command value");
        assert_eq!(command_value["status"], "completed", "command packet: {command_value}");
        let command_result_artifact = Uuid::parse_str(command_value["output"]["artifact"]["artifactId"].as_str().expect("command result artifact")).expect("command result artifact id");
        let command_result: String = sqlx::query_scalar("SELECT content FROM execution_output_artifacts WHERE id=$1")
            .bind(command_result_artifact)
            .fetch_one(&test_db.pool)
            .await
            .expect("command result content");
        let command_envelope: Value = serde_json::from_str(command_result.trim()).expect("command envelope json");
        let command_stdout = Uuid::parse_str(command_envelope["stdoutArtifact"]["artifactId"].as_str().expect("stdout artifact id")).expect("stdout uuid");
        let command_stderr = Uuid::parse_str(command_envelope["stderrArtifact"]["artifactId"].as_str().expect("stderr artifact id")).expect("stderr uuid");
        let command_combined = Uuid::parse_str(command_envelope["artifact"]["artifactId"].as_str().expect("combined artifact id")).expect("combined uuid");
        let streams: Vec<(String, String)> = sqlx::query("SELECT stream, content FROM execution_output_artifacts WHERE id = ANY($1) ORDER BY stream")
            .bind(&[command_stdout, command_stderr, command_combined])
            .fetch_all(&test_db.pool)
            .await
            .expect("command stream artifacts")
            .into_iter()
            .map(|row| (row.get("stream"), row.get("content")))
            .collect();
        assert_eq!(streams.len(), 3);
        assert!(streams.contains(&("stdout".to_string(), "stdout-artifact".to_string())));
        assert!(streams.contains(&("stderr".to_string(), "stderr-artifact".to_string())));
        assert!(streams.contains(&("combined".to_string(), "stdout-artifactstderr-artifact".to_string())));

        let mut proc_seed = admin_command_seed("cmd.output_artifacts.process");
        proc_seed["binaryName"] = json!("sh");
        proc_seed["candidatePaths"] = json!(["/bin/sh"]);
        proc_seed["starlarkObject"] = json!("process_sh");
        proc_seed["argvPrefix"] = json!(["-c"]);
        proc_seed["syncAllowed"] = json!(false);
        proc_seed["asyncAllowed"] = json!(true);
        proc_seed["minAwaitMs"] = json!(500);
        proc_seed["maxAwaitMs"] = json!(2000);
        proc_seed["outputLimitBytes"] = json!(1000);
        let proc_seed: command_registry::CommandSeed = serde_json::from_value(proc_seed).expect("process seed");
        apply_registry_seed(&test_db.pool, session_id, proc_seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;
        let stdout_large = (0..1500).map(|i| format!("pout-{i:04}")).collect::<Vec<_>>().join("\n") + "\n";
        let stderr_large = (0..1400).map(|i| format!("perr-{i:04}")).collect::<Vec<_>>().join("\n") + "\n";
        let process_shell = "i=0; while [ $i -lt 1500 ]; do printf 'pout-%04d\\n' \"$i\"; i=$((i+1)); done; i=0; while [ $i -lt 1400 ]; do printf 'perr-%04d\\n' \"$i\" >&2; i=$((i+1)); done";
        let process_source = format!(
            "h = cmd[\"process_sh\"].run(args=[{}]).start()\nproc[h].await_for(mins=0)\noutput(proc[h].flush_buffer())",
            serde_json::to_string(process_shell).expect("process shell")
        );
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, &process_source).await;
        let process_packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, &process_source, &root, &role).await.expect("process packet");
        let process_value = serde_json::to_value(process_packet).expect("process value");
        assert_eq!(process_value["status"], "completed", "process packet: {process_value}");
        assert!(!process_value.to_string().contains(&stdout_large));
        let process_result_artifact = Uuid::parse_str(process_value["output"]["artifact"]["artifactId"].as_str().expect("process result artifact")).expect("process result uuid");
        let process_result: String = sqlx::query_scalar("SELECT content FROM execution_output_artifacts WHERE id=$1")
            .bind(process_result_artifact)
            .fetch_one(&test_db.pool)
            .await
            .expect("process result content");
        let process_envelope: Value = serde_json::from_str(process_result.trim()).expect("process envelope json");
        assert_eq!(process_envelope["stdoutArtifact"]["truncated"], true);
        assert_eq!(process_envelope["stderrArtifact"]["truncated"], true);
        assert_eq!(process_envelope["artifact"]["truncated"], true);
        let process_stdout = Uuid::parse_str(process_envelope["stdoutArtifact"]["artifactId"].as_str().expect("process stdout id")).expect("process stdout uuid");
        let process_stderr = Uuid::parse_str(process_envelope["stderrArtifact"]["artifactId"].as_str().expect("process stderr id")).expect("process stderr uuid");
        let process_combined = Uuid::parse_str(process_envelope["artifact"]["artifactId"].as_str().expect("process combined id")).expect("process combined uuid");
        let process_streams: Vec<(String, String)> = sqlx::query("SELECT stream, content FROM execution_output_artifacts WHERE id = ANY($1) ORDER BY stream")
            .bind(&[process_stdout, process_stderr, process_combined])
            .fetch_all(&test_db.pool)
            .await
            .expect("process stream artifacts")
            .into_iter()
            .map(|row| (row.get("stream"), row.get("content")))
            .collect();
        assert_eq!(process_streams.len(), 3);
        assert!(process_streams.contains(&("stdout".to_string(), stdout_large.clone())));
        assert!(process_streams.contains(&("stderr".to_string(), stderr_large.clone())));
        assert!(process_streams.contains(&("combined".to_string(), format!("{stdout_large}{stderr_large}"))));

        test_db.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn output_artifact_retrieval_rejects_cross_session_ids() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_a = db::new_session(&test_db.pool, &role, Some("output-owner"), ".", Some("."), None, None).await.expect("session a");
        let session_b = db::new_session(&test_db.pool, &role, Some("output-intruder"), ".", Some("."), None, None).await.expect("session b");
        let root = starlark_host::ExecutionRoot::new(".").expect("root");
        let secret = "session-a-secret-output-line";
        let source = format!("output({})", serde_json::to_string(secret).expect("source string"));
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_a, &source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_a, turn_id, tool_call_id, &source, &root, &role).await.expect("execute owner output");
        let value = serde_json::to_value(packet).expect("owner packet");
        let artifact_id = value["output"]["artifact"]["artifactId"].as_str().expect("artifact id");
        let quoted_artifact_id = serde_json::to_string(artifact_id).expect("quoted artifact id");

        let retrieval_sources = [
            format!("output(outputs.head({quoted_artifact_id}, lines=1))"),
            format!("output(outputs.tail({quoted_artifact_id}, lines=1))"),
            format!("output(outputs.slice({quoted_artifact_id}, start_line=1, end_line=1))"),
            format!("output(outputs.search({quoted_artifact_id}, \"secret\", context=1))"),
            format!("output(outputs.stats({quoted_artifact_id}))"),
        ];

        for retrieval_source in retrieval_sources {
            let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_b, &retrieval_source).await;
            let packet = starlark_host::execute_code(&test_db.pool, session_b, turn_id, tool_call_id, &retrieval_source, &root, &role).await.expect("cross-session retrieval packet");
            let value = serde_json::to_value(packet).expect("cross-session packet");
            assert_eq!(value["status"], "failed", "cross-session retrieval should fail: {value}");
            let packet_text = value.to_string();
            assert!(!packet_text.contains(secret), "cross-session retrieval leaked artifact content: {packet_text}");
            assert!(
                packet_text.contains("output artifact not found for current session"),
                "cross-session retrieval should report session-scoped miss: {packet_text}"
            );
        }

        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn compaction_reconstruction_uses_checkpoint_plus_post_boundary_turns() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("compact-reconstruct"), ".", Some("."), None, None).await.expect("session");
        let t1 = insert_completed_turn(&test_db.pool, session_id, "pre-one raw history", "pre-one assistant").await;
        let t2 = insert_completed_turn(&test_db.pool, session_id, "pre-two raw history", "pre-two assistant").await;
        let t3 = insert_completed_turn(&test_db.pool, session_id, "post-boundary history", "post assistant").await;

        let checkpoint = compaction::compact_session_through_turn(&test_db.pool, session_id, t2, compaction::CompactionBudget::default()).await.expect("manual compact");
        assert_eq!(checkpoint.status, "completed");
        assert_eq!(checkpoint.compacted_through_turn_id, Some(t2));
        let history = db::reconstructed_history(&test_db.pool, session_id).await.expect("history");
        assert_eq!(history.len(), 2, "history should contain checkpoint root plus post-boundary turn");
        assert_eq!(history[0].source, "compaction_checkpoint");
        assert_eq!(history[0].checkpoint_id, Some(checkpoint.id));
        assert!(history[0].assistant.as_deref().unwrap_or_default().contains("Runtime compaction checkpoint"));
        assert_eq!(history[1].turn_id, t3);
        assert!(!history.iter().any(|item| item.turn_id == t1 || item.user == "pre-one raw history"));
        let model_history = crate::model_input::model_history_from_items(&history);
        let active_context = RuntimeInputMessage {
            text: "<runtime_context epoch=\"9\"><cwd state=\"known\" source=\"session.workdir\">.</cwd></runtime_context>".to_string(),
            metadata: json!({"source":"runtime_context","contextEpoch":9}),
        };
        let input = crate::model_input::responses_input(&role, &model_history, &[active_context], Some("continue after compact"));
        assert!(input[0]["content"][0]["text"].as_str().unwrap_or_default().contains("<role_instructions"));
        assert!(input[1]["content"][0]["text"].as_str().unwrap_or_default().contains("<runtime_context"));
        assert!(input[2]["metadata"]["source"] == "compaction_checkpoint", "compacted output must be preserved as base history after active context: {input:?}");
        assert!(input[2]["content"][0]["text"].as_str().unwrap_or_default().contains("Compaction checkpoint"), "standalone compact output user marker is preserved");
        assert!(input[3]["role"] == "assistant" && input[3]["content"][0]["text"].as_str().unwrap_or_default().contains("Runtime compaction checkpoint"));
        assert_eq!(input.last().and_then(|item| item.get("role")).and_then(Value::as_str), Some("user"));

        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn manual_compaction_preserves_audit_rows_and_changes_history() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("compact-audit"), ".", Some("."), Some("Concrete owner active goal"), None).await.expect("session");
        let t1 = insert_completed_turn(&test_db.pool, session_id, "audit turn one", "assistant one").await;
        let t2 = insert_completed_turn(&test_db.pool, session_id, "audit turn two", "assistant two").await;
        db::append_event(&test_db.pool, session_id, Some(t1), "policy", None, "policy.decision", Some("allow"), json!({"action":"tool.execute_code","decision":"allow","reason":"concrete decision"})).await.expect("policy decision");
        let tool_call_id = Uuid::new_v4();
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status) VALUES ($1,$2,$3,'execute_code','compact-script',$4,'completed')")
            .bind(tool_call_id)
            .bind(session_id)
            .bind(t1)
            .bind(json!({"source":"fs.write(\"src/main.rs\", \"content\")"}))
            .execute(&test_db.pool)
            .await
            .expect("insert tool call");
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status, final_output) VALUES ($1,$2,$3,'completed','ok')")
            .bind(Uuid::new_v4())
            .bind(tool_call_id)
            .bind("fs.write(\"src/main.rs\", \"content\")")
            .execute(&test_db.pool)
            .await
            .expect("insert script run");
        let before_turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1").bind(session_id).fetch_one(&test_db.pool).await.expect("turn count");
        let before_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1").bind(session_id).fetch_one(&test_db.pool).await.expect("event count");
        let before_model_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_events WHERE session_id=$1").bind(session_id).fetch_one(&test_db.pool).await.expect("model count");

        let checkpoint = compaction::compact_session_through_latest_completed_turn(&test_db.pool, session_id, compaction::CompactionBudget::default()).await.expect("compact latest");
        assert_eq!(checkpoint.compacted_through_turn_id, Some(t2));
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM turns WHERE session_id=$1").bind(session_id).fetch_one(&test_db.pool).await.expect("turn count after"), before_turns);
        assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM model_events WHERE session_id=$1").bind(session_id).fetch_one(&test_db.pool).await.expect("model count after"), before_model_events);
        let after_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1").bind(session_id).fetch_one(&test_db.pool).await.expect("event count after");
        assert!(after_events > before_events, "compaction appends event evidence without deleting audit rows");
        let history = db::reconstructed_history(&test_db.pool, session_id).await.expect("history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].source, "compaction_checkpoint");
        assert_eq!(history[0].checkpoint_id, Some(checkpoint.id));
        let context = history[0].assistant.as_deref().unwrap_or_default();
        assert!(context.contains("Active task goal: Concrete owner active goal"));
        assert!(context.contains("policy.decision"));
        assert!(context.contains("concrete decision"));
        assert!(context.contains("fs.write("));
        assert!(context.contains("src/main.rs"));
        assert!(!history[0].assistant.as_deref().unwrap_or_default().contains("raw command output"));
        assert_ne!(t1, Uuid::nil());

        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn runtime_send_pre_send_compacts_before_model_dispatch_and_skips_under_threshold() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let over_session = db::new_session(&test_db.pool, &role, Some("runtime-send-over"), ".", Some("."), None, None).await.expect("over session");
        insert_completed_turn(&test_db.pool, over_session, &"large prior ".repeat(200), "prior assistant").await;
        let fake_over = FakeModelClient::default();
        let over_budget = compaction::CompactionBudget { pre_send_threshold: 1, fail_closed_threshold: 200_000, ..Default::default() };
        crate::runtime::send_with_model_client(&test_db.pool, over_session, "current over", &fake_over, over_budget).await.expect("over send");
        let observed = fake_over.observed_history.lock().expect("history").clone();
        assert_eq!(observed.len(), 1);
        assert!(observed[0].iter().any(|item| item.source == "compaction_checkpoint"), "model dispatch should see checkpoint-rooted history: {:?}", observed[0]);
        let checkpoints: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM compaction_checkpoints WHERE session_id=$1 AND status='completed'")
            .bind(over_session)
            .fetch_one(&test_db.pool)
            .await
            .expect("checkpoint count");
        assert_eq!(checkpoints, 1);

        let under_session = db::new_session(&test_db.pool, &role, Some("runtime-send-under"), ".", Some("."), None, None).await.expect("under session");
        insert_completed_turn(&test_db.pool, under_session, "small prior", "small assistant").await;
        let fake_under = FakeModelClient::default();
        let under_budget = compaction::CompactionBudget { pre_send_threshold: 200_000, fail_closed_threshold: 210_000, ..Default::default() };
        crate::runtime::send_with_model_client(&test_db.pool, under_session, "current under", &fake_under, under_budget).await.expect("under send");
        let observed = fake_under.observed_history.lock().expect("history").clone();
        assert_eq!(observed.len(), 1);
        assert!(observed[0].iter().any(|item| item.source == "reconstructed_session_history" && item.user == "small prior"));
        assert!(!observed[0].iter().any(|item| item.source == "compaction_checkpoint"));
        let checkpoints: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM compaction_checkpoints WHERE session_id=$1")
            .bind(under_session)
            .fetch_one(&test_db.pool)
            .await
            .expect("checkpoint count");
        assert_eq!(checkpoints, 0);

        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn runtime_send_fail_closed_after_compaction_without_model_dispatch() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("runtime-send-fail-closed"), ".", Some("."), None, None).await.expect("session");
        insert_completed_turn(&test_db.pool, session_id, "prior turn before unsafe send", "prior assistant").await;
        let fake = FakeModelClient::default();
        let budget = compaction::CompactionBudget { pre_send_threshold: 1, fail_closed_threshold: 1, ..Default::default() };
        let result = crate::runtime::send_with_model_client(&test_db.pool, session_id, "current unsafe send", &fake, budget).await;
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("fail-closed threshold"));
        assert!(fake.observed_history.lock().expect("history").is_empty(), "model client must not be dispatched after fail-closed compaction");
        let checkpoints: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM compaction_checkpoints WHERE session_id=$1 AND status='completed'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("checkpoint count");
        assert_eq!(checkpoints, 1, "compaction attempted before fail-closed rejection");
        let turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND input_text='current unsafe send'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("current turn count");
        assert_eq!(turns, 0, "fail-closed rejection must happen before creating the current running turn");

        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn requirements_review_creates_hidden_reviewer_and_filters_top_level_sessions() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-project"), ".", Some("."), None, None).await.expect("source session");
        let set_id = requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("test requirements".to_string()),
            requirements: vec![requirements::RequirementInput {
                key: "prove_hidden_reviewer".to_string(),
                statement: "Create a hidden reviewer session and keep top-level lists clean.".to_string(),
                severity: "must".to_string(),
                verification_method: json!({"method":"test"}),
            }],
        }).await.expect("set requirements");
        let turn_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user','claim','completed',now())")
            .bind(turn_id)
            .bind(source)
            .execute(&test_db.pool)
            .await
            .expect("turn");
        let claim = json!({
            "summary": "done",
            "requirements": {
                "prove_hidden_reviewer": {
                    "claim": "satisfied",
                    "evidence": ["test evidence"],
                    "justification": "reviewable",
                    "risk": "low"
                }
            }
        }).to_string();
        let outcome = requirements::record_source_final_response(&test_db.pool, source, turn_id, &claim).await.expect("claim outcome").expect("claim record");
        assert_eq!(outcome.outcome, requirements::SourcePacketOutcome::Reviewable);
        let status = requirements::status(&test_db.pool, source).await.expect("status");
        assert_eq!(status.active_set_id, Some(set_id));
        let reviewer_id = status.reviewer_session_id.expect("reviewer");
        let reviewer = db::session_record(&test_db.pool, reviewer_id).await.expect("reviewer record");
        assert_eq!(reviewer.parent_session_id, Some(source));
        assert_eq!(reviewer.session_kind, "requirementsReviewer");
        assert!(reviewer.hidden);
        let list = db::list_sessions(&test_db.pool, true).await.expect("list sessions");
        assert!(list.iter().any(|session| session.id == source));
        assert!(!list.iter().any(|session| session.id == reviewer_id));
        let snapshot = projection::build_runtime_projection_snapshot(&test_db.pool, Some(source)).await.expect("snapshot");
        assert!(snapshot.sessions.iter().any(|session| session.id == source.to_string()));
        assert!(!snapshot.sessions.iter().any(|session| session.id == reviewer_id.to_string()));
        assert_eq!(snapshot.selected_session.unwrap().metadata.pointer("/requirementsReview/reviewerSessionId").and_then(Value::as_str), Some(reviewer_id.to_string().as_str()));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn requirements_reviewer_verdict_updates_progress_and_deactivates_on_pass() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-verdict"), ".", Some("."), None, None).await.expect("source session");
        requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("verdict requirements".to_string()),
            requirements: vec![
                requirements::RequirementInput { key: "first_requirement".to_string(), statement: "First requirement passes.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) },
                requirements::RequirementInput { key: "second_requirement".to_string(), statement: "Second requirement remains waived.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) },
            ],
        }).await.expect("set requirements");
        let claim_turn = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user','claim','completed',now())")
            .bind(claim_turn)
            .bind(source)
            .execute(&test_db.pool)
            .await
            .expect("claim turn");
        let claim = json!({"summary":"done","requirements":{
            "first_requirement":{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"},
            "second_requirement":{"claim":"blocked","evidence":["waiver request"],"justification":"needs waiver review","risk":"medium"}
        }}).to_string();
        requirements::record_source_final_response(&test_db.pool, source, claim_turn, &claim).await.expect("claim");
        let reviewer_id = requirements::status(&test_db.pool, source).await.expect("status").reviewer_session_id.expect("reviewer");
        let verdict_turn = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'assistant','verdict','completed',now())")
            .bind(verdict_turn)
            .bind(reviewer_id)
            .execute(&test_db.pool)
            .await
            .expect("verdict turn");
        let verdict = json!({
            "summary": "all pass",
            "requirements": {
                "first_requirement": {"verdict": "pass", "evidence": ["ok"], "justification": "complete", "risk": "low"},
                "second_requirement": {"verdict": "waiverAccepted", "evidence": ["waived"], "justification": "waiver accepted", "risk": "medium"}
            },
            "overallVerdict": "pass",
            "route": "source"
        }).to_string();
        assert!(requirements::record_reviewer_verdict(&test_db.pool, reviewer_id, verdict_turn, &verdict).await.expect("verdict"));
        let inactive = requirements::status(&test_db.pool, source).await.expect("inactive");
        assert!(!inactive.active);
        let packets = requirements::packet_history(&test_db.pool, source).await.expect("packets");
        assert!(packets.iter().any(|packet| packet["packetKind"] == "claim"));
        assert!(packets.iter().any(|packet| packet["packetKind"] == "verdict"));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn requirements_schema_attaches_only_for_active_source_turns() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let active_session = db::new_session(&test_db.pool, &role, Some("requirements-schema"), ".", Some("."), None, None).await.expect("active");
        let inactive_session = db::new_session(&test_db.pool, &role, Some("requirements-schema"), ".", Some("."), None, None).await.expect("inactive");
        requirements::set_active_requirements(&test_db.pool, active_session, requirements::RequirementSetInput {
            title: Some("schema requirements".to_string()),
            requirements: vec![requirements::RequirementInput { key: "schema_attached".to_string(), statement: "Schema is attached.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) }],
        }).await.expect("set requirements");
        let observed = Arc::new(StdMutex::new(Vec::<Value>::new()));
        let fake = FakeModelClient {
            observed_request_shapes: Arc::clone(&observed),
            direct_final_text: Some(r#"{"summary":"progress","requirements":null}"#),
            ..Default::default()
        };
        runtime::send_with_model_client(&test_db.pool, active_session, "active", &fake, compaction::CompactionBudget::from_env()).await.expect("active send");
        runtime::send_with_model_client(&test_db.pool, inactive_session, "inactive", &fake, compaction::CompactionBudget::from_env()).await.expect("inactive send");
        let shapes = observed.lock().expect("shapes");
        assert_eq!(shapes.len(), 2);
        assert_eq!(shapes[0].pointer("/requirements_schema_evidence/kind").and_then(Value::as_str), Some("sourceClaim"));
        assert_eq!(shapes[0].pointer("/text/format/type").and_then(Value::as_str), Some("json_schema"));
        assert_eq!(shapes[0].pointer("/requirements_schema_evidence/canonicalCount").and_then(Value::as_u64), Some(1));
        println!("SOURCE_REQUIREMENTS_SCHEMA_EXAMPLE={}", serde_json::to_string_pretty(&shapes[0]["text"]["format"]).expect("source schema evidence"));
        println!("SOURCE_REQUIREMENTS_SCHEMA_EVIDENCE={}", serde_json::to_string_pretty(&shapes[0]["requirements_schema_evidence"]).expect("source schema metadata"));
        assert!(shapes[1].get("requirements_schema_evidence").is_none());
        assert!(shapes[1].pointer("/text/format").is_none());
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn requirements_reviewable_source_claim_dispatches_fake_reviewer_and_updates_progress() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-dispatch"), ".", Some("."), None, None).await.expect("source");
        requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("dispatch requirements".to_string()),
            requirements: vec![requirements::RequirementInput {
                key: "dispatch_happens".to_string(),
                statement: "A reviewable source claim starts a nested reviewer turn.".to_string(),
                severity: "must".to_string(),
                verification_method: json!({"method":"test"}),
            }],
        }).await.expect("set");
        let observed = Arc::new(StdMutex::new(Vec::<Value>::new()));
        let fake = FakeModelClient {
            observed_request_shapes: Arc::clone(&observed),
            direct_final_text: Some(r#"{"summary":"source done","requirements":{"dispatch_happens":{"claim":"satisfied","evidence":["turn"],"justification":"done","risk":"low"}}}"#),
            reviewer_final_text: Some(r#"{"summary":"passes","requirements":{"dispatch_happens":{"verdict":"pass","evidence":["reviewed"],"justification":"ok","risk":"low"}},"overallVerdict":"pass","route":"source"}"#),
            ..Default::default()
        };
        runtime::send_with_model_client(&test_db.pool, source, "claim completion", &fake, compaction::CompactionBudget::default()).await.expect("send");
        let reviewer_id: Uuid = sqlx::query_scalar("SELECT id FROM sessions WHERE parent_session_id=$1 AND session_kind='requirementsReviewer' AND hidden=true")
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("reviewer");
        let reviewer_turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1")
            .bind(reviewer_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("reviewer turns");
        assert_eq!(reviewer_turns, 1);
        let shapes = observed.lock().expect("observed shapes");
        assert!(shapes.iter().any(|shape| shape.pointer("/requirements_schema_evidence/kind").and_then(Value::as_str) == Some("reviewerVerdict")));
        if let Some(reviewer_shape) = shapes.iter().find(|shape| shape.pointer("/requirements_schema_evidence/kind").and_then(Value::as_str) == Some("reviewerVerdict")) {
            println!("REVIEWER_REQUIREMENTS_SCHEMA_EXAMPLE={}", serde_json::to_string_pretty(&reviewer_shape["text"]["format"]).expect("reviewer schema evidence"));
            println!("REVIEWER_REQUIREMENTS_SCHEMA_EVIDENCE={}", serde_json::to_string_pretty(&reviewer_shape["requirements_schema_evidence"]).expect("reviewer schema metadata"));
        }
        let inactive = requirements::status(&test_db.pool, source).await.expect("status");
        assert!(!inactive.active, "pass verdict deactivates the active RequirementSet");
        let packets = requirements::packet_history(&test_db.pool, source).await.expect("packets");
        assert!(packets.iter().any(|packet| packet["packetKind"] == "claim"));
        assert!(packets.iter().any(|packet| packet["packetKind"] == "verdict"));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn requirements_non_reviewable_packets_emit_corrections_and_do_not_create_reviewer() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-corrections"), ".", Some("."), None, None).await.expect("source");
        requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("correction requirements".to_string()),
            requirements: vec![requirements::RequirementInput {
                key: "work_first".to_string(),
                statement: "Work must be done before review.".to_string(),
                severity: "must".to_string(),
                verification_method: json!({"method":"test"}),
            }],
        }).await.expect("set");
        for body in [
            r#"{"summary":"commentary","requirements":null}"#,
            r#"{"summary":"not yet","requirements":{"work_first":{"claim":"notSatisfied","evidence":[],"justification":"not yet","risk":"unknown"}}}"#,
            "not json",
        ] {
            let turn_id = Uuid::new_v4();
            sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user','claim','completed',now())")
                .bind(turn_id)
                .bind(source)
                .execute(&test_db.pool)
                .await
                .expect("turn");
            requirements::record_source_final_response(&test_db.pool, source, turn_id, body).await.expect("packet");
        }
        let reviewers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE parent_session_id=$1 AND session_kind='requirementsReviewer'")
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("reviewer count");
        assert_eq!(reviewers, 0);
        let corrections: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='requirements.sourceCorrection'")
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("corrections");
        assert_eq!(corrections, 3);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn requirements_lifecycle_close_archive_and_fork_preserve_hidden_reviewer_semantics() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-lifecycle"), ".", Some("."), None, None).await.expect("source");
        requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("lifecycle requirements".to_string()),
            requirements: vec![requirements::RequirementInput { key: "lifecycle_checked".to_string(), statement: "Lifecycle is checked.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) }],
        }).await.expect("set");
        let turn_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user','claim','completed',now())")
            .bind(turn_id)
            .bind(source)
            .execute(&test_db.pool)
            .await
            .expect("turn");
        requirements::record_source_final_response(&test_db.pool, source, turn_id, r#"{"summary":"done","requirements":{"lifecycle_checked":{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"}}}"#).await.expect("claim");
        let reviewer = requirements::status(&test_db.pool, source).await.expect("status").reviewer_session_id.expect("reviewer");
        let fork_turn = insert_completed_turn(&test_db.pool, source, "before fork", "assistant").await;
        let fork = db::fork_session(&test_db.pool, source, fork_turn).await.expect("fork");
        let fork_reviewers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE parent_session_id=$1 AND session_kind='requirementsReviewer'")
            .bind(fork)
            .fetch_one(&test_db.pool)
            .await
            .expect("fork reviewers");
        assert_eq!(fork_reviewers, 0);
        db::archive_session(&test_db.pool, source).await.expect("archive");
        assert!(!db::list_sessions(&test_db.pool, true).await.expect("list").iter().any(|session| session.id == reviewer));
        assert_eq!(db::session_record(&test_db.pool, reviewer).await.expect("exact reviewer").id, reviewer);
        db::close_session(&test_db.pool, source, "done", 0).await.expect("close");
        let status: String = sqlx::query_scalar("SELECT status FROM sessions WHERE id=$1")
            .bind(reviewer)
            .fetch_one(&test_db.pool)
            .await
            .expect("reviewer status");
        assert_eq!(status, "closed");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn requirements_server_api_and_gui_projection_shapes_are_typed_and_reviewers_hidden() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-api"), ".", Some("."), None, None).await.expect("source");
        let app = app(ServerState::new(test_db.pool.clone()));
        let (status, body) = request_json(app.clone(), Method::POST, &format!("/sessions/{source}/requirements"), json!({
            "title": "api requirements",
            "requirements": [{"key":"api_visible","statement":"API shape is visible.","severity":"must","verificationMethod":{"method":"test"}}]
        })).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.pointer("/status/active").and_then(Value::as_bool), Some(true));
        let (status, body) = request_json(app.clone(), Method::GET, &format!("/sessions/{source}/requirements"), Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.pointer("/requirements/progress/0/requirementKey").and_then(Value::as_str), Some("api_visible"));
        let claim_turn = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user','claim','completed',now())")
            .bind(claim_turn)
            .bind(source)
            .execute(&test_db.pool)
            .await
            .expect("turn");
        requirements::record_source_final_response(&test_db.pool, source, claim_turn, r#"{"summary":"done","requirements":{"api_visible":{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"}}}"#).await.expect("claim");
        let reviewer = requirements::status(&test_db.pool, source).await.expect("status").reviewer_session_id.expect("reviewer");
        let (status, body) = request_json(app.clone(), Method::GET, &format!("/sessions/{source}/requirements/packets"), Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.pointer("/packets/0/packetKind").and_then(Value::as_str), Some("claim"));
        let snapshot = projection::build_runtime_projection_snapshot(&test_db.pool, Some(source)).await.expect("snapshot");
        let selected = snapshot.selected_session.expect("selected");
        let reviewer_text = reviewer.to_string();
        assert_eq!(selected.requirements_review.as_ref().and_then(|summary| summary.reviewer_session_id.as_deref()), Some(reviewer_text.as_str()));
        assert!(!snapshot.sessions.iter().any(|session| session.id == reviewer_text));
        let (status, body) = request_json(app.clone(), Method::POST, &format!("/sessions/{source}/requirements/clear"), json!({})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.pointer("/status").and_then(Value::as_str), Some("inactive"));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn requirements_rows_and_packets_survive_compaction() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-compact"), ".", Some("."), None, None).await.expect("source");
        requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("compact requirements".to_string()),
            requirements: vec![requirements::RequirementInput { key: "compact_preserves".to_string(), statement: "Compaction preserves requirements state.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) }],
        }).await.expect("set");
        let turn = insert_completed_turn(&test_db.pool, source, "claim", r#"{"summary":"commentary","requirements":null}"#).await;
        requirements::record_source_final_response(&test_db.pool, source, turn, r#"{"summary":"commentary","requirements":null}"#).await.expect("packet");
        compaction::compact_session_through_turn(&test_db.pool, source, turn, compaction::CompactionBudget::default()).await.expect("compact");
        let status = requirements::status(&test_db.pool, source).await.expect("status");
        assert!(status.active);
        assert_eq!(status.total, 1);
        let packets = requirements::packet_history(&test_db.pool, source).await.expect("packets");
        assert!(packets.iter().any(|packet| packet["packetKind"] == "claimNull"));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn requirements_invalid_source_packet_is_recorded_and_not_routed() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-invalid-source"), ".", Some("."), None, None).await.expect("source");
        requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("invalid source requirements".to_string()),
            requirements: vec![requirements::RequirementInput { key: "schema_checked".to_string(), statement: "Source packet schema is checked.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) }],
        }).await.expect("set");
        let turn = insert_completed_turn(&test_db.pool, source, "claim", "assistant").await;
        let invalid = r#"{"summary":"done","requirements":{"schema_checked":{"claim":"satisfied","evidence":["e"],"justification":"missing risk"}}}"#;
        let record = requirements::record_source_final_response(&test_db.pool, source, turn, invalid).await.expect("record").expect("active");
        assert_eq!(record.outcome, requirements::SourcePacketOutcome::Invalid);
        let reviewer_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE parent_session_id=$1 AND session_kind='requirementsReviewer'")
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("reviewers");
        assert_eq!(reviewer_count, 0);
        let packets = requirements::packet_history(&test_db.pool, source).await.expect("packets");
        assert!(packets.iter().any(|packet| packet["packetKind"] == "claimInvalid" && packet["validationError"].as_str().unwrap_or_default().contains("risk")));
        let corrections: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='requirements.sourceCorrection' AND status='requirementsInvalid'")
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("corrections");
        assert_eq!(corrections, 1);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn requirements_invalid_reviewer_packet_does_not_mutate_progress() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-invalid-reviewer"), ".", Some("."), None, None).await.expect("source");
        requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("invalid reviewer requirements".to_string()),
            requirements: vec![requirements::RequirementInput { key: "verdict_checked".to_string(), statement: "Reviewer verdict schema is checked.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) }],
        }).await.expect("set");
        let claim_turn = insert_completed_turn(&test_db.pool, source, "claim", "assistant").await;
        requirements::record_source_final_response(&test_db.pool, source, claim_turn, r#"{"summary":"done","requirements":{"verdict_checked":{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"}}}"#).await.expect("claim");
        let reviewer = requirements::status(&test_db.pool, source).await.expect("status").reviewer_session_id.expect("reviewer");
        let verdict_turn = insert_completed_turn(&test_db.pool, reviewer, "verdict", "assistant").await;
        let invalid = r#"{"summary":"bad","requirements":{"verdict_checked":{"verdict":"pass","evidence":["e"],"justification":"j","risk":"low"}},"route":"source"}"#;
        assert!(requirements::record_reviewer_verdict(&test_db.pool, reviewer, verdict_turn, invalid).await.expect("verdict processed"));
        let status = requirements::status(&test_db.pool, source).await.expect("status");
        assert!(status.active);
        assert_eq!(status.unresolved, 1);
        assert_eq!(status.passed, 0);
        assert!(status.latest_verdict_packet_id.is_none());
        let packets = requirements::packet_history(&test_db.pool, source).await.expect("packets");
        assert!(packets.iter().any(|packet| packet["packetKind"] == "verdictInvalid" && packet["validationError"].as_str().unwrap_or_default().contains("overallVerdict")));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn requirements_reviewer_reconstruction_context_survives_compaction() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-reviewer-reconstruction"), ".", Some("."), None, None).await.expect("source");
        requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("reconstruction requirements".to_string()),
            requirements: vec![requirements::RequirementInput { key: "reconstructs_claim".to_string(), statement: "Reviewer reconstruction includes source claim context.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) }],
        }).await.expect("set");
        let turn = insert_completed_turn(&test_db.pool, source, "claim", "assistant").await;
        requirements::record_source_final_response(&test_db.pool, source, turn, r#"{"summary":"done","requirements":{"reconstructs_claim":{"claim":"satisfied","evidence":["claim evidence"],"justification":"j","risk":"low"}}}"#).await.expect("claim");
        let reviewer = requirements::status(&test_db.pool, source).await.expect("status").reviewer_session_id.expect("reviewer");
        compaction::compact_session_through_turn(&test_db.pool, source, turn, compaction::CompactionBudget::default()).await.expect("compact source");
        let runtime_message = requirements::requirements_runtime_message(&test_db.pool, reviewer).await.expect("runtime message").expect("reviewer schema");
        assert!(runtime_message.text.contains("<requirements_review_context>"));
        assert!(runtime_message.text.contains("canonicalSet"));
        assert!(runtime_message.text.contains("latestClaimPacket"));
        assert!(runtime_message.text.contains("reconstructs_claim"));
        assert!(runtime_message.text.contains("claim evidence"));
        println!("REVIEWER_RECONSTRUCTION_CONTEXT={}", runtime_message.text);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn compaction_failure_paths_record_failed_checkpoints() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("compact-fail-record"), ".", Some("."), None, None).await.expect("session");
        let no_turn = compaction::compact_session_through_latest_completed_turn(&test_db.pool, session_id, compaction::CompactionBudget::default()).await;
        assert!(no_turn.is_err());
        let failed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM compaction_checkpoints WHERE session_id=$1 AND status='failed' AND failure_info->>'reason' LIKE '%no completed turns%'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("failed no-turn checkpoint");
        assert_eq!(failed, 1);

        let invalid_turn = Uuid::new_v4();
        let invalid = compaction::compact_session_through_turn(&test_db.pool, session_id, invalid_turn, compaction::CompactionBudget::default()).await;
        assert!(invalid.is_err());
        let failed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM compaction_checkpoints WHERE session_id=$1 AND status='failed' AND failure_info->>'requestedThroughTurnId'=$2 AND failure_info->>'reason' LIKE '%not found%'")
            .bind(session_id)
            .bind(invalid_turn.to_string())
            .fetch_one(&test_db.pool)
            .await
            .expect("failed invalid checkpoint");
        assert_eq!(failed, 1);

        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn forked_session_reconstruction_honors_compaction_boundaries() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let parent = db::new_session(&test_db.pool, &role, Some("compact-fork-parent"), ".", Some("."), None, None).await.expect("parent");
        let t1 = insert_completed_turn(&test_db.pool, parent, "parent before fork one", "parent assistant one").await;
        let t2 = insert_completed_turn(&test_db.pool, parent, "parent fork boundary", "parent assistant two").await;
        let child = db::fork_session(&test_db.pool, parent, t2).await.expect("fork child");
        insert_completed_turn(&test_db.pool, child, "child local turn", "child assistant").await;
        let parent_checkpoint = compaction::compact_session_through_turn(&test_db.pool, parent, t2, compaction::CompactionBudget::default()).await.expect("parent compact");
        insert_completed_turn(&test_db.pool, parent, "parent after fork not inherited", "parent later").await;

        let child_history = db::reconstructed_history(&test_db.pool, child).await.expect("child history");
        assert!(child_history.iter().any(|item| item.source == "compaction_checkpoint" && item.checkpoint_id == Some(parent_checkpoint.id)));
        assert!(child_history.iter().any(|item| item.user == "child local turn"));
        assert!(!child_history.iter().any(|item| item.user == "parent after fork not inherited"));
        assert!(!child_history.iter().any(|item| item.turn_id == t1 && item.source == "reconstructed_session_history"));

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
        workflow_memory::insert_memory_event(&test_db.pool, session_id, Some(turn_id), Some(script_id), Some(memory_id), "workflow_memory.promoted", json!({"sourceHash":"hash"})).await.expect("memory event");

        let projected = projection::build_runtime_projection_snapshot(&test_db.pool, Some(session_id)).await.expect("projection");
        assert_eq!(projected.workflow_memories.len(), 1);
        assert_eq!(projected.workflow_memories[0].id, memory_id.to_string());
        assert_eq!(projected.workflow_memories[0].source_script_run_id.as_deref(), Some(script_id.to_string().as_str()));
        assert_eq!(projected.workflow_memories[0].source_starlark.as_deref(), Some("output(\"ok\")"));
        assert_eq!(projected.workflow_memories[0].provider.as_deref(), Some("deterministic"));
        assert_eq!(projected.workflow_memories[0].recent_events[0].event_type, "workflow_memory.promoted");
        let invisible_projected = projection::build_runtime_projection_snapshot(&test_db.pool, Some(other_session_id)).await.expect("other projection");
        assert!(invisible_projected.workflow_memories.is_empty());

        let response = router.clone().oneshot(Request::builder().uri(format!("/workflow-memories?sessionId={session_id}")).body(Body::empty()).expect("request")).await.expect("list");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("list body");
        let list: Value = serde_json::from_slice(&bytes).expect("list json");
        assert_eq!(list[0]["id"], memory_id.to_string());
        assert_eq!(list[0]["sourceScriptRunId"], script_id.to_string());
        assert_eq!(list[0]["sourcePreview"], "output(\"ok\")");
        assert_eq!(list[0]["provider"], "deterministic");
        assert_eq!(list[0]["dimensions"], workflow_memory::DEFAULT_DIMENSIONS as i64);
        assert_eq!(list[0]["sourceHash"], "hash");
        let response = router.clone().oneshot(Request::builder().uri(format!("/workflow-memories/{memory_id}?sessionId={session_id}")).body(Body::empty()).expect("request")).await.expect("show");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("show body");
        let show: Value = serde_json::from_slice(&bytes).expect("show json");
        assert_eq!(show["sourceStarlark"], "output(\"ok\")");
        assert_eq!(show["commandFingerprint"], "plain");
        for (feedback, expected_event) in [
            ("attempted", "workflow_memory.mark_attempted"),
            ("helpful", "workflow_memory.helpful"),
            ("notHelpful", "workflow_memory.mark_not_helpful"),
        ] {
            let (status, _) = request_json(router.clone(), Method::POST, &format!("/workflow-memories/{memory_id}/feedback"), json!({"sessionId": session_id, "feedback": feedback, "payload":{"variant":true}})).await;
            assert_eq!(status, StatusCode::OK);
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_memory_events WHERE memory_id=$1 AND event_type=$2")
                .bind(memory_id)
                .bind(expected_event)
                .fetch_one(&test_db.pool)
                .await
                .expect("feedback event count");
            assert_eq!(count, 1, "{expected_event} persisted");
        }
        let response = router.clone().oneshot(Request::builder().uri(format!("/workflow-memories/{memory_id}/events?sessionId={session_id}")).body(Body::empty()).expect("request")).await.expect("events");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("events body");
        let events: Value = serde_json::from_slice(&bytes).expect("events json");
        assert!(events.as_array().unwrap().iter().any(|event| event["eventType"] == "workflow_memory.mark_attempted"));
        assert!(events.as_array().unwrap().iter().any(|event| event["eventType"] == "workflow_memory.helpful"));
        assert!(events.as_array().unwrap().iter().any(|event| event["eventType"] == "workflow_memory.mark_not_helpful"));
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
