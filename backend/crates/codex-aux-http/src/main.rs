use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use anyhow::{Context, Result, anyhow, bail};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use clap::Parser;
use codex_backend_core::{HealthResponse, HttpArgs, init_tracing};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, env = "CODEX_AUX_BIND", default_value = "127.0.0.1")]
    host: std::net::IpAddr,

    #[arg(long, env = "CODEX_AUX_PORT", default_value_t = 8871)]
    port: u16,
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
    command_parser: Arc<CommandParserRuntime>,
    request_review: Arc<RequestReviewRuntime>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandParserRequest {
    command: Vec<String>,
    output: String,
    include_warnings: Option<bool>,
    additional_request: Option<String>,
    cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RequestReviewRequest {
    repo_path: String,
    title: String,
    profile: Option<String>,
    uncommitted: bool,
    commit_ref: Option<String>,
    prompt: Option<String>,
}

#[derive(Debug, Serialize)]
struct RequestReviewResponse {
    result: RequestReviewResult,
}

#[derive(Debug, Serialize)]
struct RequestReviewResult {
    message: String,
    exit_code: i32,
}

#[derive(Debug, Clone)]
struct CommandParserRuntime {
    skill_env_file: PathBuf,
    config_file: PathBuf,
    role_file: PathBuf,
    auth_file: PathBuf,
    rule_file: PathBuf,
    usage_log_file: PathBuf,
    default_profile: String,
}

#[derive(Debug, Clone)]
struct RequestReviewRuntime {
    codex_home: PathBuf,
    config_file: PathBuf,
    default_profile: String,
}

impl CommandParserRuntime {
    fn from_environment() -> Self {
        let codex_home = PathBuf::from(
            env::var("CODEX_HOME").unwrap_or_else(|_| format!("{}/.codex", env::var("HOME").unwrap_or_default())),
        );
        let scripts_dir = codex_home.join("scripts");
        Self {
            skill_env_file: env_path("COMMAND_PARSER_SKILL_ENV_FILE", scripts_dir.join("command-parser.env")),
            config_file: env_path("COMMAND_PARSER_CODEX_CONFIG_FILE", codex_home.join("config.toml")),
            role_file: codex_home.join("roles/command-parser.md"),
            auth_file: codex_home.join("auth.json"),
            rule_file: env_path("COMMAND_PARSER_RULE_FILE", scripts_dir.join("command-parser.rule")),
            usage_log_file: env_path("COMMAND_PARSER_USAGE_LOG_FILE", codex_home.join("command-parser-usage.log")),
            default_profile: "command-parser".to_string(),
        }
    }

    fn handle_parse(&self, request: CommandParserRequest) -> Result<String> {
        if request.command.is_empty() {
            bail!("command is required");
        }

        let cwd = resolve_cwd(request.cwd.as_deref())?;
        self.append_usage_log(&request.command, &cwd)?;
        if let Some(message) = self.check_command_policy(&request.command, &cwd)? {
            return Ok(format!("Command blocked by command-parser.rule: {message}"));
        }

        let profile = self.load_profile_from_env()?;
        self.assert_profile_exists(&profile)?;

        let temp_dir = tempfile::Builder::new()
            .prefix("codex-aux-command-parser.")
            .tempdir()
            .context("failed to create parser temp dir")?;

        let temp_path = temp_dir.path();
        fs::write(temp_path.join("output.log"), request.output).context("failed to write output.log")?;
        fs::write(
            temp_path.join("command.txt"),
            format!("{}\n", shell_join(&request.command)),
        )
        .context("failed to write command.txt")?;

        let staged_home = self.stage_parser_codex_home(temp_path)?;
        let response_log = temp_path.join("response.log");
        let prompt = format!(
            "Parse ./output.log from this raw command:\n{}\n\nInclude warnings: {}\n\nAdditional request: {}\n\nRead the provided files and return only the extraction result.",
            shell_join(&request.command),
            if request.include_warnings.unwrap_or(false) { "yes" } else { "no" },
            request.additional_request.as_deref().filter(|value| !value.trim().is_empty()).unwrap_or("<none>"),
        );

        let mut cmd = Command::new("codex");
        cmd.arg("exec")
            .arg("--skip-git-repo-check")
            .arg("--ephemeral")
            .arg("-s")
            .arg("workspace-write")
            .arg("-C")
            .arg(temp_path)
            .arg("-p")
            .arg(profile)
            .arg("-c")
            .arg("web_search=\"disabled\"")
            .arg("-c")
            .arg("features.unified_exec=false")
            .arg("-c")
            .arg("features.multi_agent=false")
            .arg("-c")
            .arg("features.steer=false")
            .arg("-c")
            .arg("features.skills=false")
            .arg("-o")
            .arg(&response_log)
            .arg(prompt)
            .env("CODEX_HOME", &staged_home)
            .env("PWD", temp_path)
            .current_dir(temp_path);

        let output = cmd.output().context("failed to run codex exec for command-parser")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("exit {}", output.status)
            };
            bail!("codex exec parser failed: {detail}");
        }

        let response = fs::read_to_string(&response_log).context("missing parser response")?;
        let trimmed = response.trim();
        if trimmed.is_empty() {
            bail!("missing parser response");
        }
        Ok(trimmed.to_string())
    }

    fn load_profile_from_env(&self) -> Result<String> {
        if !self.skill_env_file.exists() {
            return Ok(self.default_profile.clone());
        }
        let raw = fs::read_to_string(&self.skill_env_file)
            .with_context(|| format!("failed to read {}", self.skill_env_file.display()))?;
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            if key.trim() != "COMMAND_PARSER_PROFILE" {
                continue;
            }
            let parsed = parse_dotenv_value(value);
            if !parsed.is_empty() {
                return Ok(parsed);
            }
        }
        Ok(self.default_profile.clone())
    }

    fn assert_profile_exists(&self, profile: &str) -> Result<()> {
        let raw = fs::read_to_string(&self.config_file)
            .with_context(|| format!("missing config file: {}", self.config_file.display()))?;
        if !raw.contains(&format!("[profiles.{profile}]")) {
            bail!("Profile '{profile}' not found in {}", self.config_file.display());
        }
        Ok(())
    }

    fn stage_parser_codex_home(&self, temp_path: &Path) -> Result<PathBuf> {
        if !self.config_file.exists() {
            bail!("Missing Codex config for parser profile: {}", self.config_file.display());
        }
        if !self.role_file.exists() {
            bail!("Missing command-parser role instructions: {}", self.role_file.display());
        }

        let staged_home = temp_path.join("codex-home");
        let staged_role_file = staged_home.join("roles/command-parser.md");
        fs::create_dir_all(
            staged_role_file
                .parent()
                .ok_or_else(|| anyhow!("invalid staged role path"))?,
        )
        .context("failed to create staged codex-home")?;
        fs::copy(&self.config_file, staged_home.join("config.toml")).context("failed to stage config.toml")?;
        fs::copy(&self.role_file, &staged_role_file).context("failed to stage command-parser role")?;
        if self.auth_file.exists() {
            fs::copy(&self.auth_file, staged_home.join("auth.json")).context("failed to stage auth.json")?;
        }
        Ok(staged_home)
    }

    fn append_usage_log(&self, command: &[String], cwd: &Path) -> Result<()> {
        let timestamp = chrono_like_timestamp();
        let line = format!("{timestamp} | {} | cwd={}\n", shell_join(command), cwd.display());
        if let Some(parent) = self.usage_log_file.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        use std::io::Write as _;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.usage_log_file)
            .with_context(|| format!("failed to open {}", self.usage_log_file.display()))?;
        file.write_all(line.as_bytes())
            .with_context(|| format!("failed to append {}", self.usage_log_file.display()))?;
        Ok(())
    }

    fn check_command_policy(&self, command: &[String], cwd: &Path) -> Result<Option<String>> {
        if !self.rule_file.exists() {
            return Ok(None);
        }

        let output = Command::new("codex")
            .arg("execpolicy")
            .arg("check")
            .arg("--rules")
            .arg(&self.rule_file)
            .arg("--")
            .args(command)
            .current_dir(cwd)
            .output()
            .context("failed to run codex execpolicy check")?;

        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let fallback = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let rendered = if !detail.is_empty() { detail } else { fallback };
            bail!(
                "execpolicy check failed for command-parser.rule at {}: {}",
                self.rule_file.display(),
                if rendered.is_empty() { format!("exit {}", output.status) } else { rendered }
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            return Ok(None);
        }

        let payload: serde_json::Value =
            serde_json::from_str(&stdout).with_context(|| format!("execpolicy output was not valid JSON: {stdout}"))?;
        Ok(execpolicy_forbidden_message(&payload))
    }
}

impl RequestReviewRuntime {
    fn from_environment() -> Self {
        let codex_home = PathBuf::from(
            env::var("CODEX_HOME").unwrap_or_else(|_| format!("{}/.codex", env::var("HOME").unwrap_or_default())),
        );
        Self {
            config_file: env_path("REQUEST_REVIEW_CODEX_CONFIG_FILE", codex_home.join("config.toml")),
            codex_home,
            default_profile: "local-review".to_string(),
        }
    }

    fn handle_review(&self, request: RequestReviewRequest) -> RequestReviewResult {
        match self.run_review(request) {
            Ok(result) => result,
            Err(error) => RequestReviewResult {
                message: error.to_string(),
                exit_code: 1,
            },
        }
    }

    fn run_review(&self, request: RequestReviewRequest) -> Result<RequestReviewResult> {
        let repo_path = resolve_repo_path(&request.repo_path)?;
        let title = request.title.trim();
        if title.is_empty() {
            bail!("title is required");
        }

        let profile = request
            .profile
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.default_profile)
            .to_string();
        self.assert_profile_exists(&profile)?;

        let commit_ref = request
            .commit_ref
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        if !request.uncommitted && commit_ref.is_none() {
            bail!("commit_ref is required when uncommitted is false");
        }

        let output_file = tempfile::NamedTempFile::new()
            .context("failed to create temporary review output file")?;
        let output_path = output_file.path().to_path_buf();

        let mut cmd = Command::new("codex");
        cmd.arg("exec")
            .arg("-C")
            .arg(&repo_path)
            .arg("-s")
            .arg("read-only")
            .arg("-p")
            .arg(&profile)
            .arg("review")
            .arg("--output-last-message")
            .arg(&output_path)
            .arg("--title")
            .arg(title)
            .env("CODEX_HOME", &self.codex_home)
            .current_dir(&repo_path);

        if request.uncommitted {
            cmd.arg("--uncommitted");
        } else if let Some(commit_ref) = &commit_ref {
            cmd.arg("--commit").arg(commit_ref);
        }

        if let Some(prompt) = request.prompt.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
            cmd.arg(prompt);
        }

        let output = cmd.output().context("failed to run codex exec review")?;
        let output_file_text = fs::read_to_string(&output_path).unwrap_or_default();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let message = extract_review_message(&output_file_text, &stdout, &stderr).unwrap_or_else(|| {
            format!(
                "request-review failed with exit code {}",
                output.status.code().unwrap_or(1)
            )
        });

        if output.status.success() && !message.is_empty() {
            fs::write(repo_path.join("review.log"), format!("{message}\n"))
                .context("failed to write review.log")?;
        }

        Ok(RequestReviewResult {
            message,
            exit_code: output.status.code().unwrap_or(1),
        })
    }

    fn assert_profile_exists(&self, profile: &str) -> Result<()> {
        let raw = fs::read_to_string(&self.config_file)
            .with_context(|| format!("missing config file: {}", self.config_file.display()))?;
        if !raw.contains(&format!("[profiles.{profile}]")) {
            bail!("profile '{profile}' not found in {}", self.config_file.display());
        }
        Ok(())
    }
}

fn env_path(key: &str, fallback: PathBuf) -> PathBuf {
    env::var(key).map(PathBuf::from).unwrap_or(fallback)
}

fn resolve_cwd(raw: Option<&str>) -> Result<PathBuf> {
    let path = raw
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or(env::current_dir().context("failed to resolve current directory")?);
    Ok(path
        .canonicalize()
        .or_else(|_| Ok::<PathBuf, anyhow::Error>(path))
        .context("failed to resolve cwd")?)
}

fn resolve_repo_path(raw: &str) -> Result<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("repo_path is required");
    }
    let path = PathBuf::from(trimmed);
    let canonical = path
        .canonicalize()
        .with_context(|| format!("failed to resolve repo_path: {trimmed}"))?;
    if !canonical.is_dir() {
        bail!("repo_path is not a directory: {}", canonical.display());
    }
    Ok(canonical)
}

fn extract_review_message(output_file_text: &str, stdout: &str, stderr: &str) -> Option<String> {
    for candidate in [output_file_text, stdout, stderr] {
        if let Some(message) = extract_last_codex_message(candidate) {
            return Some(message);
        }
    }

    for candidate in [output_file_text, stderr, stdout] {
        let trimmed = candidate.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}

fn extract_last_codex_message(raw: &str) -> Option<String> {
    let normalized = raw.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.lines().collect();

    for idx in (0..lines.len()).rev() {
        if lines[idx].trim() != "codex" {
            continue;
        }

        let mut collected = Vec::new();
        for line in &lines[idx + 1..] {
            let trimmed = line.trim();
            if trimmed.starts_with("Warning: no last agent message")
                || trimmed.starts_with("hook ")
                || trimmed == "tokens used"
            {
                break;
            }
            collected.push(*line);
        }

        let message = collected.join("\n");
        let trimmed = message.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}

fn parse_dotenv_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 {
        let first = trimmed.as_bytes()[0] as char;
        let last = trimmed.as_bytes()[trimmed.len() - 1] as char;
        if first == last && (first == '"' || first == '\'') {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

fn shell_join(command: &[String]) -> String {
    command
        .iter()
        .map(|part| shell_quote(part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '/' | '.' | ':' | '-'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', r"'\''"))
    }
}

fn execpolicy_forbidden_message(payload: &serde_json::Value) -> Option<String> {
    let decision = payload.get("decision").and_then(|value| value.as_str()).unwrap_or("").trim().to_ascii_lowercase();
    if decision == "forbidden" {
        if let Some(message) = forbidden_message_from_matches(payload.get("matchedRules")) {
            return Some(message);
        }
        return Some("Command is forbidden by command-parser.rule.".to_string());
    }
    forbidden_message_from_matches(payload.get("matchedRules"))
}

fn forbidden_message_from_matches(value: Option<&serde_json::Value>) -> Option<String> {
    let rules = value?.as_array()?;
    for entry in rules {
        let prefix = entry.get("prefixRuleMatch")?.as_object()?;
        if prefix
            .get("decision")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("forbidden")
        {
            let justification = prefix
                .get("justification")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if !justification.is_empty() {
                return Some(justification);
            }
            return Some("Command is forbidden by command-parser.rule.".to_string());
        }
    }
    None
}

fn chrono_like_timestamp() -> String {
    let output = Command::new("date")
        .arg("-u")
        .arg("+%Y-%m-%dT%H:%M:%SZ")
        .output();
    match output {
        Ok(result) if result.status.success() => String::from_utf8_lossy(&result.stdout).trim().to_string(),
        _ => "unknown-time".to_string(),
    }
}

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "codex-aux-http",
        status: "ok",
        phase: "command-parser",
    })
}

async fn parse_command(
    State(state): State<AppState>,
    Json(request): Json<CommandParserRequest>,
) -> impl IntoResponse {
    match tokio::task::spawn_blocking(move || state.command_parser.handle_parse(request)).await {
        Ok(Ok(message)) => (StatusCode::OK, message),
        Ok(Err(error)) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("command-parser worker join error: {error}"),
        ),
    }
}

async fn run_request_review(
    State(state): State<AppState>,
    Json(request): Json<RequestReviewRequest>,
) -> impl IntoResponse {
    let result = match tokio::task::spawn_blocking(move || state.request_review.handle_review(request)).await {
        Ok(result) => result,
        Err(error) => RequestReviewResult {
            message: format!("request-review worker join error: {error}"),
            exit_code: 1,
        },
    };
    Json(RequestReviewResponse { result })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing("codex_aux_http")?;

    let state = AppState {
        command_parser: Arc::new(CommandParserRuntime::from_environment()),
        request_review: Arc::new(RequestReviewRuntime::from_environment()),
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/command-parser/parse", post(parse_command))
        .route("/v1/request-review/run", post(run_request_review))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(args.http().socket_addr()).await?;
    info!("codex-aux-http scaffold listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
