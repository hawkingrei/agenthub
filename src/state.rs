use std::sync::Arc;

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::acp::AcpPermissionService;
use crate::agent::AgentManager;
use crate::auth::AuthService;
use crate::push::PushService;
use crate::team::{
    TeamManager, TeamOrchestratorWorker, TeamOrchestratorWorkerSettings,
    TeamRemoteRelayWorkerSettings,
};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub agents: Arc<AgentManager>,
    pub teams: Arc<TeamManager>,
    pub push: Arc<PushService>,
    pub auth: Arc<AuthService>,
    pub acp_permissions: Arc<AcpPermissionService>,
    pub default_worktree_root: String,
}

impl AppState {
    pub async fn init(config: crate::config::AppConfig) -> anyhow::Result<Self> {
        let db = crate::db::init_db().await?;
        if let Some(retention_days) = config.history_event_retention_days() {
            let vacuum_on_cleanup = config.history_vacuum_on_cleanup();
            match crate::db::cleanup_agent_event_history(&db, retention_days, vacuum_on_cleanup)
                .await
            {
                Ok(result) => {
                    if result.deleted_rows > 0 || result.vacuum_ran {
                        tracing::info!(
                            "history cleanup applied: retention_days={} cutoff_ts={} deleted_rows={} vacuum_ran={}",
                            retention_days,
                            result.cutoff_ts,
                            result.deleted_rows,
                            result.vacuum_ran
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        "history cleanup skipped due to error: retention_days={} error={}",
                        retention_days,
                        err
                    );
                }
            }
        }
        let push = Arc::new(PushService::new(db.clone(), &config)?);
        let acp_permissions = Arc::new(AcpPermissionService::new(db.clone()));
        let auth = Arc::new(AuthService::new(db.clone(), &config).await?);
        let agents = Arc::new(AgentManager::new(
            db.clone(),
            push.clone(),
            config.proxy_env(),
            config.codex_acp_binary(),
            config.codex_acp_default_mode(),
            acp_permissions.clone(),
            auth.clone(),
        ));
        let teams = Arc::new(TeamManager::new(db.clone()));
        teams
            .clone()
            .spawn_remote_relay_worker(TeamRemoteRelayWorkerSettings::default());
        agents.mark_exited_on_startup().await?;
        let startup_canceled_runs = teams.cancel_active_runs_on_startup().await?;
        if startup_canceled_runs > 0 {
            tracing::info!(
                canceled_run_count = startup_canceled_runs,
                "team startup policy: canceled active runs to require manual start"
            );
        }
        let _orchestrator_handle = TeamOrchestratorWorker::new(teams.clone(), agents.clone())
            .spawn(TeamOrchestratorWorkerSettings::default());
        Self::ensure_root(&db).await?;
        Self::seed_safe_paths(&db, &config).await?;
        let default_worktree_root = config.default_worktree_root();
        Ok(Self {
            db,
            agents,
            teams,
            push,
            auth,
            acp_permissions,
            default_worktree_root,
        })
    }

    async fn ensure_root(db: &SqlitePool) -> anyhow::Result<()> {
        let exists = sqlx::query(
            r#"
            SELECT id FROM users WHERE role = 'root' LIMIT 1
            "#,
        )
        .fetch_optional(db)
        .await?;

        if exists.is_none() {
            let id = Uuid::new_v4().to_string();
            let now = chrono::Utc::now().timestamp();
            sqlx::query(
                r#"
                INSERT INTO users (id, username, display_name, role, password_hash, created_at)
                VALUES (?1, ?2, ?3, 'root', NULL, ?4)
                "#,
            )
            .bind(&id)
            .bind("root")
            .bind("Root")
            .bind(now)
            .execute(db)
            .await?;
        }
        Ok(())
    }

    async fn seed_safe_paths(
        db: &SqlitePool,
        config: &crate::config::AppConfig,
    ) -> anyhow::Result<()> {
        for path in config.safe_paths() {
            let now = chrono::Utc::now().timestamp();
            let _ = sqlx::query(
                r#"
                INSERT OR IGNORE INTO safe_paths (path, created_at)
                VALUES (?1, ?2)
                "#,
            )
            .bind(path)
            .bind(now)
            .execute(db)
            .await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use crate::config::AppConfig;
    use sqlx::Row;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn test_db() -> sqlx::SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect sqlite");
        sqlx::query(
            r#"
            CREATE TABLE users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                role TEXT NOT NULL,
                password_hash TEXT,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create users table");
        sqlx::query(
            r#"
            CREATE TABLE safe_paths (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create safe_paths table");
        pool
    }

    #[tokio::test]
    async fn ensure_root_inserts_once() {
        let db = test_db().await;
        AppState::ensure_root(&db).await.expect("ensure root first");
        AppState::ensure_root(&db)
            .await
            .expect("ensure root second should be no-op");

        let row = sqlx::query("SELECT COUNT(*) AS cnt FROM users WHERE role = 'root'")
            .fetch_one(&db)
            .await
            .expect("count root users");
        let count: i64 = row.get("cnt");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn seed_safe_paths_inserts_distinct_paths() {
        let db = test_db().await;
        let config = AppConfig {
            safe_paths: Some(vec![
                "/tmp/a".to_string(),
                "/tmp/b".to_string(),
                "/tmp/a".to_string(),
            ]),
            ..Default::default()
        };

        AppState::seed_safe_paths(&db, &config)
            .await
            .expect("seed safe paths");

        let row = sqlx::query("SELECT COUNT(*) AS cnt FROM safe_paths")
            .fetch_one(&db)
            .await
            .expect("count safe paths");
        let count: i64 = row.get("cnt");
        assert_eq!(count, 3);

        let row = sqlx::query(
            "SELECT COUNT(*) AS cnt FROM safe_paths WHERE path = '~/.agenthub/worktrees'",
        )
        .fetch_one(&db)
        .await
        .expect("count default safe path");
        let default_count: i64 = row.get("cnt");
        assert_eq!(default_count, 1);
    }

    #[tokio::test]
    async fn seed_safe_paths_inserts_default_when_not_configured() {
        let db = test_db().await;
        let config = AppConfig::default();

        AppState::seed_safe_paths(&db, &config)
            .await
            .expect("seed default safe path");

        let row = sqlx::query("SELECT path FROM safe_paths ORDER BY id ASC")
            .fetch_one(&db)
            .await
            .expect("fetch default safe path");
        let path: String = row.get("path");
        assert_eq!(path, "~/.agenthub/worktrees");
    }
}
