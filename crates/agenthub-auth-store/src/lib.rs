use chrono::{Duration, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use agenthub_auth_domain::UserRecord;

#[derive(Clone)]
pub struct AuthStore {
    db: SqlitePool,
}

impl AuthStore {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn is_passkey_enabled(&self, default_enabled: bool) -> anyhow::Result<bool> {
        let row = sqlx::query("SELECT value FROM system_config WHERE key = 'passkey_enabled'")
            .fetch_optional(&self.db)
            .await?;

        if let Some(row) = row {
            let val: String = row.get("value");
            Ok(val == "true")
        } else {
            Ok(default_enabled)
        }
    }

    pub async fn set_passkey_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO system_config (key, value)
            VALUES ('passkey_enabled', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
        )
        .bind(if enabled { "true" } else { "false" })
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn update_user_password_hash(
        &self,
        user_id: &str,
        password_hash: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET password_hash = ?1
            WHERE id = ?2
            "#,
        )
        .bind(password_hash)
        .bind(user_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn insert_user(
        &self,
        user_id: &str,
        username: &str,
        display_name: &str,
        role: &str,
        password_hash: Option<&str>,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(user_id)
        .bind(username)
        .bind(display_name)
        .bind(role)
        .bind(password_hash)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn create_session(&self, user_id: &str) -> anyhow::Result<String> {
        let token = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let expires_at = (Utc::now() + Duration::hours(12)).timestamp();

        sqlx::query(
            r#"
            INSERT INTO auth_sessions (token, user_id, created_at, expires_at, revoked_at)
            VALUES (?1, ?2, ?3, ?4, NULL)
            "#,
        )
        .bind(&token)
        .bind(user_id)
        .bind(now)
        .bind(expires_at)
        .execute(&self.db)
        .await?;

        Ok(token)
    }

    pub async fn validate_session(&self, token: &str) -> anyhow::Result<UserRecord> {
        let row = sqlx::query(
            r#"
            SELECT u.id, u.username, u.display_name, u.role, u.password_hash
            FROM auth_sessions s
            JOIN users u ON s.user_id = u.id
            WHERE s.token = ?1 AND s.expires_at > ?2 AND s.revoked_at IS NULL
            "#,
        )
        .bind(token)
        .bind(Utc::now().timestamp())
        .fetch_one(&self.db)
        .await?;

        let user = map_user_row(&row);
        if user.role == "device" {
            let active = sqlx::query(
                r#"
                SELECT id FROM devices
                WHERE user_id = ?1 AND status = 'active'
                LIMIT 1
                "#,
            )
            .bind(&user.id)
            .fetch_optional(&self.db)
            .await?;
            if active.is_none() {
                anyhow::bail!("device revoked");
            }
        }

        Ok(user)
    }

    pub async fn get_user_by_id(&self, user_id: &str) -> anyhow::Result<UserRecord> {
        let row = sqlx::query(
            r#"
            SELECT id, username, display_name, role, password_hash
            FROM users
            WHERE id = ?1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.db)
        .await?;

        Ok(map_user_row(&row))
    }

    pub async fn get_user_by_username(&self, username: &str) -> anyhow::Result<Option<UserRecord>> {
        let row = sqlx::query(
            r#"
            SELECT id, username, display_name, role, password_hash
            FROM users
            WHERE username = ?1
            "#,
        )
        .bind(username)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.as_ref().map(map_user_row))
    }

    pub async fn record_audit(
        &self,
        user_id: Option<&str>,
        device_id: Option<&str>,
        event: &str,
        detail: Option<&str>,
        ip: Option<&str>,
        user_agent: Option<&str>,
    ) -> anyhow::Result<()> {
        let ts = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO login_audit (user_id, device_id, event, ip, user_agent, detail, ts)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(user_id)
        .bind(device_id)
        .bind(event)
        .bind(ip)
        .bind(user_agent)
        .bind(detail)
        .bind(ts)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn get_device_id_for_user(&self, user_id: &str) -> anyhow::Result<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT id
            FROM devices
            WHERE user_id = ?1 AND status = 'active'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(|r| r.get("id")))
    }

    pub async fn touch_device_login(&self, user_id: &str) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE devices
            SET last_login_at = ?1
            WHERE user_id = ?2
            "#,
        )
        .bind(now)
        .bind(user_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    pub async fn root_exists(&self) -> anyhow::Result<bool> {
        let row = sqlx::query("SELECT COUNT(*) FROM users WHERE role = 'root'")
            .fetch_one(&self.db)
            .await?;
        let count: i64 = row.get(0);
        Ok(count > 0)
    }

    pub async fn insert_device(
        &self,
        user_id: &str,
        name: &str,
        user_agent: &str,
    ) -> anyhow::Result<()> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO devices (id, user_id, name, user_agent, status, created_at)
            VALUES (?1, ?2, ?3, ?4, 'active', ?5)
            "#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(name)
        .bind(user_agent)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

fn map_user_row(row: &sqlx::sqlite::SqliteRow) -> UserRecord {
    UserRecord {
        id: row.get("id"),
        username: row.get("username"),
        display_name: row.get("display_name"),
        role: row.get("role"),
        password_hash: row.get("password_hash"),
    }
}
