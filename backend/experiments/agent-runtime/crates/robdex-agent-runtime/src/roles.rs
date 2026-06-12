use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::actions;

pub const DEFAULT_ROLE_ID: &str = "runtime-allow";

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingMetadata {
    pub mode: String,
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
        let manifest_json: Value = serde_json::from_str(&content)
            .with_context(|| format!("role manifest is not valid JSON: {}", path.display()))?;
        let manifest: RoleManifest = serde_json::from_value(manifest_json.clone())
            .with_context(|| format!("role manifest schema is invalid: {}", path.display()))?;
        let base = path.parent().unwrap_or(&self.root);
        self.validate_manifest(&manifest, base)
            .with_context(|| format!("invalid role manifest: {}", path.display()))?;
        let instruction_text = self.resolve_prompt(&manifest, base)?;
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

    pub fn validate_all(&self) -> Result<Vec<RoleManifest>> {
        let roles = self.list_manifests()?;
        if roles.is_empty() {
            bail!("no role manifests found in {}", self.root.display());
        }
        Ok(roles)
    }

    pub fn validate_manifest(&self, manifest: &RoleManifest, base: &Path) -> Result<()> {
        validate_role_id(&manifest.id)?;
        validate_non_empty("version", &manifest.version)?;
        validate_non_empty("displayName", &manifest.display_name)?;
        validate_non_empty("modelDefaults.model", &manifest.model_defaults.model)?;
        validate_non_empty("modelDefaults.reasoningEffort", &manifest.model_defaults.reasoning_effort)?;

        let prompt_path = base.join(&manifest.prompt.path);
        if !prompt_path.is_file() {
            bail!("prompt file path does not exist: {}", prompt_path.display());
        }

        for action in &manifest.capabilities {
            actions::validate_known_action(action)?;
        }
        for action in manifest.policy.keys() {
            actions::validate_known_action(action)?;
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
