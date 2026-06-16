use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub const DEFAULT_VISIBLE_BYTE_LIMIT: usize = 12_000;
pub const DEFAULT_VISIBLE_LINE_LIMIT: usize = 400;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputArtifactEnvelope {
    pub artifact_id: Uuid,
    pub stream: String,
    pub byte_count: usize,
    pub line_count: usize,
    pub estimated_tokens: usize,
    pub truncated: bool,
    pub omitted_bytes: usize,
    pub preview: String,
    pub tail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputRetrievalPacket {
    pub artifact_id: Uuid,
    pub mode: String,
    pub stream: String,
    pub byte_count: usize,
    pub line_count: usize,
    pub estimated_tokens: usize,
    pub returned_bytes: usize,
    pub returned_lines: usize,
    pub omitted_bytes: usize,
    pub omitted_lines: usize,
    pub truncated: bool,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub matches: Option<usize>,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct NewOutputArtifact<'a> {
    pub id: Uuid,
    pub session_id: Uuid,
    pub turn_id: Option<Uuid>,
    pub tool_call_id: Option<Uuid>,
    pub script_run_id: Option<Uuid>,
    pub command_run_id: Option<Uuid>,
    pub process_id: Option<Uuid>,
    pub source_type: &'a str,
    pub stream: &'a str,
    pub content: &'a str,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
struct StoredArtifact {
    id: Uuid,
    stream: String,
    content: String,
    byte_count: usize,
    line_count: usize,
}

pub async fn store(pool: &PgPool, artifact: NewOutputArtifact<'_>) -> Result<OutputArtifactEnvelope> {
    let byte_count = artifact.content.len();
    let line_count = line_count(artifact.content);
    sqlx::query(
        r#"
        INSERT INTO execution_output_artifacts (
            id, session_id, turn_id, tool_call_id, script_run_id, command_run_id, process_id,
            source_type, stream, content, byte_count, line_count, metadata
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
        "#,
    )
    .bind(artifact.id)
    .bind(artifact.session_id)
    .bind(artifact.turn_id)
    .bind(artifact.tool_call_id)
    .bind(artifact.script_run_id)
    .bind(artifact.command_run_id)
    .bind(artifact.process_id)
    .bind(artifact.source_type)
    .bind(artifact.stream)
    .bind(artifact.content)
    .bind(byte_count as i64)
    .bind(line_count as i64)
    .bind(artifact.metadata)
    .execute(pool)
    .await?;
    Ok(envelope_for(artifact.id, artifact.stream, artifact.content))
}

pub async fn last_artifact_id(pool: &PgPool, session_id: Uuid) -> Result<Option<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM execution_output_artifacts WHERE session_id=$1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn retrieve(pool: &PgPool, session_id: Uuid, artifact_id: Uuid, mode: &str, lines: Option<usize>, start_line: Option<usize>, end_line: Option<usize>, pattern: Option<&str>, context: Option<usize>) -> Result<OutputRetrievalPacket> {
    let artifact = load(pool, session_id, artifact_id).await?;
    let max_lines = lines.unwrap_or(DEFAULT_VISIBLE_LINE_LIMIT).min(DEFAULT_VISIBLE_LINE_LIMIT);
    let packet = match mode {
        "head" => build_packet(&artifact, mode, 0, max_lines, None),
        "tail" => {
            let start = artifact.line_count.saturating_sub(max_lines);
            build_packet(&artifact, mode, start, artifact.line_count, None)
        }
        "slice" => {
            let start = start_line.unwrap_or(1).saturating_sub(1);
            let requested_end = end_line.unwrap_or(start + max_lines).max(start);
            build_packet(&artifact, mode, start, requested_end.min(start + max_lines), None)
        }
        "search" => {
            let Some(pattern) = pattern else {
                bail!("outputs.search requires a pattern");
            };
            build_search_packet(&artifact, pattern, context.unwrap_or(20).min(50), max_lines)
        }
        "stats" => OutputRetrievalPacket {
            artifact_id: artifact.id,
            mode: mode.to_string(),
            stream: artifact.stream,
            byte_count: artifact.byte_count,
            line_count: artifact.line_count,
            estimated_tokens: artifact.byte_count / 4,
            returned_bytes: 0,
            returned_lines: 0,
            omitted_bytes: artifact.byte_count,
            omitted_lines: artifact.line_count,
            truncated: false,
            start_line: None,
            end_line: None,
            matches: None,
            content: String::new(),
        },
        other => bail!("unsupported output retrieval mode: {other}"),
    };
    Ok(packet)
}

pub fn envelope_for(id: Uuid, stream: &str, content: &str) -> OutputArtifactEnvelope {
    let byte_count = content.len();
    let line_count = line_count(content);
    let (preview, preview_truncated) = bounded_head(content, 40, DEFAULT_VISIBLE_BYTE_LIMIT / 2);
    let (tail, tail_truncated) = bounded_tail(content, 80, DEFAULT_VISIBLE_BYTE_LIMIT / 2);
    OutputArtifactEnvelope {
        artifact_id: id,
        stream: stream.to_string(),
        byte_count,
        line_count,
        estimated_tokens: byte_count / 4,
        truncated: preview_truncated || tail_truncated || byte_count > DEFAULT_VISIBLE_BYTE_LIMIT,
        omitted_bytes: byte_count.saturating_sub(preview.len() + tail.len()),
        preview,
        tail,
    }
}

async fn load(pool: &PgPool, session_id: Uuid, artifact_id: Uuid) -> Result<StoredArtifact> {
    let row = sqlx::query("SELECT id, stream, content, byte_count, line_count FROM execution_output_artifacts WHERE id=$1 AND session_id=$2")
        .bind(artifact_id)
        .bind(session_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("output artifact not found for current session"))?;
    Ok(StoredArtifact {
        id: row.get("id"),
        stream: row.get("stream"),
        content: row.get("content"),
        byte_count: row.get::<i64, _>("byte_count") as usize,
        line_count: row.get::<i64, _>("line_count") as usize,
    })
}

fn build_packet(artifact: &StoredArtifact, mode: &str, start: usize, end: usize, matches: Option<usize>) -> OutputRetrievalPacket {
    let all: Vec<&str> = artifact.content.lines().collect();
    let start = start.min(all.len());
    let end = end.min(all.len()).max(start);
    let mut content = all[start..end].join("\n");
    let before = content.len();
    let (bounded, byte_truncated) = truncate_text(&content, DEFAULT_VISIBLE_BYTE_LIMIT);
    content = bounded;
    let returned_lines = line_count(&content);
    let requested_lines = end.saturating_sub(start);
    let omitted_bytes = artifact.byte_count.saturating_sub(content.len());
    let omitted_lines = artifact.line_count.saturating_sub(returned_lines);
    OutputRetrievalPacket {
        artifact_id: artifact.id,
        mode: mode.to_string(),
        stream: artifact.stream.clone(),
        byte_count: artifact.byte_count,
        line_count: artifact.line_count,
        estimated_tokens: artifact.byte_count / 4,
        returned_bytes: content.len(),
        returned_lines,
        omitted_bytes,
        omitted_lines,
        truncated: byte_truncated || requested_lines > returned_lines || omitted_bytes > 0 || omitted_lines > 0,
        start_line: Some(start + 1),
        end_line: Some(start + returned_lines),
        matches,
        content: if before > DEFAULT_VISIBLE_BYTE_LIMIT { content } else { content },
    }
}

fn build_search_packet(artifact: &StoredArtifact, pattern: &str, context: usize, max_lines: usize) -> OutputRetrievalPacket {
    let lines: Vec<&str> = artifact.content.lines().collect();
    let mut selected = Vec::new();
    let mut matched = 0usize;
    for (idx, line) in lines.iter().enumerate() {
        if !line.contains(pattern) {
            continue;
        }
        matched += 1;
        let start = idx.saturating_sub(context);
        let end = (idx + context + 1).min(lines.len());
        for line in lines.iter().take(end).skip(start) {
            if selected.len() >= max_lines {
                break;
            }
            selected.push(*line);
        }
        if selected.len() >= max_lines {
            break;
        }
    }
    let mut content = selected.join("\n");
    let (bounded, byte_truncated) = truncate_text(&content, DEFAULT_VISIBLE_BYTE_LIMIT);
    content = bounded;
    let returned_lines = line_count(&content);
    let omitted_bytes = artifact.byte_count.saturating_sub(content.len());
    let omitted_lines = artifact.line_count.saturating_sub(returned_lines);
    OutputRetrievalPacket {
        artifact_id: artifact.id,
        mode: "search".to_string(),
        stream: artifact.stream.clone(),
        byte_count: artifact.byte_count,
        line_count: artifact.line_count,
        estimated_tokens: artifact.byte_count / 4,
        returned_bytes: content.len(),
        returned_lines,
        omitted_bytes,
        omitted_lines,
        truncated: byte_truncated || matched > 0 && selected.len() >= max_lines || omitted_bytes > 0 || omitted_lines > 0,
        start_line: None,
        end_line: None,
        matches: Some(matched),
        content,
    }
}

fn bounded_head(content: &str, max_lines: usize, max_bytes: usize) -> (String, bool) {
    let lines = content.lines().take(max_lines).collect::<Vec<_>>().join("\n");
    truncate_text(&lines, max_bytes)
}

fn bounded_tail(content: &str, max_lines: usize, max_bytes: usize) -> (String, bool) {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    truncate_text(&lines[start..].join("\n"), max_bytes)
}

fn line_count(content: &str) -> usize {
    if content.is_empty() {
        0
    } else {
        content.lines().count()
    }
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

pub fn retrieval_json(packet: OutputRetrievalPacket) -> String {
    serde_json::to_string(&json!(packet)).unwrap_or_else(|error| json!({"error": error.to_string()}).to_string())
}
