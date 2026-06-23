use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::db;
use crate::roles::{LifecycleAuthorityMetadata, ManifestDecision, RoleSnapshot, RoutingMetadata, VisibilityMetadata};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementSetInput {
    pub title: Option<String>,
    pub requirements: Vec<RequirementInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementInput {
    pub key: String,
    pub statement: String,
    #[serde(default = "default_severity")]
    pub severity: String,
    #[serde(default)]
    pub verification_method: Value,
}

fn default_severity() -> String {
    "must".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementStatus {
    pub active_set_id: Option<Uuid>,
    pub active: bool,
    pub enforce_on_turns: bool,
    pub total: usize,
    pub unresolved: usize,
    pub passed: usize,
    pub blocked: usize,
    pub waived: usize,
    pub reviewer_session_id: Option<Uuid>,
    pub review_status: Option<String>,
    pub latest_claim_packet_id: Option<Uuid>,
    pub latest_verdict_packet_id: Option<Uuid>,
    #[serde(default)]
    pub progress: Vec<Value>,
    #[serde(default)]
    pub owner_action: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct ActiveRequirementSet {
    pub id: Uuid,
    pub source_session_id: Uuid,
    pub canonical_set: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceClaimRecord {
    pub outcome: SourcePacketOutcome,
    pub requirement_set_id: Uuid,
    pub packet_id: Uuid,
    pub reviewer_session_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SourcePacketOutcome {
    Null,
    Invalid,
    Continuation,
    AllNotSatisfied,
    Reviewable,
}

pub fn reviewer_role_snapshot(source: &RoleSnapshot) -> RoleSnapshot {
    let mut policy = BTreeMap::new();
    policy.insert("fs.read".to_string(), ManifestDecision::Allow);
    policy.insert("workflow_memory.search".to_string(), ManifestDecision::Allow);
    RoleSnapshot {
        id: "requirements-reviewer".to_string(),
        version: "1.0.0".to_string(),
        display_name: "Requirements Reviewer".to_string(),
        role_version_id: Uuid::new_v4(),
        instruction_text: "You are a requirements-reviewer. Review the source session's completion claims adversarially. Do not implement. Inspect only the bounded evidence needed to determine whether each canonical requirement passes, fails, remains blocked, needs waiver, or is still passing. Do not mutate files, apply patches, administer the command registry, or grant implementation authority. Return only the required verdict packet.".to_string(),
        model_defaults: source.model_defaults.clone(),
        capabilities: vec!["fs.read".to_string(), "workflow_memory.search".to_string()],
        policy,
        routing: RoutingMetadata { mode: "direct".to_string(), default_recipient: Some("owner".to_string()), allowed_recipients: vec!["owner".to_string()], reserved_actions: vec![] },
        visibility: VisibilityMetadata { listed: false, owner_visible: false },
        lifecycle_authority: LifecycleAuthorityMetadata { can_spawn_agents: false, can_archive_agents: false, reserved_actions: vec![] },
        manifest: json!({"seededBy":"agent-runtime","role":"requirements-reviewer"}),
        created_at: Utc::now(),
    }
}

pub fn validate_requirement_set(input: &RequirementSetInput) -> Result<Value> {
    if input.requirements.is_empty() {
        bail!("RequirementSet must contain at least one requirement");
    }
    let mut seen = BTreeSet::new();
    let mut canonical = Vec::new();
    for (index, item) in input.requirements.iter().enumerate() {
        let key = item.key.trim();
        if key.is_empty() {
            bail!("requirement key must not be blank");
        }
        if key.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
            bail!("requirement key must be semantic, not numbered: {key}");
        }
        if !key.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-') || !key.chars().any(|ch| ch.is_ascii_alphabetic()) {
            bail!("requirement key must be semantic lowercase kebab/snake/camel-free token: {key}");
        }
        if !seen.insert(key.to_string()) {
            bail!("duplicate requirement key: {key}");
        }
        let statement = item.statement.trim();
        if statement.is_empty() {
            bail!("requirement statement must not be blank: {key}");
        }
        if !matches!(item.severity.as_str(), "must" | "should" | "blocker" | "critical") {
            bail!("unsupported requirement severity for {key}: {}", item.severity);
        }
        if !(item.verification_method.is_null() || item.verification_method.is_string() || item.verification_method.is_object()) {
            bail!("unsupported verification-method shape for {key}");
        }
        if let Some(method) = item.verification_method.as_object().and_then(|object| object.get("method")) {
            if !method.is_string() || method.as_str().unwrap_or_default().trim().is_empty() {
                bail!("unsupported verification-method method value for {key}");
            }
        }
        canonical.push(json!({
            "key": key,
            "statement": statement,
            "severity": item.severity,
            "verificationMethod": item.verification_method,
            "sortOrder": index as i64,
        }));
    }
    Ok(json!({
        "title": input.title,
        "requirements": canonical,
        "schemaVersion": 1,
    }))
}

pub fn requirement_set_from_lines(lines: &str) -> Result<RequirementSetInput> {
    let requirements = lines
        .lines()
        .filter_map(|line| {
            let text = line.trim().trim_start_matches("- ").trim();
            if text.is_empty() { None } else { Some(text.to_string()) }
        })
        .enumerate()
        .map(|(idx, statement)| RequirementInput {
            key: semantic_key(&statement, idx),
            statement,
            severity: "must".to_string(),
            verification_method: json!({"method":"review"}),
        })
        .collect::<Vec<_>>();
    Ok(RequirementSetInput { title: Some("Generated Requirements".to_string()), requirements })
}

pub fn load_requirement_set_file(path: &Path) -> Result<RequirementSetInput> {
    let raw = std::fs::read_to_string(path)?;
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or_default() {
        "yaml" | "yml" => Ok(serde_yaml::from_str(&raw)?),
        _ => Ok(serde_json::from_str(&raw)?),
    }
}

pub fn compose_requirement_sets(
    permanent: &[RequirementSetInput],
    included: &[RequirementSetInput],
    task: RequirementSetInput,
) -> Result<RequirementSetInput> {
    let mut requirements = Vec::new();
    let mut seen = BTreeSet::new();
    for input in permanent.iter().chain(included.iter()).chain(std::iter::once(&task)) {
        for requirement in &input.requirements {
            if !seen.insert(requirement.key.trim().to_string()) {
                bail!("conflicting duplicate composable requirement key: {}", requirement.key);
            }
            requirements.push(requirement.clone());
        }
    }
    let title = task.title.or_else(|| Some("Composed Requirements".to_string()));
    let composed = RequirementSetInput { title, requirements };
    validate_requirement_set(&composed)?;
    Ok(composed)
}

fn semantic_key(statement: &str, idx: usize) -> String {
    let words = statement
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty() && !part.chars().all(|ch| ch.is_ascii_digit()))
        .take(8)
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if words.is_empty() { format!("requirement_{idx}") } else { words.join("_") }
}

pub async fn set_active_requirements(pool: &PgPool, source_session_id: Uuid, input: RequirementSetInput) -> Result<Uuid> {
    let canonical = validate_requirement_set(&input)?;
    sqlx::query("UPDATE requirement_sets SET status='inactive', enforce_on_turns=false, deactivated_at=now(), updated_at=now(), outcome='replaced' WHERE source_session_id=$1 AND status='active'")
        .bind(source_session_id)
        .execute(pool)
        .await?;
    let set_id = Uuid::new_v4();
    sqlx::query("INSERT INTO requirement_sets (id, source_session_id, title, canonical_set, status, enforce_on_turns) VALUES ($1,$2,$3,$4,'active',true)")
        .bind(set_id)
        .bind(source_session_id)
        .bind(input.title.as_deref())
        .bind(&canonical)
        .execute(pool)
        .await?;
    for item in canonical["requirements"].as_array().cloned().unwrap_or_default() {
        let id = Uuid::new_v4();
        let key = item["key"].as_str().unwrap_or_default();
        sqlx::query("INSERT INTO requirement_items (id, requirement_set_id, requirement_key, statement, severity, verification_method, sort_order) VALUES ($1,$2,$3,$4,$5,$6,$7)")
            .bind(id)
            .bind(set_id)
            .bind(key)
            .bind(item["statement"].as_str().unwrap_or_default())
            .bind(item["severity"].as_str().unwrap_or("must"))
            .bind(item.get("verificationMethod").cloned().unwrap_or(Value::Null))
            .bind(item["sortOrder"].as_i64().unwrap_or(0) as i32)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO requirement_progress (requirement_set_id, requirement_key, status) VALUES ($1,$2,'unresolved')")
            .bind(set_id)
            .bind(key)
            .execute(pool)
            .await?;
    }
    db::append_event(pool, source_session_id, None, "requirements", Some(set_id), "requirements.set", Some("active"), json!({"requirementSetId": set_id, "count": input.requirements.len()})).await?;
    Ok(set_id)
}

pub async fn active_requirement_set(pool: &PgPool, source_session_id: Uuid) -> Result<Option<ActiveRequirementSet>> {
    let row = sqlx::query("SELECT id, canonical_set FROM requirement_sets WHERE source_session_id=$1 AND status='active' AND enforce_on_turns=true ORDER BY created_at DESC LIMIT 1")
        .bind(source_session_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| ActiveRequirementSet {
        id: row.get("id"),
        source_session_id,
        canonical_set: row.get("canonical_set"),
    }))
}

pub async fn source_output_schema(pool: &PgPool, source_session_id: Uuid) -> Result<Option<Value>> {
    let Some(active) = active_requirement_set(pool, source_session_id).await? else {
        return Ok(None);
    };
    Ok(Some(source_schema_from_active(pool, &active).await?))
}

pub async fn reviewer_output_schema(pool: &PgPool, reviewer_session_id: Uuid) -> Result<Option<Value>> {
    let row = sqlx::query(
        "SELECT requirement_set_id FROM requirement_review_bindings WHERE reviewer_session_id=$1 ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(reviewer_session_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else { return Ok(None); };
    let set_id: Uuid = row.get("requirement_set_id");
    let active = active_by_id(pool, set_id).await?;
    Ok(Some(reviewer_schema_from_active(pool, &active).await?))
}

async fn active_by_id(pool: &PgPool, set_id: Uuid) -> Result<ActiveRequirementSet> {
    let row = sqlx::query("SELECT source_session_id, canonical_set FROM requirement_sets WHERE id=$1")
        .bind(set_id)
        .fetch_one(pool)
        .await?;
    Ok(ActiveRequirementSet { id: set_id, source_session_id: row.get("source_session_id"), canonical_set: row.get("canonical_set") })
}

async fn source_schema_from_active(pool: &PgPool, active: &ActiveRequirementSet) -> Result<Value> {
    let rows = sqlx::query("SELECT requirement_key, status FROM requirement_progress WHERE requirement_set_id=$1 ORDER BY requirement_key ASC")
        .bind(active.id)
        .fetch_all(pool)
        .await?;
    let resolved = rows.into_iter().filter_map(|row| {
        let status: String = row.get("status");
        if matches!(status.as_str(), "passed" | "blocked" | "waived") {
            Some(row.get::<String, _>("requirement_key"))
        } else {
            None
        }
    }).collect::<BTreeSet<_>>();
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for item in active.canonical_set["requirements"].as_array().into_iter().flatten() {
        let Some(key) = item["key"].as_str() else { continue; };
        if resolved.contains(key) {
            continue;
        }
        required.push(Value::String(key.to_string()));
        properties.insert(key.to_string(), json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "claim": {"type":"string","enum":["satisfied","notSatisfied","blocked","notApplicable"]},
                "justification": {"type":"string"},
                "evidence": {"type":"array","items":{"type":"string"}},
                "risk": {"type":"string","enum":["none","low","medium","high","unknown"]}
            },
            "required": ["claim","justification","evidence","risk"]
        }));
    }
    Ok(json!({
        "name": "requirements_source_claim",
        "schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "summary": {"type":"string"},
                "requirements": {
                    "anyOf": [
                        {"type":"null"},
                        {"type":"object", "additionalProperties": false, "properties": properties, "required": required}
                    ]
                }
            },
            "required": ["summary","requirements"]
        },
        "metadata": {"kind":"sourceClaim","requirementSetId": active.id, "canonicalCount": active.canonical_set["requirements"].as_array().map(Vec::len).unwrap_or_default(), "unresolvedCount": required.len()}
    }))
}

async fn reviewer_schema_from_active(pool: &PgPool, active: &ActiveRequirementSet) -> Result<Value> {
    let passed = sqlx::query("SELECT requirement_key FROM requirement_progress WHERE requirement_set_id=$1 AND status='passed'")
        .bind(active.id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| row.get::<String, _>("requirement_key"))
        .collect::<BTreeSet<_>>();
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for item in active.canonical_set["requirements"].as_array().into_iter().flatten() {
        let Some(key) = item["key"].as_str() else { continue; };
        required.push(Value::String(key.to_string()));
        let full = json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "verdict": {"type":"string","enum":["pass","fail","acceptedBlocked","rejectedBlocked","waiverRequired","waiverAccepted"]},
                "evidence": {"type":"array","items":{"type":"string"}},
                "justification": {"type":"string"},
                "risk": {"type":"string","enum":["none","low","medium","high","unknown"]}
            },
            "required": ["verdict","evidence","justification","risk"]
        });
        let schema = if passed.contains(key) {
            json!({"anyOf": [full, {"type":"object","additionalProperties": false, "properties": {"verdict": {"const":"stillPassing"}}, "required":["verdict"]}]})
        } else {
            full
        };
        properties.insert(key.to_string(), schema);
    }
    Ok(json!({
        "name": "requirements_reviewer_verdict",
        "schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "summary": {"type":"string"},
                "requirements": {"type":"object", "additionalProperties": false, "properties": properties, "required": required},
                "overallVerdict": {"type":"string","enum":["pass","fail","acceptedBlocked","rejectedBlocked","needsHumanWaiver","waiverAccepted"]},
                "route": {"type":"string","enum":["source","owner","orchestrator"]}
            },
            "required": ["summary","requirements","overallVerdict","route"]
        },
        "metadata": {"kind":"reviewerVerdict","requirementSetId": active.id, "canonicalCount": active.canonical_set["requirements"].as_array().map(Vec::len).unwrap_or_default(), "unresolvedCount": required.len()}
    }))
}

pub async fn requirements_runtime_message(pool: &PgPool, session_id: Uuid) -> Result<Option<crate::model::RuntimeInputMessage>> {
    let session = db::session_record(pool, session_id).await?;
    let schema = if session.session_kind == "requirementsReviewer" {
        reviewer_output_schema(pool, session_id).await?
    } else {
        source_output_schema(pool, session_id).await?
    };
    let context = if session.session_kind == "requirementsReviewer" {
        reviewer_reconstruction_context(pool, session_id).await?
    } else {
        Value::Null
    };
    Ok(schema.map(|schema| crate::model::RuntimeInputMessage {
        text: if context.is_null() {
            format!("<requirements_schema>{}</requirements_schema>", schema)
        } else {
            format!(
                "<requirements_schema>{}</requirements_schema>\n<requirements_review_context>{}</requirements_review_context>",
                schema,
                context
            )
        },
        metadata: json!({
            "source": "requirements_output_schema",
            "schemaKind": schema.pointer("/metadata/kind").cloned().unwrap_or(Value::Null),
            "requirementSetId": schema.pointer("/metadata/requirementSetId").cloned().unwrap_or(Value::Null),
            "canonicalRequirementCount": schema.pointer("/metadata/canonicalCount").cloned().unwrap_or(Value::Null),
            "unresolvedRequirementCount": schema.pointer("/metadata/unresolvedCount").cloned().unwrap_or(Value::Null),
            "mode": if session.session_kind == "requirementsReviewer" {"reviewer"} else {"source"},
            "hasReviewContext": !context.is_null(),
        }),
    }))
}

async fn reviewer_reconstruction_context(pool: &PgPool, reviewer_session_id: Uuid) -> Result<Value> {
    let Some(row) = sqlx::query(
        "SELECT requirement_set_id, source_session_id, latest_claim_packet_id, latest_verdict_packet_id FROM requirement_review_bindings WHERE reviewer_session_id=$1 ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(reviewer_session_id)
    .fetch_optional(pool)
    .await? else {
        return Ok(Value::Null);
    };
    let set_id: Uuid = row.get("requirement_set_id");
    let source_session_id: Uuid = row.get("source_session_id");
    let active = active_by_id(pool, set_id).await?;
    let latest_claim_packet_id: Option<Uuid> = row.get("latest_claim_packet_id");
    let latest_claim = if let Some(packet_id) = latest_claim_packet_id {
        sqlx::query("SELECT id, packet_kind, status, payload, validation_error, created_at FROM requirement_packets WHERE id=$1")
            .bind(packet_id)
            .fetch_optional(pool)
            .await?
            .map(|row| json!({
                "id": row.get::<Uuid, _>("id"),
                "packetKind": row.get::<String, _>("packet_kind"),
                "status": row.get::<String, _>("status"),
                "payload": row.get::<Value, _>("payload"),
                "validationError": row.get::<Option<String>, _>("validation_error"),
                "createdAt": row.get::<chrono::DateTime<Utc>, _>("created_at"),
            }))
    } else {
        None
    };
    Ok(json!({
        "requirementSetId": set_id,
        "sourceSessionId": source_session_id,
        "canonicalSet": active.canonical_set,
        "progress": progress_rows(pool, set_id).await?,
        "latestClaimPacket": latest_claim,
        "latestVerdictPacketId": row.get::<Option<Uuid>, _>("latest_verdict_packet_id"),
    }))
}

pub async fn deactivate(pool: &PgPool, source_session_id: Uuid, outcome: &str) -> Result<()> {
    sqlx::query("UPDATE requirement_sets SET status='inactive', enforce_on_turns=false, deactivated_at=now(), outcome=$2, updated_at=now() WHERE source_session_id=$1 AND status='active'")
        .bind(source_session_id)
        .bind(outcome)
        .execute(pool)
        .await?;
    db::append_event(pool, source_session_id, None, "requirements", None, "requirements.deactivated", Some(outcome), json!({"outcome": outcome})).await?;
    Ok(())
}

pub async fn ensure_reviewer_session(pool: &PgPool, active: &ActiveRequirementSet) -> Result<Uuid> {
    if let Some(id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT reviewer_session_id FROM requirement_review_bindings WHERE requirement_set_id=$1 AND source_session_id=$2 AND status IN ('ready','inReview') AND reviewer_session_id IS NOT NULL LIMIT 1",
    )
    .bind(active.id)
    .bind(active.source_session_id)
    .fetch_optional(pool)
    .await? {
        return Ok(id);
    }
    let source = db::session_record(pool, active.source_session_id).await?;
    let source_role = db::session_role_snapshot(pool, active.source_session_id).await?;
    let reviewer_role = reviewer_role_snapshot(&source_role);
    let reviewer_id = db::new_session(
        pool,
        &reviewer_role,
        source.project_key.as_deref(),
        &source.workdir,
        source.worktree_root.as_deref(),
        Some("Requirements reviewer"),
        Some("requirements-reviewer"),
    )
    .await?;
    sqlx::query("UPDATE sessions SET parent_session_id=$2, session_kind='requirementsReviewer', hidden=true, tracked=false, metadata = metadata || $3 WHERE id=$1")
        .bind(reviewer_id)
        .bind(active.source_session_id)
        .bind(json!({"requirementsReviewer": true, "sourceSessionId": active.source_session_id, "requirementSetId": active.id}))
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO requirement_review_bindings (id, requirement_set_id, source_session_id, reviewer_session_id, status) VALUES ($1,$2,$3,$4,'ready') ON CONFLICT (requirement_set_id, source_session_id) DO UPDATE SET reviewer_session_id=$4, status='ready', updated_at=now()")
        .bind(Uuid::new_v4())
        .bind(active.id)
        .bind(active.source_session_id)
        .bind(reviewer_id)
        .execute(pool)
        .await?;
    db::append_event(pool, active.source_session_id, None, "requirements", Some(active.id), "requirements.reviewerReady", Some("ready"), json!({"reviewerSessionId": reviewer_id})).await?;
    Ok(reviewer_id)
}

pub async fn record_source_final_response(pool: &PgPool, source_session_id: Uuid, turn_id: Uuid, final_text: &str) -> Result<Option<SourceClaimRecord>> {
    let Some(active) = active_requirement_set(pool, source_session_id).await? else {
        return Ok(None);
    };
    let parsed = serde_json::from_str::<Value>(final_text);
    let (kind, status, payload, validation_error, outcome) = match parsed {
        Err(error) => ("claimInvalid", "invalid", json!({"raw": final_text}), Some(error.to_string()), SourcePacketOutcome::Invalid),
        Ok(value) if value.get("requirements").is_some_and(Value::is_null) => ("claimNull", "commentary", value, None, SourcePacketOutcome::Null),
        Ok(value) => match value.get("requirements").and_then(Value::as_object) {
            None => ("claimInvalid", "invalid", value, Some("requirements object missing".to_string()), SourcePacketOutcome::Invalid),
            Some(reqs) => {
                if let Err(error) = validate_source_claim_payload(pool, &active, &value).await {
                    ("claimInvalid", "invalid", value, Some(error.to_string()), SourcePacketOutcome::Invalid)
                } else if !reqs.values().any(|claim| matches!(claim.get("claim").and_then(Value::as_str), Some("satisfied" | "blocked" | "notApplicable"))) {
                    ("claimContinuation", "notReviewable", value, None, SourcePacketOutcome::AllNotSatisfied)
                } else {
                    ("claim", "reviewable", value, None, SourcePacketOutcome::Reviewable)
                }
            }
        }
    };
    let packet_id = Uuid::new_v4();
    let mut reviewer_session_id = None;
    sqlx::query("INSERT INTO requirement_packets (id, requirement_set_id, source_session_id, turn_id, packet_kind, status, payload, validation_error) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(packet_id)
        .bind(active.id)
        .bind(source_session_id)
        .bind(turn_id)
        .bind(kind)
        .bind(status)
        .bind(&payload)
        .bind(validation_error)
        .execute(pool)
        .await?;
    if outcome == SourcePacketOutcome::Reviewable {
        let reviewer_id = ensure_reviewer_session(pool, &active).await?;
        reviewer_session_id = Some(reviewer_id);
        sqlx::query("UPDATE requirement_review_bindings SET reviewer_session_id=$3, status='inReview', latest_claim_packet_id=$4, updated_at=now() WHERE requirement_set_id=$1 AND source_session_id=$2")
            .bind(active.id)
            .bind(source_session_id)
            .bind(reviewer_id)
            .bind(packet_id)
            .execute(pool)
            .await?;
    }
    match outcome {
        SourcePacketOutcome::Null => {
            db::append_event(pool, source_session_id, Some(turn_id), "requirements", Some(packet_id), "requirements.sourceCorrection", Some("requirementsNull"), json!({"message":"Requirements are active. Return a final Requirements claim packet when ready for review; requirements:null is commentary only."})).await?;
        }
        SourcePacketOutcome::Invalid => {
            db::append_event(pool, source_session_id, Some(turn_id), "requirements", Some(packet_id), "requirements.sourceCorrection", Some("requirementsInvalid"), json!({"message":"Requirements are active. Return valid structured Requirements JSON matching the active schema."})).await?;
        }
        SourcePacketOutcome::Continuation | SourcePacketOutcome::AllNotSatisfied => {
            db::append_event(pool, source_session_id, Some(turn_id), "requirements", Some(packet_id), "requirements.sourceCorrection", Some("requirementsNotReviewable"), json!({"message":"No review was started because every requirement is still not satisfied. Continue work or claim satisfied, blocked, or not applicable with evidence."})).await?;
        }
        SourcePacketOutcome::Reviewable => {}
    }
    db::append_event(pool, source_session_id, Some(turn_id), "requirements", Some(packet_id), kind, Some(status), json!({"packetId": packet_id, "requirementSetId": active.id})).await?;
    Ok(Some(SourceClaimRecord { outcome, requirement_set_id: active.id, packet_id, reviewer_session_id }))
}

pub async fn record_reviewer_verdict(pool: &PgPool, reviewer_session_id: Uuid, turn_id: Uuid, final_text: &str) -> Result<bool> {
    let row = sqlx::query(
        "SELECT requirement_set_id, source_session_id FROM requirement_review_bindings WHERE reviewer_session_id=$1 ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(reviewer_session_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else { return Ok(false); };
    let set_id: Uuid = row.get("requirement_set_id");
    let source_session_id: Uuid = row.get("source_session_id");
    let parsed = serde_json::from_str::<Value>(final_text);
    let (payload, kind, status, validation_error) = match parsed {
        Ok(value) if value.get("requirements").is_some_and(Value::is_null) => (value, "verdictNull", "commentary", None),
        Ok(value) => match validate_reviewer_verdict_payload(pool, set_id, &value).await {
            Ok(()) => (value, "verdict", "completed", None),
            Err(error) => (value, "verdictInvalid", "failed", Some(error.to_string())),
        },
        Err(error) => (json!({"raw": final_text}), "verdictInvalid", "failed", Some(error.to_string())),
    };
    let packet_id = Uuid::new_v4();
    sqlx::query("INSERT INTO requirement_packets (id, requirement_set_id, source_session_id, reviewer_session_id, turn_id, packet_kind, status, payload, validation_error) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(packet_id)
        .bind(set_id)
        .bind(source_session_id)
        .bind(reviewer_session_id)
        .bind(turn_id)
        .bind(kind)
        .bind(status)
        .bind(&payload)
        .bind(validation_error)
        .execute(pool)
        .await?;
    if kind == "verdict" {
        apply_verdict_packet(pool, set_id, source_session_id, &payload).await?;
        sqlx::query("UPDATE requirement_review_bindings SET latest_verdict_packet_id=$3, status='reviewed', updated_at=now() WHERE requirement_set_id=$1 AND source_session_id=$2")
            .bind(set_id)
            .bind(source_session_id)
            .bind(packet_id)
            .execute(pool)
            .await?;
    }
    db::append_event(pool, source_session_id, Some(turn_id), "requirements", Some(packet_id), kind, Some(status), json!({"packetId": packet_id, "reviewerSessionId": reviewer_session_id})).await?;
    Ok(true)
}

async fn validate_source_claim_payload(pool: &PgPool, active: &ActiveRequirementSet, payload: &Value) -> Result<()> {
    let packet = payload.as_object().ok_or_else(|| anyhow::anyhow!("source packet must be an object"))?;
    let top_allowed = BTreeSet::from(["summary", "requirements"]);
    if packet.keys().any(|field| !top_allowed.contains(field.as_str())) {
        bail!("source packet contains unsupported fields");
    }
    if !payload.get("summary").is_some_and(Value::is_string) {
        bail!("summary must be a string");
    }
    let reqs = payload.get("requirements").and_then(Value::as_object).ok_or_else(|| anyhow::anyhow!("requirements must be an object or null"))?;
    let required = unresolved_requirement_keys(pool, active.id).await?;
    let actual = reqs.keys().cloned().collect::<BTreeSet<_>>();
    if actual != required {
        bail!("requirements keys must exactly match unresolved active requirements");
    }
    for (key, claim) in reqs {
        let object = claim.as_object().ok_or_else(|| anyhow::anyhow!("claim for {key} must be an object"))?;
        let allowed = BTreeSet::from(["claim", "justification", "evidence", "risk"]);
        if object.keys().any(|field| !allowed.contains(field.as_str())) {
            bail!("claim for {key} contains unsupported fields");
        }
        if !matches!(object.get("claim").and_then(Value::as_str), Some("satisfied" | "notSatisfied" | "blocked" | "notApplicable")) {
            bail!("claim for {key} has unsupported claim value");
        }
        if !object.get("justification").is_some_and(Value::is_string) {
            bail!("claim for {key} justification must be a string");
        }
        if !object.get("evidence").is_some_and(|value| value.as_array().is_some_and(|items| items.iter().all(Value::is_string))) {
            bail!("claim for {key} evidence must be an array of strings");
        }
        if !matches!(object.get("risk").and_then(Value::as_str), Some("none" | "low" | "medium" | "high" | "unknown")) {
            bail!("claim for {key} has unsupported risk value");
        }
    }
    Ok(())
}

async fn validate_reviewer_verdict_payload(pool: &PgPool, set_id: Uuid, payload: &Value) -> Result<()> {
    let packet = payload.as_object().ok_or_else(|| anyhow::anyhow!("reviewer packet must be an object"))?;
    let top_allowed = BTreeSet::from(["summary", "requirements", "overallVerdict", "route"]);
    if packet.keys().any(|field| !top_allowed.contains(field.as_str())) {
        bail!("reviewer packet contains unsupported fields");
    }
    if !payload.get("summary").is_some_and(Value::is_string) {
        bail!("summary must be a string");
    }
    if !matches!(payload.get("overallVerdict").and_then(Value::as_str), Some("pass" | "fail" | "acceptedBlocked" | "rejectedBlocked" | "needsHumanWaiver" | "waiverAccepted")) {
        bail!("overallVerdict is missing or unsupported");
    }
    if !matches!(payload.get("route").and_then(Value::as_str), Some("source" | "owner" | "orchestrator")) {
        bail!("route is missing or unsupported");
    }
    let reqs = payload.get("requirements").and_then(Value::as_object).ok_or_else(|| anyhow::anyhow!("requirements must be an object"))?;
    let required = canonical_requirement_keys(pool, set_id).await?;
    let actual = reqs.keys().cloned().collect::<BTreeSet<_>>();
    if actual != required {
        bail!("reviewer requirements keys must exactly match the canonical RequirementSet");
    }
    let previous = sqlx::query("SELECT requirement_key, status FROM requirement_progress WHERE requirement_set_id=$1")
        .bind(set_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| (row.get::<String, _>("requirement_key"), row.get::<String, _>("status")))
        .collect::<BTreeMap<_, _>>();
    for (key, verdict) in reqs {
        let object = verdict.as_object().ok_or_else(|| anyhow::anyhow!("verdict for {key} must be an object"))?;
        let allowed = BTreeSet::from(["verdict", "justification", "evidence", "risk"]);
        if object.keys().any(|field| !allowed.contains(field.as_str())) {
            bail!("verdict for {key} contains unsupported fields");
        }
        let verdict_text = object.get("verdict").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("verdict for {key} missing verdict"))?;
        if verdict_text == "stillPassing" && previous.get(key).map(String::as_str) != Some("passed") {
            bail!("stillPassing is only valid for previously passed requirement {key}");
        }
        if !matches!(verdict_text, "pass" | "stillPassing" | "fail" | "acceptedBlocked" | "rejectedBlocked" | "waiverRequired" | "waiverAccepted") {
            bail!("verdict for {key} has unsupported verdict value");
        }
        if verdict_text == "stillPassing" {
            if object.len() != 1 {
                bail!("stillPassing verdict for {key} must not include changed verdict fields");
            }
            continue;
        }
        if !object.get("justification").is_some_and(Value::is_string) {
            bail!("verdict for {key} justification must be a string");
        }
        if !object.get("evidence").is_some_and(|value| value.as_array().is_some_and(|items| items.iter().all(Value::is_string))) {
            bail!("verdict for {key} evidence must be an array of strings");
        }
        if !matches!(object.get("risk").and_then(Value::as_str), Some("none" | "low" | "medium" | "high" | "unknown")) {
            bail!("verdict for {key} has unsupported risk value");
        }
    }
    Ok(())
}

async fn unresolved_requirement_keys(pool: &PgPool, set_id: Uuid) -> Result<BTreeSet<String>> {
    Ok(sqlx::query("SELECT requirement_key FROM requirement_progress WHERE requirement_set_id=$1 AND status NOT IN ('passed','blocked','waived') ORDER BY requirement_key ASC")
        .bind(set_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| row.get::<String, _>("requirement_key"))
        .collect())
}

async fn canonical_requirement_keys(pool: &PgPool, set_id: Uuid) -> Result<BTreeSet<String>> {
    Ok(sqlx::query("SELECT requirement_key FROM requirement_items WHERE requirement_set_id=$1 ORDER BY requirement_key ASC")
        .bind(set_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| row.get::<String, _>("requirement_key"))
        .collect())
}

async fn apply_verdict_packet(pool: &PgPool, set_id: Uuid, source_session_id: Uuid, payload: &Value) -> Result<()> {
    let previous = sqlx::query("SELECT requirement_key, status FROM requirement_progress WHERE requirement_set_id=$1")
        .bind(set_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| (row.get::<String, _>("requirement_key"), row.get::<String, _>("status")))
        .collect::<BTreeMap<_, _>>();
    if let Some(reqs) = payload.get("requirements").and_then(Value::as_object) {
        for (key, verdict) in reqs {
            let verdict_text = verdict.get("verdict").and_then(Value::as_str).unwrap_or("unknown");
            let status = progress_status_for_verdict(verdict_text, previous.get(key).map(String::as_str));
            sqlx::query("UPDATE requirement_progress SET status=$3, latest_verdict=$4, updated_at=now() WHERE requirement_set_id=$1 AND requirement_key=$2")
                .bind(set_id)
                .bind(key)
                .bind(status)
                .bind(verdict)
                .execute(pool)
                .await?;
        }
    }
    match payload.get("overallVerdict").and_then(Value::as_str).unwrap_or("unknown") {
        "pass" => deactivate(pool, source_session_id, "pass").await?,
        "waiverAccepted" => deactivate(pool, source_session_id, "waiverAccepted").await?,
        "acceptedBlocked" | "needsHumanWaiver" => {
            db::append_event(pool, source_session_id, None, "requirements", Some(set_id), "requirements.ownerAction", Some("blocked"), json!({"overallVerdict": payload.get("overallVerdict")})).await?;
        }
        "fail" | "rejectedBlocked" => {
            db::append_event(pool, source_session_id, None, "requirements", Some(set_id), "requirements.correction", Some("failed"), json!({"summary": payload.get("summary")})).await?;
        }
        _ => {}
    }
    Ok(())
}

fn progress_status_for_verdict(verdict: &str, previous_status: Option<&str>) -> &'static str {
    match verdict {
        "pass" => "passed",
        "stillPassing" if previous_status == Some("passed") => "passed",
        "stillPassing" => "failed",
        "acceptedBlocked" | "waiverRequired" => "blocked",
        "waiverAccepted" => "waived",
        "fail" | "rejectedBlocked" => "failed",
        _ => "unresolved",
    }
}

pub async fn status(pool: &PgPool, source_session_id: Uuid) -> Result<RequirementStatus> {
    let Some(active) = active_requirement_set(pool, source_session_id).await? else {
        return Ok(RequirementStatus { active_set_id: None, active: false, enforce_on_turns: false, total: 0, unresolved: 0, passed: 0, blocked: 0, waived: 0, reviewer_session_id: None, review_status: None, latest_claim_packet_id: None, latest_verdict_packet_id: None, progress: Vec::new(), owner_action: None });
    };
    let rows = sqlx::query("SELECT status, count(*) AS count FROM requirement_progress WHERE requirement_set_id=$1 GROUP BY status")
        .bind(active.id)
        .fetch_all(pool)
        .await?;
    let mut counts = BTreeMap::new();
    for row in rows {
        counts.insert(row.get::<String, _>("status"), row.get::<i64, _>("count") as usize);
    }
    let binding = sqlx::query("SELECT reviewer_session_id, status, latest_claim_packet_id, latest_verdict_packet_id FROM requirement_review_bindings WHERE requirement_set_id=$1 AND source_session_id=$2")
        .bind(active.id)
        .bind(source_session_id)
        .fetch_optional(pool)
        .await?;
    Ok(RequirementStatus {
        active_set_id: Some(active.id),
        active: true,
        enforce_on_turns: true,
        total: active.canonical_set["requirements"].as_array().map(Vec::len).unwrap_or_default(),
        unresolved: *counts.get("unresolved").unwrap_or(&0),
        passed: *counts.get("passed").unwrap_or(&0),
        blocked: *counts.get("blocked").unwrap_or(&0),
        waived: *counts.get("waived").unwrap_or(&0),
        reviewer_session_id: binding.as_ref().and_then(|row| row.get("reviewer_session_id")),
        review_status: binding.as_ref().map(|row| row.get("status")),
        latest_claim_packet_id: binding.as_ref().and_then(|row| row.get("latest_claim_packet_id")),
        latest_verdict_packet_id: binding.as_ref().and_then(|row| row.get("latest_verdict_packet_id")),
        progress: progress_rows(pool, active.id).await?,
        owner_action: latest_owner_action(pool, source_session_id).await?,
    })
}

pub async fn active_reviewer_for_source(pool: &PgPool, source_session_id: Uuid) -> Result<Option<Uuid>> {
    let Some(active) = active_requirement_set(pool, source_session_id).await? else {
        return Ok(None);
    };
    let reviewer = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT reviewer_session_id
        FROM requirement_review_bindings
        WHERE requirement_set_id=$1
          AND source_session_id=$2
          AND status IN ('ready','inReview')
          AND reviewer_session_id IS NOT NULL
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(active.id)
    .bind(source_session_id)
    .fetch_optional(pool)
    .await?;
    Ok(reviewer)
}

async fn progress_rows(pool: &PgPool, set_id: Uuid) -> Result<Vec<Value>> {
    let rows = sqlx::query("SELECT requirement_key, status, latest_verdict, updated_at FROM requirement_progress WHERE requirement_set_id=$1 ORDER BY requirement_key ASC")
        .bind(set_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|row| json!({
        "requirementKey": row.get::<String, _>("requirement_key"),
        "status": row.get::<String, _>("status"),
        "latestVerdict": row.get::<Option<Value>, _>("latest_verdict"),
        "updatedAt": row.get::<chrono::DateTime<Utc>, _>("updated_at"),
    })).collect())
}

async fn latest_owner_action(pool: &PgPool, source_session_id: Uuid) -> Result<Option<Value>> {
    let row = sqlx::query("SELECT event_type, status, payload, created_at FROM event_stream WHERE session_id=$1 AND event_type IN ('requirements.ownerAction','requirements.correction','requirements.deactivated') ORDER BY sequence DESC LIMIT 1")
        .bind(source_session_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| json!({
        "eventType": row.get::<String, _>("event_type"),
        "status": row.get::<Option<String>, _>("status"),
        "payload": row.get::<Value, _>("payload"),
        "createdAt": row.get::<chrono::DateTime<Utc>, _>("created_at"),
    })))
}

pub async fn packet_history(pool: &PgPool, source_session_id: Uuid) -> Result<Vec<Value>> {
    let rows = sqlx::query("SELECT id, requirement_set_id, reviewer_session_id, turn_id, packet_kind, status, payload, validation_error, created_at FROM requirement_packets WHERE source_session_id=$1 ORDER BY created_at ASC")
        .bind(source_session_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|row| json!({
        "id": row.get::<Uuid, _>("id"),
        "requirementSetId": row.get::<Uuid, _>("requirement_set_id"),
        "reviewerSessionId": row.get::<Option<Uuid>, _>("reviewer_session_id"),
        "turnId": row.get::<Option<Uuid>, _>("turn_id"),
        "packetKind": row.get::<String, _>("packet_kind"),
        "status": row.get::<String, _>("status"),
        "payload": row.get::<Value, _>("payload"),
        "validationError": row.get::<Option<String>, _>("validation_error"),
        "createdAt": row.get::<chrono::DateTime<Utc>, _>("created_at"),
    })).collect())
}

pub async fn close_nested_reviewers(pool: &PgPool, source_session_id: Uuid) -> Result<()> {
    let rows = sqlx::query("SELECT id FROM sessions WHERE parent_session_id=$1 AND session_kind='requirementsReviewer' AND status='open'")
        .bind(source_session_id)
        .fetch_all(pool)
        .await?;
    for row in rows {
        let reviewer_id: Uuid = row.get("id");
        sqlx::query("UPDATE sessions SET status='closed', closed_at=COALESCE(closed_at, now()), close_reason='source session closed', updated_at=now() WHERE id=$1")
            .bind(reviewer_id)
            .execute(pool)
            .await?;
        db::append_event(pool, source_session_id, None, "requirements", Some(reviewer_id), "requirements.reviewerClosed", Some("closed"), json!({"reviewerSessionId": reviewer_id})).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirement_set_validation_rejects_empty_duplicate_numbered_and_blank() {
        assert!(validate_requirement_set(&RequirementSetInput { title: None, requirements: vec![] }).is_err());
        assert!(validate_requirement_set(&RequirementSetInput {
            title: None,
            requirements: vec![RequirementInput { key: "1".to_string(), statement: "numbered".to_string(), severity: "must".to_string(), verification_method: Value::Null }],
        }).is_err());
        assert!(validate_requirement_set(&RequirementSetInput {
            title: None,
            requirements: vec![
                RequirementInput { key: "same_key".to_string(), statement: "one".to_string(), severity: "must".to_string(), verification_method: Value::Null },
                RequirementInput { key: "same_key".to_string(), statement: "two".to_string(), severity: "must".to_string(), verification_method: Value::Null },
            ],
        }).is_err());
        assert!(validate_requirement_set(&RequirementSetInput {
            title: None,
            requirements: vec![RequirementInput { key: "blank_statement".to_string(), statement: " ".to_string(), severity: "must".to_string(), verification_method: Value::Null }],
        }).is_err());
        assert!(validate_requirement_set(&RequirementSetInput {
            title: None,
            requirements: vec![RequirementInput { key: "bad_severity".to_string(), statement: "bad".to_string(), severity: "maybe".to_string(), verification_method: Value::Null }],
        }).is_err());
        assert!(validate_requirement_set(&RequirementSetInput {
            title: None,
            requirements: vec![RequirementInput { key: "bad_verification".to_string(), statement: "bad".to_string(), severity: "must".to_string(), verification_method: json!(["not", "supported"]) }],
        }).is_err());
    }

    #[test]
    fn requirement_set_validation_preserves_canonical_items() {
        let canonical = validate_requirement_set(&RequirementSetInput {
            title: Some("canonical".to_string()),
            requirements: vec![RequirementInput {
                key: "semantic_requirement".to_string(),
                statement: "Preserve this contract.".to_string(),
                severity: "critical".to_string(),
                verification_method: json!({"method":"unit"}),
            }],
        }).expect("valid");
        assert_eq!(canonical["requirements"][0]["key"], "semantic_requirement");
        assert_eq!(canonical["requirements"][0]["statement"], "Preserve this contract.");
        assert_eq!(canonical["requirements"][0]["verificationMethod"]["method"], "unit");
    }

    #[test]
    fn verdict_progress_mapping_handles_still_passing_and_unknowns() {
        assert_eq!(progress_status_for_verdict("pass", None), "passed");
        assert_eq!(progress_status_for_verdict("stillPassing", Some("passed")), "passed");
        assert_eq!(progress_status_for_verdict("stillPassing", Some("unresolved")), "failed");
        assert_eq!(progress_status_for_verdict("fail", None), "failed");
        assert_eq!(progress_status_for_verdict("rejectedBlocked", None), "failed");
        assert_eq!(progress_status_for_verdict("acceptedBlocked", None), "blocked");
        assert_eq!(progress_status_for_verdict("waiverRequired", None), "blocked");
        assert_eq!(progress_status_for_verdict("waiverAccepted", None), "waived");
        assert_eq!(progress_status_for_verdict("bogus", None), "unresolved");
    }

    #[test]
    fn composable_requirement_sets_merge_in_deterministic_order_and_reject_duplicates() {
        let permanent = RequirementSetInput {
            title: Some("permanent".to_string()),
            requirements: vec![RequirementInput { key: "permanent_contract".to_string(), statement: "Permanent contract.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"review"}) }],
        };
        let included = RequirementSetInput {
            title: Some("included".to_string()),
            requirements: vec![RequirementInput { key: "included_contract".to_string(), statement: "Included contract.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"review"}) }],
        };
        let task = RequirementSetInput {
            title: Some("task".to_string()),
            requirements: vec![RequirementInput { key: "task_contract".to_string(), statement: "Task contract.".to_string(), severity: "must".to_string(), verification_method: json!({"method":"review"}) }],
        };
        let composed = compose_requirement_sets(std::slice::from_ref(&permanent), std::slice::from_ref(&included), task).expect("composed");
        let keys = composed.requirements.iter().map(|requirement| requirement.key.as_str()).collect::<Vec<_>>();
        assert_eq!(keys, vec!["permanent_contract", "included_contract", "task_contract"]);
        assert!(compose_requirement_sets(&[permanent], &[], RequirementSetInput {
            title: None,
            requirements: vec![RequirementInput { key: "permanent_contract".to_string(), statement: "duplicate".to_string(), severity: "must".to_string(), verification_method: json!({"method":"review"}) }],
        }).is_err());
    }
}
