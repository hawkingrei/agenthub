use sqlx::Row;

use agenthub_db::AgentEventDbRouter;

#[derive(Debug, Clone)]
pub(super) struct MemoryFlushEventRow {
    pub(super) id: i64,
    pub(super) stream: String,
    pub(super) message: Vec<u8>,
}

pub(super) async fn load_memory_flush_team_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    run_id: &str,
) -> anyhow::Result<String> {
    let run_meta_row = sqlx::query(
        r#"
        SELECT team_id
        FROM team_runs
        WHERE id = ?1
        "#,
    )
    .bind(run_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(run_meta_row.get("team_id"))
}

pub(super) async fn load_memory_flush_checkpoint_event_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    run_id: &str,
    member_id: &str,
    session_id: &str,
) -> anyhow::Result<i64> {
    let checkpoint_event_id = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT last_event_id
        FROM team_context_flush_checkpoint
        WHERE run_id = ?1
          AND member_id = ?2
          AND session_id = ?3
        "#,
    )
    .bind(run_id)
    .bind(member_id)
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(0);
    Ok(checkpoint_event_id)
}

pub(super) async fn load_memory_flush_event_rows(
    event_dbs: &AgentEventDbRouter,
    member_id: &str,
    session_id: &str,
    checkpoint_event_id: i64,
    max_events: i64,
) -> anyhow::Result<Vec<MemoryFlushEventRow>> {
    let event_db = event_dbs.pool_for_agent(member_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, stream, message
        FROM agent_events
        WHERE session_id = ?1
          AND id > ?2
        ORDER BY id ASC
        LIMIT ?3
        "#,
    )
    .bind(session_id)
    .bind(checkpoint_event_id)
    .bind(max_events)
    .fetch_all(&event_db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| MemoryFlushEventRow {
            id: row.get("id"),
            stream: row.get("stream"),
            message: row.get("message"),
        })
        .collect())
}

pub(super) async fn upsert_memory_flush_checkpoint_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    team_id: &str,
    run_id: &str,
    member_id: &str,
    session_id: &str,
    event_id_to: i64,
    now: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO team_context_flush_checkpoint (
            team_id,
            run_id,
            member_id,
            session_id,
            last_event_id,
            updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(run_id, member_id, session_id)
        DO UPDATE SET
            last_event_id = excluded.last_event_id,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(team_id)
    .bind(run_id)
    .bind(member_id)
    .bind(session_id)
    .bind(event_id_to)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn resolve_memory_flush_session_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    run_id: &str,
    member_id: &str,
    requested_session_id: Option<&str>,
) -> anyhow::Result<Option<String>> {
    if let Some(session_id) = requested_session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(Some(session_id.to_string()));
    }

    let row = sqlx::query(
        r#"
        SELECT remote_task_id
        FROM team_steps
        WHERE run_id = ?1
          AND member_id = ?2
          AND remote_task_id IS NOT NULL
        ORDER BY COALESCE(ended_at, started_at, 0) DESC, attempt DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .bind(member_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.and_then(|entry| {
        entry
            .get::<Option<String>, _>("remote_task_id")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }))
}

pub(super) fn safe_i64_len(len: usize) -> i64 {
    i64::try_from(len).unwrap_or(i64::MAX)
}
