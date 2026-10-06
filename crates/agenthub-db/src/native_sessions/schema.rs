use sqlx::{Sqlite, Transaction};

/// Writer arbitration needs both namespaces in the same migrated control store.
pub(crate) async fn migrate_in_transaction(tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<()> {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS native_execution_owners (
            agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
            generation INTEGER NOT NULL CHECK(generation > 0),
            local_session_id TEXT NOT NULL UNIQUE,
            owner_id TEXT NOT NULL,
            state TEXT NOT NULL CHECK(state IN ('reserved', 'guarded', 'retired')),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY(agent_id, generation),
            UNIQUE(agent_id, local_session_id)
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_native_execution_active
            ON native_execution_owners(agent_id) WHERE state != 'retired';
        CREATE TABLE IF NOT EXISTS native_standalone_conversations (
            agent_id TEXT PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE,
            local_session_id TEXT NOT NULL,
            configuration_digest TEXT NOT NULL,
            native_session_id TEXT,
            state TEXT NOT NULL CHECK(state IN ('opening', 'bound')),
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(agent_id, local_session_id)
                REFERENCES native_execution_owners(agent_id, local_session_id) ON DELETE CASCADE,
            CHECK(state != 'bound' OR native_session_id IS NOT NULL)
        );",
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
