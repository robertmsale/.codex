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
use crate::lifecycle::{self, TerminalStatus};
use crate::policy::{PolicyEngine, RuntimeDecision};
use crate::roles::RoleSnapshot;

const OUTPUT_LIMIT_BYTES: usize = 12_000;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct BinaryRegistry {
    rg: BinaryEntry,
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
        }
    }

    fn rg(&self) -> Result<ResolvedBinary> {
        self.rg.resolve()
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
        persist_record(pool, session_id, turn_id, script_run_id, record).await?;
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
cmd = {{"rg": cmd_rg}}
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
    builder.namespace_no_docs("cmd_rg", cmd_rg_builtins);
    output_builtins(builder);
}

#[starlark_module]
fn fs_builtins(builder: &mut GlobalsBuilder) {
    fn read<'v>(path: &'v str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<String> {
        host_kernel(eval).run_fs_read(path)
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
        let input = json!({"binary": "rg", "argv": args, "cwd": cwd});
        let policy = self.decide("cmd.rg.run", input.clone());
        if !policy.decision.can_execute() {
            bail!("cmd.rg.run blocked by policy: {}", policy.decision.as_str());
        }
        let binary = self.registry.rg()?;
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
            "action": "cmd.rg.run",
            "decision": RuntimeDecision::Allow.as_str(),
            "reason": policy.reason,
            "role": {"id": policy.role_id, "version": policy.role_version},
        });
        let host_api_call_id = Uuid::new_v4();
        self.records.borrow_mut().push(HostRecord::HostApi(HostApiRecord {
            id: host_api_call_id,
            action: "cmd.rg.run",
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
}

async fn persist_record(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    script_run_id: Uuid,
    record: &HostRecord,
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
