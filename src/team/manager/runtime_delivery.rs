use sqlx::Row;

use super::TeamManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TeamRuntimeDeliveryReceipt {
    pub delivery_id: String,
    pub run_id: String,
    pub message_id: i64,
    pub actor_id: String,
    pub prompt: String,
    pub state: String,
    pub attempt: i64,
    pub next_retry_at: Option<i64>,
    pub lease_expires_at: Option<i64>,
    pub last_error: Option<String>,
    pub session_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub delivered_at: Option<i64>,
}

pub(crate) fn mailbox_runtime_delivery_id(run_id: &str, message_id: i64, actor_id: &str) -> String {
    format!("team-mailbox:{run_id}:{message_id}:{actor_id}")
}

impl TeamManager {
    pub(crate) async fn ensure_mailbox_runtime_deliveries(
        &self,
        run_id: &str,
        message_id: i64,
        actor_ids: &[String],
        prompt: &str,
        now: i64,
    ) -> anyhow::Result<Vec<TeamRuntimeDeliveryReceipt>> {
        let mut tx = self.db.begin().await?;
        let mut receipts = Vec::with_capacity(actor_ids.len());
        for actor_id in actor_ids {
            let delivery_id = mailbox_runtime_delivery_id(run_id, message_id, actor_id);
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO team_runtime_delivery_receipts (
                    delivery_id,
                    run_id,
                    message_id,
                    actor_id,
                    prompt,
                    state,
                    attempt,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, 'pending', 0, ?6, ?6)
                "#,
            )
            .bind(&delivery_id)
            .bind(run_id)
            .bind(message_id)
            .bind(actor_id)
            .bind(prompt)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            let row = sqlx::query(
                r#"
                SELECT
                    delivery_id,
                    run_id,
                    message_id,
                    actor_id,
                    prompt,
                    state,
                    attempt,
                    next_retry_at,
                    lease_expires_at,
                    last_error,
                    session_id,
                    created_at,
                    updated_at,
                    delivered_at
                FROM team_runtime_delivery_receipts
                WHERE delivery_id = ?1
                "#,
            )
            .bind(&delivery_id)
            .fetch_one(&mut *tx)
            .await?;
            receipts.push(parse_runtime_delivery_receipt(&row));
        }
        tx.commit().await?;
        Ok(receipts)
    }

    pub(crate) async fn list_due_mailbox_runtime_deliveries(
        &self,
        now: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamRuntimeDeliveryReceipt>> {
        let rows = sqlx::query(
            r#"
            SELECT
                delivery_id,
                run_id,
                message_id,
                actor_id,
                prompt,
                state,
                attempt,
                next_retry_at,
                lease_expires_at,
                last_error,
                session_id,
                created_at,
                updated_at,
                delivered_at
            FROM team_runtime_delivery_receipts
            WHERE (
                    state = 'pending'
                    AND (next_retry_at IS NULL OR next_retry_at <= ?1)
                  )
               OR (
                    state = 'in_flight'
                    AND lease_expires_at IS NOT NULL
                    AND lease_expires_at <= ?1
                  )
            ORDER BY created_at ASC, delivery_id ASC
            LIMIT ?2
            "#,
        )
        .bind(now)
        .bind(limit.clamp(1, 1_000))
        .fetch_all(&self.db)
        .await?;
        Ok(rows.iter().map(parse_runtime_delivery_receipt).collect())
    }

    pub(crate) async fn claim_mailbox_runtime_delivery(
        &self,
        delivery_id: &str,
        now: i64,
        lease_seconds: i64,
    ) -> anyhow::Result<Option<TeamRuntimeDeliveryReceipt>> {
        let row = sqlx::query(
            r#"
            UPDATE team_runtime_delivery_receipts
            SET
                state = 'in_flight',
                attempt = attempt + 1,
                next_retry_at = NULL,
                lease_expires_at = ?2,
                updated_at = ?1
            WHERE delivery_id = ?3
              AND (
                    (
                        state = 'pending'
                        AND (next_retry_at IS NULL OR next_retry_at <= ?1)
                    )
                    OR (
                        state = 'in_flight'
                        AND lease_expires_at IS NOT NULL
                        AND lease_expires_at <= ?1
                    )
                  )
            RETURNING
                delivery_id,
                run_id,
                message_id,
                actor_id,
                prompt,
                state,
                attempt,
                next_retry_at,
                lease_expires_at,
                last_error,
                session_id,
                created_at,
                updated_at,
                delivered_at
            "#,
        )
        .bind(now)
        .bind(now.saturating_add(lease_seconds.max(1)))
        .bind(delivery_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.as_ref().map(parse_runtime_delivery_receipt))
    }

    pub(crate) async fn acknowledge_mailbox_runtime_delivery(
        &self,
        delivery_id: &str,
        attempt: i64,
        session_id: &str,
        now: i64,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE team_runtime_delivery_receipts
            SET
                state = 'delivered',
                session_id = COALESCE(session_id, ?2),
                delivered_at = COALESCE(delivered_at, ?3),
                next_retry_at = NULL,
                lease_expires_at = NULL,
                last_error = NULL,
                updated_at = ?3
            WHERE delivery_id = ?1
              AND state = 'in_flight'
              AND attempt = ?4
            "#,
        )
        .bind(delivery_id)
        .bind(session_id)
        .bind(now)
        .bind(attempt)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub(crate) async fn retry_mailbox_runtime_delivery(
        &self,
        delivery_id: &str,
        attempt: i64,
        now: i64,
        next_retry_at: i64,
        error: &str,
    ) -> anyhow::Result<bool> {
        let error = truncate_runtime_delivery_error(error);
        let result = sqlx::query(
            r#"
            UPDATE team_runtime_delivery_receipts
            SET
                state = 'pending',
                next_retry_at = ?2,
                lease_expires_at = NULL,
                last_error = ?3,
                session_id = NULL,
                updated_at = ?1
            WHERE delivery_id = ?4
              AND state = 'in_flight'
              AND attempt = ?5
            "#,
        )
        .bind(now)
        .bind(next_retry_at.max(now))
        .bind(error)
        .bind(delivery_id)
        .bind(attempt)
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    #[cfg(test)]
    pub(crate) async fn mailbox_runtime_delivery_for_test(
        &self,
        delivery_id: &str,
    ) -> anyhow::Result<TeamRuntimeDeliveryReceipt> {
        let row = sqlx::query(
            r#"
            SELECT
                delivery_id,
                run_id,
                message_id,
                actor_id,
                prompt,
                state,
                attempt,
                next_retry_at,
                lease_expires_at,
                last_error,
                session_id,
                created_at,
                updated_at,
                delivered_at
            FROM team_runtime_delivery_receipts
            WHERE delivery_id = ?1
            "#,
        )
        .bind(delivery_id)
        .fetch_one(&self.db)
        .await?;
        Ok(parse_runtime_delivery_receipt(&row))
    }
}

fn parse_runtime_delivery_receipt(row: &sqlx::sqlite::SqliteRow) -> TeamRuntimeDeliveryReceipt {
    TeamRuntimeDeliveryReceipt {
        delivery_id: row.get("delivery_id"),
        run_id: row.get("run_id"),
        message_id: row.get("message_id"),
        actor_id: row.get("actor_id"),
        prompt: row.get("prompt"),
        state: row.get("state"),
        attempt: row.get("attempt"),
        next_retry_at: row.get("next_retry_at"),
        lease_expires_at: row.get("lease_expires_at"),
        last_error: row.get("last_error"),
        session_id: row.get("session_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        delivered_at: row.get("delivered_at"),
    }
}

fn truncate_runtime_delivery_error(error: &str) -> String {
    const MAX_CHARS: usize = 2_048;
    error.chars().take(MAX_CHARS).collect()
}

pub(crate) fn runtime_delivery_retry_delay_seconds(attempt: i64) -> i64 {
    const MAX_DELAY_SECONDS: i64 = 60;
    let exponent = attempt.saturating_sub(1).clamp(0, 30) as u32;
    (1_i64 << exponent).min(MAX_DELAY_SECONDS)
}
