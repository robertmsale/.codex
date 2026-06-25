use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::db;
use crate::errors::RuntimeDomainError;

pub const DEFAULT_DIMENSIONS: usize = 2560;

#[derive(Debug, Clone)]
pub enum EmbeddingProviderKind {
    Disabled,
    Deterministic,
    LmStudio,
}

#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub provider: EmbeddingProviderKind,
    pub base_url: String,
    pub model: String,
    pub dimensions: usize,
    pub storage_type: String,
}

#[derive(Debug, Clone)]
pub struct EmbeddedText {
    pub vector: Vec<f32>,
    pub provider: String,
    pub model: String,
    pub dimensions: usize,
    pub storage_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelpResult {
    pub id: Uuid,
    pub title: String,
    pub summary: String,
    pub reason: String,
    pub similarity: f64,
    pub rank_score: f64,
    pub rank_reason: String,
    pub scope: String,
    pub project_key: Option<String>,
    pub source_hash: String,
}

#[derive(Debug, Clone)]
pub struct RememberCandidate {
    pub title: String,
    pub reason: String,
}

impl EmbeddingConfig {
    pub fn from_env() -> Result<Self> {
        let provider = match std::env::var("ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER")
            .unwrap_or_else(|_| "disabled".to_string())
            .as_str()
        {
            "disabled" => EmbeddingProviderKind::Disabled,
            "deterministic" => EmbeddingProviderKind::Deterministic,
            "lmstudio" => EmbeddingProviderKind::LmStudio,
            other => bail!("unsupported embedding provider: {other}"),
        };
        let base_url = std::env::var("ROBDEX_AGENT_RUNTIME_EMBEDDING_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:1234".to_string());
        let model = std::env::var("ROBDEX_AGENT_RUNTIME_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "qwen3-embedding-4b-dwq".to_string());
        let dimensions = std::env::var("ROBDEX_AGENT_RUNTIME_EMBEDDING_DIMENSIONS")
            .ok()
            .map(|raw| raw.parse::<usize>())
            .transpose()
            .context("ROBDEX_AGENT_RUNTIME_EMBEDDING_DIMENSIONS must be an integer")?
            .unwrap_or(DEFAULT_DIMENSIONS);
        if dimensions != DEFAULT_DIMENSIONS {
            bail!("this schema stores halfvec({DEFAULT_DIMENSIONS}); configured dimensions={dimensions}");
        }
        Ok(Self {
            provider,
            base_url,
            model,
            dimensions,
            storage_type: "halfvec".to_string(),
        })
    }

    pub fn enabled(&self) -> bool {
        !matches!(self.provider, EmbeddingProviderKind::Disabled)
    }

    pub async fn embed(&self, text: &str) -> Result<Option<EmbeddedText>> {
        match self.provider {
            EmbeddingProviderKind::Disabled => Ok(None),
            EmbeddingProviderKind::Deterministic => Ok(Some(EmbeddedText {
                vector: deterministic_embedding(text, self.dimensions),
                provider: "deterministic".to_string(),
                model: self.model.clone(),
                dimensions: self.dimensions,
                storage_type: self.storage_type.clone(),
            })),
            EmbeddingProviderKind::LmStudio => {
                let vector = lmstudio_embedding(self, text).await?;
                if vector.len() != self.dimensions {
                    bail!("embedding dimension mismatch: expected {} got {}", self.dimensions, vector.len());
                }
                Ok(Some(EmbeddedText {
                    vector,
                    provider: "lmstudio".to_string(),
                    model: self.model.clone(),
                    dimensions: self.dimensions,
                    storage_type: self.storage_type.clone(),
                }))
            }
        }
    }
}

pub fn source_hash(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}

pub fn command_fingerprint(source: &str) -> String {
    let mut tokens = Vec::new();
    for marker in ["cmd[", "fs.", "patch.", "workflow_memory."] {
        if source.contains(marker) {
            tokens.push(marker.trim_end_matches('.').trim_end_matches('[').to_string());
        }
    }
    if tokens.is_empty() {
        "plain".to_string()
    } else {
        tokens.join("+")
    }
}

pub async fn index_script(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    script_run_id: Uuid,
    source: &str,
) -> Result<()> {
    let config = EmbeddingConfig::from_env()?;
    let Some(embedding) = config.embed(source).await? else {
        return Ok(());
    };
    let project_key = db::session_project_key(pool, session_id).await?;
    sqlx::query(
        r#"
        INSERT INTO workflow_memory_script_embeddings (
            id, script_run_id, session_id, project_key, provider, model, dimensions,
            storage_type, source_hash, command_fingerprint, embedding
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11::halfvec)
        ON CONFLICT (script_run_id) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(script_run_id)
    .bind(session_id)
    .bind(project_key.as_deref())
    .bind(&embedding.provider)
    .bind(&embedding.model)
    .bind(embedding.dimensions as i32)
    .bind(&embedding.storage_type)
    .bind(source_hash(source))
    .bind(command_fingerprint(source))
    .bind(vector_literal(&embedding.vector))
    .execute(pool)
    .await?;
    db::append_event(
        pool,
        session_id,
        Some(turn_id),
        "workflow_memory",
        Some(script_run_id),
        "workflow_memory.script_indexed",
        Some("indexed"),
        json!({"provider": embedding.provider, "model": embedding.model, "dimensions": embedding.dimensions, "sourceHash": source_hash(source)}),
    )
    .await?;
    Ok(())
}

pub async fn help_results_for_latest_prior_script(
    pool: &PgPool,
    session_id: Uuid,
    current_script_run_id: Uuid,
    limit: i64,
) -> Result<Vec<HelpResult>> {
    let Some((prior_source, project_key)) = latest_prior_failed_non_memory_script(pool, session_id, current_script_run_id).await? else {
        return Ok(Vec::new());
    };
    search(pool, session_id, project_key.as_deref(), &prior_source, limit).await
}

pub async fn promote_project_memory(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Uuid,
    script_run_id: Uuid,
    source: &str,
    title: &str,
    reason: &str,
) -> Result<Uuid> {
    let config = EmbeddingConfig::from_env()?;
    let Some(embedding) = config.embed(source).await? else {
        bail!("workflow memory embedding provider is disabled");
    };
    let project_key = db::session_project_key(pool, session_id).await?;
    let source_hash = source_hash(source);
    let fingerprint = command_fingerprint(source);
    if let Some(existing) = exact_duplicate(pool, project_key.as_deref(), &source_hash).await? {
        insert_memory_event(pool, session_id, Some(turn_id), Some(script_run_id), Some(existing), "workflow_memory.duplicate_collapsed", json!({"sourceHash": source_hash, "reason": "exact source hash duplicate"})).await?;
        return Ok(existing);
    }
    if let Some(existing) = near_duplicate(pool, project_key.as_deref(), &fingerprint, &embedding.vector).await? {
        insert_memory_event(pool, session_id, Some(turn_id), Some(script_run_id), Some(existing), "workflow_memory.duplicate_collapsed", json!({"sourceHash": source_hash, "commandFingerprint": fingerprint, "reason": "near embedding match with same command fingerprint"})).await?;
        return Ok(existing);
    }
    let memory_id = Uuid::new_v4();
    let summary = summarize_source(source);
    sqlx::query(
        r#"
        INSERT INTO workflow_memories (
            id, script_run_id, session_id, scope_type, project_key, title, reason, summary,
            provider, model, dimensions, storage_type, source_hash, command_fingerprint, embedding
        )
        VALUES ($1,$2,$3,'project',$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14::halfvec)
        "#,
    )
    .bind(memory_id)
    .bind(script_run_id)
    .bind(session_id)
    .bind(project_key.as_deref())
    .bind(title)
    .bind(reason)
    .bind(&summary)
    .bind(&embedding.provider)
    .bind(&embedding.model)
    .bind(embedding.dimensions as i32)
    .bind(&embedding.storage_type)
    .bind(&source_hash)
    .bind(&fingerprint)
    .bind(vector_literal(&embedding.vector))
    .execute(pool)
    .await?;
    insert_memory_event(pool, session_id, Some(turn_id), Some(script_run_id), Some(memory_id), "workflow_memory.promoted", json!({"title": title, "reason": reason, "scope": "project", "projectKey": project_key, "sourceHash": source_hash})).await?;
    Ok(memory_id)
}

pub async fn insert_memory_event(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    script_run_id: Option<Uuid>,
    memory_id: Option<Uuid>,
    event_type: &str,
    payload: Value,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO workflow_memory_events (id, session_id, turn_id, script_run_id, memory_id, event_type, payload)
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(turn_id)
    .bind(script_run_id)
    .bind(memory_id)
    .bind(event_type)
    .bind(&payload)
    .execute(pool)
    .await?;
    db::append_event(pool, session_id, turn_id, "workflow_memory", memory_id, event_type, Some("recorded"), payload).await?;
    Ok(())
}

pub async fn record_provider_failure(
    pool: &PgPool,
    session_id: Uuid,
    turn_id: Option<Uuid>,
    script_run_id: Option<Uuid>,
    event_type: &str,
    error: &str,
    context: Value,
) -> Result<()> {
    insert_memory_event(
        pool,
        session_id,
        turn_id,
        script_run_id,
        None,
        event_type,
        json!({
            "error": error,
            "context": context,
        }),
    )
    .await
}

pub async fn memory_visible_to_session(pool: &PgPool, session_id: Uuid, memory_id: Uuid) -> Result<bool> {
    let project_key = db::session_project_key(pool, session_id).await?;
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*) FROM workflow_memories
        WHERE id=$1
          AND (scope_type='global' OR (scope_type='project' AND COALESCE(project_key,'')=COALESCE($2,'')))
        "#,
    )
    .bind(memory_id)
    .bind(project_key.as_deref())
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn memory_exists(pool: &PgPool, memory_id: Uuid) -> Result<bool> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM workflow_memories WHERE id=$1)")
        .bind(memory_id)
        .fetch_one(pool)
        .await?;
    Ok(exists)
}

pub async fn list_visible_memories(pool: &PgPool, session_id: Uuid) -> Result<Vec<Value>> {
    let project_key = db::session_project_key(pool, session_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT wm.id, wm.session_id, wm.script_run_id, wm.scope_type, wm.project_key, wm.title, wm.reason, wm.summary,
               wm.provider, wm.model, wm.dimensions, wm.storage_type, wm.source_hash, wm.command_fingerprint,
               wm.helpful_score, wm.promoted_at, sr.source
        FROM workflow_memories wm
        LEFT JOIN script_runs sr ON sr.id = wm.script_run_id
        WHERE wm.scope_type='global' OR (wm.scope_type='project' AND COALESCE(wm.project_key,'')=COALESCE($1,''))
        ORDER BY wm.promoted_at DESC
        LIMIT 200
        "#,
    )
    .bind(project_key.as_deref())
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(memory_row_to_json).collect())
}

pub async fn show_visible_memory(pool: &PgPool, session_id: Uuid, memory_id: Uuid) -> Result<Value> {
    if !memory_visible_to_session(pool, session_id, memory_id).await? {
        return Err(RuntimeDomainError::forbidden(format!("workflow memory is not visible to session: {memory_id}"), serde_json::json!({"entity":"workflow_memory","id": memory_id, "sessionId": session_id})).into());
    }
    let row = sqlx::query(
        r#"
        SELECT wm.id, wm.session_id, wm.script_run_id, wm.scope_type, wm.project_key, wm.title, wm.reason, wm.summary,
               wm.provider, wm.model, wm.dimensions, wm.storage_type, wm.source_hash, wm.command_fingerprint,
               wm.helpful_score, wm.promoted_at, sr.source
        FROM workflow_memories wm
        LEFT JOIN script_runs sr ON sr.id = wm.script_run_id
        WHERE wm.id=$1
        "#,
    )
    .bind(memory_id)
    .fetch_one(pool)
    .await?;
    Ok(memory_row_to_json(row))
}

pub async fn list_memory_events(pool: &PgPool, session_id: Uuid, memory_id: Uuid) -> Result<Vec<Value>> {
    if !memory_visible_to_session(pool, session_id, memory_id).await? {
        return Err(RuntimeDomainError::forbidden(format!("workflow memory is not visible to session: {memory_id}"), serde_json::json!({"entity":"workflow_memory","id": memory_id, "sessionId": session_id})).into());
    }
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, turn_id, script_run_id, memory_id, event_type, payload, created_at
        FROM workflow_memory_events
        WHERE memory_id=$1
        ORDER BY created_at ASC
        "#,
    )
    .bind(memory_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(memory_event_row_to_json).collect())
}

pub async fn record_visible_feedback(
    pool: &PgPool,
    session_id: Uuid,
    memory_id: Uuid,
    feedback: &str,
    payload: Value,
) -> Result<()> {
    if !memory_visible_to_session(pool, session_id, memory_id).await? {
        return Err(RuntimeDomainError::forbidden(format!("workflow memory is not visible to session: {memory_id}"), serde_json::json!({"entity":"workflow_memory","id": memory_id, "sessionId": session_id})).into());
    }
    let event_type = match feedback {
        "attempted" => "workflow_memory.mark_attempted",
        "notHelpful" => "workflow_memory.mark_not_helpful",
        "helpful" => "workflow_memory.helpful",
        other => return Err(RuntimeDomainError::validation_failed(format!("unsupported workflow memory feedback: {other}")).into()),
    };
    insert_memory_event(pool, session_id, None, None, Some(memory_id), event_type, payload).await
}

fn memory_row_to_json(row: sqlx::postgres::PgRow) -> Value {
    let source: Option<String> = row.try_get("source").ok();
    let source_preview = source
        .as_deref()
        .map(|source| {
            let trimmed = source.trim();
            if trimmed.len() <= 900 { trimmed.to_string() } else { format!("{}…", trimmed.chars().take(900).collect::<String>()) }
        })
        .unwrap_or_default();
    json!({
        "id": row.get::<Uuid, _>("id"),
        "sessionId": row.get::<Uuid, _>("session_id"),
        "sourceScriptRunId": row.try_get::<Uuid, _>("script_run_id").ok(),
        "scopeType": row.get::<String, _>("scope_type"),
        "projectKey": row.get::<Option<String>, _>("project_key"),
        "title": row.get::<String, _>("title"),
        "reason": row.get::<String, _>("reason"),
        "summary": row.get::<String, _>("summary"),
        "provider": row.try_get::<String, _>("provider").ok(),
        "model": row.try_get::<String, _>("model").ok(),
        "dimensions": row.try_get::<i32, _>("dimensions").ok(),
        "storageType": row.try_get::<String, _>("storage_type").ok(),
        "sourceHash": row.try_get::<String, _>("source_hash").ok(),
        "commandFingerprint": row.try_get::<String, _>("command_fingerprint").ok(),
        "sourcePreview": source_preview,
        "sourceStarlark": source,
        "helpfulScore": row.get::<f64, _>("helpful_score"),
        "promotedAt": row.get::<chrono::DateTime<chrono::Utc>, _>("promoted_at"),
    })
}

fn memory_event_row_to_json(row: sqlx::postgres::PgRow) -> Value {
    json!({
        "id": row.get::<Uuid, _>("id"),
        "sessionId": row.get::<Uuid, _>("session_id"),
        "turnId": row.get::<Option<Uuid>, _>("turn_id"),
        "scriptRunId": row.get::<Option<Uuid>, _>("script_run_id"),
        "memoryId": row.get::<Option<Uuid>, _>("memory_id"),
        "eventType": row.get::<String, _>("event_type"),
        "payload": row.get::<Value, _>("payload"),
        "createdAt": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
    })
}

async fn latest_prior_failed_non_memory_script(
    pool: &PgPool,
    session_id: Uuid,
    current_script_run_id: Uuid,
) -> Result<Option<(String, Option<String>)>> {
    let row = sqlx::query(
        r#"
        SELECT sr.source, s.project_key
        FROM script_runs sr
        JOIN tool_calls tc ON tc.id = sr.tool_call_id
        JOIN turns t ON t.id = tc.turn_id
        JOIN sessions s ON s.id = t.session_id
        WHERE t.session_id=$1
          AND sr.id <> $2
          AND sr.status = 'failed'
          AND sr.source NOT LIKE '%workflow_memory.%'
        ORDER BY COALESCE(sr.completed_at, sr.started_at) DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .bind(current_script_run_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| (row.get("source"), row.get("project_key"))))
}

async fn search(pool: &PgPool, session_id: Uuid, project_key: Option<&str>, query: &str, limit: i64) -> Result<Vec<HelpResult>> {
    let config = EmbeddingConfig::from_env()?;
    let Some(embedding) = config.embed(query).await? else {
        return Ok(Vec::new());
    };
    let rows = sqlx::query(
        r#"
        SELECT wm.id, wm.title, wm.summary, wm.reason, wm.scope_type, wm.project_key, wm.source_hash,
               (wm.embedding <=> $1::halfvec) AS distance,
               COALESCE(sum(CASE
                   WHEN e.event_type='workflow_memory.mark_not_helpful' THEN -0.20
                   WHEN e.event_type='workflow_memory.mark_attempted' THEN -0.05
                   WHEN e.event_type='workflow_memory.helpful' THEN 0.25
                   ELSE 0
               END), 0)::float8 AS event_adjustment,
               EXTRACT(EPOCH FROM (now() - wm.promoted_at))::float8 AS age_seconds
        FROM workflow_memories wm
        LEFT JOIN workflow_memory_events e ON e.memory_id = wm.id
        WHERE wm.scope_type='global' OR (wm.scope_type='project' AND COALESCE(wm.project_key,'') = COALESCE($2,''))
        GROUP BY wm.id
        ORDER BY (1 - (wm.embedding <=> $1::halfvec)) + COALESCE(sum(CASE
                   WHEN e.event_type='workflow_memory.mark_not_helpful' THEN -0.20
                   WHEN e.event_type='workflow_memory.mark_attempted' THEN -0.05
                   WHEN e.event_type='workflow_memory.helpful' THEN 0.25
                   ELSE 0
               END), 0)::float8 - LEAST(EXTRACT(EPOCH FROM (now() - wm.promoted_at))::float8 / 6048000.0, 0.1) DESC
        LIMIT $3
        "#,
    )
    .bind(vector_literal(&embedding.vector))
    .bind(project_key)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let results = rows
        .into_iter()
        .map(|row| {
            let distance: f64 = row.get("distance");
            let event_adjustment: f64 = row.get("event_adjustment");
            let age_seconds: f64 = row.get("age_seconds");
            let similarity = 1.0 - distance;
            let recency_penalty = (age_seconds / 6_048_000.0).min(0.1);
            HelpResult {
                id: row.get("id"),
                title: row.get("title"),
                summary: row.get("summary"),
                reason: row.get("reason"),
                similarity,
                rank_score: similarity + event_adjustment - recency_penalty,
                rank_reason: format!("similarity={similarity:.4}; eventAdjustment={event_adjustment:.2}; recencyPenalty={recency_penalty:.4}; project/global scope eligible"),
                scope: row.get("scope_type"),
                project_key: row.get("project_key"),
                source_hash: row.get("source_hash"),
            }
        })
        .collect::<Vec<_>>();
    let _ = session_id;
    Ok(results)
}

async fn exact_duplicate(pool: &PgPool, project_key: Option<&str>, source_hash: &str) -> Result<Option<Uuid>> {
    let row = sqlx::query(
        "SELECT id FROM workflow_memories WHERE scope_type='project' AND COALESCE(project_key,'')=COALESCE($1,'') AND source_hash=$2 LIMIT 1",
    )
    .bind(project_key)
    .bind(source_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| row.get("id")))
}

async fn near_duplicate(pool: &PgPool, project_key: Option<&str>, fingerprint: &str, vector: &[f32]) -> Result<Option<Uuid>> {
    let row = sqlx::query(
        r#"
        SELECT id FROM workflow_memories
        WHERE scope_type='project'
          AND COALESCE(project_key,'')=COALESCE($1,'')
          AND command_fingerprint=$2
          AND (embedding <=> $3::halfvec) < 0.02
        ORDER BY embedding <=> $3::halfvec
        LIMIT 1
        "#,
    )
    .bind(project_key)
    .bind(fingerprint)
    .bind(vector_literal(vector))
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| row.get("id")))
}

fn vector_literal(vector: &[f32]) -> String {
    let values = vector.iter().map(|value| format!("{value:.7}")).collect::<Vec<_>>().join(",");
    format!("[{values}]")
}

fn deterministic_embedding(text: &str, dimensions: usize) -> Vec<f32> {
    let mut vector = vec![0f32; dimensions];
    for token in text.split(|ch: char| !ch.is_ascii_alphanumeric()).filter(|token| !token.is_empty()) {
        let digest = Sha256::digest(token.to_ascii_lowercase().as_bytes());
        let idx = u64::from_le_bytes(digest[0..8].try_into().unwrap()) as usize % dimensions;
        vector[idx] += 1.0;
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

async fn lmstudio_embedding(config: &EmbeddingConfig, text: &str) -> Result<Vec<f32>> {
    let base = config.base_url.trim_end_matches('/');
    let url = if base.ends_with("/v1") {
        format!("{base}/embeddings")
    } else {
        format!("{base}/v1/embeddings")
    };
    let response: Value = Client::new()
        .post(url)
        .json(&json!({"model": config.model, "input": text}))
        .send()
        .await
        .context("LM Studio embedding request failed")?
        .error_for_status()
        .context("LM Studio embedding endpoint returned an error")?
        .json()
        .await
        .context("LM Studio embedding response was not JSON")?;
    let embedding = response
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("embedding"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("LM Studio embedding response missing data[0].embedding"))?;
    embedding
        .iter()
        .map(|value| value.as_f64().map(|value| value as f32).ok_or_else(|| anyhow::anyhow!("LM Studio embedding vector contains a non-number")))
        .collect()
}

fn summarize_source(source: &str) -> String {
    let compact = source.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(" ");
    if compact.len() <= 480 {
        compact
    } else {
        format!("{}…", &compact[..480])
    }
}

#[allow(dead_code)]
pub fn lmstudio_validation_curl() -> String {
    "curl http://localhost:1234/v1/embeddings -H \"Content-Type: application/json\" -d '{\"model\":\"qwen3-embedding-4b-dwq\",\"input\":\"workflow memory validation input\"}'".to_string()
}
