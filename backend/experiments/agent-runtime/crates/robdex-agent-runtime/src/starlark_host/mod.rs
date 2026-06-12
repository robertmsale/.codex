use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::process::Stdio;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use starlark::any::ProvidesStaticType;
use starlark::environment::{GlobalsBuilder, Module};
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::Value;
use starlark::values::list::UnpackList;
use starlark::values::none::NoneType;
use uuid::Uuid;
use wait_timeout::ChildExt;

use crate::db;
use crate::approvals;
use crate::lifecycle::{self, TerminalStatus};
use crate::policy::{PolicyEngine, RuntimeDecision};
use crate::roles::RoleSnapshot;

const OUTPUT_LIMIT_BYTES: usize = 12_000;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct BinaryRegistry {
    rg: BinaryEntry,
    git: BinaryEntry,
    cargo: BinaryEntry,
}

impl BinaryRegistry {
    pub fn new() -> Self {
        Self {
            rg: BinaryEntry {
                name: "rg",
                candidate_paths: vec![
                    PathBuf::from("/opt/homebrew/bin/rg"),
                    PathBuf::from("/usr/local/bin/rg"),
                    PathBuf::from("/usr/bin/rg"),
                ],
                cwd_policy: CwdPolicy::UnderExecutionRoot,
                env_policy: EnvPolicy::Empty,
                timeout: COMMAND_TIMEOUT,
                output_limit: OUTPUT_LIMIT_BYTES,
            },
            git: BinaryEntry {
                name: "git",
                candidate_paths: vec![
                    PathBuf::from("/opt/homebrew/bin/git"),
                    PathBuf::from("/usr/local/bin/git"),
                    PathBuf::from("/usr/bin/git"),
                ],
                cwd_policy: CwdPolicy::UnderExecutionRoot,
                env_policy: EnvPolicy::Empty,
                timeout: COMMAND_TIMEOUT,
                output_limit: OUTPUT_LIMIT_BYTES,
            },
            cargo: BinaryEntry {
                name: "cargo",
                candidate_paths: vec![
                    PathBuf::from("/Users/robertsale/.cargo/bin/cargo"),
                    PathBuf::from("/opt/homebrew/bin/cargo"),
                    PathBuf::from("/usr/local/bin/cargo"),
                    PathBuf::from("/usr/bin/cargo"),
                ],
                cwd_policy: CwdPolicy::UnderExecutionRoot,
                env_policy: EnvPolicy::MinimalCargo,
                timeout: Duration::from_secs(120),
                output_limit: OUTPUT_LIMIT_BYTES,
            },
        }
    }

    fn rg(&self) -> Result<ResolvedBinary> {
        self.rg.resolve()
    }

    fn git(&self) -> Result<ResolvedBinary> {
        self.git.resolve()
    }

    fn cargo(&self) -> Result<ResolvedBinary> {
        self.cargo.resolve()
    }
}

impl Default for BinaryRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct BinaryEntry {
    name: &'static str,
    candidate_paths: Vec<PathBuf>,
    cwd_policy: CwdPolicy,
    env_policy: EnvPolicy,
    timeout: Duration,
    output_limit: usize,
}

#[derive(Debug, Clone, Copy, Serialize)]
enum CwdPolicy {
    UnderExecutionRoot,
}

#[derive(Debug, Clone, Copy, Serialize)]
enum EnvPolicy {
    Empty,
    MinimalCargo,
}

#[derive(Debug, Clone)]
struct ResolvedBinary {
    name: &'static str,
    path: PathBuf,
    cwd_policy: CwdPolicy,
    env_policy: EnvPolicy,
    timeout: Duration,
    output_limit: usize,
}

impl BinaryEntry {
    fn resolve(&self) -> Result<ResolvedBinary> {
        let path = self
            .candidate_paths
            .iter()
            .find(|candidate| candidate.is_file())
            .cloned()
            .with_context(|| format!("registered binary `{}` is not available", self.name))?;
        Ok(ResolvedBinary {
            name: self.name,
            path,
            cwd_policy: self.cwd_policy,
            env_policy: self.env_policy,
            timeout: self.timeout,
            output_limit: self.output_limit,
        })
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
    root: ExecutionRoot,
    registry: BinaryRegistry,
    role_snapshot: RoleSnapshot,
    output: RefCell<Vec<String>>,
    records: RefCell<Vec<HostRecord>>,
}

#[derive(Debug)]
enum HostRecord {
    Policy(PolicyDecisionRecord),
    HostApi(HostApiRecord),
    Command(CommandRecord),
    FileMutation(FileMutationRecord),
    PatchRun(PatchRunRecord),
}

#[derive(Debug)]
struct PolicyDecisionRecord {
    decision: String,
    payload: JsonValue,
}

#[derive(Debug)]
struct HostApiRecord {
    id: Uuid,
    action: &'static str,
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
    binary_name: String,
    binary_path: String,
    argv: Vec<String>,
    cwd: String,
    status: String,
    stdout: String,
    stderr: String,
    exit_status: Option<i32>,
    timeout_ms: i64,
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

    let result = evaluate_starlark(source, root.clone(), role_snapshot.clone());
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
    error: Option<String>,
}

fn evaluate_starlark(source: &str, root: ExecutionRoot, role_snapshot: RoleSnapshot) -> EvalResult {
    let script = format!(
        r#"
cmd = {{"rg": cmd_rg, "git": cmd_git, "cargo": cmd_cargo}}
{source}
"#
    );
    let kernel = HostKernel {
        root,
        registry: BinaryRegistry::new(),
        role_snapshot,
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
    let records = kernel.records.into_inner();
    EvalResult {
        output,
        records,
        error,
    }
}

fn add_host_builtins(builder: &mut GlobalsBuilder) {
    builder.namespace("fs", fs_builtins);
    builder.namespace("patch", patch_builtins);
    builder.namespace_no_docs("cmd_rg", cmd_rg_builtins);
    builder.namespace_no_docs("cmd_git", cmd_git_builtins);
    builder.namespace_no_docs("cmd_cargo", cmd_cargo_builtins);
    output_builtins(builder);
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
fn cmd_rg_builtins(builder: &mut GlobalsBuilder) {
    fn run<'v>(
        args: UnpackList<Value<'v>>,
        cwd: Option<&'v str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        let args = args
            .items
            .iter()
            .map(|value| {
                value
                    .unpack_str()
                    .map(ToString::to_string)
                    .ok_or_else(|| anyhow::anyhow!("cmd[\"rg\"].run args must be strings"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        host_kernel(eval).run_rg(args, cwd.unwrap_or("."))
    }
}

#[starlark_module]
fn cmd_git_builtins(builder: &mut GlobalsBuilder) {
    fn status<'v>(eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).run_git_status()
    }

    fn diff<'v>(
        args: UnpackList<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        let args = args
            .items
            .iter()
            .map(|value| {
                value
                    .unpack_str()
                    .map(ToString::to_string)
                    .ok_or_else(|| anyhow::anyhow!("cmd[\"git\"].diff args must be strings"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        host_kernel(eval).run_git_diff(args)
    }
}

#[starlark_module]
fn cmd_cargo_builtins(builder: &mut GlobalsBuilder) {
    fn check<'v>(
        args: UnpackList<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        let args = args
            .items
            .iter()
            .map(|value| {
                value
                    .unpack_str()
                    .map(ToString::to_string)
                    .ok_or_else(|| anyhow::anyhow!("cmd[\"cargo\"].check args must be strings"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        host_kernel(eval).run_cargo_check(args)
    }
}

#[starlark_module]
fn output_builtins(builder: &mut GlobalsBuilder) {
    fn output<'v>(value: Value<'v>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<NoneType> {
        host_kernel(eval).output.borrow_mut().push(value.to_string());
        Ok(NoneType)
    }
}

fn host_kernel<'v, 'a>(eval: &Evaluator<'v, 'a, '_>) -> &'a HostKernel {
    eval.extra
        .expect("HostKernel must be installed in Evaluator.extra")
        .downcast_ref::<HostKernel>()
        .expect("Evaluator.extra must be HostKernel")
}

impl HostKernel {
    fn decide(&self, action: &'static str, input: JsonValue) -> crate::policy::PolicyResult {
        let decision = PolicyEngine::decide(&self.role_snapshot, action, input);
        self.records.borrow_mut().push(HostRecord::Policy(PolicyDecisionRecord {
            decision: decision.decision.as_str().to_string(),
            payload: decision.to_event_payload(),
        }));
        decision
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
            action: "fs.read",
            status: "completed".to_string(),
            input,
            output: json!({"content": output, "resolvedPath": resolved.display().to_string()}),
            duration_ms: started.elapsed().as_millis() as i64,
            truncation: json!({"contentTruncated": truncated, "limitBytes": OUTPUT_LIMIT_BYTES}),
        }));
        Ok(output)
    }

    fn run_rg(&self, args: Vec<String>, cwd: &str) -> anyhow::Result<String> {
        self.run_registered_command("cmd.rg.run", self.registry.rg()?, args, cwd)
    }

    fn run_git_status(&self) -> anyhow::Result<String> {
        self.run_registered_command("cmd.git.status", self.registry.git()?, vec!["status".to_string(), "--short".to_string()], ".")
    }

    fn run_git_diff(&self, args: Vec<String>) -> anyhow::Result<String> {
        if args.iter().any(|arg| arg == "--output" || arg.starts_with("--output=")) {
            bail!("cmd[\"git\"].diff does not allow output-writing arguments");
        }
        let mut argv = vec!["diff".to_string()];
        argv.extend(args);
        self.run_registered_command("cmd.git.diff", self.registry.git()?, argv, ".")
    }

    fn run_cargo_check(&self, args: Vec<String>) -> anyhow::Result<String> {
        let mut argv = vec!["check".to_string()];
        argv.extend(args);
        self.run_registered_command("cmd.cargo.check", self.registry.cargo()?, argv, ".")
    }

    fn run_registered_command(&self, action: &'static str, binary: ResolvedBinary, args: Vec<String>, cwd: &str) -> anyhow::Result<String> {
        let input = json!({
            "binary": binary.name,
            "argv": args,
            "cwd": cwd,
            "executionRoot": self.root.as_path().display().to_string(),
        });
        let policy = self.decide(action, input.clone());
        if !policy.decision.can_execute() {
            bail!("{action} blocked by policy: {}", policy.decision.as_str());
        }
        let started = Instant::now();
        let resolved_cwd = match binary.cwd_policy {
            CwdPolicy::UnderExecutionRoot => self.root.resolve_cwd(cwd)?,
        };
        let mut command = Command::new(&binary.path);
        command.args(&args).current_dir(&resolved_cwd);
        match binary.env_policy {
            EnvPolicy::Empty => {
                command.env_clear();
            }
            EnvPolicy::MinimalCargo => {
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
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let output = command.spawn().and_then(|mut child| {
            match child.wait_timeout(binary.timeout)? {
                Some(_) => child.wait_with_output(),
                None => {
                    let _ = child.kill();
                    let mut output = child.wait_with_output()?;
                    output.status = std::process::ExitStatus::from_raw(124);
                    Ok(output)
                }
            }
        });
        let (status, exit_status, stdout, stderr) = match output {
            Ok(output) => (
                if output.status.code() == Some(124) {
                    "timeout"
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
        let (stdout, stdout_truncated) = truncate_text(&stdout, binary.output_limit);
        let (stderr, stderr_truncated) = truncate_text(&stderr, binary.output_limit);
        let truncation = json!({
            "stdoutTruncated": stdout_truncated,
            "stderrTruncated": stderr_truncated,
            "limitBytes": binary.output_limit,
        });
        let policy_decision = json!({
            "action": action,
            "decision": RuntimeDecision::Allow.as_str(),
            "reason": policy.reason,
            "role": {"id": policy.role_id, "version": policy.role_version},
        });
        let host_api_call_id = Uuid::new_v4();
        self.records.borrow_mut().push(HostRecord::HostApi(HostApiRecord {
            id: host_api_call_id,
            action,
            status: status.to_string(),
            input,
            output: json!({"stdout": stdout, "stderr": stderr, "exitStatus": exit_status}),
            duration_ms: started.elapsed().as_millis() as i64,
            truncation: truncation.clone(),
        }));
        self.records.borrow_mut().push(HostRecord::Command(CommandRecord {
            id: Uuid::new_v4(),
            host_api_call_id,
            binary_name: binary.name.to_string(),
            binary_path: binary.path.display().to_string(),
            argv: args,
            cwd: resolved_cwd.display().to_string(),
            status: status.to_string(),
            stdout: stdout.clone(),
            stderr: stderr.clone(),
            exit_status,
            timeout_ms: binary.timeout.as_millis() as i64,
            duration_ms: started.elapsed().as_millis() as i64,
            truncation,
            policy_decision,
        }));
        if status == "completed" {
            Ok(stdout)
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
}

pub async fn execute_resumed_rg(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    script_run_id: Uuid,
    input: &JsonValue,
    policy_decision: JsonValue,
) -> Result<JsonValue> {
    let argv = input
        .get("argv")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| anyhow::anyhow!("paused cmd.rg.run input missing argv"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow::anyhow!("paused cmd.rg.run argv must contain strings"))
        })
        .collect::<Result<Vec<_>>>()?;
    let cwd = input
        .get("cwd")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow::anyhow!("paused cmd.rg.run input missing cwd"))?;
    let execution_root = input
        .get("executionRoot")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| anyhow::anyhow!("paused cmd.rg.run input missing executionRoot"))?;
    let root = ExecutionRoot::new(execution_root)?;
    let registry = BinaryRegistry::new();
    let binary = registry.rg()?;
    let started = Instant::now();
    let resolved_cwd = match binary.cwd_policy {
        CwdPolicy::UnderExecutionRoot => root.resolve_cwd(cwd)?,
    };
    let mut command = Command::new(&binary.path);
    command.args(&argv).current_dir(&resolved_cwd);
    match binary.env_policy {
        EnvPolicy::Empty => {
            command.env_clear();
        }
        EnvPolicy::MinimalCargo => {
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
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = command.spawn().and_then(|mut child| match child.wait_timeout(binary.timeout)? {
        Some(_) => child.wait_with_output(),
        None => {
            let _ = child.kill();
            let mut output = child.wait_with_output()?;
            output.status = std::process::ExitStatus::from_raw(124);
            Ok(output)
        }
    });
    let (status, exit_status, stdout, stderr) = match output {
        Ok(output) => (
            if output.status.code() == Some(124) {
                "timeout"
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
    let duration_ms = started.elapsed().as_millis() as i64;
    let (stdout, stdout_truncated) = truncate_text(&stdout, binary.output_limit);
    let (stderr, stderr_truncated) = truncate_text(&stderr, binary.output_limit);
    let truncation = json!({
        "stdoutTruncated": stdout_truncated,
        "stderrTruncated": stderr_truncated,
        "limitBytes": binary.output_limit,
    });
    let host_api_call_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO host_api_calls (id, script_run_id, api_name, input, status, started_at)
        VALUES ($1, $2, 'cmd.rg.run', $3, 'running', $4)
        "#,
    )
    .bind(host_api_call_id)
    .bind(script_run_id)
    .bind(input)
    .bind(Utc::now())
    .execute(pool)
    .await?;
    lifecycle::complete_host_api_call(
        pool,
        host_api_call_id,
        TerminalStatus::try_from(status)?,
        &json!({"stdout": stdout, "stderr": stderr, "exitStatus": exit_status}),
        duration_ms,
        &truncation,
        Utc::now(),
    )
    .await?;
    let command_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO command_runs (id, host_api_call_id, binary_name, argv, cwd, status, started_at, timeout_ms)
        VALUES ($1, $2, $3, $4, $5, 'running', $6, $7)
        "#,
    )
    .bind(command_id)
    .bind(host_api_call_id)
    .bind(binary.name)
    .bind(json!(argv))
    .bind(resolved_cwd.display().to_string())
    .bind(Utc::now())
    .bind(binary.timeout.as_millis() as i64)
    .execute(pool)
    .await?;
    lifecycle::complete_command_run(
        pool,
        command_id,
        TerminalStatus::try_from(status)?,
        &stdout,
        &stderr,
        exit_status,
        duration_ms,
        &policy_decision,
        &truncation,
        Utc::now(),
    )
    .await?;
    db::append_event(
        pool,
        session_id,
        turn_id,
        "command",
        Some(command_id),
        "command.completed",
        Some(status),
        json!({
            "binary": binary.name,
            "binaryPath": binary.path.display().to_string(),
            "argv": input.get("argv").cloned().unwrap_or(JsonValue::Null),
            "cwd": resolved_cwd.display().to_string(),
            "status": status,
            "stdout": stdout,
            "stderr": stderr,
            "exitStatus": exit_status,
            "timeoutMs": binary.timeout.as_millis() as i64,
            "durationMs": duration_ms,
            "truncation": truncation,
            "policyDecision": policy_decision,
        }),
    )
    .await?;
    Ok(json!({
        "commandRunId": command_id,
        "hostApiCallId": host_api_call_id,
        "status": status,
        "stdout": stdout,
        "stderr": stderr,
        "exitStatus": exit_status,
    }))
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
        "cmd.rg.run" => execute_resumed_rg(pool, session_id, turn_id, script_run_id, input, policy_decision).await,
        "cmd.git.status" | "cmd.git.diff" | "cmd.cargo.check" => {
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
    let registry = BinaryRegistry::new();
    let (binary, argv) = match action {
        "cmd.git.status" => (registry.git()?, vec!["status".to_string(), "--short".to_string()]),
        "cmd.git.diff" => {
            let mut argv = vec!["diff".to_string()];
            argv.extend(json_string_array(input, "argv")?.into_iter().skip(1));
            (registry.git()?, argv)
        }
        "cmd.cargo.check" => {
            let mut argv = vec!["check".to_string()];
            argv.extend(json_string_array(input, "argv")?.into_iter().skip(1));
            (registry.cargo()?, argv)
        }
        other => bail!("unsupported resumed command action: {other}"),
    };
    execute_resumed_command_with_binary(pool, session_id, turn_id, script_run_id, input, policy_decision, binary, argv).await
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

async fn execute_resumed_command_with_binary(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    script_run_id: Uuid,
    input: &JsonValue,
    policy_decision: JsonValue,
    binary: ResolvedBinary,
    argv: Vec<String>,
) -> Result<JsonValue> {
    let cwd = input.get("cwd").and_then(JsonValue::as_str).unwrap_or(".");
    let execution_root = input.get("executionRoot").and_then(JsonValue::as_str).ok_or_else(|| anyhow::anyhow!("paused command missing executionRoot"))?;
    let root = ExecutionRoot::new(execution_root)?;
    let started = Instant::now();
    let resolved_cwd = match binary.cwd_policy {
        CwdPolicy::UnderExecutionRoot => root.resolve_cwd(cwd)?,
    };
    let mut command = Command::new(&binary.path);
    command.args(&argv).current_dir(&resolved_cwd);
    match binary.env_policy {
        EnvPolicy::Empty => {
            command.env_clear();
        }
        EnvPolicy::MinimalCargo => {
            command.env_clear();
            if let Ok(value) = std::env::var("PATH") { command.env("PATH", value); }
            if let Ok(value) = std::env::var("HOME") { command.env("HOME", value); }
            if let Ok(value) = std::env::var("CARGO_HOME") { command.env("CARGO_HOME", value); }
            if let Ok(value) = std::env::var("RUSTUP_HOME") { command.env("RUSTUP_HOME", value); }
        }
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = command.spawn().and_then(|mut child| match child.wait_timeout(binary.timeout)? {
        Some(_) => child.wait_with_output(),
        None => {
            let _ = child.kill();
            let mut output = child.wait_with_output()?;
            output.status = std::process::ExitStatus::from_raw(124);
            Ok(output)
        }
    });
    let (status, exit_status, stdout, stderr) = match output {
        Ok(output) => (
            if output.status.code() == Some(124) { "timeout" } else if output.status.success() { "completed" } else { "failed" },
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ),
        Err(error) => ("failed", None, String::new(), error.to_string()),
    };
    let duration_ms = started.elapsed().as_millis() as i64;
    let (stdout, stdout_truncated) = truncate_text(&stdout, binary.output_limit);
    let (stderr, stderr_truncated) = truncate_text(&stderr, binary.output_limit);
    let truncation = json!({"stdoutTruncated": stdout_truncated, "stderrTruncated": stderr_truncated, "limitBytes": binary.output_limit});
    let host_api_call_id = Uuid::new_v4();
    sqlx::query("INSERT INTO host_api_calls (id, script_run_id, api_name, input, status, started_at) VALUES ($1, $2, $3, $4, 'running', $5)")
        .bind(host_api_call_id)
        .bind(script_run_id)
        .bind(policy_decision.get("action").and_then(JsonValue::as_str).unwrap_or(binary.name))
        .bind(input)
        .bind(Utc::now())
        .execute(pool)
        .await?;
    lifecycle::complete_host_api_call(pool, host_api_call_id, TerminalStatus::try_from(status)?, &json!({"stdout": stdout, "stderr": stderr, "exitStatus": exit_status}), duration_ms, &truncation, Utc::now()).await?;
    let command_id = Uuid::new_v4();
    sqlx::query("INSERT INTO command_runs (id, host_api_call_id, binary_name, argv, cwd, status, started_at, timeout_ms) VALUES ($1, $2, $3, $4, $5, 'running', $6, $7)")
        .bind(command_id)
        .bind(host_api_call_id)
        .bind(binary.name)
        .bind(json!(argv))
        .bind(resolved_cwd.display().to_string())
        .bind(Utc::now())
        .bind(binary.timeout.as_millis() as i64)
        .execute(pool)
        .await?;
    lifecycle::complete_command_run(pool, command_id, TerminalStatus::try_from(status)?, &stdout, &stderr, exit_status, duration_ms, &policy_decision, &truncation, Utc::now()).await?;
    db::append_event(pool, session_id, turn_id, "command", Some(command_id), "command.completed", Some(status), json!({"binary":binary.name,"binaryPath":binary.path.display().to_string(),"argv":argv,"cwd":resolved_cwd.display().to_string(),"status":status,"stdout":stdout,"stderr":stderr,"exitStatus":exit_status,"timeoutMs":binary.timeout.as_millis() as i64,"durationMs":duration_ms,"truncation":truncation,"policyDecision":policy_decision})).await?;
    Ok(json!({"commandRunId": command_id, "hostApiCallId": host_api_call_id, "status": status, "stdout": stdout, "stderr": stderr, "exitStatus": exit_status}))
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
                let policy_result = PolicyEngine::decide(
                    role_snapshot,
                    policy.payload["action"].as_str().unwrap_or(""),
                    policy.payload["input"].clone(),
                );
                let approval_id = approvals::request_approval(pool, session_id, Some(turn_id), &policy_result, role_snapshot).await?;
                if matches!(
                    policy_result.action.as_str(),
                    "cmd.rg.run" | "fs.write" | "patch.apply" | "cmd.git.status" | "cmd.git.diff" | "cmd.cargo.check"
                ) {
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
            .bind(call.action)
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
                INSERT INTO command_runs (id, host_api_call_id, binary_name, argv, cwd, status, started_at, timeout_ms)
                VALUES ($1, $2, $3, $4, $5, 'running', $6, $7)
                "#,
            )
            .bind(command.id)
            .bind(command.host_api_call_id)
            .bind(&command.binary_name)
            .bind(json!(command.argv))
            .bind(&command.cwd)
            .bind(Utc::now())
            .bind(command.timeout_ms)
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
                    "binaryPath": command.binary_path,
                    "argv": command.argv,
                    "cwd": command.cwd,
                    "status": command.status,
                    "stdout": command.stdout,
                    "stderr": command.stderr,
                    "exitStatus": command.exit_status,
                    "timeoutMs": command.timeout_ms,
                    "durationMs": command.duration_ms,
                    "truncation": command.truncation,
                    "policyDecision": command.policy_decision,
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
