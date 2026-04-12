use axum::{
    response::sse::{Event, KeepAlive, Sse},
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use codex_backend_core::HealthResponse;
use futures_util::stream::unfold;

use crate::{
    events::HarnessEvent,
    models::{ApiError, CommandRequest, LeaseRequest, StartRequest},
    runtime::SharedHarnessRuntime,
};

pub fn build_router(runtime: SharedHarnessRuntime) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/events", get(events))
        .route("/projects", get(list_projects))
        .route("/projects/{project_id}/devices", get(list_devices))
        .route("/projects/{project_id}/devices/{device_key}", get(get_device))
        .route(
            "/projects/{project_id}/devices/{device_key}/lease",
            post(acquire_lease).delete(release_lease),
        )
        .route("/projects/{project_id}/devices/{device_key}/start", post(start))
        .route("/projects/{project_id}/devices/{device_key}/restart", post(restart))
        .route("/projects/{project_id}/devices/{device_key}/teardown", post(teardown))
        .route("/projects/{project_id}/devices/{device_key}/commands", post(command))
        .route("/projects/{project_id}/devices/{device_key}/simulator", get(simulator_status))
        .with_state(runtime)
}

async fn health(State(runtime): State<SharedHarnessRuntime>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "codex-qa-harness",
        status: "ok",
        phase: if runtime.project_count() > 0 {
            "configured"
        } else {
            "empty"
        },
    })
}

async fn events(
    State(runtime): State<SharedHarnessRuntime>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let receiver = runtime.subscribe_events();
    let stream = unfold(receiver, |mut receiver| async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    let payload = serde_json::to_string(&event)
                        .unwrap_or_else(|_| "{\"kind\":\"serialize_error\"}".to_string());
                    let sse_event = Event::default().event(event_name(&event)).data(payload);
                    return Some((Ok(sse_event), receiver));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn list_projects(State(runtime): State<SharedHarnessRuntime>) -> Json<Vec<crate::models::ProjectSummary>> {
    Json(runtime.project_summaries())
}

async fn list_devices(
    State(runtime): State<SharedHarnessRuntime>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<crate::models::DeviceSummary>>, (StatusCode, Json<ApiError>)> {
    runtime
        .list_devices(&project_id)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn get_device(
    State(runtime): State<SharedHarnessRuntime>,
    Path((project_id, device_key)): Path<(String, String)>,
) -> Result<Json<crate::models::DeviceSummary>, (StatusCode, Json<ApiError>)> {
    runtime
        .device_summary(&project_id, &device_key)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn acquire_lease(
    State(runtime): State<SharedHarnessRuntime>,
    Path((project_id, device_key)): Path<(String, String)>,
    Json(request): Json<LeaseRequest>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    runtime
        .acquire_lease(&project_id, &device_key, request)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn release_lease(
    State(runtime): State<SharedHarnessRuntime>,
    Path((project_id, device_key)): Path<(String, String)>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    runtime
        .release_lease(&project_id, &device_key)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn start(
    State(runtime): State<SharedHarnessRuntime>,
    Path((project_id, device_key)): Path<(String, String)>,
    Json(request): Json<StartRequest>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    runtime
        .start(&project_id, &device_key, request)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn restart(
    State(runtime): State<SharedHarnessRuntime>,
    Path((project_id, device_key)): Path<(String, String)>,
    Json(request): Json<StartRequest>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    runtime
        .restart(&project_id, &device_key, request)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn teardown(
    State(runtime): State<SharedHarnessRuntime>,
    Path((project_id, device_key)): Path<(String, String)>,
    Json(request): Json<StartRequest>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    runtime
        .teardown(&project_id, &device_key, request)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn command(
    State(runtime): State<SharedHarnessRuntime>,
    Path((project_id, device_key)): Path<(String, String)>,
    Json(request): Json<CommandRequest>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    runtime
        .command(&project_id, &device_key, request)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn simulator_status(
    State(runtime): State<SharedHarnessRuntime>,
    Path((project_id, device_key)): Path<(String, String)>,
) -> Result<Json<crate::ios_sim::SimulatorStatus>, (StatusCode, Json<ApiError>)> {
    runtime
        .simulator_status(&project_id, &device_key)
        .await
        .map(Json)
        .map_err(internal_error)
}

fn internal_error(error: anyhow::Error) -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiError::new(error.to_string())),
    )
}

fn event_name(event: &HarnessEvent) -> &str {
    &event.kind
}
