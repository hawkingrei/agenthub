use sqlx::{Row, Sqlite, SqlitePool};

use super::codec_rows::parse_team_actor_message_row;
use super::mailbox_store_inbox_enrichment::enrich_actor_messages;
use crate::team::TeamActorMessageRecord;

pub(super) async fn resolve_team_id_for_run(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
) -> Result<String, sqlx::Error> {
    let row = sqlx::query("SELECT team_id FROM team_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(&mut **tx)
        .await?;
    row.try_get("team_id")
}

async fn fetch_message_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    message_id: i64,
) -> Result<TeamActorMessageRecord, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            id,
            run_id,
            from_actor_id,
            from_peer_id,
            to_actor_id,
            to_peer_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            message_kind,
            handling_disposition,
            handled_by_actor_id,
            handled_at,
            status,
            created_at,
            delivered_at
        FROM team_actor_messages
        WHERE id = ?1
        "#,
    )
    .bind(message_id)
    .fetch_one(&mut **tx)
    .await?;
    parse_team_actor_message_row(&row).map_err(|err| sqlx::Error::Protocol(err.to_string()))
}

pub(super) async fn fetch_enriched_message_by_id(
    pool: &SqlitePool,
    message_id: i64,
) -> Result<TeamActorMessageRecord, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let mut message = fetch_message_by_id(&mut tx, message_id).await?;
    tx.commit().await?;
    enrich_actor_messages(pool, std::slice::from_mut(&mut message)).await?;
    Ok(message)
}

pub(super) async fn fetch_message_for_actor(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
    actor_id: &str,
    peer_id: &str,
    message_id: i64,
) -> Result<TeamActorMessageRecord, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            id,
            run_id,
            from_actor_id,
            from_peer_id,
            to_actor_id,
            to_peer_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            message_kind,
            handling_disposition,
            handled_by_actor_id,
            handled_at,
            status,
            created_at,
            delivered_at
        FROM team_actor_messages
        WHERE id = ?1 AND run_id = ?2 AND to_actor_id = ?3 AND to_peer_id = ?4
        "#,
    )
    .bind(message_id)
    .bind(run_id)
    .bind(actor_id)
    .bind(peer_id)
    .fetch_one(&mut **tx)
    .await?;
    parse_team_actor_message_row(&row).map_err(|err| sqlx::Error::Protocol(err.to_string()))
}

pub(super) async fn fetch_message_by_idempotency(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
    from_actor_id: &str,
    from_peer_id: &str,
    idempotency_key: &str,
) -> Result<TeamActorMessageRecord, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            id,
            run_id,
            from_actor_id,
            from_peer_id,
            to_actor_id,
            to_peer_id,
            channel,
            transport,
            route_json,
            payload_json,
            idempotency_key,
            message_kind,
            handling_disposition,
            handled_by_actor_id,
            handled_at,
            status,
            created_at,
            delivered_at
        FROM team_actor_messages
        WHERE run_id = ?1 AND from_actor_id = ?2 AND from_peer_id = ?3 AND idempotency_key = ?4
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .bind(from_actor_id)
    .bind(from_peer_id)
    .bind(idempotency_key)
    .fetch_one(&mut **tx)
    .await?;
    parse_team_actor_message_row(&row).map_err(|err| sqlx::Error::Protocol(err.to_string()))
}
