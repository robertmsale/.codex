use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    routing::{get, post},
};
use clap::Parser;
use codex_backend_core::{HttpArgs, init_tracing};
use rmcp::ErrorData as McpError;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, JsonObject, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    net::TcpListener,
    sync::{Mutex, Notify},
    time::sleep,
};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

const SLOT_COUNT: usize = 1000;
const DEFAULT_JOB_DIR: &str = "/tmp/codex-command-jobs";
const DEFAULT_POLL_INTERVAL_MS: u64 = 500;
const WAIT_TOOL_NAME: &str = "command_execution_wait";

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, env = "CODEX_COMMAND_EXECUTION_BIND", default_value = "127.0.0.1")]
    host: std::net::IpAddr,

    #[arg(long, env = "CODEX_COMMAND_EXECUTION_PORT", default_value_t = 8772)]
    port: u16,

    #[arg(long, env = "CODEX_COMMAND_EXECUTION_JOB_DIR", default_value = DEFAULT_JOB_DIR)]
    job_dir: PathBuf,

    #[arg(long, env = "CODEX_COMMAND_EXECUTION_POLL_INTERVAL_MS", default_value_t = DEFAULT_POLL_INTERVAL_MS)]
    poll_interval_ms: u64,
}

impl Args {
    fn http(&self) -> HttpArgs {
        HttpArgs {
            host: self.host,
            port: self.port,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ReservationInfo {
    job_id: u16,
    job_file: Option<String>,
    cwd: Option<String>,
    started_at: Option<String>,
    launcher_pid: Option<u32>,
    cmd_pid: Option<u32>,
    reserved_epoch_ms: u128,
}

#[derive(Debug)]
struct SlotState {
    reservation: Option<ReservationInfo>,
    notify: Arc<Notify>,
}

impl Default for SlotState {
    fn default() -> Self {
        Self {
            reservation: None,
            notify: Arc::new(Notify::new()),
        }
    }
}

#[derive(Debug)]
struct CommandExecutionState {
    cursor: usize,
    slots: Vec<SlotState>,
}

impl Default for CommandExecutionState {
    fn default() -> Self {
        Self {
            cursor: 0,
            slots: (0..SLOT_COUNT).map(|_| SlotState::default()).collect(),
        }
    }
}

#[derive(Clone)]
struct AppState {
    state: Arc<Mutex<CommandExecutionState>>,
    job_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ReserveRequest {
    job_file: Option<String>,
    cwd: Option<String>,
    started_at: Option<String>,
    launcher_pid: Option<u32>,
    cmd_pid: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ReserveResponse {
    ok: bool,
    job_id: u16,
}

#[derive(Debug, Deserialize)]
struct UpdateRequest {
    job_file: Option<String>,
    cwd: Option<String>,
    started_at: Option<String>,
    launcher_pid: Option<u32>,
    cmd_pid: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct WaitRequest {
    job_id: u16,
}

#[derive(Debug, Serialize)]
struct WaitResponse {
    ok: bool,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    service: &'static str,
    active_jobs: usize,
    next_slot: usize,
    job_dir: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    ok: bool,
    error: String,
}

#[derive(Debug, Deserialize)]
struct WaitToolArgs {
    job_id: u16,
}

#[derive(Clone)]
struct CommandExecutionMcpServer {
    app: AppState,
    tools: Arc<Vec<Tool>>,
}

impl CommandExecutionMcpServer {
    fn new(app: AppState) -> Self {
        Self {
            app,
            tools: Arc::new(vec![Self::wait_tool()]),
        }
    }

    fn wait_tool() -> Tool {
        #[expect(clippy::expect_used)]
        let schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "job_id": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": SLOT_COUNT - 1
                }
            },
            "required": ["job_id"],
            "additionalProperties": false
        }))
        .expect("wait tool schema should deserialize");

        Tool::new(
            Cow::Borrowed(WAIT_TOOL_NAME),
            Cow::Borrowed(
                "Wait until the command-execution job file disappears for the given numeric job id.",
            ),
            Arc::new(schema),
        )
    }
}

impl ServerHandler for CommandExecutionMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let tools = self.tools.clone();
        async move {
            Ok(ListToolsResult {
                tools: (*tools).clone(),
                next_cursor: None,
                meta: None,
            })
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        match request.name.as_ref() {
            WAIT_TOOL_NAME => {
                let args: WaitToolArgs = parse_tool_args(request.arguments)?;
                wait_for_job(&self.app, args.job_id)
                    .await
                    .map_err(|error| McpError::invalid_params(error, None))?;

                Ok(CallToolResult::success(vec![Content::text("all done")]))
            }
            other => Err(McpError::invalid_params(
                format!("unknown tool: {other}"),
                None,
            )),
        }
    }
}

fn parse_tool_args<T: serde::de::DeserializeOwned>(
    arguments: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<T, McpError> {
    match arguments {
        Some(arguments) => serde_json::from_value(serde_json::Value::Object(arguments))
            .map_err(|err| McpError::invalid_params(err.to_string(), None)),
        None => Err(McpError::invalid_params("missing tool arguments", None)),
    }
}

fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn normalize_job_file(job_dir: &Path, raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("job_file is required".to_string());
    }
    let path = PathBuf::from(trimmed);
    let resolved = if path.is_absolute() {
        path
    } else {
        job_dir.join(path)
    };
    Ok(resolved)
}

async fn healthz(State(app): State<AppState>) -> Json<HealthResponse> {
    let inner = app.state.lock().await;
    let active_jobs = inner.slots.iter().filter(|slot| slot.reservation.is_some()).count();
    Json(HealthResponse {
        ok: true,
        service: "codex-command-execution-http",
        active_jobs,
        next_slot: inner.cursor,
        job_dir: app.job_dir.display().to_string(),
    })
}

async fn reserve(
    State(app): State<AppState>,
    Json(request): Json<ReserveRequest>,
) -> Result<Json<ReserveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let job_file = request
        .job_file
        .as_deref()
        .map(|raw| normalize_job_file(&app.job_dir, raw))
        .transpose()
        .map_err(bad_request)?;

    let mut inner = app.state.lock().await;
    for offset in 0..SLOT_COUNT {
        let index = (inner.cursor + offset) % SLOT_COUNT;
        if inner.slots[index].reservation.is_none() {
            inner.slots[index].notify = Arc::new(Notify::new());
            inner.slots[index].reservation = Some(ReservationInfo {
                job_id: index as u16,
                job_file: job_file.map(|path| path.display().to_string()),
                cwd: request.cwd.clone(),
                started_at: request.started_at.clone(),
                launcher_pid: request.launcher_pid,
                cmd_pid: request.cmd_pid,
                reserved_epoch_ms: now_epoch_ms(),
            });
            inner.cursor = (index + 1) % SLOT_COUNT;
            return Ok(Json(ReserveResponse {
                ok: true,
                job_id: index as u16,
            }));
        }
    }

    Err(service_unavailable("command execution capacity exhausted"))
}

async fn update(
    State(app): State<AppState>,
    AxumPath(job_id): AxumPath<u16>,
    Json(request): Json<UpdateRequest>,
) -> Result<Json<ReservationInfo>, (StatusCode, Json<ErrorResponse>)> {
    let index = usize::from(job_id);
    if index >= SLOT_COUNT {
        return Err(not_found(format!("unknown job_id: {job_id}")));
    }

    let mut inner = app.state.lock().await;
    let Some(current) = inner.slots[index].reservation.as_mut() else {
        return Err(not_found(format!("job_id {job_id} is not reserved")));
    };

    if let Some(job_file) = request.job_file.as_deref() {
        current.job_file = normalize_job_file(&app.job_dir, job_file)
            .map_err(bad_request)?
            .display()
            .to_string()
            .into();
    }
    if let Some(cwd) = request.cwd {
        current.cwd = Some(cwd);
    }
    if let Some(started_at) = request.started_at {
        current.started_at = Some(started_at);
    }
    if let Some(launcher_pid) = request.launcher_pid {
        current.launcher_pid = Some(launcher_pid);
    }
    if let Some(cmd_pid) = request.cmd_pid {
        current.cmd_pid = Some(cmd_pid);
    }

    Ok(Json(current.clone()))
}

async fn wait_http(
    State(app): State<AppState>,
    Json(request): Json<WaitRequest>,
) -> Result<Json<WaitResponse>, (StatusCode, Json<ErrorResponse>)> {
    wait_for_job(&app, request.job_id)
        .await
        .map_err(not_found)?;
    Ok(Json(WaitResponse {
        ok: true,
        status: "all done",
    }))
}

async fn wait_for_job(app: &AppState, job_id: u16) -> Result<(), String> {
    let index = usize::from(job_id);
    if index >= SLOT_COUNT {
        return Err(format!("unknown job_id: {job_id}"));
    }

    loop {
        let notify = {
            let inner = app.state.lock().await;
            if inner.slots[index].reservation.is_none() {
                return Ok(());
            }
            inner.slots[index].notify.clone()
        };
        notify.notified().await;
    }
}

async fn watcher(app: AppState, interval: Duration) {
    loop {
        sleep(interval).await;

        let mut completed = Vec::new();
        let mut stale = Vec::new();
        {
            let mut inner = app.state.lock().await;
            for slot in &mut inner.slots {
                let Some(reservation) = slot.reservation.as_ref() else {
                    continue;
                };
                let stale_reason = stale_reservation_reason(reservation);
                let missing_job_file = reservation
                    .job_file
                    .as_deref()
                    .map(|job_file| !Path::new(job_file).exists())
                    .unwrap_or(false);
                if !missing_job_file && stale_reason.is_none() {
                    continue;
                }

                if let Some(reason) = stale_reason {
                    stale.push((reservation.job_id, reason, reservation.job_file.clone()));
                } else {
                    completed.push(reservation.job_id);
                }

                slot.reservation = None;
                let notify = slot.notify.clone();
                slot.notify = Arc::new(Notify::new());
                notify.notify_waiters();
            }
        }

        for job_id in completed {
            info!("reaped completed command-execution slot job_id={job_id}");
        }
        for (job_id, reason, job_file) in stale {
            warn!(
                "reaped stale command-execution slot job_id={} reason={} job_file={}",
                job_id,
                reason,
                job_file.as_deref().unwrap_or("(none)")
            );
        }
    }
}

fn stale_reservation_reason(reservation: &ReservationInfo) -> Option<&'static str> {
    if let Some(cmd_pid) = reservation.cmd_pid
        && !pid_is_alive(cmd_pid)
    {
        return Some("cmd_pid_missing");
    }

    if let Some(launcher_pid) = reservation.launcher_pid
        && !pid_is_alive(launcher_pid)
    {
        return Some("launcher_pid_missing");
    }

    None
}

fn pid_is_alive(pid: u32) -> bool {
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }

    // SAFETY: kill(pid, 0) does not send a signal; it only checks whether the
    // target process exists and is signalable by the current user.
    let result = unsafe { libc::kill(pid as i32, 0) };
    if result == 0 {
        return true;
    }

    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EPERM) => true,
        Some(libc::ESRCH) => false,
        _ => false,
    }
}

fn bad_request(error: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            ok: false,
            error: error.into(),
        }),
    )
}

fn not_found(error: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            ok: false,
            error: error.into(),
        }),
    )
}

fn service_unavailable(error: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            ok: false,
            error: error.into(),
        }),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing("codex_command_execution_http")?;

    std::fs::create_dir_all(&args.job_dir)?;

    let app_state = AppState {
        state: Arc::new(Mutex::new(CommandExecutionState::default())),
        job_dir: args.job_dir.clone(),
    };

    tokio::spawn(watcher(
        app_state.clone(),
        Duration::from_millis(args.poll_interval_ms.max(50)),
    ));

    let mcp_service = StreamableHttpService::new(
        {
            let app_state = app_state.clone();
            move || Ok(CommandExecutionMcpServer::new(app_state.clone()))
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig {
            stateful_mode: false,
            ..StreamableHttpServerConfig::default()
        },
    );

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/reserve", post(reserve))
        .route("/jobs/{job_id}", post(update))
        .route("/command_execution_wait", post(wait_http))
        .route_service("/", mcp_service)
        .with_state(app_state)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(args.http().socket_addr()).await?;
    info!(
        "codex-command-execution-http listening on {}",
        listener.local_addr()?
    );
    axum::serve(listener, app).await?;
    Ok(())
}
