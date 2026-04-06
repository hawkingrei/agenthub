use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use tokio::sync::Mutex;

use agenthub_config::path_utils::expand_tilde;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentEventCleanupResult {
    pub cutoff_ts: i64,
    pub deleted_rows: u64,
    pub delete_batches: u64,
    pub vacuum_ran: bool,
}

#[derive(Debug, Clone, Copy)]
struct AgentEventIdleGcState {
    generation: u64,
    checked_for_generation: bool,
    completed_generation: u64,
}

#[derive(Clone)]
pub struct AgentEventIdleGc {
    event_dbs: AgentEventDbRouter,
    retention_days: u32,
    vacuum_on_cleanup: bool,
    delete_batch_size: u32,
    idle_timeout: Duration,
    states: Arc<Mutex<HashMap<String, AgentEventIdleGcState>>>,
}

impl AgentEventIdleGc {
    pub fn new(
        event_dbs: AgentEventDbRouter,
        retention_days: u32,
        vacuum_on_cleanup: bool,
        delete_batch_size: u32,
        idle_timeout: Duration,
    ) -> Self {
        Self {
            event_dbs,
            retention_days,
            vacuum_on_cleanup,
            delete_batch_size,
            idle_timeout,
            states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn record_activity(&self, agent_id: &str) {
        let generation = {
            let mut states = self.states.lock().await;
            let state = states
                .entry(agent_id.to_string())
                .or_insert(AgentEventIdleGcState {
                    generation: 0,
                    checked_for_generation: false,
                    completed_generation: 0,
                });
            state.generation = state.generation.wrapping_add(1);
            state.checked_for_generation = false;
            state.generation
        };

        let agent_id = agent_id.to_string();
        let states = self.states.clone();
        let event_dbs = self.event_dbs.clone();
        let retention_days = self.retention_days;
        let vacuum_on_cleanup = self.vacuum_on_cleanup;
        let delete_batch_size = self.delete_batch_size;
        let idle_timeout = self.idle_timeout;
        tokio::spawn(async move {
            tokio::time::sleep(idle_timeout).await;
            let should_run = {
                let mut states = states.lock().await;
                match states.get_mut(&agent_id) {
                    Some(state)
                        if state.generation == generation && !state.checked_for_generation =>
                    {
                        state.checked_for_generation = true;
                        true
                    }
                    _ => false,
                }
            };
            if !should_run {
                return;
            }
            let event_db = match event_dbs.pool_for_agent(&agent_id).await {
                Ok(pool) => pool,
                Err(err) => {
                    tracing::warn!(
                        agent_id = %agent_id,
                        error = %err,
                        "idle gc skipped: failed to open per-agent event db"
                    );
                    return;
                }
            };
            match cleanup_agent_event_history(
                &event_db,
                retention_days,
                vacuum_on_cleanup,
                delete_batch_size,
            )
            .await
            {
                Ok(result) => {
                    tracing::debug!(
                        agent_id = %agent_id,
                        retention_days,
                        deleted_rows = result.deleted_rows,
                        delete_batches = result.delete_batches,
                        vacuum_ran = result.vacuum_ran,
                        "idle gc check completed"
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        agent_id = %agent_id,
                        retention_days,
                        error = %err,
                        "idle gc check failed"
                    );
                }
            }
            let mut states = states.lock().await;
            if let Some(state) = states.get_mut(&agent_id)
                && state.generation == generation
            {
                state.completed_generation = generation;
            }
        });
    }

    pub async fn remove_agent(&self, agent_id: &str) {
        let mut states = self.states.lock().await;
        states.remove(agent_id);
    }

    pub async fn tracked_agent_count(&self) -> usize {
        self.states.lock().await.len()
    }
}

#[derive(Clone)]
pub struct AgentEventDbRouter {
    base_dir: PathBuf,
    pools: Arc<Mutex<HashMap<String, Arc<tokio::sync::OnceCell<SqlitePool>>>>>,
}

impl AgentEventDbRouter {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            pools: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_default_base_dir() -> Self {
        Self::new(default_agent_event_db_dir())
    }

    pub async fn pool_for_agent(&self, agent_id: &str) -> anyhow::Result<SqlitePool> {
        let cell = {
            let mut pools = self.pools.lock().await;
            pools
                .entry(agent_id.to_string())
                .or_insert_with(|| Arc::new(tokio::sync::OnceCell::new()))
                .clone()
        };

        cell.get_or_try_init(|| async {
            let db_path = self.db_path_for_agent(agent_id);
            ensure_sqlite_path(&db_path)?;
            let pool = connect_sqlite_with_defaults(&db_path, 2).await?;
            init_agent_event_db_schema(&pool).await?;
            Ok(pool)
        })
        .await
        .cloned()
    }

    pub async fn remove_agent_db(&self, agent_id: &str) -> anyhow::Result<()> {
        let cell = {
            let mut pools = self.pools.lock().await;
            pools.remove(agent_id)
        };
        let db_path = self.db_path_for_agent(agent_id);
        if let Some(pool) = cell.and_then(|c| c.get().cloned()) {
            if let Err(err) = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(&pool)
                .await
            {
                tracing::warn!(
                    agent_id = agent_id,
                    db_path = %db_path.display(),
                    error = %err,
                    "WAL checkpoint before deleting agent event db failed"
                );
            }
            pool.close().await;
        }
        Self::remove_file_with_retry(&db_path).await?;
        Self::remove_file_with_retry(&Self::suffixed_path(&db_path, "-wal")).await?;
        Self::remove_file_with_retry(&Self::suffixed_path(&db_path, "-shm")).await?;
        Ok(())
    }

    pub fn db_path_for_agent(&self, agent_id: &str) -> PathBuf {
        self.base_dir.join(format!("{agent_id}.db"))
    }

    fn suffixed_path(path: &Path, suffix: &str) -> PathBuf {
        let mut raw = path.as_os_str().to_os_string();
        raw.push(suffix);
        PathBuf::from(raw)
    }

    async fn remove_file_with_retry(path: &Path) -> std::io::Result<()> {
        if !tokio::fs::try_exists(path).await? {
            return Ok(());
        }

        let mut last_err = None;
        for attempt in 0..5 {
            match tokio::fs::remove_file(path).await {
                Ok(()) => return Ok(()),
                Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
                Err(err) => {
                    last_err = Some(err);
                    if attempt < 4 {
                        tokio::time::sleep(Duration::from_millis(20 * (attempt as u64 + 1))).await;
                    }
                }
            }
        }

        Err(last_err.expect("remove_file_with_retry should record the final deletion error"))
    }
}

pub async fn init_db() -> anyhow::Result<SqlitePool> {
    let db_path = default_db_path();
    init_db_at_path(&db_path).await
}

async fn init_db_at_path(db_path: &std::path::Path) -> anyhow::Result<SqlitePool> {
    let pool = try_connect(db_path).await.map_err(|err| {
        tracing::error!(
            db_path = %db_path.display(),
            error = %err,
            "db init failed to open sqlite database"
        );
        anyhow::anyhow!("failed to open db at {}: {}", db_path.display(), err)
    })?;

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
            agent_loop_enabled INTEGER NOT NULL DEFAULT 0,
            agent_loop_idle_seconds INTEGER,
            agent_loop_prompt TEXT,
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
        CREATE TABLE IF NOT EXISTS system_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
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
        CREATE TABLE IF NOT EXISTS agent_nodes (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            grpc_target TEXT NOT NULL,
            tls_server_name TEXT,
            default_worktree_root TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
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
            message BLOB NOT NULL,
            FOREIGN KEY(agent_id) REFERENCES agents(id),
            FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    migrate_agent_events_message_column_to_blob(&pool).await?;

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
        CREATE TABLE IF NOT EXISTS team_tasks (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            created_by_actor_id TEXT NOT NULL,
            assigned_member_id TEXT,
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
            task_id TEXT NOT NULL UNIQUE,
            mode TEXT NOT NULL,
            topic TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(task_id) REFERENCES team_tasks(id)
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
            task_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            to_actor_id TEXT,
            route TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            idempotency_key TEXT,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(conversation_id) REFERENCES team_conversations(id),
            FOREIGN KEY(task_id) REFERENCES team_tasks(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_channel_message_replicas (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            authority_message_id INTEGER NOT NULL,
            run_id TEXT NOT NULL,
            team_id TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            from_actor_id TEXT NOT NULL,
            source_node_id TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            stored_at INTEGER NOT NULL,
            UNIQUE(authority_message_id)
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
            from_peer_id TEXT NOT NULL DEFAULT 'main',
            to_actor_id TEXT NOT NULL,
            to_peer_id TEXT NOT NULL DEFAULT 'main',
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

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_context_artifacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            team_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            session_id TEXT,
            artifact_seq INTEGER NOT NULL,
            artifact_kind TEXT NOT NULL,
            artifact_path TEXT NOT NULL,
            artifact_size_bytes INTEGER NOT NULL,
            content_checksum TEXT,
            created_at INTEGER NOT NULL,
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS team_context_flush_checkpoint (
            team_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            member_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            last_event_id INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (run_id, member_id, session_id),
            FOREIGN KEY(team_id) REFERENCES team_definitions(id),
            FOREIGN KEY(run_id) REFERENCES team_runs(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS agent_time_triggers (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            created_by_actor_id TEXT NOT NULL,
            message_text TEXT NOT NULL,
            fire_at INTEGER NOT NULL,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            fired_at INTEGER,
            last_error TEXT,
            FOREIGN KEY(agent_id) REFERENCES agents(id)
        );
        "#,
    )
    .execute(&pool)
    .await?;

    migrate_legacy_team_task_schema(&pool).await?;
    migrate_team_tasks_add_assigned_member_id(&pool).await?;
    migrate_safe_paths_to_absolute(&pool).await?;
    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_agent_nodes_name
        ON agent_nodes(name);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!("db init: failed to create idx_agent_nodes_name: {}", err);
    }

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
        CREATE INDEX IF NOT EXISTS idx_team_tasks_team_updated
        ON team_tasks(team_id, updated_at DESC);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_tasks_team_updated: {}",
            err
        );
    }

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_conversations_team_task
        ON team_conversations(team_id, task_id);
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
        CREATE INDEX IF NOT EXISTS idx_agent_time_triggers_agent_created
        ON agent_time_triggers(agent_id, created_at DESC);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_agent_time_triggers_agent_created: {}",
            err
        );
    }

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_agent_time_triggers_status_fire_at
        ON agent_time_triggers(status, fire_at);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_agent_time_triggers_status_fire_at: {}",
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
        CREATE INDEX IF NOT EXISTS idx_team_channel_message_replicas_run_channel
        ON team_channel_message_replicas(run_id, channel_id, stored_at DESC);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_channel_message_replicas_run_channel: {}",
            err
        );
    }

    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_actor_messages_run_peer_to_id
        ON team_actor_messages(run_id, to_peer_id, to_actor_id, id);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_actor_messages_run_peer_to_id: {}",
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
    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_context_artifacts_run_member_created
        ON team_context_artifacts(run_id, member_id, created_at DESC);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_context_artifacts_run_member_created: {}",
            err
        );
    }
    if let Err(err) = sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_team_context_artifacts_run_seq
        ON team_context_artifacts(run_id, artifact_seq);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_context_artifacts_run_seq: {}",
            err
        );
    }
    if let Err(err) = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_team_context_flush_checkpoint_run_member
        ON team_context_flush_checkpoint(run_id, member_id, updated_at DESC);
        "#,
    )
    .execute(&pool)
    .await
    {
        tracing::warn!(
            "db init: failed to create idx_team_context_flush_checkpoint_run_member: {}",
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
            team_id TEXT,
            requester_actor_id TEXT,
            requester_role TEXT,
            review_target_actor_id TEXT,
            review_dispatch_status TEXT,
            review_delivery_run_id TEXT,
            review_message_id INTEGER,
            review_dispatched_at INTEGER,
            reviewed_by_actor_id TEXT,
            human_review_notified_at INTEGER,
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
        "ALTER TABLE agent_nodes ADD COLUMN default_worktree_root TEXT",
        "agent_nodes.default_worktree_root",
    )
    .await;
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
        "ALTER TABLE agents ADD COLUMN agent_loop_enabled INTEGER NOT NULL DEFAULT 0",
        "agents.agent_loop_enabled",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE agents ADD COLUMN agent_loop_idle_seconds INTEGER",
        "agents.agent_loop_idle_seconds",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE agents ADD COLUMN agent_loop_prompt TEXT",
        "agents.agent_loop_prompt",
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
        "ALTER TABLE agents ADD COLUMN target_node_id TEXT",
        "agents.target_node_id",
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
        "ALTER TABLE acp_permission_requests ADD COLUMN team_id TEXT",
        "acp_permission_requests.team_id",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE acp_permission_requests ADD COLUMN requester_actor_id TEXT",
        "acp_permission_requests.requester_actor_id",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE acp_permission_requests ADD COLUMN requester_role TEXT",
        "acp_permission_requests.requester_role",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE acp_permission_requests ADD COLUMN review_target_actor_id TEXT",
        "acp_permission_requests.review_target_actor_id",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE acp_permission_requests ADD COLUMN review_dispatch_status TEXT",
        "acp_permission_requests.review_dispatch_status",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE acp_permission_requests ADD COLUMN review_delivery_run_id TEXT",
        "acp_permission_requests.review_delivery_run_id",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE acp_permission_requests ADD COLUMN review_message_id INTEGER",
        "acp_permission_requests.review_message_id",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE acp_permission_requests ADD COLUMN review_dispatched_at INTEGER",
        "acp_permission_requests.review_dispatched_at",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE acp_permission_requests ADD COLUMN reviewed_by_actor_id TEXT",
        "acp_permission_requests.reviewed_by_actor_id",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE acp_permission_requests ADD COLUMN human_review_notified_at INTEGER",
        "acp_permission_requests.human_review_notified_at",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE team_conversation_messages ADD COLUMN idempotency_key TEXT",
        "team_conversation_messages.idempotency_key",
    )
    .await;
    create_team_conversation_messages_idempotency_index(&pool).await?;
    add_column_if_missing(
        &pool,
        "ALTER TABLE team_actor_messages ADD COLUMN from_peer_id TEXT NOT NULL DEFAULT 'main'",
        "team_actor_messages.from_peer_id",
    )
    .await;
    add_column_if_missing(
        &pool,
        "ALTER TABLE team_actor_messages ADD COLUMN to_peer_id TEXT NOT NULL DEFAULT 'main'",
        "team_actor_messages.to_peer_id",
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
    if let Err(err) = sqlx::query("DROP INDEX IF EXISTS idx_team_actor_messages_idempotency")
        .execute(&pool)
        .await
    {
        tracing::warn!(
            "db init: failed to drop idx_team_actor_messages_idempotency: {}",
            err
        );
    }
    if let Err(err) = sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_team_actor_messages_idempotency
        ON team_actor_messages(run_id, from_actor_id, from_peer_id, idempotency_key)
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
    delete_batch_size: u32,
) -> anyhow::Result<AgentEventCleanupResult> {
    const SECONDS_PER_DAY: i64 = 24 * 60 * 60;
    let retention_seconds = i64::from(retention_days).saturating_mul(SECONDS_PER_DAY);
    let cutoff_ts = chrono::Utc::now()
        .timestamp()
        .saturating_sub(retention_seconds);
    let batch_size = i64::from(delete_batch_size.max(1));
    let mut deleted_rows = 0_u64;
    let mut delete_batches = 0_u64;
    loop {
        let deleted = sqlx::query(
            r#"
            DELETE FROM agent_events
            WHERE id IN (
                SELECT id
                FROM agent_events
                WHERE ts < ?1
                ORDER BY ts, id
                LIMIT ?2
            )
            "#,
        )
        .bind(cutoff_ts)
        .bind(batch_size)
        .execute(pool)
        .await
        .map_err(|err| {
            tracing::error!(
                cutoff_ts,
                retention_days,
                batch_size,
                delete_batches,
                error = %err,
                "db cleanup failed during batched agent_events delete"
            );
            err
        })?
        .rows_affected();
        if deleted == 0 {
            break;
        }
        deleted_rows = deleted_rows.saturating_add(deleted);
        delete_batches = delete_batches.saturating_add(1);
    }

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
        delete_batches,
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

async fn create_team_conversation_messages_idempotency_index(
    pool: &SqlitePool,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_team_conversation_messages_idempotency
        ON team_conversation_messages(conversation_id, from_actor_id, idempotency_key)
        WHERE idempotency_key IS NOT NULL
        "#,
    )
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|err| {
        anyhow::anyhow!(
            "db init: failed to create idx_team_conversation_messages_idempotency: {}",
            err
        )
    })
}

async fn try_connect(db_path: &std::path::Path) -> anyhow::Result<SqlitePool> {
    ensure_sqlite_path(db_path)?;
    let pool = connect_sqlite_with_defaults(db_path, 5).await?;
    Ok(pool)
}

fn default_db_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&home).join(".agenthub/agenthub.db")
}

fn default_agent_event_db_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&home).join(".agenthub/agent-events")
}

async fn connect_sqlite_with_defaults(
    db_path: &std::path::Path,
    max_connections: u32,
) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await
        .map_err(|err| {
            tracing::error!(
                db_path = %db_path.display(),
                error = %err,
                "db connect_with failed"
            );
            err
        })?;
    Ok(pool)
}

fn ensure_sqlite_path(db_path: &std::path::Path) -> anyhow::Result<()> {
    create_parent_dir(db_path)?;
    Ok(())
}

fn create_parent_dir(path: &std::path::Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| {
            tracing::error!(
                parent = %parent.display(),
                error = %err,
                "db init failed to create parent directory"
            );
            err
        })?;
    }
    Ok(())
}

async fn init_agent_event_db_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS agent_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            seq TEXT NOT NULL,
            ts INTEGER NOT NULL,
            stream TEXT NOT NULL,
            message BLOB NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_agent_events_session_id
        ON agent_events(session_id, id);
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_agent_events_session_seq
        ON agent_events(session_id, seq);
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_agent_events_ts
        ON agent_events(ts);
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn migrate_agent_events_message_column_to_blob(pool: &SqlitePool) -> anyhow::Result<()> {
    let columns = sqlx::query("PRAGMA table_info(agent_events)")
        .fetch_all(pool)
        .await?;
    if columns.is_empty() {
        return Ok(());
    }

    let message_type = columns.iter().find_map(|row| {
        let name = row.get::<String, _>("name");
        if name.eq_ignore_ascii_case("message") {
            Some(row.get::<String, _>("type"))
        } else {
            None
        }
    });

    if message_type
        .as_deref()
        .map(|ty| ty.eq_ignore_ascii_case("BLOB"))
        .unwrap_or(false)
    {
        return Ok(());
    }

    tracing::info!(
        from = ?message_type,
        "db init: migrating agent_events.message column to BLOB"
    );

    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        CREATE TABLE agent_events_migrated (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            seq TEXT NOT NULL,
            ts INTEGER NOT NULL,
            stream TEXT NOT NULL,
            message BLOB NOT NULL,
            FOREIGN KEY(agent_id) REFERENCES agents(id),
            FOREIGN KEY(session_id) REFERENCES agent_sessions(id)
        );
        "#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO agent_events_migrated (id, agent_id, session_id, seq, ts, stream, message)
        SELECT id, agent_id, session_id, seq, ts, stream, CAST(message AS BLOB)
        FROM agent_events
        ORDER BY id ASC
        "#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query("DROP TABLE agent_events")
        .execute(&mut *tx)
        .await?;
    sqlx::query("ALTER TABLE agent_events_migrated RENAME TO agent_events")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn migrate_legacy_team_task_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    let legacy_main_tasks_exists = sqlite_table_exists(pool, "team_main_tasks").await?;
    let conversations_use_main_task_id =
        sqlite_table_has_column(pool, "team_conversations", "main_task_id").await?;
    let messages_use_main_task_id =
        sqlite_table_has_column(pool, "team_conversation_messages", "main_task_id").await?;

    if !legacy_main_tasks_exists && !conversations_use_main_task_id && !messages_use_main_task_id {
        return Ok(());
    }

    tracing::info!(
        legacy_main_tasks_exists,
        conversations_use_main_task_id,
        messages_use_main_task_id,
        "db init: migrating legacy team task schema"
    );

    let mut tx = pool.begin().await?;
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *tx)
        .await?;

    if legacy_main_tasks_exists {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO team_tasks (
                id, team_id, title, status, created_by_actor_id, context_json, created_at, updated_at
            )
            SELECT
                id, team_id, title, status, created_by_actor_id, context_json, created_at, updated_at
            FROM team_main_tasks
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .execute(&mut *tx)
        .await?;
    }

    if conversations_use_main_task_id {
        sqlx::query("DROP TABLE IF EXISTS team_conversations_migrated")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            CREATE TABLE team_conversations_migrated (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                task_id TEXT NOT NULL UNIQUE,
                mode TEXT NOT NULL,
                topic TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY(team_id) REFERENCES team_definitions(id),
                FOREIGN KEY(task_id) REFERENCES team_tasks(id)
            );
            "#,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO team_conversations_migrated (
                id, team_id, task_id, mode, topic, created_at, updated_at
            )
            SELECT
                id, team_id, main_task_id, mode, topic, created_at, updated_at
            FROM team_conversations
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .execute(&mut *tx)
        .await?;
    }

    if messages_use_main_task_id {
        sqlx::query("DROP TABLE IF EXISTS team_conversation_messages_staged")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            CREATE TABLE team_conversation_messages_staged (
                id INTEGER PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                from_actor_id TEXT NOT NULL,
                to_actor_id TEXT,
                route TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO team_conversation_messages_staged (
                id, conversation_id, task_id, from_actor_id, to_actor_id, route, payload_json, created_at
            )
            SELECT
                id, conversation_id, main_task_id, from_actor_id, to_actor_id, route, payload_json, created_at
            FROM team_conversation_messages
            ORDER BY id ASC
            "#,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query("DROP TABLE team_conversation_messages")
            .execute(&mut *tx)
            .await?;
    }

    if conversations_use_main_task_id {
        sqlx::query("DROP TABLE team_conversations")
            .execute(&mut *tx)
            .await?;
        sqlx::query("ALTER TABLE team_conversations_migrated RENAME TO team_conversations")
            .execute(&mut *tx)
            .await?;
    }

    if legacy_main_tasks_exists {
        sqlx::query("DROP TABLE team_main_tasks")
            .execute(&mut *tx)
            .await?;
    }

    if messages_use_main_task_id {
        sqlx::query(
            r#"
            CREATE TABLE team_conversation_messages_migrated (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                from_actor_id TEXT NOT NULL,
                to_actor_id TEXT,
                route TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(conversation_id) REFERENCES team_conversations(id),
                FOREIGN KEY(task_id) REFERENCES team_tasks(id)
            );
            "#,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO team_conversation_messages_migrated (
                id, conversation_id, task_id, from_actor_id, to_actor_id, route, payload_json, created_at
            )
            SELECT
                id, conversation_id, task_id, from_actor_id, to_actor_id, route, payload_json, created_at
            FROM team_conversation_messages_staged
            ORDER BY id ASC
            "#,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query("DROP TABLE team_conversation_messages_staged")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "ALTER TABLE team_conversation_messages_migrated RENAME TO team_conversation_messages",
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

async fn migrate_team_tasks_add_assigned_member_id(pool: &SqlitePool) -> anyhow::Result<()> {
    let has_column = sqlx::query_scalar::<_, String>(
        r#"
        SELECT name
        FROM pragma_table_info('team_tasks')
        WHERE name = 'assigned_member_id'
        "#,
    )
    .fetch_optional(pool)
    .await?
    .is_some();
    if has_column {
        return Ok(());
    }

    sqlx::query("ALTER TABLE team_tasks ADD COLUMN assigned_member_id TEXT")
        .execute(pool)
        .await?;
    Ok(())
}

async fn migrate_safe_paths_to_absolute(pool: &SqlitePool) -> anyhow::Result<()> {
    let rows = sqlx::query("SELECT id, path, created_at FROM safe_paths ORDER BY id ASC")
        .fetch_all(pool)
        .await?;
    if rows.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;
    let mut migrated = 0_u64;
    for row in rows {
        let id: i64 = row.get("id");
        let path: String = row.get("path");
        let created_at: i64 = row.get("created_at");
        let expanded = expand_tilde(&path);
        if expanded == path {
            continue;
        }
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO safe_paths (path, created_at)
            VALUES (?1, ?2)
            "#,
        )
        .bind(&expanded)
        .bind(created_at)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM safe_paths WHERE id = ?1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        migrated += 1;
    }
    tx.commit().await?;

    if migrated > 0 {
        tracing::info!(
            migrated_safe_path_count = migrated,
            "db init: normalized safe_paths entries to absolute paths"
        );
    }
    Ok(())
}

async fn sqlite_table_exists(pool: &SqlitePool, table_name: &str) -> anyhow::Result<bool> {
    let exists = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT 1
        FROM sqlite_master
        WHERE type = 'table' AND name = ?1
        LIMIT 1
        "#,
    )
    .bind(table_name)
    .fetch_optional(pool)
    .await?;
    Ok(exists.is_some())
}

async fn sqlite_table_has_column(
    pool: &SqlitePool,
    table_name: &str,
    column_name: &str,
) -> anyhow::Result<bool> {
    if !sqlite_table_exists(pool, table_name).await? {
        return Ok(false);
    }
    let query = format!("PRAGMA table_info('{table_name}')");
    let columns = sqlx::query(&query).fetch_all(pool).await?;
    Ok(columns.iter().any(|row| {
        row.get::<String, _>("name")
            .eq_ignore_ascii_case(column_name)
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        AgentEventDbRouter, AgentEventIdleGc, cleanup_agent_event_history, create_parent_dir,
        init_db_at_path, try_connect,
    };
    use agenthub_config::path_utils::expand_tilde;
    use sqlx::Row;
    use sqlx::SqlitePool;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::time::Duration;
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
                'team_tasks',
                'team_conversations',
                'team_conversation_messages',
                'team_actor_messages',
                'team_member_continuity_state',
                'team_context_artifacts',
                'team_context_flush_checkpoint'
              )
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("count tables");
        assert_eq!(table_count, 11);

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
    async fn init_db_normalizes_safe_paths_to_absolute_paths() {
        let dir = unique_temp_dir("db-safe-path-migration");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("agenthub.db");
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect sqlite");

        sqlx::query(
            r#"
            CREATE TABLE safe_paths (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create safe_paths");
        sqlx::query("INSERT INTO safe_paths (path, created_at) VALUES (?1, ?2)")
            .bind("~/.agenthub/worktrees")
            .bind(1_i64)
            .execute(&pool)
            .await
            .expect("insert legacy safe path");
        pool.close().await;

        let pool = init_db_at_path(&db_path).await.expect("init db");
        let rows = sqlx::query("SELECT path FROM safe_paths ORDER BY id ASC")
            .fetch_all(&pool)
            .await
            .expect("load safe paths");
        let paths = rows
            .into_iter()
            .map(|row| row.get::<String, _>("path"))
            .collect::<Vec<_>>();
        assert_eq!(paths, vec![expand_tilde("~/.agenthub/worktrees")]);

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn init_db_adds_agent_nodes_default_worktree_root_column() {
        let dir = unique_temp_dir("db-agent-nodes-default-worktree-root");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("agenthub.db");
        let pool = try_connect(&db_path).await.expect("connect sqlite");

        sqlx::query(
            r#"
            CREATE TABLE agent_nodes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                grpc_target TEXT NOT NULL,
                tls_server_name TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create legacy agent_nodes");
        pool.close().await;

        let pool = init_db_at_path(&db_path).await.expect("init db");
        let rows = sqlx::query("SELECT name FROM pragma_table_info('agent_nodes')")
            .fetch_all(&pool)
            .await
            .expect("load pragma table info");
        let column_names = rows
            .into_iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();
        assert!(
            column_names
                .iter()
                .any(|name| name == "default_worktree_root"),
            "agent_nodes columns missing default_worktree_root: {column_names:?}"
        );

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn init_db_migrates_agent_events_message_column_to_blob() {
        let dir = unique_temp_dir("db-migrate-agent-events");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("agenthub.db");
        let pool = try_connect(&db_path).await.expect("connect sqlite");

        sqlx::query(
            r#"
            CREATE TABLE agents (
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
        .await
        .expect("create agents");
        sqlx::query(
            r#"
            CREATE TABLE agent_sessions (
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
        .await
        .expect("create agent_sessions");
        sqlx::query(
            r#"
            CREATE TABLE agent_events (
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
        .await
        .expect("create legacy agent_events");

        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
                code_mode, source, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
        )
        .bind("agent-migrate")
        .bind("migrate-agent")
        .bind("/tmp")
        .bind("echo")
        .bind("[]")
        .bind("reuse_worktree")
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(0_i64)
        .bind("manual")
        .bind("running")
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
        .bind("session-migrate")
        .bind("agent-migrate")
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
        .bind("agent-migrate")
        .bind("session-migrate")
        .bind("seq-legacy")
        .bind(now)
        .bind("acp")
        .bind("{\"type\":\"agent_message\",\"text\":\"legacy\"}")
        .execute(&pool)
        .await
        .expect("insert legacy agent_event row");
        pool.close().await;

        let pool = init_db_at_path(&db_path)
            .await
            .expect("init db with migration");

        let message_column_type: String = sqlx::query_scalar(
            r#"
            SELECT type
            FROM pragma_table_info('agent_events')
            WHERE name = 'message'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("read message column type");
        assert!(
            message_column_type.eq_ignore_ascii_case("BLOB"),
            "expected message column type to migrate to BLOB, got {message_column_type}"
        );

        let message_storage_type: String =
            sqlx::query_scalar("SELECT typeof(message) FROM agent_events WHERE seq = 'seq-legacy'")
                .fetch_one(&pool)
                .await
                .expect("read message storage type");
        assert_eq!(
            message_storage_type, "blob",
            "expected migrated row storage type to be blob"
        );

        let stored_message: Vec<u8> =
            sqlx::query_scalar("SELECT message FROM agent_events WHERE seq = 'seq-legacy'")
                .fetch_one(&pool)
                .await
                .expect("read migrated message bytes");
        assert_eq!(
            stored_message,
            b"{\"type\":\"agent_message\",\"text\":\"legacy\"}".to_vec()
        );

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn init_db_migrates_legacy_team_task_schema() {
        let dir = unique_temp_dir("db-migrate-legacy-team-task-schema");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("agenthub.db");
        let pool = try_connect(&db_path).await.expect("connect sqlite");

        sqlx::query(
            r#"
            CREATE TABLE team_definitions (
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
        .await
        .expect("create team_definitions");
        sqlx::query(
            r#"
            CREATE TABLE team_main_tasks (
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
        .await
        .expect("create legacy team_main_tasks");
        sqlx::query(
            r#"
            CREATE TABLE team_conversations (
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
        .await
        .expect("create legacy team_conversations");
        sqlx::query(
            r#"
            CREATE TABLE team_conversation_messages (
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
        .await
        .expect("create legacy team_conversation_messages");

        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO team_definitions (
                id, name, description, spec_json, owner_user_id, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind("team-legacy")
        .bind("legacy-team")
        .bind(Some("legacy task schema"))
        .bind(r#"{"members":[{"member_id":"leader","role":"leader"}]}"#)
        .bind(Some("root"))
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert team");
        sqlx::query(
            r#"
            INSERT INTO team_main_tasks (
                id, team_id, title, status, created_by_actor_id, context_json, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind("task-legacy")
        .bind("team-legacy")
        .bind("all")
        .bind("open")
        .bind("user:test")
        .bind("{}")
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert legacy main task");
        sqlx::query(
            r#"
            INSERT INTO team_conversations (
                id, team_id, main_task_id, mode, topic, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind("conversation-legacy")
        .bind("team-legacy")
        .bind("task-legacy")
        .bind("group_chat")
        .bind(Some("all"))
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert legacy conversation");
        sqlx::query(
            r#"
            INSERT INTO team_conversation_messages (
                id, conversation_id, main_task_id, from_actor_id, to_actor_id, route, payload_json, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(1_i64)
        .bind("conversation-legacy")
        .bind("task-legacy")
        .bind("user:test")
        .bind(None::<String>)
        .bind("group_chat")
        .bind(r#"{"text":"hello"}"#)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert legacy message");
        pool.close().await;

        let pool = init_db_at_path(&db_path)
            .await
            .expect("init db with legacy team-task migration");

        let legacy_exists: Option<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'team_main_tasks'",
        )
        .fetch_optional(&pool)
        .await
        .expect("query legacy table");
        assert!(
            legacy_exists.is_none(),
            "legacy team_main_tasks should be removed"
        );

        let task_id_column: String = sqlx::query_scalar(
            r#"
            SELECT name
            FROM pragma_table_info('team_conversations')
            WHERE name = 'task_id'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("read migrated task_id column");
        assert_eq!(task_id_column, "task_id");

        let main_task_id_column: Option<String> = sqlx::query_scalar(
            r#"
            SELECT name
            FROM pragma_table_info('team_conversations')
            WHERE name = 'main_task_id'
            "#,
        )
        .fetch_optional(&pool)
        .await
        .expect("read removed main_task_id column");
        assert!(main_task_id_column.is_none());

        let migrated_task_row = sqlx::query(
            r#"
            SELECT id, team_id, title, created_by_actor_id
            FROM team_tasks
            WHERE id = 'task-legacy'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("read migrated task");
        assert_eq!(migrated_task_row.get::<String, _>("team_id"), "team-legacy");
        assert_eq!(migrated_task_row.get::<String, _>("title"), "all");
        assert_eq!(
            migrated_task_row.get::<String, _>("created_by_actor_id"),
            "user:test"
        );

        let migrated_conversation_row = sqlx::query(
            r#"
            SELECT id, team_id, task_id, mode, topic
            FROM team_conversations
            WHERE id = 'conversation-legacy'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("read migrated conversation");
        assert_eq!(
            migrated_conversation_row.get::<String, _>("task_id"),
            "task-legacy"
        );
        assert_eq!(
            migrated_conversation_row.get::<String, _>("mode"),
            "group_chat"
        );
        assert_eq!(
            migrated_conversation_row.get::<Option<String>, _>("topic"),
            Some("all".to_string())
        );

        let migrated_message_row = sqlx::query(
            r#"
            SELECT id, conversation_id, task_id, route, payload_json
            FROM team_conversation_messages
            WHERE id = 1
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("read migrated message");
        assert_eq!(
            migrated_message_row.get::<String, _>("conversation_id"),
            "conversation-legacy"
        );
        assert_eq!(
            migrated_message_row.get::<String, _>("task_id"),
            "task-legacy"
        );
        assert_eq!(migrated_message_row.get::<String, _>("route"), "group_chat");
        assert_eq!(
            migrated_message_row.get::<String, _>("payload_json"),
            r#"{"text":"hello"}"#
        );

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn init_db_adds_assigned_member_id_to_existing_team_tasks_table() {
        let dir = unique_temp_dir("db-migrate-team-task-assignee");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("agenthub.db");
        let pool = try_connect(&db_path).await.expect("connect sqlite");

        sqlx::query(
            r#"
            CREATE TABLE team_tasks (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                created_by_actor_id TEXT NOT NULL,
                context_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create legacy team_tasks");
        pool.close().await;

        let pool = init_db_at_path(&db_path)
            .await
            .expect("init db with team task assignee migration");

        let assigned_member_id_column: String = sqlx::query_scalar(
            r#"
            SELECT name
            FROM pragma_table_info('team_tasks')
            WHERE name = 'assigned_member_id'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("read assigned_member_id column");
        assert_eq!(assigned_member_id_column, "assigned_member_id");

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn init_db_adds_task_message_idempotency_key_and_index_to_existing_messages_table() {
        let dir = unique_temp_dir("db-migrate-task-message-idempotency");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("agenthub.db");
        let pool = try_connect(&db_path).await.expect("connect sqlite");

        sqlx::query(
            r#"
            CREATE TABLE team_conversation_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                from_actor_id TEXT NOT NULL,
                to_actor_id TEXT,
                route TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create legacy team_conversation_messages");
        pool.close().await;

        let pool = init_db_at_path(&db_path)
            .await
            .expect("init db with task message idempotency migration");

        let idempotency_key_column: String = sqlx::query_scalar(
            r#"
            SELECT name
            FROM pragma_table_info('team_conversation_messages')
            WHERE name = 'idempotency_key'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("read idempotency_key column");
        assert_eq!(idempotency_key_column, "idempotency_key");

        let index_sql: String = sqlx::query_scalar(
            r#"
            SELECT sql
            FROM sqlite_master
            WHERE type = 'index'
              AND name = 'idx_team_conversation_messages_idempotency'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("read task message idempotency index");
        assert!(index_sql.contains("conversation_id"));
        assert!(index_sql.contains("from_actor_id"));
        assert!(index_sql.contains("idempotency_key"));

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn init_db_fails_when_task_message_idempotency_index_cannot_be_created() {
        let dir = unique_temp_dir("db-migrate-task-message-idempotency-duplicates");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("agenthub.db");
        let pool = try_connect(&db_path).await.expect("connect sqlite");

        sqlx::query(
            r#"
            CREATE TABLE team_conversation_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                from_actor_id TEXT NOT NULL,
                to_actor_id TEXT,
                route TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                idempotency_key TEXT,
                created_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create legacy team_conversation_messages with idempotency_key");
        sqlx::query(
            r#"
            INSERT INTO team_conversation_messages (
                conversation_id,
                task_id,
                from_actor_id,
                to_actor_id,
                route,
                payload_json,
                idempotency_key,
                created_at
            )
            VALUES
                ('conv-1', 'task-1', 'user', NULL, 'group_chat', '{}', 'dup-key', 1),
                ('conv-1', 'task-1', 'user', NULL, 'group_chat', '{}', 'dup-key', 2)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert duplicate idempotency rows");
        pool.close().await;

        let err = init_db_at_path(&db_path)
            .await
            .expect_err("duplicate idempotency rows should fail migration");
        assert!(
            err.to_string()
                .contains("idx_team_conversation_messages_idempotency"),
            "unexpected error: {err:?}"
        );

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

    #[test]
    fn create_parent_dir_returns_error_when_parent_is_file() {
        let dir = unique_temp_dir("db-parent-fail");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let blocker = dir.join("not-a-directory");
        std::fs::write(&blocker, "block").expect("create blocker file");

        let file_path = blocker.join("child.sqlite");
        let err = create_parent_dir(&file_path).expect_err("parent file should fail mkdir");
        assert!(
            !err.to_string().is_empty(),
            "expected non-empty create_parent_dir error"
        );

        let _ = std::fs::remove_file(&blocker);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn try_connect_returns_error_for_directory_path() {
        let dir = unique_temp_dir("db-connect-dir");
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let err = try_connect(&dir)
            .await
            .expect_err("sqlite filename pointing to directory should fail");
        assert!(
            !err.to_string().is_empty(),
            "expected non-empty sqlite error"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn init_db_at_path_returns_error_for_directory_path() {
        let dir = unique_temp_dir("db-init-dir");
        std::fs::create_dir_all(&dir).expect("create temp dir");

        let err = init_db_at_path(&dir)
            .await
            .expect_err("init db should fail when db path points to directory");
        assert!(!err.to_string().is_empty(), "expected non-empty init error");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn cleanup_agent_event_history_returns_error_without_agent_events_table() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect scratch sqlite");

        let err = cleanup_agent_event_history(&pool, 5, false, 1_000)
            .await
            .expect_err("cleanup should fail without agent_events table");
        assert!(
            !err.to_string().is_empty(),
            "expected non-empty cleanup error"
        );

        pool.close().await;
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

        let result = cleanup_agent_event_history(&pool, 5, false, 1)
            .await
            .expect("cleanup history");
        assert_eq!(result.deleted_rows, 1);
        assert_eq!(result.delete_batches, 1);
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

    #[tokio::test]
    async fn cleanup_agent_event_history_uses_ts_index_for_batch_selection() {
        let dir = unique_temp_dir("db-cleanup-plan");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("cleanup-plan.db");
        let pool = init_db_at_path(&db_path).await.expect("init db");

        let now = chrono::Utc::now().timestamp();
        let cutoff_ts = now - (5 * 24 * 60 * 60);
        let rows = sqlx::query(
            r#"
            EXPLAIN QUERY PLAN
            SELECT id
            FROM agent_events
            WHERE ts < ?1
            ORDER BY ts, id
            LIMIT ?2
            "#,
        )
        .bind(cutoff_ts)
        .bind(1000_i64)
        .fetch_all(&pool)
        .await
        .expect("explain cleanup subquery");
        let details: Vec<String> = rows.iter().map(|row| row.get("detail")).collect();
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("idx_agent_events_ts")),
            "expected cleanup plan to use idx_agent_events_ts, got: {details:?}"
        );

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn idle_gc_checks_only_once_per_idle_window() {
        let dir = unique_temp_dir("db-idle-gc-once");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let event_dbs = AgentEventDbRouter::new(dir.clone());
        let idle_gc =
            AgentEventIdleGc::new(event_dbs.clone(), 5, false, 100, Duration::from_millis(60));
        let pool = event_dbs
            .pool_for_agent("agent-idle-gc")
            .await
            .expect("open per-agent db");
        let old_ts = chrono::Utc::now().timestamp() - (10 * 24 * 60 * 60);

        sqlx::query(
            r#"
            INSERT INTO agent_events (session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind("session-idle")
        .bind("seq-1")
        .bind(old_ts)
        .bind("stdout")
        .bind("first-old")
        .execute(&pool)
        .await
        .expect("insert first old event");

        idle_gc.record_activity("agent-idle-gc").await;

        let remaining_after_first_check =
            wait_for_old_event_count(&pool, old_ts + 1, 0, Duration::from_millis(500)).await;
        assert_eq!(remaining_after_first_check, 0);
        wait_for_idle_gc_generation_completion(
            &idle_gc,
            "agent-idle-gc",
            1,
            Duration::from_millis(500),
        )
        .await;

        sqlx::query(
            r#"
            INSERT INTO agent_events (session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind("session-idle")
        .bind("seq-2")
        .bind(old_ts)
        .bind("stdout")
        .bind("second-old")
        .execute(&pool)
        .await
        .expect("insert second old event");

        tokio::time::sleep(Duration::from_millis(180)).await;
        let remaining_without_new_activity: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_events WHERE ts < ?1")
                .bind(old_ts + 1)
                .fetch_one(&pool)
                .await
                .expect("count without new activity");
        assert_eq!(
            remaining_without_new_activity, 1,
            "idle gc should not re-run without new activity"
        );

        idle_gc.record_activity("agent-idle-gc").await;
        let remaining_after_second_idle_window =
            wait_for_old_event_count(&pool, old_ts + 1, 0, Duration::from_millis(500)).await;
        assert_eq!(remaining_after_second_idle_window, 0);

        pool.close().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn wait_for_old_event_count(
        pool: &SqlitePool,
        cutoff_ts: i64,
        expected: i64,
        timeout: Duration,
    ) -> i64 {
        let started_at = tokio::time::Instant::now();
        let deadline = started_at + timeout;
        loop {
            let remaining: i64 =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent_events WHERE ts < ?1")
                    .bind(cutoff_ts)
                    .fetch_one(pool)
                    .await
                    .expect("count old events");
            if remaining == expected {
                return remaining;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!(
                    "timed out waiting for old event count: cutoff_ts={cutoff_ts}, expected={expected}, last_remaining={remaining}, timeout_ms={}",
                    timeout.as_millis()
                );
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn wait_for_idle_gc_generation_completion(
        idle_gc: &AgentEventIdleGc,
        agent_id: &str,
        expected_generation: u64,
        timeout: Duration,
    ) {
        let started_at = tokio::time::Instant::now();
        let deadline = started_at + timeout;
        loop {
            let completed_generation = {
                let states = idle_gc.states.lock().await;
                states
                    .get(agent_id)
                    .map(|state| state.completed_generation)
                    .unwrap_or_default()
            };
            if completed_generation >= expected_generation {
                return;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!(
                    "timed out waiting for idle gc generation completion: agent_id={agent_id}, expected_generation={expected_generation}, completed_generation={completed_generation}, timeout_ms={}",
                    timeout.as_millis()
                );
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[tokio::test]
    async fn remove_agent_db_retries_cleanup_and_reopens_empty_history() {
        let dir = unique_temp_dir("db-remove-agent");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let event_dbs = AgentEventDbRouter::new(dir.clone());
        let agent_id = "agent-remove";
        let db_path = event_dbs.db_path_for_agent(agent_id);
        let wal_path = AgentEventDbRouter::suffixed_path(&db_path, "-wal");
        let shm_path = AgentEventDbRouter::suffixed_path(&db_path, "-shm");

        let pool = event_dbs
            .pool_for_agent(agent_id)
            .await
            .expect("open per-agent db");
        sqlx::query(
            r#"
            INSERT INTO agent_events (session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind("session-remove")
        .bind("seq-1")
        .bind(chrono::Utc::now().timestamp())
        .bind("stdout")
        .bind("cleanup-test")
        .execute(&pool)
        .await
        .expect("insert event");

        assert!(
            tokio::fs::try_exists(&db_path)
                .await
                .expect("check db path")
        );

        event_dbs
            .remove_agent_db(agent_id)
            .await
            .expect("remove agent db");

        assert!(
            !tokio::fs::try_exists(&db_path)
                .await
                .expect("check db removal")
        );
        assert!(
            !tokio::fs::try_exists(&wal_path)
                .await
                .expect("check wal removal")
        );
        assert!(
            !tokio::fs::try_exists(&shm_path)
                .await
                .expect("check shm removal")
        );

        let reopened = event_dbs
            .pool_for_agent(agent_id)
            .await
            .expect("reopen per-agent db");
        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_events")
            .fetch_one(&reopened)
            .await
            .expect("count reopened events");
        assert_eq!(event_count, 0);

        reopened.close().await;
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn cleanup_agent_event_history_deletes_in_multiple_batches() {
        let dir = unique_temp_dir("db-cleanup-batch");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("cleanup-batch.db");
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
        .bind("agent-cleanup-batch")
        .bind("cleanup-batch-agent")
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
        .bind("session-cleanup-batch")
        .bind("agent-cleanup-batch")
        .bind("running")
        .bind(now)
        .bind(None::<i64>)
        .execute(&pool)
        .await
        .expect("insert session");

        for seq in ["seq-old-1", "seq-old-2", "seq-old-3"] {
            sqlx::query(
                r#"
                INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
            )
            .bind("agent-cleanup-batch")
            .bind("session-cleanup-batch")
            .bind(seq)
            .bind(old_ts)
            .bind("acp")
            .bind("old")
            .execute(&pool)
            .await
            .expect("insert old event");
        }

        sqlx::query(
            r#"
            INSERT INTO agent_events (agent_id, session_id, seq, ts, stream, message)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind("agent-cleanup-batch")
        .bind("session-cleanup-batch")
        .bind("seq-new")
        .bind(new_ts)
        .bind("acp")
        .bind("new")
        .execute(&pool)
        .await
        .expect("insert new event");

        let result = cleanup_agent_event_history(&pool, 5, false, 1)
            .await
            .expect("cleanup history");
        assert_eq!(result.deleted_rows, 3);
        assert_eq!(result.delete_batches, 3);
        assert!(!result.vacuum_ran);

        let remaining_old: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_events WHERE session_id = ?1 AND ts < ?2",
        )
        .bind("session-cleanup-batch")
        .bind(result.cutoff_ts)
        .fetch_one(&pool)
        .await
        .expect("count remaining old events");
        assert_eq!(remaining_old, 0);

        let remaining_total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_events WHERE session_id = ?1")
                .bind("session-cleanup-batch")
                .fetch_one(&pool)
                .await
                .expect("count remaining session events");
        assert_eq!(remaining_total, 1);

        pool.close().await;
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
