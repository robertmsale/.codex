use serde::Serialize;
use serde_json::{Value, json};

use crate::actions;
use crate::roles::{ManifestDecision, RoleSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeDecision {
    Allow,
    Deny,
    ApprovalRequired,
}

impl RuntimeDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::ApprovalRequired => "approvalRequired",
        }
    }

    pub fn can_execute(self) -> bool {
        self == Self::Allow
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyResult {
    pub action: String,
    pub decision: RuntimeDecision,
    pub reason: String,
    pub input: Value,
    pub role_id: String,
    pub role_version: String,
    pub source_decision: Option<String>,
}

impl PolicyResult {
    pub fn to_event_payload(&self) -> Value {
        json!({
            "action": self.action,
            "decision": self.decision.as_str(),
            "reason": self.reason,
            "input": self.input,
            "role": {"id": self.role_id, "version": self.role_version},
            "sourceDecision": self.source_decision,
        })
    }
}

pub struct PolicyEngine;

impl PolicyEngine {
    pub fn decide(snapshot: &RoleSnapshot, action: &str, input: Value) -> PolicyResult {
        if !actions::is_active_action(action) {
            return PolicyResult {
                action: action.to_string(),
                decision: RuntimeDecision::Deny,
                reason: "action is not active in the kernel catalog".to_string(),
                input,
                role_id: snapshot.id.clone(),
                role_version: snapshot.version.clone(),
                source_decision: None,
            };
        }

        let manifest_decision = snapshot.policy.get(action);
        let (decision, reason, source_decision) = match manifest_decision {
            Some(ManifestDecision::Allow) => (
                RuntimeDecision::Allow,
                "role policy allows action".to_string(),
                Some("allow".to_string()),
            ),
            Some(ManifestDecision::Deny) => (
                RuntimeDecision::Deny,
                "role policy denies action".to_string(),
                Some("deny".to_string()),
            ),
            Some(ManifestDecision::OwnerApproval) => (
                RuntimeDecision::ApprovalRequired,
                "role policy requires owner approval".to_string(),
                Some("ownerApproval".to_string()),
            ),
            Some(ManifestDecision::OrchestratorApproval) => (
                RuntimeDecision::ApprovalRequired,
                "role policy requires orchestrator approval".to_string(),
                Some("orchestratorApproval".to_string()),
            ),
            None => (
                RuntimeDecision::Deny,
                "default deny: action is absent from role policy".to_string(),
                None,
            ),
        };

        PolicyResult {
            action: action.to_string(),
            decision,
            reason,
            input,
            role_id: snapshot.id.clone(),
            role_version: snapshot.version.clone(),
            source_decision,
        }
    }
}
