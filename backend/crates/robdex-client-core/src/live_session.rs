use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    bridge::BridgeEndpoint,
    workbench::build_workbench,
};
use robdex_protocol::WorkbenchViewData;

#[derive(Debug)]
pub enum LiveSessionEvent {
    View(WorkbenchViewData),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct LiveSessionHandle {
    command_tx: mpsc::UnboundedSender<LiveSessionCommand>,
}

impl LiveSessionHandle {
    pub fn select_thread(&self, thread_id: Option<String>) {
        let _ = self.command_tx.send(LiveSessionCommand::SelectThread(thread_id));
    }

    pub fn sync_view(&self, view: WorkbenchViewData) {
        let _ = self.command_tx.send(LiveSessionCommand::SyncView(view));
    }
}

#[derive(Debug)]
enum LiveSessionCommand {
    SelectThread(Option<String>),
    SyncView(WorkbenchViewData),
}

pub fn start_live_session(
    initial_view: WorkbenchViewData,
    endpoint: BridgeEndpoint,
) -> (LiveSessionHandle, mpsc::UnboundedReceiver<LiveSessionEvent>) {
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    tokio::spawn(run_live_session(initial_view, endpoint, command_rx, event_tx));
    (LiveSessionHandle { command_tx }, event_rx)
}

async fn run_live_session(
    mut current_view: WorkbenchViewData,
    endpoint: BridgeEndpoint,
    mut command_rx: mpsc::UnboundedReceiver<LiveSessionCommand>,
    event_tx: mpsc::UnboundedSender<LiveSessionEvent>,
) {
    let mut selected_thread_id = current_view.selection.thread_id.clone();

    loop {
        let ws_url = match endpoint.workbench_ws_url() {
            Ok(url) => url,
            Err(error) => {
                let _ = event_tx.send(LiveSessionEvent::Error(error.to_string()));
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        match connect_async(ws_url.as_str()).await {
            Ok((socket, _)) => {
                let (mut write, mut read) = socket.split();

                if send_json(&mut write, &json!({"type":"hello","payload":{}})).await.is_err() {
                    let _ = event_tx.send(LiveSessionEvent::Error("Failed to send hello".to_string()));
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
                if let Some(thread_id) = selected_thread_id.clone() {
                    if send_thread_selection(&mut write, &thread_id).await.is_err() {
                        let _ = event_tx.send(LiveSessionEvent::Error("Failed to send thread selection".to_string()));
                    }
                }

                loop {
                    tokio::select! {
                        maybe_command = command_rx.recv() => {
                            let Some(command) = maybe_command else {
                                return;
                            };
                            match command {
                                LiveSessionCommand::SelectThread(thread_id) => {
                                    selected_thread_id = thread_id.clone();
                                    if let Some(thread_id) = thread_id {
                                        if send_thread_selection(&mut write, &thread_id).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                                LiveSessionCommand::SyncView(view) => {
                                    selected_thread_id = view.selection.thread_id.clone();
                                    current_view = view;
                                    if let Some(thread_id) = selected_thread_id.clone() {
                                        if send_thread_selection(&mut write, &thread_id).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        maybe_message = read.next() => {
                            let Some(message_result) = maybe_message else {
                                break;
                            };
                            let Ok(message) = message_result else {
                                break;
                            };
                            match message {
                                Message::Text(text) => {
                                    match reduce_message(
                                        &text,
                                        &current_view,
                                        selected_thread_id.as_deref(),
                                        &endpoint,
                                    ).await {
                                        Ok(Some(next_view)) => {
                                            current_view = next_view.clone();
                                            let _ = event_tx.send(LiveSessionEvent::View(next_view));
                                        }
                                        Ok(None) => {}
                                        Err(error) => {
                                            let _ = event_tx.send(LiveSessionEvent::Error(error.to_string()));
                                        }
                                    }
                                }
                                Message::Ping(payload) => {
                                    if write.send(Message::Pong(payload)).await.is_err() {
                                        break;
                                    }
                                }
                                Message::Close(_) => break,
                                Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {}
                            }
                        }
                    }
                }
            }
            Err(error) => {
                let _ = event_tx.send(LiveSessionEvent::Error(error.to_string()));
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}

async fn reduce_message(
    text: &str,
    _current_view: &WorkbenchViewData,
    selected_thread_id: Option<&str>,
    endpoint: &BridgeEndpoint,
) -> Result<Option<WorkbenchViewData>> {
    let envelope: Value = serde_json::from_str(text)?;
    if envelope.get("type").and_then(Value::as_str) != Some("event") {
        return Ok(None);
    }
    let Some(event) = envelope
        .get("payload")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("event"))
        .and_then(Value::as_object)
    else {
        return Ok(None);
    };

    match event.get("name").and_then(Value::as_str) {
        Some("appStateSnapshot") => {
            let next_view = refresh_selected_view(selected_thread_id, endpoint).await?;
            Ok(Some(next_view))
        }
        Some("threadMessagesChanged") => {
            let Some(payload) = event.get("data").cloned() else {
                return Ok(None);
            };
            let thread_id = payload
                .get("threadID")
                .and_then(Value::as_str)
                .or_else(|| payload.get("threadId").and_then(Value::as_str));
            if thread_id != selected_thread_id {
                return Ok(None);
            }
            if thread_id.is_none() {
                return Ok(None);
            }
            let next_view = refresh_selected_view(selected_thread_id, endpoint).await?;
            Ok(Some(next_view))
        }
        _ => Ok(None),
    }
}

async fn refresh_selected_view(
    selected_thread_id: Option<&str>,
    endpoint: &BridgeEndpoint,
) -> Result<WorkbenchViewData> {
    let snapshot = reqwest::Client::new()
        .get(endpoint.workbench_bootstrap_url()?)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    build_workbench(snapshot, selected_thread_id, None, endpoint).await
}

async fn send_json(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    value: &Value,
) -> Result<()> {
    write.send(Message::Text(value.to_string().into())).await?;
    Ok(())
}

async fn send_thread_selection(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    thread_id: &str,
) -> Result<()> {
    send_json(
        write,
        &json!({
            "type": "command",
            "payload": {
                "id": format!("thread-select-{thread_id}"),
                "command": {
                    "name": "threadSelectionSet",
                    "payload": {
                        "threadId": thread_id,
                    }
                }
            }
        }),
    )
    .await
}
