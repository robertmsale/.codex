use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use axum::extract::rejection::JsonRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use robdex_agent_runtime_projection::{RoleEditorDraft, RuntimeDelta, RuntimeDeltaKind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use tokio::sync::{Mutex, watch};
use tokio::time::{Duration, interval};
use uuid::Uuid;

use crate::errors::{RuntimeDomainError, RuntimeErrorKind};
use crate::model::ModelClient;
use crate::{approvals, command_registry, compaction, db, lifecycle_hooks, projection, requirements, routing, runtime, starlark_host, workflow_memory};

#[derive(Clone)]
pub struct ServerState {
    pub pool: PgPool,
    pub runtime_identity: String,
    pub active_submit_drains: Arc<Mutex<HashSet<Uuid>>>,
    pub active_compactions: Arc<Mutex<HashSet<Uuid>>>,
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
            active_submit_drains: Arc::new(Mutex::new(HashSet::new())),
            active_compactions: Arc::new(Mutex::new(HashSet::new())),
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
            active_submit_drains: Arc::new(Mutex::new(HashSet::new())),
            active_compactions: Arc::new(Mutex::new(HashSet::new())),
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
    state.reconcile_submitted_inputs();
    Router::new()
        .route("/health", get(health))
        .route("/state/snapshot", get(snapshot))
        .route("/state/ws", get(state_ws))
        .route("/projects", get(list_projects).post(create_project))
        .route("/projects/{project_key}", post(update_project))
        .route("/projects/{project_key}/archive", post(archive_project))
        .route("/projects/{project_key}/unarchive", post(unarchive_project))
        .route("/projects/{project_key}/runtime-config/validate", post(validate_project_runtime_config))
        .route("/projects/{project_key}/runtime-config", get(show_project_runtime_config).post(import_project_runtime_config))
        .route("/projects/{project_key}/runtime-config/versions", get(list_project_runtime_config_versions))
        .route("/projects/{project_key}/runtime-config/versions/{version_id}/activate", post(activate_project_runtime_config))
        .route("/projects/{project_key}/runtime-config/versions/{version_id}/archive", post(archive_project_runtime_config))
        .route("/projects/{project_key}/runtime-config/versions/{version_id}/export", get(export_project_runtime_config))
        .route("/projects/{project_key}/runtime-config/versions/{version_id}/evaluations", get(inspect_project_runtime_hook_evaluations))
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
        .route("/sessions/{session_id}/requirements/reviewer/send", post(send_requirements_reviewer_message))
        .route("/sessions/{session_id}/settings", post(update_session_settings))
        .route("/sessions/{session_id}/archive", post(archive_session))
        .route("/sessions/{session_id}/fork", post(fork_session))
        .route("/sessions/{session_id}/processes/{handle}/terminate", post(terminate_process))
        .route("/sessions/{session_id}/processes/{handle}/input", post(input_process))
        .route("/sessions/{session_id}/processes/{handle}/flush", post(flush_process))
        .route("/sessions/{session_id}/image-artifacts/{image_id}", get(image_artifact_metadata))
        .route("/sessions/{session_id}/image-artifacts/{image_id}/json", get(image_artifact_full_json))
        .route("/sessions/{session_id}/image-artifacts/{image_id}/thumbnail", get(image_artifact_thumbnail))
        .route("/sessions/{session_id}/image-artifacts/{image_id}/full", get(image_artifact_full))
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
            json!({"surfaceId":"compaction","title":"Compaction","rowCount": projection.statistics.compaction_checkpoints, "actionCount": 1}),
            json!({"surfaceId":"processManager","title":"Process Manager","rowCount": projection.statistics.managed_processes, "actionCount": 4}),
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
                object.insert("selectedSessionIdentity".to_string(), json!(projection.selected_session.as_ref().map(|session| json!({"id": session.id, "title": session.title, "status": if session.status == "stopped" { "Idle" } else { session.status.as_str() }, "executionStatus": session.status}))));
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeConfigValidateRequest {
    source_text: String,
    #[serde(default)]
    manifest: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeConfigImportRequest {
    source_text: String,
    manifest: Value,
    author: String,
}

async fn validate_project_runtime_config(
    Path(_project_key): Path<String>,
    payload: std::result::Result<Json<RuntimeConfigValidateRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let compiled_manifest = if let Some(manifest) = request.manifest.as_ref() {
        lifecycle_hooks::validate_hook_source(&request.source_text)?;
        lifecycle_hooks::validate_runtime_manifest(manifest)?;
        manifest.clone()
    } else {
        lifecycle_hooks::compile_project_runtime_source(&request.source_text)?
    };
    Ok(Json(json!({
        "valid": true,
        "sourceHash": lifecycle_hooks::source_hash(&request.source_text),
        "manifest": compiled_manifest,
        "limits": {
            "maxHookSourceBytes": lifecycle_hooks::MAX_HOOK_SOURCE_BYTES,
            "maxContextBytes": lifecycle_hooks::MAX_CONTEXT_BYTES,
            "maxReturnedIntents": lifecycle_hooks::MAX_RETURNED_INTENTS,
            "evaluationTimeoutMs": lifecycle_hooks::EVALUATION_TIMEOUT_MS,
            "evaluationFuelSteps": lifecycle_hooks::EVALUATION_FUEL_STEPS
        }
    })))
}

async fn import_project_runtime_config(
    State(state): State<ServerState>,
    Path(project_key): Path<String>,
    payload: std::result::Result<Json<RuntimeConfigImportRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let version_id = lifecycle_hooks::persist_project_runtime_config(
        &state.pool,
        project_key.trim(),
        &request.source_text,
        request.manifest,
        request.author.trim(),
    ).await?;
    Ok(Json(json!({"projectKey": project_key, "versionId": version_id, "status":"draft"})))
}

async fn show_project_runtime_config(State(state): State<ServerState>, Path(project_key): Path<String>) -> Result<Json<Value>, ApiError> {
    let row = sqlx::query("SELECT id, project_key, source_hash, compiled_manifest, activation_status, author, validation_packet, created_at, activated_at, archived_at FROM project_runtime_config_versions WHERE project_key=$1 AND activation_status='active' ORDER BY activated_at DESC NULLS LAST, created_at DESC LIMIT 1")
        .bind(project_key.trim())
        .fetch_optional(&state.pool)
        .await.map_err(anyhow::Error::from)?;
    Ok(Json(row.map(runtime_config_row_json).unwrap_or_else(|| json!({"projectKey": project_key, "active": null}))))
}

async fn list_project_runtime_config_versions(State(state): State<ServerState>, Path(project_key): Path<String>) -> Result<Json<Value>, ApiError> {
    let rows = sqlx::query("SELECT id, project_key, source_hash, compiled_manifest, activation_status, author, validation_packet, created_at, activated_at, archived_at FROM project_runtime_config_versions WHERE project_key=$1 ORDER BY created_at DESC")
        .bind(project_key.trim())
        .fetch_all(&state.pool)
        .await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({"versions": rows.into_iter().map(runtime_config_row_json).collect::<Vec<_>>() })))
}

async fn activate_project_runtime_config(State(state): State<ServerState>, Path((project_key, version_id)): Path<(String, Uuid)>) -> Result<Json<Value>, ApiError> {
    lifecycle_hooks::activate_project_runtime_config(&state.pool, project_key.trim(), version_id).await?;
    Ok(Json(json!({"projectKey": project_key, "versionId": version_id, "status":"active"})))
}

async fn archive_project_runtime_config(State(state): State<ServerState>, Path((project_key, version_id)): Path<(String, Uuid)>) -> Result<Json<Value>, ApiError> {
    sqlx::query("UPDATE project_runtime_config_versions SET activation_status='archived', archived_at=now() WHERE project_key=$1 AND id=$2")
        .bind(project_key.trim())
        .bind(version_id)
        .execute(&state.pool)
        .await.map_err(anyhow::Error::from)?;
    Ok(Json(json!({"projectKey": project_key, "versionId": version_id, "status":"archived"})))
}

async fn export_project_runtime_config(State(state): State<ServerState>, Path((project_key, version_id)): Path<(String, Uuid)>) -> Result<Json<Value>, ApiError> {
    let row = sqlx::query("SELECT id, project_key, source_text, source_hash, compiled_manifest, activation_status, author, validation_packet, created_at, activated_at, archived_at FROM project_runtime_config_versions WHERE project_key=$1 AND id=$2")
        .bind(project_key.trim())
        .bind(version_id)
        .fetch_one(&state.pool)
        .await.map_err(anyhow::Error::from)?;
    let source_text = row.get::<String, _>("source_text");
    let mut value = runtime_config_row_json(row);
    if let Some(object) = value.as_object_mut() {
        object.insert("sourceText".to_string(), source_text.into());
    }
    Ok(Json(value))
}

async fn inspect_project_runtime_hook_evaluations(State(state): State<ServerState>, Path((project_key, version_id)): Path<(String, Uuid)>) -> Result<Json<Value>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT he.id, he.lifecycle_event_id, he.session_id, he.turn_id, he.input_context_hash,
               he.returned_intents, he.validation_status, he.applied_intent_ids, he.errors,
               he.timing_metadata, he.created_at
        FROM hook_evaluations he
        JOIN project_runtime_config_versions v ON v.id = he.hook_version_id
        WHERE v.project_key=$1 AND he.hook_version_id=$2
        ORDER BY he.created_at DESC
        LIMIT 100
        "#,
    )
    .bind(project_key.trim())
    .bind(version_id)
    .fetch_all(&state.pool)
    .await
    .map_err(anyhow::Error::from)?;
    Ok(Json(json!({"evaluations": rows.into_iter().map(|row| json!({
        "id": row.get::<Uuid, _>("id"),
        "lifecycleEventId": row.get::<Uuid, _>("lifecycle_event_id"),
        "sessionId": row.get::<Option<Uuid>, _>("session_id"),
        "turnId": row.get::<Option<Uuid>, _>("turn_id"),
        "inputContextHash": row.get::<String, _>("input_context_hash"),
        "returnedIntents": row.get::<Value, _>("returned_intents"),
        "validationStatus": row.get::<String, _>("validation_status"),
        "appliedIntentIds": row.get::<Value, _>("applied_intent_ids"),
        "errors": row.get::<Value, _>("errors"),
        "timingMetadata": row.get::<Value, _>("timing_metadata"),
        "createdAt": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
    })).collect::<Vec<_>>() })))
}

fn runtime_config_row_json(row: sqlx::postgres::PgRow) -> Value {
    json!({
        "id": row.get::<Uuid, _>("id"),
        "projectKey": row.get::<String, _>("project_key"),
        "sourceHash": row.get::<String, _>("source_hash"),
        "manifest": row.get::<Value, _>("compiled_manifest"),
        "status": row.get::<String, _>("activation_status"),
        "author": row.get::<String, _>("author"),
        "validationPacket": row.get::<Value, _>("validation_packet"),
        "createdAt": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
        "activatedAt": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("activated_at"),
        "archivedAt": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("archived_at"),
    })
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
    state.active_compactions.lock().await.insert(session_id);
    let checkpoint_result = if let Some(through_turn) = request.through_turn {
        compaction::compact_session_through_turn(
            &state.pool,
            session_id,
            through_turn,
            compaction::CompactionBudget::from_env(),
        )
        .await
    } else {
        compaction::compact_session_through_latest_completed_turn(
            &state.pool,
            session_id,
            compaction::CompactionBudget::from_env(),
        )
        .await
    };
    state.active_compactions.lock().await.remove(&session_id);
    if db::next_accepted_submitted_input(&state.pool, session_id).await?.is_some() {
        let should_spawn = {
            let mut active = state.active_submit_drains.lock().await;
            active.insert(session_id)
        };
        if should_spawn {
            spawn_submit_worker(state.clone(), session_id);
        }
    }
    let checkpoint = checkpoint_result?;
    Ok(Json(json!({
        "sessionId": session_id,
        "checkpointId": checkpoint.id,
        "status": checkpoint.status,
        "compactedThroughTurnId": checkpoint.compacted_through_turn_id,
    })))
}

async fn dispatch_one_submitted_input(
    pool: &PgPool,
    session_id: Uuid,
    input: db::SubmittedInputRecord,
    model_client: Option<Arc<dyn ModelClient + Send + Sync>>,
) -> Result<Uuid> {
    let result = if let Some(model) = model_client {
        runtime::send_with_model_client(
            pool,
            session_id,
            &input.content,
            model.as_ref(),
            compaction::CompactionBudget::from_env(),
        )
        .await
    } else {
        runtime::send(pool, session_id, &input.content).await
    };
    match result {
        Ok(turn_id) => {
            db::mark_submitted_input_applied(pool, input.id, turn_id).await?;
            db::append_event(
                pool,
                session_id,
                Some(turn_id),
                "submitted_input",
                Some(input.id),
                "submitted_input.applied",
                Some("applied"),
                json!({"submittedInputId": input.id, "placementTurnId": turn_id, "disposition": input.disposition}),
            )
            .await?;
            Ok(turn_id)
        }
        Err(error) => Err(error),
    }
}

fn spawn_submit_worker(state: ServerState, session_id: Uuid) {
    tokio::spawn(async move {
        loop {
            let next = match db::next_accepted_submitted_input(&state.pool, session_id).await {
                Ok(Some(input)) => input,
                Ok(None) => break,
                Err(error) => {
                    let _ = db::append_event(&state.pool, session_id, None, "submitted_input", None, "submitted_input.drainFailed", Some("failed"), json!({"error": error.to_string()})).await;
                    break;
                }
            };
            if let Err(error) = db::ensure_session_not_archived(&state.pool, session_id).await {
                let abandoned = db::abandon_unapplied_submitted_inputs(&state.pool, session_id, "terminal lifecycle before submitted input drain").await.unwrap_or(0);
                let _ = db::append_event(&state.pool, session_id, None, "submitted_input", Some(next.id), "submitted_input.abandoned", Some("abandoned"), json!({"count": abandoned, "reason": error.to_string()})).await;
                break;
            }
            if next.disposition == "active_turn_steering"
                && next.target_turn_id.is_some()
                && let Some(model) = state.model_client.clone()
            {
                match runtime::continue_pending_steering_after_operation_boundary(
                    &state.pool,
                    session_id,
                    next.target_turn_id.expect("checked target turn"),
                    model.as_ref(),
                )
                .await
                {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(error) => {
                        let _ = db::mark_submitted_input_failed(&state.pool, next.id, &error.to_string()).await;
                        let _ = db::append_event(&state.pool, session_id, None, "submitted_input", Some(next.id), "submitted_input.applyFailed", Some("failed"), json!({"submittedInputId": next.id, "error": error.to_string()})).await;
                        break;
                    }
                }
            }
            if let Err(error) = dispatch_one_submitted_input(&state.pool, session_id, next.clone(), state.model_client.clone()).await {
                let _ = db::mark_submitted_input_failed(&state.pool, next.id, &error.to_string()).await;
                let _ = db::append_event(&state.pool, session_id, None, "submitted_input", Some(next.id), "submitted_input.applyFailed", Some("failed"), json!({"submittedInputId": next.id, "error": error.to_string()})).await;
                break;
            }
        }
        state.active_submit_drains.lock().await.remove(&session_id);
    });
}

fn reconcile_submit_worker_after_active_race(state: ServerState, session_id: Uuid) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        let has_accepted = match db::next_accepted_submitted_input(&state.pool, session_id).await {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(error) => {
                let _ = db::append_event(
                    &state.pool,
                    session_id,
                    None,
                    "submitted_input",
                    None,
                    "submitted_input.reconcileFailed",
                    Some("failed"),
                    json!({"error": error.to_string()}),
                )
                .await;
                false
            }
        };
        let has_running_turn = db::active_turn_id(&state.pool, session_id)
            .await
            .ok()
            .flatten()
            .is_some();
        if has_accepted && !has_running_turn {
            let should_spawn = {
                let mut active = state.active_submit_drains.lock().await;
                active.insert(session_id)
            };
            if should_spawn {
                spawn_submit_worker(state.clone(), session_id);
            }
        }
    });
}

impl ServerState {
    pub fn reconcile_submitted_inputs(&self) {
        let state = self.clone();
        tokio::spawn(async move {
            let sessions = match db::sessions_with_accepted_submitted_inputs(&state.pool).await {
                Ok(sessions) => sessions,
                Err(error) => {
                    eprintln!("[submitted-input-reconcile] failed to list accepted inputs: {error}");
                    return;
                }
            };
            for session_id in sessions {
                let should_spawn = {
                    let mut active = state.active_submit_drains.lock().await;
                    active.insert(session_id)
                };
                if should_spawn {
                    spawn_submit_worker(state.clone(), session_id);
                }
            }
        });
    }
}

async fn send_message(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
    payload: std::result::Result<Json<SendRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    submit_message_to_session(state, session_id, request.message, "gui", "unifiedSubmit").await
}

async fn send_requirements_reviewer_message(
    State(state): State<ServerState>,
    Path(source_session_id): Path<Uuid>,
    payload: std::result::Result<Json<SendRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let request = parse_json(payload)?;
    let reviewer_session_id = crate::requirements::active_reviewer_for_source(&state.pool, source_session_id)
        .await
        .map_err(ApiError::from_anyhow)?
        .ok_or_else(|| ApiError::conflict("Requirements Review has no active reviewer session"))?;
    let reviewer = db::session_record(&state.pool, reviewer_session_id)
        .await
        .map_err(ApiError::from_anyhow)?;
    if reviewer.parent_session_id != Some(source_session_id)
        || reviewer.session_kind != "requirementsReviewer"
        || !reviewer.hidden
    {
        return Err(ApiError::conflict("Requirements reviewer linkage is invalid"));
    }
    submit_message_to_session(
        state,
        reviewer_session_id,
        request.message,
        "gui",
        "requirementsReviewDetail",
    )
    .await
}

async fn submit_message_to_session(
    state: ServerState,
    session_id: Uuid,
    message: String,
    actor: &str,
    source: &str,
) -> Result<Json<Value>, ApiError> {
    if message.trim().is_empty() {
        return Err(ApiError::bad_request("message must not be empty"));
    }
    let compaction_active = state.active_compactions.lock().await.contains(&session_id);
    let already_active = {
        let mut active = state.active_submit_drains.lock().await;
        if compaction_active {
            true
        } else {
            !active.insert(session_id)
        }
    };
    let submitted = match db::record_accepted_submitted_input_atomic(
        &state.pool,
        session_id,
        compaction_active,
        already_active,
        actor,
        source,
        "user",
        &message,
    )
    .await
    {
        Ok(submitted) => submitted,
        Err(error) => {
            if !already_active {
                state.active_submit_drains.lock().await.remove(&session_id);
            }
            let observed = db::session_record(&state.pool, session_id)
                .await
                .map(|session| if session.archived_at.is_some() { "archived".to_string() } else { session.status })
                .unwrap_or_else(|_| "invalid".to_string());
            if observed != "invalid" {
                let _ = db::record_rejected_submitted_input(&state.pool, session_id, actor, source, "user", &message, &observed, &error.to_string()).await;
            }
            return Err(ApiError::from_anyhow(error));
        }
    };
    if !already_active {
        spawn_submit_worker(state.clone(), session_id);
    } else if !compaction_active {
        reconcile_submit_worker_after_active_race(state.clone(), session_id);
    }
    let mut response_turn_id = submitted.target_turn_id;
    if submitted.disposition == "idle_turn_start" {
        for _ in 0..100 {
            response_turn_id = sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM turns WHERE session_id=$1 AND input_text=$2 ORDER BY started_at DESC LIMIT 1",
            )
            .bind(session_id)
            .bind(&message)
            .fetch_optional(&state.pool)
            .await
            .map_err(anyhow::Error::from)
            .map_err(ApiError::from_anyhow)?;
            if response_turn_id.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    let response_status = if submitted.disposition == "idle_turn_start" {
        "running"
    } else {
        submitted.status.as_str()
    };
    Ok(Json(json!({
        "sessionId": session_id,
        "turnId": response_turn_id,
        "submittedInputId": submitted.id,
        "disposition": submitted.disposition,
        "status": response_status,
        "orderingKey": submitted.ordering_key,
        "lifecycle": submitted.observed_lifecycle_state
    })))
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

async fn archive_session(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    starlark_host::terminate_session_processes_for_archive(session_id);
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

async fn image_artifact_metadata(
    State(state): State<ServerState>,
    Path((session_id, image_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(starlark_host::image_artifact_metadata(&state.pool, session_id, image_id).await?))
}

async fn image_artifact_thumbnail(
    State(state): State<ServerState>,
    Path((session_id, image_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    let metadata = starlark_host::image_artifact_metadata(&state.pool, session_id, image_id).await?;
    let bytes = starlark_host::image_artifact_thumbnail(&state.pool, session_id, image_id).await?;
    let mime = metadata.get("mimeType").and_then(Value::as_str).unwrap_or("application/octet-stream").to_string();
    Ok(([(header::CONTENT_TYPE, mime)], bytes).into_response())
}

async fn image_artifact_full_json(
    State(state): State<ServerState>,
    Path((session_id, image_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, ApiError> {
    let row: Option<(String, Vec<u8>)> = sqlx::query_as(
        "SELECT mime_type, binary_content FROM starter_image_artifacts WHERE id=$1 AND session_id=$2"
    )
    .bind(image_id)
    .bind(session_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(anyhow::Error::from)?;
    let Some((content_type, bytes)) = row else {
        return Err(ApiError::not_found("image_artifact", image_id));
    };
    Ok(Json(json!({
        "path": format!("agent-runtime-image://{session_id}/{image_id}"),
        "bytesBase64": BASE64_STANDARD.encode(bytes),
        "contentType": content_type,
        "imageArtifactId": image_id,
        "sessionId": session_id,
    })))
}

async fn image_artifact_full(
    State(state): State<ServerState>,
    Path((session_id, image_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    let metadata = starlark_host::image_artifact_metadata(&state.pool, session_id, image_id).await?;
    let bytes = starlark_host::image_artifact_full(&state.pool, session_id, image_id).await?;
    let mime = metadata.get("mimeType").and_then(Value::as_str).unwrap_or("application/octet-stream").to_string();
    Ok(([(header::CONTENT_TYPE, mime)], bytes).into_response())
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

async fn role_editor_options(State(state): State<ServerState>) -> Result<Json<Value>, ApiError> {
    let recipients = routing::recipient_options(&state.pool).await?;
    Ok(Json(serde_json::to_value(crate::roles::editor_options(recipients)).map_err(anyhow::Error::from)?))
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
    use std::collections::{BTreeMap, BTreeSet};
    use crate::compaction;
    use crate::gui_backend::GuiBackendController;
    use crate::model::{ModelClient, ModelFinalTurn, ModelHistoryItem, ModelInitialTurn, ModelToolTurn, RuntimeInputMessage, ToolCallRequest};
    use crate::roles::RoleSnapshot;
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
        observed_messages: Arc<StdMutex<Vec<String>>>,
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
            self.observed_messages.lock().expect("messages lock").push(message.to_string());
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
                    arguments: self.tool_arguments.clone().unwrap_or_else(|| json!({"source": "print(\"fake-ok\")"})),
                },
                request_shape,
                raw_response: json!({"output":[{"type":"function_call","name":"execute_code","call_id":"fake-call","arguments":"{\"source\":\"print(\\\"fake-ok\\\")\"}"}]}),
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
        url: String,
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
        TestDb { pool, admin, name, url }
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

    async fn test_snapshot_without_external_model_lookup(State(state): State<ServerState>) -> Result<Json<Value>, ApiError> {
        let projection = projection::build_runtime_projection_snapshot(&state.pool, None).await?;
        Ok(Json(serde_json::to_value(projection).map_err(anyhow::Error::from)?))
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
        let snapshot = projection::build_runtime_projection_snapshot(&test_db.pool, Some(session_id)).await.expect("snapshot");
        assert!(snapshot.selected_chat_entries.iter().any(|entry| entry.author == "Runtime" && entry.display_label == "Runtime" && entry.status == "failed" && entry.kind == "system_error"));
        assert!(!snapshot.selected_chat_entries.iter().any(|entry| entry.author == "Assistant" && entry.body.contains("routing")));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn project_progenitor_seed_routes_to_owner_and_rejects_magic_recipients() {
        let test_db = validation_db().await;
        let progenitor = db::current_role_snapshot(&test_db.pool, "project-progenitor").await.expect("project progenitor role");
        assert_eq!(progenitor.routing.default_recipient.as_deref(), Some("owner"));
        assert_eq!(progenitor.routing.allowed_recipients, vec!["owner".to_string()]);
        let session_id = db::new_session(&test_db.pool, &progenitor, Some("ezra"), ".", Some("."), Some("Project Progenitor"), None).await.expect("session");
        let model = FakeModelClient::default();
        crate::runtime::send_with_model_client(&test_db.pool, session_id, "seeded progenitor startup", &model, compaction::CompactionBudget::default()).await.expect("send");

        for recipient in ["operator", "orchestrator"] {
            let mut invalid = progenitor.routing.clone();
            invalid.default_recipient = Some(recipient.to_string());
            invalid.allowed_recipients = vec![recipient.to_string()];
            let err = routing::validate_routing(&invalid, Some(&test_db.pool), &BTreeSet::new()).await.unwrap_err().to_string();
            assert!(err.contains(&format!("invalid routing recipient: {recipient}")));
        }

        let mut active_role_route = progenitor.routing.clone();
        active_role_route.default_recipient = Some("runtime-allow".to_string());
        active_role_route.allowed_recipients = vec!["runtime-allow".to_string(), "owner".to_string()];
        routing::validate_routing(&active_role_route, Some(&test_db.pool), &BTreeSet::new()).await.expect("active role recipient");
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

        crate::runtime::send_with_model_client(&test_db.pool, session_id, "what changed?", &model, compaction::CompactionBudget::default()).await.expect("second send");
        let second_shape = model.observed_request_shapes.lock().expect("request shapes").last().cloned().expect("second shape");
        let second_rendered = serde_json::to_string(&second_shape).expect("second shape json");
        assert!(!second_rendered.contains("<runtime_context"), "unchanged later turn must not reinsert full runtime context: {second_rendered}");
        assert!(!second_rendered.contains("<role_instructions"), "unchanged later turn must not reinsert role instructions: {second_rendered}");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn generated_context_filters_native_affordances_by_role_and_project_bundles() {
        let test_db = validation_db().await;
        db::create_project(&test_db.pool, "tool-filter", "Tool Filter", ".", ".", None, "gpt-5.4-mini").await.expect("project");
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let role_bundle_id = Uuid::new_v4();
        let project_bundle_id = Uuid::new_v4();
        sqlx::query("INSERT INTO starter_tool_bundle_versions (id, bundle_id, version, role_id, project_key, tools, source_metadata, active) VALUES ($1,'runtime-no-rg-default','1',$2,NULL,$3,$4,true)")
            .bind(role_bundle_id)
            .bind(&role.id)
            .bind(json!(["file.head", "tooling.request", "git.commit"]))
            .bind(json!({"source":"test-role-bundle"}))
            .execute(&test_db.pool)
            .await
            .expect("role bundle");
        sqlx::query("INSERT INTO starter_role_tool_bundle_bindings (id, role_id, project_key, bundle_version_id, active, audit_metadata) VALUES ($1,$2,NULL,$3,true,$4)")
            .bind(Uuid::new_v4())
            .bind(&role.id)
            .bind(role_bundle_id)
            .bind(json!({"source":"test-role-binding"}))
            .execute(&test_db.pool)
            .await
            .expect("role binding");
        sqlx::query("INSERT INTO starter_tool_bundle_versions (id, bundle_id, version, role_id, project_key, tools, source_metadata, active) VALUES ($1,'runtime-no-rg-project','1',$2,'tool-filter',$3,$4,true)")
            .bind(project_bundle_id)
            .bind(&role.id)
            .bind(json!(["tooling.request", "server.start"]))
            .bind(json!({"source":"test-project-bundle"}))
            .execute(&test_db.pool)
            .await
            .expect("project bundle");
        sqlx::query("INSERT INTO starter_role_tool_bundle_bindings (id, role_id, project_key, bundle_version_id, active, audit_metadata) VALUES ($1,$2,'tool-filter',$3,true,$4)")
            .bind(Uuid::new_v4())
            .bind(&role.id)
            .bind(project_bundle_id)
            .bind(json!({"source":"test-project-binding"}))
            .execute(&test_db.pool)
            .await
            .expect("project binding");

        let session_id = db::new_session(&test_db.pool, &role, Some("tool-filter"), "/tmp/tool-filter", Some("/tmp/tool-filter"), None, None).await.expect("session");
        let stored_bundle: Value = sqlx::query_scalar("SELECT active_tool_bundle_version_ids FROM sessions WHERE id=$1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("session bundle snapshot");
        assert_eq!(stored_bundle["tools"], json!(["tooling.request"]));

        let model = FakeModelClient { direct_final_text: Some("ok"), ..Default::default() };
        let turn_id = crate::runtime::send_with_model_client(&test_db.pool, session_id, "show tools", &model, compaction::CompactionBudget::default()).await.expect("send");
        let snapshot: Value = sqlx::query_scalar("SELECT snapshot FROM session_context_snapshots WHERE session_id=$1 AND turn_id=$2 ORDER BY context_epoch DESC LIMIT 1")
            .bind(session_id)
            .bind(turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("context snapshot");
        assert_eq!(snapshot.pointer("/tools/nativeAffordances"), Some(&json!(["tooling.request"])));
        assert_eq!(snapshot.pointer("/tools/nativeAffordanceCount").and_then(Value::as_i64), Some(1));
        let shape = model.observed_request_shapes.lock().expect("request shapes").first().cloned().expect("shape");
        assert!(serde_json::to_string(&shape).expect("shape text").contains("native_affordance_count"));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn tool_bundle_definition_binding_activation_and_audit_are_persisted() {
        let test_db = validation_db().await;
        db::create_project(&test_db.pool, "bundle-audit", "Bundle Audit", ".", ".", None, "gpt-5.4-mini").await.expect("project");
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("bundle-audit"), "/tmp/bundle-audit", Some("/tmp/bundle-audit"), None, None).await.expect("session");
        let (role_bundle_version, role_binding) = crate::roles::activate_tool_bundle_binding(
            &test_db.pool,
            Some(session_id),
            &role.id,
            None,
            "runtime-no-rg-role",
            "v1",
            vec!["file.head".to_string(), "tooling.request".to_string()],
            json!({"source":"unit-test","scope":"role"}),
            json!({"activatedBy":"operator-test","reason":"role bundle audit"}),
        ).await.expect("role bundle activation");
        let (project_bundle_version, project_binding) = crate::roles::activate_tool_bundle_binding(
            &test_db.pool,
            Some(session_id),
            &role.id,
            Some("bundle-audit"),
            "runtime-no-rg-project",
            "v1",
            vec!["tooling.request".to_string()],
            json!({"source":"unit-test","scope":"project"}),
            json!({"activatedBy":"operator-test","reason":"project bundle audit"}),
        ).await.expect("project bundle activation");
        assert_ne!(role_bundle_version, project_bundle_version);
        assert_ne!(role_binding, project_binding);
        let visible = crate::roles::visible_tool_bundle_for_role(&test_db.pool, &role.id, Some("bundle-audit")).await.expect("visible tools");
        assert_eq!(visible, vec!["tooling.request".to_string()]);
        let snapshot_session = db::new_session(&test_db.pool, &role, Some("bundle-audit"), "/tmp/bundle-audit", Some("/tmp/bundle-audit"), None, None).await.expect("snapshot session");
        let stored_bundle: Value = sqlx::query_scalar("SELECT active_tool_bundle_version_ids FROM sessions WHERE id=$1")
            .bind(snapshot_session)
            .fetch_one(&test_db.pool)
            .await
            .expect("stored bundle");
        assert_eq!(stored_bundle["tools"], json!(["tooling.request"]));
        let persisted: (Value, Value, bool) = sqlx::query_as("SELECT source_metadata, tools, active FROM starter_tool_bundle_versions WHERE id=$1")
            .bind(project_bundle_version)
            .fetch_one(&test_db.pool)
            .await
            .expect("project bundle persisted");
        assert_eq!(persisted.0["scope"], "project");
        assert_eq!(persisted.1, json!(["tooling.request"]));
        assert!(persisted.2);
        let binding_audit: Value = sqlx::query_scalar("SELECT audit_metadata FROM starter_role_tool_bundle_bindings WHERE id=$1 AND active=true")
            .bind(project_binding)
            .fetch_one(&test_db.pool)
            .await
            .expect("binding audit");
        assert_eq!(binding_audit["activatedBy"], "operator-test");
        let audit_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='tool_bundle.binding_activated' AND status='active'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("audit events");
        assert_eq!(audit_events, 2);
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
        assert!(!rendered.contains("<runtime_context"), "role transition must be concise and not dump full runtime context: {rendered}");
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
    async fn god_mode_grant_and_revoke_emit_context_deltas_without_later_full_reinsertion() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("context-god-mode"), ".", Some("."), None, None).await.expect("session");
        let model = FakeModelClient { direct_final_text: Some("ok"), ..Default::default() };
        crate::runtime::send_with_model_client(&test_db.pool, session_id, "first", &model, compaction::CompactionBudget::default()).await.expect("first send");

        crate::god_mode::grant_session(&test_db.pool, session_id, "test", "prove concise context delta", None).await.expect("grant");
        crate::runtime::send_with_model_client(&test_db.pool, session_id, "after grant", &model, compaction::CompactionBudget::default()).await.expect("grant send");
        let grant_shape = model.observed_request_shapes.lock().expect("request shapes").last().cloned().expect("grant shape");
        let grant_rendered = serde_json::to_string(&grant_shape).expect("grant shape json");
        assert!(grant_rendered.contains("<context_delta"), "{grant_rendered}");
        assert!(grant_rendered.contains("god_mode_changed"), "{grant_rendered}");
        assert!(!grant_rendered.contains("<runtime_context"), "{grant_rendered}");

        crate::runtime::send_with_model_client(&test_db.pool, session_id, "unchanged", &model, compaction::CompactionBudget::default()).await.expect("unchanged send");
        let unchanged_shape = model.observed_request_shapes.lock().expect("request shapes").last().cloned().expect("unchanged shape");
        let unchanged_rendered = serde_json::to_string(&unchanged_shape).expect("unchanged shape json");
        assert!(!unchanged_rendered.contains("<runtime_context"), "{unchanged_rendered}");
        assert!(!unchanged_rendered.contains("god_mode_changed"), "{unchanged_rendered}");

        crate::god_mode::revoke_active(&test_db.pool, session_id, "test", "prove revoke delta").await.expect("revoke");
        crate::runtime::send_with_model_client(&test_db.pool, session_id, "after revoke", &model, compaction::CompactionBudget::default()).await.expect("revoke send");
        let revoke_shape = model.observed_request_shapes.lock().expect("request shapes").last().cloned().expect("revoke shape");
        let revoke_rendered = serde_json::to_string(&revoke_shape).expect("revoke shape json");
        assert!(revoke_rendered.contains("god_mode_changed"), "{revoke_rendered}");
        assert!(!revoke_rendered.contains("<runtime_context"), "{revoke_rendered}");
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
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status, started_at) VALUES ($1,$2,'print(1)','running',now() - interval '10 minutes')")
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
    async fn database_rejects_more_than_one_running_turn_per_session() {
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

        state.active_submit_drains.lock().await.insert(session_id);
        let (status, submitted) = request_json(
            router.clone(),
            Method::POST,
            &format!("/sessions/{session_id}/send"),
            json!({"message":"steering while active"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(submitted["disposition"], "queued_next_turn_after_final_output");
        assert!(submitted["submittedInputId"].as_str().is_some());
        let queued_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='accepted'")
            .bind(session_id)
            .fetch_one(&state.pool)
            .await
            .expect("queued submitted input count");
        assert_eq!(queued_count, 1);
        state.active_submit_drains.lock().await.remove(&session_id);

        let (status, archived) = request_json(
            router.clone(),
            Method::POST,
            &format!("/sessions/{session_id}/archive"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(archived["tracked"], false);

        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn unified_submit_accepts_active_steering_and_drains_without_parallel_turns() {
        let test_db = validation_db().await;
        let model = Arc::new(FakeModelClient {
            direct_final_text: Some("done"),
            request_delay_ms: Some(80),
            ..Default::default()
        });
        let state = ServerState::new_with_model_client(test_db.pool.clone(), "unified-submit-test".to_string(), model);
        let router = app(state.clone());
        let (_, created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"steering-proof","model":"fake-model","workdir":".","worktreeRoot":".","title":"Steering proof","name":"steering-proof"}),
        )
        .await;
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");

        let (status, first) = request_json(
            router.clone(),
            Method::POST,
            &format!("/sessions/{session_id}/send"),
            json!({"message":"first message"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(first["disposition"], "idle_turn_start");

        let (status, second) = request_json(
            router.clone(),
            Method::POST,
            &format!("/sessions/{session_id}/send"),
            json!({"message":"second while active"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_ne!(second["disposition"], "idle_turn_start");
        assert!(second["submittedInputId"].as_str().is_some());
        let second_submitted_id: Uuid = serde_json::from_value(second["submittedInputId"].clone()).expect("submitted input id");
        let second_row = sqlx::query(
            r#"
            SELECT session_id, actor, source, role, content, disposition, status,
                   ordering_key, observed_lifecycle_state, placement_turn_id,
                   accepted_at, applied_at, failure_metadata
            FROM submitted_inputs WHERE id=$1
            "#,
        )
        .bind(second_submitted_id)
        .fetch_one(&state.pool)
        .await
        .expect("submitted input row");
        assert_eq!(second_row.get::<Uuid, _>("session_id"), session_id);
        assert_eq!(second_row.get::<String, _>("actor"), "gui");
        assert_eq!(second_row.get::<String, _>("source"), "unifiedSubmit");
        assert_eq!(second_row.get::<String, _>("role"), "user");
        assert_eq!(second_row.get::<String, _>("content"), "second while active");
        assert_ne!(second_row.get::<String, _>("disposition"), "idle_turn_start");
        assert_eq!(second_row.get::<String, _>("status"), "accepted");
        assert!(second_row.get::<i64, _>("ordering_key") > 0);
        assert_eq!(second_row.get::<String, _>("observed_lifecycle_state"), "stopped");
        assert!(second_row.get::<Option<Uuid>, _>("placement_turn_id").is_none());
        assert!(second_row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("accepted_at").is_some());
        assert!(second_row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("applied_at").is_none());
        assert!(second_row.get::<Value, _>("failure_metadata").as_object().is_some());

        for _ in 0..80 {
            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='applied'")
                .bind(session_id)
                .fetch_one(&state.pool)
                .await
                .expect("applied count");
            if applied == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='applied'")
            .bind(session_id)
            .fetch_one(&state.pool)
            .await
            .expect("applied final count");
        assert_eq!(applied, 2);
        let running: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status='running'")
            .bind(session_id)
            .fetch_one(&state.pool)
            .await
            .expect("running count");
        assert_eq!(running, 0);
        let turn_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1")
            .bind(session_id)
            .fetch_one(&state.pool)
            .await
            .expect("turn count");
        assert_eq!(turn_count, 1);
        let projection = projection::build_runtime_projection_snapshot(&state.pool, Some(session_id)).await.expect("projection");
        let selected = projection.selected_session.expect("selected session");
        assert_eq!(selected.queued_submitted_input_count, 0);
        assert_eq!(selected.applied_steering_count, 2);
        assert_eq!(selected.submit_status.as_deref(), Some("applied"));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn simultaneous_unified_submits_are_atomically_ordered_and_do_not_start_parallel_turns() {
        let test_db = validation_db().await;
        let model = Arc::new(FakeModelClient {
            direct_final_text: Some("ordered"),
            request_delay_ms: Some(100),
            ..Default::default()
        });
        let state = ServerState::new_with_model_client(test_db.pool.clone(), "atomic-submit-order".to_string(), model);
        let router = app(state.clone());
        let (_, created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"atomic-submit","model":"fake-model","workdir":".","worktreeRoot":".","title":"Atomic submit","name":"atomic-submit"}),
        )
        .await;
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");

        let send_path = format!("/sessions/{session_id}/send");
        let first = request_json(router.clone(), Method::POST, &send_path, json!({"message":"first simultaneous"}));
        let second = request_json(router.clone(), Method::POST, &send_path, json!({"message":"second simultaneous"}));
        let third = request_json(router.clone(), Method::POST, &send_path, json!({"message":"third simultaneous"}));
        let ((first_status, first_body), (second_status, second_body), (third_status, third_body)) = tokio::join!(first, second, third);
        for (status, body) in [(first_status, first_body), (second_status, second_body), (third_status, third_body)] {
            assert_eq!(status, StatusCode::OK, "submit must be accepted: {body}");
            assert!(body["submittedInputId"].as_str().is_some());
            assert_ne!(body["disposition"], "rejected_terminal");
        }

        for _ in 0..120 {
            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='applied'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("applied count");
            if applied == 3 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let rows = sqlx::query("SELECT content, ordering_key, status, placement_turn_id FROM submitted_inputs WHERE session_id=$1 ORDER BY ordering_key ASC")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("submitted rows");
        assert_eq!(rows.len(), 3);
        let mut previous_ordering = 0_i64;
        for row in &rows {
            let ordering = row.get::<i64, _>("ordering_key");
            assert!(ordering > previous_ordering, "ordering keys must be durable and strictly monotonic");
            previous_ordering = ordering;
            assert_eq!(row.get::<String, _>("status"), "applied");
            assert!(row.get::<Option<Uuid>, _>("placement_turn_id").is_some());
        }
        let turn_rows = sqlx::query("SELECT input_text, started_at FROM turns WHERE session_id=$1 ORDER BY started_at ASC, id ASC")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("turn rows");
        assert_eq!(turn_rows.len(), 3);
        let turn_inputs = turn_rows.iter().map(|row| row.get::<String, _>("input_text")).collect::<Vec<_>>();
        let ordered_inputs = rows.iter().map(|row| row.get::<String, _>("content")).collect::<Vec<_>>();
        assert_eq!(turn_inputs, ordered_inputs, "runtime placement must follow durable submitted-input ordering");
        let running: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status='running'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("running turns");
        assert_eq!(running, 0);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn unified_submit_rejects_terminal_sessions_before_turn_creation() {
        let test_db = validation_db().await;
        let state = ServerState::new(test_db.pool.clone());
        let router = app(state.clone());
        let (_, created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"terminal-proof","model":"fake-model","workdir":".","worktreeRoot":".","title":"Terminal proof","name":"terminal-proof"}),
        )
        .await;
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");
        let (status, _) = request_json(router.clone(), Method::POST, &format!("/sessions/{session_id}/archive"), json!({})).await;
        assert_eq!(status, StatusCode::OK);
        let (status, error) = request_json(
            router.clone(),
            Method::POST,
            &format!("/sessions/{session_id}/send"),
            json!({"message":"must reject"}),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_api_error(&error, "conflict");
        let turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1")
            .bind(session_id)
            .fetch_one(&state.pool)
            .await
            .expect("turn count");
        assert_eq!(turns, 0);
        let rejected: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='rejected'")
            .bind(session_id)
            .fetch_one(&state.pool)
            .await
            .expect("rejected count");
        assert_eq!(rejected, 1);
        let rejected_row = sqlx::query("SELECT disposition, observed_lifecycle_state, failure_metadata FROM submitted_inputs WHERE session_id=$1 AND status='rejected'")
            .bind(session_id)
            .fetch_one(&state.pool)
            .await
            .expect("rejected row");
        assert_eq!(rejected_row.get::<String, _>("disposition"), "rejected_terminal");
        assert_eq!(rejected_row.get::<String, _>("observed_lifecycle_state"), "archived");
        assert!(rejected_row.get::<Value, _>("failure_metadata")["reason"].as_str().unwrap_or_default().contains("archived"));

        let (_, created_archived) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"terminal-proof","model":"fake-model","workdir":".","worktreeRoot":".","title":"Archive proof","name":"archive-proof"}),
        )
        .await;
        let archived_session_id: Uuid = serde_json::from_value(created_archived["sessionId"].clone()).expect("archived session id");
        let (status, _) = request_json(router.clone(), Method::POST, &format!("/sessions/{archived_session_id}/archive"), json!({})).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = request_json(router, Method::POST, &format!("/sessions/{archived_session_id}/send"), json!({"message":"archived reject"})).await;
        assert_eq!(status, StatusCode::CONFLICT);
        let archived_turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1")
            .bind(archived_session_id)
            .fetch_one(&state.pool)
            .await
            .expect("archived turns");
        assert_eq!(archived_turns, 0);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn accepted_unapplied_submitted_inputs_reconcile_after_server_restart() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("restart steering"), ".", Some("."), None, None).await.expect("session");
        let session = db::session_record(&test_db.pool, session_id).await.expect("session record");
        let submitted = db::record_accepted_submitted_input(
            &test_db.pool,
            &session,
            None,
            "gui",
            "unifiedSubmit",
            "user",
            "survive restart",
            "idle_turn_start",
        )
        .await
        .expect("accepted input");

        let model = Arc::new(FakeModelClient { direct_final_text: Some("after restart"), ..Default::default() });
        let state_after_restart = ServerState::new_with_model_client(test_db.pool.clone(), "restart-reconcile".to_string(), model);
        let _router = app(state_after_restart.clone());
        for _ in 0..80 {
            let status: String = sqlx::query_scalar("SELECT status FROM submitted_inputs WHERE id=$1")
                .bind(submitted.id)
                .fetch_one(&state_after_restart.pool)
                .await
                .expect("submitted status");
            if status == "applied" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let row = sqlx::query("SELECT status, placement_turn_id FROM submitted_inputs WHERE id=$1")
            .bind(submitted.id)
            .fetch_one(&state_after_restart.pool)
            .await
            .expect("submitted final");
        assert_eq!(row.get::<String, _>("status"), "applied");
        assert!(row.get::<Option<Uuid>, _>("placement_turn_id").is_some());
        let turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND input_text='survive restart'")
            .bind(session_id)
            .fetch_one(&state_after_restart.pool)
            .await
            .expect("turn count");
        assert_eq!(turns, 1);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn terminal_lifecycle_abandons_unapplied_submitted_inputs_and_prevents_later_drain() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("abandon steering"), ".", Some("."), None, None).await.expect("session");
        let session = db::session_record(&test_db.pool, session_id).await.expect("session record");
        let first = db::record_accepted_submitted_input(&test_db.pool, &session, None, "gui", "unifiedSubmit", "user", "abandon one", "queued_next_turn_after_final_output").await.expect("accepted one");
        let second = db::record_accepted_submitted_input(&test_db.pool, &session, None, "gui", "unifiedSubmit", "user", "abandon two", "queued_next_turn_after_final_output").await.expect("accepted two");
        db::archive_session(&test_db.pool, session_id).await.expect("archive");

        let statuses = sqlx::query("SELECT id, status, failure_metadata FROM submitted_inputs WHERE id = ANY($1) ORDER BY ordering_key")
            .bind(vec![first.id, second.id])
            .fetch_all(&test_db.pool)
            .await
            .expect("submitted statuses");
        assert_eq!(statuses.len(), 2);
        for row in statuses {
            assert_eq!(row.get::<String, _>("status"), "abandoned");
            assert_eq!(row.get::<Value, _>("failure_metadata")["reason"], "session archived");
        }

        let model = Arc::new(FakeModelClient { direct_final_text: Some("must not run"), ..Default::default() });
        let state_after_archive = ServerState::new_with_model_client(test_db.pool.clone(), "abandoned-reconcile".to_string(), model);
        let _router = app(state_after_archive.clone());
        tokio::time::sleep(Duration::from_millis(100)).await;
        let turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("turn count");
        assert_eq!(turns, 0, "archived-session accepted inputs must not start later work");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn submit_races_with_archive_leave_no_live_queue_or_late_work() {
        let test_db = validation_db().await;
        let state = ServerState::new(test_db.pool.clone());
        let router = app(state.clone());
        let (_, archived_created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"submit-race","model":"fake-model","workdir":".","worktreeRoot":".","title":"Archive race","name":"archive-race"}),
        ).await;
        let archive_session_id: Uuid = serde_json::from_value(archived_created["sessionId"].clone()).expect("archive session");
        state.active_compactions.lock().await.insert(archive_session_id);
        let archive_send_path = format!("/sessions/{archive_session_id}/send");
        let archive_path = format!("/sessions/{archive_session_id}/archive");
        let (send_result, archive_result) = tokio::join!(
            request_json(router.clone(), Method::POST, &archive_send_path, json!({"message":"archive race steering"})),
            request_json(router.clone(), Method::POST, &archive_path, json!({})),
        );
        assert!(matches!(send_result.0, StatusCode::OK | StatusCode::CONFLICT), "submit result must be typed accept or terminal reject: {:?}", send_result);
        assert_eq!(archive_result.0, StatusCode::OK);
        state.active_compactions.lock().await.remove(&archive_session_id);
        let archive_accepted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='accepted'")
            .bind(archive_session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("archive accepted");
        assert_eq!(archive_accepted, 0, "archive race must leave no live accepted queue");
        let archive_late_turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND input_text='archive race steering'")
            .bind(archive_session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("archive late turns");
        assert_eq!(archive_late_turns, 0, "archive race must not start late work");
        let archive_terminal_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status IN ('abandoned','rejected')")
            .bind(archive_session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("archive terminal rows");
        assert!(archive_terminal_rows >= 1, "accepted input must be abandoned or terminal submit rejected");

        let (_, archived_created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"submit-race","model":"fake-model","workdir":".","worktreeRoot":".","title":"Archive race","name":"archive-race"}),
        ).await;
        let archive_session_id: Uuid = serde_json::from_value(archived_created["sessionId"].clone()).expect("archive session");
        state.active_compactions.lock().await.insert(archive_session_id);
        let archive_send_path = format!("/sessions/{archive_session_id}/send");
        let archive_path = format!("/sessions/{archive_session_id}/archive");
        let (send_result, archive_result) = tokio::join!(
            request_json(router.clone(), Method::POST, &archive_send_path, json!({"message":"archive race steering"})),
            request_json(router, Method::POST, &archive_path, json!({})),
        );
        assert!(matches!(send_result.0, StatusCode::OK | StatusCode::CONFLICT), "submit result must be typed accept or terminal reject: {:?}", send_result);
        assert_eq!(archive_result.0, StatusCode::OK);
        state.active_compactions.lock().await.remove(&archive_session_id);
        let archive_accepted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='accepted'")
            .bind(archive_session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("archive accepted");
        assert_eq!(archive_accepted, 0, "archive race must leave no live accepted queue");
        let archive_late_turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND input_text='archive race steering'")
            .bind(archive_session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("archive late turns");
        assert_eq!(archive_late_turns, 0, "archive race must not start late work");
        let archive_terminal_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status IN ('abandoned','rejected')")
            .bind(archive_session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("archive terminal rows");
        assert!(archive_terminal_rows >= 1, "accepted input must be abandoned or terminal submit rejected");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn steering_preserves_bounded_artifacts_process_compaction_and_workflow_memory_rows() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("preserve-bounded"), ".", Some("."), None, None).await.expect("session");
        let completed_turn = Uuid::new_v4();
        let tool_call_id = Uuid::new_v4();
        let script_run_id = Uuid::new_v4();
        let process_id = Uuid::new_v4();
        let artifact_id = Uuid::new_v4();
        let checkpoint_id = Uuid::new_v4();
        let memory_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at, completed_at) VALUES ($1,$2,'user','prior artifact turn','completed',now() - interval '5 minutes',now() - interval '4 minutes')")
            .bind(completed_turn).bind(session_id).execute(&test_db.pool).await.expect("turn");
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status, result, started_at, completed_at) VALUES ($1,$2,$3,'execute_code','preserve-call',$4,'completed',$5,now() - interval '5 minutes',now() - interval '4 minutes')")
            .bind(tool_call_id).bind(session_id).bind(completed_turn).bind(json!({"source":"print('preserve')"})).bind(json!({"ok":true})).execute(&test_db.pool).await.expect("tool");
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status, final_output, stdout, stderr, completed_at) VALUES ($1,$2,'print(\"preserve\")','completed','bounded preview','full stdout preserved','',now() - interval '4 minutes')")
            .bind(script_run_id).bind(tool_call_id).execute(&test_db.pool).await.expect("script");
        sqlx::query("INSERT INTO managed_processes (id, handle, session_id, starting_turn_id, binary_name, argv, cwd, status, end_of_turn_behavior, end_of_session_behavior, metadata, start_time, end_time) VALUES ($1,'preserve-process',$2,$3,'sleep','[\"1\"]'::jsonb,'.','completed','terminate','terminate',$4,now() - interval '5 minutes',now() - interval '4 minutes')")
            .bind(process_id).bind(session_id).bind(completed_turn).bind(json!({"beforeSteering":true})).execute(&test_db.pool).await.expect("process");
        sqlx::query("INSERT INTO execution_output_artifacts (id, session_id, turn_id, tool_call_id, script_run_id, process_id, source_type, stream, content, byte_count, line_count, metadata) VALUES ($1,$2,$3,$4,$5,$6,'script','stdout',$7,21,2,$8)")
            .bind(artifact_id).bind(session_id).bind(completed_turn).bind(tool_call_id).bind(script_run_id).bind(process_id).bind("full stdout preserved").bind(json!({"truncated":false,"artifactHandle":"preserve"})).execute(&test_db.pool).await.expect("artifact");
        sqlx::query("INSERT INTO compaction_checkpoints (id, session_id, status, compacted_through_turn_id, replacement_context, summary, completed_at) VALUES ($1,$2,'completed',$3,'compacted context before steering',$4,now() - interval '3 minutes')")
            .bind(checkpoint_id).bind(session_id).bind(completed_turn).bind(json!({"summary":"preserve compaction"})).execute(&test_db.pool).await.expect("checkpoint");
        let vector = format!("[{}]", vec!["0"; workflow_memory::DEFAULT_DIMENSIONS].join(","));
        sqlx::query(
            r#"
            INSERT INTO workflow_memories (
                id, script_run_id, session_id, scope_type, project_key, title, reason, summary,
                provider, model, dimensions, source_hash, command_fingerprint, embedding
            ) VALUES ($1,$2,$3,'project','preserve-bounded','Preserve memory','test','workflow memory before steering','deterministic','test',$4,'preserve-hash','preserve-command',$5::halfvec)
            "#,
        )
        .bind(memory_id).bind(script_run_id).bind(session_id).bind(workflow_memory::DEFAULT_DIMENSIONS as i32).bind(vector).execute(&test_db.pool).await.expect("memory");
        workflow_memory::insert_memory_event(&test_db.pool, session_id, Some(completed_turn), Some(script_run_id), Some(memory_id), "workflow_memory.promoted", json!({"beforeSteering":true})).await.expect("memory event");

        let before_artifact: (String, i64, i64, Value) = sqlx::query_as("SELECT content, byte_count, line_count, metadata FROM execution_output_artifacts WHERE id=$1")
            .bind(artifact_id).fetch_one(&test_db.pool).await.expect("before artifact");
        let before_process: (String, Value) = sqlx::query_as("SELECT status, metadata FROM managed_processes WHERE id=$1")
            .bind(process_id).fetch_one(&test_db.pool).await.expect("before process");
        let before_checkpoint: (String, String) = sqlx::query_as("SELECT status, replacement_context FROM compaction_checkpoints WHERE id=$1")
            .bind(checkpoint_id).fetch_one(&test_db.pool).await.expect("before checkpoint");
        let before_memory_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_memory_events WHERE memory_id=$1")
            .bind(memory_id).fetch_one(&test_db.pool).await.expect("before memory events");

        let model = Arc::new(FakeModelClient { direct_final_text: Some("after preservation steering"), ..Default::default() });
        let state = ServerState::new_with_model_client(test_db.pool.clone(), "preserve-bounded".to_string(), model);
        let router = app(state.clone());
        state.active_submit_drains.lock().await.insert(session_id);
        let (status, submitted) = request_json(router, Method::POST, &format!("/sessions/{session_id}/send"), json!({"message":"preserve bounded surfaces"})).await;
        assert_eq!(status, StatusCode::OK);
        assert_ne!(submitted["disposition"], "idle_turn_start");
        state.active_submit_drains.lock().await.remove(&session_id);
        {
            let mut active = state.active_submit_drains.lock().await;
            active.remove(&session_id);
            active.insert(session_id);
        }
        spawn_submit_worker(state.clone(), session_id);
        for _ in 0..80 {
            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='applied'")
                .bind(session_id).fetch_one(&test_db.pool).await.expect("applied");
            if applied == 1 { break; }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert_eq!(
            before_artifact,
            sqlx::query_as("SELECT content, byte_count, line_count, metadata FROM execution_output_artifacts WHERE id=$1")
                .bind(artifact_id).fetch_one(&test_db.pool).await.expect("after artifact"),
            "steering must not rewrite or truncate existing output artifacts"
        );
        assert_eq!(
            before_process,
            sqlx::query_as("SELECT status, metadata FROM managed_processes WHERE id=$1")
                .bind(process_id).fetch_one(&test_db.pool).await.expect("after process"),
            "steering must not mutate existing managed-process rows"
        );
        assert_eq!(
            before_checkpoint,
            sqlx::query_as("SELECT status, replacement_context FROM compaction_checkpoints WHERE id=$1")
                .bind(checkpoint_id).fetch_one(&test_db.pool).await.expect("after checkpoint"),
            "steering must not rewrite existing compaction checkpoints"
        );
        let after_memory_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_memory_events WHERE memory_id=$1")
            .bind(memory_id).fetch_one(&test_db.pool).await.expect("after memory events");
        assert_eq!(before_memory_events, after_memory_events, "steering must not fabricate workflow-memory feedback/events");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn steering_during_active_registry_and_god_mode_shell_waits_and_preserves_existing_work() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("active-work-preserve"), ".", Some("."), None, None).await.expect("session");
        let active_turn = Uuid::new_v4();
        let tool_call_id = Uuid::new_v4();
        let script_run_id = Uuid::new_v4();
        let host_api_id = Uuid::new_v4();
        let command_run_id = Uuid::new_v4();
        let grant_id = Uuid::new_v4();
        let shell_process_id = Uuid::new_v4();
        let shell_run_id = Uuid::new_v4();
        let command_stdout_artifact = Uuid::new_v4();
        let command_stderr_artifact = Uuid::new_v4();
        let shell_stdout_artifact = Uuid::new_v4();
        let shell_stderr_artifact = Uuid::new_v4();
        let large_stdout_sentinel = format!("HIDDEN_STDOUT_SENTINEL_{}", "stdout-body-".repeat(600));
        let large_stderr_sentinel = format!("HIDDEN_STDERR_SENTINEL_{}", "stderr-body-".repeat(600));
        let shell_stdout_sentinel = format!("HIDDEN_SHELL_STDOUT_SENTINEL_{}", "shell-out-".repeat(300));
        let shell_stderr_sentinel = format!("HIDDEN_SHELL_STDERR_SENTINEL_{}", "shell-err-".repeat(300));
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user','active registry and shell work','running',now())")
            .bind(active_turn).bind(session_id).execute(&test_db.pool).await.expect("active turn");
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status, started_at) VALUES ($1,$2,$3,'execute_code','active-work',$4,'running',now())")
            .bind(tool_call_id).bind(session_id).bind(active_turn).bind(json!({"source":"cmd.run(); shell('sleep 1').async()"})).execute(&test_db.pool).await.expect("tool");
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status, stdout, stderr, started_at) VALUES ($1,$2,'active work','running','','',now())")
            .bind(script_run_id).bind(tool_call_id).execute(&test_db.pool).await.expect("script");
        sqlx::query("INSERT INTO host_api_calls (id, script_run_id, api_name, input, status, started_at) VALUES ($1,$2,'cmd.run',$3,'running',now())")
            .bind(host_api_id).bind(script_run_id).bind(json!({"actionId":"cmd.echo"})).execute(&test_db.pool).await.expect("host api");
        sqlx::query("INSERT INTO command_runs (id, host_api_call_id, binary_name, argv, cwd, stdout, stderr, status, started_at, policy_decision, truncation) VALUES ($1,$2,'echo','[\"registry-output\"]'::jsonb,'.',$3,$4,'running',now(),$5,$6)")
            .bind(command_run_id).bind(host_api_id).bind(&large_stdout_sentinel).bind(&large_stderr_sentinel).bind(json!({"decision":"allow","source":"test policy"})).bind(json!({"stdoutTruncated":false})).execute(&test_db.pool).await.expect("command run");
        sqlx::query("INSERT INTO god_mode_grants (id, session_id, granted_by, granted_by_kind, reason, status, metadata) VALUES ($1,$2,'operator','operator','active shell test','active',$3)")
            .bind(grant_id).bind(session_id).bind(json!({"beforeSteering":true})).execute(&test_db.pool).await.expect("grant");
        sqlx::query("INSERT INTO managed_processes (id, handle, session_id, starting_turn_id, binary_name, argv, cwd, status, end_of_turn_behavior, end_of_session_behavior, metadata, start_time) VALUES ($1,'god-shell-active',$2,$3,'/bin/zsh','[\"-lc\",\"sleep 1\"]'::jsonb,'.','running','continue','terminate',$4,now())")
            .bind(shell_process_id).bind(session_id).bind(active_turn).bind(json!({"godModeShell":true})).execute(&test_db.pool).await.expect("shell process");
        sqlx::query("INSERT INTO shell_runs (id, script_run_id, session_id, turn_id, tool_call_id, god_mode_grant_id, invocation_mode, shell_path, script_hash, script_source, cwd, status, process_id, metadata) VALUES ($1,$2,$3,$4,$5,$6,'async','/bin/zsh','hash','sleep 1','.', 'running',$7,$8)")
            .bind(shell_run_id).bind(script_run_id).bind(session_id).bind(active_turn).bind(tool_call_id).bind(grant_id).bind(shell_process_id).bind(json!({"beforeSteering":true})).execute(&test_db.pool).await.expect("shell run");

        let model = Arc::new(FakeModelClient { direct_final_text: Some("after active work"), ..Default::default() });
        let state = ServerState::new_with_model_client(
            test_db.pool.clone(),
            "active-registry-shell-preserve".to_string(),
            model.clone(),
        );
        let router = app(state.clone());
        state.active_submit_drains.lock().await.insert(session_id);
        let (status, submitted) = request_json(router, Method::POST, &format!("/sessions/{session_id}/send"), json!({"message":"steer after active work"})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(submitted["disposition"], "active_turn_steering");
        tokio::time::sleep(Duration::from_millis(100)).await;
        let premature_turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND input_text='steer after active work'")
            .bind(session_id).fetch_one(&test_db.pool).await.expect("premature turns");
        assert_eq!(premature_turns, 0, "steering must not inject while registry/shell work is still active");
        assert_eq!(
            sqlx::query_as::<_, (String, String, Value)>("SELECT status, stdout, policy_decision FROM command_runs WHERE id=$1")
                .bind(command_run_id).fetch_one(&test_db.pool).await.expect("command before boundary"),
            ("running".to_string(), large_stdout_sentinel.clone(), json!({"decision":"allow","source":"test policy"})),
            "active steering must not rewrite registry command run state before the durable boundary"
        );
        assert_eq!(
            sqlx::query_as::<_, (String, Value)>("SELECT status, metadata FROM god_mode_grants WHERE id=$1")
                .bind(grant_id).fetch_one(&test_db.pool).await.expect("grant before boundary"),
            ("active".to_string(), json!({"beforeSteering":true})),
            "steering must not grant, revoke, or mutate God Mode authority"
        );
        assert_eq!(
            sqlx::query_as::<_, (String, Value)>("SELECT status, metadata FROM shell_runs WHERE id=$1")
                .bind(shell_run_id).fetch_one(&test_db.pool).await.expect("shell before boundary"),
            ("running".to_string(), json!({"beforeSteering":true})),
            "steering must not terminate or rewrite active shell run state"
        );

        sqlx::query("UPDATE command_runs SET status='completed', completed_at=now(), exit_status=0 WHERE id=$1").bind(command_run_id).execute(&test_db.pool).await.expect("complete command");
        sqlx::query("UPDATE host_api_calls SET status='completed', output=$2, completed_at=now() WHERE id=$1").bind(host_api_id).bind(json!({"ok":true})).execute(&test_db.pool).await.expect("complete host");
        sqlx::query("INSERT INTO execution_output_artifacts (id, session_id, turn_id, tool_call_id, script_run_id, command_run_id, source_type, stream, content, byte_count, line_count, metadata) VALUES ($1,$2,$3,$4,$5,$6,'command_run','stdout',$7,$8,1,'{}'::jsonb), ($9,$2,$3,$4,$5,$6,'command_run','stderr',$10,$11,1,'{}'::jsonb)")
            .bind(command_stdout_artifact).bind(session_id).bind(active_turn).bind(tool_call_id).bind(script_run_id).bind(command_run_id).bind(&large_stdout_sentinel).bind(large_stdout_sentinel.len() as i64).bind(command_stderr_artifact).bind(&large_stderr_sentinel).bind(large_stderr_sentinel.len() as i64)
            .execute(&test_db.pool).await.expect("command artifacts");
        sqlx::query("INSERT INTO execution_output_artifacts (id, session_id, turn_id, tool_call_id, script_run_id, process_id, source_type, stream, content, byte_count, line_count, metadata) VALUES ($1,$2,$3,$4,$5,$6,'shell_run','stdout',$7,$8,1,'{}'::jsonb), ($9,$2,$3,$4,$5,$6,'shell_run','stderr',$10,$11,1,'{}'::jsonb)")
            .bind(shell_stdout_artifact).bind(session_id).bind(active_turn).bind(tool_call_id).bind(script_run_id).bind(shell_process_id).bind(&shell_stdout_sentinel).bind(shell_stdout_sentinel.len() as i64).bind(shell_stderr_artifact).bind(&shell_stderr_sentinel).bind(shell_stderr_sentinel.len() as i64)
            .execute(&test_db.pool).await.expect("shell artifacts");
        sqlx::query("UPDATE shell_runs SET status='completed', completed_at=now(), exit_status=0, stdout_artifact_id=$2, stderr_artifact_id=$3 WHERE id=$1").bind(shell_run_id).bind(shell_stdout_artifact).bind(shell_stderr_artifact).execute(&test_db.pool).await.expect("complete shell");
        sqlx::query("UPDATE managed_processes SET status='completed', end_time=now() WHERE id=$1").bind(shell_process_id).execute(&test_db.pool).await.expect("complete process");
        sqlx::query("UPDATE script_runs SET status='completed', final_output='active work complete', completed_at=now() WHERE id=$1").bind(script_run_id).execute(&test_db.pool).await.expect("complete script");
        let explicit_small_value = "EXPLICIT_SMALL_OUTPUT_VALUE_VISIBLE";
        sqlx::query("UPDATE tool_calls SET status='completed', result=$2, completed_at=now() WHERE id=$1")
            .bind(tool_call_id)
            .bind(json!({
                "ok": true,
                "status": "completed",
                "output": {
                    "stdoutArtifact": {
                        "artifactId": Uuid::new_v4(),
                        "stream": "stdout",
                        "preview": explicit_small_value,
                        "byteCount": explicit_small_value.len(),
                        "lineCount": 1,
                        "truncated": false
                    },
                    "stderrArtifact": {
                        "artifactId": Uuid::new_v4(),
                        "stream": "stderr",
                        "preview": "",
                        "byteCount": 0,
                        "lineCount": 0,
                        "truncated": false
                    }
                }
            }))
            .execute(&test_db.pool).await.expect("complete tool");
        crate::lifecycle::complete_turn(&test_db.pool, active_turn, crate::lifecycle::TerminalStatus::Completed, chrono::Utc::now()).await.expect("complete active turn");
        db::append_event(&test_db.pool, session_id, Some(active_turn), "turn", Some(active_turn), "turn.completed", Some("completed"), json!({"activeWorkBoundary":true})).await.expect("boundary event");
        let continued = crate::runtime::continue_pending_steering_after_operation_boundary(&test_db.pool, session_id, active_turn, state.model_client.as_ref().expect("model").as_ref())
            .await
            .expect("continue after active operation boundary");
        assert!(continued, "pending steering must continue same turn after registry/shell/process boundary");
        let steering_turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND input_text='steer after active work' AND status='completed'")
            .bind(session_id).fetch_one(&test_db.pool).await.expect("steering turns");
        assert_eq!(steering_turns, 0, "steering must apply to the active turn instead of creating a second turn after registry/shell work");
        let transcript_types: Vec<String> = sqlx::query_scalar("SELECT item_type FROM current_turn_transcript_items WHERE turn_id=$1 ORDER BY ordering_key ASC")
            .bind(active_turn)
            .fetch_all(&test_db.pool)
            .await
            .expect("transcript types");
        for required in ["command_registry_process", "god_mode_shell_process", "managed_async_process", "applied_steering"] {
            assert!(transcript_types.contains(&required.to_string()), "missing {required}: {transcript_types:?}");
        }
        let transcript_contents: String = sqlx::query_scalar("SELECT string_agg(content, E'\\n' ORDER BY ordering_key) FROM current_turn_transcript_items WHERE turn_id=$1")
            .bind(active_turn)
            .fetch_one(&test_db.pool)
            .await
            .expect("transcript content");
        for hidden in [&large_stdout_sentinel, &large_stderr_sentinel, &shell_stdout_sentinel, &shell_stderr_sentinel] {
            assert!(!transcript_contents.contains(hidden), "hidden output leaked into transcript content: {hidden}");
        }
        assert!(transcript_contents.contains("artifact handles"), "transcript should guide artifact retrieval without embedding output bodies: {transcript_contents}");
        let histories = model.observed_history.lock().expect("history").clone();
        let model_request_text = serde_json::to_string(&histories).expect("history json");
        for hidden in [&large_stdout_sentinel, &large_stderr_sentinel, &shell_stdout_sentinel, &shell_stderr_sentinel] {
            assert!(!model_request_text.contains(hidden), "hidden output leaked into continuation model request: {hidden}");
        }
        assert!(model_request_text.contains(explicit_small_value), "explicit bounded print(...) value must remain visible in same-turn continuation");
        assert!(model_request_text.contains(&command_stdout_artifact.to_string()));
        assert!(model_request_text.contains(&command_stderr_artifact.to_string()));
        let before_rebuild: Vec<(String, String, i64)> = sqlx::query_as("SELECT stable_key, content, ordering_key FROM current_turn_transcript_items WHERE turn_id=$1 ORDER BY ordering_key")
            .bind(active_turn)
            .fetch_all(&test_db.pool)
            .await
            .expect("before transcript rebuild");
        crate::runtime::persist_tool_boundary_transcript(
            &test_db.pool,
            session_id,
            active_turn,
            tool_call_id,
            "active registry and shell work",
            "operation boundary completed",
            &json!({
                "ok": true,
                "status": "completed",
                "output": {
                    "stdoutArtifact": {
                        "artifactId": Uuid::new_v4(),
                        "stream": "stdout",
                        "preview": explicit_small_value,
                        "byteCount": explicit_small_value.len(),
                        "lineCount": 1,
                        "truncated": false
                    },
                    "stderrArtifact": {
                        "artifactId": Uuid::new_v4(),
                        "stream": "stderr",
                        "preview": "",
                        "byteCount": 0,
                        "lineCount": 0,
                        "truncated": false
                    }
                }
            }),
        )
        .await
        .expect("idempotent transcript rebuild");
        let after_rebuild: Vec<(String, String, i64)> = sqlx::query_as("SELECT stable_key, content, ordering_key FROM current_turn_transcript_items WHERE turn_id=$1 ORDER BY ordering_key")
            .bind(active_turn)
            .fetch_all(&test_db.pool)
            .await
            .expect("after transcript rebuild");
        assert_eq!(before_rebuild, after_rebuild, "rebuilding boundary transcript must preserve row identity, content, and ordering");
        let grant_after: (String, Value) = sqlx::query_as("SELECT status, metadata FROM god_mode_grants WHERE id=$1")
            .bind(grant_id).fetch_one(&test_db.pool).await.expect("grant after");
        assert_eq!(grant_after, ("active".to_string(), json!({"beforeSteering":true})));
        let command_after: (String, String, Value) = sqlx::query_as("SELECT status, stdout, policy_decision FROM command_runs WHERE id=$1")
            .bind(command_run_id).fetch_one(&test_db.pool).await.expect("command after");
        assert_eq!(command_after, ("completed".to_string(), large_stdout_sentinel, json!({"decision":"allow","source":"test policy"})));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn agent_path_registry_command_smoke_preserves_output_when_steered_while_active() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("registry-smoke"), ".", Some("."), None, None).await.expect("session");
        let root = starlark_host::ExecutionRoot::new(".").expect("root");
        let mut sh_seed = admin_command_seed("cmd.registry_smoke.sh");
        sh_seed["binaryName"] = json!("sh");
        sh_seed["candidatePaths"] = json!(["/bin/sh"]);
        sh_seed["starlarkObject"] = json!("registry_smoke_sh");
        sh_seed["argvPrefix"] = json!(["-c"]);
        let sh_seed: command_registry::CommandSeed = serde_json::from_value(sh_seed).expect("sh seed");
        apply_registry_seed(&test_db.pool, session_id, sh_seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;

        let command_source = r#"print(cmd["registry_smoke_sh"].run(args=["printf smoke-registry-output"]).sync())"#;
        let (command_turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, command_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, command_turn_id, tool_call_id, command_source, &root, &role)
            .await
            .expect("execute registry smoke command");
        let packet_value = serde_json::to_value(packet).expect("packet value");
        assert_eq!(packet_value["status"], "completed", "normal registry command must complete in agent execute_code path: {packet_value}");
        let before_command_rows: i64 = sqlx::query_scalar("SELECT COUNT(*)
            FROM command_runs cr
            JOIN host_api_calls ha ON ha.id = cr.host_api_call_id
            JOIN script_runs sr ON sr.id = ha.script_run_id
            JOIN tool_calls tc ON tc.id = sr.tool_call_id
            WHERE tc.session_id=$1 AND cr.stdout LIKE '%smoke-registry-output%'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("command rows before steering");
        assert_eq!(before_command_rows, 1, "registry output must be persisted before steering");
        let active_turn = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user','active work while command output is preserved','running',now())")
            .bind(active_turn)
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("active turn for steering smoke");

        let state = ServerState::new_with_model_client(
            test_db.pool.clone(),
            "registry-smoke-steering".to_string(),
            Arc::new(FakeModelClient { direct_final_text: Some("smoke steering applied"), ..Default::default() }),
        );
        let router = app(state.clone());
        state.active_submit_drains.lock().await.insert(session_id);
        let (status, submitted) = request_json(router, Method::POST, &format!("/sessions/{session_id}/send"), json!({"message":"apply smoke steering after command"})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(submitted["disposition"], "active_turn_steering");
        let during_command_rows: i64 = sqlx::query_scalar("SELECT COUNT(*)
            FROM command_runs cr
            JOIN host_api_calls ha ON ha.id = cr.host_api_call_id
            JOIN script_runs sr ON sr.id = ha.script_run_id
            JOIN tool_calls tc ON tc.id = sr.tool_call_id
            WHERE tc.session_id=$1 AND cr.stdout LIKE '%smoke-registry-output%'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("command rows during steering");
        assert_eq!(during_command_rows, 1, "steering must not abort or erase registry output");

        crate::lifecycle::complete_turn(&test_db.pool, active_turn, crate::lifecycle::TerminalStatus::Completed, chrono::Utc::now())
            .await
            .expect("complete active smoke turn boundary");
        {
            let mut active = state.active_submit_drains.lock().await;
            active.remove(&session_id);
            active.insert(session_id);
        }
        spawn_submit_worker(state.clone(), session_id);
        for _ in 0..80 {
            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='applied' AND content='apply smoke steering after command'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("applied count");
            if applied == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let after_command_rows: i64 = sqlx::query_scalar("SELECT COUNT(*)
            FROM command_runs cr
            JOIN host_api_calls ha ON ha.id = cr.host_api_call_id
            JOIN script_runs sr ON sr.id = ha.script_run_id
            JOIN tool_calls tc ON tc.id = sr.tool_call_id
            WHERE tc.session_id=$1 AND cr.stdout LIKE '%smoke-registry-output%'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("command rows after steering");
        assert_eq!(after_command_rows, 1, "registry command output must remain preserved after steering applies");
        let steering_turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND input_text='apply smoke steering after command' AND status='completed'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("steering turns");
        assert!(steering_turns <= 1, "steered input handling must not duplicate turns or abort existing command output");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn submitted_input_during_compaction_waits_and_uses_compacted_context() {
        let test_db = validation_db().await;
        let model = Arc::new(FakeModelClient { direct_final_text: Some("after compaction"), ..Default::default() });
        let state = ServerState::new_with_model_client(test_db.pool.clone(), "compaction-handoff".to_string(), model.clone());
        let router = app(state.clone());
        let (_, created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"compaction-proof","model":"fake-model","workdir":".","worktreeRoot":".","title":"Compaction proof","name":"compaction-proof"}),
        )
        .await;
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");
        let completed_turn = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at, completed_at) VALUES ($1,$2,'user','old context before compact','completed',now() - interval '2 minutes',now() - interval '2 minutes')")
            .bind(completed_turn)
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("completed turn");
        sqlx::query("INSERT INTO model_events (id, session_id, turn_id, event_type, payload) VALUES ($1,$2,$3,'final_response',$4)")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .bind(completed_turn)
            .bind(json!({"summary":"old assistant before compact"}))
            .execute(&test_db.pool)
            .await
            .expect("final model event");

        state.active_compactions.lock().await.insert(session_id);
        let send_path = format!("/sessions/{session_id}/send");
        let (status, submitted) = request_json(
            router.clone(),
            Method::POST,
            &send_path,
            json!({"message":"continue after compact one"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(submitted["disposition"], "queued_continuation_after_compaction");
        let (status, submitted_two) = request_json(
            router,
            Method::POST,
            &send_path,
            json!({"message":"continue after compact two"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(submitted_two["disposition"], "queued_continuation_after_compaction");
        tokio::time::sleep(Duration::from_millis(100)).await;
        let premature_turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND input_text LIKE 'continue after compact%'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("premature turns");
        assert_eq!(premature_turns, 0, "queued compaction continuations must not apply before compaction completes");

        compaction::compact_session_through_latest_completed_turn(&test_db.pool, session_id, compaction::CompactionBudget::default()).await.expect("compact");
        state.active_compactions.lock().await.remove(&session_id);
        let should_spawn = {
            let mut active = state.active_submit_drains.lock().await;
            active.insert(session_id)
        };
        assert!(should_spawn);
        spawn_submit_worker(state.clone(), session_id);
        for _ in 0..80 {
            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='applied'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("applied count");
            if applied == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let history = model.observed_history.lock().expect("observed history").clone();
        assert!(history.iter().any(|items| items.iter().any(|item| item.source == "compaction_checkpoint")), "model dispatch must use compacted context after handoff: {history:?}");
        let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='applied'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("applied final");
        assert_eq!(applied, 2);
        let submitted_order = sqlx::query("SELECT content, status, placement_turn_id FROM submitted_inputs WHERE session_id=$1 ORDER BY ordering_key ASC")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("submitted order");
        assert_eq!(submitted_order.iter().map(|row| row.get::<String, _>("content")).collect::<Vec<_>>(), vec![
            "continue after compact one".to_string(),
            "continue after compact two".to_string(),
        ]);
        assert!(submitted_order.iter().all(|row| row.get::<String, _>("status") == "applied" && row.get::<Option<Uuid>, _>("placement_turn_id").is_some()));
        let turn_order = sqlx::query("SELECT input_text FROM turns WHERE session_id=$1 AND input_text LIKE 'continue after compact%' ORDER BY started_at ASC, id ASC")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("turn order");
        assert_eq!(turn_order.iter().map(|row| row.get::<String, _>("input_text")).collect::<Vec<_>>(), vec![
            "continue after compact one".to_string(),
            "continue after compact two".to_string(),
        ]);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn submit_racing_with_compaction_completion_drains_after_checkpoint_commit() {
        let test_db = validation_db().await;
        let model = Arc::new(FakeModelClient { direct_final_text: Some("after compact race"), ..Default::default() });
        let state = ServerState::new_with_model_client(test_db.pool.clone(), "compaction-completion-race".to_string(), model.clone());
        let router = app(state.clone());
        let (_, created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"compaction-race","model":"fake-model","workdir":".","worktreeRoot":".","title":"Compaction race","name":"compaction-race"}),
        )
        .await;
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");
        let completed_turn = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at, completed_at) VALUES ($1,$2,'user','context before compaction completion race','completed',now() - interval '2 minutes',now() - interval '2 minutes')")
            .bind(completed_turn)
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("completed turn");
        sqlx::query("INSERT INTO model_events (id, session_id, turn_id, event_type, payload) VALUES ($1,$2,$3,'final_response',$4)")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .bind(completed_turn)
            .bind(json!({"summary":"assistant before race compact"}))
            .execute(&test_db.pool)
            .await
            .expect("final model event");

        state.active_compactions.lock().await.insert(session_id);
        let send_path = format!("/sessions/{session_id}/send");
        let compact_path = format!("/sessions/{session_id}/compact");
        let (submit_status, submitted) = request_json(
            router.clone(),
            Method::POST,
            &send_path,
            json!({"message":"queued through compaction completion race"}),
        )
        .await;
        assert_eq!(submit_status, StatusCode::OK, "submit must be accepted: {submitted}");
        assert_eq!(submitted["disposition"], "queued_continuation_after_compaction");
        let queued_before_compaction: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='accepted'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("queued before compaction completion");
        assert_eq!(queued_before_compaction, 1, "submit must be durable before compaction completes");
        let (compact_status, compacted) = request_json(router.clone(), Method::POST, &compact_path, json!({"throughTurn": completed_turn})).await;
        assert_eq!(compact_status, StatusCode::OK, "compaction must complete: {compacted}");

        for _ in 0..120 {
            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='applied' AND content='queued through compaction completion race'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("applied count");
            if applied == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let applied_row = sqlx::query("SELECT status, placement_turn_id FROM submitted_inputs WHERE session_id=$1 AND content='queued through compaction completion race'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("submitted row");
        assert_eq!(applied_row.get::<String, _>("status"), "applied");
        assert!(applied_row.get::<Option<Uuid>, _>("placement_turn_id").is_some());
        let history = model.observed_history.lock().expect("observed history").clone();
        assert!(
            history.iter().any(|items| items.iter().any(|item| item.source == "compaction_checkpoint")),
            "submit racing with compaction completion must dispatch only after compacted context is committed: {history:?}"
        );
        let active = state.active_compactions.lock().await.contains(&session_id);
        assert!(!active, "compaction race must not leave active compaction state behind");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn submitted_input_after_final_response_commit_preserves_current_final_and_starts_next_turn() {
        let test_db = validation_db().await;
        let model = Arc::new(FakeModelClient {
            direct_final_text: Some("stable final response"),
            request_delay_ms: Some(100),
            ..Default::default()
        });
        let state = ServerState::new_with_model_client(test_db.pool.clone(), "final-output-handoff".to_string(), model);
        let router = app(state.clone());
        let (_, created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"final-proof","model":"fake-model","workdir":".","worktreeRoot":".","title":"Final proof","name":"final-proof"}),
        )
        .await;
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");
        let (status, first) = request_json(router.clone(), Method::POST, &format!("/sessions/{session_id}/send"), json!({"message":"first final"})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(first["disposition"], "idle_turn_start");
        for _ in 0..80 {
            let completed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status='completed'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("completed first count");
            if completed == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let (status, second) = request_json(router, Method::POST, &format!("/sessions/{session_id}/send"), json!({"message":"second next turn"})).await;
        assert_eq!(status, StatusCode::OK);
        assert!(matches!(
            second["disposition"].as_str(),
            Some("idle_turn_start" | "queued_next_turn_after_final_output")
        ));
        for _ in 0..80 {
            let completed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status='completed'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("completed count");
            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='applied'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("applied count");
            if completed == 2 && applied == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let rows = sqlx::query("SELECT id, input_text, status FROM turns WHERE session_id=$1 ORDER BY started_at ASC")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("turn rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get::<String, _>("input_text"), "first final");
        assert_eq!(rows[1].get::<String, _>("input_text"), "second next turn");
        assert_eq!(rows[0].get::<String, _>("status"), "completed");
        assert_eq!(rows[1].get::<String, _>("status"), "completed");
        let first_turn_id = rows[0].get::<Uuid, _>("id");
        let first_final: String = sqlx::query_scalar("SELECT payload->>'summary' FROM model_events WHERE turn_id=$1 AND event_type='final_response' ORDER BY ordinal DESC LIMIT 1")
            .bind(first_turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("first final");
        assert_eq!(first_final, "stable final response");
        let applied_order = sqlx::query("SELECT content, status, placement_turn_id FROM submitted_inputs WHERE session_id=$1 ORDER BY ordering_key ASC")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("submitted order");
        assert_eq!(applied_order.len(), 2);
        assert_eq!(applied_order[0].get::<String, _>("content"), "first final");
        assert_eq!(applied_order[1].get::<String, _>("content"), "second next turn");
        assert!(applied_order.iter().all(|row| row.get::<String, _>("status") == "applied" && row.get::<Option<Uuid>, _>("placement_turn_id").is_some()));
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn direct_final_with_pending_steering_continues_same_turn_with_current_transcript() {
        let test_db = validation_db().await;
        let model = Arc::new(FakeModelClient {
            direct_final_text: Some("direct final before steering"),
            request_delay_ms: Some(1000),
            ..Default::default()
        });
        let state = ServerState::new_with_model_client(test_db.pool.clone(), "direct-final-same-turn".to_string(), model.clone());
        let router = app(state.clone());
        let (_, created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"direct-final-same-turn","model":"fake-model","workdir":".","worktreeRoot":".","title":"Direct final same turn","name":"direct-final-same-turn"}),
        )
        .await;
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");
        let send_path = format!("/sessions/{session_id}/send");
        let first = tokio::spawn({
            let router = router.clone();
            let send_path = send_path.clone();
            async move { request_json(router, Method::POST, &send_path, json!({"message":"initial direct final prompt"})).await }
        });
        for _ in 0..80 {
            let running: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status='running'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("running turn count");
            if running == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let (steer_status, steer_body) = request_json(router, Method::POST, &send_path, json!({"message":"same turn steering text"})).await;
        assert_eq!(steer_status, StatusCode::OK, "steering submit must be accepted while direct final is in flight: {steer_body}");
        assert_eq!(steer_body["disposition"], "active_turn_steering");
        let (first_status, first_body) = first.await.expect("first send join");
        assert_eq!(first_status, StatusCode::OK, "first send completes: {first_body}");
        for _ in 0..120 {
            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND content='same turn steering text' AND status='applied'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("applied steering count");
            if applied == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let turns: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, input_text FROM turns WHERE session_id=$1 ORDER BY started_at ASC")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("turns");
        assert_eq!(turns.len(), 1, "pending steering after direct final must continue the same turn, not start a second turn");
        assert_eq!(turns[0].1, "initial direct final prompt");
        let submitted = sqlx::query("SELECT target_turn_id, placement_turn_id, status FROM submitted_inputs WHERE session_id=$1 AND content='same turn steering text'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("submitted row");
        assert_eq!(submitted.get::<String, _>("status"), "applied");
        assert_eq!(submitted.get::<Option<Uuid>, _>("target_turn_id"), Some(turns[0].0));
        assert_eq!(submitted.get::<Option<Uuid>, _>("placement_turn_id"), Some(turns[0].0));
        let messages = model.observed_messages.lock().expect("messages").clone();
        assert!(messages.contains(&"initial direct final prompt".to_string()));
        let histories = model.observed_history.lock().expect("history").clone();
        if histories.len() > 1 {
            let current = histories[1]
                .iter()
                .find(|item| item.source == "current_turn_transcript")
                .expect("current-turn transcript item");
            assert_eq!(current.turn_id, turns[0].0.to_string());
            assert_eq!(current.user, "initial direct final prompt");
            assert_eq!(current.assistant.as_deref(), Some("direct final before steering"));
        }
        let continuation_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='model.direct_final_continuation'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("continuation events");
        assert!(continuation_events <= 1);
        let transcript_types: Vec<String> = sqlx::query_scalar("SELECT item_type FROM current_turn_transcript_items WHERE turn_id=$1 ORDER BY ordering_key ASC")
            .bind(turns[0].0)
            .fetch_all(&test_db.pool)
            .await
            .expect("direct transcript types");
        if !transcript_types.is_empty() {
            assert!(transcript_types.contains(&"initial_user_input".to_string()));
            assert!(transcript_types.contains(&"assistant_final_text".to_string()));
        }
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn tool_boundary_pending_steering_continues_same_turn_with_tool_transcript() {
        let test_db = validation_db().await;
        let model = Arc::new(FakeModelClient {
            request_delay_ms: Some(1000),
            direct_final_text: None,
            ..Default::default()
        });
        let state = ServerState::new_with_model_client(test_db.pool.clone(), "tool-boundary-same-turn".to_string(), model.clone());
        let router = app(state.clone());
        let (_, created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"tool-boundary-same-turn","model":"fake-model","workdir":".","worktreeRoot":".","title":"Tool boundary same turn","name":"tool-boundary-same-turn"}),
        )
        .await;
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");
        let send_path = format!("/sessions/{session_id}/send");
        let first = tokio::spawn({
            let router = router.clone();
            let send_path = send_path.clone();
            async move { request_json(router, Method::POST, &send_path, json!({"message":"initial tool prompt"})).await }
        });
        for _ in 0..80 {
            let running: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND status='running'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("running turn count");
            if running == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let (steer_status, steer_body) = request_json(router, Method::POST, &send_path, json!({"message":"same turn after tool"})).await;
        assert_eq!(steer_status, StatusCode::OK, "steering submit must be accepted while tool path is in flight: {steer_body}");
        assert_eq!(steer_body["disposition"], "active_turn_steering");
        let (first_status, first_body) = first.await.expect("first send join");
        assert_eq!(first_status, StatusCode::OK, "first send completes: {first_body}");
        for _ in 0..120 {
            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND content='same turn after tool' AND status='applied'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("applied steering count");
            if applied == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let turns: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, input_text FROM turns WHERE session_id=$1 ORDER BY started_at ASC")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("turns");
        assert_eq!(turns.len(), 1, "tool-boundary steering must continue the same turn, not start a second turn");
        assert_eq!(turns[0].1, "initial tool prompt");
        let submitted = sqlx::query("SELECT target_turn_id, placement_turn_id, status FROM submitted_inputs WHERE session_id=$1 AND content='same turn after tool'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("submitted row");
        assert_eq!(submitted.get::<String, _>("status"), "applied");
        assert_eq!(submitted.get::<Option<Uuid>, _>("target_turn_id"), Some(turns[0].0));
        assert_eq!(submitted.get::<Option<Uuid>, _>("placement_turn_id"), Some(turns[0].0));
        let messages = model.observed_messages.lock().expect("messages").clone();
        assert!(messages.contains(&"initial tool prompt".to_string()));
        let histories = model.observed_history.lock().expect("history").clone();
        if histories.len() > 1 {
            let current = histories[1]
                .iter()
                .find(|item| item.source == "current_turn_transcript")
                .expect("current-turn transcript item");
            assert_eq!(current.turn_id, turns[0].0.to_string());
            assert_eq!(current.user, "initial tool prompt");
            assert!(current.assistant.as_deref().unwrap_or_default().contains("tool_result"));
        }
        let continuation_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='model.same_turn_continuation'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("continuation events");
        assert!(continuation_events <= 1);
        let transcript_types: Vec<String> = sqlx::query_scalar("SELECT item_type FROM current_turn_transcript_items WHERE turn_id=$1 ORDER BY ordering_key ASC")
            .bind(turns[0].0)
            .fetch_all(&test_db.pool)
            .await
            .expect("tool transcript types");
        if !transcript_types.is_empty() {
            for required in [
                "initial_user_input",
                "assistant_intermediate",
                "tool_call",
            ] {
                assert!(transcript_types.contains(&required.to_string()), "missing transcript type {required}: {transcript_types:?}");
            }
        }
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn explicit_final_output_commit_queues_submit_until_current_turn_finishes() {
        let test_db = validation_db().await;
        let model = Arc::new(FakeModelClient {
            direct_final_text: Some("next turn response"),
            ..Default::default()
        });
        let state = ServerState::new_with_model_client(test_db.pool.clone(), "explicit-final-commit".to_string(), model);
        let router = app(state.clone());
        let (_, created) = request_json(
            router.clone(),
            Method::POST,
            "/sessions",
            json!({"role":"runtime-no-rg","project":"explicit-final","model":"fake-model","workdir":".","worktreeRoot":".","title":"Explicit final","name":"explicit-final"}),
        )
        .await;
        let session_id: Uuid = serde_json::from_value(created["sessionId"].clone()).expect("session id");
        let committed_turn_id = Uuid::new_v4();
        let final_event_id = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user','first committed final','running',now())")
            .bind(committed_turn_id)
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("running turn");
        sqlx::query("INSERT INTO model_events (id, session_id, turn_id, event_type, payload) VALUES ($1,$2,$3,'final_response',$4)")
            .bind(final_event_id)
            .bind(session_id)
            .bind(committed_turn_id)
            .bind(json!({"summary":"committed final text", "finalText":"committed final text"}))
            .execute(&test_db.pool)
            .await
            .expect("committed final event");
        db::append_event(
            &test_db.pool,
            session_id,
            Some(committed_turn_id),
            "model",
            Some(final_event_id),
            "model.final_output_committed",
            Some("committed"),
            json!({"finalText":"committed final text", "test":"explicit final-output handoff"}),
        ).await.expect("commit event");
        assert!(state.active_submit_drains.lock().await.insert(session_id));

        let (status, submitted) = request_json(
            router,
            Method::POST,
            &format!("/sessions/{session_id}/send"),
            json!({"message":"next after committed final"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(submitted["disposition"], "queued_next_turn_after_final_output");
        assert_eq!(submitted["turnId"], committed_turn_id.to_string());
        tokio::time::sleep(Duration::from_millis(100)).await;
        let premature_next_turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND input_text='next after committed final'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("premature next turns");
        assert_eq!(premature_next_turns, 0, "next turn must not start before committed final turn completes");

        crate::lifecycle::complete_turn(&test_db.pool, committed_turn_id, crate::lifecycle::TerminalStatus::Completed, chrono::Utc::now()).await.expect("complete committed turn");
        db::append_event(&test_db.pool, session_id, Some(committed_turn_id), "turn", Some(committed_turn_id), "turn.completed", Some("completed"), json!({"finalOutputCommitted": true})).await.expect("turn completed event");
        state.active_submit_drains.lock().await.remove(&session_id);
        assert!(state.active_submit_drains.lock().await.insert(session_id));
        spawn_submit_worker(state.clone(), session_id);
        for _ in 0..80 {
            let next_turns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turns WHERE session_id=$1 AND input_text='next after committed final' AND status='completed'")
                .bind(session_id)
                .fetch_one(&test_db.pool)
                .await
                .expect("next completed turns");
            if next_turns == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let turns = sqlx::query("SELECT id, input_text, status FROM turns WHERE session_id=$1 ORDER BY started_at ASC, id ASC")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("turns");
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].get::<Uuid, _>("id"), committed_turn_id);
        assert_eq!(turns[0].get::<String, _>("input_text"), "first committed final");
        assert_eq!(turns[0].get::<String, _>("status"), "completed");
        assert_eq!(turns[1].get::<String, _>("input_text"), "next after committed final");
        assert_eq!(turns[1].get::<String, _>("status"), "completed");
        let first_final: String = sqlx::query_scalar("SELECT payload->>'summary' FROM model_events WHERE turn_id=$1 AND event_type='final_response'")
            .bind(committed_turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("first final text");
        assert_eq!(first_final, "committed final text");
        let submitted_row = sqlx::query("SELECT content, disposition, status, placement_turn_id FROM submitted_inputs WHERE session_id=$1 ORDER BY ordering_key ASC")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("submitted row");
        assert_eq!(submitted_row.get::<String, _>("content"), "next after committed final");
        assert_eq!(submitted_row.get::<String, _>("disposition"), "queued_next_turn_after_final_output");
        assert_eq!(submitted_row.get::<String, _>("status"), "applied");
        assert_eq!(submitted_row.get::<Option<Uuid>, _>("placement_turn_id"), Some(turns[1].get::<Uuid, _>("id")));
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
        let terminable_archive = transport
            .send(GuiTransportRequestPacket {
                packet_id: "settings-archive-terminates-process".to_string(),
                intent: GuiTransportRequest::DispatchOperation {
                    operation: GuiOperationRequest::ArchiveSession { session_id: terminable_session.to_string() },
                },
            })
            .await;
        assert!(terminable_archive.iter().any(|packet| matches!(&packet.output, GuiTransportOutput::WorkbenchView { view_model }
            if !view_model.shell.sessions.iter().any(|row| row.id == terminable_session.to_string())
        )), "Archive must terminate session-ending managed processes and refresh the connected projection: {terminable_archive:?}");
        let terminated_process_row: (String, Option<String>, bool) = sqlx::query_as(
            "SELECT status, termination_reason, end_time IS NOT NULL FROM managed_processes WHERE id=$1",
        )
        .bind(terminable_process)
        .fetch_one(&test_db.pool)
        .await
        .expect("terminated process row");
        assert_eq!(terminated_process_row.0, "sessionTerminated");
        assert_eq!(terminated_process_row.1.as_deref(), Some("session archived"));
        assert!(terminated_process_row.2);
        let terminable_session_row: (String, Option<chrono::DateTime<chrono::Utc>>) =
            sqlx::query_as("SELECT status, archived_at FROM sessions WHERE id=$1")
                .bind(terminable_session)
                .fetch_one(&test_db.pool)
                .await
                .expect("terminable archived session row");
        assert_eq!(terminable_session_row.0, "stopped");
        assert!(terminable_session_row.1.is_some());
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
        assert_eq!(created_row.8, "stopped");

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
        )), "Archive must update connected GUI state immediately: {archive:?}");
        let archived_row: (String, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as("SELECT status, archived_at FROM sessions WHERE id=$1")
            .bind(forked_uuid)
            .fetch_one(&test_db.pool)
            .await
            .expect("archived row");
        assert_eq!(archived_row.0, "stopped");
        assert!(archived_row.1.is_some());

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
                GuiOperationRequest::ArchiveSession { session_id: Uuid::nil().to_string() },
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

        let mut view = None;
        for attempt in 0..20 {
            let rehydrated = transport
                .send(GuiTransportRequestPacket {
                    packet_id: format!("live-validation-rehydrate-{attempt}"),
                    intent: GuiTransportRequest::Rehydrate {
                        selected_session_id: Some(session_id.clone()),
                    },
                })
                .await;
            let candidate = rehydrated
                .iter()
                .find_map(|packet| match &packet.output {
                    GuiTransportOutput::WorkbenchView { view_model } => Some(view_model.clone()),
                    _ => None,
                })
                .expect("Workbench view after send");
            if candidate
                .shell
                .selected_conversation
                .iter()
                .any(|row| row.author == "Assistant" && row.body.contains("fake final fake-call completed"))
            {
                view = Some(candidate);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let view = view.expect("Workbench view after send");
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
                        message: "Use execute_code with exactly this harmless read-only Starlark: print({\"validation\":\"ok\",\"source\":\"live-real-gui-e2e\"})".to_string(),
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
                intent: GuiTransportRequest::ReadNextGuiStreamPacket,
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

        let archive_session_id = db::new_session(
            &test_db.pool,
            &role,
            Some("semantic-session"),
            ".",
            Some("."),
            Some("Semantic archive session"),
            Some("semantic-archive-session"),
        )
        .await
        .expect("new archive session");
        apply_until(&mut ws, &mut client_projection, |_delta, projection| {
            projection.sessions.iter().any(|session| session.id == archive_session_id.to_string())
        })
        .await;
        db::archive_session(&test_db.pool, archive_session_id)
            .await
            .expect("archive");
        apply_until(&mut ws, &mut client_projection, |_delta, projection| {
            projection
                .sessions
                .iter()
                .all(|session| session.id != archive_session_id.to_string())
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
            json!({"path":"resume.txt","content":"resumed","description":"resume approved file write","executionRoot": temp_root.display().to_string()}),
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
    async fn mutation_descriptions_survive_approval_pause_resume_failure_and_audit_replay() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("mutation-approval"), ".", Some("."), None, None).await.expect("session");
        let temp_root = std::env::temp_dir().join(format!("robdex-agent-runtime-mutation-approval-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).expect("mutation approval temp root");
        std::fs::write(temp_root.join("patch.txt"), "before\n").expect("patch file");
        std::fs::write(temp_root.join("replace.txt"), "old text\n").expect("replace file");
        std::fs::write(temp_root.join("bad.txt"), "unchanged\n").expect("bad file");

        async fn paused_mutation(pool: &PgPool, session_id: Uuid, role: &RoleSnapshot, action: &str, action_input: Value) -> Uuid {
            let turn_id = Uuid::new_v4();
            sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status) VALUES ($1,$2,'user',$3,'completed')")
                .bind(turn_id)
                .bind(session_id)
                .bind(format!("paused {action}"))
                .execute(pool)
                .await
                .expect("turn");
            let tool_id = Uuid::new_v4();
            sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status) VALUES ($1,$2,$3,'execute_code',$4,'{}'::jsonb,'completed')")
                .bind(tool_id)
                .bind(session_id)
                .bind(turn_id)
                .bind(format!("paused-{action}"))
                .execute(pool)
                .await
                .expect("tool");
            let script_id = Uuid::new_v4();
            sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status) VALUES ($1,$2,$3,'completed')")
                .bind(script_id)
                .bind(tool_id)
                .bind(format!("paused {action}"))
                .execute(pool)
                .await
                .expect("script");
            let policy = crate::policy::PolicyResult {
                action: action.to_string(),
                decision: crate::policy::RuntimeDecision::ApprovalRequired,
                reason: "deterministic mutation approval".to_string(),
                input: action_input.clone(),
                role_id: role.id.clone(),
                role_version: role.version.clone(),
                role_version_id: role.role_version_id.to_string(),
                source_decision: Some("ownerApproval".to_string()),
                required_approver_kind: Some(crate::approvals::ApproverKind::Owner),
            };
            let approval_id = approvals::request_approval(pool, session_id, Some(turn_id), &policy, role).await.expect("approval");
            approvals::create_paused_action(pool, approval_id, session_id, Some(turn_id), Some(tool_id), Some(script_id), action, action_input, role).await.expect("paused");
            approvals::decide(pool, approval_id, approvals::ApprovalDecision::Approved, "deterministic mutation resume").await.expect("decide");
            approval_id
        }

        let fs_description = "resume approved file write description";
        let fs_approval = paused_mutation(&test_db.pool, session_id, &role, "fs.write", json!({"path":"write.txt","content":"resumed write\n","description":fs_description,"executionRoot": temp_root.display().to_string()})).await;
        approvals::resume(&test_db.pool, fs_approval).await.expect("resume fs.write");

        let patch_description = "resume approved patch description";
        let patch = "--- a/patch.txt\n+++ b/patch.txt\n@@ -1 +1 @@\n-before\n+after\n";
        let patch_approval = paused_mutation(&test_db.pool, session_id, &role, "patch.apply", json!({"unifiedDiff":patch,"description":patch_description,"executionRoot": temp_root.display().to_string()})).await;
        approvals::resume(&test_db.pool, patch_approval).await.expect("resume patch.apply");

        let replace_description = "resume approved exact replace description";
        let replace_approval = paused_mutation(&test_db.pool, session_id, &role, "file.replace_exact", json!({"path":"replace.txt","old":"old text","new":"new text","description":replace_description,"executionRoot": temp_root.display().to_string()})).await;
        approvals::resume(&test_db.pool, replace_approval).await.expect("resume file.replace_exact");

        let failed_description = "resume failed patch keeps description";
        let failed_patch = "--- a/bad.txt\n+++ b/bad.txt\n@@ -1 +1 @@\n-not present\n+changed\n";
        let failed_approval = paused_mutation(&test_db.pool, session_id, &role, "patch.apply", json!({"unifiedDiff":failed_patch,"description":failed_description,"executionRoot": temp_root.display().to_string()})).await;
        assert!(approvals::resume(&test_db.pool, failed_approval).await.is_err(), "failed patch resume must report replay failure");

        let mutation_descriptions: Vec<String> = sqlx::query_scalar("SELECT mutation_description FROM file_mutations WHERE script_run_id IN (SELECT script_run_id FROM paused_actions WHERE session_id=$1) ORDER BY action_name")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("file mutation descriptions");
        assert!(mutation_descriptions.contains(&fs_description.to_string()), "{mutation_descriptions:?}");
        assert!(mutation_descriptions.contains(&replace_description.to_string()), "{mutation_descriptions:?}");
        let patch_descriptions: Vec<(String, String)> = sqlx::query_as("SELECT mutation_description, status FROM patch_runs WHERE script_run_id IN (SELECT script_run_id FROM paused_actions WHERE session_id=$1) ORDER BY mutation_description")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("patch descriptions");
        assert!(patch_descriptions.contains(&(patch_description.to_string(), "completed".to_string())), "{patch_descriptions:?}");
        assert!(patch_descriptions.contains(&(failed_description.to_string(), "failed".to_string())), "{patch_descriptions:?}");
        let paused_inputs: Vec<Value> = sqlx::query_scalar("SELECT action_input FROM paused_actions WHERE session_id=$1")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("paused inputs");
        for expected in [fs_description, patch_description, replace_description, failed_description] {
            assert!(paused_inputs.iter().any(|input| input.get("description").and_then(Value::as_str) == Some(expected)), "paused input missing description {expected}: {paused_inputs:?}");
        }
        let completed_events: Vec<Value> = sqlx::query_scalar("SELECT payload FROM event_stream WHERE session_id=$1 AND event_type IN ('file_mutation.completed','patch.completed') ORDER BY created_at")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("mutation events");
        for expected in [fs_description, patch_description, replace_description, failed_description] {
            assert!(completed_events.iter().any(|payload| payload.get("description").and_then(Value::as_str) == Some(expected)), "event replay missing description {expected}: {completed_events:?}");
        }
        assert_eq!(std::fs::read_to_string(temp_root.join("write.txt")).expect("write file"), "resumed write\n");
        assert_eq!(std::fs::read_to_string(temp_root.join("patch.txt")).expect("patch file"), "after");
        assert_eq!(std::fs::read_to_string(temp_root.join("replace.txt")).expect("replace file"), "new text\n");
        std::fs::remove_dir_all(&temp_root).expect("cleanup mutation approval temp root");
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
        assert!(options["defaultRecipients"].as_array().expect("recipients").iter().any(|value| value == "owner"));
        assert!(!options["defaultRecipients"].as_array().expect("recipients").iter().any(|value| value == "operator" || value == "orchestrator" || value == "runtime"));

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

        let mut draft_v2 = role_editor_draft_json("gui-role", "1.0.1", "inline gui role instructions v2");
        draft_v2["capabilities"] = json!(["fs.read"]);
        draft_v2["policy"] = json!({"fs.read": "allow"});
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
        let current_snapshot = db::current_role_snapshot(&test_db.pool, "gui-role").await.expect("current role snapshot");
        assert_eq!(current_snapshot.role_version_id.to_string(), version_v2);
        assert_eq!(current_snapshot.capabilities, vec!["fs.read".to_string()]);
        assert_eq!(current_snapshot.policy.get("fs.read"), Some(&crate::roles::ManifestDecision::Allow));
        assert!(!current_snapshot.policy.contains_key("tool.execute_code"));
        let mut all_decisions = role_editor_draft_json("gui-role", "1.0.2", "inline gui role instructions all decisions");
        all_decisions["capabilities"] = json!(["file.head", "file.tail", "git.status", "git.diff"]);
        all_decisions["policy"] = json!({
            "file.head": "allow",
            "file.tail": "deny",
            "git.status": "ownerApproval",
            "git.diff": "orchestratorApproval"
        });
        let (status, validation) = request_json(router.clone(), Method::POST, "/roles/editor/validate", all_decisions.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(validation["valid"], true);
        let (status, updated) = request_json(router.clone(), Method::POST, "/roles/gui-role/versions", all_decisions).await;
        assert_eq!(status, StatusCode::OK);
        let all_decisions_version = updated["versionId"].as_str().expect("all decisions version").to_string();
        let all_decisions_snapshot = db::current_role_snapshot(&test_db.pool, "gui-role").await.expect("all decisions role snapshot");
        assert_eq!(all_decisions_snapshot.role_version_id.to_string(), all_decisions_version);
        assert_eq!(
            all_decisions_snapshot.capabilities,
            vec!["file.head".to_string(), "file.tail".to_string(), "git.status".to_string(), "git.diff".to_string()]
        );
        assert_eq!(all_decisions_snapshot.policy.get("file.head"), Some(&crate::roles::ManifestDecision::Allow));
        assert_eq!(all_decisions_snapshot.policy.get("file.tail"), Some(&crate::roles::ManifestDecision::Deny));
        assert_eq!(all_decisions_snapshot.policy.get("git.status"), Some(&crate::roles::ManifestDecision::OwnerApproval));
        assert_eq!(all_decisions_snapshot.policy.get("git.diff"), Some(&crate::roles::ManifestDecision::OrchestratorApproval));
        assert!(!all_decisions_snapshot.capabilities.contains(&"tool.execute_code".to_string()));
        assert!(!all_decisions_snapshot.policy.contains_key("tool.execute_code"));
        let mut mismatched = role_editor_draft_json("gui-role", "1.0.2", "inline gui role instructions mismatch");
        mismatched["capabilities"] = json!(["tool.execute_code"]);
        mismatched["policy"] = json!({"fs.read": "allow"});
        let (status, validation) = request_json(router.clone(), Method::POST, "/roles/editor/validate", mismatched.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(validation["valid"], false);
        assert!(validation["errors"].to_string().contains("capabilities must exactly match policy keys"));
        let (status, error) = request_json(router.clone(), Method::POST, "/roles/gui-role/versions", mismatched).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_api_error(&error, "validation_failed");
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
    async fn role_update_gui_operation_persists_current_authority_snapshot() {
        let test_db = validation_db().await;
        let state = ServerState::new(test_db.pool.clone());
        let router = Router::new()
            .route("/state/snapshot", get(test_snapshot_without_external_model_lookup))
            .route("/state/ws", get(state_ws))
            .route("/roles/{role_id}", get(show_role))
            .route("/roles/{role_id}/versions", get(role_versions).post(update_role_from_draft))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind role gui server");
        let addr = listener.local_addr().expect("role gui addr");
        let server_task = tokio::spawn(async move {
            axum::serve(listener, router).await.expect("serve role gui server");
        });
        let base_url = format!("http://{addr}");
        let before_version: Uuid = sqlx::query_scalar("SELECT current_version_id FROM roles WHERE id='project-progenitor'")
            .fetch_one(&test_db.pool)
            .await
            .expect("before current version");
        let before_count: i64 = sqlx::query_scalar("SELECT count(*) FROM role_versions WHERE role_id='project-progenitor'")
            .fetch_one(&test_db.pool)
            .await
            .expect("before role version count");
        let mut controller = GuiBackendController::new();
        let connect = tokio::time::timeout(
            Duration::from_secs(10),
            controller.dispatch(GuiOperationRequest::Connect {
                base_url,
                selected_session_id: None,
            }),
        )
        .await
        .expect("role GUI connect timed out");
        assert!(matches!(connect.outcome, GuiOperationOutcome::ProjectionUpdated { .. }), "connect outcome: {:?}", connect.outcome);

        let draft: RoleEditorDraft = serde_json::from_value(json!({
            "id": "project-progenitor",
            "version": "99.99.99-gui",
            "displayName": "Project Progenitor",
            "modelDefaults": {"model": "gpt-5.4-mini", "reasoningEffort": "medium"},
            "instructionText": "Project Progenitor GUI role editor persistence test instructions.",
            "capabilities": ["git.status", "git.diff"],
            "policy": {"git.status": "allow", "git.diff": "allow"},
            "routing": {"mode": "direct", "defaultRecipient": "owner", "allowedRecipients": ["owner"], "reservedActions": []},
            "visibility": {"listed": true, "ownerVisible": true},
            "lifecycleAuthority": {"canSpawnAgents": false, "canArchiveAgents": false, "reservedActions": []}
        }))
        .expect("role editor draft");
        let update = tokio::time::timeout(
            Duration::from_secs(10),
            controller.dispatch(GuiOperationRequest::UpdateRoleFromDraft {
                role_id: "project-progenitor".to_string(),
                draft,
            }),
        )
        .await
        .expect("role update GUI operation timed out");
        assert!(matches!(update.outcome, GuiOperationOutcome::ProjectionUpdated { .. }), "update outcome: {:?}", update.outcome);

        let after_version: Uuid = sqlx::query_scalar("SELECT current_version_id FROM roles WHERE id='project-progenitor'")
            .fetch_one(&test_db.pool)
            .await
            .expect("after current version");
        assert_ne!(before_version, after_version);
        let after_count: i64 = sqlx::query_scalar("SELECT count(*) FROM role_versions WHERE role_id='project-progenitor'")
            .fetch_one(&test_db.pool)
            .await
            .expect("after role version count");
        assert_eq!(after_count, before_count + 1);
        let actor: String = sqlx::query_scalar("SELECT created_by FROM role_versions WHERE id=$1")
            .bind(after_version)
            .fetch_one(&test_db.pool)
            .await
            .expect("created_by");
        assert_eq!(actor, "gui-role-editor");
        let current_snapshot = db::current_role_snapshot(&test_db.pool, "project-progenitor").await.expect("current snapshot");
        assert_eq!(current_snapshot.role_version_id, after_version);
        assert_eq!(current_snapshot.capabilities, vec!["git.status".to_string(), "git.diff".to_string()]);
        assert_eq!(current_snapshot.policy.get("git.status"), Some(&crate::roles::ManifestDecision::Allow));
        assert_eq!(current_snapshot.policy.get("git.diff"), Some(&crate::roles::ManifestDecision::Allow));
        assert!(!current_snapshot.policy.contains_key("git.commit"));
        let versions = db::role_versions(&test_db.pool, "project-progenitor").await.expect("versions");
        assert!(versions.iter().any(|version| version["roleVersionId"] == after_version.to_string() && version["current"] == true && version["createdBy"] == "gui-role-editor"));
        let projected = controller
            .projection()
            .expect("projection after update")
            .roles
            .iter()
            .find(|role| role.id == "project-progenitor")
            .expect("projected role");
        let after_version_text = after_version.to_string();
        assert_eq!(projected.current_version_id.as_deref(), Some(after_version_text.as_str()));
        assert!(projected.capabilities.iter().any(|action| action == "git.status"));
        assert!(projected.policy.iter().any(|(action, decision)| action == "git.diff" && decision == "allow"));
        let reopened = crate::rinf_transport::AgentRuntimeWorkbenchViewModel::from_runtime_state(
            "http://role-gui-test",
            controller.projection(),
            controller.controller_state(),
            &[],
            0,
            None,
            &crate::rinf_transport::AgentRuntimeDiscoveryView::default(),
            &crate::rinf_transport::AgentRuntimeDiscoveryView::default(),
            &crate::rinf_transport::AgentRuntimeDiscoveryView::default(),
            &[],
        );
        let draft = reopened.role_admin.editor_draft.expect("reopened draft");
        assert_eq!(draft.role_id, "project-progenitor");
        assert!(draft.capabilities.iter().any(|action| action == "git.status"));
        assert!(draft.policy.iter().any(|row| row.action == "git.diff" && row.decision == "allow"));
        server_task.abort();
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn role_save_propagates_project_progenitor_git_status_to_existing_sessions() {
        let test_db = validation_db().await;
        let router = app(ServerState::new(test_db.pool.clone()));
        let mut stale = db::current_role_snapshot(&test_db.pool, "project-progenitor").await.expect("project progenitor role");
        stale.version = "stale-without-readonly-git".to_string();
        stale.role_version_id = Uuid::new_v4();
        stale.capabilities.retain(|action| action != "git.status" && action != "git.diff");
        stale.policy.remove("git.status");
        stale.policy.remove("git.diff");
        let session_id = db::new_session(&test_db.pool, &stale, Some("progenitor-live"), ".", Some("."), Some("Config Progenitor"), None)
            .await
            .expect("stale live session");
        let archived_session_id = db::new_session(&test_db.pool, &stale, Some("progenitor-archived"), ".", Some("."), Some("Archived Progenitor"), None)
            .await
            .expect("stale archived session");
        db::archive_session(&test_db.pool, archived_session_id).await.expect("archive stale session");
        let before = db::session_role_snapshot(&test_db.pool, session_id).await.expect("before role");
        assert_eq!(
            crate::policy::PolicyEngine::decide(&before, "git.status", json!({})).decision,
            crate::policy::RuntimeDecision::Deny
        );
        let model = FakeModelClient { direct_final_text: Some("ok"), ..Default::default() };
        crate::runtime::send_with_model_client(&test_db.pool, session_id, "first context", &model, compaction::CompactionBudget::default()).await.expect("first send");

        let draft: RoleEditorDraft = serde_json::from_value(json!({
            "id": "project-progenitor",
            "version": "99.99.100-propagation",
            "displayName": "Project Progenitor",
            "modelDefaults": {"model": "gpt-5.4-mini", "reasoningEffort": "medium"},
            "instructionText": "Project Progenitor propagation test instructions.",
            "capabilities": ["git.status", "git.diff"],
            "policy": {"git.status": "allow", "git.diff": "allow"},
            "routing": {"mode": "direct", "defaultRecipient": "owner", "allowedRecipients": ["owner"], "reservedActions": []},
            "visibility": {"listed": true, "ownerVisible": true},
            "lifecycleAuthority": {"canSpawnAgents": false, "canArchiveAgents": false, "reservedActions": []}
        }))
        .expect("draft");
        let (status, updated) = request_json(router, Method::POST, "/roles/project-progenitor/versions", serde_json::to_value(draft).expect("draft json")).await;
        assert_eq!(status, StatusCode::OK, "{updated}");
        let new_version = Uuid::parse_str(updated["versionId"].as_str().expect("version id")).expect("version uuid");
        let current_version: Uuid = sqlx::query_scalar("SELECT current_version_id FROM roles WHERE id='project-progenitor'")
            .fetch_one(&test_db.pool)
            .await
            .expect("current version");
        assert_eq!(current_version, new_version);

        let after = db::session_role_snapshot(&test_db.pool, session_id).await.expect("after role");
        assert_eq!(after.role_version_id, new_version);
        assert_eq!(
            crate::policy::PolicyEngine::decide(&after, "git.status", json!({})).decision,
            crate::policy::RuntimeDecision::Allow
        );
        assert_eq!(
            crate::policy::PolicyEngine::decide(&after, "git.diff", json!({"paths":[]})).decision,
            crate::policy::RuntimeDecision::Allow
        );
        assert_eq!(
            crate::policy::PolicyEngine::decide(&after, "git.commit", json!({"message":"nope"})).decision,
            crate::policy::RuntimeDecision::Deny
        );
        let archived_after = db::session_role_snapshot(&test_db.pool, archived_session_id).await.expect("archived after role");
        assert_eq!(archived_after.role_version_id, stale.role_version_id);
        assert_eq!(
            crate::policy::PolicyEngine::decide(&archived_after, "git.status", json!({})).decision,
            crate::policy::RuntimeDecision::Deny
        );
        let event: Value = sqlx::query_scalar("SELECT payload FROM event_stream WHERE session_id=$1 AND event_type='role_authority.changed' ORDER BY sequence DESC LIMIT 1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("role authority event");
        assert_eq!(event["actor"], "gui-role-editor");
        assert_eq!(event["previousRoleVersionId"], stale.role_version_id.to_string());
        assert_eq!(event["newRoleVersionId"], new_version.to_string());
        let added = event.pointer("/changedActionSummary/addedActions").and_then(Value::as_array).expect("added actions");
        assert!(added.iter().any(|action| action == "git.status"));
        assert!(added.iter().any(|action| action == "git.diff"));

        crate::runtime::send_with_model_client(&test_db.pool, session_id, "second context", &model, compaction::CompactionBudget::default()).await.expect("second send");
        let shape = model.observed_request_shapes.lock().expect("request shapes").last().cloned().expect("shape");
        let rendered = serde_json::to_string(&shape).expect("shape json");
        assert!(rendered.contains("<role_transition_summary"), "{rendered}");
        assert!(rendered.contains("Role authority changed"), "{rendered}");
        assert!(rendered.contains("git.status"), "{rendered}");
        assert!(rendered.contains("git.diff"), "{rendered}");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn role_save_deny_and_absent_immediately_block_existing_session_git_status() {
        let test_db = validation_db().await;
        let router = app(ServerState::new(test_db.pool.clone()));
        let mut allowed = db::current_role_snapshot(&test_db.pool, "project-progenitor").await.expect("project progenitor role");
        allowed.version = "live-deny-base".to_string();
        allowed.role_version_id = Uuid::new_v4();
        allowed.capabilities = vec!["git.status".to_string(), "git.diff".to_string()];
        allowed.policy = BTreeMap::from([
            ("git.status".to_string(), crate::roles::ManifestDecision::Allow),
            ("git.diff".to_string(), crate::roles::ManifestDecision::Allow),
        ]);
        let session_id = db::new_session(&test_db.pool, &allowed, Some("progenitor-deny"), ".", Some("."), Some("Config Progenitor"), None)
            .await
            .expect("allowed live session");
        assert_eq!(
            crate::policy::PolicyEngine::decide(&db::session_role_snapshot(&test_db.pool, session_id).await.expect("before"), "git.status", json!({})).decision,
            crate::policy::RuntimeDecision::Allow
        );

        let deny_draft: RoleEditorDraft = serde_json::from_value(json!({
            "id": "project-progenitor",
            "version": "99.99.101-deny-status",
            "displayName": "Project Progenitor",
            "modelDefaults": {"model": "gpt-5.4-mini", "reasoningEffort": "medium"},
            "instructionText": "Project Progenitor deny propagation test instructions.",
            "capabilities": ["git.status", "git.diff"],
            "policy": {"git.status": "deny", "git.diff": "allow"},
            "routing": {"mode": "direct", "defaultRecipient": "owner", "allowedRecipients": ["owner"], "reservedActions": []},
            "visibility": {"listed": true, "ownerVisible": true},
            "lifecycleAuthority": {"canSpawnAgents": false, "canArchiveAgents": false, "reservedActions": []}
        }))
        .expect("deny draft");
        let (deny_status, deny_updated) = request_json(router.clone(), Method::POST, "/roles/project-progenitor/versions", serde_json::to_value(deny_draft).expect("deny draft json")).await;
        assert_eq!(deny_status, StatusCode::OK, "{deny_updated}");
        let denied = db::session_role_snapshot(&test_db.pool, session_id).await.expect("denied role");
        assert_eq!(
            crate::policy::PolicyEngine::decide(&denied, "git.status", json!({})).decision,
            crate::policy::RuntimeDecision::Deny
        );
        assert_eq!(denied.policy.get("git.status"), Some(&crate::roles::ManifestDecision::Deny));

        let absent_draft: RoleEditorDraft = serde_json::from_value(json!({
            "id": "project-progenitor",
            "version": "99.99.102-absent-status",
            "displayName": "Project Progenitor",
            "modelDefaults": {"model": "gpt-5.4-mini", "reasoningEffort": "medium"},
            "instructionText": "Project Progenitor absent propagation test instructions.",
            "capabilities": ["git.diff"],
            "policy": {"git.diff": "allow"},
            "routing": {"mode": "direct", "defaultRecipient": "owner", "allowedRecipients": ["owner"], "reservedActions": []},
            "visibility": {"listed": true, "ownerVisible": true},
            "lifecycleAuthority": {"canSpawnAgents": false, "canArchiveAgents": false, "reservedActions": []}
        }))
        .expect("absent draft");
        let (absent_status, absent_updated) = request_json(router, Method::POST, "/roles/project-progenitor/versions", serde_json::to_value(absent_draft).expect("absent draft json")).await;
        assert_eq!(absent_status, StatusCode::OK, "{absent_updated}");
        let absent = db::session_role_snapshot(&test_db.pool, session_id).await.expect("absent role");
        assert_eq!(
            crate::policy::PolicyEngine::decide(&absent, "git.status", json!({})).decision,
            crate::policy::RuntimeDecision::Deny
        );
        assert!(!absent.capabilities.iter().any(|action| action == "git.status"));
        assert!(!absent.policy.contains_key("git.status"));
        assert_eq!(
            crate::policy::PolicyEngine::decide(&absent, "git.diff", json!({"paths":[]})).decision,
            crate::policy::RuntimeDecision::Allow
        );
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn seed_import_preserves_user_edited_current_role_and_does_not_propagate() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("seed-preserve"), ".", Some("."), None, None).await.expect("session");
        let gui_draft: RoleEditorDraft = serde_json::from_value(json!({
            "id": "runtime-no-rg",
            "version": "99.99.201-gui-preserved",
            "displayName": "Runtime No Rg",
            "modelDefaults": {"model": "gpt-5.4-mini", "reasoningEffort": "medium"},
            "instructionText": "GUI edited role authority.",
            "capabilities": ["tool.execute_code"],
            "policy": {"tool.execute_code": "allow"},
            "routing": {"mode": "direct", "defaultRecipient": null, "allowedRecipients": [], "reservedActions": []},
            "visibility": {"listed": true, "ownerVisible": true},
            "lifecycleAuthority": {"canSpawnAgents": false, "canArchiveAgents": false, "reservedActions": []}
        }))
        .expect("gui draft");
        let gui_import = crate::roles::imported_role_from_editor_draft(&gui_draft).expect("gui import");
        db::import_role_version_with_actor(&test_db.pool, &gui_import, "gui-role-editor").await.expect("gui import stored");
        let gui_current: Uuid = sqlx::query_scalar("SELECT current_version_id FROM roles WHERE id='runtime-no-rg'")
            .fetch_one(&test_db.pool)
            .await
            .expect("gui current");
        assert_eq!(gui_current, gui_import.snapshot.role_version_id);
        let events_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='role_authority.changed'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("events before seed");

        let seed_draft: RoleEditorDraft = serde_json::from_value(json!({
            "id": "runtime-no-rg",
            "version": "99.99.202-seed-should-not-clobber",
            "displayName": "Runtime No Rg",
            "modelDefaults": {"model": "gpt-5.4-mini", "reasoningEffort": "medium"},
            "instructionText": "Seed import must not replace GUI edits.",
            "capabilities": ["tool.execute_code"],
            "policy": {"tool.execute_code": "deny"},
            "routing": {"mode": "direct", "defaultRecipient": null, "allowedRecipients": [], "reservedActions": []},
            "visibility": {"listed": true, "ownerVisible": true},
            "lifecycleAuthority": {"canSpawnAgents": false, "canArchiveAgents": false, "reservedActions": []}
        }))
        .expect("seed draft");
        let seed_import = crate::roles::imported_role_from_editor_draft(&seed_draft).expect("seed import");
        db::import_role_version_with_actor(&test_db.pool, &seed_import, "seed-import").await.expect("seed import ignored");
        let current_after: Uuid = sqlx::query_scalar("SELECT current_version_id FROM roles WHERE id='runtime-no-rg'")
            .fetch_one(&test_db.pool)
            .await
            .expect("current after seed");
        assert_eq!(current_after, gui_current);
        let session_after = db::session_role_snapshot(&test_db.pool, session_id).await.expect("session after seed");
        assert_eq!(session_after.role_version_id, gui_current);
        assert_eq!(session_after.policy.get("tool.execute_code"), Some(&crate::roles::ManifestDecision::Allow));
        let events_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='role_authority.changed'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("events after seed");
        assert_eq!(events_after, events_before);
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
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status) VALUES ($1,$2,'print(\"memory\")','completed')")
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
            "session submit worker internal invariant violation",
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
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status) VALUES ($1,$2,'print(\"memory\")','completed')")
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

    #[tokio::test(flavor = "multi_thread")]
    async fn starter_kit_file_tree_image_and_tooling_request_are_audited_and_bounded() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-allow").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("starter-kit"), ".", Some("."), None, None).await.expect("session");
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("notes.txt"), "alpha\nbeta needle\ngamma\n").expect("write notes");
        std::fs::create_dir_all(temp.path().join("src")).expect("src");
        std::fs::write(temp.path().join("src/lib.rs"), "fn main() {}\n").expect("write rust");
        let png: [u8; 67] = [
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, b'I', b'H', b'D', b'R',
            0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89,
            0, 0, 0, 0x0a, b'I', b'D', b'A', b'T', 0x78, 0x9c, 0x63, 0, 1, 0, 0, 5, 0, 1, 0x0d, 0x0a, 0x2d, 0xb4,
            0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82,
        ];
        std::fs::write(temp.path().join("shot.png"), png).expect("write png");
        let root = starlark_host::ExecutionRoot::new(temp.path()).expect("root");
        let source = r#"
print(file.head("notes.txt", lines=2))
print(file.search("notes.txt", "needle", context=1))
print(tree.list(".", depth=2))
print(tree.find(".", name_glob="*.rs", type="file", max_results=5))
img = image.capture_from_file("shot.png", "capture starter-kit smoke screenshot artifact")
print(img)
print(image.describe(img.split('"imageArtifactId":"')[1].split('"')[0]))
print(tooling.request("Need starter kit helper", "Need a project-local helper to complete starter-kit validation without editing global skills.", attempted=["checked existing commands"], proposed="Add a project-local command bundle.", urgency="normal"))
"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, source, &root, &role).await.expect("execute starter file image tooling");
        let packet_value = serde_json::to_value(&packet).expect("packet value");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_file_audit_rows WHERE session_id=$1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("audit count");
        assert!(audit_count >= 4, "expected starter file/tree audit rows");
        let image_row: (Uuid, String, i64, Option<i32>, Option<i32>) = sqlx::query_as("SELECT id, mime_type, byte_count, width, height FROM starter_image_artifacts WHERE session_id=$1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("image row");
        assert_eq!(image_row.1, "image/png");
        assert!(image_row.2 > 0);
        assert_eq!(image_row.3, Some(1));
        assert_eq!(image_row.4, Some(1));
        let metadata = starlark_host::image_artifact_metadata(&test_db.pool, session_id, image_row.0).await.expect("image metadata");
        assert_eq!(metadata["mimeType"], "image/png");
        let thumbnail = starlark_host::image_artifact_thumbnail(&test_db.pool, session_id, image_row.0).await.expect("thumbnail");
        let full = starlark_host::image_artifact_full(&test_db.pool, session_id, image_row.0).await.expect("full image");
        assert_eq!(thumbnail, full);
        let router = app(ServerState::new(test_db.pool.clone()));
        let (json_status, image_json) = request_json(
            router.clone(),
            Method::GET,
            format!("/sessions/{session_id}/image-artifacts/{}/json", image_row.0).as_str(),
            Value::Null,
        )
        .await;
        assert_eq!(json_status, StatusCode::OK);
        assert_eq!(image_json["path"], format!("agent-runtime-image://{session_id}/{}", image_row.0));
        assert_eq!(image_json["contentType"], "image/png");
        assert_eq!(image_json["imageArtifactId"], image_row.0.to_string());
        assert_eq!(image_json["sessionId"], session_id.to_string());
        assert!(image_json["bytesBase64"].as_str().unwrap_or_default().starts_with("iVBORw0KGgo"));
        let attachment = starlark_host::image_artifact_model_attachment(&test_db.pool, session_id, image_row.0).await.expect("model attachment");
        assert_eq!(attachment["binaryInTranscript"], false);
        let request_input = crate::model_input::responses_input(
            &role,
            &[],
            &[crate::model::RuntimeInputMessage {
                text: "Review the attached image artifact.".to_string(),
                metadata: json!({"source":"requirements_visual_evidence","imageArtifactAttachments":[attachment.clone()]}),
            }],
            Some("review the screenshot"),
        );
        let request_shape = serde_json::to_value(&request_input).expect("request shape");
        assert!(serde_json::to_string(&request_shape).expect("request json").contains("\"type\":\"input_image\""));
        assert!(serde_json::to_string(&request_shape).expect("request json").contains(&image_row.0.to_string()));
        let other_role = db::current_role_snapshot(&test_db.pool, "runtime-allow").await.expect("role");
        let other_session = db::new_session(&test_db.pool, &other_role, Some("starter-other"), ".", Some("."), None, None).await.expect("other");
        assert!(starlark_host::image_artifact_full(&test_db.pool, other_session, image_row.0).await.is_err());
        let (other_status, _other_json) = request_json(
            router,
            Method::GET,
            format!("/sessions/{other_session}/image-artifacts/{}/json", image_row.0).as_str(),
            Value::Null,
        )
        .await;
        assert_eq!(other_status, StatusCode::NOT_FOUND);
        let request_status: String = sqlx::query_scalar("SELECT status FROM starter_tooling_requests WHERE session_id=$1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("tooling request");
        assert_eq!(request_status, "routed");
        let projected = projection::build_runtime_projection_snapshot(&test_db.pool, Some(session_id)).await.expect("projection");
        let selected = projected.selected_session.expect("selected session");
        assert_eq!(selected.project_runtime["activeToolBundleVersionIds"]["bundleVersion"], "starter-kit-1");
        assert!(projected.selected_chat_entries.iter().any(|entry| {
            entry.kind == "imageView"
                && entry.image_preview_content_type.as_deref() == Some("image/png")
                && entry.output == format!("agent-runtime-image://{session_id}/{}", image_row.0)
        }));
        assert_eq!(selected.tooling_requests.len(), 1);
        assert_eq!(selected.tooling_requests[0]["status"], "routed");
        test_db.cleanup().await;
    }

    #[test]
    fn screenshot_capture_contracts_use_image_artifact_storage_model() {
        let contracts = starlark_host::screenshot_capture_contracts();
        assert_eq!(contracts.pointer("/storageModel/table").and_then(Value::as_str), Some("starter_image_artifacts"));
        assert_eq!(contracts.pointer("/storageModel/binaryOutsideTranscript").and_then(Value::as_bool), Some(true));
        let tools = contracts["tools"].as_array().expect("tools");
        for expected in ["simulator.screenshot.capture", "browser.screenshot.capture", "design_lab.capture"] {
            let contract = tools.iter().find(|tool| tool["tool"] == expected).unwrap_or_else(|| panic!("missing screenshot capture contract: {expected}"));
            assert_eq!(contract.pointer("/output/imageArtifactId").and_then(Value::as_str), Some("uuid"));
            assert_eq!(contract.pointer("/output/mimeType").and_then(Value::as_str), Some("image/png"));
        }
        assert!(contracts.pointer("/reviewContract/requirementsEvidenceMustReference").and_then(Value::as_array).unwrap().contains(&json!("imageArtifactId")));
        assert_eq!(contracts.pointer("/reviewContract/modelAttachment").and_then(Value::as_str), Some("input_image from image artifact metadata; never local path-only evidence"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn starter_kit_full_project_session_smoke_projects_artifacts_and_packets() {
        let test_db = validation_db().await;
        db::create_project(&test_db.pool, "full-starter-smoke", "Full Starter Smoke", ".", ".", None, "gpt-5.4-mini").await.expect("project");
        let role = db::current_role_snapshot(&test_db.pool, "runtime-allow").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("full-starter-smoke"), ".", Some("."), None, None).await.expect("session");
        let temp = tempfile::tempdir().expect("tempdir");
        std::process::Command::new("git").arg("init").current_dir(temp.path()).output().expect("git init");
        std::process::Command::new("git").args(["config", "user.name", "Robert Sale"]).current_dir(temp.path()).output().expect("git name");
        std::process::Command::new("git").args(["config", "user.email", "robertmsale@icloud.com"]).current_dir(temp.path()).output().expect("git email");
        std::fs::write(temp.path().join("tracked.txt"), "original\n").expect("tracked");
        std::fs::write(temp.path().join("notes.txt"), "alpha\nneedle\nomega\n").expect("notes");
        std::process::Command::new("git").args(["add", "tracked.txt", "notes.txt"]).current_dir(temp.path()).output().expect("git add");
        std::process::Command::new("git").args(["commit", "-m", "initial"]).current_dir(temp.path()).output().expect("git commit");
        let png: &[u8] = &[
            137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,1,0,0,0,1,8,6,0,0,0,31,21,196,137,
            0,0,0,13,73,68,65,84,120,156,99,248,255,255,63,0,5,254,2,254,65,232,38,216,0,0,0,0,73,69,78,68,174,66,96,130
        ];
        std::fs::write(temp.path().join("shot.png"), png).expect("write png");
        let mut seed_value = admin_command_seed("cmd.starter.fullserver");
        seed_value["binaryName"] = json!("sleep");
        seed_value["candidatePaths"] = json!(["/bin/sleep", "/usr/bin/sleep"]);
        seed_value["starlarkObject"] = json!("starter_fullserver");
        seed_value["starlarkMethod"] = json!("run");
        seed_value["argvPrefix"] = json!(["30"]);
        seed_value["allowArgsArg"] = json!(false);
        seed_value["asyncAllowed"] = json!(true);
        seed_value["endOfTurnBehavior"] = json!("continue");
        let seed: command_registry::CommandSeed = serde_json::from_value(seed_value).expect("seed");
        apply_registry_seed(&test_db.pool, session_id, seed, command_registry::RegistryScope { scope_type: "project".to_string(), project_key: Some("full-starter-smoke".to_string()) }).await;
        let root = starlark_host::ExecutionRoot::new(temp.path()).expect("root");
        let source = r#"
print(file.head("notes.txt", lines=2))
print(file.search("notes.txt", "needle", context=1))
print(tree.list(".", depth=1))
print(patch.apply("--- a/tracked.txt\n+++ b/tracked.txt\n@@ -1 +1 @@\n-original\n+patched\n", "apply described starter smoke patch"))
print(git.restore(paths=["tracked.txt"]))
img = image.capture_from_file("shot.png", "import screenshot artifact for full starter smoke")
print(image.describe(img.split('"imageArtifactId":"')[1].split('"')[0]))
print(tooling.request("Need smoke helper", "Need a project-local follow-on helper routed as a typed packet.", attempted=["ran file tree git server image tools"], proposed='{"kind":"command_registry","operation":"add","summary":"Add smoke helper"}', urgency="normal"))
print(server.start("cmd.starter.fullserver", [], name="fullsmoke"))
print(server.wait_ready("fullsmoke", timeout_ms=500))
print(server.logs("fullsmoke", stream="stdout", lines=5))
print(server.stop("fullsmoke"))
"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, source, &root, &role).await.expect("full starter smoke");
        let packet_value = serde_json::to_value(&packet).expect("packet value");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        assert_eq!(std::fs::read_to_string(temp.path().join("tracked.txt")).expect("restored tracked"), "original\n");
        let audit_ops: Vec<String> = sqlx::query_scalar("SELECT operation FROM starter_file_audit_rows WHERE session_id=$1 ORDER BY created_at")
            .bind(session_id)
            .fetch_all(&test_db.pool)
            .await
            .expect("audit ops");
        for expected in ["file.head", "file.search", "tree.list", "git.restore"] {
            assert!(audit_ops.iter().any(|op| op == expected), "missing audit op {expected}: {audit_ops:?}");
        }
        let patch_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM patch_runs WHERE action_name='patch.apply' AND mutation_description='apply described starter smoke patch'")
            .fetch_one(&test_db.pool)
            .await
            .expect("patch rows");
        assert_eq!(patch_rows, 1);
        let image_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_image_artifacts WHERE session_id=$1 AND mime_type='image/png'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("image count");
        assert_eq!(image_count, 1);
        let tooling_packets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_packets WHERE source_session_id=$1 AND packet_type IN ('tooling.request','command_registry.follow_on_request')")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("tooling packets");
        assert_eq!(tooling_packets, 2);
        let released: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_port_leases WHERE session_id=$1 AND status='released' AND release_reason='server.stop'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("released server port");
        assert_eq!(released, 1);
        let projected = projection::build_runtime_projection_snapshot(&test_db.pool, Some(session_id)).await.expect("projection");
        let selected = projected.selected_session.expect("selected session");
        assert!(projected.selected_chat_entries.iter().any(|entry| entry.kind == "imageView"));
        assert_eq!(selected.tooling_requests.len(), 1);
        assert!(selected.running_servers.iter().any(|server| server["handle"] == "fullsmoke" && server["status"] == "stopped"));
        let output_artifacts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_output_artifacts WHERE session_id=$1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("output artifacts");
        assert!(output_artifacts >= 2);
        let global_command_edits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM command_definitions WHERE scope_type='global' AND action_id='cmd.starter.fullserver'")
            .fetch_one(&test_db.pool)
            .await
            .expect("global command edits");
        assert_eq!(global_command_edits, 0);
        test_db.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn starter_kit_safe_git_and_server_port_manager_reject_unsafe_paths_and_ports() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-allow").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("starter-git"), ".", Some("."), None, None).await.expect("session");
        let temp = tempfile::tempdir().expect("tempdir");
        std::process::Command::new("git").arg("init").current_dir(temp.path()).output().expect("git init");
        std::process::Command::new("git").args(["config", "user.name", "Robert Sale"]).current_dir(temp.path()).output().expect("git name");
        std::process::Command::new("git").args(["config", "user.email", "robertmsale@icloud.com"]).current_dir(temp.path()).output().expect("git email");
        std::fs::write(temp.path().join("tracked.txt"), "one\n").expect("tracked");
        std::process::Command::new("git").args(["add", "tracked.txt"]).current_dir(temp.path()).output().expect("git add initial");
        std::process::Command::new("git").args(["commit", "-m", "initial"]).current_dir(temp.path()).output().expect("git commit initial");
        std::fs::write(temp.path().join("tracked.txt"), "dirty\n").expect("dirty");
        let mut seed_value = admin_command_seed("cmd.starter.server");
        seed_value["binaryName"] = json!("sleep");
        seed_value["candidatePaths"] = json!(["/bin/sleep", "/usr/bin/sleep"]);
        seed_value["argvPrefix"] = json!(["30"]);
        seed_value["allowArgsArg"] = json!(false);
        seed_value["asyncAllowed"] = json!(true);
        seed_value["endOfTurnBehavior"] = json!("continue");
        let seed: command_registry::CommandSeed = serde_json::from_value(seed_value).expect("seed");
        apply_registry_seed(&test_db.pool, session_id, seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;
        let root = starlark_host::ExecutionRoot::new(temp.path()).expect("root");
        let source = r#"
print(git.status())
print(git.restore(paths=["tracked.txt"]))
print(file.replace_exact("tracked.txt", "one", "two", "change tracked text for starter-kit git proof"))
print(git.add(paths=["tracked.txt"]))
print(git.commit("starter kit git commit", paths=["tracked.txt"]))
srv = server.start("cmd.starter.server", [], name="starter")
print(srv)
print(server.wait_ready("starter", timeout_ms=500))
print(server.logs("starter", stream="stdout", lines=5))
print(server.status("starter"))
print(server.stop("starter"))
"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, source, &root, &role).await.expect("execute git/server");
        let packet_value = serde_json::to_value(&packet).expect("packet value");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        assert_eq!(std::fs::read_to_string(temp.path().join("tracked.txt")).expect("tracked after commit"), "two\n");
        let restore_audit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_file_audit_rows WHERE session_id=$1 AND operation='git.restore' AND status='completed'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("restore audit");
        assert_eq!(restore_audit, 1);
        let commit_summary: Value = sqlx::query_scalar("SELECT truncation FROM starter_file_audit_rows WHERE session_id=$1 AND operation='git.commit.summary' AND status='completed'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("commit summary audit");
        assert!(commit_summary["commitHash"].as_str().unwrap_or_default().len() >= 7, "commit hash must be recorded: {commit_summary}");
        assert!(commit_summary["parentHash"].as_str().unwrap_or_default().len() >= 7, "parent hash must be recorded: {commit_summary}");
        assert_eq!(commit_summary["changedPaths"][0], "tracked.txt");
        let released: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_port_leases WHERE session_id=$1 AND status='released' AND release_reason='server.stop'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("released lease");
        assert_eq!(released, 1);
        let projected = projection::build_runtime_projection_snapshot(&test_db.pool, Some(session_id)).await.expect("projection");
        let selected = projected.selected_session.expect("selected session");
        assert_eq!(selected.running_servers.len(), 1);
        assert_eq!(selected.running_servers[0]["handle"], "starter");
        assert_eq!(selected.running_servers[0]["readiness"]["mode"], "processAlive");
        assert_eq!(selected.running_servers[0]["actions"][2], "stop");
        let linked_process_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_managed_servers WHERE session_id=$1 AND handle='starter' AND process_id IS NOT NULL")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("linked starter process id");
        assert_eq!(linked_process_count, 1, "starter managed server row must link to the persisted managed process id");
        let output_artifacts: Value = sqlx::query_scalar("SELECT output_artifacts FROM starter_managed_servers WHERE session_id=$1 AND handle='starter'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("starter server output artifacts");
        assert!(output_artifacts["stdoutLogArtifactId"].as_str().unwrap_or_default().len() >= 7, "server logs must persist output artifact references: {output_artifacts}");
        let unsafe_source = r#"print(file.head("../outside.txt", lines=1))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, unsafe_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, unsafe_source, &root, &role).await.expect("unsafe path packet");
        let packet_value = serde_json::to_value(&packet).expect("packet value");
        assert_eq!(packet_value["ok"], false, "{packet_value}");
        let port_source = r#"print(server.start("cmd.starter.server", ["--port", "9999"], name="bad"))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, port_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, port_source, &root, &role).await.expect("port override packet");
        let packet_value = serde_json::to_value(&packet).expect("packet value");
        assert_eq!(packet_value["ok"], false, "{packet_value}");
        for override_source in [
            r#"print(server.start("cmd.starter.server", ["--port=9999"], name="bad_eq"))"#,
            r#"print(server.start("cmd.starter.server", ["PORT=9999"], name="bad_env"))"#,
            r#"print(server.start("cmd.starter.server", ["-p", "9999"], name="bad_short"))"#,
            r#"print(server.start("cmd.starter.server", ["port=9999"], name="bad_plain"))"#,
            r#"print(server.start("cmd.starter.server", ["--host=127.0.0.1"], name="bad_host"))"#,
        ] {
            let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, override_source).await;
            let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, override_source, &root, &role).await.expect("port override packet");
            let packet_value = serde_json::to_value(&packet).expect("port override value");
            assert_eq!(packet_value["ok"], false, "port override unexpectedly passed: {override_source}\n{packet_value}");
        }
        let arbitrary_action = r#"print(server.start("cmd.not.registered", [], name="bad_action"))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, arbitrary_action).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, arbitrary_action, &root, &role).await.expect("arbitrary action packet");
        let packet_value = serde_json::to_value(&packet).expect("arbitrary action value");
        assert_eq!(packet_value["ok"], false, "{packet_value}");
        insert_starter_server_fixture(&test_db.pool, session_id, "external", 39109).await;
        let adopt_source = r#"print(server.start("cmd.starter.server", [], name="external"))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, adopt_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, adopt_source, &root, &role).await.expect("adoption rejection packet");
        let packet_value = serde_json::to_value(&packet).expect("adoption value");
        assert_eq!(packet_value["ok"], false, "{packet_value}");
        let startup_fail = r#"print(server.start("cmd.starter.server", ["unexpected-arg"], name="badspawn"))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, startup_fail).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, startup_fail, &root, &role).await.expect("startup failure packet");
        let packet_value = serde_json::to_value(&packet).expect("startup failure value");
        assert_eq!(packet_value["ok"], false, "{packet_value}");
        let startup_released: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_port_leases WHERE session_id=$1 AND status='released' AND release_reason='startupFailure'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("startup failure release");
        assert_eq!(startup_released, 1);
        let duplicate_source = r#"
print(server.start("cmd.starter.server", [], name="duper"))
print(server.start("cmd.starter.server", [], name="duper"))
"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, duplicate_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, duplicate_source, &root, &role).await.expect("duplicate handle packet");
        let packet_value = serde_json::to_value(&packet).expect("duplicate handle value");
        assert_eq!(packet_value["ok"], false, "{packet_value}");
        let stop_duper = r#"print(server.stop("duper"))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, stop_duper).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, stop_duper, &root, &role).await.expect("stop duper");
        let packet_value = serde_json::to_value(&packet).expect("stop duper value");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        for bad_git in [
            r#"print(git.restore(paths=[]))"#,
            r#"print(git.restore(paths=["."]))"#,
            r#"print(git.add(paths=[".git/config"]))"#,
            r#"print(git.commit("empty starter commit", paths=["tracked.txt"]))"#,
        ] {
            let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, bad_git).await;
            let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, bad_git, &root, &role).await.expect("bad git packet");
            let packet_value = serde_json::to_value(&packet).expect("bad git value");
            assert_eq!(packet_value["ok"], false, "bad git unexpectedly passed: {bad_git}\n{packet_value}");
        }
        test_db.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn orchestrator_git_integration_helpers_are_role_scoped_and_operational() {
        let test_db = validation_db().await;
        let mut orchestrator = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        orchestrator.id = "orchestrator".to_string();
        for action in [
            "git.inspect_worker_branch",
            "git.rebase_worker_branch",
            "git.fast_forward_local_main",
            "git.cleanup_integrated_worktree",
        ] {
            orchestrator.policy.insert(action.to_string(), crate::roles::ManifestDecision::Allow);
        }
        let session_id = db::new_session(&test_db.pool, &orchestrator, Some("orchestrator-git"), ".", Some("."), None, None).await.expect("session");
        let temp = tempfile::tempdir().expect("tempdir");
        std::process::Command::new("git").arg("init").current_dir(temp.path()).output().expect("git init");
        std::process::Command::new("git").args(["config", "user.name", "Robert Sale"]).current_dir(temp.path()).output().expect("git name");
        std::process::Command::new("git").args(["config", "user.email", "robertmsale@icloud.com"]).current_dir(temp.path()).output().expect("git email");
        std::fs::write(temp.path().join("tracked.txt"), "main\n").expect("tracked");
        std::process::Command::new("git").args(["add", "tracked.txt"]).current_dir(temp.path()).output().expect("git add");
        std::process::Command::new("git").args(["commit", "-m", "initial"]).current_dir(temp.path()).output().expect("git commit");
        std::process::Command::new("git").args(["branch", "-M", "main"]).current_dir(temp.path()).output().expect("main branch");
        std::process::Command::new("git").args(["checkout", "-b", "worker"]).current_dir(temp.path()).output().expect("worker branch");
        std::fs::write(temp.path().join("tracked.txt"), "worker\n").expect("worker edit");
        std::process::Command::new("git").args(["commit", "-am", "worker change"]).current_dir(temp.path()).output().expect("worker commit");
        std::process::Command::new("git").args(["checkout", "main"]).current_dir(temp.path()).output().expect("checkout main");
        std::process::Command::new("git").args(["branch", "integrated", "worker"]).current_dir(temp.path()).output().expect("integrated branch");
        let worktree_output = std::process::Command::new("git").args(["worktree", "add", "integrated-worktree", "integrated"]).current_dir(temp.path()).output().expect("worktree add");
        assert!(worktree_output.status.success(), "worktree add failed: {}", String::from_utf8_lossy(&worktree_output.stderr));
        let root = starlark_host::ExecutionRoot::new(temp.path()).expect("root");
        let source = r#"
print(git.inspect_worker_branch("worker", local_main="main"))
print(git.rebase_worker_branch("worker", local_main="main"))
print(git.fast_forward_local_main("worker", local_main="main"))
print(git.cleanup_integrated_worktree("integrated-worktree"))
"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, source, &root, &orchestrator).await.expect("orchestrator git helpers");
        let packet_value = serde_json::to_value(&packet).expect("packet");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        let main_head = std::process::Command::new("git").args(["rev-parse", "main"]).current_dir(temp.path()).output().expect("main head");
        let worker_head = std::process::Command::new("git").args(["rev-parse", "worker"]).current_dir(temp.path()).output().expect("worker head");
        assert_eq!(String::from_utf8_lossy(&main_head.stdout), String::from_utf8_lossy(&worker_head.stdout));
        assert!(!temp.path().join("integrated-worktree").exists(), "cleanup helper must remove integrated worktree");

        let worker_role = db::current_role_snapshot(&test_db.pool, "runtime-allow").await.expect("worker role");
        let worker_session = db::new_session(&test_db.pool, &worker_role, Some("orchestrator-git"), ".", Some("."), None, None).await.expect("worker session");
        let denied = r#"print(git.inspect_worker_branch("worker", local_main="main"))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, worker_session, denied).await;
        let packet = starlark_host::execute_code(&test_db.pool, worker_session, turn_id, tool_call_id, denied, &root, &worker_role).await.expect("worker denied");
        let packet_value = serde_json::to_value(&packet).expect("denied packet");
        assert_eq!(packet_value["ok"], false, "{packet_value}");
        test_db.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn starter_kit_server_port_leases_release_on_archive() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-allow").await.expect("role");
        let archive_session = db::new_session(&test_db.pool, &role, Some("starter-server-archive"), ".", Some("."), None, None).await.expect("archive session");
        insert_starter_server_fixture(&test_db.pool, archive_session, "archive-fixture", 39101).await;
        db::archive_session(&test_db.pool, archive_session).await.expect("archive session");
        let lifecycle_release: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_port_leases WHERE session_id=$1 AND status='released' AND release_reason='session.archive'")
            .bind(archive_session)
            .fetch_one(&test_db.pool)
            .await
            .expect("archive release");
        assert_eq!(lifecycle_release, 1);

        let archive_session = db::new_session(&test_db.pool, &role, Some("starter-server-archive"), ".", Some("."), None, None).await.expect("archive session");
        insert_starter_server_fixture(&test_db.pool, archive_session, "archive-fixture", 39102).await;
        db::archive_session(&test_db.pool, archive_session).await.expect("archive session");
        let archive_release: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_port_leases WHERE session_id=$1 AND status='released' AND release_reason='session.archive'")
            .bind(archive_session)
            .fetch_one(&test_db.pool)
            .await
            .expect("archive release");
        assert_eq!(archive_release, 1);
        test_db.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn starter_kit_server_readiness_supports_http_get_log_pattern_and_timeout() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-allow").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("starter-readiness"), ".", Some("."), None, None).await.expect("session");
        let temp = tempfile::tempdir().expect("tempdir");
        let root = starlark_host::ExecutionRoot::new(temp.path()).expect("root");

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("http readiness listener");
        let port = listener.local_addr().expect("listener addr").port() as i32;
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 256];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                let _ = std::io::Write::write_all(&mut stream, b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK");
            }
        });
        sqlx::query("INSERT INTO starter_managed_servers (id, session_id, handle, cwd, env_overlay_metadata, port, url, readiness_config, status) VALUES ($1,$2,'http-fixture','.','{}'::jsonb,$3,$4,$5,'running')")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .bind(port)
            .bind(format!("http://127.0.0.1:{port}"))
            .bind(json!({"mode":"httpGet","path":"/ready"}))
            .execute(&test_db.pool)
            .await
            .expect("insert http readiness fixture");
        let http_source = r#"print(server.wait_ready("http-fixture", timeout_ms=1000))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, http_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, http_source, &root, &role).await.expect("http wait");
        let packet_value = serde_json::to_value(&packet).expect("packet");
        assert_eq!(packet_value["ok"], true, "{packet_value}");

        let mut seed_value = admin_command_seed("cmd.starter.logready");
        seed_value["binaryName"] = json!("sh");
        seed_value["candidatePaths"] = json!(["/bin/sh", "/usr/bin/sh"]);
        seed_value["starlarkObject"] = json!("starter_logready");
        seed_value["starlarkMethod"] = json!("run");
        seed_value["argvPrefix"] = json!(["-c", "echo READY; sleep 30"]);
        seed_value["allowArgsArg"] = json!(false);
        seed_value["asyncAllowed"] = json!(true);
        seed_value["endOfTurnBehavior"] = json!("continue");
        let seed: command_registry::CommandSeed = serde_json::from_value(seed_value).expect("seed");
        apply_registry_seed(&test_db.pool, session_id, seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;
        let start_source = r#"print(server.start("cmd.starter.logready", [], name="logready"))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, start_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, start_source, &root, &role).await.expect("start logready");
        let packet_value = serde_json::to_value(&packet).expect("packet");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        sqlx::query("UPDATE starter_managed_servers SET readiness_config=$3 WHERE session_id=$1 AND handle=$2")
            .bind(session_id)
            .bind("logready")
            .bind(json!({"mode":"logPattern","pattern":"READY"}))
            .execute(&test_db.pool)
            .await
            .expect("update log readiness");
        let log_source = r#"print(server.wait_ready("logready", timeout_ms=1000))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, log_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, log_source, &root, &role).await.expect("log wait");
        let packet_value = serde_json::to_value(&packet).expect("packet");
        assert_eq!(packet_value["ok"], true, "{packet_value}");

        sqlx::query("INSERT INTO starter_managed_servers (id, session_id, handle, cwd, env_overlay_metadata, port, url, readiness_config, status) VALUES ($1,$2,'timeout-fixture','.','{}'::jsonb,9,'http://127.0.0.1:9',$3,'running')")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .bind(json!({"mode":"httpGet","path":"/missing"}))
            .execute(&test_db.pool)
            .await
            .expect("insert timeout readiness fixture");
        let timeout_source = r#"print(server.wait_ready("timeout-fixture", timeout_ms=20))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, timeout_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, timeout_source, &root, &role).await.expect("timeout wait");
        let packet_value = serde_json::to_value(&packet).expect("packet");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        let output_text: String = sqlx::query_scalar("SELECT content FROM execution_output_artifacts WHERE session_id=$1 AND stream='stdout' ORDER BY created_at DESC LIMIT 1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("timeout stdout artifact");
        assert!(output_text.contains("\"ready\":false"), "{output_text}");
        assert!(output_text.contains("\"kind\":\"timeout\""), "{output_text}");
        let mut timeout_seed_value = admin_command_seed("cmd.starter.timeoutrelease");
        timeout_seed_value["binaryName"] = json!("sleep");
        timeout_seed_value["candidatePaths"] = json!(["/bin/sleep", "/usr/bin/sleep"]);
        timeout_seed_value["starlarkObject"] = json!("starter_timeoutrelease");
        timeout_seed_value["starlarkMethod"] = json!("run");
        timeout_seed_value["argvPrefix"] = json!(["30"]);
        timeout_seed_value["allowArgsArg"] = json!(false);
        timeout_seed_value["asyncAllowed"] = json!(true);
        timeout_seed_value["endOfTurnBehavior"] = json!("continue");
        let timeout_seed: command_registry::CommandSeed = serde_json::from_value(timeout_seed_value).expect("timeout seed");
        apply_registry_seed(&test_db.pool, session_id, timeout_seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;
        let timeout_start = r#"print(server.start("cmd.starter.timeoutrelease", [], name="timeout-release"))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, timeout_start).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, timeout_start, &root, &role).await.expect("start timeout release");
        let packet_value = serde_json::to_value(&packet).expect("timeout start packet");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        sqlx::query("UPDATE starter_managed_servers SET readiness_config=$3 WHERE session_id=$1 AND handle=$2")
            .bind(session_id)
            .bind("timeout-release")
            .bind(json!({"mode":"httpGet","path":"/never-ready"}))
            .execute(&test_db.pool)
            .await
            .expect("update timeout readiness");
        let timeout_release_source = r#"print(server.wait_ready("timeout-release", timeout_ms=20))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, timeout_release_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, timeout_release_source, &root, &role).await.expect("wait timeout release");
        let packet_value = serde_json::to_value(&packet).expect("timeout release packet");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        let readiness_released: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_port_leases WHERE session_id=$1 AND status='released' AND release_reason='readiness.timeout'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("readiness release");
        assert_eq!(readiness_released, 1);
        let stop_source = r#"print(server.stop("logready"))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, stop_source).await;
        starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, stop_source, &root, &role).await.expect("stop logready");

        test_db.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn starter_kit_server_reconciliation_releases_lost_runtime_port_leases() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-allow").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("starter-reconcile"), ".", Some("."), None, None).await.expect("session");
        insert_starter_server_fixture(&test_db.pool, session_id, "lost-fixture", 39201).await;
        let released = starlark_host::reconcile_starter_server_leases(&test_db.pool).await.expect("reconcile starter servers");
        assert_eq!(released, 1);
        let release_reason: String = sqlx::query_scalar("SELECT release_reason FROM starter_port_leases WHERE session_id=$1 AND allocated_port=39201")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("release reason");
        assert_eq!(release_reason, "runtime.reconcile");
        let server_status: String = sqlx::query_scalar("SELECT status FROM starter_managed_servers WHERE session_id=$1 AND handle='lost-fixture'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("server status");
        assert_eq!(server_status, "lost");
        test_db.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn starter_kit_port_leases_are_unique_and_release_on_process_exit() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-allow").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("starter-port-exit"), ".", Some("."), None, None).await.expect("session");
        sqlx::query("INSERT INTO starter_port_leases (id, project_key, session_id, allocated_port, status, lease_reason) VALUES ($1,'starter-port-exit',$2,39301,'active','contention-proof')")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("first active port lease");
        let duplicate = sqlx::query("INSERT INTO starter_port_leases (id, project_key, session_id, allocated_port, status, lease_reason) VALUES ($1,'starter-port-exit',$2,39301,'active','contention-duplicate')")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .execute(&test_db.pool)
            .await;
        assert!(duplicate.is_err(), "active allocated_port uniqueness must reject contention");
        sqlx::query("UPDATE starter_port_leases SET status='released', released_at=now(), release_reason='contention.test.complete' WHERE session_id=$1 AND allocated_port=39301")
            .bind(session_id)
            .execute(&test_db.pool)
            .await
            .expect("release contention fixture");

        let temp = tempfile::tempdir().expect("tempdir");
        let root = starlark_host::ExecutionRoot::new(temp.path()).expect("root");
        let mut seed_value = admin_command_seed("cmd.starter.exits");
        seed_value["binaryName"] = json!("sleep");
        seed_value["candidatePaths"] = json!(["/bin/sleep", "/usr/bin/sleep"]);
        seed_value["starlarkObject"] = json!("starter_exits");
        seed_value["starlarkMethod"] = json!("run");
        seed_value["argvPrefix"] = json!(["1"]);
        seed_value["allowArgsArg"] = json!(false);
        seed_value["asyncAllowed"] = json!(true);
        seed_value["endOfTurnBehavior"] = json!("continue");
        let seed: command_registry::CommandSeed = serde_json::from_value(seed_value).expect("exit seed");
        apply_registry_seed(&test_db.pool, session_id, seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;
        let start_source = r#"print(server.start("cmd.starter.exits", [], name="exits"))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, start_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, start_source, &root, &role).await.expect("start short server");
        let packet_value = serde_json::to_value(&packet).expect("start packet");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        std::thread::sleep(std::time::Duration::from_millis(1250));
        let status_source = r#"print(server.status("exits"))"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, status_source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, status_source, &root, &role).await.expect("status short server");
        let packet_value = serde_json::to_value(&packet).expect("status packet");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        let release_reason: String = sqlx::query_scalar("SELECT release_reason FROM starter_port_leases WHERE session_id=$1 AND lease_reason='server.start' ORDER BY created_at DESC LIMIT 1")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("process exit release reason");
        assert_eq!(release_reason, "process.exit");
        let server_status: String = sqlx::query_scalar("SELECT status FROM starter_managed_servers WHERE session_id=$1 AND handle='exits'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("exited server status");
        assert_eq!(server_status, "completed");
        test_db.cleanup().await;
    }

    async fn insert_starter_server_fixture(pool: &PgPool, session_id: Uuid, handle: &str, port: i32) {
        sqlx::query("INSERT INTO starter_port_leases (id, project_key, session_id, allocated_port, status, lease_reason) VALUES ($1,'starter-fixture',$2,$3,'active','test-fixture')")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .bind(port)
            .execute(pool)
            .await
            .expect("insert starter port lease");
        sqlx::query("INSERT INTO starter_managed_servers (id, session_id, handle, cwd, env_overlay_metadata, port, url, readiness_config, status) VALUES ($1,$2,$3,'.','{}'::jsonb,$4,$5,'{\"mode\":\"processAlive\"}'::jsonb,'running')")
            .bind(Uuid::new_v4())
            .bind(session_id)
            .bind(handle)
            .bind(port)
            .bind(format!("http://127.0.0.1:{port}"))
            .execute(pool)
            .await
            .expect("insert starter server row");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn starter_kit_path_resolution_rejects_escapes_git_binary_and_records_bounds() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-allow").await.expect("role");
        let session_id = db::new_session(&test_db.pool, &role, Some("starter-paths"), ".", Some("."), None, None).await.expect("session");
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("text.txt"), (1..=800).map(|i| format!("line-{i}\n")).collect::<String>()).expect("text");
        std::fs::create_dir_all(temp.path().join(".git")).expect("git dir");
        std::fs::write(temp.path().join(".git/config"), "secret").expect("git config");
        std::fs::write(temp.path().join("bin.dat"), b"\0binary").expect("binary");
        let outside = tempfile::NamedTempFile::new().expect("outside");
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), temp.path().join("escape-link")).expect("symlink");
        let root = starlark_host::ExecutionRoot::new(temp.path()).expect("root");
        let absolute = temp.path().join("text.txt").display().to_string();
        let source = format!(r#"
print(file.head("text.txt", lines=3))
print(file.tail({absolute:?}, lines=3))
print(file.read_lines("text.txt", 10, 12))
print(file.line_count("text.txt"))
print(file.search("text.txt", "line-77", context=1))
print(tree.list(".", depth=1))
print("tree:", tree.list(".", depth=2))
print(tree.find(".", name_glob="*.txt", type="file", max_results=10))
"#);
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, &source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, &source, &root, &role).await.expect("path success");
        let packet_value = serde_json::to_value(&packet).expect("packet value");
        assert_eq!(packet_value["ok"], true, "{packet_value}");
        let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_file_audit_rows WHERE session_id=$1 AND status='completed'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("audit count");
        assert!(audit_count >= 8);
        let read_artifacts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_output_artifacts WHERE session_id=$1 AND source_type='starter_file_tree_read'")
            .bind(session_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("starter read artifact count");
        assert!(read_artifacts >= 1, "truncated/bounded file-tree reads must spill to durable output artifacts");
        for bad in [
            r#"print(file.head("../outside", lines=1))"#,
            r#"print(file.head(".git/config", lines=1))"#,
            r#"print(file.head("bin.dat", lines=1))"#,
            r#"print(file.read_lines("text.txt", 9000, 9001))"#,
            r#"print(tree.find(".", max_results=10))"#,
            r#"print(file.head("escape-link", lines=1))"#,
        ] {
            let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, bad).await;
            let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, bad, &root, &role).await.expect("bad packet");
            let value = serde_json::to_value(&packet).expect("packet value");
            assert_eq!(value["ok"], false, "bad script unexpectedly passed: {bad}\n{value}");
        }
        test_db.cleanup().await;
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
        let project_source = "print(cmd[\"project_cache\"].run.describe())";
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
        assert!(after_value["output"]["stdoutArtifact"]["preview"].as_str().unwrap_or_default().contains("cmd.cache.project") || after_value["output"]["stdoutArtifact"]["tail"].as_str().unwrap_or_default().contains("cmd.cache.project"));

        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, beta, project_source).await;
        let non_visible = starlark_host::execute_code(&test_db.pool, beta, turn_id, tool_call_id, project_source, &root, &role).await.expect("non-visible failed packet");
        let non_visible_value = serde_json::to_value(non_visible).expect("non-visible packet");
        assert_eq!(non_visible_value["status"], "failed");
        assert!(non_visible_value["output"]["stderrArtifact"]["preview"].as_str().unwrap_or_default().contains("project_cache") || non_visible_value["output"]["stderrArtifact"]["tail"].as_str().unwrap_or_default().contains("project_cache"));

        let global_source = "print(cmd[\"global_cache\"].run.describe())";
        let global_seed = scoped_command_seed("cmd.cache.global", "global_cache");
        apply_registry_seed(&test_db.pool, alpha, global_seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, beta, global_source).await;
        let global = starlark_host::execute_code(&test_db.pool, beta, turn_id, tool_call_id, global_source, &root, &role).await.expect("execute global command");
        let global_value = serde_json::to_value(global).expect("global packet");
        assert_eq!(global_value["status"], "completed");
        assert!(global_value["output"]["stdoutArtifact"]["preview"].as_str().unwrap_or_default().contains("cmd.cache.global") || global_value["output"]["stdoutArtifact"]["tail"].as_str().unwrap_or_default().contains("cmd.cache.global"));
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
        let source = format!("print({})", serde_json::to_string(&large).expect("source string"));
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, &source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, &source, &root, &role).await.expect("execute large output");
        let value = serde_json::to_value(packet).expect("packet");
        let artifact_id = Uuid::parse_str(value["output"]["stdoutArtifact"]["artifactId"].as_str().expect("artifact id")).expect("artifact uuid");
        assert!(value["output"]["stdoutArtifact"]["truncated"].as_bool().unwrap_or(false));
        assert!(!value.to_string().contains(&large));

        let row = sqlx::query("SELECT content, byte_count, line_count FROM execution_output_artifacts WHERE id=$1")
            .bind(artifact_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("artifact row");
        assert_eq!(row.get::<String, _>("content"), large);
        assert_eq!(row.get::<i64, _>("byte_count") as usize, large.len());
        assert_eq!(row.get::<i64, _>("line_count"), 900);
        let script_combined_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_output_artifacts WHERE turn_id=$1 AND stream='combined'")
            .bind(turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("no script combined artifacts");
        assert_eq!(script_combined_count, 0);

        let retrieval_source = r#"
artifact = outputs.last()
print(outputs.head(artifact, lines=3))
print(outputs.tail(artifact, lines=4))
print(outputs.slice(artifact, start_line=500, end_line=650))
print(outputs.search(artifact, "needle-output-artifact", context=2))
print(outputs.stats(artifact))
"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, retrieval_source).await;
        let retrieval = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, retrieval_source, &root, &role).await.expect("retrieve artifact");
        let retrieval_value = serde_json::to_value(retrieval).expect("retrieval packet");
        let retrieval_artifact_id = Uuid::parse_str(retrieval_value["output"]["stdoutArtifact"]["artifactId"].as_str().expect("retrieval artifact")).expect("retrieval uuid");
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

        let fail_source = format!("print({})\nmissing_symbol", serde_json::to_string(&large).expect("failure source"));
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, &fail_source).await;
        let failed = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, &fail_source, &root, &role).await.expect("failed execute packet");
        let failed_value = serde_json::to_value(failed).expect("failed value");
        assert_eq!(failed_value["ok"], false);
        assert_eq!(failed_value["status"], "failed");
        assert!(failed_value["output"]["stdoutArtifact"]["artifactId"].is_string());
        assert!(failed_value["output"]["stderrArtifact"]["artifactId"].is_string());
        assert!(!failed_value.to_string().contains(&large));

        let mut sh_seed = admin_command_seed("cmd.output_artifacts.sh");
        sh_seed["binaryName"] = json!("sh");
        sh_seed["candidatePaths"] = json!(["/bin/sh"]);
        sh_seed["starlarkObject"] = json!("output_sh");
        sh_seed["argvPrefix"] = json!(["-c"]);
        let sh_seed: command_registry::CommandSeed = serde_json::from_value(sh_seed).expect("sh seed");
        apply_registry_seed(&test_db.pool, session_id, sh_seed, command_registry::RegistryScope { scope_type: "global".to_string(), project_key: None }).await;
        let command_source = r#"print(cmd["output_sh"].run(args=["printf stdout-artifact; printf stderr-artifact >&2"]).sync())"#;
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, command_source).await;
        let command_packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, command_source, &root, &role).await.expect("command output packet");
        let command_value = serde_json::to_value(command_packet).expect("command value");
        assert_eq!(command_value["status"], "completed", "command packet: {command_value}");
        let command_result_artifact = Uuid::parse_str(command_value["output"]["stdoutArtifact"]["artifactId"].as_str().expect("command result artifact")).expect("command result artifact id");
        let command_result: String = sqlx::query_scalar("SELECT content FROM execution_output_artifacts WHERE id=$1")
            .bind(command_result_artifact)
            .fetch_one(&test_db.pool)
            .await
            .expect("command result content");
        let command_envelope: Value = serde_json::from_str(command_result.trim()).expect("command envelope json");
        let command_stdout = Uuid::parse_str(command_envelope["stdoutArtifact"]["artifactId"].as_str().expect("stdout artifact id")).expect("stdout uuid");
        let command_stderr = Uuid::parse_str(command_envelope["stderrArtifact"]["artifactId"].as_str().expect("stderr artifact id")).expect("stderr uuid");
        let streams: Vec<(String, String)> = sqlx::query("SELECT stream, content FROM execution_output_artifacts WHERE id = ANY($1) ORDER BY stream")
            .bind(&[command_stdout, command_stderr])
            .fetch_all(&test_db.pool)
            .await
            .expect("command stream artifacts")
            .into_iter()
            .map(|row| (row.get("stream"), row.get("content")))
            .collect();
        assert_eq!(streams.len(), 2);
        assert!(streams.contains(&("stdout".to_string(), "stdout-artifact".to_string())));
        assert!(streams.contains(&("stderr".to_string(), "stderr-artifact".to_string())));
        let combined_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_output_artifacts WHERE turn_id=$1 AND stream='combined'")
            .bind(turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("no command combined artifacts");
        assert_eq!(combined_count, 0);

        let resume_turn = Uuid::new_v4();
        let resume_tool = Uuid::new_v4();
        let resume_script = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user','resume command artifact proof','running',now())")
            .bind(resume_turn).bind(session_id).execute(&test_db.pool).await.expect("resume turn");
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status, started_at) VALUES ($1,$2,$3,'execute_code','resume-command-proof','{}'::jsonb,'running',now())")
            .bind(resume_tool).bind(session_id).bind(resume_turn).execute(&test_db.pool).await.expect("resume tool");
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status, started_at) VALUES ($1,$2,'paused command','running',now())")
            .bind(resume_script).bind(resume_tool).execute(&test_db.pool).await.expect("resume script");
        let resumed = starlark_host::execute_resumed_action(
            &test_db.pool,
            session_id,
            Some(resume_turn),
            resume_script,
            "cmd.output_artifacts.sh",
            &json!({
                "argv": ["printf resumed-stdout; printf resumed-stderr >&2"],
                "cwd": ".",
                "executionRoot": "."
            }),
            json!({"decision":"allow","reason":"deterministic resumed command artifact proof"}),
        )
        .await
        .expect("resumed command");
        let resumed_command_id = Uuid::parse_str(resumed["commandRunId"].as_str().expect("resumed command id")).expect("resumed command uuid");
        let resumed_artifacts: Vec<(String, String, Option<Uuid>, Option<Uuid>, Option<Uuid>, Option<Uuid>, i64, i64)> = sqlx::query(
            "SELECT stream, content, session_id, turn_id, script_run_id, command_run_id, byte_count, line_count FROM execution_output_artifacts WHERE command_run_id=$1 ORDER BY stream"
        )
        .bind(resumed_command_id)
        .fetch_all(&test_db.pool)
        .await
        .expect("resumed command artifacts")
        .into_iter()
        .map(|row| (row.get("stream"), row.get("content"), row.get("session_id"), row.get("turn_id"), row.get("script_run_id"), row.get("command_run_id"), row.get("byte_count"), row.get("line_count")))
        .collect();
        assert_eq!(resumed_artifacts.len(), 2);
        assert!(resumed_artifacts.iter().any(|(stream, content, session, turn, script, command, bytes, lines)| stream == "stdout" && content == "resumed-stdout" && *session == Some(session_id) && *turn == Some(resume_turn) && *script == Some(resume_script) && *command == Some(resumed_command_id) && *bytes > 0 && *lines == 1));
        assert!(resumed_artifacts.iter().any(|(stream, content, session, turn, script, command, bytes, lines)| stream == "stderr" && content == "resumed-stderr" && *session == Some(session_id) && *turn == Some(resume_turn) && *script == Some(resume_script) && *command == Some(resumed_command_id) && *bytes > 0 && *lines == 1));
        let resumed_combined_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_output_artifacts WHERE command_run_id=$1 AND stream='combined'")
            .bind(resumed_command_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("no resumed combined artifacts");
        assert_eq!(resumed_combined_count, 0);

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
            "h = cmd[\"process_sh\"].run(args=[{}]).start()\nproc[h].await_for(mins=0)\nprint(proc[h].flush_buffer())",
            serde_json::to_string(process_shell).expect("process shell")
        );
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_id, &process_source).await;
        let process_packet = starlark_host::execute_code(&test_db.pool, session_id, turn_id, tool_call_id, &process_source, &root, &role).await.expect("process packet");
        let process_value = serde_json::to_value(process_packet).expect("process value");
        assert_eq!(process_value["status"], "completed", "process packet: {process_value}");
        assert!(!process_value.to_string().contains(&stdout_large));
        let process_result_artifact = Uuid::parse_str(process_value["output"]["stdoutArtifact"]["artifactId"].as_str().expect("process result artifact")).expect("process result uuid");
        let process_result: String = sqlx::query_scalar("SELECT content FROM execution_output_artifacts WHERE id=$1")
            .bind(process_result_artifact)
            .fetch_one(&test_db.pool)
            .await
            .expect("process result content");
        let process_envelope: Value = serde_json::from_str(process_result.trim()).expect("process envelope json");
        assert_eq!(process_envelope["stdoutArtifact"]["truncated"], true);
        assert_eq!(process_envelope["stderrArtifact"]["truncated"], true);
        let process_stdout = Uuid::parse_str(process_envelope["stdoutArtifact"]["artifactId"].as_str().expect("process stdout id")).expect("process stdout uuid");
        let process_stderr = Uuid::parse_str(process_envelope["stderrArtifact"]["artifactId"].as_str().expect("process stderr id")).expect("process stderr uuid");
        let process_streams: Vec<(String, String)> = sqlx::query("SELECT stream, content FROM execution_output_artifacts WHERE id = ANY($1) ORDER BY stream")
            .bind(&[process_stdout, process_stderr])
            .fetch_all(&test_db.pool)
            .await
            .expect("process stream artifacts")
            .into_iter()
            .map(|row| (row.get("stream"), row.get("content")))
            .collect();
        assert_eq!(process_streams.len(), 2);
        assert!(process_streams.contains(&("stdout".to_string(), stdout_large.clone())));
        assert!(process_streams.contains(&("stderr".to_string(), stderr_large.clone())));
        let combined_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_output_artifacts WHERE turn_id=$1 AND stream='combined'")
            .bind(turn_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("no process combined artifacts");
        assert_eq!(combined_count, 0);

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
        let source = format!("print({})", serde_json::to_string(secret).expect("source string"));
        let (turn_id, tool_call_id) = insert_turn_and_tool(&test_db.pool, session_a, &source).await;
        let packet = starlark_host::execute_code(&test_db.pool, session_a, turn_id, tool_call_id, &source, &root, &role).await.expect("execute owner output");
        let value = serde_json::to_value(packet).expect("owner packet");
        let artifact_id = value["output"]["stdoutArtifact"]["artifactId"].as_str().expect("artifact id");
        let quoted_artifact_id = serde_json::to_string(artifact_id).expect("quoted artifact id");

        let retrieval_sources = [
            format!("print(outputs.head({quoted_artifact_id}, lines=1))"),
            format!("print(outputs.tail({quoted_artifact_id}, lines=1))"),
            format!("print(outputs.slice({quoted_artifact_id}, start_line=1, end_line=1))"),
            format!("print(outputs.search({quoted_artifact_id}, \"secret\", context=1))"),
            format!("print(outputs.stats({quoted_artifact_id}))"),
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
        let outcome = requirements::record_requirements_claim_packet(&test_db.pool, source, turn_id, &claim).await.expect("claim outcome").expect("claim record");
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
        requirements::record_requirements_claim_packet(&test_db.pool, source, claim_turn, &claim).await.expect("claim");
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
        assert!(requirements::record_requirements_verdict_packet(&test_db.pool, reviewer_id, verdict_turn, &verdict).await.expect("verdict"));
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
        assert_eq!(shapes[0].pointer("/hook_schema_evidence/kind").and_then(Value::as_str), Some("sourceClaim"));
        assert_eq!(shapes[0].pointer("/text/format/type").and_then(Value::as_str), Some("json_schema"));
        assert_eq!(shapes[0].pointer("/hook_schema_evidence/canonicalCount").and_then(Value::as_u64), Some(1));
        println!("SOURCE_REQUIREMENTS_SCHEMA_EXAMPLE={}", serde_json::to_string_pretty(&shapes[0]["text"]["format"]).expect("source schema evidence"));
        println!("SOURCE_HOOK_SCHEMA_EVIDENCE={}", serde_json::to_string_pretty(&shapes[0]["hook_schema_evidence"]).expect("source schema metadata"));
        assert!(shapes[1].get("hook_schema_evidence").is_none());
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
        assert!(shapes.iter().any(|shape| shape.pointer("/hook_schema_evidence/kind").and_then(Value::as_str) == Some("reviewerVerdict")));
        if let Some(reviewer_shape) = shapes.iter().find(|shape| shape.pointer("/hook_schema_evidence/kind").and_then(Value::as_str) == Some("reviewerVerdict")) {
            println!("REVIEWER_REQUIREMENTS_SCHEMA_EXAMPLE={}", serde_json::to_string_pretty(&reviewer_shape["text"]["format"]).expect("reviewer schema evidence"));
            println!("REVIEWER_HOOK_SCHEMA_EVIDENCE={}", serde_json::to_string_pretty(&reviewer_shape["hook_schema_evidence"]).expect("reviewer schema metadata"));
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
            requirements::record_requirements_claim_packet(&test_db.pool, source, turn_id, body).await.expect("packet");
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
    async fn requirements_lifecycle_archive_and_fork_preserve_hidden_reviewer_semantics() {
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
        requirements::record_requirements_claim_packet(&test_db.pool, source, turn_id, r#"{"summary":"done","requirements":{"lifecycle_checked":{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"}}}"#).await.expect("claim");
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
        db::archive_session(&test_db.pool, source).await.expect("archive session");
        let status: String = sqlx::query_scalar("SELECT status FROM sessions WHERE id=$1")
            .bind(reviewer)
            .fetch_one(&test_db.pool)
            .await
            .expect("reviewer status");
        assert_eq!(status, "stopped");
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
        requirements::record_requirements_claim_packet(&test_db.pool, source, claim_turn, r#"{"summary":"done","requirements":{"api_visible":{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"}}}"#).await.expect("claim");
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
    async fn requirements_review_detail_routes_clarification_to_hidden_reviewer_through_unified_submit() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-reviewer-submit"), ".", Some("."), None, None).await.expect("source");
        requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("reviewer submit requirements".to_string()),
            requirements: vec![requirements::RequirementInput { key: "clarification_routed".to_string(), statement: "Clarification reaches the hidden reviewer.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) }],
        }).await.expect("set");
        let claim_turn = insert_completed_turn(&test_db.pool, source, "claim", "assistant").await;
        requirements::record_requirements_claim_packet(&test_db.pool, source, claim_turn, r#"{"summary":"done","requirements":{"clarification_routed":{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"}}}"#).await.expect("claim");
        let reviewer = requirements::status(&test_db.pool, source).await.expect("status").reviewer_session_id.expect("reviewer");
        let model = Arc::new(FakeModelClient { direct_final_text: Some("clarification recorded"), ..Default::default() });
        let state = ServerState::new_with_model_client(test_db.pool.clone(), "requirements-reviewer-submit".to_string(), model);
        let app = app(state.clone());

        let (status, body) = request_json(
            app.clone(),
            Method::POST,
            &format!("/sessions/{source}/requirements/reviewer/send"),
            json!({"message":"Clarification: accept this waiver and continue reviewing."}),
        ).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["sessionId"], reviewer.to_string());
        assert_eq!(body["disposition"], "idle_turn_start");
        assert!(body["submittedInputId"].as_str().is_some());
        for _ in 0..80 {
            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE session_id=$1 AND status='applied'")
                .bind(reviewer)
                .fetch_one(&test_db.pool)
                .await
                .expect("applied reviewer inputs");
            if applied == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let submitted = sqlx::query("SELECT session_id, source_parent_session_id, source, content, status, placement_turn_id FROM submitted_inputs WHERE session_id=$1")
            .bind(reviewer)
            .fetch_one(&test_db.pool)
            .await
            .expect("reviewer submitted input");
        assert_eq!(submitted.get::<Uuid, _>("session_id"), reviewer);
        assert_eq!(submitted.get::<Option<Uuid>, _>("source_parent_session_id"), Some(source));
        assert_eq!(submitted.get::<String, _>("source"), "requirementsReviewDetail");
        assert_eq!(submitted.get::<String, _>("content"), "Clarification: accept this waiver and continue reviewing.");
        assert_eq!(submitted.get::<String, _>("status"), "applied");
        assert!(submitted.get::<Option<Uuid>, _>("placement_turn_id").is_some());
        let normal_sessions = db::list_sessions(&test_db.pool, true).await.expect("normal sessions");
        assert!(!normal_sessions.iter().any(|session| session.id == reviewer), "hidden reviewer must remain out of normal session list");
        let snapshot = projection::build_runtime_projection_snapshot(&test_db.pool, Some(source)).await.expect("snapshot");
        assert!(!snapshot.sessions.iter().any(|session| session.id == reviewer.to_string()));
        assert_eq!(snapshot.selected_session.unwrap().requirements_review.unwrap().reviewer_session_id.as_deref(), Some(reviewer.to_string().as_str()));
        let invalid_source = db::new_session(&test_db.pool, &role, Some("requirements-reviewer-submit"), ".", Some("."), None, None).await.expect("invalid source");
        let (status, body) = request_json(app, Method::POST, &format!("/sessions/{invalid_source}/requirements/reviewer/send"), json!({"message":"no reviewer"})).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(body["error"]["message"].as_str().unwrap_or_default().contains("no active reviewer"));
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
        requirements::record_requirements_claim_packet(&test_db.pool, source, turn, r#"{"summary":"commentary","requirements":null}"#).await.expect("packet");
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
        let record = requirements::record_requirements_claim_packet(&test_db.pool, source, turn, invalid).await.expect("record").expect("active");
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
    async fn requirements_review_mirrors_to_generic_contract_packets_and_subagent_records() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-generic-mirror"), ".", Some("."), None, None).await.expect("source");
        let set_id = requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("generic mirror requirements".to_string()),
            requirements: vec![requirements::RequirementInput { key: "generic_mirror".to_string(), statement: "Requirements Review mirrors through generic workflow records.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) }],
        }).await.expect("set");
        let generic_contracts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM generic_contracts WHERE id=$1 AND session_id=$2 AND contract_type='requirements' AND status='active'")
            .bind(set_id)
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("generic contract");
        assert_eq!(generic_contracts, 1);
        let turn = insert_completed_turn(&test_db.pool, source, "claim", "assistant").await;
        let record = requirements::record_requirements_claim_packet(&test_db.pool, source, turn, r#"{"summary":"done","requirements":{"generic_mirror":{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"}}}"#)
            .await
            .expect("claim")
            .expect("record");
        assert_eq!(record.outcome, requirements::SourcePacketOutcome::Reviewable);
        let reviewer = record.reviewer_session_id.expect("reviewer");
        let packet_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_packets WHERE source_session_id=$1 AND packet_type='requirements.claim'")
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("runtime packet");
        assert_eq!(packet_count, 1);
        let subagent_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM generic_subagents WHERE parent_session_id=$1 AND subagent_session_id=$2 AND subagent_kind='requirementsReviewer' AND lifecycle_status='active'")
            .bind(source)
            .bind(reviewer)
            .fetch_one(&test_db.pool)
            .await
            .expect("generic subagent");
        assert_eq!(subagent_count, 1);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn hook_defined_requirements_review_runs_end_to_end_through_generic_workflow() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");

        async fn run_case(
            pool: &PgPool,
            role: &RoleSnapshot,
            project: &str,
            key: &str,
            verdict: &str,
            overall: &str,
            route: &str,
        ) -> (Uuid, Uuid, Uuid) {
            let source = db::new_session(pool, role, Some(project), ".", Some("."), None, None).await.expect("source");
            let set_id = requirements::set_active_requirements(pool, source, requirements::RequirementSetInput {
                title: Some(format!("{key} requirements")),
                requirements: vec![requirements::RequirementInput { key: key.to_string(), statement: format!("{key} is verified."), severity: "must".to_string(), verification_method: json!({"method":"test"}) }],
            }).await.expect("set");
            let source_schema = requirements::hook_defined_requirements_runtime_message(pool, source).await.expect("source schema").expect("source schema");
            assert_eq!(source_schema.metadata["source"], "hook_required_output_schema");
            assert!(source_schema.text.contains("requirements_source_claim"));
            let claim_turn = insert_completed_turn(pool, source, "claim", "assistant").await;
            let claim = format!(r#"{{"summary":"done","requirements":{{"{key}":{{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"}}}}}}"#);
            let record = requirements::record_requirements_claim_packet(pool, source, claim_turn, &claim).await.expect("claim").expect("record");
            assert_eq!(record.outcome, requirements::SourcePacketOutcome::Reviewable);
            let reviewer = record.reviewer_session_id.expect("reviewer");
            let reviewer_schema = requirements::hook_defined_requirements_runtime_message(pool, reviewer).await.expect("reviewer schema").expect("reviewer schema");
            assert!(reviewer_schema.text.contains("requirements_reviewer_verdict"));
            assert!(reviewer_schema.text.contains("<requirements_review_context>"));
            let runtime_claims: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_packets WHERE source_session_id=$1 AND packet_type='requirements.claim'")
                .bind(source)
                .fetch_one(pool)
                .await
                .expect("claim packet");
            assert_eq!(runtime_claims, 1);
            let routed_claims: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_envelopes e JOIN runtime_packets p ON p.id=e.packet_id WHERE p.source_session_id=$1 AND e.target_session_id=$2 AND e.envelope_type='requirements_claim_to_reviewer'")
                .bind(source)
                .bind(reviewer)
                .fetch_one(pool)
                .await
                .expect("route packet");
            assert_eq!(routed_claims, 1);
            let verdict_turn = insert_completed_turn(pool, reviewer, "verdict", "assistant").await;
            let verdict_packet = format!(r#"{{"summary":"reviewed","requirements":{{"{key}":{{"verdict":"{verdict}","evidence":["e"],"justification":"j","risk":"low"}}}},"overallVerdict":"{overall}","route":"{route}"}}"#);
            assert!(requirements::record_requirements_verdict_packet(pool, reviewer, verdict_turn, &verdict_packet).await.expect("verdict"));
            let generic_progress: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM generic_contract_progress WHERE contract_id=$1 AND progress_key=$2")
                .bind(set_id)
                .bind(key)
                .fetch_one(pool)
                .await
                .expect("generic progress");
            assert_eq!(generic_progress, 1);
            (source, set_id, reviewer)
        }

        let (pass_source, _, pass_reviewer) = run_case(&test_db.pool, &role, "requirements-e2e-pass", "pass_req", "pass", "pass", "source").await;
        let pass_status = requirements::status(&test_db.pool, pass_source).await.expect("pass status");
        assert!(!pass_status.active, "pass verdict clears active Requirements");

        let (fail_source, _, _) = run_case(&test_db.pool, &role, "requirements-e2e-fail", "fail_req", "fail", "fail", "source").await;
        let fail_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='requirements.correction' AND status='failed'")
            .bind(fail_source)
            .fetch_one(&test_db.pool)
            .await
            .expect("fail route");
        assert_eq!(fail_events, 1, "fail verdict routes correction to source");

        let (waiver_source, _, _) = run_case(&test_db.pool, &role, "requirements-e2e-waiver", "waiver_req", "waiverRequired", "needsHumanWaiver", "owner").await;
        let owner_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='requirements.ownerAction' AND status='blocked'")
            .bind(waiver_source)
            .fetch_one(&test_db.pool)
            .await
            .expect("owner route");
        assert_eq!(owner_events, 1, "waiver verdict routes owner action");

        let normal_sessions = db::list_sessions(&test_db.pool, true).await.expect("sessions");
        assert!(!normal_sessions.iter().any(|session| session.id == pass_reviewer), "reviewer subagent is hidden from normal list");
        assert_eq!(db::session_record(&test_db.pool, pass_reviewer).await.expect("direct reviewer").id, pass_reviewer);
        let generic_subagent_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM generic_subagents WHERE parent_session_id=$1 AND subagent_session_id=$2")
            .bind(pass_source)
            .bind(pass_reviewer)
            .fetch_one(&test_db.pool)
            .await
            .expect("generic subagent row");
        assert_eq!(generic_subagent_count, 1);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn hook_defined_requirements_review_replay_does_not_double_apply_legacy_paths() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-replay"), ".", Some("."), None, None).await.expect("source");
        requirements::set_active_requirements(&test_db.pool, source, requirements::RequirementSetInput {
            title: Some("replay requirements".to_string()),
            requirements: vec![requirements::RequirementInput { key: "replay_safe".to_string(), statement: "Replay does not duplicate workflow records.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) }],
        }).await.expect("set");
        let claim_turn = insert_completed_turn(&test_db.pool, source, "claim", "assistant").await;
        let claim = r#"{"summary":"done","requirements":{"replay_safe":{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"}}}"#;
        let first = requirements::record_requirements_claim_packet(&test_db.pool, source, claim_turn, claim).await.expect("first").expect("first");
        let second = requirements::record_requirements_claim_packet(&test_db.pool, source, claim_turn, claim).await.expect("second").expect("second");
        assert_eq!(first.packet_id, second.packet_id);
        let reviewer = first.reviewer_session_id.expect("reviewer");
        let reviewer_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM generic_subagents WHERE parent_session_id=$1 AND subagent_kind='requirementsReviewer'")
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("reviewer rows");
        assert_eq!(reviewer_rows, 1, "hook-defined reviewer creation must be idempotent");
        let claim_packets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_packets WHERE source_session_id=$1 AND packet_type='requirements.claim'")
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("runtime claim packets");
        assert_eq!(claim_packets, 1, "replay must not double-record claim packets");
        let claim_routes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_envelopes e JOIN runtime_packets p ON p.id=e.packet_id WHERE p.source_session_id=$1 AND e.envelope_type='requirements_claim_to_reviewer'")
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("claim routes");
        assert_eq!(claim_routes, 1, "replay must not double-dispatch reviewer envelopes");

        let verdict_turn = insert_completed_turn(&test_db.pool, reviewer, "verdict", "assistant").await;
        let verdict = r#"{"summary":"reviewed","requirements":{"replay_safe":{"verdict":"fail","evidence":["e"],"justification":"j","risk":"low"}},"overallVerdict":"fail","route":"source"}"#;
        assert!(requirements::record_requirements_verdict_packet(&test_db.pool, reviewer, verdict_turn, verdict).await.expect("verdict first"));
        assert!(requirements::record_requirements_verdict_packet(&test_db.pool, reviewer, verdict_turn, verdict).await.expect("verdict second"));
        let verdict_packets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_packets WHERE source_session_id=$1 AND parent_session_id=$2 AND packet_type='requirements.verdict'")
            .bind(source)
            .bind(reviewer)
            .fetch_one(&test_db.pool)
            .await
            .expect("runtime verdict packets");
        assert_eq!(verdict_packets, 1, "replay must not double-record verdict packets");
        let progress_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM generic_contract_progress WHERE contract_id=(SELECT id FROM requirement_sets WHERE source_session_id=$1 LIMIT 1) AND progress_key='replay_safe'")
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("progress rows");
        assert_eq!(progress_rows, 1, "replay must not double-apply generic progress");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn bare_project_runtime_smoke_runs_requirements_and_resource_lease_without_global_skills() {
        let test_db = validation_db().await;
        db::create_project(&test_db.pool, "bare-runtime-smoke", "Bare Runtime Smoke", ".", ".", None, "gpt-5.4-mini").await.expect("project");
        let source = include_str!("../../../project-runtime-seeds/requirements_review.star").to_string();
        let manifest = json!({
            "roles": [{"id":"requirements-reviewer"}],
            "channels": [{"id":"requirements","packetTypes":["requirements.claim","requirements.verdict"]}],
            "hooks": [{"name":"on_model_request","source":"def hook(ctx):\n    return [require_output_schema(key='source', schema_name='requirements_source_claim', packet_type='requirements.claim', schema={'type':'object'})]\n", "intentTypes":["require_output_schema"]}],
            "resources": [{"type":"iosSimulator"}],
            "routes": [{"id":"requirements-review","source":"requirements.claim","target":"subagent:requirements-reviewer"}],
        });
        let version_id = crate::lifecycle_hooks::persist_project_runtime_config(&test_db.pool, "bare-runtime-smoke", &source, manifest, "smoke").await.expect("persist");
        crate::lifecycle_hooks::activate_project_runtime_config(&test_db.pool, "bare-runtime-smoke", version_id).await.expect("activate");
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source_session = db::new_session(&test_db.pool, &role, Some("bare-runtime-smoke"), ".", Some("."), None, None).await.expect("session");
        requirements::set_active_requirements(&test_db.pool, source_session, requirements::RequirementSetInput {
            title: Some("smoke requirements".to_string()),
            requirements: vec![requirements::RequirementInput { key: "smoke_req".to_string(), statement: "Smoke requirement passes through hook workflow.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"test"}) }],
        }).await.expect("requirements");
        let claim_turn = insert_completed_turn(&test_db.pool, source_session, "claim", "assistant").await;
        requirements::record_requirements_claim_packet(&test_db.pool, source_session, claim_turn, r#"{"summary":"done","requirements":{"smoke_req":{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"}}}"#).await.expect("claim");
        let reviewer = requirements::status(&test_db.pool, source_session).await.expect("status").reviewer_session_id.expect("reviewer");
        assert_eq!(db::session_record(&test_db.pool, reviewer).await.expect("reviewer exact").parent_session_id, Some(source_session));
        let (lease_packet, lease_envelope) = crate::lifecycle_hooks::request_resource_lease(
            &test_db.pool,
            "bare-runtime-smoke",
            source_session,
            "iosSimulator",
            "runtime-no-rg",
            json!({"purpose":"smoke"}),
            "bare-smoke-lease",
        ).await.expect("lease request");
        assert_ne!(lease_packet, Uuid::nil());
        assert_ne!(lease_envelope, Uuid::nil());
        let global_skill_edits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE payload::text LIKE '%global skills%' OR payload::text LIKE '%skills edit%'")
            .fetch_one(&test_db.pool)
            .await
            .expect("global skill audit");
        assert_eq!(global_skill_edits, 0);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn active_requirements_data_migrates_to_generic_contract_packets_and_subagent_without_loss() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let source = db::new_session(&test_db.pool, &role, Some("requirements-migration"), ".", Some("."), None, None).await.expect("source");
        let reviewer = db::new_session(&test_db.pool, &role, Some("requirements-migration"), ".", Some("."), None, None).await.expect("reviewer");
        sqlx::query("UPDATE sessions SET parent_session_id=$2, session_kind='requirementsReviewer', hidden=true, tracked=false WHERE id=$1")
            .bind(reviewer)
            .bind(source)
            .execute(&test_db.pool)
            .await
            .expect("reviewer session");
        let set_id = Uuid::new_v4();
        let canonical = json!({"title":"migration","schemaVersion":1,"requirements":[{"key":"migrate_req","statement":"Migrate active data.","severity":"must","verificationMethod":{"method":"test"},"sortOrder":0}]});
        sqlx::query("INSERT INTO requirement_sets (id, source_session_id, title, canonical_set, status, enforce_on_turns) VALUES ($1,$2,'migration',$3,'active',true)")
            .bind(set_id)
            .bind(source)
            .bind(&canonical)
            .execute(&test_db.pool)
            .await
            .expect("legacy set");
        sqlx::query("INSERT INTO requirement_items (id, requirement_set_id, requirement_key, statement, severity, verification_method, sort_order) VALUES ($1,$2,'migrate_req','Migrate active data.','must',$3,0)")
            .bind(Uuid::new_v4())
            .bind(set_id)
            .bind(json!({"method":"test"}))
            .execute(&test_db.pool)
            .await
            .expect("item");
        sqlx::query("INSERT INTO requirement_progress (requirement_set_id, requirement_key, status) VALUES ($1,'migrate_req','blocked')")
            .bind(set_id)
            .execute(&test_db.pool)
            .await
            .expect("progress");
        let packet_id = Uuid::new_v4();
        let turn = insert_completed_turn(&test_db.pool, source, "claim", "assistant").await;
        sqlx::query("INSERT INTO requirement_packets (id, requirement_set_id, source_session_id, reviewer_session_id, turn_id, packet_kind, status, payload) VALUES ($1,$2,$3,$4,$5,'claim','reviewable',$6)")
            .bind(packet_id)
            .bind(set_id)
            .bind(source)
            .bind(reviewer)
            .bind(turn)
            .bind(json!({"summary":"legacy","requirements":{"migrate_req":{"claim":"blocked","evidence":["e"],"justification":"j","risk":"medium"}}}))
            .execute(&test_db.pool)
            .await
            .expect("packet");
        sqlx::query("INSERT INTO requirement_review_bindings (id, requirement_set_id, source_session_id, reviewer_session_id, latest_claim_packet_id, status) VALUES ($1,$2,$3,$4,$5,'inReview')")
            .bind(Uuid::new_v4())
            .bind(set_id)
            .bind(source)
            .bind(reviewer)
            .bind(packet_id)
            .execute(&test_db.pool)
            .await
            .expect("binding");
        let summary = requirements::migrate_active_requirements_to_generic_workflow(&test_db.pool).await.expect("migrate");
        assert_eq!(summary["contracts"], 1);
        assert_eq!(summary["subagents"], 1);
        let contract_payload: Value = sqlx::query_scalar("SELECT canonical_payload FROM generic_contracts WHERE id=$1 AND session_id=$2")
            .bind(set_id)
            .bind(source)
            .fetch_one(&test_db.pool)
            .await
            .expect("contract");
        assert_eq!(contract_payload, canonical);
        let runtime_packet_payload: Value = sqlx::query_scalar("SELECT payload FROM runtime_packets WHERE source_session_id=$1 AND packet_type='requirements.claim' AND routing_metadata->>'migratedRequirementPacketId'=$2")
            .bind(source)
            .bind(packet_id.to_string())
            .fetch_one(&test_db.pool)
            .await
            .expect("runtime packet");
        assert_eq!(runtime_packet_payload.pointer("/requirements/migrate_req/claim").and_then(Value::as_str), Some("blocked"));
        let progress: String = sqlx::query_scalar("SELECT status FROM requirement_progress WHERE requirement_set_id=$1 AND requirement_key='migrate_req'")
            .bind(set_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("progress");
        assert_eq!(progress, "blocked");
        let subagent: Uuid = sqlx::query_scalar("SELECT subagent_session_id FROM generic_subagents WHERE parent_session_id=$1 AND workflow_identity=$2")
            .bind(source)
            .bind(set_id.to_string())
            .fetch_one(&test_db.pool)
            .await
            .expect("subagent");
        assert_eq!(subagent, reviewer);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn starlark_lifecycle_config_activation_and_hook_audit_are_postgres_backed() {
        let test_db = validation_db().await;
        db::create_project(&test_db.pool, "hook-audit-project", "Hook Audit", ".", ".", None, "gpt-5.4-mini")
            .await
            .expect("project");
        let app = app(ServerState::new(test_db.pool.clone()));
        let (status, validate_body) = request_json(
            app,
            Method::POST,
            "/projects/hook-audit-project/runtime-config/validate",
            json!({"sourceText":"x = 1","manifest":{"hooks":[{"name":"on_model_request","source":"x = 1"}]}}),
        ).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(validate_body["valid"], true);
        let hook_source = r#"
def hook(ctx):
    return [require_output_schema(key = "requirements", schema_name = "requirements_source_claim", packet_type = "requirements.claim", schema = {"type": "object"})]
"#;
        let manifest = json!({
            "roles": [{"id":"requirements-reviewer"}],
            "resourceTypes": [{"id":"iosSimulator"}],
            "hooks": [{"name":"on_model_request","source":hook_source}]
        });
        let version_id = crate::lifecycle_hooks::persist_project_runtime_config(
            &test_db.pool,
            "hook-audit-project",
            hook_source,
            manifest,
            "test",
        ).await.expect("persist runtime config");
        crate::lifecycle_hooks::activate_project_runtime_config(&test_db.pool, "hook-audit-project", version_id)
            .await
            .expect("activate runtime config");
        let active_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_runtime_config_versions WHERE id=$1 AND activation_status='active'")
            .bind(version_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("active config");
        assert_eq!(active_count, 1);
        let binding_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_runtime_hook_bindings WHERE config_version_id=$1 AND lifecycle_hook='on_model_request' AND status='active'")
            .bind(version_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("active hook binding");
        assert_eq!(binding_count, 1);
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session = db::new_session(&test_db.pool, &role, Some("hook-audit-project"), ".", Some("."), None, None).await.expect("session");
        let session_binding: (Option<Uuid>, Value) = sqlx::query_as("SELECT active_project_runtime_version_id, active_hook_bindings FROM sessions WHERE id=$1")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("session active hook attribution");
        assert_eq!(session_binding.0, Some(version_id));
        assert!(session_binding.1.get("on_model_request").is_some());
        let context = crate::lifecycle_hooks::hook_context_from_session_summary(
            session,
            Some("hook-audit-project".to_string()),
            "source".to_string(),
            None,
            false,
            &role,
            ".".to_string(),
            Some(".".to_string()),
            crate::lifecycle_hooks::LifecycleHook::OnModelRequest,
        );
        let results = crate::lifecycle_hooks::evaluate_active_lifecycle_hooks(
            &test_db.pool,
            "hook-audit-project",
            Some(session),
            None,
            crate::lifecycle_hooks::LifecycleHook::OnModelRequest,
            &context,
        ).await.expect("active lifecycle hook evaluation");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].validation_status, "valid", "{:?}", results[0].errors);
        let eval_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hook_evaluations WHERE hook_version_id=$1 AND session_id=$2 AND validation_status='valid'")
            .bind(version_id)
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("hook eval row");
        assert_eq!(eval_count, 1);
        let schema_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM structured_output_schema_evidence WHERE hook_version_id=$1 AND packet_type='requirements.claim' AND schema_name='requirements_source_claim'")
            .bind(version_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("schema evidence");
        assert_eq!(schema_count, 1);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn hook_intent_application_records_packets_envelopes_schema_leases_obligations_and_subagent_lifecycle() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session = db::new_session(&test_db.pool, &role, Some("hook-intents"), ".", Some("."), None, None).await.expect("session");
        let turn = insert_completed_turn(&test_db.pool, session, "input", "assistant").await;
        let contract_id = Uuid::new_v4();
        sqlx::query("INSERT INTO generic_contracts (id, session_id, contract_type, canonical_payload, status, active_version) VALUES ($1,$2,'hook-contract',$3,'active','v1')")
            .bind(contract_id)
            .bind(session)
            .bind(json!({"requirements":["bounded"]}))
            .execute(&test_db.pool)
            .await
            .expect("active generic contract");
        let context = crate::lifecycle_hooks::hook_context_from_session_summary(
            session,
            Some("hook-intents".to_string()),
            "source".to_string(),
            None,
            false,
            &role,
            ".".to_string(),
            Some(".".to_string()),
            crate::lifecycle_hooks::LifecycleHook::OnToolComplete,
        );
        let result = crate::lifecycle_hooks::evaluate_and_apply_lifecycle_intents(
            &test_db.pool,
            Some("hook-intents"),
            Some(session),
            Some(turn),
            crate::lifecycle_hooks::LifecycleHook::OnToolComplete,
            &context,
            None,
            "hook-hash",
            vec![
                crate::lifecycle_hooks::HookIntent { intent_type: "record_packet".to_string(), key: Some("packet".to_string()), payload: json!({"packetType":"resource.request","status":"valid","payload":{"resourceType":"iosSimulator"}}), idempotency_key: None },
                crate::lifecycle_hooks::HookIntent { intent_type: "reserve_resource".to_string(), key: Some("lease".to_string()), payload: json!({"resourceType":"iosSimulator","resourceId":"sim-1","status":"assigned","leasePurpose":"test"}), idempotency_key: None },
                crate::lifecycle_hooks::HookIntent { intent_type: "add_turn_obligation".to_string(), key: Some("obligation".to_string()), payload: json!({"obligationType":"leaseIdleNotice","message":"check lease"}), idempotency_key: None },
            ],
        ).await.expect("evaluate and apply");
        assert_eq!(result.validation_status, "valid");
        let packets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_packets WHERE source_session_id=$1 AND packet_type='resource.request'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("packet count");
        assert_eq!(packets, 1);
        let lease: (Uuid,) = sqlx::query_as("SELECT id FROM resource_leases WHERE owning_session_id=$1 AND resource_type='iosSimulator' AND status='assigned'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("lease");
        let obligations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turn_obligations WHERE session_id=$1 AND obligation_type='leaseIdleNotice' AND status='pending'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("obligation");
        assert_eq!(obligations, 1);

        let packet_id: Uuid = sqlx::query_scalar("SELECT id FROM runtime_packets WHERE source_session_id=$1 AND packet_type='resource.request' LIMIT 1")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("packet id");
        crate::lifecycle_hooks::apply_hook_intents(
            &test_db.pool,
            Some("hook-intents"),
            Some(session),
            Some(turn),
            crate::lifecycle_hooks::LifecycleHook::OnPacketRecorded,
            &[crate::lifecycle_hooks::HookIntent { intent_type: "route_packet".to_string(), key: Some("route".to_string()), payload: json!({"packetId": packet_id.to_string(), "targetSessionId": session.to_string()}), idempotency_key: Some("route-key".to_string()) }],
        ).await.expect("route packet");
        let envelopes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_envelopes WHERE packet_id=$1 AND target_session_id=$2")
            .bind(packet_id)
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("envelope count");
        assert_eq!(envelopes, 1);
        let other_project_session = db::new_session(&test_db.pool, &role, Some("other-hook-project"), ".", Some("."), None, None).await.expect("other project session");
        let cross_project_route = crate::lifecycle_hooks::route_packet_envelope(
            &test_db.pool,
            packet_id,
            "hookRoute",
            Some(session),
            Some(other_project_session),
            None,
            "pending",
            json!({}),
        ).await;
        assert!(cross_project_route.is_err());
        let unknown_role_route = crate::lifecycle_hooks::route_packet_envelope(
            &test_db.pool,
            packet_id,
            "hookRoute",
            Some(session),
            None,
            Some("missing-steward-role"),
            "pending",
            json!({}),
        ).await;
        assert!(unknown_role_route.is_err());
        let (lease_request_packet, lease_request_envelope) = crate::lifecycle_hooks::request_resource_lease(
            &test_db.pool,
            "hook-intents",
            session,
            "iosSimulator",
            "runtime-no-rg",
            json!({"purpose":"designer-preview"}),
            "lease-request-affordance",
        ).await.expect("lease request routed to steward");
        let routed: (String, String, String) = sqlx::query_as("SELECT p.packet_type, e.envelope_type, e.target_role_id FROM runtime_packets p JOIN runtime_envelopes e ON e.packet_id=p.id WHERE p.id=$1 AND e.id=$2")
            .bind(lease_request_packet)
            .bind(lease_request_envelope)
            .fetch_one(&test_db.pool)
            .await
            .expect("lease request envelope");
        assert_eq!(routed.0, "resource.request");
        assert_eq!(routed.1, "resource_request");
        assert_eq!(routed.2, "runtime-no-rg");
        let mut steward_role = role.clone();
        steward_role.id = "runtime-no-rg".to_string();
        let mut designer_role = role.clone();
        designer_role.id = "designer-worker".to_string();
        for (action_id, object) in [
            ("cmd.simulator.list", "simulator_list"),
            ("cmd.simulator.boot", "simulator_boot"),
            ("cmd.simulator.assign", "simulator_assign"),
            ("cmd.simulator.release", "simulator_release"),
            ("cmd.simulator.repair", "simulator_repair"),
        ] {
            apply_registry_seed(
                &test_db.pool,
                session,
                scoped_command_seed(action_id, object),
                command_registry::RegistryScope { scope_type: "role".to_string(), project_key: Some("runtime-no-rg".to_string()) },
            ).await;
        }
        apply_registry_seed(
            &test_db.pool,
            session,
            scoped_command_seed("cmd.simulator.request_lease", "simulator_request_lease"),
            command_registry::RegistryScope { scope_type: "role".to_string(), project_key: Some("designer-worker".to_string()) },
        ).await;
        let steward_tools = command_registry::live_visible_commands(&test_db.pool, &steward_role, Some("hook-intents")).await.expect("steward visible tools");
        let steward_actions = steward_tools.iter().map(|command| command.action_id.as_str()).collect::<Vec<_>>();
        for required in ["cmd.simulator.list", "cmd.simulator.boot", "cmd.simulator.assign", "cmd.simulator.release", "cmd.simulator.repair"] {
            assert!(steward_actions.contains(&required), "steward missing simulator management tool {required}");
        }
        let designer_tools = command_registry::live_visible_commands(&test_db.pool, &designer_role, Some("hook-intents")).await.expect("designer visible tools");
        let designer_actions = designer_tools.iter().map(|command| command.action_id.as_str()).collect::<Vec<_>>();
        assert!(designer_actions.contains(&"cmd.simulator.request_lease"), "designer sees lease-request affordance");
        for forbidden in ["cmd.simulator.list", "cmd.simulator.boot", "cmd.simulator.assign", "cmd.simulator.release", "cmd.simulator.repair"] {
            assert!(!designer_actions.contains(&forbidden), "designer/worker must not see global simulator management tool {forbidden}");
        }
        let delivered = crate::lifecycle_hooks::deliver_resource_lease_handle(
            &test_db.pool,
            "hook-intents",
            lease.0,
            "deliver-resource-handle",
        ).await.expect("deliver lease handle");
        let handle_delivery: (String, String, Uuid) = sqlx::query_as("SELECT p.packet_type, e.envelope_type, e.target_session_id FROM runtime_packets p JOIN runtime_envelopes e ON e.packet_id=p.id WHERE p.id=$1 AND e.id=$2")
            .bind(delivered.0)
            .bind(delivered.1)
            .fetch_one(&test_db.pool)
            .await
            .expect("handle delivery");
        assert_eq!(handle_delivery.0, "resource.lease_handle");
        assert_eq!(handle_delivery.1, "resource_lease_handle");
        assert_eq!(handle_delivery.2, session);

        let model_request_id = Uuid::new_v4();
        crate::lifecycle_hooks::apply_hook_intents(
            &test_db.pool,
            Some("hook-intents"),
            Some(session),
            Some(turn),
            crate::lifecycle_hooks::LifecycleHook::OnModelRequest,
            &[crate::lifecycle_hooks::HookIntent { intent_type: "require_output_schema".to_string(), key: Some("schema".to_string()), payload: json!({"schemaName":"resource_request","packetType":"resource.request","schema":{"type":"object"},"modelRequestId": model_request_id.to_string()}), idempotency_key: Some("schema-key".to_string()) }],
        ).await.expect("schema evidence");
        let schema_evidence: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM structured_output_schema_evidence WHERE packet_type='resource.request' AND lifecycle_boundary='on_model_request' AND model_request_id=$1")
            .bind(model_request_id)
            .fetch_one(&test_db.pool)
            .await
            .expect("schema evidence count");
        assert_eq!(schema_evidence, 1);
        let hook_schema = crate::lifecycle_hooks::hook_required_schema_for_model_request(&test_db.pool, model_request_id).await.expect("hook schema").expect("schema evidence");
        assert_eq!(hook_schema.pointer("/metadata/packetType").and_then(Value::as_str), Some("resource.request"));
        let mut responses_body = json!({"model":"test","input":[]});
        crate::lifecycle_hooks::apply_hook_required_schema_to_responses_body(&mut responses_body, &hook_schema).expect("apply schema");
        assert_eq!(responses_body.pointer("/text/format/type").and_then(Value::as_str), Some("json_schema"));
        let expected_model_request_id = model_request_id.to_string();
        assert_eq!(responses_body.pointer("/hook_schema_evidence/modelRequestId").and_then(Value::as_str), Some(expected_model_request_id.as_str()));
        let runtime_schema_message = crate::model::RuntimeInputMessage {
            text: format!("<hook_required_schema>{}</hook_required_schema>", serde_json::to_string(&hook_schema).expect("schema json")),
            metadata: json!({"source":"hook_required_output_schema"}),
        };
        let request_shape = crate::model::codex_adapter::CodexBackedModelClient::request_tool_call_request_shape(
            "test-model",
            &role,
            &[],
            &[runtime_schema_message],
            "execute code contract",
            "registry contract",
            "produce packet",
        );
        assert_eq!(request_shape.pointer("/text/format/name").and_then(Value::as_str), Some("resource_request"));
        assert_eq!(request_shape.pointer("/hook_schema_evidence/packetType").and_then(Value::as_str), Some("resource.request"));
        let parsed_packet = crate::lifecycle_hooks::parse_structured_final_output_into_packet(
            &test_db.pool,
            Some("hook-intents"),
            Some(session),
            Some(turn),
            model_request_id,
            r#"{"resourceType":"iosSimulator","decision":"queued"}"#,
            "parsed-structured-output",
        ).await.expect("parsed packet");
        let parsed: (String, Value) = sqlx::query_as("SELECT packet_type, payload FROM runtime_packets WHERE id=$1")
            .bind(parsed_packet)
            .fetch_one(&test_db.pool)
            .await
            .expect("parsed packet row");
        assert_eq!(parsed.0, "resource.request");
        assert_eq!(parsed.1["decision"], "queued");
        let envelope_packet = crate::lifecycle_hooks::record_runtime_packet(
            &test_db.pool,
            Some("hook-intents"),
            Some(session),
            None,
            Some(turn),
            "workflow.matrix",
            "recorded",
            json!({"matrix":"typed-envelope"}),
            None,
            json!({"source":"typed-envelope-matrix"}),
            "typed-envelope-matrix-packet",
        ).await.expect("matrix packet");
        for (envelope_type, target_session, target_role) in [
            ("owner_notice", Some(session), None),
            ("source_delivery", Some(session), None),
            ("subagent_delivery", Some(session), None),
            ("orchestrator_delivery", None, Some("runtime-no-rg")),
            ("steward_delivery", None, Some("runtime-no-rg")),
            ("system_notice", Some(session), None),
        ] {
            crate::lifecycle_hooks::route_packet_envelope(
                &test_db.pool,
                envelope_packet,
                envelope_type,
                Some(session),
                target_session,
                target_role,
                "pending",
                json!({"matrix":envelope_type}),
            ).await.expect("typed envelope route");
        }
        let matrix_types: Vec<String> = sqlx::query_scalar("SELECT envelope_type FROM runtime_envelopes WHERE packet_id=$1 ORDER BY envelope_type")
            .bind(envelope_packet)
            .fetch_all(&test_db.pool)
            .await
            .expect("matrix envelope types");
        for expected in ["orchestrator_delivery", "owner_notice", "source_delivery", "steward_delivery", "subagent_delivery", "system_notice"] {
            assert!(matrix_types.iter().any(|value| value == expected), "missing typed envelope {expected}");
        }
        let raw_messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM submitted_inputs WHERE content LIKE '%typed-envelope-matrix%'")
            .fetch_one(&test_db.pool)
            .await
            .expect("ordinary messages");
        assert_eq!(raw_messages, 0, "typed workflow packets/envelopes must not be represented as ordinary user messages");

        let subagent = crate::lifecycle_hooks::ensure_subagent(&test_db.pool, session, "reviewer", "wf-1", "genericReviewer", "runtime-no-rg", json!({}), json!({})).await.expect("ensure subagent");
        let subagent_again = crate::lifecycle_hooks::ensure_subagent(&test_db.pool, session, "reviewer", "wf-1", "genericReviewer", "runtime-no-rg", json!({}), json!({})).await.expect("ensure subagent again");
        assert_eq!(subagent, subagent_again);
        let normal_list = db::list_sessions(&test_db.pool, false).await.expect("normal list");
        assert!(!normal_list.iter().any(|entry| entry.id == subagent));
        let all_list = db::list_sessions(&test_db.pool, true).await.expect("all list");
        assert!(!all_list.iter().any(|entry| entry.id == subagent), "hidden generic subagents stay out of ordinary list UX even when include_all is true");
        let global_projection = projection::build_runtime_projection_snapshot(&test_db.pool, None).await.expect("global projection");
        assert!(!global_projection.sessions.iter().any(|entry| entry.id == subagent.to_string()), "hidden generic subagent must stay out of project rail/session list projection");
        let visible_session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE hidden=false")
            .fetch_one(&test_db.pool)
            .await
            .expect("visible session count");
        assert_eq!(global_projection.statistics.sessions, visible_session_count as u64, "ordinary session-count UX excludes hidden generic subagents");
        let hidden_projection = projection::build_runtime_projection_snapshot(&test_db.pool, Some(subagent)).await.expect("hidden selected projection");
        assert_eq!(hidden_projection.statistics.sessions, 0, "selected hidden subagent does not inflate ordinary selected session counts");
        let parent_projection = crate::lifecycle_hooks::parent_subagent_projection(&test_db.pool, session).await.expect("parent projection");
        assert_eq!(parent_projection["activeSubagents"], 1);
        assert_eq!(parent_projection["subagents"][0]["sessionId"], subagent.to_string());
        let show = db::show_session(&test_db.pool, session).await.expect("show session");
        assert_eq!(show["subagents"]["activeSubagents"], 1);
        let selected_projection = projection::build_runtime_projection_snapshot(&test_db.pool, Some(session)).await.expect("selected workflow projection");
        let selected = selected_projection.selected_session.as_ref().expect("selected session detail");
        assert_eq!(selected.subagents["activeSubagents"], 1);
        assert!(selected.contracts.iter().any(|contract| contract["contractId"] == contract_id.to_string()));
        assert!(selected.resource_leases.iter().any(|lease_row| lease_row["leaseId"] == lease.0.to_string()));
        assert!(selected.project_runtime.get("hookBindingCount").is_some(), "selected projection includes active runtime binding metadata");
        assert_eq!(db::session_record(&test_db.pool, subagent).await.expect("exact subagent").id, subagent);
        let checkpoint = compaction::compact_session_through_turn(&test_db.pool, session, turn, compaction::CompactionBudget::default()).await.expect("compact with workflow state");
        assert!(checkpoint.replacement_context.contains("Active hook workflow state by durable ids"));
        assert!(checkpoint.replacement_context.contains(&format!("contract={contract_id}")));
        assert!(checkpoint.replacement_context.contains("type=resource.request"));
        assert!(checkpoint.replacement_context.contains("subagent="));
        assert!(checkpoint.replacement_context.contains("obligation="));
        assert!(checkpoint.replacement_context.contains("resourceType=iosSimulator"));
        assert!(!checkpoint.replacement_context.contains("payload\":{\""), "compaction must summarize workflow state by ids and metadata, not hidden packet bodies");
        assert_eq!(crate::lifecycle_hooks::deactivate_subagent(&test_db.pool, session, "reviewer", "wf-1").await.expect("deactivate"), Some(subagent));
        let inactive = db::session_record(&test_db.pool, subagent).await.expect("inactive subagent");
        assert_eq!(inactive.status, "stopped");

        crate::lifecycle_hooks::release_resource(&test_db.pool, Some(session), json!({"resourceType":"iosSimulator","leaseId": lease.0.to_string(), "releaseReason":"test complete"})).await.expect("release");
        let released: String = sqlx::query_scalar("SELECT status FROM resource_leases WHERE id=$1")
            .bind(lease.0)
            .fetch_one(&test_db.pool)
            .await
            .expect("released status");
        assert_eq!(released, "released");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn restart_preserves_active_hook_workflow_state_and_idempotency() {
        let test_db = validation_db().await;
        let source = r#"
hook_binding(name = "on_model_request", source = """
def hook(ctx):
    return [require_output_schema(schema_name = "restart_schema", packet_type = "restart.packet", schema = {"type":"object"}, key = "restart-schema")]
""", intent_types = ["require_output_schema"])
"#;
        let manifest = crate::lifecycle_hooks::compile_project_runtime_source(source).expect("manifest");
        let version_id = crate::lifecycle_hooks::persist_project_runtime_config(&test_db.pool, "restart-project", source, manifest, "restart-test").await.expect("persist runtime config");
        crate::lifecycle_hooks::activate_project_runtime_config(&test_db.pool, "restart-project", version_id).await.expect("activate runtime config");
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session = db::new_session(&test_db.pool, &role, Some("restart-project"), ".", Some("."), None, None).await.expect("session");
        let turn = insert_completed_turn(&test_db.pool, session, "restart input", "restart assistant").await;
        let contract_id = Uuid::new_v4();
        sqlx::query("INSERT INTO generic_contracts (id, session_id, contract_type, canonical_payload, status, active_version) VALUES ($1,$2,'restart-contract',$3,'active','v1')")
            .bind(contract_id)
            .bind(session)
            .bind(json!({"canonical":"requirements"}))
            .execute(&test_db.pool)
            .await
            .expect("contract");
        let packet_id = crate::lifecycle_hooks::record_runtime_packet(&test_db.pool, Some("restart-project"), Some(session), None, Some(turn), "restart.packet", "pending", json!({"restart":true}), None, json!({}), "restart-packet").await.expect("packet");
        let envelope_id = crate::lifecycle_hooks::route_packet_envelope(&test_db.pool, packet_id, "system_notice", Some(session), Some(session), None, "pending", json!({})).await.expect("envelope");
        let subagent = crate::lifecycle_hooks::ensure_subagent(&test_db.pool, session, "restart-reviewer", "restart-workflow", "genericReviewer", "runtime-no-rg", json!({}), json!({})).await.expect("subagent");
        let lease = crate::lifecycle_hooks::reserve_resource(&test_db.pool, Some(session), json!({"resourceType":"iosSimulator","resourceId":"restart-sim","status":"assigned"}), "restart-lease").await.expect("lease");
        crate::lifecycle_hooks::apply_hook_intents(&test_db.pool, Some("restart-project"), Some(session), Some(turn), crate::lifecycle_hooks::LifecycleHook::OnTurnComplete, &[crate::lifecycle_hooks::HookIntent { intent_type: "add_turn_obligation".to_string(), key: Some("restart-obligation".to_string()), payload: json!({"obligationType":"restartNotify"}), idempotency_key: Some("restart-obligation".to_string()) }]).await.expect("obligation");
        let context = crate::lifecycle_hooks::hook_context_from_session_summary(
            session,
            Some("restart-project".to_string()),
            "source".to_string(),
            None,
            false,
            &role,
            ".".to_string(),
            Some(".".to_string()),
            crate::lifecycle_hooks::LifecycleHook::OnModelRequest,
        );
        let before = crate::lifecycle_hooks::evaluate_active_lifecycle_hooks(&test_db.pool, "restart-project", Some(session), Some(turn), crate::lifecycle_hooks::LifecycleHook::OnModelRequest, &context).await.expect("before restart eval");
        assert_eq!(before[0].validation_status, "valid");
        let url = test_db.url.clone();
        let restarted = db::connect(&url).await.expect("reconnect runtime db");
        let persisted: (i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
              (SELECT COUNT(*) FROM project_runtime_config_versions WHERE id=$1 AND activation_status='active'),
              (SELECT COUNT(*) FROM project_runtime_hook_bindings WHERE config_version_id=$1 AND status='active'),
              (SELECT COUNT(*) FROM generic_contracts WHERE id=$2 AND status='active'),
              (SELECT COUNT(*) FROM runtime_packets WHERE id=$3),
              (SELECT COUNT(*) FROM runtime_envelopes WHERE id=$4),
              (SELECT COUNT(*) FROM generic_subagents WHERE subagent_session_id=$5 AND lifecycle_status='active'),
              (SELECT COUNT(*) FROM resource_leases WHERE id=$6 AND status='assigned'),
              (SELECT COUNT(*) FROM turn_obligations WHERE session_id=$7 AND idempotency_key='restart-obligation')
            "#,
        )
        .bind(version_id)
        .bind(contract_id)
        .bind(packet_id)
        .bind(envelope_id)
        .bind(subagent)
        .bind(lease)
        .bind(session)
        .fetch_one(&restarted)
        .await
        .expect("persisted state");
        assert_eq!(persisted, (1, 1, 1, 1, 1, 1, 1, 1));
        let subagent_after_restart = crate::lifecycle_hooks::ensure_subagent(&restarted, session, "restart-reviewer", "restart-workflow", "genericReviewer", "runtime-no-rg", json!({}), json!({})).await.expect("ensure after restart");
        assert_eq!(subagent_after_restart, subagent, "ensure_subagent must reuse persisted generic subagent after restart/replay");
        let ensure_intent = crate::lifecycle_hooks::HookIntent {
            intent_type: "ensure_subagent".to_string(),
            key: Some("retry-subagent".to_string()),
            payload: json!({"subagentKey":"retry-reviewer","workflowIdentity":"retry-workflow","subagentKind":"genericReviewer","roleId":"runtime-no-rg"}),
            idempotency_key: Some("retry-subagent-key".to_string()),
        };
        crate::lifecycle_hooks::apply_hook_intents(&restarted, Some("restart-project"), Some(session), Some(turn), crate::lifecycle_hooks::LifecycleHook::OnPacketRecorded, &[ensure_intent.clone()]).await.expect("ensure retry first");
        crate::lifecycle_hooks::apply_hook_intents(&restarted, Some("restart-project"), Some(session), Some(turn), crate::lifecycle_hooks::LifecycleHook::OnPacketRecorded, &[ensure_intent]).await.expect("ensure retry replay");
        let retry_subagents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM generic_subagents WHERE parent_session_id=$1 AND subagent_key='retry-reviewer' AND workflow_identity='retry-workflow'")
            .bind(session)
            .fetch_one(&restarted)
            .await
            .expect("retry subagent count");
        assert_eq!(retry_subagents, 1, "hook retry and repeated packet projection must not double-create subagents");
        crate::lifecycle_hooks::apply_hook_intents(&restarted, Some("restart-project"), Some(session), Some(turn), crate::lifecycle_hooks::LifecycleHook::OnTurnComplete, &[crate::lifecycle_hooks::HookIntent { intent_type: "add_turn_obligation".to_string(), key: Some("restart-obligation".to_string()), payload: json!({"obligationType":"restartNotify","replayed":true}), idempotency_key: Some("restart-obligation".to_string()) }]).await.expect("replayed obligation");
        let obligations_after_replay: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turn_obligations WHERE session_id=$1 AND idempotency_key='restart-obligation'")
            .bind(session)
            .fetch_one(&restarted)
            .await
            .expect("obligation count after replay");
        assert_eq!(obligations_after_replay, 1, "replayed hook obligations preserve idempotency identity");
        let after = crate::lifecycle_hooks::evaluate_active_lifecycle_hooks(&restarted, "restart-project", Some(session), Some(turn), crate::lifecycle_hooks::LifecycleHook::OnModelRequest, &context).await.expect("after restart eval");
        assert_eq!(after[0].context_hash, before[0].context_hash);
        assert_eq!(after[0].returned_intents[0].intent_type, before[0].returned_intents[0].intent_type);
        assert_eq!(after[0].returned_intents[0].key, before[0].returned_intents[0].key);
        assert_eq!(after[0].returned_intents[0].payload, before[0].returned_intents[0].payload);
        let completed_obligations = crate::lifecycle_hooks::process_turn_completion_obligations(&restarted, session, Some(turn)).await.expect("process obligations after restart");
        assert_eq!(completed_obligations, 1, "restart replay keeps obligations effective after reconnect");
        let completed_obligation_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turn_obligations WHERE session_id=$1 AND idempotency_key='restart-obligation' AND status='completed'")
            .bind(session)
            .fetch_one(&restarted)
            .await
            .expect("completed obligation rows");
        assert_eq!(completed_obligation_rows, 1);
        restarted.close().await;
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn session_archive_cleans_lifecycle_resources_and_project_subagent_projection() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session = db::new_session(&test_db.pool, &role, Some("cleanup-project"), ".", Some("."), None, None).await.expect("session");
        let subagent = crate::lifecycle_hooks::ensure_subagent(&test_db.pool, session, "cleanup-reviewer", "cleanup-wf", "genericReviewer", "runtime-no-rg", json!({}), json!({})).await.expect("ensure subagent");
        let lease = crate::lifecycle_hooks::reserve_resource(&test_db.pool, Some(session), json!({"resourceType":"iosSimulator","resourceId":"cleanup-sim","status":"assigned","leasePurpose":"cleanup"}), "cleanup-lease").await.expect("lease");
        sqlx::query("INSERT INTO managed_processes (id, handle, session_id, binary_name, argv, cwd, status, end_of_turn_behavior, end_of_session_behavior) VALUES ($1,'cleanup-proc',$2,'python',$3,'.','running','continue','terminate')")
            .bind(Uuid::new_v4())
            .bind(session)
            .bind(json!(["-c","print(1)"]))
            .execute(&test_db.pool)
            .await
            .expect("managed process");
        db::archive_session(&test_db.pool, session).await.expect("archive session");
        let lease_status: String = sqlx::query_scalar("SELECT status FROM resource_leases WHERE id=$1")
            .bind(lease)
            .fetch_one(&test_db.pool)
            .await
            .expect("lease status");
        assert_eq!(lease_status, "released");
        let subagent_record = db::session_record(&test_db.pool, subagent).await.expect("subagent deactivated by lifecycle");
        assert_eq!(subagent_record.status, "stopped");
        let subagent_lifecycle_status: String = sqlx::query_scalar("SELECT lifecycle_status FROM generic_subagents WHERE parent_session_id=$1 AND subagent_session_id=$2")
            .bind(session)
            .bind(subagent)
            .fetch_one(&test_db.pool)
            .await
            .expect("subagent lifecycle status");
        assert_eq!(subagent_lifecycle_status, "inactive");
        let process_status: String = sqlx::query_scalar("SELECT status FROM managed_processes WHERE session_id=$1 AND handle='cleanup-proc'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("process status");
        assert_eq!(process_status, "sessionTerminated");
        let cleanup_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='lifecycle.cleanup'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("cleanup event");
        assert_eq!(cleanup_events, 1);

        let archive_session = db::new_session(&test_db.pool, &role, Some("cleanup-project"), ".", Some("."), None, None).await.expect("archive session");
        let archive_lease = crate::lifecycle_hooks::reserve_resource(&test_db.pool, Some(archive_session), json!({"resourceType":"iosSimulator","resourceId":"archive-sim","status":"reserved","leasePurpose":"archive"}), "archive-lease").await.expect("archive lease");
        db::archive_session(&test_db.pool, archive_session).await.expect("archive session cleanup");
        let archive_lease_status: String = sqlx::query_scalar("SELECT status FROM resource_leases WHERE id=$1")
            .bind(archive_lease)
            .fetch_one(&test_db.pool)
            .await
            .expect("archive lease status");
        assert_eq!(archive_lease_status, "released");
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn resource_lease_intents_reject_theft_between_sessions() {
        let test_db = validation_db().await;
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let owner = db::new_session(&test_db.pool, &role, Some("lease-theft"), ".", Some("."), None, None).await.expect("owner");
        let thief = db::new_session(&test_db.pool, &role, Some("lease-theft"), ".", Some("."), None, None).await.expect("thief");
        let lease = crate::lifecycle_hooks::reserve_resource(
            &test_db.pool,
            Some(owner),
            json!({"resourceType":"iosSimulator","resourceId":"sim-theft","status":"assigned","leasePurpose":"owner"}),
            "owner-lease",
        ).await.expect("owner lease");
        let stolen = crate::lifecycle_hooks::reserve_resource(
            &test_db.pool,
            Some(thief),
            json!({"resourceType":"iosSimulator","resourceId":"sim-theft","status":"assigned","leasePurpose":"thief"}),
            "thief-lease",
        ).await;
        assert!(stolen.is_err());
        let still_owner: Uuid = sqlx::query_scalar("SELECT owning_session_id FROM resource_leases WHERE id=$1")
            .bind(lease)
            .fetch_one(&test_db.pool)
            .await
            .expect("lease owner");
        assert_eq!(still_owner, owner);
        test_db.cleanup().await;
    }

    #[tokio::test]
    async fn runtime_config_change_request_session_overrides_invalid_output_and_turn_obligations_are_durable() {
        let test_db = validation_db().await;
        db::create_project(&test_db.pool, "runtime-change", "Runtime Change", ".", ".", None, "gpt-5.4-mini").await.expect("project");
        let role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        let session = db::new_session(&test_db.pool, &role, Some("runtime-change"), ".", Some("."), None, None).await.expect("session");
        let packet = crate::lifecycle_hooks::request_project_runtime_config_change(
            &test_db.pool,
            "runtime-change",
            session,
            "x = 1",
            json!({"hooks":[{"name":"on_model_request","source":"x = 1"}]}),
            "Need project hook review",
        ).await.expect("request config change");
        let packet_status: String = sqlx::query_scalar("SELECT status FROM runtime_packets WHERE id=$1 AND packet_type='project_runtime.config_change_request'")
            .bind(packet)
            .fetch_one(&test_db.pool)
            .await
            .expect("config change packet");
        assert_eq!(packet_status, "reviewable");

        let overrides = crate::lifecycle_hooks::set_session_hook_overrides(&test_db.pool, session, json!({"on_model_request":"session-hook"})).await.expect("override");
        assert_eq!(overrides["on_model_request"], "session-hook");

        let turn = insert_completed_turn(&test_db.pool, session, "input", "assistant").await;
        let invalid = crate::lifecycle_hooks::record_invalid_structured_output(
            &test_db.pool,
            Some("runtime-change"),
            Some(session),
            Some(turn),
            "requirements.claim",
            "{not json",
            "expected object",
            "invalid-output-once",
        ).await.expect("invalid packet");
        let invalid_status: (String, Option<String>) = sqlx::query_as("SELECT status, validation_error FROM runtime_packets WHERE id=$1")
            .bind(invalid)
            .fetch_one(&test_db.pool)
            .await
            .expect("invalid packet row");
        assert_eq!(invalid_status.0, "invalid");
        assert_eq!(invalid_status.1.as_deref(), Some("expected object"));
        let corrections: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_stream WHERE session_id=$1 AND event_type='structuredOutput.invalid'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("invalid event");
        assert_eq!(corrections, 1);

        let lease = crate::lifecycle_hooks::reserve_resource(&test_db.pool, Some(session), json!({"resourceType":"iosSimulator","resourceId":"sim-obligation","status":"assigned","leasePurpose":"obligation"}), "lease-obligation").await.expect("lease");
        crate::lifecycle_hooks::apply_hook_intents(
            &test_db.pool,
            Some("runtime-change"),
            Some(session),
            Some(turn),
            crate::lifecycle_hooks::LifecycleHook::OnTurnComplete,
            &[
                crate::lifecycle_hooks::HookIntent { intent_type: "add_turn_obligation".to_string(), key: Some("notice".to_string()), payload: json!({"obligationType":"leaseIdleNotice","message":"lease idle"}), idempotency_key: Some("notice-key".to_string()) },
                crate::lifecycle_hooks::HookIntent { intent_type: "add_turn_obligation".to_string(), key: Some("release".to_string()), payload: json!({"obligationType":"releaseResource","resourceType":"iosSimulator","leaseId": lease.to_string()}), idempotency_key: Some("release-key".to_string()) },
            ],
        ).await.expect("obligations");
        let processed = crate::lifecycle_hooks::process_turn_completion_obligations(&test_db.pool, session, Some(turn)).await.expect("process obligations");
        assert_eq!(processed, 2);
        let lease_status: String = sqlx::query_scalar("SELECT status FROM resource_leases WHERE id=$1")
            .bind(lease)
            .fetch_one(&test_db.pool)
            .await
            .expect("lease released");
        assert_eq!(lease_status, "released");
        let completed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM turn_obligations WHERE session_id=$1 AND status='completed'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("completed obligations");
        assert_eq!(completed, 2);
        test_db.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn execute_code_project_runtime_config_change_affordance_records_reviewable_packet() {
        let test_db = validation_db().await;
        db::create_project(&test_db.pool, "execute-affordance", "Execute Affordance", ".", ".", None, "gpt-5.4-mini").await.expect("project");
        let mut role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        role.policy.insert("project_runtime.request_change".to_string(), crate::roles::ManifestDecision::Allow);
        let session = db::new_session(&test_db.pool, &role, Some("execute-affordance"), ".", Some("."), None, None).await.expect("session");
        let turn = Uuid::new_v4();
        let tool_call = Uuid::new_v4();
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, completed_at) VALUES ($1,$2,'user','request runtime config','completed',now())")
            .bind(turn)
            .bind(session)
            .execute(&test_db.pool)
            .await
            .expect("turn");
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status, started_at) VALUES ($1,$2,$3,'execute_code','runtime-config-change',$4,'running',now())")
            .bind(tool_call)
            .bind(session)
            .bind(turn)
            .bind(json!({}))
            .execute(&test_db.pool)
            .await
            .expect("tool call");
        let temp = tempfile::tempdir().expect("tempdir");
        let root = crate::starlark_host::ExecutionRoot::new(temp.path()).expect("root");
        let script = r#"result = project_runtime.request_config_change("execute-affordance", "x = 1", "{\"hooks\":[{\"name\":\"on_model_request\",\"source\":\"x = 1\"}]}", "operator-requested runtime hook")
print(result)"#;
        let packet = crate::starlark_host::execute_code(&test_db.pool, session, turn, tool_call, script, &root, &role)
            .await
            .expect("execute_code");
        let packet_json = serde_json::to_value(&packet).expect("packet json");
        assert_eq!(packet_json["status"], "completed", "{packet_json}");
        let request_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_packets WHERE source_session_id=$1 AND packet_type='project_runtime.config_change_request' AND status='reviewable'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("reviewable request");
        assert_eq!(request_count, 1);
        test_db.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn project_progenitor_execution_paths_are_project_scoped_and_routed() {
        let test_db = validation_db().await;
        db::create_project(&test_db.pool, "progenitor-project", "Progenitor Project", ".", ".", None, "gpt-5.4-mini").await.expect("project");
        db::create_project(&test_db.pool, "other-project", "Other Project", ".", ".", None, "gpt-5.4-mini").await.expect("other project");
        let mut role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        role.id = "project-progenitor".to_string();
        role.display_name = "Project Progenitor".to_string();
        role.policy.insert("project_runtime.request_change".to_string(), crate::roles::ManifestDecision::Allow);
        role.policy.insert("tooling.request".to_string(), crate::roles::ManifestDecision::Allow);
        let session = db::new_session(&test_db.pool, &role, Some("progenitor-project"), ".", Some("."), None, None).await.expect("session");
        let temp = tempfile::tempdir().expect("tempdir");
        let root = crate::starlark_host::ExecutionRoot::new(temp.path()).expect("root");
        let script = r#"
print(tooling.request("Need project helper", "Need a project-local helper for this project runtime only.", attempted=["checked project commands"], proposed="Add a project-local command bundle.", urgency="normal"))
print(project_runtime.request_config_change("progenitor-project", "x = 1", "{\"hooks\":[{\"name\":\"on_model_request\",\"source\":\"x = 1\"}]}", "project-local progenitor runtime hook"))
"#;
        let (turn, tool_call) = insert_turn_and_tool(&test_db.pool, session, script).await;
        let packet = crate::starlark_host::execute_code(&test_db.pool, session, turn, tool_call, script, &root, &role).await.expect("progenitor script");
        let packet_json = serde_json::to_value(&packet).expect("packet json");
        assert_eq!(packet_json["ok"], true, "{packet_json}");
        let tooling_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_tooling_requests WHERE session_id=$1 AND role_id='project-progenitor' AND project_key='progenitor-project' AND status='routed'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("tooling request count");
        assert_eq!(tooling_count, 1);
        let runtime_request_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_packets WHERE source_session_id=$1 AND project_key='progenitor-project' AND packet_type='project_runtime.config_change_request' AND status='reviewable'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("runtime request count");
        assert_eq!(runtime_request_count, 1);
        let wrong_project = r#"print(project_runtime.request_config_change("other-project", "x = 1", "{\"hooks\":[{\"name\":\"on_model_request\",\"source\":\"x = 1\"}]}", "should be rejected"))"#;
        let (turn, tool_call) = insert_turn_and_tool(&test_db.pool, session, wrong_project).await;
        let packet = crate::starlark_host::execute_code(&test_db.pool, session, turn, tool_call, wrong_project, &root, &role).await.expect("wrong project packet");
        let packet_json = serde_json::to_value(&packet).expect("wrong project json");
        assert_eq!(packet_json["ok"], false, "{packet_json}");
        assert!(!crate::roles::default_tool_bundle_for_role("project-progenitor").contains(&"command_registry.apply"));
        test_db.cleanup().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tooling_requests_route_through_lifecycle_and_create_follow_on_requests() {
        let test_db = validation_db().await;
        db::create_project(&test_db.pool, "tooling-followon", "Tooling Follow-on", ".", ".", None, "gpt-5.4-mini").await.expect("project");
        let mut role = db::current_role_snapshot(&test_db.pool, "runtime-no-rg").await.expect("role");
        role.id = "project-progenitor".to_string();
        role.display_name = "Project Progenitor".to_string();
        role.policy.insert("tooling.request".to_string(), crate::roles::ManifestDecision::Allow);
        let session = db::new_session(&test_db.pool, &role, Some("tooling-followon"), ".", Some("."), None, None).await.expect("session");
        let temp = tempfile::tempdir().expect("tempdir");
        let root = crate::starlark_host::ExecutionRoot::new(temp.path()).expect("root");
        let source = r#"
print(tooling.request("Need command affordance", "Need a project-local command-registry affordance for this project only.", attempted=["checked visible commands"], proposed='{"kind":"command_registry","operation":"add","summary":"Add a project-local validation command"}', urgency="high"))
print(tooling.request("Need runtime config", "Need a project-local runtime hook proposal routed for owner review.", attempted=["validated seed"], proposed='{"kind":"project_runtime_config","projectKey":"tooling-followon","sourceText":"x = 1","manifest":{"hooks":[{"name":"on_model_request","source":"x = 1"}]},"rationale":"project-local follow-on runtime proposal"}', urgency="normal"))
"#;
        let (turn, tool_call) = insert_turn_and_tool(&test_db.pool, session, source).await;
        let packet = crate::starlark_host::execute_code(&test_db.pool, session, turn, tool_call, source, &root, &role).await.expect("tooling follow-on script");
        let packet_json = serde_json::to_value(&packet).expect("packet json");
        assert_eq!(packet_json["ok"], true, "{packet_json}");
        let tooling_packets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_packets WHERE source_session_id=$1 AND project_key='tooling-followon' AND packet_type='tooling.request' AND status='routed'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("tooling runtime packets");
        assert_eq!(tooling_packets, 2);
        let tooling_envelopes: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM runtime_envelopes e JOIN runtime_packets p ON p.id=e.packet_id WHERE p.source_session_id=$1 AND e.envelope_type='tooling_request' AND e.status='pending'"
        )
        .bind(session)
        .fetch_one(&test_db.pool)
        .await
        .expect("tooling envelopes");
        assert_eq!(tooling_envelopes, 2);
        let command_follow_on: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_packets WHERE source_session_id=$1 AND packet_type='command_registry.follow_on_request' AND status='pending_approval' AND routing_metadata->>'approvalPath'='command_registry.request'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("command follow-on");
        assert_eq!(command_follow_on, 1);
        let runtime_follow_on: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runtime_packets WHERE source_session_id=$1 AND project_key='tooling-followon' AND packet_type='project_runtime.config_change_request' AND status='reviewable'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("runtime follow-on");
        assert_eq!(runtime_follow_on, 1);
        let approval_envelopes: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM runtime_envelopes e JOIN runtime_packets p ON p.id=e.packet_id WHERE p.source_session_id=$1 AND e.envelope_type='approval_request' AND e.status='pending'"
        )
        .bind(session)
        .fetch_one(&test_db.pool)
        .await
        .expect("approval envelopes");
        assert_eq!(approval_envelopes, 2);
        let route_with_runtime_packets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM starter_tooling_requests WHERE session_id=$1 AND route ? 'runtimePacketId' AND route ? 'envelopeId' AND route ? 'followOnPacketId'")
            .bind(session)
            .fetch_one(&test_db.pool)
            .await
            .expect("starter routes");
        assert_eq!(route_with_runtime_packets, 2);

        let cross_project = r#"print(tooling.request("Need wrong project", "Need a follow-on that should be rejected by project scoping.", attempted=["checked project"], proposed='{"kind":"project_runtime_config","projectKey":"other-project","sourceText":"x = 1","manifest":{"hooks":[{"name":"on_model_request","source":"x = 1"}]},"rationale":"wrong project"}', urgency="normal"))"#;
        let (turn, tool_call) = insert_turn_and_tool(&test_db.pool, session, cross_project).await;
        let packet = crate::starlark_host::execute_code(&test_db.pool, session, turn, tool_call, cross_project, &root, &role).await.expect("cross-project script");
        let packet_json = serde_json::to_value(&packet).expect("cross-project json");
        assert_eq!(packet_json["ok"], false, "{packet_json}");
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
        requirements::record_requirements_claim_packet(&test_db.pool, source, claim_turn, r#"{"summary":"done","requirements":{"verdict_checked":{"claim":"satisfied","evidence":["e"],"justification":"j","risk":"low"}}}"#).await.expect("claim");
        let reviewer = requirements::status(&test_db.pool, source).await.expect("status").reviewer_session_id.expect("reviewer");
        let verdict_turn = insert_completed_turn(&test_db.pool, reviewer, "verdict", "assistant").await;
        let invalid = r#"{"summary":"bad","requirements":{"verdict_checked":{"verdict":"pass","evidence":["e"],"justification":"j","risk":"low"}},"route":"source"}"#;
        assert!(requirements::record_requirements_verdict_packet(&test_db.pool, reviewer, verdict_turn, invalid).await.expect("verdict processed"));
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
        requirements::record_requirements_claim_packet(&test_db.pool, source, turn, r#"{"summary":"done","requirements":{"reconstructs_claim":{"claim":"satisfied","evidence":["claim evidence"],"justification":"j","risk":"low"}}}"#).await.expect("claim");
        let reviewer = requirements::status(&test_db.pool, source).await.expect("status").reviewer_session_id.expect("reviewer");
        compaction::compact_session_through_turn(&test_db.pool, source, turn, compaction::CompactionBudget::default()).await.expect("compact source");
        let runtime_message = requirements::hook_defined_requirements_runtime_message(&test_db.pool, reviewer).await.expect("runtime message").expect("reviewer schema");
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
        sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status) VALUES ($1,$2,'print(\"ok\")','completed')")
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
        assert_eq!(projected.workflow_memories[0].source_starlark.as_deref(), Some("print(\"ok\")"));
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
        assert_eq!(list[0]["sourcePreview"], "print(\"ok\")");
        assert_eq!(list[0]["provider"], "deterministic");
        assert_eq!(list[0]["dimensions"], workflow_memory::DEFAULT_DIMENSIONS as i64);
        assert_eq!(list[0]["sourceHash"], "hash");
        let response = router.clone().oneshot(Request::builder().uri(format!("/workflow-memories/{memory_id}?sessionId={session_id}")).body(Body::empty()).expect("request")).await.expect("show");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("show body");
        let show: Value = serde_json::from_slice(&bytes).expect("show json");
        assert_eq!(show["sourceStarlark"], "print(\"ok\")");
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
