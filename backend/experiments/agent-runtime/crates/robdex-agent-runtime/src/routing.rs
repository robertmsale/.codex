use std::collections::BTreeSet;

use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::db;
use crate::roles::{RoleManifest, RoleSnapshot, RoutingMetadata};

pub const OWNER_RECIPIENT: &str = "owner";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteDecision {
    pub mode: String,
    pub recipient: String,
    pub reason: String,
    pub role_id: String,
    pub role_version: String,
}

impl RouteDecision {
    pub fn to_event_payload(&self) -> Value {
        json!({
            "mode": self.mode,
            "recipient": self.recipient,
            "reason": self.reason,
            "role": {"id": self.role_id, "version": self.role_version},
        })
    }
}

pub async fn validate_manifest_against_db(pool: &PgPool, manifest: &RoleManifest) -> Result<()> {
    validate_routing(&manifest.routing, Some(pool), &BTreeSet::new()).await
}

pub async fn validate_snapshot_routing_against_db(pool: &PgPool, snapshot: &RoleSnapshot) -> Result<()> {
    validate_routing(&snapshot.routing, Some(pool), &BTreeSet::new()).await
}

pub async fn validate_routing(
    routing: &RoutingMetadata,
    pool: Option<&PgPool>,
    import_context_roles: &BTreeSet<String>,
) -> Result<()> {
    if routing.mode != "direct" {
        bail!("unsupported routing mode: {}", routing.mode);
    }
    let Some(default) = &routing.default_recipient else {
        bail!("routing.defaultRecipient is required for direct routing");
    };
    if !routing.allowed_recipients.iter().any(|recipient| recipient == default) {
        bail!("routing.defaultRecipient must be present in allowedRecipients: {default}");
    }
    for recipient in &routing.allowed_recipients {
        validate_recipient(recipient, pool, import_context_roles).await?;
    }
    validate_recipient(default, pool, import_context_roles).await?;
    Ok(())
}

pub async fn decide_route(pool: &PgPool, session_id: Uuid, turn_id: Option<Uuid>, role: &RoleSnapshot) -> Result<RouteDecision> {
    let recipient = role
        .routing
        .default_recipient
        .clone()
        .ok_or_else(|| anyhow::anyhow!("role snapshot missing routing.defaultRecipient"))?;
    validate_routing(&role.routing, Some(pool), &BTreeSet::new()).await?;
    let decision = RouteDecision {
        mode: role.routing.mode.clone(),
        recipient,
        reason: "direct routing selected default recipient".to_string(),
        role_id: role.id.clone(),
        role_version: role.version.clone(),
    };
    db::append_event(
        pool,
        session_id,
        turn_id,
        "route",
        None,
        "route.decision",
        Some("direct"),
        decision.to_event_payload(),
    )
    .await?;
    Ok(decision)
}

async fn validate_recipient(recipient: &str, pool: Option<&PgPool>, import_context_roles: &BTreeSet<String>) -> Result<()> {
    if recipient == OWNER_RECIPIENT || import_context_roles.contains(recipient) {
        return Ok(());
    }
    if let Some(pool) = pool {
        let row = sqlx::query("SELECT EXISTS (SELECT 1 FROM roles WHERE id = $1 AND status = 'active')")
            .bind(recipient)
            .fetch_one(pool)
            .await?;
        if row.get::<bool, _>(0) {
            return Ok(());
        }
    }
    bail!("invalid routing recipient: {recipient}")
}

pub fn recipient_options_from_active_role_ids<I>(active_role_ids: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut recipients = vec![OWNER_RECIPIENT.to_string()];
    for role_id in active_role_ids {
        if !recipients.iter().any(|recipient| recipient == &role_id) {
            recipients.push(role_id);
        }
    }
    recipients.sort();
    recipients
}

pub async fn recipient_options(pool: &PgPool) -> Result<Vec<String>> {
    let rows = sqlx::query("SELECT id FROM roles WHERE status = 'active' ORDER BY id ASC")
        .fetch_all(pool)
        .await?;
    Ok(recipient_options_from_active_role_ids(
        rows.into_iter().map(|row| row.get::<String, _>("id")),
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::validate_routing;
    use crate::roles::RoutingMetadata;

    #[tokio::test]
    async fn routing_validation_accepts_import_context_role_without_static_allowlist() {
        let routing = RoutingMetadata {
            mode: "direct".to_string(),
            default_recipient: Some("future-db-role".to_string()),
            allowed_recipients: vec!["future-db-role".to_string(), "owner".to_string()],
            reserved_actions: vec![],
        };
        let context = BTreeSet::from(["future-db-role".to_string()]);
        validate_routing(&routing, None, &context).await.unwrap();
    }

    #[tokio::test]
    async fn routing_validation_rejects_unknown_recipient() {
        let routing = RoutingMetadata {
            mode: "direct".to_string(),
            default_recipient: Some("missing-role".to_string()),
            allowed_recipients: vec!["missing-role".to_string()],
            reserved_actions: vec![],
        };
        let err = validate_routing(&routing, None, &BTreeSet::new()).await.unwrap_err().to_string();
        assert!(err.contains("invalid routing recipient: missing-role"));
    }

    #[tokio::test]
    async fn routing_validation_rejects_magic_operator_and_orchestrator_without_role_context() {
        for recipient in ["operator", "orchestrator", "runtime"] {
            let routing = RoutingMetadata {
                mode: "direct".to_string(),
                default_recipient: Some(recipient.to_string()),
                allowed_recipients: vec![recipient.to_string()],
                reserved_actions: vec![],
            };
            let err = validate_routing(&routing, None, &BTreeSet::new()).await.unwrap_err().to_string();
            assert!(err.contains(&format!("invalid routing recipient: {recipient}")));
        }
    }

    #[test]
    fn recipient_options_are_owner_plus_active_role_ids() {
        let options = super::recipient_options_from_active_role_ids([
            "runtime-allow".to_string(),
            "owner".to_string(),
            "runtime-allow".to_string(),
        ]);
        assert_eq!(options, vec!["owner".to_string(), "runtime-allow".to_string()]);
        assert!(!options.contains(&"operator".to_string()));
        assert!(!options.contains(&"orchestrator".to_string()));
        assert!(!options.contains(&"runtime".to_string()));
    }
}
