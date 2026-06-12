use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::roles::RoleSnapshot;
use crate::policy::{PolicyEngine, RuntimeDecision};
use crate::{approvals, db};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSeed {
    pub action_id: String,
    pub binary_name: String,
    pub candidate_paths: Vec<String>,
    pub starlark_object: String,
    pub starlark_method: String,
    #[serde(default)]
    pub argv_prefix: Vec<String>,
    #[serde(default = "default_cwd")]
    pub default_cwd: String,
    #[serde(default = "default_cwd_policy")]
    pub cwd_policy: String,
    #[serde(default = "default_env_policy")]
    pub env_policy: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: i64,
    #[serde(default = "default_output_limit")]
    pub output_limit_bytes: i64,
    #[serde(default = "default_mutation_class")]
    pub mutation_class: String,
    pub model_description: String,
    #[serde(default)]
    pub allow_cwd_arg: bool,
    #[serde(default)]
    pub allow_args_arg: bool,
    #[serde(default)]
    pub forbidden_args: Vec<String>,
    #[serde(default = "default_execution_policy")]
    pub execution_policy: String,
}

fn default_cwd() -> String { ".".to_string() }
fn default_cwd_policy() -> String { "underExecutionRoot".to_string() }
fn default_env_policy() -> String { "empty".to_string() }
fn default_timeout_ms() -> i64 { 5_000 }
fn default_output_limit() -> i64 { 12_000 }
fn default_mutation_class() -> String { "readOnly".to_string() }
fn default_execution_policy() -> String { "allow".to_string() }

#[derive(Debug, Clone)]
pub struct CommandVersion {
    pub version_id: Uuid,
    pub definition_id: Uuid,
    pub action_id: String,
    pub binary_name: String,
    pub candidate_paths: Vec<PathBuf>,
    pub starlark_object: String,
    pub starlark_method: String,
    pub argv_prefix: Vec<String>,
    pub default_cwd: String,
    pub cwd_policy: String,
    pub env_policy: String,
    pub timeout: Duration,
    pub output_limit: usize,
    pub mutation_class: String,
    pub model_description: String,
    pub allow_cwd_arg: bool,
    pub allow_args_arg: bool,
    pub forbidden_args: Vec<String>,
    pub execution_policy: String,
}

impl CommandVersion {
    pub fn resolve_binary(&self) -> Result<PathBuf> {
        self.candidate_paths
            .iter()
            .find(|candidate| candidate.is_file())
            .cloned()
            .with_context(|| format!("registered binary `{}` for `{}` is not available", self.binary_name, self.action_id))
    }

pub fn model_line(&self) -> String {
        let args = if self.allow_args_arg { "args=[...]" } else { "" };
        let cwd = if self.allow_cwd_arg { ", cwd=\".\"" } else { "" };
        format!(
            "cmd[\"{}\"].{}({}{}) -> {}; action {}; command_version_id {}",
            self.starlark_object, self.starlark_method, args, cwd, self.model_description, self.action_id, self.version_id
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryScope {
    pub scope_type: String,
    #[serde(default)]
    pub project_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalExecutionPolicy {
    pub decision: String,
    #[serde(default)]
    pub reason: Option<String>,
}

pub fn is_registry_command_action(action: &str) -> bool {
    action.starts_with("cmd.") && action.matches('.').count() >= 2
}

pub async fn bootstrap_seed_defaults(pool: &PgPool) -> Result<()> {
    let existing = sqlx::query("SELECT count(*) FROM command_definitions")
        .fetch_one(pool)
        .await?
        .get::<i64, _>(0);
    if existing > 0 {
        return Ok(());
    }
    for seed in default_command_seeds()? {
        add_command(pool, &seed, "seed-bootstrap", &RegistryScope { scope_type: "global".to_string(), project_key: None }).await?;
    }
    Ok(())
}

fn default_command_seeds() -> Result<Vec<CommandSeed>> {
    let raw = include_str!("../../../command-seeds/commands.json");
    Ok(serde_json::from_str(raw)?)
}

async fn add_command(pool: &PgPool, seed: &CommandSeed, created_by: &str, scope: &RegistryScope) -> Result<Uuid> {
    validate_seed(seed)?;
    validate_scope(scope)?;
    let mut tx = pool.begin().await?;
    let existing = sqlx::query("SELECT id FROM command_definitions WHERE action_id = $1 AND scope_type=$2 AND COALESCE(project_key, '')=COALESCE($3, '')")
        .bind(&seed.action_id)
        .bind(&scope.scope_type)
        .bind(&scope.project_key)
        .fetch_optional(&mut *tx)
        .await?;
    if existing.is_some() {
        bail!("command action already exists: {}", seed.action_id);
    }
    let definition_id = Uuid::new_v4();
    sqlx::query("INSERT INTO command_definitions (id, action_id, scope_type, project_key, enabled, metadata) VALUES ($1, $2, $3, $4, true, '{}'::jsonb)")
        .bind(definition_id)
        .bind(&seed.action_id)
        .bind(&scope.scope_type)
        .bind(&scope.project_key)
        .execute(&mut *tx)
        .await?;
    let version_id = insert_command_version(&mut tx, definition_id, seed, created_by).await?;
    let updated = sqlx::query("UPDATE command_definitions SET current_version_id=$2, enabled=true, updated_at=now() WHERE id=$1")
        .bind(definition_id)
        .bind(version_id)
        .execute(&mut *tx)
        .await?;
    if updated.rows_affected() != 1 {
        bail!("command add failed to update definition: {}", seed.action_id);
    }
    tx.commit().await?;
    Ok(version_id)
}

async fn update_command(pool: &PgPool, seed: &CommandSeed, created_by: &str, scope: &RegistryScope) -> Result<Uuid> {
    validate_seed(seed)?;
    validate_scope(scope)?;
    let mut tx = pool.begin().await?;
    let existing = sqlx::query("SELECT id FROM command_definitions WHERE action_id = $1 AND scope_type=$2 AND COALESCE(project_key, '')=COALESCE($3, '')")
        .bind(&seed.action_id)
        .bind(&scope.scope_type)
        .bind(&scope.project_key)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("command action does not exist: {}", seed.action_id))?;
    let definition_id = existing.get::<Uuid, _>("id");
    let version_id = insert_command_version(&mut tx, definition_id, seed, created_by).await?;
    let updated = sqlx::query("UPDATE command_definitions SET current_version_id=$2, updated_at=now() WHERE id=$1")
        .bind(definition_id)
        .bind(version_id)
        .execute(&mut *tx)
        .await?;
    if updated.rows_affected() != 1 {
        bail!("command update failed to update definition: {}", seed.action_id);
    }
    tx.commit().await?;
    Ok(version_id)
}

async fn insert_command_version(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    definition_id: Uuid,
    seed: &CommandSeed,
    created_by: &str,
) -> Result<Uuid> {
    let version_id = Uuid::new_v4();
    let config = serde_json::to_value(seed)?;
    sqlx::query(
        r#"
        INSERT INTO command_versions (id, definition_id, version_number, action_id, binary_name, starlark_object, starlark_method, config, model_description, created_by)
        VALUES ($1, $2, COALESCE((SELECT max(version_number)+1 FROM command_versions WHERE definition_id=$2), 1), $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(version_id)
    .bind(definition_id)
    .bind(&seed.action_id)
    .bind(&seed.binary_name)
    .bind(&seed.starlark_object)
    .bind(&seed.starlark_method)
    .bind(&config)
    .bind(&seed.model_description)
    .bind(created_by)
    .execute(&mut **tx)
    .await?;
    Ok(version_id)
}

fn validate_seed(seed: &CommandSeed) -> Result<()> {
    if !is_registry_command_action(&seed.action_id) { bail!("command action id must be registry command action: {}", seed.action_id); }
    validate_identifier("starlarkObject", &seed.starlark_object)?;
    validate_identifier("starlarkMethod", &seed.starlark_method)?;
    if seed.binary_name.trim().is_empty() { bail!("binaryName must not be empty"); }
    if seed.candidate_paths.is_empty() { bail!("candidatePaths must not be empty"); }
    if seed.cwd_policy != "underExecutionRoot" { bail!("unsupported cwdPolicy: {}", seed.cwd_policy); }
    if !matches!(seed.env_policy.as_str(), "empty" | "minimalCargo") { bail!("unsupported envPolicy: {}", seed.env_policy); }
    if seed.timeout_ms <= 0 || seed.output_limit_bytes <= 0 { bail!("timeout/output limits must be positive"); }
    Ok(())
}

fn validate_scope(scope: &RegistryScope) -> Result<()> {
    match scope.scope_type.as_str() {
        "global" => {
            if scope.project_key.is_some() {
                bail!("global command scope must not include projectKey");
            }
        }
        "project" => {
            let Some(project_key) = scope.project_key.as_deref() else {
                bail!("project command scope requires projectKey");
            };
            if project_key.trim().is_empty() {
                bail!("project command scope requires non-empty projectKey");
            }
        }
        other => bail!("unsupported command scope: {other}"),
    }
    Ok(())
}

fn validate_execution_policy(policy: &FinalExecutionPolicy) -> Result<()> {
    if !matches!(policy.decision.as_str(), "allow" | "deny" | "ownerApproval" | "orchestratorApproval") {
        bail!("unsupported final execution policy decision: {}", policy.decision);
    }
    Ok(())
}

fn validate_identifier(field: &str, value: &str) -> Result<()> {
    if value.is_empty() || !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || value.chars().next().unwrap().is_ascii_digit() {
        bail!("{field} must be a Starlark identifier: {value}");
    }
    Ok(())
}

pub async fn live_visible_commands(pool: &PgPool, _snapshot: &RoleSnapshot, project_key: Option<&str>) -> Result<Vec<CommandVersion>> {
    let rows = sqlx::query(
        r#"
        SELECT cd.id AS definition_id, cd.scope_type, cd.project_key, cv.id AS version_id, cv.action_id, cv.binary_name, cv.starlark_object, cv.starlark_method, cv.config, cv.model_description
        FROM command_definitions cd
        JOIN command_versions cv ON cv.id = cd.current_version_id
        WHERE cd.enabled = true
          AND (cd.scope_type='global' OR (cd.scope_type='project' AND cd.project_key=$1))
        ORDER BY cv.starlark_object, cv.starlark_method
        "#,
    )
    .bind(project_key)
    .fetch_all(pool)
    .await?;
    let mut commands = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for row in rows {
        let action_id: String = row.get("action_id");
        let config: Value = row.get("config");
        let seed: CommandSeed = serde_json::from_value(config)?;
        if !seen.insert(action_id.clone()) {
            bail!("ambiguous active command surface for action id: {action_id}");
        }
        commands.push(CommandVersion {
            definition_id: row.get("definition_id"),
            version_id: row.get("version_id"),
            action_id,
            binary_name: row.get("binary_name"),
            candidate_paths: seed.candidate_paths.into_iter().map(PathBuf::from).collect(),
            starlark_object: row.get("starlark_object"),
            starlark_method: row.get("starlark_method"),
            argv_prefix: seed.argv_prefix,
            default_cwd: seed.default_cwd,
            cwd_policy: seed.cwd_policy,
            env_policy: seed.env_policy,
            timeout: Duration::from_millis(seed.timeout_ms as u64),
            output_limit: seed.output_limit_bytes as usize,
            mutation_class: seed.mutation_class,
            model_description: row.get("model_description"),
            allow_cwd_arg: seed.allow_cwd_arg,
            allow_args_arg: seed.allow_args_arg,
            forbidden_args: seed.forbidden_args,
            execution_policy: seed.execution_policy,
        });
    }
    Ok(commands)
}

pub async fn command_by_action(pool: &PgPool, action_id: &str) -> Result<CommandVersion> {
    let row = sqlx::query(
        r#"
        SELECT cd.id AS definition_id, cv.id AS version_id, cv.action_id, cv.binary_name, cv.starlark_object, cv.starlark_method, cv.config, cv.model_description
        FROM command_definitions cd JOIN command_versions cv ON cv.id = cd.current_version_id
        WHERE cd.enabled=true AND cv.action_id=$1
        "#,
    ).bind(action_id).fetch_one(pool).await?;
    let seed: CommandSeed = serde_json::from_value(row.get("config"))?;
    Ok(CommandVersion {
        definition_id: row.get("definition_id"),
        version_id: row.get("version_id"),
        action_id: row.get("action_id"),
        binary_name: row.get("binary_name"),
        candidate_paths: seed.candidate_paths.into_iter().map(PathBuf::from).collect(),
        starlark_object: row.get("starlark_object"),
        starlark_method: row.get("starlark_method"),
        argv_prefix: seed.argv_prefix,
        default_cwd: seed.default_cwd,
        cwd_policy: seed.cwd_policy,
        env_policy: seed.env_policy,
        timeout: Duration::from_millis(seed.timeout_ms as u64),
        output_limit: seed.output_limit_bytes as usize,
        mutation_class: seed.mutation_class,
        model_description: row.get("model_description"),
        allow_cwd_arg: seed.allow_cwd_arg,
        allow_args_arg: seed.allow_args_arg,
        forbidden_args: seed.forbidden_args,
        execution_policy: seed.execution_policy,
    })
}

pub async fn command_by_version(pool: &PgPool, version_id: Uuid) -> Result<CommandVersion> {
    let row = sqlx::query(
        r#"
        SELECT cv.definition_id, cv.id AS version_id, cv.action_id, cv.binary_name, cv.starlark_object, cv.starlark_method, cv.config, cv.model_description
        FROM command_versions cv
        WHERE cv.id=$1
        "#,
    ).bind(version_id).fetch_one(pool).await?;
    let seed: CommandSeed = serde_json::from_value(row.get("config"))?;
    Ok(CommandVersion {
        definition_id: row.get("definition_id"),
        version_id: row.get("version_id"),
        action_id: row.get("action_id"),
        binary_name: row.get("binary_name"),
        candidate_paths: seed.candidate_paths.into_iter().map(PathBuf::from).collect(),
        starlark_object: row.get("starlark_object"),
        starlark_method: row.get("starlark_method"),
        argv_prefix: seed.argv_prefix,
        default_cwd: seed.default_cwd,
        cwd_policy: seed.cwd_policy,
        env_policy: seed.env_policy,
        timeout: Duration::from_millis(seed.timeout_ms as u64),
        output_limit: seed.output_limit_bytes as usize,
        mutation_class: seed.mutation_class,
        model_description: row.get("model_description"),
        allow_cwd_arg: seed.allow_cwd_arg,
        allow_args_arg: seed.allow_args_arg,
        forbidden_args: seed.forbidden_args,
        execution_policy: seed.execution_policy,
    })
}

pub fn execute_code_contract(commands: &[CommandVersion]) -> String {
    let mut lines = vec![
        "Evaluate Starlark in the experimental host runtime. Use output(value) to emit final tool output; host calls return script values and do not implicitly become final output. Native APIs: fs.read(path), fs.write(path, content), patch.apply(unified_diff). Registry commands available now:".to_string(),
    ];
    for command in commands { lines.push(format!("- {}", command.model_line())); }
    lines.push("No raw shell, network, arbitrary environment, unregistered binaries, or undocumented command surfaces are available.".to_string());
    lines.join("\n")
}

pub fn request_tool_contract() -> String {
    "Use request_command_registry_change when execute_code is blocked by a missing or outdated command registry entry. Submit operation add/update/disable/enable, proposedCommand, rationale, intendedUse, currentBlockerOrNeed, and requesterContext. Do not choose authoritative scope or execution policy; approvers select final scope, final execution policy, and final command edits before separate apply. This is a native model tool outside Starlark.".to_string()
}

pub fn starlark_prelude(commands: &[CommandVersion]) -> Result<String> {
    use std::collections::BTreeMap;
    let mut out = String::from("cmd = {}\n");
    let mut by_object: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for command in commands {
        validate_identifier("starlarkObject", &command.starlark_object)?;
        validate_identifier("starlarkMethod", &command.starlark_method)?;
        let fname = format!("__cmd_{}_{}", command.starlark_object, command.starlark_method);
        let params = match (command.allow_args_arg, command.allow_cwd_arg) {
            (true, true) => "args=[], cwd=\".\"",
            (true, false) => "args=[]",
            (false, true) => "cwd=\".\"",
            (false, false) => "",
        };
        let args_expr = if command.allow_args_arg { "args" } else { "[]" };
        let cwd_expr = if command.allow_cwd_arg { "cwd".to_string() } else { format!("\"{}\"", command.default_cwd) };
        out.push_str(&format!("def {fname}({params}):\n    return __cmd.run(\"{}\", {args_expr}, {cwd_expr})\n", command.action_id));
        by_object.entry(command.starlark_object.clone()).or_default().push((command.starlark_method.clone(), fname));
    }
    for (object, methods) in by_object {
        let fields = methods.into_iter().map(|(method, fname)| format!("{method}={fname}")).collect::<Vec<_>>().join(", ");
        out.push_str(&format!("cmd[\"{object}\"] = struct({fields})\n"));
    }
    Ok(out)
}

pub async fn list(pool: &PgPool) -> Result<Vec<Value>> {
    let rows = sqlx::query("SELECT cd.action_id, cd.scope_type, cd.project_key, cd.enabled, cd.current_version_id, cv.config FROM command_definitions cd LEFT JOIN command_versions cv ON cv.id=cd.current_version_id ORDER BY cd.scope_type, cd.project_key, cd.action_id")
        .fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| json!({"actionId": r.get::<String,_>("action_id"), "scope": {"type": r.get::<String,_>("scope_type"), "projectKey": r.get::<Option<String>,_>("project_key")}, "enabled": r.get::<bool,_>("enabled"), "currentVersionId": r.get::<Option<Uuid>,_>("current_version_id"), "config": r.get::<Option<Value>,_>("config")})).collect())
}

pub async fn validate_policy_actions_exist(pool: &PgPool, actions: impl Iterator<Item = String>) -> Result<()> {
    for action in actions {
        if is_registry_command_action(&action) {
            let _ = pool;
            bail!("concrete command actions are not valid role policy entries: {action}");
        }
    }
    Ok(())
}

pub async fn show(pool: &PgPool, action_id: &str) -> Result<Value> {
    let row = sqlx::query("SELECT cd.id, cd.action_id, cd.scope_type, cd.project_key, cd.enabled, cd.current_version_id, cv.config FROM command_definitions cd LEFT JOIN command_versions cv ON cv.id=cd.current_version_id WHERE cd.action_id=$1 ORDER BY cd.scope_type, cd.project_key LIMIT 1")
        .bind(action_id).fetch_one(pool).await?;
    Ok(json!({"id": row.get::<Uuid,_>("id"), "actionId": row.get::<String,_>("action_id"), "scope": {"type": row.get::<String,_>("scope_type"), "projectKey": row.get::<Option<String>,_>("project_key")}, "enabled": row.get::<bool,_>("enabled"), "currentVersionId": row.get::<Option<Uuid>,_>("current_version_id"), "config": row.get::<Option<Value>,_>("config")}))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequestInput { pub operation: String, pub command: CommandSeed, pub rationale: String, pub recommended_policy: String, pub requester: String }

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRegistryChangeRequest {
    pub operation: String,
    pub proposed_command: CommandSeed,
    pub rationale: String,
    pub intended_use: String,
    pub current_blocker_or_need: String,
    pub requester_context: Value,
}

async fn authorize_registry_action(
    pool: &PgPool,
    session_id: Uuid,
    action: &str,
    input: Value,
    linked_approval_request_id: Option<Uuid>,
) -> Result<RoleSnapshot> {
    let snapshot = db::session_role_snapshot(pool, session_id).await?;
    let policy = PolicyEngine::decide(&snapshot, action, input);
    db::append_event(pool, session_id, None, "policy", None, "policy.decision", Some(policy.decision.as_str()), policy.to_event_payload()).await?;
    match policy.decision {
        RuntimeDecision::Allow => Ok(snapshot),
        RuntimeDecision::ApprovalRequired => {
            if let Some(approval_id) = linked_approval_request_id {
                validate_consumable_registry_approval(pool, approval_id, session_id, action, &policy.input).await?;
                return Ok(snapshot);
            }
            let approval_id = approvals::request_approval(pool, session_id, None, &policy, &snapshot).await?;
            if action == "command_registry.apply" {
                if let Some(request_id) = policy.input.get("requestId").and_then(Value::as_str).and_then(|raw| Uuid::parse_str(raw).ok()) {
                    sqlx::query("UPDATE command_registry_requests SET approval_request_id=$2 WHERE id=$1")
                        .bind(request_id)
                        .bind(approval_id)
                        .execute(pool)
                        .await?;
                }
            }
            bail!("{action} requires approval before registry mutation: approvalRequestId={approval_id}");
        }
        RuntimeDecision::Deny => bail!("{action} denied by role policy: {}", policy.reason),
    }
}

async fn validate_consumable_registry_approval(
    pool: &PgPool,
    approval_id: Uuid,
    session_id: Uuid,
    action: &str,
    input: &Value,
) -> Result<()> {
    let row = sqlx::query(
        r#"
        SELECT session_id, action_name, status, input_context
        FROM approval_requests
        WHERE id=$1
        "#,
    )
    .bind(approval_id)
    .fetch_one(pool)
    .await?;
    let approval_session: Uuid = row.get("session_id");
    let approval_action: String = row.get("action_name");
    let status: String = row.get("status");
    let input_context: Value = row.get("input_context");
    if approval_session != session_id {
        bail!("linked approval request {approval_id} belongs to a different session");
    }
    if approval_action != action {
        bail!("linked approval request {approval_id} is for {approval_action}, not {action}");
    }
    if status != "approved" {
        bail!("linked approval request {approval_id} is not approved: status={status}");
    }
    let approved_input = input_context.get("input").unwrap_or(&Value::Null);
    if approved_input != input {
        bail!("linked approval request {approval_id} input does not match requested registry action");
    }
    Ok(())
}

pub async fn create_request(pool: &PgPool, session_id: Uuid, input: ChangeRequestInput) -> Result<Uuid> {
    validate_seed(&input.command)?;
    let snapshot = authorize_registry_action(pool, session_id, "command_registry.request", serde_json::to_value(&input)?, None).await?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO command_registry_requests (id, session_id, operation, proposed_command, requester_context, rationale, recommended_policy, requester, requested_by_role, approval_status, application_status) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'pending','pending')")
        .bind(id).bind(session_id).bind(input.operation).bind(serde_json::to_value(input.command)?).bind(json!({})).bind(input.rationale).bind(input.recommended_policy).bind(input.requester).bind(json!({"id":snapshot.id,"version":snapshot.version,"roleVersionId":snapshot.role_version_id})).execute(pool).await?;
    Ok(id)
}

pub async fn create_native_model_request(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    input: NativeRegistryChangeRequest,
    role_snapshot: &RoleSnapshot,
    project_key: Option<&str>,
) -> Result<Uuid> {
    validate_seed(&input.proposed_command)?;
    if !matches!(input.operation.as_str(), "add" | "update" | "disable" | "enable") {
        bail!("unsupported command registry operation: {}", input.operation);
    }
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO command_registry_requests (id, session_id, operation, proposed_command, requester_context, rationale, recommended_policy, requester, requested_by_role, approval_status, application_status) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'pending','pending')",
    )
    .bind(id)
    .bind(session_id)
    .bind(&input.operation)
    .bind(serde_json::to_value(&input.proposed_command)?)
    .bind(json!({
        "turnId": turn_id,
        "role": {"id": role_snapshot.id, "version": role_snapshot.version, "roleVersionId": role_snapshot.role_version_id},
        "projectKey": project_key,
        "requesterContext": input.requester_context,
        "intendedUse": input.intended_use,
        "currentBlockerOrNeed": input.current_blocker_or_need,
    }))
    .bind(input.rationale)
    .bind("advisory only; final scope and execution policy must be selected by approver")
    .bind("native-model-tool")
    .bind(json!({"id":role_snapshot.id,"version":role_snapshot.version,"roleVersionId":role_snapshot.role_version_id}))
    .execute(pool)
    .await?;
    db::append_event(pool, session_id, Some(turn_id), "command_registry_request", Some(id), "command_registry.requested", Some("pending"), json!({"requestId": id, "operation": input.operation, "actionId": input.proposed_command.action_id, "projectKey": project_key})).await?;
    Ok(id)
}

pub async fn create_seed_import_requests(pool: &PgPool, session_id: Uuid, mode: &str) -> Result<Vec<Uuid>> {
    if !matches!(mode, "missing" | "refresh") {
        bail!("seed import mode must be missing or refresh");
    }
    let mut ids = Vec::new();
    for seed in default_command_seeds()? {
        let exists = sqlx::query("SELECT EXISTS (SELECT 1 FROM command_definitions WHERE action_id=$1)")
            .bind(&seed.action_id)
            .fetch_one(pool)
            .await?
            .get::<bool, _>(0);
        let operation = match (mode, exists) {
            ("missing", false) => "add",
            ("refresh", true) => "update",
            ("refresh", false) => "add",
            ("missing", true) => continue,
            _ => unreachable!(),
        };
        ids.push(
            create_request(
                pool,
                session_id,
                ChangeRequestInput {
                    operation: operation.to_string(),
                    command: seed,
                    rationale: format!("explicit seed import mode={mode}"),
                    recommended_policy: "operator-reviewed seed import".to_string(),
                    requester: "command-registry seed-requests".to_string(),
                },
            )
            .await?,
        );
    }
    Ok(ids)
}

pub async fn decide_request(
    pool: &PgPool,
    session_id: Uuid,
    id: Uuid,
    status: &str,
    final_scope: Option<RegistryScope>,
    final_execution_policy: Option<FinalExecutionPolicy>,
    final_command: Option<CommandSeed>,
) -> Result<()> {
    if !matches!(status, "approved" | "denied") { bail!("approval status must be approved or denied"); }
    if status == "approved" {
        validate_scope(final_scope.as_ref().ok_or_else(|| anyhow::anyhow!("approved registry request requires final scope"))?)?;
        validate_execution_policy(final_execution_policy.as_ref().ok_or_else(|| anyhow::anyhow!("approved registry request requires final execution policy"))?)?;
        validate_seed(final_command.as_ref().ok_or_else(|| anyhow::anyhow!("approved registry request requires final command"))?)?;
    }
    authorize_registry_action(pool, session_id, "command_registry.decide", json!({"requestId": id, "status": status}), None).await?;
    let done = sqlx::query("UPDATE command_registry_requests SET approval_status=$2, final_scope=$3, final_execution_policy=$4, final_command=$5, decided_at=now() WHERE id=$1 AND approval_status='pending'")
        .bind(id).bind(status)
        .bind(serde_json::to_value(final_scope)?)
        .bind(serde_json::to_value(final_execution_policy)?)
        .bind(serde_json::to_value(final_command)?)
        .execute(pool).await?.rows_affected();
    if done != 1 { bail!("command registry request is not pending or does not exist: {id}"); }
    Ok(())
}

pub async fn list_requests(pool: &PgPool) -> Result<Vec<Value>> {
    let rows = sqlx::query("SELECT id, session_id, operation, proposed_command, requester_context, final_scope, final_execution_policy, final_command, approval_status, application_status, requester, requested_by_role FROM command_registry_requests ORDER BY created_at DESC").fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| json!({"id": r.get::<Uuid,_>("id"), "sessionId": r.get::<Option<Uuid>,_>("session_id"), "operation": r.get::<String,_>("operation"), "proposedCommand": r.get::<Value,_>("proposed_command"), "requesterContext": r.get::<Value,_>("requester_context"), "finalScope": r.get::<Option<Value>,_>("final_scope"), "finalExecutionPolicy": r.get::<Option<Value>,_>("final_execution_policy"), "finalCommand": r.get::<Option<Value>,_>("final_command"), "approvalStatus": r.get::<String,_>("approval_status"), "applicationStatus": r.get::<String,_>("application_status"), "requester": r.get::<String,_>("requester"), "requestedByRole": r.get::<Value,_>("requested_by_role")})).collect())
}

pub async fn show_request(pool: &PgPool, id: Uuid) -> Result<Value> {
    let row = sqlx::query("SELECT * FROM command_registry_requests WHERE id=$1").bind(id).fetch_one(pool).await?;
    Ok(json!({"id": row.get::<Uuid,_>("id"), "sessionId": row.get::<Option<Uuid>,_>("session_id"), "operation": row.get::<String,_>("operation"), "proposedCommand": row.get::<Value,_>("proposed_command"), "requesterContext": row.get::<Value,_>("requester_context"), "rationale": row.get::<String,_>("rationale"), "recommendedPolicy": row.get::<String,_>("recommended_policy"), "requester": row.get::<String,_>("requester"), "requestedByRole": row.get::<Value,_>("requested_by_role"), "approvalRequestId": row.get::<Option<Uuid>,_>("approval_request_id"), "finalScope": row.get::<Option<Value>,_>("final_scope"), "finalExecutionPolicy": row.get::<Option<Value>,_>("final_execution_policy"), "finalCommand": row.get::<Option<Value>,_>("final_command"), "approvalStatus": row.get::<String,_>("approval_status"), "applicationStatus": row.get::<String,_>("application_status")}))
}

pub async fn apply_request(pool: &PgPool, session_id: Uuid, id: Uuid) -> Result<()> {
    let row = sqlx::query("SELECT operation, proposed_command, final_scope, final_execution_policy, final_command, approval_status, application_status, approval_request_id FROM command_registry_requests WHERE id=$1").bind(id).fetch_one(pool).await?;
    if row.get::<String,_>("approval_status") != "approved" { bail!("command registry request must be approved before apply"); }
    if row.get::<String,_>("application_status") != "pending" { bail!("command registry request already applied or failed"); }
    let linked_approval_request_id: Option<Uuid> = row.get("approval_request_id");
    authorize_registry_action(pool, session_id, "command_registry.apply", json!({"requestId": id}), linked_approval_request_id).await?;
    let operation: String = row.get("operation");
    let scope: RegistryScope = serde_json::from_value(row.get::<Option<Value>, _>("final_scope")
        .ok_or_else(|| anyhow::anyhow!("approved registry request missing final scope"))?)?;
    let policy: FinalExecutionPolicy = serde_json::from_value(row.get::<Option<Value>, _>("final_execution_policy")
        .ok_or_else(|| anyhow::anyhow!("approved registry request missing final execution policy"))?)?;
    validate_scope(&scope)?;
    validate_execution_policy(&policy)?;
    let mut command: CommandSeed = serde_json::from_value(row.get::<Option<Value>, _>("final_command")
        .ok_or_else(|| anyhow::anyhow!("approved registry request missing final command"))?)?;
    command.execution_policy = policy.decision.clone();
    reject_scoped_conflict(pool, &command.action_id, &scope, operation.as_str()).await?;
    match operation.as_str() {
        "add" => {
            add_command(pool, &command, "registry-request", &scope).await?;
        }
        "update" => {
            update_command(pool, &command, "registry-request", &scope).await?;
        }
        "enable" => {
            validate_seed(&command)?;
            let existing = sqlx::query("SELECT enabled FROM command_definitions WHERE action_id=$1 AND scope_type=$2 AND COALESCE(project_key, '')=COALESCE($3, '')")
                .bind(&command.action_id)
                .bind(&scope.scope_type)
                .bind(&scope.project_key)
                .fetch_optional(pool)
                .await?;
            let Some(existing) = existing else {
                bail!("command action does not exist: {}", command.action_id);
            };
            if existing.get::<bool, _>("enabled") {
                bail!("command enable did not change exactly one disabled row: {}", command.action_id);
            }
            update_command(pool, &command, "registry-request", &scope).await?;
            let updated = sqlx::query("UPDATE command_definitions SET enabled=true, updated_at=now() WHERE action_id=$1 AND scope_type=$2 AND COALESCE(project_key, '')=COALESCE($3, '') AND enabled=false")
                .bind(&command.action_id)
                .bind(&scope.scope_type)
                .bind(&scope.project_key)
                .execute(pool)
                .await?;
            if updated.rows_affected() != 1 {
                bail!("command enable did not change exactly one disabled row: {}", command.action_id);
            }
        }
        "disable" => {
            validate_seed(&command)?;
            let updated = sqlx::query("UPDATE command_definitions SET enabled=false, updated_at=now() WHERE action_id=$1 AND scope_type=$2 AND COALESCE(project_key, '')=COALESCE($3, '') AND enabled=true")
                .bind(&command.action_id)
                .bind(&scope.scope_type)
                .bind(&scope.project_key)
                .execute(pool)
                .await?;
            if updated.rows_affected() != 1 {
                bail!("command disable did not change exactly one enabled row: {}", command.action_id);
            }
        }
        other => bail!("unsupported command registry operation: {other}"),
    }
    let updated = sqlx::query("UPDATE command_registry_requests SET application_status='applied', applied_at=now() WHERE id=$1 AND application_status='pending'")
        .bind(id)
        .execute(pool)
        .await?;
    if updated.rows_affected() != 1 {
        bail!("command registry request apply status update failed: {id}");
    }
    Ok(())
}

async fn reject_scoped_conflict(pool: &PgPool, action_id: &str, scope: &RegistryScope, operation: &str) -> Result<()> {
    if !matches!(operation, "add" | "update" | "enable") {
        return Ok(());
    }
    if scope.scope_type == "project" {
        let global_exists = sqlx::query("SELECT EXISTS (SELECT 1 FROM command_definitions WHERE action_id=$1 AND scope_type='global' AND enabled=true)")
            .bind(action_id)
            .fetch_one(pool)
            .await?
            .get::<bool, _>(0);
        if global_exists {
            bail!("scoped command action conflict: global command already visible for {action_id}");
        }
    } else {
        let project_exists = sqlx::query("SELECT EXISTS (SELECT 1 FROM command_definitions WHERE action_id=$1 AND scope_type='project' AND enabled=true)")
            .bind(action_id)
            .fetch_one(pool)
            .await?
            .get::<bool, _>(0);
        if project_exists {
            bail!("scoped command action conflict: project command already visible for {action_id}");
        }
    }
    Ok(())
}
