use sqlx::SqlitePool;

pub async fn migrate_mcp_operations(pool: &SqlitePool) -> anyhow::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS mcp_operations (
            id TEXT PRIMARY KEY,
            actor_id TEXT NOT NULL,
            team_id TEXT NOT NULL,
            origin_activation_id TEXT NOT NULL REFERENCES loop_activations(id),
            request_key TEXT NOT NULL,
            server_id TEXT NOT NULL,
            scope_digest TEXT NOT NULL,
            tool_name TEXT NOT NULL,
            arguments_digest TEXT NOT NULL,
            identity_digest TEXT,
            intent_json TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN (
                'prepared', 'sent', 'succeeded', 'failed', 'outcome_unknown'
            )),
            attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
            completion_json TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(team_id, actor_id, request_key),
            FOREIGN KEY(actor_id, team_id) REFERENCES loop_policies(actor_id, team_id)
        );
        CREATE INDEX IF NOT EXISTS idx_mcp_operation_semantics
            ON mcp_operations(team_id, scope_digest, tool_name, arguments_digest);
        CREATE INDEX IF NOT EXISTS idx_mcp_operation_identity
            ON mcp_operations(team_id, scope_digest, tool_name, identity_digest)
            WHERE identity_digest IS NOT NULL;
        CREATE INDEX IF NOT EXISTS idx_mcp_operation_actor_history
            ON mcp_operations(team_id, actor_id, created_at, id);
        CREATE TABLE IF NOT EXISTS mcp_operation_attempts (
            operation_id TEXT NOT NULL REFERENCES mcp_operations(id),
            number INTEGER NOT NULL CHECK(number > 0),
            permit_id TEXT NOT NULL UNIQUE,
            activation_id TEXT NOT NULL REFERENCES loop_activations(id),
            generation INTEGER NOT NULL CHECK(generation > 0),
            daemon_node_id TEXT NOT NULL,
            daemon_generation INTEGER NOT NULL CHECK(daemon_generation > 0),
            daemon_owner_id TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('sent', 'succeeded', 'failed', 'outcome_unknown')),
            completion_json TEXT,
            sent_at INTEGER NOT NULL,
            completed_at INTEGER,
            PRIMARY KEY(operation_id, number)
        );
        CREATE INDEX IF NOT EXISTS idx_mcp_attempt_recovery
            ON mcp_operation_attempts(daemon_node_id, status, daemon_generation);
        CREATE TABLE IF NOT EXISTS mcp_operation_continuations (
            operation_id TEXT NOT NULL,
            attempt_number INTEGER NOT NULL,
            parent_attempt_number INTEGER NOT NULL CHECK(parent_attempt_number > 0),
            parent_response_digest TEXT NOT NULL,
            request_key TEXT NOT NULL,
            request_id_digest TEXT NOT NULL,
            request_digest TEXT NOT NULL,
            PRIMARY KEY(operation_id, attempt_number),
            UNIQUE(operation_id, parent_attempt_number),
            UNIQUE(operation_id, request_id_digest),
            CHECK(attempt_number = parent_attempt_number + 1),
            FOREIGN KEY(operation_id, attempt_number)
                REFERENCES mcp_operation_attempts(operation_id, number),
            FOREIGN KEY(operation_id, parent_attempt_number)
                REFERENCES mcp_operation_attempts(operation_id, number)
        );
        CREATE INDEX IF NOT EXISTS idx_mcp_continuation_request
            ON mcp_operation_continuations(request_key);
        CREATE TABLE IF NOT EXISTS mcp_operation_continuation_retries (
            operation_id TEXT NOT NULL,
            attempt_number INTEGER NOT NULL,
            continuation_attempt_number INTEGER NOT NULL,
            request_key TEXT NOT NULL,
            request_id_digest TEXT NOT NULL,
            PRIMARY KEY(operation_id, attempt_number),
            UNIQUE(operation_id, request_id_digest),
            CHECK(attempt_number > continuation_attempt_number),
            FOREIGN KEY(operation_id, attempt_number)
                REFERENCES mcp_operation_attempts(operation_id, number),
            FOREIGN KEY(operation_id, continuation_attempt_number)
                REFERENCES mcp_operation_continuations(operation_id, attempt_number)
        );
        CREATE INDEX IF NOT EXISTS idx_mcp_continuation_retry_request
            ON mcp_operation_continuation_retries(request_key);
        CREATE TABLE IF NOT EXISTS mcp_operation_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            operation_id TEXT NOT NULL REFERENCES mcp_operations(id),
            attempt_number INTEGER NOT NULL CHECK(attempt_number >= 0),
            activation_id TEXT NOT NULL REFERENCES loop_activations(id),
            status TEXT NOT NULL CHECK(status IN (
                'prepared', 'sent', 'succeeded', 'failed', 'outcome_unknown'
            )),
            completion_json TEXT,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_mcp_event_operation
            ON mcp_operation_events(operation_id, id);
        CREATE INDEX IF NOT EXISTS idx_mcp_event_activation
            ON mcp_operation_events(activation_id, id);
        "#,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
