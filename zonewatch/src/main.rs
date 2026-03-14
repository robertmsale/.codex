use std::collections::VecDeque;
use std::net::SocketAddr;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

const SAMPLE_INTERVAL_SECONDS: u64 = 5;
const HISTORY_WINDOW_SECONDS: i64 = 10 * 60;
const HISTORY_DEFAULT_LIMIT: usize = 50;
const HISTORY_MAX_LIMIT: usize = 200;
const LEAK_WINDOW_SECONDS: i64 = 60;
const LEAK_RATE_THRESHOLD: f64 = 5.0;
const GROWING_RATE_THRESHOLD: f64 = 0.5;
const DANGER_ZONE_MAP_PERCENT: f64 = 80.0;
const CRITICAL_ZONE_MAP_PERCENT: f64 = 90.0;
const BIND_ADDR: &str = "127.0.0.1:9032";

type SharedState = Arc<RwLock<AppState>>;

#[derive(Clone, Debug, Serialize)]
struct Sample {
    timestamp: DateTime<Utc>,
    zone_map_size: Option<u64>,
    zone_map_max: Option<u64>,
    zone_map_pct: Option<f64>,
    zone_name: String,
    object_size: u64,
    allocated_memory: String,
    capacity: String,
    elements: u64,
    elements_in_use: u64,
    kalloc_1024_used: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum HealthStatus {
    Stable,
    Growing,
    LeakDetected,
    Danger,
    Critical,
}

#[derive(Clone, Debug, Serialize)]
struct HealthResponse {
    timestamp: DateTime<Utc>,
    zone_map_pct: Option<f64>,
    kalloc_1024_used: u64,
    growth_rate: f64,
    status: HealthStatus,
    zone_map_size: Option<u64>,
    zone_map_max: Option<u64>,
    object_size: u64,
    allocated_memory: String,
    capacity: String,
    elements: u64,
    elements_in_use: u64,
    last_error: Option<String>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct HistoryEntry {
    t: DateTime<Utc>,
    zone_map_pct: Option<f64>,
    kalloc: u64,
}

#[derive(Clone, Debug, Serialize)]
struct HistoryResponse {
    samples: Vec<HistoryEntry>,
    next_cursor: Option<DateTime<Utc>>,
    has_more: bool,
    limit: usize,
}

#[derive(Clone, Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
    cursor: Option<String>,
    start: Option<String>,
    end: Option<String>,
}

#[derive(Clone, Debug)]
struct HistoryPageRequest {
    limit: usize,
    cursor: Option<DateTime<Utc>>,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
struct AppState {
    history: VecDeque<Sample>,
    last_error: Option<String>,
}

#[derive(Clone, Debug)]
struct ZoneRow {
    zone_name: String,
    object_size: u64,
    allocated_memory: String,
    capacity: String,
    elements: u64,
    elements_in_use: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let state = Arc::new(RwLock::new(AppState {
        history: VecDeque::new(),
        last_error: None,
    }));

    {
        let mut guard = state.write().await;
        if let Err(err) = sample_once(&mut guard) {
            guard.last_error = Some(err.to_string());
            eprintln!("zonewatch: initial sample failed: {err:#}");
        }
    }

    let sampler_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(SAMPLE_INTERVAL_SECONDS));
        interval.tick().await;
        loop {
            interval.tick().await;
            let mut guard = sampler_state.write().await;
            if let Err(err) = sample_once(&mut guard) {
                guard.last_error = Some(err.to_string());
                eprintln!("zonewatch: sample failed: {err:#}");
            }
        }
    });

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/history", get(history_handler))
        .with_state(state);

    let addr: SocketAddr = BIND_ADDR.parse().context("invalid bind address")?;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {BIND_ADDR}"))?;

    println!("zonewatch: listening on http://{BIND_ADDR}");
    axum::serve(listener, app)
        .await
        .context("zonewatch server exited unexpectedly")
}

async fn health_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().await;
    match build_health_response(&guard.history, guard.last_error.clone()) {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: err.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn history_handler(
    State(state): State<SharedState>,
    Query(query): Query<HistoryQuery>,
) -> impl IntoResponse {
    let request = match parse_history_request(query) {
        Ok(request) => request,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse { error: err }),
            )
                .into_response();
        }
    };

    let guard = state.read().await;
    (
        StatusCode::OK,
        Json(build_history_response(&guard.history, &request)),
    )
        .into_response()
}

fn sample_once(state: &mut AppState) -> Result<()> {
    let zone_map = read_zone_map_metrics()?;
    let zprint_output = run_command("zprint", &[])?;
    let zone_row = parse_zone_row(&zprint_output, "data_shared.kalloc.1024")?;
    let now = Utc::now();

    let sample = Sample {
        timestamp: now,
        zone_map_size: zone_map.as_ref().map(|metrics| metrics.zone_map_size),
        zone_map_max: zone_map.as_ref().map(|metrics| metrics.zone_map_max),
        zone_map_pct: zone_map.as_ref().map(|metrics| metrics.zone_map_pct),
        zone_name: zone_row.zone_name,
        object_size: zone_row.object_size,
        allocated_memory: zone_row.allocated_memory,
        capacity: zone_row.capacity,
        elements: zone_row.elements,
        elements_in_use: zone_row.elements_in_use,
        kalloc_1024_used: zone_row.elements_in_use,
    };

    state.history.push_back(sample);
    state.last_error = None;
    trim_history(&mut state.history, now);
    Ok(())
}

fn trim_history(history: &mut VecDeque<Sample>, now: DateTime<Utc>) {
    let cutoff = now - ChronoDuration::seconds(HISTORY_WINDOW_SECONDS);
    while history
        .front()
        .is_some_and(|sample| sample.timestamp < cutoff)
    {
        history.pop_front();
    }
}

fn build_health_response(
    history: &VecDeque<Sample>,
    last_error: Option<String>,
) -> Result<HealthResponse> {
    let current = history
        .back()
        .cloned()
        .ok_or_else(|| anyhow!("no samples collected yet"))?;
    let growth_rate = compute_growth_rate(history);
    let status = classify_status(history, growth_rate, current.zone_map_pct);
    let mut warnings = Vec::new();
    if current.zone_map_pct.is_none() {
        warnings.push("zone map sysctls are unavailable on this system; zone map percentage metrics are omitted".to_string());
    }

    Ok(HealthResponse {
        timestamp: current.timestamp,
        zone_map_pct: current.zone_map_pct,
        kalloc_1024_used: current.kalloc_1024_used,
        growth_rate,
        status,
        zone_map_size: current.zone_map_size,
        zone_map_max: current.zone_map_max,
        object_size: current.object_size,
        allocated_memory: current.allocated_memory,
        capacity: current.capacity,
        elements: current.elements,
        elements_in_use: current.elements_in_use,
        last_error,
        warnings,
    })
}

fn parse_history_request(query: HistoryQuery) -> Result<HistoryPageRequest, String> {
    let limit = query
        .limit
        .unwrap_or(HISTORY_DEFAULT_LIMIT)
        .clamp(1, HISTORY_MAX_LIMIT);
    let cursor = parse_history_timestamp("cursor", query.cursor.as_deref())?;
    let start = parse_history_timestamp("start", query.start.as_deref())?;
    let end = parse_history_timestamp("end", query.end.as_deref())?;

    if let (Some(start), Some(end)) = (start, end) {
        if start > end {
            return Err("start must be less than or equal to end".to_string());
        }
    }

    Ok(HistoryPageRequest {
        limit,
        cursor,
        start,
        end,
    })
}

fn parse_history_timestamp(field: &str, raw: Option<&str>) -> Result<Option<DateTime<Utc>>, String> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|_| format!("{field} must be an RFC3339 timestamp"))
}

fn build_history_response(history: &VecDeque<Sample>, request: &HistoryPageRequest) -> HistoryResponse {
    let mut samples: Vec<HistoryEntry> = history
        .iter()
        .rev()
        .filter(|sample| request.start.is_none_or(|start| sample.timestamp >= start))
        .filter(|sample| request.end.is_none_or(|end| sample.timestamp <= end))
        .filter(|sample| request.cursor.is_none_or(|cursor| sample.timestamp < cursor))
        .take(request.limit + 1)
        .map(|sample| HistoryEntry {
            t: sample.timestamp,
            zone_map_pct: sample.zone_map_pct,
            kalloc: sample.kalloc_1024_used,
        })
        .collect();

    let has_more = samples.len() > request.limit;
    if has_more {
        samples.truncate(request.limit);
    }
    let next_cursor = samples.last().map(|sample| sample.t).filter(|_| has_more);

    HistoryResponse {
        samples,
        next_cursor,
        has_more,
        limit: request.limit,
    }
}

fn classify_status(
    history: &VecDeque<Sample>,
    growth_rate: f64,
    zone_map_pct: Option<f64>,
) -> HealthStatus {
    if let Some(zone_map_pct) = zone_map_pct {
        if zone_map_pct > CRITICAL_ZONE_MAP_PERCENT {
            return HealthStatus::Critical;
        }
        if zone_map_pct > DANGER_ZONE_MAP_PERCENT {
            return HealthStatus::Danger;
        }
    }

    if sustained_leak_rate(history).is_some_and(|rate| rate > LEAK_RATE_THRESHOLD) {
        return HealthStatus::LeakDetected;
    }

    if growth_rate > GROWING_RATE_THRESHOLD {
        return HealthStatus::Growing;
    }

    HealthStatus::Stable
}

fn compute_growth_rate(history: &VecDeque<Sample>) -> f64 {
    if let Some(rate) = sustained_leak_rate(history) {
        return rate;
    }

    let mut iter = history.iter().rev();
    let current = match iter.next() {
        Some(sample) => sample,
        None => return 0.0,
    };
    let previous = match iter.next() {
        Some(sample) => sample,
        None => return 0.0,
    };

    rate_between(previous, current).unwrap_or(0.0)
}

fn sustained_leak_rate(history: &VecDeque<Sample>) -> Option<f64> {
    let current = history.back()?;
    let baseline = history
        .iter()
        .rev()
        .find(|sample| (current.timestamp - sample.timestamp).num_seconds() >= LEAK_WINDOW_SECONDS)?;

    rate_between(baseline, current)
}

fn rate_between(older: &Sample, newer: &Sample) -> Option<f64> {
    let elapsed = (newer.timestamp - older.timestamp).num_milliseconds();
    if elapsed <= 0 {
        return None;
    }

    let delta = newer.kalloc_1024_used as f64 - older.kalloc_1024_used as f64;
    Some(delta / (elapsed as f64 / 1000.0))
}

fn read_sysctl_value(key: &str) -> Result<u64> {
    let output = run_command("sysctl", &[key])?;
    parse_sysctl_value(&output).with_context(|| format!("failed to parse sysctl output for {key}"))
}

#[derive(Clone, Copy, Debug)]
struct ZoneMapMetrics {
    zone_map_size: u64,
    zone_map_max: u64,
    zone_map_pct: f64,
}

fn read_zone_map_metrics() -> Result<Option<ZoneMapMetrics>> {
    let zone_map_size = match read_sysctl_value("vm.zone_map_size") {
        Ok(value) => value,
        Err(err) if is_unknown_oid_error(&err) => return Ok(None),
        Err(err) => return Err(err),
    };
    let zone_map_max = match read_sysctl_value("vm.zone_map_max") {
        Ok(value) => value,
        Err(err) if is_unknown_oid_error(&err) => return Ok(None),
        Err(err) => return Err(err),
    };

    Ok(Some(ZoneMapMetrics {
        zone_map_size,
        zone_map_max,
        zone_map_pct: percent(zone_map_size, zone_map_max)?,
    }))
}

fn is_unknown_oid_error(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.to_string().contains("unknown oid"))
}

fn parse_sysctl_value(output: &str) -> Result<u64> {
    let (_, value) = output
        .split_once(':')
        .ok_or_else(|| anyhow!("expected sysctl output in 'key: value' form"))?;
    parse_numeric_token(value.trim())
}

fn parse_zone_row(output: &str, zone_name: &str) -> Result<ZoneRow> {
    let line = output
        .lines()
        .find(|line| line.trim_start().starts_with(zone_name))
        .ok_or_else(|| anyhow!("zone row '{zone_name}' not found in zprint output"))?;

    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 6 {
        bail!("zone row '{zone_name}' had too few fields: {line}");
    }

    Ok(ZoneRow {
        zone_name: fields[0].to_string(),
        object_size: parse_numeric_token(fields[1])?,
        allocated_memory: fields[2].to_string(),
        capacity: fields[3].to_string(),
        elements: parse_numeric_token(fields[4])?,
        elements_in_use: parse_numeric_token(fields[5])?,
    })
}

fn percent(numerator: u64, denominator: u64) -> Result<f64> {
    if denominator == 0 {
        bail!("zone map max is zero");
    }
    Ok((numerator as f64 / denominator as f64) * 100.0)
}

fn parse_numeric_token(token: &str) -> Result<u64> {
    let cleaned: String = token
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect();
    if cleaned.is_empty() {
        bail!("expected numeric token, got '{token}'");
    }
    cleaned
        .parse::<u64>()
        .with_context(|| format!("failed to parse numeric token '{token}'"))
}

fn run_command(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to launch {program}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!("{program} failed: {stderr}");
    }

    String::from_utf8(output.stdout)
        .with_context(|| format!("{program} output was not valid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_at(seconds: i64, zone_map_pct: f64, kalloc: u64) -> Sample {
        Sample {
            timestamp: Utc::now() + ChronoDuration::seconds(seconds),
            zone_map_size: Some(1),
            zone_map_max: Some(2),
            zone_map_pct: Some(zone_map_pct),
            zone_name: "data_shared.kalloc.1024".to_string(),
            object_size: 1024,
            allocated_memory: "10M".to_string(),
            capacity: "20M".to_string(),
            elements: 10_000,
            elements_in_use: kalloc,
            kalloc_1024_used: kalloc,
        }
    }

    #[test]
    fn parses_sysctl_value() {
        let value = parse_sysctl_value("vm.zone_map_size: 123456789").unwrap();
        assert_eq!(value, 123_456_789);
    }

    #[test]
    fn parses_zone_row() {
        let output = "\
ZONE NAME                SIZE  ALLOCATED  CAPACITY   ELEMS   INUSE
data_shared.kalloc.1024  1024      128K      256K     128      96
";
        let row = parse_zone_row(output, "data_shared.kalloc.1024").unwrap();
        assert_eq!(row.zone_name, "data_shared.kalloc.1024");
        assert_eq!(row.object_size, 1024);
        assert_eq!(row.allocated_memory, "128K");
        assert_eq!(row.capacity, "256K");
        assert_eq!(row.elements, 128);
        assert_eq!(row.elements_in_use, 96);
    }

    #[test]
    fn stable_status_when_usage_is_flat() {
        let history = VecDeque::from([
            sample_at(0, 40.0, 1_000),
            sample_at(5, 40.1, 1_001),
        ]);

        let status = classify_status(&history, compute_growth_rate(&history), Some(40.1));
        assert_eq!(status, HealthStatus::Stable);
    }

    #[test]
    fn growing_status_for_small_positive_slope() {
        let history = VecDeque::from([
            sample_at(0, 45.0, 1_000),
            sample_at(5, 45.2, 1_006),
        ]);

        let growth = compute_growth_rate(&history);
        let status = classify_status(&history, growth, Some(45.2));
        assert!(growth > GROWING_RATE_THRESHOLD);
        assert_eq!(status, HealthStatus::Growing);
    }

    #[test]
    fn leak_detected_for_sustained_growth() {
        let history = VecDeque::from([
            sample_at(0, 50.0, 1_000),
            sample_at(65, 50.5, 1_400),
        ]);

        let growth = compute_growth_rate(&history);
        let status = classify_status(&history, growth, Some(50.5));
        assert!(growth > LEAK_RATE_THRESHOLD);
        assert_eq!(status, HealthStatus::LeakDetected);
    }

    #[test]
    fn danger_overrides_growth() {
        let history = VecDeque::from([
            sample_at(0, 82.0, 1_000),
            sample_at(65, 82.5, 2_000),
        ]);

        let status = classify_status(&history, compute_growth_rate(&history), Some(82.5));
        assert_eq!(status, HealthStatus::Danger);
    }

    #[test]
    fn critical_overrides_everything() {
        let history = VecDeque::from([
            sample_at(0, 91.0, 1_000),
            sample_at(65, 91.5, 2_000),
        ]);

        let status = classify_status(&history, compute_growth_rate(&history), Some(91.5));
        assert_eq!(status, HealthStatus::Critical);
    }

    #[test]
    fn growing_status_still_works_without_zone_map_metrics() {
        let history = VecDeque::from([
            sample_at(0, 45.0, 1_000),
            sample_at(5, 45.2, 1_006),
        ]);

        let status = classify_status(&history, compute_growth_rate(&history), None);
        assert_eq!(status, HealthStatus::Growing);
    }

    #[test]
    fn build_health_response_warns_when_zone_map_metrics_are_missing() {
        let mut sample = sample_at(0, 45.0, 1_000);
        sample.zone_map_size = None;
        sample.zone_map_max = None;
        sample.zone_map_pct = None;
        let history = VecDeque::from([sample]);

        let response = build_health_response(&history, None).unwrap();
        assert!(response.zone_map_pct.is_none());
        assert_eq!(response.warnings.len(), 1);
    }

    #[test]
    fn sustained_leak_rate_uses_nearest_window_baseline() {
        let history = VecDeque::from([
            sample_at(0, 50.0, 1_000),
            sample_at(30, 50.0, 1_000),
            sample_at(61, 50.0, 1_100),
            sample_at(90, 50.0, 1_400),
        ]);

        let rate = sustained_leak_rate(&history).unwrap();
        assert!(rate > 6.0 && rate < 7.0);
    }

    #[test]
    fn trims_history_outside_window() {
        let now = Utc::now();
        let mut history = VecDeque::from([
            Sample {
                timestamp: now - ChronoDuration::seconds(HISTORY_WINDOW_SECONDS + 1),
                ..sample_at(0, 30.0, 100)
            },
            Sample {
                timestamp: now,
                ..sample_at(0, 31.0, 105)
            },
        ]);

        trim_history(&mut history, now);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].kalloc_1024_used, 105);
    }

    #[test]
    fn history_defaults_to_latest_page() {
        let base = Utc::now();
        let history: VecDeque<Sample> = (0..60)
            .map(|idx| Sample {
                timestamp: base + ChronoDuration::seconds(idx),
                ..sample_at(0, 30.0, idx as u64)
            })
            .collect();

        let response = build_history_response(
            &history,
            &HistoryPageRequest {
                limit: HISTORY_DEFAULT_LIMIT,
                cursor: None,
                start: None,
                end: None,
            },
        );

        assert_eq!(response.limit, HISTORY_DEFAULT_LIMIT);
        assert_eq!(response.samples.len(), HISTORY_DEFAULT_LIMIT);
        assert!(response.has_more);
        assert_eq!(response.samples.first().unwrap().kalloc, 59);
        assert_eq!(response.samples.last().unwrap().kalloc, 10);
        assert_eq!(response.next_cursor, Some(response.samples.last().unwrap().t));
    }

    #[test]
    fn history_cursor_pages_backward() {
        let base = Utc::now();
        let history: VecDeque<Sample> = (0..6)
            .map(|idx| Sample {
                timestamp: base + ChronoDuration::seconds(idx),
                ..sample_at(0, 30.0, idx as u64)
            })
            .collect();

        let first_page = build_history_response(
            &history,
            &HistoryPageRequest {
                limit: 2,
                cursor: None,
                start: None,
                end: None,
            },
        );
        let second_page = build_history_response(
            &history,
            &HistoryPageRequest {
                limit: 2,
                cursor: first_page.next_cursor,
                start: None,
                end: None,
            },
        );

        assert_eq!(
            first_page.samples.iter().map(|sample| sample.kalloc).collect::<Vec<_>>(),
            vec![5, 4]
        );
        assert_eq!(
            second_page.samples.iter().map(|sample| sample.kalloc).collect::<Vec<_>>(),
            vec![3, 2]
        );
    }

    #[test]
    fn history_respects_time_range_filters() {
        let base = Utc::now();
        let history: VecDeque<Sample> = (0..6)
            .map(|idx| Sample {
                timestamp: base + ChronoDuration::seconds(idx),
                ..sample_at(0, 30.0, idx as u64)
            })
            .collect();

        let response = build_history_response(
            &history,
            &HistoryPageRequest {
                limit: 10,
                cursor: None,
                start: Some(base + ChronoDuration::seconds(2)),
                end: Some(base + ChronoDuration::seconds(4)),
            },
        );

        assert_eq!(
            response.samples.iter().map(|sample| sample.kalloc).collect::<Vec<_>>(),
            vec![4, 3, 2]
        );
        assert!(!response.has_more);
        assert!(response.next_cursor.is_none());
    }

    #[test]
    fn history_rejects_invalid_ranges() {
        let err = parse_history_request(HistoryQuery {
            limit: Some(10),
            cursor: None,
            start: Some("2026-03-14T20:00:05Z".to_string()),
            end: Some("2026-03-14T20:00:00Z".to_string()),
        })
        .unwrap_err();

        assert_eq!(err, "start must be less than or equal to end");
    }
}
