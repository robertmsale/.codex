use std::{collections::BTreeMap, env, path::{Path, PathBuf}, time::{Duration, Instant}};

use anyhow::{Context, Result, bail};
use codex_app_server_adapter::app_server_protocol::RequestId;
use robdex_protocol::HookFailureNotice;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::timeout;

use crate::{
    app_server_overrides::{AppServerThreadOverrides, AppServerTurnOverrides, simple_sandbox_policy},
    hooks::{
        HookEvent, HookLifecycleState, HookResult, HookTelemetry, append_prompt_segments,
        compaction_payload, maybe_run_project_hook, qa_archive_payload, qa_create_payload, worker_archive_payload,
        worker_create_payload,
    },
    models::{BridgeAgentSummary, BridgeAppStateSnapshot, BridgeInstanceSummary, LiveProcessRecord, PendingApproval, ThreadCachePayload},
    runtime::BridgeRuntime,
    transforms::{resolve_role_instructions, summarize_scoped_agent_record},
};

const HOOK_LIFECYCLE_STATE_KEY: &str = "robdexHookLifecycle";
const HOOK_TELEMETRY_KEY: &str = "robdexHookTelemetry";
const PROJECT_HOOK_TELEMETRY_KEY: &str = "robdexRecentHookTelemetry";
const LIVE_PROCESSES_KEY: &str = "robdexLiveProcesses";
const COMPACTION_STATE_KEY: &str = "robdexCompaction";

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompactionState {
    pub count: u64,
    #[serde(default)]
    pub last_compacted_at: Option<u64>,
}

#[derive(Debug)]
pub struct CommandOutcome {
    pub payload: Value,
    pub error_message: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PersistedState {
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
    model_provider: Option<String>,
    reasoning_effort: Option<String>,
    service_tier: Option<Value>,
    approvals_reviewer: Option<Value>,
    personality: Option<Value>,
    config: Option<Value>,
    base_instructions: Option<String>,
    developer_instructions: Option<String>,
    persist_extended_history: Option<bool>,
    service_name: Option<String>,
    ephemeral: Option<bool>,
    dynamic_tools: Option<Value>,
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
struct ExplicitThreadSettings {
    cwd: String,
    approval_policy: Option<String>,
    sandbox_mode: Option<String>,
    network_access: Option<bool>,
    model: Option<String>,
    model_provider: Option<String>,
    reasoning_effort: Option<String>,
    service_tier: Option<Value>,
    approvals_reviewer: Option<Value>,
    personality: Option<Value>,
    config: Option<Value>,
    base_instructions: Option<String>,
    developer_instructions: Option<String>,
    persist_extended_history: Option<bool>,
    service_name: Option<String>,
    ephemeral: Option<bool>,
    dynamic_tools: Option<Value>,
}

impl ExplicitThreadSettings {
    fn to_app_server_thread_overrides(&self) -> AppServerThreadOverrides {
        AppServerThreadOverrides {
            cwd: Some(self.cwd.clone()),
            approval_policy: self.approval_policy.clone().map(Value::String),
            sandbox: self.sandbox_mode.clone(),
            model: self.model.clone(),
            model_provider: self.model_provider.clone(),
            reasoning_effort: self.reasoning_effort.clone(),
            service_tier: self.service_tier.clone(),
            approvals_reviewer: self.approvals_reviewer.clone(),
            personality: self.personality.clone(),
            config: self.config.clone(),
            base_instructions: self.base_instructions.clone(),
            developer_instructions: self.developer_instructions.clone(),
            service_name: self.service_name.clone(),
            ephemeral: self.ephemeral,
            persist_extended_history: self.persist_extended_history,
            dynamic_tools: self.dynamic_tools.clone(),
        }
    }

    fn to_registration_payload(&self, thread: Value, project_path: &str, role: &str) -> Value {
        build_tracked_thread_registration_payload(
            thread,
            project_path,
            &self.cwd,
            role,
            self.approval_policy.as_deref(),
            self.sandbox_mode.as_deref(),
            self.network_access,
            self.model.as_deref(),
            self.model_provider.as_deref(),
            self.reasoning_effort.as_deref(),
            self.service_tier.clone(),
            self.approvals_reviewer.clone(),
            self.personality.clone(),
            self.config.clone(),
            self.base_instructions.as_deref(),
            self.developer_instructions.as_deref(),
            self.persist_extended_history,
            self.service_name.as_deref(),
            self.ephemeral,
            self.dynamic_tools.clone(),
        )
    }
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
            crate::models::BridgeEvent::LiveProcessesChanged { payload } => {
                json!({
                    "sequence": sequenced.sequence,
                    "event": {
                        "name": "liveProcessesChanged",
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
            crate::models::BridgeEvent::HookFailure { payload } => {
                json!({
                    "sequence": sequenced.sequence,
                    "event": {
                        "name": "hookFailure",
                        "data": payload,
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
                    "modelProvider": agent.model_provider,
                    "reasoningEffort": agent.reasoning_effort,
                    "serviceTier": agent.service_tier,
                    "approvalsReviewer": agent.approvals_reviewer,
                    "personality": agent.personality,
                    "config": agent.config,
                    "baseInstructions": agent.base_instructions,
                    "developerInstructions": agent.developer_instructions,
                    "persistExtendedHistory": agent.persist_extended_history,
                    "serviceName": agent.service_name,
                    "ephemeral": agent.ephemeral,
                    "dynamicTools": agent.dynamic_tools,
                    "robdexHookLifecycle": agent.extras.get(HOOK_LIFECYCLE_STATE_KEY).cloned(),
                    "robdexCompaction": agent.extras.get(COMPACTION_STATE_KEY).cloned(),
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
            let approval_policy = payload
                .get("approvalPolicy")
                .and_then(Value::as_str)
                .map(str::to_string);
            let settings = explicit_thread_settings_for_new_thread(
                &state,
                &json!({
                    "modelID": payload.get("model").cloned().unwrap_or(Value::Null),
                    "modelProvider": payload.get("modelProvider").cloned().unwrap_or(Value::Null),
                    "reasoningEffort": payload.get("reasoningEffort").cloned().unwrap_or(Value::Null),
                    "serviceTier": payload.get("serviceTier").cloned().unwrap_or(Value::Null),
                    "approvalsReviewer": payload.get("approvalsReviewer").cloned().unwrap_or(Value::Null),
                    "personality": payload.get("personality").cloned().unwrap_or(Value::Null),
                    "config": payload.get("config").cloned().unwrap_or(Value::Null),
                    "baseInstructions": payload.get("baseInstructions").cloned().unwrap_or(Value::Null),
                    "developerInstructions": payload.get("developerInstructions").cloned().unwrap_or(Value::Null),
                    "persistExtendedHistory": payload.get("persistExtendedHistory").cloned().unwrap_or(Value::Null),
                    "serviceName": payload.get("serviceName").cloned().unwrap_or(Value::Null),
                    "ephemeral": payload.get("ephemeral").cloned().unwrap_or(Value::Null),
                    "dynamicTools": payload.get("dynamicTools").cloned().unwrap_or(Value::Null),
                }),
                project_path.unwrap_or(""),
                &cwd,
                role,
                approval_policy,
                sandbox_mode,
                payload.get("networkAccess").and_then(Value::as_bool),
            );
            let params = settings.to_app_server_thread_overrides().thread_start_params();
            let result = app_server_request_json(runtime, "thread/start", params).await?;
            if let Some(project_path) = project_path {
                let mut next_state = parse_state(&runtime.state_document_value().await);
                register_tracked_thread(
                    &mut next_state,
                    &settings.to_registration_payload(
                        result.get("thread").cloned().unwrap_or(result.clone()),
                        project_path,
                        role.unwrap_or("worker"),
                    ),
                )?;
                persist_state(runtime, &next_state).await?;
            }
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
            let approval_policy =
                payload_value_or_string(&payload, "approvalPolicy", tracked_approval_policy_for_thread(&state, &thread_id));
            let sandbox_mode = payload
                .get("sandbox")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_sandbox_mode_for_thread(&state, &thread_id));
            let settings = explicit_thread_settings_for_existing_thread(
                &state,
                &payload,
                &thread_id,
                role.as_deref(),
                cwd.clone().unwrap_or_default(),
                approval_policy.and_then(|value| value.as_str().map(str::to_string)),
                sandbox_mode,
            );
            let params = settings.to_app_server_thread_overrides().thread_resume_params(
                thread_id.clone(),
                payload.get("history").cloned(),
                payload.get("path").cloned(),
            );
            let result = app_server_request_json(runtime, "thread/resume", params).await?;
            let mut state = parse_state(&runtime.state_document_value().await);
            if apply_explicit_thread_settings_to_tracked_thread(&mut state, &thread_id, &settings) {
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
            let approval_policy =
                payload_value_or_string(&payload, "approvalPolicy", tracked_approval_policy_for_thread(&state, &thread_id));
            let sandbox_mode = payload
                .get("sandbox")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| tracked_sandbox_mode_for_thread(&state, &thread_id));
            let settings = explicit_thread_settings_for_existing_thread(
                &state,
                &payload,
                &thread_id,
                role.as_deref(),
                cwd.clone().unwrap_or_default(),
                approval_policy.and_then(|value| value.as_str().map(str::to_string)),
                sandbox_mode,
            );
            let params = settings
                .to_app_server_thread_overrides()
                .thread_fork_params(thread_id.clone(), payload.get("path").cloned());
            let result = app_server_request_json(runtime, "thread/fork", params).await?;
            if let Some(new_thread) = result.get("thread").cloned() {
                let mut next_state = parse_state(&runtime.state_document_value().await);
                register_tracked_thread(
                    &mut next_state,
                    &settings.to_registration_payload(
                        new_thread,
                        &tracked_project_path_for_thread(&state, &thread_id)
                            .unwrap_or_else(|| cwd.clone().unwrap_or_default()),
                        role.as_deref().unwrap_or("worker"),
                    ),
                )?;
                persist_state(runtime, &next_state).await?;
            }
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
            let approval_policy =
                payload_value_or_string(&payload, "approvalPolicy", tracked_approval_policy_for_thread(&state, &thread_id));
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
            let params = AppServerTurnOverrides {
                cwd,
                model,
                effort,
                approval_policy,
                sandbox_policy,
                service_tier: payload.get("serviceTier").cloned().or_else(|| tracked_service_tier_for_thread(&state, &thread_id)),
                approvals_reviewer: payload
                    .get("approvalsReviewer")
                    .cloned()
                    .or_else(|| tracked_approvals_reviewer_for_thread(&state, &thread_id)),
                summary: payload.get("summary").cloned(),
                personality: payload.get("personality").cloned().or_else(|| tracked_personality_for_thread(&state, &thread_id)),
                output_schema: payload.get("outputSchema").cloned(),
                collaboration_mode: payload.get("collaborationMode").cloned(),
            }
            .turn_start_params(thread_id, json!([{"type":"text","text": required_string(&payload, "text")?}]));
            let result = app_server_request_json(runtime, "turn/start", params).await?;
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
        "mcpRefresh" => {
            let running_thread_ids = runtime.snapshot().await?.thread_cache.running_thread_ids;
            let mut interrupted_thread_ids = Vec::new();
            let mut interrupt_errors = Vec::new();

            for thread_id in running_thread_ids {
                let Some(turn_id) = runtime.active_turn_id_for_thread(&thread_id).await else {
                    continue;
                };

                match app_server_request_json(
                    runtime,
                    "turn/interrupt",
                    json!({
                        "threadId": thread_id,
                        "turnId": turn_id,
                    }),
                )
                .await
                {
                    Ok(_) => interrupted_thread_ids.push(thread_id),
                    Err(error) => interrupt_errors.push(json!({
                        "threadId": thread_id,
                        "error": error.to_string(),
                    })),
                }
            }

            app_server_request_json(runtime, "config/mcpServer/reload", json!({})).await?;

            Ok(success(json!({
                "type": "mcpRefresh",
                "payload": {
                    "interruptedThreadIDs": interrupted_thread_ids,
                    "interruptErrors": interrupt_errors,
                    "refreshed": true,
                }
            })))
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
            if let Some(approval) = approval {
                runtime.clear_pending_approval(&approval.id).await;
                runtime
                    .maybe_run_approval_resolved_hook(&approval, &decision, message.as_deref(), None)
                    .await?;
            }
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
            if let Some(approval) = approval {
                runtime.clear_pending_approval(&approval.id).await;
                runtime
                    .maybe_run_approval_resolved_hook(&approval, &decision, message.as_deref(), None)
                    .await?;
            }
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
                Some(&normalized_text),
                &[],
                payload.get("modelID").and_then(Value::as_str),
                payload.get("reasoningEffort").and_then(Value::as_str),
            )
            .await?;
            Ok(success(json!({"type":"turn","payload": result})))
        }
        "commandExecutionTerminate" => {
            let thread_id = required_string(&payload, "threadId")?;
            let process_id = required_string(&payload, "processId")?;
            if !terminate_live_process(runtime, &thread_id, &process_id).await? {
                bail!("No registered live process found for thread {thread_id} / processId {process_id}");
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

pub(crate) fn parse_state(value: &Value) -> PersistedState {
    serde_json::from_value(value.clone()).unwrap_or_default()
}

pub(crate) fn tracked_project_identity_for_thread(
    value: &Value,
    thread_id: &str,
) -> Option<(String, String, String)> {
    let state = parse_state(value);
    state.projects.values().find_map(|project| {
        let matches_agent = project.agents.contains_key(thread_id);
        let matches_orchestrator = project.orchestrator_thread_id.as_deref() == Some(thread_id);
        if !matches_agent && !matches_orchestrator {
            return None;
        }
        let project_root = project.project_root.clone()?;
        let project_id = project.id.clone().unwrap_or_else(|| project_root.clone());
        let project_name = project.name.clone().unwrap_or_else(|| project_id.clone());
        Some((project_id, project_name, project_root))
    })
}

fn prune_dead_live_processes_from_state(state: &mut PersistedState) -> bool {
    let mut changed = false;
    for project in state.projects.values_mut() {
        for agent in project.agents.values_mut() {
            let Some(value) = agent.extras.get(LIVE_PROCESSES_KEY).cloned() else {
                continue;
            };
            let Some(mut processes) = serde_json::from_value::<Vec<LiveProcessRecord>>(value).ok() else {
                continue;
            };
            let before = processes.len();
            processes.retain(live_process_is_alive);
            if processes.len() != before {
                agent.extras.insert(
                    LIVE_PROCESSES_KEY.to_string(),
                    serde_json::to_value(&processes).unwrap_or_else(|_| Value::Array(Vec::new())),
                );
                changed = true;
            }
        }
    }
    if changed {
        state.updated_at = Some(unix_now());
    }
    changed
}

fn live_process_is_alive(process: &LiveProcessRecord) -> bool {
    let target = process
        .process_group_id
        .filter(|pgid| *pgid > 0)
        .map(|pgid| -(pgid as libc::pid_t))
        .unwrap_or(process.pid as libc::pid_t);
    let rc = unsafe { libc::kill(target, 0) };
    if rc == 0 {
        return true;
    }
    let error = std::io::Error::last_os_error();
    error.raw_os_error() == Some(libc::EPERM)
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

pub(crate) async fn persist_state(runtime: &BridgeRuntime, state: &PersistedState) -> Result<()> {
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
    if let Some(role_defaults) = payload
        .get("roleDeveloperInstructionsDefaults")
        .cloned()
        .filter(|value| value.is_object())
    {
        let mut configs = project.configs.as_object().cloned().unwrap_or_default();
        configs.insert("roleDeveloperInstructionsDefaults".to_string(), role_defaults);
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
            if payload.get("modelProvider").is_some() {
                agent.model_provider = payload
                    .get("modelProvider")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if payload.get("reasoningEffort").is_some() {
                agent.reasoning_effort = payload
                    .get("reasoningEffort")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if payload.get("serviceTier").is_some() {
                agent.service_tier = payload.get("serviceTier").cloned().filter(|value| !value.is_null());
            }
            if payload.get("approvalsReviewer").is_some() {
                agent.approvals_reviewer = payload
                    .get("approvalsReviewer")
                    .cloned()
                    .filter(|value| !value.is_null());
            }
            if payload.get("personality").is_some() {
                agent.personality = payload.get("personality").cloned().filter(|value| !value.is_null());
            }
            if payload.get("config").is_some() {
                agent.config = payload.get("config").cloned().filter(|value| !value.is_null());
            }
            if payload.get("baseInstructions").is_some() {
                agent.base_instructions = payload
                    .get("baseInstructions")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if payload.get("developerInstructions").is_some() {
                agent.developer_instructions = payload
                    .get("developerInstructions")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if payload.get("persistExtendedHistory").is_some() {
                agent.persist_extended_history = payload.get("persistExtendedHistory").and_then(Value::as_bool);
            }
            if payload.get("serviceName").is_some() {
                agent.service_name = payload
                    .get("serviceName")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if payload.get("ephemeral").is_some() {
                agent.ephemeral = payload.get("ephemeral").and_then(Value::as_bool);
            }
            if payload.get("dynamicTools").is_some() {
                agent.dynamic_tools = payload.get("dynamicTools").cloned().filter(|value| !value.is_null());
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
            model_provider: payload.get("modelProvider").and_then(Value::as_str).map(str::to_string),
            reasoning_effort: payload.get("reasoningEffort").and_then(Value::as_str).map(str::to_string),
            service_tier: payload.get("serviceTier").cloned().filter(|value| !value.is_null()),
            approvals_reviewer: payload.get("approvalsReviewer").cloned().filter(|value| !value.is_null()),
            personality: payload.get("personality").cloned().filter(|value| !value.is_null()),
            config: payload.get("config").cloned().filter(|value| !value.is_null()),
            base_instructions: payload.get("baseInstructions").and_then(Value::as_str).map(str::to_string),
            developer_instructions: payload.get("developerInstructions").and_then(Value::as_str).map(str::to_string),
            persist_extended_history: payload.get("persistExtendedHistory").and_then(Value::as_bool),
            service_name: payload.get("serviceName").and_then(Value::as_str).map(str::to_string),
            ephemeral: payload.get("ephemeral").and_then(Value::as_bool),
            dynamic_tools: payload.get("dynamicTools").cloned().filter(|value| !value.is_null()),
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

fn apply_explicit_thread_settings_to_tracked_thread(
    state: &mut PersistedState,
    thread_id: &str,
    settings: &ExplicitThreadSettings,
) -> bool {
    for project in state.projects.values_mut() {
        if let Some(agent) = project.agents.get_mut(thread_id) {
            agent.cwd = Some(settings.cwd.clone());
            agent.approval_policy = settings.approval_policy.clone();
            agent.sandbox_mode = settings.sandbox_mode.clone();
            agent.network_access = settings.network_access;
            agent.model = settings.model.clone();
            agent.model_provider = settings.model_provider.clone();
            agent.reasoning_effort = settings.reasoning_effort.clone();
            agent.service_tier = settings.service_tier.clone();
            agent.approvals_reviewer = settings.approvals_reviewer.clone();
            agent.personality = settings.personality.clone();
            agent.config = settings.config.clone();
            agent.base_instructions = settings.base_instructions.clone();
            agent.developer_instructions = settings.developer_instructions.clone();
            agent.persist_extended_history = settings.persist_extended_history;
            agent.service_name = settings.service_name.clone();
            agent.ephemeral = settings.ephemeral;
            agent.dynamic_tools = settings.dynamic_tools.clone();
            project.updated_at = Some(unix_now());
            state.updated_at = Some(unix_now());
            return true;
        }
    }
    false
}

fn build_tracked_thread_registration_payload(
    thread: Value,
    project_path: &str,
    preferred_cwd: &str,
    role: &str,
    approval_policy: Option<&str>,
    sandbox_mode: Option<&str>,
    network_access: Option<bool>,
    model: Option<&str>,
    model_provider: Option<&str>,
    reasoning_effort: Option<&str>,
    service_tier: Option<Value>,
    approvals_reviewer: Option<Value>,
    personality: Option<Value>,
    config: Option<Value>,
    base_instructions: Option<&str>,
    developer_instructions: Option<&str>,
    persist_extended_history: Option<bool>,
    service_name: Option<&str>,
    ephemeral: Option<bool>,
    dynamic_tools: Option<Value>,
) -> Value {
    json!({
        "thread": thread,
        "projectPath": project_path,
        "preferredCWD": preferred_cwd,
        "role": role,
        "approvalPolicy": approval_policy,
        "sandboxMode": sandbox_mode,
        "networkAccess": network_access,
        "modelID": model,
        "modelProvider": model_provider,
        "reasoningEffort": reasoning_effort,
        "serviceTier": service_tier,
        "approvalsReviewer": approvals_reviewer,
        "personality": personality,
        "config": config,
        "baseInstructions": base_instructions,
        "developerInstructions": developer_instructions,
        "persistExtendedHistory": persist_extended_history,
        "serviceName": service_name,
        "ephemeral": ephemeral,
        "dynamicTools": dynamic_tools,
    })
}

fn preferred_model_provider_for_project(
    state: &PersistedState,
    project_path: Option<&str>,
) -> Option<String> {
    let target_path = normalize_path(project_path?.to_string());
    state.projects.values().find_map(|project| {
        project.project_root.as_ref().and_then(|root| {
            (normalize_path(root.clone()) == target_path)
                .then(|| project.preferred_model_provider.clone())
                .flatten()
        })
    })
}

fn explicit_thread_settings_for_new_thread(
    state: &PersistedState,
    payload: &Value,
    project_path: &str,
    cwd: &str,
    role: Option<&str>,
    approval_policy: Option<String>,
    sandbox_mode: Option<String>,
    network_access: Option<bool>,
) -> ExplicitThreadSettings {
    ExplicitThreadSettings {
        cwd: cwd.to_string(),
        approval_policy,
        sandbox_mode,
        network_access,
        model: payload
            .get("modelID")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| role_default_model(state, Some(project_path), role)),
        model_provider: payload
            .get("modelProvider")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| preferred_model_provider_for_project(state, Some(project_path))),
        reasoning_effort: payload
            .get("reasoningEffort")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| role_default_reasoning_effort(state, Some(project_path), role)),
        service_tier: payload.get("serviceTier").cloned().filter(|value| !value.is_null()),
        approvals_reviewer: payload
            .get("approvalsReviewer")
            .cloned()
            .filter(|value| !value.is_null()),
        personality: payload.get("personality").cloned().filter(|value| !value.is_null()),
        config: payload.get("config").cloned().filter(|value| !value.is_null()),
        base_instructions: payload
            .get("baseInstructions")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| resolve_role_instructions_for(role).ok().flatten()),
        developer_instructions: payload
            .get("developerInstructions")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| developer_instructions_for_role(state, role, Some(project_path), Some(cwd))),
        persist_extended_history: payload.get("persistExtendedHistory").and_then(Value::as_bool),
        service_name: payload
            .get("serviceName")
            .and_then(Value::as_str)
            .map(str::to_string),
        ephemeral: payload.get("ephemeral").and_then(Value::as_bool),
        dynamic_tools: payload.get("dynamicTools").cloned().filter(|value| !value.is_null()),
    }
}

fn explicit_thread_settings_for_existing_thread(
    state: &PersistedState,
    payload: &Value,
    thread_id: &str,
    role: Option<&str>,
    cwd: String,
    approval_policy: Option<String>,
    sandbox_mode: Option<String>,
) -> ExplicitThreadSettings {
    let project_path = tracked_project_path_for_thread(state, thread_id);
    ExplicitThreadSettings {
        cwd: cwd.clone(),
        approval_policy,
        sandbox_mode: sandbox_mode.clone(),
        network_access: effective_network_access_for_sandbox(
            sandbox_mode.as_deref(),
            payload.get("networkAccess").and_then(Value::as_bool),
            tracked_network_access_for_thread(state, thread_id),
        ),
        model: payload
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| tracked_model_for_thread(state, thread_id)),
        model_provider: payload
            .get("modelProvider")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| tracked_model_provider_for_thread(state, thread_id))
            .or_else(|| preferred_model_provider_for_project(state, project_path.as_deref())),
        reasoning_effort: payload
            .get("reasoningEffort")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| tracked_reasoning_effort_for_thread(state, thread_id)),
        service_tier: payload
            .get("serviceTier")
            .cloned()
            .filter(|value| !value.is_null())
            .or_else(|| tracked_service_tier_for_thread(state, thread_id)),
        approvals_reviewer: payload
            .get("approvalsReviewer")
            .cloned()
            .filter(|value| !value.is_null())
            .or_else(|| tracked_approvals_reviewer_for_thread(state, thread_id)),
        personality: payload
            .get("personality")
            .cloned()
            .filter(|value| !value.is_null())
            .or_else(|| tracked_personality_for_thread(state, thread_id)),
        config: payload
            .get("config")
            .cloned()
            .filter(|value| !value.is_null())
            .or_else(|| tracked_config_for_thread(state, thread_id)),
        base_instructions: payload
            .get("baseInstructions")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| tracked_base_instructions_for_thread(state, thread_id))
            .or_else(|| resolve_role_instructions_for(role).ok().flatten()),
        developer_instructions: payload
            .get("developerInstructions")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| tracked_developer_instructions_for_thread(state, thread_id))
            .or_else(|| developer_instructions_for_role(
                state,
                role,
                project_path.as_deref(),
                Some(cwd.as_str()),
            )),
        persist_extended_history: payload
            .get("persistExtendedHistory")
            .and_then(Value::as_bool)
            .or_else(|| tracked_persist_extended_history_for_thread(state, thread_id))
            .or(Some(true)),
        service_name: payload
            .get("serviceName")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| tracked_service_name_for_thread(state, thread_id)),
        ephemeral: payload
            .get("ephemeral")
            .and_then(Value::as_bool)
            .or_else(|| tracked_ephemeral_for_thread(state, thread_id)),
        dynamic_tools: payload
            .get("dynamicTools")
            .cloned()
            .filter(|value| !value.is_null())
            .or_else(|| tracked_dynamic_tools_for_thread(state, thread_id)),
    }
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

fn authoritative_spawn_defaults_for_project(
    state: &PersistedState,
    project_path: &str,
) -> Option<ExplicitThreadSettings> {
    let target_path = normalize_path(project_path.to_string());
    let project = state.projects.values().find(|project| {
        project
            .project_root
            .as_ref()
            .map(|value| normalize_path(value.clone()) == target_path)
            .unwrap_or(false)
    })?;

    let cwd = normalize_path(
        project
            .cwd
            .clone()
            .or_else(|| project.project_root.clone())
            .unwrap_or_default(),
    );
    let approval_policy = state
        .global_configs
        .get("approvalPolicy")
        .and_then(Value::as_str)
        .map(str::to_string);
    let sandbox_mode = state
        .global_configs
        .get("sandboxMode")
        .and_then(Value::as_str)
        .map(str::to_string);
    let default_network_access = state.global_configs.get("networkAccess").and_then(Value::as_bool);
    let network_access = effective_network_access_for_sandbox(
        sandbox_mode.as_deref(),
        None,
        default_network_access,
    );

    Some(ExplicitThreadSettings {
        cwd,
        approval_policy,
        sandbox_mode,
        network_access,
        model: None,
        model_provider: project.preferred_model_provider.clone(),
        reasoning_effort: None,
        service_tier: None,
        approvals_reviewer: None,
        personality: None,
        config: None,
        base_instructions: None,
        developer_instructions: None,
        persist_extended_history: Some(true),
        service_name: None,
        ephemeral: None,
        dynamic_tools: None,
    })
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
    simple_sandbox_policy(sandbox_mode, network_access, cwd)
}

fn role_default_model(state: &PersistedState, project_path: Option<&str>, role: Option<&str>) -> Option<String> {
    let key = match role {
        Some("designer") => "designer",
        Some("qa") => "qa",
        Some("orchestrator") => "orchestrator",
        Some("worker") | Some("hidden") | Some("operator") | _ => "worker",
    };
    let target_path = normalize_path(project_path?.to_string());
    let project = state.projects.values().find(|project| {
        project
            .project_root
            .as_ref()
            .map(|value| normalize_path(value.clone()) == target_path)
            .unwrap_or(false)
    })?;
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
        Some("designer") => "designer",
        Some("qa") => "qa",
        Some("orchestrator") => "orchestrator",
        Some("worker") | Some("hidden") | Some("operator") | _ => "worker",
    };
    let target_path = normalize_path(project_path?.to_string());
    let project = state.projects.values().find(|project| {
        project
            .project_root
            .as_ref()
            .map(|value| normalize_path(value.clone()) == target_path)
            .unwrap_or(false)
    })?;
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
    let normalized_project_path = project_path.map(|value| normalize_path(value.to_string()));
    let normalized_cwd = cwd.map(|value| normalize_path(value.to_string()));
    let project = state.projects.values().find(|project| {
        let project_root = project.project_root.as_ref().map(|value| normalize_path(value.clone()));
        project_root.as_ref() == normalized_project_path.as_ref()
            || project_root.as_ref() == normalized_cwd.as_ref()
    })?;
    let configured = project
        .configs
        .get("roleDeveloperInstructionsDefaults")
        .and_then(|value| value.get(role))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let mut segments = Vec::new();
    if let Some(configured) = configured {
        segments.push(configured);
    }
    if role != "orchestrator" && project.orchestrator_thread_id.is_some() {
        if role == "designer" {
            segments.push("Use the same communication rules as workers, but final assistant replies are not auto-forwarded for designers. If the administrator needs your final status, send it explicitly through the sanctioned Robdex path.".to_string());
        } else if project.auto_route_replies.unwrap_or(false) {
            segments.push("Final assistant replies are auto-forwarded to this project's orchestrator. Mid-turn messages and coordination are fine, but do not manually send a redundant final handoff when your turn ends unless you need to add distinct information.".to_string());
        } else {
            segments.push("Final assistant replies are not auto-forwarded. If the orchestrator needs your final status, use $robdex-orchestrator to send it manually.".to_string());
        }
        if role != "designer" && project.route_approval_requests.unwrap_or(false) {
            segments.push("Command and file-change approval requests are forwarded to this project's orchestrator so they can guide approval decisions in real time.".to_string());
        }
    }
    (!segments.is_empty()).then(|| segments.join("\n\n"))
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

fn payload_value_or_string(payload: &Value, field: &str, fallback: Option<String>) -> Option<Value> {
    payload
        .get(field)
        .cloned()
        .filter(|value| !value.is_null())
        .or_else(|| fallback.map(Value::String))
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

pub(crate) async fn send_thread_input(
    runtime: &BridgeRuntime,
    state: &PersistedState,
    thread_id: &str,
    text: Option<&str>,
    local_image_paths: &[String],
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<Value> {
    let input = build_user_input_payload(text, local_image_paths)?;
    if let Some(active_turn_id) = runtime.active_turn_id_for_thread(thread_id).await {
        let steer_result = app_server_request_json(
            runtime,
            "turn/steer",
            json!({
                "threadId": thread_id,
                "input": input,
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
    let params = AppServerTurnOverrides {
        cwd,
        approval_policy: approval_policy.map(Value::String),
        sandbox_policy,
        model: effective_model,
        effort: effective_effort,
        service_tier: tracked_service_tier_for_thread(state, thread_id),
        approvals_reviewer: tracked_approvals_reviewer_for_thread(state, thread_id),
        personality: tracked_personality_for_thread(state, thread_id),
        ..Default::default()
    }
    .turn_start_params(thread_id, input);
    let response = app_server_request_json(runtime, "turn/start", params).await?;
    Ok(response.get("turn").cloned().unwrap_or(response))
}

fn build_user_input_payload(text: Option<&str>, local_image_paths: &[String]) -> Result<Value> {
    let mut input = Vec::new();
    if let Some(text) = text.map(str::trim).filter(|value| !value.is_empty()) {
        input.push(json!({
            "type": "text",
            "text": text,
        }));
    }
    for path in local_image_paths {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            continue;
        }
        input.push(json!({
            "type": "localImage",
            "path": trimmed,
        }));
    }
    if input.is_empty() {
        anyhow::bail!("message must include text or images");
    }
    Ok(Value::Array(input))
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
            return effective_network_access_for_sandbox(
                sandbox_mode.as_deref(),
                agent.network_access,
                default_network_access,
            );
        }
    }
    effective_network_access_for_sandbox(default_sandbox_mode.as_deref(), None, default_network_access)
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

fn tracked_model_provider_for_thread(state: &PersistedState, thread_id: &str) -> Option<String> {
    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id) {
            return agent
                .model_provider
                .clone()
                .or_else(|| project.preferred_model_provider.clone());
        }
    }
    None
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

fn tracked_service_tier_for_thread(state: &PersistedState, thread_id: &str) -> Option<Value> {
    state
        .projects
        .values()
        .find_map(|project| project.agents.get(thread_id).and_then(|agent| agent.service_tier.clone()))
}

fn tracked_approvals_reviewer_for_thread(state: &PersistedState, thread_id: &str) -> Option<Value> {
    state.projects.values().find_map(|project| {
        project
            .agents
            .get(thread_id)
            .and_then(|agent| agent.approvals_reviewer.clone())
    })
}

fn tracked_personality_for_thread(state: &PersistedState, thread_id: &str) -> Option<Value> {
    state
        .projects
        .values()
        .find_map(|project| project.agents.get(thread_id).and_then(|agent| agent.personality.clone()))
}

fn tracked_config_for_thread(state: &PersistedState, thread_id: &str) -> Option<Value> {
    state
        .projects
        .values()
        .find_map(|project| project.agents.get(thread_id).and_then(|agent| agent.config.clone()))
}

fn tracked_base_instructions_for_thread(state: &PersistedState, thread_id: &str) -> Option<String> {
    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id)
            && agent.base_instructions.is_some()
        {
            return agent.base_instructions.clone();
        }
    }
    resolve_role_instructions_for(effective_role_for_thread(state, thread_id, None).as_deref()).ok().flatten()
}

fn tracked_developer_instructions_for_thread(state: &PersistedState, thread_id: &str) -> Option<String> {
    for project in state.projects.values() {
        if let Some(agent) = project.agents.get(thread_id)
            && agent.developer_instructions.is_some()
        {
            return agent.developer_instructions.clone();
        }
    }
    developer_instructions_for_role(
        state,
        effective_role_for_thread(state, thread_id, None).as_deref(),
        tracked_project_path_for_thread(state, thread_id).as_deref(),
        tracked_cwd_for_thread(state, thread_id).as_deref(),
    )
}

fn tracked_persist_extended_history_for_thread(state: &PersistedState, thread_id: &str) -> Option<bool> {
    state.projects.values().find_map(|project| {
        project
            .agents
            .get(thread_id)
            .and_then(|agent| agent.persist_extended_history)
    })
}

fn tracked_service_name_for_thread(state: &PersistedState, thread_id: &str) -> Option<String> {
    state
        .projects
        .values()
        .find_map(|project| project.agents.get(thread_id).and_then(|agent| agent.service_name.clone()))
}

fn tracked_ephemeral_for_thread(state: &PersistedState, thread_id: &str) -> Option<bool> {
    state
        .projects
        .values()
        .find_map(|project| project.agents.get(thread_id).and_then(|agent| agent.ephemeral))
}

fn tracked_dynamic_tools_for_thread(state: &PersistedState, thread_id: &str) -> Option<Value> {
    state
        .projects
        .values()
        .find_map(|project| project.agents.get(thread_id).and_then(|agent| agent.dynamic_tools.clone()))
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

pub(crate) async fn send_follow_up_message(
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
    let state = parse_state(&runtime.state_document_value().await);
    let role = payload.get("role").and_then(Value::as_str);
    let role_value = role.unwrap_or("worker").to_string();
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
    let display_name_value = display_name
        .map(str::to_string)
        .unwrap_or_else(|| role_value.clone());
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
    let settings = explicit_thread_settings_for_new_thread(
        &state,
        payload,
        &project_path,
        &cwd,
        role,
        approval_policy.clone(),
        sandbox_mode.clone(),
        network_access,
    );
    let (project_id, project_name) = project_identity_for_root(&state, &project_path);
    let hook_spawn_context = json!({
        "approvalPolicy": approval_policy,
        "sandboxMode": sandbox_mode,
        "networkAccess": network_access,
        "modelID": settings.model,
        "modelProvider": settings.model_provider,
        "reasoningEffort": settings.reasoning_effort,
        "serviceTier": payload.get("serviceTier").cloned().unwrap_or(Value::Null),
        "serviceName": payload.get("serviceName").cloned().unwrap_or(Value::Null),
        "ephemeral": payload.get("ephemeral").cloned().unwrap_or(Value::Null),
    });
    let hook_result = match role {
        Some("worker") => {
            maybe_run_project_hook(
                &project_path,
                HookEvent::WorkerCreate,
                worker_create_payload(
                    None,
                    &project_id,
                    &project_name,
                    &project_path,
                    &display_name_value,
                    &role_value,
                    &cwd,
                    payload.get("parentAgentId").and_then(Value::as_str),
                    hook_spawn_context.clone(),
                ),
            )
            .await
        }
        Some("qa") => {
            maybe_run_project_hook(
                &project_path,
                HookEvent::QaCreate,
                qa_create_payload(
                    None,
                    &project_id,
                    &project_name,
                    &project_path,
                    &display_name_value,
                    &role_value,
                    &cwd,
                    payload.get("parentAgentId").and_then(Value::as_str),
                    hook_spawn_context.clone(),
                ),
            )
            .await
        }
        _ => Default::default(),
    };
    let params = settings.to_app_server_thread_overrides().thread_start_params();
    let result = match app_server_request_json(runtime, "thread/start", params).await {
        Ok(result) => result,
        Err(error) => {
            if let Some(hook) = hook_result.result.as_ref() {
                if let Some(telemetry) = maybe_compensate_spawn_hook_resources(
                    &project_path,
                    &project_id,
                    &project_name,
                    &display_name_value,
                    &role_value,
                    &cwd,
                    hook,
                )
                .await
                {
                    let _state_guard = runtime.lock_state_mutation().await;
                    let mut state = parse_state(&runtime.state_document_value().await);
                    record_project_hook_telemetry(
                        &mut state,
                        &project_path,
                        None,
                        &display_name_value,
                        &role_value,
                        &telemetry,
                    );
                    persist_state(runtime, &state).await?;
                    runtime
                        .push_event(crate::models::BridgeEvent::HookFailure {
                            payload: hook_failure_notice(
                                &project_id,
                                &project_name,
                                None,
                                &display_name_value,
                                &role_value,
                                &telemetry,
                            ),
                        })
                        .await;
                }
            }
            return Err(error);
        }
    };
    let thread = result
        .get("thread")
        .cloned()
        .unwrap_or(result.clone());
    let thread_id = thread
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("thread/start response missing thread.id"));
    let thread_id = match thread_id {
        Ok(thread_id) => thread_id.to_string(),
        Err(error) => {
            if let Some(hook) = hook_result.result.as_ref() {
                if let Some(telemetry) = maybe_compensate_spawn_hook_resources(
                    &project_path,
                    &project_id,
                    &project_name,
                    &display_name_value,
                    &role_value,
                    &cwd,
                    hook,
                )
                .await
                {
                    let _state_guard = runtime.lock_state_mutation().await;
                    let mut state = parse_state(&runtime.state_document_value().await);
                    record_project_hook_telemetry(
                        &mut state,
                        &project_path,
                        None,
                        &display_name_value,
                        &role_value,
                        &telemetry,
                    );
                    persist_state(runtime, &state).await?;
                    runtime
                        .push_event(crate::models::BridgeEvent::HookFailure {
                            payload: hook_failure_notice(
                                &project_id,
                                &project_name,
                                None,
                                &display_name_value,
                                &role_value,
                                &telemetry,
                            ),
                        })
                        .await;
                }
            }
            return Err(error);
        }
    };
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
    let _state_guard = runtime.lock_state_mutation().await;
    let mut state = parse_state(&runtime.state_document_value().await);
    register_tracked_thread(
        &mut state,
        &settings.to_registration_payload(thread, &project_path, &role_value),
    )?;
    if let Some(display_name) = display_name {
        set_tracked_thread_display_name(&mut state, &thread_id, display_name);
    }
    if let Some(hook_result) = hook_result.result.as_ref() {
        persist_agent_hook_state(&mut state, &thread_id, hook_result);
    }
    if let Some(telemetry) = hook_result.telemetry.as_ref() {
        persist_agent_hook_telemetry(&mut state, &thread_id, telemetry);
        record_project_hook_telemetry(
            &mut state,
            &project_path,
            Some(&thread_id),
            &display_name_value,
            &role_value,
            telemetry,
        );
    }
    persist_state(runtime, &state).await?;
    if let Some(telemetry) = hook_result.telemetry.as_ref() {
        runtime
            .push_event(crate::models::BridgeEvent::HookFailure {
                payload: hook_failure_notice(
                    &project_id,
                    &project_name,
                    Some(&thread_id),
                    &display_name_value,
                    &role_value,
                    telemetry,
                ),
            })
            .await;
    }

    let initial_prompt = payload
        .get("initialPrompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let prompt_to_send = match (initial_prompt, hook_result.result.as_ref()) {
        (Some(prompt), Some(hook)) => {
            let appended = append_prompt_segments(&prompt, &hook.prompt_append);
            (!appended.trim().is_empty()).then_some(appended)
        }
        (Some(prompt), None) => Some(prompt),
        (None, Some(hook)) => {
            let appended = append_prompt_segments("", &hook.prompt_append);
            (!appended.trim().is_empty()).then_some(appended)
        }
        (None, None) => None,
    };
    if let Some(prompt) = prompt_to_send {
        let _ = send_thread_input(runtime, &state, &thread_id, Some(&prompt), &[], None, None)
            .await?;
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
        "modelProvider": payload.get("modelProvider").cloned().unwrap_or_else(|| project.preferred_model_provider.clone().map(Value::String).unwrap_or(Value::Null)),
        "reasoningEffort": payload.get("reasoningEffort").cloned().unwrap_or(Value::Null),
        "serviceTier": payload.get("serviceTier").cloned().unwrap_or(Value::Null),
        "approvalsReviewer": payload.get("approvalsReviewer").cloned().unwrap_or(Value::Null),
        "personality": payload.get("personality").cloned().unwrap_or(Value::Null),
        "config": payload.get("config").cloned().unwrap_or(Value::Null),
        "baseInstructions": payload.get("baseInstructions").cloned().unwrap_or(Value::Null),
        "developerInstructions": payload.get("developerInstructions").cloned().unwrap_or(Value::Null),
        "persistExtendedHistory": payload.get("persistExtendedHistory").cloned().unwrap_or(Value::Null),
        "serviceName": payload.get("serviceName").cloned().unwrap_or(Value::Null),
        "ephemeral": payload.get("ephemeral").cloned().unwrap_or(Value::Null),
        "dynamicTools": payload.get("dynamicTools").cloned().unwrap_or(Value::Null),
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
    let text = payload
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default();
    let local_image_paths = payload
        .get("localImagePaths")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let result = send_thread_input(
        runtime,
        &state,
        &thread_id,
        Some(text.as_str()),
        &local_image_paths,
        payload.get("modelID").and_then(Value::as_str),
        payload.get("reasoningEffort").and_then(Value::as_str),
    )
    .await?;
    runtime
        .append_local_user_message(&thread_id, &text, &local_image_paths)
        .await?;
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
    let state = parse_state(&runtime.state_document_value().await);
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

fn project_identity_for_root(state: &PersistedState, project_root: &str) -> (String, String) {
    state
        .projects
        .iter()
        .find_map(|(key, project)| {
            (project.project_root.as_deref() == Some(project_root)).then(|| {
                (
                    project.id.clone().unwrap_or_else(|| key.clone()),
                    project
                        .name
                        .clone()
                        .unwrap_or_else(|| project_root.to_string()),
                )
            })
        })
        .unwrap_or_else(|| (project_root.to_string(), project_root.to_string()))
}

fn agent_state_for_thread_mut<'a>(
    state: &'a mut PersistedState,
    thread_id: &str,
) -> Option<&'a mut PersistedAgentState> {
    state
        .projects
        .values_mut()
        .find_map(|project| project.agents.get_mut(thread_id))
}

fn persisted_agent_hook_state(state: &PersistedState, thread_id: &str) -> Option<Value> {
    agent_state_for_thread(state, thread_id)
        .and_then(|agent| agent.extras.get(HOOK_LIFECYCLE_STATE_KEY).cloned())
}

fn persisted_live_processes(state: &PersistedState, thread_id: &str) -> Vec<LiveProcessRecord> {
    agent_state_for_thread(state, thread_id)
        .and_then(|agent| agent.extras.get(LIVE_PROCESSES_KEY).cloned())
        .and_then(|value| serde_json::from_value::<Vec<LiveProcessRecord>>(value).ok())
        .unwrap_or_default()
}

fn persist_live_processes(state: &mut PersistedState, thread_id: &str, processes: &[LiveProcessRecord]) {
    if let Some(agent) = agent_state_for_thread_mut(state, thread_id) {
        agent.extras.insert(
            LIVE_PROCESSES_KEY.to_string(),
            serde_json::to_value(processes).unwrap_or_else(|_| Value::Array(Vec::new())),
        );
    }
}

fn prune_dead_live_processes_for_thread(state: &mut PersistedState, thread_id: &str) -> bool {
    let mut processes = persisted_live_processes(state, thread_id);
    let before = processes.len();
    processes.retain(live_process_is_alive);
    if processes.len() == before {
        return false;
    }
    persist_live_processes(state, thread_id, &processes);
    state.updated_at = Some(unix_now());
    true
}

pub(crate) fn increment_compaction_count(
    state: &mut PersistedState,
    thread_id: &str,
) -> Option<CompactionState> {
    let agent = agent_state_for_thread_mut(state, thread_id)?;
    let mut current = agent
        .extras
        .get(COMPACTION_STATE_KEY)
        .cloned()
        .and_then(|value| serde_json::from_value::<CompactionState>(value).ok())
        .unwrap_or_default();
    current.count = current.count.saturating_add(1);
    current.last_compacted_at = Some(unix_now());
    agent.extras.insert(
        COMPACTION_STATE_KEY.to_string(),
        serde_json::to_value(&current).unwrap_or_else(|_| Value::Null),
    );
    Some(current)
}

pub(crate) async fn run_compaction_hook_best_effort(
    runtime: &BridgeRuntime,
    thread_id: &str,
    compaction: &CompactionState,
) -> Result<()> {
    let state_value = runtime.state_document_value().await;
    let mut state = parse_state(&state_value);
    let Some(agent) = agent_state_for_thread(&state, thread_id).cloned() else {
        return Ok(());
    };
    let project_root = agent.project_root.clone().unwrap_or_default();
    if project_root.trim().is_empty() {
        return Ok(());
    }
    let (project_id, project_name) = project_identity_for_root(&state, &project_root);
    let agent_name = agent.display_name.clone().unwrap_or_else(|| thread_id.to_string());
    let role = agent.role.clone().unwrap_or_else(|| "worker".to_string());
    let payload = compaction_payload(
        thread_id,
        &project_id,
        &project_name,
        &project_root,
        &agent_name,
        &role,
        agent.cwd.as_deref(),
        compaction.count,
    );
    let invocation = maybe_run_project_hook(&project_root, HookEvent::Compaction, payload).await;
    if let Some(telemetry) = invocation.telemetry {
        persist_agent_hook_telemetry(&mut state, thread_id, &telemetry);
        record_project_hook_telemetry(
            &mut state,
            &project_root,
            Some(thread_id),
            &agent_name,
            &role,
            &telemetry,
        );
        persist_state(runtime, &state).await?;
        runtime
            .push_event(crate::models::BridgeEvent::HookFailure {
                payload: hook_failure_notice(
                    &project_id,
                    &project_name,
                    Some(thread_id),
                    &agent_name,
                    &role,
                    &telemetry,
                ),
            })
            .await;
    }
    Ok(())
}

fn persist_agent_hook_state(state: &mut PersistedState, thread_id: &str, hook_result: &HookResult) {
    if let Some(agent) = agent_state_for_thread_mut(state, thread_id) {
        agent.extras.insert(
            HOOK_LIFECYCLE_STATE_KEY.to_string(),
            serde_json::to_value(HookLifecycleState::from_hook_result(hook_result))
                .unwrap_or_else(|_| Value::Null),
        );
    }
}

fn persist_agent_hook_telemetry(state: &mut PersistedState, thread_id: &str, telemetry: &HookTelemetry) {
    if let Some(agent) = agent_state_for_thread_mut(state, thread_id) {
        agent.extras.insert(
            HOOK_TELEMETRY_KEY.to_string(),
            serde_json::to_value(telemetry).unwrap_or_else(|_| Value::Null),
        );
    }
}

fn record_project_hook_telemetry(
    state: &mut PersistedState,
    project_root: &str,
    thread_id: Option<&str>,
    agent_name: &str,
    role: &str,
    telemetry: &HookTelemetry,
) {
    let Some(project) = state
        .projects
        .values_mut()
        .find(|project| project.project_root.as_deref() == Some(project_root))
    else {
        return;
    };
    let entry = json!({
        "createdAt": unix_now(),
        "threadId": thread_id,
        "agentName": agent_name,
        "role": role,
        "event": telemetry.event,
        "status": telemetry.status,
        "detail": telemetry.detail,
    });
    let recent = project
        .extras
        .entry(PROJECT_HOOK_TELEMETRY_KEY.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    match recent {
        Value::Array(items) => {
            items.insert(0, entry);
            if items.len() > 20 {
                items.truncate(20);
            }
        }
        other => {
            *other = Value::Array(vec![entry]);
        }
    }
}

fn hook_failure_notice(
    project_id: &str,
    project_name: &str,
    thread_id: Option<&str>,
    agent_name: &str,
    role: &str,
    telemetry: &HookTelemetry,
) -> HookFailureNotice {
    HookFailureNotice {
        project_id: project_id.to_string(),
        project_name: project_name.to_string(),
        thread_id: thread_id.map(str::to_string),
        agent_name: agent_name.to_string(),
        role: role.to_string(),
        event: telemetry.event.clone(),
        status: telemetry.status.clone(),
        detail: telemetry.detail.clone().unwrap_or_default(),
    }
}

async fn maybe_compensate_spawn_hook_resources(
    project_root: &str,
    project_id: &str,
    project_name: &str,
    agent_name: &str,
    role: &str,
    requested_cwd: &str,
    hook_result: &HookResult,
) -> Option<HookTelemetry> {
    let lifecycle = serde_json::to_value(HookLifecycleState::from_hook_result(hook_result)).ok();
    let event = match role {
        "qa" => HookEvent::QaArchive,
        _ => HookEvent::WorkerArchive,
    };
    let payload = if role == "qa" {
        qa_archive_payload(
            "spawn-failed",
            project_id,
            project_name,
            project_root,
            agent_name,
            role,
            Some(requested_cwd),
            lifecycle,
        )
    } else {
        worker_archive_payload(
            "spawn-failed",
            project_id,
            project_name,
            project_root,
            agent_name,
            role,
            Some(requested_cwd),
            lifecycle,
        )
    };
    let outcome = maybe_run_project_hook(project_root, event, payload).await;
    let _ = (project_id, project_name, agent_name, role);
    outcome.telemetry
}

async fn release_qa_harness_leases_for_thread(
    runtime: &BridgeRuntime,
    project_root: &str,
    thread_id: &str,
) -> Result<()> {
    let client = reqwest::Client::new();
    let base_url = runtime.settings().qa_harness_url.trim_end_matches('/').to_string();
    let projects: Vec<Value> = client
        .get(format!("{base_url}/projects"))
        .send()
        .await
        .context("qa harness projects request failed")?
        .json()
        .await
        .context("qa harness projects decode failed")?;
    let normalized_project_root = normalize_harness_project_root(project_root);
    let Some(project_id) = projects.into_iter().find_map(|project| {
        let repo_root = project.get("repo_root")?.as_str()?;
        (normalize_harness_project_root(repo_root) == normalized_project_root)
            .then(|| project.get("id")?.as_str().map(str::to_string))
            .flatten()
    }) else {
        return Ok(());
    };

    let devices: Vec<Value> = client
        .get(format!("{base_url}/projects/{project_id}/devices"))
        .send()
        .await
        .with_context(|| format!("qa harness devices request failed for project {project_id}"))?
        .json()
        .await
        .with_context(|| format!("qa harness devices decode failed for project {project_id}"))?;

    for device in devices {
        let lease_owner = device
            .get("state")
            .and_then(|value| value.get("lease"))
            .and_then(|value| value.get("owner"))
            .and_then(Value::as_str);
        if lease_owner != Some(thread_id) {
            continue;
        }
        let Some(device_key) = device.get("device_key").and_then(Value::as_str) else {
            continue;
        };
        let response = client
            .delete(format!("{base_url}/projects/{project_id}/devices/{device_key}/lease"))
            .send()
            .await
            .with_context(|| format!("qa harness release lease request failed for {project_id}/{device_key}"))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("qa harness release lease returned {status} for {project_id}/{device_key}; body={body}");
        }
    }

    Ok(())
}

fn normalize_harness_project_root(root: &str) -> String {
    let path = Path::new(root);
    if path.file_name().and_then(|name| name.to_str()) == Some(".worktrees") {
        if let Some(parent) = path.parent() {
            return parent.to_string_lossy().into_owned();
        }
    }
    root.to_string()
}

pub(crate) async fn register_live_process(
    runtime: &BridgeRuntime,
    thread_id: &str,
    process: LiveProcessRecord,
) -> Result<()> {
    let state = parse_state(&runtime.state_document_value().await);
    if agent_state_for_thread(&state, thread_id).is_none() {
        bail!("Unknown thread `{thread_id}`");
    }
    runtime.register_live_process(thread_id, process).await;
    Ok(())
}

pub(crate) async fn complete_live_process(
    runtime: &BridgeRuntime,
    thread_id: &str,
    process_id: &str,
) -> Result<()> {
    let state = parse_state(&runtime.state_document_value().await);
    if agent_state_for_thread(&state, thread_id).is_none() {
        return Ok(());
    }
    let _ = runtime.complete_live_process(thread_id, process_id).await;
    Ok(())
}

async fn terminate_live_process(
    runtime: &BridgeRuntime,
    thread_id: &str,
    process_id: &str,
) -> Result<bool> {
    let Some(process) = runtime.live_process(thread_id, process_id).await else {
        return Ok(false);
    };

    let target = process
        .process_group_id
        .filter(|pgid| *pgid > 0)
        .map(|pgid| -(pgid as libc::pid_t))
        .unwrap_or(process.pid as libc::pid_t);
    let rc = unsafe { libc::kill(target, libc::SIGTERM) };
    if rc == 0 {
        return Ok(true);
    }

    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        complete_live_process(runtime, thread_id, process_id).await?;
        return Ok(true);
    }

    Err(error).with_context(|| {
        format!(
            "failed to terminate process {} (pid {}, pgid {:?}) for thread {thread_id}",
            process.process_id, process.pid, process.process_group_id
        )
    })
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
    let result = send_thread_input(
        runtime,
        &state,
        &recipient.thread_id,
        Some(&normalized_text),
        &[],
        None,
        None,
    )
    .await?;
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
    // Important policy boundary:
    // - this path is for orchestrator-as-agent subordinate spawns only
    // - it is not the administrator thread-start surface
    // - project/global bridge state is authoritative for cwd/approval/sandbox/network here
    // - the orchestrator may only choose the subordinate role and display name/prompt
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
    let target_role = match role.unwrap_or("worker") {
        "worker" => "worker",
        "qa" => "qa",
        other => bail!("Orchestrators may only spawn worker or qa agents, not `{other}`."),
    };
    let authoritative =
        authoritative_spawn_defaults_for_project(&state, sender.project_path.as_str()).ok_or_else(
            || anyhow::anyhow!("Project `{}` has no authoritative spawn defaults.", sender.project_path),
        )?;
    let model =
        role_default_model(&state, Some(sender.project_path.as_str()), Some(target_role));
    let reasoning_effort =
        role_default_reasoning_effort(&state, Some(sender.project_path.as_str()), Some(target_role));
    let base_instructions = resolve_role_instructions_for(Some(target_role)).ok().flatten();
    let developer_instructions = developer_instructions_for_role(
        &state,
        Some(target_role),
        Some(sender.project_path.as_str()),
        Some(authoritative.cwd.as_str()),
    )
    .filter(|value| !value.trim().is_empty());
    let payload = json!({
        "displayName": name,
        "initialPrompt": prompt,
        "cwd": authoritative.cwd,
        "projectPath": sender.project_path,
        "role": target_role,
        "parentAgentId": sender_thread_id,
        "approvalPolicy": authoritative.approval_policy,
        "sandboxMode": authoritative.sandbox_mode,
        "networkAccess": authoritative.network_access,
        "modelID": model,
        "modelProvider": authoritative.model_provider,
        "reasoningEffort": reasoning_effort,
        "serviceTier": authoritative.service_tier,
        "approvalsReviewer": authoritative.approvals_reviewer,
        "personality": authoritative.personality,
        "config": authoritative.config,
        "baseInstructions": base_instructions,
        "developerInstructions": developer_instructions,
        "persistExtendedHistory": authoritative.persist_extended_history,
        "serviceName": authoritative.service_name,
        "ephemeral": authoritative.ephemeral,
        "dynamicTools": authoritative.dynamic_tools,
    });
    let mut agent = spawn_agent(runtime, &payload).await?;
    if let Some(issue_number) = issue_number {
        let _state_guard = runtime.lock_state_mutation().await;
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
    let self_handoff_allowed = sender.thread_id == recipient.thread_id
        && sender.project_path == recipient.project_path
        && matches!(sender.role.as_str(), "orchestrator" | "operator" | "hidden" | "designer");
    let project_orchestrator_handoff_allowed =
        sender.is_orchestrator && sender.project_path == recipient.project_path;
    if !self_handoff_allowed && !project_orchestrator_handoff_allowed {
        bail!(
            "Only the configured orchestrator thread for project `{}` can warm handoff agents in that project, except self-handoff for orchestrator, operator, hidden, and designer threads.",
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
        "approvalPolicy": recipient_state.approval_policy.clone(),
        "sandboxMode": recipient_state.sandbox_mode.clone(),
        "networkAccess": recipient_state.network_access,
        "modelID": recipient_state.model.clone(),
        "modelProvider": recipient_state.model_provider.clone(),
        "reasoningEffort": recipient_state.reasoning_effort.clone(),
        "serviceTier": recipient_state.service_tier.clone(),
        "approvalsReviewer": recipient_state.approvals_reviewer.clone(),
        "personality": recipient_state.personality.clone(),
        "config": recipient_state.config.clone(),
        "baseInstructions": recipient_state.base_instructions.clone(),
        "developerInstructions": recipient_state.developer_instructions.clone(),
        "persistExtendedHistory": recipient_state.persist_extended_history,
        "serviceName": recipient_state.service_name.clone(),
        "ephemeral": recipient_state.ephemeral,
        "dynamicTools": recipient_state.dynamic_tools.clone(),
        "parentAgentId": sender_thread_id,
    });
    let mut replacement = spawn_agent(runtime, &spawn_payload).await?;

    let mut next_state = parse_state(&runtime.state_document_value().await);
    let mut old_was_project_orchestrator = false;
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
        old_was_project_orchestrator =
            project.orchestrator_thread_id.as_deref() == Some(recipient.thread_id.as_str());
        for group in &mut project.thread_groups {
            if group.thread_ids.iter().any(|value| value == &recipient.thread_id)
                && !group.thread_ids.iter().any(|value| value == &replacement.id)
            {
                group.thread_ids.push(replacement.id.clone());
            }
        }
        project.updated_at = Some(unix_now());
    }
    let pruned_old = prune_archived_thread_locally(&mut next_state, &recipient.thread_id);
    if old_was_project_orchestrator {
        for project in next_state.projects.values_mut() {
            if project.agents.contains_key(&replacement.id) {
                project.orchestrator_thread_id = Some(replacement.id.clone());
                project.updated_at = Some(unix_now());
                break;
            }
        }
    }
    next_state.updated_at = Some(unix_now());
    persist_state(runtime, &next_state).await?;
    if pruned_old {
        runtime.prune_thread_local(&recipient.thread_id).await?;
    }
    let _ = app_server_request_json(
        runtime,
        "thread/archive",
        json!({"threadId": recipient.thread_id}),
    )
    .await;
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
    let hook_context = agent_state_for_thread(&state, thread_id).and_then(|agent| {
        matches!(agent.role.as_deref(), Some("worker") | Some("qa")).then(|| {
            Some((
                agent.project_root.clone()?,
                agent
                    .display_name
                    .clone()
                    .unwrap_or_else(|| thread_id.to_string()),
                agent.role.clone().unwrap_or_else(|| "worker".to_string()),
                agent.cwd.clone(),
                persisted_agent_hook_state(&state, thread_id),
            ))
        })?
    });
    if let Some((project_root, agent_name, role, agent_cwd, lifecycle)) = hook_context {
        let (project_id, project_name) = project_identity_for_root(&state, &project_root);
        let (event, payload) = if role == "qa" {
            (
                HookEvent::QaArchive,
                qa_archive_payload(
                    thread_id,
                    &project_id,
                    &project_name,
                    &project_root,
                    &agent_name,
                    &role,
                    agent_cwd.as_deref(),
                    lifecycle,
                ),
            )
        } else {
            (
                HookEvent::WorkerArchive,
                worker_archive_payload(
                    thread_id,
                    &project_id,
                    &project_name,
                    &project_root,
                    &agent_name,
                    &role,
                    agent_cwd.as_deref(),
                    lifecycle,
                ),
            )
        };
        let hook_outcome = maybe_run_project_hook(&project_root, event, payload).await;
        if let Some(telemetry) = hook_outcome.telemetry.as_ref() {
            record_project_hook_telemetry(
                &mut state,
                &project_root,
                Some(thread_id),
                &agent_name,
                &role,
                telemetry,
            );
            runtime
                .push_event(crate::models::BridgeEvent::HookFailure {
                    payload: hook_failure_notice(
                        &project_id,
                        &project_name,
                        Some(thread_id),
                        &agent_name,
                        &role,
                        telemetry,
                    ),
                })
                .await;
        }
        if role == "qa" {
            if let Err(error) = release_qa_harness_leases_for_thread(runtime, &project_root, thread_id).await {
                let telemetry = HookTelemetry {
                    event: HookEvent::QaArchive.wire_name().to_string(),
                    status: "failed".to_string(),
                    detail: Some(format!("qa harness lease release failed: {error}")),
                };
                record_project_hook_telemetry(
                    &mut state,
                    &project_root,
                    Some(thread_id),
                    &agent_name,
                    &role,
                    &telemetry,
                );
                runtime
                    .push_event(crate::models::BridgeEvent::HookFailure {
                        payload: hook_failure_notice(
                            &project_id,
                            &project_name,
                            Some(thread_id),
                            &agent_name,
                            &role,
                            &telemetry,
                        ),
                    })
                    .await;
            }
        }
    }
    let pruned = prune_archived_thread_locally(&mut state, thread_id);
    if pruned {
        persist_state(runtime, &state).await?;
        runtime.prune_thread_local(thread_id).await?;
    }
    if runtime.info().await.connection_status != "connected" {
        return Ok(());
    }
    if let Err(error) = app_server_request_json(runtime, "thread/archive", json!({"threadId": thread_id})).await {
        let message = error.to_string();
        if !message.contains("no rollout found for thread id") && !message.contains("\"code\": -32600") {
            return Err(error);
        }
    }
    Ok(())
}

pub(crate) fn prune_archived_thread_locally(state: &mut PersistedState, thread_id: &str) -> bool {
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
    runtime.clear_pending_approval(&approval.id).await;
    runtime
        .maybe_run_approval_resolved_hook(&approval, decision, follow_up, Some(sender_thread_id))
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


fn uuid() -> String {
    format!(
        "stub-{}-{}",
        unix_now(),
        std::process::id()
    )
}

pub(crate) fn unix_now() -> u64 {
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
        CommandExecutionRequestApprovalParams, JSONRPCMessage, JSONRPCNotification, JSONRPCRequest, JSONRPCResponse,
        RequestId, ServerNotification, ServerRequest, Turn, TurnCompletedNotification, TurnStartedNotification,
        TurnStatus,
    };
    use codex_backend_core::HttpArgs;
    use futures_util::{SinkExt, StreamExt};
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        os::unix::fs::PermissionsExt,
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
        project_beta.agents.insert(
            "qa-b".to_string(),
            PersistedAgentState {
                display_name: Some("QA B".to_string()),
                role: Some("qa".to_string()),
                project_root: Some("/beta".to_string()),
                ..Default::default()
            },
        );
        project_alpha.agents.insert(
            "hidden-a".to_string(),
            PersistedAgentState {
                display_name: Some("Hidden A".to_string()),
                role: Some("hidden".to_string()),
                project_root: Some("/alpha".to_string()),
                ..Default::default()
            },
        );

        state.projects.insert("alpha".to_string(), project_alpha);
        state.projects.insert("beta".to_string(), project_beta);
        state
    }

    fn scoped_ids(scoped: &ScopedContext) -> Vec<&str> {
        scoped
            .visible
            .iter()
            .map(|record| record.thread_id.as_str())
            .collect::<Vec<_>>()
    }

    #[test]
    fn increment_compaction_count_persists_thread_state() {
        let mut state = sample_state();
        let first = increment_compaction_count(&mut state, "worker-a").expect("first compaction");
        assert_eq!(first.count, 1);
        assert!(first.last_compacted_at.is_some());

        let second = increment_compaction_count(&mut state, "worker-a").expect("second compaction");
        assert_eq!(second.count, 2);
        assert!(second.last_compacted_at.is_some());

        let persisted = state
            .projects
            .values()
            .find_map(|project| project.agents.get("worker-a"))
            .and_then(|agent| agent.extras.get(COMPACTION_STATE_KEY))
            .cloned()
            .expect("persisted compaction state");
        assert_eq!(persisted["count"], 2);
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
        assert!(!ids.contains(&"qa-b"));
        assert!(!ids.contains(&"hidden-a"));
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
        assert!(!ids.contains(&"qa-b"));
        assert!(!ids.contains(&"hidden-a"));
    }

    #[test]
    fn qa_sees_same_project_orchestrator_operator_worker_and_qa_only() {
        let state = sample_state();
        let records = all_agent_records(&state, &["qa-a".to_string()]);
        let scoped = scoped_agent_context(&records, "qa-a", false).expect("scoped");
        let ids = scoped_ids(&scoped);
        assert!(ids.contains(&"orch-a"));
        assert!(ids.contains(&"worker-a"));
        assert!(ids.contains(&"qa-a"));
        assert!(ids.contains(&"operator-a"));
        assert!(!ids.contains(&"orch-b"));
        assert!(!ids.contains(&"operator-b"));
        assert!(!ids.contains(&"worker-b"));
        assert!(!ids.contains(&"qa-b"));
        assert!(!ids.contains(&"hidden-a"));
    }

    #[test]
    fn operator_sees_same_project_workers_and_qa_plus_cross_project_orchestrators_and_operators() {
        let state = sample_state();
        let records = all_agent_records(&state, &["operator-a".to_string()]);
        let scoped = scoped_agent_context(&records, "operator-a", false).expect("scoped");
        let ids = scoped_ids(&scoped);
        assert!(ids.contains(&"orch-a"));
        assert!(ids.contains(&"worker-a"));
        assert!(ids.contains(&"qa-a"));
        assert!(ids.contains(&"operator-a"));
        assert!(ids.contains(&"orch-b"));
        assert!(ids.contains(&"operator-b"));
        assert!(!ids.contains(&"worker-b"));
        assert!(!ids.contains(&"qa-b"));
        assert!(!ids.contains(&"hidden-a"));
    }

    #[test]
    fn hidden_sender_cannot_use_scoped_communication_surface() {
        let state = sample_state();
        let records = all_agent_records(&state, &[]);
        let error = scoped_agent_context(&records, "hidden-a", false).expect_err("hidden sender should fail");
        assert!(error.to_string().contains("hidden from orchestrator communication"));
    }

    #[test]
    fn worker_cannot_resolve_cross_project_recipients() {
        let state = sample_state();
        let records = all_agent_records(&state, &[]);
        let scoped = scoped_agent_context(&records, "worker-a", false).expect("scoped");
        let error = resolve_scoped_recipient(&scoped.visible, Some("worker-b"), None, None)
            .expect_err("worker should not resolve cross-project worker");
        assert!(error.to_string().contains("not visible"));
        let error = resolve_scoped_recipient(&scoped.visible, Some("orch-b"), None, None)
            .expect_err("worker should not resolve cross-project orchestrator");
        assert!(error.to_string().contains("not visible"));
    }

    #[test]
    fn operator_can_resolve_cross_project_orchestrator_and_operator_but_not_cross_project_workers() {
        let state = sample_state();
        let records = all_agent_records(&state, &[]);
        let scoped = scoped_agent_context(&records, "operator-a", false).expect("scoped");
        let orch = resolve_scoped_recipient(&scoped.visible, Some("orch-b"), None, None)
            .expect("operator should resolve cross-project orchestrator");
        assert_eq!(orch.thread_id, "orch-b");
        let operator = resolve_scoped_recipient(&scoped.visible, Some("operator-b"), None, None)
            .expect("operator should resolve cross-project operator");
        assert_eq!(operator.thread_id, "operator-b");
        let error = resolve_scoped_recipient(&scoped.visible, Some("worker-b"), None, None)
            .expect_err("operator should not resolve cross-project worker");
        assert!(error.to_string().contains("not visible"));
    }

    #[tokio::test]
    async fn list_agents_without_sender_returns_all_tracked_agents_including_hidden() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:9".to_string()))
            .await
            .expect("runtime");
        runtime
            .persist_state_document(serde_json::to_value(sample_state()).expect("state json"))
            .await
            .expect("persist state");

        let outcome = execute_bridge_command(&runtime, "listAgents", json!({}))
            .await
            .expect("listAgents");
        let payload = outcome.payload["payload"]
            .as_array()
            .expect("agents payload");
        let ids = payload
            .iter()
            .filter_map(|agent| agent.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert!(ids.contains(&"orch-a"));
        assert!(ids.contains(&"worker-a"));
        assert!(ids.contains(&"qa-a"));
        assert!(ids.contains(&"operator-a"));
        assert!(ids.contains(&"hidden-a"));
        assert!(ids.contains(&"orch-b"));
        assert!(ids.contains(&"operator-b"));
        assert!(ids.contains(&"worker-b"));
        assert!(ids.contains(&"qa-b"));
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

    fn sample_settings(root: &TempDir, app_server_url: String) -> BridgeSettings {
        BridgeSettings {
            http: HttpArgs {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 42080,
            },
            app_server_url,
            qa_harness_url: "http://127.0.0.1:8775".to_string(),
            project_path: root.path().to_path_buf(),
            cwd: root.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(root.path()).join("state")),
        }
    }

    fn write_executable(path: &std::path::Path, content: &str) {
        std::fs::write(path, content).expect("write file");
        let mut perms = std::fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).expect("set permissions");
    }

    fn write_project_hook(temp: &TempDir, event: &str, script_name: &str, script_body: &str) {
        let config_dir = temp.path().join(".codex");
        let hooks_dir = config_dir.join("hooks");
        std::fs::create_dir_all(&hooks_dir).expect("mkdirs");
        write_executable(&hooks_dir.join(script_name), script_body);
        std::fs::write(
            config_dir.join("robdex-hooks.json"),
            serde_json::to_string(&json!({
                "version": 1,
                "hooks": {
                    event: format!("./.codex/hooks/{script_name}")
                }
            }))
            .expect("serialize hook config"),
        )
        .expect("write hook config");
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
        let transport = runtime.spawn_transport();

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
        transport.abort();
    }

    #[tokio::test]
    async fn send_agent_input_uses_turn_steer_with_active_turn() {
        let temp = TempDir::new().expect("tempdir");
        let (request_tx, request_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            let mut request_tx = Some(request_tx);
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

                loop {
                    let next = ws.next().await.expect("request").expect("request frame");
                    let text = match next {
                        Message::Text(text) => text,
                        other => panic!("unexpected request frame: {other:?}"),
                    };
                    let request = match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                        JSONRPCMessage::Request(request) => request,
                        other => panic!("unexpected request message: {other:?}"),
                    };
                    let result = if request.method == "turn/steer" {
                        request_tx
                            .take()
                            .expect("request sender available")
                            .send(request.clone())
                            .expect("record request");
                        json!({"turnId":"turn-active-1"})
                    } else if request.method == "thread/read" {
                        json!({
                            "thread": {
                                "status": "inProgress",
                                "turns": [
                                    {
                                        "id": "turn-active-1",
                                        "status": "inProgress"
                                    }
                                ]
                            }
                        })
                    } else {
                        json!({})
                    };
                    ws.send(Message::Text(
                        serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                            id: request.id,
                            result,
                        }))
                        .expect("response")
                        .into(),
                    ))
                    .await
                    .expect("send response");
                    if request.method == "turn/steer" {
                        break;
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        seed_agent_state(&runtime).await;
        let transport = runtime.spawn_transport();
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
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if runtime.active_turn_id_for_thread("recipient").await.as_deref() == Some("turn-active-1") {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("active turn became visible");

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
        transport.abort();
    }

    #[test]
    fn persist_agent_hook_state_extracts_common_fields() {
        let mut state = sample_state();
        let hook_result = HookResult {
            ok: true,
            artifacts: BTreeMap::from([
                ("branchName".to_string(), Value::String("codex/worker-a".to_string())),
                (
                    "worktreePath".to_string(),
                    Value::String("/tmp/project/.worktrees/worker-a".to_string()),
                ),
                ("baseUrl".to_string(), Value::String("http://127.0.0.1:54136".to_string())),
                ("stackName".to_string(), Value::String("worker-a-stack".to_string())),
                ("custom".to_string(), json!({"proof": true})),
            ]),
            prompt_append: vec!["Use the prepared worktree.".to_string()],
            cleanup: Some(json!({"onArchive": true})),
            metadata: Some(json!({"simulator": "ios-1"})),
            actions: Vec::new(),
            error: None,
        };

        persist_agent_hook_state(&mut state, "worker-a", &hook_result);

        let stored = persisted_agent_hook_state(&state, "worker-a").expect("stored state");
        assert_eq!(stored["branchName"], "codex/worker-a");
        assert_eq!(stored["worktreePath"], "/tmp/project/.worktrees/worker-a");
        assert_eq!(stored["baseUrl"], "http://127.0.0.1:54136");
        assert_eq!(stored["stackName"], "worker-a-stack");
        assert_eq!(stored["artifacts"]["custom"], json!({"proof": true}));
        assert_eq!(stored["cleanup"], json!({"onArchive": true}));
        assert_eq!(stored["metadata"], json!({"simulator": "ios-1"}));
        assert_eq!(stored["promptAppend"][0], "Use the prepared worktree.");
    }

    #[test]
    fn record_project_hook_telemetry_prepends_and_truncates() {
        let mut state = sample_state();
        for index in 0..25 {
            record_project_hook_telemetry(
                &mut state,
                "/alpha",
                Some("worker-a"),
                "Worker A",
                "worker",
                &HookTelemetry {
                    event: format!("event-{index}"),
                    status: "failed".to_string(),
                    detail: Some(format!("detail-{index}")),
                },
            );
        }

        let project = state.projects.get("alpha").expect("project");
        let entries = project
            .extras
            .get(PROJECT_HOOK_TELEMETRY_KEY)
            .and_then(Value::as_array)
            .expect("telemetry array");
        assert_eq!(entries.len(), 20);
        assert_eq!(entries[0]["event"], "event-24");
        assert_eq!(entries[19]["event"], "event-5");
    }

    #[test]
    fn hook_failure_notice_defaults_empty_detail() {
        let notice = hook_failure_notice(
            "project-alpha",
            "Alpha",
            Some("thread-1"),
            "Worker A",
            "worker",
            &HookTelemetry {
                event: "onWorkerCreate".to_string(),
                status: "failed".to_string(),
                detail: None,
            },
        );

        assert_eq!(notice.project_id, "project-alpha");
        assert_eq!(notice.thread_id.as_deref(), Some("thread-1"));
        assert_eq!(notice.detail, "");
    }

    #[test]
    fn persist_live_processes_round_trips_records() {
        let mut state = sample_state();
        let processes = vec![
            LiveProcessRecord {
                process_id: "2001".to_string(),
                pid: 2001,
                process_group_id: None,
                command: "sleep 30".to_string(),
                cwd: Some("/alpha".to_string()),
                started_at: 10,
            },
            LiveProcessRecord {
                process_id: "2002".to_string(),
                pid: 2002,
                process_group_id: None,
                command: "cargo check".to_string(),
                cwd: Some("/alpha".to_string()),
                started_at: 20,
            },
        ];

        persist_live_processes(&mut state, "worker-a", &processes);

        assert_eq!(persisted_live_processes(&state, "worker-a"), processes);
    }

    #[tokio::test]
    async fn register_and_complete_live_process_updates_persisted_state() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:9".to_string()))
            .await
            .expect("runtime");
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "id": "project-alpha",
                        "name": "Alpha",
                        "projectRoot": "/alpha",
                        "cwd": "/alpha",
                        "agents": {
                            "worker-a": {
                                "displayName": "Worker A",
                                "role": "worker",
                                "projectRoot": "/alpha",
                                "cwd": "/alpha"
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        register_live_process(
            &runtime,
            "worker-a",
            LiveProcessRecord {
                process_id: "3001".to_string(),
                pid: 3001,
                process_group_id: None,
                command: "sleep 30".to_string(),
                cwd: Some("/alpha".to_string()),
                started_at: 30,
            },
        )
        .await
        .expect("register live process");

        let state = parse_state(&runtime.state_document_value().await);
        let processes = persisted_live_processes(&state, "worker-a");
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].process_id, "3001");
        assert_eq!(processes[0].command, "sleep 30");

        complete_live_process(&runtime, "worker-a", "3001")
            .await
            .expect("complete live process");

        let state = parse_state(&runtime.state_document_value().await);
        assert!(persisted_live_processes(&state, "worker-a").is_empty());
    }

    #[tokio::test]
    async fn spawn_agent_applies_hook_prompt_and_persists_lifecycle() {
        let temp = TempDir::new().expect("tempdir");
        write_project_hook(
            &temp,
            "onWorkerCreate",
            "on-worker-create",
            "#!/bin/bash\ncat >/dev/null\necho '{\"ok\":true,\"promptAppend\":[\"Your worktree is ready.\"],\"artifacts\":{\"branchName\":\"codex/worker-one\",\"worktreePath\":\"/tmp/project/.worktrees/worker-one\",\"baseUrl\":\"http://127.0.0.1:54136\"}}'\n",
        );

        let (prompt_tx, prompt_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            let mut prompt_tx = Some(prompt_tx);
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request =
                    match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
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

                loop {
                    let next = ws.next().await.expect("request").expect("request frame");
                    let text = match next {
                        Message::Text(text) => text,
                        other => panic!("unexpected request frame: {other:?}"),
                    };
                    let request =
                        match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                            JSONRPCMessage::Request(request) => request,
                            other => panic!("unexpected request message: {other:?}"),
                        };
                    let result = match request.method.as_str() {
                        "thread/start" => json!({"thread": {"id": "thread-worker-1", "title": "Worker One"}}),
                        "thread/name/set" => json!({}),
                        "turn/start" => {
                            prompt_tx
                                .take()
                                .expect("prompt sender")
                                .send(request.clone())
                                .expect("record prompt request");
                            json!({"turn": {"id": "turn-1", "status": "inProgress", "items": [], "error": null}})
                        }
                        _ => json!({}),
                    };
                    ws.send(Message::Text(
                        serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                            id: request.id,
                            result,
                        }))
                        .expect("response")
                        .into(),
                    ))
                    .await
                    .expect("send response");
                    if request.method == "turn/start" {
                        break;
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();

        let outcome = execute_bridge_command(
            &runtime,
            "spawnAgent",
            json!({
                "role": "worker",
                "projectPath": temp.path().display().to_string(),
                "displayName": "Worker One",
                "initialPrompt": "Fix the regression",
            }),
        )
        .await
        .expect("spawnAgent");

        assert_eq!(outcome.payload["type"], "agent");
        assert_eq!(outcome.payload["payload"]["threadId"], "thread-worker-1");

        let prompt_request = prompt_rx.await.expect("captured prompt request");
        assert_eq!(prompt_request.method, "turn/start");
        let prompt_params = prompt_request.params.expect("prompt params");
        assert_eq!(prompt_params["threadId"], "thread-worker-1");
        assert_eq!(
            prompt_params["input"][0]["text"],
            "Fix the regression\n\nYour worktree is ready."
        );

        let state = parse_state(&runtime.state_document_value().await);
        let lifecycle = persisted_agent_hook_state(&state, "thread-worker-1").expect("lifecycle state");
        assert_eq!(lifecycle["branchName"], "codex/worker-one");
        assert_eq!(lifecycle["worktreePath"], "/tmp/project/.worktrees/worker-one");
        assert_eq!(lifecycle["baseUrl"], "http://127.0.0.1:54136");
        let tracked = agent_state_for_thread(&state, "thread-worker-1").expect("tracked agent state");
        assert_eq!(tracked.cwd.as_deref(), Some(temp.path().to_str().expect("temp path")));
        transport.abort();
    }

    #[tokio::test]
    async fn orchestrator_spawn_agent_uses_target_role_defaults_not_orchestrator_role_settings() {
        let temp = TempDir::new().expect("tempdir");
        let (thread_tx, thread_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            let mut thread_tx = Some(thread_tx);
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request =
                    match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
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

                loop {
                    let next = ws.next().await.expect("request").expect("request frame");
                    let text = match next {
                        Message::Text(text) => text,
                        other => panic!("unexpected request frame: {other:?}"),
                    };
                    let request =
                        match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                            JSONRPCMessage::Request(request) => request,
                            other => panic!("unexpected request message: {other:?}"),
                        };
                    if request.method == "thread/start" {
                        thread_tx
                            .take()
                            .expect("thread sender")
                            .send(request.clone())
                            .expect("record thread start request");
                    }
                    let result = match request.method.as_str() {
                        "thread/start" => json!({"thread": {"id": "thread-worker-2", "title": "Worker Role Defaults"}}),
                        "thread/name/set" => json!({}),
                        "turn/start" => json!({"turn": {"id": "turn-worker-2", "status": "inProgress", "items": [], "error": null}}),
                        _ => json!({}),
                    };
                    ws.send(Message::Text(
                        serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                            id: request.id,
                            result,
                        }))
                        .expect("response")
                        .into(),
                    ))
                    .await
                    .expect("send response");
                    if request.method == "turn/start" {
                        break;
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();
        runtime
            .persist_state_document(json!({
                "projects": {
                    "ezra": {
                        "id": "project-ezra",
                        "name": "Ezra",
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().join(".worktrees").display().to_string(),
                        "orchestratorThreadId": "thread-orchestrator",
                        "configs": {
                            "roleModelReasoningDefaults": {
                                "orchestrator": { "modelID": "gpt-5.4", "reasoningEffort": "high" },
                                "worker": { "modelID": "gpt-5.4-mini", "reasoningEffort": "medium" }
                            },
                            "roleDeveloperInstructionsDefaults": {
                                "orchestrator": "orchestrator developer instructions",
                                "worker": "worker developer instructions"
                            }
                        },
                        "agents": {
                            "thread-orchestrator": {
                                "displayName": "Ezra Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().join(".worktrees").display().to_string(),
                                "approvalPolicy": "on-failure",
                                "sandboxMode": "workspace-write",
                                "networkAccess": true,
                                "modelID": "gpt-5.4",
                                "reasoningEffort": "high",
                                "baseInstructions": "orchestrator base instructions",
                                "developerInstructions": "orchestrator developer instructions",
                                "persistExtendedHistory": true
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let result = orchestrator_spawn_agent(
            &runtime,
            "thread-orchestrator",
            "Worker Role Defaults",
            "Fix the bug",
            None,
            Some("worker"),
            None,
        )
        .await
        .expect("orchestrator spawn");

        assert_eq!(result["agent"]["id"], "thread-worker-2");

        let thread_request = thread_rx.await.expect("captured thread start request");
        let thread_params = thread_request.params.expect("thread params");
        assert_eq!(thread_params["model"], "gpt-5.4-mini");
        assert_eq!(thread_params["config"]["model_reasoning_effort"], "medium");
        assert!(
            thread_params["developerInstructions"]
                .as_str()
                .map(|value| value.starts_with("worker developer instructions"))
                .unwrap_or(false)
        );
        assert_ne!(
            thread_params["baseInstructions"].as_str(),
            Some("orchestrator base instructions")
        );

        let state = parse_state(&runtime.state_document_value().await);
        let agent_state = state
            .projects
            .values()
            .find_map(|project| project.agents.get("thread-worker-2"))
            .expect("persisted worker state");
        assert_eq!(agent_state.model.as_deref(), Some("gpt-5.4-mini"));
        assert_eq!(agent_state.reasoning_effort.as_deref(), Some("medium"));
        assert!(
            agent_state
                .developer_instructions
                .as_deref()
                .map(|value| value.starts_with("worker developer instructions"))
                .unwrap_or(false)
        );

        transport.abort();
    }

    #[tokio::test]
    async fn orchestrator_spawn_agent_uses_authoritative_project_defaults_for_cwd_and_sandbox() {
        let temp = TempDir::new().expect("tempdir");
        let (thread_tx, thread_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            let mut thread_tx = Some(thread_tx);
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request =
                    match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
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

                loop {
                    let next = ws.next().await.expect("request").expect("request frame");
                    let text = match next {
                        Message::Text(text) => text,
                        other => panic!("unexpected request frame: {other:?}"),
                    };
                    let request =
                        match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                            JSONRPCMessage::Request(request) => request,
                            other => panic!("unexpected request message: {other:?}"),
                        };
                    if request.method == "thread/start" {
                        thread_tx
                            .take()
                            .expect("thread sender")
                            .send(request.clone())
                            .expect("record thread start request");
                    }
                    let result = match request.method.as_str() {
                        "thread/start" => json!({"thread": {"id": "thread-worker-3", "title": "Worker Policy Defaults"}}),
                        "thread/name/set" => json!({}),
                        "turn/start" => json!({"turn": {"id": "turn-worker-3", "status": "inProgress", "items": [], "error": null}}),
                        _ => json!({}),
                    };
                    ws.send(Message::Text(
                        serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                            id: request.id,
                            result,
                        }))
                        .expect("response")
                        .into(),
                    ))
                    .await
                    .expect("send response");
                    if request.method == "turn/start" {
                        break;
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();
        runtime
            .persist_state_document(json!({
                "globalConfigs": {
                    "approvalPolicy": "on-request",
                    "sandboxMode": "workspace-write",
                    "networkAccess": true
                },
                "projects": {
                    "ezra": {
                        "id": "project-ezra",
                        "name": "Ezra",
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().join(".worktrees").display().to_string(),
                        "orchestratorThreadId": "thread-orchestrator",
                        "configs": {
                            "roleModelReasoningDefaults": {
                                "worker": { "modelID": "gpt-5.4-mini", "reasoningEffort": "medium" }
                            }
                        },
                        "agents": {
                            "thread-orchestrator": {
                                "displayName": "Ezra Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": "/tmp/not-the-project-cwd",
                                "approvalPolicy": "never",
                                "sandboxMode": "danger-full-access",
                                "networkAccess": false,
                                "modelID": "gpt-5.4"
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        orchestrator_spawn_agent(
            &runtime,
            "thread-orchestrator",
            "Worker Policy Defaults",
            "Use the safe path",
            Some("/tmp/ignored"),
            Some("worker"),
            None,
        )
        .await
        .expect("orchestrator spawn");

        let thread_request = thread_rx.await.expect("captured thread start request");
        let thread_params = thread_request.params.expect("thread params");
        assert_eq!(
            thread_params["cwd"],
            temp.path().join(".worktrees").display().to_string()
        );
        assert_eq!(thread_params["approvalPolicy"], "on-request");
        assert_eq!(thread_params["sandbox"], "workspace-write");

        let state = parse_state(&runtime.state_document_value().await);
        let agent_state = state
            .projects
            .values()
            .find_map(|project| project.agents.get("thread-worker-3"))
            .expect("persisted worker state");
        assert_eq!(
            agent_state.cwd.as_deref(),
            Some(temp.path().join(".worktrees").to_string_lossy().as_ref())
        );
        assert_eq!(agent_state.approval_policy.as_deref(), Some("on-request"));
        assert_eq!(agent_state.sandbox_mode.as_deref(), Some("workspace-write"));
        assert_eq!(agent_state.network_access, Some(true));

        transport.abort();
    }

    #[tokio::test]
    async fn orchestrator_spawn_agent_rejects_non_subordinate_roles() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:9".to_string()))
            .await
            .expect("runtime");
        runtime
            .persist_state_document(json!({
                "globalConfigs": {
                    "approvalPolicy": "on-failure",
                    "sandboxMode": "workspace-write",
                    "networkAccess": true
                },
                "projects": {
                    "ezra": {
                        "id": "project-ezra",
                        "name": "Ezra",
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().join(".worktrees").display().to_string(),
                        "orchestratorThreadId": "thread-orchestrator",
                        "agents": {
                            "thread-orchestrator": {
                                "displayName": "Ezra Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().join(".worktrees").display().to_string()
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let error = orchestrator_spawn_agent(
            &runtime,
            "thread-orchestrator",
            "Bad Spawn",
            "Do not run",
            None,
            Some("designer"),
            None,
        )
        .await
        .expect_err("orchestrator spawn should reject designer role");

        assert!(
            error
                .to_string()
                .contains("only spawn worker or qa agents")
        );
    }

    #[tokio::test]
    async fn thread_start_admin_surface_allows_explicit_role_and_session_overrides() {
        let temp = TempDir::new().expect("tempdir");
        let (thread_tx, thread_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            let mut thread_tx = Some(thread_tx);
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request =
                    match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
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
                let request =
                    match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                        JSONRPCMessage::Request(request) => request,
                        other => panic!("unexpected request message: {other:?}"),
                    };
                thread_tx
                    .take()
                    .expect("thread sender")
                    .send(request.clone())
                    .expect("record thread start request");
                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                        id: request.id,
                        result: json!({"thread": {"id": "thread-admin-1", "title": "Admin Hidden"}}),
                    }))
                    .expect("response")
                    .into(),
                ))
                .await
                .expect("send response");
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();

        let outcome = execute_bridge_command(
            &runtime,
            "threadStart",
            json!({
                "role": "hidden",
                "projectPath": temp.path().display().to_string(),
                "cwd": temp.path().join("custom-cwd").display().to_string(),
                "approvalPolicy": "never",
                "sandbox": "danger-full-access",
                "networkAccess": true,
                "model": "gpt-5.4",
                "reasoningEffort": "high",
                "developerInstructions": "hidden developer instructions",
                "baseInstructions": "hidden base instructions",
                "persistExtendedHistory": true,
            }),
        )
        .await
        .expect("threadStart");

        assert_eq!(outcome.payload["payload"]["id"], "thread-admin-1");

        let thread_request = thread_rx.await.expect("captured thread start request");
        let thread_params = thread_request.params.expect("thread params");
        assert_eq!(
            thread_params["cwd"],
            temp.path().join("custom-cwd").display().to_string()
        );
        assert_eq!(thread_params["approvalPolicy"], "never");
        assert_eq!(thread_params["sandbox"], "danger-full-access");
        assert_eq!(thread_params["model"], "gpt-5.4");
        assert_eq!(thread_params["config"]["model_reasoning_effort"], "high");
        assert_eq!(thread_params["developerInstructions"], "hidden developer instructions");
        assert_eq!(thread_params["baseInstructions"], "hidden base instructions");

        transport.abort();
    }

    #[test]
    fn designer_role_defaults_use_designer_key() {
        let temp = TempDir::new().expect("tempdir");
        let state = parse_state(&json!({
            "projects": {
                "design": {
                    "projectRoot": temp.path().display().to_string(),
                    "configs": {
                        "roleModelReasoningDefaults": {
                            "designer": { "modelID": "gpt-5.4-mini", "reasoningEffort": "high" },
                            "worker": { "modelID": "gpt-5.4-nano", "reasoningEffort": "low" }
                        }
                    }
                }
            }
        }));

        assert_eq!(
            role_default_model(&state, Some(&temp.path().display().to_string()), Some("designer")).as_deref(),
            Some("gpt-5.4-mini")
        );
        assert_eq!(
            role_default_reasoning_effort(&state, Some(&temp.path().display().to_string()), Some("designer"))
                .as_deref(),
            Some("high")
        );
    }

    #[test]
    fn designer_developer_instructions_disable_auto_route_even_when_project_enables_it() {
        let temp = TempDir::new().expect("tempdir");
        let state = parse_state(&json!({
            "projects": {
                "design": {
                    "projectRoot": temp.path().display().to_string(),
                    "cwd": temp.path().join(".worktrees/designer").display().to_string(),
                    "autoRouteReplies": true,
                    "routeApprovalRequests": true,
                    "orchestratorThreadId": "thread-orchestrator",
                    "configs": {}
                }
            }
        }));

        let guidance = developer_instructions_for_role(
            &state,
            Some("designer"),
            Some(&temp.path().display().to_string()),
            Some(&temp.path().join(".worktrees/designer").display().to_string()),
        )
        .expect("guidance");

        assert!(guidance.contains("not auto-forwarded for designers"));
        assert!(!guidance.contains("approval requests are forwarded"));
    }

    #[tokio::test]
    async fn designer_self_handoff_is_allowed() {
        let temp = TempDir::new().expect("tempdir");
        let (thread_tx, thread_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            let mut thread_tx = Some(thread_tx);
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request =
                    match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
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

                loop {
                    let next = ws.next().await.expect("request").expect("request frame");
                    let text = match next {
                        Message::Text(text) => text,
                        other => panic!("unexpected request frame: {other:?}"),
                    };
                    let request =
                        match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                            JSONRPCMessage::Request(request) => request,
                            other => panic!("unexpected request message: {other:?}"),
                        };
                    let result = match request.method.as_str() {
                        "thread/start" => {
                            thread_tx
                                .take()
                                .expect("thread sender")
                                .send(request.clone())
                                .expect("record thread start request");
                            json!({"thread": {"id": "thread-designer-2", "title": "Ezra Designer Refresh"}})
                        }
                        "thread/name/set" => json!({}),
                        "thread/archive" => json!({}),
                        _ => json!({}),
                    };
                    ws.send(Message::Text(
                        serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                            id: request.id,
                            result,
                        }))
                        .expect("response")
                        .into(),
                    ))
                    .await
                    .expect("send response");
                    if request.method == "thread/archive" {
                        break;
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();
        runtime
            .persist_state_document(json!({
                "projects": {
                    "design": {
                        "id": "project-design",
                        "name": "Design",
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().join(".worktrees/designer").display().to_string(),
                        "orchestratorThreadId": "thread-orchestrator",
                        "agents": {
                            "thread-orchestrator": {
                                "displayName": "Design Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().join(".worktrees").display().to_string()
                            },
                            "thread-designer-1": {
                                "displayName": "Ezra Designer",
                                "role": "designer",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().join(".worktrees/designer").display().to_string(),
                                "approvalPolicy": "never",
                                "sandboxMode": "danger-full-access",
                                "networkAccess": true,
                                "modelID": "gpt-5.4",
                                "developerInstructions": "designer guidance"
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let response = orchestrator_warm_handoff(
            &runtime,
            "thread-designer-1",
            Some("thread-designer-1"),
            None,
            Some(&temp.path().display().to_string()),
            "Resume the Ezra redesign from the product shelf screen.",
        )
        .await
        .expect("designer self handoff");

        assert_eq!(response["previousThreadId"], "thread-designer-1");
        assert_eq!(response["replacementThreadId"], "thread-designer-2");

        let thread_request = thread_rx.await.expect("captured thread start request");
        assert_eq!(thread_request.method, "thread/start");

        let state = parse_state(&runtime.state_document_value().await);
        let project = state.projects.values().next().expect("project state");
        assert!(!project.agents.contains_key("thread-designer-1"));
        assert!(project.agents.contains_key("thread-designer-2"));
        assert_eq!(
            project
                .agents
                .get("thread-designer-2")
                .and_then(|agent| agent.role.as_deref()),
            Some("designer")
        );

        transport.abort();
    }

    #[tokio::test]
    async fn spawn_agent_hook_failure_falls_back_and_emits_telemetry() {
        let temp = TempDir::new().expect("tempdir");
        write_project_hook(
            &temp,
            "onWorkerCreate",
            "on-worker-create",
            "#!/bin/bash\ncat >/dev/null\necho hook-broke >&2\nexit 9\n",
        );

        let addr = spawn_ws_server(move |mut ws| {
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request =
                    match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
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

                loop {
                    let next = ws.next().await.expect("request").expect("request frame");
                    let text = match next {
                        Message::Text(text) => text,
                        other => panic!("unexpected request frame: {other:?}"),
                    };
                    let request =
                        match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                            JSONRPCMessage::Request(request) => request,
                            other => panic!("unexpected request message: {other:?}"),
                        };
                    let result = match request.method.as_str() {
                        "thread/start" => json!({"thread": {"id": "thread-worker-2", "title": "Worker Broken Hook"}}),
                        "thread/name/set" => json!({}),
                        _ => json!({}),
                    };
                    ws.send(Message::Text(
                        serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                            id: request.id,
                            result,
                        }))
                        .expect("response")
                        .into(),
                    ))
                    .await
                    .expect("send response");
                    if request.method == "thread/name/set" {
                        break;
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();

        let outcome = execute_bridge_command(
            &runtime,
            "spawnAgent",
            json!({
                "role": "worker",
                "projectPath": temp.path().display().to_string(),
                "displayName": "Worker Broken Hook",
            }),
        )
        .await
        .expect("spawnAgent");

        assert_eq!(outcome.payload["payload"]["threadId"], "thread-worker-2");
        let state = parse_state(&runtime.state_document_value().await);
        let agent = agent_state_for_thread(&state, "thread-worker-2").expect("agent");
        let telemetry = agent.extras.get(HOOK_TELEMETRY_KEY).expect("hook telemetry");
        assert_eq!(telemetry["event"], "onWorkerCreate");
        assert_eq!(telemetry["status"], "failed");

        let project = state
            .projects
            .values()
            .find(|project| project.project_root.as_deref() == Some(temp.path().to_str().expect("project root")))
            .expect("project");
        let recent = project
            .extras
            .get(PROJECT_HOOK_TELEMETRY_KEY)
            .and_then(Value::as_array)
            .expect("recent hook telemetry");
        assert_eq!(recent[0]["event"], "onWorkerCreate");

        let replay = runtime.replay_events(None).await;
        assert!(replay.events.iter().any(|entry| matches!(
            &entry.event,
            crate::models::BridgeEvent::HookFailure { payload }
                if payload.thread_id.as_deref() == Some("thread-worker-2")
                    && payload.event == "onWorkerCreate"
        )));
        transport.abort();
    }

    #[tokio::test]
    async fn spawn_agent_failure_after_hook_runs_compensating_cleanup() {
        let temp = TempDir::new().expect("tempdir");
        let marker_path = temp.path().join("worker-created.marker");
        let marker = marker_path.display().to_string();
        write_project_hook(
            &temp,
            "onWorkerCreate",
            "on-worker-create",
            &format!(
                "#!/bin/bash\ncat >/dev/null\nprintf created > '{marker}'\necho '{{\"ok\":true,\"artifacts\":{{\"branchName\":\"codex/worker-compensate\",\"worktreePath\":\"/tmp/project/.worktrees/worker-compensate\"}},\"promptAppend\":[\"Worker prepared.\"]}}'\n",
            ),
        );
        write_project_hook(
            &temp,
            "onWorkerArchive",
            "on-worker-archive",
            &format!(
                "#!/bin/bash\ncat >/dev/null\nrm -f '{marker}'\necho '{{\"ok\":true}}'\n",
            ),
        );

        let addr = spawn_ws_server(move |mut ws| {
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request =
                    match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
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
                let request =
                    match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                        JSONRPCMessage::Request(request) => request,
                        other => panic!("unexpected request message: {other:?}"),
                    };
                assert_eq!(request.method, "thread/start");
                ws.send(Message::Text(
                    serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                        id: request.id,
                        result: json!({}),
                    }))
                    .expect("response")
                    .into(),
                ))
                .await
                .expect("send response");
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();

        let error = execute_bridge_command(
            &runtime,
            "spawnAgent",
            json!({
                "role": "worker",
                "projectPath": temp.path().display().to_string(),
                "displayName": "Worker Compensate",
            }),
        )
        .await
        .expect_err("spawnAgent should fail");

        assert!(error.to_string().contains("thread/start response missing thread.id"));
        assert!(!marker_path.exists(), "compensating cleanup should remove marker");
        transport.abort();
    }

    #[tokio::test]
    async fn spawn_qa_agent_applies_qa_hook_prompt_and_persists_lifecycle() {
        let temp = TempDir::new().expect("tempdir");
        write_project_hook(
            &temp,
            "onQaCreate",
            "on-qa-create",
            "#!/bin/bash\ncat >/dev/null\necho '{\"ok\":true,\"promptAppend\":[\"QA lane is prepared.\"],\"artifacts\":{\"baseUrl\":\"http://127.0.0.1:55123\",\"stackName\":\"qa-sim-1\"}}'\n",
        );

        let (prompt_tx, prompt_rx) = oneshot::channel();
        let addr = spawn_ws_server(move |mut ws| {
            let mut prompt_tx = Some(prompt_tx);
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request =
                    match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
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

                loop {
                    let next = ws.next().await.expect("request").expect("request frame");
                    let text = match next {
                        Message::Text(text) => text,
                        other => panic!("unexpected request frame: {other:?}"),
                    };
                    let request =
                        match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                            JSONRPCMessage::Request(request) => request,
                            other => panic!("unexpected request message: {other:?}"),
                        };
                    let result = match request.method.as_str() {
                        "thread/start" => json!({"thread": {"id": "thread-qa-1", "title": "QA Focus Mode"}}),
                        "thread/name/set" => json!({}),
                        "turn/start" => {
                            prompt_tx
                                .take()
                                .expect("prompt sender")
                                .send(request.clone())
                                .expect("record prompt request");
                            json!({"turn": {"id": "turn-qa-1", "status": "inProgress", "items": [], "error": null}})
                        }
                        _ => json!({}),
                    };
                    ws.send(Message::Text(
                        serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                            id: request.id,
                            result,
                        }))
                        .expect("response")
                        .into(),
                    ))
                    .await
                    .expect("send response");
                    if request.method == "turn/start" {
                        break;
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();

        let outcome = execute_bridge_command(
            &runtime,
            "spawnAgent",
            json!({
                "role": "qa",
                "projectPath": temp.path().display().to_string(),
                "displayName": "QA Focus Mode",
                "initialPrompt": "Retest the flow",
            }),
        )
        .await
        .expect("spawnAgent");

        assert_eq!(outcome.payload["payload"]["threadId"], "thread-qa-1");
        let prompt_request = prompt_rx.await.expect("captured prompt request");
        assert_eq!(prompt_request.method, "turn/start");
        let prompt_params = prompt_request.params.expect("prompt params");
        assert_eq!(
            prompt_params["input"][0]["text"],
            "Retest the flow\n\nQA lane is prepared."
        );

        let state = parse_state(&runtime.state_document_value().await);
        let lifecycle = persisted_agent_hook_state(&state, "thread-qa-1").expect("lifecycle state");
        assert_eq!(lifecycle["baseUrl"], "http://127.0.0.1:55123");
        assert_eq!(lifecycle["stackName"], "qa-sim-1");
        transport.abort();
    }

    #[tokio::test]
    async fn spawn_qa_agent_with_malformed_hook_config_falls_back_and_emits_telemetry() {
        let temp = TempDir::new().expect("tempdir");
        let config_dir = temp.path().join(".codex");
        std::fs::create_dir_all(&config_dir).expect("mkdirs");
        std::fs::write(config_dir.join("robdex-hooks.json"), "{not-json").expect("write config");

        let addr = spawn_ws_server(move |mut ws| {
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request =
                    match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
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

                loop {
                    let next = ws.next().await.expect("request").expect("request frame");
                    let text = match next {
                        Message::Text(text) => text,
                        other => panic!("unexpected request frame: {other:?}"),
                    };
                    let request =
                        match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                            JSONRPCMessage::Request(request) => request,
                            other => panic!("unexpected request message: {other:?}"),
                        };
                    let result = match request.method.as_str() {
                        "thread/start" => json!({"thread": {"id": "thread-qa-2", "title": "QA Broken Config"}}),
                        "thread/name/set" => json!({}),
                        _ => json!({}),
                    };
                    ws.send(Message::Text(
                        serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                            id: request.id,
                            result,
                        }))
                        .expect("response")
                        .into(),
                    ))
                    .await
                    .expect("send response");
                    if request.method == "thread/name/set" {
                        break;
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();

        let outcome = execute_bridge_command(
            &runtime,
            "spawnAgent",
            json!({
                "role": "qa",
                "projectPath": temp.path().display().to_string(),
                "displayName": "QA Broken Config",
            }),
        )
        .await
        .expect("spawnAgent");

        assert_eq!(outcome.payload["payload"]["threadId"], "thread-qa-2");
        let state = parse_state(&runtime.state_document_value().await);
        let agent = agent_state_for_thread(&state, "thread-qa-2").expect("agent");
        let telemetry = agent.extras.get(HOOK_TELEMETRY_KEY).expect("hook telemetry");
        assert_eq!(telemetry["event"], "onQaCreate");
        assert_eq!(telemetry["status"], "failed");
        assert!(telemetry["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("parse hook config"));

        let replay = runtime.replay_events(None).await;
        assert!(replay.events.iter().any(|entry| matches!(
            &entry.event,
            crate::models::BridgeEvent::HookFailure { payload }
                if payload.thread_id.as_deref() == Some("thread-qa-2")
                    && payload.event == "onQaCreate"
        )));
        transport.abort();
    }

    #[tokio::test]
    async fn archive_thread_records_hook_failure_and_prunes_agent() {
        let temp = TempDir::new().expect("tempdir");
        write_project_hook(
            &temp,
            "onWorkerArchive",
            "on-worker-archive",
            "#!/bin/bash\ncat >/dev/null\necho archive-broke >&2\nexit 3\n",
        );

        let addr = spawn_ws_server(move |mut ws| {
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request =
                    match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
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

                loop {
                    let next = ws.next().await.expect("request").expect("request frame");
                    let text = match next {
                        Message::Text(text) => text,
                        other => panic!("unexpected request frame: {other:?}"),
                    };
                    let request =
                        match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                            JSONRPCMessage::Request(request) => request,
                            other => panic!("unexpected request message: {other:?}"),
                        };
                    ws.send(Message::Text(
                        serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                            id: request.id,
                            result: json!({}),
                        }))
                        .expect("archive response")
                        .into(),
                    ))
                    .await
                    .expect("send response");
                    if request.method == "thread/archive" {
                        break;
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "id": "project-alpha",
                        "name": "Alpha",
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "agents": {
                            "recipient": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string(),
                                "extras": {
                                    "robdexHookLifecycle": {
                                        "branchName": "codex/worker-one"
                                    }
                                }
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        archive_thread(&runtime, "recipient").await.expect("archive thread");

        let state = parse_state(&runtime.state_document_value().await);
        assert!(agent_state_for_thread(&state, "recipient").is_none());
        let project = state.projects.get("alpha").expect("project");
        let recent = project
            .extras
            .get(PROJECT_HOOK_TELEMETRY_KEY)
            .and_then(Value::as_array)
            .expect("recent telemetry");
        assert_eq!(recent[0]["event"], "onWorkerArchive");
        assert_eq!(recent[0]["status"], "failed");

        let replay = runtime.replay_events(None).await;
        assert!(replay.events.iter().any(|entry| matches!(
            &entry.event,
            crate::models::BridgeEvent::HookFailure { payload }
                if payload.thread_id.as_deref() == Some("recipient")
                    && payload.event == "onWorkerArchive"
        )));
        transport.abort();
    }

    #[tokio::test]
    async fn archive_qa_thread_records_hook_failure_and_prunes_agent() {
        let temp = TempDir::new().expect("tempdir");
        write_project_hook(
            &temp,
            "onQaArchive",
            "on-qa-archive",
            "#!/bin/bash\ncat >/dev/null\necho qa-archive-broke >&2\nexit 4\n",
        );

        let addr = spawn_ws_server(move |mut ws| {
            Box::pin(async move {
                let init = ws.next().await.expect("init").expect("init frame");
                let init_text = match init {
                    Message::Text(text) => text,
                    other => panic!("unexpected init frame: {other:?}"),
                };
                let init_request =
                    match serde_json::from_str::<JSONRPCMessage>(&init_text).expect("jsonrpc init") {
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

                loop {
                    let next = ws.next().await.expect("request").expect("request frame");
                    let text = match next {
                        Message::Text(text) => text,
                        other => panic!("unexpected request frame: {other:?}"),
                    };
                    let request =
                        match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                            JSONRPCMessage::Request(request) => request,
                            other => panic!("unexpected request message: {other:?}"),
                        };
                    ws.send(Message::Text(
                        serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                            id: request.id,
                            result: json!({}),
                        }))
                        .expect("archive response")
                        .into(),
                    ))
                    .await
                    .expect("send response");
                    if request.method == "thread/archive" {
                        break;
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        let transport = runtime.spawn_transport();
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "id": "project-alpha",
                        "name": "Alpha",
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "agents": {
                            "qa-thread": {
                                "displayName": "QA Focus Mode",
                                "role": "qa",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string(),
                                "extras": {
                                    "robdexHookLifecycle": {
                                        "baseUrl": "http://127.0.0.1:55123"
                                    }
                                }
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        archive_thread(&runtime, "qa-thread").await.expect("archive thread");

        let state = parse_state(&runtime.state_document_value().await);
        assert!(agent_state_for_thread(&state, "qa-thread").is_none());
        let project = state.projects.get("alpha").expect("project");
        let recent = project
            .extras
            .get(PROJECT_HOOK_TELEMETRY_KEY)
            .and_then(Value::as_array)
            .expect("recent telemetry");
        assert_eq!(recent[0]["event"], "onQaArchive");
        assert_eq!(recent[0]["status"], "failed");

        let replay = runtime.replay_events(None).await;
        assert!(replay.events.iter().any(|entry| matches!(
            &entry.event,
            crate::models::BridgeEvent::HookFailure { payload }
                if payload.thread_id.as_deref() == Some("qa-thread")
                    && payload.event == "onQaArchive"
        )));
        transport.abort();
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

                loop {
                    let response = ws.next().await.expect("response").expect("response frame");
                    let response_text = match response {
                        Message::Text(text) => text,
                        other => panic!("unexpected response frame: {other:?}"),
                    };
                    let response_message = serde_json::from_str::<JSONRPCMessage>(&response_text).expect("jsonrpc message");
                    match response_message {
                        JSONRPCMessage::Request(request) => {
                            ws.send(Message::Text(
                                serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                                    id: request.id,
                                    result: json!({}),
                                }))
                                .expect("request response")
                                .into(),
                            ))
                            .await
                            .expect("send request response");
                        }
                        other => {
                            response_tx.send(other).expect("record response");
                            break;
                        }
                    }
                }

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

        tokio::time::sleep(Duration::from_millis(300)).await;
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

        tokio::time::sleep(Duration::from_millis(300)).await;
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
    async fn orchestrator_approval_decision_clears_pending_approval_without_resolved_notification() {
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

                loop {
                    let response = ws.next().await.expect("response").expect("response frame");
                    let response_text = match response {
                        Message::Text(text) => text,
                        other => panic!("unexpected response frame: {other:?}"),
                    };
                    let response_message = serde_json::from_str::<JSONRPCMessage>(&response_text).expect("jsonrpc message");
                    match response_message {
                        JSONRPCMessage::Request(request) => {
                            ws.send(Message::Text(
                                serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                                    id: request.id,
                                    result: json!({}),
                                }))
                                .expect("request response")
                                .into(),
                            ))
                            .await
                            .expect("send request response");
                        }
                        other => {
                            response_tx.send(other).expect("record response");
                            break;
                        }
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        runtime
            .persist_state_document(serde_json::to_value(sample_state()).expect("state json"))
            .await
            .expect("persist state");
        runtime
            .insert_pending_approval_for_test(crate::models::PendingApproval {
                id: "approval-1".to_string(),
                instance_id: runtime.settings().project_path.display().to_string(),
                request_id: RequestId::Integer(99),
                thread_id: "worker-a".to_string(),
                turn_id: "turn-approval-1".to_string(),
                item_id: "item-1".to_string(),
                kind: crate::models::PendingApprovalKind::CommandExecution,
                title: "Command approval".to_string(),
                detail: Some("needs approval".to_string()),
                approval_reason: Some("needs approval".to_string()),
                tool_name: None,
                tool_arguments: None,
                tool_questions: Vec::new(),
                auth_refresh_reason: None,
                command: Some("make build".to_string()),
                command_cwd: Some("/alpha".to_string()),
                file_grant_root: None,
                file_changes: Vec::new(),
            })
            .await;
        let transport = runtime.spawn_transport();

        tokio::time::sleep(Duration::from_millis(150)).await;
        let snapshot = make_app_state_snapshot(&runtime, true).await.expect("snapshot");
        assert_eq!(snapshot.pending_approvals.len(), 1);
        let approval_id = snapshot.pending_approvals[0].id.clone();

        let outcome = orchestrator_approval_decision(
            &runtime,
            "orch-a",
            &approval_id,
            "decline",
            Some("Denied"),
        )
        .await
        .expect("approval decision");
        assert_eq!(outcome["decision"], "decline");
        assert_eq!(outcome["resolved"], true);

        let response = response_rx.await.expect("response");
        match response {
            JSONRPCMessage::Response(response) => {
                assert_eq!(response.id, RequestId::Integer(99));
                assert_eq!(response.result["decision"], "decline");
            }
            other => panic!("unexpected message: {other:?}"),
        }

        tokio::time::sleep(Duration::from_millis(150)).await;
        let snapshot = make_app_state_snapshot(&runtime, true).await.expect("snapshot");
        assert!(snapshot.pending_approvals.is_empty());

        transport.abort();
    }

    #[tokio::test]
    async fn approval_requested_hook_can_auto_decline_request() {
        let temp = TempDir::new().expect("tempdir");
        write_project_hook(
            &temp,
            "onApprovalRequested",
            "on-approval-requested",
            "#!/bin/bash\ncat >/dev/null\necho '{\"ok\":true,\"actions\":[{\"type\":\"declineApproval\",\"message\":\"Use privileged exec plainly.\"}]}'\n",
        );
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

                loop {
                    let frame = ws.next().await.expect("message").expect("message frame");
                    let text = match frame {
                        Message::Text(text) => text,
                        other => panic!("unexpected frame: {other:?}"),
                    };
                    let message = serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc message");
                    match message {
                        JSONRPCMessage::Request(request) => {
                            ws.send(Message::Text(
                                serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                                    id: request.id,
                                    result: json!({}),
                                }))
                                .expect("request response")
                                .into(),
                            ))
                            .await
                            .expect("send request response");
                        }
                        JSONRPCMessage::Response(response) => {
                            response_tx.send(response).expect("record response");
                            break;
                        }
                        other => panic!("unexpected message: {other:?}"),
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        seed_agent_state(&runtime).await;
        let transport = runtime.spawn_transport();
        tokio::time::sleep(Duration::from_millis(150)).await;

        runtime
            .upstream_sender()
            .send(UpstreamRuntimeEvent::ServerRequest(
                ServerRequest::CommandExecutionRequestApproval {
                    request_id: RequestId::Integer(99),
                    params: CommandExecutionRequestApprovalParams {
                        thread_id: "recipient".to_string(),
                        turn_id: "turn-approval-1".to_string(),
                        item_id: "item-1".to_string(),
                        approval_id: None,
                        reason: Some("needs approval".to_string()),
                        network_approval_context: None,
                        command: Some("make build".to_string()),
                        cwd: Some(PathBuf::from("/tmp/project")),
                        command_actions: None,
                        additional_permissions: None,
                        skill_metadata: None,
                        proposed_execpolicy_amendment: None,
                        proposed_network_policy_amendments: None,
                        available_decisions: None,
                    },
                },
            ))
            .await
            .expect("send request");

        let response = response_rx.await.expect("response");
        assert_eq!(response.id, RequestId::Integer(99));
        assert_eq!(response.result["decision"], "decline");

        tokio::time::sleep(Duration::from_millis(150)).await;
        let snapshot = make_app_state_snapshot(&runtime, true).await.expect("snapshot");
        assert!(snapshot.pending_approvals.is_empty());

        transport.abort();
    }

    #[tokio::test]
    async fn stopped_hook_can_send_follow_up_message() {
        let temp = TempDir::new().expect("tempdir");
        write_project_hook(
            &temp,
            "onStopped",
            "on-stopped",
            "#!/bin/bash\ncat >/dev/null\necho '{\"ok\":true,\"actions\":[{\"type\":\"sendMessage\",\"recipientThreadId\":\"sender\",\"text\":\"hook-routed stop\"}]}'\n",
        );
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

                loop {
                    let frame = ws.next().await.expect("message").expect("message frame");
                    let text = match frame {
                        Message::Text(text) => text,
                        other => panic!("unexpected frame: {other:?}"),
                    };
                    let message = serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc message");
                    match message {
                        JSONRPCMessage::Request(request) => {
                            if request.method == "turn/start" {
                                request_tx.send(request.clone()).expect("record request");
                                ws.send(Message::Text(
                                    serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                                        id: request.id,
                                        result: json!({"turn":{"id":"turn-hook","items":[],"status":"inProgress","error":null}}),
                                    }))
                                    .expect("turn start response")
                                    .into(),
                                ))
                                .await
                                .expect("send turn start response");
                                break;
                            }
                            ws.send(Message::Text(
                                serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                                    id: request.id,
                                    result: json!({}),
                                }))
                                .expect("request response")
                                .into(),
                            ))
                            .await
                            .expect("send request response");
                        }
                        other => panic!("unexpected message: {other:?}"),
                    }
                }
            })
        })
        .await;

        let runtime = BridgeRuntime::new(sample_settings(&temp, format!("ws://{addr}")))
            .await
            .expect("runtime");
        seed_agent_state(&runtime).await;
        let transport = runtime.spawn_transport();
        tokio::time::sleep(Duration::from_millis(150)).await;

        runtime
            .upstream_sender()
            .send(UpstreamRuntimeEvent::Notification(ServerNotification::TurnCompleted(
                TurnCompletedNotification {
                    thread_id: "recipient".to_string(),
                    turn: Turn {
                        id: "turn-stopped-1".to_string(),
                        items: Vec::new(),
                        status: TurnStatus::Completed,
                        error: None,
                    },
                },
            )))
            .await
            .expect("send notification");

        let request = request_rx.await.expect("request");
        assert_eq!(request.method, "turn/start");
        let params = request.params.expect("params");
        assert_eq!(params["threadId"], "sender");
        assert_eq!(params["input"][0]["text"], "hook-routed stop");

        transport.abort();
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
