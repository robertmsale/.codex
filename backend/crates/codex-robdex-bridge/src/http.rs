use std::{
    path::{Path as FsPath, PathBuf},
    sync::Arc,
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        Path,
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::broadcast;
use tower_http::{
    cors::{Any, CorsLayer},
    services::{ServeDir, ServeFile},
};
use tracing::error;

use crate::{
    commands::{
        CommandOutcome, execute_bridge_command, make_app_state_snapshot, make_event_replay_response, orchestrator_agents, orchestrator_approval_decision,
        orchestrator_archive_agent, orchestrator_lookup, orchestrator_pending_approvals, orchestrator_rename_agent,
        orchestrator_request_requirements_review, orchestrator_requirements_status,
        orchestrator_send_message, orchestrator_set_requirements,
        orchestrator_spawn_agent, orchestrator_thread_group_archive,
        orchestrator_thread_group_create, orchestrator_thread_group_delete, orchestrator_thread_group_move_thread,
        orchestrator_thread_group_update, orchestrator_thread_groups, orchestrator_threads,
        orchestrator_warm_handoff, register_live_process, complete_live_process,
        orchestrator_update_worker_metadata, orchestrator_whoami,
    },
    models::{
        BridgeEvent, LiveProcessRecord, MAX_TRANSPORT_MESSAGES_PER_THREAD, PROTOCOL_VERSION, SequencedEvent, SERVER_NAME, SERVER_VERSION, ThreadMessagesResponse,
    },
    runtime::BridgeRuntime,
};

pub fn build_router(runtime: Arc<BridgeRuntime>) -> Router {
    Router::new()
        .route("/health", get(healthz))
        .route("/healthz", get(healthz))
        .route("/info", get(info))
        .route("/models", get(models))
        .route("/state/app", get(app_state))
        .route("/state/snapshot", get(snapshot))
        .route("/workbench/bootstrap", get(workbench_bootstrap))
        .route("/services/qa-harness/summary", get(legacy_qa_harness_http))
        .route("/state/project-catalog", post(save_project_catalog_http))
        .route("/projects", post(project_create_http))
        .route("/projects/select", post(project_select_http))
        .route("/projects/{project_id}", post(project_update_http).delete(project_delete_http))
        .route("/projects/{project_id}/hook-logs", get(project_hook_logs_http).delete(project_hook_logs_clear_http))
        .route("/projects/{project_id}/orchestrator", post(project_orchestrator_http))
        .route("/threads", post(thread_create_http))
        .route("/threads/{thread_id}", delete(thread_archive_http))
        .route("/threads/{thread_id}/name", post(thread_name_set_http))
        .route("/threads/{thread_id}/metadata", post(thread_metadata_update_http))
        .route("/threads/{thread_id}/compact", post(thread_compact_http))
        .route("/threads/{thread_id}/commands/terminate", post(thread_command_terminate_http))
        .route("/threads/{thread_id}/qa/devices", get(legacy_qa_harness_http))
        .route("/threads/{thread_id}/qa/devices/{device_key}/reserve", post(legacy_qa_harness_http))
        .route("/threads/{thread_id}/qa/devices/{device_key}/reboot", post(legacy_qa_harness_http))
        .route("/threads/{thread_id}/processes/register", post(thread_process_register_http))
        .route("/threads/{thread_id}/processes/{process_id}/complete", post(thread_process_complete_http))
        .route("/threads/{thread_id}/running-state", post(thread_running_state_http))
        .route("/threads/{thread_id}/interrupt", post(thread_interrupt_http))
        .route("/mcp/refresh", post(mcp_refresh_http))
        .route("/threads/{thread_id}/messages", post(thread_message_create_http))
        .route("/uploads/images", post(image_upload_http))
        .route("/uploads/images/instant", post(image_upload_http))
        .route("/images/thumbnail", get(image_thumbnail_http))
        .route("/images/image", get(image_file_http))
        .route("/threads/messages", get(thread_messages))
        .route("/events/replay", get(replay_events))
        .route("/orchestrator/whoami", get(orchestrator_whoami_route))
        .route("/orchestrator/lookup", get(orchestrator_lookup_route))
        .route("/orchestrator/threads", get(orchestrator_threads_route))
        .route("/orchestrator/agents", get(orchestrator_agents_route))
        .route("/orchestrator/pending-approvals", get(orchestrator_pending_approvals_route))
        .route("/orchestrator/thread-groups", get(orchestrator_thread_groups_route))
        .route("/orchestrator/thread-groups/create", post(orchestrator_thread_group_create_route))
        .route("/orchestrator/thread-groups/update", post(orchestrator_thread_group_update_route))
        .route("/orchestrator/thread-groups/move-thread", post(orchestrator_thread_group_move_thread_route))
        .route("/orchestrator/thread-groups/delete", post(orchestrator_thread_group_delete_route))
        .route("/orchestrator/thread-groups/archive", post(orchestrator_thread_group_archive_route))
        .route("/orchestrator/spawn-agent", post(orchestrator_spawn_agent_route))
        .route("/orchestrator/warm-handoff", post(orchestrator_warm_handoff_route))
        .route("/orchestrator/agent-message", post(orchestrator_agent_message_route))
        .route("/orchestrator/archive-agent", post(orchestrator_archive_agent_route))
        .route("/orchestrator/rename-agent", post(orchestrator_rename_agent_route))
        .route("/orchestrator/worker-metadata", post(orchestrator_worker_metadata_route))
        .route("/orchestrator/requirements/set", post(orchestrator_requirements_set_route))
        .route("/orchestrator/requirements/status", post(orchestrator_requirements_status_route))
        .route("/orchestrator/requirements/request-review", post(orchestrator_requirements_request_review_route))
        .route("/orchestrator/approval-decision", post(orchestrator_approval_decision_route))
        .route("/ws", get(ws_upgrade))
        .route("/workbench/ws", get(workbench_ws_upgrade))
        .fallback_service(ServeDir::new(web_build_dir()).not_found_service(ServeFile::new(
            web_build_dir().join("index.html"),
        )))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(middleware::map_response(add_no_store_headers))
        .with_state(runtime)
}

async fn add_no_store_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate, max-age=0"),
    );
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(header::EXPIRES, HeaderValue::from_static("0"));
    response
}

fn web_build_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../frontend/robdex_app/build/web")
}

async fn healthz() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "service": "codex-robdex-bridge",
            "status": "ok",
            "phase": "fanout"
        })),
    )
}

async fn info(State(runtime): State<Arc<BridgeRuntime>>) -> Json<crate::models::BridgeInfo> {
    Json(runtime.info().await)
}

async fn snapshot(State(runtime): State<Arc<BridgeRuntime>>) -> impl IntoResponse {
    match runtime.snapshot().await {
        Ok(snapshot) => (StatusCode::OK, Json(snapshot)).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn app_state(State(runtime): State<Arc<BridgeRuntime>>) -> impl IntoResponse {
    (StatusCode::OK, Json(runtime.state_document_value().await)).into_response()
}

async fn models(State(runtime): State<Arc<BridgeRuntime>>) -> impl IntoResponse {
    match runtime.cached_model_list().await {
        Ok(models) => (StatusCode::OK, Json(models)).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn workbench_bootstrap(State(runtime): State<Arc<BridgeRuntime>>) -> impl IntoResponse {
    (StatusCode::OK, Json(runtime.workbench_snapshot_value().await)).into_response()
}

async fn legacy_qa_harness_http() -> impl IntoResponse {
    (
        StatusCode::GONE,
        Json(json!({
            "ok": false,
            "status": "deprecated",
            "error": "Managed QA harness and flutter-sim broker endpoints are deprecated. QA agents should use an assigned worktree and device UDID with designer-runtime tooling.",
        })),
    )
        .into_response()
}

async fn save_project_catalog_http(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(catalog): Json<Value>,
) -> impl IntoResponse {
    match execute_bridge_command(
        &runtime,
        "saveProjectCatalog",
        json!({
            "projectCatalog": catalog,
        }),
    )
    .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn project_create_http(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match execute_bridge_command(&runtime, "projectCreate", payload).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn project_select_http(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match execute_bridge_command(&runtime, "projectSelect", payload).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn project_update_http(
    Path(project_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let body = match payload {
        Value::Object(mut object) => {
            object.insert("projectId".to_string(), Value::String(project_id));
            Value::Object(object)
        }
        _ => json!({ "projectId": project_id }),
    };
    match execute_bridge_command(&runtime, "projectUpdate", body).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn project_delete_http(
    Path(project_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
) -> impl IntoResponse {
    match execute_bridge_command(
        &runtime,
        "projectDelete",
        json!({ "projectId": project_id }),
    )
    .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn project_hook_logs_http(
    Path(project_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
) -> impl IntoResponse {
    let state = runtime.state_document_value().await;
    let Some(project) = state
        .get("projects")
        .and_then(Value::as_object)
        .and_then(|projects| projects.get(&project_id))
        .and_then(Value::as_object)
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": format!("unknown project `{project_id}`") })),
        )
            .into_response();
    };
    let logs = project
        .get("robdexRecentHookTelemetry")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "projectId": project_id,
            "logs": logs,
        })),
    )
        .into_response()
}

async fn project_hook_logs_clear_http(
    Path(project_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
) -> impl IntoResponse {
    let mut state = runtime.state_document_value().await;
    let Some(project) = state
        .get_mut("projects")
        .and_then(Value::as_object_mut)
        .and_then(|projects| projects.get_mut(&project_id))
        .and_then(Value::as_object_mut)
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "ok": false, "error": format!("unknown project `{project_id}`") })),
        )
            .into_response();
    };
    project.insert("robdexRecentHookTelemetry".to_string(), Value::Array(Vec::new()));
    match runtime.persist_state_document(state.clone()).await {
        Ok(()) => {
            runtime
                .push_event(crate::models::BridgeEvent::AppStateSnapshot { state })
                .await;
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "projectId": project_id,
                    "cleared": true,
                })),
            )
                .into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": error.to_string() })),
        )
            .into_response(),
    }
}

async fn project_orchestrator_http(
    Path(_project_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match execute_bridge_command(
        &runtime,
        "setOrchestratorThread",
        json!({
            "threadId": payload.get("threadId").cloned().unwrap_or(Value::Null),
            "projectPath": payload.get("projectPath").cloned().unwrap_or(Value::Null),
        }),
    )
    .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn thread_create_http(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match execute_bridge_command(&runtime, "threadCreate", payload).await {
        Ok(outcome) => (StatusCode::OK, Json(outcome.payload["payload"].clone())).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn thread_message_create_http(
    Path(thread_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let body = match payload {
        Value::Object(mut object) => {
            object.insert("threadId".to_string(), Value::String(thread_id));
            Value::Object(object)
        }
        _ => json!({ "threadId": thread_id }),
    };
    match execute_bridge_command(&runtime, "threadMessageCreate", body).await {
        Ok(outcome) => (StatusCode::OK, Json(outcome.payload["payload"].clone())).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct ImageUploadQuery {
    filename: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SavedImageQuery {
    saved_path: String,
}

async fn image_upload_http(
    State(_runtime): State<Arc<BridgeRuntime>>,
    Query(query): Query<ImageUploadQuery>,
    body: Bytes,
) -> impl IntoResponse {
    if body.is_empty() {
        return (StatusCode::BAD_REQUEST, "image body is required").into_response();
    }

    let source_name = query.filename.as_deref().unwrap_or("image");
    let extension = FsPath::new(source_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| {
            matches!(
                value.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "heic" | "heif"
            )
        })
        .unwrap_or_else(|| "png".to_string());

    let upload_dir = std::env::temp_dir().join("robdex-uploads");
    if let Err(error) = tokio::fs::create_dir_all(&upload_dir).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
    }

    let now = crate::commands::unix_now();
    let target = upload_dir.join(format!("upload-{now}-{}.{}", uuid_suffix(), extension));
    if let Err(error) = tokio::fs::write(&target, body).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
    }

    (
        StatusCode::OK,
        Json(json!({
            "path": target.to_string_lossy().into_owned(),
        })),
    )
        .into_response()
}

async fn image_file_http(Query(query): Query<SavedImageQuery>) -> impl IntoResponse {
    let path = FsPath::new(query.saved_path.trim());
    if !path.is_absolute() {
        return (StatusCode::BAD_REQUEST, "saved_path must be absolute").into_response();
    }

    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) => return (StatusCode::NOT_FOUND, error.to_string()).into_response(),
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, image_content_type(path))],
        bytes,
    )
        .into_response()
}

async fn image_thumbnail_http(Query(query): Query<SavedImageQuery>) -> impl IntoResponse {
    let path = FsPath::new(query.saved_path.trim());
    if !path.is_absolute() {
        return (StatusCode::BAD_REQUEST, "saved_path must be absolute").into_response();
    }

    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) => return (StatusCode::NOT_FOUND, error.to_string()).into_response(),
    };
    let image = match image::load_from_memory(&bytes) {
        Ok(image) => image,
        Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    };
    let thumbnail = image.thumbnail(100, 100);
    let mut output = std::io::Cursor::new(Vec::new());
    if let Err(error) = thumbnail.write_to(&mut output, image::ImageFormat::Png) {
        return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
    }

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/png")],
        output.into_inner(),
    )
        .into_response()
}

fn image_content_type(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("heic" | "heif") => "image/heif",
        _ => "image/png",
    }
}

fn uuid_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string()
}

async fn thread_archive_http(
    Path(thread_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
) -> impl IntoResponse {
    match execute_bridge_command(&runtime, "threadArchive", json!({ "threadId": thread_id })).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn thread_name_set_http(
    Path(thread_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match execute_bridge_command(
        &runtime,
        "threadNameSet",
        json!({
            "threadId": thread_id,
            "name": payload.get("name").cloned().unwrap_or(Value::Null),
        }),
    )
    .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn thread_metadata_update_http(
    Path(thread_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match execute_bridge_command(
        &runtime,
        "threadMetadataUpdate",
        json!({
        "threadId": thread_id,
        "role": payload.get("role").cloned().unwrap_or(Value::Null),
        "approvalPolicy": payload.get("approvalPolicy").cloned().unwrap_or(Value::Null),
        "sandboxMode": payload.get("sandboxMode").cloned().unwrap_or(Value::Null),
            "networkAccess": payload.get("networkAccess").cloned().unwrap_or(Value::Null),
            "modelID": payload.get("modelID").cloned().unwrap_or(Value::Null),
            "reasoningEffort": payload.get("reasoningEffort").cloned().unwrap_or(Value::Null),
            "serviceTier": payload.get("serviceTier").cloned().unwrap_or(Value::Null),
        }),
    )
    .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn thread_compact_http(
    Path(thread_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
) -> impl IntoResponse {
    match execute_bridge_command(
        &runtime,
        "threadCompactStart",
        json!({
            "threadId": thread_id,
        }),
    )
    .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn thread_command_terminate_http(
    Path(thread_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match execute_bridge_command(
        &runtime,
        "commandExecutionTerminate",
        json!({
            "threadId": thread_id,
            "processId": payload.get("processId").cloned().unwrap_or(Value::Null),
        }),
    )
    .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterLiveProcessRequest {
    pid: i64,
    process_group_id: Option<i64>,
    command: String,
    cwd: Option<String>,
    started_at: Option<u64>,
}

async fn thread_process_register_http(
    Path(thread_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<RegisterLiveProcessRequest>,
) -> impl IntoResponse {
    let process = LiveProcessRecord {
        process_id: payload.pid.to_string(),
        pid: payload.pid,
        process_group_id: payload.process_group_id,
        command: payload.command,
        cwd: payload.cwd,
        started_at: payload.started_at.unwrap_or_else(crate::commands::unix_now),
    };
    match register_live_process(&runtime, &thread_id, process).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn thread_process_complete_http(
    Path((thread_id, process_id)): Path<(String, String)>,
    State(runtime): State<Arc<BridgeRuntime>>,
) -> impl IntoResponse {
    match complete_live_process(&runtime, &thread_id, &process_id).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn thread_running_state_http(
    Path(thread_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match execute_bridge_command(
        &runtime,
        "threadRunningStateSet",
        json!({
            "threadId": thread_id,
            "running": payload.get("running").cloned().unwrap_or(Value::Bool(false)),
        }),
    )
    .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn thread_interrupt_http(
    Path(thread_id): Path<String>,
    State(runtime): State<Arc<BridgeRuntime>>,
) -> impl IntoResponse {
    match execute_bridge_command(
        &runtime,
        "turnInterrupt",
        json!({
            "threadId": thread_id,
        }),
    )
    .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

async fn mcp_refresh_http(State(runtime): State<Arc<BridgeRuntime>>) -> impl IntoResponse {
    match execute_bridge_command(&runtime, "mcpRefresh", json!({})).await {
        Ok(outcome) => (StatusCode::OK, Json(outcome.payload["payload"].clone())).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct ThreadMessagesQuery {
    #[serde(rename = "threadId")]
    thread_id_camel: Option<String>,
    thread_id: Option<String>,
    limit: Option<String>,
}

async fn thread_messages(
    State(runtime): State<Arc<BridgeRuntime>>,
    Query(query): Query<ThreadMessagesQuery>,
) -> impl IntoResponse {
    let thread_id = query
        .thread_id_camel
        .as_deref()
        .or(query.thread_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(thread_id) = thread_id else {
        return (StatusCode::BAD_REQUEST, "threadId is required").into_response();
    };

    let limit = match query.limit.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        Some(raw) => match raw.parse::<usize>() {
            Ok(value) => Some(value),
            Err(_) => return (StatusCode::BAD_REQUEST, "limit must be an integer").into_response(),
        },
        None => None,
    };

    match runtime.thread_messages(thread_id, limit).await {
        Ok(Some(messages)) => (StatusCode::OK, Json(messages)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "thread not found").into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct ReplayQuery {
    since: Option<u64>,
}

async fn replay_events(
    State(runtime): State<Arc<BridgeRuntime>>,
    Query(query): Query<ReplayQuery>,
) -> impl IntoResponse {
    match make_event_replay_response(&runtime, query.since).await {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrchestratorWhoAmIQuery {
    sender_thread_id: Option<String>,
    thread_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrchestratorLookupQuery {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrchestratorThreadsQuery {
    path: Option<String>,
    project_path: Option<String>,
    cwd: Option<String>,
    include_archived: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrchestratorAgentsQuery {
    sender_thread_id: Option<String>,
    thread_id: Option<String>,
    include_archived: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrchestratorPendingApprovalsQuery {
    sender_thread_id: Option<String>,
    thread_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrchestratorThreadGroupsQuery {
    sender_thread_id: Option<String>,
    thread_id: Option<String>,
    project_path: Option<String>,
}

async fn orchestrator_whoami_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Query(query): Query<OrchestratorWhoAmIQuery>,
) -> impl IntoResponse {
    match require_sender_thread(query.sender_thread_id.as_deref().or(query.thread_id.as_deref())) {
        Ok(thread_id) => match orchestrator_whoami(&runtime, thread_id).await {
            Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        Err(error) => map_bad_request(error),
    }
}

async fn orchestrator_lookup_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Query(query): Query<OrchestratorLookupQuery>,
) -> impl IntoResponse {
    match orchestrator_lookup(&runtime, &query.path).await {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(error) => map_orchestrator_error(error.to_string()),
    }
}

async fn orchestrator_threads_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Query(query): Query<OrchestratorThreadsQuery>,
) -> impl IntoResponse {
    let requested = query
        .path
        .as_deref()
        .or(query.project_path.as_deref())
        .or(query.cwd.as_deref());
    let include_archived = query.include_archived.as_deref() == Some("1");
    match orchestrator_threads(&runtime, requested, include_archived).await {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(error) => map_orchestrator_error(error.to_string()),
    }
}

async fn orchestrator_agents_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Query(query): Query<OrchestratorAgentsQuery>,
) -> impl IntoResponse {
    let include_archived = query.include_archived.as_deref() == Some("1");
    match require_sender_thread(query.sender_thread_id.as_deref().or(query.thread_id.as_deref())) {
        Ok(thread_id) => match orchestrator_agents(&runtime, thread_id, include_archived).await {
            Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        Err(error) => map_bad_request(error),
    }
}

async fn orchestrator_pending_approvals_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Query(query): Query<OrchestratorPendingApprovalsQuery>,
) -> impl IntoResponse {
    match require_sender_thread(query.sender_thread_id.as_deref().or(query.thread_id.as_deref())) {
        Ok(thread_id) => match orchestrator_pending_approvals(&runtime, thread_id).await {
            Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        Err(error) => map_bad_request(error),
    }
}

async fn orchestrator_thread_groups_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Query(query): Query<OrchestratorThreadGroupsQuery>,
) -> impl IntoResponse {
    match require_sender_thread(query.sender_thread_id.as_deref().or(query.thread_id.as_deref())) {
        Ok(thread_id) => match orchestrator_thread_groups(&runtime, thread_id, query.project_path.as_deref()).await {
            Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        Err(error) => map_bad_request(error),
    }
}

async fn orchestrator_thread_group_create_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let title = payload.get("title").and_then(Value::as_str);
    match (require_sender_thread(payload.get("senderThreadId").and_then(Value::as_str)), title) {
        (Ok(sender_thread_id), Some(title)) => match orchestrator_thread_group_create(
            &runtime,
            sender_thread_id,
            payload.get("projectPath").and_then(Value::as_str),
            title,
            payload.get("seedThreadId").and_then(Value::as_str),
        ).await {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        (Err(error), _) => map_bad_request(error),
        (_, None) => map_bad_request("title is required"),
    }
}

async fn orchestrator_thread_group_update_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let group_id = payload.get("groupId").and_then(Value::as_str);
    match (require_sender_thread(payload.get("senderThreadId").and_then(Value::as_str)), group_id) {
        (Ok(sender_thread_id), Some(group_id)) => match orchestrator_thread_group_update(
            &runtime,
            sender_thread_id,
            payload.get("projectPath").and_then(Value::as_str),
            group_id,
            payload.get("title").and_then(Value::as_str),
            payload.get("collapsed").and_then(Value::as_bool),
        ).await {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        (Err(error), _) => map_bad_request(error),
        (_, None) => map_bad_request("groupId is required"),
    }
}

async fn orchestrator_thread_group_move_thread_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let thread_id = payload.get("threadId").and_then(Value::as_str);
    match (require_sender_thread(payload.get("senderThreadId").and_then(Value::as_str)), thread_id) {
        (Ok(sender_thread_id), Some(thread_id)) => match orchestrator_thread_group_move_thread(
            &runtime,
            sender_thread_id,
            payload.get("projectPath").and_then(Value::as_str),
            thread_id,
            payload.get("targetGroupId").and_then(Value::as_str),
        ).await {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        (Err(error), _) => map_bad_request(error),
        (_, None) => map_bad_request("threadId is required"),
    }
}

async fn orchestrator_thread_group_delete_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let group_id = payload.get("groupId").and_then(Value::as_str);
    match (require_sender_thread(payload.get("senderThreadId").and_then(Value::as_str)), group_id) {
        (Ok(sender_thread_id), Some(group_id)) => match orchestrator_thread_group_delete(
            &runtime,
            sender_thread_id,
            payload.get("projectPath").and_then(Value::as_str),
            group_id,
        ).await {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        (Err(error), _) => map_bad_request(error),
        (_, None) => map_bad_request("groupId is required"),
    }
}

async fn orchestrator_thread_group_archive_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let group_id = payload.get("groupId").and_then(Value::as_str);
    match (require_sender_thread(payload.get("senderThreadId").and_then(Value::as_str)), group_id) {
        (Ok(sender_thread_id), Some(group_id)) => match orchestrator_thread_group_archive(
            &runtime,
            sender_thread_id,
            payload.get("projectPath").and_then(Value::as_str),
            group_id,
        ).await {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        (Err(error), _) => map_bad_request(error),
        (_, None) => map_bad_request("groupId is required"),
    }
}

async fn orchestrator_spawn_agent_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let sender = payload.get("senderThreadId").and_then(Value::as_str);
    let name = payload.get("name").and_then(Value::as_str).or(payload.get("displayName").and_then(Value::as_str));
    match (require_sender_thread(sender), name) {
        (Ok(sender_thread_id), Some(name)) => match orchestrator_spawn_agent(
            &runtime,
            sender_thread_id,
            name,
            payload.get("prompt").and_then(Value::as_str).or(payload.get("initialPrompt").and_then(Value::as_str)).unwrap_or(""),
            payload.get("cwd").and_then(Value::as_str),
            payload.get("role").and_then(Value::as_str),
            payload.get("issueNumber").and_then(Value::as_u64),
            payload.get("requirementSet").cloned(),
        ).await {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        (Err(error), _) => map_bad_request(error),
        (_, None) => map_bad_request("name is required"),
    }
}

async fn orchestrator_agent_message_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let sender = payload.get("senderThreadId").and_then(Value::as_str);
    let text = payload.get("text").and_then(Value::as_str);
    match (require_sender_thread(sender), text) {
        (Ok(sender_thread_id), Some(text)) => match orchestrator_send_message(
            &runtime,
            sender_thread_id,
            payload.get("recipientThreadId").and_then(Value::as_str),
            payload.get("recipientName").and_then(Value::as_str),
            payload.get("projectPath").and_then(Value::as_str),
            text,
        ).await {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        (Err(error), _) => map_bad_request(error),
        (_, None) => map_bad_request("text is required"),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrchestratorWarmHandoffPayload {
    sender_thread_id: String,
    recipient_thread_id: Option<String>,
    recipient_name: Option<String>,
    project_path: Option<String>,
    prompt: String,
}

async fn orchestrator_warm_handoff_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<OrchestratorWarmHandoffPayload>,
) -> impl IntoResponse {
    match orchestrator_warm_handoff(
        &runtime,
        &payload.sender_thread_id,
        payload.recipient_thread_id.as_deref(),
        payload.recipient_name.as_deref(),
        payload.project_path.as_deref(),
        &payload.prompt,
    )
    .await
    {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(error) => map_orchestrator_error(error.to_string()),
    }
}

async fn orchestrator_archive_agent_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match require_sender_thread(payload.get("senderThreadId").and_then(Value::as_str)) {
        Ok(sender_thread_id) => match orchestrator_archive_agent(
            &runtime,
            sender_thread_id,
            payload.get("recipientThreadId").and_then(Value::as_str),
            payload.get("recipientName").and_then(Value::as_str),
            payload.get("projectPath").and_then(Value::as_str),
        ).await {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        Err(error) => map_bad_request(error),
    }
}

async fn orchestrator_rename_agent_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let new_name = payload.get("newName").and_then(Value::as_str);
    match (require_sender_thread(payload.get("senderThreadId").and_then(Value::as_str)), new_name) {
        (Ok(sender_thread_id), Some(new_name)) => match orchestrator_rename_agent(
            &runtime,
            sender_thread_id,
            payload.get("recipientThreadId").and_then(Value::as_str),
            payload.get("recipientName").and_then(Value::as_str),
            payload.get("projectPath").and_then(Value::as_str),
            new_name,
        ).await {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        (Err(error), _) => map_bad_request(error),
        (_, None) => map_bad_request("newName is required"),
    }
}

async fn orchestrator_worker_metadata_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match require_sender_thread(payload.get("senderThreadId").and_then(Value::as_str)) {
        Ok(sender_thread_id) => match orchestrator_update_worker_metadata(
            &runtime,
            sender_thread_id,
            payload.get("recipientThreadId").and_then(Value::as_str),
            payload.get("recipientName").and_then(Value::as_str),
            payload.get("projectPath").and_then(Value::as_str),
            &payload,
        ).await {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        Err(error) => map_bad_request(error),
    }
}

async fn orchestrator_requirements_set_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let sender = payload.get("senderThreadId").and_then(Value::as_str);
    let set_payload = if payload.get("requirementSet").is_some() {
        payload.get("requirementSet").cloned()
    } else {
        payload.get("requirements").cloned()
    };
    match (require_sender_thread(sender), set_payload) {
        (Ok(sender_thread_id), Some(set_payload)) => match orchestrator_set_requirements(
            &runtime,
            sender_thread_id,
            payload.get("recipientThreadId").and_then(Value::as_str),
            payload.get("recipientName").and_then(Value::as_str),
            payload.get("projectPath").and_then(Value::as_str),
            set_payload,
        )
        .await
        {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        (Err(error), _) => map_bad_request(error),
        (_, None) => map_bad_request("requirementSet or requirements is required"),
    }
}

async fn orchestrator_requirements_request_review_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match require_sender_thread(payload.get("senderThreadId").and_then(Value::as_str)) {
        Ok(sender_thread_id) => match orchestrator_request_requirements_review(
            &runtime,
            sender_thread_id,
            payload.get("recipientThreadId").and_then(Value::as_str),
            payload.get("recipientName").and_then(Value::as_str),
            payload.get("projectPath").and_then(Value::as_str),
            payload.get("note").and_then(Value::as_str),
        )
        .await
        {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        Err(error) => map_bad_request(error),
    }
}

async fn orchestrator_requirements_status_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    match require_sender_thread(payload.get("senderThreadId").and_then(Value::as_str)) {
        Ok(sender_thread_id) => match orchestrator_requirements_status(
            &runtime,
            sender_thread_id,
            payload.get("recipientThreadId").and_then(Value::as_str),
            payload.get("recipientName").and_then(Value::as_str),
            payload.get("projectPath").and_then(Value::as_str),
        )
        .await
        {
            Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        Err(error) => map_bad_request(error),
    }
}

async fn orchestrator_approval_decision_route(
    State(runtime): State<Arc<BridgeRuntime>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let approval_id = payload.get("approvalId").and_then(Value::as_str);
    let decision = payload.get("decision").and_then(Value::as_str);
    match (
        require_sender_thread(payload.get("senderThreadId").and_then(Value::as_str)),
        approval_id,
        decision,
    ) {
        (Ok(sender_thread_id), Some(approval_id), Some(decision)) => match orchestrator_approval_decision(
            &runtime,
            sender_thread_id,
            approval_id,
            decision,
            payload.get("message").and_then(Value::as_str),
        ).await {
            Ok(body) => (StatusCode::OK, Json(json!({ "result": body }))).into_response(),
            Err(error) => map_orchestrator_error(error.to_string()),
        },
        (Err(error), _, _) => map_bad_request(error),
        _ => map_bad_request("approvalId and decision are required"),
    }
}

fn require_sender_thread(value: Option<&str>) -> Result<&str, &'static str> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("senderThreadId is required.")
}

fn map_bad_request(message: impl Into<String>) -> axum::response::Response {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": message.into() }))).into_response()
}

fn map_orchestrator_error(message: impl Into<String>) -> axum::response::Response {
    (StatusCode::FORBIDDEN, Json(serde_json::json!({ "error": message.into() }))).into_response()
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(runtime): State<Arc<BridgeRuntime>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, runtime))
}

async fn workbench_ws_upgrade(
    ws: WebSocketUpgrade,
    State(runtime): State<Arc<BridgeRuntime>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_workbench_socket(socket, runtime))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BridgeCapabilities {
    supports_multi_client_fanout: bool,
    supports_approval_callbacks: bool,
    supports_experimental_sessions: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HelloAck {
    protocol_version: u32,
    server_name: String,
    server_version: String,
    codex_version: String,
    capabilities: BridgeCapabilities,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandResultPayload {
    id: String,
    payload: Value,
    error_message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "name", rename_all = "camelCase")]
enum OutboundEvent {
    #[serde(rename = "appStateSnapshot")]
    AppStateSnapshot { data: crate::models::BridgeAppStateSnapshot },
    #[serde(rename = "connectionStatus")]
    ConnectionStatus { message: String },
    #[serde(rename = "threadMessagesChanged")]
    ThreadMessagesChanged { data: ThreadMessagesResponse },
    #[serde(rename = "liveProcessesChanged")]
    LiveProcessesChanged { data: Value },
    #[serde(rename = "hookFailure")]
    HookFailure { data: robdex_protocol::HookFailureNotice },
    #[serde(rename = "commandResult")]
    CommandResult { data: CommandResultPayload },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutboundSequencedEvent {
    sequence: Option<u64>,
    event: OutboundEvent,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
enum OutboundEnvelope {
    HelloAck(HelloAck),
    Event(OutboundSequencedEvent),
}

#[derive(Debug, Serialize)]
#[serde(tag = "name", rename_all = "camelCase")]
enum WorkbenchOutboundEvent {
    #[serde(rename = "appStateSnapshot")]
    AppStateSnapshot { data: Value },
    #[serde(rename = "connectionStatus")]
    ConnectionStatus { message: String },
    #[serde(rename = "threadMessagesChanged")]
    ThreadMessagesChanged { data: ThreadMessagesResponse },
    #[serde(rename = "liveProcessesChanged")]
    LiveProcessesChanged { data: Value },
    #[serde(rename = "hookFailure")]
    HookFailure { data: robdex_protocol::HookFailureNotice },
    #[serde(rename = "commandResult")]
    CommandResult { data: CommandResultPayload },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkbenchOutboundSequencedEvent {
    sequence: Option<u64>,
    event: WorkbenchOutboundEvent,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
enum WorkbenchOutboundEnvelope {
    HelloAck(HelloAck),
    Event(WorkbenchOutboundSequencedEvent),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
enum InboundEnvelope {
    Hello(Value),
    Command(InboundCommand),
}

#[derive(Debug, Deserialize)]
struct InboundCommand {
    id: String,
    command: InboundCommandBody,
}

#[derive(Debug, Deserialize)]
struct InboundCommandBody {
    name: String,
    #[serde(default)]
    payload: Value,
}

async fn handle_socket(socket: WebSocket, runtime: Arc<BridgeRuntime>) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = runtime.subscribe_events();
    let mut selected_thread_id: Option<String> = None;

    if send_envelope(&mut sender, OutboundEnvelope::HelloAck(make_hello_ack(&runtime).await))
        .await
        .is_err()
    {
        error!("ws helloAck send failed");
        return;
    }

    let initial_snapshot = match runtime.snapshot().await {
        Ok(_) => match make_app_state_snapshot(&runtime, false).await {
            Ok(snapshot) => snapshot,
            Err(err) => {
                error!("ws initial appStateSnapshot build failed: {err:#}");
                return;
            }
        },
        Err(err) => {
            error!("ws runtime snapshot failed: {err:#}");
            return;
        }
    };
    if send_envelope(
        &mut sender,
        OutboundEnvelope::Event(OutboundSequencedEvent {
            sequence: None,
            event: OutboundEvent::AppStateSnapshot { data: initial_snapshot },
        }),
    )
    .await
    .is_err()
    {
        error!("ws initial appStateSnapshot send failed");
        return;
    }

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                let Some(Ok(message)) = incoming else {
                    break;
                };
                match handle_incoming_message(message, &runtime, &mut sender, &mut selected_thread_id).await {
                    Ok(should_continue) => {
                        if !should_continue {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            outgoing = event_rx.recv() => {
                let event = match outgoing {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if should_send_event(&event, selected_thread_id.as_deref()) {
                    let envelope = match outbound_envelope_from_event(&runtime, event).await {
                        Ok(envelope) => envelope,
                        Err(_) => {
                            error!("dropping outbound bridge event after serialization/build failure");
                            continue;
                        }
                    };
                    if send_envelope(&mut sender, envelope).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

async fn handle_workbench_socket(socket: WebSocket, runtime: Arc<BridgeRuntime>) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = runtime.subscribe_events();
    let mut selected_thread_id: Option<String> = None;

    if send_workbench_envelope(
        &mut sender,
        WorkbenchOutboundEnvelope::HelloAck(make_hello_ack(&runtime).await),
    )
    .await
    .is_err()
    {
        error!("workbench ws helloAck send failed");
        return;
    }

    if send_workbench_envelope(
        &mut sender,
        WorkbenchOutboundEnvelope::Event(WorkbenchOutboundSequencedEvent {
            sequence: None,
            event: WorkbenchOutboundEvent::AppStateSnapshot {
                data: runtime.workbench_snapshot_value().await,
            },
        }),
    )
    .await
    .is_err()
    {
        error!("workbench ws initial appStateSnapshot send failed");
        return;
    }

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                let Some(Ok(message)) = incoming else {
                    break;
                };
                match handle_workbench_incoming_message(message, &runtime, &mut sender, &mut selected_thread_id).await {
                    Ok(should_continue) => {
                        if !should_continue {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            outgoing = event_rx.recv() => {
                let event = match outgoing {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if should_send_event(&event, selected_thread_id.as_deref()) {
                    let envelope = match workbench_outbound_envelope_from_event(&runtime, event).await {
                        Ok(envelope) => envelope,
                        Err(_) => {
                            error!("dropping outbound workbench event after serialization/build failure");
                            continue;
                        }
                    };
                    if send_workbench_envelope(&mut sender, envelope).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

async fn handle_incoming_message(
    message: Message,
    runtime: &Arc<BridgeRuntime>,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    selected_thread_id: &mut Option<String>,
) -> Result<bool, ()> {
    match message {
        Message::Text(text) => {
            let envelope = serde_json::from_str::<InboundEnvelope>(&text).map_err(|_| ())?;
            match envelope {
                InboundEnvelope::Hello(payload) => {
                    drop(payload);
                    send_envelope(sender, OutboundEnvelope::HelloAck(make_hello_ack(runtime).await))
                        .await
                        .map_err(|_| ())?;
                }
                InboundEnvelope::Command(command) => {
                    handle_command(command, runtime, sender, selected_thread_id).await?;
                }
            }
            Ok(true)
        }
        Message::Close(_) => Ok(false),
        Message::Ping(payload) => {
            sender.send(Message::Pong(payload)).await.map_err(|_| ())?;
            Ok(true)
        }
        Message::Pong(_) | Message::Binary(_) => Ok(true),
    }
}

async fn handle_command(
    command: InboundCommand,
    runtime: &Arc<BridgeRuntime>,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    selected_thread_id: &mut Option<String>,
) -> Result<(), ()> {
    if command.command.name == "threadSelectionSet" {
        let thread_id = command
            .command
            .payload
            .get("threadId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        *selected_thread_id = thread_id.clone();

        if let Some(thread_id) = thread_id {
            if let Ok(Some(messages)) = runtime
                .thread_messages(&thread_id, Some(MAX_TRANSPORT_MESSAGES_PER_THREAD))
                .await
            {
                send_envelope(
                    sender,
                    OutboundEnvelope::Event(OutboundSequencedEvent {
                        sequence: None,
                        event: OutboundEvent::ThreadMessagesChanged { data: messages },
                    }),
                )
                .await
                .map_err(|_| ())?;
            }
        }
        return Ok(());
    }
    let outcome = match execute_bridge_command(runtime, &command.command.name, command.command.payload.clone()).await {
        Ok(outcome) => outcome,
        Err(error) => CommandOutcome {
            payload: json!({"type":"empty"}),
            error_message: Some(error.to_string()),
        },
    };

    send_envelope(
        sender,
        OutboundEnvelope::Event(OutboundSequencedEvent {
            sequence: None,
            event: OutboundEvent::CommandResult {
                data: CommandResultPayload {
                    id: command.id,
                    payload: outcome.payload,
                    error_message: outcome.error_message,
                },
            },
        }),
    )
    .await
    .map_err(|_| ())
}

async fn handle_workbench_incoming_message(
    message: Message,
    runtime: &Arc<BridgeRuntime>,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    selected_thread_id: &mut Option<String>,
) -> Result<bool, ()> {
    match message {
        Message::Text(text) => {
            let envelope = serde_json::from_str::<InboundEnvelope>(&text).map_err(|_| ())?;
            match envelope {
                InboundEnvelope::Hello(payload) => {
                    drop(payload);
                    send_workbench_envelope(
                        sender,
                        WorkbenchOutboundEnvelope::HelloAck(make_hello_ack(runtime).await),
                    )
                    .await
                    .map_err(|_| ())?;
                }
                InboundEnvelope::Command(command) => {
                    handle_workbench_command(command, runtime, sender, selected_thread_id).await?;
                }
            }
            Ok(true)
        }
        Message::Close(_) => Ok(false),
        Message::Ping(payload) => {
            sender.send(Message::Pong(payload)).await.map_err(|_| ())?;
            Ok(true)
        }
        Message::Pong(_) | Message::Binary(_) => Ok(true),
    }
}

async fn handle_workbench_command(
    command: InboundCommand,
    runtime: &Arc<BridgeRuntime>,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    selected_thread_id: &mut Option<String>,
) -> Result<(), ()> {
    if command.command.name == "threadSelectionSet" {
        let thread_id = command
            .command
            .payload
            .get("threadId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        *selected_thread_id = thread_id.clone();

        if let Some(thread_id) = thread_id {
            if let Ok(Some(messages)) = runtime
                .thread_messages(&thread_id, Some(MAX_TRANSPORT_MESSAGES_PER_THREAD))
                .await
            {
                send_workbench_envelope(
                    sender,
                    WorkbenchOutboundEnvelope::Event(WorkbenchOutboundSequencedEvent {
                        sequence: None,
                        event: WorkbenchOutboundEvent::ThreadMessagesChanged { data: messages },
                    }),
                )
                .await
                .map_err(|_| ())?;
            }
        }
        return Ok(());
    }

    let outcome = match execute_bridge_command(runtime, &command.command.name, command.command.payload.clone()).await {
        Ok(outcome) => outcome,
        Err(error) => CommandOutcome {
            payload: json!({"type":"empty"}),
            error_message: Some(error.to_string()),
        },
    };

    send_workbench_envelope(
        sender,
        WorkbenchOutboundEnvelope::Event(WorkbenchOutboundSequencedEvent {
            sequence: None,
            event: WorkbenchOutboundEvent::CommandResult {
                data: CommandResultPayload {
                    id: command.id,
                    payload: outcome.payload,
                    error_message: outcome.error_message,
                },
            },
        }),
    )
    .await
    .map_err(|_| ())
}

fn should_send_event(event: &SequencedEvent, selected_thread_id: Option<&str>) -> bool {
    match &event.event {
        BridgeEvent::ThreadMessagesChanged { payload } => match selected_thread_id {
            Some(selected) => selected.trim() == payload.thread_id,
            None => false,
        },
        BridgeEvent::LiveProcessesChanged { payload } => match selected_thread_id {
            Some(selected) => payload.get("threadId").and_then(Value::as_str) == Some(selected.trim()),
            None => false,
        },
        _ => true,
    }
}

async fn outbound_envelope_from_event(
    runtime: &Arc<BridgeRuntime>,
    event: SequencedEvent,
) -> Result<OutboundEnvelope, ()> {
    let outbound = match event.event {
        BridgeEvent::ConnectionStatus { message } => OutboundEvent::ConnectionStatus { message },
        BridgeEvent::AppStateSnapshot { .. } => OutboundEvent::AppStateSnapshot {
            data: make_app_state_snapshot(runtime, false).await.map_err(|_| ())?,
        },
        BridgeEvent::LiveProcessesChanged { payload } => {
            OutboundEvent::LiveProcessesChanged { data: payload }
        }
        BridgeEvent::ThreadMessagesChanged { payload } => {
            OutboundEvent::ThreadMessagesChanged { data: payload }
        }
        BridgeEvent::HookFailure { payload } => OutboundEvent::HookFailure { data: payload },
    };
    Ok(OutboundEnvelope::Event(OutboundSequencedEvent {
        sequence: Some(event.sequence),
        event: outbound,
    }))
}

async fn workbench_outbound_envelope_from_event(
    runtime: &Arc<BridgeRuntime>,
    event: SequencedEvent,
) -> Result<WorkbenchOutboundEnvelope, ()> {
    let outbound = match event.event {
        BridgeEvent::ConnectionStatus { message } => WorkbenchOutboundEvent::ConnectionStatus { message },
        BridgeEvent::AppStateSnapshot { .. } => WorkbenchOutboundEvent::AppStateSnapshot {
            data: runtime.workbench_snapshot_value().await,
        },
        BridgeEvent::LiveProcessesChanged { payload } => {
            WorkbenchOutboundEvent::LiveProcessesChanged { data: payload }
        }
        BridgeEvent::ThreadMessagesChanged { payload } => {
            WorkbenchOutboundEvent::ThreadMessagesChanged { data: payload }
        }
        BridgeEvent::HookFailure { payload } => {
            WorkbenchOutboundEvent::HookFailure { data: payload }
        }
    };
    Ok(WorkbenchOutboundEnvelope::Event(WorkbenchOutboundSequencedEvent {
        sequence: Some(event.sequence),
        event: outbound,
    }))
}

async fn make_hello_ack(runtime: &Arc<BridgeRuntime>) -> HelloAck {
    let info = runtime.info().await;
    HelloAck {
        protocol_version: PROTOCOL_VERSION,
        server_name: SERVER_NAME.to_string(),
        server_version: SERVER_VERSION.to_string(),
        codex_version: info.codex_version,
        capabilities: BridgeCapabilities {
            supports_multi_client_fanout: true,
            supports_approval_callbacks: true,
            supports_experimental_sessions: true,
        },
    }
}

async fn send_envelope(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    envelope: OutboundEnvelope,
) -> Result<(), ()> {
    let payload = serde_json::to_string(&envelope).map_err(|_| ())?;
    sender.send(Message::Text(payload.into())).await.map_err(|_| ())
}

async fn send_workbench_envelope(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    envelope: WorkbenchOutboundEnvelope,
) -> Result<(), ()> {
    let payload = serde_json::to_string(&envelope).map_err(|_| ())?;
    sender.send(Message::Text(payload.into())).await.map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{BridgePaths, BridgeSettings},
        models::BridgeEvent,
        runtime::BridgeRuntime,
    };
    use codex_backend_core::HttpArgs;
    use futures_util::StreamExt;
    use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, path::PathBuf};
    use tempfile::TempDir;
    use tokio::net::TcpListener;
    use tokio_tungstenite::connect_async;

    async fn spawn_server(runtime: Arc<BridgeRuntime>) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            axum::serve(listener, build_router(runtime)).await.expect("serve");
        });
        addr
    }

    fn sample_settings(root: &TempDir) -> BridgeSettings {
        BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url: "ws://127.0.0.1:4200".to_string(),
            project_path: root.path().to_path_buf(),
            cwd: root.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(root.path()).join("state")),
        }
    }

    #[tokio::test]
    async fn ws_sends_hello_ack_and_initial_snapshot() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp)).await.expect("runtime");
        let addr = spawn_server(runtime).await;

        let (mut socket, _) = connect_async(format!("ws://{addr}/ws")).await.expect("connect");
        let hello = socket.next().await.expect("hello").expect("hello frame");
        let snapshot = socket.next().await.expect("snapshot").expect("snapshot frame");

        let hello_text = match hello {
            tokio_tungstenite::tungstenite::Message::Text(text) => text,
            other => panic!("unexpected hello frame: {other:?}"),
        };
        let snapshot_text = match snapshot {
            tokio_tungstenite::tungstenite::Message::Text(text) => text,
            other => panic!("unexpected snapshot frame: {other:?}"),
        };

        assert!(hello_text.contains("\"type\":\"helloAck\""));
        assert!(snapshot_text.contains("\"appStateSnapshot\""));
    }

    #[tokio::test]
    async fn ws_forwards_live_events() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp)).await.expect("runtime");
        let addr = spawn_server(runtime.clone()).await;

        let (mut socket, _) = connect_async(format!("ws://{addr}/ws")).await.expect("connect");
        let _ = socket.next().await;
        let _ = socket.next().await;

        runtime
            .push_event(BridgeEvent::ConnectionStatus {
                message: "connected".to_string(),
            })
            .await;

        let event = socket.next().await.expect("event").expect("event frame");
        let event_text = match event {
            tokio_tungstenite::tungstenite::Message::Text(text) => text,
            other => panic!("unexpected event frame: {other:?}"),
        };
        assert!(event_text.contains("\"connectionStatus\""));
        assert!(event_text.contains("\"connected\""));
    }

    #[tokio::test]
    async fn legacy_qa_harness_routes_are_deprecated() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp)).await.expect("runtime");
        let addr = spawn_server(runtime).await;
        let client = reqwest::Client::new();

        let devices = client
            .get(format!("http://{addr}/threads/test/qa/devices"))
            .send()
            .await
            .expect("devices response");
        assert_eq!(devices.status(), reqwest::StatusCode::GONE);

        let reserve = client
            .post(format!("http://{addr}/threads/test/qa/devices/example/reserve"))
            .json(&serde_json::json!({}))
            .send()
            .await
            .expect("reserve response");
        assert_eq!(reserve.status(), reqwest::StatusCode::GONE);
    }
}
