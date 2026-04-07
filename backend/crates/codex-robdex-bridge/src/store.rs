use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, Result};
use serde_json::Value;
use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};

use crate::models::{RobdexChatMessage, ThreadCachePayload, ThreadContextWindowStatus};

#[derive(Debug, Clone)]
pub struct RobdexBridgeStore {
    pool: SqlitePool,
}

impl RobdexBridgeStore {
    pub async fn connect(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let url = format!("sqlite:{}?mode=rwc", path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .with_context(|| format!("failed to open {}", path.display()))?;

        let store = Self { pool };
        store.initialize().await?;
        Ok(store)
    }

    async fn initialize(&self) -> Result<()> {
        sqlx::query("PRAGMA journal_mode = WAL;").execute(&self.pool).await?;
        sqlx::query("PRAGMA synchronous = NORMAL;").execute(&self.pool).await?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS documents (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL,
              updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS thread_messages (
              thread_id TEXT NOT NULL,
              ordinal INTEGER NOT NULL,
              payload TEXT NOT NULL,
              PRIMARY KEY (thread_id, ordinal)
            );

            CREATE TABLE IF NOT EXISTS thread_context_window_status (
              thread_id TEXT PRIMARY KEY,
              payload TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS running_threads (
              thread_id TEXT PRIMARY KEY
            );

            CREATE TABLE IF NOT EXISTS metadata (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_state_document(&self, fallback: Value) -> Result<Value> {
        let row = sqlx::query("SELECT value FROM documents WHERE key = ?")
            .bind("state")
            .fetch_optional(&self.pool)
            .await?;
        Ok(match row {
            Some(row) => serde_json::from_str(row.try_get::<&str, _>("value")?).unwrap_or(fallback),
            None => fallback,
        })
    }

    pub async fn save_state_document(&self, value: &Value) -> Result<()> {
        let encoded = serde_json::to_string(value)?;
        sqlx::query(
            r#"
            INSERT INTO documents (key, value, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at = excluded.updated_at
            "#,
        )
        .bind("state")
        .bind(encoded)
        .bind(unix_now() as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_thread_cache_payload(&self, fallback: ThreadCachePayload) -> Result<ThreadCachePayload> {
        let mut payload = ThreadCachePayload {
            updated_at: fallback.updated_at,
            ..ThreadCachePayload::default()
        };

        let message_rows = sqlx::query(
            r#"
            SELECT thread_id, ordinal, payload
            FROM thread_messages
            ORDER BY thread_id ASC, ordinal ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        for row in message_rows {
            let thread_id: String = row.try_get("thread_id")?;
            let raw: &str = row.try_get("payload")?;
            let message = decode_thread_message_payload(&thread_id, raw)
                .with_context(|| format!("invalid chat message payload for thread {thread_id}"))?;
            payload
                .message_cache_by_thread_id
                .entry(thread_id)
                .or_default()
                .push(message);
        }

        let status_rows = sqlx::query(
            r#"
            SELECT thread_id, payload
            FROM thread_context_window_status
            ORDER BY thread_id ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        for row in status_rows {
            let thread_id: String = row.try_get("thread_id")?;
            let raw: &str = row.try_get("payload")?;
            let status = serde_json::from_str::<ThreadContextWindowStatus>(raw)
                .with_context(|| format!("invalid context window payload for thread {thread_id}"))?;
            payload.context_window_status_by_thread_id.insert(thread_id, status);
        }

        let running_rows = sqlx::query("SELECT thread_id FROM running_threads ORDER BY thread_id ASC")
            .fetch_all(&self.pool)
            .await?;
        payload.running_thread_ids = running_rows
            .into_iter()
            .map(|row| row.try_get::<String, _>("thread_id"))
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let updated_at = sqlx::query("SELECT value FROM metadata WHERE key = ?")
            .bind("thread_cache_updated_at")
            .fetch_optional(&self.pool)
            .await?;
        if let Some(row) = updated_at {
            let raw: String = row.try_get("value")?;
            payload.updated_at = raw.parse::<u64>().ok().or(fallback.updated_at);
        }

        Ok(payload)
    }

    pub async fn save_thread_cache_payload(&self, payload: &ThreadCachePayload) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM thread_messages").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM thread_context_window_status").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM running_threads").execute(&mut *tx).await?;

        for (thread_id, messages) in &payload.message_cache_by_thread_id {
            for (ordinal, message) in messages.iter().enumerate() {
                sqlx::query("INSERT INTO thread_messages (thread_id, ordinal, payload) VALUES (?, ?, ?)")
                    .bind(thread_id)
                    .bind(ordinal as i64)
                    .bind(serde_json::to_string(message)?)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        for (thread_id, status) in &payload.context_window_status_by_thread_id {
            sqlx::query("INSERT INTO thread_context_window_status (thread_id, payload) VALUES (?, ?)")
                .bind(thread_id)
                .bind(serde_json::to_string(status)?)
                .execute(&mut *tx)
                .await?;
        }

        for thread_id in &payload.running_thread_ids {
            sqlx::query("INSERT INTO running_threads (thread_id) VALUES (?)")
                .bind(thread_id)
                .execute(&mut *tx)
                .await?;
        }

        sqlx::query(
            r#"
            INSERT INTO metadata (key, value)
            VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
        )
        .bind("thread_cache_updated_at")
        .bind(payload.updated_at.unwrap_or_else(unix_now).to_string())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn load_thread_messages(&self, thread_id: &str) -> Result<Vec<RobdexChatMessage>> {
        let rows = sqlx::query(
            r#"
            SELECT payload
            FROM thread_messages
            WHERE thread_id = ?
            ORDER BY ordinal ASC
            "#,
        )
        .bind(thread_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                let raw = row.try_get::<&str, _>("payload")?;
                decode_thread_message_payload(thread_id, raw).context("invalid thread message row")
            })
            .collect()
    }

    pub async fn save_thread_messages(&self, thread_id: &str, messages: &[RobdexChatMessage]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM thread_messages WHERE thread_id = ?")
            .bind(thread_id)
            .execute(&mut *tx)
            .await?;
        for (ordinal, message) in messages.iter().enumerate() {
            sqlx::query("INSERT INTO thread_messages (thread_id, ordinal, payload) VALUES (?, ?, ?)")
                .bind(thread_id)
                .bind(ordinal as i64)
                .bind(serde_json::to_string(message)?)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn save_thread_cache_delta(
        &self,
        payload: &ThreadCachePayload,
        changed_thread_ids: &[String],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for thread_id in changed_thread_ids {
            sqlx::query("DELETE FROM thread_messages WHERE thread_id = ?")
                .bind(thread_id)
                .execute(&mut *tx)
                .await?;
            if let Some(messages) = payload.message_cache_by_thread_id.get(thread_id) {
                for (ordinal, message) in messages.iter().enumerate() {
                    sqlx::query("INSERT INTO thread_messages (thread_id, ordinal, payload) VALUES (?, ?, ?)")
                        .bind(thread_id)
                        .bind(ordinal as i64)
                        .bind(serde_json::to_string(message)?)
                        .execute(&mut *tx)
                        .await?;
                }
            }

            sqlx::query("DELETE FROM thread_context_window_status WHERE thread_id = ?")
                .bind(thread_id)
                .execute(&mut *tx)
                .await?;
            if let Some(status) = payload.context_window_status_by_thread_id.get(thread_id) {
                sqlx::query("INSERT INTO thread_context_window_status (thread_id, payload) VALUES (?, ?)")
                    .bind(thread_id)
                    .bind(serde_json::to_string(status)?)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        sqlx::query("DELETE FROM running_threads").execute(&mut *tx).await?;
        for thread_id in &payload.running_thread_ids {
            sqlx::query("INSERT INTO running_threads (thread_id) VALUES (?)")
                .bind(thread_id)
                .execute(&mut *tx)
                .await?;
        }

        sqlx::query(
            r#"
            INSERT INTO metadata (key, value)
            VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
        )
        .bind("thread_cache_updated_at")
        .bind(payload.updated_at.unwrap_or_else(unix_now).to_string())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_thread_cache(&self, thread_id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for query in [
            "DELETE FROM thread_messages WHERE thread_id = ?",
            "DELETE FROM thread_context_window_status WHERE thread_id = ?",
            "DELETE FROM running_threads WHERE thread_id = ?",
        ] {
            sqlx::query(query).bind(thread_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

fn decode_thread_message_payload(thread_id: &str, raw: &str) -> Result<RobdexChatMessage> {
    let mut value = serde_json::from_str::<Value>(raw)?;
    if let Value::Object(object) = &mut value
        && !object.contains_key("threadId")
        && !object.contains_key("threadID")
    {
        object.insert("threadId".to_string(), Value::String(thread_id.to_string()));
    }
    Ok(serde_json::from_value(value)?)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_message(thread_id: &str) -> RobdexChatMessage {
        RobdexChatMessage {
            id: "msg-1".to_string(),
            thread_id: thread_id.to_string(),
            role: "assistant".to_string(),
            text: "hello".to_string(),
            created_at: 1,
            subtitle: None,
            tool_metadata: None,
            delivery_state: "confirmed".to_string(),
        }
    }

    #[tokio::test]
    async fn state_document_round_trips_in_temp_db() {
        let temp = TempDir::new().expect("tempdir");
        let store = RobdexBridgeStore::connect(&temp.path().join("robdex.sqlite"))
            .await
            .expect("store");

        let payload = serde_json::json!({"projects": [{"id": "demo"}]});
        store.save_state_document(&payload).await.expect("save");

        let loaded = store
            .load_state_document(serde_json::json!({}))
            .await
            .expect("load");
        assert_eq!(loaded, payload);
    }

    #[tokio::test]
    async fn thread_cache_round_trips_in_temp_db() {
        let temp = TempDir::new().expect("tempdir");
        let store = RobdexBridgeStore::connect(&temp.path().join("robdex.sqlite"))
            .await
            .expect("store");

        let mut payload = ThreadCachePayload::default();
        payload
            .message_cache_by_thread_id
            .insert("thr-1".to_string(), vec![sample_message("thr-1")]);
        payload.context_window_status_by_thread_id.insert(
            "thr-1".to_string(),
            ThreadContextWindowStatus {
                remaining_percent: 82,
                used_tokens_in_context_window: 100,
                model_context_window: Some(1000),
            },
        );
        payload.running_thread_ids = vec!["thr-1".to_string()];
        payload.updated_at = Some(42);

        store.save_thread_cache_payload(&payload).await.expect("save");
        let loaded = store
            .load_thread_cache_payload(ThreadCachePayload::default())
            .await
            .expect("load");

        assert_eq!(loaded, payload);
    }

    #[tokio::test]
    async fn deleting_thread_cache_only_touches_requested_thread() {
        let temp = TempDir::new().expect("tempdir");
        let store = RobdexBridgeStore::connect(&temp.path().join("robdex.sqlite"))
            .await
            .expect("store");

        let mut payload = ThreadCachePayload {
            message_cache_by_thread_id: BTreeMap::new(),
            context_window_status_by_thread_id: BTreeMap::new(),
            running_thread_ids: vec!["thr-a".to_string(), "thr-b".to_string()],
            updated_at: Some(1),
        };
        payload
            .message_cache_by_thread_id
            .insert("thr-a".to_string(), vec![sample_message("thr-a")]);
        payload
            .message_cache_by_thread_id
            .insert("thr-b".to_string(), vec![sample_message("thr-b")]);
        store.save_thread_cache_payload(&payload).await.expect("save");

        store.delete_thread_cache("thr-a").await.expect("delete");

        let remaining = store
            .load_thread_messages("thr-b")
            .await
            .expect("load remaining");
        let deleted = store
            .load_thread_messages("thr-a")
            .await
            .expect("load deleted");
        assert_eq!(remaining.len(), 1);
        assert!(deleted.is_empty());
    }
}
