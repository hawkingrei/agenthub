use chrono::Utc;
use sqlx::{Row, SqlitePool};
use webauthn_rs::prelude::Passkey;

#[derive(Clone)]
pub struct PasskeyStore {
    db: SqlitePool,
}

impl PasskeyStore {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn load_passkeys(&self, user_id: &str) -> anyhow::Result<Vec<Passkey>> {
        let row = sqlx::query(
            r#"
            SELECT passkeys
            FROM user_passkeys
            WHERE user_id = ?1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?;

        if let Some(row) = row {
            let json: String = row.get("passkeys");
            let passkeys = serde_json::from_str::<Vec<Passkey>>(&json)?;
            Ok(passkeys)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn save_passkeys(&self, user_id: &str, passkeys: &[Passkey]) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        let json = serde_json::to_string(passkeys)?;
        sqlx::query(
            r#"
            INSERT INTO user_passkeys (user_id, passkeys, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(user_id) DO UPDATE SET
                passkeys = excluded.passkeys,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(user_id)
        .bind(json)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}
