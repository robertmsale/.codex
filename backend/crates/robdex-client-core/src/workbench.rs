#[cfg(not(target_arch = "wasm32"))]
use anyhow::anyhow;
use anyhow::Result;
use serde_json::{Value, json};

use robdex_protocol::{
    UiChatEntry, UiInspectorFact, UiLiveProcessItem, UiModelItem, UiPendingApprovalItem,
    UiProjectItem, UiRequirementReviewRequirement, UiRequirementReviewSummary,
    UiRequirementVerdictSummary, UiThreadGroupItem, UiThreadItem, UiWorkerMetadata,
    UiWorkspaceFile, UiWorkspaceSelection, WorkbenchViewData,
};

use crate::{
    bridge::BridgeEndpoint,
    net::{HttpClient, delete as http_delete, get_json, http_client, post_empty, post_json},
};
#[cfg(not(target_arch = "wasm32"))]
use crate::net::post_bytes;

#[derive(Debug, Clone)]
pub struct WorkbenchClient {
    endpoint: BridgeEndpoint,
    client: HttpClient,
    selected_thread_id: Option<String>,
    available_models: Option<Vec<UiModelItem>>,
}

impl WorkbenchClient {
    pub fn localhost() -> Self {
        Self::new(BridgeEndpoint::localhost())
    }

    pub fn new(endpoint: BridgeEndpoint) -> Self {
        Self {
            endpoint,
            client: http_client(),
            selected_thread_id: None,
            available_models: None,
        }
    }

    pub fn endpoint(&self) -> &BridgeEndpoint {
        &self.endpoint
    }

    pub fn sync_view(&mut self, view: &WorkbenchViewData) {
        self.selected_thread_id = view.selection.thread_id.clone();
        if !view.available_models.is_empty() {
            self.available_models = Some(view.available_models.clone());
        }
    }

    pub async fn load_initial_view(&mut self) -> Result<WorkbenchViewData> {
        let snapshot = self.fetch_snapshot_json().await?;
        self.selected_thread_id = None;
        self.build_view_from_snapshot(snapshot, None, None).await
    }

    pub async fn select_thread(&mut self, thread_id: String) -> Result<WorkbenchViewData> {
        let snapshot = self.fetch_snapshot_json().await?;
        self.selected_thread_id = Some(thread_id.clone());
        self.build_view_from_snapshot(snapshot, Some(&thread_id), None).await
    }

    pub async fn refresh_thread_with_preserved_messages(
        &mut self,
        thread_id: String,
        preserved_messages: Vec<UiChatEntry>,
    ) -> Result<WorkbenchViewData> {
        let snapshot = self.fetch_snapshot_json().await?;
        self.selected_thread_id = Some(thread_id.clone());
        self.build_view_from_snapshot(snapshot, Some(&thread_id), Some(preserved_messages))
            .await
    }

    pub async fn fetch_thread_history(&self, thread_id: &str) -> Result<Vec<UiChatEntry>> {
        fetch_thread_messages(&self.endpoint, thread_id, None).await
    }

    pub async fn create_project(
        &mut self,
        name: String,
        root_path: String,
        default_cwd: String,
    ) -> Result<()> {
        post_json(&self.client, self.endpoint.http_base.join("/projects")?, json!({
                "name": name,
                "rootPath": root_path,
                "defaultCWD": default_cwd,
            }))
            .await?;
        Ok(())
    }

    pub async fn select_project(&mut self, project_id: Option<String>) -> Result<()> {
        post_json(
            &self.client,
            self.endpoint.http_base.join("/projects/select")?,
            json!({ "projectId": project_id }),
        )
        .await?;
        Ok(())
    }

    pub async fn delete_project(&mut self, project_id: String) -> Result<()> {
        http_delete(
            &self.client,
            self.endpoint.http_base.join(&format!("/projects/{project_id}"))?,
        )
        .await?;
        Ok(())
    }

    pub async fn update_project(
        &mut self,
        project_id: String,
        name: String,
        default_cwd: String,
        auto_route_replies: bool,
        route_approval_requests: bool,
        preferred_model_provider: Option<String>,
        orchestrator_model_id: Option<String>,
        orchestrator_reasoning_effort: Option<String>,
        worker_model_id: Option<String>,
        worker_reasoning_effort: Option<String>,
        qa_model_id: Option<String>,
        qa_reasoning_effort: Option<String>,
        designer_model_id: Option<String>,
        designer_reasoning_effort: Option<String>,
        orchestrator_developer_instructions: Option<String>,
        worker_developer_instructions: Option<String>,
        qa_developer_instructions: Option<String>,
        designer_developer_instructions: Option<String>,
        operator_developer_instructions: Option<String>,
        hidden_developer_instructions: Option<String>,
    ) -> Result<()> {
        post_json(&self.client, self.endpoint.http_base.join(&format!("/projects/{project_id}"))?, json!({
                "name": name,
                "defaultCWD": default_cwd,
                "autoRouteReplies": auto_route_replies,
                "routeApprovalRequests": route_approval_requests,
                "preferredModelProvider": preferred_model_provider,
                "roleModelReasoningDefaults": {
                    "orchestrator": {
                        "modelID": orchestrator_model_id,
                        "reasoningEffort": orchestrator_reasoning_effort,
                    },
                    "worker": {
                        "modelID": worker_model_id,
                        "reasoningEffort": worker_reasoning_effort,
                    },
                    "qa": {
                        "modelID": qa_model_id,
                        "reasoningEffort": qa_reasoning_effort,
                    },
                    "designer": {
                        "modelID": designer_model_id,
                        "reasoningEffort": designer_reasoning_effort,
                    }
                },
                "roleDeveloperInstructionsDefaults": {
                    "orchestrator": orchestrator_developer_instructions,
                    "worker": worker_developer_instructions,
                    "qa": qa_developer_instructions,
                    "designer": designer_developer_instructions,
                    "operator": operator_developer_instructions,
                    "hidden": hidden_developer_instructions,
                }
            }))
            .await?;
        Ok(())
    }

    pub async fn create_thread(
        &mut self,
        project_id: String,
        title: String,
        initial_prompt: String,
        role: String,
        approval_policy: Option<String>,
        sandbox_mode: Option<String>,
        network_access: Option<bool>,
        model_id: Option<String>,
        reasoning_effort: Option<String>,
    ) -> Result<()> {
        let payload = post_json(&self.client, self.endpoint.http_base.join("/threads")?, json!({
                "projectId": project_id,
                "title": title,
                "initialPrompt": initial_prompt,
                "role": role,
                "approvalPolicy": approval_policy,
                "sandboxMode": sandbox_mode,
                "networkAccess": network_access,
                "modelID": model_id,
                "reasoningEffort": reasoning_effort,
            }))
            .await?;
        self.selected_thread_id = payload.get("threadId").and_then(Value::as_str).map(str::to_string);
        Ok(())
    }

    pub async fn spawn_agent(
        &mut self,
        name: String,
        role: String,
        prompt: String,
    ) -> Result<()> {
        let sender_thread_id = self
            .selected_thread_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No orchestrator thread selected"))?;
        let payload = post_json(&self.client, self.endpoint.http_base.join("/orchestrator/spawn-agent")?, json!({
                "senderThreadId": sender_thread_id,
                "name": name,
                "role": role,
                "prompt": prompt,
            }))
            .await?;
        let thread_id = payload
            .get("agent")
            .and_then(Value::as_object)
            .and_then(|agent| agent.get("threadID").or_else(|| agent.get("threadId")).or_else(|| agent.get("id")))
            .and_then(Value::as_str)
            .map(str::to_string);
        self.selected_thread_id = thread_id;
        Ok(())
    }

    pub async fn set_project_orchestrator(
        &mut self,
        project_id: &str,
        project_path: &str,
        thread_id: &str,
    ) -> Result<()> {
        post_json(&self.client, self.endpoint.http_base.join(&format!("/projects/{project_id}/orchestrator"))?, json!({
                "threadId": thread_id,
                "projectPath": project_path,
            }))
            .await?;
        Ok(())
    }

    pub async fn create_thread_group(
        &mut self,
        sender_thread_id: &str,
        project_path: &str,
        title: &str,
        seed_thread_id: Option<&str>,
    ) -> Result<()> {
        post_json(&self.client, self.endpoint.http_base.join("/orchestrator/thread-groups/create")?, json!({
                "senderThreadId": sender_thread_id,
                "projectPath": project_path,
                "title": title,
                "seedThreadId": seed_thread_id,
            }))
            .await?;
        Ok(())
    }

    pub async fn update_thread_group(
        &mut self,
        sender_thread_id: &str,
        project_path: &str,
        group_id: &str,
        title: Option<&str>,
        collapsed: Option<bool>,
    ) -> Result<()> {
        post_json(&self.client, self.endpoint.http_base.join("/orchestrator/thread-groups/update")?, json!({
                "senderThreadId": sender_thread_id,
                "projectPath": project_path,
                "groupId": group_id,
                "title": title,
                "collapsed": collapsed,
            }))
            .await?;
        Ok(())
    }

    pub async fn move_thread_to_group(
        &mut self,
        sender_thread_id: &str,
        project_path: &str,
        thread_id: &str,
        target_group_id: Option<&str>,
    ) -> Result<()> {
        post_json(&self.client, self.endpoint.http_base.join("/orchestrator/thread-groups/move-thread")?, json!({
                "senderThreadId": sender_thread_id,
                "projectPath": project_path,
                "threadId": thread_id,
                "targetGroupId": target_group_id,
            }))
            .await?;
        Ok(())
    }

    pub async fn delete_thread_group(
        &mut self,
        sender_thread_id: &str,
        project_path: &str,
        group_id: &str,
    ) -> Result<()> {
        post_json(&self.client, self.endpoint.http_base.join("/orchestrator/thread-groups/delete")?, json!({
                "senderThreadId": sender_thread_id,
                "projectPath": project_path,
                "groupId": group_id,
            }))
            .await?;
        Ok(())
    }

    pub async fn archive_thread_group(
        &mut self,
        sender_thread_id: &str,
        project_path: &str,
        group_id: &str,
    ) -> Result<()> {
        post_json(&self.client, self.endpoint.http_base.join("/orchestrator/thread-groups/archive")?, json!({
                "senderThreadId": sender_thread_id,
                "projectPath": project_path,
                "groupId": group_id,
            }))
            .await?;
        Ok(())
    }

    pub async fn update_worker_metadata(
        &mut self,
        sender_thread_id: &str,
        recipient_thread_id: &str,
        project_path: &str,
        issue_number: Option<u64>,
        pull_request_number: Option<u64>,
        blocked_reason: Option<&str>,
        unblock_when: Option<&str>,
        clear_blocked: bool,
    ) -> Result<()> {
        post_json(&self.client, self.endpoint.http_base.join("/orchestrator/worker-metadata")?, json!({
                "senderThreadId": sender_thread_id,
                "recipientThreadId": recipient_thread_id,
                "projectPath": project_path,
                "issueNumber": issue_number,
                "clearIssueNumber": issue_number.is_none(),
                "pullRequestNumber": pull_request_number,
                "clearPullRequestNumber": pull_request_number.is_none(),
                "blockedReason": blocked_reason,
                "unblockWhen": unblock_when,
                "clearBlocked": clear_blocked,
            }))
            .await?;
        Ok(())
    }

    pub async fn warm_handoff(
        &mut self,
        sender_thread_id: &str,
        recipient_thread_id: &str,
        project_path: &str,
        prompt: &str,
    ) -> Result<WorkbenchViewData> {
        let payload = post_json(&self.client, self.endpoint.http_base.join("/orchestrator/warm-handoff")?, json!({
                "senderThreadId": sender_thread_id,
                "recipientThreadId": recipient_thread_id,
                "projectPath": project_path,
                "prompt": prompt,
            }))
            .await?;
        let replacement_thread_id = payload
            .get("replacementThreadId")
            .and_then(Value::as_str)
            .map(str::to_string);
        let snapshot = self.fetch_snapshot_json().await?;
        self.selected_thread_id = replacement_thread_id
            .clone()
            .or_else(|| self.selected_thread_id.clone());
        build_workbench(snapshot, self.selected_thread_id.as_deref(), None, &self.endpoint).await
    }

    pub async fn send_message(
        &self,
        thread_id: &str,
        text: &str,
        local_image_paths: &[String],
    ) -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        let uploaded_paths = local_image_paths.to_vec();
        #[cfg(not(target_arch = "wasm32"))]
        let mut uploaded_paths = Vec::with_capacity(local_image_paths.len());
        #[cfg(not(target_arch = "wasm32"))]
        for local_path in local_image_paths {
            uploaded_paths.push(self.upload_local_image(local_path).await?);
        }
        post_json(
            &self.client,
            self.endpoint
                .http_base
                .join(&format!("/threads/{thread_id}/messages"))?,
            json!({
                "text": text,
                "localImagePaths": uploaded_paths,
            }),
        )
        .await?;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn upload_local_image(&self, local_path: &str) -> Result<String> {
        let path = std::path::Path::new(local_path);
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("image");
        let bytes = std::fs::read(path)?;
        let response = post_bytes(
            &self.client,
            self.endpoint.http_base.join("/uploads/images")?,
            filename,
            bytes,
            "application/octet-stream",
        )
        .await?;
        response
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow!("bridge upload response missing path"))
    }

    pub async fn terminate_command_execution(&self, thread_id: &str, process_id: &str) -> Result<()> {
        post_json(
            &self.client,
            self.endpoint
                .http_base
                .join(&format!("/threads/{thread_id}/commands/terminate"))?,
            json!({ "processId": process_id }),
        )
        .await?;
        Ok(())
    }

    pub async fn decide_approval(
        &self,
        sender_thread_id: &str,
        approval_id: &str,
        decision: &str,
        message: Option<&str>,
    ) -> Result<()> {
        post_json(&self.client, self.endpoint.http_base.join("/orchestrator/approval-decision")?, json!({
                "senderThreadId": sender_thread_id,
                "approvalId": approval_id,
                "decision": decision,
                "message": message,
            }))
            .await?;
        Ok(())
    }

    pub async fn update_thread_metadata(
        &self,
        thread_id: &str,
        role: Option<&str>,
        approval_policy: Option<&str>,
        sandbox_mode: Option<&str>,
        network_access: Option<bool>,
        model_id: Option<&str>,
        reasoning_effort: Option<&str>,
        service_tier: Option<&str>,
    ) -> Result<()> {
        post_json(
            &self.client,
            self.endpoint
                .http_base
                .join(&format!("/threads/{thread_id}/metadata"))?,
            json!({
                "role": role,
                "approvalPolicy": approval_policy,
                "sandboxMode": sandbox_mode,
                "networkAccess": network_access,
                "modelID": model_id,
                "reasoningEffort": reasoning_effort,
                "serviceTier": service_tier,
            }),
        )
        .await?;
        Ok(())
    }

    pub async fn thread_compact(&self, thread_id: &str) -> Result<()> {
        post_empty(
            &self.client,
            self.endpoint
                .http_base
                .join(&format!("/threads/{thread_id}/compact"))?,
        )
        .await?;
        Ok(())
    }

    pub async fn set_thread_running_state(&self, thread_id: &str, running: bool) -> Result<()> {
        post_json(
            &self.client,
            self.endpoint
                .http_base
                .join(&format!("/threads/{thread_id}/running-state"))?,
            json!({
                "running": running,
            }),
        )
        .await?;
        Ok(())
    }

    pub async fn interrupt_thread(&self, thread_id: &str) -> Result<()> {
        post_empty(
            &self.client,
            self.endpoint
                .http_base
                .join(&format!("/threads/{thread_id}/interrupt"))?,
        )
        .await?;
        Ok(())
    }

    pub async fn rename_thread(&self, thread_id: &str, name: &str) -> Result<()> {
        post_json(
            &self.client,
            self.endpoint
                .http_base
                .join(&format!("/threads/{thread_id}/name"))?,
            json!({ "name": name }),
        )
        .await?;
        Ok(())
    }

    pub async fn archive_thread(&self, thread_id: &str) -> Result<()> {
        http_delete(
            &self.client,
            self.endpoint
                .http_base
                .join(&format!("/threads/{thread_id}"))?,
        )
        .await?;
        Ok(())
    }

    pub async fn refresh(&mut self, preserved_messages: Option<Vec<UiChatEntry>>) -> Result<WorkbenchViewData> {
        let snapshot = self.fetch_snapshot_json().await?;
        let selected_thread_id = self.selected_thread_id.clone();
        self.build_view_from_snapshot(snapshot, selected_thread_id.as_deref(), preserved_messages)
            .await
    }

    async fn fetch_snapshot_json(&self) -> Result<Value> {
        get_json(&self.client, self.endpoint.workbench_bootstrap_url()?).await
    }

    async fn ensure_available_models(&mut self) -> Result<Vec<UiModelItem>> {
        if let Some(models) = &self.available_models {
            return Ok(models.clone());
        }
        let models = fetch_available_models(&self.endpoint).await?;
        self.available_models = Some(models.clone());
        Ok(models)
    }

    async fn build_view_from_snapshot(
        &mut self,
        snapshot: Value,
        selected_thread_id: Option<&str>,
        preserved_messages: Option<Vec<UiChatEntry>>,
    ) -> Result<WorkbenchViewData> {
        let models = self.ensure_available_models().await.unwrap_or_default();
        let view = build_workbench_with_models(
            snapshot,
            selected_thread_id,
            preserved_messages,
            &self.endpoint,
            Some(models),
        )
        .await?;
        self.sync_view(&view);
        Ok(view)
    }
}

async fn fetch_available_models(endpoint: &BridgeEndpoint) -> Result<Vec<UiModelItem>> {
    let client = http_client();
    let payload = get_json(&client, endpoint.models_url()?).await?;
    Ok(payload
        .as_array()
        .map(|models| {
            models
                .iter()
                .filter_map(Value::as_object)
                .map(|model| UiModelItem {
                    id: model
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    name: model
                        .get("displayName")
                        .and_then(Value::as_str)
                        .or_else(|| model.get("name").and_then(Value::as_str))
                        .map(str::to_string),
                    hidden: model.get("hidden").and_then(Value::as_bool).unwrap_or(false),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default())
}

pub async fn build_workbench(
    snapshot: Value,
    selected_thread_id: Option<&str>,
    preserved_messages: Option<Vec<UiChatEntry>>,
    endpoint: &BridgeEndpoint,
) -> Result<WorkbenchViewData> {
    build_workbench_with_models(
        snapshot,
        selected_thread_id,
        preserved_messages,
        endpoint,
        None,
    )
    .await
}

pub async fn build_workbench_with_models(
    snapshot: Value,
    selected_thread_id: Option<&str>,
    preserved_messages: Option<Vec<UiChatEntry>>,
    endpoint: &BridgeEndpoint,
    available_models_override: Option<Vec<UiModelItem>>,
) -> Result<WorkbenchViewData> {
    let connection_status = snapshot
        .get("connectionStatus")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let available_models = available_models_override
        .unwrap_or_else(|| Vec::new());
    let available_models = if available_models.is_empty() {
        fetch_available_models(endpoint).await.unwrap_or_default()
    } else {
        available_models
    };
    let selected_project_id = selected_project_id(&snapshot);
    let running_ids = running_thread_ids(&snapshot);
    let project_records = extract_project_records(&snapshot);
    let visible_thread_records = extract_thread_records(&snapshot);
    let selectable_thread_records = extract_thread_records_including_hidden(&snapshot);

    let projects = project_records
        .iter()
        .map(|record| UiProjectItem {
            id: record.id.clone(),
            name: record.name.clone(),
            root_path: record.root_path.clone(),
            default_cwd: record.default_cwd.clone(),
            auto_route_replies: record.auto_route_replies,
            route_approval_requests: record.route_approval_requests,
            preferred_model_provider: record.preferred_model_provider.clone(),
            orchestrator_default_model: record.orchestrator_default_model.clone(),
            orchestrator_default_reasoning_effort: record.orchestrator_default_reasoning_effort.clone(),
            worker_default_model: record.worker_default_model.clone(),
            worker_default_reasoning_effort: record.worker_default_reasoning_effort.clone(),
            qa_default_model: record.qa_default_model.clone(),
            qa_default_reasoning_effort: record.qa_default_reasoning_effort.clone(),
            designer_default_model: record.designer_default_model.clone(),
            designer_default_reasoning_effort: record.designer_default_reasoning_effort.clone(),
            orchestrator_developer_instructions: record.orchestrator_developer_instructions.clone(),
            worker_developer_instructions: record.worker_developer_instructions.clone(),
            qa_developer_instructions: record.qa_developer_instructions.clone(),
            designer_developer_instructions: record.designer_developer_instructions.clone(),
            operator_developer_instructions: record.operator_developer_instructions.clone(),
            hidden_developer_instructions: record.hidden_developer_instructions.clone(),
            is_selected: Some(record.id.as_str()) == selected_project_id.as_deref(),
        })
        .collect::<Vec<_>>();

    let threads = visible_thread_records
        .iter()
        .map(|record| UiThreadItem {
            id: record.id.clone(),
            title: record.display_name.clone(),
            role: record.role.clone(),
            project_name: record.project_name.clone(),
            preview: record.preview.clone(),
            is_running: running_ids.contains(&record.id),
            unread_count: 0,
            requirement_review: record.requirement_review.clone(),
        })
        .collect::<Vec<_>>();

    let selected = selected_thread_id
        .and_then(|thread_id| selectable_thread_records.iter().find(|record| record.id == thread_id))
        .or_else(|| {
            if selected_thread_id.is_some() {
                visible_thread_records.first()
            } else {
                None
            }
        });

    let messages = if let Some(selected) = selected {
        if preserved_messages.is_some() && Some(selected.id.as_str()) == selected_thread_id {
            preserved_messages.unwrap_or_default()
        } else if let Some(messages) =
            snapshot_thread_messages(&snapshot, selected.id.as_str(), Some(50))
        {
            messages
        } else {
            fetch_thread_messages(endpoint, selected.id.as_str(), Some(50)).await?
        }
    } else {
        Vec::new()
    };

    let context_window_remaining_percent = selected.and_then(|selected| {
        snapshot
            .get("threadCache")
            .and_then(Value::as_object)
            .and_then(|cache| {
                cache.get("contextWindowStatusByThreadID")
                    .or_else(|| cache.get("contextWindowStatusByThreadId"))
            })
            .and_then(Value::as_object)
            .and_then(|items| items.get(&selected.id))
            .and_then(Value::as_object)
            .and_then(|status| {
                status
                    .get("remainingPercent")
                    .or_else(|| status.get("remaining_percent"))
            })
            .and_then(Value::as_u64)
            .map(|value| value as u32)
    });

    let selected_project_record = selected
        .and_then(|value| {
            extract_project_records(&snapshot)
                .into_iter()
                .find(|project| project.id == value.project_id)
        })
        .or_else(|| selected_project(&snapshot, selected_project_id.as_deref()));
    let global_defaults = extract_global_defaults(&snapshot);
    let effective_sandbox_mode = selected
        .and_then(|value| value.sandbox_mode.clone())
        .or_else(|| global_defaults.sandbox_mode.clone());
    let effective_network_access = selected
        .and_then(|value| value.network_access)
        .or(global_defaults.network_access);
    let effective_approval_policy = selected
        .and_then(|value| value.approval_policy.clone())
        .or_else(|| global_defaults.approval_policy.clone());
    let effective_model = selected
        .and_then(|value| value.model.clone())
        .or_else(|| role_default_model(selected_project_record.as_ref(), selected.map(|value| value.role.as_str())));
    let effective_reasoning_effort = selected
        .and_then(|value| value.reasoning_effort.clone())
        .or_else(|| {
            role_default_reasoning_effort(
                selected_project_record.as_ref(),
                selected.map(|value| value.role.as_str()),
            )
        });
    let effective_service_tier = selected.and_then(|value| value.service_tier.clone());

    let selection = UiWorkspaceSelection {
        project_id: selected
            .map(|value| value.project_id.clone())
            .or(selected_project_id.clone()),
        project_root_path: selected
            .map(|value| value.project_root.clone())
            .or_else(|| selected_project(&snapshot, selected_project_id.as_deref()).map(|project| project.root_path)),
        project_orchestrator_thread_id: selected
            .map(|value| value.project_orchestrator_thread_id.clone())
            .unwrap_or_else(|| {
                selected_project(&snapshot, selected_project_id.as_deref())
                    .and_then(|project| project.orchestrator_thread_id)
            }),
        project_orchestrator_name: selected
            .and_then(|value| value.project_orchestrator_name.clone())
            .or_else(|| {
                selected_project(&snapshot, selected_project_id.as_deref())
                    .and_then(|project| project.orchestrator_thread_id)
                    .and_then(|thread_id| {
                        selectable_thread_records
                            .iter()
                            .find(|record| record.id == thread_id)
                            .map(|record| record.display_name.clone())
                    })
            }),
        thread_id: selected.map(|value| value.id.clone()),
        thread_role: selected.map(|value| value.role.clone()),
        project_name: selected
            .map(|value| value.project_name.clone())
            .unwrap_or_else(|| "No Project".to_string()),
        thread_name: selected
            .map(|value| value.display_name.clone())
            .unwrap_or_else(|| "No Thread Selected".to_string()),
        connection_label: format!("Bridge {}", capitalize(connection_status)),
        sandbox_mode: selected.and_then(|value| value.sandbox_mode.clone()),
        network_access: selected.and_then(|value| value.network_access),
        approval_policy: selected.and_then(|value| value.approval_policy.clone()),
        model: selected.and_then(|value| value.model.clone()),
        reasoning_effort: selected.and_then(|value| value.reasoning_effort.clone()),
        service_tier: selected.and_then(|value| value.service_tier.clone()),
        effective_sandbox_mode,
        effective_network_access,
        effective_approval_policy,
        effective_model,
        effective_reasoning_effort,
        effective_service_tier,
        is_running: selected.map(|value| running_ids.contains(&value.id)).unwrap_or(false),
    };

    let workspace_files = selected
        .map(|selected| {
            vec![
                UiWorkspaceFile {
                    path: selected.cwd.clone(),
                    kind: "cwd".to_string(),
                    status: "active".to_string(),
                },
                UiWorkspaceFile {
                    path: selected.project_root.clone(),
                    kind: "project".to_string(),
                    status: "mounted".to_string(),
                },
            ]
        })
        .unwrap_or_default();

    let inspector_facts = selected
        .map(|selected| {
            vec![
                UiInspectorFact {
                    label: "Role".to_string(),
                    value: selected.role.clone(),
                },
                UiInspectorFact {
                    label: "Model".to_string(),
                    value: selected.model.clone().unwrap_or_else(|| "default".to_string()),
                },
                UiInspectorFact {
                    label: "Sandbox".to_string(),
                    value: selected
                        .sandbox_mode
                        .clone()
                        .unwrap_or_else(|| "default".to_string()),
                },
                UiInspectorFact {
                    label: "Network".to_string(),
                    value: match selected.network_access {
                        Some(true) => "enabled".to_string(),
                        Some(false) => "disabled".to_string(),
                        None => "default".to_string(),
                    },
                },
                UiInspectorFact {
                    label: "Project".to_string(),
                    value: selected.project_name.clone(),
                },
            ]
        })
        .unwrap_or_default();

    Ok(WorkbenchViewData {
        projects,
        selection,
        threads,
        available_models,
        thread_groups: selected_project_thread_groups(&snapshot, selected_project_id.as_deref(), selected.as_ref()),
        live_processes: selected
            .map(|value| value.live_processes.clone())
            .unwrap_or_default(),
        chat_entries: messages,
        context_window_remaining_percent,
        workspace_files,
        inspector_facts,
        pending_approvals: extract_pending_approvals(&snapshot),
        worker_metadata: selected.and_then(worker_metadata_from_record),
        requirement_review: selected.and_then(|record| record.requirement_review.clone()),
        status_headline: format!("Bridge {}", capitalize(connection_status)),
        status_detail: format!(
            "{} visible threads across {} projects. Selected history loads on demand from the live bridge.",
            visible_thread_records.len(),
            project_records.len()
        ),
        composer_hint: String::new(),
    })
}

pub fn context_window_remaining_percent_from_thread_payload(payload: &Value) -> Option<u32> {
    payload
        .get("contextWindowStatus")
        .or_else(|| payload.get("context_window_status"))
        .and_then(Value::as_object)
        .and_then(|status| {
            status
                .get("remainingPercent")
                .or_else(|| status.get("remaining_percent"))
        })
        .and_then(Value::as_u64)
        .map(|value| value as u32)
}

fn snapshot_thread_messages(
    snapshot: &Value,
    thread_id: &str,
    limit: Option<usize>,
) -> Option<Vec<UiChatEntry>> {
    let messages = snapshot
        .get("threadCache")
        .and_then(Value::as_object)
        .and_then(|cache| {
            cache.get("messageCacheByThreadID")
                .or_else(|| cache.get("messageCacheByThreadId"))
                .or_else(|| cache.get("message_cache_by_thread_id"))
        })
        .and_then(Value::as_object)
        .and_then(|items| items.get(thread_id))
        .and_then(Value::as_array)?;

    let start = limit
        .map(|value| messages.len().saturating_sub(value))
        .unwrap_or(0);

    Some(
        messages[start..]
            .iter()
            .filter_map(Value::as_object)
            .map(chat_entry_from_message)
            .collect(),
    )
}

pub async fn fetch_thread_messages(
    endpoint: &BridgeEndpoint,
    thread_id: &str,
    limit: Option<usize>,
) -> Result<Vec<UiChatEntry>> {
    let client = http_client();
    let mut url = endpoint.http_base.join("/threads/messages")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("threadId", thread_id);
        if let Some(limit) = limit {
            query.append_pair("limit", &limit.to_string());
        }
    }
    let payload = match get_json(&client, url).await {
        Ok(payload) => payload,
        Err(error) => {
            return Ok(vec![UiChatEntry {
                id: "messages-unavailable".to_string(),
                author: "Bridge".to_string(),
                display_label: "Bridge".to_string(),
                timestamp: None,
                body: format!("Thread history unavailable ({error})."),
                subtitle: None,
                kind: None,
                status: None,
                process_id: None,
                command: None,
                output: None,
                delivery_state: None,
                is_streaming: false,
                is_tool: true,
            }]);
        }
    };
    Ok(chat_entries_from_thread_payload(&payload))
}

pub fn chat_entries_from_thread_payload(payload: &Value) -> Vec<UiChatEntry> {
    payload
        .get("messages")
        .and_then(Value::as_array)
        .map(|messages| {
            messages
                .iter()
                .filter_map(Value::as_object)
                .map(chat_entry_from_message)
                .collect()
        })
        .unwrap_or_default()
}

fn chat_entry_from_message(message: &serde_json::Map<String, Value>) -> UiChatEntry {
    let role = message.get("role").and_then(Value::as_str);
    let tool_metadata = message.get("toolMetadata").and_then(Value::as_object);
    let kind = tool_metadata
        .and_then(|tool| tool.get("kind"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let status = tool_metadata
        .and_then(|tool| tool.get("status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let body = message
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let subtitle = message.get("subtitle").and_then(Value::as_str).map(str::to_string);
    let author = author_for_role(role);
    let display_label = display_label_for_message(role, kind.as_deref(), subtitle.as_deref(), body.as_str());
    UiChatEntry {
        id: message
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("message")
            .to_string(),
        author,
        display_label,
        timestamp: parse_timestamp(message.get("createdAt")),
        body,
        subtitle,
        kind,
        status,
        process_id: tool_metadata
            .and_then(|tool| tool.get("processId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        command: tool_metadata
            .and_then(|tool| tool.get("command"))
            .and_then(Value::as_str)
            .map(str::to_string),
        output: tool_metadata
            .and_then(|tool| tool.get("output"))
            .and_then(Value::as_str)
            .map(str::to_string),
        delivery_state: message
            .get("deliveryState")
            .and_then(Value::as_str)
            .map(str::to_string),
        is_streaming: is_streaming(message),
        is_tool: role == Some("tool"),
    }
}

fn selected_project_id(snapshot: &Value) -> Option<String> {
    snapshot
        .get("state")
        .and_then(Value::as_object)
        .and_then(|state| {
            state
                .get("selectedProjectID")
                .and_then(Value::as_str)
                .or_else(|| state.get("selectedProjectId").and_then(Value::as_str))
        })
        .map(str::to_string)
}

fn running_thread_ids(snapshot: &Value) -> std::collections::BTreeSet<String> {
    snapshot
        .get("threadCache")
        .and_then(Value::as_object)
        .and_then(|cache| {
            cache
                .get("runningThreadIDs")
                .or_else(|| cache.get("runningThreadIds"))
        })
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Clone)]
struct ThreadRecord {
    id: String,
    project_id: String,
    project_name: String,
    project_root: String,
    project_orchestrator_thread_id: Option<String>,
    project_orchestrator_name: Option<String>,
    display_name: String,
    role: String,
    cwd: String,
    sandbox_mode: Option<String>,
    network_access: Option<bool>,
    approval_policy: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<String>,
    service_tier: Option<String>,
    issue_number: Option<u64>,
    pull_request_number: Option<u64>,
    blocked_reason: Option<String>,
    unblock_when: Option<String>,
    live_processes: Vec<UiLiveProcessItem>,
    requirement_review: Option<UiRequirementReviewSummary>,
    preview: String,
}

#[derive(Clone)]
struct ProjectRecord {
    id: String,
    name: String,
    root_path: String,
    default_cwd: String,
    orchestrator_thread_id: Option<String>,
    auto_route_replies: bool,
    route_approval_requests: bool,
    preferred_model_provider: Option<String>,
    orchestrator_default_model: Option<String>,
    orchestrator_default_reasoning_effort: Option<String>,
    worker_default_model: Option<String>,
    worker_default_reasoning_effort: Option<String>,
    qa_default_model: Option<String>,
    qa_default_reasoning_effort: Option<String>,
    designer_default_model: Option<String>,
    designer_default_reasoning_effort: Option<String>,
    orchestrator_developer_instructions: Option<String>,
    worker_developer_instructions: Option<String>,
    qa_developer_instructions: Option<String>,
    designer_developer_instructions: Option<String>,
    operator_developer_instructions: Option<String>,
    hidden_developer_instructions: Option<String>,
}

#[derive(Clone)]
struct GlobalDefaults {
    approval_policy: Option<String>,
    sandbox_mode: Option<String>,
    network_access: Option<bool>,
}

fn extract_thread_records(snapshot: &Value) -> Vec<ThreadRecord> {
    extract_thread_records_with_options(snapshot, false)
}

fn extract_thread_records_including_hidden(snapshot: &Value) -> Vec<ThreadRecord> {
    extract_thread_records_with_options(snapshot, true)
}

fn extract_thread_records_with_options(snapshot: &Value, include_hidden_from_peer_list: bool) -> Vec<ThreadRecord> {
    let mut records = Vec::new();
    let Some(projects) = snapshot
        .get("state")
        .and_then(Value::as_object)
        .and_then(|state| state.get("projects"))
        .and_then(Value::as_object)
    else {
        return records;
    };

    for (project_key, project) in projects {
        let Some(project) = project.as_object() else {
            continue;
        };
        let project_name = project
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(project_key)
            .to_string();
        let project_root = project
            .get("projectRoot")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let project_id = project
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(project_key)
            .to_string();
        let Some(agents) = project.get("agents").and_then(Value::as_object) else {
            continue;
        };
        let orchestrator_thread_id = project
            .get("orchestratorThreadID")
            .and_then(Value::as_str)
            .or_else(|| project.get("orchestratorThreadId").and_then(Value::as_str))
            .map(str::to_string);
        for (thread_id, agent) in agents {
            let Some(agent) = agent.as_object() else {
                continue;
            };
            let archived = agent.get("archived").and_then(Value::as_bool) == Some(true);
            let hidden_from_peer_list = agent
                .get("hiddenFromPeerList")
                .and_then(Value::as_bool)
                == Some(true);
            let role = agent.get("role").and_then(Value::as_str).unwrap_or("worker");
            if archived || (!include_hidden_from_peer_list && hidden_from_peer_list) {
                continue;
            }
            let cwd = agent
                .get("cwd")
                .and_then(Value::as_str)
                .unwrap_or(project_root.as_str())
                .to_string();
            records.push(ThreadRecord {
                id: thread_id.clone(),
                project_id: project_id.clone(),
                project_name: project_name.clone(),
                project_root: project_root.clone(),
                project_orchestrator_thread_id: orchestrator_thread_id.clone(),
                project_orchestrator_name: None,
                display_name: agent
                    .get("displayName")
                    .and_then(Value::as_str)
                    .unwrap_or(thread_id)
                    .to_string(),
                role: role.to_string(),
                cwd: cwd.clone(),
                sandbox_mode: agent.get("sandboxMode").and_then(Value::as_str).map(str::to_string),
                network_access: agent.get("networkAccess").and_then(Value::as_bool),
                approval_policy: agent
                    .get("approvalPolicy")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                model: agent.get("model").and_then(Value::as_str).map(str::to_string),
                reasoning_effort: agent
                    .get("reasoningEffort")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                service_tier: agent
                    .get("serviceTier")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                issue_number: agent.get("issueNumber").and_then(Value::as_u64),
                pull_request_number: agent.get("pullRequestNumber").and_then(Value::as_u64),
                blocked_reason: agent
                    .get("blockedReason")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                unblock_when: agent
                    .get("unblockWhen")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                live_processes: snapshot_live_processes(snapshot, thread_id),
                requirement_review: requirement_review_summary(agent),
                preview: format!("{} · {}", role.to_uppercase(), cwd),
            });
        }
    }
    let display_name_by_id = records
        .iter()
        .map(|record| (record.id.clone(), record.display_name.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    for record in &mut records {
        record.project_orchestrator_name = record
            .project_orchestrator_thread_id
            .as_ref()
            .and_then(|thread_id| display_name_by_id.get(thread_id))
            .cloned();
    }
    records.sort_by(|left, right| {
        left.project_name
            .cmp(&right.project_name)
            .then(role_sort_key(&left.role).cmp(&role_sort_key(&right.role)))
            .then(left.display_name.to_lowercase().cmp(&right.display_name.to_lowercase()))
    });
    records
}

fn snapshot_live_processes(snapshot: &Value, thread_id: &str) -> Vec<UiLiveProcessItem> {
    snapshot
        .get("liveProcessesByThreadID")
        .and_then(Value::as_object)
        .and_then(|items| items.get(thread_id))
        .and_then(Value::as_array)
        .map(|items| parse_live_process_items(items))
        .unwrap_or_default()
}

fn requirement_review_summary(agent: &serde_json::Map<String, Value>) -> Option<UiRequirementReviewSummary> {
    let requirements_set = agent.get("requirements").and_then(Value::as_object);
    let active_requirements = requirements_set
        .and_then(|set| set.get("requirements"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_object)
                .filter_map(|requirement| {
                    let key = requirement.get("key").and_then(Value::as_str)?.to_string();
                    Some(UiRequirementReviewRequirement {
                        key,
                        statement: requirement
                            .get("statement")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        severity: requirement
                            .get("severity")
                            .and_then(Value::as_str)
                            .unwrap_or("medium")
                            .to_string(),
                        verification_method: requirement
                            .get("verificationMethod")
                            .and_then(Value::as_str)
                            .unwrap_or("manualEvidence")
                            .to_string(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let review = agent.get("requirementReview").and_then(Value::as_object);
    let parent_thread_id = agent
        .get("parentThreadId")
        .and_then(Value::as_str)
        .map(str::to_string);
    if active_requirements.is_empty() && review.is_none() && parent_thread_id.is_none() {
        return None;
    }

    let latest_verdict_packet = review
        .and_then(|review| review.get("latestVerdictPacket"))
        .cloned();
    let latest_claim_packet = review
        .and_then(|review| review.get("latestClaimPacket"))
        .cloned();
    let mut passed_count = 0;
    let mut failed_count = 0;
    let mut blocked_count = 0;
    let mut waiver_required_count = 0;
    let mut unknown_count = 0;
    let verdicts = active_requirements
        .iter()
        .map(|requirement| {
            let verdict_object = latest_verdict_packet
                .as_ref()
                .and_then(|packet| packet.get(&requirement.key))
                .and_then(Value::as_object);
            let verdict = verdict_object
                .and_then(|item| item.get("verdict"))
                .and_then(Value::as_str)
                .map(str::to_string);
            match verdict.as_deref() {
                Some("pass") => passed_count += 1,
                Some("fail") | Some("rejectedBlocked") => failed_count += 1,
                Some("acceptedBlocked") => blocked_count += 1,
                Some("waiverRequired") => waiver_required_count += 1,
                _ => unknown_count += 1,
            }
            UiRequirementVerdictSummary {
                key: requirement.key.clone(),
                verdict,
                reason: verdict_object
                    .and_then(|item| item.get("reason"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                evidence_assessment: verdict_object
                    .and_then(|item| item.get("evidenceAssessment"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                required_correction: verdict_object
                    .and_then(|item| item.get("requiredCorrection"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            }
        })
        .collect::<Vec<_>>();

    Some(UiRequirementReviewSummary {
        active_requirement_count: active_requirements.len(),
        status: review
            .and_then(|review| review.get("status"))
            .and_then(Value::as_str)
            .map(str::to_string),
        reviewer_thread_id: review
            .and_then(|review| review.get("reviewerThreadId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        parent_thread_id,
        requirement_set_id: review
            .and_then(|review| review.get("requirementSetId"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                requirements_set
                    .and_then(|set| set.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
        latest_claim_packet,
        latest_verdict_packet,
        passed_count,
        failed_count,
        blocked_count,
        waiver_required_count,
        unknown_count,
        updated_at: review
            .and_then(|review| review.get("updatedAt"))
            .and_then(Value::as_u64),
        requirements: active_requirements,
        verdicts,
    })
}

pub fn parse_live_process_items(items: &[Value]) -> Vec<UiLiveProcessItem> {
    items
        .iter()
        .filter_map(Value::as_object)
        .map(|item| UiLiveProcessItem {
            process_id: item
                .get("processId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            pid: item.get("pid").and_then(Value::as_i64),
            process_group_id: item.get("processGroupId").and_then(Value::as_i64),
            command: item
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            cwd: item.get("cwd").and_then(Value::as_str).map(str::to_string),
            started_at: item.get("startedAt").and_then(Value::as_u64),
        })
        .filter(|item| !item.process_id.trim().is_empty())
        .collect()
}

fn role_sort_key(role: &str) -> (u8, &str) {
    match role {
        "operator" => (0, role),
        "orchestrator" => (1, role),
        "worker" => (2, role),
        "designer" => (3, role),
        "qa" => (4, role),
        "hidden" => (9, role),
        _ => (5, role),
    }
}

fn extract_project_records(snapshot: &Value) -> Vec<ProjectRecord> {
    let mut records = Vec::new();
    let Some(projects) = snapshot
        .get("state")
        .and_then(Value::as_object)
        .and_then(|state| state.get("projects"))
        .and_then(Value::as_object)
    else {
        return records;
    };
    for (project_key, project) in projects {
        let Some(project) = project.as_object() else {
            continue;
        };
        if project.get("archived").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let root_path = project
            .get("projectRoot")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let role_defaults = project
            .get("configs")
            .and_then(|value| value.get("roleModelReasoningDefaults"))
            .cloned()
            .unwrap_or(Value::Null);
        let developer_defaults = project
            .get("configs")
            .and_then(|value| value.get("roleDeveloperInstructionsDefaults"))
            .cloned()
            .unwrap_or(Value::Null);
        records.push(ProjectRecord {
            id: project
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(project_key)
                .to_string(),
            name: project
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(project_key)
                .to_string(),
            default_cwd: project
                .get("cwd")
                .and_then(Value::as_str)
                .unwrap_or(root_path.as_str())
                .to_string(),
            auto_route_replies: project
                .get("autoRouteReplies")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            route_approval_requests: project
                .get("routeApprovalRequests")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            preferred_model_provider: project
                .get("preferredModelProvider")
                .and_then(Value::as_str)
                .map(str::to_string),
            orchestrator_default_model: role_defaults
                .get("orchestrator")
                .and_then(|value| value.get("modelID"))
                .and_then(Value::as_str)
                .map(str::to_string),
            orchestrator_default_reasoning_effort: role_defaults
                .get("orchestrator")
                .and_then(|value| value.get("reasoningEffort"))
                .and_then(Value::as_str)
                .map(str::to_string),
            worker_default_model: role_defaults
                .get("worker")
                .and_then(|value| value.get("modelID"))
                .and_then(Value::as_str)
                .map(str::to_string),
            worker_default_reasoning_effort: role_defaults
                .get("worker")
                .and_then(|value| value.get("reasoningEffort"))
                .and_then(Value::as_str)
                .map(str::to_string),
            qa_default_model: role_defaults
                .get("qa")
                .and_then(|value| value.get("modelID"))
                .and_then(Value::as_str)
                .map(str::to_string),
            qa_default_reasoning_effort: role_defaults
                .get("qa")
                .and_then(|value| value.get("reasoningEffort"))
                .and_then(Value::as_str)
                .map(str::to_string),
            designer_default_model: role_defaults
                .get("designer")
                .and_then(|value| value.get("modelID"))
                .and_then(Value::as_str)
                .map(str::to_string),
            designer_default_reasoning_effort: role_defaults
                .get("designer")
                .and_then(|value| value.get("reasoningEffort"))
                .and_then(Value::as_str)
                .map(str::to_string),
            orchestrator_developer_instructions: developer_defaults
                .get("orchestrator")
                .and_then(Value::as_str)
                .map(str::to_string),
            worker_developer_instructions: developer_defaults
                .get("worker")
                .and_then(Value::as_str)
                .map(str::to_string),
            qa_developer_instructions: developer_defaults
                .get("qa")
                .and_then(Value::as_str)
                .map(str::to_string),
            designer_developer_instructions: developer_defaults
                .get("designer")
                .and_then(Value::as_str)
                .map(str::to_string),
            operator_developer_instructions: developer_defaults
                .get("operator")
                .and_then(Value::as_str)
                .map(str::to_string),
            hidden_developer_instructions: developer_defaults
                .get("hidden")
                .and_then(Value::as_str)
                .map(str::to_string),
            root_path,
            orchestrator_thread_id: project
                .get("orchestratorThreadID")
                .and_then(Value::as_str)
                .or_else(|| project.get("orchestratorThreadId").and_then(Value::as_str))
                .map(str::to_string),
        });
    }
    records.sort_by(|left, right| left.name.cmp(&right.name));
    records
}

fn extract_global_defaults(snapshot: &Value) -> GlobalDefaults {
    let state = snapshot.get("state").and_then(Value::as_object);
    let configs = state.and_then(|value| value.get("globalConfigs"));
    GlobalDefaults {
        approval_policy: configs
            .and_then(|value| value.get("approvalPolicy"))
            .and_then(Value::as_str)
            .map(str::to_string),
        sandbox_mode: configs
            .and_then(|value| value.get("sandboxMode"))
            .and_then(Value::as_str)
            .map(str::to_string),
        network_access: configs
            .and_then(|value| value.get("networkAccess"))
            .and_then(Value::as_bool),
    }
}

fn role_default_model(project: Option<&ProjectRecord>, role: Option<&str>) -> Option<String> {
    match role {
        Some("orchestrator") => project.and_then(|value| value.orchestrator_default_model.clone()),
        Some("worker") | Some("hidden") | Some("operator") => {
            project.and_then(|value| value.worker_default_model.clone())
        }
        Some("designer") => project.and_then(|value| value.designer_default_model.clone()),
        Some("qa") => project.and_then(|value| value.qa_default_model.clone()),
        _ => None,
    }
}

fn role_default_reasoning_effort(
    project: Option<&ProjectRecord>,
    role: Option<&str>,
) -> Option<String> {
    match role {
        Some("orchestrator") => {
            project.and_then(|value| value.orchestrator_default_reasoning_effort.clone())
        }
        Some("worker") | Some("hidden") | Some("operator") => {
            project.and_then(|value| value.worker_default_reasoning_effort.clone())
        }
        Some("designer") => project.and_then(|value| value.designer_default_reasoning_effort.clone()),
        Some("qa") => project.and_then(|value| value.qa_default_reasoning_effort.clone()),
        _ => None,
    }
}

fn selected_project<'a>(snapshot: &'a Value, selected_project_id: Option<&str>) -> Option<ProjectRecord> {
    let project_id = selected_project_id?;
    extract_project_records(snapshot)
        .into_iter()
        .find(|project| project.id == project_id)
}

fn selected_project_thread_groups(
    snapshot: &Value,
    selected_project_id: Option<&str>,
    selected_thread: Option<&&ThreadRecord>,
) -> Vec<UiThreadGroupItem> {
    let project_root = selected_thread
        .map(|thread| thread.project_root.clone())
        .or_else(|| selected_project(snapshot, selected_project_id).map(|project| project.root_path));
    let Some(project_root) = project_root else {
        return Vec::new();
    };
    let Some(projects) = snapshot
        .get("state")
        .and_then(Value::as_object)
        .and_then(|state| state.get("projects"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    projects
        .values()
        .filter_map(Value::as_object)
        .find(|project| {
            project
                .get("projectRoot")
                .and_then(Value::as_str)
                .unwrap_or_default()
                == project_root
        })
        .and_then(|project| project.get("threadGroups"))
        .and_then(Value::as_array)
        .map(|groups| {
            groups
                .iter()
                .filter_map(Value::as_object)
                .map(|group| UiThreadGroupItem {
                    id: group.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
                    title: group
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("Group")
                        .to_string(),
                    thread_ids: group
                        .get("threadIDs")
                        .or_else(|| group.get("threadIds"))
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default(),
                    is_collapsed: group
                        .get("isCollapsed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn worker_metadata_from_record(record: &ThreadRecord) -> Option<UiWorkerMetadata> {
    if record.role == "orchestrator" {
        return None;
    }
    Some(UiWorkerMetadata {
        thread_id: record.id.clone(),
        issue_number: record.issue_number,
        pull_request_number: record.pull_request_number,
        blocked_reason: record.blocked_reason.clone(),
        unblock_when: record.unblock_when.clone(),
    })
}

fn extract_pending_approvals(snapshot: &Value) -> Vec<UiPendingApprovalItem> {
    snapshot
        .get("pendingApprovals")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter()
                .filter_map(Value::as_object)
                .map(|approval| UiPendingApprovalItem {
                    id: approval.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
                    thread_id: approval
                        .get("threadID")
                        .and_then(Value::as_str)
                        .or_else(|| approval.get("threadId").and_then(Value::as_str))
                        .unwrap_or_default()
                        .to_string(),
                    kind: approval
                        .get("kind")
                        .and_then(Value::as_object)
                        .and_then(|kind| kind.keys().next().cloned())
                        .unwrap_or_else(|| "approval".to_string()),
                    title: approval
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("Approval Request")
                        .to_string(),
                    detail: approval
                        .get("detail")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| approval.get("approvalReason").and_then(Value::as_str).map(str::to_string)),
                    command: approval.get("command").and_then(Value::as_str).map(str::to_string),
                    command_cwd: approval
                        .get("commandCWD")
                        .and_then(Value::as_str)
                        .or_else(|| approval.get("commandCwd").and_then(Value::as_str))
                        .map(str::to_string),
                    file_paths: approval
                        .get("fileChanges")
                        .and_then(Value::as_array)
                        .map(|changes| {
                            let mut out = std::collections::BTreeSet::new();
                            for change in changes {
                                if let Some(path) = change.get("path").and_then(Value::as_str) {
                                    out.insert(path.to_string());
                                }
                            }
                            out.into_iter().collect()
                        })
                        .unwrap_or_default(),
                })
                .filter(|item| !item.id.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_review_threads_are_filtered_but_selectable() {
        let snapshot = json!({
            "state": {
                "projects": {
                    "p1": {
                        "id": "p1",
                        "name": "Project",
                        "projectRoot": "/tmp/project",
                        "agents": {
                            "source": {
                                "displayName": "Source",
                                "role": "worker",
                                "cwd": "/tmp/project/source"
                            },
                            "reviewer": {
                                "displayName": "Requirements Reviewer",
                                "role": "requirementsReviewer",
                                "cwd": "/tmp/project",
                                "parentThreadId": "source",
                                "hiddenFromPeerList": true
                            }
                        }
                    }
                }
            }
        });
        let visible = extract_thread_records(&snapshot);
        let all = extract_thread_records_including_hidden(&snapshot);
        assert_eq!(visible.iter().map(|record| record.id.as_str()).collect::<Vec<_>>(), vec!["source"]);
        assert!(all.iter().any(|record| record.id == "reviewer"));
        assert_eq!(
            all.iter()
                .find(|record| record.id == "reviewer")
                .and_then(|record| record.requirement_review.as_ref())
                .and_then(|summary| summary.parent_thread_id.as_deref()),
            Some("source")
        );
    }

    #[test]
    fn requirement_review_summary_counts_latest_verdict_packet() {
        let agent = json!({
            "requirements": {
                "id": "reqs",
                "requirements": [
                    {"key": "nativeGuiIsSourceOfTruth", "statement": "Mirror native.", "severity": "blocker", "verificationMethod": "screenshotReview"},
                    {"key": "buildAndTypecheckPass", "statement": "Build passes.", "severity": "high", "verificationMethod": "commandOutput"},
                    {"key": "externalCredential", "statement": "Needs credential.", "severity": "high", "verificationMethod": "manualEvidence"}
                ]
            },
            "requirementReview": {
                "status": "failed",
                "reviewerThreadId": "reviewer",
                "requirementSetId": "reqs",
                "updatedAt": 123,
                "latestVerdictPacket": {
                    "nativeGuiIsSourceOfTruth": {"verdict": "pass", "reason": "Matched."},
                    "buildAndTypecheckPass": {"verdict": "fail", "reason": "No check output."},
                    "externalCredential": {"verdict": "acceptedBlocked", "reason": "Secret missing."}
                }
            }
        });
        let summary = requirement_review_summary(agent.as_object().unwrap()).unwrap();
        assert_eq!(summary.active_requirement_count, 3);
        assert_eq!(summary.passed_count, 1);
        assert_eq!(summary.failed_count, 1);
        assert_eq!(summary.blocked_count, 1);
        assert_eq!(summary.unknown_count, 0);
        assert_eq!(summary.reviewer_thread_id.as_deref(), Some("reviewer"));
    }
}

fn author_for_role(role: Option<&str>) -> String {
    match role {
        Some("assistant") => "Assistant",
        Some("user") => "User",
        Some("tool") => "Tool",
        Some("system") => "System",
        Some(other) => other,
        None => "Unknown",
    }
    .to_string()
}

fn display_label_for_message(
    role: Option<&str>,
    kind: Option<&str>,
    subtitle: Option<&str>,
    body: &str,
) -> String {
    match kind {
        Some("commandExecution") => "Command".to_string(),
        Some("mcpToolCall") => "MCP".to_string(),
        Some("fileChange") => {
            if body.eq_ignore_ascii_case("turn diff updated")
                || subtitle
                    .map(|value| value.to_ascii_lowercase().contains("git diff"))
                    .unwrap_or(false)
            {
                "Diff".to_string()
            } else {
                "File Change".to_string()
            }
        }
        Some(other) => other.to_string(),
        None => author_for_role(role),
    }
}

fn parse_timestamp(value: Option<&Value>) -> Option<u64> {
    match value {
        Some(Value::Number(number)) => number.as_u64(),
        Some(Value::String(text)) => text.parse::<u64>().ok(),
        _ => None,
    }
}

fn is_streaming(message: &serde_json::Map<String, Value>) -> bool {
    let tool_metadata = message.get("toolMetadata").and_then(Value::as_object);
    let kind = tool_metadata
        .and_then(|tool| tool.get("kind"))
        .and_then(Value::as_str);
    let subtitle = message.get("subtitle").and_then(Value::as_str).unwrap_or_default();
    let body = message.get("text").and_then(Value::as_str).unwrap_or_default();
    let is_turn_diff = kind == Some("fileChange")
        && (body.eq_ignore_ascii_case("Turn diff updated")
            || subtitle.to_ascii_lowercase().contains("git diff"));
    if is_turn_diff {
        return false;
    }
    matches!(
        tool_metadata
            .and_then(|tool| tool.get("status"))
            .and_then(Value::as_str),
        Some("inProgress" | "in_progress")
    )
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
