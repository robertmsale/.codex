use std::{
    env, fs,
    path::PathBuf,
    process::Command,
};

use anyhow::{Context, Result, bail};
use axum::{
    Json, Router,
    extract::State,
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
    request_review: RequestReviewRuntime,
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
struct RequestReviewRuntime {
    codex_home: PathBuf,
    config_file: PathBuf,
    default_profile: String,
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
            .arg("-o")
            .arg(&output_path)
            .arg(request.prompt.unwrap_or_else(|| {
                match (request.uncommitted, commit_ref.as_deref()) {
                    (true, _) => format!("Review the uncommitted changes for: {title}"),
                    (false, Some(reference)) => format!("Review commit {reference} for: {title}"),
                    (false, None) => format!("Review the current repository state for: {title}"),
                }
            }))
            .env("CODEX_HOME", &self.codex_home)
            .current_dir(&repo_path);

        if let Some(reference) = commit_ref {
            cmd.arg("-c").arg(format!("review_commit_ref={reference}"));
        }

        let output = cmd.output().context("failed to run codex exec for request-review")?;
        let message = fs::read_to_string(&output_path).unwrap_or_default();
        let exit_code = output.status.code().unwrap_or(1);
        let fallback = if message.trim().is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.is_empty() {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            } else {
                stderr
            }
        } else {
            message
        };

        Ok(RequestReviewResult {
            message: fallback.trim().to_string(),
            exit_code,
        })
    }

    fn assert_profile_exists(&self, profile: &str) -> Result<()> {
        let raw = fs::read_to_string(&self.config_file)
            .with_context(|| format!("missing config file: {}", self.config_file.display()))?;
        if !raw.contains(&format!("[profiles.{profile}]")) {
            bail!("Profile '{profile}' not found in {}", self.config_file.display());
        }
        Ok(())
    }
}

fn env_path(key: &str, default: PathBuf) -> PathBuf {
    env::var_os(key).map(PathBuf::from).unwrap_or(default)
}

fn resolve_repo_path(raw: &str) -> Result<PathBuf> {
    let path = PathBuf::from(raw.trim());
    if path.as_os_str().is_empty() {
        bail!("repo_path is required");
    }
    let path = resolve_cwd(Some(path.to_string_lossy().as_ref()))?;
    if !path.exists() {
        bail!("repo_path does not exist: {}", path.display());
    }
    Ok(path)
}

fn resolve_cwd(raw: Option<&str>) -> Result<PathBuf> {
    let path = match raw.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => PathBuf::from(value),
        None => env::current_dir().context("failed to resolve current directory")?,
    };
    let path = expand_home(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        env::current_dir()
            .context("failed to resolve current directory")
            .map(|cwd| cwd.join(path))
    }
}

fn expand_home(path: PathBuf) -> PathBuf {
    let raw = path.to_string_lossy();
    if raw == "~" {
        return home_dir();
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    path
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "codex-aux-http",
        status: "ok",
        phase: "request-review",
    })
}

async fn run_request_review(
    State(state): State<AppState>,
    Json(request): Json<RequestReviewRequest>,
) -> Json<RequestReviewResponse> {
    let runtime = state.request_review.clone();
    let result = match tokio::task::spawn_blocking(move || runtime.handle_review(request)).await {
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
        request_review: RequestReviewRuntime::from_environment(),
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/request-review/run", post(run_request_review))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(args.http().socket_addr()).await?;
    info!("codex-aux-http listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
