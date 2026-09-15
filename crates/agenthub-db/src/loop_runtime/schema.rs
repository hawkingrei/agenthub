use sqlx::{Row, SqlitePool};

/// Add loop records without inferring opt-in from any legacy runtime setting.
pub async fn migrate_loop_runtime(pool: &SqlitePool) -> anyhow::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS loop_policies (
            actor_id TEXT PRIMARY KEY REFERENCES agents(id),
            team_id TEXT NOT NULL REFERENCES team_definitions(id),
            state TEXT NOT NULL CHECK(state IN ('disabled', 'enabled', 'suspended')),
            session_policy TEXT NOT NULL CHECK(session_policy IN ('fresh', 'resume')),
            revision INTEGER NOT NULL CHECK(revision > 0),
            limits_json TEXT NOT NULL,
            mailbox_run_id TEXT REFERENCES team_runs(id),
            generation INTEGER NOT NULL DEFAULT 0 CHECK(generation >= 0),
            no_progress_count INTEGER NOT NULL DEFAULT 0 CHECK(no_progress_count >= 0),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(actor_id, team_id)
        );
        CREATE TABLE IF NOT EXISTS loop_activations (
            id TEXT PRIMARY KEY,
            actor_id TEXT NOT NULL,
            team_id TEXT NOT NULL,
            state TEXT NOT NULL CHECK(state IN (
                'pending', 'starting', 'running', 'finalizing', 'finished', 'interrupted', 'canceled'
            )),
            due_at INTEGER NOT NULL,
            next_admission_at INTEGER NOT NULL DEFAULT 0,
            coalesce_key TEXT,
            policy_revision INTEGER NOT NULL,
            generation INTEGER NOT NULL DEFAULT 0 CHECK(generation >= 0),
            attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
            mailbox_run_id TEXT REFERENCES team_runs(id),
            session_id TEXT REFERENCES agent_sessions(id),
            launch_json TEXT,
            outcome_json TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            finished_at INTEGER,
            UNIQUE(id, actor_id, team_id),
            FOREIGN KEY(actor_id, team_id) REFERENCES loop_policies(actor_id, team_id)
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_loop_pending_coalesce
            ON loop_activations(actor_id, coalesce_key) WHERE state = 'pending';
        CREATE INDEX IF NOT EXISTS idx_loop_activation_due
            ON loop_activations(state, due_at, created_at, id);
        CREATE INDEX IF NOT EXISTS idx_loop_activation_history
            ON loop_activations(team_id, actor_id, created_at, id);
        CREATE TABLE IF NOT EXISTS loop_trigger_sources (
            id TEXT PRIMARY KEY,
            activation_id TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            team_id TEXT NOT NULL,
            source_kind TEXT NOT NULL,
            source_key TEXT NOT NULL,
            input_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            UNIQUE(actor_id, team_id, source_kind, source_key),
            FOREIGN KEY(activation_id, actor_id, team_id) REFERENCES loop_activations(id, actor_id, team_id),
            FOREIGN KEY(actor_id, team_id) REFERENCES loop_policies(actor_id, team_id)
        );
        CREATE INDEX IF NOT EXISTS idx_loop_trigger_activation
            ON loop_trigger_sources(activation_id, created_at, id);
        CREATE TABLE IF NOT EXISTS loop_execution_reservations (
            actor_id TEXT PRIMARY KEY REFERENCES loop_policies(actor_id),
            activation_id TEXT UNIQUE REFERENCES loop_activations(id),
            generation INTEGER NOT NULL CHECK(generation > 0),
            owner_id TEXT NOT NULL,
            lease_expires_at INTEGER NOT NULL,
            lease_seconds INTEGER NOT NULL DEFAULT 60 CHECK(lease_seconds > 0),
            renewal_seconds INTEGER NOT NULL DEFAULT 15 CHECK(renewal_seconds > 0),
            session_id TEXT REFERENCES agent_sessions(id),
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS loop_activation_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            activation_id TEXT NOT NULL REFERENCES loop_activations(id),
            kind TEXT NOT NULL,
            generation INTEGER NOT NULL CHECK(generation >= 0),
            trigger_id TEXT REFERENCES loop_trigger_sources(id),
            reason_code TEXT,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_loop_event_activation
            ON loop_activation_events(activation_id, id);
        CREATE TABLE IF NOT EXISTS loop_finish_receipts (
            activation_id TEXT PRIMARY KEY REFERENCES loop_activations(id),
            generation INTEGER NOT NULL,
            owner_id TEXT NOT NULL,
            receipt_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS loop_progress_receipts (
            task_note_id INTEGER PRIMARY KEY REFERENCES team_conversation_messages(id),
            activation_id TEXT NOT NULL REFERENCES loop_activations(id)
        );
        CREATE TABLE IF NOT EXISTS loop_revoked_sources (
            trigger_id TEXT PRIMARY KEY REFERENCES loop_trigger_sources(id),
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS loop_mailbox_partitions (
            run_id TEXT PRIMARY KEY REFERENCES team_runs(id),
            team_id TEXT NOT NULL REFERENCES team_definitions(id),
            active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0, 1)),
            created_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&mut *tx)
    .await?;
    let partition_columns = sqlx::query("PRAGMA table_info(loop_mailbox_partitions)")
        .fetch_all(&mut *tx)
        .await?;
    if !partition_columns
        .iter()
        .any(|row| row.get::<&str, _>("name") == "active")
    {
        sqlx::query("ALTER TABLE loop_mailbox_partitions ADD COLUMN active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0, 1))")
            .execute(&mut *tx).await?;
    }
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_loop_active_partition ON loop_mailbox_partitions(team_id) WHERE active = 1")
        .execute(&mut *tx).await?;
    let columns = sqlx::query("PRAGMA table_info(loop_activations)")
        .fetch_all(&mut *tx)
        .await?;
    if !columns
        .iter()
        .any(|row| row.get::<&str, _>("name") == "next_admission_at")
    {
        sqlx::query(
            "ALTER TABLE loop_activations ADD COLUMN next_admission_at INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE loop_activations SET next_admission_at = due_at")
            .execute(&mut *tx)
            .await?;
    }
    let columns = sqlx::query("PRAGMA table_info(loop_activation_events)")
        .fetch_all(&mut *tx)
        .await?;
    if !columns
        .iter()
        .any(|row| row.get::<&str, _>("name") == "reason_code")
    {
        sqlx::query("ALTER TABLE loop_activation_events ADD COLUMN reason_code TEXT")
            .execute(&mut *tx)
            .await?;
    }
    let columns = sqlx::query("PRAGMA table_info(loop_execution_reservations)")
        .fetch_all(&mut *tx)
        .await?;
    if !columns
        .iter()
        .any(|row| row.get::<&str, _>("name") == "lease_seconds")
    {
        sqlx::query("ALTER TABLE loop_execution_reservations ADD COLUMN lease_seconds INTEGER NOT NULL DEFAULT 60 CHECK(lease_seconds > 0)")
            .execute(&mut *tx).await?;
    }
    if !columns
        .iter()
        .any(|row| row.get::<&str, _>("name") == "renewal_seconds")
    {
        sqlx::query("ALTER TABLE loop_execution_reservations ADD COLUMN renewal_seconds INTEGER NOT NULL DEFAULT 15 CHECK(renewal_seconds > 0)")
            .execute(&mut *tx).await?;
        sqlx::query(
            "UPDATE loop_execution_reservations SET renewal_seconds = MAX(1, lease_seconds / 4)",
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::raw_sql(
        "CREATE INDEX IF NOT EXISTS idx_loop_admission_due ON loop_activations(state, next_admission_at, id); \
         CREATE INDEX IF NOT EXISTS idx_loop_admission_events ON loop_activation_events(created_at, activation_id) WHERE kind = 'admitted';",
    ).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
