use axum::{
    response::sse::{Event, KeepAlive, Sse},
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use codex_backend_core::HealthResponse;
use futures_util::stream::unfold;
use crate::{
    events::HarnessEvent,
    models::{ApiError, CommandRequest, LeaseRequest, StartRequest},
    runtime::SharedHarnessRuntime,
};

pub fn build_router(runtime: SharedHarnessRuntime) -> Router {
    build_router_with_service_name(runtime, "codex-qa-harness")
}

pub fn build_router_with_service_name(runtime: SharedHarnessRuntime, service_name: &'static str) -> Router {
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
        .with_state(RouterState {
            runtime,
            service_name,
        })
}

#[derive(Clone)]
struct RouterState {
    runtime: SharedHarnessRuntime,
    service_name: &'static str,
}

async fn health(State(state): State<RouterState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: state.service_name,
        status: "ok",
        phase: if state.runtime.project_count() > 0 {
            "configured"
        } else {
            "empty"
        },
    })
}

async fn events(
    State(state): State<RouterState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let receiver = state.runtime.subscribe_events();
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

async fn list_projects(State(state): State<RouterState>) -> Json<Vec<crate::models::ProjectSummary>> {
    Json(state.runtime.project_summaries())
}

async fn list_devices(
    State(state): State<RouterState>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<crate::models::DeviceSummary>>, (StatusCode, Json<ApiError>)> {
    state
        .runtime
        .list_devices(&project_id)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn get_device(
    State(state): State<RouterState>,
    Path((project_id, device_key)): Path<(String, String)>,
) -> Result<Json<crate::models::DeviceSummary>, (StatusCode, Json<ApiError>)> {
    state
        .runtime
        .device_summary(&project_id, &device_key)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn acquire_lease(
    State(state): State<RouterState>,
    Path((project_id, device_key)): Path<(String, String)>,
    Json(request): Json<LeaseRequest>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    state
        .runtime
        .acquire_lease(&project_id, &device_key, request)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn release_lease(
    State(state): State<RouterState>,
    Path((project_id, device_key)): Path<(String, String)>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    state
        .runtime
        .release_lease(&project_id, &device_key)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn start(
    State(state): State<RouterState>,
    Path((project_id, device_key)): Path<(String, String)>,
    Json(request): Json<StartRequest>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    state
        .runtime
        .start(&project_id, &device_key, request)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn restart(
    State(state): State<RouterState>,
    Path((project_id, device_key)): Path<(String, String)>,
    Json(request): Json<StartRequest>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    state
        .runtime
        .restart(&project_id, &device_key, request)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn teardown(
    State(state): State<RouterState>,
    Path((project_id, device_key)): Path<(String, String)>,
    Json(request): Json<StartRequest>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    state
        .runtime
        .teardown(&project_id, &device_key, request)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn command(
    State(state): State<RouterState>,
    Path((project_id, device_key)): Path<(String, String)>,
    Json(request): Json<CommandRequest>,
) -> Result<Json<crate::models::SlotRuntimeState>, (StatusCode, Json<ApiError>)> {
    state
        .runtime
        .command(&project_id, &device_key, request)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn simulator_status(
    State(state): State<RouterState>,
    Path((project_id, device_key)): Path<(String, String)>,
) -> Result<Json<crate::ios_sim::SimulatorStatus>, (StatusCode, Json<ApiError>)> {
    state
        .runtime
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::{DeviceConfig, HarnessConfig, HooksConfig, ProjectConfig}, models::DeviceType, runtime::HarnessRuntime};
    use axum::{body::Body, http::Request};
    use tempfile::tempdir;
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn healthz_uses_supplied_service_name() {
        let temp = tempdir().expect("tempdir");
        let config = HarnessConfig {
            projects: std::iter::once((
                "demo".to_string(),
                ProjectConfig {
                    id: "demo".to_string(),
                    display_name: "Demo".to_string(),
                    repo_root: temp.path().join("repo"),
                    runtime_root: temp.path().join("runtime"),
                    env: Default::default(),
                    devices: std::iter::once((
                        "primary".to_string(),
                        DeviceConfig {
                            device_type: DeviceType::IosSim,
                            device_id: "SIM-123".to_string(),
                            name: "Primary".to_string(),
                            runtime_subdir: "primary".to_string(),
                            boot_policy: "lazy".to_string(),
                        },
                    ))
                    .collect(),
                    hooks: HooksConfig {
                        prepare_source: temp.path().join("prepare.sh"),
                        start_dependencies: None,
                        start_runtime: temp.path().join("start.sh"),
                        check_readiness: temp.path().join("ready.sh"),
                        teardown: temp.path().join("teardown.sh"),
                        command: temp.path().join("command.sh"),
                    },
                    timeouts: Default::default(),
                },
            ))
            .collect(),
        };
        let runtime = HarnessRuntime::from_config(config, temp.path().join("state")).expect("runtime");
        let app = build_router_with_service_name(runtime, "codex-flutter-sim-http");
        let response = app
            .oneshot(Request::builder().uri("/healthz").body(Body::empty()).expect("request"))
            .await
            .expect("response");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(payload["service"], "codex-flutter-sim-http");
        assert_eq!(payload["phase"], "configured");
    }
}
