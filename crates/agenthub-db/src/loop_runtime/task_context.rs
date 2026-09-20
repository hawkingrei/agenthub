use agenthub_agent_domain::loop_runtime::{LoopReservation, LoopTaskContext, validate_loop_id};
use sha2::{Digest, Sha256};
use sqlx::Row;

use super::{LoopStore, LoopStoreError, require_live_reservation, require_member};

impl LoopStore {
    /// Pin routing before a provider sees the task. Only live, addressed work can create it.
    pub async fn pin_task_context(
        &self,
        expected: &LoopReservation,
        task_id: &str,
        now: i64,
    ) -> anyhow::Result<LoopTaskContext> {
        validate_loop_id(task_id)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = require_live_reservation(&mut tx, expected, now).await?;
        require_member(&mut tx, &current.team_id, &current.actor_id).await?;
        let row = sqlx::query(
            "SELECT t.title, json_extract(t.context_json, '$.summary') AS summary \
             FROM team_tasks t WHERE t.id = ? AND t.team_id = ? AND EXISTS (\
             SELECT 1 FROM loop_trigger_sources s WHERE s.activation_id = ? \
             AND s.actor_id = ? AND s.team_id = t.team_id \
             AND json_extract(s.input_json, '$.references.task_id') = t.id \
             AND NOT EXISTS (SELECT 1 FROM loop_revoked_sources r WHERE r.trigger_id = s.id))",
        )
        .bind(task_id)
        .bind(&current.team_id)
        .bind(&current.activation_id)
        .bind(&current.actor_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(LoopStoreError::ScopeMismatch)?;
        let title = normalize(row.try_get("title")?, 1024)?;
        anyhow::ensure!(!title.is_empty(), "task title must not be empty");
        let summary = row
            .try_get::<Option<String>, _>("summary")?
            .map(|text| normalize(&text, 4096))
            .transpose()?
            .filter(|text| !text.is_empty());
        let digest = Sha256::digest(serde_json::to_vec(&(
            "task-memory-v1",
            &current.team_id,
            task_id,
            &title,
            &summary,
        ))?);
        let digest: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        let candidate = format!("task-memory-v1:{digest}");
        sqlx::query(
            "INSERT INTO loop_task_memory_prefixes(task_id, team_id, prefix, created_at) \
             VALUES (?, ?, ?, ?) ON CONFLICT(task_id) DO NOTHING",
        )
        .bind(task_id)
        .bind(&current.team_id)
        .bind(candidate)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let memory_prefix = sqlx::query_scalar(
            "SELECT prefix FROM loop_task_memory_prefixes WHERE task_id = ? AND team_id = ?",
        )
        .bind(task_id)
        .bind(&current.team_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(LoopTaskContext {
            task_id: task_id.into(),
            title,
            summary,
            memory_prefix,
        })
    }
}

fn normalize(text: &str, max_bytes: usize) -> anyhow::Result<String> {
    anyhow::ensure!(
        text.len() <= max_bytes * 4,
        "canonical task expression input is too large"
    );
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    anyhow::ensure!(
        text.len() <= max_bytes && !text.chars().any(char::is_control),
        "canonical task expression exceeds its text boundary"
    );
    Ok(text)
}
