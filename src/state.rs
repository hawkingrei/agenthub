use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::acp::AcpPermissionService;
use crate::agent::{
    AgentManager, AgentTimeTriggerManager, AgentTimeTriggerWorker, AgentTimeTriggerWorkerSettings,
};
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

const IDLE_GC_TIMEOUT_SECONDS: u64 = 5 * 60;
const GLOBAL_GITIGNORE_ENTRY: &str = ".agenthubmemory";
const GLOBAL_GITIGNORE_FILENAME: &str = ".gitignore_global";

impl AppState {
    pub async fn init(config: agenthub_config::AppConfig) -> anyhow::Result<Self> {
        if let Err(error) = Self::ensure_global_gitignore_agenthubmemory() {
            tracing::warn!(
                ?error,
                "failed to ensure global gitignore entry for .agenthubmemory"
            );
        }
        let db = Self::setup_database(&config).await?;
        let event_dbs = agenthub_db::AgentEventDbRouter::with_default_base_dir();

        let (agents, teams, push, auth, acp_permissions) =
            Self::initialize_services(&config, db.clone(), event_dbs.clone()).await?;

        Self::run_startup_cleanup(&agents, &teams).await?;

        let _orchestrator_handle = TeamOrchestratorWorker::new(teams.clone(), agents.clone())
            .spawn(TeamOrchestratorWorkerSettings::default());
        let trigger_manager = Arc::new(AgentTimeTriggerManager::new(db.clone()));
        let recovered_dispatching = trigger_manager.reset_inflight_on_startup().await?;
        if recovered_dispatching > 0 {
            tracing::info!(
                recovered_dispatching,
                "agent time triggers reset to scheduled on startup"
            );
        }
        let _agent_trigger_handle = AgentTimeTriggerWorker::new(trigger_manager, agents.clone())
            .spawn(AgentTimeTriggerWorkerSettings::default());

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

    async fn setup_database(config: &agenthub_config::AppConfig) -> anyhow::Result<SqlitePool> {
        let db = agenthub_db::init_db().await?;
        Self::ensure_root(&db).await?;
        Self::seed_safe_paths(&db, config).await?;
        Ok(db)
    }

    async fn initialize_services(
        config: &agenthub_config::AppConfig,
        db: SqlitePool,
        event_dbs: agenthub_db::AgentEventDbRouter,
    ) -> anyhow::Result<(
        Arc<AgentManager>,
        Arc<TeamManager>,
        Arc<PushService>,
        Arc<AuthService>,
        Arc<AcpPermissionService>,
    )> {
        let idle_gc = config.history_event_retention_days().map(|retention_days| {
            let vacuum_on_cleanup = config.history_vacuum_on_cleanup();
            let delete_batch_size = config.history_delete_batch_size();
            tracing::info!(
                "history gc configured with idle trigger: retention_days={} idle_timeout_seconds={} batch_size={} vacuum_on_cleanup={}",
                retention_days,
                IDLE_GC_TIMEOUT_SECONDS,
                delete_batch_size,
                vacuum_on_cleanup,
            );
            agenthub_db::AgentEventIdleGc::new(
                event_dbs.clone(),
                retention_days,
                vacuum_on_cleanup,
                delete_batch_size,
                std::time::Duration::from_secs(IDLE_GC_TIMEOUT_SECONDS),
            )
        });

        let push = Arc::new(PushService::new(db.clone(), config)?);
        let acp_permissions = Arc::new(AcpPermissionService::new(db.clone()));
        let auth = Arc::new(AuthService::new(db.clone(), config).await?);
        let agents = Arc::new(AgentManager::new(
            db.clone(),
            event_dbs.clone(),
            idle_gc,
            push.clone(),
            config.proxy_env(),
            config.codex_acp_binary(),
            config.codex_acp_default_mode(),
            acp_permissions.clone(),
            auth.clone(),
        ));

        let teams = Arc::new(TeamManager::new_with_event_dbs(
            db.clone(),
            event_dbs.clone(),
        ));
        teams
            .clone()
            .spawn_remote_relay_worker(TeamRemoteRelayWorkerSettings::default());

        Ok((agents, teams, push, auth, acp_permissions))
    }

    async fn run_startup_cleanup(agents: &AgentManager, teams: &TeamManager) -> anyhow::Result<()> {
        agents.mark_exited_on_startup().await?;
        // Startup policy: never auto-resume in-flight team runs after a process restart.
        // We force manual restart so users can re-confirm intent and avoid duplicate execution.
        let startup_canceled_runs = teams.cancel_active_runs_on_startup().await?;
        if startup_canceled_runs > 0 {
            tracing::info!(
                canceled_run_count = startup_canceled_runs,
                "team startup policy: canceled active runs to require manual start"
            );
        }
        Ok(())
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
        config: &agenthub_config::AppConfig,
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

    fn ensure_global_gitignore_agenthubmemory() -> anyhow::Result<()> {
        let Some(home) = std::env::var_os("HOME").filter(|home| !home.is_empty()) else {
            return Ok(());
        };

        let home_path = PathBuf::from(home);
        std::fs::create_dir_all(&home_path)?;
        let gitignore_path = home_path.join(GLOBAL_GITIGNORE_FILENAME);
        let existing = match std::fs::read_to_string(&gitignore_path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };

        if existing
            .lines()
            .any(|line| line.trim() == GLOBAL_GITIGNORE_ENTRY)
        {
            return Ok(());
        }

        let mut updated = existing;
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str(GLOBAL_GITIGNORE_ENTRY);
        updated.push('\n');
        std::fs::write(gitignore_path, updated)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use agenthub_config::AppConfig;
    use sqlx::Row;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::ffi::OsString;
    use std::sync::Mutex;
    use uuid::Uuid;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct HomeGuard(Option<OsString>);

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            if let Some(value) = self.0.take() {
                unsafe {
                    std::env::set_var("HOME", value);
                }
            } else {
                unsafe {
                    std::env::remove_var("HOME");
                }
            }
        }
    }

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

        let row = sqlx::query("SELECT COUNT(*) AS cnt FROM safe_paths WHERE path = ?1")
            .bind(
                std::path::Path::new(&std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                    .join(".agenthub/worktrees")
                    .to_string_lossy()
                    .to_string(),
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
        let expected =
            std::path::Path::new(&std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join(".agenthub/worktrees")
                .to_string_lossy()
                .to_string();
        assert_eq!(path, expected);
    }

    #[test]
    fn ensure_global_gitignore_contains_agenthubmemory_entry() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp_home = std::env::temp_dir().join(format!("agenthub-home-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_home).expect("create temp home");
        let _home_guard = HomeGuard(std::env::var_os("HOME"));
        unsafe {
            std::env::set_var("HOME", &temp_home);
        }

        AppState::ensure_global_gitignore_agenthubmemory().expect("ensure global gitignore");

        let gitignore_path = temp_home.join(".gitignore_global");
        let content = std::fs::read_to_string(&gitignore_path).expect("read global gitignore");
        assert_eq!(content, ".agenthubmemory\n");

        let _ = std::fs::remove_dir_all(&temp_home);
    }

    #[test]
    fn ensure_global_gitignore_keeps_agenthubmemory_entry_idempotent() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp_home = std::env::temp_dir().join(format!("agenthub-home-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_home).expect("create temp home");
        let gitignore_path = temp_home.join(".gitignore_global");
        std::fs::write(&gitignore_path, "*.log\n.agenthubmemory\n").expect("seed global gitignore");
        let _home_guard = HomeGuard(std::env::var_os("HOME"));
        unsafe {
            std::env::set_var("HOME", &temp_home);
        }

        AppState::ensure_global_gitignore_agenthubmemory().expect("ensure global gitignore");

        let content = std::fs::read_to_string(&gitignore_path).expect("read global gitignore");
        assert_eq!(content, "*.log\n.agenthubmemory\n");

        let _ = std::fs::remove_dir_all(&temp_home);
    }
}
