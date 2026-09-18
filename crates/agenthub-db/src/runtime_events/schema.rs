use sqlx::SqlitePool;

pub(crate) async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS runtime_event_owners (
            runtime_id TEXT PRIMARY KEY,
            local_session_id TEXT NOT NULL UNIQUE,
            closed INTEGER NOT NULL DEFAULT 0 CHECK (closed IN (0, 1))
        );
        CREATE TABLE IF NOT EXISTS runtime_event_streams (
            runtime_id TEXT NOT NULL REFERENCES runtime_event_owners(runtime_id),
            native_session_id TEXT NOT NULL,
            last_sequence INTEGER NOT NULL DEFAULT 0 CHECK (last_sequence >= 0),
            gap_after INTEGER,
            gap_oldest INTEGER,
            gap_latest INTEGER,
            PRIMARY KEY (runtime_id, native_session_id),
            CHECK ((gap_after IS NULL AND gap_oldest IS NULL AND gap_latest IS NULL)
                OR (gap_after IS NOT NULL AND gap_oldest IS NOT NULL AND gap_latest IS NOT NULL
                    AND gap_after >= 0 AND gap_oldest > 0 AND gap_latest >= 0
                    AND (gap_after < gap_oldest - 1 OR gap_after > gap_latest)
                    AND gap_oldest - 1 <= gap_latest))
        );
        CREATE TABLE IF NOT EXISTS runtime_event_receipts (
            runtime_id TEXT NOT NULL,
            native_session_id TEXT NOT NULL,
            event_id TEXT NOT NULL,
            sequence INTEGER NOT NULL CHECK (sequence > 0),
            fingerprint BLOB NOT NULL CHECK (length(fingerprint) = 32),
            PRIMARY KEY (runtime_id, native_session_id, event_id),
            UNIQUE (runtime_id, native_session_id, sequence),
            FOREIGN KEY (runtime_id, native_session_id)
                REFERENCES runtime_event_streams(runtime_id, native_session_id)
        );
        CREATE TABLE IF NOT EXISTS runtime_control_receipts (
            runtime_id TEXT NOT NULL REFERENCES runtime_event_owners(runtime_id),
            request_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            target_session_id TEXT,
            expected_turn_id TEXT,
            status TEXT NOT NULL CHECK (status IN
                ('prepared', 'sent', 'accepted', 'queued', 'rejected', 'outcome_unknown', 'not_sent')),
            ack_json TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (runtime_id, request_id),
            CHECK ((status IN ('accepted', 'queued', 'rejected') AND ack_json IS NOT NULL)
                OR (status NOT IN ('accepted', 'queued', 'rejected') AND ack_json IS NULL))
        );
        CREATE TABLE IF NOT EXISTS runtime_event_history (
            history_id INTEGER PRIMARY KEY REFERENCES agent_events(id) ON DELETE CASCADE,
            runtime_id TEXT NOT NULL,
            native_session_id TEXT NOT NULL,
            event_id TEXT NOT NULL,
            FOREIGN KEY (runtime_id, native_session_id, event_id)
                REFERENCES runtime_event_receipts(runtime_id, native_session_id, event_id)
        );
        "#,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
