#[cfg(test)]
use agenthub_config::ServerRole;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::AppState;

impl AppState {
    #[cfg(test)]
    pub(super) async fn setup_database(
        config: &agenthub_config::AppConfig,
    ) -> anyhow::Result<SqlitePool> {
        let db = Self::open_database().await?;
        if config.server_role() == ServerRole::Main {
            Self::ensure_root(&db).await?;
        }
        Ok(db)
    }

    pub(super) async fn open_database() -> anyhow::Result<SqlitePool> {
        agenthub_db::init_db().await
    }

    pub(super) async fn ensure_root(db: &SqlitePool) -> anyhow::Result<()> {
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
}
