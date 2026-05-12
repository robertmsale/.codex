use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

#[cfg(target_arch = "wasm32")]
use gloo_net::websocket::{Message, futures::WebSocket};
#[cfg(not(target_arch = "wasm32"))]
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    bridge::BridgeEndpoint,
    workbench::{
        build_workbench_with_models, chat_entries_from_thread_payload,
        context_window_remaining_percent_from_thread_payload,
        parse_live_process_items,
    },
};
use robdex_protocol::{HookFailureNotice, WorkbenchViewData};

#[derive(Debug)]
pub enum LiveSessionEvent {
    View(WorkbenchViewData),
    HookFailure(HookFailureNotice),
    Error(String),
}

enum ReduceOutcome {
    View(WorkbenchViewData),
    HookFailure(HookFailureNotice),
    None,
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

#[cfg(not(target_arch = "wasm32"))]
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
                                    if selected_thread_id == thread_id {
                                        current_view.selection.thread_id = thread_id;
                                        continue;
                                    }
                                    selected_thread_id = thread_id.clone();
                                    current_view.selection.thread_id = thread_id.clone();
                                    if let Some(thread_id) = thread_id {
                                        if send_thread_selection(&mut write, &thread_id).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                                LiveSessionCommand::SyncView(view) => {
                                    let next_thread_id = view.selection.thread_id.clone();
                                    let selection_changed = selected_thread_id != next_thread_id;
                                    selected_thread_id = view.selection.thread_id.clone();
                                    current_view = view;
                                    if selection_changed && let Some(thread_id) = selected_thread_id.clone() {
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
                                        Ok(ReduceOutcome::View(next_view)) => {
                                            current_view = next_view.clone();
                                            let _ = event_tx.send(LiveSessionEvent::View(next_view));
                                        }
                                        Ok(ReduceOutcome::HookFailure(notice)) => {
                                            let _ = event_tx.send(LiveSessionEvent::HookFailure(notice));
                                        }
                                        Ok(ReduceOutcome::None) => {}
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

#[cfg(target_arch = "wasm32")]
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

        match WebSocket::open(ws_url.as_str()) {
            Ok(socket) => {
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
                                    if selected_thread_id == thread_id {
                                        current_view.selection.thread_id = thread_id;
                                        continue;
                                    }
                                    selected_thread_id = thread_id.clone();
                                    current_view.selection.thread_id = thread_id.clone();
                                    if let Some(thread_id) = thread_id {
                                        if send_thread_selection(&mut write, &thread_id).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                                LiveSessionCommand::SyncView(view) => {
                                    let next_thread_id = view.selection.thread_id.clone();
                                    let selection_changed = selected_thread_id != next_thread_id;
                                    selected_thread_id = view.selection.thread_id.clone();
                                    current_view = view;
                                    if selection_changed && let Some(thread_id) = selected_thread_id.clone() {
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
                                        Ok(ReduceOutcome::View(next_view)) => {
                                            current_view = next_view.clone();
                                            let _ = event_tx.send(LiveSessionEvent::View(next_view));
                                        }
                                        Ok(ReduceOutcome::HookFailure(notice)) => {
                                            let _ = event_tx.send(LiveSessionEvent::HookFailure(notice));
                                        }
                                        Ok(ReduceOutcome::None) => {}
                                        Err(error) => {
                                            let _ = event_tx.send(LiveSessionEvent::Error(error.to_string()));
                                        }
                                    }
                                }
                                Message::Bytes(_) => {}
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
    current_view: &WorkbenchViewData,
    selected_thread_id: Option<&str>,
    endpoint: &BridgeEndpoint,
) -> Result<ReduceOutcome> {
    let envelope: Value = serde_json::from_str(text)?;
    if envelope.get("type").and_then(Value::as_str) != Some("event") {
        return Ok(ReduceOutcome::None);
    }
    let Some(event) = envelope
        .get("payload")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("event"))
        .and_then(Value::as_object)
    else {
        return Ok(ReduceOutcome::None);
    };

    match event.get("name").and_then(Value::as_str) {
        Some("appStateSnapshot") => {
            let Some(snapshot) = event.get("data").cloned() else {
                return Ok(ReduceOutcome::None);
            };
            let should_preserve_messages = current_view
                .selection
                .thread_id
                .as_ref()
                .map(|thread_id| Some(thread_id.as_str()) == selected_thread_id)
                .unwrap_or(false)
                && current_view.chat_entries.iter().any(|entry| {
                    entry.is_streaming
                        || entry
                            .delivery_state
                            .as_deref()
                            .map(|state| state == "pending")
                            .unwrap_or(false)
                });
            let preserved_messages =
                should_preserve_messages.then(|| current_view.chat_entries.clone());
            let next_view = build_workbench_with_models(
                snapshot,
                selected_thread_id,
                preserved_messages,
                endpoint,
                Some(current_view.available_models.clone()),
            )
            .await?;
            Ok(ReduceOutcome::View(next_view))
        }
        Some("threadMessagesChanged") => {
            let Some(payload) = event.get("data").cloned() else {
                return Ok(ReduceOutcome::None);
            };
            let thread_id = payload
                .get("threadID")
                .and_then(Value::as_str)
                .or_else(|| payload.get("threadId").and_then(Value::as_str));
            if thread_id != selected_thread_id {
                return Ok(ReduceOutcome::None);
            }
            if thread_id.is_none() {
                return Ok(ReduceOutcome::None);
            }
            let mut next_view = current_view.clone();
            next_view.chat_entries = chat_entries_from_thread_payload(&payload);
            next_view.context_window_remaining_percent =
                context_window_remaining_percent_from_thread_payload(&payload);
            Ok(ReduceOutcome::View(next_view))
        }
        Some("liveProcessesChanged") => {
            let Some(payload) = event.get("data").cloned() else {
                return Ok(ReduceOutcome::None);
            };
            let thread_id = payload
                .get("threadId")
                .and_then(Value::as_str)
                .or_else(|| payload.get("threadID").and_then(Value::as_str));
            if thread_id != selected_thread_id || thread_id.is_none() {
                return Ok(ReduceOutcome::None);
            }
            let mut next_view = current_view.clone();
            next_view.live_processes = payload
                .get("processes")
                .and_then(Value::as_array)
                .map(|items| parse_live_process_items(items))
                .unwrap_or_default();
            Ok(ReduceOutcome::View(next_view))
        }
        Some("hookFailure") => {
            let Some(payload) = event.get("data").cloned() else {
                return Ok(ReduceOutcome::None);
            };
            let notice: HookFailureNotice = serde_json::from_value(payload)?;
            Ok(ReduceOutcome::HookFailure(notice))
        }
        _ => Ok(ReduceOutcome::None),
    }
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(target_arch = "wasm32")]
async fn send_json(
    write: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    value: &Value,
) -> Result<()> {
    write.send(Message::Text(value.to_string())).await?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(target_arch = "wasm32")]
async fn send_thread_selection(
    write: &mut futures_util::stream::SplitSink<WebSocket, Message>,
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
