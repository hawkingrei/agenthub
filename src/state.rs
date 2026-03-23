use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::acp::AcpPermissionService;
use crate::agent::{
    AGENT_NODE_MAIN_ID, AgentManager, AgentTimeTriggerManager, AgentTimeTriggerWorker,
    AgentTimeTriggerWorkerSettings,
};
use crate::auth::AuthService;
use crate::internal::client::InternalGrpcPeerClientConfig;
use crate::internal::tls::{InternalGrpcSecurityMode, ensure_shared_secret, ensure_tls_material};
use crate::push::PushService;
use crate::team::{
    TeamMailboxUnreadHintWorker, TeamMailboxUnreadHintWorkerSettings, TeamManager,
    TeamPermissionReviewDispatcher, TeamPermissionReviewDispatcherSettings,
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
const DEFAULT_GIT_IGNORE_SUBPATH: &str = "git/ignore";

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

        let _mailbox_hint_handle = TeamMailboxUnreadHintWorker::new(teams.clone(), agents.clone())
            .spawn(TeamMailboxUnreadHintWorkerSettings::default());
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
        let internal_peer_client = if config.internal_grpc_enabled() {
            let internal_grpc_cert_dir = std::path::PathBuf::from(config.internal_grpc_cert_dir());
            let internal_grpc_security_mode =
                InternalGrpcSecurityMode::parse(&config.internal_grpc_security_mode())?;
            let internal_shared_secret = ensure_shared_secret(
                &internal_grpc_cert_dir,
                config.internal_grpc_auth_shared_secret(),
            )?;
            let _ = ensure_tls_material(&internal_grpc_cert_dir, internal_grpc_security_mode)?;
            Some(InternalGrpcPeerClientConfig {
                shared_secret: internal_shared_secret,
                expected_issuer: config.internal_grpc_auth_issuer(),
                expected_audience: config.internal_grpc_auth_audience(),
                source_node_id: AGENT_NODE_MAIN_ID.to_string(),
                cert_dir: internal_grpc_cert_dir.to_string_lossy().to_string(),
                security_mode: internal_grpc_security_mode,
            })
        } else {
            None
        };

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
        let agents = Arc::new(AgentManager::new_with_internal_grpc(
            db.clone(),
            event_dbs.clone(),
            idle_gc,
            push.clone(),
            config.proxy_env(),
            config.codex_acp_binary(),
            config.codex_acp_default_mode(),
            acp_permissions.clone(),
            auth.clone(),
            internal_peer_client.clone(),
        ));

        let teams = Arc::new(TeamManager::new_with_event_dbs(
            db.clone(),
            event_dbs.clone(),
        ));
        if let Some(peer_client) = internal_peer_client.as_ref() {
            teams.configure_internal_grpc_relay(
                std::path::Path::new(&peer_client.cert_dir),
                peer_client.security_mode,
            );
            teams.configure_internal_grpc_peer_client(Some(peer_client.clone()));
        }
        teams
            .clone()
            .spawn_remote_relay_worker(TeamRemoteRelayWorkerSettings::default());
        agents.set_permission_review_dispatcher(Some(Arc::new(
            TeamPermissionReviewDispatcher::new(
                teams.clone(),
                agents.clone(),
                acp_permissions.clone(),
                TeamPermissionReviewDispatcherSettings::default(),
            ),
        )));

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
        for gitignore_path in resolve_global_gitignore_paths(&home_path) {
            append_gitignore_entry(&gitignore_path, GLOBAL_GITIGNORE_ENTRY)?;
        }
        Ok(())
    }
}

fn resolve_global_gitignore_paths(home_path: &Path) -> Vec<PathBuf> {
    let mut paths = vec![home_path.join(GLOBAL_GITIGNORE_FILENAME)];
    let xdg_root = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_path.join(".config"));
    let default_ignore_path = xdg_root.join(DEFAULT_GIT_IGNORE_SUBPATH);
    if !paths.iter().any(|path| path == &default_ignore_path) {
        paths.push(default_ignore_path);
    }
    paths
}

fn append_gitignore_entry(path: &Path, entry: &str) -> anyhow::Result<()> {
    let existing = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };

    if existing.lines().any(|line| line.trim() == entry) {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(entry);
    updated.push('\n');
    std::fs::write(path, updated)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AppState, DEFAULT_GIT_IGNORE_SUBPATH, GLOBAL_GITIGNORE_ENTRY, GLOBAL_GITIGNORE_FILENAME,
        append_gitignore_entry, resolve_global_gitignore_paths,
    };
    use agenthub_config::{
        AppConfig, InternalGrpcConfig, InternalGrpcSecurityConfig, PushConfig, WebConfig,
    };
    use agenthub_db::AgentEventDbRouter;
    use sqlx::Row;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::ffi::OsString;
    use std::sync::Mutex;
    use uuid::Uuid;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        key: &'static str,
        value: Option<OsString>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(value) = self.value.take() {
                unsafe {
                    std::env::set_var(self.key, value);
                }
            } else {
                unsafe {
                    std::env::remove_var(self.key);
                }
            }
        }
    }

    fn set_env_var(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> EnvGuard {
        let guard = EnvGuard {
            key,
            value: std::env::var_os(key),
        };
        unsafe {
            std::env::set_var(key, value);
        }
        guard
    }

    fn clear_env_var(key: &'static str) -> EnvGuard {
        let guard = EnvGuard {
            key,
            value: std::env::var_os(key),
        };
        unsafe {
            std::env::remove_var(key);
        }
        guard
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
        let _home_guard = set_env_var("HOME", &temp_home);
        let _xdg_guard = clear_env_var("XDG_CONFIG_HOME");

        AppState::ensure_global_gitignore_agenthubmemory().expect("ensure global gitignore");

        let gitignore_path = temp_home.join(GLOBAL_GITIGNORE_FILENAME);
        let content = std::fs::read_to_string(&gitignore_path).expect("read global gitignore");
        assert_eq!(content, ".agenthubmemory\n");

        let default_ignore_path = temp_home.join(".config").join(DEFAULT_GIT_IGNORE_SUBPATH);
        let default_content =
            std::fs::read_to_string(&default_ignore_path).expect("read default git ignore");
        assert_eq!(default_content, ".agenthubmemory\n");

        let _ = std::fs::remove_dir_all(&temp_home);
    }

    #[test]
    fn ensure_global_gitignore_keeps_agenthubmemory_entry_idempotent() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp_home = std::env::temp_dir().join(format!("agenthub-home-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_home).expect("create temp home");
        let gitignore_path = temp_home.join(GLOBAL_GITIGNORE_FILENAME);
        let default_ignore_path = temp_home.join(".config").join(DEFAULT_GIT_IGNORE_SUBPATH);
        std::fs::write(&gitignore_path, "*.log\n.agenthubmemory\n").expect("seed global gitignore");
        append_gitignore_entry(&default_ignore_path, GLOBAL_GITIGNORE_ENTRY)
            .expect("seed default gitignore");
        let _home_guard = set_env_var("HOME", &temp_home);
        let _xdg_guard = clear_env_var("XDG_CONFIG_HOME");

        AppState::ensure_global_gitignore_agenthubmemory().expect("ensure global gitignore");

        let content = std::fs::read_to_string(&gitignore_path).expect("read global gitignore");
        assert_eq!(content, "*.log\n.agenthubmemory\n");

        let default_content =
            std::fs::read_to_string(&default_ignore_path).expect("read default gitignore");
        assert_eq!(default_content, ".agenthubmemory\n");

        let _ = std::fs::remove_dir_all(&temp_home);
    }

    #[test]
    fn ensure_global_gitignore_prefers_xdg_config_home_when_present() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp_home = std::env::temp_dir().join(format!("agenthub-home-{}", Uuid::new_v4()));
        let temp_xdg = std::env::temp_dir().join(format!("agenthub-xdg-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_home).expect("create temp home");
        std::fs::create_dir_all(&temp_xdg).expect("create temp xdg");
        let _home_guard = set_env_var("HOME", &temp_home);
        let _xdg_guard = set_env_var("XDG_CONFIG_HOME", &temp_xdg);

        let paths = resolve_global_gitignore_paths(&temp_home);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], temp_home.join(GLOBAL_GITIGNORE_FILENAME));
        assert_eq!(paths[1], temp_xdg.join(DEFAULT_GIT_IGNORE_SUBPATH));

        AppState::ensure_global_gitignore_agenthubmemory().expect("ensure global gitignore");

        let xdg_content = std::fs::read_to_string(temp_xdg.join(DEFAULT_GIT_IGNORE_SUBPATH))
            .expect("read xdg git ignore");
        assert_eq!(xdg_content, ".agenthubmemory\n");

        let _ = std::fs::remove_dir_all(&temp_home);
        let _ = std::fs::remove_dir_all(&temp_xdg);
    }

    #[tokio::test]
    async fn initialize_services_skips_internal_grpc_material_when_disabled() {
        let db = test_db().await;
        let cert_dir =
            std::env::temp_dir().join(format!("agenthub-state-internal-grpc-{}", Uuid::new_v4()));
        let keys_dir =
            std::env::temp_dir().join(format!("agenthub-state-push-keys-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&keys_dir).expect("create keys dir");
        let keys_path = keys_dir.join("vapid.json");
        let config = AppConfig {
            web: Some(WebConfig {
                rp_id: Some("localhost".to_string()),
                rp_origin: Some("http://localhost:8080".to_string()),
                rp_name: Some("AgentHub Test".to_string()),
            }),
            push: Some(PushConfig {
                subject: Some("mailto:test@example.com".to_string()),
                keys_path: Some(keys_path.to_string_lossy().to_string()),
            }),
            internal_grpc: Some(InternalGrpcConfig {
                enabled: Some(false),
                listen: None,
                security: Some(InternalGrpcSecurityConfig {
                    mode: Some("mtls".to_string()),
                    cert_dir: Some(cert_dir.to_string_lossy().to_string()),
                }),
                auth: None,
                bootstrap: None,
            }),
            ..Default::default()
        };

        let services =
            AppState::initialize_services(&config, db, AgentEventDbRouter::with_default_base_dir())
                .await;
        assert!(
            services.is_ok(),
            "initialize services should succeed when internal grpc is disabled"
        );
        assert!(
            !cert_dir.exists(),
            "disabled internal grpc should not create cert dir {}",
            cert_dir.display()
        );
        let _ = std::fs::remove_dir_all(&keys_dir);
    }
}
