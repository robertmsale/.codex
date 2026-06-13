use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::os::unix::process::ExitStatusExt;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use once_cell::sync::Lazy;
use starlark::any::ProvidesStaticType;
use starlark::environment::{GlobalsBuilder, Module};
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::Value;
use starlark::values::AllocValue;
use starlark::values::list::UnpackList;
use starlark::values::none::NoneType;
use starlark::values::structs::AllocStruct;
use starlark_map::small_map::SmallMap;
use uuid::Uuid;
use wait_timeout::ChildExt;

use crate::db;
use crate::approvals;
use crate::command_registry::{self, CommandVersion};
use crate::lifecycle::{self, TerminalStatus};
use crate::policy::{PolicyEngine, RuntimeDecision};
use crate::roles::RoleSnapshot;
use crate::workflow_memory::RememberCandidate;

const OUTPUT_LIMIT_BYTES: usize = 12_000;

static PROCESS_MANAGER: Lazy<Mutex<BTreeMap<Uuid, BTreeMap<String, ManagedProcess>>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

pub fn terminate_session_processes_for_close(session_id: Uuid) -> usize {
    let Ok(mut manager) = PROCESS_MANAGER.lock() else {
        return 0;
    };
    let Some(processes) = manager.get_mut(&session_id) else {
        return 0;
    };
    let mut terminated = 0usize;
    let mut remove = Vec::new();
    for (handle, process) in processes.iter_mut() {
        if process.status == "running" && process.end_of_session_behavior == "terminate" {
            let _ = process.terminate("sessionClosed", true);
            terminated += 1;
            remove.push(handle.clone());
        }
    }
    for handle in remove {
        processes.remove(&handle);
    }
    terminated
}

fn end_of_session_behavior(command_version: &CommandVersion) -> String {
    if command_version.end_of_turn_behavior == "terminate" {
        "terminate".to_string()
    } else {
        "block".to_string()
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionRoot {
    root: PathBuf,
}

impl ExecutionRoot {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            root: std::fs::canonicalize(path.as_ref())?,
        })
    }

    fn resolve_cwd(&self, cwd: &str) -> Result<PathBuf> {
        if cwd.trim().is_empty() {
            bail!("cwd must not be empty");
        }
        let resolved = std::fs::canonicalize(self.root.join(cwd))
            .with_context(|| format!("cwd is not accessible: {cwd}"))?;
        if !resolved.starts_with(&self.root) {
            bail!("cwd escapes execution root: {cwd}");
        }
        Ok(resolved)
    }

    fn resolve_read_path(&self, path: &str) -> Result<PathBuf> {
        if path.trim().is_empty() {
            bail!("fs.read path must not be empty");
        }
        let resolved = std::fs::canonicalize(self.root.join(path))
            .with_context(|| format!("fs.read path is not accessible: {path}"))?;
        if !resolved.starts_with(&self.root) {
            bail!("fs.read path escapes execution root: {path}");
        }
        Ok(resolved)
    }

    fn resolve_write_path(&self, path: &str) -> Result<PathBuf> {
        if path.trim().is_empty() {
            bail!("fs.write path must not be empty");
        }
        let joined = self.root.join(path);
        let parent = joined
            .parent()
            .ok_or_else(|| anyhow::anyhow!("fs.write path has no parent: {path}"))?;
        let parent = std::fs::canonicalize(parent)
            .with_context(|| format!("fs.write parent is not accessible: {path}"))?;
        if !parent.starts_with(&self.root) {
            bail!("fs.write path escapes execution root: {path}");
        }
        let resolved = parent.join(joined.file_name().ok_or_else(|| anyhow::anyhow!("fs.write path has no file name: {path}"))?);
        reject_git_internal(&resolved, &self.root, "fs.write")?;
        Ok(resolved)
    }

    fn validate_patch_path(&self, path: &str) -> Result<PathBuf> {
        if path.trim().is_empty() || path == "/dev/null" {
            bail!("patch path must not be empty or /dev/null in this phase");
        }
        let relative = path.strip_prefix("a/").or_else(|| path.strip_prefix("b/")).unwrap_or(path);
        let joined = self.root.join(relative);
        let parent = joined.parent().ok_or_else(|| anyhow::anyhow!("patch path has no parent: {path}"))?;
        let parent = std::fs::canonicalize(parent)
            .with_context(|| format!("patch parent is not accessible: {path}"))?;
        if !parent.starts_with(&self.root) {
            bail!("patch path escapes execution root: {path}");
        }
        let resolved = parent.join(joined.file_name().ok_or_else(|| anyhow::anyhow!("patch path has no file name: {path}"))?);
        reject_git_internal(&resolved, &self.root, "patch.apply")?;
        Ok(resolved)
    }

    pub fn as_path(&self) -> &Path {
        &self.root
    }
}

fn reject_git_internal(path: &Path, root: &Path, action: &str) -> Result<()> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    if rel.components().any(|component| component.as_os_str() == ".git") {
        bail!("{action} must not touch .git or git internals: {}", rel.display());
    }
    Ok(())
}

fn file_state(path: &Path) -> JsonValue {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            let bytes = std::fs::read(path).unwrap_or_default();
            let hash = Sha256::digest(&bytes);
            json!({
                "exists": true,
                "size": metadata.len(),
                "sha256": format!("{hash:x}"),
            })
        }
        Ok(metadata) => json!({"exists": true, "size": metadata.len(), "sha256": null, "kind": "nonFile"}),
        Err(_) => json!({"exists": false, "size": 0, "sha256": null}),
    }
}

#[derive(Debug, Serialize)]
pub struct ExecuteCodePacket {
    ok: bool,
    status: String,
    output: String,
    script_run_id: Uuid,
    host_api_calls: usize,
}

#[derive(Debug, ProvidesStaticType)]
struct HostKernel {
    pool: PgPool,
    session_id: Uuid,
    script_run_id: Uuid,
    root: ExecutionRoot,
    commands: std::collections::BTreeMap<String, CommandVersion>,
    role_snapshot: RoleSnapshot,
    memory_candidates: RefCell<Vec<RememberCandidate>>,
    output: RefCell<Vec<String>>,
    records: RefCell<Vec<HostRecord>>,
}

#[derive(Debug)]
struct ManagedProcess {
    id: Uuid,
    handle: String,
    command_version: CommandVersion,
    command_version_id: Uuid,
    binary_name: String,
    binary_path: String,
    argv: Vec<String>,
    cwd: String,
    child: Child,
    stdout: Arc<Mutex<String>>,
    stderr: Arc<Mutex<String>>,
    stdout_flush_cursor: usize,
    stderr_flush_cursor: usize,
    started: Instant,
    started_at: chrono::DateTime<Utc>,
    status: String,
    end_of_session_behavior: String,
    termination_reason: Option<String>,
}

#[derive(Debug)]
struct ManagedProcessRecord {
    id: Uuid,
    handle: String,
    command_version_id: Uuid,
    binary_name: String,
    binary_path: String,
    argv: Vec<String>,
    cwd: String,
    os_pid: Option<i64>,
    os_pgid: Option<i64>,
    status: String,
    end_of_turn_behavior: String,
    end_of_session_behavior: String,
    max_runtime_ms: Option<i64>,
    termination_reason: Option<String>,
    event: String,
    payload: JsonValue,
}

#[derive(Debug)]
struct ProcessOutputRecord {
    process_id: Uuid,
    handle: String,
    stream: String,
    content: String,
    truncated: bool,
}

#[derive(Debug)]
struct ApprovalPauseRecord {
    action: String,
    policy: crate::policy::PolicyResult,
    action_input: JsonValue,
}

#[derive(Debug)]
enum HostRecord {
    Policy(PolicyDecisionRecord),
    HostApi(HostApiRecord),
    Command(CommandRecord),
    ManagedProcess(ManagedProcessRecord),
    ProcessOutput(ProcessOutputRecord),
    ApprovalPause(ApprovalPauseRecord),
    FileMutation(FileMutationRecord),
    PatchRun(PatchRunRecord),
    WorkflowMemory(WorkflowMemoryRecord),
}

#[derive(Debug)]
struct WorkflowMemoryRecord {
    event_type: String,
    memory_id: Option<Uuid>,
    payload: JsonValue,
}

#[derive(Debug)]
struct PolicyDecisionRecord {
    decision: String,
    payload: JsonValue,
}

#[derive(Debug)]
struct HostApiRecord {
    id: Uuid,
    action: String,
    status: String,
    input: JsonValue,
    output: JsonValue,
    duration_ms: i64,
    truncation: JsonValue,
}

#[derive(Debug)]
struct CommandRecord {
    id: Uuid,
    host_api_call_id: Uuid,
    command_version_id: Uuid,
    binary_name: String,
    binary_path: String,
    argv: Vec<String>,
    cwd: String,
    status: String,
    stdout: String,
    stderr: String,
    exit_status: Option<i32>,
    max_runtime_ms: Option<i64>,
    duration_ms: i64,
    truncation: JsonValue,
    policy_decision: JsonValue,
}

#[derive(Debug)]
struct FileMutationRecord {
    id: Uuid,
    action: &'static str,
    path: String,
    before_state: JsonValue,
    after_state: JsonValue,
    status: String,
    error: Option<String>,
    duration_ms: i64,
    policy_decision: JsonValue,
    truncation: JsonValue,
}

#[derive(Debug)]
struct PatchRunRecord {
    id: Uuid,
    action: &'static str,
    affected_paths: JsonValue,
    before_state: JsonValue,
    after_state: JsonValue,
    status: String,
    error: Option<String>,
    duration_ms: i64,
    policy_decision: JsonValue,
    truncation: JsonValue,
}

pub async fn execute_code(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    tool_call_id: Uuid,
    source: &str,
    root: &ExecutionRoot,
    role_snapshot: &RoleSnapshot,
) -> Result<ExecuteCodePacket> {
    let script_run_id = Uuid::new_v4();
    let script_started = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO script_runs (id, tool_call_id, source, status, started_at)
        VALUES ($1, $2, $3, 'running', $4)
        "#,
    )
    .bind(script_run_id)
    .bind(tool_call_id)
    .bind(source)
    .bind(script_started)
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "script",
        Some(script_run_id),
        "script.started",
        Some("running"),
        json!({"source": source}),
    )
    .await?;

    let project_key = crate::db::session_project_key(pool, session_id).await?;
    let live_commands = command_registry::live_visible_commands(pool, role_snapshot, project_key.as_deref()).await?;
    let mut process_handles = crate::db::session_process_handles(pool, session_id).await?;
    process_handles.extend(active_process_handles(session_id));
    process_handles.sort();
    process_handles.dedup();
    let result = evaluate_starlark(
        pool.clone(),
        session_id,
        script_run_id,
        source,
        root.clone(),
        role_snapshot.clone(),
        live_commands,
        process_handles,
    );
    let status = if result.error.is_some() {
        TerminalStatus::Failed
    } else {
        TerminalStatus::Completed
    };
    let final_output = result.output;
    let stderr = result.error.unwrap_or_default();
    let records = result.records;

    for record in &records {
        persist_record(pool, session_id, turn_id, tool_call_id, script_run_id, record, role_snapshot).await?;
    }

    let (final_output, final_truncated) = truncate_text(&final_output, OUTPUT_LIMIT_BYTES);
    let (stderr, stderr_truncated) = truncate_text(&stderr, OUTPUT_LIMIT_BYTES);
    let script_truncation = json!({
        "finalOutputTruncated": final_truncated,
        "stderrTruncated": stderr_truncated,
        "limitBytes": OUTPUT_LIMIT_BYTES,
    });
    lifecycle::complete_script_run(
        pool,
        script_run_id,
        status,
        &final_output,
        &stderr,
        &script_truncation,
        Utc::now(),
    )
    .await?;
    if let Err(error) = crate::workflow_memory::index_script(pool, session_id, turn_id, script_run_id, source).await {
        crate::workflow_memory::record_provider_failure(
            pool,
            session_id,
            Some(turn_id),
            Some(script_run_id),
            "workflow_memory.index_failed",
            &error.to_string(),
            json!({"phase": "script_index", "scriptRunId": script_run_id}),
        )
        .await?;
    }
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "script",
        Some(script_run_id),
        "script.completed",
        Some(status.as_str()),
        json!({"finalOutput": final_output, "stderr": stderr}),
    )
    .await?;

    if status == TerminalStatus::Failed {
        bail!(stderr);
    }

    for candidate in &result.memory_candidates {
        if let Err(error) = crate::workflow_memory::promote_project_memory(
            pool,
            session_id,
            turn_id,
            script_run_id,
            source,
            &candidate.title,
            &candidate.reason,
        )
        .await
        {
            crate::workflow_memory::record_provider_failure(
                pool,
                session_id,
                Some(turn_id),
                Some(script_run_id),
                "workflow_memory.promotion_failed",
                &error.to_string(),
                json!({"phase": "promotion", "title": candidate.title}),
            )
            .await?;
        }
    }

    Ok(ExecuteCodePacket {
        ok: true,
        status: status.as_str().to_string(),
        output: final_output,
        script_run_id,
        host_api_calls: records
            .iter()
            .filter(|record| matches!(record, HostRecord::HostApi(_)))
            .count(),
    })
}

struct EvalResult {
    output: String,
    records: Vec<HostRecord>,
    memory_candidates: Vec<RememberCandidate>,
    error: Option<String>,
}

fn evaluate_starlark(
    pool: PgPool,
    session_id: Uuid,
    script_run_id: Uuid,
    source: &str,
    root: ExecutionRoot,
    role_snapshot: RoleSnapshot,
    live_commands: Vec<CommandVersion>,
    process_handles: Vec<String>,
) -> EvalResult {
    let prelude = match command_registry::starlark_prelude(&live_commands, &process_handles) {
        Ok(prelude) => prelude,
        Err(error) => {
            return EvalResult {
                output: String::new(),
                records: Vec::new(),
                memory_candidates: Vec::new(),
                error: Some(error.to_string()),
            };
        }
    };
    let script = format!("{prelude}\n{source}");
    let kernel = HostKernel {
        pool,
        session_id,
        script_run_id,
        root,
        commands: live_commands.into_iter().map(|command| (command.action_id.clone(), command)).collect(),
        role_snapshot,
        memory_candidates: RefCell::new(Vec::new()),
        output: RefCell::new(Vec::new()),
        records: RefCell::new(Vec::new()),
    };
    let error = match AstModule::parse("execute_code.star", script, &Dialect::Standard) {
        Ok(ast) => {
            let globals = GlobalsBuilder::standard().with(add_host_builtins).build();
            let module = Module::new();
            let mut eval = Evaluator::new(&module);
            eval.extra = Some(&kernel);
            eval.eval_module(ast, &globals)
                .err()
                .map(|error| format!("Starlark evaluation failed: {error}"))
        }
        Err(error) => Some(format!("execute_code source is not valid Starlark syntax: {error}")),
    };
    let output = kernel.output.borrow().join("\n");
    kernel.cleanup_end_of_turn();
    let records = kernel.records.into_inner();
    let memory_candidates = kernel.memory_candidates.into_inner();
    EvalResult {
        output,
        records,
        memory_candidates,
        error,
    }
}

fn add_host_builtins(builder: &mut GlobalsBuilder) {
    builder.namespace("fs", fs_builtins);
    builder.namespace("patch", patch_builtins);
    builder.namespace_no_docs("__cmd", cmd_dynamic_builtins);
    builder.namespace_no_docs("__proc", proc_dynamic_builtins);
    builder.namespace("workflow_memory", workflow_memory_builtins);
    struct_builtins(builder);
    output_builtins(builder);
}

fn active_process_handles(session_id: Uuid) -> Vec<String> {
    PROCESS_MANAGER
        .lock()
        .map(|manager| {
            manager
                .get(&session_id)
                .map(|processes| processes.keys().cloned().collect())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

fn spawn_max_runtime_supervisor(
    pool: PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    process_id: Uuid,
    handle: String,
    command_version_id: Uuid,
    max_runtime_ms: i64,
) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(max_runtime_ms as u64)).await;
        let supervised = {
            let mut manager = PROCESS_MANAGER.lock().ok();
            manager
                .as_mut()
                .and_then(|manager| manager.get_mut(&session_id))
                .and_then(|processes| processes.get_mut(&handle))
                .map(|process| {
                    let _ = process.refresh_status();
                    if process.status == "running" {
                        let _ = process.terminate("maxRuntimeExceeded", true);
                    }
                    let event = if process.status == "maxRuntimeExceeded" {
                        "process.maxRuntimeExceeded"
                    } else {
                        "process.naturalExit"
                    };
                    (process.status.clone(), process.termination_reason.clone(), event.to_string())
                })
        };
        let Some((status, termination_reason, event_type)) = supervised else {
            return;
        };
        let updated = sqlx::query(
            r#"
            UPDATE managed_processes
            SET status = $2,
                end_time = now(),
                termination_reason = $3,
                metadata = metadata || $4
            WHERE id = $1 AND status = 'running'
            "#,
        )
        .bind(process_id)
        .bind(&status)
        .bind(&termination_reason)
        .bind(json!({"maxRuntimeSupervisor": true, "observedStatus": status}))
        .execute(&pool)
        .await;
        if matches!(updated.as_ref().map(|result| result.rows_affected()), Ok(1)) {
            let _ = db::append_event(
                &pool,
                session_id,
                turn_id,
                "process",
                Some(process_id),
                &event_type,
                Some(&status),
                json!({
                    "handle": handle,
                    "commandVersionId": command_version_id,
                    "maxRuntimeMs": max_runtime_ms,
                    "recordedBy": "processManagerSupervisor",
                }),
            )
            .await;
        }
    });
}

#[starlark_module]
fn fs_builtins(builder: &mut GlobalsBuilder) {
    fn read<'v>(path: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).run_fs_read(path)
    }

    fn write<'v>(path: &'v str, content: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).run_fs_write(path, content)
    }
}

#[starlark_module]
fn patch_builtins(builder: &mut GlobalsBuilder) {
    fn apply<'v>(unified_diff: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).run_patch_apply(unified_diff)
    }
}

#[starlark_module]
fn cmd_dynamic_builtins(builder: &mut GlobalsBuilder) {
    fn sync<'v>(
        action: &'v str,
        args: UnpackList<Value<'v>>,
        cwd: &'v str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        let args = args
            .items
            .iter()
            .map(|value| {
                value
                    .unpack_str()
                    .map(ToString::to_string)
                    .ok_or_else(|| anyhow::anyhow!("registry command args must be strings"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        host_kernel(eval).run_registry_command(action, args, cwd)
    }

    fn start<'v>(
        action: &'v str,
        args: UnpackList<Value<'v>>,
        cwd: &'v str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        let args = args
            .items
            .iter()
            .map(|value| {
                value
                    .unpack_str()
                    .map(ToString::to_string)
                    .ok_or_else(|| anyhow::anyhow!("registry command args must be strings"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        host_kernel(eval).start_registry_command(action, args, cwd)
    }
}

#[starlark_module]
fn proc_dynamic_builtins(builder: &mut GlobalsBuilder) {
    fn is_running<'v>(handle: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<bool> {
        host_kernel(eval).proc_is_running(handle)
    }

    fn await_for<'v>(handle: &'v str, #[starlark(default = 0)] mins: i32, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        host_kernel(eval).proc_await_for(handle, mins)?;
        Ok(NoneType)
    }

    fn flush_buffer<'v>(handle: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).proc_flush(handle)
    }

    fn terminate<'v>(handle: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).proc_terminate(handle)
    }

    fn input<'v>(handle: &'v str, text: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).proc_input(handle, text)
    }
}

#[starlark_module]
fn workflow_memory_builtins(builder: &mut GlobalsBuilder) {
    fn help<'v>(eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).workflow_memory_help()
    }

    fn remember_when<'v>(
        condition: bool,
        title: &'v str,
        reason: &'v str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        host_kernel(eval).workflow_memory_remember_when(condition, title, reason)
    }

    fn mark_not_helpful<'v>(
        id: &'v str,
        reason: &'v str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        host_kernel(eval).workflow_memory_feedback("workflow_memory.mark_not_helpful", id, json!({"reason": reason}))
    }

    fn mark_attempted<'v>(
        id: &'v str,
        #[starlark(default = true)] variant: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        host_kernel(eval).workflow_memory_feedback("workflow_memory.mark_attempted", id, json!({"variant": variant}))
    }
}

#[starlark_module]
fn output_builtins(builder: &mut GlobalsBuilder) {
    fn output<'v>(value: Value<'v>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        host_kernel(eval).output.borrow_mut().push(value.to_string());
        Ok(NoneType)
    }
}

#[starlark_module]
fn struct_builtins(builder: &mut GlobalsBuilder) {
    fn r#struct<'v>(
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
    ) -> anyhow::Result<impl AllocValue<'v>> {
        Ok(AllocStruct(kwargs.into_iter().collect::<Vec<_>>()))
    }
}

fn host_kernel<'v, 'a>(eval: &Evaluator<'v, 'a, '_>) -> &'a HostKernel {
    eval.extra
        .expect("HostKernel must be installed in Evaluator.extra")
        .downcast_ref::<HostKernel>()
        .expect("Evaluator.extra must be HostKernel")
}

fn block_on_host_future<F: std::future::Future>(future: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}

impl HostKernel {
    fn decide(&self, action: &str, input: JsonValue) -> crate::policy::PolicyResult {
        let decision = PolicyEngine::decide(&self.role_snapshot, action, input);
        self.records.borrow_mut().push(HostRecord::Policy(PolicyDecisionRecord {
            decision: decision.decision.as_str().to_string(),
            payload: decision.to_event_payload(),
        }));
        decision
    }

    fn workflow_memory_help(&self) -> anyhow::Result<String> {
        let input = json!({"mode": "latestPriorRelevantScript", "limit": 5});
        let policy = self.decide("workflow_memory.search", input.clone());
        if !policy.decision.can_execute() {
            bail!("workflow_memory.help blocked by policy: {}", policy.decision.as_str());
        }
        let results = match block_on_host_future(crate::workflow_memory::help_results_for_latest_prior_script(
            &self.pool,
            self.session_id,
            self.script_run_id,
            5,
        )) {
            Ok(results) => results,
            Err(error) => {
                self.records.borrow_mut().push(HostRecord::WorkflowMemory(WorkflowMemoryRecord {
                    event_type: "workflow_memory.provider_failure".to_string(),
                    memory_id: None,
                    payload: json!({"phase": "help", "error": error.to_string()}),
                }));
                Vec::new()
            }
        };
        let payload = json!({
            "input": input,
            "resultCount": results.len(),
            "results": results,
        });
        self.records.borrow_mut().push(HostRecord::WorkflowMemory(WorkflowMemoryRecord {
            event_type: "workflow_memory.help".to_string(),
            memory_id: None,
            payload: payload.clone(),
        }));
        Ok(serde_json::to_string(payload.get("results").unwrap_or(&JsonValue::Null))?)
    }

    fn workflow_memory_remember_when(&self, condition: bool, title: &str, reason: &str) -> anyhow::Result<String> {
        if !condition {
            self.records.borrow_mut().push(HostRecord::WorkflowMemory(WorkflowMemoryRecord {
                event_type: "workflow_memory.remember_skipped".to_string(),
                memory_id: None,
                payload: json!({"condition": false, "title": title}),
            }));
            return Ok("remember_when condition false; no candidate recorded".to_string());
        }
        let input = json!({"scope": "project", "title": title, "reason": reason});
        let policy = self.decide("workflow_memory.remember.project", input.clone());
        if !policy.decision.can_execute() {
            bail!("workflow_memory.remember_when blocked by policy: {}", policy.decision.as_str());
        }
        self.memory_candidates.borrow_mut().push(RememberCandidate {
            title: title.to_string(),
            reason: reason.to_string(),
        });
        self.records.borrow_mut().push(HostRecord::WorkflowMemory(WorkflowMemoryRecord {
            event_type: "workflow_memory.remember_candidate".to_string(),
            memory_id: None,
            payload: input,
        }));
        Ok("remember candidate recorded; it will promote only if this script completes successfully".to_string())
    }

    fn workflow_memory_feedback(&self, event_type: &str, id: &str, payload: JsonValue) -> anyhow::Result<String> {
        let memory_id = Uuid::parse_str(id).with_context(|| format!("workflow memory id is not a UUID: {id}"))?;
        let input = json!({"memoryId": memory_id, "eventType": event_type, "payload": payload});
        let policy = self.decide("workflow_memory.feedback", input.clone());
        if !policy.decision.can_execute() {
            bail!("{event_type} blocked by policy: {}", policy.decision.as_str());
        }
        let visible = block_on_host_future(crate::workflow_memory::memory_visible_to_session(&self.pool, self.session_id, memory_id))
            .with_context(|| format!("failed to validate workflow memory feedback target: {memory_id}"))?;
        if !visible {
            bail!("workflow memory feedback target is not visible to this session: {memory_id}");
        }
        self.records.borrow_mut().push(HostRecord::WorkflowMemory(WorkflowMemoryRecord {
            event_type: event_type.to_string(),
            memory_id: Some(memory_id),
            payload: input,
        }));
        Ok(format!("{event_type} recorded for {memory_id}"))
    }

    fn run_fs_read(&self, path: &str) -> anyhow::Result<String> {
        let started = Instant::now();
        let input = json!({"path": path});
        let policy = self.decide("fs.read", input.clone());
        if !policy.decision.can_execute() {
            bail!("fs.read blocked by policy: {}", policy.decision.as_str());
        }
        let resolved = self.root.resolve_read_path(path)?;
        let text = std::fs::read_to_string(&resolved)?;
        let (output, truncated) = truncate_text(&text, OUTPUT_LIMIT_BYTES);
        self.records.borrow_mut().push(HostRecord::HostApi(HostApiRecord {
            id: Uuid::new_v4(),
            action: "fs.read".to_string(),
            status: "completed".to_string(),
            input,
            output: json!({"content": output, "resolvedPath": resolved.display().to_string()}),
            duration_ms: started.elapsed().as_millis() as i64,
            truncation: json!({"contentTruncated": truncated, "limitBytes": OUTPUT_LIMIT_BYTES}),
        }));
        Ok(output)
    }

    fn run_registry_command(&self, action: &str, args: Vec<String>, cwd: &str) -> anyhow::Result<String> {
        let command_version = self
            .commands
            .get(action)
            .ok_or_else(|| anyhow::anyhow!("registry command is not visible in this execute_code call: {action}"))?;
        if !command_version.sync_allowed {
            bail!("{action} does not allow synchronous execution");
        }
        self.execute_registry_command(action, args, cwd)
    }

    fn start_registry_command(&self, action: &str, args: Vec<String>, cwd: &str) -> anyhow::Result<String> {
        let command_version = self
            .commands
            .get(action)
            .ok_or_else(|| anyhow::anyhow!("registry command is not visible in this execute_code call: {action}"))?;
        if !command_version.async_allowed {
            bail!("{action} does not allow asynchronous execution");
        }
        let handle = format!("proc_{}", Uuid::new_v4().simple());
        let (mut command, binary_path, argv, resolved_cwd, policy, input) = self.prepare_registry_command(action, args, cwd, "start")?;
        let started_at = Utc::now();
        command.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::piped());
        command.process_group(0);
        let mut child = command.spawn().with_context(|| format!("failed to start async command: {action}"))?;
        let child_pid = child.id();
        let stdout_buf = Arc::new(Mutex::new(String::new()));
        let stderr_buf = Arc::new(Mutex::new(String::new()));
        let buffer_limit = command_version.output_buffer_bytes;
        if let Some(mut stdout) = child.stdout.take() {
            let target = Arc::clone(&stdout_buf);
            thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                loop {
                    match stdout.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Ok(mut buffer) = target.lock() {
                                buffer.push_str(&String::from_utf8_lossy(&chunk[..n]));
                                enforce_string_buffer_limit(&mut buffer, buffer_limit);
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }
        if let Some(mut stderr) = child.stderr.take() {
            let target = Arc::clone(&stderr_buf);
            thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                loop {
                    match stderr.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Ok(mut buffer) = target.lock() {
                                buffer.push_str(&String::from_utf8_lossy(&chunk[..n]));
                                enforce_string_buffer_limit(&mut buffer, buffer_limit);
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }
        let process_id = Uuid::new_v4();
        let os_pid = Some(child_pid as i64);
        self.records.borrow_mut().push(HostRecord::ManagedProcess(ManagedProcessRecord {
            id: process_id,
            handle: handle.clone(),
            command_version_id: command_version.version_id,
            binary_name: command_version.binary_name.clone(),
            binary_path: binary_path.display().to_string(),
            argv: argv.clone(),
            cwd: resolved_cwd.display().to_string(),
            os_pid,
            os_pgid: os_pid,
            status: "running".to_string(),
            end_of_turn_behavior: command_version.end_of_turn_behavior.clone(),
            end_of_session_behavior: end_of_session_behavior(&command_version),
            max_runtime_ms: command_version.max_runtime.map(|d| d.as_millis() as i64),
            termination_reason: None,
            event: "process.started".to_string(),
            payload: json!({"handle": handle, "commandVersionId": command_version.version_id, "policyDecision": policy.to_event_payload(), "input": input}),
        }));
        PROCESS_MANAGER
            .lock()
            .map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?
            .entry(self.session_id)
            .or_default()
            .insert(handle.clone(), ManagedProcess {
            id: process_id,
            handle: handle.clone(),
            command_version: command_version.clone(),
            command_version_id: command_version.version_id,
            binary_name: command_version.binary_name.clone(),
            binary_path: binary_path.display().to_string(),
            argv,
            cwd: resolved_cwd.display().to_string(),
            child,
            stdout: stdout_buf,
            stderr: stderr_buf,
            stdout_flush_cursor: 0,
            stderr_flush_cursor: 0,
            started: Instant::now(),
            started_at,
            status: "running".to_string(),
            end_of_session_behavior: end_of_session_behavior(&command_version),
            termination_reason: None,
        });
        Ok(handle)
    }

    fn proc_is_running(&self, handle: &str) -> anyhow::Result<bool> {
        let mut manager = PROCESS_MANAGER.lock().map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
        let proc = manager.get_mut(&self.session_id).and_then(|processes| processes.get_mut(handle)).ok_or_else(|| anyhow::anyhow!("session-only process no longer attached to this runtime session: {handle}"))?;
        let running = proc.refresh_status()?;
        if proc.status == "completed" {
            self.records.borrow_mut().push(HostRecord::ManagedProcess(proc.snapshot_record("process.naturalExit", json!({"handle": handle}))));
        } else if proc.status == "maxRuntimeExceeded" {
            self.records.borrow_mut().push(HostRecord::ManagedProcess(proc.snapshot_record("process.maxRuntimeExceeded", json!({"handle": handle}))));
        }
        Ok(running)
    }

    fn proc_await_for(&self, handle: &str, mins: i32) -> anyhow::Result<()> {
        let mut manager = PROCESS_MANAGER.lock().map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
        let proc = manager.get_mut(&self.session_id).and_then(|processes| processes.get_mut(handle)).ok_or_else(|| anyhow::anyhow!("session-only process no longer attached to this runtime session: {handle}"))?;
        let requested_ms = (mins.max(0) as u64).saturating_mul(60_000);
        let min_ms = proc.command_version.min_await_ms.max(0) as u64;
        let max_ms = proc.command_version.max_await_ms.max(0) as u64;
        let wait_ms = requested_ms.max(min_ms);
        if max_ms > 0 && wait_ms > max_ms {
            bail!("await_for exceeds maxAwaitMs: requested {wait_ms}ms, max {max_ms}ms");
        }
        let deadline = Instant::now() + Duration::from_millis(wait_ms);
        while Instant::now() < deadline {
            if !proc.refresh_status()? {
                break;
            }
            proc.enforce_max_runtime()?;
            thread::sleep(Duration::from_millis(25));
        }
        self.records.borrow_mut().push(HostRecord::ManagedProcess(proc.snapshot_record("process.awaited", json!({"handle": handle, "requestedMins": mins, "effectiveMs": wait_ms}))));
        if proc.status == "completed" {
            self.records.borrow_mut().push(HostRecord::ManagedProcess(proc.snapshot_record("process.naturalExit", json!({"handle": handle}))));
        } else if proc.status == "maxRuntimeExceeded" {
            self.records.borrow_mut().push(HostRecord::ManagedProcess(proc.snapshot_record("process.maxRuntimeExceeded", json!({"handle": handle}))));
        }
        Ok(())
    }

    fn proc_flush(&self, handle: &str) -> anyhow::Result<String> {
        let mut manager = PROCESS_MANAGER.lock().map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
        let proc = manager.get_mut(&self.session_id).and_then(|processes| processes.get_mut(handle)).ok_or_else(|| anyhow::anyhow!("session-only process no longer attached to this runtime session: {handle}"))?;
        let (stdout, stdout_truncated) = proc.take_stdout_since_flush();
        let (stderr, stderr_truncated) = proc.take_stderr_since_flush();
        if !stdout.is_empty() {
            self.records.borrow_mut().push(HostRecord::ProcessOutput(ProcessOutputRecord { process_id: proc.id, handle: handle.to_string(), stream: "stdout".to_string(), content: stdout.clone(), truncated: stdout_truncated }));
        }
        if !stderr.is_empty() {
            self.records.borrow_mut().push(HostRecord::ProcessOutput(ProcessOutputRecord { process_id: proc.id, handle: handle.to_string(), stream: "stderr".to_string(), content: stderr.clone(), truncated: stderr_truncated }));
        }
        if stdout.is_empty() && stderr.is_empty() {
            self.records.borrow_mut().push(HostRecord::ProcessOutput(ProcessOutputRecord { process_id: proc.id, handle: handle.to_string(), stream: "stdout".to_string(), content: String::new(), truncated: false }));
        }
        self.records.borrow_mut().push(HostRecord::ManagedProcess(proc.snapshot_record("process.flushed", json!({"handle": handle, "stdoutBytes": stdout.len(), "stderrBytes": stderr.len()}))));
        Ok(format!("{stdout}{stderr}"))
    }

    fn proc_terminate(&self, handle: &str) -> anyhow::Result<String> {
        let mut manager = PROCESS_MANAGER.lock().map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
        let proc = manager.get_mut(&self.session_id).and_then(|processes| processes.get_mut(handle)).ok_or_else(|| anyhow::anyhow!("session-only process no longer attached to this runtime session: {handle}"))?;
        proc.terminate("terminated", true)?;
        self.records.borrow_mut().push(HostRecord::ManagedProcess(proc.snapshot_record("process.terminated", json!({"handle": handle, "terminateGraceMs": proc.command_version.terminate_grace_ms}))));
        Ok("terminated".to_string())
    }

    fn proc_input(&self, handle: &str, text: &str) -> anyhow::Result<String> {
        let mut manager = PROCESS_MANAGER.lock().map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
        let proc = manager.get_mut(&self.session_id).and_then(|processes| processes.get_mut(handle)).ok_or_else(|| anyhow::anyhow!("session-only process no longer attached to this runtime session: {handle}"))?;
        if proc.command_version.stdin_policy != "allow" {
            bail!("stdinPolicy forbids input for process handle: {handle}");
        }
        if let Some(stdin) = proc.child.stdin.as_mut() {
            stdin.write_all(text.as_bytes())?;
            stdin.flush()?;
            self.records.borrow_mut().push(HostRecord::ManagedProcess(proc.snapshot_record("process.stdin", json!({"handle": handle, "bytes": text.len()}))));
            Ok("input accepted".to_string())
        } else {
            bail!("stdin is no longer attached for process handle: {handle}");
        }
    }

    fn execute_registry_command(&self, action: &str, args: Vec<String>, cwd: &str) -> anyhow::Result<String> {
        let command_version = self
            .commands
            .get(action)
            .ok_or_else(|| anyhow::anyhow!("registry command is not visible in this execute_code call: {action}"))?;
        let (mut command, binary_path, argv, resolved_cwd, policy, input) = self.prepare_registry_command(action, args, cwd, "sync")?;
        let started = Instant::now();
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        command.process_group(0);
        let output = command.spawn().and_then(|mut child| {
            match command_version.max_runtime {
                Some(max_runtime) => match child.wait_timeout(max_runtime)? {
                    Some(_) => child.wait_with_output(),
                    None => {
                        terminate_process_group(child.id(), command_version.terminate_grace_ms);
                        let _ = child.kill();
                        let mut output = child.wait_with_output()?;
                        output.status = std::process::ExitStatus::from_raw(124 << 8);
                        Ok(output)
                    }
                },
                None => child.wait_with_output(),
            }
        });
        self.record_command_result(command_version, action, input, policy, binary_path.display().to_string(), argv, resolved_cwd.display().to_string(), started, output)
    }

    fn prepare_registry_command(&self, action: &str, args: Vec<String>, cwd: &str, execution_mode: &str) -> anyhow::Result<(Command, PathBuf, Vec<String>, PathBuf, crate::policy::PolicyResult, JsonValue)> {
        let command_version = self
            .commands
            .get(action)
            .ok_or_else(|| anyhow::anyhow!("registry command is not visible in this execute_code call: {action}"))?;
        if !command_version.allow_args_arg && !args.is_empty() {
            bail!("{action} does not accept args");
        }
        let mut argv = command_version.argv_prefix.clone();
        argv.extend(args);
        reject_forbidden_args(&command_version, &argv)?;
        let cwd = if command_version.allow_cwd_arg { cwd } else { &command_version.default_cwd };
        let binary_path = command_version.resolve_binary()?;
        let input = json!({
            "binary": command_version.binary_name,
            "argv": argv,
            "cwd": cwd,
            "executionRoot": self.root.as_path().display().to_string(),
            "commandVersionId": command_version.version_id,
            "executionMode": execution_mode,
        });
        let decision = command_version.execution_policy.as_str();
        let policy = crate::policy::PolicyResult {
            action: action.to_string(),
            decision: match decision {
                "allow" => RuntimeDecision::Allow,
                "ownerApproval" | "orchestratorApproval" => RuntimeDecision::ApprovalRequired,
                _ => RuntimeDecision::Deny,
            },
            reason: "approver-selected scoped command execution policy".to_string(),
            input: input.clone(),
            role_id: self.role_snapshot.id.clone(),
            role_version: self.role_snapshot.version.clone(),
            role_version_id: self.role_snapshot.role_version_id.to_string(),
            required_approver_kind: match decision {
                "ownerApproval" => Some(crate::approvals::ApproverKind::Owner),
                "orchestratorApproval" => Some(crate::approvals::ApproverKind::Orchestrator),
                _ => None,
            },
            source_decision: Some(decision.to_string()),
        };
        self.records.borrow_mut().push(HostRecord::Policy(PolicyDecisionRecord {
            decision: policy.decision.as_str().to_string(),
            payload: policy.to_event_payload(),
        }));
        if !policy.decision.can_execute() {
            if policy.decision == RuntimeDecision::ApprovalRequired {
                self.records.borrow_mut().push(HostRecord::ApprovalPause(ApprovalPauseRecord {
                    action: action.to_string(),
                    policy: policy.clone(),
                    action_input: input.clone(),
                }));
            }
            bail!("{action} blocked by policy: {}", policy.decision.as_str());
        }
        if command_version.cwd_policy != "underExecutionRoot" {
            bail!("unsupported cwd policy for {action}: {}", command_version.cwd_policy);
        }
        let resolved_cwd = self.root.resolve_cwd(cwd)?;
        let mut command = Command::new(&binary_path);
        command.args(&argv).current_dir(&resolved_cwd);
        match command_version.env_policy.as_str() {
            "empty" => {
                command.env_clear();
            }
            "minimalCargo" => {
                command.env_clear();
                if let Ok(value) = std::env::var("PATH") {
                    command.env("PATH", value);
                }
                if let Ok(value) = std::env::var("HOME") {
                    command.env("HOME", value);
                }
                if let Ok(value) = std::env::var("CARGO_HOME") {
                    command.env("CARGO_HOME", value);
                }
                if let Ok(value) = std::env::var("RUSTUP_HOME") {
                    command.env("RUSTUP_HOME", value);
                }
            }
            other => bail!("unsupported env policy for {action}: {other}"),
        }
        Ok((command, binary_path, argv, resolved_cwd, policy, input))
    }

    fn record_command_result(
        &self,
        command_version: &CommandVersion,
        action: &str,
        input: JsonValue,
        policy: crate::policy::PolicyResult,
        binary_path: String,
        argv: Vec<String>,
        cwd: String,
        started: Instant,
        output: std::io::Result<std::process::Output>,
    ) -> anyhow::Result<String> {
        let (status, exit_status, stdout, stderr) = match output {
            Ok(output) => (
                if output.status.code() == Some(124) {
                    "maxRuntimeExceeded"
                } else if output.status.success() {
                    "completed"
                } else {
                    "failed"
                },
                output.status.code(),
                String::from_utf8_lossy(&output.stdout).to_string(),
                String::from_utf8_lossy(&output.stderr).to_string(),
            ),
            Err(error) => ("failed", None, String::new(), error.to_string()),
        };
        let (stdout, stdout_truncated) = truncate_text(&stdout, command_version.output_limit);
        let (stderr, stderr_truncated) = truncate_text(&stderr, command_version.output_limit);
        let truncation = json!({
            "stdoutTruncated": stdout_truncated,
            "stderrTruncated": stderr_truncated,
            "limitBytes": command_version.output_limit,
        });
        let policy_decision = json!({
            "action": action,
            "decision": RuntimeDecision::Allow.as_str(),
            "reason": policy.reason,
            "role": {"id": policy.role_id, "version": policy.role_version},
            "commandVersionId": command_version.version_id,
        });
        let host_api_call_id = Uuid::new_v4();
        self.records.borrow_mut().push(HostRecord::HostApi(HostApiRecord {
            id: host_api_call_id,
            action: action.to_string(),
            status: status.to_string(),
            input,
            output: json!({"stdout": stdout, "stderr": stderr, "exitStatus": exit_status}),
            duration_ms: started.elapsed().as_millis() as i64,
            truncation: truncation.clone(),
        }));
        self.records.borrow_mut().push(HostRecord::Command(CommandRecord {
            id: Uuid::new_v4(),
            host_api_call_id,
            command_version_id: command_version.version_id,
            binary_name: command_version.binary_name.clone(),
            binary_path,
            argv,
            cwd,
            status: status.to_string(),
            stdout: stdout.clone(),
            stderr: stderr.clone(),
            exit_status,
            max_runtime_ms: command_version.max_runtime.map(|d| d.as_millis() as i64),
            duration_ms: started.elapsed().as_millis() as i64,
            truncation,
            policy_decision,
        }));
        if status == "completed" {
            Ok(stdout)
        } else if status == "maxRuntimeExceeded" {
            bail!("command exceeded maxRuntimeMs")
        } else {
            bail!(stderr)
        }
    }

    fn run_fs_write(&self, path: &str, content: &str) -> anyhow::Result<String> {
        let started = Instant::now();
        let input = json!({"path": path, "content": content, "executionRoot": self.root.as_path().display().to_string()});
        let policy = self.decide("fs.write", input.clone());
        if !policy.decision.can_execute() {
            bail!("fs.write blocked by policy: {}", policy.decision.as_str());
        }
        let resolved = self.root.resolve_write_path(path)?;
        let before = file_state(&resolved);
        let result = std::fs::write(&resolved, content.as_bytes()).with_context(|| format!("fs.write failed: {path}"));
        let after = file_state(&resolved);
        let (status, error) = match result {
            Ok(()) => ("completed".to_string(), None),
            Err(error) => ("failed".to_string(), Some(error.to_string())),
        };
        let policy_decision = json!({"action": "fs.write", "decision": "allow", "reason": policy.reason, "role": {"id": policy.role_id, "version": policy.role_version}});
        self.records.borrow_mut().push(HostRecord::FileMutation(FileMutationRecord {
            id: Uuid::new_v4(),
            action: "fs.write",
            path: resolved.display().to_string(),
            before_state: before,
            after_state: after,
            status: status.clone(),
            error: error.clone(),
            duration_ms: started.elapsed().as_millis() as i64,
            policy_decision,
            truncation: json!({"contentBytes": content.len()}),
        }));
        if let Some(error) = error {
            bail!(error);
        }
        Ok(json!({"path": path, "status": status}).to_string())
    }

    fn run_patch_apply(&self, unified_diff: &str) -> anyhow::Result<String> {
        let started = Instant::now();
        let affected = affected_paths(unified_diff)?;
        let mut resolved = Vec::new();
        for path in &affected {
            resolved.push(self.root.validate_patch_path(path)?);
        }
        let input = json!({"unifiedDiff": unified_diff, "affectedPaths": affected, "executionRoot": self.root.as_path().display().to_string()});
        let policy = self.decide("patch.apply", input.clone());
        if !policy.decision.can_execute() {
            bail!("patch.apply blocked by policy: {}", policy.decision.as_str());
        }
        let before = json!(resolved.iter().map(|path| json!({"path": path.display().to_string(), "state": file_state(path)})).collect::<Vec<_>>());
        let result = apply_unified_patch(self.root.as_path(), unified_diff);
        let after = json!(resolved.iter().map(|path| json!({"path": path.display().to_string(), "state": file_state(path)})).collect::<Vec<_>>());
        let (status, error) = match result {
            Ok(()) => ("completed".to_string(), None),
            Err(error) => ("failed".to_string(), Some(error.to_string())),
        };
        let policy_decision = json!({"action": "patch.apply", "decision": "allow", "reason": policy.reason, "role": {"id": policy.role_id, "version": policy.role_version}});
        self.records.borrow_mut().push(HostRecord::PatchRun(PatchRunRecord {
            id: Uuid::new_v4(),
            action: "patch.apply",
            affected_paths: json!(affected),
            before_state: before,
            after_state: after,
            status: status.clone(),
            error: error.clone(),
            duration_ms: started.elapsed().as_millis() as i64,
            policy_decision,
            truncation: json!({"diffBytes": unified_diff.len()}),
        }));
        if let Some(error) = error {
            bail!(error);
        }
        Ok(json!({"status": status}).to_string())
    }

    fn cleanup_end_of_turn(&self) {
        let Ok(mut manager) = PROCESS_MANAGER.lock() else {
            return;
        };
        let Some(processes) = manager.get_mut(&self.session_id) else {
            return;
        };
        let mut remove_handles = Vec::new();
        for (handle, process) in processes.iter_mut() {
            if process.status == "running" && process.command_version.end_of_turn_behavior == "terminate" {
                let _ = process.terminate("endOfTurnCleanup", true);
                self.records.borrow_mut().push(HostRecord::ManagedProcess(process.snapshot_record("process.endOfTurnCleanup", json!({"handle": process.handle}))));
                remove_handles.push(handle.clone());
            } else if process.status == "running" {
                self.records.borrow_mut().push(HostRecord::ManagedProcess(process.snapshot_record("process.continued", json!({"handle": process.handle, "note": "session-only process remains attached only while this runtime instance owns it"}))));
            } else {
                remove_handles.push(handle.clone());
            }
        }
        for handle in remove_handles {
            processes.remove(&handle);
        }
    }
}

impl ManagedProcess {
    fn refresh_status(&mut self) -> anyhow::Result<bool> {
        if self.status != "running" {
            return Ok(false);
        }
        match self.child.try_wait()? {
            Some(status) => {
                if self.command_version.max_runtime.is_some_and(|max| self.started.elapsed() >= max) && !status.success() {
                    self.status = "maxRuntimeExceeded".to_string();
                    self.termination_reason = Some("maxRuntimeExceeded".to_string());
                } else {
                    self.status = if status.success() { "completed" } else { "failed" }.to_string();
                    self.termination_reason = Some("naturalExit".to_string());
                }
                Ok(false)
            }
            None => Ok(true),
        }
    }

    fn enforce_max_runtime(&mut self) -> anyhow::Result<()> {
        if self.status == "running" {
            if let Some(max_runtime) = self.command_version.max_runtime {
                if self.started.elapsed() >= max_runtime {
                    self.terminate("maxRuntimeExceeded", false)?;
                }
            }
        }
        Ok(())
    }

    fn terminate(&mut self, reason: &str, graceful: bool) -> anyhow::Result<()> {
        if self.status != "running" {
            return Ok(());
        }
        if graceful {
            terminate_process_group(self.child.id(), self.command_version.terminate_grace_ms);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.status = if reason == "maxRuntimeExceeded" { "maxRuntimeExceeded" } else { "terminated" }.to_string();
        self.termination_reason = Some(reason.to_string());
        Ok(())
    }

    fn take_stdout_since_flush(&mut self) -> (String, bool) {
        let _ = self.refresh_status();
        if self.status != "running" {
            thread::sleep(Duration::from_millis(50));
        }
        let text = self.stdout.lock().map(|buffer| buffer.clone()).unwrap_or_default();
        if self.stdout_flush_cursor > text.len() {
            self.stdout_flush_cursor = 0;
        }
        let new = text.get(self.stdout_flush_cursor..).unwrap_or("").to_string();
        self.stdout_flush_cursor = text.len();
        truncate_text(&new, self.command_version.output_limit)
    }

    fn take_stderr_since_flush(&mut self) -> (String, bool) {
        let _ = self.refresh_status();
        if self.status != "running" {
            thread::sleep(Duration::from_millis(50));
        }
        let text = self.stderr.lock().map(|buffer| buffer.clone()).unwrap_or_default();
        if self.stderr_flush_cursor > text.len() {
            self.stderr_flush_cursor = 0;
        }
        let new = text.get(self.stderr_flush_cursor..).unwrap_or("").to_string();
        self.stderr_flush_cursor = text.len();
        truncate_text(&new, self.command_version.output_limit)
    }

    fn snapshot_record(&self, event: &str, payload: JsonValue) -> ManagedProcessRecord {
        ManagedProcessRecord {
            id: self.id,
            handle: self.handle.clone(),
            command_version_id: self.command_version_id,
            binary_name: self.binary_name.clone(),
            binary_path: self.binary_path.clone(),
            argv: self.argv.clone(),
            cwd: self.cwd.clone(),
            os_pid: Some(self.child.id() as i64),
            os_pgid: Some(self.child.id() as i64),
            status: self.status.clone(),
            end_of_turn_behavior: self.command_version.end_of_turn_behavior.clone(),
            end_of_session_behavior: self.end_of_session_behavior.clone(),
            max_runtime_ms: self.command_version.max_runtime.map(|d| d.as_millis() as i64),
            termination_reason: self.termination_reason.clone(),
            event: event.to_string(),
            payload: json!({"startedAt": self.started_at, "details": payload}),
        }
    }
}

fn terminate_process_group(pid: u32, grace_ms: i64) {
    unsafe {
        let _ = libc::killpg(pid as i32, libc::SIGTERM);
    }
    if grace_ms > 0 {
        thread::sleep(Duration::from_millis(grace_ms as u64));
    }
    unsafe {
        let _ = libc::killpg(pid as i32, libc::SIGKILL);
    }
}

fn reject_forbidden_args(command_version: &CommandVersion, argv: &[String]) -> Result<()> {
    for forbidden in &command_version.forbidden_args {
        if argv.iter().any(|arg| arg == forbidden || arg.starts_with(&format!("{forbidden}="))) {
            bail!(
                "{} rejects forbidden argv item from registry policy: {}",
                command_version.action_id,
                forbidden
            );
        }
    }
    Ok(())
}

pub async fn execute_resumed_action(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    script_run_id: Uuid,
    action: &str,
    input: &JsonValue,
    policy_decision: JsonValue,
) -> Result<JsonValue> {
    match action {
        action if command_registry::is_registry_command_action(action) => {
            execute_resumed_command(pool, session_id, turn_id, script_run_id, action, input, policy_decision).await
        }
        "fs.write" => execute_resumed_fs_write(pool, session_id, turn_id, script_run_id, input, policy_decision).await,
        "patch.apply" => execute_resumed_patch_apply(pool, session_id, turn_id, script_run_id, input, policy_decision).await,
        other => bail!("unsupported resumed action: {other}"),
    }
}

async fn execute_resumed_fs_write(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    script_run_id: Uuid,
    input: &JsonValue,
    policy_decision: JsonValue,
) -> Result<JsonValue> {
    let path = input.get("path").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused fs.write missing path"))?;
    let content = input.get("content").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused fs.write missing content"))?;
    let execution_root = input.get("executionRoot").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused fs.write missing executionRoot"))?;
    let root = ExecutionRoot::new(execution_root)?;
    let resolved = root.resolve_write_path(path)?;
    let started = Instant::now();
    let before = file_state(&resolved);
    std::fs::write(&resolved, content.as_bytes())?;
    let after = file_state(&resolved);
    let duration_ms = started.elapsed().as_millis() as i64;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO file_mutations (id, script_run_id, action_name, path, before_state, after_state, status, started_at, completed_at, duration_ms, policy_decision, truncation)
        VALUES ($1, $2, 'fs.write', $3, $4, $5, 'completed', $6, $6, $7, $8, $9)
        "#,
    )
    .bind(id)
    .bind(script_run_id)
    .bind(resolved.display().to_string())
    .bind(&before)
    .bind(&after)
    .bind(Utc::now())
    .bind(duration_ms)
    .bind(&policy_decision)
    .bind(json!({"contentBytes": content.len()}))
    .execute(pool)
    .await?;
    db::append_event(pool, session_id, turn_id, "file_mutation", Some(id), "file_mutation.completed", Some("completed"), json!({"action":"fs.write","path":resolved.display().to_string(),"before":before,"after":after,"durationMs":duration_ms,"policyDecision":policy_decision})).await?;
    Ok(json!({"fileMutationId": id, "status": "completed", "path": resolved.display().to_string()}))
}

async fn execute_resumed_patch_apply(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    script_run_id: Uuid,
    input: &JsonValue,
    policy_decision: JsonValue,
) -> Result<JsonValue> {
    let diff = input.get("unifiedDiff").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused patch.apply missing unifiedDiff"))?;
    let execution_root = input.get("executionRoot").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused patch.apply missing executionRoot"))?;
    let root = ExecutionRoot::new(execution_root)?;
    let paths = affected_paths(diff).unwrap_or_else(|_| Vec::new());
    let resolved_result = paths.iter().map(|path| root.validate_patch_path(path)).collect::<Result<Vec<_>>>();
    let resolved = resolved_result.unwrap_or_else(|_| Vec::new());
    let before = json!(resolved.iter().map(|path| json!({"path": path.display().to_string(), "state": file_state(path)})).collect::<Vec<_>>());
    let started = Instant::now();
    let apply_result = if paths.is_empty() {
        Err(anyhow::anyhow!("patch.apply requires at least one affected file path"))
    } else {
        apply_unified_patch(root.as_path(), diff)
    };
    let after = json!(resolved.iter().map(|path| json!({"path": path.display().to_string(), "state": file_state(path)})).collect::<Vec<_>>());
    let duration_ms = started.elapsed().as_millis() as i64;
    let id = Uuid::new_v4();
    let (status, error) = match apply_result {
        Ok(()) => ("completed", None),
        Err(error) => ("failed", Some(error.to_string())),
    };
    sqlx::query(
        r#"
        INSERT INTO patch_runs (id, script_run_id, action_name, affected_paths, before_state, after_state, status, error, started_at, completed_at, duration_ms, policy_decision, truncation)
        VALUES ($1, $2, 'patch.apply', $3, $4, $5, $6, $7, $8, $8, $9, $10, $11)
        "#,
    )
    .bind(id)
    .bind(script_run_id)
    .bind(json!(paths))
    .bind(&before)
    .bind(&after)
    .bind(status)
    .bind(&error)
    .bind(Utc::now())
    .bind(duration_ms)
    .bind(&policy_decision)
    .bind(json!({"diffBytes": diff.len()}))
    .execute(pool)
    .await?;
    db::append_event(pool, session_id, turn_id, "patch", Some(id), "patch.completed", Some(status), json!({"action":"patch.apply","affectedPaths":paths,"before":before,"after":after,"status":status,"error":error,"durationMs":duration_ms,"policyDecision":policy_decision})).await?;
    if let Some(error) = error {
        bail!(error);
    }
    Ok(json!({"patchRunId": id, "status": status}))
}

async fn execute_resumed_command(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    script_run_id: Uuid,
    action: &str,
    input: &JsonValue,
    policy_decision: JsonValue,
) -> Result<JsonValue> {
    let command_version = if let Some(version_id) = input
        .get("commandVersionId")
        .and_then(JsonValue::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
    {
        command_registry::command_by_version(pool, version_id).await?
    } else {
        command_registry::command_by_action(pool, action).await?
    };
    let stored_argv = json_string_array(input, "argv")?;
    let mut argv = command_version.argv_prefix.clone();
    if stored_argv.starts_with(&command_version.argv_prefix) {
        argv = stored_argv;
    } else {
        argv.extend(stored_argv);
    }
    reject_forbidden_args(&command_version, &argv)?;
    if input.get("executionMode").and_then(JsonValue::as_str) == Some("start") {
        return execute_resumed_async_command_with_version(pool, session_id, turn_id, script_run_id, input, policy_decision, command_version, argv).await;
    }
    execute_resumed_command_with_version(pool, session_id, turn_id, script_run_id, input, policy_decision, command_version, argv).await
}

fn json_string_array(input: &JsonValue, key: &str) -> Result<Vec<String>> {
    input
        .get(key)
        .and_then(JsonValue::as_array)
        .ok_or_else(|| anyhow::anyhow!("paused action input missing {key}"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow::anyhow!("paused action {key} must contain strings"))
        })
        .collect()
}

async fn execute_resumed_command_with_version(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    script_run_id: Uuid,
    input: &JsonValue,
    policy_decision: JsonValue,
    command_version: CommandVersion,
    argv: Vec<String>,
) -> Result<JsonValue> {
    let cwd = input.get("cwd").and_then(JsonValue::as_str).unwrap_or(".");
    let execution_root = input.get("executionRoot").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused command missing executionRoot"))?;
    let root = ExecutionRoot::new(execution_root)?;
    let started = Instant::now();
    if command_version.cwd_policy != "underExecutionRoot" {
        bail!("unsupported cwd policy for {}: {}", command_version.action_id, command_version.cwd_policy);
    }
    let resolved_cwd = root.resolve_cwd(cwd)?;
    let binary_path = command_version.resolve_binary()?;
    let mut command = Command::new(&binary_path);
    command.args(&argv).current_dir(&resolved_cwd);
    command.process_group(0);
    match command_version.env_policy.as_str() {
        "empty" => {
            command.env_clear();
        }
        "minimalCargo" => {
            command.env_clear();
            if let Ok(value) = std::env::var("PATH") { command.env("PATH", value); }
            if let Ok(value) = std::env::var("HOME") { command.env("HOME", value); }
            if let Ok(value) = std::env::var("CARGO_HOME") { command.env("CARGO_HOME", value); }
            if let Ok(value) = std::env::var("RUSTUP_HOME") { command.env("RUSTUP_HOME", value); }
        }
        other => bail!("unsupported env policy for {}: {other}", command_version.action_id),
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = command.spawn().and_then(|mut child| match command_version.max_runtime {
        Some(max_runtime) => match child.wait_timeout(max_runtime)? {
            Some(_) => child.wait_with_output(),
            None => {
                terminate_process_group(child.id(), command_version.terminate_grace_ms);
                let _ = child.kill();
                let mut output = child.wait_with_output()?;
                output.status = std::process::ExitStatus::from_raw(124 << 8);
                Ok(output)
            }
        },
        None => child.wait_with_output(),
    });
    let (status, exit_status, stdout, stderr) = match output {
        Ok(output) => (
            if output.status.code() == Some(124) { "maxRuntimeExceeded" } else if output.status.success() { "completed" } else { "failed" },
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ),
        Err(error) => ("failed", None, String::new(), error.to_string()),
    };
    let duration_ms = started.elapsed().as_millis() as i64;
    let (stdout, stdout_truncated) = truncate_text(&stdout, command_version.output_limit);
    let (stderr, stderr_truncated) = truncate_text(&stderr, command_version.output_limit);
    let truncation = json!({"stdoutTruncated": stdout_truncated, "stderrTruncated": stderr_truncated, "limitBytes": command_version.output_limit});
    let host_api_call_id = Uuid::new_v4();
    sqlx::query("INSERT INTO host_api_calls (id, script_run_id, api_name, input, status, started_at) VALUES ($1, $2, $3, $4, 'running', $5)")
        .bind(host_api_call_id)
        .bind(script_run_id)
        .bind(&command_version.action_id)
        .bind(input)
        .bind(Utc::now())
        .execute(pool)
        .await?;
    lifecycle::complete_host_api_call(pool, host_api_call_id, TerminalStatus::try_from(status)?, &json!({"stdout": stdout, "stderr": stderr, "exitStatus": exit_status}), duration_ms, &truncation, Utc::now()).await?;
    let command_id = Uuid::new_v4();
    sqlx::query("INSERT INTO command_runs (id, host_api_call_id, binary_name, argv, cwd, status, started_at, max_runtime_ms, command_version_id) VALUES ($1, $2, $3, $4, $5, 'running', $6, $7, $8)")
        .bind(command_id)
        .bind(host_api_call_id)
        .bind(&command_version.binary_name)
        .bind(json!(argv))
        .bind(resolved_cwd.display().to_string())
        .bind(Utc::now())
        .bind(command_version.max_runtime.map(|d| d.as_millis() as i64))
        .bind(command_version.version_id)
        .execute(pool)
        .await?;
    lifecycle::complete_command_run(pool, command_id, TerminalStatus::try_from(status)?, &stdout, &stderr, exit_status, duration_ms, &policy_decision, &truncation, Utc::now()).await?;
    db::append_event(pool, session_id, turn_id, "command", Some(command_id), "command.completed", Some(status), json!({"binary":command_version.binary_name,"binaryPath":binary_path.display().to_string(),"commandVersionId":command_version.version_id,"argv":argv,"cwd":resolved_cwd.display().to_string(),"status":status,"stdout":stdout,"stderr":stderr,"exitStatus":exit_status,"maxRuntimeMs":command_version.max_runtime.map(|d| d.as_millis() as i64),"durationMs":duration_ms,"truncation":truncation,"policyDecision":policy_decision})).await?;
    Ok(json!({"commandRunId": command_id, "hostApiCallId": host_api_call_id, "status": status, "stdout": stdout, "stderr": stderr, "exitStatus": exit_status}))
}

async fn execute_resumed_async_command_with_version(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    script_run_id: Uuid,
    input: &JsonValue,
    policy_decision: JsonValue,
    command_version: CommandVersion,
    argv: Vec<String>,
) -> Result<JsonValue> {
    if !command_version.async_allowed {
        bail!("{} does not allow asynchronous execution", command_version.action_id);
    }
    let cwd = input.get("cwd").and_then(JsonValue::as_str).unwrap_or(".");
    let execution_root = input.get("executionRoot").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused command missing executionRoot"))?;
    let root = ExecutionRoot::new(execution_root)?;
    if command_version.cwd_policy != "underExecutionRoot" {
        bail!("unsupported cwd policy for {}: {}", command_version.action_id, command_version.cwd_policy);
    }
    let resolved_cwd = root.resolve_cwd(cwd)?;
    let binary_path = command_version.resolve_binary()?;
    let mut command = Command::new(&binary_path);
    command.args(&argv).current_dir(&resolved_cwd).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::piped());
    command.process_group(0);
    match command_version.env_policy.as_str() {
        "empty" => {
            command.env_clear();
        }
        "minimalCargo" => {
            command.env_clear();
            if let Ok(value) = std::env::var("PATH") { command.env("PATH", value); }
            if let Ok(value) = std::env::var("HOME") { command.env("HOME", value); }
            if let Ok(value) = std::env::var("CARGO_HOME") { command.env("CARGO_HOME", value); }
            if let Ok(value) = std::env::var("RUSTUP_HOME") { command.env("RUSTUP_HOME", value); }
        }
        other => bail!("unsupported env policy for {}: {other}", command_version.action_id),
    }
    let handle = format!("proc_{}", Uuid::new_v4().simple());
    let process_id = Uuid::new_v4();
    let mut child = command.spawn().with_context(|| format!("failed to resume async command: {}", command_version.action_id))?;
    let child_pid = child.id();
    let stdout_buf = Arc::new(Mutex::new(String::new()));
    let stderr_buf = Arc::new(Mutex::new(String::new()));
    let buffer_limit = command_version.output_buffer_bytes;
    if let Some(mut stdout) = child.stdout.take() {
        let target = Arc::clone(&stdout_buf);
        thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => if let Ok(mut buffer) = target.lock() {
                        buffer.push_str(&String::from_utf8_lossy(&chunk[..n]));
                        enforce_string_buffer_limit(&mut buffer, buffer_limit);
                    },
                    Err(_) => break,
                }
            }
        });
    }
    if let Some(mut stderr) = child.stderr.take() {
        let target = Arc::clone(&stderr_buf);
        thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match stderr.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => if let Ok(mut buffer) = target.lock() {
                        buffer.push_str(&String::from_utf8_lossy(&chunk[..n]));
                        enforce_string_buffer_limit(&mut buffer, buffer_limit);
                    },
                    Err(_) => break,
                }
            }
        });
    }
    PROCESS_MANAGER
        .lock()
        .map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?
        .entry(session_id)
        .or_default()
        .insert(handle.clone(), ManagedProcess {
            id: process_id,
            handle: handle.clone(),
            command_version: command_version.clone(),
            command_version_id: command_version.version_id,
            binary_name: command_version.binary_name.clone(),
            binary_path: binary_path.display().to_string(),
            argv: argv.clone(),
            cwd: resolved_cwd.display().to_string(),
            child,
            stdout: stdout_buf,
            stderr: stderr_buf,
            stdout_flush_cursor: 0,
            stderr_flush_cursor: 0,
            started: Instant::now(),
            started_at: Utc::now(),
            status: "running".to_string(),
            end_of_session_behavior: end_of_session_behavior(&command_version),
            termination_reason: None,
        });
    sqlx::query(
        r#"
        INSERT INTO managed_processes (
            id, handle, session_id, starting_turn_id, command_version_id, binary_name, argv, cwd,
            os_pid, os_pgid, status, start_time, end_of_turn_behavior, end_of_session_behavior, max_runtime_ms, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9, 'running', now(), $10, $11, $12, $13)
        "#,
    )
    .bind(process_id)
    .bind(&handle)
    .bind(session_id)
    .bind(turn_id)
    .bind(command_version.version_id)
    .bind(&command_version.binary_name)
    .bind(json!(argv))
    .bind(resolved_cwd.display().to_string())
    .bind(child_pid as i64)
    .bind(&command_version.end_of_turn_behavior)
    .bind(end_of_session_behavior(&command_version))
    .bind(command_version.max_runtime.map(|d| d.as_millis() as i64))
    .bind(json!({"binaryPath": binary_path.display().to_string(), "policyDecision": policy_decision, "resumed": true, "input": input}))
    .execute(pool)
    .await?;
    if let Some(max_runtime_ms) = command_version.max_runtime.map(|d| d.as_millis() as i64) {
        spawn_max_runtime_supervisor(
            pool.clone(),
            session_id,
            turn_id,
            process_id,
            handle.clone(),
            command_version.version_id,
            max_runtime_ms,
        );
    }
    db::append_event(pool, session_id, turn_id, "process", Some(process_id), "process.started", Some("running"), json!({"handle": handle, "commandVersionId": command_version.version_id, "binary": command_version.binary_name, "argv": argv, "cwd": resolved_cwd.display().to_string(), "resumed": true})).await?;
    let _ = script_run_id;
    Ok(json!({"processId": process_id, "handle": handle, "status": "running", "executionMode": "start"}))
}

fn affected_paths(diff: &str) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            let path = rest.split_whitespace().next().unwrap_or(rest).trim();
            if path != "/dev/null" {
                paths.push(path.strip_prefix("b/").unwrap_or(path).to_string());
            }
        }
    }
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        bail!("patch.apply requires at least one affected file path");
    }
    Ok(paths)
}

fn apply_unified_patch(root: &Path, diff: &str) -> Result<()> {
    let files = parse_patch_files(diff)?;
    for file in files {
        let path = root.join(&file.path);
        let parent = path.parent().ok_or_else(|| anyhow::anyhow!("patch target has no parent"))?;
        let parent = std::fs::canonicalize(parent)?;
        let path = parent.join(path.file_name().ok_or_else(|| anyhow::anyhow!("patch target has no file name"))?);
        if !path.starts_with(root) {
            bail!("patch target escapes execution root: {}", file.path.display());
        }
        reject_git_internal(&path, root, "patch.apply")?;
        let original = std::fs::read_to_string(&path).with_context(|| format!("patch target is not readable: {}", path.display()))?;
        let mut lines: Vec<String> = original.lines().map(ToString::to_string).collect();
        if original.ends_with('\n') {
            lines.push(String::new());
        }
        for hunk in file.hunks {
            let idx = hunk.old_start.saturating_sub(1);
            let mut replacement = Vec::new();
            let mut remove_count = 0usize;
            for op in &hunk.lines {
                match op.kind {
                    ' ' => {
                        let current = lines.get(idx + remove_count).map(String::as_str).unwrap_or("");
                        if current != op.text {
                            bail!("patch context mismatch in {}", path.display());
                        }
                        replacement.push(op.text.clone());
                        remove_count += 1;
                    }
                    '-' => {
                        let current = lines.get(idx + remove_count).map(String::as_str).unwrap_or("");
                        if current != op.text {
                            bail!("patch removal mismatch in {}", path.display());
                        }
                        remove_count += 1;
                    }
                    '+' => replacement.push(op.text.clone()),
                    _ => {}
                }
            }
            lines.splice(idx..idx + remove_count, replacement);
        }
        let mut output = lines.join("\n");
        if output.ends_with('\n') {
            output.pop();
        }
        std::fs::write(&path, output)?;
    }
    Ok(())
}

struct ParsedPatchFile {
    path: PathBuf,
    hunks: Vec<ParsedHunk>,
}

struct ParsedHunk {
    old_start: usize,
    lines: Vec<PatchLine>,
}

struct PatchLine {
    kind: char,
    text: String,
}

fn parse_patch_files(diff: &str) -> Result<Vec<ParsedPatchFile>> {
    let raw: Vec<&str> = diff.lines().collect();
    let mut i = 0usize;
    let mut files = Vec::new();
    while i < raw.len() {
        if !raw[i].starts_with("--- ") {
            i += 1;
            continue;
        }
        i += 1;
        if i >= raw.len() || !raw[i].starts_with("+++ ") {
            bail!("invalid patch: missing +++ header");
        }
        let target = raw[i].trim_start_matches("+++ ").split_whitespace().next().unwrap_or("");
        let target = target.strip_prefix("b/").unwrap_or(target);
        let path = PathBuf::from(target);
        i += 1;
        let mut hunks = Vec::new();
        while i < raw.len() {
            if raw[i].starts_with("--- ") {
                break;
            }
            if !raw[i].starts_with("@@") {
                i += 1;
                continue;
            }
            let header = raw[i];
            let old_start = parse_hunk_old_start(header)?;
            i += 1;
            let mut lines = Vec::new();
            while i < raw.len() && !raw[i].starts_with("@@") && !raw[i].starts_with("--- ") {
                let line = raw[i];
                if line.starts_with('\\') {
                    i += 1;
                    continue;
                }
                let (kind, text) = line.split_at(1);
                let kind = kind.chars().next().unwrap_or(' ');
                if !matches!(kind, ' ' | '-' | '+') {
                    bail!("invalid patch line: {line}");
                }
                lines.push(PatchLine { kind, text: text.to_string() });
                i += 1;
            }
            hunks.push(ParsedHunk { old_start, lines });
        }
        files.push(ParsedPatchFile { path, hunks });
    }
    if files.is_empty() {
        bail!("invalid patch: no file hunks");
    }
    Ok(files)
}

fn parse_hunk_old_start(header: &str) -> Result<usize> {
    let old = header
        .split_whitespace()
        .find(|part| part.starts_with('-'))
        .ok_or_else(|| anyhow::anyhow!("invalid hunk header: {header}"))?;
    let old = old.trim_start_matches('-');
    let start = old.split(',').next().unwrap_or(old).parse::<usize>()?;
    Ok(start.max(1))
}

fn policy_result_from_event_payload(
    payload: &JsonValue,
    role_snapshot: &RoleSnapshot,
) -> Result<crate::policy::PolicyResult> {
    let action = payload
        .get("action")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow::anyhow!("policy event payload missing action"))?;
    let decision = match payload.get("decision").and_then(JsonValue::as_str) {
        Some("allow") => RuntimeDecision::Allow,
        Some("deny") => RuntimeDecision::Deny,
        Some("approvalRequired") => RuntimeDecision::ApprovalRequired,
        Some(other) => bail!("unsupported policy event decision: {other}"),
        None => bail!("policy event payload missing decision"),
    };
    let required_approver_kind = payload
        .get("requiredApproverKind")
        .and_then(JsonValue::as_str)
        .map(crate::approvals::ApproverKind::try_from)
        .transpose()?;
    Ok(crate::policy::PolicyResult {
        action: action.to_string(),
        decision,
        reason: payload
            .get("reason")
            .and_then(JsonValue::as_str)
            .unwrap_or("policy decision")
            .to_string(),
        input: payload.get("input").cloned().unwrap_or_else(|| json!({})),
        role_id: payload
            .get("role")
            .and_then(|role| role.get("id"))
            .and_then(JsonValue::as_str)
            .unwrap_or(&role_snapshot.id)
            .to_string(),
        role_version: payload
            .get("role")
            .and_then(|role| role.get("version"))
            .and_then(JsonValue::as_str)
            .unwrap_or(&role_snapshot.version)
            .to_string(),
        role_version_id: payload
            .get("role")
            .and_then(|role| role.get("roleVersionId"))
            .and_then(JsonValue::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| role_snapshot.role_version_id.to_string()),
        source_decision: payload
            .get("sourceDecision")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
        required_approver_kind,
    })
}

async fn persist_record(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    tool_call_id: Uuid,
    script_run_id: Uuid,
    record: &HostRecord,
    role_snapshot: &RoleSnapshot,
) -> Result<()> {
    match record {
        HostRecord::Policy(policy) => {
            db::append_event(
                pool,
                session_id,
                Some(turn_id),
                "policy",
                None,
                "policy.decision",
                Some(&policy.decision),
                policy.payload.clone(),
            )
            .await?;
            if policy.decision == RuntimeDecision::ApprovalRequired.as_str() {
                let policy_result = policy_result_from_event_payload(&policy.payload, role_snapshot)?;
                let approval_id = approvals::request_approval(pool, session_id, Some(turn_id), &policy_result, role_snapshot).await?;
                if command_registry::is_registry_command_action(&policy_result.action)
                    || matches!(policy_result.action.as_str(), "fs.write" | "patch.apply")
                {
                    approvals::create_paused_action(
                        pool,
                        approval_id,
                        session_id,
                        Some(turn_id),
                        Some(tool_call_id),
                        Some(script_run_id),
                        &policy_result.action,
                        policy_result.input.clone(),
                        role_snapshot,
                    )
                    .await?;
                }
            }
        }
        HostRecord::HostApi(call) => {
            sqlx::query(
                r#"
                INSERT INTO host_api_calls (id, script_run_id, api_name, input, status, started_at)
                VALUES ($1, $2, $3, $4, 'running', $5)
                "#,
            )
            .bind(call.id)
            .bind(script_run_id)
            .bind(&call.action)
            .bind(&call.input)
            .bind(Utc::now())
            .execute(pool)
            .await?;
            lifecycle::complete_host_api_call(
                pool,
                call.id,
                TerminalStatus::try_from(call.status.as_str())?,
                &call.output,
                call.duration_ms,
                &call.truncation,
                Utc::now(),
            )
            .await?;
            db::append_event(
                pool,
                session_id,
                Some(turn_id),
                "host_api",
                Some(call.id),
                "host_api.completed",
                Some(&call.status),
                json!({
                    "api": call.action,
                    "input": call.input,
                    "output": call.output,
                    "durationMs": call.duration_ms,
                    "truncation": call.truncation,
                }),
            )
            .await?;
        }
        HostRecord::Command(command) => {
            sqlx::query(
                r#"
                INSERT INTO command_runs (id, host_api_call_id, binary_name, argv, cwd, status, started_at, max_runtime_ms, command_version_id)
                VALUES ($1, $2, $3, $4, $5, 'running', $6, $7, $8)
                "#,
            )
            .bind(command.id)
            .bind(command.host_api_call_id)
            .bind(&command.binary_name)
            .bind(json!(command.argv))
            .bind(&command.cwd)
            .bind(Utc::now())
            .bind(command.max_runtime_ms)
            .bind(command.command_version_id)
            .execute(pool)
            .await?;
            lifecycle::complete_command_run(
                pool,
                command.id,
                TerminalStatus::try_from(command.status.as_str())?,
                &command.stdout,
                &command.stderr,
                command.exit_status,
                command.duration_ms,
                &command.policy_decision,
                &command.truncation,
                Utc::now(),
            )
            .await?;
            db::append_event(
                pool,
                session_id,
                Some(turn_id),
                "command",
                Some(command.id),
                "command.completed",
                Some(&command.status),
                json!({
                    "binary": command.binary_name,
                    "commandVersionId": command.command_version_id,
                    "binaryPath": command.binary_path,
                    "argv": command.argv,
                    "cwd": command.cwd,
                    "status": command.status,
                    "stdout": command.stdout,
                    "stderr": command.stderr,
                    "exitStatus": command.exit_status,
                    "maxRuntimeMs": command.max_runtime_ms,
                    "durationMs": command.duration_ms,
                    "truncation": command.truncation,
                    "policyDecision": command.policy_decision,
                    "processGroupTermination": {
                        "attempted": command.status == "maxRuntimeExceeded",
                        "reason": if command.status == "maxRuntimeExceeded" { "maxRuntimeExceeded" } else { "naturalExit" },
                    },
                }),
            )
            .await?;
        }
        HostRecord::ManagedProcess(process) => {
            sqlx::query(
                r#"
                INSERT INTO managed_processes (
                    id, handle, session_id, starting_turn_id, command_version_id, binary_name, argv, cwd,
                    os_pid, os_pgid, status, start_time, end_time, end_of_turn_behavior,
                    end_of_session_behavior, max_runtime_ms, termination_reason, metadata
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now(), CASE WHEN $11 = 'running' THEN NULL ELSE now() END, $12, $13, $14, $15, $16)
                ON CONFLICT (handle) DO UPDATE SET
                    status = EXCLUDED.status,
                    end_time = CASE WHEN EXCLUDED.status = 'running' THEN managed_processes.end_time ELSE now() END,
                    termination_reason = EXCLUDED.termination_reason,
                    metadata = managed_processes.metadata || EXCLUDED.metadata
                "#,
            )
            .bind(process.id)
            .bind(&process.handle)
            .bind(session_id)
            .bind(turn_id)
            .bind(process.command_version_id)
            .bind(&process.binary_name)
            .bind(json!(process.argv))
            .bind(&process.cwd)
            .bind(process.os_pid)
            .bind(process.os_pgid)
            .bind(&process.status)
            .bind(&process.end_of_turn_behavior)
            .bind(&process.end_of_session_behavior)
            .bind(process.max_runtime_ms)
            .bind(&process.termination_reason)
            .bind(json!({
                "binaryPath": process.binary_path,
                "lastEvent": process.event,
                "payload": process.payload,
            }))
            .execute(pool)
            .await?;
            if process.event == "process.started" && process.status == "running" {
                if let Some(max_runtime_ms) = process.max_runtime_ms {
                    spawn_max_runtime_supervisor(
                        pool.clone(),
                        session_id,
                        Some(turn_id),
                        process.id,
                        process.handle.clone(),
                        process.command_version_id,
                        max_runtime_ms,
                    );
                }
            }
            db::append_event(
                pool,
                session_id,
                Some(turn_id),
                "process",
                Some(process.id),
                &process.event,
                Some(&process.status),
                json!({
                    "handle": process.handle,
                    "commandVersionId": process.command_version_id,
                    "binary": process.binary_name,
                    "argv": process.argv,
                    "cwd": process.cwd,
                    "pid": process.os_pid,
                    "pgid": process.os_pgid,
                    "endOfTurnBehavior": process.end_of_turn_behavior,
                    "maxRuntimeMs": process.max_runtime_ms,
                    "terminationReason": process.termination_reason,
                    "payload": process.payload,
                }),
            )
            .await?;
        }
        HostRecord::ProcessOutput(output) => {
            sqlx::query(
                r#"
                INSERT INTO process_output_chunks (id, process_id, stream, content, truncated)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(output.process_id)
            .bind(&output.stream)
            .bind(&output.content)
            .bind(output.truncated)
            .execute(pool)
            .await?;
            db::append_event(
                pool,
                session_id,
                Some(turn_id),
                "process",
                Some(output.process_id),
                "process.output",
                Some("completed"),
                json!({
                    "handle": output.handle,
                    "stream": output.stream,
                    "content": output.content,
                    "truncated": output.truncated,
                }),
            )
            .await?;
        }
        HostRecord::ApprovalPause(pause) => {
            let approval_id = approvals::request_approval(pool, session_id, Some(turn_id), &pause.policy, role_snapshot).await?;
            let paused_id = approvals::create_paused_action(
                pool,
                approval_id,
                session_id,
                Some(turn_id),
                Some(tool_call_id),
                Some(script_run_id),
                &pause.action,
                pause.action_input.clone(),
                role_snapshot,
            )
            .await?;
            db::append_event(
                pool,
                session_id,
                Some(turn_id),
                "approval",
                Some(approval_id),
                "approval.paused_action_created",
                Some("pendingApproval"),
                json!({
                    "approvalRequestId": approval_id,
                    "pausedActionId": paused_id,
                    "action": pause.action,
                    "input": pause.action_input,
                }),
            )
            .await?;
        }
        HostRecord::FileMutation(mutation) => {
            sqlx::query(
                r#"
                INSERT INTO file_mutations (
                    id, script_run_id, action_name, path, before_state, after_state, status,
                    error, started_at, completed_at, duration_ms, policy_decision, truncation
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9, $10, $11, $12)
                "#,
            )
            .bind(mutation.id)
            .bind(script_run_id)
            .bind(mutation.action)
            .bind(&mutation.path)
            .bind(&mutation.before_state)
            .bind(&mutation.after_state)
            .bind(&mutation.status)
            .bind(&mutation.error)
            .bind(Utc::now())
            .bind(mutation.duration_ms)
            .bind(&mutation.policy_decision)
            .bind(&mutation.truncation)
            .execute(pool)
            .await?;
            db::append_event(
                pool,
                session_id,
                Some(turn_id),
                "file_mutation",
                Some(mutation.id),
                "file_mutation.completed",
                Some(&mutation.status),
                json!({
                    "action": mutation.action,
                    "path": mutation.path,
                    "before": mutation.before_state,
                    "after": mutation.after_state,
                    "status": mutation.status,
                    "error": mutation.error,
                    "durationMs": mutation.duration_ms,
                    "truncation": mutation.truncation,
                    "policyDecision": mutation.policy_decision,
                }),
            )
            .await?;
        }
        HostRecord::PatchRun(patch) => {
            sqlx::query(
                r#"
                INSERT INTO patch_runs (
                    id, script_run_id, action_name, affected_paths, before_state, after_state,
                    status, error, started_at, completed_at, duration_ms, policy_decision, truncation
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9, $10, $11, $12)
                "#,
            )
            .bind(patch.id)
            .bind(script_run_id)
            .bind(patch.action)
            .bind(&patch.affected_paths)
            .bind(&patch.before_state)
            .bind(&patch.after_state)
            .bind(&patch.status)
            .bind(&patch.error)
            .bind(Utc::now())
            .bind(patch.duration_ms)
            .bind(&patch.policy_decision)
            .bind(&patch.truncation)
            .execute(pool)
            .await?;
            db::append_event(
                pool,
                session_id,
                Some(turn_id),
                "patch",
                Some(patch.id),
                "patch.completed",
                Some(&patch.status),
                json!({
                    "action": patch.action,
                    "affectedPaths": patch.affected_paths,
                    "before": patch.before_state,
                    "after": patch.after_state,
                    "status": patch.status,
                    "error": patch.error,
                    "durationMs": patch.duration_ms,
                    "truncation": patch.truncation,
                    "policyDecision": patch.policy_decision,
                }),
            )
            .await?;
        }
        HostRecord::WorkflowMemory(memory) => {
            crate::workflow_memory::insert_memory_event(
                pool,
                session_id,
                Some(turn_id),
                Some(script_run_id),
                memory.memory_id,
                &memory.event_type,
                memory.payload.clone(),
            )
            .await?;
        }
    }
    Ok(())
}

fn truncate_text(input: &str, limit: usize) -> (String, bool) {
    if input.len() <= limit {
        return (input.to_string(), false);
    }
    let mut end = limit;
    while !input.is_char_boundary(end) {
        end -= 1;
    }
    (input[..end].to_string(), true)
}

fn enforce_string_buffer_limit(buffer: &mut String, limit: usize) {
    if limit == 0 || buffer.len() <= limit {
        return;
    }
    let mut start = buffer.len() - limit;
    while !buffer.is_char_boundary(start) {
        start += 1;
    }
    buffer.drain(..start);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::{
        LifecycleAuthorityMetadata, ManifestDecision, ModelDefaults, RoleSnapshot, RoutingMetadata,
        VisibilityMetadata,
    };
    use chrono::Utc;
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;
    use std::collections::BTreeMap;

    fn test_pool() -> PgPool {
        PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime_test")
            .expect("test pool URL must parse")
    }

    fn test_role() -> RoleSnapshot {
        let mut policy = BTreeMap::new();
        policy.insert("fs.read".to_string(), ManifestDecision::Allow);
        policy.insert("fs.write".to_string(), ManifestDecision::Allow);
        policy.insert("workflow_memory.search".to_string(), ManifestDecision::Allow);
        policy.insert("workflow_memory.remember.project".to_string(), ManifestDecision::Allow);
        policy.insert("workflow_memory.feedback".to_string(), ManifestDecision::Allow);
        RoleSnapshot {
            id: "test-role".to_string(),
            version: "1.0.0".to_string(),
            display_name: "Test Role".to_string(),
            role_version_id: Uuid::new_v4(),
            instruction_text: "test".to_string(),
            model_defaults: ModelDefaults { model: "test".to_string(), reasoning_effort: "low".to_string() },
            capabilities: vec![],
            policy,
            routing: RoutingMetadata { mode: "direct".to_string(), default_recipient: None, allowed_recipients: vec![], reserved_actions: vec![] },
            visibility: VisibilityMetadata { listed: false, owner_visible: false },
            lifecycle_authority: LifecycleAuthorityMetadata { can_spawn_agents: false, can_archive_agents: false, reserved_actions: vec![] },
            manifest: json!({}),
            created_at: Utc::now(),
        }
    }

    fn test_command(binary_name: &str, paths: &[&str], action: &str, object: &str, stdin_policy: &str, end_of_turn_behavior: &str, max_runtime: Option<Duration>) -> CommandVersion {
        CommandVersion {
            version_id: Uuid::new_v4(),
            definition_id: Uuid::new_v4(),
            action_id: action.to_string(),
            binary_name: binary_name.to_string(),
            candidate_paths: paths.iter().map(PathBuf::from).collect(),
            starlark_object: object.to_string(),
            starlark_method: "run".to_string(),
            argv_prefix: vec![],
            default_cwd: ".".to_string(),
            cwd_policy: "underExecutionRoot".to_string(),
            env_policy: "empty".to_string(),
            max_runtime,
            output_limit: 4096,
            mutation_class: "readOnly".to_string(),
            model_description: "test command".to_string(),
            allow_cwd_arg: true,
            allow_args_arg: true,
            forbidden_args: vec![],
            execution_policy: "allow".to_string(),
            sync_allowed: true,
            async_allowed: true,
            end_of_turn_behavior: end_of_turn_behavior.to_string(),
            stdin_policy: stdin_policy.to_string(),
            min_await_ms: 100,
            max_await_ms: 60_000,
            output_buffer_bytes: 4096,
            terminate_grace_ms: 10,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn continuing_process_is_controllable_later_in_same_runtime_and_isolated_by_session() {
        let session = Uuid::new_v4();
        let other_session = Uuid::new_v4();
        let root = ExecutionRoot::new(".").unwrap();
        let cmd = test_command("yes", &["/usr/bin/yes"], "cmd.yes.run", "yes", "forbid", "continue", Some(Duration::from_millis(500)));
        let first = evaluate_starlark(test_pool(), session, Uuid::new_v4(), "h = cmd[\"yes\"].run(args=[], cwd=\".\").start(); output(h)", root.clone(), test_role(), vec![cmd.clone()], vec![]);
        assert!(first.error.is_none(), "{:?}", first.error);
        let handle = first.output.trim().trim_matches('"').to_string();
        assert!(handle.starts_with("proc_"));
        assert!(first.records.iter().any(|record| matches!(record, HostRecord::ManagedProcess(process) if process.event == "process.continued")));

        let second = evaluate_starlark(test_pool(), session, Uuid::new_v4(), &format!("proc[{handle:?}].await_for(mins=0); out = proc[{handle:?}].flush_buffer(); proc[{handle:?}].terminate(); output(out)"), root.clone(), test_role(), vec![cmd.clone()], vec![handle.clone()]);
        assert!(second.error.is_none(), "{:?}", second.error);
        assert!(second.output.contains('y'));
        assert!(second.records.iter().any(|record| matches!(record, HostRecord::ManagedProcess(process) if process.event == "process.terminated")));

        let isolated = evaluate_starlark(test_pool(), other_session, Uuid::new_v4(), &format!("proc[{handle:?}].is_running(); output(\"bad\")"), root, test_role(), vec![cmd], vec![handle]);
        assert!(isolated.error.unwrap_or_default().contains("session-only process no longer attached"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stdin_allowed_process_accepts_input_and_flush_cursor_advances() {
        let session = Uuid::new_v4();
        let root = ExecutionRoot::new(".").unwrap();
        let cmd = test_command("cat", &["/bin/cat"], "cmd.cat.run", "cat", "allow", "terminate", Some(Duration::from_secs(2)));
        let script = r#"
h = cmd["cat"].run(args=[], cwd=".").start()
proc[h].input("hello-process\n")
proc[h].await_for(mins=0)
first = proc[h].flush_buffer()
second = proc[h].flush_buffer()
proc[h].terminate()
output(first + "|second=" + second)
"#;
        let result = evaluate_starlark(test_pool(), session, Uuid::new_v4(), script, root, test_role(), vec![cmd], vec![]);
        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(result.output.contains("hello-process"));
        assert!(result.output.contains("|second="));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn async_max_runtime_expires_without_handle_polling_and_detached_handle_errors_are_clear() {
        let session = Uuid::new_v4();
        let root = ExecutionRoot::new(".").unwrap();
        let cmd = test_command("yes", &["/usr/bin/yes"], "cmd.yes.max.run", "yes_max", "forbid", "continue", Some(Duration::from_millis(100)));
        let first = evaluate_starlark(test_pool(), session, Uuid::new_v4(), "h = cmd[\"yes_max\"].run(args=[], cwd=\".\").start(); output(h)", root.clone(), test_role(), vec![cmd.clone()], vec![]);
        assert!(first.error.is_none(), "{:?}", first.error);
        let handle = first.output.trim().trim_matches('"').to_string();
        thread::sleep(Duration::from_millis(250));
        let second = evaluate_starlark(test_pool(), session, Uuid::new_v4(), &format!("running = proc[{handle:?}].is_running(); output(str(running))"), root.clone(), test_role(), vec![cmd.clone()], vec![handle.clone()]);
        assert!(second.error.is_none(), "{:?}", second.error);
        assert!(second.records.iter().any(|record| matches!(record, HostRecord::ManagedProcess(process) if process.status == "maxRuntimeExceeded" || process.event == "process.continued")));

        PROCESS_MANAGER.lock().unwrap().clear();
        let detached = evaluate_starlark(test_pool(), session, Uuid::new_v4(), &format!("proc[{handle:?}].is_running(); output(\"bad\")"), root, test_role(), vec![cmd], vec![handle]);
        assert!(detached.error.unwrap_or_default().contains("session-only process no longer attached"));
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn workflow_memory_deterministic_validation() {
        let url = std::env::var("ROBDEX_AGENT_RUNTIME_DATABASE_URL").expect("validation database URL must be set");
        let pool = PgPoolOptions::new().connect(&url).await.unwrap();
        crate::db::init(&pool).await.unwrap();
        let role = test_role();
        let root_dir = std::env::temp_dir().join(format!("agent-runtime-workflow-memory-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root_dir).unwrap();
        std::fs::write(root_dir.join("seed.txt"), "seed").unwrap();
        let root = ExecutionRoot::new(&root_dir).unwrap();
        let seed_session = crate::db::new_session(&pool, &role, Some("alpha"), root_dir.to_str().unwrap(), None, None, None).await.unwrap();
        let fail_session = crate::db::new_session(&pool, &role, Some("alpha"), root_dir.to_str().unwrap(), None, None, None).await.unwrap();
        let beta_session = crate::db::new_session(&pool, &role, Some("beta"), root_dir.to_str().unwrap(), None, None, None).await.unwrap();
        let mut deny_role = role.clone();
        deny_role.policy.insert("workflow_memory.remember.project".to_string(), ManifestDecision::Deny);
        let deny_session = crate::db::new_session(&pool, &deny_role, Some("alpha"), root_dir.to_str().unwrap(), None, None, None).await.unwrap();

        async fn run_script(pool: &PgPool, session: Uuid, root: &ExecutionRoot, role: &RoleSnapshot, source: &str) -> Result<serde_json::Value> {
            let turn_id = Uuid::new_v4();
            let tool_call_id = Uuid::new_v4();
            sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user',$3,'running',now())")
                .bind(turn_id)
                .bind(session)
                .bind(source)
                .execute(pool)
                .await?;
            sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status, started_at) VALUES ($1,$2,$3,'execute_code',$4,$5,'running',now())")
                .bind(tool_call_id)
                .bind(session)
                .bind(turn_id)
                .bind(format!("call_{tool_call_id}"))
                .bind(json!({"source": source}))
                .execute(pool)
                .await?;
            let packet = execute_code(pool, session, turn_id, tool_call_id, source, root, role).await?;
            Ok(serde_json::to_value(packet)?)
        }

        let promote_source = r#"fs.write("memory-target.txt", "needle workflow memory success")
text = fs.read("memory-target.txt")
workflow_memory.remember_when(text == "needle workflow memory success", "Write memory target", "Use fs.write then fs.read to verify exact content after missing-file failures")
output("promoted")"#;
        run_script(&pool, seed_session, &root, &role, promote_source).await.unwrap();
        let memory_id: Uuid = sqlx::query_scalar("SELECT id FROM workflow_memories WHERE session_id=$1 LIMIT 1").bind(seed_session).fetch_one(&pool).await.unwrap();
        run_script(&pool, seed_session, &root, &role, promote_source).await.unwrap();
        let memory_count: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memories WHERE session_id=$1").bind(seed_session).fetch_one(&pool).await.unwrap();
        assert_eq!(memory_count, 1);
        let duplicate_events: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.duplicate_collapsed'").bind(seed_session).fetch_one(&pool).await.unwrap();
        assert!(duplicate_events > 0);

        let failed = run_script(&pool, fail_session, &root, &role, r#"fs.read("missing-workflow-memory-file.txt")
output("unreachable")"#).await;
        assert!(failed.is_err());
        let failed_embeddings: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_script_embeddings WHERE session_id=$1").bind(fail_session).fetch_one(&pool).await.unwrap();
        assert!(failed_embeddings > 0);
        let failed_promotions: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memories WHERE session_id=$1").bind(fail_session).fetch_one(&pool).await.unwrap();
        assert_eq!(failed_promotions, 0);

        run_script(&pool, fail_session, &root, &role, r#"tips = workflow_memory.help()
output(tips)"#).await.unwrap();
        let help_results: i64 = sqlx::query_scalar("SELECT (payload->>'resultCount')::bigint FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.help' ORDER BY created_at DESC LIMIT 1").bind(fail_session).fetch_one(&pool).await.unwrap();
        assert!(help_results > 0);

        run_script(&pool, fail_session, &root, &role, &format!(r#"workflow_memory.mark_attempted("{memory_id}", variant=True)
workflow_memory.mark_not_helpful("{memory_id}", "not enough context")
output("feedback")"#)).await.unwrap();
        let feedback_events: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_events WHERE memory_id=$1 AND event_type IN ('workflow_memory.mark_attempted','workflow_memory.mark_not_helpful')").bind(memory_id).fetch_one(&pool).await.unwrap();
        assert_eq!(feedback_events, 2);

        let denied = run_script(&pool, deny_session, &root, &deny_role, r#"workflow_memory.remember_when(True, "Denied memory", "restrictive role should block")
output("denied")"#).await;
        assert!(denied.is_err());
        let denied_memories: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memories WHERE session_id=$1").bind(deny_session).fetch_one(&pool).await.unwrap();
        assert_eq!(denied_memories, 0);

        let invisible_feedback = run_script(&pool, beta_session, &root, &role, &format!(r#"workflow_memory.mark_attempted("{memory_id}", variant=True)
output("bad")"#)).await;
        assert!(invisible_feedback.unwrap_err().to_string().contains("not visible"));

        let beta_failed = run_script(&pool, beta_session, &root, &role, r#"fs.read("missing-workflow-memory-file.txt")
output("unreachable")"#).await;
        assert!(beta_failed.is_err());
        run_script(&pool, beta_session, &root, &role, r#"tips = workflow_memory.help()
output(tips)"#).await.unwrap();
        let beta_help_results: i64 = sqlx::query_scalar("SELECT (payload->>'resultCount')::bigint FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.help' ORDER BY created_at DESC LIMIT 1").bind(beta_session).fetch_one(&pool).await.unwrap();
        assert_eq!(beta_help_results, 0);
        let beta_help_mentions_alpha: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.help' AND payload::text LIKE $2").bind(beta_session).bind(format!("%{memory_id}%")).fetch_one(&pool).await.unwrap();
        assert_eq!(beta_help_mentions_alpha, 0);

        let ordinary_before: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.help'").bind(seed_session).fetch_one(&pool).await.unwrap();
        run_script(&pool, seed_session, &root, &role, r#"output("ordinary no help")"#).await.unwrap();
        let ordinary_after: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.help'").bind(seed_session).fetch_one(&pool).await.unwrap();
        assert_eq!(ordinary_before, ordinary_after);
    }
}
