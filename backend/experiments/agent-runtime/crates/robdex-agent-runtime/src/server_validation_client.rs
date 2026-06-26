use std::env;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use robdex_agent_runtime::{command_registry, db, workflow_memory};
use robdex_agent_runtime::gui_sync::{RuntimeSyncClient, RuntimeSyncConfig, SyncOutcome};
use robdex_agent_runtime_projection::RuntimeDeltaKind;
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use tokio_tungstenite::connect_async;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    let base_url = env::var("ROBDEX_AGENT_RUNTIME_SERVER_VALIDATION_BASE_URL")
        .context("ROBDEX_AGENT_RUNTIME_SERVER_VALIDATION_BASE_URL is required")?;
    let database_url = env::var("ROBDEX_AGENT_RUNTIME_DATABASE_URL")
        .context("ROBDEX_AGENT_RUNTIME_DATABASE_URL is required")?;
    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
    let pool = db::connect(&database_url).await?;
    if env::var("ROBDEX_AGENT_RUNTIME_SERVER_VALIDATION_HOLD_WS").ok().as_deref() == Some("1") {
        hold_websocket_until_shutdown(&client, base_url.trim_end_matches('/')).await?;
        pool.close().await;
        return Ok(());
    }
    run_validation(&client, &pool, base_url.trim_end_matches('/')).await?;
    pool.close().await;
    println!("[server-validation-client] deterministic resident server validation complete");
    Ok(())
}

async fn run_validation(client: &Client, pool: &PgPool, base: &str) -> Result<()> {
    step("health");
    let health = get_json(client, base, "/health", StatusCode::OK).await?;
    assert_eq_str(&health, "/status", "ok")?;
    verify_preseeded_reconciliation(pool).await?;

    step("snapshot");
    let snapshot = get_json(client, base, "/state/snapshot", StatusCode::OK).await?;
    let _watermark = snapshot.pointer("/watermark").and_then(Value::as_i64).context("snapshot watermark missing")?;

    step("session create/show/history");
    let created = post_json(
        client,
        base,
        "/sessions",
        json!({"role":"runtime-no-rg","project":"server-validation","workdir":".","worktreeRoot":"."}),
        StatusCode::OK,
    )
    .await?;
    let session_id = uuid_at(&created, "/sessionId")?;
    let _shown = get_json(client, base, &format!("/sessions/{session_id}"), StatusCode::OK).await?;
    let _history = get_json(client, base, &format!("/sessions/{session_id}/history"), StatusCode::OK).await?;

    step("role admin inspection/export");
    let _roles = get_json(client, base, "/roles", StatusCode::OK).await?;
    let _role = get_json(client, base, "/roles/runtime-no-rg", StatusCode::OK).await?;
    let _versions = get_json(client, base, "/roles/runtime-no-rg/versions", StatusCode::OK).await?;
    let _export = get_json(client, base, "/roles/runtime-no-rg/export", StatusCode::OK).await?;

    step("command registry request/review/preview/decide/apply");
    let _commands = get_json(client, base, "/command-registry", StatusCode::OK).await?;
    let seed = command_seed("cmd.server.validation");
    let request_id = command_registry::create_request(
        pool,
        session_id,
        command_registry::ChangeRequestInput {
            operation: "add".to_string(),
            command: serde_json::from_value(seed.clone())?,
            rationale: "deterministic resident server validation".to_string(),
            recommended_policy: "operator reviewed deterministic validation".to_string(),
            requester: "server-validation-client".to_string(),
        },
    )
    .await?;
    let _request = get_json(client, base, &format!("/command-registry/requests/{request_id}"), StatusCode::OK).await?;
    let _review = get_json(client, base, &format!("/command-registry/requests/{request_id}/review"), StatusCode::OK).await?;
    let _template = get_json(client, base, &format!("/command-registry/requests/{request_id}/final-template"), StatusCode::OK).await?;
    let decision = json!({
        "sessionId": session_id,
        "status": "approved",
        "finalScope": {"scopeType":"project","projectKey":"server-validation"},
        "finalExecutionPolicy": {"decision":"allow","reason":"deterministic validation"},
        "finalCommand": seed
    });
    let _preview = post_json(client, base, &format!("/command-registry/requests/{request_id}/preview-decision"), decision.clone(), StatusCode::OK).await?;
    let _decided = post_json(client, base, &format!("/command-registry/requests/{request_id}/decide"), decision, StatusCode::OK).await?;
    let _applied = post_json(client, base, &format!("/command-registry/requests/{request_id}/apply"), json!({"sessionId": session_id}), StatusCode::OK).await?;
    let scoped = get_json(client, base, "/command-registry/cmd.server.validation?project=server-validation", StatusCode::OK).await?;
    assert_eq_str(&scoped, "/actionId", "cmd.server.validation")?;

    step("workflow memory inspection/feedback");
    let memory_id = seed_workflow_memory(pool, session_id).await?;
    let memories = get_json(client, base, &format!("/workflow-memories?sessionId={session_id}"), StatusCode::OK).await?;
    assert_array_contains_id(&memories, memory_id)?;
    let _memory = get_json(client, base, &format!("/workflow-memories/{memory_id}?sessionId={session_id}"), StatusCode::OK).await?;
    let _events_before = get_json(client, base, &format!("/workflow-memories/{memory_id}/events?sessionId={session_id}"), StatusCode::OK).await?;
    let _feedback = post_json(
        client,
        base,
        &format!("/workflow-memories/{memory_id}/feedback"),
        json!({"sessionId": session_id, "feedback":"attempted", "payload":{"variant":true,"source":"server-validation"}}),
        StatusCode::OK,
    )
    .await?;
    let events_after = get_json(client, base, &format!("/workflow-memories/{memory_id}/events?sessionId={session_id}"), StatusCode::OK).await?;
    ensure_array_non_empty(&events_after, "workflow memory events after feedback")?;

    step("structured error packets");
    let bad_request = post_raw(client, base, "/sessions", "{", StatusCode::BAD_REQUEST).await?;
    assert_error_code(&bad_request, "bad_request")?;
    let missing_session = get_json(client, base, &format!("/sessions/{}", Uuid::new_v4()), StatusCode::NOT_FOUND).await?;
    assert_error_code(&missing_session, "not_found")?;
    let archived = post_json(client, base, &format!("/sessions/{session_id}/archive"), json!({"reason":"server validation conflict setup"}), StatusCode::OK).await?;
    assert_eq_str(&archived, "/status", "archived")?;
    let conflict = post_json(client, base, &format!("/sessions/{session_id}/send"), json!({"message":"must not call model because session is archived"}), StatusCode::CONFLICT).await?;
    assert_error_code(&conflict, "conflict")?;
    let validation = post_json(client, base, &format!("/command-registry/requests/{request_id}/preview-decision"), json!({"status":"maybe"}), StatusCode::UNPROCESSABLE_ENTITY).await?;
    assert_error_code(&validation, "validation_failed")?;
    let other_session = post_json(client, base, "/sessions", json!({"role":"runtime-no-rg","project":"other-server-validation","workdir":".","worktreeRoot":"."}), StatusCode::OK).await?;
    let other_session_id = uuid_at(&other_session, "/sessionId")?;
    let forbidden = post_json(client, base, &format!("/workflow-memories/{memory_id}/feedback"), json!({"sessionId": other_session_id, "feedback":"helpful", "payload":{}}), StatusCode::FORBIDDEN).await?;
    assert_error_code(&forbidden, "forbidden")?;

    step("websocket semantic delta and resync");
    let ws_session = post_json(client, base, "/sessions", json!({"role":"runtime-no-rg","project":"server-validation-ws","workdir":".","worktreeRoot":"."}), StatusCode::OK).await?;
    let ws_session_id = uuid_at(&ws_session, "/sessionId")?;
    let ws_snapshot = get_json(client, base, "/state/snapshot", StatusCode::OK).await?;
    let watermark = ws_snapshot.pointer("/watermark").and_then(Value::as_i64).context("websocket snapshot watermark missing")?;
    let pending_delta_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM event_stream WHERE sequence > $1")
        .bind(watermark)
        .fetch_one(pool)
        .await?;
    if pending_delta_rows != 0 {
        bail!("websocket validation expected no catch-up rows before live mutation, found {pending_delta_rows} rows after watermark {watermark}");
    }
    println!("[server-validation-client] websocket after={watermark} pending_event_rows={pending_delta_rows}");
    let ws_url = websocket_url(base, &format!("/state/ws?after={watermark}&selectedSessionId={ws_session_id}"))?;
    let (mut ws, _) = connect_async(&ws_url).await.with_context(|| format!("connect websocket {ws_url}"))?;
    let hello = next_ws_json(&mut ws).await?;
    assert_eq_str(&hello, "/type", "hello")?;
    let _archived = post_json(client, base, &format!("/sessions/{ws_session_id}/archive"), json!({}), StatusCode::OK).await?;
    let event_count: i64 = sqlx::query_scalar("SELECT count(*) FROM event_stream WHERE entity_id=$1 AND event_type='session.archived'")
        .bind(ws_session_id)
        .fetch_one(pool)
        .await?;
    if event_count != 1 {
        bail!("archive did not write expected session.archived event for {ws_session_id}; count={event_count}");
    }
    let mut saw_session_archive = false;
    for _ in 0..50 {
        let message = next_ws_json(&mut ws).await?;
        if message.pointer("/type").and_then(Value::as_str) == Some("delta")
            && message.pointer("/delta/type").and_then(Value::as_str) == Some("sessionArchive")
            && message.pointer("/delta/session_id").and_then(Value::as_str) == Some(&ws_session_id.to_string())
        {
            saw_session_archive = true;
            break;
        }
    }
    if !saw_session_archive {
        bail!("websocket did not emit expected sessionArchive semantic delta for {ws_session_id}");
    }
    let resync_url = websocket_url(base, "/state/ws")?;
    let (mut resync_ws, _) = connect_async(&resync_url).await.with_context(|| format!("connect websocket {resync_url}"))?;
    let _hello = next_ws_json(&mut resync_ws).await?;
    let resync = next_ws_json(&mut resync_ws).await?;
    assert_eq_str(&resync, "/type", "resyncRequired")?;
    assert_eq_str(&resync, "/delta/type", "resyncRequired")?;

    step("gui sync client state convergence");
    let mut sync = RuntimeSyncClient::new(RuntimeSyncConfig::new(base.to_string()));
    let sync_watermark = sync.hydrate().await.context("GUI sync hydrate")?.watermark;
    let mut sync_stream = sync.connect_after(Some(sync_watermark)).await.context("GUI sync websocket connect")?;
    match sync_stream.next_outcome(&mut sync).await.context("GUI sync hello")? {
        SyncOutcome::Hello { .. } => {}
        outcome => bail!("GUI sync expected hello, got {outcome:?}"),
    }
    let sync_created = post_json(client, base, "/sessions", json!({"role":"runtime-no-rg","project":"server-validation-gui-sync","workdir":".","worktreeRoot":"."}), StatusCode::OK).await?;
    let sync_session_id = uuid_at(&sync_created, "/sessionId")?;
    let mut saw_sync_upsert = false;
    let mut saw_sync_timeline = false;
    for _ in 0..80 {
        match sync_stream.next_outcome(&mut sync).await.context("GUI sync delta")? {
            SyncOutcome::DeltaApplied { delta, .. } => {
                saw_sync_upsert |= matches!(&delta.kind, RuntimeDeltaKind::SessionUpsert { session } if session.id == sync_session_id.to_string());
                saw_sync_timeline |= matches!(&delta.kind, RuntimeDeltaKind::TimelineAppend { item } if item.event_type == "session.created" && item.session_id.as_deref() == Some(&sync_session_id.to_string()));
                if saw_sync_upsert && saw_sync_timeline {
                    break;
                }
            }
            outcome => bail!("GUI sync expected delta, got {outcome:?}"),
        }
    }
    if !saw_sync_upsert || !saw_sync_timeline {
        bail!("GUI sync did not apply expected session create deltas for {sync_session_id}; saw_upsert={saw_sync_upsert} saw_timeline={saw_sync_timeline}");
    }
    let fresh = get_json(client, base, "/state/snapshot", StatusCode::OK).await?;
    let local_has_session = sync.projection().context("GUI sync projection missing")?.sessions.iter().any(|session| session.id == sync_session_id.to_string());
    let fresh_has_session = fresh
        .pointer("/sessions")
        .and_then(Value::as_array)
        .context("fresh snapshot sessions missing")?
        .iter()
        .any(|session| session.pointer("/id").and_then(Value::as_str) == Some(&sync_session_id.to_string()));
    if local_has_session != fresh_has_session {
        bail!("GUI sync local projection did not converge with fresh snapshot for session {sync_session_id}");
    }

    let mut sync_resync = RuntimeSyncClient::new(RuntimeSyncConfig::new(base.to_string()));
    sync_resync.hydrate().await.context("GUI sync resync hydrate")?;
    let mut omitted_after = sync_resync.connect_after(None).await.context("GUI sync omitted-after connect")?;
    match omitted_after.next_outcome(&mut sync_resync).await.context("GUI sync omitted-after hello")? {
        SyncOutcome::Hello { .. } => {}
        outcome => bail!("GUI sync omitted-after expected hello, got {outcome:?}"),
    }
    match omitted_after.next_outcome(&mut sync_resync).await.context("GUI sync omitted-after resync")? {
        SyncOutcome::ResyncRequired { .. } => {}
        outcome => bail!("GUI sync omitted-after expected resyncRequired, got {outcome:?}"),
    }
    if !sync_resync.resync_required() {
        bail!("GUI sync client did not mark resync_required after omitted after watermark");
    }
    sync_resync.rehydrate().await.context("GUI sync rehydrate after resync")?;
    if sync_resync.resync_required() {
        bail!("GUI sync client did not clear resync_required after rehydrate");
    }
    Ok(())
}

async fn hold_websocket_until_shutdown(client: &Client, base: &str) -> Result<()> {
    step("hold websocket for shutdown");
    let snapshot = get_json(client, base, "/state/snapshot", StatusCode::OK).await?;
    let watermark = snapshot.pointer("/watermark").and_then(Value::as_i64).context("hold websocket snapshot watermark missing")?;
    let ws_url = websocket_url(base, &format!("/state/ws?after={watermark}"))?;
    let (mut ws, _) = connect_async(&ws_url).await.with_context(|| format!("connect websocket {ws_url}"))?;
    let hello = next_ws_json(&mut ws).await?;
    assert_eq_str(&hello, "/type", "hello")?;
    if let Ok(path) = env::var("ROBDEX_AGENT_RUNTIME_SERVER_VALIDATION_HOLD_WS_READY") {
        std::fs::write(&path, "ready\n").with_context(|| format!("write hold-ws ready file {path}"))?;
    }
    loop {
        let message = next_ws_json(&mut ws).await?;
        if message.pointer("/type").and_then(Value::as_str) == Some("serverShutdown") {
            println!("[server-validation-client] websocket observed serverShutdown");
            return Ok(());
        }
    }
}

async fn get_json(client: &Client, base: &str, path: &str, status: StatusCode) -> Result<Value> {
    let url = format!("{base}{path}");
    let response = client.get(&url).send().await.with_context(|| format!("GET {url} failed"))?;
    response_json(response, status, "GET", &url).await
}

async fn post_json(client: &Client, base: &str, path: &str, body: Value, status: StatusCode) -> Result<Value> {
    let url = format!("{base}{path}");
    let response = client.post(&url).json(&body).send().await.with_context(|| format!("POST {url} failed"))?;
    response_json(response, status, "POST", &url).await
}

async fn post_raw(client: &Client, base: &str, path: &str, body: &str, status: StatusCode) -> Result<Value> {
    let url = format!("{base}{path}");
    let response = client.post(&url).header("content-type", "application/json").body(body.to_string()).send().await.with_context(|| format!("POST {url} failed"))?;
    response_json(response, status, "POST", &url).await
}

async fn response_json(response: reqwest::Response, expected: StatusCode, method: &str, url: &str) -> Result<Value> {
    let status = response.status();
    let text = response.text().await.unwrap_or_else(|error| format!("<body read failed: {error}>"));
    if status != expected {
        bail!("{method} {url} expected {expected}, got {status}, body={text}");
    }
    serde_json::from_str(&text).with_context(|| format!("{method} {url} returned non-JSON body: {text}"))
}

async fn next_ws_json(ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>) -> Result<Value> {
    let message = tokio::time::timeout(Duration::from_secs(10), ws.next())
        .await
        .context("timed out waiting for websocket message")?
        .context("websocket archived")?
        .context("websocket message error")?;
    let text = message.into_text().context("websocket message was not text")?;
    serde_json::from_str(&text).with_context(|| format!("websocket text was not JSON: {text}"))
}

async fn seed_workflow_memory(pool: &PgPool, session_id: Uuid) -> Result<Uuid> {
    let turn_id = Uuid::new_v4();
    sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status) VALUES ($1,$2,'user','memory','completed')")
        .bind(turn_id)
        .bind(session_id)
        .execute(pool)
        .await?;
    let tool_id = Uuid::new_v4();
    sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status) VALUES ($1,$2,$3,'execute_code','server-validation-memory','{}'::jsonb,'completed')")
        .bind(tool_id)
        .bind(session_id)
        .bind(turn_id)
        .execute(pool)
        .await?;
    let script_id = Uuid::new_v4();
    sqlx::query("INSERT INTO script_runs (id, tool_call_id, source, status) VALUES ($1,$2,'print(\"server validation memory\")','completed')")
        .bind(script_id)
        .bind(tool_id)
        .execute(pool)
        .await?;
    let memory_id = Uuid::new_v4();
    let vector = format!("[{}]", vec!["0"; workflow_memory::DEFAULT_DIMENSIONS].join(","));
    sqlx::query(
        r#"
        INSERT INTO workflow_memories (
            id, script_run_id, session_id, scope_type, project_key, title, reason, summary,
            provider, model, dimensions, storage_type, source_hash, command_fingerprint, embedding
        ) VALUES ($1,$2,$3,'project','server-validation','Server Validation Memory','Reason','Summary','deterministic','test',$4,'halfvec','server-validation-hash','plain',$5::halfvec)
        "#,
    )
    .bind(memory_id)
    .bind(script_id)
    .bind(session_id)
    .bind(workflow_memory::DEFAULT_DIMENSIONS as i32)
    .bind(vector)
    .execute(pool)
    .await?;
    Ok(memory_id)
}

async fn verify_preseeded_reconciliation(pool: &PgPool) -> Result<()> {
    let Ok(raw_process_id) = env::var("ROBDEX_AGENT_RUNTIME_SERVER_VALIDATION_PRESEEDED_PROCESS_ID") else {
        return Ok(());
    };
    let process_id = Uuid::parse_str(&raw_process_id)?;
    let row = sqlx::query("SELECT session_id, status, termination_reason FROM managed_processes WHERE id=$1")
        .bind(process_id)
        .fetch_one(pool)
        .await
        .with_context(|| format!("preseeded process row missing: {process_id}"))?;
    let session_id: Uuid = row.get("session_id");
    let status: String = row.get("status");
    let reason: Option<String> = row.get("termination_reason");
    if status != "lost" || reason.as_deref() != Some("runtimeRestart") {
        bail!("preseeded process {process_id} expected lost/runtimeRestart, got {status}/{reason:?}");
    }
    let process_events: i64 = sqlx::query_scalar("SELECT count(*) FROM event_stream WHERE entity_type='process' AND entity_id=$1 AND event_type='process.lost' AND status='lost'")
        .bind(process_id)
        .fetch_one(pool)
        .await?;
    let session_events: i64 = sqlx::query_scalar("SELECT count(*) FROM event_stream WHERE entity_type='session' AND entity_id=$1 AND event_type='session.recoveryDegraded' AND status='degraded'")
        .bind(session_id)
        .fetch_one(pool)
        .await?;
    if process_events != 1 || session_events != 1 {
        bail!("preseeded process reconciliation missing event evidence: process_events={process_events}, session_events={session_events}");
    }
    println!("[server-validation-client] preseeded process reconciliation process={process_id} status={status} reason={}", reason.unwrap_or_default());
    Ok(())
}

fn command_seed(action_id: &str) -> Value {
    json!({
        "actionId": action_id,
        "binaryName": "echo",
        "candidatePaths": ["/bin/echo", "/usr/bin/echo"],
        "starlarkObject": "server_validation_echo",
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
        "modelDescription": "deterministic resident server validation command",
        "allowCwdArg": false,
        "allowArgsArg": true,
        "forbiddenArgs": [],
        "executionPolicy": "allow"
    })
}

fn websocket_url(base: &str, path: &str) -> Result<String> {
    if let Some(rest) = base.strip_prefix("http://") {
        Ok(format!("ws://{rest}{path}"))
    } else if let Some(rest) = base.strip_prefix("https://") {
        Ok(format!("wss://{rest}{path}"))
    } else {
        bail!("unsupported base URL for websocket conversion: {base}")
    }
}

fn uuid_at(value: &Value, pointer: &str) -> Result<Uuid> {
    let raw = value.pointer(pointer).and_then(Value::as_str).with_context(|| format!("missing uuid at {pointer}: {value}"))?;
    Uuid::parse_str(raw).with_context(|| format!("invalid uuid at {pointer}: {raw}"))
}

fn assert_eq_str(value: &Value, pointer: &str, expected: &str) -> Result<()> {
    let actual = value.pointer(pointer).and_then(Value::as_str).with_context(|| format!("missing string at {pointer}: {value}"))?;
    if actual != expected {
        bail!("expected {pointer}={expected}, got {actual}; value={value}");
    }
    Ok(())
}

fn assert_error_code(value: &Value, expected: &str) -> Result<()> {
    assert_eq_str(value, "/error/code", expected)?;
    if value.pointer("/error/message").and_then(Value::as_str).is_none() {
        bail!("error packet missing message: {value}");
    }
    if !value.pointer("/error/details").is_some_and(Value::is_object) {
        bail!("error packet missing details object: {value}");
    }
    Ok(())
}

fn assert_array_contains_id(value: &Value, id: Uuid) -> Result<()> {
    let id = id.to_string();
    let Some(items) = value.as_array() else {
        bail!("expected array, got {value}");
    };
    if !items.iter().any(|item| item.get("id").and_then(Value::as_str) == Some(id.as_str())) {
        bail!("array does not contain id {id}: {value}");
    }
    Ok(())
}

fn ensure_array_non_empty(value: &Value, label: &str) -> Result<()> {
    match value.as_array() {
        Some(items) if !items.is_empty() => Ok(()),
        _ => bail!("{label} expected non-empty array, got {value}"),
    }
}

fn step(name: &str) {
    println!("[server-validation-client] {name}");
}
