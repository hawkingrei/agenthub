use std::time::Duration;

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentEventCleanupResult {
    pub cutoff_ts: i64,
    pub deleted_rows: u64,
    pub vacuum_ran: bool,
}

pub async fn init_db() -> anyhow::Result<SqlitePool> {
    let db_path = default_db_path();
    init_db_at_path(&db_path).await
}

async fn init_db_at_path(db_path: &std::path::Path) -> anyhow::Result<SqlitePool> {
    let pool = try_connect(db_path)
        .await
        .map_err(|err| anyhow::anyhow!("failed to open db at {}: {}", db_path.display(), err))?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS agents (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            workdir TEXT NOT NULL,
            command TEXT NOT NULL,
            args TEXT NOT NULL,
            worktree_mode TEXT NOT NULL,
            worktree_repo TEXT,
            worktree_ref TEXT,
            code_mode INTEGER NOT NULL DEFAULT 0,
            source TEXT NOT NULL DEFAULT 'manual',
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            role TEXT NOT NULL,
            password_hash TEXT,
            created_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user_passkeys (
            user_id TEXT PRIMARY KEY,
            passkeys TEXT NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS auth_sessions (
            token TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL,
            revoked_at INTEGER,
            FOREIGN KEY(user_id) REFERENCES users(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS devices (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            user_agent TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            last_login_at INTEGER,
            FOREIGN KEY(user_id) REFERENCES users(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS join_challenges (
            token TEXT PRIMARY KEY,
            pin_hash TEXT NOT NULL,
            expires_at INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS login_audit (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT,
            device_id TEXT,
            event TEXT NOT NULL,
            ip TEXT,
            user_agent TEXT,
            detail TEXT,
            ts INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS safe_paths (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS agent_sessions (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at INTEGER NOT NULL,
            ended_at INTEGER,
            FOREIGN KEY(agent_id) REFERENCES agents(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS agent_persistent_sessions (
            agent_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            session_id TEXT NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (agent_id, provider),
            FOREIGN KEY(agent_id) REFERENCES agents(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS agent_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            seq TEXT NOT NULL,
            ts INTEGER NOT NULL,
            stream TEXT NOT NULL,
            message TEXT NOT NULL,
            FOREIGN KEY(agent_id) REFERENCES agents(id),
            FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_definitions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            description TEXT,
            spec_json TEXT NOT NULL,
            owner_user_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_runs (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            context_id TEXT NOT NULL,
            status TEXT NOT NULL,
            input_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            started_at INTEGER,
            ended_at INTEGER,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_steps (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            step_key TEXT NOT NULL,
            member_id TEXT NOT NULL,
            remote_task_id TEXT,
            status TEXT NOT NULL,
            attempt INTEGER NOT NULL DEFAULT 0,
            depends_on_json TEXT NOT NULL DEFAULT '[]',
            input_json TEXT,
            output_json TEXT,
            error_text TEXT,
            started_at INTEGER,
            ended_at INTEGER,
            UNIQUE(run_id, step_key, attempt),
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_run_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            step_id TEXT,
            event_type TEXT NOT NULL,
            ts INTEGER NOT NULL,
            payload_json TEXT NOT NULL,
            FOREIGN KEY(run_id) REFERENCES team_runs(id),
            FOREIGN KEY(step_id) REFERENCES team_steps(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_main_tasks (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            created_by_actor_id TEXT NOT NULL,
            context_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_conversations (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            main_task_id TEXT NOT NULL UNIQUE,
            mode TEXT NOT NULL,
            topic TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(main_task_id) REFERENCES team_main_tasks(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_conversation_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id TEXT NOT NULL,
            main_task_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            to_actor_id TEXT,
            route TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES team_conversations(id),
            FOREIGN KEY(main_task_id) REFERENCES team_main_tasks(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_actor_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            to_actor_id TEXT NOT NULL,
            channel TEXT NOT NULL,
            transport TEXT NOT NULL,
            route_json TEXT,
            payload_json TEXT NOT NULL,
            idempotency_key TEXT,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            delivered_at INTEGER,
            relay_attempt INTEGER NOT NULL DEFAULT 0,
            relay_next_retry_at INTEGER,
            relay_last_error TEXT,
            dead_letter_at INTEGER,
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_member_continuity_state (
            team_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            source_run_id TEXT NOT NULL,
            source_session_id TEXT,
            summary_text TEXT NOT NULL,
            history_window_json TEXT NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (team_id, member_id),
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(source_run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_agent_events_agent_seq
        ON agent_events(agent_id, seq);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_agent_events_agent_seq: {}",
            err
        );
    }
    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_agent_events_agent_session_seq
        ON agent_events(agent_id, session_id, seq);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_agent_events_agent_session_seq: {}",
            err
        );
    }
    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_agent_events_agent_id
        ON agent_events(agent_id, id);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_agent_events_agent_id: {}",
            err
        );
    }
    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_agent_events_agent_session_id
        ON agent_events(agent_id, session_id, id);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_agent_events_agent_session_id: {}",
            err
        );
    }
    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_agent_events_ts
        ON agent_events(ts);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!("db init: failed to create idx_agent_events_ts: {}", err);
    }

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_runs_team_created
        ON team_runs(team_id, created_at DESC);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_runs_team_created: {}",
            err
        );
    }

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_run_events_run_id
        ON team_run_events(run_id, id);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_run_events_run_id: {}",
            err
        );
    }

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_steps_run_status
        ON team_steps(run_id, status);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_steps_run_status: {}",
            err
        );
    }

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_main_tasks_team_updated
        ON team_main_tasks(team_id, updated_at DESC);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_main_tasks_team_updated: {}",
            err
        );
    }

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_conversations_team_task
        ON team_conversations(team_id, main_task_id);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_conversations_team_task: {}",
            err
        );
    }

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_conversation_messages_conv_id
        ON team_conversation_messages(conversation_id, id);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_conversation_messages_conv_id: {}",
            err
        );
    }

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_actor_messages_run_to_id
        ON team_actor_messages(run_id, to_actor_id, id);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_actor_messages_run_to_id: {}",
            err
        );
    }

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_actor_messages_run_status_id
        ON team_actor_messages(run_id, status, id);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_actor_messages_run_status_id: {}",
            err
        );
    }
    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_actor_messages_remote_pending
        ON team_actor_messages(transport, status, relay_next_retry_at, id);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_actor_messages_remote_pending: {}",
            err
        );
    }
    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_member_continuity_state_team_updated
        ON team_member_continuity_state(team_id, updated_at DESC);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_member_continuity_state_team_updated: {}",
            err
        );
    }
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS push_subscriptions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            endpoint TEXT NOT NULL,
            p256dh TEXT NOT NULL,
            auth TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS acp_permission_requests (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            acp_session_id TEXT,
            tool_call_id TEXT,
            options_json TEXT NOT NULL,
            tool_call_json TEXT,
            status TEXT NOT NULL,
            selected_option_id TEXT,
            created_at INTEGER NOT NULL,
            responded_at INTEGER,
            FOREIGN KEY(agent_id) REFERENCES agents(id),
            FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    add_column_if_missing(
        &pool,
        "ALTER TABLE team_definitions ADD COLUMN owner_user_id TEXT",
        "team_definitions.owner_user_id",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE auth_sessions ADD COLUMN revoked_at INTEGER",
        "auth_sessions.revoked_at",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE devices ADD COLUMN last_login_at INTEGER",
        "devices.last_login_at",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE agents ADD COLUMN worktree_mode TEXT",
        "agents.worktree_mode",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE agents ADD COLUMN worktree_repo TEXT",
        "agents.worktree_repo",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE agents ADD COLUMN worktree_ref TEXT",
        "agents.worktree_ref",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE agents ADD COLUMN code_mode INTEGER",
        "agents.code_mode",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE agents ADD COLUMN source TEXT NOT NULL DEFAULT 'manual'",
        "agents.source",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE acp_permission_requests ADD COLUMN acp_session_id TEXT",
        "acp_permission_requests.acp_session_id",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE team_actor_messages ADD COLUMN relay_attempt INTEGER NOT NULL DEFAULT 0",
        "team_actor_messages.relay_attempt",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE team_actor_messages ADD COLUMN relay_next_retry_at INTEGER",
        "team_actor_messages.relay_next_retry_at",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE team_actor_messages ADD COLUMN relay_last_error TEXT",
        "team_actor_messages.relay_last_error",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE team_actor_messages ADD COLUMN dead_letter_at INTEGER",
        "team_actor_messages.dead_letter_at",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE team_actor_messages ADD COLUMN idempotency_key TEXT",
        "team_actor_messages.idempotency_key",
    )
    .await;
    if let Err(err) = sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_team_actor_messages_idempotency
        ON team_actor_messages(run_id, from_actor_id, idempotency_key)
        WHERE idempotency_key IS NOT NULL
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_actor_messages_idempotency: {}",
            err
        );
    }

    Ok(pool)
}

pub async fn cleanup_agent_event_history(
    pool: &SqlitePool,
    retention_days: u32,
    vacuum_on_cleanup: bool,
) -> anyhow::Result<AgentEventCleanupResult> {
    const SECONDS_PER_DAY: i64 = 24 * 60 * 60;
    let retention_seconds = i64::from(retention_days).saturating_mul(SECONDS_PER_DAY);
    let cutoff_ts = chrono::Utc::now()
        .timestamp()
        .saturating_sub(retention_seconds);

    let deleted_rows = sqlx::query(
        r#"
        DELETE FROM agent_events
        WHERE ts < ?1
        "#,
    )
    .bind(cutoff_ts)
    .execute(pool)
    .await?
    .rows_affected();

    if let Err(err) = sqlx::query("PRAGMA wal_checkpoint(PASSIVE)")
        .execute(pool)
        .await
    {
        tracing::warn!("db cleanup: wal checkpoint failed: {}", err);
    }

    let mut vacuum_ran = false;
    if vacuum_on_cleanup && deleted_rows > 0 {
        if let Err(err) = sqlx::query("VACUUM").execute(pool).await {
            tracing::warn!("db cleanup: vacuum failed: {}", err);
        } else {
            vacuum_ran = true;
        }
    }

    Ok(AgentEventCleanupResult {
        cutoff_ts,
        deleted_rows,
        vacuum_ran,
    })
}

async fn add_column_if_missing(pool: &SqlitePool, sql: &str, column: &str) {
    if let Err(err) = sqlx::query(sql).execute(pool).await {
        let message = err.to_string();
        if !message.contains("duplicate column name") {
            tracing::warn!("db init: failed to add {} column: {}", column, message);
        }
    }
}

async fn try_connect(db_path: &std::path::Path) -> anyhow::Result<SqlitePool> {
    ensure_sqlite_path(db_path)?;
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    Ok(pool)
}

fn default_db_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&home).join(".agenthub/agenthub.db")
}

fn ensure_sqlite_path(db_path: &std::path::Path) -> anyhow::Result<()> {
    create_parent_dir(db_path)?;
    Ok(())
}

fn create_parent_dir(path: &std::path::Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cleanup_agent_event_history, create_parent_dir, init_db_at_path, try_connect};
    use sqlx::Row;
    use uuid::Uuid;

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("agenthub-{name}-{}", Uuid::new_v4()))
    }

    #[tokio::test]
    async fn init_db_creates_schema_and_enforces_foreign_keys() {
        let dir = unique_temp_dir("db-init");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("agenthub.db");

        let pool = init_db_at_path(&db_path).await.expect("init db");

        let table_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'table'
              AND name IN (
                'team_definitions',
                'team_runs',
                'team_steps',
                'team_run_events',
                'team_main_tasks',
                'team_conversations',
                'team_conversation_messages',
                'team_actor_messages',
                'team_member_continuity_state'
              )
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("count tables");
        assert_eq!(table_count, 9);

        let fk_enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .expect("read pragma foreign_keys");
        assert_eq!(fk_enabled, 1);

        let fk_err = sqlx::query(
            r#"
            INSERT INTO team_runs (id, team_id, context_id, status, input_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind("run-without-team")
        .bind("missing-team")
        .bind("ctx")
        .bind("submitted")
        .bind("{}")
        .bind(1_i64)
        .execute(&pool)
        .await
        .expect_err("fk violation expected");
        assert!(
            fk_err.to_string().contains("FOREIGN KEY constraint failed"),
            "unexpected fk error: {fk_err}"
        );

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn try_connect_sets_foreign_key_pragma() {
        let dir = unique_temp_dir("db-pragma");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("pragma.db");

        let pool = try_connect(&db_path).await.expect("connect sqlite");
        let fk_enabled: i64 = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .expect("query pragma")
            .get(0);
        assert_eq!(fk_enabled, 1);

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_parent_dir_creates_nested_directories() {
        let dir = unique_temp_dir("db-parent");
        let file_path = dir.join("nested/a/b/c.sqlite");
        create_parent_dir(&file_path).expect("create nested parent dirs");
        assert!(
            file_path
                .parent()
                .expect("parent path")
                .try_exists()
                .expect("check parent exists"),
            "parent directory should exist"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn cleanup_agent_event_history_deletes_rows_older_than_retention() {
        let dir = unique_temp_dir("db-cleanup");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("cleanup.db");
        let pool = init_db_at_path(&db_path).await.expect("init db");

        let now = chrono::Utc::now().timestamp();
        let old_ts = now - (10 * 24 * 60 * 60);
        let new_ts = now - (2 * 24 * 60 * 60);

        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
                code_mode, source, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
        )
        .bind("agent-cleanup")
        .bind("cleanup-agent")
        .bind("/tmp")
        .bind("echo")
        .bind("[]")
        .bind("reuse_worktree")
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(0_i64)
        .bind("manual")
        .bind("stopped")
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert agent");

        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind("session-cleanup")
        .bind("agent-cleanup")
        .bind("running")
        .bind(now)
        .bind(None::<i64>)
        .execute(&pool)
        .await
        .expect("insert session");

        sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind("agent-cleanup")
        .bind("session-cleanup")
        .bind("seq-old")
        .bind(old_ts)
        .bind("acp")
        .bind("old")
        .execute(&pool)
        .await
        .expect("insert old event");

        sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind("agent-cleanup")
        .bind("session-cleanup")
        .bind("seq-new")
        .bind(new_ts)
        .bind("acp")
        .bind("new")
        .execute(&pool)
        .await
        .expect("insert new event");

        let result = cleanup_agent_event_history(&pool, 5, false)
            .await
            .expect("cleanup history");
        assert_eq!(result.deleted_rows, 1);
        assert!(!result.vacuum_ran);
        assert!(result.cutoff_ts > old_ts);
        assert!(result.cutoff_ts < new_ts);

        let remaining: i64 = sqlx::query("SELECT COUNT(*) AS cnt FROM agent_events")
            .fetch_one(&pool)
            .await
            .expect("count remaining events")
            .get("cnt");
        assert_eq!(remaining, 1);

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
