use anyhow::Context;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonGeneration {
    pub node_id: String,
    pub generation: i64,
    pub owner_id: String,
    pub owner_pid: i64,
    pub claimed_at: i64,
}

pub(crate) async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS daemon_generations (
            node_id TEXT PRIMARY KEY,
            generation INTEGER NOT NULL CHECK(generation > 0),
            owner_id TEXT NOT NULL,
            owner_pid INTEGER NOT NULL,
            claimed_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .context("create daemon_generations table")?;
    Ok(())
}

pub async fn claim_daemon_generation(
    pool: &SqlitePool,
    node_id: &str,
    owner_id: &str,
    owner_pid: i64,
    claimed_at: i64,
) -> anyhow::Result<DaemonGeneration> {
    ensure_schema(pool).await?;
    let generation = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO daemon_generations (
            node_id,
            generation,
            owner_id,
            owner_pid,
            claimed_at,
            updated_at
        )
        VALUES (?1, 1, ?2, ?3, ?4, ?4)
        ON CONFLICT(node_id) DO UPDATE SET
            generation = daemon_generations.generation + 1,
            owner_id = excluded.owner_id,
            owner_pid = excluded.owner_pid,
            claimed_at = excluded.claimed_at,
            updated_at = excluded.updated_at
        RETURNING generation
        "#,
    )
    .bind(node_id)
    .bind(owner_id)
    .bind(owner_pid)
    .bind(claimed_at)
    .fetch_one(pool)
    .await
    .context("claim daemon generation")?;

    Ok(DaemonGeneration {
        node_id: node_id.to_string(),
        generation,
        owner_id: owner_id.to_string(),
        owner_pid,
        claimed_at,
    })
}

pub async fn is_current_daemon_generation(
    pool: &SqlitePool,
    generation: &DaemonGeneration,
) -> anyhow::Result<bool> {
    let current = sqlx::query_as::<_, (i64, String)>(
        r#"
        SELECT generation, owner_id
        FROM daemon_generations
        WHERE node_id = ?1
        "#,
    )
    .bind(&generation.node_id)
    .fetch_optional(pool)
    .await
    .context("read current daemon generation")?;

    Ok(matches!(
        current,
        Some((current_generation, current_owner_id))
            if current_generation == generation.generation
                && current_owner_id == generation.owner_id
    ))
}

#[cfg(test)]
mod tests {
    use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

    use super::{claim_daemon_generation, ensure_schema, is_current_daemon_generation};

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory database");
        ensure_schema(&pool).await.expect("create schema");
        pool
    }

    #[tokio::test]
    async fn claims_increment_per_node_and_fence_stale_owners() {
        let pool = test_pool().await;

        let first = claim_daemon_generation(&pool, "main", "owner-1", 101, 1_000)
            .await
            .expect("claim first generation");
        let other_node = claim_daemon_generation(&pool, "node-east", "owner-2", 102, 1_001)
            .await
            .expect("claim other node generation");
        assert_eq!(first.generation, 1);
        assert_eq!(other_node.generation, 1);
        assert!(
            is_current_daemon_generation(&pool, &first)
                .await
                .expect("verify first generation")
        );

        let replacement = claim_daemon_generation(&pool, "main", "owner-3", 103, 1_002)
            .await
            .expect("claim replacement generation");
        assert_eq!(replacement.generation, 2);
        assert!(
            !is_current_daemon_generation(&pool, &first)
                .await
                .expect("reject stale generation")
        );
        assert!(
            is_current_daemon_generation(&pool, &replacement)
                .await
                .expect("verify replacement generation")
        );
        assert!(
            is_current_daemon_generation(&pool, &other_node)
                .await
                .expect("other node remains current")
        );
    }
}
