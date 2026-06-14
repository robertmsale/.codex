use std::fmt;

use futures_util::StreamExt;
use robdex_agent_runtime_projection::{ApplyOutcome, GuiConnectionState, GuiControllerState, RuntimeDelta, RuntimeDeltaKind, RuntimeProjection, Watermark};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite, MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSyncConfig {
    pub base_url: String,
    pub selected_session_id: Option<Uuid>,
}

impl RuntimeSyncConfig {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            selected_session_id: None,
        }
    }

    pub fn with_selected_session(mut self, selected_session_id: Uuid) -> Self {
        self.selected_session_id = Some(selected_session_id);
        self
    }

    pub fn snapshot_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        match self.selected_session_id {
            Some(session_id) => format!("{base}/state/snapshot?selectedSessionId={session_id}"),
            None => format!("{base}/state/snapshot"),
        }
    }

    pub fn websocket_url(&self, after: Option<Watermark>) -> Result<String, SyncError> {
        let base = self.base_url.trim_end_matches('/');
        let ws_base = if let Some(rest) = base.strip_prefix("http://") {
            format!("ws://{rest}")
        } else if let Some(rest) = base.strip_prefix("https://") {
            format!("wss://{rest}")
        } else if base.starts_with("ws://") || base.starts_with("wss://") {
            base.to_string()
        } else {
            return Err(SyncError::Protocol(format!(
                "unsupported runtime server URL scheme: {base}"
            )));
        };
        let mut params = Vec::new();
        if let Some(after) = after {
            params.push(format!("after={after}"));
        }
        if let Some(session_id) = self.selected_session_id {
            params.push(format!("selectedSessionId={session_id}"));
        }
        if params.is_empty() {
            Ok(format!("{ws_base}/state/ws"))
        } else {
            Ok(format!("{ws_base}/state/ws?{}", params.join("&")))
        }
    }
}

#[derive(Debug)]
pub struct RuntimeSyncClient {
    config: RuntimeSyncConfig,
    http: reqwest::Client,
    projection: Option<RuntimeProjection>,
    server_runtime_identity: Option<String>,
    server_watermark: Option<Watermark>,
    resync_required: bool,
    server_shutdown_seen: bool,
    controller_state: GuiControllerState,
}

impl RuntimeSyncClient {
    pub fn new(config: RuntimeSyncConfig) -> Self {
        let selected_session_id = config.selected_session_id.map(|id| id.to_string());
        Self {
            config,
            http: reqwest::Client::new(),
            projection: None,
            server_runtime_identity: None,
            server_watermark: None,
            resync_required: false,
            server_shutdown_seen: false,
            controller_state: GuiControllerState {
                selected_session_id,
                ..GuiControllerState::default()
            },
        }
    }

    pub fn config(&self) -> &RuntimeSyncConfig {
        &self.config
    }

    pub fn projection(&self) -> Option<&RuntimeProjection> {
        self.projection.as_ref()
    }

    pub fn projection_mut(&mut self) -> Option<&mut RuntimeProjection> {
        self.projection.as_mut()
    }

    pub fn server_runtime_identity(&self) -> Option<&str> {
        self.server_runtime_identity.as_deref()
    }

    pub fn server_watermark(&self) -> Option<Watermark> {
        self.server_watermark
    }

    pub fn resync_required(&self) -> bool {
        self.resync_required
    }

    pub fn server_shutdown_seen(&self) -> bool {
        self.server_shutdown_seen
    }

    pub fn controller_state(&self) -> &GuiControllerState {
        &self.controller_state
    }

    pub async fn hydrate(&mut self) -> Result<&RuntimeProjection, SyncError> {
        self.controller_state.connection_state = GuiConnectionState::Hydrating;
        let snapshot = self
            .http
            .get(self.config.snapshot_url())
            .send()
            .await?
            .error_for_status()?
            .json::<RuntimeProjection>()
            .await?;
        self.server_watermark = Some(snapshot.watermark);
        self.resync_required = false;
        self.controller_state.pending_rehydrate = false;
        self.controller_state.resync_required = None;
        self.controller_state.connection_state = GuiConnectionState::Streaming;
        self.projection = Some(snapshot);
        Ok(self.projection.as_ref().expect("projection was just hydrated"))
    }

    pub async fn rehydrate(&mut self) -> Result<&RuntimeProjection, SyncError> {
        self.hydrate().await
    }

    pub async fn connect_after(&self, after: Option<Watermark>) -> Result<RuntimeStateStream, SyncError> {
        let url = self.config.websocket_url(after)?;
        let (socket, _) = connect_async(url).await?;
        Ok(RuntimeStateStream { socket })
    }

    pub fn handle_server_message_value(&mut self, value: Value) -> Result<SyncOutcome, SyncError> {
        let message_type = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| SyncError::Protocol(format!("server message missing type: {value}")))?;
        match message_type {
            "hello" => {
                let watermark = value
                    .get("watermark")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| SyncError::Protocol(format!("hello missing watermark: {value}")))?;
                let runtime_identity = value
                    .get("runtimeIdentity")
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                if let Some(existing) = self.server_runtime_identity.as_deref() {
                    if let Some(incoming) = runtime_identity.as_deref() {
                        if existing != incoming {
                            return Err(SyncError::Protocol(format!(
                                "runtime identity changed from {existing} to {incoming}"
                            )));
                        }
                    }
                }
                self.server_watermark = Some(watermark);
                if let Some(runtime_identity) = runtime_identity.clone() {
                    self.server_runtime_identity = Some(runtime_identity);
                }
                Ok(SyncOutcome::Hello {
                    watermark,
                    runtime_identity,
                })
            }
            "delta" => {
                let delta_value = value
                    .get("delta")
                    .ok_or_else(|| SyncError::Protocol(format!("delta message missing delta: {value}")))?;
                let delta: RuntimeDelta = serde_json::from_value(delta_value.clone())?;
                let projection = self
                    .projection
                    .as_mut()
                    .ok_or_else(|| SyncError::Protocol("delta received before snapshot hydration".to_string()))?;
                let apply_outcome = projection.apply_delta(delta.clone());
                if matches!(apply_outcome, ApplyOutcome::ResyncRequired) {
                    self.resync_required = true;
                }
                Ok(SyncOutcome::DeltaApplied {
                    delta,
                    apply_outcome,
                })
            }
            "resyncRequired" => {
                let mut reason = None;
                let mut expected = None;
                let mut received = None;
                if let Some(delta_value) = value.get("delta") {
                    let delta: RuntimeDelta = serde_json::from_value(delta_value.clone())?;
                    if let RuntimeDeltaKind::ResyncRequired { reason: delta_reason } = &delta.kind {
                        reason = Some(delta_reason.clone());
                    }
                    if let Some(projection) = self.projection.as_mut() {
                        projection.apply_delta(delta);
                        if let Some(state) = projection.resync_required.as_ref() {
                            expected = state.expected_watermark;
                            received = state.received_watermark;
                        }
                    }
                }
                self.resync_required = true;
                self.controller_state.record_resync_required(
                    reason.clone().unwrap_or_else(|| "server requested resync".to_string()),
                    expected,
                    received,
                );
                Ok(SyncOutcome::ResyncRequired { reason })
            }
            "serverShutdown" => {
                self.server_shutdown_seen = true;
                self.controller_state.connection_state = GuiConnectionState::ShuttingDown;
                Ok(SyncOutcome::ServerShutdown)
            }
            other => Err(SyncError::Protocol(format!(
                "unknown runtime server message type: {other}"
            ))),
        }
    }
}

pub struct RuntimeStateStream {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl RuntimeStateStream {
    pub async fn next_outcome(&mut self, client: &mut RuntimeSyncClient) -> Result<SyncOutcome, SyncError> {
        loop {
            let Some(message) = self.socket.next().await else {
                return Ok(SyncOutcome::StreamClosed);
            };
            let message = message?;
            if message.is_text() {
                let value: Value = serde_json::from_str(message.to_text()?)?;
                return client.handle_server_message_value(value);
            }
            if message.is_close() {
                return Ok(SyncOutcome::StreamClosed);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncOutcome {
    Hello {
        watermark: Watermark,
        runtime_identity: Option<String>,
    },
    DeltaApplied {
        delta: RuntimeDelta,
        apply_outcome: ApplyOutcome,
    },
    ResyncRequired {
        reason: Option<String>,
    },
    ServerShutdown,
    StreamClosed,
}

#[derive(Debug)]
pub enum SyncError {
    Http(reqwest::Error),
    WebSocket(tungstenite::Error),
    Json(serde_json::Error),
    Protocol(String),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncError::Http(error) => write!(f, "http error: {error}"),
            SyncError::WebSocket(error) => write!(f, "websocket error: {error}"),
            SyncError::Json(error) => write!(f, "json error: {error}"),
            SyncError::Protocol(message) => write!(f, "runtime sync protocol error: {message}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<reqwest::Error> for SyncError {
    fn from(error: reqwest::Error) -> Self {
        SyncError::Http(error)
    }
}

impl From<tungstenite::Error> for SyncError {
    fn from(error: tungstenite::Error) -> Self {
        SyncError::WebSocket(error)
    }
}

impl From<serde_json::Error> for SyncError {
    fn from(error: serde_json::Error) -> Self {
        SyncError::Json(error)
    }
}
