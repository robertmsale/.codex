use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use codex_app_server_adapter::{
    app_server_protocol::{
        JSONRPCMessage, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse, RequestId, ServerRequest,
    },
    wire::{
        encode_jsonrpc_message, initialize_request, parse_jsonrpc_message, parse_server_notification,
        parse_server_request,
    },
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, protocol::Role},
};

use crate::upstream::UpstreamRuntimeEvent;

pub const DEFAULT_RECONNECT_DELAY_MS: u64 = 5_000;

#[derive(Debug)]
pub enum TransportControlMessage {
    SendServerResponse {
        request_id: RequestId,
        result: serde_json::Value,
        ack: oneshot::Sender<Result<()>>,
    },
    SendJsonRequest {
        request_id: RequestId,
        method: String,
        params: serde_json::Value,
        ack: oneshot::Sender<Result<serde_json::Value>>,
    },
}

pub struct AppServerConnection {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl AppServerConnection {
    pub async fn connect(url: &str) -> Result<Self> {
        let (stream, _) = connect_async(url)
            .await
            .with_context(|| format!("failed to connect to app-server websocket at {url}"))?;
        Ok(Self { stream })
    }

    pub async fn initialize(
        &mut self,
        client_name: &str,
        client_version: &str,
        experimental_api: bool,
    ) -> Result<JSONRPCResponse> {
        let request = initialize_request(
            RequestId::Integer(1),
            client_name.to_string(),
            client_version.to_string(),
            experimental_api,
        )?;
        self.send_request(request).await?;
        self.read_response().await
    }

    pub async fn send_request(&mut self, request: JSONRPCRequest) -> Result<()> {
        let wire = encode_jsonrpc_message(&JSONRPCMessage::Request(request))?;
        self.stream
            .send(Message::Text(wire))
            .await
            .context("failed to send JSON-RPC request")
    }

    pub async fn send_response(
        &mut self,
        request_id: RequestId,
        result: serde_json::Value,
    ) -> Result<()> {
        let wire = encode_jsonrpc_message(&JSONRPCMessage::Response(JSONRPCResponse {
            id: request_id,
            result,
        }))?;
        self.stream
            .send(Message::Text(wire))
            .await
            .context("failed to send JSON-RPC response")
    }

    pub async fn read_message(&mut self) -> Result<JSONRPCMessage> {
        while let Some(message) = self.stream.next().await {
            let message = message.context("websocket receive error")?;
            match message {
                Message::Text(text) => return parse_jsonrpc_message(&text),
                Message::Binary(bytes) => {
                    let text = String::from_utf8(bytes.to_vec()).context("binary websocket frame was not utf-8")?;
                    return parse_jsonrpc_message(&text);
                }
                Message::Ping(payload) => {
                    self.stream
                        .send(Message::Pong(payload))
                        .await
                        .context("failed to reply to websocket ping")?;
                }
                Message::Pong(_) => {}
                Message::Close(frame) => {
                    bail!("app-server websocket closed: {frame:?}");
                }
                Message::Frame(_) => {}
            }
        }
        bail!("app-server websocket ended unexpectedly")
    }

    pub async fn read_response(&mut self) -> Result<JSONRPCResponse> {
        loop {
            match self.read_message().await? {
                JSONRPCMessage::Response(response) => return Ok(response),
                JSONRPCMessage::Error(error) => bail!("app-server returned JSON-RPC error: {}", error.error.message),
                JSONRPCMessage::Notification(_) => continue,
                JSONRPCMessage::Request(_) => continue,
            }
        }
    }

    pub async fn request_json(
        &mut self,
        request_id: RequestId,
        method: impl Into<String>,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.send_request(JSONRPCRequest {
            id: request_id.clone(),
            method: method.into(),
            params: Some(params),
            trace: None,
        })
        .await?;

        loop {
            match self.read_message().await? {
                JSONRPCMessage::Response(response) if response.id == request_id => return Ok(response.result),
                JSONRPCMessage::Error(error) if error.id == request_id => {
                    bail!("app-server returned JSON-RPC error: {}", error.error.message)
                }
                JSONRPCMessage::Notification(_) => continue,
                JSONRPCMessage::Response(_) => continue,
                JSONRPCMessage::Request(_) => continue,
                JSONRPCMessage::Error(error) => {
                    bail!("unexpected JSON-RPC error message: {}", error.error.message)
                }
            }
        }
    }
}

pub async fn request_json(
    url: &str,
    method: impl Into<String>,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let mut connection = AppServerConnection::connect(url).await?;
    connection
        .initialize("robdex-bridge", env!("CARGO_PKG_VERSION"), true)
        .await?;
    connection
        .request_json(RequestId::Integer(2), method.into(), params)
        .await
}

pub async fn run_transport_loop(
    url: String,
    tx: mpsc::Sender<UpstreamRuntimeEvent>,
    mut control_rx: mpsc::Receiver<TransportControlMessage>,
    reconnect_delay: Duration,
) {
    let tx = Arc::new(tx);
    loop {
        if let Err(error) = tx
            .send(UpstreamRuntimeEvent::ConnectionStatus(format!("connecting: {url}")))
            .await
        {
            tracing::warn!("transport status send failed before connect: {error}");
            return;
        }

        match AppServerConnection::connect(&url).await {
            Ok(mut connection) => {
                if let Err(error) = tx
                    .send(UpstreamRuntimeEvent::ConnectionStatus("connected".to_string()))
                    .await
                {
                    tracing::warn!("transport status send failed after connect: {error}");
                    return;
                }

                match connection.initialize("robdex-bridge", env!("CARGO_PKG_VERSION"), true).await {
                    Ok(_) => {
                        if let Err(error) =
                            run_connected_loop(&mut connection, tx.clone(), &mut control_rx).await
                        {
                            if tx
                                .send(UpstreamRuntimeEvent::ConnectionStatus(format!("disconnected: {error}")))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                    Err(error) => {
                        if tx
                            .send(UpstreamRuntimeEvent::ConnectionStatus(format!(
                                "initialize failed: {error}"
                            )))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            }
            Err(error) => {
                if tx
                    .send(UpstreamRuntimeEvent::ConnectionStatus(format!("connect failed: {error}")))
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }

        tokio::time::sleep(reconnect_delay).await;
    }
}

async fn forward_notifications(
    connection: &mut AppServerConnection,
    tx: Arc<mpsc::Sender<UpstreamRuntimeEvent>>,
) -> Result<()> {
    loop {
        match connection.read_message().await? {
            JSONRPCMessage::Notification(notification) => forward_notification(notification, tx.clone()).await?,
            JSONRPCMessage::Response(_) => {}
            JSONRPCMessage::Request(_) => {}
            JSONRPCMessage::Error(error) => bail!("unexpected JSON-RPC error message: {}", error.error.message),
        }
    }
}

async fn run_connected_loop(
    connection: &mut AppServerConnection,
    tx: Arc<mpsc::Sender<UpstreamRuntimeEvent>>,
    control_rx: &mut mpsc::Receiver<TransportControlMessage>,
) -> Result<()> {
    loop {
        tokio::select! {
            incoming = connection.read_message() => {
                match incoming? {
                    JSONRPCMessage::Notification(notification) => forward_notification(notification, tx.clone()).await?,
                    JSONRPCMessage::Request(request) => forward_server_request(request, tx.clone()).await?,
                    JSONRPCMessage::Response(_) => {}
                    JSONRPCMessage::Error(error) => bail!("unexpected JSON-RPC error message: {}", error.error.message),
                }
            }
            outbound = control_rx.recv() => {
                let Some(control) = outbound else {
                    bail!("transport control channel closed");
                };
                handle_transport_control(connection, tx.clone(), control).await?;
            }
        }
    }
}

async fn forward_notification(
    notification: JSONRPCNotification,
    tx: Arc<mpsc::Sender<UpstreamRuntimeEvent>>,
) -> Result<()> {
    let parsed = parse_server_notification(notification)?;
    tx.send(UpstreamRuntimeEvent::Notification(parsed))
        .await
        .context("failed to forward upstream notification")
}

async fn forward_server_request(
    request: JSONRPCRequest,
    tx: Arc<mpsc::Sender<UpstreamRuntimeEvent>>,
) -> Result<()> {
    let parsed: ServerRequest = parse_server_request(request)?;
    tx.send(UpstreamRuntimeEvent::ServerRequest(parsed))
        .await
        .context("failed to forward upstream server request")
}

async fn handle_transport_control(
    connection: &mut AppServerConnection,
    tx: Arc<mpsc::Sender<UpstreamRuntimeEvent>>,
    control: TransportControlMessage,
) -> Result<()> {
    match control {
        TransportControlMessage::SendServerResponse {
            request_id,
            result,
            ack,
        } => {
            match connection.send_response(request_id, result).await {
                Ok(()) => {
                    let _ = ack.send(Ok(()));
                    Ok(())
                }
                Err(error) => {
                    let message = error.to_string();
                    let _ = ack.send(Err(anyhow::anyhow!(message.clone())));
                    Err(anyhow::anyhow!(message))
                }
            }
        }
        TransportControlMessage::SendJsonRequest {
            request_id,
            method,
            params,
            ack,
        } => match request_json_over_connection(connection, tx, request_id, method, params).await {
            Ok(result) => {
                let _ = ack.send(Ok(result));
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                let _ = ack.send(Err(anyhow::anyhow!(message.clone())));
                Err(anyhow::anyhow!(message))
            }
        },
    }
}

async fn request_json_over_connection(
    connection: &mut AppServerConnection,
    tx: Arc<mpsc::Sender<UpstreamRuntimeEvent>>,
    request_id: RequestId,
    method: String,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    connection
        .send_request(JSONRPCRequest {
            id: request_id.clone(),
            method,
            params: Some(params),
            trace: None,
        })
        .await?;

    loop {
        match connection.read_message().await? {
            JSONRPCMessage::Response(response) if response.id == request_id => return Ok(response.result),
            JSONRPCMessage::Error(error) if error.id == request_id => {
                bail!("app-server returned JSON-RPC error: {}", error.error.message)
            }
            JSONRPCMessage::Notification(notification) => forward_notification(notification, tx.clone()).await?,
            JSONRPCMessage::Request(request) => forward_server_request(request, tx.clone()).await?,
            JSONRPCMessage::Response(_) => {}
            JSONRPCMessage::Error(error) => {
                bail!("unexpected JSON-RPC error message: {}", error.error.message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_adapter::app_server_protocol::{
        JSONRPCError, JSONRPCErrorError, JSONRPCMessage, JSONRPCNotification, JSONRPCResponse,
    };
    use futures_util::{SinkExt, StreamExt};
    use std::net::SocketAddr;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    async fn spawn_test_server(
        handler: impl Fn(WebSocketStream<TcpStream>) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync
        + 'static,
    ) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let handler = Arc::new(handler);
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let ws = accept_async(stream).await.expect("ws accept");
            handler(ws).await;
        });
        Ok((addr, task))
    }

    #[tokio::test]
    async fn initialize_sends_real_jsonrpc_request() {
        let (addr, task) = spawn_test_server(|mut ws| {
            Box::pin(async move {
                let message = ws.next().await.expect("message").expect("ws");
                let text = match message {
                    Message::Text(text) => text,
                    other => panic!("unexpected message: {other:?}"),
                };
                let parsed = parse_jsonrpc_message(&text).expect("jsonrpc");
                match parsed {
                    JSONRPCMessage::Request(request) => {
                        assert_eq!(request.method, "initialize");
                        ws.send(Message::Text(
                            serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                                id: RequestId::Integer(1),
                                result: serde_json::json!({
                                    "userAgent": "codex",
                                    "platformFamily": "unix",
                                    "platformOs": "macos"
                                }),
                            }))
                            .expect("response"),
                        ))
                        .await
                        .expect("send response");
                    }
                    other => panic!("unexpected message: {other:?}"),
                }
            })
        })
        .await
        .expect("server");

        let mut connection =
            AppServerConnection::connect(&format!("ws://{addr}")).await.expect("connection");
        let response = connection
            .initialize("robdex-bridge", "0.1.0", true)
            .await
            .expect("initialize");
        assert_eq!(response.id, RequestId::Integer(1));
        task.await.expect("task");
    }

    #[tokio::test]
    async fn transport_loop_forwards_connection_and_notification_events() {
        let (addr, task) = spawn_test_server(|mut ws| {
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("ws");
                let text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init: {other:?}"),
                };
                let parsed = parse_jsonrpc_message(&text).expect("parse");
                match parsed {
                    JSONRPCMessage::Request(_) => {
                        ws.send(Message::Text(
                            serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                                id: RequestId::Integer(1),
                                result: serde_json::json!({
                                    "userAgent": "codex",
                                    "platformFamily": "unix",
                                    "platformOs": "macos"
                                }),
                            }))
                            .expect("response"),
                        ))
                        .await
                        .expect("send response");
                    }
                    other => panic!("unexpected parsed init: {other:?}"),
                }

                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Notification(JSONRPCNotification {
                        method: "thread/closed".to_string(),
                        params: Some(serde_json::json!({ "threadId": "thread-1" })),
                    }))
                    .expect("notification"),
                ))
                .await
                .expect("send notification");
            })
        })
        .await
        .expect("server");

        let (tx, mut rx) = mpsc::channel(16);
        let (_control_tx, control_rx) = mpsc::channel(16);
        let transport = tokio::spawn(run_transport_loop(
            format!("ws://{addr}"),
            tx,
            control_rx,
            Duration::from_millis(10),
        ));

        let first = rx.recv().await.expect("first");
        let second = rx.recv().await.expect("second");
        let third = rx.recv().await.expect("third");

        assert!(matches!(first, UpstreamRuntimeEvent::ConnectionStatus(ref value) if value.starts_with("connecting:")));
        assert!(matches!(second, UpstreamRuntimeEvent::ConnectionStatus(ref value) if value == "connected"));
        assert!(matches!(third, UpstreamRuntimeEvent::Notification(_)));

        transport.abort();
        task.await.expect("task");
    }
}
