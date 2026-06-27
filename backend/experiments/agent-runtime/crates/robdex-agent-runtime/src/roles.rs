use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use robdex_agent_runtime_projection::{
    RoleEditorDraft, RoleEditorLifecycleAuthorityMetadata, RoleEditorModelDefaults,
    RoleEditorOptions, RoleEditorRoutingMetadata, RoleEditorValidationResult,
    RoleEditorVisibilityMetadata,
};

use crate::actions;

pub const DEFAULT_ROLE_ID: &str = "runtime-allow";

pub fn default_tool_bundle_for_role(role_id: &str) -> Vec<&'static str> {
    match role_id {
        "worker" | "runtime-allow" => vec![
            "file.head", "file.tail", "file.read_lines", "file.line_count", "file.search",
            "tree.list", "tree.find", "file.replace_exact", "fs.write", "patch.apply",
            "git.status", "git.diff", "git.restore", "git.add", "git.commit",
            "tooling.request", "outputs.head", "outputs.tail", "outputs.search",
        ],
        "designer" => vec![
            "file.head", "file.tail", "file.read_lines", "file.line_count", "file.search",
            "tree.list", "tree.find", "image.capture_from_file", "image.describe",
            "tooling.request",
        ],
        "qa" => vec![
            "file.head", "file.tail", "file.read_lines", "file.line_count", "file.search",
            "tree.list", "tree.find", "server.status", "server.logs",
            "image.capture_from_file", "image.describe", "tooling.request",
        ],
        "orchestrator" => vec![
            "agent.spawn.<role>", "requirements.set.other", "git.status", "git.diff",
            "git.inspect_worker_branch", "git.rebase_worker_branch", "git.fast_forward_local_main",
            "git.cleanup_integrated_worktree", "packet.routing.inspect", "tooling.request",
            "project_runtime.request_change",
        ],
        "project-progenitor" => vec![
            "project_runtime.request_change", "tooling.request", "workflow_memory.search",
            "file.head", "tree.list", "tree.find", "git.status", "git.diff",
        ],
        "simulator-steward" => vec![
            "simulator.inventory", "simulator.lease.assign", "simulator.lease.release",
            "simulator.boot.targeted", "simulator.shutdown.targeted", "image.capture_from_file",
        ],
        "operator-admin" => vec![
            "project_runtime.request_change", "command_registry.decide", "command_registry.apply",
            "server.start", "server.stop", "server.status", "server.logs", "tooling.request",
        ],
        _ => Vec::new(),
    }
}

fn tools_json_to_set(value: Value) -> BTreeSet<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

pub async fn visible_tool_bundle_for_role(pool: &PgPool, role_id: &str, project_key: Option<&str>) -> Result<Vec<String>> {
    let mut visible: BTreeSet<String> = default_tool_bundle_for_role(role_id)
        .into_iter()
        .map(ToString::to_string)
        .collect();
    let role_rows = sqlx::query(
        r#"
        SELECT tb.tools
        FROM starter_role_tool_bundle_bindings rb
        JOIN starter_tool_bundle_versions tb ON tb.id=rb.bundle_version_id
        WHERE rb.active=true AND tb.active=true AND rb.role_id=$1 AND rb.project_key IS NULL
        ORDER BY rb.created_at ASC
        "#,
    )
    .bind(role_id)
    .fetch_all(pool)
    .await?;
    if !role_rows.is_empty() {
        visible.clear();
        for row in role_rows {
            visible.extend(tools_json_to_set(row.get("tools")));
        }
    }
    if let Some(project_key) = project_key {
        let project_rows = sqlx::query(
            r#"
            SELECT tb.tools
            FROM starter_role_tool_bundle_bindings rb
            JOIN starter_tool_bundle_versions tb ON tb.id=rb.bundle_version_id
            WHERE rb.active=true AND tb.active=true AND rb.role_id=$1 AND rb.project_key=$2
            ORDER BY rb.created_at ASC
            "#,
        )
        .bind(role_id)
        .bind(project_key)
        .fetch_all(pool)
        .await?;
        if !project_rows.is_empty() {
            let mut project_visible = BTreeSet::new();
            for row in project_rows {
                project_visible.extend(tools_json_to_set(row.get("tools")));
            }
            visible = visible.intersection(&project_visible).cloned().collect();
        }
    }
    Ok(visible.into_iter().collect())
}

pub async fn activate_tool_bundle_binding(
    pool: &PgPool,
    audit_session_id: Option<Uuid>,
    role_id: &str,
    project_key: Option<&str>,
    bundle_id: &str,
    version: &str,
    tools: Vec<String>,
    source_metadata: Value,
    audit_metadata: Value,
) -> Result<(Uuid, Uuid)> {
    if bundle_id.trim().is_empty() || version.trim().is_empty() || role_id.trim().is_empty() {
        bail!("tool bundle activation requires non-empty bundle id, version, and role id");
    }
    if tools.is_empty() {
        bail!("tool bundle activation requires at least one tool");
    }
    for tool in &tools {
        if !actions::is_known_action(tool) && !tool.starts_with("agent.spawn.") && !tool.starts_with("simulator.") && !tool.starts_with("packet.") {
            bail!("tool bundle contains unknown tool: {tool}");
        }
    }
    let mut tx = pool.begin().await?;
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM starter_tool_bundle_versions WHERE bundle_id=$1 AND version=$2 AND COALESCE(project_key,'')=COALESCE($3,'') LIMIT 1",
    )
    .bind(bundle_id)
    .bind(version)
    .bind(project_key)
    .fetch_optional(&mut *tx)
    .await?;
    let bundle_version_id = if let Some(id) = existing {
        sqlx::query("UPDATE starter_tool_bundle_versions SET tools=$2, source_metadata=$3, active=true WHERE id=$1")
            .bind(id)
            .bind(json!(tools))
            .bind(&source_metadata)
            .execute(&mut *tx)
            .await?;
        id
    } else {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO starter_tool_bundle_versions (id, bundle_id, version, role_id, project_key, tools, source_metadata, active) VALUES ($1,$2,$3,$4,$5,$6,$7,true)")
            .bind(id)
            .bind(bundle_id)
            .bind(version)
            .bind(role_id)
            .bind(project_key)
            .bind(json!(tools))
            .bind(&source_metadata)
            .execute(&mut *tx)
            .await?;
        id
    };
    sqlx::query("UPDATE starter_role_tool_bundle_bindings SET active=false WHERE role_id=$1 AND COALESCE(project_key,'')=COALESCE($2,'') AND active=true")
        .bind(role_id)
        .bind(project_key)
        .execute(&mut *tx)
        .await?;
    let binding_id = Uuid::new_v4();
    sqlx::query("INSERT INTO starter_role_tool_bundle_bindings (id, role_id, project_key, bundle_version_id, active, audit_metadata) VALUES ($1,$2,$3,$4,true,$5)")
        .bind(binding_id)
        .bind(role_id)
        .bind(project_key)
        .bind(bundle_version_id)
        .bind(&audit_metadata)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO event_stream (session_id, entity_type, entity_id, event_type, status, payload) VALUES ($1,'tool_bundle_binding',$2,'tool_bundle.binding_activated','active',$3)")
        .bind(audit_session_id)
        .bind(binding_id)
        .bind(json!({
            "roleId": role_id,
            "projectKey": project_key,
            "bundleId": bundle_id,
            "version": version,
            "bundleVersionId": bundle_version_id,
            "sourceMetadata": source_metadata,
            "auditMetadata": audit_metadata,
        }))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((bundle_version_id, binding_id))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleManifest {
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub prompt: PromptSource,
    pub model_defaults: ModelDefaults,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub policy: BTreeMap<String, ManifestDecision>,
    pub routing: RoutingMetadata,
    pub visibility: VisibilityMetadata,
    pub lifecycle_authority: LifecycleAuthorityMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptSource {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDefaults {
    pub model: String,
    pub reasoning_effort: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ManifestDecision {
    Allow,
    Deny,
    OwnerApproval,
    OrchestratorApproval,
}

impl ManifestDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::OwnerApproval => "ownerApproval",
            Self::OrchestratorApproval => "orchestratorApproval",
        }
    }
}

impl TryFrom<&str> for ManifestDecision {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "allow" => Ok(Self::Allow),
            "deny" => Ok(Self::Deny),
            "ownerApproval" => Ok(Self::OwnerApproval),
            "orchestratorApproval" => Ok(Self::OrchestratorApproval),
            other => bail!("unsupported role policy decision: {other}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingMetadata {
    pub mode: String,
    pub default_recipient: Option<String>,
    #[serde(default)]
    pub allowed_recipients: Vec<String>,
    #[serde(default)]
    pub reserved_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibilityMetadata {
    pub listed: bool,
    pub owner_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleAuthorityMetadata {
    pub can_spawn_agents: bool,
    pub can_archive_agents: bool,
    #[serde(default)]
    pub reserved_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleSnapshot {
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub role_version_id: Uuid,
    pub instruction_text: String,
    pub model_defaults: ModelDefaults,
    pub capabilities: Vec<String>,
    pub policy: BTreeMap<String, ManifestDecision>,
    pub routing: RoutingMetadata,
    pub visibility: VisibilityMetadata,
    pub lifecycle_authority: LifecycleAuthorityMetadata,
    pub manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ImportedRoleVersion {
    pub snapshot: RoleSnapshot,
    pub manifest: RoleManifest,
    pub manifest_json: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleValidationPacket {
    pub valid: bool,
    pub role_id: Option<String>,
    pub version: Option<String>,
    pub prompt_byte_count: usize,
    pub model_defaults: Option<ModelDefaults>,
    pub policy_actions: Vec<String>,
    pub routing_recipients: Vec<String>,
    pub lifecycle_authority: Option<LifecycleAuthorityMetadata>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RoleRegistry {
    root: PathBuf,
}

impl RoleRegistry {
    pub fn default_for_workspace() -> Result<Self> {
        let root = std::env::var("ROBDEX_AGENT_RUNTIME_ROLE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roles"));
        Ok(Self { root: root.canonicalize().unwrap_or(root) })
    }

    pub fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn list_manifests(&self) -> Result<Vec<RoleManifest>> {
        let mut roles = Vec::new();
        for entry in std::fs::read_dir(&self.root)
            .with_context(|| format!("role directory is not readable: {}", self.root.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                roles.push(self.load_path(&path)?);
            }
        }
        roles.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(roles)
    }

    pub fn manifest_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(&self.root)
            .with_context(|| format!("role directory is not readable: {}", self.root.display()))?
        {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                paths.push(path);
            }
        }
        paths.sort();
        Ok(paths)
    }

    pub fn load(&self, id: &str) -> Result<RoleManifest> {
        let path = self.root.join(format!("{id}.json"));
        self.load_path(&path)
    }

    pub fn load_path(&self, path: &Path) -> Result<RoleManifest> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("role manifest is not readable: {}", path.display()))?;
        let manifest: RoleManifest = serde_json::from_str(&content)
            .with_context(|| format!("role manifest is not valid JSON: {}", path.display()))?;
        self.validate_manifest(&manifest, path.parent().unwrap_or(&self.root))
            .with_context(|| format!("invalid role manifest: {}", path.display()))?;
        Ok(manifest)
    }

    pub fn load_for_import(&self, path: &Path) -> Result<ImportedRoleVersion> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("role manifest is not readable: {}", path.display()))?;
        let raw_json: Value = serde_json::from_str(&content)
            .with_context(|| format!("role manifest is not valid JSON: {}", path.display()))?;
        let manifest_json = raw_json.get("manifest").cloned().unwrap_or_else(|| raw_json.clone());
        let manifest: RoleManifest = serde_json::from_value(manifest_json.clone())
            .with_context(|| format!("role manifest schema is invalid: {}", path.display()))?;
        let base = path.parent().unwrap_or(&self.root);
        let has_embedded_instruction = raw_json.get("instructionText").and_then(Value::as_str).is_some();
        self.validate_manifest_with_options(&manifest, base, false, !has_embedded_instruction)
            .with_context(|| format!("invalid role manifest: {}", path.display()))?;
        let instruction_text = if let Some(text) = raw_json.get("instructionText").and_then(Value::as_str) {
            if text.trim().is_empty() {
                bail!("prompt instruction body must not be empty in DB role export: {}", path.display());
            }
            text.to_string()
        } else {
            self.resolve_prompt(&manifest, base)?
        };
        let role_version_id = Uuid::new_v4();
        let created_at = Utc::now();
        let snapshot = RoleSnapshot {
            id: manifest.id.clone(),
            version: manifest.version.clone(),
            display_name: manifest.display_name.clone(),
            role_version_id,
            instruction_text,
            model_defaults: manifest.model_defaults.clone(),
            capabilities: manifest.capabilities.clone(),
            policy: manifest.policy.clone(),
            routing: manifest.routing.clone(),
            visibility: manifest.visibility.clone(),
            lifecycle_authority: manifest.lifecycle_authority.clone(),
            manifest: manifest_json.clone(),
            created_at,
        };
        Ok(ImportedRoleVersion { snapshot, manifest, manifest_json })
    }

    pub fn validation_packet_for_path(&self, path: &Path) -> RoleValidationPacket {
        let mut errors = Vec::new();
        let mut role_id = None;
        let mut version = None;
        let mut prompt_byte_count = 0usize;
        let mut model_defaults = None;
        let mut policy_actions = Vec::new();
        let mut routing_recipients = Vec::new();
        let mut lifecycle_authority = None;
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Value>(&content) {
                Ok(raw_json) => {
                    let manifest_json = raw_json.get("manifest").cloned().unwrap_or(raw_json.clone());
                    match serde_json::from_value::<RoleManifest>(manifest_json) {
                        Ok(manifest) => {
                            role_id = Some(manifest.id.clone());
                            version = Some(manifest.version.clone());
                            model_defaults = Some(manifest.model_defaults.clone());
                            policy_actions = manifest.policy.keys().cloned().collect();
                            routing_recipients = manifest.routing.allowed_recipients.clone();
                            if let Some(default) = &manifest.routing.default_recipient {
                                if !routing_recipients.contains(default) {
                                    routing_recipients.push(default.clone());
                                }
                            }
                            lifecycle_authority = Some(manifest.lifecycle_authority.clone());
                            match raw_json.get("instructionText").and_then(Value::as_str) {
                                Some(text) => prompt_byte_count = text.len(),
                                None => {
                                    let base = path.parent().unwrap_or(&self.root);
                                    if let Ok(text) = self.resolve_prompt(&manifest, base) {
                                        prompt_byte_count = text.len();
                                    }
                                }
                            }
                            if let Err(error) = self.load_for_import(path) {
                                errors.push(error_chain_message(&error));
                            }
                        }
                        Err(error) => errors.push(format!("role manifest schema is invalid: {error}")),
                    }
                }
                Err(error) => errors.push(format!("role manifest is not valid JSON: {error}")),
            },
            Err(error) => errors.push(format!("role manifest is not readable: {error}")),
        }
        RoleValidationPacket {
            valid: errors.is_empty(),
            role_id,
            version,
            prompt_byte_count,
            model_defaults,
            policy_actions,
            routing_recipients,
            lifecycle_authority,
            errors,
            warnings: Vec::new(),
        }
    }

    pub fn validate_all(&self) -> Result<Vec<RoleManifest>> {
        let roles = self.list_manifests()?;
        if roles.is_empty() {
            bail!("no role manifests found in {}", self.root.display());
        }
        Ok(roles)
    }

    pub fn validate_manifest(&self, manifest: &RoleManifest, base: &Path) -> Result<()> {
        self.validate_manifest_with_options(manifest, base, false, true)
    }

    fn validate_manifest_with_options(
        &self,
        manifest: &RoleManifest,
        base: &Path,
        allow_registry_command_actions: bool,
        prompt_required: bool,
    ) -> Result<()> {
        validate_role_id(&manifest.id)?;
        validate_non_empty("version", &manifest.version)?;
        validate_non_empty("displayName", &manifest.display_name)?;
        validate_non_empty("modelDefaults.model", &manifest.model_defaults.model)?;
        validate_non_empty("modelDefaults.reasoningEffort", &manifest.model_defaults.reasoning_effort)?;

        let prompt_path = base.join(&manifest.prompt.path);
        if prompt_required && !prompt_path.is_file() {
            bail!("prompt file path does not exist: {}", prompt_path.display());
        }
        if prompt_required {
            let instruction = self.resolve_prompt(manifest, base)?;
            if instruction.trim().is_empty() {
                bail!("prompt instruction body must not be empty: {}", prompt_path.display());
            }
        }

        for action in &manifest.capabilities {
            validate_manifest_action(action, allow_registry_command_actions)?;
        }
        for action in manifest.policy.keys() {
            validate_manifest_action(action, allow_registry_command_actions)?;
        }
        let capability_set: BTreeSet<_> = manifest.capabilities.iter().collect();
        let policy_set: BTreeSet<_> = manifest.policy.keys().collect();
        if capability_set != policy_set {
            bail!("capabilities must exactly match policy keys because policy is execution authority");
        }
        for action in &manifest.routing.reserved_actions {
            actions::validate_known_action(action)?;
        }
        for action in &manifest.lifecycle_authority.reserved_actions {
            actions::validate_known_action(action)?;
        }
        validate_non_empty("routing.mode", &manifest.routing.mode)?;
        if manifest.routing.mode != "direct" {
            bail!("unsupported routing mode: {}", manifest.routing.mode);
        }
        Ok(())
    }

    fn resolve_prompt(&self, manifest: &RoleManifest, base: &Path) -> Result<String> {
        let prompt_path = base.join(&manifest.prompt.path);
        let text = std::fs::read_to_string(&prompt_path)
            .with_context(|| format!("prompt file is not readable: {}", prompt_path.display()))?;
        if text.trim().is_empty() {
            bail!("prompt instruction body must not be empty: {}", prompt_path.display());
        }
        Ok(text)
    }
}

fn error_chain_message(error: &anyhow::Error) -> String {
    error.chain().map(ToString::to_string).collect::<Vec<_>>().join(": ")
}

fn validate_manifest_action(action: &str, allow_registry_command_actions: bool) -> Result<()> {
    if crate::command_registry::is_registry_command_action(action) {
        bail!("concrete command actions are not valid role policy entries: {action}");
    }
    let _ = allow_registry_command_actions;
    actions::validate_known_action(action)
}

pub fn snapshot_from_value(value: Value) -> Result<RoleSnapshot> {
    Ok(serde_json::from_value(value).context("stored role snapshot is invalid")?)
}

pub fn snapshot_to_value(snapshot: &RoleSnapshot) -> Result<Value> {
    Ok(serde_json::to_value(snapshot).context("role snapshot is not serializable")?)
}

pub fn db_display_row(snapshot: &RoleSnapshot) -> Value {
    json!({
        "id": snapshot.id,
        "version": snapshot.version,
        "displayName": snapshot.display_name,
        "roleVersionId": snapshot.role_version_id,
        "modelDefaults": snapshot.model_defaults,
        "instructionText": snapshot.instruction_text,
        "policy": snapshot.policy,
        "routing": snapshot.routing,
        "visibility": snapshot.visibility,
        "lifecycleAuthority": snapshot.lifecycle_authority,
        "createdAt": snapshot.created_at,
    })
}

pub fn editor_options(recipients: Vec<String>) -> RoleEditorOptions {
    RoleEditorOptions {
        policy_decisions: vec![
            "allow".to_string(),
            "deny".to_string(),
            "ownerApproval".to_string(),
            "orchestratorApproval".to_string(),
        ],
        routing_modes: vec!["direct".to_string()],
        default_recipients: recipients,
        known_actions: actions::ACTIVE_ACTIONS
            .iter()
            .chain(actions::RESERVED_ACTIONS.iter())
            .map(|action| (*action).to_string())
            .collect(),
    }
}

pub fn imported_role_from_editor_draft(draft: &RoleEditorDraft) -> Result<ImportedRoleVersion> {
    if draft.instruction_text.trim().is_empty() {
        bail!("role instructionText must not be empty");
    }
    let manifest = editor_draft_manifest(draft)?;
    let registry = RoleRegistry::from_root(PathBuf::new());
    registry.validate_manifest_with_options(&manifest, Path::new("."), false, false)?;
    let manifest_json = serde_json::to_value(&manifest).context("role editor manifest is not serializable")?;
    let role_version_id = Uuid::new_v4();
    let created_at = Utc::now();
    let snapshot = RoleSnapshot {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        display_name: manifest.display_name.clone(),
        role_version_id,
        instruction_text: draft.instruction_text.clone(),
        model_defaults: manifest.model_defaults.clone(),
        capabilities: manifest.capabilities.clone(),
        policy: manifest.policy.clone(),
        routing: manifest.routing.clone(),
        visibility: manifest.visibility.clone(),
        lifecycle_authority: manifest.lifecycle_authority.clone(),
        manifest: manifest_json.clone(),
        created_at,
    };
    Ok(ImportedRoleVersion { snapshot, manifest, manifest_json })
}

pub fn validation_result_for_editor_draft(draft: &RoleEditorDraft) -> RoleEditorValidationResult {
    let mut errors = Vec::new();
    if let Err(error) = imported_role_from_editor_draft(draft) {
        errors.push(error_chain_message(&error));
    }
    RoleEditorValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings: Vec::new(),
        role_id: Some(draft.id.clone()),
        version: Some(draft.version.clone()),
    }
}

fn editor_draft_manifest(draft: &RoleEditorDraft) -> Result<RoleManifest> {
    let mut policy = BTreeMap::new();
    for (action, decision) in &draft.policy {
        policy.insert(action.clone(), ManifestDecision::try_from(decision.as_str())?);
    }
    Ok(RoleManifest {
        id: draft.id.clone(),
        version: draft.version.clone(),
        display_name: draft.display_name.clone(),
        prompt: PromptSource { path: "inline://gui-role-editor".to_string() },
        model_defaults: model_defaults_from_editor(&draft.model_defaults),
        capabilities: draft.capabilities.clone(),
        policy,
        routing: routing_from_editor(&draft.routing),
        visibility: visibility_from_editor(&draft.visibility),
        lifecycle_authority: lifecycle_from_editor(&draft.lifecycle_authority),
    })
}

fn model_defaults_from_editor(value: &RoleEditorModelDefaults) -> ModelDefaults {
    ModelDefaults {
        model: value.model.clone(),
        reasoning_effort: value.reasoning_effort.clone(),
    }
}

fn routing_from_editor(value: &RoleEditorRoutingMetadata) -> RoutingMetadata {
    RoutingMetadata {
        mode: value.mode.clone(),
        default_recipient: value.default_recipient.clone(),
        allowed_recipients: value.allowed_recipients.clone(),
        reserved_actions: value.reserved_actions.clone(),
    }
}

fn visibility_from_editor(value: &RoleEditorVisibilityMetadata) -> VisibilityMetadata {
    VisibilityMetadata {
        listed: value.listed,
        owner_visible: value.owner_visible,
    }
}

fn lifecycle_from_editor(value: &RoleEditorLifecycleAuthorityMetadata) -> LifecycleAuthorityMetadata {
    LifecycleAuthorityMetadata {
        can_spawn_agents: value.can_spawn_agents,
        can_archive_agents: value.can_archive_agents,
        reserved_actions: value.reserved_actions.clone(),
    }
}

fn validate_role_id(id: &str) -> Result<()> {
    validate_non_empty("id", id)?;
    if id.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-') {
        Ok(())
    } else {
        bail!("role id must use lowercase letters, digits, and hyphens: {id}")
    }
}

fn validate_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("role manifest field must not be empty: {field}")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use super::{
        LifecycleAuthorityMetadata, ManifestDecision, ModelDefaults, PromptSource, RoleManifest,
        RoleRegistry, RoutingMetadata, VisibilityMetadata, default_tool_bundle_for_role,
    };
    use crate::policy::{PolicyEngine, RuntimeDecision};

    fn temp_role_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "robdex_agent_runtime_role_unit_{}_{}_{}",
            name,
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("prompts")).unwrap();
        fs::write(dir.join("prompts/role.md"), "role instructions\n").unwrap();
        dir
    }

    fn manifest() -> RoleManifest {
        RoleManifest {
            id: "unit-role".to_string(),
            version: "1.0.0".to_string(),
            display_name: "Unit Role".to_string(),
            prompt: PromptSource { path: "prompts/role.md".to_string() },
            model_defaults: ModelDefaults {
                model: "gpt-5.5".to_string(),
                reasoning_effort: "medium".to_string(),
            },
            capabilities: vec!["tool.execute_code".to_string()],
            policy: BTreeMap::from([("tool.execute_code".to_string(), ManifestDecision::Allow)]),
            routing: RoutingMetadata {
                mode: "direct".to_string(),
                default_recipient: Some("owner".to_string()),
                allowed_recipients: vec!["owner".to_string()],
                reserved_actions: vec!["message.send".to_string(), "message.route".to_string()],
            },
            visibility: VisibilityMetadata { listed: true, owner_visible: true },
            lifecycle_authority: LifecycleAuthorityMetadata {
                can_spawn_agents: false,
                can_archive_agents: false,
                reserved_actions: vec!["agent.spawn.<role>".to_string(), "agent.archive".to_string()],
            },
        }
    }

    #[test]
    fn manifest_validation_rejects_unknown_actions() {
        let dir = temp_role_dir("unknown_action");
        let registry = RoleRegistry::from_root(dir.clone());
        let mut manifest = manifest();
        manifest.capabilities.push("cmd.nope.run".to_string());
        manifest.policy.insert("cmd.nope.run".to_string(), ManifestDecision::Allow);
        let error = registry.validate_manifest(&manifest, &dir).unwrap_err().to_string();
        assert!(error.contains("concrete command actions are not valid role policy entries: cmd.nope.run"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn manifest_validation_rejects_capability_policy_mismatch() {
        let dir = temp_role_dir("capability_mismatch");
        let registry = RoleRegistry::from_root(dir.clone());
        let mut manifest = manifest();
        manifest.capabilities.push("fs.read".to_string());
        let error = registry.validate_manifest(&manifest, &dir).unwrap_err().to_string();
        assert!(error.contains("capabilities must exactly match policy keys"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn editor_options_expose_all_supported_policy_decisions() {
        let options = super::editor_options(vec!["owner".to_string()]);
        assert_eq!(options.policy_decisions, vec![
            "allow".to_string(),
            "deny".to_string(),
            "ownerApproval".to_string(),
            "orchestratorApproval".to_string(),
        ]);
        assert!(!options.policy_decisions.contains(&"off".to_string()));
    }

    #[test]
    fn starter_default_tool_bundles_expose_intended_role_tools_only() {
        let worker = default_tool_bundle_for_role("worker");
        assert!(worker.contains(&"file.head"));
        assert!(worker.contains(&"git.commit"));
        assert!(!worker.contains(&"simulator.shutdown.targeted"));
        for prohibited in ["git.branch", "git.checkout", "git.reset", "git.cherry_pick", "git.reflog", "git.merge", "git.remote", "git.push", "git.pull", "git.fetch"] {
            assert!(!worker.contains(&prohibited), "worker-like bundles must not expose arbitrary git affordance {prohibited}");
        }

        let designer = default_tool_bundle_for_role("designer");
        assert!(designer.contains(&"image.capture_from_file"));
        assert!(designer.contains(&"tooling.request"));
        assert!(!designer.contains(&"git.commit"));

        let qa = default_tool_bundle_for_role("qa");
        assert!(qa.contains(&"server.logs"));
        assert!(!qa.contains(&"fs.write"));

        let orchestrator = default_tool_bundle_for_role("orchestrator");
        assert!(orchestrator.contains(&"agent.spawn.<role>"));
        assert!(orchestrator.contains(&"project_runtime.request_change"));
        assert!(!orchestrator.contains(&"file.replace_exact"));

        let progenitor = default_tool_bundle_for_role("project-progenitor");
        assert!(progenitor.contains(&"project_runtime.request_change"));
        assert!(progenitor.contains(&"workflow_memory.search"));
        assert!(!progenitor.contains(&"command_registry.apply"));

        let steward = default_tool_bundle_for_role("simulator-steward");
        assert!(steward.contains(&"simulator.lease.assign"));
        assert!(!steward.contains(&"simulator.global.restart"));

        let admin = default_tool_bundle_for_role("operator-admin");
        assert!(admin.contains(&"command_registry.apply"));
        assert!(admin.contains(&"server.stop"));
    }

    #[test]
    fn project_progenitor_manifest_is_project_local_and_owner_routed() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../roles");
        let manifest_text = fs::read_to_string(root.join("project-progenitor.json")).expect("project progenitor manifest");
        let manifest: RoleManifest = serde_json::from_str(&manifest_text).expect("parse project progenitor manifest");
        let registry = RoleRegistry::from_root(root.clone());
        registry.validate_manifest(&manifest, &root).expect("valid project progenitor manifest");
        let imported = registry.load_for_import(&root.join("project-progenitor.json")).expect("import project progenitor manifest");
        let status_decision = PolicyEngine::decide(&imported.snapshot, "git.status", serde_json::json!({}));
        let diff_decision = PolicyEngine::decide(&imported.snapshot, "git.diff", serde_json::json!({"paths":[]}));
        let commit_decision = PolicyEngine::decide(&imported.snapshot, "git.commit", serde_json::json!({"message":"nope","paths":[]}));
        assert_eq!(status_decision.decision, RuntimeDecision::Allow);
        assert_eq!(diff_decision.decision, RuntimeDecision::Allow);
        assert_eq!(commit_decision.decision, RuntimeDecision::Deny);
        assert_eq!(manifest.id, "project-progenitor");
        assert!(manifest.visibility.owner_visible);
        assert!(manifest.capabilities.contains(&"project_runtime.request_change".to_string()));
        assert!(manifest.capabilities.contains(&"tooling.request".to_string()));
        assert!(manifest.capabilities.contains(&"workflow_memory.search".to_string()));
        assert!(manifest.capabilities.contains(&"git.status".to_string()));
        assert!(manifest.capabilities.contains(&"git.diff".to_string()));
        assert_eq!(manifest.policy.get("workflow_memory.search"), Some(&ManifestDecision::Allow));
        assert_eq!(manifest.policy.get("git.status"), Some(&ManifestDecision::Allow));
        assert_eq!(manifest.policy.get("git.diff"), Some(&ManifestDecision::Allow));
        assert!(!manifest.capabilities.contains(&"command_registry.apply".to_string()));
        for mutating_git in ["git.restore", "git.add", "git.commit", "git.reset", "git.merge", "git.cherry-pick", "git.pull", "git.fetch", "git.push"] {
            assert!(!manifest.capabilities.contains(&mutating_git.to_string()));
            assert!(!manifest.policy.contains_key(mutating_git));
            assert!(!default_tool_bundle_for_role("project-progenitor").contains(&mutating_git));
        }
        assert_eq!(manifest.routing.default_recipient.as_deref(), Some("owner"));
        assert_eq!(manifest.routing.allowed_recipients, vec!["owner".to_string()]);
        assert!(!manifest.lifecycle_authority.can_spawn_agents);
        assert!(!manifest.lifecycle_authority.can_archive_agents);
        let prompt = fs::read_to_string(root.join("prompts/project-progenitor.md")).expect("project progenitor prompt");
        assert!(prompt.contains("project-local"));
        assert!(prompt.contains("Do not edit global skills"));
        assert!(default_tool_bundle_for_role("project-progenitor").contains(&"project_runtime.request_change"));
        assert!(default_tool_bundle_for_role("project-progenitor").contains(&"workflow_memory.search"));
        assert!(default_tool_bundle_for_role("project-progenitor").contains(&"git.status"));
        assert!(default_tool_bundle_for_role("project-progenitor").contains(&"git.diff"));
        assert!(!default_tool_bundle_for_role("project-progenitor").contains(&"command_registry.apply"));
    }
}
