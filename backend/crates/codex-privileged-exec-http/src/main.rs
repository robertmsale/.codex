use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use clap::Parser;
use codex_backend_core::{HttpArgs, init_tracing};
use codex_execpolicy::{
    Decision, MatchOptions, Policy, RuleMatch,
    execpolicycheck::load_policies,
};
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_command::parse_command::extract_shell_command;
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::{
    io::AsyncReadExt,
    net::TcpListener,
    process::Command,
    sync::RwLock,
};
use tower_http::trace::TraceLayer;
use tracing::info;

const DEFAULT_MAX_OUTPUT_BYTES: usize = 1024 * 1024;
#[derive(Debug, Parser)]
struct Args {
    #[arg(long, env = "CODEX_PRIVILEGED_EXEC_BIND", default_value = "127.0.0.1")]
    host: std::net::IpAddr,

    #[arg(long, env = "CODEX_PRIVILEGED_EXEC_PORT", default_value_t = 8776)]
    port: u16,

    #[arg(long = "rules", env = "CODEX_PRIVILEGED_EXEC_RULES", value_delimiter = ',', required = true)]
    rules: Vec<PathBuf>,

    #[arg(long, env = "CODEX_PRIVILEGED_EXEC_RESOLVE_HOST_EXECUTABLES", default_value_t = true)]
    resolve_host_executables: bool,

    #[arg(long, env = "CODEX_PRIVILEGED_EXEC_MAX_OUTPUT_BYTES", default_value_t = DEFAULT_MAX_OUTPUT_BYTES)]
    max_output_bytes: usize,
}

impl Args {
    fn http(&self) -> HttpArgs {
        HttpArgs {
            host: self.host,
            port: self.port,
        }
    }
}

#[derive(Clone)]
struct AppState {
    policy: Arc<RwLock<LoadedPolicy>>,
    config: Arc<ServerConfig>,
    reload_dirty: Arc<AtomicBool>,
}

#[derive(Clone)]
struct ServerConfig {
    rule_inputs: Vec<PathBuf>,
    resolve_host_executables: bool,
    max_output_bytes: usize,
}

#[derive(Clone)]
struct LoadedPolicy {
    policy: Policy,
    rule_inputs: Vec<PathBuf>,
    rule_paths: Vec<PathBuf>,
    rule_count: usize,
    loaded_at_epoch_sec: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecRequest {
    command: Vec<String>,
    cwd: String,
    #[serde(default)]
    caller_env: BTreeMap<String, String>,
    #[serde(default)]
    env_overrides: BTreeMap<String, String>,
    #[serde(default)]
    // Accepted for backward compatibility but intentionally ignored.
    timeout_sec: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    ok: bool,
    service: &'static str,
    status: &'static str,
    rule_paths: Vec<String>,
    rule_count: usize,
    loaded_at_epoch_sec: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyReloadResponse {
    ok: bool,
    status: &'static str,
    rule_count: usize,
    loaded_at_epoch_sec: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckResponse {
    ok: bool,
    eligible: bool,
    classification: &'static str,
    reason: Option<String>,
    normalized_argv: Option<Vec<String>>,
    matched_rules: Vec<RuleMatch>,
    decision: Option<Decision>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunResponse {
    ok: bool,
    status: &'static str,
    classification: &'static str,
    reason: Option<String>,
    normalized_argv: Option<Vec<String>>,
    matched_rules: Vec<RuleMatch>,
    decision: Option<Decision>,
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: String,
    stderr: String,
    truncated_stdout: bool,
    truncated_stderr: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    ok: bool,
    error: String,
}

#[derive(Debug)]
struct NormalizedCommand {
    classification: &'static str,
    argv: Option<Vec<String>>,
    policy_argvs: Vec<Vec<String>>,
    reason: Option<String>,
}

#[derive(Debug)]
struct EvaluationResult {
    matched_rules: Vec<RuleMatch>,
    decision: Option<Decision>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing("codex_privileged_exec_http")?;

    let loaded = load_policy_state(&args.rules)?;
    let app = AppState {
        policy: Arc::new(RwLock::new(loaded)),
        config: Arc::new(ServerConfig {
            rule_inputs: args.rules.clone(),
            resolve_host_executables: args.resolve_host_executables,
            max_output_bytes: args.max_output_bytes,
        }),
        reload_dirty: Arc::new(AtomicBool::new(false)),
    };
    start_policy_watcher(app.clone())?;

    let router = Router::new()
        .route("/healthz", get(healthz))
        .route("/policy/check", post(policy_check))
        .route("/policy/reload", post(policy_reload))
        .route("/exec/run", post(exec_run))
        .with_state(app)
        .layer(TraceLayer::new_for_http());

    let bind = args.http();
    let listener = TcpListener::bind((bind.host, bind.port))
        .await
        .with_context(|| format!("bind {}:{}", bind.host, bind.port))?;
    info!("codex-privileged-exec-http listening on {}:{}", bind.host, bind.port);
    axum::serve(listener, router).await?;
    Ok(())
}

async fn healthz(State(app): State<AppState>) -> Json<HealthResponse> {
    let _ = maybe_reload_policy(&app).await;
    let loaded = app.policy.read().await;
    Json(HealthResponse {
        ok: true,
        service: "codex-privileged-exec-http",
        status: "ok",
        rule_paths: loaded
            .rule_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        rule_count: loaded.rule_count,
        loaded_at_epoch_sec: loaded.loaded_at_epoch_sec,
    })
}

async fn policy_reload(State(app): State<AppState>) -> Result<Json<PolicyReloadResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    reload_policy(&app).await.map_err(into_http_error)?;
    let loaded = app.policy.read().await;
    Ok(Json(PolicyReloadResponse {
        ok: true,
        status: "reloaded",
        rule_count: loaded.rule_count,
        loaded_at_epoch_sec: loaded.loaded_at_epoch_sec,
    }))
}

async fn policy_check(
    State(app): State<AppState>,
    Json(request): Json<ExecRequest>,
) -> Result<Json<CheckResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    maybe_reload_policy(&app).await.map_err(into_http_error)?;
    validate_request(&request).map_err(into_http_error)?;

    let normalized = normalize_command(&request.command);
    let evaluation = evaluate_normalized(&app, &normalized).await;
    Ok(Json(CheckResponse {
        ok: true,
        eligible: normalized.argv.is_some() && matches!(evaluation.decision, Some(Decision::Allow)),
        classification: normalized.classification,
        reason: normalized.reason,
        normalized_argv: normalized.argv,
        matched_rules: evaluation.matched_rules,
        decision: evaluation.decision,
    }))
}

async fn exec_run(
    State(app): State<AppState>,
    Json(request): Json<ExecRequest>,
) -> Result<Json<RunResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let _ = request.timeout_sec;
    maybe_reload_policy(&app).await.map_err(into_http_error)?;
    validate_request(&request).map_err(into_http_error)?;

    let normalized = normalize_command(&request.command);
    let evaluation = evaluate_normalized(&app, &normalized).await;
    let Some(argv) = normalized.argv.clone() else {
        return Ok(Json(RunResponse {
            ok: false,
            status: "rejected",
            classification: normalized.classification,
            reason: normalized.reason,
            normalized_argv: None,
            matched_rules: evaluation.matched_rules,
            decision: evaluation.decision,
            exit_code: None,
            timed_out: false,
            stdout: String::new(),
            stderr: String::new(),
            truncated_stdout: false,
            truncated_stderr: false,
        }));
    };

    match evaluation.decision {
        Some(Decision::Allow) => {}
        Some(Decision::Prompt) => {
            return Ok(Json(rejected_run_response(
                normalized,
                evaluation,
                "policy decision is prompt; privileged path requires explicit allow".to_string(),
            )));
        }
        Some(Decision::Forbidden) => {
            return Ok(Json(rejected_run_response(
                normalized,
                evaluation,
                "policy decision is forbidden".to_string(),
            )));
        }
        None => {
            return Ok(Json(rejected_run_response(
                normalized,
                evaluation,
                "no privileged execpolicy rule matched; fall back to local sandboxed execution".to_string(),
            )));
        }
    }

    let output = execute_argv(
        &argv,
        Path::new(&request.cwd),
        &request.caller_env,
        &request.env_overrides,
        app.config.max_output_bytes,
    )
    .await
    .map_err(into_http_error)?;

    Ok(Json(RunResponse {
        ok: output.exit_code == Some(0) && !output.timed_out,
        status: if output.timed_out { "timed_out" } else { "completed" },
        classification: normalized.classification,
        reason: normalized.reason,
        normalized_argv: Some(argv),
        matched_rules: evaluation.matched_rules,
        decision: evaluation.decision,
        exit_code: output.exit_code,
        timed_out: output.timed_out,
        stdout: output.stdout,
        stderr: output.stderr,
        truncated_stdout: output.truncated_stdout,
        truncated_stderr: output.truncated_stderr,
    }))
}

fn rejected_run_response(
    normalized: NormalizedCommand,
    evaluation: EvaluationResult,
    reason: String,
) -> RunResponse {
    RunResponse {
        ok: false,
        status: "rejected",
        classification: normalized.classification,
        reason: Some(reason),
        normalized_argv: normalized.argv,
        matched_rules: evaluation.matched_rules,
        decision: evaluation.decision,
        exit_code: None,
        timed_out: false,
        stdout: String::new(),
        stderr: String::new(),
        truncated_stdout: false,
        truncated_stderr: false,
    }
}

fn validate_request(request: &ExecRequest) -> Result<()> {
    if request.command.is_empty() {
        bail!("command must not be empty");
    }
    if request.cwd.trim().is_empty() {
        bail!("cwd must not be empty");
    }
    let cwd = Path::new(&request.cwd);
    if !cwd.is_absolute() {
        bail!("cwd must be an absolute path");
    }
    if !cwd.exists() {
        bail!("cwd does not exist: {}", cwd.display());
    }
    if !cwd.is_dir() {
        bail!("cwd is not a directory: {}", cwd.display());
    }
    for (key, value) in &request.env_overrides {
        if !is_valid_env_key(key) {
            bail!("invalid env override key: {key}");
        }
        if value.contains('\0') {
            bail!("env override values must not contain NUL bytes");
        }
    }
    for (key, value) in &request.caller_env {
        if !is_valid_env_key(key) {
            bail!("invalid caller env key: {key}");
        }
        if value.contains('\0') {
            bail!("caller env values must not contain NUL bytes");
        }
    }
    Ok(())
}

fn is_valid_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn filtered_caller_env(env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    env.iter()
        .filter(|(key, _)| is_allowed_caller_env_key(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn is_allowed_caller_env_key(key: &str) -> bool {
    matches!(
        key,
        "PATH" | "HOME" | "LANG" | "LC_ALL" | "LC_CTYPE" | "LC_MESSAGES" | "TERM" | "TMPDIR" | "SHELL"
    ) || key.starts_with("LC_")
        || key.starts_with("CODEX_")
        || key.starts_with("ROBDEX_")
}

fn normalize_command(command: &[String]) -> NormalizedCommand {
    if command.is_empty() {
        return NormalizedCommand {
            classification: "empty",
            argv: None,
            policy_argvs: Vec::new(),
            reason: Some("command was empty".to_string()),
        };
    }

    if extract_shell_command(command).is_none() {
        return NormalizedCommand {
            classification: "argv",
            argv: Some(command.to_vec()),
            policy_argvs: vec![command.to_vec()],
            reason: None,
        };
    }

    let Some(parsed_commands) = parse_shell_lc_plain_commands(command) else {
        return NormalizedCommand {
            classification: "shell_script",
            argv: None,
            policy_argvs: Vec::new(),
            reason: Some(
                "shell command uses advanced shell features and is not eligible for privileged argv execution"
                    .to_string(),
            ),
        };
    };
    if parsed_commands.len() != 1 {
        return NormalizedCommand {
            classification: "shell_sequence",
            argv: None,
            policy_argvs: parsed_commands,
            reason: Some(
                "only a single plain command is eligible for privileged execution; multi-command shell sequences fall back to local execution"
                    .to_string(),
            ),
        };
    }
    let argv = parsed_commands.into_iter().next().unwrap_or_default();
    NormalizedCommand {
        classification: "shell_plain_single",
        argv: Some(argv.clone()),
        policy_argvs: vec![argv],
        reason: None,
    }
}

async fn evaluate_normalized(app: &AppState, normalized: &NormalizedCommand) -> EvaluationResult {
    if normalized.policy_argvs.is_empty() {
        return EvaluationResult {
            matched_rules: Vec::new(),
            decision: None,
        };
    }
    let loaded = app.policy.read().await;
    let mut matched_rules = Vec::new();
    let mut decision = None;
    for argv in &normalized.policy_argvs {
        let mut per_command = loaded.policy.matches_for_command_with_options(
            argv,
            None,
            &MatchOptions {
                resolve_host_executables: app.config.resolve_host_executables,
            },
        );
        if let Some(per_decision) = per_command.iter().map(RuleMatch::decision).max() {
            decision = Some(decision.map_or(per_decision, |current: Decision| current.max(per_decision)));
        }
        matched_rules.append(&mut per_command);
    }
    EvaluationResult {
        matched_rules,
        decision,
    }
}

async fn maybe_reload_policy(app: &AppState) -> Result<()> {
    let should_reload = {
        app.reload_dirty.load(Ordering::SeqCst)
    };
    if should_reload {
        reload_policy(app).await?;
    }
    Ok(())
}

async fn reload_policy(app: &AppState) -> Result<()> {
    let loaded = load_policy_state(&app.config.rule_inputs)?;
    let mut state = app.policy.write().await;
    *state = loaded;
    app.reload_dirty.store(false, Ordering::SeqCst);
    Ok(())
}

fn load_policy_state(rule_inputs: &[PathBuf]) -> Result<LoadedPolicy> {
    let rule_paths = expand_rule_inputs(rule_inputs)?;
    let policy = load_policies(&rule_paths)?;
    let rule_count = policy.rules().iter_all().map(|(_, rules)| rules.len()).sum();
    Ok(LoadedPolicy {
        policy,
        rule_inputs: rule_inputs.to_vec(),
        rule_paths,
        rule_count,
        loaded_at_epoch_sec: unix_now(),
    })
}

fn expand_rule_inputs(rule_inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut resolved = Vec::new();
    for input in rule_inputs {
        let metadata = std::fs::metadata(input)
            .with_context(|| format!("stat rules input {}", input.display()))?;
        if metadata.is_dir() {
            let mut entries = std::fs::read_dir(input)
                .with_context(|| format!("read rules directory {}", input.display()))?
                .filter_map(|entry| entry.ok().map(|value| value.path()))
                .filter(|path| path.is_file() && is_rule_file(path))
                .collect::<Vec<_>>();
            entries.sort();
            resolved.extend(entries);
        } else {
            resolved.push(input.clone());
        }
    }
    if resolved.is_empty() {
        bail!("no execpolicy rule files were found in configured inputs");
    }
    Ok(resolved)
}

fn is_rule_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("rules") | Some("codexpolicy")
    )
}

fn start_policy_watcher(app: AppState) -> Result<()> {
    let inputs = app.config.rule_inputs.clone();
    let dirty = app.reload_dirty.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(
        move |event| {
            let _ = tx.send(event);
        },
        NotifyConfig::default(),
    )
    .context("create policy watcher")?;

    for input in &inputs {
        let mode = if input.is_dir() {
            RecursiveMode::NonRecursive
        } else {
            RecursiveMode::NonRecursive
        };
        watcher
            .watch(input, mode)
            .with_context(|| format!("watch policy input {}", input.display()))?;
    }

    tokio::spawn(async move {
        let _watcher = watcher;
        while let Some(event) = rx.recv().await {
            match event {
                Ok(event) => {
                    tracing::info!("policy watcher noticed change: {:?}", event.paths);
                    dirty.store(true, Ordering::SeqCst);
                }
                Err(error) => {
                    tracing::warn!("policy watcher error: {error}");
                }
            }
        }
    });

    Ok(())
}

struct CapturedOutput {
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: String,
    stderr: String,
    truncated_stdout: bool,
    truncated_stderr: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BridgeLiveProcessRecord {
    pid: i64,
    process_group_id: Option<i64>,
    command: String,
    cwd: String,
    started_at: u64,
}

async fn execute_argv(
    argv: &[String],
    cwd: &Path,
    caller_env: &BTreeMap<String, String>,
    env_overrides: &BTreeMap<String, String>,
    max_output_bytes: usize,
) -> Result<CapturedOutput> {
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    command.current_dir(cwd);
    command.kill_on_drop(true);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    for (key, value) in filtered_caller_env(caller_env) {
        command.env(key, value);
    }
    for (key, value) in env_overrides {
        command.env(key, value);
    }

    let mut child = command
        .spawn()
        .with_context(|| format!("spawn {}", argv.join(" ")))?;

    let child_pid = child
        .id()
        .ok_or_else(|| anyhow!("spawned process missing pid"))? as i64;
    let process_group_id = process_group_id_for_pid(child_pid);
    let process_id = child_pid.to_string();
    notify_bridge_process_started(
        caller_env,
        &process_id,
        &BridgeLiveProcessRecord {
            pid: child_pid,
            process_group_id,
            command: shell_join(argv),
            cwd: cwd.display().to_string(),
            started_at: unix_now(),
        },
    )
    .await;

    let stdout = child.stdout.take().ok_or_else(|| anyhow!("child stdout was not captured"))?;
    let stderr = child.stderr.take().ok_or_else(|| anyhow!("child stderr was not captured"))?;

    let stdout_task = tokio::spawn(read_stream_capped(stdout, max_output_bytes));
    let stderr_task = tokio::spawn(read_stream_capped(stderr, max_output_bytes));

    let status = child
        .wait()
        .await
        .with_context(|| format!("wait for {}", argv.join(" ")))?;

    let (stdout, truncated_stdout) = stdout_task.await.unwrap_or_default();
    let (stderr, truncated_stderr) = stderr_task.await.unwrap_or_default();
    notify_bridge_process_completed(caller_env, &process_id).await;
    Ok(CapturedOutput {
        exit_code: status.code(),
        timed_out: false,
        stdout,
        stderr,
        truncated_stdout,
        truncated_stderr,
    })
}

async fn read_stream_capped<R>(mut reader: R, max_bytes: usize) -> (String, bool)
where
    R: AsyncReadExt + Unpin,
{
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;

    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => {
                let remaining = max_bytes.saturating_sub(buf.len());
                if remaining == 0 {
                    truncated = true;
                    continue;
                }
                if n > remaining {
                    buf.extend_from_slice(&chunk[..remaining]);
                    truncated = true;
                } else {
                    buf.extend_from_slice(&chunk[..n]);
                }
            }
            Err(_) => break,
        }
    }

    (String::from_utf8_lossy(&buf).to_string(), truncated)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|part| shell_escape(part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_escape(part: &str) -> String {
    if part.is_empty() {
        return "''".to_string();
    }
    if part
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.' | ':' | '='))
    {
        return part.to_string();
    }
    format!("'{}'", part.replace('\'', "'\"'\"'"))
}

fn process_group_id_for_pid(pid: i64) -> Option<i64> {
    #[cfg(unix)]
    {
        let pgid = unsafe { libc::getpgid(pid as libc::pid_t) };
        if pgid > 0 {
            return Some(pgid as i64);
        }
    }
    None
}

async fn notify_bridge_process_started(
    caller_env: &BTreeMap<String, String>,
    process_id: &str,
    process: &BridgeLiveProcessRecord,
) {
    let Some(thread_id) = caller_env.get("CODEX_THREAD_ID").map(|value| value.trim()).filter(|value| !value.is_empty()) else {
        return;
    };
    let url = format!("http://127.0.0.1:42080/threads/{thread_id}/processes/register");
    let _ = Client::new().post(url).json(process).send().await;
    let _ = process_id;
}

async fn notify_bridge_process_completed(
    caller_env: &BTreeMap<String, String>,
    process_id: &str,
) {
    let Some(thread_id) = caller_env.get("CODEX_THREAD_ID").map(|value| value.trim()).filter(|value| !value.is_empty()) else {
        return;
    };
    let url = format!("http://127.0.0.1:42080/threads/{thread_id}/processes/{process_id}/complete");
    let _ = Client::new().post(url).send().await;
}

fn into_http_error(error: anyhow::Error) -> (axum::http::StatusCode, Json<ErrorResponse>) {
    (
        axum::http::StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            ok: false,
            error: error.to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_rules(root: &Path, body: &str) -> PathBuf {
        let path = root.join("policy.codexpolicy");
        std::fs::write(&path, body).expect("write policy");
        path
    }

    #[test]
    fn direct_argv_is_privileged_candidate() {
        let normalized = normalize_command(&["xcodebuild".to_string(), "-version".to_string()]);
        assert_eq!(normalized.classification, "argv");
        assert_eq!(
            normalized.argv,
            Some(vec!["xcodebuild".to_string(), "-version".to_string()])
        );
    }

    #[test]
    fn shell_plain_single_command_is_reduced_to_argv() {
        let normalized = normalize_command(&[
            "bash".to_string(),
            "-lc".to_string(),
            "xcodebuild -version".to_string(),
        ]);
        assert_eq!(normalized.classification, "shell_plain_single");
        assert_eq!(
            normalized.argv,
            Some(vec!["xcodebuild".to_string(), "-version".to_string()])
        );
    }

    #[test]
    fn shell_sequence_is_not_privileged() {
        let normalized = normalize_command(&[
            "bash".to_string(),
            "-lc".to_string(),
            "xcodebuild -version && flutter --version".to_string(),
        ]);
        assert_eq!(normalized.classification, "shell_sequence");
        assert!(normalized.argv.is_none());
    }

    #[test]
    fn shell_redirect_is_not_privileged() {
        let normalized = normalize_command(&[
            "bash".to_string(),
            "-lc".to_string(),
            "xcodebuild -version > out.log".to_string(),
        ]);
        assert_eq!(normalized.classification, "shell_script");
        assert!(normalized.argv.is_none());
    }

    #[test]
    fn shell_wildcard_token_is_reduced_to_plain_argv() {
        let normalized = normalize_command(&[
            "bash".to_string(),
            "-lc".to_string(),
            "rg *.rs".to_string(),
        ]);
        assert_eq!(normalized.classification, "shell_plain_single");
        assert_eq!(
            normalized.argv,
            Some(vec!["rg".to_string(), "*.rs".to_string()])
        );
    }

    #[test]
    fn shell_quoted_commit_message_with_brackets_is_reduced_to_plain_argv() {
        let normalized = normalize_command(&[
            "bash".to_string(),
            "-lc".to_string(),
            "git-commit /tmp/worktree \"[QBO-SLICE 4A] Add inbound customer sync MVP\"".to_string(),
        ]);
        assert_eq!(normalized.classification, "shell_plain_single");
        assert_eq!(
            normalized.argv,
            Some(vec![
                "git-commit".to_string(),
                "/tmp/worktree".to_string(),
                "[QBO-SLICE 4A] Add inbound customer sync MVP".to_string(),
            ])
        );
    }

    #[test]
    fn shell_env_flag_value_is_reduced_to_plain_argv() {
        let normalized = normalize_command(&[
            "bash".to_string(),
            "-lc".to_string(),
            "api-integration-harness up /tmp/worktree --env QBO_BASE_URL_OVERRIDE=http://host.docker.internal:58081/v3/company/".to_string(),
        ]);
        assert_eq!(normalized.classification, "shell_plain_single");
        assert_eq!(
            normalized.argv,
            Some(vec![
                "api-integration-harness".to_string(),
                "up".to_string(),
                "/tmp/worktree".to_string(),
                "--env".to_string(),
                "QBO_BASE_URL_OVERRIDE=http://host.docker.internal:58081/v3/company/".to_string(),
            ])
        );
    }

    #[test]
    fn shell_inline_env_assignment_remains_ineligible_without_extra_token_filter() {
        let normalized = normalize_command(&[
            "bash".to_string(),
            "-lc".to_string(),
            "QBO_BASE_URL_OVERRIDE=http://host.docker.internal:58081/v3/company/ api-integration-harness up /tmp/worktree".to_string(),
        ]);
        assert!(normalized.argv.is_none());
        assert_eq!(normalized.classification, "shell_script");
    }

    #[test]
    fn policy_matches_allow_rule() {
        let temp = tempdir().expect("tempdir");
        let rules = write_rules(
            temp.path(),
            r#"
prefix_rule(
    pattern = ["xcodebuild"],
    decision = "allow",
    match = [["xcodebuild", "-version"]],
)
"#,
        );
        let loaded = load_policy_state(&[rules]).expect("load policy");
        let matches = loaded.policy.matches_for_command_with_options(
            &["xcodebuild".to_string(), "-version".to_string()],
            None,
            &MatchOptions {
                resolve_host_executables: true,
            },
        );
        assert!(!matches.is_empty());
        assert_eq!(matches[0].decision(), Decision::Allow);
    }

    #[test]
    fn policy_matches_forbidden_rule() {
        let temp = tempdir().expect("tempdir");
        let rules = write_rules(
            temp.path(),
            r#"
prefix_rule(
    pattern = ["rm"],
    decision = "forbidden",
    match = [["rm", "-rf", "/tmp/foo"]],
)
"#,
        );
        let loaded = load_policy_state(&[rules]).expect("load policy");
        let matches = loaded.policy.matches_for_command_with_options(
            &["rm".to_string(), "-rf".to_string(), "/tmp/foo".to_string()],
            None,
            &MatchOptions::default(),
        );
        assert!(!matches.is_empty());
        assert_eq!(matches[0].decision(), Decision::Forbidden);
    }

    #[test]
    fn policy_no_match_returns_empty_matches() {
        let temp = tempdir().expect("tempdir");
        let rules = write_rules(
            temp.path(),
            r#"
prefix_rule(
    pattern = ["xcodebuild"],
    decision = "allow",
    match = [["xcodebuild", "-version"]],
)
"#,
        );
        let loaded = load_policy_state(&[rules]).expect("load policy");
        let matches = loaded.policy.matches_for_command_with_options(
            &["flutter".to_string(), "--version".to_string()],
            None,
            &MatchOptions::default(),
        );
        assert!(matches.is_empty());
    }

    #[test]
    fn filters_caller_env_to_safe_subset() {
        let env = BTreeMap::from([
            ("PATH".to_string(), "/opt/homebrew/bin".to_string()),
            ("HOME".to_string(), "/Users/test".to_string()),
            ("TMPDIR".to_string(), "/tmp/test".to_string()),
            ("LC_ALL".to_string(), "en_US.UTF-8".to_string()),
            ("ROBDEX_BRIDGE_BASE_URL".to_string(), "http://127.0.0.1:42080".to_string()),
            ("CODEX_THREAD_ID".to_string(), "thread-1".to_string()),
            ("PYTHONPATH".to_string(), "/tmp/evil".to_string()),
            ("DYLD_INSERT_LIBRARIES".to_string(), "/tmp/inject.dylib".to_string()),
        ]);

        let filtered = filtered_caller_env(&env);
        assert_eq!(filtered.get("PATH").map(String::as_str), Some("/opt/homebrew/bin"));
        assert_eq!(filtered.get("HOME").map(String::as_str), Some("/Users/test"));
        assert_eq!(filtered.get("TMPDIR").map(String::as_str), Some("/tmp/test"));
        assert_eq!(filtered.get("LC_ALL").map(String::as_str), Some("en_US.UTF-8"));
        assert_eq!(
            filtered.get("ROBDEX_BRIDGE_BASE_URL").map(String::as_str),
            Some("http://127.0.0.1:42080")
        );
        assert_eq!(filtered.get("CODEX_THREAD_ID").map(String::as_str), Some("thread-1"));
        assert!(!filtered.contains_key("PYTHONPATH"));
        assert!(!filtered.contains_key("DYLD_INSERT_LIBRARIES"));
    }

    #[test]
    fn expands_rule_inputs_from_directory_in_sorted_order() {
        let temp = tempdir().expect("tempdir");
        std::fs::write(temp.path().join("20-b.rules"), "prefix_rule(pattern=[\"b\"])\n").expect("write b");
        std::fs::write(temp.path().join("10-a.codexpolicy"), "prefix_rule(pattern=[\"a\"])\n").expect("write a");
        std::fs::write(temp.path().join("ignored.txt"), "nope").expect("write ignored");

        let expanded = expand_rule_inputs(&[temp.path().to_path_buf()]).expect("expand");
        let names = expanded
            .iter()
            .map(|path| path.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["10-a.codexpolicy".to_string(), "20-b.rules".to_string()]);
    }
}
