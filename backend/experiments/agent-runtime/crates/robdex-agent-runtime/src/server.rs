use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
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
use tokio::sync::Mutex;
use tokio::time::{Duration, interval};
use uuid::Uuid;

use crate::roles::DEFAULT_ROLE_ID;
use crate::{db, projection, runtime, starlark_host};

#[derive(Clone)]
pub struct ServerState {
    pub pool: PgPool,
    pub runtime_identity: String,
    pub active_sends: Arc<Mutex<HashSet<Uuid>>>,
}

impl ServerState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            runtime_identity: format!("robdex-agent-runtime/{}", env!("CARGO_PKG_VERSION")),
            active_sends: Arc::new(Mutex::new(HashSet::new())),
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
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, message: message.into() }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self { status: StatusCode::CONFLICT, message: message.into() }
    }

    fn internal(error: anyhow::Error) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: error.to_string() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({"error": self.message}))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self::internal(error)
    }
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
        .with_state(state)
}

pub async fn serve(pool: PgPool, host: &str, port: u16) -> Result<()> {
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app(ServerState::new(pool))).await?;
    Ok(())
}

async fn health(State(state): State<ServerState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("SELECT 1").execute(&state.pool).await.map_err(|error| {
        ApiError { status: StatusCode::SERVICE_UNAVAILABLE, message: error.to_string() }
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
    Ok(Json(db::show_session(&state.pool, session_id).await?))
}

async fn session_history(
    State(state): State<ServerState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(db::history_json(&state.pool, session_id).await?))
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
    Json(request): Json<CreateSessionRequest>,
) -> Result<Json<Value>, ApiError> {
    let role_id = request.role.as_deref().unwrap_or(DEFAULT_ROLE_ID);
    let workdir = request.workdir.as_deref().unwrap_or(".");
    let role = db::current_role_snapshot(&state.pool, role_id).await?;
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
    Json(request): Json<SendRequest>,
) -> Result<Json<Value>, ApiError> {
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
    Json(request): Json<CloseRequest>,
) -> Result<Json<Value>, ApiError> {
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
    Json(request): Json<ForkRequest>,
) -> Result<Json<Value>, ApiError> {
    let forked = db::fork_session(&state.pool, session_id, request.at_turn).await?;
    Ok(Json(json!({"sessionId": forked, "forkedFromSessionId": session_id, "forkedFromTurnId": request.at_turn})))
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
    tokio::spawn(async move {
        while receiver.next().await.is_some() {}
    });
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
        tick.tick().await;
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
        let value = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).expect("json") };
        (status, value)
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
        assert!(conflict["error"].as_str().unwrap_or_default().contains("active send"));
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
