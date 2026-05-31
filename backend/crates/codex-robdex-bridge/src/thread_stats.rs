use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStatsResponse {
    pub thread_id: String,
    pub session_path: String,
    pub generated_at_ms: u64,
    pub totals: TokenTotals,
    pub estimates: TokenEstimates,
    pub compaction_count: u64,
    pub timeline: Vec<TokenTimelinePoint>,
    pub categories: Vec<TokenCategoryBreakdown>,
    pub top_items: Vec<TokenTopItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenTotals {
    pub input_tokens: u64,
    pub uncached_input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenEstimates {
    pub user_message_input_tokens: u64,
    pub tool_output_input_tokens: u64,
    pub tool_call_output_tokens: u64,
    pub skill_instruction_input_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenTimelinePoint {
    pub index: u64,
    pub line: u64,
    pub input_tokens: u64,
    pub uncached_input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
    pub delta_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenCategoryBreakdown {
    pub key: String,
    pub label: String,
    pub tokens: u64,
    pub estimated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenTopItem {
    pub label: String,
    pub kind: String,
    pub line: u64,
    pub tokens: u64,
    pub estimated: bool,
}

#[derive(Debug, Clone)]
pub struct ThreadStatsJob {
    pub codex_home: PathBuf,
    pub thread_id: String,
}

pub fn compute_thread_stats(job: ThreadStatsJob) -> Result<Option<ThreadStatsResponse>> {
    let sessions_dir = job.codex_home.join("sessions");
    let Some(session_path) = resolve_session_file(&sessions_dir, &job.thread_id)? else {
        return Ok(None);
    };
    let contents = fs::read_to_string(&session_path)
        .with_context(|| format!("failed to read session log {}", session_path.display()))?;
    let mut stats = aggregate_session_jsonl(&job.thread_id, &session_path, &contents);
    stats.generated_at_ms = generated_now_ms();
    Ok(Some(stats))
}

pub fn resolve_session_file(sessions_dir: &Path, thread_id: &str) -> Result<Option<PathBuf>> {
    if !sessions_dir.is_dir() {
        return Ok(None);
    }

    let mut candidates = Vec::new();
    for year in read_dirs_sorted(sessions_dir)? {
        if !year.path().is_dir() {
            continue;
        }
        for month in read_dirs_sorted(&year.path())? {
            if !month.path().is_dir() {
                continue;
            }
            for day in read_dirs_sorted(&month.path())? {
                if !day.path().is_dir() {
                    continue;
                }
                for entry in read_dirs_sorted(&day.path())? {
                    let path = entry.path();
                    if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                        continue;
                    }
                    let file_name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
                    if !file_name.contains(thread_id) {
                        continue;
                    }
                    let modified = entry
                        .metadata()
                        .and_then(|metadata| metadata.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH);
                    candidates.push((modified, path));
                }
            }
        }
    }

    candidates.sort_by(|left, right| right.0.cmp(&left.0));
    Ok(candidates.into_iter().map(|(_, path)| path).next())
}

fn read_dirs_sorted(path: &Path) -> Result<Vec<fs::DirEntry>> {
    let mut entries = fs::read_dir(path)
        .with_context(|| format!("failed to scan {}", path.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("failed to scan entries in {}", path.display()))?;
    entries.sort_by_key(|entry| entry.path());
    Ok(entries)
}

pub fn aggregate_session_jsonl(
    thread_id: &str,
    session_path: &Path,
    contents: &str,
) -> ThreadStatsResponse {
    let mut warnings = Vec::new();
    let mut totals = TokenTotals::default();
    let mut estimates = TokenEstimates::default();
    let mut timeline = Vec::new();
    let mut top_items = Vec::new();
    let mut categories: BTreeMap<String, TokenCategoryBreakdown> = BTreeMap::new();
    let mut compaction_lines = BTreeSet::new();
    let mut last_total = 0;
    let mut seen_total_snapshots = BTreeSet::new();

    for (line_index, raw_line) in contents.lines().enumerate() {
        let line = (line_index + 1) as u64;
        if raw_line.trim().is_empty() {
            continue;
        }
        let value = match serde_json::from_str::<Value>(raw_line) {
            Ok(value) => value,
            Err(error) => {
                warnings.push(format!("line {line}: skipped malformed JSON ({error})"));
                continue;
            }
        };

        if line_mentions_compaction(&value) {
            compaction_lines.insert(line);
        }

        if let Some(usage) = token_count_usage(&value) {
            totals = usage.total;
            let snapshot_key = (
                usage.total.input_tokens,
                usage.total.output_tokens,
                usage.total.cached_input_tokens,
                usage.total.reasoning_output_tokens,
                usage.total.total_tokens,
            );
            if seen_total_snapshots.insert(snapshot_key) {
                let event_usage = usage.last.unwrap_or(usage.total);
                let delta_tokens = usage
                    .last
                    .map(event_usage_tokens)
                    .unwrap_or_else(|| usage.total.total_tokens.saturating_sub(last_total));
                last_total = usage.total.total_tokens;
                timeline.push(TokenTimelinePoint {
                    index: timeline.len() as u64 + 1,
                    line,
                    input_tokens: event_usage.input_tokens,
                    uncached_input_tokens: event_usage.uncached_input_tokens,
                    output_tokens: event_usage.output_tokens,
                    cached_input_tokens: event_usage.cached_input_tokens,
                    reasoning_output_tokens: event_usage.reasoning_output_tokens,
                    total_tokens: timeline
                        .last()
                        .map(|point: &TokenTimelinePoint| point.total_tokens)
                        .unwrap_or(0)
                        .saturating_add(delta_tokens),
                    delta_tokens,
                });
            }
            continue;
        }

        if let Some((kind, label, text)) = text_item(&value) {
            let tokens = estimate_tokens(&text);
            if tokens == 0 {
                continue;
            }
            match kind.as_str() {
                "user_message" => estimates.user_message_input_tokens += tokens,
                "tool_output" => estimates.tool_output_input_tokens += tokens,
                "tool_call" => estimates.tool_call_output_tokens += tokens,
                "skill_instruction" => estimates.skill_instruction_input_tokens += tokens,
                _ => {}
            }
            add_category(&mut categories, &kind, category_label(&kind), tokens, true);
            top_items.push(TokenTopItem {
                label,
                kind,
                line,
                tokens,
                estimated: true,
            });
        }
    }

    top_items.sort_by(|left, right| right.tokens.cmp(&left.tokens).then_with(|| left.line.cmp(&right.line)));
    top_items.truncate(12);

    if totals.total_tokens == 0 && !timeline.is_empty() {
        warnings.push("token timeline exists but final total was zero".to_string());
    }
    if estimates.user_message_input_tokens > totals.input_tokens && totals.input_tokens > 0 {
        warnings.push("estimated user-message tokens exceed reported input total; treating estimate as directional".to_string());
    }

    ThreadStatsResponse {
        thread_id: thread_id.to_string(),
        session_path: session_path.display().to_string(),
        generated_at_ms: 0,
        totals,
        estimates,
        compaction_count: compaction_lines.len() as u64,
        timeline,
        categories: categories.into_values().collect(),
        top_items,
        warnings,
    }
}

#[derive(Debug, Clone, Copy)]
struct TokenCountUsage {
    total: TokenTotals,
    last: Option<TokenTotals>,
}

fn token_count_usage(value: &Value) -> Option<TokenCountUsage> {
    let payload = value.get("payload")?;
    if payload.get("type").and_then(Value::as_str) != Some("token_count") {
        return None;
    }
    let info = payload.get("info")?;
    let total = token_totals_from_value(info.get("total_token_usage")?)?;
    let last = info.get("last_token_usage").and_then(token_totals_from_value);
    Some(TokenCountUsage { total, last })
}

fn token_totals_from_value(usage: &Value) -> Option<TokenTotals> {
    let input_tokens = usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
    let cached_input_tokens = usage.get("cached_input_tokens").and_then(Value::as_u64).unwrap_or(0);
    Some(TokenTotals {
        input_tokens: input_tokens,
        uncached_input_tokens: input_tokens.saturating_sub(cached_input_tokens),
        output_tokens: usage.get("output_tokens").and_then(Value::as_u64).unwrap_or(0),
        cached_input_tokens,
        reasoning_output_tokens: usage.get("reasoning_output_tokens").and_then(Value::as_u64).unwrap_or(0),
        total_tokens: usage.get("total_tokens").and_then(Value::as_u64).unwrap_or(0),
    })
}

fn event_usage_tokens(usage: TokenTotals) -> u64 {
    usage
        .uncached_input_tokens
        .saturating_add(usage.output_tokens)
        .saturating_add(usage.reasoning_output_tokens)
}

fn text_item(value: &Value) -> Option<(String, String, String)> {
    let payload = value.get("payload")?;
    let payload_type = payload.get("type").and_then(Value::as_str).unwrap_or_default();

    if payload_type == "user_message" {
        let text = payload.get("message").or_else(|| payload.get("text")).and_then(Value::as_str)?;
        return Some(("user_message".to_string(), "User message".to_string(), text.to_string()));
    }

    if value.get("type").and_then(Value::as_str) == Some("response_item") || payload_type == "response_item" {
        let item = payload.get("item").or_else(|| payload.get("response_item")).or(Some(payload))?;
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
        if item_type == "function_call_output" {
            let text = item.get("output").and_then(value_text)?;
            let label = item
                .get("call_id")
                .and_then(Value::as_str)
                .map(|id| format!("Tool output {id}"))
                .unwrap_or_else(|| "Tool output".to_string());
            return Some(("tool_output".to_string(), label, text));
        }
        if item_type == "function_call" {
            let text = item.get("arguments").and_then(value_text)?;
            let label = item
                .get("name")
                .and_then(Value::as_str)
                .map(|name| format!("Tool call {name}"))
                .unwrap_or_else(|| "Tool call".to_string());
            return Some(("tool_call".to_string(), label, text));
        }
        if item_type == "message" && item.get("role").and_then(Value::as_str) == Some("user") {
            let text = item.get("content").and_then(value_text)?;
            return Some(("user_message".to_string(), "User message".to_string(), text));
        }
        if item_type == "message" && item.get("role").and_then(Value::as_str) == Some("assistant") {
            let text = item.get("content").and_then(value_text)?;
            return Some(("assistant_message".to_string(), "Assistant message".to_string(), text));
        }
    }

    if payload_type == "session_meta" {
        let text = payload.get("instructions").and_then(Value::as_str)?;
        if text.contains("skills/") || text.contains("SKILL.md") {
            return Some(("skill_instruction".to_string(), "Skill instructions".to_string(), text.to_string()));
        }
    }

    None
}

fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let parts = items
                .iter()
                .filter_map(|item| {
                    if let Some(text) = item.get("text").and_then(Value::as_str) {
                        Some(text.to_string())
                    } else {
                        item.as_str().map(str::to_string)
                    }
                })
                .collect::<Vec<_>>();
            (!parts.is_empty()).then(|| parts.join("\n"))
        }
        Value::Object(_) => Some(value.to_string()),
        _ => None,
    }
}

fn line_mentions_compaction(value: &Value) -> bool {
    let payload_type = value
        .get("payload")
        .and_then(|payload| payload.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if payload_type.contains("compact") {
        return true;
    }
    let text = value.to_string().to_lowercase();
    text.contains("context compaction") || text.contains("compaction completed")
}

fn estimate_tokens(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    if chars == 0 { 0 } else { chars.div_ceil(4) }
}

fn add_category(
    categories: &mut BTreeMap<String, TokenCategoryBreakdown>,
    key: &str,
    label: &str,
    tokens: u64,
    estimated: bool,
) {
    if tokens == 0 {
        return;
    }
    let entry = categories.entry(key.to_string()).or_insert_with(|| TokenCategoryBreakdown {
        key: key.to_string(),
        label: label.to_string(),
        tokens: 0,
        estimated,
    });
    entry.tokens += tokens;
    entry.estimated = entry.estimated || estimated;
}

fn category_label(key: &str) -> &str {
    match key {
        "user_message" => "User messages",
        "tool_output" => "Tool outputs",
        "tool_call" => "Tool call inputs",
        "assistant_message" => "Assistant messages",
        "skill_instruction" => "Skill instructions",
        _ => "Other",
    }
}

fn generated_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

pub fn codex_home_from_state_root(state_root: &Path) -> Result<PathBuf> {
    state_root
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("state root has no parent: {}", state_root.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn aggregates_totals_estimates_compactions_and_timeline() {
        let jsonl = r#"{"type":"event_msg","payload":{"type":"user_message","message":"hello there user"}}
{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":20,"output_tokens":30,"reasoning_output_tokens":7,"total_tokens":130}}}}
{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":20,"output_tokens":30,"reasoning_output_tokens":7,"total_tokens":130}}}}
{"type":"response_item","payload":{"type":"response_item","item":{"type":"function_call","name":"shell","arguments":"{\"command\":\"cargo test\"}"}}}
{"type":"response_item","payload":{"type":"response_item","item":{"type":"function_call_output","call_id":"call_1","output":"lots of command output here"}}}
{"type":"event_msg","payload":{"type":"agent_message","message":"Context compaction completed for this thread."}}
{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":160,"cached_input_tokens":25,"output_tokens":55,"reasoning_output_tokens":12,"total_tokens":215}}}}
"#;
        let stats = aggregate_session_jsonl("thread-a", Path::new("/tmp/thread-a.jsonl"), jsonl);
        assert_eq!(stats.totals.input_tokens, 160);
        assert_eq!(stats.totals.uncached_input_tokens, 135);
        assert_eq!(stats.totals.output_tokens, 55);
        assert_eq!(stats.totals.cached_input_tokens, 25);
        assert_eq!(stats.totals.reasoning_output_tokens, 12);
        assert_eq!(stats.compaction_count, 1);
        assert_eq!(stats.timeline.len(), 2);
        assert_eq!(stats.timeline[1].delta_tokens, 85);
        assert_eq!(stats.timeline[1].input_tokens, 160);
        assert_eq!(stats.timeline[1].uncached_input_tokens, 135);
        assert!(stats.estimates.user_message_input_tokens > 0);
        assert!(stats.estimates.tool_output_input_tokens > 0);
        assert!(stats.estimates.tool_call_output_tokens > 0);
        assert!(stats.top_items.iter().any(|item| item.kind == "tool_output"));
        assert!(stats.categories.iter().all(|category| category.estimated));
        assert!(!stats.categories.iter().any(|category| category.key == "cached_input"));
    }

    #[test]
    fn malformed_lines_are_parser_warnings_not_hard_failures() {
        let jsonl = r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12}}}}
not-json
"#;
        let stats = aggregate_session_jsonl("thread-a", Path::new("/tmp/thread-a.jsonl"), jsonl);
        assert_eq!(stats.totals.total_tokens, 12);
        assert_eq!(stats.warnings.len(), 1);
    }

    #[test]
    fn top_level_response_items_are_counted_as_tool_attribution() {
        let jsonl = r#"{"type":"response_item","payload":{"type":"function_call","name":"shell_command","arguments":"{\"command\":\"sed -n '1,220p' scripts/zsh\",\"workdir\":\"/Users/robertsale/.codex\"}","call_id":"call_1"}}
{"type":"response_item","payload":{"type":"function_call_output","call_id":"call_1","output":"Exit code: 0\nWall time: 1 seconds\nOutput:\nlarge command output"}}
{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":500,"output_tokens":50,"reasoning_output_tokens":10,"total_tokens":1050},"last_token_usage":{"input_tokens":100,"cached_input_tokens":50,"output_tokens":5,"reasoning_output_tokens":1,"total_tokens":105}}}}
"#;
        let stats = aggregate_session_jsonl("thread-a", Path::new("/tmp/thread-a.jsonl"), jsonl);
        assert!(stats.estimates.tool_call_output_tokens > 0);
        assert!(stats.estimates.tool_output_input_tokens > 0);
        assert_eq!(stats.timeline[0].delta_tokens, 56);
        assert_eq!(stats.timeline[0].uncached_input_tokens, 50);
        assert!(stats.categories.iter().any(|category| category.key == "tool_call"));
        assert!(stats.categories.iter().any(|category| category.key == "tool_output"));
    }

    #[test]
    fn resolves_session_file_under_sessions_tree_only() {
        let temp = TempDir::new().expect("tempdir");
        let day = temp.path().join("sessions/2026/05/30");
        fs::create_dir_all(&day).expect("mkdir");
        let path = day.join("rollout-2026-05-30T00-00-00-thread-a.jsonl");
        fs::write(&path, "{}\n").expect("write");
        let resolved = resolve_session_file(&temp.path().join("sessions"), "thread-a")
            .expect("resolve")
            .expect("session");
        assert_eq!(resolved, path);
    }
}
