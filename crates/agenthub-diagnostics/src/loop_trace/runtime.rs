use std::path::Path;

use agenthub_agent_domain::loop_runtime::LoopActivation;
use agenthub_db::runtime_events::{RuntimeEventStore, RuntimeHistory};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

pub(crate) async fn load(
    event_db_dir: &Path,
    activation: &LoopActivation,
    limit: i64,
) -> anyhow::Result<Option<RuntimeHistory>> {
    let Some(session_id) = activation.session_id.as_deref() else {
        return Ok(None);
    };
    let path = event_db_dir.join(format!("{}.db", activation.actor_id));
    if !path.try_exists()? {
        return Ok(None);
    }
    // Doctor must neither migrate a legacy event database nor allocate runtime ownership.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(path).read_only(true))
        .await?;
    let result = async {
        let available: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'runtime_event_owners')",
        )
        .fetch_one(&pool)
        .await?;
        if !available {
            return Ok(None);
        }
        let Some(store) = RuntimeEventStore::load(pool.clone(), session_id).await? else {
            return Ok(None);
        };
        store.history(limit, None).await.map(Some)
    }
    .await;
    pool.close().await;
    result
}
