use std::{collections::{BTreeMap, BTreeSet}, env, fs, path::{Path, PathBuf}, time::{Duration, Instant}};

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
    manifest::{manifest_archive_denial_for_agent, manifest_runs_payload},
    models::{
        BridgeAgentSummary, BridgeAppStateSnapshot, BridgeInstanceSummary, LiveProcessRecord, PendingApproval,
        PendingApprovalKind, ThreadCachePayload,
    },
    runtime::BridgeRuntime,
    transforms::{resolve_role_instructions, summarize_scoped_agent_record},
};

const HOOK_LIFECYCLE_STATE_KEY: &str = "robdexHookLifecycle";
const HOOK_TELEMETRY_KEY: &str = "robdexHookTelemetry";
const PROJECT_HOOK_TELEMETRY_KEY: &str = "robdexRecentHookTelemetry";
const COMPACTION_STATE_KEY: &str = "robdexCompaction";
const PROJECT_PERMANENT_REQUIREMENT_COMPOSABLES_KEY: &str = "requirementsPermanentComposables";

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
    pub(crate) global_configs: Value,
    #[serde(default)]
    pub(crate) projects: BTreeMap<String, PersistedProjectState>,
    #[serde(rename = "selectedProjectID", alias = "selectedProjectId")]
    pub(crate) selected_project_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_timestamp")]
    pub(crate) updated_at: Option<u64>,
    #[serde(flatten, default)]
    pub(crate) extras: BTreeMap<String, Value>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PersistedProjectState {
    pub(crate) id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) project_root: Option<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) auto_route_replies: Option<bool>,
    pub(crate) route_approval_requests: Option<bool>,
    pub(crate) preferred_model_provider: Option<String>,
    #[serde(default)]
    pub(crate) configs: Value,
    #[serde(default)]
    pub(crate) agents: BTreeMap<String, PersistedAgentState>,
    #[serde(rename = "orchestratorThreadID", alias = "orchestratorThreadId")]
    pub(crate) orchestrator_thread_id: Option<String>,
    #[serde(default)]
    pub(crate) thread_groups: Vec<ThreadGroupState>,
    pub(crate) archived: Option<bool>,
    pub(crate) detached: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_timestamp")]
    pub(crate) updated_at: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_timestamp")]
    pub(crate) created_at: Option<u64>,
    #[serde(flatten, default)]
    pub(crate) extras: BTreeMap<String, Value>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PersistedAgentState {
    pub(crate) display_name: Option<String>,
    pub(crate) role: Option<String>,
    pub(crate) project_root: Option<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) approval_policy: Option<String>,
    pub(crate) sandbox_mode: Option<String>,
    pub(crate) network_access: Option<bool>,
    pub(crate) model: Option<String>,
    pub(crate) model_provider: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) service_tier: Option<Value>,
    pub(crate) approvals_reviewer: Option<Value>,
    pub(crate) personality: Option<Value>,
    pub(crate) config: Option<Value>,
    pub(crate) base_instructions: Option<String>,
    pub(crate) developer_instructions: Option<String>,
    pub(crate) persist_extended_history: Option<bool>,
    pub(crate) service_name: Option<String>,
    pub(crate) ephemeral: Option<bool>,
    pub(crate) dynamic_tools: Option<Value>,
    pub(crate) issue_number: Option<u64>,
    pub(crate) pull_request_number: Option<u64>,
    pub(crate) blocked_reason: Option<String>,
    pub(crate) unblock_when: Option<String>,
    #[serde(default)]
    pub(crate) requirements: Option<RequirementSetState>,
    #[serde(default)]
    pub(crate) requirement_packets: Vec<RequirementPacketState>,
    #[serde(default)]
    pub(crate) requirement_review: Option<RequirementReviewBindingState>,
    #[serde(default)]
    pub(crate) parent_thread_id: Option<String>,
    #[serde(default)]
    pub(crate) hidden_from_peer_list: bool,
    pub(crate) archived: Option<bool>,
    #[serde(flatten, default)]
    pub(crate) extras: BTreeMap<String, Value>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequirementSetState {
    pub id: Option<String>,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default = "default_true")]
    pub enforce_on_turns: bool,
    #[serde(default)]
    pub reviewer_thread_id: Option<String>,
    #[serde(default)]
    pub requirements: Vec<RequirementState>,
    #[serde(default)]
    pub review_progress: BTreeMap<String, RequirementReviewProgressState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequirementState {
    pub key: String,
    pub statement: String,
    #[serde(default = "default_requirement_severity")]
    pub severity: String,
    #[serde(default)]
    pub claim_schema_description: Option<String>,
    #[serde(default)]
    pub verdict_schema_description: Option<String>,
    #[serde(default = "default_verification_method")]
    pub verification_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequirementReviewProgressState {
    pub status: String,
    #[serde(default)]
    pub updated_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequirementComposableState {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub applies_to: Vec<String>,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    #[serde(default)]
    pub requirements: Vec<RequirementState>,
    #[serde(skip)]
    pub scope: String,
    #[serde(skip)]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequirementPacketState {
    pub packet_type: String,
    pub source_thread_id: String,
    pub turn_id: Option<String>,
    pub target_thread_id: Option<String>,
    pub payload: Value,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequirementReviewBindingState {
    pub source_thread_id: String,
    pub reviewer_thread_id: String,
    pub requirement_set_id: Option<String>,
    pub status: String,
    #[serde(default)]
    pub latest_claim_packet: Option<Value>,
    #[serde(default)]
    pub latest_verdict_packet: Option<Value>,
    pub updated_at: u64,
}

fn default_true() -> bool {
    true
}

fn default_requirement_severity() -> String {
    "medium".to_string()
}

fn default_verification_method() -> String {
    "manualEvidence".to_string()
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadGroupState {
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
            exclude_turns: None,
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
            "defaultModel": project.configs.get("modelID").cloned().unwrap_or(Value::Null),
            "defaultReasoningEffort": project.configs.get("reasoningEffort").cloned().unwrap_or(Value::Null),
            "defaultSandboxMode": project.configs.get("sandboxMode").cloned().unwrap_or(Value::Null),
            "defaultApprovalPolicy": project.configs.get("approvalPolicy").cloned().unwrap_or(Value::Null),
            "defaultNetworkAccess": project.configs.get("networkAccess").cloned().unwrap_or(Value::Null),
            "roleRuntimeDefaults": project.configs.get("roleRuntimeDefaults").cloned().unwrap_or(Value::Null),
            "permanentRequirementComposables": permanent_requirement_composable_ids(project),
            "manifestRuns": manifest_runs_payload(state, normalized_root.as_str()),
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
                    "hiddenFromPeerList": agent.hidden_from_peer_list,
                    "parentThreadId": agent.parent_thread_id,
                    "requirements": agent.requirements,
                    "requirementReview": agent.requirement_review,
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
        "globalSettingsUpdate" => {
            let mut state = parse_state(&runtime.state_document_value().await);
            update_global_settings(&mut state, &payload);
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
            let _requirement_set = parse_optional_requirement_set_payload(&payload)?;
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
                let requirement_set = compose_optional_requirement_set_payload_for_project_path(
                    runtime,
                    &next_state,
                    project_path,
                    &payload,
                )?;
                register_tracked_thread(
                    &mut next_state,
                    &registration_payload_with_requirement_set(
                        settings.to_registration_payload(
                            result.get("thread").cloned().unwrap_or(result.clone()),
                            project_path,
                            role.unwrap_or("worker"),
                        ),
                        requirement_set,
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
            let mut overrides = settings.to_app_server_thread_overrides();
            overrides.exclude_turns = Some(true);
            let params = overrides.thread_resume_params(
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
            let mut overrides = settings.to_app_server_thread_overrides();
            overrides.exclude_turns = Some(true);
            let params = overrides.thread_fork_params(thread_id.clone(), payload.get("path").cloned());
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
                output_schema: payload
                    .get("outputSchema")
                    .cloned()
                    .or_else(|| output_schema_for_thread_turn(&state, &thread_id)),
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
            app_server_request_json(runtime, "turn/interrupt", json!({"threadId": thread_id, "turnId": ""})).await?;
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
                    approval_response_payload(approval.as_ref(), &decision),
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
                    approval_response_payload(approval.as_ref(), &decision),
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
    match serde_json::from_value(value.clone()) {
        Ok(state) => state,
        Err(error) => {
            tracing::warn!("falling back to lossy Robdex state parsing after persisted state error: {error}");
            parse_state_lossy(value)
        }
    }
}

fn parse_state_lossy(value: &Value) -> PersistedState {
    let mut state = PersistedState::default();
    let Some(object) = value.as_object() else {
        return state;
    };

    state.global_configs = object.get("globalConfigs").cloned().unwrap_or(Value::Null);
    state.selected_project_id = object
        .get("selectedProjectID")
        .or_else(|| object.get("selectedProjectId"))
        .and_then(Value::as_str)
        .map(str::to_string);
    state.updated_at = object
        .get("updatedAt")
        .and_then(|value| serde_json::from_value::<Option<u64>>(value.clone()).ok())
        .flatten();
    state.extras = object
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "globalConfigs" | "projects" | "selectedProjectID" | "selectedProjectId" | "updatedAt"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();

    if let Some(projects) = object.get("projects").and_then(Value::as_object) {
        for (key, project_value) in projects {
            if let Some(project) = parse_project_lossy(project_value) {
                state.projects.insert(key.clone(), project);
            }
        }
    }

    state
}

fn parse_project_lossy(value: &Value) -> Option<PersistedProjectState> {
    let mut project_value = value.clone();
    let agents_value = project_value
        .as_object_mut()
        .and_then(|object| object.remove("agents"));
    let mut project = serde_json::from_value::<PersistedProjectState>(project_value).ok()?;
    project.agents.clear();

    if let Some(agents) = agents_value.and_then(|value| value.as_object().cloned()) {
        for (thread_id, agent_value) in agents {
            match serde_json::from_value::<PersistedAgentState>(agent_value) {
                Ok(agent) => {
                    project.agents.insert(thread_id, agent);
                }
                Err(error) => {
                    tracing::warn!("skipping malformed Robdex agent `{thread_id}` during lossy state parse: {error}");
                }
            }
        }
    }

    Some(project)
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

pub(crate) fn prune_missing_project_roots(state: &mut PersistedState) -> Vec<String> {
    let mut removed = Vec::new();
    state.projects.retain(|key, project| {
        let root = project
            .project_root
            .as_deref()
            .or(project.cwd.as_deref())
            .map(str::trim)
            .unwrap_or_default();
        if root.is_empty() || Path::new(root).is_dir() {
            return true;
        }

        let label = project
            .name
            .as_deref()
            .or(project.id.as_deref())
            .unwrap_or(key.as_str());
        removed.push(format!("{label} ({root})"));
        false
    });

    if removed.is_empty() {
        return removed;
    }

    if state
        .selected_project_id
        .as_deref()
        .is_some_and(|selected| {
            !state
                .projects
                .values()
                .any(|project| project.id.as_deref() == Some(selected))
        })
    {
        state.selected_project_id = state.projects.values().find_map(|project| project.id.clone());
    }
    state.updated_at = Some(unix_now());
    removed
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
    for key in [
        "modelID",
        "reasoningEffort",
        "approvalPolicy",
        "sandboxMode",
        "networkAccess",
    ] {
        if payload.get(key).is_some() {
            let mut configs = project.configs.as_object().cloned().unwrap_or_default();
            match payload.get(key).cloned().filter(|value| {
                if let Some(value) = value.as_str() {
                    !value.trim().is_empty()
                } else {
                    !value.is_null()
                }
            }) {
                Some(value) => {
                    configs.insert(key.to_string(), value);
                }
                None => {
                    configs.remove(key);
                }
            }
            project.configs = Value::Object(configs);
        }
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
    if let Some(role_defaults) = payload
        .get("roleRuntimeDefaults")
        .cloned()
        .filter(|value| value.is_object())
    {
        let mut configs = project.configs.as_object().cloned().unwrap_or_default();
        configs.insert("roleRuntimeDefaults".to_string(), role_defaults);
        project.configs = Value::Object(configs);
    }
    if let Some(permanent_composables) = payload.get(PROJECT_PERMANENT_REQUIREMENT_COMPOSABLES_KEY) {
        let mut configs = project.configs.as_object().cloned().unwrap_or_default();
        configs.insert(
            PROJECT_PERMANENT_REQUIREMENT_COMPOSABLES_KEY.to_string(),
            json!(parse_composable_id_array(permanent_composables, PROJECT_PERMANENT_REQUIREMENT_COMPOSABLES_KEY)?),
        );
        project.configs = Value::Object(configs);
    }
    project.updated_at = Some(unix_now());
    state.updated_at = Some(unix_now());
    Ok(())
}

fn update_global_settings(state: &mut PersistedState, payload: &Value) {
    let mut configs = state.global_configs.as_object().cloned().unwrap_or_default();
    for key in ["approvalPolicy", "sandboxMode", "networkAccess"] {
        if payload.get(key).is_some() {
            match payload.get(key).cloned().filter(|value| {
                if let Some(value) = value.as_str() {
                    !value.trim().is_empty()
                } else {
                    !value.is_null()
                }
            }) {
                Some(value) => {
                    configs.insert(key.to_string(), value);
                }
                None => {
                    configs.remove(key);
                }
            }
        }
    }
    state.global_configs = Value::Object(configs);
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
    let requirements = parse_optional_requirement_set_payload(payload)?;

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
            requirements,
            requirement_packets: Vec::new(),
            requirement_review: None,
            parent_thread_id: payload.get("parentThreadId").and_then(Value::as_str).map(str::to_string),
            hidden_from_peer_list: payload
                .get("hiddenFromPeerList")
                .and_then(Value::as_bool)
                .unwrap_or(false),
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

fn registration_payload_with_requirement_set(
    mut registration_payload: Value,
    requirement_set: Option<RequirementSetState>,
) -> Value {
    if let Some(requirement_set) = requirement_set {
        if let Some(object) = registration_payload.as_object_mut() {
            object.insert("requirementSet".to_string(), json!(requirement_set));
        }
    }
    registration_payload
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

fn project_config_string(state: &PersistedState, project_path: &str, key: &str) -> Option<String> {
    let target_path = normalize_path(project_path.to_string());
    state.projects.values().find_map(|project| {
        let matches_project = project
            .project_root
            .as_ref()
            .map(|value| normalize_path(value.clone()) == target_path)
            .unwrap_or(false);
        matches_project.then(|| {
            project
                .configs
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
        })?
    })
}

fn project_config_bool(state: &PersistedState, project_path: &str, key: &str) -> Option<bool> {
    let target_path = normalize_path(project_path.to_string());
    state.projects.values().find_map(|project| {
        let matches_project = project
            .project_root
            .as_ref()
            .map(|value| normalize_path(value.clone()) == target_path)
            .unwrap_or(false);
        matches_project.then(|| project.configs.get(key).and_then(Value::as_bool))?
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
            if agent.hidden_from_peer_list {
                continue;
            }
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
    record.is_orchestrator || matches!(record.role.as_str(), "operator" | "planner")
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
    role: Option<&str>,
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
    let approval_policy = project
        .configs
        .get("roleRuntimeDefaults")
        .and_then(|value| value.get(role_runtime_default_key(role)))
        .and_then(|value| value.get("approvalPolicy"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| project
        .configs
        .get("approvalPolicy")
        .and_then(Value::as_str)
        .map(str::to_string))
        .or_else(|| state
        .global_configs
        .get("approvalPolicy")
        .and_then(Value::as_str)
        .map(str::to_string));
    let sandbox_mode = project
        .configs
        .get("roleRuntimeDefaults")
        .and_then(|value| value.get(role_runtime_default_key(role)))
        .and_then(|value| value.get("sandboxMode"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| project
        .configs
        .get("sandboxMode")
        .and_then(Value::as_str)
        .map(str::to_string))
        .or_else(|| state
        .global_configs
        .get("sandboxMode")
        .and_then(Value::as_str)
        .map(str::to_string));
    let default_network_access = project
        .configs
        .get("roleRuntimeDefaults")
        .and_then(|value| value.get(role_runtime_default_key(role)))
        .and_then(|value| value.get("networkAccess"))
        .and_then(Value::as_bool)
        .or_else(|| project
        .configs
        .get("networkAccess")
        .and_then(Value::as_bool)
        )
        .or_else(|| state.global_configs.get("networkAccess").and_then(Value::as_bool));
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
        model: project
            .configs
            .get("modelID")
            .and_then(Value::as_str)
            .map(str::to_string),
        model_provider: project.preferred_model_provider.clone(),
        reasoning_effort: project
            .configs
            .get("reasoningEffort")
            .and_then(Value::as_str)
            .map(str::to_string),
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
            explicit_network_access.or(default_network_access).or(Some(true))
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

fn role_model_reasoning_default_key(role: Option<&str>) -> &'static str {
    match role {
        Some("designer") => "designer",
        Some("planner") => "planner",
        Some("qa") => "qa",
        Some("orchestrator") => "orchestrator",
        Some("requirements-reviewer") | Some("requirementsReviewer") => "requirements-reviewer",
        Some("worker") | Some("hidden") | Some("operator") | _ => "worker",
    }
}

fn role_runtime_default_key(role: Option<&str>) -> &'static str {
    match role {
        Some("designer") => "designer",
        Some("planner") => "planner",
        Some("qa") => "qa",
        Some("orchestrator") => "orchestrator",
        Some("operator") => "operator",
        Some("hidden") => "hidden",
        Some("requirements-reviewer") | Some("requirementsReviewer") => "requirements-reviewer",
        Some("worker") | _ => "worker",
    }
}

fn role_runtime_default_string(
    state: &PersistedState,
    project_path: Option<&str>,
    role: Option<&str>,
    setting: &str,
) -> Option<String> {
    let key = role_runtime_default_key(role);
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
        .get("roleRuntimeDefaults")
        .and_then(|value| value.get(key))
        .and_then(|value| value.get(setting))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn role_runtime_default_bool(
    state: &PersistedState,
    project_path: Option<&str>,
    role: Option<&str>,
    setting: &str,
) -> Option<bool> {
    let key = role_runtime_default_key(role);
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
        .get("roleRuntimeDefaults")
        .and_then(|value| value.get(key))
        .and_then(|value| value.get(setting))
        .and_then(Value::as_bool)
}

fn role_default_model(state: &PersistedState, project_path: Option<&str>, role: Option<&str>) -> Option<String> {
    let key = role_model_reasoning_default_key(role);
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
        .or_else(|| {
            project
                .configs
                .get("modelID")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn role_default_reasoning_effort(
    state: &PersistedState,
    project_path: Option<&str>,
    role: Option<&str>,
) -> Option<String> {
    let key = role_model_reasoning_default_key(role);
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
        .or_else(|| {
            project
                .configs
                .get("reasoningEffort")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
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
    if !matches!(role, "orchestrator" | "operator" | "planner") && project.orchestrator_thread_id.is_some() {
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
    let home = env::var_os("HOME").map(PathBuf::from);
    resolve_role_instructions_for_home(home, role)
}

fn resolve_role_instructions_for_home(
    home: Option<PathBuf>,
    role: Option<&str>,
) -> Result<Option<String>> {
    match role {
        None => Ok(None),
        Some(value) => resolve_role_instructions(home, Some(value)),
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
        output_schema: output_schema_for_thread_turn(state, thread_id),
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
            if matches!(
                agent.role.as_deref(),
                Some("requirements-reviewer") | Some("requirementsReviewer")
            ) {
                return Some("never".to_string());
            }
            return agent
                .approval_policy
                .clone()
                .or_else(|| {
                    project
                        .project_root
                        .as_deref()
                        .and_then(|project_path| {
                            role_runtime_default_string(
                                state,
                                Some(project_path),
                                agent.role.as_deref(),
                                "approvalPolicy",
                            )
                        })
                })
                .or_else(|| {
                    project
                        .configs
                        .get("approvalPolicy")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
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
            return agent
                .sandbox_mode
                .clone()
                .or_else(|| {
                    project
                        .project_root
                        .as_deref()
                        .and_then(|project_path| {
                            role_runtime_default_string(
                                state,
                                Some(project_path),
                                agent.role.as_deref(),
                                "sandboxMode",
                            )
                        })
                })
                .or_else(|| {
                    project
                        .configs
                        .get("sandboxMode")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .or_else(|| default_sandbox_mode.clone());
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
                .or_else(|| {
                    project
                        .project_root
                        .as_deref()
                        .and_then(|project_path| {
                            role_runtime_default_string(
                                state,
                                Some(project_path),
                                agent.role.as_deref(),
                                "sandboxMode",
                            )
                        })
                })
                .or_else(|| {
                    project
                        .configs
                        .get("sandboxMode")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .or_else(|| default_sandbox_mode.clone());
            let default_network_access = project
                .project_root
                .as_deref()
                .and_then(|project_path| {
                    role_runtime_default_bool(
                        state,
                        Some(project_path),
                        agent.role.as_deref(),
                        "networkAccess",
                    )
                })
                .or_else(|| project.configs.get("networkAccess").and_then(Value::as_bool))
                .or(default_network_access);
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

pub(crate) fn active_requirements_for_thread(
    state: &PersistedState,
    thread_id: &str,
) -> Option<RequirementSetState> {
    state
        .projects
        .values()
        .find_map(|project| project.agents.get(thread_id))
        .and_then(|agent| agent.requirements.clone())
        .filter(|set| set.active && !set.requirements.is_empty())
}

pub(crate) fn active_requirements_claim_schema_for_thread(
    state: &PersistedState,
    thread_id: &str,
) -> Option<Value> {
    let set = active_requirements_for_thread(state, thread_id)?;
    set.enforce_on_turns.then(|| requirements_worker_claim_schema(&set))
}

fn output_schema_for_thread_turn(state: &PersistedState, thread_id: &str) -> Option<Value> {
    if let Some(schema) = active_requirements_claim_schema_for_thread(state, thread_id) {
        return Some(schema);
    }
    if effective_role_for_thread(state, thread_id, None).as_deref() == Some("planner") {
        return Some(planner_turn_output_schema());
    }
    Some(Value::Null)
}

fn planner_turn_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["response", "clarification", "currentPlan"],
        "properties": {
            "response": {
                "type": "string",
                "description": "Plaintext response to the owner."
            },
            "clarification": {
                "anyOf": [
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["question", "options"],
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "Clarifying question for the owner."
                            },
                            "options": {
                                "type": "array",
                                "minItems": 1,
                                "maxItems": 4,
                                "items": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "required": ["label", "description"],
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "Short button label."
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "One-sentence impact or tradeoff."
                                        }
                                    }
                                }
                            }
                        }
                    },
                    { "type": "null" }
                ]
            },
            "currentPlan": {
                "anyOf": [
                    { "type": "string" },
                    { "type": "null" }
                ],
                "description": "Short current plan title, or null when no planning topic is active."
            }
        }
    })
}

pub(crate) fn requirements_worker_claim_schema(set: &RequirementSetState) -> Value {
    requirements_claim_schema_for_requirements(
        set.requirements
            .iter()
            .filter(|requirement| requirement_is_unresolved(set, requirement.key.as_str())),
    )
}

fn requirement_is_unresolved(set: &RequirementSetState, key: &str) -> bool {
    !matches!(
        set.review_progress.get(key).map(|progress| progress.status.as_str()),
        Some("passed") | Some("blocked") | Some("waived")
    )
}

fn requirements_claim_schema_for_requirements<'a>(
    requirements: impl Iterator<Item = &'a RequirementState>,
) -> Value {
    let mut requirement_properties = serde_json::Map::new();
    let mut requirement_required = Vec::new();
    for requirement in requirements {
        let key = requirement.key.trim();
        if key.is_empty() {
            continue;
        }
        requirement_required.push(key.to_string());
        let default_description = format!("Requirement: {}", requirement.statement);
        requirement_properties.insert(
            key.to_string(),
            claim_property_schema(
                requirement
                    .claim_schema_description
                    .as_deref()
                    .unwrap_or(default_description.as_str()),
            ),
        );
    }
    let mut properties = serde_json::Map::new();
    properties.insert(
        "summary".to_string(),
        json!({
            "type": "string",
            "description": "Concise global outcome or progress note. Do not duplicate per-requirement evidence here."
        }),
    );
    properties.insert(
        "requirements".to_string(),
        json!({
            "type": ["object", "null"],
            "description": "Use null for mid-turn commentary. Use the object only for an end-of-turn Requirements claim packet.",
            "properties": requirement_properties,
            "required": requirement_required,
            "additionalProperties": false
        }),
    );
    json!({
        "type": "object",
        "properties": properties,
        "required": ["summary", "requirements"],
        "additionalProperties": false
    })
}

pub(crate) fn requirements_verdict_schema(set: &RequirementSetState) -> Value {
    let (properties, required) = requirements_verdict_properties(set);
    json!({
        "type": "object",
        "properties": {
            "summary": {
                "type": "string",
                "description": "Concise reviewer progress note or final verdict summary. Do not duplicate every per-requirement verdict."
            },
            "requirements": {
                "type": ["object", "null"],
                "description": "Use null for reviewer commentary/progress. Use the object only for a final Requirements review verdict packet.",
                "properties": properties,
                "required": required,
                "additionalProperties": false
            }
        },
        "required": ["summary", "requirements"],
        "additionalProperties": false
    })
}

fn requirements_verdict_properties(set: &RequirementSetState) -> (serde_json::Map<String, Value>, Vec<String>) {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for requirement in &set.requirements {
        let key = requirement.key.trim();
        if key.is_empty() {
            continue;
        }
        required.push(key.to_string());
        let default_description = format!("Review requirement: {}", requirement.statement);
        properties.insert(
            key.to_string(),
            verdict_property_schema(
                requirement
                    .verdict_schema_description
                    .as_deref()
                    .unwrap_or(default_description.as_str()),
                requirement_review_previously_passed(set, key),
            ),
        );
    }
    required.push("overallVerdict".to_string());
    required.push("route".to_string());
    properties.insert(
        "overallVerdict".to_string(),
        json!({
            "type": "string",
            "enum": ["pass", "fail", "acceptedBlocked", "rejectedBlocked", "needsHumanWaiver", "waiverAccepted"],
            "description": "Overall gate verdict. Blocked is not success; only acceptedBlocked can leave the source agent, and only when evidence proves a true external dependency. Use waiverAccepted only after an explicit human/owner waiver has been accepted."
        }),
    );
    properties.insert(
        "route".to_string(),
        json!({
            "type": "object",
            "properties": {
                "destination": {
                    "type": "string",
                    "enum": ["sourceAgent", "orchestrator", "owner", "none"]
                },
                "message": {
                    "type": "string",
                    "description": "Curated routing message with exact failed requirements or owner action."
                }
            },
            "required": ["destination", "message"],
            "additionalProperties": false
        }),
    );
    (properties, required)
}

fn claim_property_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "description": description,
        "properties": {
            "claim": {
                "type": "string",
                "enum": ["satisfied", "notSatisfied", "blocked", "notApplicable"]
            },
            "justification": { "type": "string" },
            "evidence": {
                "type": "array",
                "items": { "type": "string" }
            },
            "risk": {
                "type": "string",
                "enum": ["none", "low", "medium", "high", "unknown"]
            }
        },
        "required": ["claim", "justification", "evidence", "risk"],
        "additionalProperties": false
    })
}

fn verdict_property_schema(description: &str, allow_still_passing: bool) -> Value {
    let full_verdict = json!({
        "type": "object",
        "description": description,
        "properties": {
            "verdict": {
                "type": "string",
                "enum": ["pass", "fail", "acceptedBlocked", "rejectedBlocked", "waiverRequired", "waiverAccepted"]
            },
            "reason": { "type": "string" },
            "evidenceAssessment": { "type": "string" },
            "requiredCorrection": { "type": "string" }
        },
        "required": ["verdict", "reason", "evidenceAssessment", "requiredCorrection"],
        "additionalProperties": false
    });
    if !allow_still_passing {
        return full_verdict;
    }
    json!({
        "description": format!("{description} Previously passed requirements may use {{\"verdict\":\"stillPassing\"}} only after rechecking that the same pass remains valid."),
        "anyOf": [
            full_verdict,
            {
                "type": "object",
                "properties": {
                    "verdict": {
                        "type": "string",
                        "enum": ["stillPassing"]
                    }
                },
                "required": ["verdict"],
                "additionalProperties": false
            }
        ]
    })
}

fn requirement_review_previously_passed(set: &RequirementSetState, key: &str) -> bool {
    set.review_progress
        .get(key)
        .map(|progress| progress.status == "passed")
        .unwrap_or(false)
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
                .or_else(|| {
                    project
                        .project_root
                        .as_deref()
                        .and_then(|project_path| {
                            role_runtime_default_string(
                                state,
                                Some(project_path),
                                agent.role.as_deref(),
                                "sandboxMode",
                            )
                        })
                })
                .or_else(|| {
                    project
                        .configs
                        .get("sandboxMode")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .or_else(|| default_sandbox_mode.clone());
            let default_network_access = project
                .project_root
                .as_deref()
                .and_then(|project_path| {
                    role_runtime_default_bool(
                        state,
                        Some(project_path),
                        agent.role.as_deref(),
                        "networkAccess",
                    )
                })
                .or_else(|| project.configs.get("networkAccess").and_then(Value::as_bool))
                .or(default_network_access);
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

pub(crate) async fn spawn_agent(runtime: &BridgeRuntime, payload: &Value) -> Result<BridgeAgentSummary> {
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
    let requirement_set = compose_optional_requirement_set_payload_for_project_path(
        runtime,
        &state,
        &project_path,
        payload,
    )?;
    let display_name = payload.get("displayName").and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty());
    let display_name_value = display_name
        .map(str::to_string)
        .unwrap_or_else(|| role_value.clone());
    let approval_policy = payload
        .get("approvalPolicy")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| role_runtime_default_string(&state, Some(&project_path), role, "approvalPolicy"))
        .or_else(|| project_config_string(&state, &project_path, "approvalPolicy"))
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
        .or_else(|| role_runtime_default_string(&state, Some(&project_path), role, "sandboxMode"))
        .or_else(|| project_config_string(&state, &project_path, "sandboxMode"))
        .or_else(|| {
            state
                .global_configs
                .get("sandboxMode")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let default_network_access = role_runtime_default_bool(&state, Some(&project_path), role, "networkAccess")
        .or_else(|| project_config_bool(&state, &project_path, "networkAccess"))
        .or_else(|| state.global_configs.get("networkAccess").and_then(Value::as_bool));
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
        &registration_payload_with_requirement_set(
            settings.to_registration_payload(thread, &project_path, &role_value),
            requirement_set,
        ),
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
        "requirementSet": payload.get("requirementSet").cloned().unwrap_or(Value::Null),
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
    let has_requirement_set = payload
        .get("requirementSet")
        .filter(|value| !value.is_null())
        .is_some();
    if has_requirement_set && runtime.active_turn_id_for_thread(&thread_id).await.is_some() {
        bail!("Requirements can only be attached when starting a new turn; turn/steer cannot change output schemas.");
    }
    let state = parse_state(&runtime.state_document_value().await);
    let state = if has_requirement_set {
        let requirement_set = compose_optional_requirement_set_payload_for_thread(
            runtime,
            &state,
            &thread_id,
            payload,
        )?
        .expect("checked requirement set");
        persist_requirements_for_thread(runtime, &thread_id, requirement_set).await?
    } else {
        state
    };
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

fn tracked_agent_record_for_thread(
    state: &PersistedState,
    running_thread_ids: &[String],
    thread_id: &str,
) -> Option<crate::models::ScopedAgentRecord> {
    let running = running_thread_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    for project in state.projects.values() {
        let Some(agent) = project.agents.get(thread_id) else {
            continue;
        };
        let project_root = project.project_root.clone().unwrap_or_default();
        let cwd = project.cwd.clone().unwrap_or_else(|| project_root.clone());
        let role = agent.role.clone().unwrap_or_else(|| "worker".to_string());
        let is_orchestrator =
            role == "orchestrator" || project.orchestrator_thread_id.as_deref() == Some(thread_id);
        return Some(crate::models::ScopedAgentRecord {
            thread_id: thread_id.to_string(),
            display_name: agent.display_name.clone(),
            project_path: agent.project_root.clone().unwrap_or(project_root),
            cwd,
            role: role.clone(),
            is_orchestrator,
            is_running: running.contains(thread_id),
            is_archived: agent.archived.unwrap_or(false),
            is_hidden: role == "hidden",
            updated_at: project.updated_at.unwrap_or_else(unix_now),
        });
    }
    None
}

fn resolve_requirements_recipient(
    state: &PersistedState,
    running_thread_ids: &[String],
    sender: &crate::models::ScopedAgentRecord,
    recipient_thread_id: Option<&str>,
    recipient_name: Option<&str>,
    project_path: Option<&str>,
) -> Result<crate::models::ScopedAgentRecord> {
    if recipient_thread_id.map(str::trim).filter(|value| !value.is_empty()).is_none()
        && recipient_name.map(str::trim).filter(|value| !value.is_empty()).is_none()
    {
        return Ok(sender.clone());
    }
    if let Some(thread_id) = recipient_thread_id.map(str::trim).filter(|value| !value.is_empty()) {
        let recipient = tracked_agent_record_for_thread(state, running_thread_ids, thread_id)
            .ok_or_else(|| anyhow::anyhow!("Thread `{thread_id}` is not tracked by the bridge."))?;
        if let Some(project_path) = project_path.map(|value| normalize_path(value.to_string()))
            && !project_path.is_empty()
            && normalize_path(recipient.project_path.clone()) != project_path
        {
            bail!("Thread `{thread_id}` is not in project `{project_path}`.");
        }
        return Ok(recipient);
    }
    if sender.is_hidden {
        bail!("Hidden threads can only resolve Requirements targets by exact thread id or self.");
    }
    let records = all_agent_records(state, running_thread_ids);
    let scoped = scoped_agent_context(&records, &sender.thread_id, true)?;
    resolve_scoped_recipient(&scoped.visible, None, recipient_name, project_path)
}

fn requirements_self_setting_allowed(role: &str) -> bool {
    !matches!(role, "worker" | "qa" | "planner")
}

fn planner_requirements_target_allowed(
    sender: &crate::models::ScopedAgentRecord,
    recipient: &crate::models::ScopedAgentRecord,
) -> bool {
    sender.role == "planner"
        && sender.project_path == recipient.project_path
        && sender.thread_id != recipient.thread_id
        && !recipient.is_hidden
}

fn ensure_requirements_view_allowed(
    sender: &crate::models::ScopedAgentRecord,
    recipient: &crate::models::ScopedAgentRecord,
) -> Result<()> {
    if sender.thread_id == recipient.thread_id {
        return Ok(());
    }
    if sender.role == "operator" {
        return Ok(());
    }
    if sender.is_orchestrator && sender.project_path == recipient.project_path && recipient.role == "worker" {
        return Ok(());
    }
    if planner_requirements_target_allowed(sender, recipient) {
        return Ok(());
    }
    bail!(
        "Requirements access denied. Orchestrators may only target workers in their project; planners may target non-hidden agents in their project; other roles may only target themselves."
    )
}

fn ensure_requirements_mutation_allowed(
    sender: &crate::models::ScopedAgentRecord,
    recipient: &crate::models::ScopedAgentRecord,
) -> Result<()> {
    if sender.thread_id == recipient.thread_id {
        if requirements_self_setting_allowed(&sender.role) {
            return Ok(());
        }
        bail!("Workers, QA, and planner threads cannot set Requirements on themselves.");
    }
    if sender.role == "operator" {
        return Ok(());
    }
    if sender.is_orchestrator && sender.project_path == recipient.project_path && recipient.role == "worker" {
        return Ok(());
    }
    if planner_requirements_target_allowed(sender, recipient) {
        return Ok(());
    }
    bail!(
        "Requirements mutation denied. Orchestrators may only set Requirements on workers in their project; planners may set Requirements on non-hidden agents in their project; other roles may only set Requirements on themselves."
    )
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

pub(crate) fn requirements_review_source_for_reviewer(
    state: &PersistedState,
    reviewer_thread_id: &str,
) -> Option<String> {
    for project in state.projects.values() {
        for (source_thread_id, agent) in &project.agents {
            if agent
                .requirement_review
                .as_ref()
                .map(|review| review.reviewer_thread_id.as_str() == reviewer_thread_id)
                .unwrap_or(false)
            {
                return Some(source_thread_id.clone());
            }
        }
    }
    None
}

fn persisted_agent_hook_state(state: &PersistedState, thread_id: &str) -> Option<Value> {
    agent_state_for_thread(state, thread_id)
        .and_then(|agent| agent.extras.get(HOOK_LIFECYCLE_STATE_KEY).cloned())
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
    let group = state
        .projects
        .get(&project_key)
        .ok_or_else(|| anyhow::anyhow!("Unknown project `{project_key}`."))?
        .thread_groups
        .iter()
        .find(|group| group.id == group_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Unknown thread group `{group_id}`."))?;
    let group_title = group.title.clone();
    let project_path_value = state
        .projects
        .get(&project_key)
        .and_then(|project| project.project_root.clone());
    let mut archived_thread_ids = BTreeSet::new();
    let mut skipped_thread_ids = Vec::new();
    for member_thread_id in &group.thread_ids {
        if member_thread_id == sender_thread_id {
            skipped_thread_ids.push(member_thread_id.clone());
            continue;
        }
        let member_state = state
            .projects
            .get(&project_key)
            .and_then(|project| project.agents.get(member_thread_id))
            .map(|agent| {
                (
                    agent.role.as_deref().unwrap_or("worker").to_string(),
                    !agent.archived.unwrap_or(false),
                )
            });
        match member_state {
            Some((role, true)) if orchestrator_can_archive_agent_role(&role) => {
                archived_thread_ids.insert(member_thread_id.clone());
            }
            Some(_) | None => {
                if !archived_thread_ids.contains(member_thread_id) {
                    skipped_thread_ids.push(member_thread_id.clone());
                }
            }
        }
    }
    let mut pruned_thread_ids = BTreeSet::new();
    for archived_thread_id in archived_thread_ids {
        for pruned_thread_id in prune_archived_thread_locally_filtered(
            &mut state,
            &archived_thread_id,
            orchestrator_can_archive_agent_role,
        ) {
            pruned_thread_ids.insert(pruned_thread_id);
        }
    }
    if !pruned_thread_ids.is_empty() {
        persist_state(runtime, &state).await?;
        for pruned_thread_id in &pruned_thread_ids {
            runtime.prune_thread_local(pruned_thread_id).await?;
        }
    }
    for archived_thread_id in &pruned_thread_ids {
        let _ = app_server_request_json(
            runtime,
            "thread/archive",
            json!({"threadId": archived_thread_id}),
        )
        .await;
    }
    Ok(json!({
        "projectPath": project_path_value,
        "groupId": group.id,
        "title": group_title,
        "archivedThreadIds": pruned_thread_ids.into_iter().collect::<Vec<_>>(),
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
    let mut visible = match scoped_agent_context(&records, sender_thread_id, true) {
        Ok(scoped) => scoped
            .visible
            .into_iter()
            .map(|record| record.thread_id)
            .collect::<std::collections::BTreeSet<_>>(),
        Err(_error) if agent_state_for_thread(&state, sender_thread_id).is_some() => {
            std::collections::BTreeSet::new()
        }
        Err(error) => return Err(error),
    };
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

pub async fn orchestrator_set_requirements(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    recipient_thread_id: Option<&str>,
    recipient_name: Option<&str>,
    project_path: Option<&str>,
    set_payload: Value,
) -> Result<Value> {
    if set_payload.is_null() {
        return orchestrator_clear_requirements(
            runtime,
            sender_thread_id,
            recipient_thread_id,
            recipient_name,
            project_path,
        )
        .await;
    }

    let _state_guard = runtime.lock_state_mutation().await;
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let sender = tracked_agent_record_for_thread(&state, &running, sender_thread_id)
        .ok_or_else(|| anyhow::anyhow!("Thread `{sender_thread_id}` is not tracked by the bridge."))?;
    let recipient =
        resolve_requirements_recipient(&state, &running, &sender, recipient_thread_id, recipient_name, project_path)?;
    ensure_requirements_mutation_allowed(&sender, &recipient)?;
    let mut set = compose_requirement_set_payload(runtime, &state, &recipient.thread_id, set_payload)?;
    validate_requirement_set(&set)?;
    if set.id.as_deref().map(str::trim).unwrap_or_default().is_empty() {
        set.id = Some(format!("requirements-{}", unix_now()));
    }
    for project in state.projects.values_mut() {
        if let Some(agent) = project.agents.get_mut(&recipient.thread_id) {
            agent.requirements = Some(set.clone());
            agent.requirement_review = None;
            persist_state(runtime, &state).await?;
            runtime
                .push_event(crate::models::BridgeEvent::AppStateSnapshot {
                    state: runtime.state_document_value().await,
                })
                .await;
            return Ok(json!({
                "threadId": recipient.thread_id,
                "displayName": recipient.display_name,
                "requirementSetId": set.id,
                "requirementCount": set.requirements.len(),
                "enforceOnTurns": set.enforce_on_turns,
            }));
        }
    }
    bail!("Recipient `{}` is not a tracked agent.", recipient.thread_id)
}

pub async fn direct_set_requirements(
    runtime: &BridgeRuntime,
    recipient_thread_id: Option<&str>,
    _project_path: Option<&str>,
    set_payload: Value,
) -> Result<Value> {
    let recipient_thread_id = recipient_thread_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("recipientThreadId is required for direct Requirements updates."))?;
    if set_payload.is_null() {
        return direct_clear_requirements(runtime, recipient_thread_id).await;
    }

    let _state_guard = runtime.lock_state_mutation().await;
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let recipient = tracked_agent_record_for_thread(&state, &running, recipient_thread_id)
        .ok_or_else(|| anyhow::anyhow!("Thread `{recipient_thread_id}` is not tracked by the bridge."))?;
    let mut set = compose_requirement_set_payload(runtime, &state, &recipient.thread_id, set_payload)?;
    validate_requirement_set(&set)?;
    if set.id.as_deref().map(str::trim).unwrap_or_default().is_empty() {
        set.id = Some(format!("requirements-{}", unix_now()));
    }
    for project in state.projects.values_mut() {
        if let Some(agent) = project.agents.get_mut(&recipient.thread_id) {
            agent.requirements = Some(set.clone());
            agent.requirement_review = None;
            persist_state(runtime, &state).await?;
            runtime
                .push_event(crate::models::BridgeEvent::AppStateSnapshot {
                    state: runtime.state_document_value().await,
                })
                .await;
            return Ok(json!({
                "threadId": recipient.thread_id,
                "displayName": recipient.display_name,
                "requirementSetId": set.id,
                "requirementCount": set.requirements.len(),
                "enforceOnTurns": set.enforce_on_turns,
            }));
        }
    }
    bail!("Recipient `{}` is not a tracked agent.", recipient.thread_id)
}

pub async fn orchestrator_requirement_composables(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    recipient_thread_id: Option<&str>,
    recipient_name: Option<&str>,
    project_path: Option<&str>,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let sender = tracked_agent_record_for_thread(&state, &running, sender_thread_id)
        .ok_or_else(|| anyhow::anyhow!("Thread `{sender_thread_id}` is not tracked by the bridge."))?;
    let recipient = if recipient_thread_id.is_none()
        && recipient_name.is_none()
        && project_path.map(str::trim).filter(|value| !value.is_empty()).is_some()
    {
        let project_path = project_path.expect("checked project path");
        return requirement_composables_for_project_path(runtime, &state, project_path);
    } else {
        resolve_requirements_recipient(&state, &running, &sender, recipient_thread_id, recipient_name, project_path)?
    };
    ensure_requirements_view_allowed(&sender, &recipient)?;
    let composables = discover_requirement_composables(runtime, &state, &recipient.thread_id)?;
    let permanent_ids: BTreeSet<String> = permanent_requirement_composable_ids_for_thread(&state, &recipient.thread_id)
        .into_iter()
        .collect();
    let items = requirement_composable_items(composables, &permanent_ids);
    Ok(json!({
        "threadId": recipient.thread_id,
        "displayName": recipient.display_name,
        "items": items,
    }))
}

pub async fn direct_requirement_composables(
    runtime: &BridgeRuntime,
    recipient_thread_id: Option<&str>,
    project_path: Option<&str>,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    if recipient_thread_id.map(str::trim).filter(|value| !value.is_empty()).is_none() {
        let project_path = project_path
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("recipientThreadId or projectPath is required."))?;
        return requirement_composables_for_project_path(runtime, &state, project_path);
    }
    let recipient_thread_id = recipient_thread_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .expect("checked recipient thread");
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let recipient = tracked_agent_record_for_thread(&state, &running, recipient_thread_id)
        .ok_or_else(|| anyhow::anyhow!("Thread `{recipient_thread_id}` is not tracked by the bridge."))?;
    let composables = discover_requirement_composables(runtime, &state, &recipient.thread_id)?;
    let permanent_ids: BTreeSet<String> = permanent_requirement_composable_ids_for_thread(&state, &recipient.thread_id)
        .into_iter()
        .collect();
    Ok(json!({
        "threadId": recipient.thread_id,
        "displayName": recipient.display_name,
        "items": requirement_composable_items(composables, &permanent_ids),
    }))
}

fn requirement_composables_for_project_path(
    runtime: &BridgeRuntime,
    state: &PersistedState,
    project_path: &str,
) -> Result<Value> {
    let normalized_project = normalize_path(project_path.to_string());
    let project = state
        .projects
        .values()
        .find(|project| normalize_path(project.project_root.clone().unwrap_or_default()) == normalized_project)
        .ok_or_else(|| anyhow::anyhow!("Unknown project `{normalized_project}`."))?;
    let mut composables = BTreeMap::new();
    load_requirement_composables_from_dir(
        &global_requirement_composables_dir(runtime),
        "global",
        &mut composables,
    )?;
    load_requirement_composables_from_dir(
        &PathBuf::from(&normalized_project).join(".codex").join("requirements").join("composables"),
        "project",
        &mut composables,
    )?;
    let permanent_ids = permanent_requirement_composable_ids(project)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let representative_thread_id = project
        .orchestrator_thread_id
        .clone()
        .or_else(|| project.agents.keys().next().cloned());
    let representative_display_name = representative_thread_id
        .as_deref()
        .and_then(|thread_id| project.agents.get(thread_id))
        .and_then(|agent| agent.display_name.clone())
        .or_else(|| project.name.clone());
    Ok(json!({
        "threadId": representative_thread_id,
        "displayName": representative_display_name,
        "projectPath": normalized_project,
        "items": requirement_composable_items(composables, &permanent_ids),
    }))
}

fn requirement_composable_items(
    composables: BTreeMap<String, RequirementComposableState>,
    permanent_ids: &BTreeSet<String>,
) -> Vec<Value> {
    composables
        .into_values()
        .map(|composable| {
            let permanent = permanent_ids.contains(&composable.id);
            json!({
                "id": composable.id,
                "title": composable.title,
                "description": composable.description,
                "appliesTo": composable.applies_to,
                "conflictsWith": composable.conflicts_with,
                "scope": composable.scope,
                "permanent": permanent,
                "permanentSource": if permanent { Some("project") } else { None },
                "path": composable.path.display().to_string(),
                "requirementCount": composable.requirements.len(),
                "requirements": composable.requirements,
            })
        })
        .collect()
}

async fn orchestrator_clear_requirements(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    recipient_thread_id: Option<&str>,
    recipient_name: Option<&str>,
    project_path: Option<&str>,
) -> Result<Value> {
    let _state_guard = runtime.lock_state_mutation().await;
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let sender = tracked_agent_record_for_thread(&state, &running, sender_thread_id)
        .ok_or_else(|| anyhow::anyhow!("Thread `{sender_thread_id}` is not tracked by the bridge."))?;
    let recipient =
        resolve_requirements_recipient(&state, &running, &sender, recipient_thread_id, recipient_name, project_path)?;
    ensure_requirements_mutation_allowed(&sender, &recipient)?;

    for project in state.projects.values_mut() {
        if let Some(agent) = project.agents.get_mut(&recipient.thread_id) {
            agent.requirements = None;
            agent.requirement_review = None;
            agent.requirement_packets.clear();
            persist_state(runtime, &state).await?;
            runtime
                .push_event(crate::models::BridgeEvent::AppStateSnapshot {
                    state: runtime.state_document_value().await,
                })
                .await;
            return Ok(json!({
                "threadId": recipient.thread_id,
                "displayName": recipient.display_name,
                "requirementSetId": Value::Null,
                "requirementCount": 0,
                "enforceOnTurns": false,
                "cleared": true,
            }));
        }
    }
    bail!("Recipient `{}` is not a tracked agent.", recipient.thread_id)
}

async fn direct_clear_requirements(
    runtime: &BridgeRuntime,
    recipient_thread_id: &str,
) -> Result<Value> {
    let _state_guard = runtime.lock_state_mutation().await;
    let mut state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let recipient = tracked_agent_record_for_thread(&state, &running, recipient_thread_id)
        .ok_or_else(|| anyhow::anyhow!("Thread `{recipient_thread_id}` is not tracked by the bridge."))?;

    for project in state.projects.values_mut() {
        if let Some(agent) = project.agents.get_mut(&recipient.thread_id) {
            agent.requirements = None;
            agent.requirement_review = None;
            agent.requirement_packets.clear();
            persist_state(runtime, &state).await?;
            runtime
                .push_event(crate::models::BridgeEvent::AppStateSnapshot {
                    state: runtime.state_document_value().await,
                })
                .await;
            return Ok(json!({
                "threadId": recipient.thread_id,
                "displayName": recipient.display_name,
                "requirementSetId": Value::Null,
                "requirementCount": 0,
                "enforceOnTurns": false,
                "cleared": true,
            }));
        }
    }
    bail!("Recipient `{}` is not a tracked agent.", recipient.thread_id)
}

pub async fn orchestrator_requirements_status(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    recipient_thread_id: Option<&str>,
    recipient_name: Option<&str>,
    project_path: Option<&str>,
) -> Result<Value> {
    let state = parse_state(&runtime.state_document_value().await);
    let running = runtime.snapshot().await?.thread_cache.running_thread_ids;
    let sender = tracked_agent_record_for_thread(&state, &running, sender_thread_id)
        .ok_or_else(|| anyhow::anyhow!("Thread `{sender_thread_id}` is not tracked by the bridge."))?;
    let recipient =
        resolve_requirements_recipient(&state, &running, &sender, recipient_thread_id, recipient_name, project_path)?;
    ensure_requirements_view_allowed(&sender, &recipient)?;
    for project in state.projects.values() {
        let Some(agent) = project.agents.get(&recipient.thread_id) else {
            continue;
        };
        let requirements = agent.requirements.clone();
        let stored_requirements = requirements
            .as_ref()
            .map(|set| set.requirements.clone())
            .unwrap_or_default();
        let requirement_set_active = requirements
            .as_ref()
            .map(|set| set.active && !set.requirements.is_empty())
            .unwrap_or(false);
        let active_requirements = if requirement_set_active {
            stored_requirements.clone()
        } else {
            Vec::new()
        };
        let review = agent.requirement_review.clone();
        let latest_verdict_packet = review
            .as_ref()
            .and_then(|binding| binding.latest_verdict_packet.clone());
        let mut passed_count = 0_u32;
        let mut failed_count = 0_u32;
        let mut blocked_count = 0_u32;
        let mut waiver_required_count = 0_u32;
        let mut waiver_accepted_count = 0_u32;
        let mut unknown_count = 0_u32;
        let verdicts = if active_requirements.is_empty() {
            Vec::new()
        } else {
            active_requirements
                    .iter()
                    .map(|requirement| {
                        let verdict = latest_verdict_packet
                            .as_ref()
                            .and_then(|packet| packet.get(&requirement.key))
                            .and_then(|item| item.get("verdict"))
                            .and_then(Value::as_str)
                            .map(str::to_string);
                        match verdict.as_deref() {
                            Some("pass") => passed_count += 1,
                            Some("fail") | Some("rejectedBlocked") => failed_count += 1,
                            Some("acceptedBlocked") => blocked_count += 1,
                            Some("waiverRequired") => waiver_required_count += 1,
                            Some("waiverAccepted") => waiver_accepted_count += 1,
                            _ => unknown_count += 1,
                        }
                        json!({
                            "key": requirement.key,
                            "statement": requirement.statement,
                            "severity": requirement.severity,
                            "verificationMethod": requirement.verification_method,
                            "verdict": verdict,
                            "reason": latest_verdict_packet
                                .as_ref()
                                .and_then(|packet| packet.get(&requirement.key))
                                .and_then(|item| item.get("reason"))
                                .and_then(Value::as_str),
                            "evidenceAssessment": latest_verdict_packet
                                .as_ref()
                                .and_then(|packet| packet.get(&requirement.key))
                                .and_then(|item| item.get("evidenceAssessment"))
                                .and_then(Value::as_str),
                            "requiredCorrection": latest_verdict_packet
                                .as_ref()
                                .and_then(|packet| packet.get(&requirement.key))
                                .and_then(|item| item.get("requiredCorrection"))
                                .and_then(Value::as_str),
                        })
                    })
                    .collect::<Vec<_>>()
        };
        return Ok(json!({
            "threadId": recipient.thread_id,
            "displayName": recipient.display_name,
            "requirements": requirements,
            "requirementReview": review,
            "requirementPackets": agent.requirement_packets,
            "summary": {
                "activeRequirementCount": active_requirements.len(),
                "storedRequirementCount": stored_requirements.len(),
                "requirementSetActive": requirement_set_active,
                "status": review.as_ref().map(|binding| binding.status.clone()),
                "reviewerThreadId": review.as_ref().map(|binding| binding.reviewer_thread_id.clone()),
                "requirementSetId": review
                    .as_ref()
                    .and_then(|binding| binding.requirement_set_id.clone())
                    .or_else(|| requirements.as_ref().and_then(|set| set.id.clone())),
                "updatedAt": review.as_ref().map(|binding| binding.updated_at),
                "passedCount": passed_count,
                "failedCount": failed_count,
                "blockedCount": blocked_count,
                "waiverRequiredCount": waiver_required_count,
                "waiverAcceptedCount": waiver_accepted_count,
                "unknownCount": unknown_count,
                "requirements": stored_requirements,
                "verdicts": verdicts,
            }
        }));
    }
    bail!("Recipient `{}` is not a tracked agent.", recipient.thread_id)
}

fn parse_requirement_set_payload(payload: Value) -> Result<RequirementSetState> {
    if payload.get("requirements").is_some() {
        Ok(serde_json::from_value(payload).context("invalid requirement set payload")?)
    } else if payload.is_array() {
        Ok(RequirementSetState {
            requirements: serde_json::from_value(payload).context("invalid requirements array")?,
            ..Default::default()
        })
    } else {
        bail!("requirements payload must be an object with `requirements` or an array")
    }
}

fn compose_requirement_set_payload(
    runtime: &BridgeRuntime,
    state: &PersistedState,
    recipient_thread_id: &str,
    payload: Value,
) -> Result<RequirementSetState> {
    let explicit = selected_composable_ids(&payload)?;
    let mut set = parse_requirement_set_payload(strip_composable_selection(payload))?;
    let selected = permanent_and_explicit_composable_ids(state, recipient_thread_id, explicit);
    if selected.is_empty() {
        return Ok(set);
    }
    let available = discover_requirement_composables(runtime, state, recipient_thread_id)?;
    merge_selected_composables_into_set(&mut set, &available, selected)?;
    Ok(set)
}

fn compose_requirement_set_payload_for_project_path(
    runtime: &BridgeRuntime,
    state: &PersistedState,
    project_path: &str,
    payload: Value,
) -> Result<RequirementSetState> {
    let explicit = selected_composable_ids(&payload)?;
    let mut set = parse_requirement_set_payload(strip_composable_selection(payload))?;
    let selected = permanent_and_explicit_composable_ids_for_project_path(state, project_path, explicit);
    if selected.is_empty() {
        return Ok(set);
    }
    let available = discover_requirement_composables_for_project_path(runtime, project_path)?;
    merge_selected_composables_into_set(&mut set, &available, selected)?;
    Ok(set)
}

fn compose_optional_requirement_set_payload_for_thread(
    runtime: &BridgeRuntime,
    state: &PersistedState,
    recipient_thread_id: &str,
    payload: &Value,
) -> Result<Option<RequirementSetState>> {
    let Some(requirement_set_payload) = payload
        .get("requirementSet")
        .filter(|value| !value.is_null())
        .cloned()
    else {
        return Ok(None);
    };
    let mut set = compose_requirement_set_payload(runtime, state, recipient_thread_id, requirement_set_payload)?;
    validate_requirement_set(&set)?;
    if set.id.as_deref().map(str::trim).unwrap_or_default().is_empty() {
        set.id = Some(format!("requirements-{}", unix_now()));
    }
    Ok(Some(set))
}

fn compose_optional_requirement_set_payload_for_project_path(
    runtime: &BridgeRuntime,
    state: &PersistedState,
    project_path: &str,
    payload: &Value,
) -> Result<Option<RequirementSetState>> {
    let Some(requirement_set_payload) = payload
        .get("requirementSet")
        .filter(|value| !value.is_null())
        .cloned()
    else {
        return Ok(None);
    };
    let mut set = compose_requirement_set_payload_for_project_path(runtime, state, project_path, requirement_set_payload)?;
    validate_requirement_set(&set)?;
    if set.id.as_deref().map(str::trim).unwrap_or_default().is_empty() {
        set.id = Some(format!("requirements-{}", unix_now()));
    }
    Ok(Some(set))
}

fn merge_selected_composables_into_set(
    set: &mut RequirementSetState,
    available: &BTreeMap<String, RequirementComposableState>,
    selected: Vec<String>,
) -> Result<()> {
    let selected_set: BTreeSet<String> = selected.iter().cloned().collect();
    for id in &selected_set {
        let Some(composable) = available.get(id) else {
            bail!("unknown requirements composable `{id}`");
        };
        for conflict in &composable.conflicts_with {
            if selected_set.contains(conflict) {
                bail!("requirements composable `{id}` conflicts with `{conflict}`");
            }
        }
    }

    let mut merged = Vec::<RequirementState>::new();
    for id in selected {
        let composable = available
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown requirements composable `{id}`"))?;
        merge_requirement_items(&mut merged, &composable.requirements)?;
    }
    merge_requirement_items(&mut merged, &set.requirements)?;
    set.requirements = merged;
    Ok(())
}

fn selected_composable_ids(payload: &Value) -> Result<Vec<String>> {
    let Some(value) = payload
        .get("includeComposables")
        .or_else(|| payload.get("composables"))
    else {
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        bail!("includeComposables must be an array of composable ids");
    };
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    for item in items {
        let Some(id) = item.as_str().map(str::trim).filter(|id| !id.is_empty()) else {
            bail!("includeComposables entries must be non-empty strings");
        };
        if seen.insert(id.to_string()) {
            selected.push(id.to_string());
        }
    }
    Ok(selected)
}

fn permanent_and_explicit_composable_ids(
    state: &PersistedState,
    recipient_thread_id: &str,
    explicit: Vec<String>,
) -> Vec<String> {
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    for id in permanent_requirement_composable_ids_for_thread(state, recipient_thread_id)
        .into_iter()
        .chain(explicit)
    {
        if seen.insert(id.clone()) {
            selected.push(id);
        }
    }
    selected
}

fn permanent_and_explicit_composable_ids_for_project_path(
    state: &PersistedState,
    project_path: &str,
    explicit: Vec<String>,
) -> Vec<String> {
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    for id in project_by_root(state, project_path)
        .map(permanent_requirement_composable_ids)
        .unwrap_or_default()
        .into_iter()
        .chain(explicit)
    {
        if seen.insert(id.clone()) {
            selected.push(id);
        }
    }
    selected
}

fn permanent_requirement_composable_ids_for_thread(
    state: &PersistedState,
    recipient_thread_id: &str,
) -> Vec<String> {
    project_for_thread(state, recipient_thread_id)
        .map(permanent_requirement_composable_ids)
        .unwrap_or_default()
}

fn permanent_requirement_composable_ids(project: &PersistedProjectState) -> Vec<String> {
    project
        .configs
        .get(PROJECT_PERMANENT_REQUIREMENT_COMPOSABLES_KEY)
        .and_then(Value::as_array)
        .map(|items| {
            let mut seen = BTreeSet::new();
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .filter_map(|id| {
                    let id = id.to_string();
                    seen.insert(id.clone()).then_some(id)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_composable_id_array(value: &Value, field: &str) -> Result<Vec<String>> {
    let Some(items) = value.as_array() else {
        bail!("{field} must be an array of composable ids");
    };
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    for item in items {
        let Some(id) = item.as_str().map(str::trim).filter(|id| !id.is_empty()) else {
            bail!("{field} entries must be non-empty strings");
        };
        if seen.insert(id.to_string()) {
            selected.push(id.to_string());
        }
    }
    Ok(selected)
}

fn strip_composable_selection(mut payload: Value) -> Value {
    if let Some(object) = payload.as_object_mut() {
        object.remove("includeComposables");
        object.remove("composables");
    }
    payload
}

fn merge_requirement_items(target: &mut Vec<RequirementState>, incoming: &[RequirementState]) -> Result<()> {
    for requirement in incoming {
        if let Some(existing) = target.iter().find(|item| item.key == requirement.key) {
            if existing != requirement {
                bail!("conflicting requirement key `{}` while composing requirements", requirement.key);
            }
            continue;
        }
        target.push(requirement.clone());
    }
    Ok(())
}

fn discover_requirement_composables(
    runtime: &BridgeRuntime,
    state: &PersistedState,
    recipient_thread_id: &str,
) -> Result<BTreeMap<String, RequirementComposableState>> {
    let mut composables = BTreeMap::new();
    load_requirement_composables_from_dir(
        &global_requirement_composables_dir(runtime),
        "global",
        &mut composables,
    )?;
    if let Some(project_root) = project_root_for_thread(state, recipient_thread_id) {
        load_requirement_composables_from_dir(
            &project_root.join(".codex").join("requirements").join("composables"),
            "project",
            &mut composables,
        )?;
    }
    Ok(composables)
}

fn discover_requirement_composables_for_project_path(
    runtime: &BridgeRuntime,
    project_path: &str,
) -> Result<BTreeMap<String, RequirementComposableState>> {
    let project_root = PathBuf::from(normalize_path(project_path.to_string()));
    let mut composables = BTreeMap::new();
    load_requirement_composables_from_dir(
        &global_requirement_composables_dir(runtime),
        "global",
        &mut composables,
    )?;
    load_requirement_composables_from_dir(
        &project_root.join(".codex").join("requirements").join("composables"),
        "project",
        &mut composables,
    )?;
    Ok(composables)
}

fn global_requirement_composables_dir(runtime: &BridgeRuntime) -> PathBuf {
    if let Some(home) = env::var_os("CODEX_HOME") {
        return PathBuf::from(home).join("requirements").join("composables");
    }
    runtime
        .settings()
        .paths
        .state_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| runtime.settings().project_path.clone())
        .join("requirements")
        .join("composables")
}

fn project_root_for_thread(state: &PersistedState, thread_id: &str) -> Option<PathBuf> {
    project_for_thread(state, thread_id).and_then(|project| {
        let agent = project.agents.get(thread_id)?;
        agent
            .project_root
            .as_deref()
            .or(project.project_root.as_deref())
            .or(project.cwd.as_deref())
            .map(PathBuf::from)
    })
}

fn project_by_root<'a>(state: &'a PersistedState, project_path: &str) -> Option<&'a PersistedProjectState> {
    let normalized = normalize_path(project_path.to_string());
    state.projects.values().find(|project| {
        project
            .project_root
            .as_deref()
            .or(project.cwd.as_deref())
            .map(|root| normalize_path(root.to_string()) == normalized)
            .unwrap_or(false)
    })
}

fn project_for_thread<'a>(
    state: &'a PersistedState,
    thread_id: &str,
) -> Option<&'a PersistedProjectState> {
    state
        .projects
        .values()
        .find(|project| project.agents.contains_key(thread_id))
}

fn load_requirement_composables_from_dir(
    dir: &Path,
    scope: &str,
    out: &mut BTreeMap<String, RequirementComposableState>,
) -> Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
        let path = entry.path();
        let Some(format) = requirement_composable_file_format(&path) else {
            continue;
        };
        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read requirements composable {}", path.display()))?;
        let mut composable = parse_requirement_composable_text(&text, format, &path)?;
        if composable.id.trim().is_empty() {
            composable.id = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("composable")
                .to_string();
        }
        validate_requirement_composable(&composable, &path)?;
        composable.scope = scope.to_string();
        composable.path = path;
        out.insert(composable.id.clone(), composable);
    }
    Ok(())
}

fn requirement_composable_file_format(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|value| value.to_str()) {
        Some("json") => Some("json"),
        Some("yaml" | "yml") => Some("yaml"),
        _ => None,
    }
}

fn parse_requirement_composable_text(
    text: &str,
    format: &str,
    path: &Path,
) -> Result<RequirementComposableState> {
    match format {
        "json" => serde_json::from_str(text)
            .with_context(|| format!("invalid requirements composable {}", path.display())),
        "yaml" => serde_yaml::from_str(text)
            .with_context(|| format!("invalid requirements composable {}", path.display())),
        _ => bail!("unsupported requirements composable format `{format}`"),
    }
}

fn validate_requirement_composable(composable: &RequirementComposableState, path: &Path) -> Result<()> {
    if !composable
        .id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        bail!(
            "requirements composable id `{}` in {} must contain only letters, numbers, hyphen, or underscore",
            composable.id,
            path.display()
        );
    }
    validate_requirement_set(&RequirementSetState {
        requirements: composable.requirements.clone(),
        ..Default::default()
    })
    .with_context(|| format!("invalid requirements composable {}", path.display()))
}

fn parse_optional_requirement_set_payload(payload: &Value) -> Result<Option<RequirementSetState>> {
    let Some(requirement_set_payload) = payload
        .get("requirementSet")
        .filter(|value| !value.is_null())
        .cloned()
    else {
        return Ok(None);
    };
    let mut set = parse_requirement_set_payload(requirement_set_payload)?;
    validate_requirement_set(&set)?;
    if set.id.as_deref().map(str::trim).unwrap_or_default().is_empty() {
        set.id = Some(format!("requirements-{}", unix_now()));
    }
    Ok(Some(set))
}

async fn persist_requirements_for_thread(
    runtime: &BridgeRuntime,
    thread_id: &str,
    set: RequirementSetState,
) -> Result<PersistedState> {
    let _state_guard = runtime.lock_state_mutation().await;
    let mut state = parse_state(&runtime.state_document_value().await);
    for project in state.projects.values_mut() {
        if let Some(agent) = project.agents.get_mut(thread_id) {
            agent.requirements = Some(set);
            agent.requirement_review = None;
            project.updated_at = Some(unix_now());
            state.updated_at = Some(unix_now());
            persist_state(runtime, &state).await?;
            runtime
                .push_event(crate::models::BridgeEvent::AppStateSnapshot {
                    state: runtime.state_document_value().await,
                })
                .await;
            return Ok(state);
        }
    }
    bail!("Thread `{thread_id}` is not a tracked agent.");
}

fn validate_requirement_set(set: &RequirementSetState) -> Result<()> {
    let mut keys = std::collections::BTreeSet::new();
    for requirement in &set.requirements {
        let key = requirement.key.trim();
        if key.is_empty() {
            bail!("requirement key must be non-empty");
        }
        if key.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
            bail!("requirement key `{key}` must be semantic, not numbered");
        }
        if !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            bail!("requirement key `{key}` must contain only letters, numbers, and underscores");
        }
        if requirement.statement.trim().is_empty() {
            bail!("requirement `{key}` statement must be non-empty");
        }
        if !keys.insert(key.to_string()) {
            bail!("duplicate requirement key `{key}`");
        }
    }
    if keys.is_empty() {
        bail!("at least one requirement is required");
    }
    Ok(())
}

pub(crate) fn requirements_review_target_for_thread(
    state: &PersistedState,
    source_thread_id: &str,
    set: &RequirementSetState,
) -> Option<String> {
    if let Some(thread_id) = set
        .reviewer_thread_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(thread_id.to_string());
    }
    for project in state.projects.values() {
        if !project.agents.contains_key(source_thread_id) {
            continue;
        }
        if let Some(thread_id) = project
            .agents
            .get(source_thread_id)
            .and_then(|agent| agent.requirement_review.as_ref())
            .map(|review| review.reviewer_thread_id.as_str())
            .filter(|thread_id| {
                project.agents.get(*thread_id).is_some_and(|agent| {
                    matches!(
                        agent.role.as_deref(),
                        Some("requirements-reviewer") | Some("requirementsReviewer")
                    )
                })
            })
        {
            return Some(thread_id.to_string());
        }
        if let Some((thread_id, _)) = project.agents.iter().find(|(_, agent)| {
            matches!(
                agent.role.as_deref(),
                Some("requirements-reviewer") | Some("requirementsReviewer")
            )
            && agent.parent_thread_id.as_deref() == Some(source_thread_id)
        }) {
            return Some(thread_id.clone());
        }
        return None;
    }
    None
}

pub(crate) async fn ensure_requirements_reviewer_for_thread(
    runtime: &BridgeRuntime,
    source_thread_id: &str,
) -> Result<Option<String>> {
    let state = parse_state(&runtime.state_document_value().await);
    for project in state.projects.values() {
        let Some(source) = project.agents.get(source_thread_id) else {
            continue;
        };
        if let Some((thread_id, _)) = project.agents.iter().find(|(_, agent)| {
            matches!(
                agent.role.as_deref(),
                Some("requirements-reviewer") | Some("requirementsReviewer")
            )
            && agent.parent_thread_id.as_deref() == Some(source_thread_id)
        }) {
            return Ok(Some(thread_id.clone()));
        }

        let source_name = source
            .display_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("source agent");
        let project_path = source
            .project_root
            .clone()
            .or_else(|| project.project_root.clone())
            .unwrap_or_else(|| runtime.settings().project_path.display().to_string());
        let cwd = source
            .cwd
            .clone()
            .or_else(|| project.cwd.clone())
            .or_else(|| project.project_root.clone())
            .unwrap_or_else(|| runtime.settings().cwd.display().to_string());
        let reviewer_model =
            role_default_model(&state, Some(project_path.as_str()), Some("requirements-reviewer"));
        let reviewer_reasoning_effort = role_default_reasoning_effort(
            &state,
            Some(project_path.as_str()),
            Some("requirements-reviewer"),
        );
        let has_reviewer_model_defaults =
            reviewer_model.is_some() || reviewer_reasoning_effort.is_some();
        let reviewer_model_provider = if has_reviewer_model_defaults {
            preferred_model_provider_for_project(&state, Some(project_path.as_str()))
                .or_else(|| source.model_provider.clone())
        } else {
            source.model_provider.clone()
        };
        let base_instructions = resolve_role_instructions_for(Some("requirements-reviewer"))
            .ok()
            .flatten();
        let payload = json!({
            "displayName": format!("Requirements Reviewer: {source_name}"),
            "cwd": cwd,
            "projectPath": project_path,
            "role": "requirements-reviewer",
            "parentAgentId": source_thread_id,
            "approvalPolicy": "never",
            "sandboxMode": "workspace-write",
            "networkAccess": false,
            "modelID": reviewer_model.or_else(|| source.model.clone()),
            "modelProvider": reviewer_model_provider,
            "reasoningEffort": reviewer_reasoning_effort.or_else(|| source.reasoning_effort.clone()),
            "serviceTier": source.service_tier.clone(),
            "approvalsReviewer": source.approvals_reviewer.clone(),
            "personality": source.personality.clone(),
            "config": source.config.clone(),
            "baseInstructions": base_instructions,
            "developerInstructions": source.developer_instructions.clone(),
            "persistExtendedHistory": source.persist_extended_history,
            "serviceName": source.service_name.clone(),
            "ephemeral": source.ephemeral,
            "dynamicTools": source.dynamic_tools.clone(),
        });
        let reviewer = spawn_agent(runtime, &payload).await?;
        return Ok(Some(reviewer.id));
    }
    Ok(None)
}

pub(crate) fn requirements_review_prompt(
    set: &RequirementSetState,
    source_label: &str,
    _source_thread_id: &str,
    _turn_id: &str,
    claim_text: &str,
) -> String {
    let mut prompt = format!(
        "Perform an adversarial Requirements Review.\n\nReview subject: {source_label}\n\nStructured responses use `summary` plus `requirements`.\n- Use `requirements: null` only for reviewer progress/commentary.\n- When finishing review, `requirements` must be the object containing every requirement verdict plus `overallVerdict` and `route`.\n\nRules:\n- Compare every canonical requirement against the actual work and available evidence, even if the source claim packet only contains currently unresolved requirements.\n- Previously passed requirements remain binding. Re-fail any previously passed requirement if later work regresses it.\n- If the schema offers `{{\"verdict\":\"stillPassing\"}}`, use it only after checking that a previously passed requirement still passes for the same reason.\n- Do not use `stillPassing` when a requirement is new, failed, blocked, waived, changed by the latest work, or lacks enough evidence to confirm it still passes.\n- For unrelated requirements or requirements that are repeatedly passing because nothing relevant changed, keep `reason` and `evidenceAssessment` brief, or use `stillPassing` when the schema allows it.\n- Fail missing, weak, circular, or unverifiable evidence.\n- Reject fake blockers.\n- Accept true external blockers only with concrete proof.\n- Never implement fixes and never relax requirements.\n- Shell/chrome or scope exclusions cannot erase core in-scope requirements.\n\nRequirements:\n"
    );
    for requirement in &set.requirements {
        prompt.push_str(&format!(
            "- `{}` [{}; verification={}]: {}\n",
            requirement.key, requirement.severity, requirement.verification_method, requirement.statement
        ));
    }
    if let Some(summary) = compact_requirements_claim_summary(claim_text) {
        prompt.push_str("\nSource evidence summary:\n");
        prompt.push_str(&summary);
    }
    prompt
}

fn compact_requirements_claim_summary(claim_text: &str) -> Option<String> {
    let payload = serde_json::from_str::<Value>(claim_text.trim()).ok()?;
    let object = payload.as_object()?;
    let mut lines = Vec::new();
    if let Some(summary) = object.get("summary").and_then(Value::as_str) {
        if !summary.trim().is_empty() {
            lines.push(format!("- Summary: {}", summary.trim()));
        }
    }
    let claims_object = object
        .get("requirements")
        .and_then(Value::as_object)
        .unwrap_or(object);
    for (key, value) in claims_object {
        let Some(claim) = value.as_object() else {
            continue;
        };
        let claim_value = claim.get("claim").and_then(Value::as_str).unwrap_or("unknown");
        let risk = claim.get("risk").and_then(Value::as_str).unwrap_or("unknown");
        lines.push(format!("- `{key}`: claim={claim_value}; risk={risk}"));
        if let Some(justification) = claim.get("justification").and_then(Value::as_str) {
            if !justification.trim().is_empty() {
                lines.push(format!("  Justification: {}", justification.trim()));
            }
        }
        if let Some(evidence) = claim.get("evidence").and_then(Value::as_array) {
            for item in evidence.iter().filter_map(Value::as_str).filter(|item| !item.trim().is_empty()) {
                lines.push(format!("  Evidence: {}", item.trim()));
            }
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

pub(crate) async fn record_requirement_packet(
    runtime: &BridgeRuntime,
    source_thread_id: &str,
    packet: RequirementPacketState,
) -> Result<()> {
    let _state_guard = runtime.lock_state_mutation().await;
    let mut state = parse_state(&runtime.state_document_value().await);
    for project in state.projects.values_mut() {
        if let Some(agent) = project.agents.get_mut(source_thread_id) {
            agent.requirement_packets.push(packet);
            persist_state(runtime, &state).await?;
            runtime
                .push_event(crate::models::BridgeEvent::AppStateSnapshot {
                    state: runtime.state_document_value().await,
                })
                .await;
            return Ok(());
        }
    }
    Ok(())
}

pub(crate) async fn mark_requirements_review_in_progress(
    runtime: &BridgeRuntime,
    source_thread_id: &str,
    reviewer_thread_id: &str,
    set: &RequirementSetState,
    claim_payload: Value,
) -> Result<()> {
    let _state_guard = runtime.lock_state_mutation().await;
    let mut state = parse_state(&runtime.state_document_value().await);
    for project in state.projects.values_mut() {
        let has_source = project.agents.contains_key(source_thread_id);
        let has_reviewer = project.agents.contains_key(reviewer_thread_id);
        if !has_source {
            continue;
        }
        if let Some(source) = project.agents.get_mut(source_thread_id) {
            source.requirement_review = Some(RequirementReviewBindingState {
                source_thread_id: source_thread_id.to_string(),
                reviewer_thread_id: reviewer_thread_id.to_string(),
                requirement_set_id: set.id.clone(),
                status: "inReview".to_string(),
                latest_claim_packet: Some(claim_payload),
                latest_verdict_packet: None,
                updated_at: unix_now(),
            });
        }
        if has_reviewer
            && let Some(reviewer) = project.agents.get_mut(reviewer_thread_id)
        {
            reviewer.parent_thread_id = Some(source_thread_id.to_string());
            reviewer.hidden_from_peer_list = true;
        }
        project.updated_at = Some(unix_now());
        state.updated_at = Some(unix_now());
        persist_state(runtime, &state).await?;
        runtime
            .push_event(crate::models::BridgeEvent::AppStateSnapshot {
                state: runtime.state_document_value().await,
            })
            .await;
        return Ok(());
    }
    Ok(())
}

pub(crate) async fn mark_requirements_review_verdict(
    runtime: &BridgeRuntime,
    source_thread_id: &str,
    reviewer_thread_id: &str,
    verdict_payload: Value,
) -> Result<()> {
    let _state_guard = runtime.lock_state_mutation().await;
    let mut state = parse_state(&runtime.state_document_value().await);
    for project in state.projects.values_mut() {
        let Some(source) = project.agents.get_mut(source_thread_id) else {
            continue;
        };
        let invalid_still_passing = source
            .requirements
            .as_ref()
            .map(|requirements| verdict_has_invalid_still_passing(requirements, &verdict_payload))
            .unwrap_or(false);
        let status = if invalid_still_passing {
            "failed".to_string()
        } else {
            requirement_status_from_verdict(&verdict_payload)
        };
        if let Some(requirements) = source.requirements.as_mut() {
            update_requirement_review_progress(requirements, &verdict_payload);
        }
        let is_terminal = matches!(status.as_str(), "passed" | "waiverAccepted");
        if is_terminal
            && let Some(requirements) = source.requirements.as_mut()
        {
            requirements.active = false;
        }
        if status == "passed" {
            source.requirement_review = None;
            if let Some(reviewer) = project.agents.get_mut(reviewer_thread_id) {
                reviewer.parent_thread_id = None;
                reviewer.hidden_from_peer_list = true;
            }
        } else if status == "waiverAccepted" {
            let latest_claim_packet = source
                .requirement_review
                .as_ref()
                .and_then(|binding| binding.latest_claim_packet.clone());
            source.requirement_review = Some(RequirementReviewBindingState {
                source_thread_id: source_thread_id.to_string(),
                reviewer_thread_id: reviewer_thread_id.to_string(),
                requirement_set_id: source.requirements.as_ref().and_then(|set| set.id.clone()),
                status,
                latest_claim_packet,
                latest_verdict_packet: Some(verdict_payload),
                updated_at: unix_now(),
            });
            if let Some(reviewer) = project.agents.get_mut(reviewer_thread_id) {
                reviewer.parent_thread_id = None;
                reviewer.hidden_from_peer_list = true;
            }
        } else if let Some(binding) = source.requirement_review.as_mut() {
            binding.reviewer_thread_id = reviewer_thread_id.to_string();
            binding.status = status;
            binding.latest_verdict_packet = Some(verdict_payload);
            binding.updated_at = unix_now();
        } else {
            source.requirement_review = Some(RequirementReviewBindingState {
                source_thread_id: source_thread_id.to_string(),
                reviewer_thread_id: reviewer_thread_id.to_string(),
                requirement_set_id: source.requirements.as_ref().and_then(|set| set.id.clone()),
                status,
                latest_claim_packet: None,
                latest_verdict_packet: Some(verdict_payload),
                updated_at: unix_now(),
            });
        }
        project.updated_at = Some(unix_now());
        state.updated_at = Some(unix_now());
        persist_state(runtime, &state).await?;
        runtime
            .push_event(crate::models::BridgeEvent::AppStateSnapshot {
                state: runtime.state_document_value().await,
            })
            .await;
        return Ok(());
    }
    Ok(())
}

fn update_requirement_review_progress(set: &mut RequirementSetState, verdict_payload: &Value) {
    let now = unix_now();
    let Some(verdict_object) = verdict_payload.as_object() else {
        return;
    };
    for requirement in &set.requirements {
        let Some(verdict) = verdict_object
            .get(requirement.key.as_str())
            .and_then(|value| value.get("verdict"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let status = match verdict {
            "pass" => "passed",
            "stillPassing" if requirement_review_previously_passed(set, requirement.key.as_str()) => "passed",
            "stillPassing" => "failed",
            "fail" | "rejectedBlocked" => "failed",
            "acceptedBlocked" | "waiverRequired" => "blocked",
            "waiverAccepted" => "waived",
            _ => "unresolved",
        };
        set.review_progress.insert(
            requirement.key.clone(),
            RequirementReviewProgressState {
                status: status.to_string(),
                updated_at: Some(now),
            },
        );
    }
}

fn verdict_has_invalid_still_passing(set: &RequirementSetState, verdict_payload: &Value) -> bool {
    let Some(verdict_object) = verdict_payload.as_object() else {
        return false;
    };
    set.requirements.iter().any(|requirement| {
        verdict_object
            .get(requirement.key.as_str())
            .and_then(|value| value.get("verdict"))
            .and_then(Value::as_str)
            == Some("stillPassing")
            && !requirement_review_previously_passed(set, requirement.key.as_str())
    })
}

fn requirement_status_from_verdict(verdict_payload: &Value) -> String {
    match verdict_payload.get("overallVerdict").and_then(Value::as_str) {
        Some("pass") => "passed",
        Some("fail") => "failed",
        Some("acceptedBlocked") => "blocked",
        Some("rejectedBlocked") => "failed",
        Some("needsHumanWaiver") => "waiverRequired",
        Some("waiverAccepted") => "waiverAccepted",
        _ => "inReview",
    }
    .to_string()
}

pub async fn orchestrator_spawn_agent(
    runtime: &BridgeRuntime,
    sender_thread_id: &str,
    name: &str,
    prompt: &str,
    _cwd: Option<&str>,
    role: Option<&str>,
    issue_number: Option<u64>,
    requirement_set: Option<Value>,
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
        "requirements-reviewer" | "requirementsReviewer" => "requirements-reviewer",
        other => bail!("Orchestrators may only spawn worker, qa, or requirements-reviewer agents, not `{other}`."),
    };
    let authoritative =
        authoritative_spawn_defaults_for_project(&state, sender.project_path.as_str(), Some(target_role)).ok_or_else(
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
    let approval_policy = if target_role == "requirements-reviewer" {
        Some("never".to_string())
    } else {
        authoritative.approval_policy
    };
    let payload = json!({
        "displayName": name,
        "initialPrompt": prompt,
        "cwd": authoritative.cwd,
        "projectPath": sender.project_path,
        "role": target_role,
        "parentAgentId": sender_thread_id,
        "approvalPolicy": approval_policy,
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
        "requirementSet": requirement_set.unwrap_or(Value::Null),
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
    let direct_sender = tracked_agent_record_for_thread(&state, &running, sender_thread_id)
        .ok_or_else(|| anyhow::anyhow!("Thread `{sender_thread_id}` is not tracked by the bridge."))?;
    let requested_self = recipient_thread_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|thread_id| thread_id == sender_thread_id)
        .unwrap_or(false)
        && recipient_name.map(str::trim).filter(|value| !value.is_empty()).is_none();
    let (sender, recipient) = if requested_self {
        (direct_sender.clone(), direct_sender)
    } else {
        let records = all_agent_records(&state, &running);
        let scoped = scoped_agent_context(&records, sender_thread_id, true)?;
        let recipient =
            resolve_scoped_recipient(&scoped.visible, recipient_thread_id, recipient_name, project_path)?;
        (scoped.sender, recipient)
    };
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
    for pruned_thread_id in &pruned_old {
        runtime.prune_thread_local(pruned_thread_id).await?;
    }
    let app_server_archive_ids = if pruned_old.is_empty() {
        vec![recipient.thread_id.clone()]
    } else {
        pruned_old
    };
    for archive_thread_id in app_server_archive_ids {
        let _ = app_server_request_json(
            runtime,
            "thread/archive",
            json!({"threadId": archive_thread_id}),
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
    if !orchestrator_can_archive_agent_role(&recipient.role) {
        bail!(
            "Orchestrators may only archive worker and qa agents, not `{}`.",
            recipient.role
        );
    }
    let already_archived = state
        .projects
        .values()
        .all(|project| !project.agents.contains_key(&recipient.thread_id))
        && records.iter().all(|record| record.thread_id != recipient.thread_id || record.is_archived);
    if !already_archived {
        archive_thread_filtered(runtime, &recipient.thread_id, orchestrator_can_archive_agent_role).await?;
    }
    Ok(json!({
        "recipientThreadId": recipient.thread_id,
        "recipientDisplayName": recipient.display_name,
        "alreadyArchived": already_archived,
    }))
}

fn orchestrator_can_archive_agent_role(role: &str) -> bool {
    matches!(role, "worker" | "qa")
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

pub(crate) async fn archive_thread(runtime: &BridgeRuntime, thread_id: &str) -> Result<()> {
    archive_thread_filtered(runtime, thread_id, |_| true).await
}

async fn archive_thread_filtered<F>(runtime: &BridgeRuntime, thread_id: &str, can_archive_role: F) -> Result<()>
where
    F: Fn(&str) -> bool + Copy,
{
    let mut state = parse_state(&runtime.state_document_value().await);
    if let Some(agent) = agent_state_for_thread(&state, thread_id) {
        if let Some(message) = manifest_archive_denial_for_agent(agent) {
            bail!("{message}");
        }
    }
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
    }
    let pruned_thread_ids = prune_archived_thread_locally_filtered(&mut state, thread_id, can_archive_role);
    if !pruned_thread_ids.is_empty() {
        persist_state(runtime, &state).await?;
        for pruned_thread_id in &pruned_thread_ids {
            runtime.prune_thread_local(pruned_thread_id).await?;
        }
    }
    if runtime.info().await.connection_status != "connected" {
        return Ok(());
    }
    let app_server_archive_ids = if pruned_thread_ids.is_empty() {
        vec![thread_id.to_string()]
    } else {
        pruned_thread_ids
    };
    for archive_thread_id in app_server_archive_ids {
        if let Err(error) = app_server_request_json(runtime, "thread/archive", json!({"threadId": archive_thread_id})).await {
            let message = error.to_string();
            if !message.contains("no rollout found for thread id") && !message.contains("\"code\": -32600") {
                return Err(error);
            }
        }
    }
    Ok(())
}

pub(crate) fn prune_archived_thread_locally(state: &mut PersistedState, thread_id: &str) -> Vec<String> {
    prune_archived_thread_locally_filtered(state, thread_id, |_| true)
}

fn prune_archived_thread_locally_filtered<F>(
    state: &mut PersistedState,
    thread_id: &str,
    can_archive_role: F,
) -> Vec<String>
where
    F: Fn(&str) -> bool + Copy,
{
    let thread_ids = linked_archive_thread_ids_filtered(state, thread_id, can_archive_role);
    let mut pruned_thread_ids = BTreeSet::new();
    let mut changed = false;
    for project in state.projects.values_mut() {
        let mut project_changed = false;
        for archive_thread_id in &thread_ids {
            if project.agents.remove(archive_thread_id).is_some() {
                pruned_thread_ids.insert(archive_thread_id.clone());
                project_changed = true;
            }
            if project.orchestrator_thread_id.as_deref() == Some(archive_thread_id.as_str()) {
                project.orchestrator_thread_id = None;
                project_changed = true;
            }
        }
        let mut next_groups = Vec::new();
        for mut group in project.thread_groups.clone() {
            let original = group.thread_ids.len();
            group.thread_ids.retain(|id| !thread_ids.contains(id));
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
    pruned_thread_ids.into_iter().collect()
}

fn linked_archive_thread_ids_filtered<F>(
    state: &PersistedState,
    thread_id: &str,
    can_archive_role: F,
) -> BTreeSet<String>
where
    F: Fn(&str) -> bool + Copy,
{
    let mut ids = BTreeSet::from([thread_id.to_string()]);
    let mut changed = true;
    while changed {
        changed = false;
        for project in state.projects.values() {
            for (agent_thread_id, agent) in &project.agents {
                let role = agent.role.as_deref().unwrap_or("worker");
                if !can_archive_role(role) {
                    continue;
                }
                let source_review_child = ids.iter().any(|id| {
                    project
                        .agents
                        .get(id)
                        .and_then(|source| source.requirement_review.as_ref())
                        .map(|review| review.reviewer_thread_id.as_str() == agent_thread_id.as_str())
                        .unwrap_or(false)
                });
                let parent_child = agent
                    .parent_thread_id
                    .as_deref()
                    .map(|parent| ids.contains(parent))
                    .unwrap_or(false);
                if (source_review_child || parent_child) && ids.insert(agent_thread_id.clone()) {
                    changed = true;
                }
            }
        }
    }
    ids
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
    let mut visible = match scoped_agent_context(&records, sender_thread_id, true) {
        Ok(scoped) => scoped
            .visible
            .into_iter()
            .map(|record| record.thread_id)
            .collect::<std::collections::BTreeSet<_>>(),
        Err(_error) if agent_state_for_thread(&state, sender_thread_id).is_some() => {
            std::collections::BTreeSet::new()
        }
        Err(error) => return Err(error),
    };
    visible.insert(sender_thread_id.to_string());
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
        .send_server_response(
            approval.request_id.clone(),
            approval_response_payload(Some(&approval), decision),
        )
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

fn approval_response_payload(approval: Option<&PendingApproval>, decision: &str) -> Value {
    if approval
        .map(|approval| approval.kind == PendingApprovalKind::McpElicitation)
        .unwrap_or(false)
    {
        let action = match decision {
            "accept" | "approve" => "accept",
            "cancel" => "cancel",
            _ => "decline",
        };
        return json!({
            "action": action,
            "content": null,
            "_meta": null,
        });
    }

    json!({ "decision": decision })
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
        CommandExecutionRequestApprovalParams, JSONRPCMessage, JSONRPCNotification, JSONRPCRequest,
        JSONRPCResponse, RequestId, ServerNotification, ServerRequest, Turn, TurnCompletedNotification,
        TurnStartedNotification, TurnStatus,
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
    use tokio::{net::TcpListener, sync::{mpsc, oneshot}};
    use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
        project_alpha.agents.insert(
            "planner-a".to_string(),
            PersistedAgentState {
                display_name: Some("Planner A".to_string()),
                role: Some("planner".to_string()),
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

    #[test]
    fn operator_role_instructions_resolve_to_operator_role_file() {
        let temp = TempDir::new().expect("tempdir");
        let roles = temp.path().join(".codex/roles");
        fs::create_dir_all(&roles).expect("roles dir");
        fs::write(roles.join("operator.md"), "operator role instructions\n")
            .expect("operator role write");

        let instructions =
            resolve_role_instructions_for_home(Some(temp.path().to_path_buf()), Some("operator"))
                .expect("operator instructions");

        assert_eq!(instructions.as_deref(), Some("operator role instructions"));
    }

    #[test]
    fn operator_start_params_use_operator_role_file_when_not_overridden() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp = TempDir::new().expect("tempdir");
        let roles = temp.path().join(".codex/roles");
        fs::create_dir_all(&roles).expect("roles dir");
        fs::write(roles.join("operator.md"), "operator role instructions\n")
            .expect("operator role write");
        let old_home = env::var_os("HOME");
        unsafe {
            env::set_var("HOME", temp.path());
        }

        let state = PersistedState::default();
        let settings = explicit_thread_settings_for_new_thread(
            &state,
            &json!({}),
            temp.path().to_str().expect("temp path"),
            temp.path().to_str().expect("temp path"),
            Some("operator"),
            None,
            None,
            None,
        );

        if let Some(old_home) = old_home {
            unsafe {
                env::set_var("HOME", old_home);
            }
        } else {
            unsafe {
                env::remove_var("HOME");
            }
        }

        assert_eq!(
            settings.base_instructions.as_deref(),
            Some("operator role instructions")
        );
    }

    fn sample_requirement_set() -> RequirementSetState {
        RequirementSetState {
            active: true,
            enforce_on_turns: true,
            requirements: vec![
                RequirementState {
                    key: "nativeGuiIsSourceOfTruth".to_string(),
                    statement: "The web GUI must mirror the native Flutter GUI.".to_string(),
                    severity: "blocker".to_string(),
                    claim_schema_description: None,
                    verdict_schema_description: None,
                    verification_method: "diffReview".to_string(),
                },
                RequirementState {
                    key: "noInventedWebsocketEventShapes".to_string(),
                    statement: "Do not invent websocket or HTTP event shapes.".to_string(),
                    severity: "blocker".to_string(),
                    claim_schema_description: None,
                    verdict_schema_description: None,
                    verification_method: "diffReview".to_string(),
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn prune_missing_project_roots_removes_stale_projects_and_repairs_selection() {
        let temp = TempDir::new().expect("tempdir");
        let existing_root = temp.path().join("existing");
        std::fs::create_dir_all(&existing_root).expect("existing root");
        let missing_root = temp.path().join("deleted");

        let mut state = PersistedState::default();
        state.selected_project_id = Some("missing-id".to_string());
        state.projects.insert(
            "existing".to_string(),
            PersistedProjectState {
                id: Some("existing-id".to_string()),
                name: Some("Existing".to_string()),
                project_root: Some(existing_root.display().to_string()),
                cwd: Some(existing_root.display().to_string()),
                ..Default::default()
            },
        );
        state.projects.insert(
            "missing".to_string(),
            PersistedProjectState {
                id: Some("missing-id".to_string()),
                name: Some("Missing".to_string()),
                project_root: Some(missing_root.display().to_string()),
                cwd: Some(missing_root.display().to_string()),
                ..Default::default()
            },
        );

        let removed = prune_missing_project_roots(&mut state);

        assert_eq!(removed.len(), 1);
        assert!(removed[0].contains("Missing"));
        assert!(state.projects.contains_key("existing"));
        assert!(!state.projects.contains_key("missing"));
        assert_eq!(state.selected_project_id.as_deref(), Some("existing-id"));
        assert!(state.updated_at.is_some());
    }

    #[test]
    fn parse_state_lossy_keeps_valid_agents_when_one_record_is_malformed() {
        let state = parse_state(&json!({
            "globalConfigs": {
                "approvalPolicy": "never"
            },
            "projects": {
                "alpha": {
                    "id": "alpha-id",
                    "name": "Alpha",
                    "projectRoot": "/alpha",
                    "cwd": "/alpha",
                    "agents": {
                        "good-thread": {
                            "displayName": "Good",
                            "role": "orchestrator",
                            "projectRoot": "/alpha"
                        },
                        "bad-thread": {
                            "displayName": "Bad",
                            "role": "worker",
                            "requirementPackets": [
                                {
                                    "packetType": "claim",
                                    "sourceThreadId": "bad-thread",
                                    "payload": {},
                                    "createdAt": "not-a-number"
                                }
                            ]
                        }
                    }
                }
            }
        }));

        let project = state.projects.get("alpha").expect("project");
        assert!(project.agents.contains_key("good-thread"));
        assert!(!project.agents.contains_key("bad-thread"));
    }

    #[test]
    fn requirements_claim_schema_uses_summary_and_nullable_requirements_object() {
        let schema = requirements_worker_claim_schema(&sample_requirement_set());
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("required array");
        assert_eq!(
            required,
            &vec![json!("summary"), json!("requirements")]
        );
        let requirements_schema = &schema["properties"]["requirements"];
        assert_eq!(requirements_schema["type"], json!(["object", "null"]));
        assert_eq!(
            requirements_schema["required"],
            json!(["nativeGuiIsSourceOfTruth", "noInventedWebsocketEventShapes"])
        );
        assert!(requirements_schema["properties"].get("nativeGuiIsSourceOfTruth").is_some());
        assert!(requirements_schema["properties"].get("noInventedWebsocketEventShapes").is_some());
        assert!(schema["properties"].get("finalDisposition").is_none());
    }

    #[test]
    fn requirements_verdict_schema_mirrors_requirement_keys() {
        let schema = requirements_verdict_schema(&sample_requirement_set());
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("required array");
        assert_eq!(required, &vec![json!("summary"), json!("requirements")]);
        let requirements_schema = &schema["properties"]["requirements"];
        assert_eq!(requirements_schema["type"], json!(["object", "null"]));
        let verdict_required = requirements_schema
            .get("required")
            .and_then(Value::as_array)
            .expect("verdict required array");
        assert!(verdict_required.iter().any(|value| value.as_str() == Some("nativeGuiIsSourceOfTruth")));
        assert!(verdict_required.iter().any(|value| value.as_str() == Some("noInventedWebsocketEventShapes")));
        assert!(verdict_required.iter().any(|value| value.as_str() == Some("overallVerdict")));
        assert!(verdict_required.iter().any(|value| value.as_str() == Some("route")));
    }

    #[test]
    fn worker_claim_schema_is_full_before_review_progress_exists() {
        let schema = requirements_worker_claim_schema(&sample_requirement_set());
        let requirements_schema = &schema["properties"]["requirements"];
        assert_eq!(
            requirements_schema["required"],
            json!(["nativeGuiIsSourceOfTruth", "noInventedWebsocketEventShapes"])
        );
    }

    #[test]
    fn worker_claim_schema_only_includes_unresolved_requirements_after_partial_review() {
        let mut set = sample_requirement_set();
        set.review_progress.insert(
            "nativeGuiIsSourceOfTruth".to_string(),
            RequirementReviewProgressState {
                status: "passed".to_string(),
                updated_at: Some(100),
            },
        );
        set.review_progress.insert(
            "noInventedWebsocketEventShapes".to_string(),
            RequirementReviewProgressState {
                status: "failed".to_string(),
                updated_at: Some(100),
            },
        );

        let worker_schema = requirements_worker_claim_schema(&set);
        let worker_requirements = &worker_schema["properties"]["requirements"];
        assert_eq!(worker_requirements["required"], json!(["noInventedWebsocketEventShapes"]));
        assert!(worker_requirements["properties"].get("nativeGuiIsSourceOfTruth").is_none());
        assert!(worker_requirements["properties"].get("noInventedWebsocketEventShapes").is_some());

        let reviewer_schema = requirements_verdict_schema(&set);
        let reviewer_required = reviewer_schema["properties"]["requirements"]["required"]
            .as_array()
            .expect("reviewer required");
        assert!(reviewer_required.iter().any(|value| value.as_str() == Some("nativeGuiIsSourceOfTruth")));
        assert!(reviewer_required.iter().any(|value| value.as_str() == Some("noInventedWebsocketEventShapes")));
    }

    #[test]
    fn reviewer_verdict_schema_offers_still_passing_only_for_previously_passed_requirements() {
        let mut set = sample_requirement_set();
        set.review_progress.insert(
            "nativeGuiIsSourceOfTruth".to_string(),
            RequirementReviewProgressState {
                status: "passed".to_string(),
                updated_at: Some(100),
            },
        );
        set.review_progress.insert(
            "noInventedWebsocketEventShapes".to_string(),
            RequirementReviewProgressState {
                status: "failed".to_string(),
                updated_at: Some(100),
            },
        );

        let schema = requirements_verdict_schema(&set);
        let requirements = &schema["properties"]["requirements"]["properties"];
        let passed_schema = &requirements["nativeGuiIsSourceOfTruth"];
        let failed_schema = &requirements["noInventedWebsocketEventShapes"];
        let any_of = passed_schema["anyOf"].as_array().expect("passed requirement anyOf");
        assert_eq!(any_of.len(), 2);
        assert_eq!(any_of[0]["required"], json!(["verdict", "reason", "evidenceAssessment", "requiredCorrection"]));
        assert_eq!(any_of[1]["properties"]["verdict"]["enum"], json!(["stillPassing"]));
        assert_eq!(any_of[1]["required"], json!(["verdict"]));
        assert!(failed_schema.get("anyOf").is_none());
        assert_eq!(
            failed_schema["required"],
            json!(["verdict", "reason", "evidenceAssessment", "requiredCorrection"])
        );

        let reviewer_required = schema["properties"]["requirements"]["required"]
            .as_array()
            .expect("reviewer required");
        assert!(reviewer_required.iter().any(|value| value.as_str() == Some("nativeGuiIsSourceOfTruth")));
        assert!(reviewer_required.iter().any(|value| value.as_str() == Some("noInventedWebsocketEventShapes")));
    }

    #[test]
    fn reviewer_prompt_preserves_full_set_and_mentions_regression_review() {
        let mut set = sample_requirement_set();
        set.review_progress.insert(
            "nativeGuiIsSourceOfTruth".to_string(),
            RequirementReviewProgressState {
                status: "passed".to_string(),
                updated_at: Some(100),
            },
        );

        let prompt = requirements_review_prompt(
            &set,
            "Worker",
            "worker-1",
            "turn-1",
            r#"{"summary":"fixed one item","requirements":{"noInventedWebsocketEventShapes":{"claim":"satisfied","evidence":["test"],"justification":"fixed","risk":"low"}}}"#,
        );
        assert!(prompt.contains("Compare every canonical requirement"));
        assert!(prompt.contains("Re-fail any previously passed requirement"));
        assert!(prompt.contains(r#"{"verdict":"stillPassing"}"#));
        assert!(prompt.contains("only after checking that a previously passed requirement still passes for the same reason"));

        assert!(prompt.contains("`nativeGuiIsSourceOfTruth`"));
        assert!(prompt.contains("`noInventedWebsocketEventShapes`"));
        assert!(prompt.contains("Previously passed requirements remain binding"));
        assert!(prompt.contains("keep `reason` and `evidenceAssessment` brief"));
    }

    #[tokio::test]
    async fn requirements_composables_use_recipient_project_override() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let global_dir = temp.path().join("requirements").join("composables");
        std::fs::create_dir_all(&global_dir).expect("global composables dir");
        std::fs::write(
            global_dir.join("shared.json"),
            r#"{"id":"shared","title":"Global","requirements":[{"key":"globalRequirement","statement":"Global statement.","severity":"high","verificationMethod":"manualEvidence"}]}"#,
        )
        .expect("write global composable");
        let project_root = temp.path().join("project");
        let project_dir = project_root.join(".codex").join("requirements").join("composables");
        std::fs::create_dir_all(&project_dir).expect("project composables dir");
        std::fs::write(
            project_dir.join("shared.json"),
            r#"{"id":"shared","title":"Project","requirements":[{"key":"projectRequirement","statement":"Project statement.","severity":"blocker","verificationMethod":"sourceInspection"}]}"#,
        )
        .expect("write project composable");

        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": project_root.display().to_string(),
                        "cwd": project_root.display().to_string(),
                        "orchestratorThreadID": "orch-1",
                        "agents": {
                            "orch-1": {"displayName": "Orch", "role": "orchestrator", "projectRoot": project_root.display().to_string()},
                            "worker-1": {"displayName": "Worker", "role": "worker", "projectRoot": project_root.display().to_string()}
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let payload = orchestrator_requirement_composables(
            &runtime,
            "orch-1",
            Some("worker-1"),
            None,
            None,
        )
        .await
        .expect("list composables");
        let items = payload["items"].as_array().expect("items");
        let shared = items
            .iter()
            .find(|item| item["id"] == json!("shared"))
            .expect("shared composable");
        assert_eq!(shared["scope"], json!("project"));
        assert_eq!(shared["requirements"][0]["key"], json!("projectRequirement"));
    }

    #[tokio::test]
    async fn requirements_composables_load_yaml_and_yml_files() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let project_root = temp.path().join("project");
        let project_dir = project_root.join(".codex").join("requirements").join("composables");
        std::fs::create_dir_all(&project_dir).expect("project composables dir");
        std::fs::write(
            project_dir.join("yaml-composable.yaml"),
            r#"
id: yaml-composable
title: YAML Composable
description: Loaded from YAML.
appliesTo:
  - code
requirements:
  - key: yamlRequirement
    statement: YAML statement.
    severity: blocker
    verificationMethod: commandEvidence
"#,
        )
        .expect("write yaml composable");
        std::fs::write(
            project_dir.join("yml-composable.yml"),
            r#"
id: yml-composable
title: YML Composable
requirements:
  - key: ymlRequirement
    statement: YML statement.
    severity: high
    verificationMethod: sourceInspection
"#,
        )
        .expect("write yml composable");

        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": project_root.display().to_string(),
                        "cwd": project_root.display().to_string(),
                        "orchestratorThreadID": "orch-1",
                        "agents": {
                            "orch-1": {"displayName": "Orch", "role": "orchestrator", "projectRoot": project_root.display().to_string()},
                            "worker-1": {"displayName": "Worker", "role": "worker", "projectRoot": project_root.display().to_string()}
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let payload = orchestrator_requirement_composables(
            &runtime,
            "orch-1",
            Some("worker-1"),
            None,
            None,
        )
        .await
        .expect("list composables");
        let items = payload["items"].as_array().expect("items");
        let yaml = items
            .iter()
            .find(|item| item["id"] == "yaml-composable")
            .expect("yaml composable");
        let yml = items
            .iter()
            .find(|item| item["id"] == "yml-composable")
            .expect("yml composable");
        assert_eq!(yaml["description"], json!("Loaded from YAML."));
        assert_eq!(yaml["requirements"][0]["key"], json!("yamlRequirement"));
        assert_eq!(yml["requirements"][0]["verificationMethod"], json!("sourceInspection"));
    }

    #[tokio::test]
    async fn project_path_composable_listing_uses_global_when_project_has_no_composable_dir() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let global_dir = temp.path().join("requirements").join("composables");
        std::fs::create_dir_all(&global_dir).expect("global composables dir");
        std::fs::write(
            global_dir.join("global-only.yaml"),
            r#"
id: global-only
title: Global Only
requirements:
  - key: globalOnlyRequirement
    statement: Global-only statement.
    severity: high
    verificationMethod: manualEvidence
"#,
        )
        .expect("write global composable");
        let project_root = temp.path().join("project-without-codex-requirements");
        std::fs::create_dir_all(&project_root).expect("project root");

        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "name": "Alpha",
                        "projectRoot": project_root.display().to_string(),
                        "cwd": project_root.display().to_string(),
                        "orchestratorThreadID": "orch-1",
                        "agents": {
                            "orch-1": {"displayName": "Orch", "role": "orchestrator", "projectRoot": project_root.display().to_string()},
                            "worker-1": {"displayName": "Worker", "role": "worker", "projectRoot": project_root.display().to_string()}
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let payload = orchestrator_requirement_composables(
            &runtime,
            "worker-1",
            None,
            None,
            Some(project_root.to_str().expect("project path")),
        )
        .await
        .expect("list composables by project path");
        assert_eq!(payload["projectPath"], json!(normalize_path(project_root.display().to_string())));
        let items = payload["items"].as_array().expect("items");
        let global = items
            .iter()
            .find(|item| item["id"] == "global-only")
            .expect("global composable");
        assert_eq!(global["scope"], json!("global"));
        assert_eq!(global["requirements"][0]["key"], json!("globalOnlyRequirement"));
    }

    fn write_project_composable(project_root: &Path, id: &str, key: &str) {
        let project_dir = project_root.join(".codex").join("requirements").join("composables");
        std::fs::create_dir_all(&project_dir).expect("project composables dir");
        std::fs::write(
            project_dir.join(format!("{id}.json")),
            json!({
                "id": id,
                "title": id,
                "requirements": [{
                    "key": key,
                    "statement": format!("{key} statement."),
                    "severity": "high",
                    "verificationMethod": "manualEvidence"
                }]
            })
            .to_string(),
        )
        .expect("write project composable");
    }

    #[tokio::test]
    async fn permanent_composables_are_merged_server_side_when_omitted() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let project_root = temp.path().join("project");
        write_project_composable(&project_root, "permanent", "permanentRequirement");

        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": project_root.display().to_string(),
                        "cwd": project_root.display().to_string(),
                        "configs": {
                            "requirementsPermanentComposables": ["permanent"]
                        },
                        "agents": {
                            "orch-1": {"displayName": "Orch", "role": "orchestrator", "projectRoot": project_root.display().to_string()},
                            "worker-1": {"displayName": "Worker", "role": "worker", "projectRoot": project_root.display().to_string()}
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        orchestrator_set_requirements(
            &runtime,
            "orch-1",
            Some("worker-1"),
            None,
            None,
            json!({
                "id": "task",
                "requirements": [{
                    "key": "taskRequirement",
                    "statement": "Task statement.",
                    "severity": "high",
                    "verificationMethod": "manualEvidence"
                }]
            }),
        )
        .await
        .expect("set requirements");

        let state = parse_state(&runtime.state_document_value().await);
        let set = state.projects["alpha"].agents["worker-1"]
            .requirements
            .as_ref()
            .expect("requirements");
        let keys = set.requirements.iter().map(|item| item.key.as_str()).collect::<Vec<_>>();
        assert_eq!(keys, vec!["permanentRequirement", "taskRequirement"]);
    }

    #[tokio::test]
    async fn permanent_and_explicit_composables_dedupe_and_preserve_order() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let project_root = temp.path().join("project");
        write_project_composable(&project_root, "permanent", "permanentRequirement");
        write_project_composable(&project_root, "explicit", "explicitRequirement");

        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": project_root.display().to_string(),
                        "cwd": project_root.display().to_string(),
                        "configs": {
                            "requirementsPermanentComposables": ["permanent"]
                        },
                        "agents": {
                            "orch-1": {"displayName": "Orch", "role": "orchestrator", "projectRoot": project_root.display().to_string()},
                            "worker-1": {"displayName": "Worker", "role": "worker", "projectRoot": project_root.display().to_string()}
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        orchestrator_set_requirements(
            &runtime,
            "orch-1",
            Some("worker-1"),
            None,
            None,
            json!({
                "id": "task",
                "includeComposables": ["permanent", "explicit"],
                "requirements": [{
                    "key": "taskRequirement",
                    "statement": "Task statement.",
                    "severity": "high",
                    "verificationMethod": "manualEvidence"
                }]
            }),
        )
        .await
        .expect("set requirements");

        let state = parse_state(&runtime.state_document_value().await);
        let set = state.projects["alpha"].agents["worker-1"]
            .requirements
            .as_ref()
            .expect("requirements");
        let keys = set.requirements.iter().map(|item| item.key.as_str()).collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec!["permanentRequirement", "explicitRequirement", "taskRequirement"]
        );
    }

    #[tokio::test]
    async fn permanent_composable_conflict_fails_requirements_set() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let project_root = temp.path().join("project");
        let project_dir = project_root.join(".codex").join("requirements").join("composables");
        std::fs::create_dir_all(&project_dir).expect("project composables dir");
        std::fs::write(
            project_dir.join("permanent.json"),
            r#"{"id":"permanent","conflictsWith":["explicit"],"requirements":[{"key":"permanentRequirement","statement":"Permanent.","severity":"high","verificationMethod":"manualEvidence"}]}"#,
        )
        .expect("write permanent");
        std::fs::write(
            project_dir.join("explicit.json"),
            r#"{"id":"explicit","requirements":[{"key":"explicitRequirement","statement":"Explicit.","severity":"high","verificationMethod":"manualEvidence"}]}"#,
        )
        .expect("write explicit");

        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": project_root.display().to_string(),
                        "cwd": project_root.display().to_string(),
                        "configs": {
                            "requirementsPermanentComposables": ["permanent"]
                        },
                        "agents": {
                            "orch-1": {"displayName": "Orch", "role": "orchestrator", "projectRoot": project_root.display().to_string()},
                            "worker-1": {"displayName": "Worker", "role": "worker", "projectRoot": project_root.display().to_string()}
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let error = orchestrator_set_requirements(
            &runtime,
            "orch-1",
            Some("worker-1"),
            None,
            None,
            json!({
                "includeComposables": ["explicit"],
                "requirements": [{
                    "key": "taskRequirement",
                    "statement": "Task statement.",
                    "severity": "high",
                    "verificationMethod": "manualEvidence"
                }]
            }),
        )
        .await
        .expect_err("conflict");

        assert!(error.to_string().contains("conflicts"));
    }

    #[tokio::test]
    async fn permanent_composables_resolve_from_recipient_project() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let alpha_root = temp.path().join("alpha");
        let beta_root = temp.path().join("beta");
        write_project_composable(&alpha_root, "alpha-permanent", "alphaRequirement");
        write_project_composable(&beta_root, "beta-permanent", "betaRequirement");

        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": alpha_root.display().to_string(),
                        "cwd": alpha_root.display().to_string(),
                        "configs": {"requirementsPermanentComposables": ["alpha-permanent"]},
                        "agents": {
                            "orch-1": {"displayName": "Orch", "role": "orchestrator", "projectRoot": alpha_root.display().to_string()}
                        }
                    },
                    "beta": {
                        "projectRoot": beta_root.display().to_string(),
                        "cwd": beta_root.display().to_string(),
                        "configs": {"requirementsPermanentComposables": ["beta-permanent"]},
                        "agents": {
                            "orch-beta": {"displayName": "Beta Orch", "role": "orchestrator", "projectRoot": beta_root.display().to_string()},
                            "worker-1": {"displayName": "Worker", "role": "worker", "projectRoot": beta_root.display().to_string()}
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        orchestrator_set_requirements(
            &runtime,
            "orch-beta",
            Some("worker-1"),
            None,
            Some(&beta_root.display().to_string()),
            json!({
                "requirements": [{
                    "key": "taskRequirement",
                    "statement": "Task statement.",
                    "severity": "high",
                    "verificationMethod": "manualEvidence"
                }]
            }),
        )
        .await
        .expect("set requirements");

        let state = parse_state(&runtime.state_document_value().await);
        let set = state.projects["beta"].agents["worker-1"]
            .requirements
            .as_ref()
            .expect("requirements");
        let keys = set.requirements.iter().map(|item| item.key.as_str()).collect::<Vec<_>>();
        assert_eq!(keys, vec!["betaRequirement", "taskRequirement"]);
    }

    #[tokio::test]
    async fn composable_listing_marks_project_permanent_items() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let project_root = temp.path().join("project");
        write_project_composable(&project_root, "permanent", "permanentRequirement");
        write_project_composable(&project_root, "optional", "optionalRequirement");

        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": project_root.display().to_string(),
                        "cwd": project_root.display().to_string(),
                        "configs": {
                            "requirementsPermanentComposables": ["permanent"]
                        },
                        "agents": {
                            "orch-1": {"displayName": "Orch", "role": "orchestrator", "projectRoot": project_root.display().to_string()},
                            "worker-1": {"displayName": "Worker", "role": "worker", "projectRoot": project_root.display().to_string()}
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let payload = orchestrator_requirement_composables(
            &runtime,
            "orch-1",
            Some("worker-1"),
            None,
            None,
        )
        .await
        .expect("list composables");
        let items = payload["items"].as_array().expect("items");
        let permanent = items.iter().find(|item| item["id"] == "permanent").expect("permanent");
        let optional = items.iter().find(|item| item["id"] == "optional").expect("optional");
        assert_eq!(permanent["permanent"], json!(true));
        assert_eq!(permanent["permanentSource"], json!("project"));
        assert_eq!(optional["permanent"], json!(false));
        assert!(optional["permanentSource"].is_null());
    }

    #[tokio::test]
    async fn composable_listing_resolves_permanent_items_from_project_path() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let alpha_root = temp.path().join("alpha");
        let beta_root = temp.path().join("beta");
        write_project_composable(&alpha_root, "alpha-permanent", "alphaRequirement");
        write_project_composable(&beta_root, "beta-permanent", "betaRequirement");

        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "projectRoot": alpha_root.display().to_string(),
                        "cwd": alpha_root.display().to_string(),
                        "configs": {"requirementsPermanentComposables": ["alpha-permanent"]},
                        "agents": {
                            "operator-1": {"displayName": "Operator", "role": "operator", "projectRoot": alpha_root.display().to_string()}
                        }
                    },
                    "beta": {
                        "projectRoot": beta_root.display().to_string(),
                        "cwd": beta_root.display().to_string(),
                        "configs": {"requirementsPermanentComposables": ["beta-permanent"]},
                        "agents": {
                            "orch-beta": {"displayName": "Beta Orchestrator", "role": "orchestrator", "projectRoot": beta_root.display().to_string()}
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let payload = orchestrator_requirement_composables(
            &runtime,
            "operator-1",
            None,
            None,
            Some(beta_root.to_str().expect("beta path")),
        )
        .await
        .expect("list composables");
        assert_eq!(payload["threadId"], json!("orch-beta"));
        let items = payload["items"].as_array().expect("items");
        let beta = items
            .iter()
            .find(|item| item["id"] == "beta-permanent")
            .expect("beta permanent");
        assert_eq!(beta["permanent"], json!(true));
        assert_eq!(beta["permanentSource"], json!("project"));
        assert!(items.iter().all(|item| item["id"] != "alpha-permanent"));
    }

    #[test]
    fn requirements_composable_merge_rejects_conflicting_keys() {
        let mut merged = vec![RequirementState {
            key: "sameKey".to_string(),
            statement: "Original statement.".to_string(),
            severity: "high".to_string(),
            claim_schema_description: None,
            verdict_schema_description: None,
            verification_method: "manualEvidence".to_string(),
        }];
        let incoming = vec![RequirementState {
            key: "sameKey".to_string(),
            statement: "Different statement.".to_string(),
            severity: "high".to_string(),
            claim_schema_description: None,
            verdict_schema_description: None,
            verification_method: "manualEvidence".to_string(),
        }];
        let error = merge_requirement_items(&mut merged, &incoming).expect_err("conflict");
        assert!(error.to_string().contains("conflicting requirement key `sameKey`"));
    }

    #[tokio::test]
    async fn passing_requirements_review_deactivates_requirement_set() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let mut state = PersistedState::default();
        let mut project = PersistedProjectState {
            project_root: Some(temp.path().display().to_string()),
            cwd: Some(temp.path().display().to_string()),
            updated_at: Some(100),
            ..Default::default()
        };
        project.agents.insert(
            "source-thread".to_string(),
            PersistedAgentState {
                display_name: Some("Source".to_string()),
                role: Some("worker".to_string()),
                requirements: Some(sample_requirement_set()),
                requirement_review: Some(RequirementReviewBindingState {
                    source_thread_id: "source-thread".to_string(),
                    reviewer_thread_id: "reviewer-thread".to_string(),
                    requirement_set_id: Some("web-gui-contract".to_string()),
                    status: "inReview".to_string(),
                    latest_claim_packet: Some(json!({"summary": "claimed"})),
                    latest_verdict_packet: None,
                    updated_at: 100,
                }),
                ..Default::default()
            },
        );
        project.agents.insert(
            "reviewer-thread".to_string(),
            PersistedAgentState {
                display_name: Some("Requirements Reviewer".to_string()),
                role: Some("requirements-reviewer".to_string()),
                parent_thread_id: Some("source-thread".to_string()),
                hidden_from_peer_list: true,
                ..Default::default()
            },
        );
        state.projects.insert("project".to_string(), project);
        persist_state(&runtime, &state).await.expect("persist state");

        mark_requirements_review_verdict(
            &runtime,
            "source-thread",
            "reviewer-thread",
            json!({
                "overallVerdict": "pass",
                "nativeGuiIsSourceOfTruth": {
                    "verdict": "pass",
                    "reason": "Evidence matches.",
                    "evidenceAssessment": "Sufficient.",
                    "requiredCorrection": ""
                },
                "noInventedWebsocketEventShapes": {
                    "verdict": "pass",
                    "reason": "Evidence matches.",
                    "evidenceAssessment": "Sufficient.",
                    "requiredCorrection": ""
                },
                "route": {
                    "destination": "orchestrator",
                    "message": "All requirements passed."
                }
            }),
        )
        .await
        .expect("mark verdict");

        let state = parse_state(&runtime.state_document_value().await);
        let agent = agent_state_for_thread(&state, "source-thread").expect("source agent");
        assert_eq!(agent.requirements.as_ref().map(|set| set.active), Some(false));
        assert_eq!(
            output_schema_for_thread_turn(&state, "source-thread"),
            Some(Value::Null)
        );
        assert!(active_requirements_claim_schema_for_thread(&state, "source-thread").is_none());
        assert!(agent.requirement_review.is_none());
        let reviewer = agent_state_for_thread(&state, "reviewer-thread").expect("reviewer agent");
        assert_eq!(reviewer.parent_thread_id, None);
        assert!(reviewer.hidden_from_peer_list);
        assert_eq!(
            requirements_review_target_for_thread(&state, "source-thread", &sample_requirement_set()),
            None
        );
    }

    #[tokio::test]
    async fn failed_review_reduces_worker_schema_and_reactivates_regressed_requirement() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let mut state = PersistedState::default();
        let mut project = PersistedProjectState {
            project_root: Some(temp.path().display().to_string()),
            cwd: Some(temp.path().display().to_string()),
            updated_at: Some(100),
            ..Default::default()
        };
        project.agents.insert(
            "source-thread".to_string(),
            PersistedAgentState {
                display_name: Some("Source".to_string()),
                role: Some("worker".to_string()),
                requirements: Some(sample_requirement_set()),
                requirement_review: Some(RequirementReviewBindingState {
                    source_thread_id: "source-thread".to_string(),
                    reviewer_thread_id: "reviewer-thread".to_string(),
                    requirement_set_id: Some("web-gui-contract".to_string()),
                    status: "inReview".to_string(),
                    latest_claim_packet: Some(json!({"summary": "claimed"})),
                    latest_verdict_packet: None,
                    updated_at: 100,
                }),
                ..Default::default()
            },
        );
        project.agents.insert(
            "reviewer-thread".to_string(),
            PersistedAgentState {
                display_name: Some("Requirements Reviewer".to_string()),
                role: Some("requirements-reviewer".to_string()),
                parent_thread_id: Some("source-thread".to_string()),
                hidden_from_peer_list: true,
                ..Default::default()
            },
        );
        state.projects.insert("project".to_string(), project);
        persist_state(&runtime, &state).await.expect("persist state");

        mark_requirements_review_verdict(
            &runtime,
            "source-thread",
            "reviewer-thread",
            json!({
                "overallVerdict": "fail",
                "nativeGuiIsSourceOfTruth": {
                    "verdict": "pass",
                    "reason": "Still preserved.",
                    "evidenceAssessment": "Brief pass.",
                    "requiredCorrection": ""
                },
                "noInventedWebsocketEventShapes": {
                    "verdict": "fail",
                    "reason": "A shape was invented.",
                    "evidenceAssessment": "Needs correction.",
                    "requiredCorrection": "Use the existing protocol."
                },
                "route": {
                    "destination": "sourceAgent",
                    "message": "Fix protocol drift."
                }
            }),
        )
        .await
        .expect("mark first verdict");

        let state = parse_state(&runtime.state_document_value().await);
        let agent = agent_state_for_thread(&state, "source-thread").expect("source agent");
        let requirements = agent.requirements.as_ref().expect("requirements");
        assert_eq!(requirements.requirements.len(), 2);
        assert_eq!(
            requirements.review_progress["nativeGuiIsSourceOfTruth"].status,
            "passed"
        );
        assert_eq!(
            requirements.review_progress["noInventedWebsocketEventShapes"].status,
            "failed"
        );
        let reduced_schema = active_requirements_claim_schema_for_thread(&state, "source-thread")
            .expect("reduced schema");
        assert_eq!(
            reduced_schema["properties"]["requirements"]["required"],
            json!(["noInventedWebsocketEventShapes"])
        );

        mark_requirements_review_verdict(
            &runtime,
            "source-thread",
            "reviewer-thread",
            json!({
                "overallVerdict": "fail",
                "nativeGuiIsSourceOfTruth": {
                    "verdict": "fail",
                    "reason": "The correction regressed native fidelity.",
                    "evidenceAssessment": "Regression found.",
                    "requiredCorrection": "Restore native fidelity."
                },
                "noInventedWebsocketEventShapes": {
                    "verdict": "pass",
                    "reason": "Protocol drift fixed.",
                    "evidenceAssessment": "Brief pass.",
                    "requiredCorrection": ""
                },
                "route": {
                    "destination": "sourceAgent",
                    "message": "Fix the regression."
                }
            }),
        )
        .await
        .expect("mark second verdict");

        let state = parse_state(&runtime.state_document_value().await);
        let agent = agent_state_for_thread(&state, "source-thread").expect("source agent");
        let requirements = agent.requirements.as_ref().expect("requirements");
        assert_eq!(requirements.requirements.len(), 2);
        assert_eq!(
            requirements.review_progress["nativeGuiIsSourceOfTruth"].status,
            "failed"
        );
        assert_eq!(
            requirements.review_progress["noInventedWebsocketEventShapes"].status,
            "passed"
        );
        let regressed_schema = active_requirements_claim_schema_for_thread(&state, "source-thread")
            .expect("regressed schema");
        assert_eq!(
            regressed_schema["properties"]["requirements"]["required"],
            json!(["nativeGuiIsSourceOfTruth"])
        );
    }

    #[tokio::test]
    async fn still_passing_keeps_previous_pass_but_cannot_mask_ineligible_requirements() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let mut requirements = sample_requirement_set();
        requirements.review_progress.insert(
            "nativeGuiIsSourceOfTruth".to_string(),
            RequirementReviewProgressState {
                status: "passed".to_string(),
                updated_at: Some(100),
            },
        );
        requirements.review_progress.insert(
            "noInventedWebsocketEventShapes".to_string(),
            RequirementReviewProgressState {
                status: "failed".to_string(),
                updated_at: Some(100),
            },
        );
        let mut state = PersistedState::default();
        let mut project = PersistedProjectState {
            project_root: Some(temp.path().display().to_string()),
            cwd: Some(temp.path().display().to_string()),
            updated_at: Some(100),
            ..Default::default()
        };
        project.agents.insert(
            "source-thread".to_string(),
            PersistedAgentState {
                display_name: Some("Source".to_string()),
                role: Some("worker".to_string()),
                requirements: Some(requirements),
                requirement_review: Some(RequirementReviewBindingState {
                    source_thread_id: "source-thread".to_string(),
                    reviewer_thread_id: "reviewer-thread".to_string(),
                    requirement_set_id: Some("web-gui-contract".to_string()),
                    status: "inReview".to_string(),
                    latest_claim_packet: Some(json!({"summary": "claimed"})),
                    latest_verdict_packet: None,
                    updated_at: 100,
                }),
                ..Default::default()
            },
        );
        project.agents.insert(
            "reviewer-thread".to_string(),
            PersistedAgentState {
                display_name: Some("Requirements Reviewer".to_string()),
                role: Some("requirements-reviewer".to_string()),
                parent_thread_id: Some("source-thread".to_string()),
                hidden_from_peer_list: true,
                ..Default::default()
            },
        );
        state.projects.insert("project".to_string(), project);
        persist_state(&runtime, &state).await.expect("persist state");

        mark_requirements_review_verdict(
            &runtime,
            "source-thread",
            "reviewer-thread",
            json!({
                "overallVerdict": "pass",
                "nativeGuiIsSourceOfTruth": {
                    "verdict": "stillPassing"
                },
                "noInventedWebsocketEventShapes": {
                    "verdict": "stillPassing"
                },
                "route": {
                    "destination": "orchestrator",
                    "message": "Invalid shorthand cannot pass."
                }
            }),
        )
        .await
        .expect("mark verdict");

        let state = parse_state(&runtime.state_document_value().await);
        let agent = agent_state_for_thread(&state, "source-thread").expect("source agent");
        let requirements = agent.requirements.as_ref().expect("requirements");
        assert_eq!(requirements.active, true);
        assert_eq!(
            requirements.review_progress["nativeGuiIsSourceOfTruth"].status,
            "passed"
        );
        assert_eq!(
            requirements.review_progress["noInventedWebsocketEventShapes"].status,
            "failed"
        );
        assert_eq!(
            agent.requirement_review.as_ref().map(|review| review.status.as_str()),
            Some("failed")
        );
    }

    #[tokio::test]
    async fn accepted_waiver_is_terminal_and_deactivates_requirement_set() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let mut state = PersistedState::default();
        let mut project = PersistedProjectState {
            project_root: Some(temp.path().display().to_string()),
            cwd: Some(temp.path().display().to_string()),
            updated_at: Some(100),
            ..Default::default()
        };
        project.agents.insert(
            "source-thread".to_string(),
            PersistedAgentState {
                display_name: Some("Source".to_string()),
                role: Some("worker".to_string()),
                requirements: Some(sample_requirement_set()),
                requirement_review: Some(RequirementReviewBindingState {
                    source_thread_id: "source-thread".to_string(),
                    reviewer_thread_id: "reviewer-thread".to_string(),
                    requirement_set_id: Some("web-gui-contract".to_string()),
                    status: "waiverRequired".to_string(),
                    latest_claim_packet: Some(json!({"summary": "claimed", "requirements": {}})),
                    latest_verdict_packet: None,
                    updated_at: 100,
                }),
                ..Default::default()
            },
        );
        project.agents.insert(
            "reviewer-thread".to_string(),
            PersistedAgentState {
                display_name: Some("Requirements Reviewer".to_string()),
                role: Some("requirements-reviewer".to_string()),
                parent_thread_id: Some("source-thread".to_string()),
                hidden_from_peer_list: true,
                ..Default::default()
            },
        );
        state.projects.insert("project".to_string(), project);
        persist_state(&runtime, &state).await.expect("persist state");

        mark_requirements_review_verdict(
            &runtime,
            "source-thread",
            "reviewer-thread",
            json!({
                "overallVerdict": "waiverAccepted",
                "nativeGuiIsSourceOfTruth": {
                    "verdict": "waiverAccepted",
                    "reason": "Owner accepted waiver.",
                    "evidenceAssessment": "Explicit owner decision.",
                    "requiredCorrection": ""
                },
                "noInventedWebsocketEventShapes": {
                    "verdict": "pass",
                    "reason": "Evidence matches.",
                    "evidenceAssessment": "Sufficient.",
                    "requiredCorrection": ""
                },
                "route": {
                    "destination": "none",
                    "message": "Owner waiver accepted."
                }
            }),
        )
        .await
        .expect("mark verdict");

        let state = parse_state(&runtime.state_document_value().await);
        let agent = agent_state_for_thread(&state, "source-thread").expect("source agent");
        assert_eq!(agent.requirements.as_ref().map(|set| set.active), Some(false));
        let review = agent.requirement_review.as_ref().expect("review state retained");
        assert_eq!(review.status, "waiverAccepted");
        assert_eq!(
            review
                .latest_verdict_packet
                .as_ref()
                .and_then(|packet| packet.get("overallVerdict"))
                .and_then(Value::as_str),
            Some("waiverAccepted")
        );
        let reviewer = agent_state_for_thread(&state, "reviewer-thread").expect("reviewer agent");
        assert_eq!(reviewer.parent_thread_id, None);
        assert!(reviewer.hidden_from_peer_list);
    }

    #[test]
    fn requirements_review_prompt_includes_compact_evidence_without_ids_or_raw_packet() {
        let prompt = requirements_review_prompt(
            &sample_requirement_set(),
            "Config Operator",
            "thread-secret",
            "turn-secret",
            r#"{
                "summary": "Rendered reviewer verdict card.",
                "requirements": {
                    "nativeGuiIsSourceOfTruth": {
                        "claim": "satisfied",
                        "evidence": ["/tmp/reviewer-verdict-card.png"],
                        "justification": "Screenshot shows formatted card.",
                        "risk": "low"
                    }
                }
            }"#,
        );
        assert!(prompt.contains("Review subject: Config Operator"));
        assert!(prompt.contains("Source evidence summary:"));
        assert!(prompt.contains("Rendered reviewer verdict card."));
        assert!(prompt.contains("/tmp/reviewer-verdict-card.png"));
        assert!(!prompt.contains("Source thread ID"));
        assert!(!prompt.contains("Source turn ID"));
        assert!(!prompt.contains("Source agent claim packet"));
        assert!(!prompt.contains("thread-secret"));
        assert!(!prompt.contains("turn-secret"));
        assert!(!prompt.contains("\"requirements\""));
    }

    fn assert_strict_object_schema(value: &Value) {
        match value {
            Value::Object(object) => {
                let is_object_schema = object
                    .get("type")
                    .and_then(Value::as_str)
                    .map(|kind| kind == "object")
                    .or_else(|| {
                        object.get("type").and_then(Value::as_array).map(|types| {
                            types.iter().any(|kind| kind.as_str() == Some("object"))
                        })
                    })
                    .unwrap_or(false);
                if is_object_schema {
                    assert_eq!(
                        object.get("additionalProperties").and_then(Value::as_bool),
                        Some(false),
                        "object schema must set additionalProperties=false: {value}"
                    );
                    let properties = object
                        .get("properties")
                        .and_then(Value::as_object)
                        .expect("object schema must define properties");
                    let required = object
                        .get("required")
                        .and_then(Value::as_array)
                        .expect("object schema must define required");
                    for key in properties.keys() {
                        assert!(
                            required.iter().any(|item| item.as_str() == Some(key.as_str())),
                            "strict OpenAI schemas require every property to be required: {key}"
                        );
                    }
                }
                for item in object.values() {
                    assert_strict_object_schema(item);
                }
            }
            Value::Array(items) => {
                for item in items {
                    assert_strict_object_schema(item);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn requirements_schemas_are_strict_openai_shapes() {
        assert_strict_object_schema(&requirements_worker_claim_schema(&sample_requirement_set()));
        assert_strict_object_schema(&requirements_verdict_schema(&sample_requirement_set()));
    }

    #[test]
    fn requirement_set_payload_validation_rejects_bad_creation_keys() {
        let error = parse_optional_requirement_set_payload(&json!({
            "requirementSet": {
                "requirements": [
                    {
                        "key": "requirement-1",
                        "statement": "Bad key names must not enter thread creation.",
                        "severity": "blocker",
                        "verificationMethod": "diffReview"
                    }
                ]
            }
        }))
        .expect_err("invalid requirement key should be rejected");
        assert!(
            error
                .to_string()
                .contains("must contain only letters, numbers, and underscores")
        );
    }

    #[test]
    fn writes_requirements_schema_validation_fixtures() {
        let fixture_dir = std::path::PathBuf::from("/tmp/robdex-requirements-schema-validation");
        std::fs::create_dir_all(&fixture_dir).expect("create requirements schema fixture dir");
        let set = sample_requirement_set();
        std::fs::write(
            fixture_dir.join("claim.schema.json"),
            serde_json::to_string_pretty(&requirements_worker_claim_schema(&set)).expect("serialize claim schema"),
        )
        .expect("write claim schema fixture");
        std::fs::write(
            fixture_dir.join("verdict.schema.json"),
            serde_json::to_string_pretty(&requirements_verdict_schema(&set)).expect("serialize verdict schema"),
        )
        .expect("write verdict schema fixture");
    }

    #[test]
    #[ignore = "live Codex/OpenAI schema validation; requires auth/network and costs tokens"]
    fn live_codex_accepts_still_passing_verdict_schema() {
        let fixture_dir = std::path::PathBuf::from("/tmp/robdex-requirements-schema-validation");
        std::fs::create_dir_all(&fixture_dir).expect("create requirements schema fixture dir");
        let mut set = sample_requirement_set();
        set.review_progress.insert(
            "nativeGuiIsSourceOfTruth".to_string(),
            RequirementReviewProgressState {
                status: "passed".to_string(),
                updated_at: Some(100),
            },
        );
        let schema_path = fixture_dir.join("verdict-still-passing.schema.json");
        let output_path = fixture_dir.join("verdict-still-passing.output.json");
        let _ = std::fs::remove_file(&output_path);
        std::fs::write(
            &schema_path,
            serde_json::to_string_pretty(&requirements_verdict_schema(&set))
                .expect("serialize stillPassing verdict schema"),
        )
        .expect("write stillPassing verdict schema fixture");

        let status = std::process::Command::new("codex")
            .arg("exec")
            .arg("--ephemeral")
            .arg("--skip-git-repo-check")
            .arg("--sandbox")
            .arg("read-only")
            .arg("--output-schema")
            .arg(&schema_path)
            .arg("--output-last-message")
            .arg(&output_path)
            .arg(
                "Return a minimal valid Requirements review verdict. Use stillPassing for nativeGuiIsSourceOfTruth. Use a full pass verdict for noInventedWebsocketEventShapes. Route to none.",
            )
            .status()
            .expect("run codex exec");
        assert!(
            status.success(),
            "codex exec rejected or failed the generated stillPassing verdict schema: {status}"
        );

        let output = std::fs::read_to_string(&output_path).expect("read codex output");
        let payload: Value = serde_json::from_str(output.trim()).expect("codex output should be JSON");
        assert_eq!(
            payload["requirements"]["nativeGuiIsSourceOfTruth"]["verdict"],
            json!("stillPassing")
        );
        assert_eq!(
            payload["requirements"]["noInventedWebsocketEventShapes"]["verdict"],
            json!("pass")
        );
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
            project_path: root.path().to_path_buf(),
            cwd: root.path().to_path_buf(),
            paths: BridgePaths::new(PathBuf::from(root.path()).join("state")),
        }
    }

    #[tokio::test]
    async fn orchestrator_set_requirements_null_clears_requirement_metadata() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let mut state = sample_state();
        let worker = state
            .projects
            .get_mut("alpha")
            .and_then(|project| project.agents.get_mut("worker-a"))
            .expect("worker");
        worker.requirements = Some(sample_requirement_set());
        worker.requirement_review = Some(RequirementReviewBindingState {
            source_thread_id: "worker-a".to_string(),
            reviewer_thread_id: "reviewer-a".to_string(),
            requirement_set_id: Some("requirements-a".to_string()),
            status: "inReview".to_string(),
            latest_claim_packet: Some(json!({"summary": "done"})),
            latest_verdict_packet: None,
            updated_at: 100,
        });
        worker.requirement_packets.push(RequirementPacketState {
            packet_type: "claim".to_string(),
            source_thread_id: "worker-a".to_string(),
            turn_id: Some("turn-a".to_string()),
            target_thread_id: Some("reviewer-a".to_string()),
            payload: json!({"summary": "done"}),
            created_at: 100,
        });
        persist_state(&runtime, &state).await.expect("persist state");

        let result = orchestrator_set_requirements(
            &runtime,
            "operator-a",
            Some("worker-a"),
            None,
            None,
            Value::Null,
        )
        .await
        .expect("clear requirements");

        assert_eq!(result["cleared"], true);
        assert_eq!(result["requirementCount"], 0);
        let next = parse_state(&runtime.state_document_value().await);
        let worker = next
            .projects
            .get("alpha")
            .and_then(|project| project.agents.get("worker-a"))
            .expect("worker");
        assert!(worker.requirements.is_none());
        assert!(worker.requirement_review.is_none());
        assert!(worker.requirement_packets.is_empty());
    }

    #[tokio::test]
    async fn requirements_self_setting_allows_hidden_but_rejects_worker_and_qa() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        persist_state(&runtime, &sample_state()).await.expect("persist state");
        let payload = serde_json::to_value(sample_requirement_set()).expect("requirements json");

        let result = orchestrator_set_requirements(
            &runtime,
            "hidden-a",
            None,
            None,
            None,
            payload.clone(),
        )
        .await
        .expect("hidden self set requirements");
        assert_eq!(result["threadId"], "hidden-a");

        let worker_error = orchestrator_set_requirements(
            &runtime,
            "worker-a",
            None,
            None,
            None,
            payload.clone(),
        )
        .await
        .expect_err("worker self set should fail");
        assert!(worker_error
            .to_string()
            .contains("Workers, QA, and planner threads cannot set Requirements on themselves"));

        let qa_error = orchestrator_set_requirements(
            &runtime,
            "qa-a",
            None,
            None,
            None,
            payload,
        )
        .await
        .expect_err("qa self set should fail");
        assert!(qa_error
            .to_string()
            .contains("Workers, QA, and planner threads cannot set Requirements on themselves"));
    }

    #[tokio::test]
    async fn requirements_targeting_allows_operator_direct_hidden_but_orchestrator_only_workers() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let mut state = sample_state();
        state
            .projects
            .get_mut("alpha")
            .and_then(|project| project.agents.get_mut("hidden-a"))
            .expect("hidden")
            .hidden_from_peer_list = true;
        persist_state(&runtime, &state).await.expect("persist state");
        let payload = serde_json::to_value(sample_requirement_set()).expect("requirements json");

        let hidden_result = orchestrator_set_requirements(
            &runtime,
            "operator-a",
            Some("hidden-a"),
            None,
            None,
            payload.clone(),
        )
        .await
        .expect("operator direct hidden requirements");
        assert_eq!(hidden_result["threadId"], "hidden-a");

        let status = orchestrator_requirements_status(&runtime, "operator-a", Some("hidden-a"), None, None)
            .await
            .expect("operator direct hidden requirements status");
        assert_eq!(status["threadId"], "hidden-a");

        let composables = orchestrator_requirement_composables(&runtime, "operator-a", Some("hidden-a"), None, None)
            .await
            .expect("operator direct hidden requirements composables");
        assert_eq!(composables["threadId"], "hidden-a");

        let worker_result = orchestrator_set_requirements(
            &runtime,
            "orch-a",
            Some("worker-a"),
            None,
            None,
            payload.clone(),
        )
        .await
        .expect("orchestrator worker requirements");
        assert_eq!(worker_result["threadId"], "worker-a");

        let qa_error = orchestrator_set_requirements(
            &runtime,
            "orch-a",
            Some("qa-a"),
            None,
            None,
            payload.clone(),
        )
        .await
        .expect_err("orchestrator qa requirements should fail");
        assert!(qa_error.to_string().contains("Orchestrators may only set Requirements on workers"));

        let hidden_error = orchestrator_set_requirements(
            &runtime,
            "orch-a",
            Some("hidden-a"),
            None,
            None,
            payload,
        )
        .await
        .expect_err("orchestrator hidden requirements should fail");
        assert!(hidden_error.to_string().contains("Orchestrators may only set Requirements on workers"));
    }

    #[tokio::test]
    async fn planner_can_set_requirements_on_same_project_non_hidden_agents() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let mut state = sample_state();
        state
            .projects
            .get_mut("alpha")
            .and_then(|project| project.agents.get_mut("hidden-a"))
            .expect("hidden")
            .hidden_from_peer_list = true;
        persist_state(&runtime, &state).await.expect("persist state");
        let payload = serde_json::to_value(sample_requirement_set()).expect("requirements json");

        let worker_result = orchestrator_set_requirements(
            &runtime,
            "planner-a",
            Some("worker-a"),
            None,
            None,
            payload.clone(),
        )
        .await
        .expect("planner worker requirements");
        assert_eq!(worker_result["threadId"], "worker-a");

        let qa_result = orchestrator_set_requirements(
            &runtime,
            "planner-a",
            Some("qa-a"),
            None,
            None,
            payload.clone(),
        )
        .await
        .expect("planner qa requirements");
        assert_eq!(qa_result["threadId"], "qa-a");

        let cross_project_error = orchestrator_set_requirements(
            &runtime,
            "planner-a",
            Some("worker-b"),
            None,
            None,
            payload.clone(),
        )
        .await
        .expect_err("planner cross-project requirements should fail");
        assert!(cross_project_error.to_string().contains("planners may set Requirements on non-hidden agents in their project"));

        let hidden_error = orchestrator_set_requirements(
            &runtime,
            "planner-a",
            Some("hidden-a"),
            None,
            None,
            payload,
        )
        .await
        .expect_err("planner hidden requirements should fail");
        assert!(hidden_error.to_string().contains("planners may set Requirements on non-hidden agents in their project"));

        let self_error = orchestrator_set_requirements(
            &runtime,
            "planner-a",
            None,
            None,
            None,
            serde_json::to_value(sample_requirement_set()).expect("requirements json"),
        )
        .await
        .expect_err("planner self requirements should fail");
        assert!(self_error
            .to_string()
            .contains("Workers, QA, and planner threads cannot set Requirements on themselves"));
    }

    #[tokio::test]
    async fn direct_requirements_update_targets_exact_thread_without_sender_identity() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let mut state = sample_state();
        state
            .projects
            .get_mut("alpha")
            .and_then(|project| project.agents.get_mut("hidden-a"))
            .expect("hidden")
            .hidden_from_peer_list = true;
        persist_state(&runtime, &state).await.expect("persist state");

        let payload = serde_json::to_value(sample_requirement_set()).expect("requirements json");
        let set_result = direct_set_requirements(&runtime, Some("hidden-a"), None, payload)
            .await
            .expect("direct hidden requirements set");
        assert_eq!(set_result["threadId"], "hidden-a");
        assert_eq!(set_result["requirementCount"], 2);

        let clear_result = direct_set_requirements(&runtime, Some("hidden-a"), None, Value::Null)
            .await
            .expect("direct hidden requirements clear");
        assert_eq!(clear_result["threadId"], "hidden-a");
        assert_eq!(clear_result["cleared"], true);
    }

    #[tokio::test]
    async fn direct_requirement_composables_can_load_for_recipient_without_sender_identity() {
        let temp = TempDir::new().expect("tempdir");
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:0".to_string()))
            .await
            .expect("runtime");
        let state = sample_state();
        persist_state(&runtime, &state).await.expect("persist state");
        let global_dir = temp.path().join("requirements").join("composables");
        std::fs::create_dir_all(&global_dir).expect("global composables dir");
        std::fs::write(
            global_dir.join("global-proof.yaml"),
            r#"
id: global-proof
title: Global Proof
description: Direct GUI discovery can load global composables without pretending to be the selected agent.
requirements:
  - key: globalProof
    statement: Global composable appears.
    severity: medium
    verificationMethod: manualEvidence
"#,
        )
        .expect("write global composable");

        let payload = direct_requirement_composables(&runtime, Some("worker-a"), None)
            .await
            .expect("direct composables");
        assert_eq!(payload["threadId"], "worker-a");
        let items = payload["items"].as_array().expect("items");
        assert!(items.iter().any(|item| item["id"] == "global-proof"));
    }

    #[test]
    fn delete_project_removes_project_tracking_and_repicks_selection() {
        let mut state = sample_state();
        state.projects.get_mut("alpha").expect("alpha").id = Some("project-alpha".to_string());
        state.projects.get_mut("beta").expect("beta").id = Some("project-beta".to_string());
        let project_id = "project-alpha".to_string();
        state.selected_project_id = Some(project_id.clone());

        delete_project(&mut state, Some(&project_id)).expect("delete project");

        assert!(!state
            .projects
            .values()
            .any(|project| project.id.as_deref() == Some(project_id.as_str())));
        assert_ne!(state.selected_project_id.as_deref(), Some(project_id.as_str()));
        assert_eq!(
            state.selected_project_id,
            state.projects.values().find_map(|project| project.id.clone())
        );
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
                        started_at: None,
                        completed_at: None,
                        duration_ms: None,
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

    #[tokio::test]
    async fn register_and_complete_live_process_updates_runtime_state() {
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

        let snapshot = runtime.workbench_snapshot_value().await;
        let processes = snapshot["liveProcessesByThreadID"]["worker-a"]
            .as_array()
            .expect("worker live processes");
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0]["processId"], "3001");
        assert_eq!(processes[0]["command"], "sleep 30");
        let state = parse_state(&runtime.state_document_value().await);
        let agent = agent_state_for_thread(&state, "worker-a").expect("agent state");
        assert!(agent.extras.is_empty());

        complete_live_process(&runtime, "worker-a", "3001")
            .await
            .expect("complete live process");

        let snapshot = runtime.workbench_snapshot_value().await;
        assert!(snapshot["liveProcessesByThreadID"].get("worker-a").is_none());
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
    async fn spawn_agent_with_requirement_set_gates_initial_turn() {
        let temp = TempDir::new().expect("tempdir");
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
                        "thread/start" => json!({"thread": {"id": "thread-requirements-worker", "title": "Requirements Worker"}}),
                        "thread/name/set" => json!({}),
                        "turn/start" => {
                            prompt_tx
                                .take()
                                .expect("prompt sender")
                                .send(request.clone())
                                .expect("record prompt request");
                            json!({"turn": {"id": "turn-requirements-1", "status": "inProgress", "items": [], "error": null}})
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
        let project_root = temp.path();
        write_project_composable(project_root, "permanent", "permanentRequirement");
        runtime
            .persist_state_document(json!({
                "projects": {
                    "project": {
                        "projectRoot": project_root.display().to_string(),
                        "cwd": project_root.display().to_string(),
                        "configs": {
                            "requirementsPermanentComposables": ["permanent"]
                        }
                    }
                }
            }))
            .await
            .expect("persist project config");
        let requirement_set = json!({
            "id": "web-gui-contract",
            "requirements": [
                {
                    "key": "nativeGuiIsSourceOfTruth",
                    "statement": "The web GUI must mirror the native Flutter GUI.",
                    "severity": "blocker",
                    "verificationMethod": "screenshotReview"
                },
                {
                    "key": "noInventedWebsocketEventShapes",
                    "statement": "Do not invent websocket event shapes.",
                    "severity": "high",
                    "verificationMethod": "diffReview"
                }
            ]
        });

        let outcome = execute_bridge_command(
            &runtime,
            "spawnAgent",
            json!({
                "role": "worker",
                "projectPath": project_root.display().to_string(),
                "displayName": "Requirements Worker",
                "initialPrompt": "Implement the web GUI slice.",
                "requirementSet": requirement_set,
            }),
        )
        .await
        .expect("spawnAgent");

        assert_eq!(outcome.payload["payload"]["threadId"], "thread-requirements-worker");
        let state = parse_state(&runtime.state_document_value().await);
        let tracked = agent_state_for_thread(&state, "thread-requirements-worker").expect("tracked agent state");
        let requirements = tracked.requirements.as_ref().expect("requirements persisted");
        assert_eq!(requirements.id.as_deref(), Some("web-gui-contract"));
        assert_eq!(
            requirements
                .requirements
                .iter()
                .map(|requirement| requirement.key.as_str())
                .collect::<Vec<_>>(),
            vec![
                "permanentRequirement",
                "nativeGuiIsSourceOfTruth",
                "noInventedWebsocketEventShapes"
            ]
        );

        let prompt_request = prompt_rx.await.expect("captured prompt request");
        assert_eq!(prompt_request.method, "turn/start");
        let prompt_params = prompt_request.params.expect("prompt params");
        assert_eq!(prompt_params["threadId"], "thread-requirements-worker");
        let schema = prompt_params
            .get("outputSchema")
            .expect("initial turn output schema");
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["required"],
            json!(["summary", "requirements"])
        );
        assert_eq!(
            schema["properties"]["requirements"]["required"],
            json!(["permanentRequirement", "nativeGuiIsSourceOfTruth", "noInventedWebsocketEventShapes"])
        );
        assert_eq!(
            schema["properties"]["requirements"]["properties"]["permanentRequirement"]["description"],
            "Requirement: permanentRequirement statement."
        );
        assert_eq!(
            schema["properties"]["requirements"]["properties"]["nativeGuiIsSourceOfTruth"]["description"],
            "Requirement: The web GUI must mirror the native Flutter GUI."
        );
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
                            "approvalPolicy": "on-request",
                            "sandboxMode": "danger-full-access",
                            "networkAccess": false,
                            "roleRuntimeDefaults": {
                                "worker": {
                                    "approvalPolicy": "on-failure",
                                    "sandboxMode": "workspace-write",
                                    "networkAccess": true
                                }
                            },
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
        assert_eq!(thread_params["approvalPolicy"], "on-failure");
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
        assert_eq!(agent_state.approval_policy.as_deref(), Some("on-failure"));
        assert_eq!(agent_state.sandbox_mode.as_deref(), Some("workspace-write"));
        assert_eq!(agent_state.network_access, Some(true));

        transport.abort();
    }

    #[tokio::test]
    async fn orchestrator_spawn_requirements_reviewer_forces_never_approval_policy() {
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
                        "thread/start" => json!({"thread": {"id": "thread-reviewer-1", "title": "Requirements Reviewer"}}),
                        "thread/name/set" => json!({}),
                        "turn/start" => json!({"turn": {"id": "turn-reviewer-1", "status": "inProgress", "items": [], "error": null}}),
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
                                "worker": { "modelID": "gpt-worker-default", "reasoningEffort": "low" },
                                "requirements-reviewer": { "modelID": "gpt-reviewer-default", "reasoningEffort": "high" }
                            }
                        },
                        "agents": {
                            "thread-orchestrator": {
                                "displayName": "Ezra Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().join(".worktrees").display().to_string(),
                                "approvalPolicy": "on-request",
                                "sandboxMode": "workspace-write",
                                "networkAccess": true
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
            "Requirements Reviewer",
            "Review requirements only.",
            None,
            Some("requirements-reviewer"),
            None,
            None,
        )
        .await
        .expect("orchestrator spawn");

        let thread_request = thread_rx.await.expect("captured thread start request");
        let thread_params = thread_request.params.expect("thread params");
        assert_eq!(thread_params["approvalPolicy"], "never");
        assert_eq!(thread_params["model"], "gpt-reviewer-default");
        assert_eq!(thread_params["config"]["model_reasoning_effort"], "high");

        let state = parse_state(&runtime.state_document_value().await);
        let agent_state = state
            .projects
            .values()
            .find_map(|project| project.agents.get("thread-reviewer-1"))
            .expect("persisted reviewer state");
        assert_eq!(agent_state.role.as_deref(), Some("requirements-reviewer"));
        assert_eq!(agent_state.approval_policy.as_deref(), Some("never"));
        assert_eq!(agent_state.model.as_deref(), Some("gpt-reviewer-default"));
        assert_eq!(agent_state.reasoning_effort.as_deref(), Some("high"));

        transport.abort();
    }

    #[tokio::test]
    async fn auto_requirements_reviewer_inherits_source_model_settings_without_role_defaults() {
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
                        "thread/start" => json!({"thread": {"id": "thread-reviewer-auto-1", "title": "Requirements Reviewer"}}),
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
        runtime
            .persist_state_document(json!({
                "projects": {
                    "ezra": {
                        "id": "project-ezra",
                        "name": "Ezra",
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "autoRouteReplies": true,
                        "routeApprovalRequests": true,
                        "agents": {
                            "thread-worker": {
                                "displayName": "Worker",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string(),
                                "model": "gpt-source",
                                "modelProvider": "source-provider",
                                "reasoningEffort": "medium",
                                "serviceTier": {"type": "priority"},
                                "approvalPolicy": "on-request"
                            }
                        }
                    }
                },
                "globalConfigs": {
                    "networkAccess": true,
                    "sandboxMode": "workspace-write"
                }
            }))
            .await
            .expect("persist state");

        let reviewer_id = ensure_requirements_reviewer_for_thread(&runtime, "thread-worker")
            .await
            .expect("reviewer")
            .expect("reviewer id");
        assert_eq!(reviewer_id, "thread-reviewer-auto-1");

        let thread_request = thread_rx.await.expect("captured thread start request");
        let thread_params = thread_request.params.expect("thread params");
        assert_eq!(thread_params["model"], "gpt-source");
        assert_eq!(thread_params["modelProvider"], "source-provider");
        assert_eq!(thread_params["config"]["model_reasoning_effort"], "medium");
        assert_eq!(thread_params["serviceTier"]["type"], "priority");
        assert_eq!(thread_params["approvalPolicy"], "never");
        assert_eq!(thread_params["sandbox"], "workspace-write");

        let state = parse_state(&runtime.state_document_value().await);
        let reviewer_state = state
            .projects
            .values()
            .find_map(|project| project.agents.get("thread-reviewer-auto-1"))
            .expect("persisted reviewer");
        assert_eq!(reviewer_state.model.as_deref(), Some("gpt-source"));
        assert_eq!(reviewer_state.model_provider.as_deref(), Some("source-provider"));
        assert_eq!(reviewer_state.reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(reviewer_state.approval_policy.as_deref(), Some("never"));
        assert_eq!(reviewer_state.sandbox_mode.as_deref(), Some("workspace-write"));
        assert_eq!(reviewer_state.network_access, Some(false));

        transport.abort();
    }

    #[tokio::test]
    async fn auto_requirements_reviewer_uses_project_role_defaults_when_configured() {
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
                        "thread/start" => json!({"thread": {"id": "thread-reviewer-auto-2", "title": "Requirements Reviewer"}}),
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
        runtime
            .persist_state_document(json!({
                "projects": {
                    "ezra": {
                        "id": "project-ezra",
                        "name": "Ezra",
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().display().to_string(),
                        "preferredModelProvider": "reviewer-provider",
                        "configs": {
                            "roleModelReasoningDefaults": {
                                "worker": { "modelID": "gpt-worker-default", "reasoningEffort": "low" },
                                "requirements-reviewer": { "modelID": "gpt-reviewer", "reasoningEffort": "high" }
                            }
                        },
                        "agents": {
                            "thread-worker": {
                                "displayName": "Worker",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string(),
                                "model": "gpt-source",
                                "modelProvider": "source-provider",
                                "reasoningEffort": "medium",
                                "serviceTier": {"type": "priority"}
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        ensure_requirements_reviewer_for_thread(&runtime, "thread-worker")
            .await
            .expect("reviewer")
            .expect("reviewer id");

        let thread_request = thread_rx.await.expect("captured thread start request");
        let thread_params = thread_request.params.expect("thread params");
        assert_eq!(thread_params["model"], "gpt-reviewer");
        assert_eq!(thread_params["modelProvider"], "reviewer-provider");
        assert_eq!(thread_params["config"]["model_reasoning_effort"], "high");
        assert_eq!(thread_params["serviceTier"]["type"], "priority");
        assert_eq!(thread_params["approvalPolicy"], "never");

        let state = parse_state(&runtime.state_document_value().await);
        let reviewer_state = state
            .projects
            .values()
            .find_map(|project| project.agents.get("thread-reviewer-auto-2"))
            .expect("persisted reviewer");
        assert_eq!(reviewer_state.model.as_deref(), Some("gpt-reviewer"));
        assert_eq!(reviewer_state.model_provider.as_deref(), Some("reviewer-provider"));
        assert_eq!(reviewer_state.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(reviewer_state.service_tier.as_ref().and_then(|value| value.get("type")).and_then(Value::as_str), Some("priority"));

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
            None,
        )
        .await
        .expect_err("orchestrator spawn should reject designer role");

        assert!(
            error
                .to_string()
                .contains("only spawn worker, qa, or requirements-reviewer agents")
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

    #[tokio::test]
    async fn turn_interrupt_uses_immediate_thread_interrupt_without_tracked_turn() {
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
                request_tx
                    .take()
                    .expect("request sender")
                    .send(request.clone())
                    .expect("record request");
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

        execute_bridge_command(
            &runtime,
            "turnInterrupt",
            json!({
                "threadId": "worker-1"
            }),
        )
        .await
        .expect("turn interrupt");

        let request = request_rx.await.expect("captured request");
        assert_eq!(request.method, "turn/interrupt");
        let params = request.params.expect("interrupt params");
        assert_eq!(params["threadId"], "worker-1");
        assert_eq!(params["turnId"], "");

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
                            "requirements-reviewer": { "modelID": "gpt-5.5", "reasoningEffort": "high" },
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
        assert_eq!(
            role_default_model(
                &state,
                Some(&temp.path().display().to_string()),
                Some("requirements-reviewer"),
            )
            .as_deref(),
            Some("gpt-5.5")
        );
        assert_eq!(
            role_default_reasoning_effort(
                &state,
                Some(&temp.path().display().to_string()),
                Some("requirementsReviewer"),
            )
            .as_deref(),
            Some("high")
        );
        assert_eq!(
            role_default_model(&state, Some(&temp.path().display().to_string()), Some("worker")).as_deref(),
            Some("gpt-5.4-nano")
        );
    }

    #[test]
    fn planner_role_defaults_use_planner_key_and_project_defaults() {
        let temp = TempDir::new().expect("tempdir");
        let project_root = temp.path().display().to_string();
        let state = parse_state(&json!({
            "projects": {
                "codex": {
                    "projectRoot": project_root,
                    "configs": {
                        "modelID": "gpt-project",
                        "reasoningEffort": "medium",
                        "roleModelReasoningDefaults": {
                            "planner": { "modelID": "gpt-planner", "reasoningEffort": "high" }
                        }
                    }
                }
            }
        }));

        assert_eq!(
            role_default_model(&state, Some(&project_root), Some("planner")).as_deref(),
            Some("gpt-planner")
        );
        assert_eq!(
            role_default_reasoning_effort(&state, Some(&project_root), Some("planner")).as_deref(),
            Some("high")
        );
        assert_eq!(
            role_default_model(&state, Some(&project_root), Some("worker")).as_deref(),
            Some("gpt-project")
        );
        assert_eq!(
            role_default_reasoning_effort(&state, Some(&project_root), Some("worker")).as_deref(),
            Some("medium")
        );
    }

    #[test]
    fn planner_turns_use_planner_schema_without_requirements() {
        let state = parse_state(&json!({
            "projects": {
                "codex": {
                    "projectRoot": "/tmp/codex",
                    "cwd": "/tmp/codex",
                    "agents": {
                        "planner-1": {
                            "displayName": "Planner",
                            "role": "planner",
                            "projectRoot": "/tmp/codex",
                            "cwd": "/tmp/codex"
                        },
                        "worker-1": {
                            "displayName": "Worker",
                            "role": "worker",
                            "projectRoot": "/tmp/codex",
                            "cwd": "/tmp/codex"
                        }
                    }
                }
            }
        }));

        let planner_schema = output_schema_for_thread_turn(&state, "planner-1").expect("schema");
        assert_eq!(planner_schema["required"], json!(["response", "clarification", "currentPlan"]));
        assert_eq!(planner_schema["properties"]["clarification"]["anyOf"][0]["required"], json!(["question", "options"]));
        assert_eq!(output_schema_for_thread_turn(&state, "worker-1"), Some(Value::Null));
    }

    #[test]
    fn project_update_persists_project_and_planner_defaults() {
        let mut state = parse_state(&json!({
            "projects": {
                "codex": {
                    "id": "project-codex",
                    "name": "Codex",
                    "projectRoot": "/tmp/codex",
                    "cwd": "/tmp/codex"
                }
            }
        }));

        update_project(
            &mut state,
            &json!({
                "projectId": "project-codex",
                "name": "Codex",
                "defaultCWD": "/tmp/codex",
                "modelID": "gpt-project",
                "reasoningEffort": "medium",
                "approvalPolicy": "on-request",
                "sandboxMode": "workspace-write",
                "networkAccess": true,
                "roleModelReasoningDefaults": {
                    "planner": { "modelID": "gpt-planner", "reasoningEffort": "high" }
                }
            }),
        )
        .expect("update project");

        let project = state.projects.get("codex").expect("project");
        assert_eq!(project.configs["modelID"], "gpt-project");
        assert_eq!(project.configs["reasoningEffort"], "medium");
        assert_eq!(project.configs["approvalPolicy"], "on-request");
        assert_eq!(project.configs["sandboxMode"], "workspace-write");
        assert_eq!(project.configs["networkAccess"], true);
        assert_eq!(project.configs["roleModelReasoningDefaults"]["planner"]["modelID"], "gpt-planner");
    }

    #[test]
    fn project_update_round_trips_requirements_reviewer_role_defaults() {
        let mut state = parse_state(&json!({
            "projects": {
                "codex": {
                    "id": "project-codex",
                    "name": "Codex",
                    "projectRoot": "/tmp/codex",
                    "cwd": "/tmp/codex"
                }
            }
        }));

        update_project(
            &mut state,
            &json!({
                "projectId": "project-codex",
                "name": "Codex",
                "defaultCWD": "/tmp/codex",
                "roleModelReasoningDefaults": {
                    "worker": { "modelID": "gpt-worker", "reasoningEffort": "medium" },
                    "requirements-reviewer": { "modelID": "gpt-reviewer", "reasoningEffort": "high" }
                },
                "requirementsPermanentComposables": ["no-legacy", "non-negotiables", "no-legacy"]
            }),
        )
        .expect("update project");

        let project = state.projects.get("codex").expect("project");
        assert_eq!(
            project.configs["roleModelReasoningDefaults"]["requirements-reviewer"]["modelID"],
            "gpt-reviewer"
        );
        let payload = bridge_state_payload(&state);
        assert_eq!(
            payload["projectRoleModelReasoningDefaultsByProjectPath"]["/tmp/codex"]
                ["requirements-reviewer"]["reasoningEffort"],
            "high"
        );
        assert_eq!(
            payload["projectCatalog"]["projects"][0]["permanentRequirementComposables"],
            json!(["no-legacy", "non-negotiables"])
        );
    }

    #[test]
    fn inactive_requirements_do_not_enforce_schema_but_remain_stored() {
        let state = parse_state(&json!({
            "projects": {
                "alpha": {
                    "projectRoot": "/tmp/alpha",
                    "agents": {
                        "worker-1": {
                            "role": "worker",
                            "requirements": {
                                "id": "requirements-alpha",
                                "active": false,
                                "enforceOnTurns": false,
                                "requirements": [{
                                    "key": "storedRequirement",
                                    "statement": "Stored inactive requirement.",
                                    "severity": "major",
                                    "verificationMethod": "manualEvidence"
                                }]
                            }
                        }
                    }
                }
            }
        }));

        let worker = state
            .projects
            .values()
            .find_map(|project| project.agents.get("worker-1"))
            .expect("worker");
        assert_eq!(worker.requirements.as_ref().map(|set| set.requirements.len()), Some(1));
        assert!(active_requirements_for_thread(&state, "worker-1").is_none());
        assert!(active_requirements_claim_schema_for_thread(&state, "worker-1").is_none());
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

    #[test]
    fn operator_developer_instructions_do_not_claim_auto_route() {
        let temp = TempDir::new().expect("tempdir");
        let state = parse_state(&json!({
            "projects": {
                "ops": {
                    "projectRoot": temp.path().display().to_string(),
                    "cwd": temp.path().display().to_string(),
                    "autoRouteReplies": true,
                    "routeApprovalRequests": true,
                    "orchestratorThreadId": "thread-orchestrator",
                    "configs": {}
                }
            }
        }));

        let guidance = developer_instructions_for_role(
            &state,
            Some("operator"),
            Some(&temp.path().display().to_string()),
            Some(&temp.path().display().to_string()),
        );

        assert!(guidance.is_none());
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
    async fn hidden_self_handoff_bypasses_peer_visibility_scope() {
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
                        "thread/start" => json!({"thread": {"id": "thread-hidden-2", "title": "Hidden Replacement"}}),
                        "turn/start" => {
                            thread_tx
                                .take()
                                .expect("thread sender")
                                .send(request.clone())
                                .expect("record prompt turn request");
                            json!({"turn": {"id": "turn-hidden-2", "status": "inProgress", "items": [], "error": null}})
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
                    "hidden-project": {
                        "id": "hidden-project",
                        "name": "Hidden",
                        "projectRoot": temp.path().display().to_string(),
                        "cwd": temp.path().join(".worktrees/hidden").display().to_string(),
                        "orchestratorThreadId": "thread-orchestrator",
                        "agents": {
                            "thread-orchestrator": {
                                "displayName": "Hidden Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().join(".worktrees").display().to_string()
                            },
                            "thread-hidden-1": {
                                "displayName": "Hidden Agent",
                                "role": "hidden",
                                "hiddenFromPeerList": true,
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().join(".worktrees/hidden").display().to_string(),
                                "approvalPolicy": "never",
                                "sandboxMode": "danger-full-access",
                                "networkAccess": true,
                                "modelID": "gpt-5.4",
                                "developerInstructions": "hidden guidance"
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let response = orchestrator_warm_handoff(
            &runtime,
            "thread-hidden-1",
            Some("thread-hidden-1"),
            None,
            Some(&temp.path().display().to_string()),
            "Resume the hidden operator work with this exact prompt.",
        )
        .await
        .expect("hidden self handoff");

        assert_eq!(response["previousThreadId"], "thread-hidden-1");
        assert_eq!(response["replacementThreadId"], "thread-hidden-2");

        let thread_request = thread_rx.await.expect("captured prompt turn request");
        assert_eq!(thread_request.method, "turn/start");
        let thread_params = thread_request.params.expect("prompt turn params");
        assert_eq!(thread_params["threadId"], json!("thread-hidden-2"));
        assert_eq!(
            thread_params["input"][0]["text"],
            json!("Resume the hidden operator work with this exact prompt.")
        );

        let state = parse_state(&runtime.state_document_value().await);
        let project = state.projects.values().next().expect("project state");
        assert!(!project.agents.contains_key("thread-hidden-1"));
        assert!(project.agents.contains_key("thread-hidden-2"));
        assert_eq!(
            project
                .agents
                .get("thread-hidden-2")
                .and_then(|agent| agent.role.as_deref()),
            Some("hidden")
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
    async fn archive_worker_with_requirements_archives_linked_reviewer_child() {
        let temp = TempDir::new().expect("tempdir");
        let (archive_tx, mut archive_rx) = mpsc::unbounded_channel::<String>();
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

                let mut archive_count = 0;
                while archive_count < 2 {
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
                    if request.method == "thread/archive" {
                        archive_count += 1;
                        archive_tx
                            .send(
                                request
                                    .params
                                    .as_ref()
                                    .and_then(|params| params.get("threadId"))
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_string(),
                            )
                            .expect("send archive id");
                    }
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
                        "threadGroups": [{
                            "id": "group-1",
                            "title": "Workers",
                            "threadIds": ["worker-1", "reviewer-1"]
                        }],
                        "agents": {
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": temp.path().display().to_string(),
                                "cwd": temp.path().display().to_string(),
                                "requirements": sample_requirement_set(),
                                "requirementReview": {
                                    "sourceThreadId": "worker-1",
                                    "reviewerThreadId": "reviewer-1",
                                    "requirementSetId": "requirements-alpha",
                                    "status": "inReview",
                                    "updatedAt": 100
                                }
                            },
                            "reviewer-1": {
                                "displayName": "Requirements Reviewer: Worker One",
                                "role": "requirements-reviewer",
                                "projectRoot": temp.path().display().to_string(),
                                "parentThreadId": "worker-1",
                                "hiddenFromPeerList": true
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        archive_thread(&runtime, "worker-1").await.expect("archive thread");

        let mut archived_ids = BTreeSet::new();
        for _ in 0..2 {
            archived_ids.insert(
                tokio::time::timeout(Duration::from_secs(1), archive_rx.recv())
                    .await
                    .expect("archive id timeout")
                    .expect("archive id"),
            );
        }
        assert_eq!(
            archived_ids,
            BTreeSet::from(["reviewer-1".to_string(), "worker-1".to_string()])
        );
        let state = parse_state(&runtime.state_document_value().await);
        assert!(agent_state_for_thread(&state, "worker-1").is_none());
        assert!(agent_state_for_thread(&state, "reviewer-1").is_none());
        let project = state.projects.get("alpha").expect("project");
        assert!(project.thread_groups.is_empty());

        transport.abort();
    }

    #[tokio::test]
    async fn orchestrator_group_archive_prunes_worker_requirements_reviewer_child() {
        let temp = TempDir::new().expect("tempdir");
        let project_root = normalize_path(temp.path().display().to_string());
        let (archive_tx, mut archive_rx) = mpsc::unbounded_channel::<String>();
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

                let mut archive_count = 0;
                while archive_count < 2 {
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
                    if request.method == "thread/archive" {
                        archive_count += 1;
                        archive_tx
                            .send(
                                request
                                    .params
                                    .as_ref()
                                    .and_then(|params| params.get("threadId"))
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_string(),
                            )
                            .expect("send archive id");
                    }
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
                        "projectRoot": project_root,
                        "cwd": project_root,
                        "orchestratorThreadId": "orch-1",
                        "threadGroups": [{
                            "id": "group-1",
                            "title": "Workers",
                            "threadIds": ["worker-1", "reviewer-1", "worker-2", "designer-1"]
                        }],
                        "agents": {
                            "orch-1": {
                                "displayName": "Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": project_root,
                                "cwd": project_root
                            },
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": project_root,
                                "cwd": project_root,
                                "requirements": sample_requirement_set(),
                                "requirementReview": {
                                    "sourceThreadId": "worker-1",
                                    "reviewerThreadId": "reviewer-1",
                                    "requirementSetId": "requirements-alpha",
                                    "status": "inReview",
                                    "updatedAt": 100
                                }
                            },
                            "reviewer-1": {
                                "displayName": "Requirements Reviewer: Worker One",
                                "role": "requirements-reviewer",
                                "projectRoot": project_root,
                                "parentThreadId": "worker-1",
                                "hiddenFromPeerList": true
                            },
                            "worker-2": {
                                "displayName": "Worker Two",
                                "role": "worker",
                                "projectRoot": project_root,
                                "cwd": project_root
                            },
                            "designer-1": {
                                "displayName": "Designer One",
                                "role": "designer",
                                "projectRoot": project_root,
                                "cwd": project_root
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let outcome = orchestrator_thread_group_archive(&runtime, "orch-1", None, "group-1")
            .await
            .expect("archive group");
        let archived_ids = outcome
            .get("archivedThreadIds")
            .and_then(Value::as_array)
            .expect("archived ids")
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            archived_ids,
            BTreeSet::from(["worker-1", "worker-2"])
        );
        let skipped_ids = outcome
            .get("skippedThreadIds")
            .and_then(Value::as_array)
            .expect("skipped ids")
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(skipped_ids, BTreeSet::from(["designer-1", "reviewer-1"]));
        let mut backend_archived_ids = BTreeSet::new();
        for _ in 0..2 {
            backend_archived_ids.insert(
                tokio::time::timeout(Duration::from_secs(1), archive_rx.recv())
                    .await
                    .expect("archive id timeout")
                    .expect("archive id"),
            );
        }
        assert_eq!(
            backend_archived_ids,
            BTreeSet::from(["worker-1".to_string(), "worker-2".to_string()])
        );
        let state = parse_state(&runtime.state_document_value().await);
        assert!(agent_state_for_thread(&state, "worker-1").is_none());
        assert!(agent_state_for_thread(&state, "reviewer-1").is_some());
        assert!(agent_state_for_thread(&state, "worker-2").is_none());
        assert!(agent_state_for_thread(&state, "orch-1").is_some());
        assert!(agent_state_for_thread(&state, "designer-1").is_some());
        let project = state.projects.get("alpha").expect("project");
        assert_eq!(project.thread_groups.len(), 1);
        assert_eq!(
            project.thread_groups[0].thread_ids,
            vec!["reviewer-1".to_string(), "designer-1".to_string()]
        );
        transport.abort();
    }

    #[tokio::test]
    async fn orchestrator_archive_worker_preserves_linked_non_worker_child() {
        let temp = TempDir::new().expect("tempdir");
        let project_root = normalize_path(temp.path().display().to_string());
        let (archive_tx, mut archive_rx) = mpsc::unbounded_channel::<String>();
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
                    if request.method == "thread/archive" {
                        archive_tx
                            .send(
                                request
                                    .params
                                    .as_ref()
                                    .and_then(|params| params.get("threadId"))
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_string(),
                            )
                            .expect("send archive id");
                    }
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
                        "projectRoot": project_root,
                        "cwd": project_root,
                        "orchestratorThreadId": "orch-1",
                        "threadGroups": [{
                            "id": "group-1",
                            "title": "Reviewing",
                            "threadIds": ["worker-1", "reviewer-1"]
                        }],
                        "agents": {
                            "orch-1": {
                                "displayName": "Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": project_root,
                                "cwd": project_root
                            },
                            "worker-1": {
                                "displayName": "Worker One",
                                "role": "worker",
                                "projectRoot": project_root,
                                "cwd": project_root,
                                "requirements": sample_requirement_set(),
                                "requirementReview": {
                                    "sourceThreadId": "worker-1",
                                    "reviewerThreadId": "reviewer-1",
                                    "requirementSetId": "requirements-alpha",
                                    "status": "inReview",
                                    "updatedAt": 100
                                }
                            },
                            "reviewer-1": {
                                "displayName": "Requirements Reviewer: Worker One",
                                "role": "requirements-reviewer",
                                "projectRoot": project_root,
                                "parentThreadId": "worker-1",
                                "hiddenFromPeerList": true
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let response = orchestrator_archive_agent(&runtime, "orch-1", Some("worker-1"), None, None)
            .await
            .expect("worker archive");
        assert_eq!(response["recipientThreadId"], "worker-1");
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), archive_rx.recv())
                .await
                .expect("archive id timeout")
                .expect("archive id"),
            "worker-1"
        );
        match tokio::time::timeout(Duration::from_millis(100), archive_rx.recv()).await {
            Ok(Some(extra)) => panic!("unexpected extra archived thread id: {extra}"),
            Ok(None) | Err(_) => {}
        }
        let state = parse_state(&runtime.state_document_value().await);
        assert!(agent_state_for_thread(&state, "worker-1").is_none());
        assert!(agent_state_for_thread(&state, "reviewer-1").is_some());
        let project = state.projects.get("alpha").expect("project");
        assert_eq!(project.thread_groups.len(), 1);
        assert_eq!(project.thread_groups[0].thread_ids, vec!["reviewer-1".to_string()]);
        transport.abort();
    }

    #[tokio::test]
    async fn orchestrator_archive_agent_rejects_designer() {
        let temp = TempDir::new().expect("tempdir");
        let project_root = normalize_path(temp.path().display().to_string());
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
                while let Some(next) = ws.next().await {
                    let next = next.expect("request frame");
                    let text = match next {
                        Message::Text(text) => text,
                        other => panic!("unexpected request frame: {other:?}"),
                    };
                    let request =
                        match serde_json::from_str::<JSONRPCMessage>(&text).expect("jsonrpc request") {
                            JSONRPCMessage::Request(request) => request,
                            other => panic!("unexpected request message: {other:?}"),
                        };
                    if request.method == "thread/archive" {
                        panic!("designer archive should be rejected before app-server archive");
                    }
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
                        "projectRoot": project_root,
                        "cwd": project_root,
                        "orchestratorThreadId": "orch-1",
                        "agents": {
                            "orch-1": {
                                "displayName": "Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": project_root,
                                "cwd": project_root
                            },
                            "designer-1": {
                                "displayName": "Designer One",
                                "role": "designer",
                                "projectRoot": project_root,
                                "cwd": project_root
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let error = orchestrator_archive_agent(&runtime, "orch-1", Some("designer-1"), None, None)
            .await
            .expect_err("designer archive should be rejected");
        assert!(
            error
                .to_string()
                .contains("Orchestrators may only archive worker and qa agents")
        );
        let state = parse_state(&runtime.state_document_value().await);
        assert!(agent_state_for_thread(&state, "designer-1").is_some());
        transport.abort();
    }

    #[test]
    fn orchestrator_archive_role_allowlist_is_worker_and_qa_only() {
        assert!(orchestrator_can_archive_agent_role("worker"));
        assert!(orchestrator_can_archive_agent_role("qa"));
        assert!(!orchestrator_can_archive_agent_role("designer"));
        assert!(!orchestrator_can_archive_agent_role("orchestrator"));
        assert!(!orchestrator_can_archive_agent_role("operator"));
        assert!(!orchestrator_can_archive_agent_role("planner"));
        assert!(!orchestrator_can_archive_agent_role("hidden"));
        assert!(!orchestrator_can_archive_agent_role("requirements-reviewer"));
    }

    #[tokio::test]
    async fn owner_archive_thread_can_archive_planner() {
        let temp = TempDir::new().expect("tempdir");
        let project_root = normalize_path(temp.path().display().to_string());
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:9".to_string()))
            .await
            .expect("runtime");
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "id": "project-alpha",
                        "name": "Alpha",
                        "projectRoot": project_root,
                        "cwd": project_root,
                        "orchestratorThreadId": "orch-1",
                        "agents": {
                            "orch-1": {
                                "displayName": "Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": project_root,
                                "cwd": project_root
                            },
                            "planner-1": {
                                "displayName": "Planner One",
                                "role": "planner",
                                "projectRoot": project_root,
                                "cwd": project_root
                            }
                        },
                        "threadGroups": [{
                            "id": "group-1",
                            "name": "Planning",
                            "threadIds": ["planner-1"]
                        }]
                    }
                }
            }))
            .await
            .expect("persist state");

        archive_thread(&runtime, "planner-1").await.expect("archive planner");

        let state = parse_state(&runtime.state_document_value().await);
        assert!(agent_state_for_thread(&state, "planner-1").is_none());
        assert!(state
            .projects
            .values()
            .all(|project| !project.agents.contains_key("planner-1")));
    }

    #[tokio::test]
    async fn orchestrator_archive_agent_rejects_planner() {
        let temp = TempDir::new().expect("tempdir");
        let project_root = normalize_path(temp.path().display().to_string());
        let runtime = BridgeRuntime::new(sample_settings(&temp, "ws://127.0.0.1:9".to_string()))
            .await
            .expect("runtime");
        runtime
            .persist_state_document(json!({
                "projects": {
                    "alpha": {
                        "id": "project-alpha",
                        "name": "Alpha",
                        "projectRoot": project_root,
                        "cwd": project_root,
                        "orchestratorThreadId": "orch-1",
                        "agents": {
                            "orch-1": {
                                "displayName": "Orchestrator",
                                "role": "orchestrator",
                                "projectRoot": project_root,
                                "cwd": project_root
                            },
                            "planner-1": {
                                "displayName": "Planner One",
                                "role": "planner",
                                "projectRoot": project_root,
                                "cwd": project_root
                            }
                        }
                    }
                }
            }))
            .await
            .expect("persist state");

        let error = orchestrator_archive_agent(&runtime, "orch-1", Some("planner-1"), None, None)
            .await
            .expect_err("planner archive should be rejected for orchestrator");
        assert!(
            error
                .to_string()
                .contains("Orchestrators may only archive worker and qa agents")
        );
        let state = parse_state(&runtime.state_document_value().await);
        assert!(agent_state_for_thread(&state, "planner-1").is_some());
    }

    #[test]
    fn planner_cannot_set_requirements_on_self() {
        assert!(!requirements_self_setting_allowed("planner"));
        assert!(!requirements_self_setting_allowed("worker"));
        assert!(!requirements_self_setting_allowed("qa"));
        assert!(requirements_self_setting_allowed("operator"));
        assert!(requirements_self_setting_allowed("hidden"));
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
    async fn approval_decision_allows_hidden_reviewer_to_resolve_own_pending_approval() {
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
        let mut state = PersistedState::default();
        let mut project = PersistedProjectState {
            project_root: Some(temp.path().display().to_string()),
            cwd: Some(temp.path().display().to_string()),
            ..Default::default()
        };
        project.agents.insert(
            "worker-a".to_string(),
            PersistedAgentState {
                display_name: Some("Worker A".to_string()),
                role: Some("worker".to_string()),
                project_root: Some(temp.path().display().to_string()),
                ..Default::default()
            },
        );
        project.agents.insert(
            "reviewer-a".to_string(),
            PersistedAgentState {
                display_name: Some("Requirements Reviewer: Worker A".to_string()),
                role: Some("requirements-reviewer".to_string()),
                project_root: Some(temp.path().display().to_string()),
                parent_thread_id: Some("worker-a".to_string()),
                hidden_from_peer_list: true,
                approval_policy: Some("never".to_string()),
                ..Default::default()
            },
        );
        state.projects.insert("alpha".to_string(), project);
        persist_state(&runtime, &state).await.expect("persist state");
        runtime
            .insert_pending_approval_for_test(crate::models::PendingApproval {
                id: "approval-hidden-reviewer".to_string(),
                instance_id: runtime.settings().project_path.display().to_string(),
                request_id: RequestId::Integer(101),
                thread_id: "reviewer-a".to_string(),
                turn_id: "turn-reviewer".to_string(),
                item_id: "item-reviewer".to_string(),
                kind: crate::models::PendingApprovalKind::CommandExecution,
                title: "Command approval".to_string(),
                detail: Some("needs approval".to_string()),
                approval_reason: Some("needs approval".to_string()),
                tool_name: None,
                tool_arguments: None,
                tool_questions: Vec::new(),
                auth_refresh_reason: None,
                command: Some("git fetch origin master".to_string()),
                command_cwd: Some(temp.path().display().to_string()),
                file_grant_root: None,
                file_changes: Vec::new(),
            })
            .await;
        let transport = runtime.spawn_transport();

        tokio::time::sleep(Duration::from_millis(150)).await;
        let outcome = orchestrator_approval_decision(
            &runtime,
            "reviewer-a",
            "approval-hidden-reviewer",
            "decline",
            Some("Requirements reviewers run with approval policy never."),
        )
        .await
        .expect("approval decision");
        assert_eq!(outcome["decision"], "decline");
        assert_eq!(outcome["resolved"], true);

        let response = response_rx.await.expect("response");
        match response {
            JSONRPCMessage::Response(response) => {
                assert_eq!(response.id, RequestId::Integer(101));
                assert_eq!(response.result["decision"], "decline");
            }
            other => panic!("unexpected message: {other:?}"),
        }
        assert!(runtime.pending_approvals().await.is_empty());

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
                        cwd: Some(PathBuf::from("/tmp/project").try_into().expect("absolute cwd")),
                        command_actions: None,
                        additional_permissions: None,
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
                        started_at: None,
                        completed_at: None,
                        duration_ms: None,
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
        let transport = runtime.spawn_transport();
        let outcome = tokio::time::timeout(
            Duration::from_secs(2),
            execute_bridge_command(
                &runtime,
                "skillsList",
                json!({
                    "cwds": ["/tmp/project"],
                    "forceReload": true
                }),
            ),
        )
        .await
        .expect("skillsList timed out")
        .expect("skillsList");

        let request = tokio::time::timeout(Duration::from_secs(2), request_rx)
            .await
            .expect("captured request timed out")
            .expect("captured request");
        assert_eq!(request.method, "skills/list");
        let params = request.params.expect("params");
        assert_eq!(params["cwds"][0], "/tmp/project");
        assert_eq!(outcome.payload["type"], "skillsList");
        assert_eq!(outcome.payload["payload"]["data"][0]["skills"][0]["name"], "robdex-orchestrator");
        transport.abort();
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

        assert_eq!(
            prune_archived_thread_locally(&mut state, "worker-1"),
            vec!["worker-1".to_string()]
        );
        let project = state.projects.get("alpha").expect("project");
        assert!(!project.agents.contains_key("worker-1"));
        assert_eq!(project.thread_groups.len(), 1);
        assert_eq!(project.thread_groups[0].thread_ids, vec!["worker-2".to_string()]);
    }

    #[test]
    fn prune_archived_thread_locally_removes_requirements_reviewer_child() {
        let mut state = PersistedState::default();
        let mut project = PersistedProjectState {
            project_root: Some("/alpha".to_string()),
            cwd: Some("/alpha".to_string()),
            orchestrator_thread_id: Some("orch-a".to_string()),
            thread_groups: vec![ThreadGroupState {
                id: "group-1".to_string(),
                title: "Reviewing".to_string(),
                thread_ids: vec![
                    "worker-1".to_string(),
                    "reviewer-1".to_string(),
                    "worker-2".to_string(),
                ],
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
                requirements: Some(sample_requirement_set()),
                requirement_review: Some(RequirementReviewBindingState {
                    source_thread_id: "worker-1".to_string(),
                    reviewer_thread_id: "reviewer-1".to_string(),
                    requirement_set_id: Some("requirements-alpha".to_string()),
                    status: "inReview".to_string(),
                    latest_claim_packet: None,
                    latest_verdict_packet: None,
                    updated_at: 100,
                }),
                ..Default::default()
            },
        );
        project.agents.insert(
            "reviewer-1".to_string(),
            PersistedAgentState {
                display_name: Some("Requirements Reviewer: Worker One".to_string()),
                role: Some("requirements-reviewer".to_string()),
                project_root: Some("/alpha".to_string()),
                parent_thread_id: Some("worker-1".to_string()),
                hidden_from_peer_list: true,
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

        assert_eq!(
            prune_archived_thread_locally(&mut state, "worker-1"),
            vec!["reviewer-1".to_string(), "worker-1".to_string()]
        );
        let project = state.projects.get("alpha").expect("project");
        assert!(!project.agents.contains_key("worker-1"));
        assert!(!project.agents.contains_key("reviewer-1"));
        assert!(project.agents.contains_key("worker-2"));
        assert_eq!(project.thread_groups.len(), 1);
        assert_eq!(project.thread_groups[0].thread_ids, vec!["worker-2".to_string()]);
    }

    #[test]
    fn requirements_review_target_uses_only_linked_reviewer_for_source() {
        let mut state = PersistedState::default();
        let mut project = PersistedProjectState {
            project_root: Some("/alpha".to_string()),
            cwd: Some("/alpha".to_string()),
            orchestrator_thread_id: Some("orch-a".to_string()),
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
        project.agents.insert(
            "reviewer-2".to_string(),
            PersistedAgentState {
                display_name: Some("Requirements Reviewer: Worker Two".to_string()),
                role: Some("requirements-reviewer".to_string()),
                project_root: Some("/alpha".to_string()),
                parent_thread_id: Some("worker-2".to_string()),
                hidden_from_peer_list: true,
                ..Default::default()
            },
        );
        state.projects.insert("alpha".to_string(), project);

        assert_eq!(
            requirements_review_target_for_thread(&state, "worker-1", &sample_requirement_set()),
            None
        );
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
