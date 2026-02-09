use std::sync::Arc;

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::acp::AcpPermissionService;
use crate::agent::AgentManager;
use crate::auth::AuthService;
use crate::push::PushService;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub agents: Arc<AgentManager>,
    pub push: Arc<PushService>,
    pub auth: Arc<AuthService>,
    pub acp_permissions: Arc<AcpPermissionService>,
}

impl AppState {
    pub async fn init(config: crate::config::AppConfig) -> anyhow::Result<Self> {
        let db = crate::db::init_db().await?;
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
        agents.mark_exited_on_startup().await?;
        Self::ensure_root(&db).await?;
        Self::seed_safe_paths(&db, &config).await?;
        Ok(Self {
            db,
            agents,
            push,
            auth,
            acp_permissions,
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
