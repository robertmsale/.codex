use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::broadcast;
use tracing::error;

use crate::{
    commands::{
        execute_bridge_command, make_app_state_snapshot, orchestrator_agents, orchestrator_approval_decision,
        orchestrator_archive_agent, orchestrator_lookup, orchestrator_pending_approvals, orchestrator_rename_agent,
        orchestrator_send_message, orchestrator_spawn_agent, orchestrator_thread_group_archive,
        orchestrator_thread_group_create, orchestrator_thread_group_delete, orchestrator_thread_group_move_thread,
        orchestrator_thread_group_update, orchestrator_thread_groups, orchestrator_threads,
        orchestrator_update_worker_metadata, orchestrator_whoami,
    },
    models::{
        BridgeEvent, PROTOCOL_VERSION, SequencedEvent, SERVER_NAME, SERVER_VERSION, ThreadMessagesResponse,
    },
    runtime::BridgeRuntime,
};

pub fn build_router(runtime: Arc<BridgeRuntime>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/info", get(info))
        .route("/state/snapshot", get(snapshot))
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
        .route("/orchestrator/agent-message", post(orchestrator_agent_message_route))
        .route("/orchestrator/archive-agent", post(orchestrator_archive_agent_route))
        .route("/orchestrator/rename-agent", post(orchestrator_rename_agent_route))
        .route("/orchestrator/worker-metadata", post(orchestrator_worker_metadata_route))
        .route("/orchestrator/approval-decision", post(orchestrator_approval_decision_route))
        .route("/ws", get(ws_upgrade))
        .with_state(runtime)
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

#[derive(Debug, Deserialize)]
struct ThreadMessagesQuery {
    thread_id: String,
}

async fn thread_messages(
    State(runtime): State<Arc<BridgeRuntime>>,
    Query(query): Query<ThreadMessagesQuery>,
) -> impl IntoResponse {
    match runtime.thread_messages(&query.thread_id).await {
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
) -> Json<crate::models::EventReplayResponse> {
    Json(runtime.replay_events(query.since).await)
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
                        Err(_) => break,
                    };
                    if send_envelope(&mut sender, envelope).await.is_err() {
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
                InboundEnvelope::Hello(_) => {
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
            if let Ok(Some(messages)) = runtime.thread_messages(&thread_id).await {
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
    }
    let outcome = execute_bridge_command(runtime, &command.command.name, command.command.payload.clone())
        .await
        .map_err(|_| ())?;

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

fn should_send_event(event: &SequencedEvent, selected_thread_id: Option<&str>) -> bool {
    match &event.event {
        BridgeEvent::ThreadMessagesChanged { payload } => match selected_thread_id {
            Some(selected) => selected.trim() == payload.thread_id,
            None => true,
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
        BridgeEvent::ThreadMessagesChanged { payload } => {
            OutboundEvent::ThreadMessagesChanged { data: payload }
        }
    };
    Ok(OutboundEnvelope::Event(OutboundSequencedEvent {
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
}
