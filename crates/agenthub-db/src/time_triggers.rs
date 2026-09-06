use sqlx::{Row, SqlitePool};

/// Extend existing reminder tables without resetting pending or historical rows.
pub async fn migrate_time_triggers(pool: &SqlitePool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let columns = sqlx::query("PRAGMA table_info(agent_time_triggers)")
        .fetch_all(&mut *tx)
        .await?;
    for (name, migration) in [
        (
            "attempt",
            "ALTER TABLE agent_time_triggers ADD COLUMN attempt INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "next_attempt_at",
            "ALTER TABLE agent_time_triggers ADD COLUMN next_attempt_at INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "lease_expires_at",
            "ALTER TABLE agent_time_triggers ADD COLUMN lease_expires_at INTEGER",
        ),
        (
            "source_json",
            "ALTER TABLE agent_time_triggers ADD COLUMN source_json TEXT",
        ),
    ] {
        if !columns
            .iter()
            .any(|row| row.get::<String, _>("name") == name)
        {
            sqlx::query(migration).execute(&mut *tx).await?;
        }
    }
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_agent_time_triggers_retry ON agent_time_triggers(status, next_attempt_at, fire_at)",
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reminder_migration_preserves_legacy_rows_and_is_repeatable() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE agent_time_triggers (id TEXT PRIMARY KEY, status TEXT, fire_at INTEGER)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO agent_time_triggers VALUES ('old', 'dispatching', 123)")
            .execute(&pool)
            .await
            .unwrap();
        migrate_time_triggers(&pool).await.unwrap();
        migrate_time_triggers(&pool).await.unwrap();
        let row = sqlx::query("SELECT * FROM agent_time_triggers WHERE id = 'old'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("status"), "dispatching");
        assert_eq!(row.get::<i64, _>("fire_at"), 123);
        assert_eq!(row.get::<i64, _>("attempt"), 0);
        assert!(row.get::<Option<i64>, _>("lease_expires_at").is_none());
        assert!(row.get::<Option<String>, _>("source_json").is_none());
    }
}
