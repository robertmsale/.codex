use std::{collections::BTreeMap, env, fs, path::{Path, PathBuf}, process::Command, time::{Duration, Instant}};

use anyhow::{Context, Result, bail};
use codex_app_server_adapter::app_server_protocol::RequestId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::timeout;

use crate::{
    models::{BridgeAgentSummary, BridgeAppStateSnapshot, BridgeInstanceSummary, PendingApproval, ThreadCachePayload},
    runtime::BridgeRuntime,
    transforms::{prune_thread_cache_payload, resolve_role_instructions, summarize_scoped_agent_record},
};

#[derive(Debug)]
pub struct CommandOutcome {
    pub payload: Value,
    pub error_message: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedState {
    #[serde(default)]
    global_configs: Value,
    #[serde(default)]
    projects: BTreeMap<String, PersistedProjectState>,
    #[serde(rename = "selectedProjectID", alias = "selectedProjectId")]
    selected_project_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_timestamp")]
    updated_at: Option<u64>,
    #[serde(flatten, default)]
    extras: BTreeMap<String, Value>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedProjectState {
    id: Option<String>,
    name: Option<String>,
    project_root: Option<String>,
    cwd: Option<String>,
    auto_route_replies: Option<bool>,
    route_approval_requests: Option<bool>,
    preferred_model_provider: Option<String>,
    #[serde(default)]
    configs: Value,
    #[serde(default)]
    agents: BTreeMap<String, PersistedAgentState>,
    #[serde(rename = "orchestratorThreadID", alias = "orchestratorThreadId")]
    orchestrator_thread_id: Option<String>,
    #[serde(default)]
    thread_groups: Vec<ThreadGroupState>,
    archived: Option<bool>,
    detached: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_timestamp")]
    updated_at: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_timestamp")]
    created_at: Option<u64>,
    #[serde(flatten, default)]
    extras: BTreeMap<String, Value>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedAgentState {
    display_name: Option<String>,
    role: Option<String>,
    project_root: Option<String>,
    cwd: Option<String>,
    approval_policy: Option<String>,
    sandbox_mode: Option<String>,
    network_access: Option<bool>,
    model: Option<String>,
    reasoning_effort: Option<String>,
    issue_number: Option<u64>,
    pull_request_number: Option<u64>,
    blocked_reason: Option<String>,
    unblock_when: Option<String>,
    archived: Option<bool>,
    #[serde(flatten, default)]
    extras: BTreeMap<String, Value>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ThreadGroupState {
    id: String,
    title: String,
    #[serde(default, rename = "threadIDs", alias = "threadIds")]
    thread_ids: Vec<String>,
    #[serde(default)]
    is_collapsed: bool,
    #[serde(default, deserialize_with = "deserialize_optional_timestamp")]
    created_at: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_timestamp")]
    updated_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
struct ScopedContext {
    sender: crate::models::ScopedAgentRecord,
    visible: Vec<crate::models::ScopedAgentRecord>,
}

#[derive(Debug, Clone)]
struct SenderThreadContext {
    project_path: String,
    project_cwd: String,
    cwd: String,
    approval_policy: Option<String>,
    sandbox_mode: Option<String>,
    network_access: Option<bool>,
    model: Option<String>,
    reasoning_effort: Option<String>,
}

pub async fn make_app_state_snapshot(
    runtime: &BridgeRuntime,
    include_message_cache: bool,
) -> Result<BridgeAppStateSnapshot> {
    let instance_id = runtime.settings().project_path.display().to_string();
    let snapshot = timeout(Duration::from_millis(250), runtime.snapshot()).await;
    let (mut thread_cache, is_running) = match snapshot {
        Ok(Ok(snapshot)) => (
            snapshot.thread_cache,
            snapshot.connection_status == "connected",
        ),
        Ok(Err(error)) => return Err(error),
        Err(_) => {
            tracing::warn!("appStateSnapshot degraded to empty thread cache because runtime snapshot was busy");
            let connection_status = runtime.info().await.connection_status;
            (ThreadCachePayload::default(), connection_status == "connected")
        }
    };
    if !include_message_cache {
        thread_cache.message_cache_by_thread_id.clear();
        thread_cache.context_window_status_by_thread_id.clear();
    }
    let persisted_state_value = runtime.state_document_value().await;
    let state = parse_state(&persisted_state_value);
    let state_value = bridge_state_payload(&state);
    let mut pending_approvals = runtime.pending_approvals().await;
    pending_approvals.sort_by(|lhs, rhs| {
        lhs.thread_id
            .cmp(&rhs.thread_id)
            .then_with(|| lhs.id.cmp(&rhs.id))
    });
    Ok(BridgeAppStateSnapshot {
        state: state_value,
        thread_cache: thread_cache.clone(),
        instances: vec![BridgeInstanceSummary {
            id: instance_id.clone(),
            project_path: instance_id,
            cwd: runtime.settings().cwd.display().to_string(),
            is_running,
            experimental_api_enabled: true,
        }],
        agents: synthesized_agents(&state, &thread_cache.running_thread_ids, &runtime.settings().project_path.display().to_string()),
        pending_approvals,
        review_jobs: Vec::new(),
        active_instance_id: Some(runtime.settings().project_path.display().to_string()),
        generated_at: unix_now(),
    })
}

pub async fn make_event_replay_response(runtime: &BridgeRuntime, since: Option<u64>) -> Result<Value> {
    let replay = runtime.replay_events(since).await;
    let mut events = Vec::with_capacity(replay.events.len());
    for sequenced in replay.events {
        let event = match sequenced.event {
            crate::models::BridgeEvent::ConnectionStatus { message } => {
                json!({
                    "sequence": sequenced.sequence,
                    "event": {
                        "name": "connectionStatus",
                        "message": message,
                    }
                })
            }
            crate::models::BridgeEvent::ThreadMessagesChanged { payload } => {
                json!({
                    "sequence": sequenced.sequence,
                    "event": {
                        "name": "threadMessagesChanged",
                        "data": payload,
                    }
                })
            }
            crate::models::BridgeEvent::AppStateSnapshot { .. } => {
                json!({
                    "sequence": sequenced.sequence,
                    "event": {
                        "name": "appStateSnapshot",
                        "data": make_app_state_snapshot(runtime, false).await?,
                    }
                })
            }
        };
        events.push(event);
    }

    Ok(json!({
        "events": events,
        "latestSequence": replay.latest_sequence,
        "requiresSnapshot": replay.requires_snapshot,
    }))
}

fn bridge_state_payload(state: &PersistedState) -> Value {
    let mut projects = Vec::new();
    let mut thread_metadata_by_id = serde_json::Map::new();
    let mut thread_groups_by_project_path = serde_json::Map::new();
    let mut role_defaults_by_project_path = serde_json::Map::new();
    let mut orchestrator_by_project_path = serde_json::Map::new();

    for project in state.projects.values() {
        let root_path = project.project_root.clone().unwrap_or_default();
        let normalized_root = normalize_path(root_path.clone());
        if normalized_root.is_empty() {
            continue;
        }
        let worktrees = project
            .extras
            .get("worktrees")
            .cloned()
            .unwrap_or_else(|| json!([]));

        projects.push(json!({
            "id": project.id,
            "name": project.name,
            "rootPath": project.project_root,
            "defaultCWD": project.cwd,
            "autoRouteReplies": project.auto_route_replies.unwrap_or(false),
            "routeApprovalRequests": project.route_approval_requests.unwrap_or(false),
            "preferredModelProvider": project.preferred_model_provider,
            "worktrees": worktrees,
        }));

        if !normalized_root.is_empty() {
            thread_groups_by_project_path.insert(
                normalized_root.clone(),
                json!(project.thread_groups),
            );

            if let Some(value) = project
                .configs
                .get("roleModelReasoningDefaults")
                .cloned()
                .filter(|value| value.is_object())
            {
                role_defaults_by_project_path.insert(normalized_root.clone(), value);
            }

            if let Some(orchestrator_thread_id) = project
                .orchestrator_thread_id
                .clone()
                .filter(|value| !value.trim().is_empty())
            {
                orchestrator_by_project_path.insert(normalized_root.clone(), Value::String(orchestrator_thread_id));
            }
        }

        for (thread_id, agent) in &project.agents {
            thread_metadata_by_id.insert(
                thread_id.clone(),
                json!({
                    "displayName": agent.display_name,
                    "role": agent.role,
                    "hidden": if agent.role.as_deref() == Some("hidden") { Some(true) } else { None::<bool> },
                    "issueNumber": agent.issue_number,
                    "pullRequestNumber": agent.pull_request_number,
                    "blockedReason": agent.blocked_reason,
                    "unblockWhen": agent.unblock_when,
                    "approvalPolicy": agent.approval_policy,
                    "sandboxMode": agent.sandbox_mode,
                    "networkAccess": agent.network_access,
                    "projectRootPath": agent.project_root,
                    "preferredCWD": agent.cwd,
                    "modelID": agent.model,
                    "reasoningEffort": agent.reasoning_effort,
                }),
            );
        }
    }

    json!({
        "projectCatalog": {
            "projects": projects,
            "selectedProjectID": state.selected_project_id,
        },
        "threadMetadataByID": thread_metadata_by_id,
        "savedPrompts": state
            .global_configs
            .get("savedPrompts")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "threadGroupsByProjectPath": thread_groups_by_project_path,
        "projectRoleModelReasoningDefaultsByProjectPath": role_defaults_by_project_path,
        "defaultApprovalPolicy": state.global_configs.get("approvalPolicy").cloned(),
        "defaultSandboxMode": state.global_configs.get("sandboxMode").cloned(),
        "defaultNetworkAccess": state.global_configs.get("networkAccess").cloned(),
        "orchestratorThreadIDByProjectPath": orchestrator_by_project_path,
        "updatedAt": state.updated_at,
    })
}

pub async fn execute_bridge_command(
    runtime: &BridgeRuntime,
    name: &str,
    payload: Value,
) -> Result<CommandOutcome> {
    match name {
        "listInstances" => Ok(success(json!({"type":"instances","payload":[make_instance_summary(runtime).await]}))),
        "processSnapshot" => Ok(success(json!({"type":"processSnapshot","payload":null}))),
        "listAgents" => {
            let state = parse_state(&runtime.state_document_value().await);
            let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
            let records = all_agent_records(&state, &running);
            let agents = match payload.get("senderThreadId").and_then(Value::as_str) {
                Some(sender_thread_id) => {
                    let include_archived = payload.get("includeArchived").and_then(Value::as_bool).unwrap_or(false);
                    scoped_agent_context(&records, sender_thread_id, include_archived)?
                        .visible
                        .iter()
                        .map(|record| summarize_scoped_agent_record(record, &runtime.settings().project_path.display().to_string()))
                        .collect::<Vec<_>>()
                }
                None => records
                    .iter()
                    .map(|record| summarize_scoped_agent_record(record, &runtime.settings().project_path.display().to_string()))
                    .collect::<Vec<_>>(),
            };
            Ok(success(json!({"type":"agents","payload": agents})))
        }
        "whoAmI" => {
            let thread_id = payload.get("threadId").and_then(Value::as_str).unwrap_or_default();
            let state = parse_state(&runtime.state_document_value().await);
            let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
            let agent = synthesized_agents(&state, &running, &runtime.settings().project_path.display().to_string())
                .into_iter()
                .find(|agent| agent.thread_id.as_deref() == Some(thread_id));
            Ok(success(json!({
                "type":"whoAmI",
                "payload": {
                    "inRobdexEnvironment": true,
                    "instanceId": runtime.settings().project_path.display().to_string(),
                    "agent": agent,
                }
            })))
        }
        "savePersistedState" => {
            tracing::warn!("ignoring deprecated savePersistedState blob write from GUI");
            Ok(success(json!({"type":"empty"})))
        }
        "saveProjectCatalog" => {
            let mut state = parse_state(&runtime.state_document_value().await);
            let catalog = payload.get("projectCatalog").cloned().unwrap_or(payload);
            save_project_catalog(&mut state, &catalog)?;
            persist_state(runtime, &state).await?;
            Ok(success(json!({"type":"empty"})))
        }
        "projectCreate" => {
            let mut state = parse_state(&runtime.state_document_value().await);
            create_project(&mut state, &payload)?;
            persist_state(runtime, &state).await?;
            Ok(success(json!({"type":"empty"})))
        }
        "projectUpdate" => {
            let mut state = parse_state(&runtime.state_document_value().await);
            update_project(&mut state, &payload)?;
            persist_state(runtime, &state).await?;
            Ok(success(json!({"type":"empty"})))
        }
        "projectSelect" => {
            let mut state = parse_state(&runtime.state_document_value().await);
            select_project(&mut state, payload.get("projectId").and_then(Value::as_str))?;
            persist_state(runtime, &state).await?;
            Ok(success(json!({"type":"empty"})))
        }
        "projectDelete" => {
            let mut state = parse_state(&runtime.state_document_value().await);
            delete_project(&mut state, payload.get("projectId").and_then(Value::as_str))?;
            persist_state(runtime, &state).await?;
            Ok(success(json!({"type":"empty"})))
        }
        "threadCreate" => {
            let payload = create_thread(runtime, &payload).await?;
            Ok(success(json!({"type":"threadCreate","payload": payload})))
        }
        "threadMessageCreate" => {
            let payload = create_thread_message(runtime, &payload).await?;
            Ok(success(json!({"type":"threadMessageCreate","payload": payload})))
        }
        "setOrchestratorThread" => {
            let mut state = parse_state(&runtime.state_document_value().await);
            set_orchestrator_thread(
                &mut state,
                payload.get("threadId").and_then(Value::as_str),
                payload.get("projectPath").and_then(Value::as_str),
            );
            persist_state(runtime, &state).await?;
            Ok(success(json!({"type":"empty"})))
        }
        "registerTrackedThread" => {
            let mut state = parse_state(&runtime.state_document_value().await);
            register_tracked_thread(&mut state, &payload)?;
            persist_state(runtime, &state).await?;
            Ok(success(json!({"type":"empty"})))
        }
        "threadRunningStateSet" => {
            let thread_id = required_string(&payload, "threadId")?;
            let is_running = payload
                .get("isRunning")
                .and_then(Value::as_bool)
                .ok_or_else(|| anyhow::anyhow!("Missing isRunning"))?;
            runtime
                .set_manual_thread_running_state(&thread_id, is_running)
                .await?;
            Ok(success(json!({"type":"empty"})))
        }
        "threadSelectionSet" => Ok(success(json!({"type":"empty"}))),
        "eventReplay" => {
            let since = payload.get("sinceSequence").and_then(Value::as_u64);
            Ok(success(json!({"type":"eventReplay","payload": make_event_replay_response(runtime, since).await?})))
        }
        "threadStart" => {
            let state = parse_state(&runtime.state_document_value().await);
            let role = payload.get("role").and_then(Value::as_str);
            let project_path = payload.get("projectPath").and_then(Value::as_str);
            let cwd = required_string(&payload, "cwd")?;
            let sandbox_mode = payload
                .get("sandbox")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    state
                        .global_configs
                        .get("sandboxMode")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            let network_access = effective_network_access_for_sandbox(
                sandbox_mode.as_deref(),
                payload.get("networkAccess").and_then(Value::as_bool),
                state.global_configs.get("networkAccess").and_then(Value::as_bool),
            );
            let reasoning_effort = payload
                .get("reasoningEffort")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| role_default_reasoning_effort(&state, project_path, role));
            let params = json!({
                "cwd": cwd,
                "approvalPolicy": payload.get("approvalPolicy").cloned().unwrap_or(Value::Null),
                "sandbox": sandbox_mode,
                "sandboxPolicy": sandbox_policy_for_spawn(sandbox_mode.as_deref(), network_access, Some(cwd.as_str())),
                "model": payload.get("model").cloned().unwrap_or(role_default_model(&state, project_path, role).map(Value::String).unwrap_or(Value::Null)),
                "effort": reasoning_effort,
                "developerInstructions": developer_instructions_for_role(&state, role, project_path, Some(cwd.as_str())),
                "baseInstructions": resolve_role_instructions_for(role)?,
                "persistExtendedHistory": true
            });
            let result = app_server_request_json(runtime, "thread/start", params).await?;
            Ok(success(json!({"type":"thread","payload": result.get("thread").cloned().unwrap_or(result)})))
        }
        "threadResume" => {
            let state = parse_state(&runtime.state_document_value().await);
            let thread_id = required_string(&payload, "threadId")?;
            let role = effective_role_for_thread(&state, &thread_id, payload.get("role").and_then(Value::as_str));
            let cwd = payload
                .get("cwd")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_cwd_for_thread(&state, &thread_id));
            let approval_policy = payload
                .get("approvalPolicy")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_approval_policy_for_thread(&state, &thread_id));
            let sandbox_mode = payload
                .get("sandbox")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_sandbox_mode_for_thread(&state, &thread_id));
            let network_access = effective_network_access_for_sandbox(
                sandbox_mode.as_deref(),
                payload.get("networkAccess").and_then(Value::as_bool)
                    .or_else(|| tracked_network_access_for_thread(&state, &thread_id)),
                state.global_configs.get("networkAccess").and_then(Value::as_bool),
            );
            let model = payload
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_model_for_thread(&state, &thread_id));
            let reasoning_effort = payload
                .get("reasoningEffort")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_reasoning_effort_for_thread(&state, &thread_id));
            let params = json!({
                "threadId": thread_id,
                "cwd": cwd,
                "approvalPolicy": approval_policy,
                "sandbox": sandbox_mode,
                "sandboxPolicy": sandbox_policy_for_spawn(sandbox_mode.as_deref(), network_access, cwd.as_deref()),
                "model": model,
                "effort": reasoning_effort,
                "baseInstructions": resolve_role_instructions_for(role.as_deref())?,
                "developerInstructions": developer_instructions_for_role(&state, role.as_deref(), tracked_project_path_for_thread(&state, &thread_id).as_deref(), cwd.as_deref()),
                "persistExtendedHistory": true
            });
            let result = app_server_request_json(runtime, "thread/resume", params).await?;
            let mut state = parse_state(&runtime.state_document_value().await);
            if update_tracked_thread_session_config(
                &mut state,
                &thread_id,
                payload.get("approvalPolicy").and_then(Value::as_str),
                payload.get("sandbox").and_then(Value::as_str),
                payload.get("networkAccess").and_then(Value::as_bool),
                payload.get("cwd").and_then(Value::as_str),
            ) {
                persist_state(runtime, &state).await?;
            }
            Ok(success(json!({"type":"thread","payload": summarize_thread_payload(&result)})))
        }
        "threadFork" => {
            let state = parse_state(&runtime.state_document_value().await);
            let thread_id = required_string(&payload, "threadId")?;
            let role = effective_role_for_thread(&state, &thread_id, payload.get("role").and_then(Value::as_str));
            let cwd = payload
                .get("cwd")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_cwd_for_thread(&state, &thread_id));
            let approval_policy = payload
                .get("approvalPolicy")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_approval_policy_for_thread(&state, &thread_id));
            let sandbox_mode = payload
                .get("sandbox")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_sandbox_mode_for_thread(&state, &thread_id));
            let network_access = effective_network_access_for_sandbox(
                sandbox_mode.as_deref(),
                payload.get("networkAccess").and_then(Value::as_bool)
                    .or_else(|| tracked_network_access_for_thread(&state, &thread_id)),
                state.global_configs.get("networkAccess").and_then(Value::as_bool),
            );
            let model = payload
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_model_for_thread(&state, &thread_id));
            let params = json!({
                "threadId": thread_id,
                "path": payload.get("path").cloned().unwrap_or(Value::Null),
                "model": model,
                "modelProvider": payload.get("modelProvider").cloned().unwrap_or(Value::Null),
                "cwd": cwd,
                "approvalPolicy": approval_policy,
                "sandbox": sandbox_mode,
                "sandboxPolicy": sandbox_policy_for_spawn(sandbox_mode.as_deref(), network_access, cwd.as_deref()),
                "config": payload.get("config").cloned().unwrap_or(Value::Null),
                "baseInstructions": resolve_role_instructions_for(role.as_deref())?,
                "developerInstructions": developer_instructions_for_role(&state, role.as_deref(), tracked_project_path_for_thread(&state, &thread_id).as_deref(), cwd.as_deref()),
                "persistExtendedHistory": payload.get("persistExtendedHistory").cloned().unwrap_or(Value::Bool(true))
            });
            let result = app_server_request_json(runtime, "thread/fork", params).await?;
            Ok(success(json!({"type":"thread","payload": result.get("thread").cloned().unwrap_or(result)})))
        }
        "threadArchive" => {
            let thread_id = required_string(&payload, "threadId")?;
            archive_thread(runtime, &thread_id).await?;
            Ok(success(json!({"type":"empty"})))
        }
        "threadUnarchive" => {
            let result = app_server_request_json(runtime, "thread/unarchive", json!({"threadId": required_string(&payload, "threadId")?})).await?;
            Ok(success(json!({"type":"thread","payload": result.get("thread").cloned().unwrap_or(result)})))
        }
        "threadNameSet" => {
            let thread_id = required_string(&payload, "threadId")?;
            let name = required_string(&payload, "name")?;
            app_server_request_json(runtime, "thread/name/set", json!({"threadId": thread_id, "name": name})).await?;
            let mut state = parse_state(&runtime.state_document_value().await);
            set_tracked_thread_display_name(&mut state, &thread_id, &name);
            persist_state(runtime, &state).await?;
            runtime
                .push_event(crate::models::BridgeEvent::AppStateSnapshot {
                    state: runtime.state_document_value().await,
                })
                .await;
            Ok(success(json!({"type":"empty"})))
        }
        "threadMetadataUpdate" => {
            let thread_id = required_string(&payload, "threadId")?;
            let mut state = parse_state(&runtime.state_document_value().await);
            if update_tracked_thread_metadata(&mut state, &thread_id, &payload) {
                persist_state(runtime, &state).await?;
                runtime
                    .push_event(crate::models::BridgeEvent::AppStateSnapshot {
                        state: runtime.state_document_value().await,
                    })
                    .await;
            }
            Ok(success(json!({"type":"threadMetadata","payload": {"threadId": thread_id}})))
        }
        "threadCompactStart" => {
            app_server_request_json(
                runtime,
                "thread/compact/start",
                json!({"threadId": required_string(&payload, "threadId")?}),
            )
            .await?;
            Ok(success(json!({"type":"empty"})))
        }
        "threadRollback" => {
            let result = app_server_request_json(
                runtime,
                "thread/rollback",
                json!({
                    "threadId": required_string(&payload, "threadId")?,
                    "numTurns": payload.get("numTurns").cloned().unwrap_or(Value::Null),
                }),
            )
            .await?;
            Ok(success(json!({"type":"thread","payload": result.get("thread").cloned().unwrap_or(result)})))
        }
        "threadLoadedList" => {
            let result = app_server_request_json(
                runtime,
                "thread/loaded/list",
                json!({
                    "cursor": payload.get("cursor").cloned().unwrap_or(Value::Null),
                    "limit": payload.get("limit").cloned().unwrap_or(Value::Null),
                }),
            )
            .await?;
            Ok(success(json!({"type":"threadLoadedList","payload": result})))
        }
        "turnStart" => {
            let state = parse_state(&runtime.state_document_value().await);
            let thread_id = required_string(&payload, "threadId")?;
            let cwd = payload
                .get("cwd")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_cwd_for_thread(&state, &thread_id));
            let approval_policy = payload
                .get("approvalPolicy")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_approval_policy_for_thread(&state, &thread_id));
            let sandbox_policy = payload
                .get("sandboxPolicy")
                .cloned()
                .filter(|value| !value.is_null())
                .or_else(|| tracked_sandbox_policy_for_thread(&state, &thread_id));
            let model = payload
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_model_for_thread(&state, &thread_id));
            let effort = payload
                .get("effort")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_reasoning_effort_for_thread(&state, &thread_id));
            let result = app_server_request_json(
                runtime,
                "turn/start",
                json!({
                    "threadId": thread_id,
                    "input": [{"type":"text","text": required_string(&payload, "text")?}],
                    "cwd": cwd,
                    "model": model,
                    "effort": effort,
                    "approvalPolicy": approval_policy,
                    "sandboxPolicy": sandbox_policy,
                }),
            )
            .await?;
            Ok(success(json!({"type":"turn","payload": result.get("turn").cloned().unwrap_or(result)})))
        }
        "commandExec" => {
            let result = app_server_request_json(
                runtime,
                "command/exec",
                json!({
                    "command": required_string(&payload, "command")?,
                    "cwd": payload.get("cwd").cloned().unwrap_or(Value::Null),
                    "timeoutMs": payload.get("timeoutMs").cloned().unwrap_or(Value::Null),
                }),
            )
            .await?;
            Ok(success(json!({"type":"commandExec","payload": result})))
        }
        "turnInterrupt" => {
            let thread_id = required_string(&payload, "threadId")?;
            let turn_id = runtime
                .active_turn_id_for_thread(&thread_id)
                .await
                .ok_or_else(|| anyhow::anyhow!("No active turn ID is tracked for thread {thread_id}."))?;
            app_server_request_json(runtime, "turn/interrupt", json!({"threadId": thread_id, "turnId": turn_id})).await?;
            Ok(success(json!({"type":"empty"})))
        }
        "commandApproval" => {
            let request_id = required_request_id(&payload, "requestId")?;
            let decision = required_string(&payload, "decision")?;
            let message = payload.get("message").and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()).map(str::to_string);
            let approval = runtime
                .pending_approvals()
                .await
                .into_iter()
                .find(|approval| approval.request_id == request_id);
            let mut follow_up_error = None;
            let follow_up_requested = decision == "decline" && message.is_some();
            if let (Some(approval), Some(message)) = (approval.as_ref(), message.as_deref()) {
                if let Err(error) = send_follow_up_message(runtime, approval, message).await {
                    follow_up_error = Some(error.to_string());
                }
            }
            runtime
                .send_server_response(
                    request_id,
                    json!({
                        "decision": decision,
                    }),
                )
                .await?;
            Ok(success(json!({
                "type":"approvalResult",
                "payload": {
                    "followUpMessageRequested": follow_up_requested,
                    "followUpMessageSent": follow_up_requested && follow_up_error.is_none(),
                    "followUpError": follow_up_error,
                }
            })))
        }
        "fileApproval" => {
            let request_id = required_request_id(&payload, "requestId")?;
            let decision = required_string(&payload, "decision")?;
            let message = payload.get("message").and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()).map(str::to_string);
            let approval = runtime
                .pending_approvals()
                .await
                .into_iter()
                .find(|approval| approval.request_id == request_id);
            let mut follow_up_error = None;
            let follow_up_requested = decision == "decline" && message.is_some();
            if let (Some(approval), Some(message)) = (approval.as_ref(), message.as_deref()) {
                if let Err(error) = send_follow_up_message(runtime, approval, message).await {
                    follow_up_error = Some(error.to_string());
                }
            }
            runtime
                .send_server_response(
                    request_id,
                    json!({
                        "decision": decision,
                    }),
                )
                .await?;
            Ok(success(json!({
                "type":"approvalResult",
                "payload": {
                    "followUpMessageRequested": follow_up_requested,
                    "followUpMessageSent": follow_up_requested && follow_up_error.is_none(),
                    "followUpError": follow_up_error,
                }
            })))
        }
        "toolUserInputResponse" => {
            let request_id = required_request_id(&payload, "requestId")?;
            let answers = payload
                .get("answers")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow::anyhow!("Missing answers"))?;
            let mut mapped = serde_json::Map::new();
            for answer in answers {
                let question_id = answer
                    .get("questionId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("Missing answers[].questionId"))?;
                let value = answer
                    .get("value")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("Missing answers[].value"))?;
                mapped.insert(question_id.to_string(), json!({"answers":[value]}));
            }
            runtime
                .send_server_response(request_id, json!({"answers": Value::Object(mapped)}))
                .await?;
            Ok(success(json!({"type":"empty"})))
        }
        "sendAgentInput" => {
            let state = parse_state(&runtime.state_document_value().await);
            let agent_id = required_string(&payload, "agentId")?;
            let text = required_string(&payload, "text")?;
            let sender_agent_id = payload.get("senderAgentId").and_then(Value::as_str);
            let recipient_thread_id = agent_id;
            let normalized_text = normalized_agent_input_text(
                &text,
                sender_display_name_for_thread(&state, sender_agent_id),
            );
            let result = send_thread_input(
                runtime,
                &state,
                &recipient_thread_id,
                &normalized_text,
                payload.get("modelID").and_then(Value::as_str),
                payload.get("reasoningEffort").and_then(Value::as_str),
            )
            .await?;
            Ok(success(json!({"type":"turn","payload": result})))
        }
        "commandExecutionTerminate" => {
            let thread_id = required_string(&payload, "threadId")?;
            let item_id = payload.get("itemId").and_then(Value::as_str);
            if let Some(target) = resolve_command_termination_target(runtime, &thread_id, item_id).await? {
                terminate_process_group_by_pid(&target)?;
            }
            Ok(success(json!({"type":"empty"})))
        }
        "modelList" => {
            let result = app_server_request_json(runtime, "model/list", json!({})).await?;
            Ok(success(json!({"type":"models","payload": result.get("data").cloned().unwrap_or(result)})))
        }
        "skillsList" => {
            let cwds = payload
                .get("cwds")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_else(|| vec![Value::String(runtime.settings().cwd.display().to_string())]);
            let result = app_server_request_json(
                runtime,
                "skills/list",
                json!({
                    "cwds": cwds,
                    "forceReload": payload.get("forceReload").and_then(Value::as_bool).unwrap_or(false),
                    "perCwdExtraUserRoots": payload.get("perCwdExtraUserRoots").cloned().unwrap_or(Value::Null),
                }),
            )
            .await?;
            Ok(success(json!({"type":"skillsList","payload": result})))
        }
        "skillsRemoteList" => {
            let result = app_server_request_json(
                runtime,
                "skills/remote/list",
                json!({
                    "hazelnutScope": payload.get("hazelnutScope").cloned().unwrap_or(json!("example")),
                    "productSurface": payload.get("productSurface").cloned().unwrap_or(json!("codex")),
                    "enabled": payload.get("enabled").and_then(Value::as_bool).unwrap_or(false),
                }),
            )
            .await?;
            Ok(success(json!({"type":"skillsRemoteList","payload": result})))
        }
        "skillsRemoteExport" => {
            let result = app_server_request_json(
                runtime,
                "skills/remote/export",
                json!({
                    "hazelnutId": required_string(&payload, "hazelnutId")?,
                }),
            )
            .await?;
            Ok(success(json!({"type":"skillsRemoteExport","payload": result})))
        }
        "skillsConfigWrite" => {
            let result = app_server_request_json(
                runtime,
                "skills/config/write",
                json!({
                    "path": required_string(&payload, "path")?,
                    "enabled": payload.get("enabled").and_then(Value::as_bool).unwrap_or(false),
                }),
            )
            .await?;
            Ok(success(json!({"type":"skillsConfigWrite","payload": result})))
        }
        "configRead" => {
            let result = app_server_request_json(
                runtime,
                "config/read",
                json!({
                    "includeLayers": payload.get("includeLayers").and_then(Value::as_bool).unwrap_or(false),
                    "cwd": payload.get("cwd").cloned().unwrap_or(Value::Null),
                }),
            )
            .await?;
            Ok(success(json!({"type":"configRead","payload": result})))
        }
        "configValueWrite" => {
            let result = app_server_request_json(
                runtime,
                "config/value/write",
                json!({
                    "keyPath": required_string(&payload, "keyPath")?,
                    "value": payload.get("value").cloned().unwrap_or(Value::Null),
                    "mergeStrategy": payload.get("mergeStrategy").cloned().unwrap_or(Value::Null),
                    "filePath": payload.get("filePath").cloned().unwrap_or(Value::Null),
                    "expectedVersion": payload.get("expectedVersion").cloned().unwrap_or(Value::Null),
                }),
            )
            .await?;
            Ok(success(json!({"type":"configWrite","payload": result})))
        }
        "configBatchWrite" => {
            let result = app_server_request_json(
                runtime,
                "config/batchWrite",
                json!({
                    "edits": payload.get("edits").cloned().unwrap_or_else(|| json!([])),
                    "filePath": payload.get("filePath").cloned().unwrap_or(Value::Null),
                    "expectedVersion": payload.get("expectedVersion").cloned().unwrap_or(Value::Null),
                }),
            )
            .await?;
            Ok(success(json!({"type":"configWrite","payload": result})))
        }
        "configRequirementsRead" => {
            let result = app_server_request_json(
                runtime,
                "configRequirements/read",
                json!({}),
            )
            .await?;
            Ok(success(json!({"type":"configRequirements","payload": result})))
        }
        "workspaceList" => {
            let entries = workspace_entries(runtime, payload.get("relativePath").and_then(Value::as_str))?;
            Ok(success(json!({"type":"workspaceFiles","payload": entries})))
        }
        "workspaceReadFile" => {
            let file = workspace_read_file(
                runtime,
                &required_string(&payload, "relativePath")?,
                payload.get("maxBytes").and_then(Value::as_u64),
            )?;
            Ok(success(json!({"type":"workspaceFile","payload": file})))
        }
        "dynamicToolCallResponse" => {
            let request_id = required_request_id(&payload, "requestId")?;
            runtime
                .send_server_response(
                    request_id,
                    json!({
                        "success": payload.get("success").and_then(Value::as_bool).unwrap_or(false),
                        "contentItems": payload.get("contentItems").cloned().unwrap_or_else(|| json!([])),
                    }),
                )
                .await?;
            Ok(success(json!({"type":"empty"})))
        }
        "spawnAgent" => {
            let agent = spawn_agent(runtime, &payload).await?;
            Ok(success(json!({"type":"agent","payload": agent})))
        }
        "waitAgent" => {
            let agent = wait_for_agent(runtime, &payload).await?;
            Ok(success(json!({"type":"agent","payload": agent})))
        }
        "closeAgent" => {
            let agent = close_agent(runtime, &payload).await?;
            Ok(success(json!({"type":"agent","payload": agent})))
        }
        other => Ok(CommandOutcome {
            payload: json!({"type":"empty"}),
            error_message: Some(format!("unsupported bridge command: {other}")),
        }),
    }
}

fn success(payload: Value) -> CommandOutcome {
    CommandOutcome {
        payload,
        error_message: None,
    }
}

async fn app_server_request_json(
    runtime: &BridgeRuntime,
    method: impl Into<String>,
    params: Value,
) -> Result<Value> {
    runtime.request_app_server_json(method, params).await
}

fn parse_state(value: &Value) -> PersistedState {
    serde_json::from_value(value.clone()).unwrap_or_default()
}

fn deserialize_optional_timestamp<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                Ok(Some(value))
            } else if let Some(value) = number.as_f64() {
                Ok(Some(value.floor().max(0.0) as u64))
            } else if let Some(value) = number.as_i64() {
                Ok(Some(value.max(0) as u64))
            } else {
                Ok(None)
            }
        }
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            if let Ok(value) = trimmed.parse::<u64>() {
                return Ok(Some(value));
            }
            if let Ok(value) = trimmed.parse::<f64>() {
                return Ok(Some(value.floor().max(0.0) as u64));
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

async fn persist_state(runtime: &BridgeRuntime, state: &PersistedState) -> Result<()> {
    runtime
        .persist_state_document(serde_json::to_value(state)?)
        .await?;
    runtime
        .push_event(crate::models::BridgeEvent::AppStateSnapshot {
            state: runtime.state_document_value().await,
        })
        .await;
    Ok(())
}

fn save_project_catalog(state: &mut PersistedState, catalog: &Value) -> Result<()> {
    let projects = catalog
        .get("projects")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let existing = state.projects.clone();
    let mut next_projects = BTreeMap::new();
    for entry in projects {
        let root_path = entry
            .get("rootPath")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing project.rootPath"))?
            .to_string();
        let name = entry
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing project.name"))?
            .to_string();
        let prior = existing
            .values()
            .find(|project| project.project_root.as_deref() == Some(root_path.as_str()))
            .cloned()
            .unwrap_or_default();
        next_projects.insert(
            name.clone(),
            PersistedProjectState {
                id: entry.get("id").and_then(Value::as_str).map(str::to_string).or(prior.id).or_else(|| Some(uuid())),
                name: Some(name),
                project_root: Some(root_path.clone()),
                cwd: entry.get("defaultCWD").and_then(Value::as_str).map(str::to_string).or(prior.cwd).or_else(|| Some(root_path)),
                auto_route_replies: entry.get("autoRouteReplies").and_then(Value::as_bool).or(prior.auto_route_replies).or(Some(false)),
                route_approval_requests: entry.get("routeApprovalRequests").and_then(Value::as_bool).or(prior.route_approval_requests).or(Some(false)),
                preferred_model_provider: entry.get("preferredModelProvider").and_then(Value::as_str).map(str::to_string).or(prior.preferred_model_provider),
                configs: prior.configs,
                agents: prior.agents,
                orchestrator_thread_id: prior.orchestrator_thread_id,
                thread_groups: prior.thread_groups,
                archived: prior.archived,
                detached: prior.detached,
                updated_at: Some(unix_now()),
                created_at: prior.created_at.or(Some(unix_now())),
                extras: prior.extras,
            },
        );
    }
    state.projects = next_projects;
    state.selected_project_id = catalog.get("selectedProjectID").and_then(Value::as_str).map(str::to_string);
    state.updated_at = Some(unix_now());
    Ok(())
}

fn create_project(state: &mut PersistedState, payload: &Value) -> Result<()> {
    let root_path = required_string(payload, "rootPath")?;
    let default_cwd = payload
        .get("defaultCWD")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(root_path.as_str())
        .to_string();
    let normalized_name = payload
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| basename(&root_path));

    let key = normalize_path(root_path.clone());
    if key.is_empty() {
        bail!("project rootPath is required");
    }
    if state
        .projects
        .values()
        .any(|project| project.project_root.as_deref() == Some(root_path.as_str()))
    {
        bail!("project already exists");
    }

    state.projects.insert(
        key,
        PersistedProjectState {
            id: Some(uuid()),
            name: Some(normalized_name),
            project_root: Some(root_path),
            cwd: Some(default_cwd),
            auto_route_replies: Some(payload.get("autoRouteReplies").and_then(Value::as_bool).unwrap_or(false)),
            route_approval_requests: Some(payload.get("routeApprovalRequests").and_then(Value::as_bool).unwrap_or(false)),
            preferred_model_provider: payload
                .get("preferredModelProvider")
                .and_then(Value::as_str)
                .map(str::to_string),
            configs: json!({}),
            agents: BTreeMap::new(),
            orchestrator_thread_id: None,
            thread_groups: Vec::new(),
            archived: Some(false),
            detached: Some(false),
            updated_at: Some(unix_now()),
            created_at: Some(unix_now()),
            extras: BTreeMap::new(),
        },
    );
    state.selected_project_id = state
        .projects
        .values()
        .find(|project| project.project_root.as_deref() == payload.get("rootPath").and_then(Value::as_str))
        .and_then(|project| project.id.clone());
    state.updated_at = Some(unix_now());
    Ok(())
}

fn select_project(state: &mut PersistedState, project_id: Option<&str>) -> Result<()> {
    match project_id {
        Some(project_id) if !project_id.trim().is_empty() => {
            if state
                .projects
                .values()
                .any(|project| project.id.as_deref() == Some(project_id))
            {
                state.selected_project_id = Some(project_id.to_string());
                state.updated_at = Some(unix_now());
                Ok(())
            } else {
                bail!("project not found");
            }
        }
        _ => {
            state.selected_project_id = None;
            state.updated_at = Some(unix_now());
            Ok(())
        }
    }
}

fn update_project(state: &mut PersistedState, payload: &Value) -> Result<()> {
    let project_id = required_string(payload, "projectId")?;
    let project = state
        .projects
        .values_mut()
        .find(|project| project.id.as_deref() == Some(project_id.as_str()))
        .ok_or_else(|| anyhow::anyhow!("project not found"))?;

    let name = payload
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("project name is required"))?;
    let default_cwd = payload
        .get("defaultCWD")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("defaultCWD is required"))?;

    project.name = Some(name.to_string());
    project.cwd = Some(default_cwd.to_string());
    if payload.get("autoRouteReplies").is_some() {
        project.auto_route_replies = Some(
            payload
                .get("autoRouteReplies")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        );
    }
    if payload.get("routeApprovalRequests").is_some() {
        project.route_approval_requests = Some(
            payload
                .get("routeApprovalRequests")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        );
    }
    if payload.get("preferredModelProvider").is_some() {
        project.preferred_model_provider = payload
            .get("preferredModelProvider")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }
    if let Some(role_defaults) = payload
        .get("roleModelReasoningDefaults")
        .cloned()
        .filter(|value| value.is_object())
    {
        let mut configs = project.configs.as_object().cloned().unwrap_or_default();
        configs.insert("roleModelReasoningDefaults".to_string(), role_defaults);
        project.configs = Value::Object(configs);
    }
    project.updated_at = Some(unix_now());
    state.updated_at = Some(unix_now());
    Ok(())
}

fn delete_project(state: &mut PersistedState, project_id: Option<&str>) -> Result<()> {
    let project_id = project_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("projectId is required"))?;
    let key = state
        .projects
        .iter()
        .find(|(_, project)| project.id.as_deref() == Some(project_id))
        .map(|(key, _)| key.clone())
        .ok_or_else(|| anyhow::anyhow!("project not found"))?;
    state.projects.remove(&key);
    if state.selected_project_id.as_deref() == Some(project_id) {
        state.selected_project_id = state.projects.values().find_map(|project| project.id.clone());
    }
    state.updated_at = Some(unix_now());
    Ok(())
}

fn selected_or_requested_project<'a>(
    state: &'a PersistedState,
    project_id: Option<&str>,
) -> Result<(&'a String, &'a PersistedProjectState)> {
    if let Some(project_id) = project_id.map(str::trim).filter(|value| !value.is_empty()) {
        return state
            .projects
            .iter()
            .find(|(_, project)| project.id.as_deref() == Some(project_id))
            .ok_or_else(|| anyhow::anyhow!("project not found"));
    }
    let selected_project_id = state
        .selected_project_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no selected project"))?;
    state
        .projects
        .iter()
        .find(|(_, project)| project.id.as_deref() == Some(selected_project_id))
        .ok_or_else(|| anyhow::anyhow!("selected project not found"))
}

fn set_orchestrator_thread(state: &mut PersistedState, thread_id: Option<&str>, project_path: Option<&str>) {
    for project in state.projects.values_mut() {
        if project.project_root.as_deref() == project_path {
            project.orchestrator_thread_id = thread_id.map(str::to_string);
            if let Some(thread_id) = thread_id {
                if let Some(agent) = project.agents.get_mut(thread_id) {
                    agent.role = Some("orchestrator".to_string());
                    if agent.display_name.is_none() {
                        agent.display_name = Some(thread_id.to_string());
                    }
                }
            }
            for (agent_thread_id, agent) in &mut project.agents {
                if Some(agent_thread_id.as_str()) != thread_id && agent.role.as_deref() == Some("orchestrator") {
                    agent.role = Some("worker".to_string());
                }
            }
            project.updated_at = Some(unix_now());
        }
    }
    state.updated_at = Some(unix_now());
}

fn update_tracked_thread_metadata(state: &mut PersistedState, thread_id: &str, payload: &Value) -> bool {
    for project in state.projects.values_mut() {
        let is_project_orchestrator = project.orchestrator_thread_id.as_deref() == Some(thread_id);
        if !project.agents.contains_key(thread_id) && !is_project_orchestrator {
            continue;
        }

        let default_display_name = if is_project_orchestrator {
            project
                .name
                .clone()
                .map(|name| format!("{name} Orchestrator"))
                .unwrap_or_else(|| thread_id.to_string())
        } else {
            thread_id.to_string()
        };
        let default_role = if is_project_orchestrator {
            "orchestrator".to_string()
        } else {
            "worker".to_string()
        };
        let project_root = project.project_root.clone();
        let project_cwd = project.cwd.clone().or_else(|| project_root.clone());

        let next_role = payload
            .get("role")
            .and_then(Value::as_str)
            .map(str::to_string);
        {
            let agent = project
                .agents
                .entry(thread_id.to_string())
                .or_insert_with(|| PersistedAgentState {
                    display_name: Some(default_display_name),
                    role: Some(default_role),
                    project_root,
                    cwd: project_cwd,
                    ..PersistedAgentState::default()
                });

            if payload.get("role").is_some() {
                agent.role = next_role.clone();
            }
            if payload.get("approvalPolicy").is_some() {
                agent.approval_policy = payload
                    .get("approvalPolicy")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if payload.get("sandboxMode").is_some() {
                agent.sandbox_mode = payload
                    .get("sandboxMode")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if payload.get("networkAccess").is_some() {
                agent.network_access = payload.get("networkAccess").and_then(Value::as_bool);
            }
            if payload.get("modelID").is_some() {
                agent.model = payload
                    .get("modelID")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if payload.get("reasoningEffort").is_some() {
                agent.reasoning_effort = payload
                    .get("reasoningEffort")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
        }
        if payload.get("role").is_some() {
            if next_role.as_deref() == Some("orchestrator") {
                project.orchestrator_thread_id = Some(thread_id.to_string());
                for (agent_thread_id, other_agent) in &mut project.agents {
                    if agent_thread_id != thread_id
                        && other_agent.role.as_deref() == Some("orchestrator")
                    {
                        other_agent.role = Some("worker".to_string());
                    }
                }
            } else if project.orchestrator_thread_id.as_deref() == Some(thread_id) {
                project.orchestrator_thread_id = None;
            }
        }
        project.updated_at = Some(unix_now());
        state.updated_at = Some(unix_now());
        return true;
    }
    false
}

fn register_tracked_thread(state: &mut PersistedState, payload: &Value) -> Result<()> {
    let thread = payload.get("thread").cloned().unwrap_or(Value::Null);
    let thread_id = thread.get("id").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("Missing thread.id"))?;
    let project_path = required_string(payload, "projectPath")?;
    let preferred_cwd = payload.get("preferredCWD").and_then(Value::as_str).map(str::to_string);
    let role = payload.get("role").and_then(Value::as_str).unwrap_or("worker").to_string();

    let key = state
        .projects
        .iter()
        .find(|(_, project)| project.project_root.as_deref() == Some(project_path.as_str()))
        .map(|(key, _)| key.clone())
        .unwrap_or_else(|| basename(&project_path));
    let project = state.projects.entry(key.clone()).or_insert_with(|| PersistedProjectState {
        id: Some(uuid()),
        name: Some(key.clone()),
        project_root: Some(project_path.clone()),
        cwd: preferred_cwd.clone().or_else(|| Some(project_path.clone())),
        auto_route_replies: Some(false),
        route_approval_requests: Some(false),
        preferred_model_provider: None,
        configs: json!({}),
        agents: BTreeMap::new(),
        orchestrator_thread_id: None,
        thread_groups: Vec::new(),
        archived: Some(false),
        detached: Some(false),
        updated_at: Some(unix_now()),
        created_at: Some(unix_now()),
        extras: BTreeMap::new(),
    });

    project.agents.insert(
        thread_id.to_string(),
        PersistedAgentState {
            display_name: thread
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| thread.get("id").and_then(Value::as_str).map(str::to_string)),
            role: Some(role.clone()),
            project_root: Some(project_path.clone()),
            cwd: project.cwd.clone().or(preferred_cwd),
            approval_policy: payload.get("approvalPolicy").and_then(Value::as_str).map(str::to_string),
            sandbox_mode: payload.get("sandboxMode").and_then(Value::as_str).map(str::to_string),
            network_access: payload.get("networkAccess").and_then(Value::as_bool),
            model: payload.get("modelID").and_then(Value::as_str).map(str::to_string),
            reasoning_effort: payload.get("reasoningEffort").and_then(Value::as_str).map(str::to_string),
            issue_number: payload.get("issueNumber").and_then(Value::as_u64),
            pull_request_number: payload.get("pullRequestNumber").and_then(Value::as_u64),
            blocked_reason: payload.get("blockedReason").and_then(Value::as_str).map(str::to_string),
            unblock_when: payload.get("unblockWhen").and_then(Value::as_str).map(str::to_string),
            archived: Some(false),
            extras: BTreeMap::new(),
        },
    );
    if role == "orchestrator" {
        project.orchestrator_thread_id = Some(thread_id.to_string());
    }
    project.updated_at = Some(unix_now());
    state.updated_at = Some(unix_now());
    Ok(())
}

fn set_tracked_thread_display_name(state: &mut PersistedState, thread_id: &str, display_name: &str) {
    for project in state.projects.values_mut() {
        if let Some(agent) = project.agents.get_mut(thread_id) {
            agent.display_name = Some(display_name.to_string());
            project.updated_at = Some(unix_now());
            state.updated_at = Some(unix_now());
            break;
        }
    }
}

fn update_tracked_thread_session_config(
    state: &mut PersistedState,
    thread_id: &str,
    approval_policy: Option<&str>,
    sandbox_mode: Option<&str>,
    network_access: Option<bool>,
    cwd: Option<&str>,
) -> bool {
    for project in state.projects.values_mut() {
        if let Some(agent) = project.agents.get_mut(thread_id) {
            let mut changed = false;
            if let Some(approval_policy) = approval_policy {
                let next = Some(approval_policy.to_string());
                if agent.approval_policy != next {
                    agent.approval_policy = next;
                    changed = true;
                }
            }
            if let Some(sandbox_mode) = sandbox_mode {
                let next = Some(sandbox_mode.to_string());
                if agent.sandbox_mode != next {
                    agent.sandbox_mode = next;
                    changed = true;
                }
            }
            if agent.network_access != network_access {
                agent.network_access = network_access;
                changed = true;
            }
            if let Some(cwd) = cwd {
                let next = Some(cwd.to_string());
                if agent.cwd != next {
                    agent.cwd = next;
                    changed = true;
                }
            }
            if changed {
                project.updated_at = Some(unix_now());
                state.updated_at = Some(unix_now());
            }
            return changed;
        }
    }
    false
}

fn summarize_thread_payload(result: &Value) -> Value {
    let mut thread = result.get("thread").cloned().unwrap_or_else(|| result.clone());
    if let Some(object) = thread.as_object_mut() {
        object.insert("turns".to_string(), Value::Array(Vec::new()));
    }
    thread
}

fn synthesized_agents(
    state: &PersistedState,
    running_thread_ids: &[String],
    instance_id: &str,
) -> Vec<BridgeAgentSummary> {
    let mut agents = all_agent_records(state, running_thread_ids)
        .iter()
        .map(|record| summarize_scoped_agent_record(record, instance_id))
        .collect::<Vec<_>>();
    agents.sort_by(|lhs, rhs| lhs.display_name.cmp(&rhs.display_name));
    agents
}

fn all_agent_records(
    state: &PersistedState,
    running_thread_ids: &[String],
) -> Vec<crate::models::ScopedAgentRecord> {
    let running = running_thread_ids.iter().cloned().collect::<std::collections::BTreeSet<_>>();
    let mut records = Vec::new();
    for project in state.projects.values() {
        let project_root = project.project_root.clone().unwrap_or_default();
        if normalize_path(project_root.clone()).is_empty() {
            continue;
        }
        let cwd = project.cwd.clone().unwrap_or_else(|| project_root.clone());
        let mut saw_orchestrator = false;
        for (thread_id, agent) in &project.agents {
            let role = agent.role.clone().unwrap_or_else(|| "worker".to_string());
            let is_orchestrator =
                role == "orchestrator" || project.orchestrator_thread_id.as_deref() == Some(thread_id.as_str());
            if is_orchestrator {
                saw_orchestrator = true;
            }
            records.push(crate::models::ScopedAgentRecord {
                thread_id: thread_id.clone(),
                display_name: agent.display_name.clone(),
                project_path: agent.project_root.clone().unwrap_or_else(|| project_root.clone()),
                cwd: cwd.clone(),
                role: role.clone(),
                is_orchestrator,
                is_running: running.contains(thread_id),
                is_archived: agent.archived.unwrap_or(false),
                is_hidden: role == "hidden",
                updated_at: project.updated_at.unwrap_or_else(unix_now),
            });
        }
        if !saw_orchestrator
            && let Some(orchestrator_thread_id) = project
                .orchestrator_thread_id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        {
            records.push(crate::models::ScopedAgentRecord {
                thread_id: orchestrator_thread_id.to_string(),
                display_name: Some(
                    project
                        .name
                        .clone()
                        .map(|name| format!("{name} Orchestrator"))
                        .unwrap_or_else(|| orchestrator_thread_id.to_string()),
                ),
                project_path: project_root.clone(),
                cwd: cwd.clone(),
                role: "orchestrator".to_string(),
                is_orchestrator: true,
                is_running: running.contains(&orchestrator_thread_id.to_string()),
                is_archived: false,
                is_hidden: false,
                updated_at: project.updated_at.unwrap_or_else(unix_now),
            });
        }
    }
    records.sort_by(|lhs, rhs| lhs.thread_id.cmp(&rhs.thread_id));
    records
}

fn scoped_agent_context(
    records: &[crate::models::ScopedAgentRecord],
    sender_thread_id: &str,
    include_archived: bool,
) -> Result<ScopedContext> {
    let sender = records
        .iter()
        .find(|record| record.thread_id == sender_thread_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Thread `{sender_thread_id}` is not tracked by the bridge."))?;
    if sender.is_hidden {
        bail!("Sender thread `{sender_thread_id}` is hidden from orchestrator communication.");
    }
    let mut visible = records
        .iter()
        .filter(|record| can_view_scoped_agent(record, &sender, include_archived))
        .cloned()
        .collect::<Vec<_>>();
    visible.sort_by(sort_scoped_agent_records);
    Ok(ScopedContext { sender, visible })
}

fn has_cross_project_communication_scope(record: &crate::models::ScopedAgentRecord) -> bool {
    record.is_orchestrator || record.role == "operator"
}

fn can_view_scoped_agent(
    record: &crate::models::ScopedAgentRecord,
    sender: &crate::models::ScopedAgentRecord,
    include_archived: bool,
) -> bool {
    if record.is_hidden {
        return false;
    }
    if record.is_archived && !include_archived {
        return has_cross_project_communication_scope(sender)
            && has_cross_project_communication_scope(record)
            && record.project_path != sender.project_path;
    }
    if has_cross_project_communication_scope(sender) {
        if record.project_path == sender.project_path {
            return true;
        }
        return has_cross_project_communication_scope(record);
    }
    record.project_path == sender.project_path
}

fn sort_scoped_agent_records(
    lhs: &crate::models::ScopedAgentRecord,
    rhs: &crate::models::ScopedAgentRecord,
) -> std::cmp::Ordering {
    match (lhs.is_running, rhs.is_running) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => rhs
            .updated_at
            .cmp(&lhs.updated_at)
            .then_with(|| lhs.thread_id.cmp(&rhs.thread_id)),
    }
}

async fn make_instance_summary(runtime: &BridgeRuntime) -> Value {
    json!({
        "id": runtime.settings().project_path.display().to_string(),
        "projectPath": runtime.settings().project_path.display().to_string(),
        "cwd": runtime.settings().cwd.display().to_string(),
        "isRunning": runtime.info().await.connection_status == "connected",
        "experimentalAPIEnabled": true
    })
}

fn effective_role_for_thread(state: &PersistedState, thread_id: &str, fallback: Option<&str>) -> Option<String> {
    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id)
            && let Some(role) = agent.role.clone()
        {
            return Some(role);
        }
        if project.orchestrator_thread_id.as_deref() == Some(thread_id) {
            return Some("orchestrator".to_string());
        }
    }
    fallback.map(str::to_string)
}

fn tracked_project_path_for_thread(state: &PersistedState, thread_id: &str) -> Option<String> {
    state.projects.values().find_map(|project| {
        if project.agents.contains_key(thread_id) || project.orchestrator_thread_id.as_deref() == Some(thread_id) {
            project.project_root.clone()
        } else {
            None
        }
    })
}

fn sender_thread_context(state: &PersistedState, thread_id: &str) -> Option<SenderThreadContext> {
    let default_approval_policy = state
        .global_configs
        .get("approvalPolicy")
        .and_then(Value::as_str)
        .map(str::to_string);
    let default_sandbox_mode = state
        .global_configs
        .get("sandboxMode")
        .and_then(Value::as_str)
        .map(str::to_string);
    let default_network_access = state
        .global_configs
        .get("networkAccess")
        .and_then(Value::as_bool);

    for project in state.projects.values() {
        let agent = project.agents.get(thread_id);
        if agent.is_none() && project.orchestrator_thread_id.as_deref() != Some(thread_id) {
            continue;
        }
        let project_path = normalize_path(project.project_root.clone().unwrap_or_default());
        let project_cwd = normalize_path(project.cwd.clone().unwrap_or_else(|| project_path.clone()));
        let cwd = normalize_path(
            agent
                .and_then(|agent| agent.cwd.clone())
                .unwrap_or_else(|| project_cwd.clone()),
        );
        let sandbox_mode = agent
            .and_then(|agent| agent.sandbox_mode.clone())
            .or(default_sandbox_mode.clone());
        let network_access = effective_network_access_for_sandbox(
            sandbox_mode.as_deref(),
            agent.and_then(|agent| agent.network_access),
            default_network_access,
        );
        return Some(SenderThreadContext {
            project_path,
            project_cwd,
            cwd,
            approval_policy: agent
                .and_then(|agent| agent.approval_policy.clone())
                .or(default_approval_policy.clone()),
            sandbox_mode,
            network_access,
            model: agent.and_then(|agent| agent.model.clone()),
            reasoning_effort: agent.and_then(|agent| agent.reasoning_effort.clone()),
        });
    }
    None
}

fn effective_network_access_for_sandbox(
    sandbox_mode: Option<&str>,
    explicit_network_access: Option<bool>,
    default_network_access: Option<bool>,
) -> Option<bool> {
    match sandbox_mode {
        Some("workspace-write") | Some("external-sandbox") => {
            if explicit_network_access == Some(true) || default_network_access == Some(true) {
                Some(true)
            } else if explicit_network_access == Some(false) || default_network_access == Some(false) {
                Some(false)
            } else {
                Some(true)
            }
        }
        _ => None,
    }
}

fn sandbox_policy_for_spawn(
    sandbox_mode: Option<&str>,
    network_access: Option<bool>,
    cwd: Option<&str>,
) -> Option<Value> {
    match sandbox_mode {
        Some("danger-full-access") => Some(json!({ "type": "dangerFullAccess" })),
        Some("read-only") => Some(json!({
            "type": "readOnly",
            "access": { "type": "fullAccess" },
            "networkAccess": network_access.unwrap_or(false),
        })),
        Some("workspace-write") => Some(json!({
            "type": "workspaceWrite",
            "writableRoots": cwd.map(|value| vec![value]).unwrap_or_default(),
            "readOnlyAccess": { "type": "fullAccess" },
            "networkAccess": network_access.unwrap_or(true),
            "excludeTmpdirEnvVar": false,
            "excludeSlashTmp": false,
        })),
        Some("external-sandbox") => Some(json!({
            "type": "externalSandbox",
            "networkAccess": if network_access.unwrap_or(true) { "enabled" } else { "restricted" },
        })),
        _ => None,
    }
}

fn role_default_model(state: &PersistedState, project_path: Option<&str>, role: Option<&str>) -> Option<String> {
    let key = match role {
        Some("qa") => "qa",
        Some("orchestrator") => "orchestrator",
        Some("worker") | Some("hidden") | Some("operator") | _ => "worker",
    };
    let project = state.projects.values().find(|project| project.project_root.as_deref() == project_path)?;
    project
        .configs
        .get("roleModelReasoningDefaults")
        .and_then(|value| value.get(key))
        .and_then(|value| value.get("modelID"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn role_default_reasoning_effort(
    state: &PersistedState,
    project_path: Option<&str>,
    role: Option<&str>,
) -> Option<String> {
    let key = match role {
        Some("qa") => "qa",
        Some("orchestrator") => "orchestrator",
        Some("worker") | Some("hidden") | Some("operator") | _ => "worker",
    };
    let project = state
        .projects
        .values()
        .find(|project| project.project_root.as_deref() == project_path)?;
    project
        .configs
        .get("roleModelReasoningDefaults")
        .and_then(|value| value.get(key))
        .and_then(|value| value.get("reasoningEffort"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn developer_instructions_for_role(
    state: &PersistedState,
    role: Option<&str>,
    project_path: Option<&str>,
    cwd: Option<&str>,
) -> Option<String> {
    let role = role?;
    if matches!(role, "hidden" | "orchestrator" | "operator") {
        return None;
    }
    let project = state.projects.values().find(|project| {
        project.project_root.as_deref() == project_path || project.project_root.as_deref() == cwd
    })?;
    if project.orchestrator_thread_id.is_none() {
        return None;
    }
    let mut guidance = Vec::new();
    if project.auto_route_replies.unwrap_or(false) {
        guidance.push("Final assistant replies are auto-forwarded to this project's orchestrator. Mid-turn messages and coordination are fine, but do not manually send a redundant final handoff when your turn ends unless you need to add distinct information.");
    } else {
        guidance.push("Final assistant replies are not auto-forwarded. If the orchestrator needs your final status, use $robdex-orchestrator to send it manually.");
    }
    if project.route_approval_requests.unwrap_or(false) {
        guidance.push("Command and file-change approval requests are forwarded to this project's orchestrator so they can guide approval decisions in real time.");
    }
    Some(guidance.join(" "))
}

fn resolve_role_instructions_for(role: Option<&str>) -> Result<Option<String>> {
    match role {
        None | Some("operator") => Ok(None),
        Some(value) => {
            let home = env::var_os("HOME").map(PathBuf::from);
            resolve_role_instructions(home, Some(value))
        }
    }
}

fn required_string(payload: &Value, field: &str) -> Result<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Missing {field}"))
}

fn required_request_id(payload: &Value, field: &str) -> Result<RequestId> {
    let value = payload
        .get(field)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Missing {field}"))?;
    Ok(serde_json::from_value(value)?)
}

fn sender_display_name_for_thread(state: &PersistedState, thread_id: Option<&str>) -> Option<String> {
    let thread_id = thread_id?;
    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id) {
            return agent
                .display_name
                .clone()
                .or_else(|| Some(thread_id.to_string()));
        }
        if project.orchestrator_thread_id.as_deref() == Some(thread_id) {
            return project
                .agents
                .get(thread_id)
                .and_then(|agent| agent.display_name.clone())
                .or_else(|| Some(thread_id.to_string()));
        }
    }
    Some(thread_id.to_string())
}

fn normalized_agent_input_text(text: &str, sender_identity: Option<String>) -> String {
    let trimmed = text.trim();
    match sender_identity {
        Some(sender) if !sender.trim().is_empty() => format!("[{}] {}", sender.trim(), trimmed),
        _ => trimmed.to_string(),
    }
}

async fn send_thread_input(
    runtime: &BridgeRuntime,
    state: &PersistedState,
    thread_id: &str,
    text: &str,
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<Value> {
    if let Some(active_turn_id) = runtime.active_turn_id_for_thread(thread_id).await {
        let steer_result = app_server_request_json(
            runtime,
            "turn/steer",
            json!({
                "threadId": thread_id,
                "input": [{"type":"text","text": text}],
                "expectedTurnId": active_turn_id,
            }),
        )
        .await;
        if let Ok(result) = steer_result {
            if steer_result_acknowledged(&result, &active_turn_id) {
                return Ok(json!({
                    "id": active_turn_id,
                    "items": [],
                    "status": "inProgress",
                    "error": null,
                }));
            }
        }
    }

    let cwd = tracked_cwd_for_thread(state, thread_id);
    let approval_policy = tracked_approval_policy_for_thread(state, thread_id);
    let sandbox_policy = tracked_sandbox_policy_for_thread(state, thread_id);
    let effective_model = model
        .map(str::to_string)
        .or_else(|| tracked_model_for_thread(state, thread_id));
    let effective_effort = effort
        .map(str::to_string)
        .or_else(|| tracked_reasoning_effort_for_thread(state, thread_id));
    let response = app_server_request_json(
        runtime,
        "turn/start",
        json!({
            "threadId": thread_id,
            "input": [{"type":"text","text": text}],
            "cwd": cwd,
            "approvalPolicy": approval_policy,
            "sandboxPolicy": sandbox_policy,
            "model": effective_model,
            "effort": effective_effort,
        }),
    )
    .await?;
    Ok(response.get("turn").cloned().unwrap_or(response))
}

fn steer_result_acknowledged(result: &Value, active_turn_id: &str) -> bool {
    if result.is_null() {
        return false;
    }
    if result
        .get("turnId")
        .and_then(Value::as_str)
        .map(|value| value == active_turn_id)
        .unwrap_or(false)
    {
        return true;
    }
    if result
        .get("id")
        .and_then(Value::as_str)
        .map(|value| value == active_turn_id)
        .unwrap_or(false)
    {
        return true;
    }
    if result
        .get("status")
        .and_then(Value::as_str)
        .map(|value| matches!(value, "inProgress" | "in_progress"))
        .unwrap_or(false)
    {
        return true;
    }
    result
        .get("accepted")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn tracked_cwd_for_thread(state: &PersistedState, thread_id: &str) -> Option<String> {
    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id) {
            return agent.cwd.clone().or_else(|| project.cwd.clone()).or_else(|| project.project_root.clone());
        }
        if project.orchestrator_thread_id.as_deref() == Some(thread_id) {
            return project.cwd.clone().or_else(|| project.project_root.clone());
        }
    }
    None
}

fn tracked_approval_policy_for_thread(state: &PersistedState, thread_id: &str) -> Option<String> {
    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id) {
            return agent
                .approval_policy
                .clone()
                .or_else(|| {
                    state
                        .global_configs
                        .get("approvalPolicy")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
        }
    }
    None
}

fn tracked_sandbox_mode_for_thread(state: &PersistedState, thread_id: &str) -> Option<String> {
    let default_sandbox_mode = state
        .global_configs
        .get("sandboxMode")
        .and_then(Value::as_str)
        .map(str::to_string);
    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id) {
            return agent.sandbox_mode.clone().or_else(|| default_sandbox_mode.clone());
        }
    }
    default_sandbox_mode
}

fn tracked_network_access_for_thread(state: &PersistedState, thread_id: &str) -> Option<bool> {
    let default_network_access = state.global_configs.get("networkAccess").and_then(Value::as_bool);
    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id) {
            let sandbox_mode = agent
                .sandbox_mode
                .clone()
                .or_else(|| {
                    state
                        .global_configs
                        .get("sandboxMode")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            return effective_network_access_for_sandbox(
                sandbox_mode.as_deref(),
                agent.network_access,
                default_network_access,
            );
        }
    }
    default_network_access
}

fn tracked_model_for_thread(state: &PersistedState, thread_id: &str) -> Option<String> {
    let role = effective_role_for_thread(state, thread_id, None);
    let project_path = tracked_project_path_for_thread(state, thread_id);
    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id) {
            return agent
                .model
                .clone()
                .or_else(|| role_default_model(state, project_path.as_deref(), role.as_deref()));
        }
    }
    role_default_model(state, project_path.as_deref(), role.as_deref())
}

fn tracked_reasoning_effort_for_thread(state: &PersistedState, thread_id: &str) -> Option<String> {
    let role = effective_role_for_thread(state, thread_id, None);
    let project_path = tracked_project_path_for_thread(state, thread_id);
    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id) {
            return agent.reasoning_effort.clone().or_else(|| {
                role_default_reasoning_effort(state, project_path.as_deref(), role.as_deref())
            });
        }
    }
    role_default_reasoning_effort(state, project_path.as_deref(), role.as_deref())
}

fn tracked_sandbox_policy_for_thread(state: &PersistedState, thread_id: &str) -> Option<Value> {
    let default_sandbox_mode = state
        .global_configs
        .get("sandboxMode")
        .and_then(Value::as_str)
        .map(str::to_string);
    let default_network_access = state.global_configs.get("networkAccess").and_then(Value::as_bool);

    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id) {
            let sandbox_mode = agent
                .sandbox_mode
                .clone()
                .or_else(|| default_sandbox_mode.clone());
            let network_access = effective_network_access_for_sandbox(
                sandbox_mode.as_deref(),
                agent.network_access,
                default_network_access,
            );
            let cwd = agent
                .cwd
                .clone()
                .or_else(|| project.cwd.clone())
                .or_else(|| project.project_root.clone());
            return sandbox_policy_for_spawn(sandbox_mode.as_deref(), network_access, cwd.as_deref());
        }
    }
    None
}

async fn send_follow_up_message(
    runtime: &BridgeRuntime,
    approval: &PendingApproval,
    message: &str,
) -> Result<()> {
    let active_turn_id = runtime
        .active_turn_id_for_thread(&approval.thread_id)
        .await
        .unwrap_or_else(|| approval.turn_id.clone());
    app_server_request_json(
        runtime,
        "turn/steer",
        json!({
            "threadId": approval.thread_id,
            "input": [{"type":"text","text": message}],
            "expectedTurnId": active_turn_id,
        }),
    )
    .await?;
    Ok(())
}

async fn spawn_agent(runtime: &BridgeRuntime, payload: &Value) -> Result<BridgeAgentSummary> {
    let _state_guard = runtime.lock_state_mutation().await;
    let mut state = parse_state(&runtime.state_document_value().await);
    let role = payload.get("role").and_then(Value::as_str);
    let project_path = payload
        .get("projectPath")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| runtime.settings().project_path.display().to_string());
    let cwd = payload
        .get("cwd")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| runtime.settings().cwd.display().to_string());
    let display_name = payload.get("displayName").and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty());
    let approval_policy = payload
        .get("approvalPolicy")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            state
                .global_configs
                .get("approvalPolicy")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let sandbox_mode = payload
        .get("sandboxMode")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            state
                .global_configs
                .get("sandboxMode")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let default_network_access = state.global_configs.get("networkAccess").and_then(Value::as_bool);
    let network_access = effective_network_access_for_sandbox(
        sandbox_mode.as_deref(),
        payload.get("networkAccess").and_then(Value::as_bool),
        default_network_access,
    );
    let model = payload
        .get("modelID")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| role_default_model(&state, Some(project_path.as_str()), role));
    let reasoning_effort = payload
        .get("reasoningEffort")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| role_default_reasoning_effort(&state, Some(project_path.as_str()), role));
    let params = json!({
        "cwd": cwd,
        "approvalPolicy": approval_policy,
        "sandbox": sandbox_mode,
        "sandboxPolicy": sandbox_policy_for_spawn(sandbox_mode.as_deref(), network_access, Some(cwd.as_str())),
        "model": model,
        "effort": reasoning_effort,
        "developerInstructions": developer_instructions_for_role(&state, role, Some(project_path.as_str()), Some(cwd.as_str())),
        "baseInstructions": resolve_role_instructions_for(role)?,
        "persistExtendedHistory": true,
    });
    let result = app_server_request_json(runtime, "thread/start", params).await?;
    let thread = result
        .get("thread")
        .cloned()
        .unwrap_or(result.clone());
    let thread_id = thread
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("thread/start response missing thread.id"))?
        .to_string();
    if let Some(display_name) = display_name {
        app_server_request_json(
            runtime,
            "thread/name/set",
            json!({
                "threadId": thread_id,
                "name": display_name,
            }),
        )
        .await?;
    }
    register_tracked_thread(
        &mut state,
        &json!({
            "thread": thread,
            "projectPath": project_path,
            "preferredCWD": cwd,
            "role": role.unwrap_or("worker"),
            "approvalPolicy": approval_policy,
            "sandboxMode": sandbox_mode,
            "networkAccess": network_access,
            "modelID": model,
            "reasoningEffort": reasoning_effort,
        }),
    )?;
    if let Some(display_name) = display_name {
        set_tracked_thread_display_name(&mut state, &thread_id, display_name);
    }
    persist_state(runtime, &state).await?;

    if let Some(initial_prompt) = payload.get("initialPrompt").and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()) {
        let _ = send_thread_input(runtime, &state, &thread_id, initial_prompt, None, None).await?;
    }

    let mut agent = synthesized_agent_for_thread(
        &state,
        &runtime.snapshot().await?.thread_cache.running_thread_ids,
        &runtime.settings().project_path.display().to_string(),
        &thread_id,
    )
    .ok_or_else(|| anyhow::anyhow!("spawned thread was not registered"))?;
    agent.parent_agent_id = payload.get("parentAgentId").and_then(Value::as_str).map(str::to_string);
    Ok(agent)
}

async fn create_thread(runtime: &BridgeRuntime, payload: &Value) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let (_, project) = selected_or_requested_project(&state, payload.get("projectId").and_then(Value::as_str))?;
    let project_path = project
        .project_root
        .clone()
        .ok_or_else(|| anyhow::anyhow!("project root missing"))?;
    let cwd = project
        .cwd
        .clone()
        .unwrap_or_else(|| project_path.clone());
    let role = payload
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("worker");
    let spawn_payload = json!({
        "displayName": payload.get("title").and_then(Value::as_str),
        "initialPrompt": payload.get("initialPrompt").and_then(Value::as_str),
        "cwd": cwd,
        "projectPath": project_path,
        "role": role,
        "approvalPolicy": payload.get("approvalPolicy").cloned().unwrap_or(Value::Null),
        "sandboxMode": payload.get("sandboxMode").cloned().unwrap_or(Value::Null),
        "networkAccess": payload.get("networkAccess").cloned().unwrap_or(Value::Null),
        "modelID": payload.get("modelID").cloned().unwrap_or(Value::Null),
        "reasoningEffort": payload.get("reasoningEffort").cloned().unwrap_or(Value::Null),
    });
    let agent = spawn_agent(runtime, &spawn_payload).await?;
    Ok(json!({
        "threadId": agent.id,
        "displayName": agent.display_name,
        "role": agent.role,
        "projectPath": agent.project_path,
        "cwd": agent.cwd,
    }))
}

async fn create_thread_message(runtime: &BridgeRuntime, payload: &Value) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let thread_id = required_string(payload, "threadId")?;
    let text = required_string(payload, "text")?;
    let result = send_thread_input(
        runtime,
        &state,
        &thread_id,
        &text,
        payload.get("modelID").and_then(Value::as_str),
        payload.get("reasoningEffort").and_then(Value::as_str),
    )
    .await?;
    runtime.append_local_user_message(&thread_id, &text).await?;
    Ok(json!({
        "threadId": thread_id,
        "turn": result,
    }))
}

async fn wait_for_agent(runtime: &BridgeRuntime, payload: &Value) -> Result<BridgeAgentSummary> {
    let agent_id = required_string(payload, "agentId")?;
    let timeout_seconds = payload.get("timeoutSeconds").and_then(Value::as_u64).unwrap_or(120);
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    loop {
        let state = parse_state(&runtime.state_document_value().await);
        let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
        let agent = synthesized_agent_for_thread(
            &state,
            &running,
            &runtime.settings().project_path.display().to_string(),
            &agent_id,
        )
        .ok_or_else(|| anyhow::anyhow!("Unknown agent `{agent_id}`"))?;
        if agent.status != "running" {
            return Ok(agent);
        }
        if Instant::now() >= deadline {
            return Ok(agent);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn close_agent(runtime: &BridgeRuntime, payload: &Value) -> Result<BridgeAgentSummary> {
    let thread_id = required_string(payload, "agentId")?;
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let mut agent = synthesized_agent_for_thread(
        &state,
        &running,
        &runtime.settings().project_path.display().to_string(),
        &thread_id,
    )
    .ok_or_else(|| anyhow::anyhow!("Unknown agent `{thread_id}`"))?;
    archive_thread(runtime, &thread_id).await?;
    agent.status = "closed".to_string();
    agent.last_event = Some("Closed".to_string());
    Ok(agent)
}

fn synthesized_agent_for_thread(
    state: &PersistedState,
    running_thread_ids: &[String],
    instance_id: &str,
    thread_id: &str,
) -> Option<BridgeAgentSummary> {
    all_agent_records(state, running_thread_ids)
        .iter()
        .find(|record| record.thread_id == thread_id)
        .map(|record| summarize_scoped_agent_record(record, instance_id))
}

fn remove_tracked_thread(state: &mut PersistedState, thread_id: &str) {
    for project in state.projects.values_mut() {
        if project.agents.remove(thread_id).is_some() {
            if project.orchestrator_thread_id.as_deref() == Some(thread_id) {
                project.orchestrator_thread_id = None;
            }
            project.updated_at = Some(unix_now());
        }
    }
    state.updated_at = Some(unix_now());
}

fn resolve_scoped_recipient(
    visible: &[crate::models::ScopedAgentRecord],
    recipient_thread_id: Option<&str>,
    recipient_name: Option<&str>,
    project_path: Option<&str>,
) -> Result<crate::models::ScopedAgentRecord> {
    if let Some(thread_id) = recipient_thread_id.map(str::trim).filter(|value| !value.is_empty()) {
        return visible
            .iter()
            .find(|record| record.thread_id == thread_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Thread `{thread_id}` is not visible to the sender."));
    }
    if let Some(name) = recipient_name.map(str::trim).filter(|value| !value.is_empty()) {
        let normalized_project = project_path.map(|value| normalize_path(value.to_string()));
        let mut matches = visible
            .iter()
            .filter(|record| {
                record
                    .display_name
                    .as_deref()
                    .map(|display| display.eq_ignore_ascii_case(name))
                    .unwrap_or(false)
            })
            .filter(|record| {
                normalized_project
                    .as_deref()
                    .map(|project| record.project_path == *project)
                    .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            return Ok(matches.remove(0));
        }
        if matches.is_empty() {
            bail!("No visible thread named `{name}`.");
        }
        bail!("Multiple visible threads named `{name}`.");
    }
    bail!("Provide recipientThreadId or recipientName.")
}

fn normalize_path(value: String) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    std::fs::canonicalize(trimmed)
        .unwrap_or_else(|_| PathBuf::from(trimmed))
        .to_string_lossy()
        .to_string()
}

fn best_matching_project_path(requested: &str, candidates: &[String]) -> Option<String> {
    let mut matches = candidates
        .iter()
        .filter(|candidate| !candidate.is_empty() && requested.starts_with(candidate.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    matches.sort_by_key(|candidate| std::cmp::Reverse(candidate.len()));
    matches.into_iter().next()
}

fn agent_state_for_thread<'a>(state: &'a PersistedState, thread_id: &str) -> Option<&'a PersistedAgentState> {
    state
        .projects
        .values()
        .find_map(|project| project.agents.get(thread_id))
}

fn worker_issue_number(state: &PersistedState, thread_id: &str) -> Option<u64> {
    agent_state_for_thread(state, thread_id).and_then(|agent| agent.issue_number)
}

fn worker_pr_number(state: &PersistedState, thread_id: &str) -> Option<u64> {
    agent_state_for_thread(state, thread_id).and_then(|agent| agent.pull_request_number)
}

fn worker_blocked_reason(state: &PersistedState, thread_id: &str) -> Option<String> {
    agent_state_for_thread(state, thread_id).and_then(|agent| agent.blocked_reason.clone())
}

fn worker_unblock_when(state: &PersistedState, thread_id: &str) -> Option<String> {
    agent_state_for_thread(state, thread_id).and_then(|agent| agent.unblock_when.clone())
}

fn workspace_entries(runtime: &BridgeRuntime, relative_path: Option<&str>) -> Result<Vec<Value>> {
    let root = runtime.settings().cwd.clone();
    let path = resolve_workspace_path(&root, relative_path.unwrap_or(""))?;
    let mut entries = std::fs::read_dir(&path)?
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let file_type = entry.file_type().ok();
            let metadata = entry.metadata().ok();
            let relative = entry
                .path()
                .strip_prefix(&root)
                .ok()
                .map(|value| value.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|| entry.file_name().to_string_lossy().to_string());
            json!({
                "relativePath": relative,
                "name": entry.file_name().to_string_lossy().to_string(),
                "isDirectory": file_type.map(|value| value.is_dir()).unwrap_or(false),
                "size": metadata.map(|value| value.len()),
                "modifiedAt": Value::Null,
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|lhs, rhs| {
        lhs.get("relativePath")
            .and_then(Value::as_str)
            .cmp(&rhs.get("relativePath").and_then(Value::as_str))
    });
    Ok(entries)
}

fn workspace_read_file(runtime: &BridgeRuntime, relative_path: &str, max_bytes: Option<u64>) -> Result<Value> {
    let root = runtime.settings().cwd.clone();
    let path = resolve_workspace_path(&root, relative_path)?;
    let bytes = std::fs::read(&path)?;
    let cap = max_bytes.unwrap_or(160_000) as usize;
    let truncated = bytes.len() > cap;
    let content = String::from_utf8_lossy(if truncated { &bytes[..cap] } else { &bytes }).to_string();
    Ok(json!({
        "relativePath": relative_path,
        "content": content,
        "isTruncated": truncated,
        "totalBytes": bytes.len(),
    }))
}

fn resolve_workspace_path(root: &Path, relative_path: &str) -> Result<PathBuf> {
    let canonical_root = root.canonicalize()?;
    let path = if relative_path.trim().is_empty() {
        canonical_root.clone()
    } else {
        canonical_root.join(relative_path)
    };
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(&canonical_root) {
        bail!("Workspace path escapes root");
    }
    Ok(canonical)
}

pub async fn orchestrator_whoami(runtime: &BridgeRuntime, sender_thread_id: &str) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let sender = records
        .iter()
        .find(|record| record.thread_id == sender_thread_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Thread `{sender_thread_id}` is not tracked by the bridge."))?;
    Ok(json!({
        "threadId": sender.thread_id,
        "displayName": sender.display_name,
        "projectPath": normalize_path(sender.project_path),
        "cwd": normalize_path(sender.cwd),
        "role": if sender.is_hidden {
            "hidden".to_string()
        } else if sender.is_orchestrator {
            "orchestrator".to_string()
        } else {
            sender.role.clone()
        },
    }))
}

pub async fn orchestrator_lookup(runtime: &BridgeRuntime, raw_path: &str) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let requested = normalize_path(raw_path.to_string());
    let mut candidates = records
        .iter()
        .map(|record| record.project_path.clone())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    let matched = best_matching_project_path(&requested, &candidates);
    let orchestrator = matched.as_deref().and_then(|project_path| {
        records
            .iter()
            .find(|record| record.project_path == project_path && record.is_orchestrator)
    });
    Ok(json!({
        "requestedPath": requested,
        "projectPath": matched,
        "orchestratorThreadId": orchestrator.map(|record| record.thread_id.clone()),
        "orchestratorDisplayName": orchestrator.and_then(|record| record.display_name.clone()),
    }))
}

pub async fn orchestrator_threads(
    runtime: &BridgeRuntime,
    requested_path: Option<&str>,
    include_archived: bool,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let mut records = all_agent_records(&state, &running);
    if let Some(requested_path) = requested_path.filter(|value| !value.trim().is_empty()) {
        let requested = normalize_path(requested_path.to_string());
        let candidates = records.iter().map(|record| record.project_path.clone()).collect::<Vec<_>>();
        if let Some(project_path) = best_matching_project_path(&requested, &candidates) {
            records.retain(|record| record.project_path == project_path);
        }
    }
    records.retain(|record| include_archived || !record.is_archived);
    records.sort_by(sort_scoped_agent_records);
    Ok(json!({
        "items": records.into_iter().map(|record| json!({
            "id": record.thread_id,
            "displayName": record.display_name,
            "projectPath": record.project_path,
            "cwd": record.cwd,
            "isOrchestrator": record.is_orchestrator,
            "isRunning": record.is_running,
            "issueNumber": worker_issue_number(&state, &record.thread_id),
            "pullRequestNumber": worker_pr_number(&state, &record.thread_id),
            "blockedReason": worker_blocked_reason(&state, &record.thread_id),
            "unblockWhen": worker_unblock_when(&state, &record.thread_id),
            "updatedAt": record.updated_at,
        })).collect::<Vec<_>>()
    }))
}

fn resolve_scoped_project_key<'a>(
    state: &'a PersistedState,
    sender: &crate::models::ScopedAgentRecord,
    project_path: Option<&str>,
) -> Result<String> {
    let requested = project_path
        .map(|value| normalize_path(value.to_string()))
        .unwrap_or_else(|| sender.project_path.clone());
    state
        .projects
        .iter()
        .find(|(_, project)| normalize_path(project.project_root.clone().unwrap_or_default()) == requested)
        .map(|(key, _)| key.clone())
        .ok_or_else(|| anyhow::anyhow!("Unknown project path `{requested}`."))
}

fn project_group_items(project: &PersistedProjectState) -> Vec<Value> {
    project
        .thread_groups
        .iter()
        .map(|group| {
            json!({
                "id": group.id,
                "title": group.title,
                "threadIDs": group.thread_ids,
                "isCollapsed": group.is_collapsed,
                "createdAt": group.created_at,
                "updatedAt": group.updated_at,
            })
        })
        .collect()
}

pub async fn orchestrator_thread_groups(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    project_path: Option<&str>,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    let project_key = resolve_scoped_project_key(&state, &scoped.sender, project_path)?;
    let project = state
        .projects
        .get(&project_key)
        .ok_or_else(|| anyhow::anyhow!("Unknown project `{project_key}`."))?;
    Ok(json!({
        "projectPath": project.project_root,
        "items": project_group_items(project),
    }))
}

pub async fn orchestrator_thread_group_create(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    project_path: Option<&str>,
    title: &str,
    seed_thread_id: Option<&str>,
) -> Result<Value> {
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    if !scoped.sender.is_orchestrator {
        bail!("Only orchestrator threads can manage thread groups.");
    }
    let project_key = resolve_scoped_project_key(&state, &scoped.sender, project_path)?;
    let seed = seed_thread_id.map(str::trim).filter(|value| !value.is_empty());
    if let Some(seed_thread_id) = seed
        && !scoped
            .visible
            .iter()
            .any(|record| record.thread_id == seed_thread_id && record.project_path == scoped.sender.project_path)
    {
        bail!("Thread `{seed_thread_id}` is not visible in the selected project.");
    }
    let project = state
        .projects
        .get_mut(&project_key)
        .ok_or_else(|| anyhow::anyhow!("Unknown project `{project_key}`."))?;
    let group = ThreadGroupState {
        id: uuid(),
        title: title.trim().to_string(),
        thread_ids: seed.map(|value| vec![value.to_string()]).unwrap_or_default(),
        is_collapsed: false,
        created_at: Some(unix_now()),
        updated_at: Some(unix_now()),
    };
    let changed_group_id = group.id.clone();
    project.thread_groups.push(group);
    project.updated_at = Some(unix_now());
    state.updated_at = Some(unix_now());
    persist_state(runtime, &state).await?;
    let project = state.projects.get(&project_key).expect("project exists");
    Ok(json!({
        "projectPath": project.project_root,
        "changedGroupId": changed_group_id,
        "items": project_group_items(project),
    }))
}

pub async fn orchestrator_thread_group_update(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    project_path: Option<&str>,
    group_id: &str,
    title: Option<&str>,
    collapsed: Option<bool>,
) -> Result<Value> {
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    if !scoped.sender.is_orchestrator {
        bail!("Only orchestrator threads can manage thread groups.");
    }
    let project_key = resolve_scoped_project_key(&state, &scoped.sender, project_path)?;
    let project = state
        .projects
        .get_mut(&project_key)
        .ok_or_else(|| anyhow::anyhow!("Unknown project `{project_key}`."))?;
    let group = project
        .thread_groups
        .iter_mut()
        .find(|group| group.id == group_id)
        .ok_or_else(|| anyhow::anyhow!("Unknown thread group `{group_id}`."))?;
    if let Some(title) = title.map(str::trim).filter(|value| !value.is_empty()) {
        group.title = title.to_string();
    }
    if let Some(collapsed) = collapsed {
        group.is_collapsed = collapsed;
    }
    group.updated_at = Some(unix_now());
    project.updated_at = Some(unix_now());
    state.updated_at = Some(unix_now());
    persist_state(runtime, &state).await?;
    let project = state.projects.get(&project_key).expect("project exists");
    Ok(json!({
        "projectPath": project.project_root,
        "changedGroupId": group_id,
        "items": project_group_items(project),
    }))
}

pub async fn orchestrator_thread_group_move_thread(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    project_path: Option<&str>,
    thread_id: &str,
    target_group_id: Option<&str>,
) -> Result<Value> {
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    if !scoped.sender.is_orchestrator {
        bail!("Only orchestrator threads can manage thread groups.");
    }
    if !scoped
        .visible
        .iter()
        .any(|record| record.thread_id == thread_id && record.project_path == scoped.sender.project_path)
    {
        bail!("Thread `{thread_id}` is not visible in the selected project.");
    }
    let project_key = resolve_scoped_project_key(&state, &scoped.sender, project_path)?;
    let project = state
        .projects
        .get_mut(&project_key)
        .ok_or_else(|| anyhow::anyhow!("Unknown project `{project_key}`."))?;
    for group in &mut project.thread_groups {
        group.thread_ids.retain(|value| value != thread_id);
    }
    let mut changed_group_id = None::<String>;
    if let Some(target_group_id) = target_group_id.map(str::trim).filter(|value| !value.is_empty()) {
        let target = project
            .thread_groups
            .iter_mut()
            .find(|group| group.id == target_group_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown thread group `{target_group_id}`."))?;
        target.thread_ids.push(thread_id.to_string());
        target.updated_at = Some(unix_now());
        changed_group_id = Some(target_group_id.to_string());
    }
    project.updated_at = Some(unix_now());
    state.updated_at = Some(unix_now());
    persist_state(runtime, &state).await?;
    let project = state.projects.get(&project_key).expect("project exists");
    Ok(json!({
        "projectPath": project.project_root,
        "changedGroupId": changed_group_id,
        "items": project_group_items(project),
    }))
}

pub async fn orchestrator_thread_group_delete(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    project_path: Option<&str>,
    group_id: &str,
) -> Result<Value> {
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    if !scoped.sender.is_orchestrator {
        bail!("Only orchestrator threads can manage thread groups.");
    }
    let project_key = resolve_scoped_project_key(&state, &scoped.sender, project_path)?;
    let project = state
        .projects
        .get_mut(&project_key)
        .ok_or_else(|| anyhow::anyhow!("Unknown project `{project_key}`."))?;
    let original_len = project.thread_groups.len();
    project.thread_groups.retain(|group| group.id != group_id);
    if project.thread_groups.len() == original_len {
        bail!("Unknown thread group `{group_id}`.");
    }
    project.updated_at = Some(unix_now());
    state.updated_at = Some(unix_now());
    persist_state(runtime, &state).await?;
    let project = state.projects.get(&project_key).expect("project exists");
    Ok(json!({
        "projectPath": project.project_root,
        "changedGroupId": group_id,
        "items": project_group_items(project),
    }))
}

pub async fn orchestrator_thread_group_archive(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    project_path: Option<&str>,
    group_id: &str,
) -> Result<Value> {
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    if !scoped.sender.is_orchestrator {
        bail!("Only orchestrator threads can manage thread groups.");
    }
    let project_key = resolve_scoped_project_key(&state, &scoped.sender, project_path)?;
    let project = state
        .projects
        .get_mut(&project_key)
        .ok_or_else(|| anyhow::anyhow!("Unknown project `{project_key}`."))?;
    let group = project
        .thread_groups
        .iter()
        .find(|group| group.id == group_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Unknown thread group `{group_id}`."))?;
    let mut archived_thread_ids = Vec::new();
    let mut skipped_thread_ids = Vec::new();
    for member_thread_id in &group.thread_ids {
        if member_thread_id == sender_thread_id {
            skipped_thread_ids.push(member_thread_id.clone());
            continue;
        }
        match project.agents.get_mut(member_thread_id) {
            Some(agent) => {
                if agent.archived.unwrap_or(false) {
                    skipped_thread_ids.push(member_thread_id.clone());
                } else {
                    agent.archived = Some(true);
                    archived_thread_ids.push(member_thread_id.clone());
                }
            }
            None => skipped_thread_ids.push(member_thread_id.clone()),
        }
    }
    project.updated_at = Some(unix_now());
    state.updated_at = Some(unix_now());
    let project_path_value = project.project_root.clone();
    let group_id_value = group.id.clone();
    let group_title = group.title.clone();
    persist_state(runtime, &state).await?;
    for archived_thread_id in &archived_thread_ids {
        let _ = app_server_request_json(
            runtime,
            "thread/archive",
            json!({"threadId": archived_thread_id}),
        )
        .await;
    }
    Ok(json!({
        "projectPath": project_path_value,
        "groupId": group_id_value,
        "title": group_title,
        "archivedThreadIds": archived_thread_ids,
        "skippedThreadIds": skipped_thread_ids,
    }))
}

pub async fn orchestrator_agents(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    include_archived: bool,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, include_archived)?;
    Ok(json!({
        "items": scoped.visible.into_iter().map(|record| json!({
            "id": record.thread_id,
            "displayName": record.display_name,
            "role": record.role,
            "projectPath": record.project_path,
            "cwd": record.cwd,
            "isOrchestrator": record.is_orchestrator,
            "isRunning": record.is_running,
            "issueNumber": worker_issue_number(&state, &record.thread_id),
            "pullRequestNumber": worker_pr_number(&state, &record.thread_id),
            "blockedReason": worker_blocked_reason(&state, &record.thread_id),
            "unblockWhen": worker_unblock_when(&state, &record.thread_id),
            "updatedAt": record.updated_at,
        })).collect::<Vec<_>>()
    }))
}

pub async fn orchestrator_pending_approvals(runtime: &BridgeRuntime, sender_thread_id: &str) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    let mut visible = scoped
        .visible
        .into_iter()
        .map(|record| record.thread_id)
        .collect::<std::collections::BTreeSet<_>>();
    visible.insert(sender_thread_id.to_string());
    let items = runtime
        .pending_approvals()
        .await
        .into_iter()
        .filter(|approval| visible.contains(&approval.thread_id))
        .map(|approval| {
            json!({
                "id": approval.id,
                "kind": approval.kind,
                "threadID": approval.thread_id,
                "title": approval.title,
                "command": approval.command,
                "commandCWD": approval.command_cwd,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "items": items }))
}

pub async fn orchestrator_send_message(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    recipient_thread_id: Option<&str>,
    recipient_name: Option<&str>,
    project_path: Option<&str>,
    text: &str,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    let sender = scoped.sender;
    let recipient = resolve_scoped_recipient(&scoped.visible, recipient_thread_id, recipient_name, project_path)?;
    let normalized_text = normalized_agent_input_text(text, sender.display_name.clone().or(Some(sender.thread_id.clone())));
    let result = send_thread_input(runtime, &state, &recipient.thread_id, &normalized_text, None, None).await?;
    Ok(json!({
        "recipientThreadId": recipient.thread_id,
        "recipientDisplayName": recipient.display_name,
        "turnId": result.get("id").cloned().unwrap_or(Value::Null),
    }))
}

pub async fn orchestrator_spawn_agent(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    name: &str,
    prompt: &str,
    _cwd: Option<&str>,
    role: Option<&str>,
    issue_number: Option<u64>,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let sender = records
        .iter()
        .find(|record| record.thread_id == sender_thread_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Thread `{sender_thread_id}` is not tracked by the bridge."))?;
    if !sender.is_orchestrator {
        bail!("Only orchestrator threads can spawn agents.");
    }
    let sender_context = sender_thread_context(&state, sender_thread_id)
        .ok_or_else(|| anyhow::anyhow!("Thread `{sender_thread_id}` has no persisted project context."))?;
    let payload = json!({
        "displayName": name,
        "initialPrompt": prompt,
        "cwd": sender_context.project_cwd,
        "projectPath": sender_context.project_path,
        "role": role.unwrap_or("worker"),
        "parentAgentId": sender_thread_id,
    });
    let mut agent = spawn_agent(runtime, &payload).await?;
    if let Some(issue_number) = issue_number {
        let mut state = parse_state(&runtime.state_document_value().await);
        if let Some(agent_state) = state
            .projects
            .values_mut()
            .find_map(|project| project.agents.get_mut(&agent.id))
        {
            agent_state.issue_number = Some(issue_number);
        }
        persist_state(runtime, &state).await?;
    }
    agent.parent_agent_id = Some(sender_thread_id.to_string());
    Ok(json!({ "agent": agent }))
}

pub async fn orchestrator_warm_handoff(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    recipient_thread_id: Option<&str>,
    recipient_name: Option<&str>,
    project_path: Option<&str>,
    prompt: &str,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    let sender = scoped.sender;
    let recipient =
        resolve_scoped_recipient(&scoped.visible, recipient_thread_id, recipient_name, project_path)?;
    if !sender.is_orchestrator || sender.project_path != recipient.project_path {
        bail!(
            "Only the configured orchestrator thread for project `{}` can warm handoff agents in that project.",
            recipient.project_path
        );
    }
    let mut recipient_state = None;
    let mut carried_group_ids = Vec::new();
    let mut project_root = None;
    let mut project_cwd = None;
    for project in state.projects.values() {
        if project.agents.contains_key(&recipient.thread_id) {
            recipient_state = project.agents.get(&recipient.thread_id).cloned();
            project_root = project.project_root.clone();
            project_cwd = project.cwd.clone();
            carried_group_ids = project
                .thread_groups
                .iter()
                .filter(|group| group.thread_ids.iter().any(|value| value == &recipient.thread_id))
                .map(|group| group.id.clone())
                .collect();
            break;
        }
    }
    let recipient_state =
        recipient_state.ok_or_else(|| anyhow::anyhow!("Recipient `{}` is not tracked.", recipient.thread_id))?;
    let role = recipient_state
        .role
        .clone()
        .unwrap_or_else(|| recipient.role.clone());
    let project_path_value = project_root
        .clone()
        .unwrap_or_else(|| recipient.project_path.clone());
    let cwd = recipient_state
        .cwd
        .clone()
        .or(project_cwd.clone())
        .or(project_root.clone())
        .unwrap_or_else(|| recipient.cwd.clone());
    let spawn_payload = json!({
        "displayName": recipient_state.display_name.clone().or(recipient.display_name.clone()),
        "initialPrompt": prompt,
        "cwd": cwd,
        "projectPath": project_path_value,
        "role": role,
        "approvalPolicy": recipient_state.approval_policy,
        "sandboxMode": recipient_state.sandbox_mode,
        "networkAccess": recipient_state.network_access,
        "modelID": recipient_state.model,
        "reasoningEffort": recipient_state.reasoning_effort,
        "parentAgentId": sender_thread_id,
    });
    let mut replacement = spawn_agent(runtime, &spawn_payload).await?;

    let mut next_state = parse_state(&runtime.state_document_value().await);
    let mut archived_old = false;
    if let Some(project) = next_state
        .projects
        .values_mut()
        .find(|project| project.agents.contains_key(&recipient.thread_id))
    {
        if let Some(new_agent_state) = project.agents.get_mut(&replacement.id) {
            new_agent_state.issue_number = recipient_state.issue_number;
            new_agent_state.pull_request_number = recipient_state.pull_request_number;
            new_agent_state.blocked_reason = recipient_state.blocked_reason.clone();
            new_agent_state.unblock_when = recipient_state.unblock_when.clone();
            for (key, value) in &recipient_state.extras {
                new_agent_state.extras.insert(key.clone(), value.clone());
            }
        }
        if let Some(old_agent_state) = project.agents.get_mut(&recipient.thread_id) {
            old_agent_state.archived = Some(true);
            archived_old = true;
        }
        for group in &mut project.thread_groups {
            if group.thread_ids.iter().any(|value| value == &recipient.thread_id)
                && !group.thread_ids.iter().any(|value| value == &replacement.id)
            {
                group.thread_ids.push(replacement.id.clone());
            }
        }
        project.updated_at = Some(unix_now());
    }
    next_state.updated_at = Some(unix_now());
    persist_state(runtime, &next_state).await?;
    if archived_old {
        let _ = app_server_request_json(
            runtime,
            "thread/archive",
            json!({"threadId": recipient.thread_id}),
        )
        .await;
    }
    replacement.parent_agent_id = Some(sender_thread_id.to_string());
    Ok(json!({
        "previousThreadId": recipient.thread_id,
        "replacementThreadId": replacement.id,
        "groupIds": carried_group_ids,
        "agent": replacement,
    }))
}

pub async fn orchestrator_archive_agent(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    recipient_thread_id: Option<&str>,
    recipient_name: Option<&str>,
    project_path: Option<&str>,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    let sender = scoped.sender;
    let recipient = resolve_scoped_recipient(&scoped.visible, recipient_thread_id, recipient_name, project_path)?;
    if !sender.is_orchestrator || sender.project_path != recipient.project_path {
        bail!("Only the configured orchestrator thread for project `{}` can archive agents in that project.", recipient.project_path);
    }
    if recipient.thread_id == sender.thread_id {
        bail!("Orchestrator thread `{}` cannot archive itself.", sender.thread_id);
    }
    let already_archived = state
        .projects
        .values()
        .all(|project| !project.agents.contains_key(&recipient.thread_id))
        && records.iter().all(|record| record.thread_id != recipient.thread_id || record.is_archived);
    if !already_archived {
        archive_thread(runtime, &recipient.thread_id).await?;
    }
    Ok(json!({
        "recipientThreadId": recipient.thread_id,
        "recipientDisplayName": recipient.display_name,
        "alreadyArchived": already_archived,
    }))
}

pub async fn orchestrator_rename_agent(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    recipient_thread_id: Option<&str>,
    recipient_name: Option<&str>,
    project_path: Option<&str>,
    new_name: &str,
) -> Result<Value> {
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    let sender = scoped.sender;
    let recipient = resolve_scoped_recipient(&scoped.visible, recipient_thread_id, recipient_name, project_path)?;
    if !sender.is_orchestrator || sender.project_path != recipient.project_path {
        bail!("Only orchestrator threads can rename agents in project `{}`.", recipient.project_path);
    }
    let previous = state
        .projects
        .values()
        .find_map(|project| project.agents.get(&recipient.thread_id))
        .and_then(|agent| agent.display_name.clone())
        .or(recipient.display_name.clone())
        .unwrap_or_else(|| recipient.thread_id.clone());
    if let Some(agent) = state
        .projects
        .values_mut()
        .find_map(|project| project.agents.get_mut(&recipient.thread_id))
    {
        agent.display_name = Some(new_name.to_string());
    }
    persist_state(runtime, &state).await?;
    Ok(json!({
        "recipientThreadId": recipient.thread_id,
        "previousDisplayName": previous,
        "newName": new_name,
    }))
}

async fn archive_thread(runtime: &BridgeRuntime, thread_id: &str) -> Result<()> {
    let mut state = parse_state(&runtime.state_document_value().await);
    let pruned = prune_archived_thread_locally(&mut state, thread_id);
    if pruned {
        persist_state(runtime, &state).await?;
        runtime.prune_thread_local(thread_id).await?;
    }
    if let Err(error) = app_server_request_json(runtime, "thread/archive", json!({"threadId": thread_id})).await {
        let message = error.to_string();
        if !message.contains("no rollout found for thread id") && !message.contains("\"code\": -32600") {
            return Err(error);
        }
    }
    Ok(())
}

fn prune_archived_thread_locally(state: &mut PersistedState, thread_id: &str) -> bool {
    let mut changed = false;
    for project in state.projects.values_mut() {
        let mut project_changed = false;
        if project.agents.remove(thread_id).is_some() {
            project_changed = true;
        }
        if project.orchestrator_thread_id.as_deref() == Some(thread_id) {
            project.orchestrator_thread_id = None;
            project_changed = true;
        }
        let mut next_groups = Vec::new();
        for mut group in project.thread_groups.clone() {
            let original = group.thread_ids.len();
            group.thread_ids.retain(|id| id != thread_id);
            let filtered_len = group.thread_ids.len();
            if !group.thread_ids.is_empty() {
                next_groups.push(group);
            }
            if filtered_len != original {
                project_changed = true;
            }
        }
        if project.thread_groups != next_groups {
            project.thread_groups = next_groups;
            project_changed = true;
        }
        if project_changed {
            project.updated_at = Some(unix_now());
            changed = true;
        }
    }
    if changed {
        state.updated_at = Some(unix_now());
    }
    changed
}

pub async fn orchestrator_update_worker_metadata(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    recipient_thread_id: Option<&str>,
    recipient_name: Option<&str>,
    project_path: Option<&str>,
    payload: &Value,
) -> Result<Value> {
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    let sender = scoped.sender;
    let recipient = resolve_scoped_recipient(&scoped.visible, recipient_thread_id, recipient_name, project_path)?;
    if !sender.is_orchestrator || sender.project_path != recipient.project_path {
        bail!("Only the configured orchestrator thread for project `{}` can set worker metadata in that project.", recipient.project_path);
    }
    if recipient.thread_id == sender.thread_id {
        bail!("Orchestrator thread `{}` cannot set metadata on itself.", sender.thread_id);
    }
    if recipient.is_orchestrator {
        bail!("Worker metadata can only be set on non-orchestrator threads.");
    }
    let agent = state
        .projects
        .values_mut()
        .find_map(|project| project.agents.get_mut(&recipient.thread_id))
        .ok_or_else(|| anyhow::anyhow!("Unknown project for thread {}", recipient.thread_id))?;
    if payload.get("issueNumber").is_some() || payload.get("clearIssueNumber").and_then(Value::as_bool) == Some(true) {
        agent.issue_number = if payload.get("clearIssueNumber").and_then(Value::as_bool) == Some(true) {
            None
        } else {
            payload.get("issueNumber").and_then(Value::as_u64)
        };
    }
    if payload.get("pullRequestNumber").is_some() || payload.get("clearPullRequestNumber").and_then(Value::as_bool) == Some(true) {
        agent.pull_request_number = if payload.get("clearPullRequestNumber").and_then(Value::as_bool) == Some(true) {
            None
        } else {
            payload.get("pullRequestNumber").and_then(Value::as_u64)
        };
    }
    if payload.get("blockedReason").is_some()
        || payload.get("unblockWhen").is_some()
        || payload.get("clearBlocked").and_then(Value::as_bool) == Some(true)
    {
        if payload.get("clearBlocked").and_then(Value::as_bool) == Some(true) {
            agent.blocked_reason = None;
            agent.unblock_when = None;
        } else {
            agent.blocked_reason = payload.get("blockedReason").and_then(Value::as_str).map(str::to_string);
            agent.unblock_when = payload.get("unblockWhen").and_then(Value::as_str).map(str::to_string);
        }
    }
    let issue_number = agent.issue_number;
    let pull_request_number = agent.pull_request_number;
    let blocked_reason = agent.blocked_reason.clone();
    let unblock_when = agent.unblock_when.clone();
    persist_state(runtime, &state).await?;
    Ok(json!({
        "recipientThreadId": recipient.thread_id,
        "recipientDisplayName": recipient.display_name,
        "issueNumber": issue_number,
        "pullRequestNumber": pull_request_number,
        "blockedReason": blocked_reason,
        "unblockWhen": unblock_when,
    }))
}

pub async fn orchestrator_approval_decision(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    approval_id: &str,
    decision: &str,
    message: Option<&str>,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let records = all_agent_records(&state, &running);
    let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
    let visible = scoped
        .visible
        .into_iter()
        .map(|record| record.thread_id)
        .collect::<std::collections::BTreeSet<_>>();
    let approval = runtime
        .pending_approvals()
        .await
        .into_iter()
        .find(|approval| approval.id == approval_id && visible.contains(&approval.thread_id))
        .ok_or_else(|| anyhow::anyhow!("Approval `{approval_id}` does not exist."))?;
    let follow_up = message.map(str::trim).filter(|value| !value.is_empty());
    let mut follow_up_error = None;
    let requested = decision == "decline" && follow_up.is_some();
    if let Some(message) = follow_up {
        if let Err(error) = send_follow_up_message(runtime, &approval, message).await {
            follow_up_error = Some(error.to_string());
        }
    }
    runtime
        .send_server_response(approval.request_id.clone(), json!({ "decision": decision }))
        .await?;
    Ok(json!({
        "approvalId": approval_id,
        "decision": decision,
        "message": message,
        "resolved": true,
        "followUpMessageRequested": requested,
        "followUpMessageSent": requested && follow_up_error.is_none(),
        "followUpError": follow_up_error,
    }))
}

fn basename(path: &str) -> String {
    path.rsplit('/').next().filter(|value| !value.is_empty()).unwrap_or("project").to_string()
}

async fn resolve_command_termination_target(
    runtime: &BridgeRuntime,
    thread_id: &str,
    item_id: Option<&str>,
) -> Result<Option<String>> {
    let normalized_item_id = item_id.map(str::trim).filter(|value| !value.is_empty());
    if let Some(item_id) = normalized_item_id {
        if let Some(target) = command_termination_target_for_item(runtime, thread_id, item_id).await {
            return Ok(Some(target));
        }
    }

    Ok(latest_command_termination_target(runtime, thread_id).await)
}

async fn command_termination_target_for_item(
    runtime: &BridgeRuntime,
    thread_id: &str,
    item_id: &str,
) -> Option<String> {
    let snapshot = runtime.snapshot().await.ok()?.thread_cache;
    snapshot
        .message_cache_by_thread_id
        .get(thread_id)
        .and_then(|messages| {
            messages.iter().find(|message| message.id == item_id).and_then(command_message_target)
        })
}

async fn latest_command_termination_target(runtime: &BridgeRuntime, thread_id: &str) -> Option<String> {
    let snapshot = runtime.snapshot().await.ok()?.thread_cache;
    snapshot
        .message_cache_by_thread_id
        .get(thread_id)
        .and_then(|messages| {
            messages
                .iter()
                .rev()
                .find(|message| {
                    message.role == "tool"
                        && message
                            .tool_metadata
                            .as_ref()
                            .map(|metadata| metadata.kind == "commandExecution")
                            .unwrap_or(false)
                })
                .and_then(command_message_target)
        })
}

fn command_message_target(message: &crate::models::RobdexChatMessage) -> Option<String> {
    let metadata = message.tool_metadata.as_ref()?;
    if metadata.kind != "commandExecution" {
        return None;
    }
    metadata
        .process_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn terminate_process_group_by_pid(process_id: &str) -> Result<()> {
    let normalized = process_id.trim();
    if let Some(job_id) = normalized.strip_prefix("job:") {
        return terminate_process_group_for_job(job_id);
    }
    if looks_like_job_id(normalized) {
        return terminate_process_group_for_job(normalized);
    }
    if !normalized.chars().all(|char| char.is_ascii_digit()) {
        bail!("Unsupported non-numeric process ID `{process_id}`.");
    }
    terminate_numeric_process_group(normalized)
}

fn terminate_process_group_for_job(job_id: &str) -> Result<()> {
    let marker = PathBuf::from("/tmp/codex-command-jobs").join(format!("{job_id}.job"));
    let contents = fs::read_to_string(&marker)
        .with_context(|| format!("failed to read {}", marker.display()))?;
    let cmd_pid = parse_job_file_value(&contents, "cmd_pid")
        .ok_or_else(|| anyhow::anyhow!("Job `{job_id}` does not contain cmd_pid."))?;
    if !cmd_pid.chars().all(|char| char.is_ascii_digit()) {
        bail!("Job `{job_id}` contains invalid cmd_pid `{cmd_pid}`.");
    }
    terminate_numeric_process_group(&cmd_pid)
}

fn terminate_numeric_process_group(normalized: &str) -> Result<()> {
    let target = format!("-{normalized}");
    let term = run_kill(["-TERM", "--", target.as_str()])?;
    if !term.status.success() {
        let stderr = String::from_utf8_lossy(&term.stderr);
        if stderr.contains("No such process") {
            return Ok(());
        }
        bail!(stderr.trim().to_string());
    }
    std::thread::sleep(std::time::Duration::from_millis(750));
    let probe = run_kill(["-0", "--", target.as_str()])?;
    if !probe.status.success() {
        return Ok(());
    }
    let kill = run_kill(["-KILL", "--", target.as_str()])?;
    if !kill.status.success() {
        let stderr = String::from_utf8_lossy(&kill.stderr);
        if !stderr.contains("No such process") {
            bail!(stderr.trim().to_string());
        }
    }
    Ok(())
}

fn parse_job_file_value(contents: &str, key: &str) -> Option<String> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")).map(str::trim))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn looks_like_job_id(value: &str) -> bool {
    value.len() >= 8
        && value
            .chars()
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '-')
}

fn run_kill<const N: usize>(args: [&str; N]) -> Result<std::process::Output> {
    Ok(Command::new("/bin/kill").args(args).output()?)
}

fn uuid() -> String {
    format!(
        "stub-{}-{}",
        unix_now(),
        std::process::id()
    )
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{BridgePaths, BridgeSettings},
        runtime::BridgeRuntime,
        upstream::UpstreamRuntimeEvent,
    };
    use codex_app_server_adapter::app_server_protocol::{
        JSONRPCMessage, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse, RequestId, ServerNotification, Turn,
        TurnStartedNotification, TurnStatus,
    };
    use codex_backend_core::HttpArgs;
    use futures_util::{SinkExt, StreamExt};
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        path::PathBuf,
        sync::Arc,
        time::Duration,
    };
    use tempfile::TempDir;
    use tokio::{net::TcpListener, sync::oneshot};
    use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};

    fn sample_state() -> PersistedState {
        let mut state = PersistedState::default();
        let mut project_alpha = PersistedProjectState {
            project_root: Some("/alpha".to_string()),
            cwd: Some("/alpha".to_string()),
            orchestrator_thread_id: Some("orch-a".to_string()),
            updated_at: Some(100),
            ..Default::default()
        };
        project_alpha.agents.insert(
            "orch-a".to_string(),
            PersistedAgentState {
                display_name: Some("Orch A".to_string()),
                role: Some("orchestrator".to_string()),
                project_root: Some("/alpha".to_string()),
                ..Default::default()
            },
        );
        project_alpha.agents.insert(
            "worker-a".to_string(),
            PersistedAgentState {
                display_name: Some("Worker A".to_string()),
                role: Some("worker".to_string()),
                project_root: Some("/alpha".to_string()),
                ..Default::default()
            },
        );
        project_alpha.agents.insert(
            "qa-a".to_string(),
            PersistedAgentState {
                display_name: Some("QA A".to_string()),
                role: Some("qa".to_string()),
                project_root: Some("/alpha".to_string()),
                ..Default::default()
            },
        );
        project_alpha.agents.insert(
            "operator-a".to_string(),
            PersistedAgentState {
                display_name: Some("Operator A".to_string()),
                role: Some("operator".to_string()),
                project_root: Some("/alpha".to_string()),
                ..Default::default()
            },
        );

        let mut project_beta = PersistedProjectState {
            project_root: Some("/beta".to_string()),
            cwd: Some("/beta".to_string()),
            orchestrator_thread_id: Some("orch-b".to_string()),
            updated_at: Some(90),
            ..Default::default()
        };
        project_beta.agents.insert(
            "orch-b".to_string(),
            PersistedAgentState {
                display_name: Some("Orch B".to_string()),
                role: Some("orchestrator".to_string()),
                project_root: Some("/beta".to_string()),
                ..Default::default()
            },
        );
        project_beta.agents.insert(
            "operator-b".to_string(),
            PersistedAgentState {
                display_name: Some("Operator B".to_string()),
                role: Some("operator".to_string()),
                project_root: Some("/beta".to_string()),
                ..Default::default()
            },
        );
        project_beta.agents.insert(
            "worker-b".to_string(),
            PersistedAgentState {
                display_name: Some("Worker B".to_string()),
                role: Some("worker".to_string()),
                project_root: Some("/beta".to_string()),
                ..Default::default()
            },
        );

        state.projects.insert("alpha".to_string(), project_alpha);
        state.projects.insert("beta".to_string(), project_beta);
        state
    }

    #[test]
    fn orchestrator_can_see_cross_project_orchestrators_and_operators() {
        let state = sample_state();
        let records = all_agent_records(&state, &["worker-a".to_string()]);
        let scoped = scoped_agent_context(&records, "orch-a", false).expect("scoped");
        let ids = scoped.visible.iter().map(|record| record.thread_id.as_str()).collect::<Vec<_>>();
        assert!(ids.contains(&"orch-a"));
        assert!(ids.contains(&"worker-a"));
        assert!(ids.contains(&"qa-a"));
        assert!(ids.contains(&"operator-a"));
        assert!(ids.contains(&"orch-b"));
        assert!(ids.contains(&"operator-b"));
        assert!(!ids.contains(&"worker-b"));
    }

    #[test]
    fn worker_sees_only_agents_in_same_project() {
        let state = sample_state();
        let records = all_agent_records(&state, &["worker-a".to_string()]);
        let scoped = scoped_agent_context(&records, "worker-a", false).expect("scoped");
        let ids = scoped.visible.iter().map(|record| record.thread_id.as_str()).collect::<Vec<_>>();
        assert!(ids.contains(&"orch-a"));
        assert!(ids.contains(&"worker-a"));
        assert!(ids.contains(&"qa-a"));
        assert!(ids.contains(&"operator-a"));
        assert!(!ids.contains(&"orch-b"));
        assert!(!ids.contains(&"operator-b"));
        assert!(!ids.contains(&"worker-b"));
    }

    #[test]
    fn workspace_read_file_truncates_to_max_bytes() {
        let temp = TempDir::new().expect("tempdir");
        let file_path = temp.path().join("output.log");
        std::fs::write(&file_path, "abcdef").expect("write");
        let settings = sample_settings(&temp, "ws://127.0.0.1:0".to_string());
        let runtime = tokio::runtime::Runtime::new().expect("runtime").block_on(BridgeRuntime::new(settings)).expect("runtime");
        let file = workspace_read_file(&runtime, "output.log", Some(4)).expect("file");
        assert_eq!(file.get("content").and_then(Value::as_str), Some("abcd"));
        assert_eq!(file.get("isTruncated").and_then(Value::as_bool), Some(true));
        assert_eq!(file.get("totalBytes").and_then(Value::as_u64), Some(6));
    }

    #[test]
    fn parse_job_file_value_reads_cmd_pid() {
        let contents = "job_id=abc\ncmd_pid=12345\n";
        assert_eq!(parse_job_file_value(contents, "cmd_pid").as_deref(), Some("12345"));
    }

    #[test]
    fn looks_like_job_id_accepts_launch_job_ids() {
        assert!(looks_like_job_id("12345678-abcd-1234-abcd-1234567890ab"));
        assert!(!looks_like_job_id("123"));
        assert!(!looks_like_job_id("PID1234"));
    }

    fn sample_settings(root: &TempDir, app_server_url: String) -> BridgeSettings {
        BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url,
            project_path: root.path().to_path_buf(),
            cwd: root.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(root.path()).join("state")),
        }
    }

    async fn seed_agent_state(runtime: &Arc<BridgeRuntime>) {
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": runtime.settings().project_path.display().to_string(),
                        "cwd": runtime.settings().cwd.display().to_string(),
                        "agents": {
                            "sender": {
                                "displayName": "Config Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": runtime.settings().project_path.display().to_string(),
                                "cwd": runtime.settings().cwd.display().to_string()
                            },
                            "recipient": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": runtime.settings().project_path.display().to_string(),
                                "cwd": runtime.settings().cwd.display().to_string(),
                                "approvalPolicy": "on-request"
                            }
                        },
                        "orchestratorThreadId": "sender"
                    }
                }
            }))
            .await
            .expect("persist state");
    }

    #[tokio::test]
    async fn thread_running_state_set_updates_snapshot() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");

        execute_bridge_command(
            &runtime,
            "threadRunningStateSet",
            json!({
                "threadId": "thread-1",
                "isRunning": true,
            }),
        )
        .await
        .expect("command");

        let snapshot = runtime.snapshot().await.expect("snapshot");
        assert_eq!(snapshot.thread_cache.running_thread_ids, vec!["thread-1".to_string()]);
    }

    #[tokio::test]
    async fn close_agent_removes_agent_from_state_and_returns_closed_status() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        seed_agent_state(&runtime).await;
        runtime
            .set_manual_thread_running_state("recipient", true)
            .await
            .expect("running state");

        let outcome = execute_bridge_command(
            &runtime,
            "closeAgent",
            json!({
                "agentId": "recipient",
            }),
        )
        .await
        .expect("command");

        assert_eq!(outcome.payload.get("type").and_then(Value::as_str), Some("agent"));
        let payload = outcome.payload.get("payload").expect("payload");
        assert_eq!(payload.get("id").and_then(Value::as_str), Some("recipient"));
        assert_eq!(payload.get("status").and_then(Value::as_str), Some("closed"));

        let state = runtime.state_document_value().await;
        assert!(
            state
                .get("projects")
                .and_then(|value| value.get("alpha"))
                .and_then(|value| value.get("agents"))
                .and_then(|value| value.get("recipient"))
                .is_none()
        );
    }

    #[tokio::test]
    async fn command_execution_terminate_prefers_item_target() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");

        runtime
            .upstream_sender()
            .send(UpstreamRuntimeEvent::Notification(ServerNotification::ItemCompleted(
                codex_app_server_adapter::app_server_protocol::ItemCompletedNotification {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item: codex_app_server_adapter::app_server_protocol::ThreadItem::CommandExecution {
                        id: "cmd-1".to_string(),
                        command: "command-parser cargo test".to_string(),
                        cwd: PathBuf::from("/tmp"),
                        process_id: Some("93456".to_string()),
                        status: codex_app_server_adapter::app_server_protocol::CommandExecutionStatus::InProgress,
                        command_actions: Vec::new(),
                        aggregated_output: Some(
                            "job_id: 12345678-abcd-1234-abcd-1234567890ab\nstill running".to_string(),
                        ),
                        exit_code: None,
                        duration_ms: None,
                    },
                },
            )))
            .await
            .expect("item");
        tokio::time::sleep(Duration::from_millis(50)).await;

        let target = resolve_command_termination_target(&runtime, "thread-1", Some("cmd-1"))
            .await
            .expect("target");
        assert_eq!(target.as_deref(), Some("job:12345678-abcd-1234-abcd-1234567890ab"));
    }

    #[tokio::test]
    async fn command_execution_terminate_falls_back_to_latest_thread_target() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");

        runtime
            .upstream_sender()
            .send(UpstreamRuntimeEvent::Notification(ServerNotification::ItemCompleted(
                codex_app_server_adapter::app_server_protocol::ItemCompletedNotification {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item: codex_app_server_adapter::app_server_protocol::ThreadItem::CommandExecution {
                        id: "cmd-1".to_string(),
                        command: "command-parser cargo test".to_string(),
                        cwd: PathBuf::from("/tmp"),
                        process_id: Some("93456".to_string()),
                        status: codex_app_server_adapter::app_server_protocol::CommandExecutionStatus::InProgress,
                        command_actions: Vec::new(),
                        aggregated_output: Some(
                            "job_id: 12345678-abcd-1234-abcd-1234567890ab\nstill running".to_string(),
                        ),
                        exit_code: None,
                        duration_ms: None,
                    },
                },
            )))
            .await
            .expect("item");
        tokio::time::sleep(Duration::from_millis(50)).await;

        let target = resolve_command_termination_target(&runtime, "thread-1", None)
            .await
            .expect("target");
        assert_eq!(target.as_deref(), Some("job:12345678-abcd-1234-abcd-1234567890ab"));
    }

    async fn spawn_ws_server(
        handler: impl FnOnce(WebSocketStream<tokio::net::TcpStream>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + 'static,
    ) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let ws = accept_async(stream).await.expect("ws");
            handler(ws).await;
        });
        addr
    }

    #[tokio::test]
    async fn send_agent_input_uses_turn_start_without_active_turn() {
        let temp = TempDir::new().expect("tempdir");
        let (request_tx, request_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request = match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
                    JSONRPCMessage::Request(request) => request,
                    other => panic!("unexpected init message: {other:?}"),
                };
                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                        id: init_request.id,
                        result: json!({}),
                    }))
                    .expect("init response")
                    .into(),
                ))
                .await
                .expect("send init response");

                let next = ws.next().await.expect("request").expect("request frame");
                let text = match next {
                    Message::Text(text) => text,
                    other => panic!("unexpected request frame: {other:?}"),
                };
                let request = match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                    JSONRPCMessage::Request(request) => request,
                    other => panic!("unexpected request message: {other:?}"),
                };
                request_tx.send(request.clone()).expect("record request");
                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                        id: request.id,
                        result: json!({
                            "turn": {
                                "id": "turn-start-1",
                                "items": [],
                                "status": "inProgress",
                                "error": null
                            }
                        }),
                    }))
                    .expect("turn response")
                    .into(),
                ))
                .await
                .expect("send turn response");
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        seed_agent_state(&runtime).await;

        let outcome = execute_bridge_command(
            &runtime,
            "sendAgentInput",
            json!({
                "agentId": "recipient",
                "text": "Please continue",
                "senderAgentId": "sender",
                "modelID": "gpt-test",
                "reasoningEffort": "medium"
            }),
        )
        .await
        .expect("command outcome");

        let request = request_rx.await.expect("captured request");
        assert_eq!(request.method, "turn/start");
        let params = request.params.expect("params");
        assert_eq!(params["threadId"], "recipient");
        assert_eq!(params["input"][0]["text"], "[Config Orchestrator] Please continue");
        assert_eq!(params["model"], "gpt-test");
        assert_eq!(params["effort"], "medium");
        assert_eq!(outcome.payload["type"], "turn");
    }

    #[tokio::test]
    async fn send_agent_input_uses_turn_steer_with_active_turn() {
        let temp = TempDir::new().expect("tempdir");
        let (request_tx, request_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request = match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
                    JSONRPCMessage::Request(request) => request,
                    other => panic!("unexpected init message: {other:?}"),
                };
                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                        id: init_request.id,
                        result: json!({}),
                    }))
                    .expect("init response")
                    .into(),
                ))
                .await
                .expect("send init response");

                let next = ws.next().await.expect("request").expect("request frame");
                let text = match next {
                    Message::Text(text) => text,
                    other => panic!("unexpected request frame: {other:?}"),
                };
                let request = match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                    JSONRPCMessage::Request(request) => request,
                    other => panic!("unexpected request message: {other:?}"),
                };
                request_tx.send(request.clone()).expect("record request");
                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                        id: request.id,
                        result: json!({"turnId":"turn-active-1"}),
                    }))
                    .expect("steer response")
                    .into(),
                ))
                .await
                .expect("send steer response");
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        seed_agent_state(&runtime).await;
        runtime
            .upstream_sender()
            .send(UpstreamRuntimeEvent::Notification(ServerNotification::TurnStarted(
                TurnStartedNotification {
                    thread_id: "recipient".to_string(),
                    turn: Turn {
                        id: "turn-active-1".to_string(),
                        items: Vec::new(),
                        status: TurnStatus::InProgress,
                        error: None,
                    },
                },
            )))
            .await
            .expect("turn started");
        tokio::time::sleep(Duration::from_millis(50)).await;

        let outcome = execute_bridge_command(
            &runtime,
            "sendAgentInput",
            json!({
                "agentId": "recipient",
                "text": "Please continue",
                "senderAgentId": "sender"
            }),
        )
        .await
        .expect("command outcome");

        let request = request_rx.await.expect("captured request");
        assert_eq!(request.method, "turn/steer");
        let params = request.params.expect("params");
        assert_eq!(params["threadId"], "recipient");
        assert_eq!(params["expectedTurnId"], "turn-active-1");
        assert_eq!(params["input"][0]["text"], "[Config Orchestrator] Please continue");
        assert_eq!(outcome.payload["type"], "turn");
    }

    #[tokio::test]
    async fn command_approval_round_trips_over_transport_and_snapshot() {
        let temp = TempDir::new().expect("tempdir");
        let (response_tx, response_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request = match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
                    JSONRPCMessage::Request(request) => request,
                    other => panic!("unexpected init message: {other:?}"),
                };
                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                        id: init_request.id,
                        result: json!({}),
                    }))
                    .expect("init response")
                    .into(),
                ))
                .await
                .expect("send init response");

                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Request(JSONRPCRequest {
                        id: RequestId::Integer(99),
                        method: "item/commandExecution/requestApproval".to_string(),
                        params: Some(json!({
                            "threadId": "recipient",
                            "turnId": "turn-approval-1",
                            "itemId": "item-1",
                            "command": "make build",
                            "cwd": "/tmp/project",
                            "reason": "needs approval"
                        })),
                        trace: None,
                    }))
                    .expect("approval request")
                    .into(),
                ))
                .await
                .expect("send approval request");

                let response = ws.next().await.expect("response").expect("response frame");
                let response_text = match response {
                    Message::Text(text) => text,
                    other => panic!("unexpected response frame: {other:?}"),
                };
                let response_message = serde_json::from_str::<JSONRPCMessage>(&response_text).expect("jsonrpc response");
                response_tx.send(response_message).expect("record response");

                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Notification(JSONRPCNotification {
                        method: "serverRequest/resolved".to_string(),
                        params: Some(json!({
                            "threadId": "recipient",
                            "requestId": 99
                        })),
                    }))
                    .expect("resolved notification")
                    .into(),
                ))
                .await
                .expect("send resolved");
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();

        tokio::time::sleep(Duration::from_millis(150)).await;
        let snapshot = make_app_state_snapshot(&runtime, true).await.expect("snapshot");
        assert_eq!(snapshot.pending_approvals.len(), 1);
        assert_eq!(snapshot.pending_approvals[0].request_id, RequestId::Integer(99));

        let outcome = execute_bridge_command(
            &runtime,
            "commandApproval",
            json!({
                "instanceId": runtime.settings().project_path.display().to_string(),
                "requestId": 99,
                "decision": "accept"
            }),
        )
        .await
        .expect("approval command");
        assert_eq!(outcome.payload["type"], "approvalResult");

        let response = response_rx.await.expect("response");
        match response {
            JSONRPCMessage::Response(response) => {
                assert_eq!(response.id, RequestId::Integer(99));
                assert_eq!(response.result["decision"], "accept");
            }
            other => panic!("unexpected message: {other:?}"),
        }

        tokio::time::sleep(Duration::from_millis(150)).await;
        let snapshot = make_app_state_snapshot(&runtime, true).await.expect("snapshot");
        assert!(snapshot.pending_approvals.is_empty());

        transport.abort();
    }

    #[tokio::test]
    async fn event_replay_serializes_full_app_state_snapshot_shape() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");

        runtime
            .push_event(crate::models::BridgeEvent::AppStateSnapshot {
                state: json!({"projects": {}}),
            })
            .await;

        let replay = make_event_replay_response(&runtime, None).await.expect("replay");
        let event = &replay["events"][1]["event"];
        assert_eq!(event["name"], "appStateSnapshot");
        assert!(event["data"]["state"].is_object());
        assert!(event["data"]["pendingApprovals"].is_array());
        assert!(event["data"]["agents"].is_array());
    }

    #[tokio::test]
    async fn skills_list_passthrough_returns_upstream_shape() {
        let temp = TempDir::new().expect("tempdir");
        let (request_tx, request_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request = match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
                    JSONRPCMessage::Request(request) => request,
                    other => panic!("unexpected init message: {other:?}"),
                };
                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                        id: init_request.id,
                        result: json!({}),
                    }))
                    .expect("init response")
                    .into(),
                ))
                .await
                .expect("send init response");

                let next = ws.next().await.expect("request").expect("request frame");
                let text = match next {
                    Message::Text(text) => text,
                    other => panic!("unexpected request frame: {other:?}"),
                };
                let request = match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                    JSONRPCMessage::Request(request) => request,
                    other => panic!("unexpected request message: {other:?}"),
                };
                request_tx.send(request.clone()).expect("record request");
                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                        id: request.id,
                        result: json!({
                            "data": [{
                                "cwd": "/tmp/project",
                                "skills": [{
                                    "name": "robdex-orchestrator",
                                    "description": "Use Robdex communication",
                                    "shortDescription": "Robdex",
                                    "interface": {
                                        "displayName": "Robdex Orchestrator",
                                        "shortDescription": "Robdex"
                                    },
                                    "path": "/Users/robertsale/.codex/skills/robdex-orchestrator/SKILL.md",
                                    "scope": "user",
                                    "enabled": true
                                }],
                                "errors": []
                            }]
                        }),
                    }))
                    .expect("skills response")
                    .into(),
                ))
                .await
                .expect("send skills response");
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let outcome = execute_bridge_command(
            &runtime,
            "skillsList",
            json!({
                "cwds": ["/tmp/project"],
                "forceReload": true
            }),
        )
        .await
        .expect("skillsList");

        let request = request_rx.await.expect("captured request");
        assert_eq!(request.method, "skills/list");
        let params = request.params.expect("params");
        assert_eq!(params["cwds"][0], "/tmp/project");
        assert_eq!(outcome.payload["type"], "skillsList");
        assert_eq!(outcome.payload["payload"]["data"][0]["skills"][0]["name"], "robdex-orchestrator");
    }

    #[tokio::test]
    async fn dynamic_tool_call_response_uses_persistent_transport() {
        let temp = TempDir::new().expect("tempdir");
        let (response_tx, response_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request = match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
                    JSONRPCMessage::Request(request) => request,
                    other => panic!("unexpected init message: {other:?}"),
                };
                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                        id: init_request.id,
                        result: json!({}),
                    }))
                    .expect("init response")
                    .into(),
                ))
                .await
                .expect("send init response");

                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Request(JSONRPCRequest {
                        id: RequestId::Integer(77),
                        method: "item/tool/call".to_string(),
                        params: Some(json!({
                            "threadId": "recipient",
                            "turnId": "turn-tool-1",
                            "callId": "call-1",
                            "tool": "client_tool",
                            "arguments": {"message":"hello"}
                        })),
                        trace: None,
                    }))
                    .expect("dynamic tool request")
                    .into(),
                ))
                .await
                .expect("send dynamic tool request");

                let response = ws.next().await.expect("response").expect("response frame");
                let response_text = match response {
                    Message::Text(text) => text,
                    other => panic!("unexpected response frame: {other:?}"),
                };
                let response_message = serde_json::from_str::<JSONRPCMessage>(&response_text).expect("jsonrpc response");
                response_tx.send(response_message).expect("record response");
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();

        tokio::time::sleep(Duration::from_millis(150)).await;
        let snapshot = make_app_state_snapshot(&runtime, true).await.expect("snapshot");
        assert_eq!(snapshot.pending_approvals.len(), 1);
        assert_eq!(snapshot.pending_approvals[0].kind, crate::models::PendingApprovalKind::DynamicToolCall);

        let outcome = execute_bridge_command(
            &runtime,
            "dynamicToolCallResponse",
            json!({
                "instanceId": runtime.settings().project_path.display().to_string(),
                "requestId": 77,
                "success": true,
                "contentItems": [{"type":"inputText","text":"done"}]
            }),
        )
        .await
        .expect("dynamicToolCallResponse");
        assert_eq!(outcome.payload["type"], "empty");

        let response = response_rx.await.expect("response");
        match response {
            JSONRPCMessage::Response(response) => {
                assert_eq!(response.id, RequestId::Integer(77));
                assert_eq!(response.result["success"], true);
                assert_eq!(response.result["contentItems"][0]["text"], "done");
            }
            other => panic!("unexpected message: {other:?}"),
        }

        transport.abort();
    }

    #[test]
    fn prune_archived_thread_locally_removes_agent_and_group_membership() {
        let mut state = PersistedState::default();
        let mut project = PersistedProjectState {
            project_root: Some("/alpha".to_string()),
            cwd: Some("/alpha".to_string()),
            orchestrator_thread_id: Some("orch-a".to_string()),
            thread_groups: vec![ThreadGroupState {
                id: "group-1".to_string(),
                title: "Workers".to_string(),
                thread_ids: vec!["worker-1".to_string(), "worker-2".to_string()],
                ..Default::default()
                }],
            ..Default::default()
        };
        project.agents.insert(
            "worker-1".to_string(),
            PersistedAgentState {
                display_name: Some("Worker One".to_string()),
                role: Some("worker".to_string()),
                project_root: Some("/alpha".to_string()),
                ..Default::default()
            },
        );
        project.agents.insert(
            "worker-2".to_string(),
            PersistedAgentState {
                display_name: Some("Worker Two".to_string()),
                role: Some("worker".to_string()),
                project_root: Some("/alpha".to_string()),
                ..Default::default()
            },
        );
        state.projects.insert("alpha".to_string(), project);

        assert!(prune_archived_thread_locally(&mut state, "worker-1"));
        let project = state.projects.get("alpha").expect("project");
        assert!(!project.agents.contains_key("worker-1"));
        assert_eq!(project.thread_groups.len(), 1);
        assert_eq!(project.thread_groups[0].thread_ids, vec!["worker-2".to_string()]);
    }

    #[test]
    fn tracked_thread_display_name_can_be_overridden_after_registration() {
        let mut state = PersistedState::default();
        register_tracked_thread(
            &mut state,
            &json!({
                "thread": {"id": "worker-1", "title": "worker-1"},
                "projectPath": "/alpha",
                "preferredCWD": "/alpha",
                "role": "worker"
            }),
        )
        .expect("register");
        set_tracked_thread_display_name(&mut state, "worker-1", "Bridge Agent Smoke");

        let display_name = state
            .projects
            .get("alpha")
            .and_then(|project| project.agents.get("worker-1"))
            .and_then(|agent| agent.display_name.clone());
        assert_eq!(display_name.as_deref(), Some("Bridge Agent Smoke"));
    }
}
