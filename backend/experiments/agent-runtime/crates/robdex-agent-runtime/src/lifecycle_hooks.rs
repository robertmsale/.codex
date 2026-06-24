use std::cell::RefCell;
use std::collections::BTreeSet;
use std::time::Instant;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use starlark::any::ProvidesStaticType;
use starlark::collections::SmallMap;
use starlark::environment::{GlobalsBuilder, Module};
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::{Value as StarlarkValue};
use starlark::values::none::NoneType;
use uuid::Uuid;

use crate::{db, roles::RoleSnapshot};

pub const MAX_HOOK_SOURCE_BYTES: usize = 128 * 1024;
pub const MAX_CONTEXT_BYTES: usize = 64 * 1024;
pub const MAX_RETURNED_INTENTS: usize = 64;
pub const MAX_INTENT_BYTES: usize = 32 * 1024;
pub const EVALUATION_TIMEOUT_MS: u64 = 250;
pub const EVALUATION_FUEL_STEPS: u64 = 20_000;
pub const EVALUATION_MAX_CALLSTACK: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleHook {
    OnProjectRuntimeActivate,
    OnSessionCreateRequest,
    OnSessionCreated,
    OnTurnSubmitted,
    OnTurnStart,
    OnModelRequest,
    OnModelFinal,
    OnToolStart,
    OnToolComplete,
    OnPacketRecorded,
    OnResourceReserved,
    OnResourceReleased,
    OnTurnComplete,
    OnSessionClose,
    OnSessionArchive,
    OnCompactionComplete,
}

impl LifecycleHook {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OnProjectRuntimeActivate => "on_project_runtime_activate",
            Self::OnSessionCreateRequest => "on_session_create_request",
            Self::OnSessionCreated => "on_session_created",
            Self::OnTurnSubmitted => "on_turn_submitted",
            Self::OnTurnStart => "on_turn_start",
            Self::OnModelRequest => "on_model_request",
            Self::OnModelFinal => "on_model_final",
            Self::OnToolStart => "on_tool_start",
            Self::OnToolComplete => "on_tool_complete",
            Self::OnPacketRecorded => "on_packet_recorded",
            Self::OnResourceReserved => "on_resource_reserved",
            Self::OnResourceReleased => "on_resource_released",
            Self::OnTurnComplete => "on_turn_complete",
            Self::OnSessionClose => "on_session_close",
            Self::OnSessionArchive => "on_session_archive",
            Self::OnCompactionComplete => "on_compaction_complete",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "on_project_runtime_activate" => Self::OnProjectRuntimeActivate,
            "on_session_create_request" => Self::OnSessionCreateRequest,
            "on_session_created" => Self::OnSessionCreated,
            "on_turn_submitted" => Self::OnTurnSubmitted,
            "on_turn_start" => Self::OnTurnStart,
            "on_model_request" => Self::OnModelRequest,
            "on_model_final" => Self::OnModelFinal,
            "on_tool_start" => Self::OnToolStart,
            "on_tool_complete" => Self::OnToolComplete,
            "on_packet_recorded" => Self::OnPacketRecorded,
            "on_resource_reserved" => Self::OnResourceReserved,
            "on_resource_released" => Self::OnResourceReleased,
            "on_turn_complete" => Self::OnTurnComplete,
            "on_session_close" => Self::OnSessionClose,
            "on_session_archive" => Self::OnSessionArchive,
            "on_compaction_complete" => Self::OnCompactionComplete,
            other => bail!("unknown lifecycle hook: {other}"),
        })
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::OnProjectRuntimeActivate,
            Self::OnSessionCreateRequest,
            Self::OnSessionCreated,
            Self::OnTurnSubmitted,
            Self::OnTurnStart,
            Self::OnModelRequest,
            Self::OnModelFinal,
            Self::OnToolStart,
            Self::OnToolComplete,
            Self::OnPacketRecorded,
            Self::OnResourceReserved,
            Self::OnResourceReleased,
            Self::OnTurnComplete,
            Self::OnSessionClose,
            Self::OnSessionArchive,
            Self::OnCompactionComplete,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookIntentType {
    RequireOutputSchema,
    RecordPacket,
    RoutePacket,
    NotifySession,
    EnsureSubagent,
    CloseSubagent,
    ReserveResource,
    ReleaseResource,
    AddTurnObligation,
    UpdateContractProgress,
    RequestOwnerApproval,
    BlockWithReason,
}

impl HookIntentType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RequireOutputSchema => "require_output_schema",
            Self::RecordPacket => "record_packet",
            Self::RoutePacket => "route_packet",
            Self::NotifySession => "notify_session",
            Self::EnsureSubagent => "ensure_subagent",
            Self::CloseSubagent => "close_subagent",
            Self::ReserveResource => "reserve_resource",
            Self::ReleaseResource => "release_resource",
            Self::AddTurnObligation => "add_turn_obligation",
            Self::UpdateContractProgress => "update_contract_progress",
            Self::RequestOwnerApproval => "request_owner_approval",
            Self::BlockWithReason => "block_with_reason",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "require_output_schema" => Self::RequireOutputSchema,
            "record_packet" => Self::RecordPacket,
            "route_packet" => Self::RoutePacket,
            "notify_session" => Self::NotifySession,
            "ensure_subagent" => Self::EnsureSubagent,
            "close_subagent" => Self::CloseSubagent,
            "reserve_resource" => Self::ReserveResource,
            "release_resource" => Self::ReleaseResource,
            "add_turn_obligation" => Self::AddTurnObligation,
            "update_contract_progress" => Self::UpdateContractProgress,
            "request_owner_approval" => Self::RequestOwnerApproval,
            "block_with_reason" => Self::BlockWithReason,
            other => bail!("unknown hook intent type: {other}"),
        })
    }
}

pub fn allowed_intents(boundary: LifecycleHook) -> BTreeSet<HookIntentType> {
    use HookIntentType::*;
    match boundary {
        LifecycleHook::OnProjectRuntimeActivate => BTreeSet::from([RecordPacket, NotifySession, BlockWithReason]),
        LifecycleHook::OnSessionCreateRequest => BTreeSet::from([BlockWithReason, RecordPacket]),
        LifecycleHook::OnSessionCreated => BTreeSet::from([RecordPacket, NotifySession, EnsureSubagent, ReserveResource]),
        LifecycleHook::OnTurnSubmitted => BTreeSet::from([RecordPacket, RoutePacket, AddTurnObligation, BlockWithReason]),
        LifecycleHook::OnTurnStart => BTreeSet::from([RequireOutputSchema, RecordPacket, RoutePacket, AddTurnObligation, BlockWithReason]),
        LifecycleHook::OnModelRequest => BTreeSet::from([RequireOutputSchema, RecordPacket, BlockWithReason]),
        LifecycleHook::OnModelFinal => BTreeSet::from([RecordPacket, RoutePacket, EnsureSubagent, UpdateContractProgress, RequestOwnerApproval, BlockWithReason]),
        LifecycleHook::OnToolStart => BTreeSet::from([RecordPacket, BlockWithReason]),
        LifecycleHook::OnToolComplete => BTreeSet::from([RecordPacket, RoutePacket, AddTurnObligation, ReserveResource, ReleaseResource, BlockWithReason]),
        LifecycleHook::OnPacketRecorded => BTreeSet::from([RoutePacket, NotifySession, EnsureSubagent, CloseSubagent, UpdateContractProgress, RequestOwnerApproval, BlockWithReason]),
        LifecycleHook::OnResourceReserved => BTreeSet::from([NotifySession, RecordPacket, AddTurnObligation]),
        LifecycleHook::OnResourceReleased => BTreeSet::from([NotifySession, RecordPacket]),
        LifecycleHook::OnTurnComplete => BTreeSet::from([NotifySession, ReleaseResource, AddTurnObligation, RecordPacket, RoutePacket]),
        LifecycleHook::OnSessionClose | LifecycleHook::OnSessionArchive => BTreeSet::from([CloseSubagent, ReleaseResource, RecordPacket, NotifySession]),
        LifecycleHook::OnCompactionComplete => BTreeSet::from([RecordPacket, RoutePacket, NotifySession]),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookIntent {
    #[serde(rename = "type")]
    pub intent_type: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookContext {
    pub project_key: Option<String>,
    pub session_id: Option<Uuid>,
    pub session_kind: Option<String>,
    pub parent_session_id: Option<Uuid>,
    pub hidden: bool,
    pub role_snapshot_summary: Value,
    pub workdir: Option<String>,
    pub worktree_root: Option<String>,
    pub turn_summary: Value,
    pub lifecycle_event: String,
    pub active_contracts: Vec<Value>,
    pub recent_packet_summaries: Vec<Value>,
    pub subagent_summaries: Vec<Value>,
    pub resource_lease_summaries: Vec<Value>,
    pub visible_command_summaries: Vec<Value>,
    pub tool_metadata: Vec<Value>,
    pub routing_state: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookEvaluationResult {
    pub lifecycle_event_id: Uuid,
    pub validation_status: String,
    pub returned_intents: Vec<HookIntent>,
    pub errors: Vec<String>,
    pub timing_metadata: Value,
    pub context_hash: String,
}

pub fn source_hash(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn validate_hook_source(source: &str) -> Result<()> {
    if source.len() > MAX_HOOK_SOURCE_BYTES {
        bail!("hook source exceeds {} bytes", MAX_HOOK_SOURCE_BYTES);
    }
    if source.contains("load(") {
        bail!("project hook source must be self-contained; load() is disabled");
    }
    if source.contains("while ") || source.contains("def __") {
        bail!("project hook source uses a rejected construct");
    }
    reject_unbounded_or_recursive_starlark(source)?;
    AstModule::parse("project_runtime.star", source.to_string(), &Dialect::Standard)
        .map(|_| ())
        .map_err(|error| anyhow::anyhow!("project runtime source is invalid Starlark: {error}"))
}

pub fn validate_runtime_manifest(manifest: &Value) -> Result<()> {
    let obj = manifest.as_object().ok_or_else(|| anyhow::anyhow!("manifest must be an object"))?;
    let mut role_ids = BTreeSet::new();
    let mut resource_ids = BTreeSet::new();
    let mut hook_names = BTreeSet::new();
    let mut tool_bundle_ids = BTreeSet::new();
    let mut packet_types = BTreeSet::new();
    for hook in obj.get("hooks").and_then(Value::as_array).into_iter().flatten() {
        let name = hook.get("name").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("hook name is required"))?;
        LifecycleHook::parse(name)?;
        if !hook_names.insert(name.to_string()) {
            bail!("duplicate lifecycle hook binding: {name}");
        }
        let source = hook.get("source").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("hook source is required for {name}"))?;
        validate_hook_source(source)?;
        let intent_types = hook.get("intentTypes").and_then(Value::as_array).cloned().unwrap_or_default();
        for intent in intent_types {
            let Some(intent_type) = intent.as_str() else {
                bail!("hook intentTypes must be strings for {name}");
            };
            let parsed = HookIntentType::parse(intent_type)?;
            if !allowed_intents(LifecycleHook::parse(name)?).contains(&parsed) {
                bail!("intent {intent_type} is not legal at hook {name}");
            }
        }
        if hook.get("source").and_then(Value::as_str).is_none() {
            bail!("hook source is required for {name}");
        }
    }
    for role in obj.get("roles").and_then(Value::as_array).into_iter().flatten() {
        let id = role.get("id").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("role id is required"))?;
        if id.trim().is_empty() {
            bail!("role id must not be empty");
        }
        if !role_ids.insert(id.to_string()) {
            bail!("duplicate role id: {id}");
        }
    }
    for bundle in obj.get("roleToolBundles").and_then(Value::as_array).into_iter().flatten() {
        let id = bundle.get("id").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("role tool bundle id is required"))?;
        if id.trim().is_empty() || !tool_bundle_ids.insert(id.to_string()) {
            bail!("role tool bundle id must be non-empty and unique: {id}");
        }
        for tool in bundle.get("tools").and_then(Value::as_array).into_iter().flatten() {
            if tool.as_str().map(str::trim).unwrap_or_default().is_empty() {
                bail!("role tool bundle tools must be non-empty strings");
            }
        }
    }
    for role in obj.get("roles").and_then(Value::as_array).into_iter().flatten() {
        if let Some(bundle) = role.get("toolBundle").and_then(Value::as_str) {
            if !tool_bundle_ids.contains(bundle) {
                bail!("role references unknown tool bundle: {bundle}");
            }
        }
    }
    for binding in obj.get("commandBundleBindings").and_then(Value::as_array).into_iter().flatten() {
        let role_id = binding.get("roleId").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("command bundle binding roleId is required"))?;
        let bundle_id = binding.get("bundleId").or_else(|| binding.get("bundle")).and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("command bundle binding bundle id is required"))?;
        if !role_ids.contains(role_id) {
            bail!("command bundle binding references unknown role: {role_id}");
        }
        if !tool_bundle_ids.contains(bundle_id) {
            bail!("command bundle binding references unknown tool bundle: {bundle_id}");
        }
    }
    for channel in obj.get("channels").and_then(Value::as_array).into_iter().flatten() {
        let id = channel.get("id").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("channel id is required"))?;
        if id.trim().is_empty() {
            bail!("channel id must not be empty");
        }
        let packets = channel.get("packetTypes").and_then(Value::as_array).ok_or_else(|| anyhow::anyhow!("channel packetTypes are required"))?;
        if packets.is_empty() {
            bail!("channel packetTypes must not be empty");
        }
        for packet in packets {
            let packet = packet.as_str().ok_or_else(|| anyhow::anyhow!("channel packetTypes must be strings"))?;
            if !packet.contains('.') {
                bail!("packet type must be namespaced: {packet}");
            }
            packet_types.insert(packet.to_string());
        }
    }
    for resource in obj.get("resourceTypes").and_then(Value::as_array).into_iter().flatten() {
        let id = resource.get("id").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("resource type id is required"))?;
        if id.trim().is_empty() {
            bail!("resource type id must not be empty");
        }
        if !resource_ids.insert(id.to_string()) {
            bail!("duplicate resource type id: {id}");
        }
    }
    for binding in obj.get("stewardBindings").and_then(Value::as_array).into_iter().flatten() {
        let resource_type = binding.get("resourceType").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("steward binding resourceType is required"))?;
        let steward_role = binding.get("stewardRole").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("steward binding stewardRole is required"))?;
        if !resource_ids.contains(resource_type) {
            bail!("steward binding references unknown resource type: {resource_type}");
        }
        if !role_ids.contains(steward_role) {
            bail!("steward binding references unknown role: {steward_role}");
        }
    }
    for workflow in obj.get("contractWorkflows").and_then(Value::as_array).into_iter().flatten() {
        let workflow_id = workflow.get("id").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("contract workflow id is required"))?;
        if workflow_id.trim().is_empty() {
            bail!("contract workflow id must not be empty");
        }
        for packet in workflow.get("packetTypes").and_then(Value::as_array).into_iter().flatten() {
            let packet = packet.as_str().ok_or_else(|| anyhow::anyhow!("contract workflow packetTypes must be strings"))?;
            if !packet.contains('.') {
                bail!("contract workflow packet type must be namespaced: {packet}");
            }
            packet_types.insert(packet.to_string());
        }
    }
    for route in obj.get("routes").and_then(Value::as_array).into_iter().flatten() {
        let source = route.get("source").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("route source packet type is required"))?;
        let target = route.get("target").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("route target is required"))?;
        if !packet_types.contains(source) {
            bail!("route references unknown packet type: {source}");
        }
        if let Some(role) = target.strip_prefix("role:").or_else(|| target.strip_prefix("subagent:")) {
            if !role_ids.contains(role) {
                bail!("route references unknown target role: {role}");
            }
        } else if target != "owner" && target != "source" && target != "system" {
            bail!("route target must be owner, source, system, role:<id>, or subagent:<role>");
        }
    }
    for policy in obj.get("lifecyclePolicies").and_then(Value::as_array).into_iter().flatten() {
        let name = policy.get("name").or_else(|| policy.get("id")).and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("lifecycle policy name is required"))?;
        if name.trim().is_empty() {
            bail!("lifecycle policy name must not be empty");
        }
        for key in ["releaseLeases", "closeSubagents", "terminateProcesses", "revokeGodMode"] {
            if policy.get(key).is_some() && !policy.get(key).and_then(Value::as_bool).is_some() {
                bail!("lifecycle policy {key} must be boolean");
            }
        }
    }
    Ok(())
}

pub fn compile_project_runtime_source(source: &str) -> Result<Value> {
    validate_hook_source(source)?;
    let ast = AstModule::parse("project_runtime.star", source.to_string(), &Dialect::Standard)
        .map_err(|error| anyhow::anyhow!("project runtime source is invalid Starlark: {error}"))?;
    let collector = ConstructorCollector::default();
    let globals = GlobalsBuilder::standard().with(project_runtime_constructor_builtins).build();
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.set_max_callstack_size(EVALUATION_MAX_CALLSTACK)?;
    eval.extra = Some(&collector);
    eval.eval_module(ast, &globals)
        .map_err(|error| anyhow::anyhow!("project runtime constructor evaluation failed: {error}"))?;
    drop(eval);
    drop(module);
    let manifest = collector.into_manifest();
    validate_runtime_manifest(&manifest)?;
    Ok(manifest)
}

#[derive(Default, ProvidesStaticType)]
struct ConstructorCollector {
    roles: RefCell<Vec<Value>>,
    role_tool_bundles: RefCell<Vec<Value>>,
    command_bundle_bindings: RefCell<Vec<Value>>,
    channels: RefCell<Vec<Value>>,
    routes: RefCell<Vec<Value>>,
    hooks: RefCell<Vec<Value>>,
    contract_workflows: RefCell<Vec<Value>>,
    resource_types: RefCell<Vec<Value>>,
    steward_bindings: RefCell<Vec<Value>>,
    lifecycle_policies: RefCell<Vec<Value>>,
}

impl ConstructorCollector {
    fn push(&self, kind: &str, kwargs: SmallMap<String, StarlarkValue<'_>>) -> Result<()> {
        let mut object = serde_json::Map::new();
        for (key, value) in kwargs {
            object.insert(starlark_key_to_camel(&key), starlark_value_to_json(value)?);
        }
        let value = Value::Object(object);
        match kind {
            "role_definition" => self.roles.borrow_mut().push(value),
            "role_tool_bundle" => self.role_tool_bundles.borrow_mut().push(value),
            "command_bundle_binding" => self.command_bundle_bindings.borrow_mut().push(value),
            "channel" => self.channels.borrow_mut().push(value),
            "route" => self.routes.borrow_mut().push(value),
            "hook_binding" => self.hooks.borrow_mut().push(value),
            "contract_workflow" => self.contract_workflows.borrow_mut().push(value),
            "resource_type" => self.resource_types.borrow_mut().push(value),
            "steward_binding" => self.steward_bindings.borrow_mut().push(value),
            "lifecycle_policy" => self.lifecycle_policies.borrow_mut().push(value),
            other => bail!("unknown project runtime constructor: {other}"),
        }
        Ok(())
    }

    fn into_manifest(self) -> Value {
        json!({
            "roles": self.roles.into_inner(),
            "roleToolBundles": self.role_tool_bundles.into_inner(),
            "commandBundleBindings": self.command_bundle_bindings.into_inner(),
            "channels": self.channels.into_inner(),
            "routes": self.routes.into_inner(),
            "hooks": self.hooks.into_inner(),
            "contractWorkflows": self.contract_workflows.into_inner(),
            "resourceTypes": self.resource_types.into_inner(),
            "stewardBindings": self.steward_bindings.into_inner(),
            "lifecyclePolicies": self.lifecycle_policies.into_inner(),
        })
    }
}

fn constructor_collector<'v, 'a>(eval: &Evaluator<'v, 'a, '_>) -> &'a ConstructorCollector {
    eval.extra
        .expect("ConstructorCollector must be installed in Evaluator.extra")
        .downcast_ref::<ConstructorCollector>()
        .expect("Evaluator.extra must be ConstructorCollector")
}

#[starlark_module]
fn project_runtime_constructor_builtins(builder: &mut GlobalsBuilder) {
    fn role_definition<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        constructor_collector(eval).push("role_definition", kwargs)?;
        Ok(NoneType)
    }

    fn role_tool_bundle<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        constructor_collector(eval).push("role_tool_bundle", kwargs)?;
        Ok(NoneType)
    }

    fn command_bundle_binding<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        constructor_collector(eval).push("command_bundle_binding", kwargs)?;
        Ok(NoneType)
    }

    fn channel<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        constructor_collector(eval).push("channel", kwargs)?;
        Ok(NoneType)
    }

    fn route<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        constructor_collector(eval).push("route", kwargs)?;
        Ok(NoneType)
    }

    fn hook_binding<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        constructor_collector(eval).push("hook_binding", kwargs)?;
        Ok(NoneType)
    }

    fn contract_workflow<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        constructor_collector(eval).push("contract_workflow", kwargs)?;
        Ok(NoneType)
    }

    fn resource_type<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        constructor_collector(eval).push("resource_type", kwargs)?;
        Ok(NoneType)
    }

    fn steward_binding<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        constructor_collector(eval).push("steward_binding", kwargs)?;
        Ok(NoneType)
    }

    fn lifecycle_policy<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        constructor_collector(eval).push("lifecycle_policy", kwargs)?;
        Ok(NoneType)
    }
}

fn starlark_key_to_camel(key: &str) -> String {
    let mut output = String::new();
    let mut uppercase = false;
    for ch in key.chars() {
        if ch == '_' {
            uppercase = true;
        } else if uppercase {
            output.extend(ch.to_uppercase());
            uppercase = false;
        } else {
            output.push(ch);
        }
    }
    output
}

fn starlark_value_to_json(value: StarlarkValue<'_>) -> Result<Value> {
    if let Some(text) = value.unpack_str() {
        return Ok(Value::String(text.to_string()));
    }
    if let Some(flag) = value.unpack_bool() {
        return Ok(Value::Bool(flag));
    }
    if let Some(number) = value.unpack_i32() {
        return Ok(json!(number));
    }
    let rendered = value.to_string();
    let jsonish = rendered
        .replace("True", "true")
        .replace("False", "false")
        .replace("None", "null");
    serde_json::from_str(&jsonish).map_err(|error| anyhow::anyhow!("constructor value must be JSON-compatible: {rendered}: {error}"))
}

fn json_to_starlark_literal(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string()),
        Value::Array(items) => format!("[{}]", items.iter().map(json_to_starlark_literal).collect::<Vec<_>>().join(", ")),
        Value::Object(object) => {
            let pairs = object
                .iter()
                .map(|(key, value)| format!("{}: {}", serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string()), json_to_starlark_literal(value)))
                .collect::<Vec<_>>();
            format!("{{{}}}", pairs.join(", "))
        }
    }
}

fn reject_unbounded_or_recursive_starlark(source: &str) -> Result<()> {
    let mut statement_budget = 0usize;
    let mut function_names = Vec::new();
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        statement_budget += 1;
        if statement_budget as u64 > EVALUATION_FUEL_STEPS {
            bail!("hook source exceeds deterministic statement budget");
        }
        if line.starts_with("while ") || line.starts_with("while:") {
            bail!("unbounded while loops are rejected");
        }
        if let Some(range_arg) = line.split("range(").nth(1).and_then(|tail| tail.split(')').next()) {
            if let Ok(limit) = range_arg.trim().parse::<u64>() {
                if limit > EVALUATION_FUEL_STEPS {
                    bail!("hook source range exceeds deterministic fuel budget");
                }
            }
        }
        if let Some(rest) = line.strip_prefix("def ") {
            if let Some((name, _)) = rest.split_once('(') {
                function_names.push(name.trim().to_string());
            }
        }
    }
    for name in function_names {
        let call = format!("{name}(");
        let recursive_call = source.lines().map(str::trim).any(|line| {
            !line.starts_with("def ")
                && (line.starts_with(&call) || line.contains(&format!(" return {call}")) || line.contains(&format!("= {call}")))
                || line.starts_with(&format!("return {call}"))
        });
        if recursive_call {
            bail!("recursive hook functions are rejected: {name}");
        }
    }
    Ok(())
}

pub fn validate_intent(boundary: LifecycleHook, intent: &HookIntent) -> Result<HookIntentType> {
    let kind = HookIntentType::parse(&intent.intent_type)?;
    if !allowed_intents(boundary).contains(&kind) {
        bail!("intent {} is not allowed at {}", intent.intent_type, boundary.as_str());
    }
    let size = serde_json::to_vec(intent)?.len();
    if size > MAX_INTENT_BYTES {
        bail!("intent {} exceeds {} bytes", intent.intent_type, MAX_INTENT_BYTES);
    }
    match kind {
        HookIntentType::RequireOutputSchema => {
            required_string(&intent.payload, "schemaName")?;
            let packet_type = required_string(&intent.payload, "packetType")?;
            if intent.payload.get("disableSystemSchemas").and_then(Value::as_bool).unwrap_or(false)
                || intent.payload.get("weakenActiveContract").and_then(Value::as_bool).unwrap_or(false)
                || intent.payload.get("hideOwnerRequirements").and_then(Value::as_bool).unwrap_or(false)
                || packet_type.starts_with("system.")
            {
                bail!("hook schema intent cannot disable system schemas, weaken active contracts, or hide owner requirements");
            }
            if intent.payload.get("schema").is_none() {
                bail!("require_output_schema requires schema");
            }
        }
        HookIntentType::RecordPacket => {
            let packet_type = required_string(&intent.payload, "packetType")?;
            if packet_type.starts_with("god_mode.") || intent.payload.get("godMode").is_some() {
                bail!("Starlark hooks cannot grant, revoke, or bypass God Mode");
            }
        }
        HookIntentType::RoutePacket => {
            required_string(&intent.payload, "packetId")?;
        }
        HookIntentType::EnsureSubagent => {
            required_string(&intent.payload, "subagentKey")?;
            required_string(&intent.payload, "subagentKind")?;
            required_string(&intent.payload, "roleId")?;
            required_string(&intent.payload, "workflowIdentity")?;
        }
        HookIntentType::CloseSubagent => {
            required_string(&intent.payload, "subagentKey")?;
            required_string(&intent.payload, "workflowIdentity")?;
        }
        HookIntentType::ReserveResource | HookIntentType::ReleaseResource => {
            required_string(&intent.payload, "resourceType")?;
        }
        HookIntentType::AddTurnObligation => {
            required_string(&intent.payload, "obligationType")?;
        }
        HookIntentType::UpdateContractProgress => {
            required_string(&intent.payload, "contractId")?;
            required_string(&intent.payload, "progressKey")?;
            required_string(&intent.payload, "status")?;
        }
        HookIntentType::RequestOwnerApproval | HookIntentType::BlockWithReason | HookIntentType::NotifySession => {
            required_string(&intent.payload, "message")?;
        }
    }
    let serialized_payload = serde_json::to_string(&intent.payload)?;
    for forbidden in ["grantGodMode", "revokeGodMode", "bypassGodMode", "godModeGrant", "godModeRevoke"] {
        if serialized_payload.contains(forbidden) {
            bail!("Starlark hooks cannot grant, revoke, or bypass God Mode");
        }
    }
    Ok(kind)
}

fn context_bool(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn visible_command_action_ids(context: &HookContext) -> BTreeSet<String> {
    context
        .visible_command_summaries
        .iter()
        .filter_map(|summary| {
            summary
                .get("actionId")
                .or_else(|| summary.get("action_id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

fn validate_intent_against_context(context: &HookContext, kind: HookIntentType, intent: &HookIntent) -> Result<()> {
    let payload_text = serde_json::to_string(&intent.payload)?;
    if payload_text.contains("approvalBypass") || payload_text.contains("bypassApproval") {
        bail!("Starlark hooks cannot bypass approval policy");
    }
    if context_bool(&intent.payload, "ownerGrantRequired") && !context_bool(&context.routing_state, "ownerGrantApproved") {
        bail!("hook intent requires an explicit owner grant that is not active in the bounded context");
    }
    let action_id = intent
        .payload
        .get("commandActionId")
        .or_else(|| intent.payload.get("requiresCommandActionId"))
        .or_else(|| intent.payload.get("actionId"))
        .and_then(Value::as_str);
    if let Some(action_id) = action_id {
        let visible_actions = visible_command_action_ids(context);
        if !visible_actions.contains(action_id) {
            bail!("hook intent references command action outside session role/project visibility: {action_id}");
        }
    }
    if matches!(kind, HookIntentType::RequestOwnerApproval) && context_bool(&intent.payload, "grantImmediately") {
        bail!("hook owner-approval intents can request review but cannot mint grants");
    }
    Ok(())
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    let Some(text) = value.get(field).and_then(Value::as_str) else {
        bail!("{field} is required");
    };
    if text.trim().is_empty() {
        bail!("{field} must not be empty");
    }
    Ok(text)
}

pub fn stable_intent_key(hook_source_hash: &str, lifecycle_event_id: Uuid, session_id: Option<Uuid>, intent: &HookIntent) -> String {
    let mut hasher = Sha256::new();
    hasher.update(hook_source_hash.as_bytes());
    hasher.update(lifecycle_event_id.as_bytes());
    if let Some(id) = session_id {
        hasher.update(id.as_bytes());
    }
    hasher.update(intent.intent_type.as_bytes());
    if let Some(key) = intent.key.as_deref() {
        hasher.update(key.as_bytes());
    }
    if let Some(packet_type) = intent.payload.get("packetType").and_then(Value::as_str) {
        hasher.update(packet_type.as_bytes());
    }
    if let Some(subagent_key) = intent.payload.get("subagentKey").and_then(Value::as_str) {
        hasher.update(subagent_key.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

pub fn validate_context_bounded(context: &HookContext) -> Result<String> {
    let bytes = serde_json::to_vec(context)?;
    if bytes.len() > MAX_CONTEXT_BYTES {
        bail!("hook context exceeds {} bytes", MAX_CONTEXT_BYTES);
    }
    let text = String::from_utf8_lossy(&bytes);
    for forbidden in ["fullStdout", "fullStderr", "shellStdout", "shellStderr", "secret", "authToken", "apiKey", "unboundedChatHistory"] {
        if text.contains(forbidden) {
            bail!("hook context contains forbidden field: {forbidden}");
        }
    }
    Ok(source_hash(&text))
}

pub fn evaluate_returned_intents(
    boundary: LifecycleHook,
    hook_source_hash: &str,
    lifecycle_event_id: Uuid,
    session_id: Option<Uuid>,
    context: &HookContext,
    returned: Vec<HookIntent>,
) -> HookEvaluationResult {
    let started = Instant::now();
    let mut errors = Vec::new();
    let context_hash = match validate_context_bounded(context) {
        Ok(hash) => hash,
        Err(error) => {
            errors.push(error.to_string());
            source_hash("{}")
        }
    };
    if returned.len() > MAX_RETURNED_INTENTS {
        errors.push(format!("hook returned more than {MAX_RETURNED_INTENTS} intents"));
    }
    let mut seen = BTreeSet::new();
    let mut valid = Vec::new();
    for mut intent in returned.into_iter().take(MAX_RETURNED_INTENTS) {
        match validate_intent(boundary, &intent).and_then(|kind| validate_intent_against_context(context, kind, &intent)) {
            Ok(_) => {
                let key = stable_intent_key(hook_source_hash, lifecycle_event_id, session_id, &intent);
                if !seen.insert(key.clone()) {
                    errors.push(format!("duplicate hook intent idempotency key: {key}"));
                    continue;
                }
                intent.idempotency_key = Some(key);
                valid.push(intent);
            }
            Err(error) => errors.push(error.to_string()),
        }
    }
    HookEvaluationResult {
        lifecycle_event_id,
        validation_status: if errors.is_empty() { "valid".to_string() } else { "invalid".to_string() },
        returned_intents: if errors.is_empty() { valid } else { Vec::new() },
        errors,
        context_hash,
        timing_metadata: json!({
            "elapsedMs": started.elapsed().as_millis(),
            "timeoutMs": EVALUATION_TIMEOUT_MS,
            "fuelSteps": EVALUATION_FUEL_STEPS,
        }),
    }
}

pub fn evaluate_starlark_hook_program(
    boundary: LifecycleHook,
    hook_source_hash: &str,
    lifecycle_event_id: Uuid,
    session_id: Option<Uuid>,
    context: &HookContext,
    source: &str,
) -> HookEvaluationResult {
    let started = Instant::now();
    let context_hash = match validate_context_bounded(context) {
        Ok(hash) => hash,
        Err(error) => {
            return HookEvaluationResult {
                lifecycle_event_id,
                validation_status: "invalid".to_string(),
                returned_intents: Vec::new(),
                errors: vec![error.to_string()],
                context_hash: source_hash("{}"),
                timing_metadata: json!({"elapsedMs":0,"timeoutMs":EVALUATION_TIMEOUT_MS,"fuelSteps":EVALUATION_FUEL_STEPS}),
            };
        }
    };
    let mut errors = Vec::new();
    let returned = match run_starlark_hook_source(source, context) {
        Ok(intents) => intents,
        Err(error) => {
            errors.push(error.to_string());
            Vec::new()
        }
    };
    let elapsed_ms = started.elapsed().as_millis();
    if elapsed_ms > EVALUATION_TIMEOUT_MS as u128 {
        errors.push(format!("hook evaluation exceeded deterministic timeout: {elapsed_ms}ms > {EVALUATION_TIMEOUT_MS}ms"));
    }
    if !errors.is_empty() {
        return HookEvaluationResult {
            lifecycle_event_id,
            validation_status: "invalid".to_string(),
            returned_intents: Vec::new(),
            errors,
            context_hash,
            timing_metadata: json!({"elapsedMs":elapsed_ms,"timeoutMs":EVALUATION_TIMEOUT_MS,"fuelSteps":EVALUATION_FUEL_STEPS}),
        };
    }
    let mut result = evaluate_returned_intents(boundary, hook_source_hash, lifecycle_event_id, session_id, context, returned);
    result.timing_metadata = json!({"elapsedMs":elapsed_ms,"timeoutMs":EVALUATION_TIMEOUT_MS,"fuelSteps":EVALUATION_FUEL_STEPS});
    result
}

fn run_starlark_hook_source(source: &str, context: &HookContext) -> Result<Vec<HookIntent>> {
    validate_hook_source(source)?;
    let context_literal = json_to_starlark_literal(&serde_json::to_value(context)?);
    let wrapped = format!("{source}\nrobdex_result = \"\\n\".join(hook({context_literal}))\n");
    let ast = AstModule::parse("project_lifecycle_hook.star", wrapped, &Dialect::Standard)
        .map_err(|error| anyhow::anyhow!("hook source is invalid Starlark: {error}"))?;
    let globals = GlobalsBuilder::standard().with(hook_intent_builtins).build();
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.set_max_callstack_size(EVALUATION_MAX_CALLSTACK)?;
    eval.eval_module(ast, &globals)
        .map_err(|error| anyhow::anyhow!("hook evaluation failed: {error}"))?;
    drop(eval);
    let text = module
        .get("robdex_result")
        .and_then(|value| value.unpack_str().map(ToString::to_string))
        .ok_or_else(|| anyhow::anyhow!("hook did not produce a string result list"))?;
    if text.len() > MAX_RETURNED_INTENTS * MAX_INTENT_BYTES {
        bail!("hook output exceeds deterministic output bound");
    }
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    text.lines()
        .map(|line| serde_json::from_str::<HookIntent>(line).map_err(Into::into))
        .collect()
}

#[starlark_module]
fn hook_intent_builtins(builder: &mut GlobalsBuilder) {
    fn require_output_schema<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("require_output_schema", kwargs)?)
    }

    fn record_packet<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("record_packet", kwargs)?)
    }

    fn route_packet<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("route_packet", kwargs)?)
    }

    fn notify_session<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("notify_session", kwargs)?)
    }

    fn ensure_subagent<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("ensure_subagent", kwargs)?)
    }

    fn close_subagent<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("close_subagent", kwargs)?)
    }

    fn reserve_resource<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("reserve_resource", kwargs)?)
    }

    fn release_resource<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("release_resource", kwargs)?)
    }

    fn add_turn_obligation<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("add_turn_obligation", kwargs)?)
    }

    fn update_contract_progress<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("update_contract_progress", kwargs)?)
    }

    fn request_owner_approval<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("request_owner_approval", kwargs)?)
    }

    fn block_with_reason<'v>(#[starlark(kwargs)] kwargs: SmallMap<String, StarlarkValue<'v>>) -> anyhow::Result<String> {
        Ok(hook_intent_json("block_with_reason", kwargs)?)
    }
}

fn hook_intent_json(intent_type: &str, kwargs: SmallMap<String, StarlarkValue<'_>>) -> Result<String> {
    let mut payload = serde_json::Map::new();
    let mut key = None;
    for (field, value) in kwargs {
        if field == "key" {
            key = value.unpack_str().map(ToString::to_string);
        } else if field == "packet_type" {
            payload.insert("packetType".to_string(), starlark_value_to_json(value)?);
        } else if field == "schema_name" {
            payload.insert("schemaName".to_string(), starlark_value_to_json(value)?);
        } else if field == "contract_id" {
            payload.insert("contractId".to_string(), starlark_value_to_json(value)?);
        } else if field == "progress_key" {
            payload.insert("progressKey".to_string(), starlark_value_to_json(value)?);
        } else if field == "subagent_key" || field == "kind" {
            let target = if field == "kind" { "subagentKind".to_string() } else { "subagentKey".to_string() };
            payload.insert(target, starlark_value_to_json(value)?);
        } else if field == "role_id" {
            payload.insert("roleId".to_string(), starlark_value_to_json(value)?);
        } else if field == "workflow_identity" {
            payload.insert("workflowIdentity".to_string(), starlark_value_to_json(value)?);
        } else if field == "resource_type" {
            payload.insert("resourceType".to_string(), starlark_value_to_json(value)?);
        } else if field == "packet_id" {
            payload.insert("packetId".to_string(), starlark_value_to_json(value)?);
        } else {
            payload.insert(starlark_key_to_camel(&field), starlark_value_to_json(value)?);
        }
    }
    serde_json::to_string(&HookIntent { intent_type: intent_type.to_string(), key, payload: Value::Object(payload), idempotency_key: None }).map_err(Into::into)
}

pub async fn persist_project_runtime_config(
    pool: &PgPool,
    project_key: &str,
    source_text: &str,
    manifest: Value,
    author: &str,
) -> Result<Uuid> {
    validate_hook_source(source_text)?;
    validate_runtime_manifest(&manifest)?;
    let id = Uuid::new_v4();
    let hash = source_hash(source_text);
    sqlx::query(
        "INSERT INTO project_runtime_config_versions (id, project_key, source_text, source_hash, compiled_manifest, scope, activation_status, author, validation_packet) VALUES ($1,$2,$3,$4,$5,'project','draft',$6,$7) ON CONFLICT (project_key, source_hash) DO UPDATE SET validation_packet=$7 RETURNING id",
    )
    .bind(id)
    .bind(project_key)
    .bind(source_text)
    .bind(hash)
    .bind(&manifest)
    .bind(author)
    .bind(json!({"valid": true}))
    .fetch_one(pool)
    .await?
    .try_get("id")
    .map_err(Into::into)
}

pub async fn activate_project_runtime_config(pool: &PgPool, project_key: &str, config_version_id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query("SELECT compiled_manifest FROM project_runtime_config_versions WHERE id=$1 AND project_key=$2")
        .bind(config_version_id)
        .bind(project_key)
        .fetch_one(&mut *tx)
        .await?;
    let manifest: Value = row.get("compiled_manifest");
    sqlx::query("UPDATE project_runtime_config_versions SET activation_status='archived', archived_at=COALESCE(archived_at, now()) WHERE project_key=$1 AND activation_status='active'")
        .bind(project_key)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE project_runtime_config_versions SET activation_status='active', activated_at=now() WHERE id=$1")
        .bind(config_version_id)
        .execute(&mut *tx)
        .await?;
    for hook in manifest.get("hooks").and_then(Value::as_array).into_iter().flatten() {
        let name = hook.get("name").and_then(Value::as_str).unwrap_or_default();
        let source = hook.get("source").and_then(Value::as_str).unwrap_or_default();
        let hook_id = Uuid::new_v4();
        sqlx::query("INSERT INTO project_runtime_hook_bindings (id, project_key, config_version_id, lifecycle_hook, hook_source, hook_source_hash, status, activated_at) VALUES ($1,$2,$3,$4,$5,$6,'active',now()) ON CONFLICT (project_key, config_version_id, lifecycle_hook) DO UPDATE SET hook_source=$5, hook_source_hash=$6, status='active', activated_at=now()")
            .bind(hook_id)
            .bind(project_key)
            .bind(config_version_id)
            .bind(name)
            .bind(source)
            .bind(source_hash(source))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    db::append_admin_event(pool, "project_runtime", Some(config_version_id), "projectRuntime.activated", Some("active"), json!({"projectKey": project_key})).await?;
    Ok(())
}

pub async fn record_lifecycle_event(pool: &PgPool, project_key: Option<&str>, session_id: Option<Uuid>, turn_id: Option<Uuid>, hook: LifecycleHook, payload: Value) -> Result<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO lifecycle_events (id, project_key, session_id, turn_id, lifecycle_hook, payload) VALUES ($1,$2,$3,$4,$5,$6)")
        .bind(id)
        .bind(project_key)
        .bind(session_id)
        .bind(turn_id)
        .bind(hook.as_str())
        .bind(payload)
        .execute(pool)
        .await?;
    Ok(id)
}

pub async fn persist_hook_evaluation(pool: &PgPool, hook_binding_id: Option<Uuid>, hook_version_id: Option<Uuid>, session_id: Option<Uuid>, turn_id: Option<Uuid>, result: &HookEvaluationResult) -> Result<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO hook_evaluations (id, hook_binding_id, hook_version_id, lifecycle_event_id, session_id, turn_id, input_context_hash, returned_intents, validation_status, applied_intent_ids, errors, timing_metadata) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'[]'::jsonb,$10,$11)")
        .bind(id)
        .bind(hook_binding_id)
        .bind(hook_version_id)
        .bind(result.lifecycle_event_id)
        .bind(session_id)
        .bind(turn_id)
        .bind(&result.context_hash)
        .bind(serde_json::to_value(&result.returned_intents)?)
        .bind(&result.validation_status)
        .bind(json!(result.errors))
        .bind(&result.timing_metadata)
        .execute(pool)
        .await?;
    Ok(id)
}

pub async fn evaluate_and_apply_lifecycle_intents(
    pool: &PgPool,
    project_key: Option<&str>,
    session_id: Option<Uuid>,
    turn_id: Option<Uuid>,
    boundary: LifecycleHook,
    context: &HookContext,
    hook_version_id: Option<Uuid>,
    hook_source_hash: &str,
    returned: Vec<HookIntent>,
) -> Result<HookEvaluationResult> {
    let event_id = record_lifecycle_event(pool, project_key, session_id, turn_id, boundary, json!({"source":"rust.lifecycle_hooks"})).await?;
    let result = evaluate_returned_intents(boundary, hook_source_hash, event_id, session_id, context, returned);
    let evaluation_id = persist_hook_evaluation(pool, None, hook_version_id, session_id, turn_id, &result).await?;
    if result.validation_status != "valid" {
        db::append_admin_event(pool, "hook_evaluation", Some(evaluation_id), "hookEvaluation.failedClosed", Some("invalid"), json!({"lifecycleEventId": event_id, "errors": result.errors})).await?;
        return Ok(result);
    }
    let applied = apply_hook_intents_with_version(pool, project_key, session_id, turn_id, boundary, hook_version_id, &result.returned_intents).await?;
    sqlx::query("UPDATE hook_evaluations SET applied_intent_ids=$2 WHERE id=$1")
        .bind(evaluation_id)
        .bind(json!(applied))
        .execute(pool)
        .await?;
    Ok(result)
}

pub async fn evaluate_active_lifecycle_hooks(
    pool: &PgPool,
    project_key: &str,
    session_id: Option<Uuid>,
    turn_id: Option<Uuid>,
    boundary: LifecycleHook,
    context: &HookContext,
) -> Result<Vec<HookEvaluationResult>> {
    let rows = if let Some(session_id) = session_id {
        sqlx::query(
            r#"
            SELECT h.id, h.config_version_id, h.hook_source, h.hook_source_hash
            FROM sessions s
            JOIN project_runtime_hook_bindings h ON h.id = ((s.active_hook_bindings ->> $2)::uuid)
            WHERE s.id=$1 AND h.lifecycle_hook=$2 AND h.status='active'
            "#,
        )
        .bind(session_id)
        .bind(boundary.as_str())
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT h.id, h.config_version_id, h.hook_source, h.hook_source_hash
            FROM project_runtime_config_versions v
            JOIN project_runtime_hook_bindings h ON h.config_version_id=v.id
            WHERE v.project_key=$1 AND v.activation_status='active' AND h.lifecycle_hook=$2 AND h.status='active'
            ORDER BY h.activated_at DESC
            "#,
        )
        .bind(project_key)
        .bind(boundary.as_str())
        .fetch_all(pool)
        .await?
    };
    let mut results = Vec::new();
    for row in rows {
        let binding_id: Uuid = row.get("id");
        let version_id: Uuid = row.get("config_version_id");
        let hook_source: String = row.get("hook_source");
        let hook_hash: String = row.get("hook_source_hash");
        let event_id = record_lifecycle_event(pool, Some(project_key), session_id, turn_id, boundary, json!({"source":"starlark.active_hook","hookBindingId":binding_id,"hookVersionId":version_id})).await?;
        let result = evaluate_starlark_hook_program(boundary, &hook_hash, event_id, session_id, context, &hook_source);
        let evaluation_id = persist_hook_evaluation(pool, Some(binding_id), Some(version_id), session_id, turn_id, &result).await?;
        if result.validation_status == "valid" {
            let applied = apply_hook_intents_with_version(pool, Some(project_key), session_id, turn_id, boundary, Some(version_id), &result.returned_intents).await?;
            sqlx::query("UPDATE hook_evaluations SET applied_intent_ids=$2 WHERE id=$1")
                .bind(evaluation_id)
                .bind(json!(applied))
                .execute(pool)
                .await?;
        } else {
            db::append_admin_event(pool, "hook_evaluation", Some(evaluation_id), "hookEvaluation.failedClosed", Some("invalid"), json!({"lifecycleEventId": event_id, "errors": result.errors})).await?;
        }
        results.push(result);
    }
    Ok(results)
}

pub async fn apply_hook_intents(
    pool: &PgPool,
    project_key: Option<&str>,
    session_id: Option<Uuid>,
    turn_id: Option<Uuid>,
    boundary: LifecycleHook,
    intents: &[HookIntent],
) -> Result<Vec<Uuid>> {
    apply_hook_intents_with_version(pool, project_key, session_id, turn_id, boundary, None, intents).await
}

async fn apply_hook_intents_with_version(
    pool: &PgPool,
    project_key: Option<&str>,
    session_id: Option<Uuid>,
    turn_id: Option<Uuid>,
    boundary: LifecycleHook,
    hook_version_id: Option<Uuid>,
    intents: &[HookIntent],
) -> Result<Vec<Uuid>> {
    let mut applied = Vec::new();
    for intent in intents {
        let kind = validate_intent(boundary, intent)?;
        let key = intent.idempotency_key.clone().unwrap_or_else(|| stable_intent_key("manual", Uuid::nil(), session_id, intent));
        let id = match kind {
            HookIntentType::RequireOutputSchema => {
                record_schema_evidence(pool, hook_version_id, None, required_string(&intent.payload, "packetType")?, required_string(&intent.payload, "schemaName")?, intent.payload.get("schema").unwrap_or(&Value::Null), boundary, intent.payload.get("modelRequestId").and_then(Value::as_str).map(Uuid::parse_str).transpose()?).await?
            }
            HookIntentType::RecordPacket => {
                record_runtime_packet(
                    pool,
                    project_key,
                    session_id,
                    None,
                    turn_id,
                    required_string(&intent.payload, "packetType")?,
                    intent.payload.get("status").and_then(Value::as_str).unwrap_or("recorded"),
                    intent.payload.get("payload").cloned().unwrap_or_else(|| intent.payload.clone()),
                    intent.payload.get("validationError").and_then(Value::as_str),
                    json!({"source":"hook","boundary":boundary.as_str()}),
                    &key,
                ).await?
            }
            HookIntentType::RoutePacket => {
                let packet_id = Uuid::parse_str(required_string(&intent.payload, "packetId")?)?;
                route_packet_envelope(
                    pool,
                    packet_id,
                    "hookRoute",
                    session_id,
                    intent.payload.get("targetSessionId").and_then(Value::as_str).map(Uuid::parse_str).transpose()?,
                    intent.payload.get("targetRoleId").and_then(Value::as_str),
                    "pending",
                    json!({"target": intent.payload.get("target"), "boundary": boundary.as_str()}),
                ).await?
            }
            HookIntentType::NotifySession => {
                db::append_event(pool, session_id.unwrap_or(Uuid::nil()), turn_id, "hook_notice", None, "hook.notifySession", Some("notice"), json!({"message": required_string(&intent.payload, "message")?, "boundary": boundary.as_str()})).await?;
                Uuid::new_v4()
            }
            HookIntentType::EnsureSubagent => {
                ensure_subagent(
                    pool,
                    session_id.ok_or_else(|| anyhow::anyhow!("ensure_subagent requires session_id"))?,
                    required_string(&intent.payload, "subagentKey")?,
                    required_string(&intent.payload, "workflowIdentity")?,
                    required_string(&intent.payload, "subagentKind")?,
                    required_string(&intent.payload, "roleId")?,
                    intent.payload.get("workspacePolicy").cloned().unwrap_or_else(|| json!({})),
                    json!({"source":"hook","idempotencyKey": key}),
                ).await?
            }
            HookIntentType::CloseSubagent => {
                close_subagent(
                    pool,
                    session_id.ok_or_else(|| anyhow::anyhow!("close_subagent requires session_id"))?,
                    required_string(&intent.payload, "subagentKey")?,
                    required_string(&intent.payload, "workflowIdentity")?,
                ).await?.unwrap_or_else(Uuid::nil)
            }
            HookIntentType::ReserveResource => {
                reserve_resource(pool, session_id, intent.payload.clone(), &key).await?
            }
            HookIntentType::ReleaseResource => {
                release_resource(pool, session_id, intent.payload.clone()).await?.unwrap_or_else(Uuid::nil)
            }
            HookIntentType::AddTurnObligation => {
                let id = Uuid::new_v4();
                sqlx::query("INSERT INTO turn_obligations (id, session_id, turn_id, obligation_type, status, payload, idempotency_key) VALUES ($1,$2,$3,$4,'pending',$5,$6) ON CONFLICT (session_id, idempotency_key) DO UPDATE SET payload=turn_obligations.payload RETURNING id")
                    .bind(id)
                    .bind(session_id.ok_or_else(|| anyhow::anyhow!("turn obligation requires session_id"))?)
                    .bind(turn_id)
                    .bind(required_string(&intent.payload, "obligationType")?)
                    .bind(&intent.payload)
                    .bind(&key)
                    .fetch_one(pool)
                    .await?
                    .get("id")
            }
            HookIntentType::UpdateContractProgress => {
                let contract_id = Uuid::parse_str(required_string(&intent.payload, "contractId")?)?;
                let id = Uuid::new_v4();
                sqlx::query("INSERT INTO generic_contract_progress (id, contract_id, progress_key, status, payload) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (contract_id, progress_key) DO UPDATE SET status=$4, payload=$5, updated_at=now() RETURNING id")
                    .bind(id)
                    .bind(contract_id)
                    .bind(required_string(&intent.payload, "progressKey")?)
                    .bind(required_string(&intent.payload, "status")?)
                    .bind(&intent.payload)
                    .fetch_one(pool)
                    .await?
                    .get("id")
            }
            HookIntentType::RequestOwnerApproval => {
                db::append_admin_event(pool, "hook_owner_approval", None, "hook.requestOwnerApproval", Some("requested"), json!({"message": required_string(&intent.payload, "message")?, "sessionId": session_id})).await?;
                Uuid::new_v4()
            }
            HookIntentType::BlockWithReason => {
                db::append_admin_event(pool, "hook_block", None, "hook.blocked", Some("blocked"), json!({"message": required_string(&intent.payload, "message")?, "sessionId": session_id})).await?;
                Uuid::new_v4()
            }
        };
        applied.push(id);
    }
    Ok(applied)
}

#[allow(clippy::too_many_arguments)]
pub async fn record_schema_evidence(
    pool: &PgPool,
    hook_version_id: Option<Uuid>,
    contract_id: Option<Uuid>,
    packet_type: &str,
    schema_name: &str,
    schema: &Value,
    lifecycle_boundary: LifecycleHook,
    model_request_id: Option<Uuid>,
) -> Result<Uuid> {
    let schema_hash = source_hash(&serde_json::to_string(schema)?);
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO structured_output_schema_evidence (id, hook_version_id, contract_id, packet_type, schema_name, schema_hash, schema, lifecycle_boundary, model_request_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(id)
        .bind(hook_version_id)
        .bind(contract_id)
        .bind(packet_type)
        .bind(schema_name)
        .bind(schema_hash)
        .bind(schema)
        .bind(lifecycle_boundary.as_str())
        .bind(model_request_id)
        .execute(pool)
        .await?;
    Ok(id)
}

pub async fn hook_required_schema_for_model_request(pool: &PgPool, model_request_id: Uuid) -> Result<Option<Value>> {
    let row = sqlx::query(
        r#"
        SELECT id, hook_version_id, contract_id, packet_type, schema_name, schema_hash, schema, lifecycle_boundary
        FROM structured_output_schema_evidence
        WHERE model_request_id=$1
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(model_request_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let evidence_id: Uuid = row.get("id");
    let hook_version_id: Option<Uuid> = row.get("hook_version_id");
    let contract_id: Option<Uuid> = row.get("contract_id");
    let packet_type: String = row.get("packet_type");
    let schema_name: String = row.get("schema_name");
    let schema_hash: String = row.get("schema_hash");
    let schema: Value = row.get("schema");
    let lifecycle_boundary: String = row.get("lifecycle_boundary");
    Ok(Some(json!({
        "name": schema_name,
        "schema": schema,
        "metadata": {
            "source": "hook_required_output_schema",
            "schemaEvidenceId": evidence_id,
            "hookVersionId": hook_version_id,
            "contractId": contract_id,
            "packetType": packet_type,
            "schemaName": schema_name,
            "schemaHash": schema_hash,
            "lifecycleBoundary": lifecycle_boundary,
            "modelRequestId": model_request_id
        }
    })))
}

pub fn apply_hook_required_schema_to_responses_body(body: &mut Value, schema: &Value) -> Result<()> {
    let name = required_string(schema, "name")?;
    let json_schema = schema.get("schema").cloned().unwrap_or_else(|| json!({"type":"object"}));
    body["text"] = json!({"format": {"type": "json_schema", "name": name, "schema": json_schema, "strict": true}});
    body["hook_schema_evidence"] = schema.get("metadata").cloned().unwrap_or_else(|| json!({}));
    Ok(())
}

pub async fn parse_structured_final_output_into_packet(
    pool: &PgPool,
    project_key: Option<&str>,
    source_session_id: Option<Uuid>,
    turn_id: Option<Uuid>,
    model_request_id: Uuid,
    final_output: &str,
    idempotency_key: &str,
) -> Result<Uuid> {
    let schema = hook_required_schema_for_model_request(pool, model_request_id).await?
        .ok_or_else(|| anyhow::anyhow!("no hook-required schema evidence for model request {model_request_id}"))?;
    let metadata = schema.get("metadata").cloned().unwrap_or_else(|| json!({}));
    let packet_type = required_string(&metadata, "packetType")?;
    let parsed: Value = match serde_json::from_str(final_output) {
        Ok(value) => value,
        Err(error) => {
            return record_invalid_structured_output(
                pool,
                project_key,
                source_session_id,
                turn_id,
                &packet_type,
                final_output,
                &error.to_string(),
                &format!("{idempotency_key}-invalid"),
            ).await;
        }
    };
    record_runtime_packet(
        pool,
        project_key,
        source_session_id,
        None,
        turn_id,
        &packet_type,
        "recorded",
        parsed,
        None,
        json!({"source":"hook_required_structured_output","schemaEvidence": metadata}),
        idempotency_key,
    ).await
}

pub async fn record_invalid_structured_output(
    pool: &PgPool,
    project_key: Option<&str>,
    source_session_id: Option<Uuid>,
    turn_id: Option<Uuid>,
    packet_type: &str,
    raw_output: &str,
    validation_error: &str,
    idempotency_key: &str,
) -> Result<Uuid> {
    let packet_id = record_runtime_packet(
        pool,
        project_key,
        source_session_id,
        None,
        turn_id,
        packet_type,
        "invalid",
        json!({"raw": raw_output}),
        Some(validation_error),
        json!({"source":"structured_output_parser"}),
        idempotency_key,
    ).await?;
    if let Some(session_id) = source_session_id {
        db::append_event(pool, session_id, turn_id, "runtime_packet", Some(packet_id), "structuredOutput.invalid", Some("invalid"), json!({"packetType": packet_type, "validationError": validation_error})).await?;
    }
    Ok(packet_id)
}

pub async fn request_project_runtime_config_change(
    pool: &PgPool,
    project_key: &str,
    session_id: Uuid,
    source_text: &str,
    manifest: Value,
    rationale: &str,
) -> Result<Uuid> {
    validate_hook_source(source_text)?;
    validate_runtime_manifest(&manifest)?;
    record_runtime_packet(
        pool,
        Some(project_key),
        Some(session_id),
        None,
        None,
        "project_runtime.config_change_request",
        "reviewable",
        json!({
            "projectKey": project_key,
            "sourceHash": source_hash(source_text),
            "sourceText": source_text,
            "manifest": manifest,
            "rationale": rationale
        }),
        None,
        json!({"requiresReview": true, "authority": "owner"}),
        &format!("project-runtime-config-request-{project_key}-{}-{}", source_hash(source_text), session_id),
    ).await
}

pub async fn set_session_hook_overrides(pool: &PgPool, session_id: Uuid, overrides: Value) -> Result<Value> {
    let current: Value = sqlx::query("SELECT active_hook_bindings FROM sessions WHERE id=$1")
        .bind(session_id)
        .fetch_one(pool)
        .await?
        .get("active_hook_bindings");
    let mut merged = current.as_object().cloned().unwrap_or_default();
    for (key, value) in overrides.as_object().cloned().unwrap_or_default() {
        LifecycleHook::parse(&key)?;
        merged.insert(key, value);
    }
    let merged = Value::Object(merged);
    sqlx::query("UPDATE sessions SET active_hook_bindings=$2, updated_at=now() WHERE id=$1")
        .bind(session_id)
        .bind(&merged)
        .execute(pool)
        .await?;
    Ok(merged)
}

pub async fn process_turn_completion_obligations(pool: &PgPool, session_id: Uuid, turn_id: Option<Uuid>) -> Result<usize> {
    let rows = sqlx::query("SELECT id, obligation_type, payload FROM turn_obligations WHERE session_id=$1 AND ($2::uuid IS NULL OR turn_id=$2) AND status='pending' ORDER BY created_at ASC")
        .bind(session_id)
        .bind(turn_id)
        .fetch_all(pool)
        .await?;
    let mut count = 0usize;
    for row in rows {
        let id: Uuid = row.get("id");
        let obligation_type: String = row.get("obligation_type");
        let payload: Value = row.get("payload");
        match obligation_type.as_str() {
            "releaseResource" => {
                let _ = release_resource(pool, Some(session_id), payload.clone()).await?;
            }
            "leaseIdleNotice" | "notifySession" => {
                db::append_event(pool, session_id, turn_id, "turn_obligation", Some(id), "turnObligation.notify", Some("completed"), json!({"payload": payload})).await?;
            }
            "markLeaseIdle" => {
                sqlx::query("UPDATE resource_leases SET status='idle', updated_at=now() WHERE owning_session_id=$1 AND status='assigned'")
                    .bind(session_id)
                    .execute(pool)
                    .await?;
            }
            _ => {
                db::append_event(pool, session_id, turn_id, "turn_obligation", Some(id), "turnObligation.completed", Some("completed"), json!({"obligationType": obligation_type, "payload": payload})).await?;
            }
        }
        sqlx::query("UPDATE turn_obligations SET status='completed', completed_at=now() WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await?;
        count += 1;
    }
    Ok(count)
}

pub fn packet_envelope_type_catalog() -> Vec<&'static str> {
    vec![
        "ordinary_human_message",
        "steering_input",
        "agent_message",
        "agent_request",
        "requirements_claim",
        "requirements_verdict",
        "approval_request",
        "approval_decision",
        "resource_request",
        "resource_response",
        "lifecycle_notice",
        "system_notice",
        "tooling_request",
    ]
}

pub async fn route_packet_envelope(
    pool: &PgPool,
    packet_id: Uuid,
    envelope_type: &str,
    source_session_id: Option<Uuid>,
    target_session_id: Option<Uuid>,
    target_role_id: Option<&str>,
    status: &str,
    delivery_metadata: Value,
) -> Result<Uuid> {
    if let Some(target) = target_session_id {
        let target_record = db::session_record(pool, target).await?;
        if target_record.status != "open" || target_record.closed_at.is_some() || target_record.archived_at.is_some() {
            bail!("routing target is not an open live session");
        }
        if target_record.hidden && target_record.parent_session_id != source_session_id {
            bail!("routing target hidden subagent is outside the source parent relationship");
        }
        if let Some(source) = source_session_id {
            let source_record = db::session_record(pool, source).await?;
            if target_record.project_key != source_record.project_key {
                bail!("routing target is outside the current project scope");
            }
        }
    }
    if let Some(role_id) = target_role_id {
        let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM roles r JOIN role_versions rv ON rv.id=r.current_version_id WHERE r.id=$1 LIMIT 1")
            .bind(role_id)
            .fetch_optional(pool)
            .await?;
        if exists.is_none() {
            bail!("routing target role is unknown");
        }
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO runtime_envelopes (id, packet_id, envelope_type, source_session_id, target_session_id, target_role_id, status, delivery_metadata) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(id)
        .bind(packet_id)
        .bind(envelope_type)
        .bind(source_session_id)
        .bind(target_session_id)
        .bind(target_role_id)
        .bind(status)
        .bind(delivery_metadata)
        .execute(pool)
        .await?;
    Ok(id)
}

pub async fn request_resource_lease(
    pool: &PgPool,
    project_key: &str,
    source_session_id: Uuid,
    resource_type: &str,
    steward_role_id: &str,
    request_payload: Value,
    idempotency_key: &str,
) -> Result<(Uuid, Uuid)> {
    let mut payload = request_payload.as_object().cloned().unwrap_or_default();
    payload.insert("resourceType".to_string(), Value::String(resource_type.to_string()));
    payload.insert("requestingSessionId".to_string(), Value::String(source_session_id.to_string()));
    let packet_id = record_runtime_packet(
        pool,
        Some(project_key),
        Some(source_session_id),
        None,
        None,
        "resource.request",
        "pending_steward_review",
        Value::Object(payload),
        None,
        json!({"source":"lease_request_affordance","targetRoleId":steward_role_id}),
        idempotency_key,
    ).await?;
    let envelope_id = route_packet_envelope(
        pool,
        packet_id,
        "resource_request",
        Some(source_session_id),
        None,
        Some(steward_role_id),
        "pending",
        json!({"resourceType":resource_type,"delivery":"steward_role"}),
    ).await?;
    Ok((packet_id, envelope_id))
}

pub async fn deliver_resource_lease_handle(
    pool: &PgPool,
    project_key: &str,
    lease_id: Uuid,
    idempotency_key: &str,
) -> Result<(Uuid, Uuid)> {
    let row = sqlx::query(
        "SELECT resource_type, resource_id, handle, owning_session_id, steward_session_id, steward_role_id, status FROM resource_leases WHERE id=$1",
    )
    .bind(lease_id)
    .fetch_one(pool)
    .await?;
    let owning_session_id: Option<Uuid> = row.get("owning_session_id");
    let Some(owning_session_id) = owning_session_id else {
        bail!("resource lease has no owning session");
    };
    let packet_id = record_runtime_packet(
        pool,
        Some(project_key),
        row.get::<Option<Uuid>, _>("steward_session_id"),
        None,
        None,
        "resource.lease_handle",
        "delivered",
        json!({
            "leaseId": lease_id,
            "resourceType": row.get::<String, _>("resource_type"),
            "resourceId": row.get::<Option<String>, _>("resource_id"),
            "handle": row.get::<Option<String>, _>("handle"),
            "status": row.get::<String, _>("status"),
            "stewardRoleId": row.get::<Option<String>, _>("steward_role_id")
        }),
        None,
        json!({"source":"resource_lease_workflow","delivery":"owning_session_handle"}),
        idempotency_key,
    ).await?;
    let envelope_id = route_packet_envelope(
        pool,
        packet_id,
        "resource_lease_handle",
        row.get::<Option<Uuid>, _>("steward_session_id"),
        Some(owning_session_id),
        None,
        "delivered",
        json!({"leaseId":lease_id,"delivery":"designer_worker_handle"}),
    ).await?;
    Ok((packet_id, envelope_id))
}

pub async fn record_runtime_packet(pool: &PgPool, project_key: Option<&str>, source_session_id: Option<Uuid>, parent_session_id: Option<Uuid>, turn_id: Option<Uuid>, packet_type: &str, status: &str, payload: Value, validation_error: Option<&str>, routing_metadata: Value, idempotency_key: &str) -> Result<Uuid> {
    let id = Uuid::new_v4();
    let row = sqlx::query("INSERT INTO runtime_packets (id, project_key, source_session_id, parent_session_id, turn_id, packet_type, status, payload, validation_error, routing_metadata, idempotency_key) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT (project_key, idempotency_key) DO UPDATE SET payload=runtime_packets.payload RETURNING id")
        .bind(id)
        .bind(project_key)
        .bind(source_session_id)
        .bind(parent_session_id)
        .bind(turn_id)
        .bind(packet_type)
        .bind(status)
        .bind(payload)
        .bind(validation_error)
        .bind(routing_metadata)
        .bind(idempotency_key)
        .fetch_one(pool)
        .await?;
    Ok(row.get("id"))
}

pub async fn ensure_subagent(pool: &PgPool, parent_session_id: Uuid, subagent_key: &str, workflow_identity: &str, subagent_kind: &str, role_id: &str, workspace_policy: Value, audit_metadata: Value) -> Result<Uuid> {
    let existing = sqlx::query("SELECT subagent_session_id FROM generic_subagents WHERE parent_session_id=$1 AND subagent_key=$2 AND workflow_identity=$3 AND lifecycle_status='open'")
        .bind(parent_session_id)
        .bind(subagent_key)
        .bind(workflow_identity)
        .fetch_optional(pool)
        .await?;
    if let Some(row) = existing {
        return Ok(row.get("subagent_session_id"));
    }
    let parent = db::session_record(pool, parent_session_id).await?;
    let mut role = db::current_role_snapshot(pool, role_id).await?;
    let parent_role = db::session_role_snapshot(pool, parent_session_id).await?;
    role.model_defaults.model = parent_role.model_defaults.model.clone();
    let session_id = db::new_session(
        pool,
        &role,
        parent.project_key.as_deref(),
        &parent.workdir,
        parent.worktree_root.as_deref(),
        Some(subagent_kind),
        Some(subagent_key),
    ).await?;
    sqlx::query("UPDATE sessions SET parent_session_id=$2, session_kind=$3, hidden=true WHERE id=$1")
        .bind(session_id)
        .bind(parent_session_id)
        .bind(subagent_kind)
        .execute(pool)
        .await?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO generic_subagents (id, parent_session_id, subagent_session_id, subagent_key, workflow_identity, subagent_kind, role_id, workspace_policy, hidden_projection_behavior, lifecycle_status, audit_metadata) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'parent_summary','open',$9)")
        .bind(id)
        .bind(parent_session_id)
        .bind(session_id)
        .bind(subagent_key)
        .bind(workflow_identity)
        .bind(subagent_kind)
        .bind(role_id)
        .bind(workspace_policy)
        .bind(audit_metadata)
        .execute(pool)
        .await?;
    Ok(session_id)
}

pub async fn close_subagent(pool: &PgPool, parent_session_id: Uuid, subagent_key: &str, workflow_identity: &str) -> Result<Option<Uuid>> {
    let row = sqlx::query("UPDATE generic_subagents SET lifecycle_status='closed', closed_at=now() WHERE parent_session_id=$1 AND subagent_key=$2 AND workflow_identity=$3 AND lifecycle_status='open' RETURNING subagent_session_id")
        .bind(parent_session_id)
        .bind(subagent_key)
        .bind(workflow_identity)
        .fetch_optional(pool)
        .await?;
    if let Some(row) = row {
        let session_id: Uuid = row.get("subagent_session_id");
        sqlx::query("UPDATE sessions SET status='closed', closed_at=COALESCE(closed_at, now()), close_reason='subagent closed by lifecycle hook', updated_at=now() WHERE id=$1")
            .bind(session_id)
            .execute(pool)
            .await?;
        Ok(Some(session_id))
    } else {
        Ok(None)
    }
}

pub async fn cleanup_session_lifecycle_resources(pool: &PgPool, session_id: Uuid, reason: &str) -> Result<Value> {
    let released_leases = sqlx::query(
        "UPDATE resource_leases SET status='released', release_reason=$2, updated_at=now() WHERE owning_session_id=$1 AND status IN ('reserved','assigned')",
    )
    .bind(session_id)
    .bind(reason)
    .execute(pool)
    .await?
    .rows_affected();
    let closed_subagents = sqlx::query(
        "UPDATE generic_subagents SET lifecycle_status='closed', closed_at=COALESCE(closed_at, now()) WHERE parent_session_id=$1 AND lifecycle_status='open'",
    )
    .bind(session_id)
    .execute(pool)
    .await?
    .rows_affected();
    let closed_subagent_sessions = sqlx::query(
        "UPDATE sessions SET status='closed', closed_at=COALESCE(closed_at, now()), close_reason=$2, updated_at=now() WHERE parent_session_id=$1 AND hidden=true AND status='open'",
    )
    .bind(session_id)
    .bind(reason)
    .execute(pool)
    .await?
    .rows_affected();
    let terminated_processes = sqlx::query(
        "UPDATE managed_processes SET status='sessionClosed', end_time=COALESCE(end_time, now()), termination_reason=$2 WHERE session_id=$1 AND status='running' AND end_of_session_behavior='terminate'",
    )
    .bind(session_id)
    .bind(reason)
    .execute(pool)
    .await?
    .rows_affected();
    let summary = json!({
        "sessionId": session_id,
        "reason": reason,
        "releasedLeases": released_leases,
        "closedSubagents": closed_subagents,
        "closedSubagentSessions": closed_subagent_sessions,
        "terminatedProcesses": terminated_processes,
        "godModeRevocation": "rust_god_mode_revoke_active_called_by_session_lifecycle",
    });
    db::append_event(pool, session_id, None, "lifecycle", Some(session_id), "lifecycle.cleanup", Some("completed"), summary.clone()).await?;
    Ok(summary)
}

pub async fn parent_subagent_projection(pool: &PgPool, parent_session_id: Uuid) -> Result<Value> {
    let rows = sqlx::query(
        r#"
        SELECT g.subagent_session_id, g.subagent_key, g.workflow_identity, g.subagent_kind, g.role_id, g.lifecycle_status, g.created_at, g.closed_at,
               (SELECT COUNT(*) FROM runtime_packets p WHERE p.parent_session_id=g.parent_session_id) AS packet_count,
               (SELECT COUNT(*) FROM runtime_envelopes e WHERE e.target_session_id=g.subagent_session_id AND e.status='pending') AS pending_envelopes
        FROM generic_subagents g
        WHERE g.parent_session_id=$1
        ORDER BY g.created_at ASC
        "#,
    )
    .bind(parent_session_id)
    .fetch_all(pool)
    .await?;
    let subagents = rows
        .into_iter()
        .map(|row| json!({
            "sessionId": row.get::<Uuid, _>("subagent_session_id"),
            "subagentKey": row.get::<String, _>("subagent_key"),
            "workflowIdentity": row.get::<String, _>("workflow_identity"),
            "subagentKind": row.get::<String, _>("subagent_kind"),
            "roleId": row.get::<String, _>("role_id"),
            "lifecycleStatus": row.get::<String, _>("lifecycle_status"),
            "createdAt": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
            "closedAt": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("closed_at"),
            "packetCount": row.get::<i64, _>("packet_count"),
            "pendingEnvelopes": row.get::<i64, _>("pending_envelopes"),
        }))
        .collect::<Vec<_>>();
    Ok(json!({
        "parentSessionId": parent_session_id,
        "activeSubagents": subagents.iter().filter(|value| value["lifecycleStatus"] == "open").count(),
        "subagents": subagents,
    }))
}

pub async fn reserve_resource(pool: &PgPool, owning_session_id: Option<Uuid>, payload: Value, idempotency_key: &str) -> Result<Uuid> {
    let resource_type = required_string(&payload, "resourceType")?;
    let resource_id = payload.get("resourceId").and_then(Value::as_str);
    let handle = payload.get("handle").and_then(Value::as_str);
    let lease_purpose = payload.get("leasePurpose").and_then(Value::as_str).unwrap_or("hook-resource-lease");
    let existing_active = sqlx::query("SELECT id, owning_session_id FROM resource_leases WHERE resource_type=$1 AND COALESCE(resource_id, handle, '')=COALESCE($2,$3,'') AND status IN ('reserved','assigned') LIMIT 1")
        .bind(resource_type)
        .bind(resource_id)
        .bind(handle)
        .fetch_optional(pool)
        .await?;
    if let Some(row) = existing_active {
        let owner: Option<Uuid> = row.get("owning_session_id");
        if owner != owning_session_id {
            bail!("resource lease is already owned by another active session");
        }
        return Ok(row.get("id"));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO resource_leases (id, resource_type, resource_id, handle, owning_session_id, steward_session_id, steward_role_id, status, lease_purpose, expiry_policy, lifecycle_policy, audit_metadata) VALUES ($1,$2,$3,$4,$5,NULL,$6,$7,$8,$9,$10,$11)")
        .bind(id)
        .bind(resource_type)
        .bind(resource_id)
        .bind(handle)
        .bind(owning_session_id)
        .bind(payload.get("stewardRoleId").and_then(Value::as_str))
        .bind(payload.get("status").and_then(Value::as_str).unwrap_or("reserved"))
        .bind(lease_purpose)
        .bind(payload.get("expiryPolicy").cloned().unwrap_or_else(|| json!({})))
        .bind(payload.get("lifecyclePolicy").cloned().unwrap_or_else(|| json!({"releaseOnSessionClose": true})))
        .bind(json!({"source":"hook","idempotencyKey": idempotency_key}))
        .execute(pool)
        .await?;
    Ok(id)
}

pub async fn release_resource(pool: &PgPool, owning_session_id: Option<Uuid>, payload: Value) -> Result<Option<Uuid>> {
    let resource_type = required_string(&payload, "resourceType")?;
    let lease_id = payload.get("leaseId").and_then(Value::as_str).map(Uuid::parse_str).transpose()?;
    let resource_id = payload.get("resourceId").and_then(Value::as_str);
    let row = if let Some(lease_id) = lease_id {
        sqlx::query("UPDATE resource_leases SET status='released', release_reason=$3, updated_at=now() WHERE id=$1 AND ($2::uuid IS NULL OR owning_session_id=$2) AND status IN ('reserved','assigned') RETURNING id")
            .bind(lease_id)
            .bind(owning_session_id)
            .bind(payload.get("releaseReason").and_then(Value::as_str).unwrap_or("hook release"))
            .fetch_optional(pool)
            .await?
    } else {
        sqlx::query("UPDATE resource_leases SET status='released', release_reason=$4, updated_at=now() WHERE resource_type=$1 AND COALESCE(resource_id, '')=COALESCE($2,'') AND ($3::uuid IS NULL OR owning_session_id=$3) AND status IN ('reserved','assigned') RETURNING id")
            .bind(resource_type)
            .bind(resource_id)
            .bind(owning_session_id)
            .bind(payload.get("releaseReason").and_then(Value::as_str).unwrap_or("hook release"))
            .fetch_optional(pool)
            .await?
    };
    Ok(row.map(|row| row.get("id")))
}

pub fn constructor_manifest_example() -> Value {
    json!({
        "constructors": [
            "role_definition", "role_tool_bundle", "command_bundle_binding", "channel", "route",
            "hook_binding", "contract_workflow", "resource_type", "steward_binding", "lifecycle_policy"
        ]
    })
}

pub fn hook_context_from_session_summary(
    session_id: Uuid,
    project_key: Option<String>,
    session_kind: String,
    parent_session_id: Option<Uuid>,
    hidden: bool,
    role_snapshot: &RoleSnapshot,
    workdir: String,
    worktree_root: Option<String>,
    lifecycle_event: LifecycleHook,
) -> HookContext {
    HookContext {
        project_key,
        session_id: Some(session_id),
        session_kind: Some(session_kind),
        parent_session_id,
        hidden,
        role_snapshot_summary: json!({
            "roleId": role_snapshot.id,
            "roleVersion": role_snapshot.version,
            "model": role_snapshot.model_defaults.model,
            "policyActions": role_snapshot.policy.keys().cloned().collect::<Vec<_>>(),
        }),
        workdir: Some(workdir),
        worktree_root,
        turn_summary: json!({}),
        lifecycle_event: lifecycle_event.as_str().to_string(),
        active_contracts: Vec::new(),
        recent_packet_summaries: Vec::new(),
        subagent_summaries: Vec::new(),
        resource_lease_summaries: Vec::new(),
        visible_command_summaries: Vec::new(),
        tool_metadata: Vec::new(),
        routing_state: json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> HookContext {
        HookContext {
            project_key: Some("project".to_string()),
            session_id: Some(Uuid::nil()),
            session_kind: Some("source".to_string()),
            parent_session_id: None,
            hidden: false,
            role_snapshot_summary: json!({"roleId":"worker","roleVersion":"v1","policyActions":["execute_code"]}),
            workdir: Some("/tmp/work".to_string()),
            worktree_root: Some("/tmp/work".to_string()),
            turn_summary: json!({"turnId": Uuid::nil(), "status":"running"}),
            lifecycle_event: "on_model_request".to_string(),
            active_contracts: vec![json!({"contractType":"requirements","status":"active"})],
            recent_packet_summaries: vec![json!({"packetType":"requirements.claim","status":"valid"})],
            subagent_summaries: vec![json!({"subagentKind":"requirementsReviewer","status":"open"})],
            resource_lease_summaries: vec![json!({"resourceType":"iosSimulator","status":"assigned"})],
            visible_command_summaries: vec![json!({"actionId":"cmd.test.echo","scope":"global"})],
            tool_metadata: vec![json!({"toolCallId": Uuid::nil(), "stdoutArtifactId": Uuid::nil(), "stderrArtifactId": Uuid::nil()})],
            routing_state: json!({"selected":true}),
        }
    }

    #[test]
    fn lifecycle_hook_list_is_complete_and_parseable() {
        let names = LifecycleHook::all().iter().map(|hook| hook.as_str()).collect::<Vec<_>>();
        assert_eq!(names.len(), 16);
        for name in names {
            assert_eq!(LifecycleHook::parse(name).unwrap().as_str(), name);
        }
    }

    #[test]
    fn boundary_allowlist_rejects_disallowed_intents() {
        let intent = HookIntent { intent_type: "reserve_resource".to_string(), key: None, payload: json!({"resourceType":"iosSimulator"}), idempotency_key: None };
        assert!(validate_intent(LifecycleHook::OnModelRequest, &intent).is_err());
        assert!(validate_intent(LifecycleHook::OnToolComplete, &intent).is_ok());
    }

    #[test]
    fn every_boundary_has_allowed_and_disallowed_intent_coverage() {
        let samples = vec![
            HookIntent { intent_type: "require_output_schema".to_string(), key: None, payload: json!({"schemaName":"s","packetType":"p","schema":{"type":"object"}}), idempotency_key: None },
            HookIntent { intent_type: "record_packet".to_string(), key: None, payload: json!({"packetType":"p"}), idempotency_key: None },
            HookIntent { intent_type: "route_packet".to_string(), key: None, payload: json!({"packetId": Uuid::nil().to_string()}), idempotency_key: None },
            HookIntent { intent_type: "notify_session".to_string(), key: None, payload: json!({"message":"m"}), idempotency_key: None },
            HookIntent { intent_type: "ensure_subagent".to_string(), key: None, payload: json!({"subagentKey":"k","subagentKind":"kind","roleId":"requirements-reviewer","workflowIdentity":"wf"}), idempotency_key: None },
            HookIntent { intent_type: "close_subagent".to_string(), key: None, payload: json!({"subagentKey":"k","workflowIdentity":"wf"}), idempotency_key: None },
            HookIntent { intent_type: "reserve_resource".to_string(), key: None, payload: json!({"resourceType":"iosSimulator"}), idempotency_key: None },
            HookIntent { intent_type: "release_resource".to_string(), key: None, payload: json!({"resourceType":"iosSimulator"}), idempotency_key: None },
            HookIntent { intent_type: "add_turn_obligation".to_string(), key: None, payload: json!({"obligationType":"leaseIdleCheck"}), idempotency_key: None },
            HookIntent { intent_type: "update_contract_progress".to_string(), key: None, payload: json!({"contractId": Uuid::nil().to_string(), "progressKey":"overall", "status":"passed"}), idempotency_key: None },
            HookIntent { intent_type: "request_owner_approval".to_string(), key: None, payload: json!({"message":"approve"}), idempotency_key: None },
            HookIntent { intent_type: "block_with_reason".to_string(), key: None, payload: json!({"message":"blocked"}), idempotency_key: None },
        ];
        for boundary in LifecycleHook::all() {
            let allow = allowed_intents(*boundary);
            let mut saw_allowed = false;
            let mut saw_disallowed = false;
            for sample in &samples {
                let kind = HookIntentType::parse(&sample.intent_type).unwrap();
                let valid = validate_intent(*boundary, sample).is_ok();
                if allow.contains(&kind) {
                    assert!(valid, "{} should allow {}", boundary.as_str(), sample.intent_type);
                    saw_allowed = true;
                } else {
                    assert!(!valid, "{} should reject {}", boundary.as_str(), sample.intent_type);
                    saw_disallowed = true;
                }
            }
            assert!(saw_allowed, "boundary {} had no positive sample", boundary.as_str());
            assert!(saw_disallowed, "boundary {} had no negative sample", boundary.as_str());
        }
    }

    #[test]
    fn hook_intents_cannot_broaden_session_privileges_beyond_context_snapshot() {
        let lifecycle_event_id = Uuid::new_v4();
        let hash = source_hash("privilege-boundary-hook");
        let allowed = HookIntent {
            intent_type: "record_packet".to_string(),
            key: Some("visible-command".to_string()),
            payload: json!({"packetType":"workflow.command_reference","commandActionId":"cmd.test.echo"}),
            idempotency_key: None,
        };
        let result = evaluate_returned_intents(
            LifecycleHook::OnToolComplete,
            &hash,
            lifecycle_event_id,
            Some(Uuid::nil()),
            &context(),
            vec![allowed],
        );
        assert_eq!(result.validation_status, "valid");
        assert_eq!(result.returned_intents.len(), 1);

        let forbidden_command = HookIntent {
            intent_type: "record_packet".to_string(),
            key: Some("hidden-command".to_string()),
            payload: json!({"packetType":"workflow.command_reference","commandActionId":"cmd.hidden.root"}),
            idempotency_key: None,
        };
        let result = evaluate_returned_intents(
            LifecycleHook::OnToolComplete,
            &hash,
            lifecycle_event_id,
            Some(Uuid::nil()),
            &context(),
            vec![forbidden_command],
        );
        assert_eq!(result.validation_status, "invalid");
        assert!(result.errors.iter().any(|error| error.contains("outside session role/project visibility")));

        let approval_bypass = HookIntent {
            intent_type: "record_packet".to_string(),
            key: Some("approval-bypass".to_string()),
            payload: json!({"packetType":"workflow.notice","approvalBypass":true}),
            idempotency_key: None,
        };
        let result = evaluate_returned_intents(
            LifecycleHook::OnToolComplete,
            &hash,
            lifecycle_event_id,
            Some(Uuid::nil()),
            &context(),
            vec![approval_bypass],
        );
        assert_eq!(result.validation_status, "invalid");
        assert!(result.errors.iter().any(|error| error.contains("approval policy")));

        let owner_grant_required = HookIntent {
            intent_type: "reserve_resource".to_string(),
            key: Some("owner-grant".to_string()),
            payload: json!({"resourceType":"iosSimulator","ownerGrantRequired":true}),
            idempotency_key: None,
        };
        let result = evaluate_returned_intents(
            LifecycleHook::OnToolComplete,
            &hash,
            lifecycle_event_id,
            Some(Uuid::nil()),
            &context(),
            vec![owner_grant_required.clone()],
        );
        assert_eq!(result.validation_status, "invalid");
        assert!(result.errors.iter().any(|error| error.contains("explicit owner grant")));

        let mut granted_context = context();
        granted_context.routing_state = json!({"ownerGrantApproved": true});
        let result = evaluate_returned_intents(
            LifecycleHook::OnToolComplete,
            &hash,
            lifecycle_event_id,
            Some(Uuid::nil()),
            &granted_context,
            vec![owner_grant_required],
        );
        assert_eq!(result.validation_status, "valid");
    }

    #[test]
    fn schema_intent_validation_requires_schema_evidence() {
        let bad = HookIntent { intent_type: "require_output_schema".to_string(), key: None, payload: json!({"schemaName":"claim"}), idempotency_key: None };
        assert!(validate_intent(LifecycleHook::OnModelRequest, &bad).is_err());
        let good = HookIntent { intent_type: "require_output_schema".to_string(), key: Some("requirements-source".to_string()), payload: json!({"schemaName":"requirements_source_claim","packetType":"requirements.claim","schema":{"type":"object"}}), idempotency_key: None };
        assert!(validate_intent(LifecycleHook::OnModelRequest, &good).is_ok());
        let disables_system = HookIntent { intent_type: "require_output_schema".to_string(), key: None, payload: json!({"schemaName":"system","packetType":"requirements.claim","schema":{"type":"object"},"disableSystemSchemas":true}), idempotency_key: None };
        assert!(validate_intent(LifecycleHook::OnModelRequest, &disables_system).is_err());
        let weakens_contract = HookIntent { intent_type: "require_output_schema".to_string(), key: None, payload: json!({"schemaName":"weak","packetType":"requirements.claim","schema":{"type":"object"},"weakenActiveContract":true}), idempotency_key: None };
        assert!(validate_intent(LifecycleHook::OnModelRequest, &weakens_contract).is_err());
    }

    #[test]
    fn hook_intents_cannot_grant_revoke_or_bypass_god_mode() {
        let direct = HookIntent { intent_type: "record_packet".to_string(), key: None, payload: json!({"packetType":"god_mode.grant","payload":{"grantGodMode":true}}), idempotency_key: None };
        assert!(validate_intent(LifecycleHook::OnModelFinal, &direct).is_err());
        let embedded = HookIntent { intent_type: "record_packet".to_string(), key: None, payload: json!({"packetType":"workflow.notice","payload":{"bypassGodMode":true}}), idempotency_key: None };
        assert!(validate_intent(LifecycleHook::OnModelFinal, &embedded).is_err());
    }

    #[test]
    fn hook_context_is_bounded_and_excludes_hidden_outputs() {
        let hash = validate_context_bounded(&context()).unwrap();
        assert_eq!(hash.len(), 64);
        for forbidden in ["fullStdout", "fullStderr", "shellStdout", "shellStderr", "secret", "authToken", "apiKey", "unboundedChatHistory"] {
            let mut bad = context();
            bad.tool_metadata.push(json!({forbidden:"forbidden body"}));
            assert!(validate_context_bounded(&bad).is_err(), "expected forbidden context field to fail: {forbidden}");
        }
    }

    #[test]
    fn evaluation_fails_closed_for_duplicate_or_disallowed_intents() {
        let event_id = Uuid::new_v4();
        let source_hash = source_hash("hook");
        let result = evaluate_returned_intents(
            LifecycleHook::OnModelRequest,
            &source_hash,
            event_id,
            Some(Uuid::nil()),
            &context(),
            vec![HookIntent { intent_type: "ensure_subagent".to_string(), key: Some("reviewer".to_string()), payload: json!({"subagentKey":"requirements","subagentKind":"requirementsReviewer","roleId":"requirements-reviewer","workflowIdentity":"requirements-v1"}), idempotency_key: None }],
        );
        assert_eq!(result.validation_status, "invalid");
        assert!(result.returned_intents.is_empty());
    }

    #[test]
    fn starlark_hook_program_evaluates_to_typed_intents_with_limits() {
        let event_id = Uuid::new_v4();
        let source = r#"
def hook(ctx):
    return [record_packet(key = "claim", packet_type = "requirements.claim", payload = {"ok": True})]
"#;
        let result = evaluate_starlark_hook_program(
            LifecycleHook::OnModelFinal,
            &source_hash(source),
            event_id,
            Some(Uuid::nil()),
            &context(),
            source,
        );
        assert_eq!(result.validation_status, "valid", "{:?}", result.errors);
        assert_eq!(result.returned_intents.len(), 1);
        assert_eq!(result.returned_intents[0].intent_type, "record_packet");
        assert_eq!(result.returned_intents[0].payload["packetType"], "requirements.claim");

        let recursive = r#"
def f():
    return f()
def hook(ctx):
    return f()
"#;
        let recursive_result = evaluate_starlark_hook_program(
            LifecycleHook::OnModelFinal,
            &source_hash(recursive),
            event_id,
            Some(Uuid::nil()),
            &context(),
            recursive,
        );
        assert_eq!(recursive_result.validation_status, "invalid");
        assert!(recursive_result.errors.iter().any(|error| error.contains("recursive")), "{:?}", recursive_result.errors);

        let over_fuel = format!("def hook(ctx):\n    for i in range({}):\n        pass\n    return []\n", EVALUATION_FUEL_STEPS + 1);
        let fuel_result = evaluate_starlark_hook_program(
            LifecycleHook::OnModelFinal,
            &source_hash(&over_fuel),
            event_id,
            Some(Uuid::nil()),
            &context(),
            &over_fuel,
        );
        assert_eq!(fuel_result.validation_status, "invalid");
        assert!(fuel_result.errors.iter().any(|error| error.contains("fuel")), "{:?}", fuel_result.errors);
    }

    #[test]
    fn idempotency_key_is_stable_for_same_hook_event_and_intent() {
        let event_id = Uuid::new_v4();
        let intent = HookIntent { intent_type: "record_packet".to_string(), key: Some("claim".to_string()), payload: json!({"packetType":"requirements.claim"}), idempotency_key: None };
        let left = stable_intent_key("abc", event_id, Some(Uuid::nil()), &intent);
        let right = stable_intent_key("abc", event_id, Some(Uuid::nil()), &intent);
        assert_eq!(left, right);
    }

    #[test]
    fn starlark_source_validation_is_syntax_bounded_and_side_effect_free() {
        assert!(validate_hook_source("x = 1\n").is_ok());
        assert!(validate_hook_source("load(\"//x\", \"y\")").is_err());
        assert!(validate_hook_source("def broken(:\n  pass").is_err());
        assert!(validate_hook_source("while True:\n  pass").is_err());
        assert!(validate_hook_source("def f():\n  return f()\nf()").is_err());
    }

    #[test]
    fn manifest_validation_checks_hook_role_and_resource_shapes() {
        let manifest = json!({
            "roles": [{"id":"simulator-steward"}],
            "resourceTypes": [{"id":"iosSimulator"}],
            "hooks": [{"name":"on_model_request","source":"x = 1"}]
        });
        assert!(validate_runtime_manifest(&manifest).is_ok());
        let bad = json!({"hooks":[{"name":"on_unknown","source":"x=1"}]});
        assert!(validate_runtime_manifest(&bad).is_err());
        let duplicate_roles = json!({"roles":[{"id":"worker"},{"id":"worker"}]});
        assert!(validate_runtime_manifest(&duplicate_roles).is_err());
        let illegal_intent = json!({"hooks":[{"name":"on_tool_start","source":"def hook(ctx):\n    return []","intentTypes":["route_packet"]}]});
        assert!(validate_runtime_manifest(&illegal_intent).is_err());
        let unknown_steward_role = json!({"roles":[{"id":"worker"}],"resourceTypes":[{"id":"iosSimulator"}],"stewardBindings":[{"resourceType":"iosSimulator","stewardRole":"missing"}]});
        assert!(validate_runtime_manifest(&unknown_steward_role).is_err());
        let unknown_resource = json!({"roles":[{"id":"simulator-steward"}],"resourceTypes":[],"stewardBindings":[{"resourceType":"iosSimulator","stewardRole":"simulator-steward"}]});
        assert!(validate_runtime_manifest(&unknown_resource).is_err());
        let unknown_bundle = json!({"roles":[{"id":"worker","toolBundle":"missing"}],"roleToolBundles":[]});
        assert!(validate_runtime_manifest(&unknown_bundle).is_err());
        let bad_command_binding = json!({"roles":[{"id":"worker"}],"roleToolBundles":[{"id":"worker_tools","tools":["cmd.x"]}],"commandBundleBindings":[{"roleId":"worker","bundleId":"missing"}]});
        assert!(validate_runtime_manifest(&bad_command_binding).is_err());
        let bad_packet = json!({"channels":[{"id":"bad","packetTypes":["unnamespaced"]}]});
        assert!(validate_runtime_manifest(&bad_packet).is_err());
        let bad_route_packet = json!({"roles":[{"id":"worker"}],"channels":[{"id":"c","packetTypes":["packet.known"]}],"routes":[{"source":"packet.missing","target":"role:worker"}]});
        assert!(validate_runtime_manifest(&bad_route_packet).is_err());
        let bad_route_target = json!({"roles":[{"id":"worker"}],"channels":[{"id":"c","packetTypes":["packet.known"]}],"routes":[{"source":"packet.known","target":"subagent:missing"}]});
        assert!(validate_runtime_manifest(&bad_route_target).is_err());
        let bad_lifecycle_policy = json!({"lifecyclePolicies":[{"id":"cleanup","releaseLeases":"yes"}]});
        assert!(validate_runtime_manifest(&bad_lifecycle_policy).is_err());
    }

    #[test]
    fn starlark_project_runtime_constructors_compile_to_manifest() {
        let source = r#"
role_definition(id = "requirements-reviewer", display_name = "Requirements Reviewer", tool_bundle = "reviewer_read_only")
role_definition(id = "simulator-steward", display_name = "Simulator Steward")
role_tool_bundle(id = "reviewer_read_only", tools = ["read_packet"])
command_bundle_binding(id = "reviewer_commands", role_id = "requirements-reviewer", bundle_id = "reviewer_read_only")
channel(id = "requirements", packet_types = ["requirements.claim", "requirements.verdict"])
route(id = "claim_to_reviewer", source = "requirements.claim", target = "subagent:requirements-reviewer")
contract_workflow(id = "requirements_review", contract_type = "requirements", packet_types = ["requirements.claim", "requirements.verdict"])
resource_type(id = "iosSimulator", lease_policy = {"exclusive": True, "release_on_session_close": True})
steward_binding(resource_type = "iosSimulator", steward_role = "simulator-steward")
lifecycle_policy(id = "close_cleanup", release_leases = True, close_subagents = True)
hook_binding(name = "on_model_request", source = "x = 1")
"#;
        let manifest = compile_project_runtime_source(source).expect("compiled manifest");
        assert_eq!(manifest["roles"][0]["id"], "requirements-reviewer");
        assert_eq!(manifest["roleToolBundles"][0]["tools"][0], "read_packet");
        assert_eq!(manifest["commandBundleBindings"][0]["roleId"], "requirements-reviewer");
        assert_eq!(manifest["channels"][0]["packetTypes"][1], "requirements.verdict");
        assert_eq!(manifest["routes"][0]["target"], "subagent:requirements-reviewer");
        assert_eq!(manifest["contractWorkflows"][0]["contractType"], "requirements");
        assert_eq!(manifest["resourceTypes"][0]["leasePolicy"]["exclusive"], true);
        assert_eq!(manifest["stewardBindings"][0]["stewardRole"], "simulator-steward");
        assert_eq!(manifest["lifecyclePolicies"][0]["releaseLeases"], true);
        assert_eq!(manifest["hooks"][0]["name"], "on_model_request");
    }

    #[test]
    fn packet_envelope_catalog_distinguishes_runtime_message_classes() {
        let catalog = packet_envelope_type_catalog();
        for required in [
            "ordinary_human_message",
            "steering_input",
            "agent_message",
            "agent_request",
            "requirements_claim",
            "requirements_verdict",
            "approval_request",
            "approval_decision",
            "resource_request",
            "resource_response",
            "lifecycle_notice",
            "system_notice",
            "tooling_request",
        ] {
            assert!(catalog.contains(&required), "missing packet/envelope class {required}");
        }
    }
}
