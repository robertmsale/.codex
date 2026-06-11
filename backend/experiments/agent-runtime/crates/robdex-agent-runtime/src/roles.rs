use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

pub type RoleSnapshot = RoleManifest;

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

    pub fn list(&self) -> Result<Vec<RoleManifest>> {
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

    pub fn load(&self, id: &str) -> Result<RoleManifest> {
        let path = self.root.join(format!("{id}.json"));
        self.load_path(&path)
    }

    pub fn load_path(&self, path: &Path) -> Result<RoleManifest> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("role manifest is not readable: {}", path.display()))?;
        let manifest: RoleManifest = serde_json::from_str(&content)
            .with_context(|| format!("role manifest is not valid JSON: {}", path.display()))?;
        self.validate_manifest(&manifest)
            .with_context(|| format!("invalid role manifest: {}", path.display()))?;
        Ok(manifest)
    }

    pub fn validate_all(&self) -> Result<Vec<RoleManifest>> {
        let roles = self.list()?;
        if roles.is_empty() {
            bail!("no role manifests found in {}", self.root.display());
        }
        Ok(roles)
    }

    pub fn validate_manifest(&self, manifest: &RoleManifest) -> Result<()> {
        validate_role_id(&manifest.id)?;
        validate_non_empty("version", &manifest.version)?;
        validate_non_empty("displayName", &manifest.display_name)?;
        validate_non_empty("modelDefaults.model", &manifest.model_defaults.model)?;
        validate_non_empty("modelDefaults.reasoningEffort", &manifest.model_defaults.reasoning_effort)?;

        let prompt_path = self.root.join(&manifest.prompt.path);
        if !prompt_path.is_file() {
            bail!("prompt file path does not exist: {}", prompt_path.display());
        }

        for action in &manifest.capabilities {
            actions::validate_known_action(action)?;
        }
        for action in manifest.policy.keys() {
            actions::validate_known_action(action)?;
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

    pub fn snapshot(&self, id: &str) -> Result<RoleSnapshot> {
        self.load(id)
    }
}

pub fn snapshot_from_value(value: Value) -> Result<RoleSnapshot> {
    Ok(serde_json::from_value(value).context("stored role snapshot is invalid")?)
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
