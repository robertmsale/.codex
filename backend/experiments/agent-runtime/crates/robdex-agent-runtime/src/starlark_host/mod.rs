use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::net::{TcpListener, TcpStream};
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
use starlark::values::tuple::UnpackTuple;
use starlark_map::small_map::SmallMap;
use uuid::Uuid;
use wait_timeout::ChildExt;

use crate::db;
use crate::approvals;
use crate::command_registry::{self, CommandVersion};
use crate::lifecycle::{self, TerminalStatus};
use crate::output_artifacts::{self, NewOutputArtifact};
use crate::policy::{PolicyEngine, RuntimeDecision};
use crate::roles::RoleSnapshot;
use crate::workflow_memory::RememberCandidate;
use crate::lifecycle_hooks;

const OUTPUT_LIMIT_BYTES: usize = 12_000;
const STARTER_FILE_LINE_LIMIT: usize = 400;
const STARTER_TREE_RESULT_LIMIT: usize = 250;
const STARTER_MAX_MUTATION_DESCRIPTION: usize = 220;
const FAILED_EXECUTE_CODE_RECOVERY_HINT: &str =
    "Hint: run print(workflow_memory.help()) to search prior successful patterns for the last failed script.";

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

pub fn terminate_all_runtime_processes(reason: &str) -> usize {
    let Ok(mut manager) = PROCESS_MANAGER.lock() else {
        return 0;
    };
    let mut terminated = 0usize;
    for processes in manager.values_mut() {
        for process in processes.values_mut() {
            if process.status == "running" {
                let _ = process.terminate(reason, true);
                terminated += 1;
            }
        }
    }
    manager.clear();
    terminated
}

pub async fn reconcile_starter_server_leases(pool: &PgPool) -> Result<usize> {
    let rows: Vec<(Uuid, String, i32)> = sqlx::query_as(
        "SELECT session_id, handle, port FROM starter_managed_servers WHERE status='running'",
    )
    .fetch_all(pool)
    .await?;
    let mut lost = Vec::new();
    {
        let mut manager = PROCESS_MANAGER
            .lock()
            .map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
        for (session_id, handle, port) in rows {
            let attached_running = manager
                .get_mut(&session_id)
                .and_then(|processes| processes.get_mut(&handle))
                .map(|process| process.refresh_status().unwrap_or(false))
                .unwrap_or(false);
            if !attached_running {
                lost.push((session_id, handle, port));
            }
        }
    }
    let mut released = 0usize;
    for (session_id, handle, port) in lost {
        let updated = sqlx::query("UPDATE starter_managed_servers SET status='lost', updated_at=now() WHERE session_id=$1 AND handle=$2 AND status='running'")
            .bind(session_id)
            .bind(&handle)
            .execute(pool)
            .await?
            .rows_affected();
        if updated > 0 {
            sqlx::query("UPDATE starter_port_leases SET status='released', released_at=COALESCE(released_at, now()), release_reason='runtime.reconcile' WHERE session_id=$1 AND allocated_port=$2 AND status='active'")
                .bind(session_id)
                .bind(port)
                .execute(pool)
                .await?;
            released += 1;
        }
    }
    Ok(released)
}

#[cfg(test)]
pub fn register_test_terminable_process(session_id: Uuid) -> Result<(Uuid, String)> {
    let id = Uuid::new_v4();
    let handle = format!("proc_{}", Uuid::new_v4().simple());
    let command_version = CommandVersion {
        version_id: Uuid::new_v4(),
        definition_id: Uuid::new_v4(),
        scope_type: "global".to_string(),
        project_key: None,
        action_id: "cmd.test.sleep".to_string(),
        binary_name: "sleep".to_string(),
        candidate_paths: vec![PathBuf::from("/bin/sleep")],
        starlark_object: "sleep".to_string(),
        starlark_method: "run".to_string(),
        argv_prefix: vec!["30".to_string()],
        default_cwd: ".".to_string(),
        cwd_policy: "allow".to_string(),
        env_policy: "inherit".to_string(),
        max_runtime: Some(Duration::from_secs(60)),
        output_limit: OUTPUT_LIMIT_BYTES,
        mutation_class: "read".to_string(),
        model_description: "test sleep".to_string(),
        allow_cwd_arg: true,
        allow_args_arg: true,
        forbidden_args: vec![],
        execution_policy: "allow".to_string(),
        sync_allowed: true,
        async_allowed: true,
        end_of_turn_behavior: "continue".to_string(),
        end_of_session_behavior: "terminate".to_string(),
        stdin_policy: "forbid".to_string(),
        min_await_ms: 0,
        max_await_ms: 1000,
        output_buffer_bytes: OUTPUT_LIMIT_BYTES,
        terminate_grace_ms: 50,
    };
    let mut command = Command::new("/bin/sleep");
    command
        .arg("30")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped())
        .process_group(0);
    let child = command.spawn().context("spawn test terminable process")?;
    let process = ManagedProcess {
        id,
        handle: handle.clone(),
        command_version_id: Some(command_version.version_id),
        binary_name: command_version.binary_name.clone(),
        binary_path: "/bin/sleep".to_string(),
        argv: vec!["30".to_string()],
        cwd: ".".to_string(),
        child,
        stdout: Arc::new(Mutex::new(String::new())),
        stderr: Arc::new(Mutex::new(String::new())),
        stdout_flush_cursor: 0,
        stderr_flush_cursor: 0,
        started: Instant::now(),
        started_at: Utc::now(),
        status: "running".to_string(),
        end_of_turn_behavior: command_version.end_of_turn_behavior.clone(),
        end_of_session_behavior: "terminate".to_string(),
        max_runtime: command_version.max_runtime,
        min_await_ms: command_version.min_await_ms,
        max_await_ms: command_version.max_await_ms,
        terminate_grace_ms: command_version.terminate_grace_ms,
        output_limit: command_version.output_limit,
        stdin_policy: command_version.stdin_policy.clone(),
        termination_reason: None,
    };
    let mut manager = PROCESS_MANAGER
        .lock()
        .map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
    manager
        .entry(session_id)
        .or_default()
        .insert(handle.clone(), process);
    Ok((id, handle))
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
        self.resolve_agent_path(path, "fs.read", true)
    }

    fn resolve_write_path(&self, path: &str) -> Result<PathBuf> {
        self.resolve_agent_path(path, "fs.write", false)
    }

    fn validate_patch_path(&self, path: &str) -> Result<PathBuf> {
        if path.trim().is_empty() || path == "/dev/null" {
            bail!("patch path must not be empty or /dev/null in this phase");
        }
        let relative = path.strip_prefix("a/").or_else(|| path.strip_prefix("b/")).unwrap_or(path);
        self.resolve_agent_path(relative, "patch.apply", false)
    }

    pub fn as_path(&self) -> &Path {
        &self.root
    }

    fn resolve_agent_path(&self, path: &str, action: &str, must_exist: bool) -> Result<PathBuf> {
        let raw = path.trim();
        if raw.is_empty() {
            bail!("{action} path error: rejectedPath={path:?}; resolutionRoot={}; reason=emptyPath", self.root.display());
        }
        let requested = Path::new(raw);
        let mut normalized = PathBuf::new();
        for component in requested.components() {
            match component {
                Component::Prefix(_) => bail!("{action} path error: rejectedPath={path}; resolutionRoot={}; reason=unsupportedPathPrefix", self.root.display()),
                Component::RootDir => {
                    if !requested.is_absolute() {
                        bail!("{action} path error: rejectedPath={path}; resolutionRoot={}; reason=invalidRootComponent", self.root.display());
                    }
                }
                Component::CurDir => {}
                Component::ParentDir => bail!("{action} path error: rejectedPath={path}; resolutionRoot={}; reason=parentTraversalRejected", self.root.display()),
                Component::Normal(part) => normalized.push(part),
            }
        }
        let candidate = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.root.join(normalized)
        };
        let resolved = if must_exist {
            std::fs::canonicalize(&candidate)
                .with_context(|| format!("{action} path error: rejectedPath={path}; resolutionRoot={}; reason=notAccessible", self.root.display()))?
        } else {
            let parent = candidate
                .parent()
                .ok_or_else(|| anyhow::anyhow!("{action} path error: rejectedPath={path}; resolutionRoot={}; reason=noParent", self.root.display()))?;
            let parent = std::fs::canonicalize(parent)
                .with_context(|| format!("{action} path error: rejectedPath={path}; resolutionRoot={}; reason=parentNotAccessible", self.root.display()))?;
            parent.join(candidate.file_name().ok_or_else(|| anyhow::anyhow!("{action} path error: rejectedPath={path}; resolutionRoot={}; reason=noFileName", self.root.display()))?)
        };
        if !resolved.starts_with(&self.root) {
            bail!("{action} path error: rejectedPath={path}; resolutionRoot={}; reason=escapesExecutionRoot", self.root.display());
        }
        reject_git_internal(&resolved, &self.root, action)?;
        Ok(resolved)
    }
}

fn reject_git_internal(path: &Path, root: &Path, action: &str) -> Result<()> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    if rel.components().any(|component| component.as_os_str() == ".git") {
        bail!("{action} path error: rejectedPath={}; resolutionRoot={}; reason=gitInternalsRejected", rel.display(), root.display());
    }
    Ok(())
}

fn require_mutation_description(action: &str, description: &str) -> Result<()> {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        bail!("{action} description must be a short non-empty description of the intended mutation");
    }
    if trimmed.len() > STARTER_MAX_MUTATION_DESCRIPTION {
        bail!("{action} description is too long; maximum is {STARTER_MAX_MUTATION_DESCRIPTION} bytes");
    }
    let generic = ["change file", "update", "fix", "edit", "misc", "stuff"];
    if generic.iter().any(|value| value.eq_ignore_ascii_case(trimmed)) {
        bail!("{action} description is too generic; describe the concrete intended mutation");
    }
    Ok(())
}

fn text_file_content(action: &str, path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("{action} failed to read {}", path.display()))?;
    if bytes.iter().take(8192).any(|b| *b == 0) {
        bail!("{action} rejected binary content by default: {}", path.display());
    }
    String::from_utf8(bytes).with_context(|| format!("{action} rejected non-UTF-8 binary content by default: {}", path.display()))
}

fn line_count(text: &str) -> usize {
    if text.is_empty() { 0 } else { text.lines().count().max(1) }
}

fn bounded_lines(text: &str, start: usize, end: usize) -> (String, bool, usize) {
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    let safe_start = start.max(1);
    let safe_end = end.min(total).max(safe_start.saturating_sub(1));
    let mut selected = Vec::new();
    for (idx, line) in lines.iter().enumerate().take(safe_end).skip(safe_start - 1) {
        selected.push(format!("{}: {}", idx + 1, line));
        if selected.len() >= STARTER_FILE_LINE_LIMIT {
            return (selected.join("\n"), true, total);
        }
    }
    let result = selected.join("\n");
    let truncated = result.len() > OUTPUT_LIMIT_BYTES;
    let (result, byte_truncated) = truncate_text(&result, OUTPUT_LIMIT_BYTES);
    (result, truncated || byte_truncated || safe_end < total, total)
}

fn artifact_id_from_envelope(envelope: &output_artifacts::OutputArtifactEnvelope) -> String {
    envelope.artifact_id.to_string()
}

fn simple_glob_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        return value.starts_with(prefix) && value.ends_with(suffix);
    }
    pattern == value
}

fn image_mime_and_dimensions(bytes: &[u8], path: &str) -> (String, Option<i32>, Option<i32>) {
    if bytes.len() >= 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let width = i32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let height = i32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        return ("image/png".to_string(), Some(width), Some(height));
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return ("image/jpeg".to_string(), None, None);
    }
    let mime = match Path::new(path).extension().and_then(|ext| ext.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    };
    (mime.to_string(), None, None)
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
    output: JsonValue,
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
    god_mode_grant_id: Option<Uuid>,
    role_snapshot: RoleSnapshot,
    memory_candidates: RefCell<Vec<RememberCandidate>>,
    output: RefCell<Vec<String>>,
    records: RefCell<Vec<HostRecord>>,
}

#[derive(Debug)]
struct ManagedProcess {
    id: Uuid,
    handle: String,
    command_version_id: Option<Uuid>,
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
    end_of_turn_behavior: String,
    end_of_session_behavior: String,
    max_runtime: Option<Duration>,
    min_await_ms: i64,
    max_await_ms: i64,
    terminate_grace_ms: i64,
    output_limit: usize,
    stdin_policy: String,
    termination_reason: Option<String>,
}

#[derive(Debug)]
struct ManagedProcessRecord {
    id: Uuid,
    handle: String,
    command_version_id: Option<Uuid>,
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
    artifact_id: Uuid,
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
    Shell(ShellRecord),
    ManagedProcess(ManagedProcessRecord),
    ProcessOutput(ProcessOutputRecord),
    ApprovalPause(ApprovalPauseRecord),
    FileMutation(FileMutationRecord),
    PatchRun(PatchRunRecord),
    WorkflowMemory(WorkflowMemoryRecord),
}

#[derive(Debug)]
struct ShellRecord {
    id: Uuid,
    god_mode_grant_id: Option<Uuid>,
    invocation_mode: String,
    shell_path: String,
    script: String,
    cwd: String,
    status: String,
    stdout_artifact_id: Option<Uuid>,
    stderr_artifact_id: Option<Uuid>,
    process_id: Option<Uuid>,
    exit_status: Option<i32>,
    failure: Option<String>,
    duration_ms: i64,
    metadata: JsonValue,
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
    stdout_artifact_id: Uuid,
    stderr_artifact_id: Uuid,
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
    let god_mode_grant_id = crate::god_mode::active_grant(pool, session_id).await?.map(|grant| grant.id);
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
        god_mode_grant_id,
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
    for record in &records {
        if let HostRecord::ManagedProcess(process) = record {
            if process.event == "server.started" {
                sqlx::query("UPDATE starter_managed_servers SET process_id=$3 WHERE session_id=$1 AND handle=$2")
                    .bind(session_id)
                    .bind(&process.handle)
                    .bind(process.id)
                    .execute(pool)
                    .await?;
            }
        }
    }

    let full_final_output = final_output;
    let full_stderr = if status == TerminalStatus::Failed && stderr.trim().is_empty() {
        FAILED_EXECUTE_CODE_RECOVERY_HINT.to_string()
    } else if status == TerminalStatus::Failed {
        format!("{stderr}\n{FAILED_EXECUTE_CODE_RECOVERY_HINT}")
    } else {
        stderr
    };
    let final_artifact_id = Uuid::new_v4();
    let stderr_artifact_id = Uuid::new_v4();
    let final_envelope = output_artifacts::store(pool, NewOutputArtifact {
        id: final_artifact_id,
        session_id,
        turn_id: Some(turn_id),
        tool_call_id: Some(tool_call_id),
        script_run_id: Some(script_run_id),
        command_run_id: None,
        process_id: None,
        source_type: "script_run",
        stream: "stdout",
        content: &full_final_output,
        metadata: json!({"role": "finalOutput"}),
    }).await?;
    let stderr_envelope = output_artifacts::store(pool, NewOutputArtifact {
        id: stderr_artifact_id,
        session_id,
        turn_id: Some(turn_id),
        tool_call_id: Some(tool_call_id),
        script_run_id: Some(script_run_id),
        command_run_id: None,
        process_id: None,
        source_type: "script_run",
        stream: "stderr",
        content: &full_stderr,
        metadata: json!({"role": "scriptError"}),
    }).await?;
    let (final_output, final_truncated) = truncate_text(&full_final_output, OUTPUT_LIMIT_BYTES);
    let (stderr, stderr_truncated) = truncate_text(&full_stderr, OUTPUT_LIMIT_BYTES);
    let script_truncation = json!({
        "finalOutputTruncated": final_truncated,
        "stderrTruncated": stderr_truncated,
        "limitBytes": OUTPUT_LIMIT_BYTES,
        "artifactIds": {
            "stdout": final_artifact_id,
            "stderr": stderr_artifact_id
        },
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
        json!({
            "artifacts": {
                "stdout": final_envelope,
                "stderr": stderr_envelope,
            },
            "preview": final_output,
            "stderrPreview": stderr,
        }),
    )
    .await?;

    if status != TerminalStatus::Failed {
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
    }

    let output = execute_code_result_output(json!(final_envelope), json!(stderr_envelope), status == TerminalStatus::Failed);
    Ok(ExecuteCodePacket {
        ok: status != TerminalStatus::Failed,
        status: status.as_str().to_string(),
        output,
        script_run_id,
        host_api_calls: records
            .iter()
            .filter(|record| matches!(record, HostRecord::HostApi(_)))
            .count(),
    })
}

fn execute_code_result_output(stdout_artifact: JsonValue, stderr_artifact: JsonValue, failed: bool) -> JsonValue {
    let mut output = json!({
        "stdoutArtifact": stdout_artifact,
        "stderrArtifact": stderr_artifact,
        "message": "Full execute_code stdout and stderr are stored as separate durable output artifacts. Use outputs.head/tail/slice/search/stats with an artifact id for bounded retrieval.",
    });
    if failed {
        output["hint"] = JsonValue::String(FAILED_EXECUTE_CODE_RECOVERY_HINT.to_string());
    }
    output
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
    god_mode_grant_id: Option<Uuid>,
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
    let source = command_registry::normalize_describe_affordances(source, &live_commands);
    let shell_prelude = r#"
def __shell_start(script, mode="-lc", cwd="."):
    handle = __shell.start(script, mode, cwd)
    proc[handle] = __proc_obj(handle)
    return handle
def shell(script, mode="-lc", cwd="."):
    return struct(**{"sync": lambda: __shell.sync(script, mode, cwd), "async": lambda: __shell_start(script, mode, cwd), "async_": lambda: __shell_start(script, mode, cwd)})
"#;
    let source = source.replace(".async()", ".async_()");
    let script = format!("{prelude}\n{shell_prelude}\n{source}");
    let kernel = HostKernel {
        pool,
        session_id,
        script_run_id,
        root,
        commands: live_commands.into_iter().map(|command| (command.action_id.clone(), command)).collect(),
        god_mode_grant_id,
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
    builder.namespace("file", file_builtins);
    builder.namespace("tree", tree_builtins);
    builder.namespace("git", git_builtins);
    builder.namespace("server", server_builtins);
    builder.namespace("image", image_builtins);
    builder.namespace("tooling", tooling_builtins);
    builder.namespace("patch", patch_builtins);
    builder.namespace_no_docs("__cmd", cmd_dynamic_builtins);
    builder.namespace_no_docs("__proc", proc_dynamic_builtins);
    builder.namespace_no_docs("__shell", shell_dynamic_builtins);
    builder.namespace("workflow_memory", workflow_memory_builtins);
    builder.namespace("project_runtime", project_runtime_builtins);
    builder.namespace("outputs", output_artifact_builtins);
    struct_builtins(builder);
    print_builtins(builder);
}

#[starlark_module]
fn project_runtime_builtins(builder: &mut GlobalsBuilder) {
    fn request_config_change<'v>(
        project: &'v str,
        source: &'v str,
        manifest_json: &'v str,
        rationale: &'v str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        host_kernel(eval).request_project_runtime_config_change(project, source, manifest_json, rationale)
    }
}

#[starlark_module]
fn shell_dynamic_builtins(builder: &mut GlobalsBuilder) {
    fn sync<'v>(
        script: &'v str,
        #[starlark(default = "-lc")] mode: &'v str,
        #[starlark(default = ".")] cwd: &'v str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        host_kernel(eval).run_shell_sync(script, mode, cwd)
    }

    fn start<'v>(
        script: &'v str,
        #[starlark(default = "-lc")] mode: &'v str,
        #[starlark(default = ".")] cwd: &'v str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        host_kernel(eval).start_shell_async(script, mode, cwd)
    }
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

fn spawn_reader(stream: Option<impl Read + Send + 'static>, target: Arc<Mutex<String>>) {
    if let Some(mut stream) = stream {
        thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(mut buffer) = target.lock() {
                            buffer.push_str(&String::from_utf8_lossy(&chunk[..n]));
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
}

pub async fn terminate_managed_process(pool: &PgPool, session_id: Uuid, handle: &str) -> Result<JsonValue> {
    let record = {
        let mut manager = PROCESS_MANAGER.lock().map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
        let proc = manager
            .get_mut(&session_id)
            .and_then(|processes| processes.get_mut(handle))
            .ok_or_else(|| anyhow::anyhow!("session process is not attached to this runtime: {handle}"))?;
        proc.terminate("terminated", true)?;
        proc.snapshot_record("process.terminated", json!({"handle": handle, "terminateGraceMs": proc.terminate_grace_ms}))
    };
    persist_managed_process_control_record(pool, session_id, record).await?;
    Ok(json!({"handle": handle, "status": "terminated"}))
}

pub async fn input_managed_process(pool: &PgPool, session_id: Uuid, handle: &str, text: &str) -> Result<JsonValue> {
    let record = {
        let mut manager = PROCESS_MANAGER.lock().map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
        let proc = manager
            .get_mut(&session_id)
            .and_then(|processes| processes.get_mut(handle))
            .ok_or_else(|| anyhow::anyhow!("session process is not attached to this runtime: {handle}"))?;
        if proc.stdin_policy != "allow" {
            bail!("process input is not allowed for this handle");
        }
        if let Some(stdin) = proc.child.stdin.as_mut() {
            stdin.write_all(text.as_bytes())?;
            stdin.flush()?;
        } else {
            bail!("process input is no longer attached");
        }
        proc.snapshot_record("process.stdin", json!({"handle": handle, "bytes": text.len()}))
    };
    persist_managed_process_control_record(pool, session_id, record).await?;
    Ok(json!({"handle": handle, "status": "input accepted"}))
}

pub async fn flush_managed_process(pool: &PgPool, session_id: Uuid, handle: &str) -> Result<JsonValue> {
    let (record, stdout_record, stderr_record, stdout_envelope, stderr_envelope) = {
        let mut manager = PROCESS_MANAGER.lock().map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
        let proc = manager
            .get_mut(&session_id)
            .and_then(|processes| processes.get_mut(handle))
            .ok_or_else(|| anyhow::anyhow!("session process is not attached to this runtime: {handle}"))?;
        let (stdout, stdout_truncated) = proc.take_stdout_since_flush();
        let (stderr, stderr_truncated) = proc.take_stderr_since_flush();
        let stdout_artifact_id = Uuid::new_v4();
        let stderr_artifact_id = Uuid::new_v4();
        let stdout_envelope = output_artifacts::envelope_for(stdout_artifact_id, "stdout", &stdout);
        let stderr_envelope = output_artifacts::envelope_for(stderr_artifact_id, "stderr", &stderr);
        (
            proc.snapshot_record("process.flushed", json!({"handle": handle, "stdoutBytes": stdout.len(), "stderrBytes": stderr.len()})),
            ProcessOutputRecord { artifact_id: stdout_artifact_id, process_id: proc.id, handle: handle.to_string(), stream: "stdout".to_string(), content: stdout, truncated: stdout_truncated },
            ProcessOutputRecord { artifact_id: stderr_artifact_id, process_id: proc.id, handle: handle.to_string(), stream: "stderr".to_string(), content: stderr, truncated: stderr_truncated },
            stdout_envelope,
            stderr_envelope,
        )
    };
    persist_process_output_record(pool, session_id, None, &stdout_record).await?;
    persist_process_output_record(pool, session_id, None, &stderr_record).await?;
    persist_managed_process_control_record(pool, session_id, record).await?;
    Ok(json!({"handle": handle, "status": "flushed", "stdoutArtifact": stdout_envelope, "stderrArtifact": stderr_envelope}))
}

fn spawn_max_runtime_supervisor(
    pool: PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    process_id: Uuid,
    handle: String,
    command_version_id: Option<Uuid>,
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

    fn write<'v>(path: &'v str, content: &'v str, description: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).run_fs_write(path, content, description)
    }
}

#[starlark_module]
fn file_builtins(builder: &mut GlobalsBuilder) {
    fn head<'v>(path: &'v str, #[starlark(default = 40)] lines: i32, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).file_head(path, lines)
    }
    fn tail<'v>(path: &'v str, #[starlark(default = 40)] lines: i32, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).file_tail(path, lines)
    }
    fn read_lines<'v>(path: &'v str, start: i32, end: i32, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).file_read_lines(path, start, end)
    }
    fn line_count<'v>(path: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).file_line_count(path)
    }
    fn search<'v>(path: &'v str, pattern: &'v str, #[starlark(default = 2)] context: i32, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).file_search(path, pattern, context)
    }
    fn replace_exact<'v>(path: &'v str, old: &'v str, new: &'v str, description: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).file_replace_exact(path, old, new, description)
    }
}

#[starlark_module]
fn tree_builtins(builder: &mut GlobalsBuilder) {
    fn list<'v>(#[starlark(default = ".")] path: &'v str, #[starlark(default = 2)] depth: i32, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).tree_list(path, depth)
    }
    fn find<'v>(#[starlark(default = ".")] path: &'v str, #[starlark(default = "")] name_glob: &'v str, #[starlark(default = "")] r#type: &'v str, #[starlark(default = 50)] max_results: i32, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).tree_find(path, name_glob, r#type, max_results)
    }
}

#[starlark_module]
fn git_builtins(builder: &mut GlobalsBuilder) {
    fn status<'v>(eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).git_status()
    }
    fn diff<'v>(paths: UnpackList<Value<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).git_diff(starlark_string_list(paths)?)
    }
    fn add<'v>(paths: UnpackList<Value<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).git_add(starlark_string_list(paths)?)
    }
    fn restore<'v>(paths: UnpackList<Value<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).git_restore(starlark_string_list(paths)?)
    }
    fn commit<'v>(message: &'v str, paths: UnpackList<Value<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).git_commit(message, starlark_string_list(paths)?)
    }
    fn inspect_worker_branch<'v>(worker_branch: &'v str, #[starlark(default = "main")] local_main: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).git_inspect_worker_branch(worker_branch, local_main)
    }
    fn rebase_worker_branch<'v>(worker_branch: &'v str, #[starlark(default = "main")] local_main: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).git_rebase_worker_branch(worker_branch, local_main)
    }
    fn fast_forward_local_main<'v>(worker_branch: &'v str, #[starlark(default = "main")] local_main: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).git_fast_forward_local_main(worker_branch, local_main)
    }
    fn cleanup_integrated_worktree<'v>(path: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).git_cleanup_integrated_worktree(path)
    }
}

#[starlark_module]
fn server_builtins(builder: &mut GlobalsBuilder) {
    fn start<'v>(action: &'v str, args: UnpackList<Value<'v>>, #[starlark(default = "")] name: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).server_start(action, starlark_string_list(args)?, name)
    }
    fn status<'v>(handle: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).server_status(handle)
    }
    fn url<'v>(handle: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).server_url(handle)
    }
    fn logs<'v>(handle: &'v str, #[starlark(default = "stdout")] stream: &'v str, #[starlark(default = 100)] lines: i32, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).server_logs(handle, stream, lines)
    }
    fn wait_ready<'v>(handle: &'v str, #[starlark(default = 1000)] timeout_ms: i32, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).server_wait_ready(handle, timeout_ms)
    }
    fn stop<'v>(handle: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).server_stop(handle)
    }
}

#[starlark_module]
fn image_builtins(builder: &mut GlobalsBuilder) {
    fn capture_from_file<'v>(path: &'v str, description: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).image_capture_from_file(path, description)
    }
    fn describe<'v>(id: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).image_describe(id)
    }
}

#[starlark_module]
fn tooling_builtins(builder: &mut GlobalsBuilder) {
    fn request<'v>(
        title: &'v str,
        need: &'v str,
        attempted: UnpackList<Value<'v>>,
        #[starlark(default = "")] proposed: &'v str,
        #[starlark(default = "normal")] urgency: &'v str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        host_kernel(eval).tooling_request(title, need, starlark_string_list(attempted)?, proposed, urgency)
    }
}

#[starlark_module]
fn patch_builtins(builder: &mut GlobalsBuilder) {
    fn apply<'v>(unified_diff: &'v str, description: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).run_patch_apply(unified_diff, description)
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
fn print_builtins(builder: &mut GlobalsBuilder) {
    fn print<'v>(#[starlark(args)] args: UnpackTuple<Value<'v>>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        let text = args
            .items
            .iter()
            .map(|value| value.unpack_str().map(ToString::to_string).unwrap_or_else(|| value.to_string()))
            .collect::<Vec<_>>()
            .join(" ");
        host_kernel(eval).output.borrow_mut().push(text);
        Ok(NoneType)
    }
}

#[starlark_module]
fn output_artifact_builtins(builder: &mut GlobalsBuilder) {
    fn last<'v>(eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).outputs_last()
    }

    fn head<'v>(id: &'v str, #[starlark(default = 100)] lines: i32, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).outputs_retrieve(id, "head", Some(lines), None, None, None, None)
    }

    fn tail<'v>(id: &'v str, #[starlark(default = 100)] lines: i32, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).outputs_retrieve(id, "tail", Some(lines), None, None, None, None)
    }

    fn slice<'v>(
        id: &'v str,
        #[starlark(default = 1)] start_line: i32,
        #[starlark(default = 100)] end_line: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        host_kernel(eval).outputs_retrieve(id, "slice", None, Some(start_line), Some(end_line), None, None)
    }

    fn search<'v>(
        id: &'v str,
        pattern: &'v str,
        #[starlark(default = 20)] context: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        host_kernel(eval).outputs_retrieve(id, "search", Some(output_artifacts::DEFAULT_VISIBLE_LINE_LIMIT as i32), None, None, Some(pattern), Some(context))
    }

    fn stats<'v>(id: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).outputs_retrieve(id, "stats", None, None, None, None, None)
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

pub async fn image_artifact_metadata(pool: &PgPool, session_id: Uuid, image_id: Uuid) -> Result<JsonValue> {
    let row: Option<(String, i64, Option<i32>, Option<i32>, JsonValue)> = sqlx::query_as(
        "SELECT mime_type, byte_count, width, height, retrieval_metadata FROM starter_image_artifacts WHERE id=$1 AND session_id=$2"
    )
    .bind(image_id)
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    let Some((mime, bytes, width, height, retrieval)) = row else {
        bail!("image artifact not found for current session: {image_id}");
    };
    Ok(json!({
        "imageArtifactId": image_id,
        "mimeType": mime,
        "byteCount": bytes,
        "width": width,
        "height": height,
        "retrieval": retrieval,
    }))
}

pub async fn image_artifact_thumbnail(pool: &PgPool, session_id: Uuid, image_id: Uuid) -> Result<Vec<u8>> {
    let bytes: Option<Vec<u8>> = sqlx::query_scalar("SELECT binary_content FROM starter_image_artifacts WHERE id=$1 AND session_id=$2")
        .bind(image_id)
        .bind(session_id)
        .fetch_optional(pool)
        .await?;
    let bytes = bytes.ok_or_else(|| anyhow::anyhow!("image artifact not found for current session: {image_id}"))?;
    Ok(bytes.into_iter().take(256 * 1024).collect())
}

pub async fn image_artifact_full(pool: &PgPool, session_id: Uuid, image_id: Uuid) -> Result<Vec<u8>> {
    let bytes: Option<Vec<u8>> = sqlx::query_scalar("SELECT binary_content FROM starter_image_artifacts WHERE id=$1 AND session_id=$2")
        .bind(image_id)
        .bind(session_id)
        .fetch_optional(pool)
        .await?;
    bytes.ok_or_else(|| anyhow::anyhow!("image artifact not found for current session: {image_id}"))
}

pub async fn image_artifact_model_attachment(pool: &PgPool, session_id: Uuid, image_id: Uuid) -> Result<JsonValue> {
    let metadata = image_artifact_metadata(pool, session_id, image_id).await?;
    Ok(json!({
        "type": "input_image",
        "source": {"kind": "agentRuntimeImageArtifact", "imageArtifactId": image_id},
        "metadata": metadata,
        "binaryInTranscript": false,
    }))
}

pub fn screenshot_capture_contracts() -> JsonValue {
    json!({
        "storageModel": {
            "table": "starter_image_artifacts",
            "binaryOutsideTranscript": true,
            "handleField": "imageArtifactId",
            "retrieval": ["metadata", "thumbnail", "full"],
        },
        "tools": [
            {
                "tool": "simulator.screenshot.capture",
                "producer": "future simulator steward",
                "input": {"leaseId": "uuid", "target": "owned simulator lease"},
                "output": {"imageArtifactId": "uuid", "mimeType": "image/png", "sourceType": "simulatorScreenshot"},
                "authorization": "owning session or simulator steward lease",
            },
            {
                "tool": "browser.screenshot.capture",
                "producer": "future browser tool",
                "input": {"sessionUrl": "managed server URL", "viewport": {"width": "u32", "height": "u32"}},
                "output": {"imageArtifactId": "uuid", "mimeType": "image/png", "sourceType": "browserScreenshot"},
                "authorization": "owning session managed server boundary",
            },
            {
                "tool": "design_lab.capture",
                "producer": "future Design Lab capture",
                "input": {"surface": "shared design-system component", "viewport": {"width": "u32", "height": "u32"}},
                "output": {"imageArtifactId": "uuid", "mimeType": "image/png", "sourceType": "designLabScreenshot"},
                "authorization": "Requirements-native design evidence request",
            }
        ],
        "reviewContract": {
            "requirementsEvidenceMustReference": ["imageArtifactId", "captureMethod", "viewport", "reviewedFlow"],
            "modelAttachment": "input_image from image artifact metadata; never local path-only evidence",
        }
    })
}

async fn record_tooling_follow_on_request(
    pool: &PgPool,
    project_key: Option<&str>,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    starter_tooling_request_id: Uuid,
    proposed: Option<&JsonValue>,
) -> Result<Option<Uuid>> {
    let Some(proposed) = proposed else {
        return Ok(None);
    };
    let kind = proposed
        .get("kind")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    match kind {
        "command_registry" | "commandRegistry" => {
            let packet_id = lifecycle_hooks::record_runtime_packet(
                pool,
                project_key,
                Some(session_id),
                None,
                turn_id,
                "command_registry.follow_on_request",
                "pending_approval",
                json!({
                    "starterToolingRequestId": starter_tooling_request_id,
                    "proposal": proposed,
                }),
                None,
                json!({
                    "source": "tooling.request",
                    "requiresApproval": true,
                    "authority": ["owner", "operator"],
                    "approvalPath": "command_registry.request",
                }),
                &format!("tooling-follow-on-command-registry-{starter_tooling_request_id}"),
            ).await?;
            lifecycle_hooks::route_packet_envelope(
                pool,
                packet_id,
                "approval_request",
                Some(session_id),
                None,
                None,
                "pending",
                json!({
                    "source": "tooling.request",
                    "authority": ["owner", "operator"],
                    "starterToolingRequestId": starter_tooling_request_id,
                }),
            ).await?;
            Ok(Some(packet_id))
        }
        "project_runtime_config" | "projectRuntimeConfig" => {
            let project = proposed
                .get("projectKey")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| anyhow::anyhow!("project runtime follow-on requires projectKey"))?;
            if project_key != Some(project) {
                bail!("project runtime follow-on is scoped to the current project");
            }
            let source_text = proposed
                .get("sourceText")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| anyhow::anyhow!("project runtime follow-on requires sourceText"))?;
            let manifest = proposed
                .get("manifest")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("project runtime follow-on requires manifest"))?;
            let rationale = proposed
                .get("rationale")
                .and_then(JsonValue::as_str)
                .unwrap_or("tooling.request follow-on project runtime proposal");
            let packet_id = lifecycle_hooks::request_project_runtime_config_change(
                pool,
                project,
                session_id,
                source_text,
                manifest,
                rationale,
            ).await?;
            lifecycle_hooks::route_packet_envelope(
                pool,
                packet_id,
                "approval_request",
                Some(session_id),
                None,
                None,
                "pending",
                json!({
                    "source": "tooling.request",
                    "authority": ["owner", "operator"],
                    "starterToolingRequestId": starter_tooling_request_id,
                }),
            ).await?;
            Ok(Some(packet_id))
        }
        _ => Ok(None),
    }
}

fn starlark_string_list<'v>(list: UnpackList<Value<'v>>) -> anyhow::Result<Vec<String>> {
    list.items
        .iter()
        .map(|value| {
            value
                .unpack_str()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow::anyhow!("list values must be strings"))
        })
        .collect()
}

impl HostKernel {
    fn outputs_last(&self) -> anyhow::Result<String> {
        let id = block_on_host_future(output_artifacts::last_artifact_id(&self.pool, self.session_id))?
            .ok_or_else(|| anyhow::anyhow!("no output artifacts are available for this session"))?;
        Ok(id.to_string())
    }

    fn outputs_retrieve(
        &self,
        id: &str,
        mode: &str,
        lines: Option<i32>,
        start_line: Option<i32>,
        end_line: Option<i32>,
        pattern: Option<&str>,
        context: Option<i32>,
    ) -> anyhow::Result<String> {
        let artifact_id = Uuid::parse_str(id).with_context(|| format!("invalid output artifact id: {id}"))?;
        let packet = block_on_host_future(output_artifacts::retrieve(
            &self.pool,
            self.session_id,
            artifact_id,
            mode,
            lines.map(|value| value.max(0) as usize),
            start_line.map(|value| value.max(1) as usize),
            end_line.map(|value| value.max(1) as usize),
            pattern,
            context.map(|value| value.max(0) as usize),
        ))?;
        Ok(output_artifacts::retrieval_json(packet))
    }

    fn starter_context(&self) -> anyhow::Result<(Option<Uuid>, Option<Uuid>, Option<String>)> {
        block_on_host_future(async {
            let row = sqlx::query(
                r#"
                SELECT tc.id AS tool_call_id, t.id AS turn_id, s.project_key AS project_key
                FROM script_runs sr
                JOIN tool_calls tc ON tc.id = sr.tool_call_id
                LEFT JOIN turns t ON t.id = tc.turn_id
                LEFT JOIN sessions s ON s.id = tc.session_id
                WHERE sr.id = $1
                "#,
            )
            .bind(self.script_run_id)
            .fetch_optional(&self.pool)
            .await?;
            if let Some(row) = row {
                use sqlx::Row;
                Ok((row.try_get("turn_id").ok(), row.try_get("tool_call_id").ok(), row.try_get("project_key").ok()))
            } else {
                Ok((None, None, None))
            }
        })
    }

    fn record_file_audit(&self, operation: &str, requested_path: &str, resolved_path: Option<&Path>, status: &str, byte_count: usize, line_count: usize, truncation: JsonValue, error: Option<String>, metadata: JsonValue) -> anyhow::Result<()> {
        let (turn_id, tool_call_id, _) = self.starter_context()?;
        let resolved = resolved_path.map(|path| path.display().to_string());
        let mutation_description = metadata
            .get("description")
            .or_else(|| truncation.get("description"))
            .and_then(JsonValue::as_str)
            .map(str::to_string);
        block_on_host_future(async {
            sqlx::query(
                r#"
                INSERT INTO starter_file_audit_rows (
                    id, session_id, turn_id, tool_call_id, script_run_id, operation, requested_path,
                    resolved_path, status, byte_count, line_count, mutation_description, truncation, error, metadata
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(self.session_id)
            .bind(turn_id)
            .bind(tool_call_id)
            .bind(self.script_run_id)
            .bind(operation)
            .bind(requested_path)
            .bind(resolved)
            .bind(status)
            .bind(byte_count as i64)
            .bind(line_count as i64)
            .bind(mutation_description)
            .bind(truncation)
            .bind(error)
            .bind(metadata)
            .execute(&self.pool)
            .await?;
            Ok::<(), anyhow::Error>(())
        })
    }

    fn file_head(&self, path: &str, lines: i32) -> anyhow::Result<String> {
        let input = json!({"path": path, "lines": lines});
        let policy = self.decide("file.head", input);
        if !policy.decision.can_execute() {
            bail!("file.head blocked by policy: {}", policy.decision.as_str());
        }
        let resolved = self.root.resolve_agent_path(path, "file.head", true)?;
        let text = text_file_content("file.head", &resolved)?;
        let count = lines.clamp(1, STARTER_FILE_LINE_LIMIT as i32) as usize;
        let (content, truncated, total) = bounded_lines(&text, 1, count);
        let artifact_id = self.store_starter_read_artifact("file.head", path, &content, truncated)?;
        self.record_file_audit("file.head", path, Some(&resolved), "completed", text.len(), total, json!({"truncated": truncated, "limitLines": count, "artifactId": artifact_id}), None, json!({}))?;
        Ok(serde_json::to_string(&json!({"path": path, "lineCount": total, "content": content, "truncated": truncated, "artifactId": artifact_id}))?)
    }

    fn file_tail(&self, path: &str, lines: i32) -> anyhow::Result<String> {
        let input = json!({"path": path, "lines": lines});
        let policy = self.decide("file.tail", input);
        if !policy.decision.can_execute() {
            bail!("file.tail blocked by policy: {}", policy.decision.as_str());
        }
        let resolved = self.root.resolve_agent_path(path, "file.tail", true)?;
        let text = text_file_content("file.tail", &resolved)?;
        let total = line_count(&text);
        let count = lines.clamp(1, STARTER_FILE_LINE_LIMIT as i32) as usize;
        let start = total.saturating_sub(count).saturating_add(1);
        let (content, truncated, total) = bounded_lines(&text, start, total);
        let artifact_id = self.store_starter_read_artifact("file.tail", path, &content, truncated)?;
        self.record_file_audit("file.tail", path, Some(&resolved), "completed", text.len(), total, json!({"truncated": truncated, "limitLines": count, "artifactId": artifact_id}), None, json!({}))?;
        Ok(serde_json::to_string(&json!({"path": path, "lineCount": total, "content": content, "truncated": truncated, "artifactId": artifact_id}))?)
    }

    fn file_read_lines(&self, path: &str, start: i32, end: i32) -> anyhow::Result<String> {
        if start < 1 || end < start {
            bail!("file.read_lines invalid 1-based inclusive line range");
        }
        let input = json!({"path": path, "start": start, "end": end});
        let policy = self.decide("file.read_lines", input);
        if !policy.decision.can_execute() {
            bail!("file.read_lines blocked by policy: {}", policy.decision.as_str());
        }
        let resolved = self.root.resolve_agent_path(path, "file.read_lines", true)?;
        let text = text_file_content("file.read_lines", &resolved)?;
        let total = line_count(&text);
        if start as usize > total + 1 {
            bail!("file.read_lines start is beyond file line count");
        }
        let (content, truncated, total) = bounded_lines(&text, start as usize, end as usize);
        let artifact_id = self.store_starter_read_artifact("file.read_lines", path, &content, truncated)?;
        self.record_file_audit("file.read_lines", path, Some(&resolved), "completed", text.len(), total, json!({"truncated": truncated, "start": start, "end": end, "artifactId": artifact_id}), None, json!({}))?;
        Ok(serde_json::to_string(&json!({"path": path, "start": start, "end": end, "lineCount": total, "content": content, "truncated": truncated, "artifactId": artifact_id}))?)
    }

    fn file_line_count(&self, path: &str) -> anyhow::Result<String> {
        let policy = self.decide("file.line_count", json!({"path": path}));
        if !policy.decision.can_execute() {
            bail!("file.line_count blocked by policy: {}", policy.decision.as_str());
        }
        let resolved = self.root.resolve_agent_path(path, "file.line_count", true)?;
        let text = text_file_content("file.line_count", &resolved)?;
        let lines = line_count(&text);
        self.record_file_audit("file.line_count", path, Some(&resolved), "completed", text.len(), lines, json!({"contentReturned": false}), None, json!({}))?;
        Ok(serde_json::to_string(&json!({"path": path, "byteCount": text.len(), "lineCount": lines, "fileKind": "text", "truncated": false}))?)
    }

    fn file_search(&self, path: &str, pattern: &str, context: i32) -> anyhow::Result<String> {
        if pattern.is_empty() {
            bail!("file.search pattern must not be empty");
        }
        let policy = self.decide("file.search", json!({"path": path, "pattern": pattern, "context": context}));
        if !policy.decision.can_execute() {
            bail!("file.search blocked by policy: {}", policy.decision.as_str());
        }
        let resolved = self.root.resolve_agent_path(path, "file.search", true)?;
        let text = text_file_content("file.search", &resolved)?;
        let lines: Vec<&str> = text.lines().collect();
        let ctx = context.clamp(0, 20) as usize;
        let mut matches = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains(pattern) {
                let start = idx.saturating_sub(ctx);
                let end = (idx + ctx + 1).min(lines.len());
                let snippet = (start..end).map(|line_idx| format!("{}: {}", line_idx + 1, lines[line_idx])).collect::<Vec<_>>().join("\n");
                matches.push(json!({"line": idx + 1, "context": snippet}));
                if matches.len() >= 50 {
                    break;
                }
            }
        }
        let truncated = matches.len() == 50;
        let serialized_matches = serde_json::to_string(&matches)?;
        let artifact_id = self.store_starter_read_artifact("file.search", path, &serialized_matches, truncated)?;
        self.record_file_audit("file.search", path, Some(&resolved), "completed", text.len(), lines.len(), json!({"matchCount": matches.len(), "truncated": truncated, "artifactId": artifact_id}), None, json!({"patternHash": format!("{:x}", Sha256::digest(pattern.as_bytes()))}))?;
        Ok(serde_json::to_string(&json!({"path": path, "matches": matches, "truncated": truncated, "artifactId": artifact_id}))?)
    }

    fn file_replace_exact(&self, path: &str, old: &str, new: &str, description: &str) -> anyhow::Result<String> {
        require_mutation_description("file.replace_exact", description)?;
        let policy = self.decide("file.replace_exact", json!({"path": path, "old": old, "new": new, "description": description, "oldBytes": old.len(), "newBytes": new.len(), "executionRoot": self.root.as_path().display().to_string()}));
        if !policy.decision.can_execute() {
            bail!("file.replace_exact blocked by policy: {}", policy.decision.as_str());
        }
        if old.is_empty() {
            bail!("file.replace_exact old text must not be empty");
        }
        let resolved = self.root.resolve_agent_path(path, "file.replace_exact", true)?;
        let text = text_file_content("file.replace_exact", &resolved)?;
        let count = text.matches(old).count();
        if count == 0 {
            bail!("file.replace_exact old text is absent");
        }
        if count > 1 {
            bail!("file.replace_exact old text is ambiguous; found {count} matches");
        }
        let before = file_state(&resolved);
        let updated = text.replacen(old, new, 1);
        std::fs::write(&resolved, updated.as_bytes())?;
        let after = file_state(&resolved);
        self.records.borrow_mut().push(HostRecord::FileMutation(FileMutationRecord {
            id: Uuid::new_v4(),
            action: "file.replace_exact",
            path: resolved.display().to_string(),
            before_state: before,
            after_state: after,
            status: "completed".to_string(),
            error: None,
            duration_ms: 0,
            policy_decision: json!({"action": "file.replace_exact", "decision": "allow", "role": self.role_snapshot.id}),
            truncation: json!({"description": description, "oldBytes": old.len(), "newBytes": new.len()}),
        }));
        self.record_file_audit("file.replace_exact", path, Some(&resolved), "completed", updated.len(), line_count(&updated), json!({"description": description}), None, json!({}))?;
        Ok(serde_json::to_string(&json!({"path": path, "status": "completed", "description": description}))?)
    }

    fn tree_list(&self, path: &str, depth: i32) -> anyhow::Result<String> {
        let depth = depth.clamp(0, 8) as usize;
        let policy = self.decide("tree.list", json!({"path": path, "depth": depth}));
        if !policy.decision.can_execute() {
            bail!("tree.list blocked by policy: {}", policy.decision.as_str());
        }
        let resolved = self.root.resolve_agent_path(path, "tree.list", true)?;
        if !resolved.is_dir() {
            bail!("tree.list path is not a directory");
        }
        let mut entries = Vec::new();
        let mut omitted = 0usize;
        self.walk_tree(&resolved, depth, &mut |entry_path, metadata| {
            if entries.len() >= STARTER_TREE_RESULT_LIMIT {
                omitted += 1;
                return;
            }
            let rel = entry_path.strip_prefix(self.root.as_path()).unwrap_or(entry_path);
            if rel.components().any(|component| component.as_os_str() == ".git") {
                return;
            }
            let kind = if metadata.is_dir() { "directory" } else if metadata.is_file() { "file" } else { "other" };
            entries.push(json!({"path": rel.display().to_string(), "type": kind, "size": metadata.len()}));
        })?;
        let serialized_entries = serde_json::to_string(&entries)?;
        let artifact_id = self.store_starter_read_artifact("tree.list", path, &serialized_entries, omitted > 0)?;
        self.record_file_audit("tree.list", path, Some(&resolved), "completed", 0, entries.len(), json!({"omitted": omitted, "maxResults": STARTER_TREE_RESULT_LIMIT, "artifactId": artifact_id}), None, json!({"depth": depth}))?;
        Ok(serde_json::to_string(&json!({"path": path, "entries": entries, "omitted": omitted, "depth": depth, "artifactId": artifact_id}))?)
    }

    fn tree_find(&self, path: &str, name_glob: &str, kind: &str, max_results: i32) -> anyhow::Result<String> {
        let max = max_results.clamp(1, STARTER_TREE_RESULT_LIMIT as i32) as usize;
        if path == "." && name_glob.is_empty() && kind.is_empty() {
            bail!("tree.find rejects unbounded broad scans; provide name_glob or type");
        }
        let policy = self.decide("tree.find", json!({"path": path, "nameGlob": name_glob, "type": kind, "maxResults": max}));
        if !policy.decision.can_execute() {
            bail!("tree.find blocked by policy: {}", policy.decision.as_str());
        }
        let resolved = self.root.resolve_agent_path(path, "tree.find", true)?;
        let mut matches = Vec::new();
        let mut omitted = 0usize;
        self.walk_tree(&resolved, 16, &mut |entry_path, metadata| {
            let rel = entry_path.strip_prefix(self.root.as_path()).unwrap_or(entry_path);
            if rel.components().any(|component| component.as_os_str() == ".git") {
                return;
            }
            let actual_kind = if metadata.is_dir() { "directory" } else if metadata.is_file() { "file" } else { "other" };
            if !kind.is_empty() && kind != actual_kind {
                return;
            }
            let name = entry_path.file_name().and_then(|value| value.to_str()).unwrap_or("");
            if !name_glob.is_empty() && !simple_glob_match(name_glob, name) {
                return;
            }
            if matches.len() >= max {
                omitted += 1;
                return;
            }
            matches.push(json!({"path": rel.display().to_string(), "type": actual_kind, "size": metadata.len()}));
        })?;
        let serialized_matches = serde_json::to_string(&matches)?;
        let artifact_id = self.store_starter_read_artifact("tree.find", path, &serialized_matches, omitted > 0)?;
        self.record_file_audit("tree.find", path, Some(&resolved), "completed", 0, matches.len(), json!({"omitted": omitted, "maxResults": max, "artifactId": artifact_id}), None, json!({"nameGlob": name_glob, "type": kind}))?;
        Ok(serde_json::to_string(&json!({"path": path, "matches": matches, "omitted": omitted, "artifactId": artifact_id}))?)
    }

    fn store_starter_read_artifact(&self, operation: &str, path: &str, content: &str, truncated: bool) -> anyhow::Result<Option<String>> {
        if !truncated && content.len() <= OUTPUT_LIMIT_BYTES {
            return Ok(None);
        }
        let (turn_id, tool_call_id, _) = self.starter_context()?;
        let envelope = block_on_host_future(output_artifacts::store(&self.pool, NewOutputArtifact {
            id: Uuid::new_v4(),
            session_id: self.session_id,
            turn_id,
            tool_call_id,
            script_run_id: Some(self.script_run_id),
            command_run_id: None,
            process_id: None,
            source_type: "starter_file_tree_read",
            stream: operation,
            content,
            metadata: json!({"operation": operation, "path": path, "truncated": truncated}),
        }))?;
        Ok(Some(artifact_id_from_envelope(&envelope)))
    }

    fn walk_tree<F>(&self, root: &Path, depth: usize, visit: &mut F) -> anyhow::Result<()>
    where
        F: FnMut(&Path, std::fs::Metadata),
    {
        fn walk<F>(path: &Path, remaining: usize, visit: &mut F) -> anyhow::Result<()>
        where
            F: FnMut(&Path, std::fs::Metadata),
        {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let p = entry.path();
                let metadata = entry.metadata()?;
                visit(&p, metadata.clone());
                if metadata.is_dir() && remaining > 0 {
                    walk(&p, remaining - 1, visit)?;
                }
            }
            Ok(())
        }
        walk(root, depth, visit)
    }

    fn git_status(&self) -> anyhow::Result<String> {
        self.git_policy("git.status", json!({}))?;
        self.run_git(&["status", "--short"], Vec::new(), "git.status", None)
    }

    fn git_diff(&self, paths: Vec<String>) -> anyhow::Result<String> {
        let resolved = self.validate_git_paths("git.diff", &paths, false)?;
        let mut args = vec!["diff".to_string(), "--".to_string()];
        args.extend(paths);
        self.run_git(&args.iter().map(String::as_str).collect::<Vec<_>>(), resolved, "git.diff", None)
    }

    fn git_add(&self, paths: Vec<String>) -> anyhow::Result<String> {
        let resolved = self.validate_git_paths("git.add", &paths, true)?;
        let mut args = vec!["add".to_string(), "--".to_string()];
        args.extend(paths.clone());
        self.run_git(&args.iter().map(String::as_str).collect::<Vec<_>>(), resolved, "git.add", Some(json!({"stagedPaths": paths})))
    }

    fn git_restore(&self, paths: Vec<String>) -> anyhow::Result<String> {
        let resolved = self.validate_git_paths("git.restore", &paths, true)?;
        let mut args = vec!["restore".to_string(), "--".to_string()];
        args.extend(paths.clone());
        self.run_git(&args.iter().map(String::as_str).collect::<Vec<_>>(), resolved, "git.restore", Some(json!({"restoredPaths": paths})))
    }

    fn git_commit(&self, message: &str, paths: Vec<String>) -> anyhow::Result<String> {
        let msg = message.trim();
        if msg.is_empty() || msg.len() > 160 {
            bail!("git.commit requires an explicit concise message");
        }
        let resolved = if paths.is_empty() { Vec::new() } else { self.validate_git_paths("git.commit", &paths, false)? };
        let before = self.run_plain_git(&["rev-parse", "HEAD"]).unwrap_or_else(|_| "NO_HEAD".to_string());
        let mut args = vec!["commit".to_string(), "-m".to_string(), msg.to_string()];
        if !paths.is_empty() {
            args.push("--".to_string());
            args.extend(paths.clone());
        }
        let result = self.run_git(&args.iter().map(String::as_str).collect::<Vec<_>>(), resolved, "git.commit", Some(json!({"message": msg, "paths": paths, "parentHash": before.trim()})))?;
        if result.contains("nothing to commit") || result.contains("no changes added") {
            bail!("git.commit refused empty commit");
        }
        let commit_hash = self.run_plain_git(&["rev-parse", "HEAD"]).unwrap_or_default();
        let changed_paths = self.run_plain_git(&["show", "--name-only", "--format=", "HEAD"])
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .collect::<Vec<_>>();
        self.record_file_audit(
            "git.commit.summary",
            "",
            None,
            "completed",
            result.len(),
            changed_paths.len(),
            json!({"commitHash": commit_hash.trim(), "parentHash": before.trim(), "changedPaths": changed_paths, "message": msg, "paths": paths}),
            None,
            json!({"linkage": {"sessionId": self.session_id, "scriptRunId": self.script_run_id}}),
        )?;
        let mut value: JsonValue = serde_json::from_str(&result)?;
        value["commitHash"] = json!(commit_hash.trim());
        value["parentHash"] = json!(before.trim());
        value["changedPaths"] = json!(changed_paths);
        Ok(serde_json::to_string(&value)?)
    }

    fn require_orchestrator_git_integration(&self, action: &str) -> anyhow::Result<()> {
        if self.role_snapshot.id != "orchestrator" {
            bail!("{action} is only visible to orchestrator roles");
        }
        self.git_policy(action, json!({"roleId": self.role_snapshot.id}))
    }

    fn validate_git_branch_name(&self, action: &str, branch: &str) -> anyhow::Result<String> {
        let branch = branch.trim();
        if branch.is_empty()
            || branch.starts_with('-')
            || branch.contains("..")
            || branch.contains(' ')
            || branch.contains('\\')
            || branch.contains(':')
            || branch.contains('~')
            || branch.contains('^')
            || branch.contains('?')
            || branch.contains('*')
            || branch.contains('[')
        {
            bail!("{action} rejects unsafe branch name");
        }
        Ok(branch.to_string())
    }

    fn git_inspect_worker_branch(&self, worker_branch: &str, local_main: &str) -> anyhow::Result<String> {
        self.require_orchestrator_git_integration("git.inspect_worker_branch")?;
        let worker = self.validate_git_branch_name("git.inspect_worker_branch", worker_branch)?;
        let main = self.validate_git_branch_name("git.inspect_worker_branch", local_main)?;
        let worker_hash = self.run_plain_git(&["rev-parse", &worker])?;
        let main_hash = self.run_plain_git(&["rev-parse", &main])?;
        let merge_base = self.run_plain_git(&["merge-base", &main, &worker])?;
        let changed = self.run_plain_git(&["diff", "--name-only", &format!("{main}...{worker}")])?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .collect::<Vec<_>>();
        Ok(serde_json::to_string(&json!({
            "workerBranch": worker,
            "localMain": main,
            "workerHead": worker_hash.trim(),
            "localMainHead": main_hash.trim(),
            "mergeBase": merge_base.trim(),
            "changedPaths": changed,
        }))?)
    }

    fn git_rebase_worker_branch(&self, worker_branch: &str, local_main: &str) -> anyhow::Result<String> {
        self.require_orchestrator_git_integration("git.rebase_worker_branch")?;
        let worker = self.validate_git_branch_name("git.rebase_worker_branch", worker_branch)?;
        let main = self.validate_git_branch_name("git.rebase_worker_branch", local_main)?;
        let before = self.run_plain_git(&["rev-parse", &worker])?;
        let output = Command::new("git")
            .args(["rebase", &main, &worker])
            .current_dir(self.root.as_path())
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !output.status.success() {
            bail!("git.rebase_worker_branch failed: {stderr}");
        }
        let after = self.run_plain_git(&["rev-parse", &worker])?;
        Ok(serde_json::to_string(&json!({"workerBranch": worker, "localMain": main, "before": before.trim(), "after": after.trim(), "stdout": stdout.lines().take(20).collect::<Vec<_>>() }))?)
    }

    fn git_fast_forward_local_main(&self, worker_branch: &str, local_main: &str) -> anyhow::Result<String> {
        self.require_orchestrator_git_integration("git.fast_forward_local_main")?;
        let worker = self.validate_git_branch_name("git.fast_forward_local_main", worker_branch)?;
        let main = self.validate_git_branch_name("git.fast_forward_local_main", local_main)?;
        let before = self.run_plain_git(&["rev-parse", &main])?;
        let checkout = Command::new("git")
            .args(["checkout", &main])
            .current_dir(self.root.as_path())
            .output()?;
        if !checkout.status.success() {
            bail!("git.fast_forward_local_main failed to checkout local main: {}", String::from_utf8_lossy(&checkout.stderr));
        }
        let output = Command::new("git")
            .args(["merge", "--ff-only", &worker])
            .current_dir(self.root.as_path())
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !output.status.success() {
            bail!("git.fast_forward_local_main failed: {stderr}");
        }
        let after = self.run_plain_git(&["rev-parse", &main])?;
        Ok(serde_json::to_string(&json!({"localMain": main, "workerBranch": worker, "before": before.trim(), "after": after.trim(), "stdout": stdout.lines().take(20).collect::<Vec<_>>() }))?)
    }

    fn git_cleanup_integrated_worktree(&self, path: &str) -> anyhow::Result<String> {
        self.require_orchestrator_git_integration("git.cleanup_integrated_worktree")?;
        let resolved = self.root.resolve_agent_path(path, "git.cleanup_integrated_worktree", true)?;
        if resolved == self.root.as_path() {
            bail!("git.cleanup_integrated_worktree rejects repository root cleanup");
        }
        let output = Command::new("git")
            .args(["worktree", "remove", "--force", resolved.to_str().unwrap_or_default()])
            .current_dir(self.root.as_path())
            .output()?;
        if !output.status.success() {
            bail!("git.cleanup_integrated_worktree failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(serde_json::to_string(&json!({"path": path, "status": "removed"}))?)
    }

    fn git_policy(&self, action: &str, input: JsonValue) -> anyhow::Result<()> {
        let policy = self.decide(action, input);
        if !policy.decision.can_execute() {
            bail!("{action} blocked by policy: {}", policy.decision.as_str());
        }
        Ok(())
    }

    fn validate_git_paths(&self, action: &str, paths: &[String], require_non_empty: bool) -> anyhow::Result<Vec<PathBuf>> {
        if require_non_empty && paths.is_empty() {
            bail!("{action} requires explicit path arguments");
        }
        if paths.iter().any(|path| path == "." || path == "/" || path.is_empty()) {
            bail!("{action} rejects broad repository path arguments");
        }
        let mut resolved = Vec::new();
        for path in paths {
            resolved.push(self.root.resolve_agent_path(path, action, action != "git.add")?);
        }
        self.git_policy(action, json!({"paths": paths}))?;
        Ok(resolved)
    }

    fn run_plain_git(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = Command::new("git").args(args).current_dir(self.root.as_path()).output()?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn run_git(&self, args: &[&str], resolved_paths: Vec<PathBuf>, action: &str, metadata: Option<JsonValue>) -> anyhow::Result<String> {
        let output = Command::new("git").args(args).current_dir(self.root.as_path()).output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let status = if output.status.success() { "completed" } else { "failed" };
        self.record_file_audit(action, "", None, status, stdout.len() + stderr.len(), line_count(&stdout) + line_count(&stderr), json!({"stdoutBytes": stdout.len(), "stderrBytes": stderr.len()}), if output.status.success() { None } else { Some(stderr.clone()) }, json!({"argv": args, "paths": resolved_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(), "summary": metadata}))?;
        if !output.status.success() {
            bail!("{action} failed: {}", stderr.trim());
        }
        Ok(serde_json::to_string(&json!({"status": status, "stdout": stdout, "stderr": stderr, "argv": args}))?)
    }

    fn allocate_port(&self, reason: &str) -> anyhow::Result<(Uuid, i32)> {
        let (_, _, project_key) = self.starter_context()?;
        for _ in 0..20 {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let port = listener.local_addr()?.port() as i32;
            drop(listener);
            let lease_id = Uuid::new_v4();
            let inserted = block_on_host_future(async {
                let active: Option<i64> = sqlx::query_scalar("SELECT 1 FROM starter_port_leases WHERE allocated_port=$1 AND status='active' LIMIT 1")
                    .bind(port)
                    .fetch_optional(&self.pool)
                    .await?;
                if active.is_some() {
                    return Ok::<bool, anyhow::Error>(false);
                }
                sqlx::query(
                    "INSERT INTO starter_port_leases (id, project_key, session_id, allocated_port, status, lease_reason) VALUES ($1,$2,$3,$4,'active',$5)"
                )
                .bind(lease_id)
                .bind(project_key.as_deref())
                .bind(self.session_id)
                .bind(port)
                .bind(reason)
                .execute(&self.pool)
                .await?;
                Ok::<bool, anyhow::Error>(true)
            })?;
            if inserted {
                return Ok((lease_id, port));
            }
        }
        bail!("server.start could not allocate a unique runtime-owned port");
    }

    fn server_start(&self, action: &str, args: Vec<String>, name: &str) -> anyhow::Result<String> {
        if args.iter().any(|arg| {
            let lowered = arg.to_ascii_lowercase();
            arg == "PORT"
                || arg.starts_with("PORT=")
                || matches!(lowered.as_str(), "--port" | "-p" | "--listen" | "--host" | "port")
                || lowered.starts_with("--port=")
                || lowered.starts_with("-p=")
                || lowered.starts_with("port=")
                || lowered.starts_with("--listen=")
                || lowered.starts_with("--host=")
        }) {
            bail!("server.start rejects user-specified ports; runtime owns PORT allocation");
        }
        let policy = self.decide("server.start", json!({"action": action, "args": args, "name": name}));
        if !policy.decision.can_execute() {
            bail!("server.start blocked by policy: {}", policy.decision.as_str());
        }
        let command_version = self
            .commands
            .get(action)
            .ok_or_else(|| anyhow::anyhow!("server.start requires a visible registry command invocation: {action}"))?;
        if !command_version.async_allowed {
            bail!("server.start requires a registry command that allows managed async execution");
        }
        let (turn_id, tool_call_id, _) = self.starter_context()?;
        let handle = if name.trim().is_empty() { format!("server_{}", Uuid::new_v4().simple()) } else { name.to_string() };
        let active_handle_exists: bool = block_on_host_future(async {
            let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM starter_managed_servers WHERE session_id=$1 AND handle=$2 AND status='running'")
                .bind(self.session_id)
                .bind(&handle)
                .fetch_optional(&self.pool)
                .await?;
            Ok::<bool, anyhow::Error>(exists.is_some())
        })?;
        if active_handle_exists {
            bail!("server.start rejects reuse of active managed server handle: {handle}");
        }
        let (lease_id, port) = self.allocate_port("server.start")?;
        let (mut command, binary_path, argv, resolved_cwd, policy, input) =
            match self.prepare_registry_command(action, args, &command_version.default_cwd, "server.start") {
                Ok(prepared) => prepared,
                Err(error) => {
                    let _ = block_on_host_future(async {
                        sqlx::query("UPDATE starter_port_leases SET status='released', released_at=now(), release_reason='startupFailure' WHERE id=$1 AND status='active'")
                            .bind(lease_id)
                            .execute(&self.pool)
                            .await
                    });
                    return Err(error);
                }
            };
        let url = format!("http://127.0.0.1:{port}");
        command.env("PORT", port.to_string());
        command.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
        command.process_group(0);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = block_on_host_future(async {
                    sqlx::query("UPDATE starter_port_leases SET status='released', released_at=now(), release_reason='startupFailure' WHERE session_id=$1 AND allocated_port=$2 AND status='active'")
                        .bind(self.session_id)
                        .bind(port)
                        .execute(&self.pool)
                        .await
                });
                return Err(error).with_context(|| format!("server.start failed to spawn registry command: {action}"));
            }
        };
        let child_pid = child.id();
        let stdout_buf = Arc::new(Mutex::new(String::new()));
        let stderr_buf = Arc::new(Mutex::new(String::new()));
        spawn_reader(child.stdout.take(), Arc::clone(&stdout_buf));
        spawn_reader(child.stderr.take(), Arc::clone(&stderr_buf));
        thread::sleep(Duration::from_millis(command_version.min_await_ms.clamp(0, 25) as u64));
        if let Some(status) = child.try_wait()? {
            let _ = block_on_host_future(async {
                sqlx::query("UPDATE starter_port_leases SET status='released', released_at=now(), release_reason='startupFailure' WHERE id=$1 AND status='active'")
                    .bind(lease_id)
                    .execute(&self.pool)
                    .await
            });
            bail!("server.start registry command exited before readiness management began: {status}");
        }
        let process_id = Uuid::new_v4();
        let started_at = Utc::now();
        self.records.borrow_mut().push(HostRecord::ManagedProcess(ManagedProcessRecord {
            id: process_id,
            handle: handle.clone(),
            command_version_id: Some(command_version.version_id),
            binary_name: command_version.binary_name.clone(),
            binary_path: binary_path.display().to_string(),
            argv: argv.clone(),
            cwd: resolved_cwd.display().to_string(),
            os_pid: Some(child_pid as i64),
            os_pgid: Some(child_pid as i64),
            status: "running".to_string(),
            end_of_turn_behavior: "continue".to_string(),
            end_of_session_behavior: "terminate".to_string(),
            max_runtime_ms: command_version.max_runtime.map(|d| d.as_millis() as i64),
            termination_reason: None,
            event: "server.started".to_string(),
            payload: json!({"handle": handle, "url": url, "port": port, "commandVersionId": command_version.version_id, "policyDecision": policy.to_event_payload(), "input": input}),
        }));
        PROCESS_MANAGER
            .lock()
            .map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?
            .entry(self.session_id)
            .or_default()
            .insert(handle.clone(), ManagedProcess {
                id: process_id,
                handle: handle.clone(),
                command_version_id: Some(command_version.version_id),
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
                end_of_turn_behavior: "continue".to_string(),
                end_of_session_behavior: "terminate".to_string(),
                max_runtime: command_version.max_runtime,
                min_await_ms: command_version.min_await_ms,
                max_await_ms: command_version.max_await_ms,
                terminate_grace_ms: command_version.terminate_grace_ms,
                output_limit: command_version.output_buffer_bytes,
                stdin_policy: "forbid".to_string(),
                termination_reason: None,
            });
        block_on_host_future(async {
            sqlx::query(
                r#"
                INSERT INTO starter_managed_servers (
                    id, session_id, turn_id, tool_call_id, script_run_id, process_id, command_version_id,
                    handle, cwd, env_overlay_metadata, port, url, readiness_config, status
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'running')
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(self.session_id)
            .bind(turn_id)
            .bind(tool_call_id)
            .bind(self.script_run_id)
            .bind(Option::<Uuid>::None)
            .bind(command_version.version_id)
            .bind(&handle)
            .bind(resolved_cwd.display().to_string())
            .bind(json!({"PORT": {"injected": true, "secret": false}}))
            .bind(port)
            .bind(&url)
            .bind(json!({"mode": "processAlive", "httpGetSupported": true, "logPatternSupported": true, "timeoutMs": 0}))
            .execute(&self.pool)
            .await?;
            Ok::<(), anyhow::Error>(())
        })?;
        Ok(serde_json::to_string(&json!({"handle": handle, "processId": process_id, "port": port, "url": url, "status": "running", "portEnv": {"PORT": port}}))?)
    }

    fn server_status(&self, handle: &str) -> anyhow::Result<String> {
        self.decide("server.status", json!({"handle": handle}));
        let process_terminal_status = PROCESS_MANAGER.lock().ok()
            .and_then(|mut manager| manager.get_mut(&self.session_id).and_then(|processes| processes.get_mut(handle)).map(|proc| {
                let _ = proc.refresh_status();
                proc.status.clone()
            }))
            .filter(|status| status != "running");
        if let Some(status) = process_terminal_status {
            let _ = block_on_host_future(async {
                let row: Option<(i32,)> = sqlx::query_as("UPDATE starter_managed_servers SET status=$3, updated_at=now() WHERE session_id=$1 AND handle=$2 AND status='running' RETURNING port")
                    .bind(self.session_id)
                    .bind(handle)
                    .bind(&status)
                    .fetch_optional(&self.pool)
                    .await?;
                if let Some((port,)) = row {
                    sqlx::query("UPDATE starter_port_leases SET status='released', released_at=COALESCE(released_at, now()), release_reason='process.exit' WHERE session_id=$1 AND allocated_port=$2 AND status='active'")
                        .bind(self.session_id)
                        .bind(port)
                        .execute(&self.pool)
                        .await?;
                }
                Ok::<(), anyhow::Error>(())
            });
        }
        block_on_host_future(async {
            let row: Option<(String, i32, String, String)> = sqlx::query_as("SELECT status, port, url, handle FROM starter_managed_servers WHERE session_id=$1 AND handle=$2")
                .bind(self.session_id)
                .bind(handle)
                .fetch_optional(&self.pool)
                .await?;
            Ok::<String, anyhow::Error>(serde_json::to_string(&row.map(|(status, port, url, handle)| json!({"handle": handle, "status": status, "port": port, "url": url})).unwrap_or_else(|| json!({"handle": handle, "status": "notFound"})))?)
        })
    }

    fn server_url(&self, handle: &str) -> anyhow::Result<String> {
        let status: JsonValue = serde_json::from_str(&self.server_status(handle)?)?;
        Ok(status.get("url").and_then(JsonValue::as_str).unwrap_or("").to_string())
    }

    fn server_logs(&self, handle: &str, stream: &str, lines: i32) -> anyhow::Result<String> {
        if stream != "stdout" && stream != "stderr" {
            bail!("server.logs stream must be stdout or stderr");
        }
        let line_limit = lines.clamp(0, 500) as usize;
        let content = PROCESS_MANAGER.lock().ok()
            .and_then(|manager| manager.get(&self.session_id).and_then(|processes| processes.get(handle)).map(|proc| {
                if stream == "stdout" {
                    proc.stdout.lock().map(|value| value.clone()).unwrap_or_default()
                } else {
                    proc.stderr.lock().map(|value| value.clone()).unwrap_or_default()
                }
            }))
            .unwrap_or_default();
        let visible = content.lines().rev().take(line_limit).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        let (turn_id, tool_call_id, _) = self.starter_context()?;
        let envelope = block_on_host_future(output_artifacts::store(&self.pool, NewOutputArtifact {
            id: Uuid::new_v4(),
            session_id: self.session_id,
            turn_id,
            tool_call_id,
            script_run_id: Some(self.script_run_id),
            command_run_id: None,
            process_id: None,
            source_type: "starter_server_logs",
            stream,
            content: &content,
            metadata: json!({"handle": handle, "requestedLines": line_limit}),
        }))?;
        let artifact_id = artifact_id_from_envelope(&envelope);
        let _ = block_on_host_future(async {
            sqlx::query("UPDATE starter_managed_servers SET output_artifacts = jsonb_set(COALESCE(output_artifacts, '{}'::jsonb), $3, to_jsonb($4::text), true), updated_at=now() WHERE session_id=$1 AND handle=$2")
                .bind(self.session_id)
                .bind(handle)
                .bind(vec![format!("{stream}LogArtifactId")])
                .bind(&artifact_id)
                .execute(&self.pool)
                .await?;
            Ok::<(), anyhow::Error>(())
        });
        Ok(serde_json::to_string(&json!({"handle": handle, "stream": stream, "lines": line_limit, "content": visible, "truncated": content.lines().count() > line_limit, "artifactId": artifact_id}))?)
    }

    fn server_wait_ready(&self, handle: &str, timeout_ms: i32) -> anyhow::Result<String> {
        let timeout = Duration::from_millis(timeout_ms.clamp(1, 30_000) as u64);
        let started = Instant::now();
        let readiness: JsonValue = block_on_host_future(async {
            let row: Option<(JsonValue,)> = sqlx::query_as("SELECT readiness_config FROM starter_managed_servers WHERE session_id=$1 AND handle=$2")
                .bind(self.session_id)
                .bind(handle)
                .fetch_optional(&self.pool)
                .await?;
            Ok::<JsonValue, anyhow::Error>(row.map(|(value,)| value).unwrap_or_else(|| json!({"mode":"processAlive"})))
        })?;
        let mode = readiness.get("mode").and_then(JsonValue::as_str).unwrap_or("processAlive").to_string();
        loop {
            let running = PROCESS_MANAGER.lock().ok()
                .and_then(|mut manager| manager.get_mut(&self.session_id).and_then(|processes| processes.get_mut(handle)).map(|proc| proc.refresh_status().unwrap_or(false)))
                .unwrap_or(false);
            let ready = match mode.as_str() {
                "processAlive" => running,
                "logPattern" => {
                    let pattern = readiness.get("pattern").and_then(JsonValue::as_str).unwrap_or("");
                    !pattern.is_empty() && PROCESS_MANAGER.lock().ok()
                        .and_then(|manager| manager.get(&self.session_id).and_then(|processes| processes.get(handle)).map(|proc| {
                            let stdout = proc.stdout.lock().map(|value| value.clone()).unwrap_or_default();
                            let stderr = proc.stderr.lock().map(|value| value.clone()).unwrap_or_default();
                            stdout.contains(pattern) || stderr.contains(pattern)
                        }))
                        .unwrap_or(false)
                }
                "httpGet" => {
                    let path = readiness.get("path").and_then(JsonValue::as_str).unwrap_or("/");
                    let port = block_on_host_future(async {
                        let row: Option<(i32,)> = sqlx::query_as("SELECT port FROM starter_managed_servers WHERE session_id=$1 AND handle=$2")
                            .bind(self.session_id)
                            .bind(handle)
                            .fetch_optional(&self.pool)
                            .await?;
                        Ok::<Option<i32>, anyhow::Error>(row.map(|(port,)| port))
                    })?;
                    if let Some(port) = port {
                        let address = format!("127.0.0.1:{port}");
                        TcpStream::connect(address)
                            .and_then(|mut stream| {
                                stream.set_read_timeout(Some(Duration::from_millis(200)))?;
                                stream.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").as_bytes())?;
                                let mut response = String::new();
                                let _ = stream.read_to_string(&mut response)?;
                                Ok(response.starts_with("HTTP/1.1 2") || response.starts_with("HTTP/1.0 2"))
                            })
                            .unwrap_or(false)
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if ready {
                return Ok(serde_json::to_string(&json!({"handle": handle, "ready": true, "mode": mode, "elapsedMs": started.elapsed().as_millis()}))?);
            }
            if started.elapsed() >= timeout {
                if let Ok(mut manager) = PROCESS_MANAGER.lock() {
                    if let Some(proc) = manager.get_mut(&self.session_id).and_then(|processes| processes.get_mut(handle)) {
                        let _ = proc.terminate("readiness.timeout", true);
                    }
                }
                let _ = block_on_host_future(async {
                    let row: Option<(i32,)> = sqlx::query_as("UPDATE starter_managed_servers SET status='readiness_timeout', updated_at=now() WHERE session_id=$1 AND handle=$2 AND status='running' RETURNING port")
                        .bind(self.session_id)
                        .bind(handle)
                        .fetch_optional(&self.pool)
                        .await?;
                    if let Some((port,)) = row {
                        sqlx::query("UPDATE starter_port_leases SET status='released', released_at=now(), release_reason='readiness.timeout' WHERE session_id=$1 AND allocated_port=$2 AND status='active'")
                            .bind(self.session_id)
                            .bind(port)
                            .execute(&self.pool)
                            .await?;
                    }
                    Ok::<(), anyhow::Error>(())
                });
                return Ok(serde_json::to_string(&json!({"handle": handle, "ready": false, "mode": mode, "failure": {"kind": "timeout", "timeoutMs": timeout.as_millis(), "readiness": readiness}}))?);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn server_stop(&self, handle: &str) -> anyhow::Result<String> {
        if let Ok(mut manager) = PROCESS_MANAGER.lock() {
            if let Some(proc) = manager.get_mut(&self.session_id).and_then(|processes| processes.get_mut(handle)) {
                let _ = proc.terminate("server.stop", true);
            }
        }
        let (port, found): (Option<i32>, bool) = block_on_host_future(async {
            let row: Option<(i32,)> = sqlx::query_as("UPDATE starter_managed_servers SET status='stopped', updated_at=now() WHERE session_id=$1 AND handle=$2 RETURNING port")
                .bind(self.session_id)
                .bind(handle)
                .fetch_optional(&self.pool)
                .await?;
            if let Some((port,)) = row {
                sqlx::query("UPDATE starter_port_leases SET status='released', released_at=now(), release_reason='server.stop' WHERE session_id=$1 AND allocated_port=$2 AND status='active'")
                    .bind(self.session_id)
                    .bind(port)
                    .execute(&self.pool)
                    .await?;
                Ok::<_, anyhow::Error>((Some(port), true))
            } else {
                Ok((None, false))
            }
        })?;
        Ok(serde_json::to_string(&json!({"handle": handle, "status": if found {"stopped"} else {"notFound"}, "port": port}))?)
    }

    fn image_capture_from_file(&self, path: &str, description: &str) -> anyhow::Result<String> {
        require_mutation_description("image.capture_from_file", description)?;
        let policy = self.decide("image.capture_from_file", json!({"path": path, "description": description}));
        if !policy.decision.can_execute() {
            bail!("image.capture_from_file blocked by policy: {}", policy.decision.as_str());
        }
        let resolved = self.root.resolve_agent_path(path, "image.capture_from_file", true)?;
        let bytes = std::fs::read(&resolved)?;
        if bytes.len() > 25_000_000 {
            bail!("image.capture_from_file rejects images over 25MB");
        }
        let (mime, width, height) = image_mime_and_dimensions(&bytes, path);
        if !mime.starts_with("image/") {
            bail!("image.capture_from_file requires an image MIME type");
        }
        let (turn_id, tool_call_id, _) = self.starter_context()?;
        let id = Uuid::new_v4();
        block_on_host_future(async {
            sqlx::query(
                r#"
                INSERT INTO starter_image_artifacts (
                    id, session_id, turn_id, tool_call_id, script_run_id, source_type, source_path,
                    mime_type, byte_count, width, height, perceptual_metadata, retrieval_metadata, binary_content
                )
                VALUES ($1,$2,$3,$4,$5,'file',$6,$7,$8,$9,$10,$11,$12,$13)
                "#,
            )
            .bind(id)
            .bind(self.session_id)
            .bind(turn_id)
            .bind(tool_call_id)
            .bind(self.script_run_id)
            .bind(resolved.display().to_string())
            .bind(&mime)
            .bind(bytes.len() as i64)
            .bind(width)
            .bind(height)
            .bind(json!({"description": description, "sha256": format!("{:x}", Sha256::digest(&bytes))}))
            .bind(json!({"thumbnailAvailable": true, "fullRequiresSession": true, "modelAttachable": true}))
            .bind(bytes)
            .execute(&self.pool)
            .await?;
            Ok::<(), anyhow::Error>(())
        })?;
        Ok(serde_json::to_string(&json!({"imageArtifactId": id, "mimeType": mime, "byteCount": std::fs::metadata(&resolved)?.len(), "width": width, "height": height, "description": description, "modelAttachment": {"kind": "imageArtifact", "id": id}}))?)
    }

    fn image_describe(&self, id: &str) -> anyhow::Result<String> {
        let image_id = Uuid::parse_str(id).with_context(|| format!("invalid image artifact id: {id}"))?;
        block_on_host_future(async {
            let row: Option<(String, i64, Option<i32>, Option<i32>, JsonValue)> = sqlx::query_as(
                "SELECT mime_type, byte_count, width, height, retrieval_metadata FROM starter_image_artifacts WHERE id=$1 AND session_id=$2"
            )
            .bind(image_id)
            .bind(self.session_id)
            .fetch_optional(&self.pool)
            .await?;
            Ok::<String, anyhow::Error>(serde_json::to_string(&row.map(|(mime, bytes, width, height, retrieval)| json!({"imageArtifactId": image_id, "mimeType": mime, "byteCount": bytes, "width": width, "height": height, "retrieval": retrieval})).unwrap_or_else(|| json!({"error": "image artifact not found for current session"})))?)
        })
    }

    fn tooling_request(&self, title: &str, need: &str, attempted: Vec<String>, proposed: &str, urgency: &str) -> anyhow::Result<String> {
        let title = title.trim();
        let need = need.trim();
        if title.is_empty() || title.len() > 80 {
            bail!("tooling.request title must be concise and non-empty");
        }
        if need.len() < 12 || need.len() > 1000 {
            bail!("tooling.request need must be concrete and bounded");
        }
        if !["low", "normal", "high", "blocking"].contains(&urgency) {
            bail!("tooling.request urgency must be low, normal, high, or blocking");
        }
        let policy = self.decide("tooling.request", json!({"title": title, "need": need, "attemptedCount": attempted.len(), "urgency": urgency}));
        if !policy.decision.can_execute() {
            bail!("tooling.request blocked by policy: {}", policy.decision.as_str());
        }
        let (turn_id, tool_call_id, project_key) = self.starter_context()?;
        let packet_id = Uuid::new_v4();
        let attempted = attempted.into_iter().take(12).collect::<Vec<_>>();
        let proposed_text = proposed.trim().to_string();
        let proposed_json = if proposed_text.is_empty() {
            None
        } else {
            serde_json::from_str::<JsonValue>(&proposed_text).ok()
        };
        let proposed_value = if proposed_text.is_empty() {
            JsonValue::Null
        } else {
            proposed_json.clone().unwrap_or_else(|| json!({"text": proposed_text}))
        };
        let project_key_for_insert = project_key.clone();
        let project_key_for_packets = project_key.clone();
        let role_id = self.role_snapshot.id.clone();
        let session_id = self.session_id;
        let script_run_id = self.script_run_id;
        let pool = self.pool.clone();
        block_on_host_future(async {
            sqlx::query(
                r#"
                INSERT INTO starter_tooling_requests (
                    id, session_id, role_id, project_key, turn_id, script_run_id, tool_call_id,
                    visible_command_summary_hash, title, need, attempted, proposed, urgency, status, route
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'routed',$14)
                "#,
            )
            .bind(packet_id)
            .bind(session_id)
            .bind(&role_id)
            .bind(project_key_for_insert)
            .bind(turn_id)
            .bind(script_run_id)
            .bind(tool_call_id)
            .bind(format!("{:x}", Sha256::digest(role_id.as_bytes())))
            .bind(title)
            .bind(need)
            .bind(json!(attempted.clone()))
            .bind(proposed_value.clone())
            .bind(urgency)
            .bind(json!({"preferred": ["project_progenitor", "orchestrator", "operator", "owner"]}))
            .execute(&self.pool)
            .await?;
            let routing_metadata = json!({
                "source": "tooling.request",
                "preferredDestinations": ["project-progenitor", "orchestrator", "operator", "owner"],
                "roleId": role_id,
                "toolCallId": tool_call_id,
                "scriptRunId": script_run_id,
            });
            let runtime_packet_id = lifecycle_hooks::record_runtime_packet(
                &pool,
                project_key_for_packets.as_deref(),
                Some(session_id),
                None,
                turn_id,
                "tooling.request",
                "routed",
                json!({
                    "starterToolingRequestId": packet_id,
                    "title": title,
                    "need": need,
                    "attempted": attempted,
                    "proposed": proposed_value,
                    "urgency": urgency,
                }),
                None,
                routing_metadata.clone(),
                &format!("tooling-request-{packet_id}"),
            ).await?;
            let target_role_id: Option<String> = sqlx::query_scalar(
                "SELECT r.id FROM roles r JOIN role_versions rv ON rv.id=r.current_version_id WHERE r.id='project-progenitor' LIMIT 1",
            )
            .fetch_optional(&pool)
            .await?;
            let envelope_id = lifecycle_hooks::route_packet_envelope(
                &pool,
                runtime_packet_id,
                "tooling_request",
                Some(session_id),
                None,
                target_role_id.as_deref(),
                "pending",
                json!({
                    "source": "tooling.request",
                    "preferredDestinations": ["project-progenitor", "orchestrator", "operator", "owner"],
                    "starterToolingRequestId": packet_id,
                }),
            ).await?;
            let mut route = json!({
                "preferred": ["project_progenitor", "orchestrator", "operator", "owner"],
                "runtimePacketId": runtime_packet_id,
                "envelopeId": envelope_id,
            });
            if let Some(target) = target_role_id {
                route["targetRoleId"] = JsonValue::String(target);
            }
            let follow_on_packet_id = record_tooling_follow_on_request(
                &pool,
                project_key_for_packets.as_deref(),
                session_id,
                turn_id,
                packet_id,
                proposed_json.as_ref(),
            ).await?;
            if let Some(follow_on_id) = follow_on_packet_id {
                route["followOnPacketId"] = JsonValue::String(follow_on_id.to_string());
            }
            sqlx::query("UPDATE starter_tooling_requests SET route=$2 WHERE id=$1")
                .bind(packet_id)
                .bind(route)
                .execute(&pool)
                .await?;
            Ok::<(), anyhow::Error>(())
        })?;
        Ok(serde_json::to_string(&json!({"packetId": packet_id, "routingStatus": "routed", "title": title, "urgency": urgency}))?)
    }

    fn decide(&self, action: &str, input: JsonValue) -> crate::policy::PolicyResult {
        let decision = PolicyEngine::decide(&self.role_snapshot, action, input);
        self.records.borrow_mut().push(HostRecord::Policy(PolicyDecisionRecord {
            decision: decision.decision.as_str().to_string(),
            payload: decision.to_event_payload(),
        }));
        decision
    }

    fn workflow_memory_help(&self) -> anyhow::Result<String> {
        let input = json!({"mode": "latestPriorFailedNonMemoryScript", "limit": 5});
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

    fn request_project_runtime_config_change(&self, project: &str, source: &str, manifest_json: &str, rationale: &str) -> anyhow::Result<String> {
        let (_, _, current_project_key) = self.starter_context()?;
        if current_project_key.as_deref() != Some(project) {
            bail!("project_runtime.request_config_change is scoped to the current project; requested={project} current={}", current_project_key.unwrap_or_else(|| "unassigned".to_string()));
        }
        let manifest: JsonValue = serde_json::from_str(manifest_json)
            .context("project_runtime.request_config_change manifest_json must be valid JSON")?;
        let input = json!({
            "projectKey": project,
            "sourceHash": lifecycle_hooks::source_hash(source),
            "manifest": manifest,
            "rationale": rationale,
        });
        let policy = self.decide("project_runtime.request_change", input.clone());
        if !policy.decision.can_execute() {
            bail!("project_runtime.request_config_change blocked by policy: {}", policy.decision.as_str());
        }
        let packet_id = block_on_host_future(lifecycle_hooks::request_project_runtime_config_change(
            &self.pool,
            project,
            self.session_id,
            source,
            input["manifest"].clone(),
            rationale,
        ))?;
        self.records.borrow_mut().push(HostRecord::HostApi(HostApiRecord {
            id: Uuid::new_v4(),
            action: "project_runtime.request_config_change".to_string(),
            status: "completed".to_string(),
            input,
            output: json!({"packetId": packet_id, "status": "reviewable"}),
            duration_ms: 0,
            truncation: json!({}),
        }));
        Ok(serde_json::to_string(&json!({"packetId": packet_id, "status": "reviewable"}))?)
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

    fn require_shell_grant(&self) -> anyhow::Result<Uuid> {
        let grant_id = self.god_mode_grant_id.ok_or_else(|| anyhow::anyhow!("God Mode required: shell(...) disabled"))?;
        #[cfg(test)]
        if grant_id == Uuid::nil() {
            return Ok(grant_id);
        }
        let grant = block_on_host_future(crate::god_mode::require_active_grant(&self.pool, self.session_id))?;
        if grant.id != grant_id {
            bail!("God Mode required: shell(...) disabled");
        }
        Ok(grant_id)
    }

    fn shell_argv(mode: &str, script: &str) -> anyhow::Result<Vec<String>> {
        match mode {
            "-lc" => Ok(vec!["-lc".to_string(), script.to_string()]),
            "-c" => Ok(vec!["-c".to_string(), script.to_string()]),
            "-l" => Ok(vec!["-l".to_string(), "-c".to_string(), script.to_string()]),
            other => bail!("invalid shell mode: {other}; expected -lc, -l, or -c"),
        }
    }

    fn run_shell_sync(&self, script: &str, mode: &str, cwd: &str) -> anyhow::Result<String> {
        if self.god_mode_grant_id.is_none() {
            self.records.borrow_mut().push(HostRecord::Shell(ShellRecord {
                id: Uuid::new_v4(),
                god_mode_grant_id: None,
                invocation_mode: mode.to_string(),
                shell_path: "/bin/zsh".to_string(),
                script: script.to_string(),
                cwd: cwd.to_string(),
                status: "rejected".to_string(),
                stdout_artifact_id: None,
                stderr_artifact_id: None,
                process_id: None,
                exit_status: None,
                failure: Some("God Mode required: shell(...) disabled".to_string()),
                duration_ms: 0,
                metadata: json!({"reason": "godModeRequired"}),
            }));
            bail!("God Mode required: shell(...) disabled");
        }
        let grant_id = self.require_shell_grant()?;
        let argv = Self::shell_argv(mode, script)?;
        let resolved_cwd = self.root.resolve_cwd(if cwd.trim().is_empty() { "." } else { cwd })?;
        let started = Instant::now();
        let shell_path = "/bin/zsh";
        let output = Command::new(shell_path)
            .args(&argv)
            .current_dir(&resolved_cwd)
            .output()
            .with_context(|| "failed to run God Mode shell through /bin/zsh")?;
        let duration_ms = started.elapsed().as_millis() as i64;
        let full_stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let full_stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = if full_stderr.is_empty() { full_stdout.clone() } else { format!("{}{}", full_stdout, full_stderr) };
        let (visible, visible_truncated) = truncate_text(&combined, OUTPUT_LIMIT_BYTES);
        let status = if output.status.success() { "completed" } else { "failed" }.to_string();
        self.records.borrow_mut().push(HostRecord::Shell(ShellRecord {
            id: Uuid::new_v4(),
            god_mode_grant_id: Some(grant_id),
            invocation_mode: mode.to_string(),
            shell_path: shell_path.to_string(),
            script: script.to_string(),
            cwd: resolved_cwd.display().to_string(),
            status: status.clone(),
            stdout_artifact_id: None,
            stderr_artifact_id: None,
            process_id: None,
            exit_status: output.status.code(),
            failure: if output.status.success() { None } else { Some(format!("zsh exited with status {:?}", output.status.code())) },
            duration_ms,
            metadata: json!({"argv": argv, "stdout": full_stdout, "stderr": full_stderr, "visibleTruncated": visible_truncated}),
        }));
        self.records.borrow_mut().push(HostRecord::HostApi(HostApiRecord {
            id: Uuid::new_v4(),
            action: "shell.sync".to_string(),
            status: status.clone(),
            input: json!({"mode": mode, "cwd": resolved_cwd, "scriptHash": format!("{:x}", Sha256::digest(script.as_bytes())), "godModeGrantId": grant_id}),
            output: json!({"stdoutBytes": full_stdout.len(), "stderrBytes": full_stderr.len()}),
            duration_ms,
            truncation: json!({"visibleTruncated": visible_truncated, "limitBytes": OUTPUT_LIMIT_BYTES}),
        }));
        self.output.borrow_mut().push(visible.clone());
        if !output.status.success() {
            bail!("shell(...).sync() failed: {visible}");
        }
        Ok(visible)
    }

    fn start_shell_async(&self, script: &str, mode: &str, cwd: &str) -> anyhow::Result<String> {
        if self.god_mode_grant_id.is_none() {
            self.records.borrow_mut().push(HostRecord::Shell(ShellRecord {
                id: Uuid::new_v4(),
                god_mode_grant_id: None,
                invocation_mode: mode.to_string(),
                shell_path: "/bin/zsh".to_string(),
                script: script.to_string(),
                cwd: cwd.to_string(),
                status: "rejected".to_string(),
                stdout_artifact_id: None,
                stderr_artifact_id: None,
                process_id: None,
                exit_status: None,
                failure: Some("God Mode required: shell(...) disabled".to_string()),
                duration_ms: 0,
                metadata: json!({"reason": "godModeRequired"}),
            }));
            bail!("God Mode required: shell(...) disabled");
        }
        let grant_id = self.require_shell_grant()?;
        let argv = Self::shell_argv(mode, script)?;
        let resolved_cwd = self.root.resolve_cwd(if cwd.trim().is_empty() { "." } else { cwd })?;
        let handle = format!("shell_{}", Uuid::new_v4().simple());
        let shell_path = "/bin/zsh";
        let mut command = Command::new(shell_path);
        command.args(&argv).current_dir(&resolved_cwd).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::piped());
        command.process_group(0);
        let mut child = command.spawn().with_context(|| "failed to start God Mode shell through /bin/zsh")?;
        let child_pid = child.id();
        let stdout_buf = Arc::new(Mutex::new(String::new()));
        let stderr_buf = Arc::new(Mutex::new(String::new()));
        spawn_reader(child.stdout.take(), Arc::clone(&stdout_buf));
        spawn_reader(child.stderr.take(), Arc::clone(&stderr_buf));
        let process_id = Uuid::new_v4();
        self.records.borrow_mut().push(HostRecord::ManagedProcess(ManagedProcessRecord {
            id: process_id,
            handle: handle.clone(),
            command_version_id: None,
            binary_name: "zsh".to_string(),
            binary_path: shell_path.to_string(),
            argv: argv.clone(),
            cwd: resolved_cwd.display().to_string(),
            os_pid: Some(child_pid as i64),
            os_pgid: Some(child_pid as i64),
            status: "running".to_string(),
            end_of_turn_behavior: "continue".to_string(),
            end_of_session_behavior: "terminate".to_string(),
            max_runtime_ms: None,
            termination_reason: None,
            event: "shell.started".to_string(),
            payload: json!({"handle": handle, "godModeGrantId": grant_id, "mode": mode, "scriptHash": format!("{:x}", Sha256::digest(script.as_bytes()))}),
        }));
        self.records.borrow_mut().push(HostRecord::Shell(ShellRecord {
            id: Uuid::new_v4(),
            god_mode_grant_id: Some(grant_id),
            invocation_mode: mode.to_string(),
            shell_path: shell_path.to_string(),
            script: script.to_string(),
            cwd: resolved_cwd.display().to_string(),
            status: "running".to_string(),
            stdout_artifact_id: None,
            stderr_artifact_id: None,
            process_id: Some(process_id),
            exit_status: None,
            failure: None,
            duration_ms: 0,
            metadata: json!({"argv": argv, "handle": handle}),
        }));
        PROCESS_MANAGER.lock().map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?.entry(self.session_id).or_default().insert(handle.clone(), ManagedProcess {
            id: process_id,
            handle: handle.clone(),
            command_version_id: None,
            binary_name: "zsh".to_string(),
            binary_path: shell_path.to_string(),
            argv,
            cwd: resolved_cwd.display().to_string(),
            child,
            stdout: stdout_buf,
            stderr: stderr_buf,
            stdout_flush_cursor: 0,
            stderr_flush_cursor: 0,
            started: Instant::now(),
            started_at: Utc::now(),
            status: "running".to_string(),
            end_of_turn_behavior: "continue".to_string(),
            end_of_session_behavior: "terminate".to_string(),
            max_runtime: None,
            min_await_ms: 0,
            max_await_ms: 600_000,
            terminate_grace_ms: 1_000,
            output_limit: OUTPUT_LIMIT_BYTES,
            stdin_policy: "allow".to_string(),
            termination_reason: None,
        });
        Ok(handle)
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
            command_version_id: Some(command_version.version_id),
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
            command_version_id: Some(command_version.version_id),
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
            end_of_turn_behavior: command_version.end_of_turn_behavior.clone(),
            end_of_session_behavior: end_of_session_behavior(&command_version),
            max_runtime: command_version.max_runtime,
            min_await_ms: command_version.min_await_ms,
            max_await_ms: command_version.max_await_ms,
            terminate_grace_ms: command_version.terminate_grace_ms,
            output_limit: command_version.output_limit,
            stdin_policy: command_version.stdin_policy.clone(),
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
        let min_ms = proc.min_await_ms.max(0) as u64;
        let max_ms = proc.max_await_ms.max(0) as u64;
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
        let stdout_artifact_id = Uuid::new_v4();
        let stderr_artifact_id = Uuid::new_v4();
        let stdout_envelope = output_artifacts::envelope_for(stdout_artifact_id, "stdout", &stdout);
        let stderr_envelope = output_artifacts::envelope_for(stderr_artifact_id, "stderr", &stderr);
        self.records.borrow_mut().push(HostRecord::ProcessOutput(ProcessOutputRecord { artifact_id: stdout_artifact_id, process_id: proc.id, handle: handle.to_string(), stream: "stdout".to_string(), content: stdout.clone(), truncated: stdout_truncated }));
        self.records.borrow_mut().push(HostRecord::ProcessOutput(ProcessOutputRecord { artifact_id: stderr_artifact_id, process_id: proc.id, handle: handle.to_string(), stream: "stderr".to_string(), content: stderr.clone(), truncated: stderr_truncated }));
        self.records.borrow_mut().push(HostRecord::ManagedProcess(proc.snapshot_record("process.flushed", json!({"handle": handle, "stdoutBytes": stdout.len(), "stderrBytes": stderr.len()}))));
        Ok(json!({
            "stdoutArtifact": stdout_envelope,
            "stderrArtifact": stderr_envelope,
            "message": "Full process stdout and stderr are stored as separate durable output artifacts. Use outputs.head/tail/slice/search/stats for bounded retrieval."
        }).to_string())
    }

    fn proc_terminate(&self, handle: &str) -> anyhow::Result<String> {
        let mut manager = PROCESS_MANAGER.lock().map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
        let proc = manager.get_mut(&self.session_id).and_then(|processes| processes.get_mut(handle)).ok_or_else(|| anyhow::anyhow!("session-only process no longer attached to this runtime session: {handle}"))?;
        proc.terminate("terminated", true)?;
        self.records.borrow_mut().push(HostRecord::ManagedProcess(proc.snapshot_record("process.terminated", json!({"handle": handle, "terminateGraceMs": proc.terminate_grace_ms}))));
        Ok("terminated".to_string())
    }

    fn proc_input(&self, handle: &str, text: &str) -> anyhow::Result<String> {
        let mut manager = PROCESS_MANAGER.lock().map_err(|_| anyhow::anyhow!("process manager lock poisoned"))?;
        let proc = manager.get_mut(&self.session_id).and_then(|processes| processes.get_mut(handle)).ok_or_else(|| anyhow::anyhow!("session-only process no longer attached to this runtime session: {handle}"))?;
        if proc.stdin_policy != "allow" {
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
        let full_stdout = stdout;
        let full_stderr = stderr;
        let stdout_artifact_id = Uuid::new_v4();
        let stderr_artifact_id = Uuid::new_v4();
        let stdout_envelope = output_artifacts::envelope_for(stdout_artifact_id, "stdout", &full_stdout);
        let stderr_envelope = output_artifacts::envelope_for(stderr_artifact_id, "stderr", &full_stderr);
        let (stdout, stdout_truncated) = truncate_text(&full_stdout, command_version.output_limit);
        let (stderr, stderr_truncated) = truncate_text(&full_stderr, command_version.output_limit);
        let truncation = json!({
            "stdoutTruncated": stdout_truncated,
            "stderrTruncated": stderr_truncated,
            "limitBytes": command_version.output_limit,
            "artifactIds": {
                "stdout": stdout_artifact_id,
                "stderr": stderr_artifact_id
            },
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
            output: json!({
                "stdout": stdout,
                "stderr": stderr,
                "exitStatus": exit_status,
                "artifacts": {
                    "stdout": stdout_envelope,
                    "stderr": stderr_envelope,
                }
            }),
            duration_ms: started.elapsed().as_millis() as i64,
            truncation: truncation.clone(),
        }));
        self.records.borrow_mut().push(HostRecord::Command(CommandRecord {
            id: Uuid::new_v4(),
            host_api_call_id,
            command_version_id: command_version.version_id,
            stdout_artifact_id,
            stderr_artifact_id,
            binary_name: command_version.binary_name.clone(),
            binary_path,
            argv,
            cwd,
            status: status.to_string(),
            stdout: full_stdout.clone(),
            stderr: full_stderr.clone(),
            exit_status,
            max_runtime_ms: command_version.max_runtime.map(|d| d.as_millis() as i64),
            duration_ms: started.elapsed().as_millis() as i64,
            truncation,
            policy_decision,
        }));
        if status == "completed" {
            Ok(json!({
                "stdoutArtifact": stdout_envelope,
                "stderrArtifact": stderr_envelope,
                "exitStatus": exit_status,
                "message": "Full command stdout and stderr are stored as separate durable output artifacts. Use outputs.head/tail/slice/search/stats for bounded retrieval."
            }).to_string())
        } else if status == "maxRuntimeExceeded" {
            bail!("command exceeded maxRuntimeMs")
        } else {
            bail!(stderr)
        }
    }

    fn run_fs_write(&self, path: &str, content: &str, description: &str) -> anyhow::Result<String> {
        require_mutation_description("fs.write", description)?;
        let started = Instant::now();
        let input = json!({"path": path, "content": content, "description": description, "executionRoot": self.root.as_path().display().to_string()});
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
            truncation: json!({"contentBytes": content.len(), "description": description}),
        }));
        if let Some(error) = error {
            bail!(error);
        }
        Ok(json!({"path": path, "status": status}).to_string())
    }

    fn run_patch_apply(&self, unified_diff: &str, description: &str) -> anyhow::Result<String> {
        require_mutation_description("patch.apply", description)?;
        let started = Instant::now();
        let affected = affected_paths(unified_diff)?;
        let mut resolved = Vec::new();
        for path in &affected {
            resolved.push(self.root.validate_patch_path(path)?);
        }
        let input = json!({"unifiedDiff": unified_diff, "description": description, "affectedPaths": affected, "executionRoot": self.root.as_path().display().to_string()});
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
            truncation: json!({"diffBytes": unified_diff.len(), "description": description}),
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
            let _ = process.refresh_status();
            if process.status == "running" && process.end_of_turn_behavior == "terminate" {
                let _ = process.terminate("endOfTurnCleanup", true);
                self.records.borrow_mut().push(HostRecord::ManagedProcess(process.snapshot_record("process.endOfTurnCleanup", json!({"handle": process.handle}))));
                remove_handles.push(handle.clone());
            } else if process.status == "running" {
                self.records.borrow_mut().push(HostRecord::ManagedProcess(process.snapshot_record("process.continued", json!({"handle": process.handle, "note": "session-only process remains attached only while this runtime instance owns it"}))));
            } else {
                let status = process.status.clone();
                let handle_for_release = handle.clone();
                let _ = block_on_host_future(async {
                    let row: Option<(i32,)> = sqlx::query_as("UPDATE starter_managed_servers SET status=$3, updated_at=now() WHERE session_id=$1 AND handle=$2 AND status='running' RETURNING port")
                        .bind(self.session_id)
                        .bind(&handle_for_release)
                        .bind(&status)
                        .fetch_optional(&self.pool)
                        .await?;
                    if let Some((port,)) = row {
                        sqlx::query("UPDATE starter_port_leases SET status='released', released_at=COALESCE(released_at, now()), release_reason='process.exit' WHERE session_id=$1 AND allocated_port=$2 AND status='active'")
                            .bind(self.session_id)
                            .bind(port)
                            .execute(&self.pool)
                            .await?;
                    }
                    Ok::<(), anyhow::Error>(())
                });
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
                if self.max_runtime.is_some_and(|max| self.started.elapsed() >= max) && !status.success() {
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
            if let Some(max_runtime) = self.max_runtime {
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
            terminate_process_group(self.child.id(), self.terminate_grace_ms);
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
        let truncated = new.len() > self.output_limit;
        (new, truncated)
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
        let truncated = new.len() > self.output_limit;
        (new, truncated)
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
            end_of_turn_behavior: self.end_of_turn_behavior.clone(),
            end_of_session_behavior: self.end_of_session_behavior.clone(),
            max_runtime_ms: self.max_runtime.map(|d| d.as_millis() as i64),
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
        "file.replace_exact" => execute_resumed_file_replace_exact(pool, session_id, turn_id, script_run_id, input, policy_decision).await,
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
    let description = input.get("description").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused fs.write missing description"))?;
    require_mutation_description("fs.write", description)?;
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
        INSERT INTO file_mutations (id, script_run_id, action_name, path, before_state, after_state, status, started_at, completed_at, duration_ms, policy_decision, mutation_description, truncation)
        VALUES ($1, $2, 'fs.write', $3, $4, $5, 'completed', $6, $6, $7, $8, $9, $10)
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
    .bind(description)
    .bind(json!({"contentBytes": content.len(), "description": description}))
    .execute(pool)
    .await?;
    db::append_event(pool, session_id, turn_id, "file_mutation", Some(id), "file_mutation.completed", Some("completed"), json!({"action":"fs.write","description":description,"path":resolved.display().to_string(),"before":before,"after":after,"durationMs":duration_ms,"policyDecision":policy_decision})).await?;
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
    let description = input.get("description").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused patch.apply missing description"))?;
    require_mutation_description("patch.apply", description)?;
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
        INSERT INTO patch_runs (id, script_run_id, action_name, affected_paths, before_state, after_state, status, error, started_at, completed_at, duration_ms, policy_decision, mutation_description, truncation)
        VALUES ($1, $2, 'patch.apply', $3, $4, $5, $6, $7, $8, $8, $9, $10, $11, $12)
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
    .bind(description)
    .bind(json!({"diffBytes": diff.len(), "description": description}))
    .execute(pool)
    .await?;
    db::append_event(pool, session_id, turn_id, "patch", Some(id), "patch.completed", Some(status), json!({"action":"patch.apply","description":description,"affectedPaths":paths,"before":before,"after":after,"status":status,"error":error,"durationMs":duration_ms,"policyDecision":policy_decision})).await?;
    if let Some(error) = error {
        bail!(error);
    }
    Ok(json!({"patchRunId": id, "status": status}))
}

async fn execute_resumed_file_replace_exact(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    script_run_id: Uuid,
    input: &JsonValue,
    policy_decision: JsonValue,
) -> Result<JsonValue> {
    let path = input.get("path").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused file.replace_exact missing path"))?;
    let old = input.get("old").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused file.replace_exact missing old"))?;
    let new = input.get("new").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused file.replace_exact missing new"))?;
    let description = input.get("description").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused file.replace_exact missing description"))?;
    require_mutation_description("file.replace_exact", description)?;
    if old.is_empty() {
        bail!("file.replace_exact old text must not be empty");
    }
    let execution_root = input.get("executionRoot").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused file.replace_exact missing executionRoot"))?;
    let root = ExecutionRoot::new(execution_root)?;
    let resolved = root.resolve_agent_path(path, "file.replace_exact", true)?;
    let text = text_file_content("file.replace_exact", &resolved)?;
    let count = text.matches(old).count();
    if count == 0 {
        bail!("file.replace_exact old text is absent");
    }
    if count > 1 {
        bail!("file.replace_exact old text is ambiguous; found {count} matches");
    }
    let before = file_state(&resolved);
    let started = Instant::now();
    let updated = text.replacen(old, new, 1);
    std::fs::write(&resolved, updated.as_bytes())?;
    let after = file_state(&resolved);
    let duration_ms = started.elapsed().as_millis() as i64;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO file_mutations (id, script_run_id, action_name, path, before_state, after_state, status, started_at, completed_at, duration_ms, policy_decision, mutation_description, truncation)
        VALUES ($1, $2, 'file.replace_exact', $3, $4, $5, 'completed', $6, $6, $7, $8, $9, $10)
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
    .bind(description)
    .bind(json!({"description": description, "oldBytes": old.len(), "newBytes": new.len()}))
    .execute(pool)
    .await?;
    db::append_event(pool, session_id, turn_id, "file_mutation", Some(id), "file_mutation.completed", Some("completed"), json!({"action":"file.replace_exact","description":description,"path":resolved.display().to_string(),"before":before,"after":after,"durationMs":duration_ms,"policyDecision":policy_decision})).await?;
    Ok(json!({"fileMutationId": id, "status": "completed", "path": resolved.display().to_string()}))
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
    let full_stdout = stdout;
    let full_stderr = stderr;
    let stdout_artifact_id = Uuid::new_v4();
    let stderr_artifact_id = Uuid::new_v4();
    let (stdout, stdout_truncated) = truncate_text(&full_stdout, command_version.output_limit);
    let (stderr, stderr_truncated) = truncate_text(&full_stderr, command_version.output_limit);
    let truncation = json!({"stdoutTruncated": stdout_truncated, "stderrTruncated": stderr_truncated, "limitBytes": command_version.output_limit, "artifactIds": {"stdout": stdout_artifact_id, "stderr": stderr_artifact_id}});
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
    lifecycle::complete_command_run(pool, command_id, TerminalStatus::try_from(status)?, &full_stdout, &full_stderr, exit_status, duration_ms, &policy_decision, &truncation, Utc::now()).await?;
    let stdout_artifact = output_artifacts::store(pool, NewOutputArtifact { id: stdout_artifact_id, session_id, turn_id, tool_call_id: None, script_run_id: Some(script_run_id), command_run_id: Some(command_id), process_id: None, source_type: "command_run", stream: "stdout", content: &full_stdout, metadata: json!({"commandVersionId": command_version.version_id, "resumed": true}) }).await?;
    let stderr_artifact = output_artifacts::store(pool, NewOutputArtifact { id: stderr_artifact_id, session_id, turn_id, tool_call_id: None, script_run_id: Some(script_run_id), command_run_id: Some(command_id), process_id: None, source_type: "command_run", stream: "stderr", content: &full_stderr, metadata: json!({"commandVersionId": command_version.version_id, "resumed": true}) }).await?;
    db::append_event(pool, session_id, turn_id, "command", Some(command_id), "command.completed", Some(status), json!({"binary":command_version.binary_name,"binaryPath":binary_path.display().to_string(),"commandVersionId":command_version.version_id,"argv":argv,"cwd":resolved_cwd.display().to_string(),"status":status,"stdoutPreview":stdout,"stderrPreview":stderr,"exitStatus":exit_status,"maxRuntimeMs":command_version.max_runtime.map(|d| d.as_millis() as i64),"durationMs":duration_ms,"truncation":truncation,"artifacts":{"stdout":stdout_artifact,"stderr":stderr_artifact},"policyDecision":policy_decision})).await?;
    Ok(json!({"commandRunId": command_id, "hostApiCallId": host_api_call_id, "status": status, "artifacts": {"stdout": stdout_artifact, "stderr": stderr_artifact}, "exitStatus": exit_status}))
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
    if let Some(mut stdout) = child.stdout.take() {
        let target = Arc::clone(&stdout_buf);
        thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => if let Ok(mut buffer) = target.lock() {
                        buffer.push_str(&String::from_utf8_lossy(&chunk[..n]));
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
            command_version_id: Some(command_version.version_id),
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
            end_of_turn_behavior: command_version.end_of_turn_behavior.clone(),
            end_of_session_behavior: end_of_session_behavior(&command_version),
            max_runtime: command_version.max_runtime,
            min_await_ms: command_version.min_await_ms,
            max_await_ms: command_version.max_await_ms,
            terminate_grace_ms: command_version.terminate_grace_ms,
            output_limit: command_version.output_limit,
            stdin_policy: command_version.stdin_policy.clone(),
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
            Some(command_version.version_id),
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
                    || matches!(policy_result.action.as_str(), "fs.write" | "patch.apply" | "file.replace_exact")
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
            let stdout_artifact = output_artifacts::store(pool, NewOutputArtifact {
                id: command.stdout_artifact_id,
                session_id,
                turn_id: Some(turn_id),
                tool_call_id: Some(tool_call_id),
                script_run_id: Some(script_run_id),
                command_run_id: Some(command.id),
                process_id: None,
                source_type: "command_run",
                stream: "stdout",
                content: &command.stdout,
                metadata: json!({"commandVersionId": command.command_version_id}),
            }).await?;
            let stderr_artifact = output_artifacts::store(pool, NewOutputArtifact {
                id: command.stderr_artifact_id,
                session_id,
                turn_id: Some(turn_id),
                tool_call_id: Some(tool_call_id),
                script_run_id: Some(script_run_id),
                command_run_id: Some(command.id),
                process_id: None,
                source_type: "command_run",
                stream: "stderr",
                content: &command.stderr,
                metadata: json!({"commandVersionId": command.command_version_id}),
            }).await?;
            let (stdout_preview, stdout_preview_truncated) = truncate_text(&command.stdout, OUTPUT_LIMIT_BYTES);
            let (stderr_preview, stderr_preview_truncated) = truncate_text(&command.stderr, OUTPUT_LIMIT_BYTES);
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
                    "stdoutPreview": stdout_preview,
                    "stderrPreview": stderr_preview,
                    "exitStatus": command.exit_status,
                    "maxRuntimeMs": command.max_runtime_ms,
                    "durationMs": command.duration_ms,
                    "truncation": command.truncation,
                    "previewTruncation": {"stdout": stdout_preview_truncated, "stderr": stderr_preview_truncated},
                    "artifacts": {"stdout": stdout_artifact, "stderr": stderr_artifact},
                    "policyDecision": command.policy_decision,
                    "processGroupTermination": {
                        "attempted": command.status == "maxRuntimeExceeded",
                        "reason": if command.status == "maxRuntimeExceeded" { "maxRuntimeExceeded" } else { "naturalExit" },
                    },
                }),
            )
            .await?;
        }
        HostRecord::Shell(shell) => {
            let full_stdout = shell.metadata.get("stdout").and_then(JsonValue::as_str).unwrap_or("").to_string();
            let full_stderr = shell.metadata.get("stderr").and_then(JsonValue::as_str).unwrap_or("").to_string();
            let mut stdout_artifact_id = shell.stdout_artifact_id;
            let mut stderr_artifact_id = shell.stderr_artifact_id;
            let artifacts = if shell.status == "running" {
                json!({})
            } else {
                let stdout_id = stdout_artifact_id.unwrap_or_else(Uuid::new_v4);
                let stderr_id = stderr_artifact_id.unwrap_or_else(Uuid::new_v4);
                stdout_artifact_id = Some(stdout_id);
                stderr_artifact_id = Some(stderr_id);
                let stdout_artifact = output_artifacts::store(pool, NewOutputArtifact {
                    id: stdout_id,
                    session_id,
                    turn_id: Some(turn_id),
                    tool_call_id: Some(tool_call_id),
                    script_run_id: Some(script_run_id),
                    command_run_id: None,
                    process_id: shell.process_id,
                    source_type: "shell_run",
                    stream: "stdout",
                    content: &full_stdout,
                    metadata: json!({"godModeGrantId": shell.god_mode_grant_id, "mode": shell.invocation_mode}),
                }).await?;
                let stderr_artifact = output_artifacts::store(pool, NewOutputArtifact {
                    id: stderr_id,
                    session_id,
                    turn_id: Some(turn_id),
                    tool_call_id: Some(tool_call_id),
                    script_run_id: Some(script_run_id),
                    command_run_id: None,
                    process_id: shell.process_id,
                    source_type: "shell_run",
                    stream: "stderr",
                    content: &full_stderr,
                    metadata: json!({"godModeGrantId": shell.god_mode_grant_id, "mode": shell.invocation_mode}),
                }).await?;
                json!({"stdout": stdout_artifact, "stderr": stderr_artifact})
            };
            sqlx::query(
                r#"
                INSERT INTO shell_runs (
                    id, script_run_id, session_id, turn_id, tool_call_id, god_mode_grant_id,
                    invocation_mode, shell_path, script_hash, script_source, cwd, status,
                    completed_at, duration_ms, stdout_artifact_id, stderr_artifact_id,
                    process_id, exit_status, failure, metadata
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                    CASE WHEN $12 = 'running' THEN NULL ELSE now() END, $13, $14, $15, $16, $17, $18, $19)
                "#,
            )
            .bind(shell.id)
            .bind(script_run_id)
            .bind(session_id)
            .bind(turn_id)
            .bind(tool_call_id)
            .bind(shell.god_mode_grant_id)
            .bind(&shell.invocation_mode)
            .bind(&shell.shell_path)
            .bind(format!("{:x}", Sha256::digest(shell.script.as_bytes())))
            .bind(&shell.script)
            .bind(&shell.cwd)
            .bind(&shell.status)
            .bind(shell.duration_ms)
            .bind(stdout_artifact_id)
            .bind(stderr_artifact_id)
            .bind(shell.process_id)
            .bind(shell.exit_status)
            .bind(&shell.failure)
            .bind(&shell.metadata)
            .execute(pool)
            .await?;
            db::append_event(
                pool,
                session_id,
                Some(turn_id),
                "shell",
                Some(shell.id),
                if shell.status == "running" { "shell.started" } else { "shell.completed" },
                Some(&shell.status),
                json!({
                    "godModeGrantId": shell.god_mode_grant_id,
                    "mode": shell.invocation_mode,
                    "shellPath": shell.shell_path,
                    "cwd": shell.cwd,
                    "status": shell.status,
                    "scriptHash": format!("{:x}", Sha256::digest(shell.script.as_bytes())),
                    "processId": shell.process_id,
                    "exitStatus": shell.exit_status,
                    "failure": shell.failure,
                    "durationMs": shell.duration_ms,
                    "artifacts": artifacts,
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
            let artifact = output_artifacts::store(pool, NewOutputArtifact {
                id: output.artifact_id,
                session_id,
                turn_id: Some(turn_id),
                tool_call_id: Some(tool_call_id),
                script_run_id: Some(script_run_id),
                command_run_id: None,
                process_id: Some(output.process_id),
                source_type: "managed_process",
                stream: &output.stream,
                content: &output.content,
                metadata: json!({"handle": output.handle, "chunkTruncated": output.truncated}),
            }).await?;
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
                    "artifact": artifact,
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
            let mutation_description = mutation
                .truncation
                .get("description")
                .and_then(JsonValue::as_str)
                .map(str::to_string);
            sqlx::query(
                r#"
                INSERT INTO file_mutations (
                    id, script_run_id, action_name, path, before_state, after_state, status,
                    error, started_at, completed_at, duration_ms, policy_decision, mutation_description, truncation
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9, $10, $11, $12, $13)
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
            .bind(mutation_description.clone())
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
                    "description": mutation_description,
                    "truncation": mutation.truncation,
                    "policyDecision": mutation.policy_decision,
                }),
            )
            .await?;
        }
        HostRecord::PatchRun(patch) => {
            let mutation_description = patch
                .truncation
                .get("description")
                .and_then(JsonValue::as_str)
                .map(str::to_string);
            sqlx::query(
                r#"
                INSERT INTO patch_runs (
                    id, script_run_id, action_name, affected_paths, before_state, after_state,
                    status, error, started_at, completed_at, duration_ms, policy_decision, mutation_description, truncation
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9, $10, $11, $12, $13)
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
            .bind(mutation_description.clone())
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
                    "description": mutation_description,
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

async fn persist_managed_process_control_record(pool: &PgPool, session_id: Uuid, process: ManagedProcessRecord) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE managed_processes
        SET status = $3,
            end_time = CASE WHEN $3 = 'running' THEN end_time ELSE now() END,
            termination_reason = $4,
            metadata = metadata || $5
        WHERE session_id = $1 AND handle = $2
        "#,
    )
    .bind(session_id)
    .bind(&process.handle)
    .bind(&process.status)
    .bind(&process.termination_reason)
    .bind(json!({
        "lastEvent": process.event,
        "payload": process.payload,
    }))
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        None,
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
            "endOfSessionBehavior": process.end_of_session_behavior,
            "maxRuntimeMs": process.max_runtime_ms,
            "terminationReason": process.termination_reason,
            "payload": process.payload,
        }),
    )
    .await?;
    Ok(())
}

async fn persist_process_output_record(pool: &PgPool, session_id: Uuid, turn_id: Option<Uuid>, output: &ProcessOutputRecord) -> Result<()> {
    let _artifact = output_artifacts::store(pool, NewOutputArtifact {
        id: output.artifact_id,
        session_id,
        turn_id,
        tool_call_id: None,
        script_run_id: None,
        command_run_id: None,
        process_id: Some(output.process_id),
        source_type: "managed_process",
        stream: &output.stream,
        content: &output.content,
        metadata: json!({"handle": output.handle, "chunkTruncated": output.truncated}),
    }).await?;
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
        turn_id,
        "process",
        Some(output.process_id),
        "process.output",
        Some("completed"),
        json!({
            "handle": output.handle,
            "stream": output.stream,
            "bytes": output.content.len(),
            "artifactId": output.artifact_id,
            "truncated": output.truncated,
        }),
    )
    .await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::{
        LifecycleAuthorityMetadata, ManifestDecision, ModelDefaults, RoleSnapshot, RoutingMetadata,
        VisibilityMetadata,
    };
    use chrono::Utc;
    use serde_json::json;
    use sqlx::{postgres::PgPoolOptions, Row};
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
        policy.insert("tree.list".to_string(), ManifestDecision::Allow);
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
            scope_type: "global".to_string(),
            project_key: None,
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
            end_of_session_behavior: "terminate".to_string(),
            stdin_policy: stdin_policy.to_string(),
            min_await_ms: 100,
            max_await_ms: 60_000,
            output_buffer_bytes: 4096,
            terminate_grace_ms: 10,
        }
    }

    #[tokio::test]
    async fn command_describe_affordances_are_visible_in_starlark() {
        let root = ExecutionRoot::new(".").expect("root");
        let session = Uuid::new_v4();
        let cmd = test_command("echo", &["/bin/echo"], "cmd.describe.echo", "echo_tool", "forbid", "terminate", None);
        let result = evaluate_starlark(
            test_pool(),
            session,
            Uuid::new_v4(),
            "print(cmd.describe())\nprint(cmd[\"echo_tool\"].describe())\nprint(cmd[\"echo_tool\"].run.describe())",
            root,
            test_role(),
            None,
            vec![cmd],
            vec![],
        );
        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(result.output.contains("cmd.describe.echo"));
        assert!(result.output.contains("echo_tool"));
        assert!(result.output.contains("sync"));
        assert!(result.output.contains("stdin"));
    }

    #[tokio::test]
    async fn print_is_visible_output_and_legacy_output_is_unavailable() {
        let root = ExecutionRoot::new(".").expect("root");
        let session = Uuid::new_v4();
        let printed = evaluate_starlark(
            test_pool(),
            session,
            Uuid::new_v4(),
            r#"print(cmd.describe())"#,
            root.clone(),
            test_role(),
            None,
            vec![test_command("echo", &["/bin/echo"], "cmd.describe.echo", "echo_tool", "forbid", "terminate", None)],
            vec![],
        );
        assert!(printed.error.is_none(), "{:?}", printed.error);
        assert!(printed.output.contains("cmd.describe.echo"));

        let legacy = evaluate_starlark(
            test_pool(),
            session,
            Uuid::new_v4(),
            r#"output(cmd.describe())"#,
            root,
            test_role(),
            None,
            vec![test_command("echo", &["/bin/echo"], "cmd.describe.echo", "echo_tool", "forbid", "terminate", None)],
            vec![],
        );
        let error = legacy.error.unwrap_or_default();
        assert!(error.contains("Variable `output` not found") || error.contains("not found: output"), "{error}");
        assert!(!error.contains("blocked by policy"), "{error}");
    }

    #[tokio::test]
    async fn print_accepts_zero_and_multiple_positional_arguments() {
        let root = ExecutionRoot::new(".").expect("root");
        let session = Uuid::new_v4();
        let result = evaluate_starlark(
            test_pool(),
            session,
            Uuid::new_v4(),
            r#"print()
print("label:", 123, True)"#,
            root,
            test_role(),
            None,
            vec![],
            vec![],
        );
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.output, "\nlabel: 123 True");
    }

    #[tokio::test]
    async fn print_commands_label_and_command_description_succeeds() {
        let root = ExecutionRoot::new(".").expect("root");
        let session = Uuid::new_v4();
        let result = evaluate_starlark(
            test_pool(),
            session,
            Uuid::new_v4(),
            r#"print("commands:", cmd.describe())"#,
            root,
            test_role(),
            None,
            vec![test_command("echo", &["/bin/echo"], "cmd.describe.echo", "echo_tool", "forbid", "terminate", None)],
            vec![],
        );
        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(result.output.starts_with("commands: "), "{}", result.output);
        assert!(result.output.contains("cmd.describe.echo"), "{}", result.output);
        assert!(result.output.contains("echo_tool"), "{}", result.output);
    }

    #[tokio::test]
    async fn print_tree_label_and_tree_list_succeeds() {
        let root = ExecutionRoot::new(".").expect("root");
        let session = Uuid::new_v4();
        let result = evaluate_starlark(
            test_pool(),
            session,
            Uuid::new_v4(),
            r#"tree = struct(list = lambda path, depth=2: "visible.txt")
print("tree:", tree.list(".", depth=2))"#,
            root,
            test_role(),
            None,
            vec![],
            vec![],
        );
        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(result.output.starts_with("tree: "), "{}", result.output);
        assert!(result.output.contains("visible.txt"), "{}", result.output);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn project_progenitor_can_call_workflow_memory_help_without_policy_denial() {
        let root = ExecutionRoot::new(".").expect("root");
        let mut role = test_role();
        role.id = "project-progenitor".to_string();
        role.policy.clear();
        role.policy.insert("tool.execute_code".to_string(), ManifestDecision::Allow);
        role.policy.insert("workflow_memory.search".to_string(), ManifestDecision::Allow);
        let result = evaluate_starlark(
            test_pool(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            r#"print(workflow_memory.help())"#,
            root,
            role,
            None,
            vec![],
            vec![],
        );
        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(!result.output.contains("blocked by policy"), "{}", result.output);
    }

    #[test]
    fn execute_code_failure_hint_is_failure_only_and_names_print_help() {
        let failed = execute_code_result_output(json!({"id": "stdout"}), json!({"id": "stderr"}), true);
        let hint = failed.get("hint").and_then(JsonValue::as_str).unwrap_or_default();
        assert_eq!(hint, FAILED_EXECUTE_CODE_RECOVERY_HINT);
        assert!(hint.contains("print(workflow_memory.help())"));
        assert!(!hint.contains("output("));

        let succeeded = execute_code_result_output(json!({"id": "stdout"}), json!({"id": "stderr"}), false);
        assert!(succeeded.get("hint").is_none(), "{succeeded}");
        assert!(!succeeded.to_string().contains(FAILED_EXECUTE_CODE_RECOVERY_HINT));
    }

    #[tokio::test]
    async fn command_discovery_updates_on_next_execute_code_boundary_and_non_visible_fails() {
        let root = ExecutionRoot::new(".").expect("root");
        let session = Uuid::new_v4();
        let source = "print(cmd[\"echo_tool\"].run.describe())";
        let missing = evaluate_starlark(
            test_pool(),
            session,
            Uuid::new_v4(),
            source,
            root.clone(),
            test_role(),
            None,
            vec![],
            vec![],
        );
        assert!(missing.error.as_deref().unwrap_or_default().to_lowercase().contains("echo_tool"), "{:?}", missing.error);
        let cmd = test_command("echo", &["/bin/echo"], "cmd.describe.echo", "echo_tool", "forbid", "terminate", None);
        let visible = evaluate_starlark(
            test_pool(),
            session,
            Uuid::new_v4(),
            source,
            root,
            test_role(),
            None,
            vec![cmd],
            vec![],
        );
        assert!(visible.error.is_none(), "{:?}", visible.error);
        assert!(visible.output.contains("cmd.describe.echo"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn continuing_process_is_controllable_later_in_same_runtime_and_isolated_by_session() {
        let session = Uuid::new_v4();
        let other_session = Uuid::new_v4();
        let root = ExecutionRoot::new(".").unwrap();
        let cmd = test_command("yes", &["/usr/bin/yes"], "cmd.yes.run", "yes", "forbid", "continue", Some(Duration::from_millis(500)));
        let first = evaluate_starlark(test_pool(), session, Uuid::new_v4(), "h = cmd[\"yes\"].run(args=[], cwd=\".\").start(); print(h)", root.clone(), test_role(), None, vec![cmd.clone()], vec![]);
        assert!(first.error.is_none(), "{:?}", first.error);
        let handle = first.output.trim().trim_matches('"').to_string();
        assert!(handle.starts_with("proc_"));
        assert!(first.records.iter().any(|record| matches!(record, HostRecord::ManagedProcess(process) if process.event == "process.continued")));

        let second = evaluate_starlark(test_pool(), session, Uuid::new_v4(), &format!("proc[{handle:?}].await_for(mins=0); out = proc[{handle:?}].flush_buffer(); proc[{handle:?}].terminate(); print(out)"), root.clone(), test_role(), None, vec![cmd.clone()], vec![handle.clone()]);
        assert!(second.error.is_none(), "{:?}", second.error);
        assert!(second.output.contains('y'));
        assert!(second.records.iter().any(|record| matches!(record, HostRecord::ManagedProcess(process) if process.event == "process.terminated")));

        let isolated = evaluate_starlark(test_pool(), other_session, Uuid::new_v4(), &format!("proc[{handle:?}].is_running(); print(\"bad\")"), root, test_role(), None, vec![cmd], vec![handle]);
        assert!(isolated.error.unwrap_or_default().contains("session-only process no longer attached"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stdin_allowed_process_accepts_input_and_flush_cursor_advances() {
        let session = Uuid::new_v4();
        let root = ExecutionRoot::new(".").unwrap();
        let python = std::env::var("PYTHON3").unwrap_or_else(|_| "/Library/Frameworks/Python.framework/Versions/3.13/bin/python3".to_string());
        let cmd = test_command("python3", &[&python], "cmd.python.stdin", "python", "allow", "terminate", Some(Duration::from_secs(2)));
        let script = r#"
h = cmd["python"].run(args=["-u", "-c", "import sys,time; print(sys.stdin.readline().strip(), flush=True); time.sleep(1)"], cwd=".").start()
proc[h].input("hello-process\n")
proc[h].await_for(mins=0)
first = proc[h].flush_buffer()
second = proc[h].flush_buffer()
print(first + "|second=" + second)
"#;
        let result = evaluate_starlark(test_pool(), session, Uuid::new_v4(), script, root, test_role(), None, vec![cmd], vec![]);
        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(result.output.contains("hello-process"));
        assert!(result.output.contains("|second="));
        let streams = result.records.iter().filter_map(|record| match record {
            HostRecord::ProcessOutput(output) => Some(output.stream.as_str()),
            _ => None,
        }).collect::<Vec<_>>();
        assert!(streams.contains(&"stdout"));
        assert!(streams.contains(&"stderr"));
        assert!(!streams.contains(&"combined"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn async_max_runtime_expires_without_handle_polling_and_detached_handle_errors_are_clear() {
        let session = Uuid::new_v4();
        let root = ExecutionRoot::new(".").unwrap();
        let cmd = test_command("yes", &["/usr/bin/yes"], "cmd.yes.max.run", "yes_max", "forbid", "continue", Some(Duration::from_millis(100)));
        let first = evaluate_starlark(test_pool(), session, Uuid::new_v4(), "h = cmd[\"yes_max\"].run(args=[], cwd=\".\").start(); print(h)", root.clone(), test_role(), None, vec![cmd.clone()], vec![]);
        assert!(first.error.is_none(), "{:?}", first.error);
        let handle = first.output.trim().trim_matches('"').to_string();
        thread::sleep(Duration::from_millis(250));
        let second = evaluate_starlark(test_pool(), session, Uuid::new_v4(), &format!("running = proc[{handle:?}].is_running(); print(str(running))"), root.clone(), test_role(), None, vec![cmd.clone()], vec![handle.clone()]);
        assert!(second.error.is_none(), "{:?}", second.error);
        assert!(second.records.iter().any(|record| matches!(record, HostRecord::ManagedProcess(process) if process.status == "maxRuntimeExceeded" || process.event == "process.continued")));

        PROCESS_MANAGER.lock().unwrap().remove(&session);
        let detached = evaluate_starlark(test_pool(), session, Uuid::new_v4(), &format!("proc[{handle:?}].is_running(); print(\"bad\")"), root, test_role(), None, vec![cmd], vec![handle]);
        assert!(detached.error.unwrap_or_default().contains("session-only process no longer attached"));
    }

    #[tokio::test]
    async fn shell_is_present_but_disabled_without_god_mode_grant() {
        let result = evaluate_starlark(
            test_pool(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            r#"shell("echo should-not-run").sync()"#,
            ExecutionRoot::new(".").unwrap(),
            test_role(),
            None,
            vec![],
            vec![],
        );
        let error = result.error.unwrap_or_default();
        assert!(error.contains("God Mode required: shell(...) disabled"), "{error}");
        assert!(!result.records.iter().any(|record| matches!(record, HostRecord::ManagedProcess(_))));
        assert!(result.records.iter().any(|record| matches!(record, HostRecord::Shell(shell) if shell.status == "rejected")));
        let async_result = evaluate_starlark(
            test_pool(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            r#"shell("echo should-not-run").async()"#,
            ExecutionRoot::new(".").unwrap(),
            test_role(),
            None,
            vec![],
            vec![],
        );
        assert!(async_result.error.unwrap_or_default().contains("God Mode required: shell(...) disabled"));
    }

    #[tokio::test]
    async fn registry_command_output_survives_before_disabled_shell_failure() {
        let cmd = test_command("echo", &["/bin/echo"], "cmd.echo.integration", "echo_tool", "forbid", "terminate", None);
        let result = evaluate_starlark(
            test_pool(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            r#"print(cmd["echo_tool"].run(args=["registry-ok"], cwd=".").sync())
shell("echo should-not-run").sync()
print("unreachable")"#,
            ExecutionRoot::new(".").unwrap(),
            test_role(),
            None,
            vec![cmd],
            vec![],
        );
        assert!(result.output.contains("registry-ok"), "registry output was not preserved: {}", result.output);
        let error = result.error.unwrap_or_default();
        assert!(error.contains("God Mode required: shell(...) disabled"), "{error}");
        assert!(result.records.iter().any(|record| matches!(record, HostRecord::Command(command) if command.stdout.contains("registry-ok"))));
        assert!(result.records.iter().any(|record| matches!(record, HostRecord::Shell(shell) if shell.status == "rejected")));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn god_mode_shell_sync_modes_and_async_handle_are_available_with_grant() {
        let session = Uuid::new_v4();
        let root = ExecutionRoot::new(".").unwrap();
        let sync_result = evaluate_starlark(
            test_pool(),
            session,
            Uuid::new_v4(),
            r#"print(shell("printf sync-ok", mode="-lc").sync())
print(shell("printf c-ok", mode="-c").sync())
print(shell("printf login-ok", mode="-l").sync())"#,
            root.clone(),
            test_role(),
            Some(Uuid::nil()),
            vec![],
            vec![],
        );
        assert!(sync_result.error.is_none(), "{:?}", sync_result.error);
        assert!(sync_result.output.contains("sync-ok"));
        assert!(sync_result.output.contains("c-ok"));
        assert!(sync_result.output.contains("login-ok"));
        assert!(sync_result.records.iter().filter(|record| matches!(record, HostRecord::Shell(shell) if shell.status == "completed")).count() >= 3);
        assert!(sync_result.records.iter().any(|record| matches!(record, HostRecord::Shell(shell) if shell.invocation_mode == "-l" && shell.metadata["argv"].as_array().is_some_and(|argv| argv.iter().any(|value| value == "-l")))));
        let invalid = evaluate_starlark(
            test_pool(),
            session,
            Uuid::new_v4(),
            r#"shell("printf bad", mode="-x").sync()"#,
            root.clone(),
            test_role(),
            Some(Uuid::nil()),
            vec![],
            vec![],
        );
        assert!(invalid.error.unwrap_or_default().contains("invalid shell mode"));
        let async_result = evaluate_starlark(
            test_pool(),
            session,
            Uuid::new_v4(),
            r#"handle = shell("read line; printf shell-input:%s\\n \"$line\"; sleep 1").async()
proc[handle].input("typed-stdin\n")
proc[handle].await_for(mins=1)
flushed = proc[handle].flush_buffer()
terminated = proc[handle].terminate()
print(handle + "\n" + flushed + "\n" + terminated)"#,
            root.clone(),
            test_role(),
            Some(Uuid::nil()),
            vec![],
            vec![],
        );
        assert!(async_result.error.is_none(), "{:?}", async_result.error);
        assert!(async_result.output.contains("shell_"));
        assert!(async_result.output.contains("shell-input:typed-stdin"), "{}", async_result.output);
        assert!(async_result.output.contains("terminated"), "{}", async_result.output);
        assert!(async_result.records.iter().any(|record| matches!(record, HostRecord::ManagedProcess(process) if process.command_version_id.is_none() && process.end_of_session_behavior == "terminate")));
        assert!(async_result.records.iter().any(|record| matches!(record, HostRecord::ManagedProcess(process) if process.event == "process.stdin")));
        assert!(async_result.records.iter().any(|record| matches!(record, HostRecord::ManagedProcess(process) if process.event == "process.awaited")));
        assert!(async_result.records.iter().any(|record| matches!(record, HostRecord::ProcessOutput(output) if output.stream == "stdout" && output.content.contains("shell-input:typed-stdin"))));
        assert!(!async_result.records.iter().any(|record| matches!(record, HostRecord::ProcessOutput(output) if output.stream == "combined")));
        assert!(async_result.records.iter().any(|record| matches!(record, HostRecord::ManagedProcess(process) if process.event == "process.terminated")));
        let _ = terminate_session_processes_for_close(session);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn god_mode_shell_runs_persist_full_output_artifacts_and_are_retrievable() -> Result<()> {
        let database_url = std::env::var("ROBDEX_AGENT_RUNTIME_TEST_DATABASE_URL")
            .or_else(|_| std::env::var("ROBDEX_AGENT_RUNTIME_DATABASE_URL"))
            .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/robdex_agent_runtime".to_string());
        let pool = PgPoolOptions::new().max_connections(5).connect(&database_url).await?;
        crate::db::init(&pool).await?;
        let role = test_role();
        let root_dir = std::env::temp_dir().join(format!("agent-runtime-shell-artifacts-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root_dir)?;
        let root = ExecutionRoot::new(&root_dir)?;
        let session = crate::db::new_session(&pool, &role, Some("shell-artifacts"), root_dir.to_str().unwrap(), None, None, None).await?;
        let grant = crate::god_mode::grant_session(&pool, session, "test", "verify shell artifact persistence", None).await?;
        let turn_id = Uuid::new_v4();
        let tool_call_id = Uuid::new_v4();
        let source = r#"result = shell("printf 'out-line\n'; printf 'err-line\n' 1>&2").sync()
print(result)"#;
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user',$3,'running',now())")
            .bind(turn_id)
            .bind(session)
            .bind(source)
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status, started_at) VALUES ($1,$2,$3,'execute_code',$4,$5,'running',now())")
            .bind(tool_call_id)
            .bind(session)
            .bind(turn_id)
            .bind(format!("call_{tool_call_id}"))
            .bind(json!({"source": source}))
            .execute(&pool)
            .await?;

        let packet = execute_code(&pool, session, turn_id, tool_call_id, source, &root, &role).await?;
        assert!(packet.ok, "{packet:?}");

        let row = sqlx::query(
            r#"
            SELECT god_mode_grant_id, invocation_mode, shell_path, script_hash, script_source, cwd,
                   status, stdout_artifact_id, stderr_artifact_id,
                   process_id, exit_status, failure
            FROM shell_runs
            WHERE session_id = $1
            ORDER BY started_at DESC
            LIMIT 1
            "#,
        )
        .bind(session)
        .fetch_one(&pool)
        .await?;
        let persisted_grant_id: Uuid = row.try_get("god_mode_grant_id")?;
        assert_eq!(persisted_grant_id, grant.id);
        assert_eq!(row.try_get::<String, _>("invocation_mode")?, "-lc");
        assert_eq!(row.try_get::<String, _>("shell_path")?, "/bin/zsh");
        assert!(!row.try_get::<String, _>("script_hash")?.is_empty());
        assert!(row.try_get::<String, _>("script_source")?.contains("out-line"));
        assert!(row.try_get::<String, _>("cwd")?.ends_with(root_dir.file_name().unwrap().to_string_lossy().as_ref()));
        assert_eq!(row.try_get::<String, _>("status")?, "completed");
        assert!(row.try_get::<Option<Uuid>, _>("process_id")?.is_none());
        assert_eq!(row.try_get::<Option<i32>, _>("exit_status")?, Some(0));
        assert!(row.try_get::<Option<String>, _>("failure")?.is_none());
        let stdout_artifact_id: Uuid = row.try_get("stdout_artifact_id")?;
        let stderr_artifact_id: Uuid = row.try_get("stderr_artifact_id")?;

        let stdout = output_artifacts::retrieve(&pool, session, stdout_artifact_id, "head", Some(10), None, None, None, None).await?;
        let stderr = output_artifacts::retrieve(&pool, session, stderr_artifact_id, "head", Some(10), None, None, None, None).await?;
        assert!(stdout.content.contains("out-line"), "{stdout:?}");
        assert!(stderr.content.contains("err-line"), "{stderr:?}");
        sqlx::query("UPDATE turns SET status='completed', completed_at=now() WHERE id=$1")
            .bind(turn_id)
            .execute(&pool)
            .await?;
        let async_turn_id = Uuid::new_v4();
        let async_tool_call_id = Uuid::new_v4();
        let async_source = r#"handle = shell("read line; printf 'async-out:%s\n' \"$line\"; printf 'async-err\n' 1>&2").async()
proc[handle].input("typed-stdin\n")
proc[handle].await_for(mins=1)
flushed = proc[handle].flush_buffer()
terminated = proc[handle].terminate()
print(flushed + "\n" + terminated)"#;
        sqlx::query("INSERT INTO turns (id, session_id, role, input_text, status, started_at) VALUES ($1,$2,'user',$3,'running',now())")
            .bind(async_turn_id)
            .bind(session)
            .bind(async_source)
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO tool_calls (id, session_id, turn_id, tool_name, call_identity, input, status, started_at) VALUES ($1,$2,$3,'execute_code',$4,$5,'running',now())")
            .bind(async_tool_call_id)
            .bind(session)
            .bind(async_turn_id)
            .bind(format!("call_{async_tool_call_id}"))
            .bind(json!({"source": async_source}))
            .execute(&pool)
            .await?;
        let async_packet = execute_code(&pool, session, async_turn_id, async_tool_call_id, async_source, &root, &role).await?;
        assert!(async_packet.ok, "{async_packet:?}");
        let async_row = sqlx::query(
            r#"
            SELECT id, script_run_id, process_id, status
            FROM shell_runs
            WHERE session_id = $1 AND turn_id = $2
            ORDER BY started_at DESC
            LIMIT 1
            "#,
        )
        .bind(session)
        .bind(async_turn_id)
        .fetch_one(&pool)
        .await?;
        let async_script_run_id: Uuid = async_row.try_get("script_run_id")?;
        let async_process_id: Uuid = async_row.try_get::<Option<Uuid>, _>("process_id")?.expect("async shell process id");
        let async_artifacts: Vec<(String, String, Option<Uuid>, Option<Uuid>, Option<Uuid>, Option<Uuid>, i64)> = sqlx::query(
            "SELECT stream, content, session_id, turn_id, tool_call_id, script_run_id, process_id, byte_count FROM execution_output_artifacts WHERE process_id=$1 ORDER BY stream"
        )
        .bind(async_process_id)
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|row| (row.get("stream"), row.get("content"), row.get("session_id"), row.get("turn_id"), row.get("tool_call_id"), row.get("script_run_id"), row.get("byte_count")))
        .collect();
        assert!(async_artifacts.iter().any(|(stream, content, artifact_session, artifact_turn, artifact_tool, artifact_script, bytes)| stream == "stdout" && content.contains("async-out:typed-stdin") && *artifact_session == Some(session) && *artifact_turn == Some(async_turn_id) && *artifact_tool == Some(async_tool_call_id) && *artifact_script == Some(async_script_run_id) && *bytes > 0));
        assert!(async_artifacts.iter().any(|(stream, content, artifact_session, artifact_turn, artifact_tool, artifact_script, bytes)| stream == "stderr" && content.contains("async-err") && *artifact_session == Some(session) && *artifact_turn == Some(async_turn_id) && *artifact_tool == Some(async_tool_call_id) && *artifact_script == Some(async_script_run_id) && *bytes > 0));
        let async_events: Vec<String> = sqlx::query_scalar("SELECT event_type FROM event_stream WHERE entity_id=$1 ORDER BY sequence ASC")
            .bind(async_process_id)
            .fetch_all(&pool)
            .await?;
        for expected in ["shell.started", "process.stdin", "process.awaited", "process.output", "process.terminated"] {
            assert!(async_events.iter().any(|event| event == expected), "missing async process event {expected}: {async_events:?}");
        }
        let combined_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_output_artifacts WHERE session_id=$1 AND stream='combined'")
            .bind(session)
            .fetch_one(&pool)
            .await?;
        assert_eq!(combined_rows, 0);
        Ok(())
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
            let turn_status = if packet.ok { "completed" } else { "failed" };
            sqlx::query("UPDATE turns SET status=$2, completed_at=now() WHERE id=$1")
                .bind(turn_id)
                .bind(turn_status)
                .execute(pool)
                .await?;
            Ok(serde_json::to_value(packet)?)
        }

        let promote_source = r#"fs.write("memory-target.txt", "needle workflow memory success", "write workflow memory promotion target")
text = fs.read("memory-target.txt")
workflow_memory.remember_when(text == "needle workflow memory success", "Write memory target", "Use fs.write then fs.read to verify exact content after missing-file failures")
print("promoted")"#;
        run_script(&pool, seed_session, &root, &role, promote_source).await.unwrap();
        let memory_id: Uuid = sqlx::query_scalar("SELECT id FROM workflow_memories WHERE session_id=$1 LIMIT 1").bind(seed_session).fetch_one(&pool).await.unwrap();
        run_script(&pool, seed_session, &root, &role, promote_source).await.unwrap();
        let memory_count: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memories WHERE session_id=$1").bind(seed_session).fetch_one(&pool).await.unwrap();
        assert_eq!(memory_count, 1);
        let duplicate_events: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.duplicate_collapsed'").bind(seed_session).fetch_one(&pool).await.unwrap();
        assert!(duplicate_events > 0);

        let failed = run_script(&pool, fail_session, &root, &role, r#"fs.read("missing-workflow-memory-file.txt")
print("unreachable")"#).await;
        assert_eq!(failed.unwrap()["ok"], false);
        let failed_embeddings: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_script_embeddings WHERE session_id=$1").bind(fail_session).fetch_one(&pool).await.unwrap();
        assert!(failed_embeddings > 0);
        let failed_promotions: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memories WHERE session_id=$1").bind(fail_session).fetch_one(&pool).await.unwrap();
        assert_eq!(failed_promotions, 0);

        run_script(&pool, fail_session, &root, &role, r#"tips = workflow_memory.help()
print(tips)"#).await.unwrap();
        let help_results: i64 = sqlx::query_scalar("SELECT (payload->>'resultCount')::bigint FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.help' ORDER BY created_at DESC LIMIT 1").bind(fail_session).fetch_one(&pool).await.unwrap();
        assert!(help_results > 0);

        run_script(&pool, fail_session, &root, &role, &format!(r#"workflow_memory.mark_attempted("{memory_id}", variant=True)
workflow_memory.mark_not_helpful("{memory_id}", "not enough context")
print("feedback")"#)).await.unwrap();
        let feedback_events: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_events WHERE memory_id=$1 AND event_type IN ('workflow_memory.mark_attempted','workflow_memory.mark_not_helpful')").bind(memory_id).fetch_one(&pool).await.unwrap();
        assert_eq!(feedback_events, 2);

        let denied = run_script(&pool, deny_session, &root, &deny_role, r#"workflow_memory.remember_when(True, "Denied memory", "restrictive role should block")
print("denied")"#).await;
        assert_eq!(denied.unwrap()["ok"], false);
        let denied_memories: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memories WHERE session_id=$1").bind(deny_session).fetch_one(&pool).await.unwrap();
        assert_eq!(denied_memories, 0);

        let invisible_feedback = run_script(&pool, beta_session, &root, &role, &format!(r#"workflow_memory.mark_attempted("{memory_id}", variant=True)
print("bad")"#)).await;
        assert_eq!(invisible_feedback.unwrap()["ok"], false);
        let invisible_error: String = sqlx::query_scalar("SELECT stderr FROM script_runs sr JOIN tool_calls tc ON tc.id=sr.tool_call_id WHERE tc.session_id=$1 ORDER BY sr.started_at DESC LIMIT 1").bind(beta_session).fetch_one(&pool).await.unwrap();
        assert!(invisible_error.contains("not visible"));

        let beta_failed = run_script(&pool, beta_session, &root, &role, r#"fs.read("missing-workflow-memory-file.txt")
print("unreachable")"#).await;
        assert_eq!(beta_failed.unwrap()["ok"], false);
        run_script(&pool, beta_session, &root, &role, r#"tips = workflow_memory.help()
print(tips)"#).await.unwrap();
        let beta_help_results: i64 = sqlx::query_scalar("SELECT (payload->>'resultCount')::bigint FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.help' ORDER BY created_at DESC LIMIT 1").bind(beta_session).fetch_one(&pool).await.unwrap();
        assert_eq!(beta_help_results, 0);
        let beta_help_mentions_alpha: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.help' AND payload::text LIKE $2").bind(beta_session).bind(format!("%{memory_id}%")).fetch_one(&pool).await.unwrap();
        assert_eq!(beta_help_mentions_alpha, 0);

        let ordinary_before: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.help'").bind(seed_session).fetch_one(&pool).await.unwrap();
        run_script(&pool, seed_session, &root, &role, r#"print("ordinary no help")"#).await.unwrap();
        let ordinary_after: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_memory_events WHERE session_id=$1 AND event_type='workflow_memory.help'").bind(seed_session).fetch_one(&pool).await.unwrap();
        assert_eq!(ordinary_before, ordinary_after);
    }
}
