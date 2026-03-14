use std::collections::VecDeque;
use std::net::SocketAddr;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

const SAMPLE_INTERVAL_SECONDS: u64 = 5;
const HISTORY_WINDOW_SECONDS: i64 = 10 * 60;
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
    zone_map_size: u64,
    zone_map_max: u64,
    zone_map_pct: f64,
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
    zone_map_pct: f64,
    kalloc_1024_used: u64,
    growth_rate: f64,
    status: HealthStatus,
    zone_map_size: u64,
    zone_map_max: u64,
    object_size: u64,
    allocated_memory: String,
    capacity: String,
    elements: u64,
    elements_in_use: u64,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct HistoryEntry {
    t: DateTime<Utc>,
    zone_map_pct: f64,
    kalloc: u64,
}

#[derive(Clone, Debug, Serialize)]
struct HistoryResponse {
    samples: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, Serialize)]
struct ErrorResponse {
    error: String,
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

async fn history_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().await;
    let samples = guard
        .history
        .iter()
        .map(|sample| HistoryEntry {
            t: sample.timestamp,
            zone_map_pct: sample.zone_map_pct,
            kalloc: sample.kalloc_1024_used,
        })
        .collect();

    (StatusCode::OK, Json(HistoryResponse { samples })).into_response()
}

fn sample_once(state: &mut AppState) -> Result<()> {
    let zone_map_size = read_sysctl_value("vm.zone_map_size")?;
    let zone_map_max = read_sysctl_value("vm.zone_map_max")?;
    let zprint_output = run_command("zprint", &[])?;
    let zone_row = parse_zone_row(&zprint_output, "data_shared.kalloc.1024")?;
    let zone_map_pct = percent(zone_map_size, zone_map_max)?;
    let now = Utc::now();

    let sample = Sample {
        timestamp: now,
        zone_map_size,
        zone_map_max,
        zone_map_pct,
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
    })
}

fn classify_status(history: &VecDeque<Sample>, growth_rate: f64, zone_map_pct: f64) -> HealthStatus {
    if zone_map_pct > CRITICAL_ZONE_MAP_PERCENT {
        return HealthStatus::Critical;
    }
    if zone_map_pct > DANGER_ZONE_MAP_PERCENT {
        return HealthStatus::Danger;
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
            zone_map_size: 1,
            zone_map_max: 2,
            zone_map_pct,
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

        let status = classify_status(&history, compute_growth_rate(&history), 40.1);
        assert_eq!(status, HealthStatus::Stable);
    }

    #[test]
    fn growing_status_for_small_positive_slope() {
        let history = VecDeque::from([
            sample_at(0, 45.0, 1_000),
            sample_at(5, 45.2, 1_006),
        ]);

        let growth = compute_growth_rate(&history);
        let status = classify_status(&history, growth, 45.2);
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
        let status = classify_status(&history, growth, 50.5);
        assert!(growth > LEAK_RATE_THRESHOLD);
        assert_eq!(status, HealthStatus::LeakDetected);
    }

    #[test]
    fn danger_overrides_growth() {
        let history = VecDeque::from([
            sample_at(0, 82.0, 1_000),
            sample_at(65, 82.5, 2_000),
        ]);

        let status = classify_status(&history, compute_growth_rate(&history), 82.5);
        assert_eq!(status, HealthStatus::Danger);
    }

    #[test]
    fn critical_overrides_everything() {
        let history = VecDeque::from([
            sample_at(0, 91.0, 1_000),
            sample_at(65, 91.5, 2_000),
        ]);

        let status = classify_status(&history, compute_growth_rate(&history), 91.5);
        assert_eq!(status, HealthStatus::Critical);
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
}

