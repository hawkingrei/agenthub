use agenthub_agent_domain::{
    app_events::{AppEventAttribution, AppEventNotification},
    loop_runtime::{LoopSourceReferences, LoopTriggerInput, LoopTriggerKind, validate_loop_id},
};
use serde::Serialize;
use sqlx::{Acquire, Row, Sqlite, Transaction};
use thiserror::Error;

use super::{AppRegistry, AppStoreError};
use crate::loop_runtime::{LoopStore, LoopStoreError};

mod budgets;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AppEventIntakeError {
    #[error("app event signing authority is unavailable")]
    SigningAuthority,
    #[error("app event route is not authorized")]
    Unauthorized,
    #[error("app event identity was reused")]
    IdConflict,
    #[error("app event cursor was replayed")]
    CursorReplay,
    #[error("app event intake capacity reached")]
    Capacity,
    #[error("app event target execution is disabled")]
    Disabled,
}

impl AppEventIntakeError {
    fn audit_code(self) -> Option<&'static str> {
        match self {
            Self::SigningAuthority => None,
            Self::Unauthorized => Some("unauthorized"),
            Self::IdConflict => Some("id_conflict"),
            Self::CursorReplay => Some("cursor_replay"),
            Self::Capacity => Some("capacity"),
            Self::Disabled => Some("disabled"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AppEventReceipt {
    pub app_id: String,
    pub event_id: String,
    pub cursor: i64,
    pub version: i64,
    pub trigger_id: String,
    pub activation_id: String,
    pub duplicate: bool,
}

/// One row per fixed rejection code; attacker-controlled event IDs never create audit keys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AppEventDenial {
    pub code: String,
    pub count: i64,
    pub last_event_id: String,
    pub last_seen_at: i64,
}

impl AppRegistry {
    /// Caller must verify the exact signed request using this key version before invoking intake.
    /// The canonical write lock rechecks rotation/revocation before deduplication or trigger writes.
    pub async fn accept_signed_event(
        &self,
        app_id: &str,
        signing_version: i64,
        notification: &AppEventNotification,
        now: i64,
    ) -> anyhow::Result<AppEventReceipt> {
        validate_loop_id(app_id)?;
        notification.validate()?;
        anyhow::ensure!(
            now >= 0 && signing_version > 0,
            "invalid app event delivery"
        );
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let authorized: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM app_event_key_versions k JOIN registered_apps a ON a.id = k.app_id \
             WHERE k.app_id = ? AND k.version = ? AND k.revoked_at IS NULL AND a.revoked_at IS NULL)",
        ).bind(app_id).bind(signing_version).fetch_one(&mut *tx).await?;
        anyhow::ensure!(authorized, AppEventIntakeError::SigningAuthority);
        // A savepoint permits bounded denial auditing without committing partial loop intake writes.
        let mut savepoint = tx.begin().await?;
        let result = accept_tx(&mut savepoint, app_id, signing_version, notification, now).await;
        match result {
            Ok(receipt) => {
                savepoint.commit().await?;
                tx.commit().await?;
                Ok(receipt)
            }
            Err(error) => {
                savepoint.rollback().await?;
                let Some(code) = error
                    .downcast_ref::<AppEventIntakeError>()
                    .and_then(|error| error.audit_code())
                else {
                    return Err(error);
                };
                sqlx::query("INSERT INTO app_event_denials(app_id, code, count, last_event_id, last_seen_at) VALUES (?, ?, 1, ?, ?) \
                    ON CONFLICT(app_id, code) DO UPDATE SET count = MIN(count, 9223372036854775806) + 1, last_event_id = excluded.last_event_id, last_seen_at = excluded.last_seen_at")
                    .bind(app_id).bind(code).bind(&notification.event_id).bind(now).execute(&mut *tx).await?;
                tx.commit().await?;
                Err(error)
            }
        }
    }

    /// The HTTP caller checks App ownership. Rows are bounded by the fixed schema code set.
    pub async fn event_denials(&self, app_id: &str) -> anyhow::Result<Vec<AppEventDenial>> {
        let rows = sqlx::query("SELECT code, count, last_event_id, last_seen_at FROM app_event_denials WHERE app_id = ? ORDER BY code LIMIT 5")
            .bind(app_id).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(AppEventDenial {
                    code: row.try_get("code")?,
                    count: row.try_get("count")?,
                    last_event_id: row.try_get("last_event_id")?,
                    last_seen_at: row.try_get("last_seen_at")?,
                })
            })
            .collect()
    }
}

async fn accept_tx(
    tx: &mut Transaction<'_, Sqlite>,
    app_id: &str,
    signing_version: i64,
    event: &AppEventNotification,
    now: i64,
) -> anyhow::Result<AppEventReceipt> {
    let route = AppRegistry::authorize_event_route_tx(
        tx,
        app_id,
        &event.team_id,
        &event.actor_id,
        &event.event_class,
    )
    .await
    .map_err(map_authority_error)?;
    if let Some(row) = sqlx::query("SELECT r.*, s.activation_id, s.input_json AS trigger_input FROM app_event_receipts r JOIN loop_trigger_sources s ON s.id = r.trigger_id \
        WHERE r.app_id = ? AND r.event_id = ?")
        .bind(app_id).bind(&event.event_id).fetch_optional(&mut **tx).await? {
        let original: AppEventNotification = serde_json::from_str(row.try_get("notification_json")?)?;
        anyhow::ensure!(original == *event, AppEventIntakeError::IdConflict);
        let input: LoopTriggerInput = serde_json::from_str(row.try_get("trigger_input")?)?;
        LoopStore::accept_in_transaction(tx, &input, now).await?;
        return Ok(AppEventReceipt {
            app_id: app_id.into(), event_id: event.event_id.clone(), cursor: row.try_get("cursor")?,
            version: row.try_get("version")?, trigger_id: row.try_get("trigger_id")?,
            activation_id: row.try_get("activation_id")?, duplicate: true,
        });
    }
    let cursor: i64 = sqlx::query_scalar("SELECT cursor FROM app_event_cursors WHERE app_id = ?")
        .bind(app_id)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(0);
    anyhow::ensure!(event.cursor > cursor, AppEventIntakeError::CursorReplay);
    budgets::admit(tx, app_id, event, now).await?;
    sqlx::query("INSERT INTO app_event_cursors(app_id, cursor) VALUES (?, ?) ON CONFLICT(app_id) DO UPDATE SET cursor = excluded.cursor")
        .bind(app_id).bind(event.cursor).execute(&mut **tx).await?;
    let input = LoopTriggerInput {
        actor_id: event.actor_id.clone(),
        team_id: event.team_id.clone(),
        kind: LoopTriggerKind::AppEvent,
        source_key: format!("app:{app_id}:{}", event.event_id),
        due_at: None,
        references: LoopSourceReferences {
            app_id: Some(app_id.into()),
            app_event: Some(AppEventAttribution {
                event_id: event.event_id.clone(),
                event_class: event.event_class.clone(),
                cursor: event.cursor,
                version: route.version,
            }),
            ..LoopSourceReferences::default()
        },
    };
    let receipt = LoopStore::accept_in_transaction(tx, &input, now)
        .await
        .map_err(|error| match error.downcast_ref::<LoopStoreError>() {
            Some(LoopStoreError::Disabled) => AppEventIntakeError::Disabled.into(),
            Some(LoopStoreError::Capacity) => AppEventIntakeError::Capacity.into(),
            _ => error,
        })?;
    sqlx::query("INSERT INTO app_event_receipts(app_id, event_id, cursor, team_id, actor_id, event_class, notification_json, version, signing_version, route_revision, trigger_id, created_at) \
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(app_id).bind(&event.event_id).bind(event.cursor).bind(&event.team_id).bind(&event.actor_id)
        .bind(&event.event_class).bind(serde_json::to_string(event)?)
        .bind(route.version).bind(signing_version).bind(route.revision).bind(&receipt.trigger_id).bind(now)
        .execute(&mut **tx).await?;
    LoopStore::observe_app_event_schedule_tx(tx, app_id, event, route.revision, now).await?;
    Ok(AppEventReceipt {
        app_id: app_id.into(),
        event_id: event.event_id.clone(),
        cursor: event.cursor,
        version: route.version,
        trigger_id: receipt.trigger_id,
        activation_id: receipt.activation_id,
        duplicate: false,
    })
}

fn map_authority_error(error: anyhow::Error) -> anyhow::Error {
    if error.downcast_ref::<AppStoreError>().is_some()
        || matches!(
            error.downcast_ref::<LoopStoreError>(),
            Some(LoopStoreError::ScopeMismatch)
        )
    {
        AppEventIntakeError::Unauthorized.into()
    } else {
        error
    }
}
