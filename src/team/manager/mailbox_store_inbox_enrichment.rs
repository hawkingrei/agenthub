use std::collections::HashMap;

use agenthub_team_actor::{
    ActorMessageTaskRelation, derive_actor_message_topic_metadata,
    parse_actor_message_task_relation, parse_actor_thread_claim_status,
};
use chrono::Utc;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use super::mailbox_store::{ActorMessageTaskLinkRecord, ActorThreadClaimRecord};
use crate::team::TeamActorMessageRecord;

fn apply_payload_task_fallback(message: &mut TeamActorMessageRecord) {
    if message.linked_task_id.is_some() {
        return;
    }
    let Some(metadata) =
        derive_actor_message_topic_metadata(message.message_id, &message.payload, None)
    else {
        return;
    };
    if let Some(task_id) = metadata.task_id {
        message.linked_task_id = Some(task_id);
        if message.linked_task_relation.is_none() {
            message.linked_task_relation = Some(ActorMessageTaskRelation::RelatedTask);
        }
    }
}

async fn load_latest_message_task_links(
    pool: &SqlitePool,
    run_id: &str,
    message_ids: &[i64],
) -> Result<HashMap<i64, ActorMessageTaskLinkRecord>, sqlx::Error> {
    if message_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT links.message_id, links.task_id, links.relation
        FROM team_actor_message_links AS links
        JOIN (
            SELECT message_id, MAX(id) AS max_id
            FROM team_actor_message_links
            WHERE run_id = "#,
    );
    builder.push_bind(run_id);
    builder.push(" AND message_id IN (");
    {
        let mut separated = builder.separated(", ");
        for message_id in message_ids {
            separated.push_bind(message_id);
        }
    }
    builder.push(
        r#")
            GROUP BY message_id
        ) AS latest
          ON latest.max_id = links.id
        "#,
    );
    let rows = builder.build().fetch_all(pool).await?;
    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let relation_raw: String = row.get("relation");
        let Some(relation) = parse_actor_message_task_relation(&relation_raw) else {
            continue;
        };
        out.insert(
            row.get("message_id"),
            ActorMessageTaskLinkRecord {
                task_id: row.get("task_id"),
                relation,
            },
        );
    }
    Ok(out)
}

async fn load_active_thread_claims(
    pool: &SqlitePool,
    run_id: &str,
    topic_keys: &[String],
) -> Result<HashMap<String, ActorThreadClaimRecord>, sqlx::Error> {
    if topic_keys.is_empty() {
        return Ok(HashMap::new());
    }
    let now = Utc::now().timestamp();
    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT
            topic_key,
            task_id,
            root_message_id,
            owner_actor_id,
            claim_status,
            claimed_at,
            lease_expires_at
        FROM team_actor_thread_claims
        WHERE run_id = "#,
    );
    builder.push_bind(run_id);
    builder.push(" AND topic_key IN (");
    {
        let mut separated = builder.separated(", ");
        for topic_key in topic_keys {
            separated.push_bind(topic_key);
        }
    }
    builder.push(
        ") AND ((claim_status = 'claimed' AND (lease_expires_at IS NULL OR lease_expires_at > ",
    );
    builder.push_bind(now);
    builder.push(")) OR claim_status = 'completed')");
    let rows = builder.build().fetch_all(pool).await?;
    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let claim_status_raw: String = row.get("claim_status");
        let Some(claim_status) = parse_actor_thread_claim_status(&claim_status_raw) else {
            continue;
        };
        let topic_key: String = row.get("topic_key");
        out.insert(
            topic_key.clone(),
            ActorThreadClaimRecord {
                topic_key,
                task_id: row.try_get("task_id").ok(),
                root_message_id: row.try_get("root_message_id").ok(),
                owner_actor_id: row.get("owner_actor_id"),
                claim_status,
                claimed_at: row.get("claimed_at"),
                lease_expires_at: row.try_get("lease_expires_at").ok(),
            },
        );
    }
    Ok(out)
}

pub(super) async fn enrich_actor_messages(
    pool: &SqlitePool,
    messages: &mut [TeamActorMessageRecord],
) -> Result<(), sqlx::Error> {
    if messages.is_empty() {
        return Ok(());
    }
    for message in messages.iter_mut() {
        apply_payload_task_fallback(message);
        message.thread_topic_key = derive_actor_message_topic_metadata(
            message.message_id,
            &message.payload,
            message.linked_task_id.as_deref(),
        )
        .map(|metadata| metadata.topic_key);
        message.thread_claim_status = None;
        message.thread_owner_actor_id = None;
        message.thread_lease_expires_at = None;
    }

    let mut run_groups = HashMap::<String, Vec<usize>>::new();
    for (idx, message) in messages.iter().enumerate() {
        run_groups
            .entry(message.run_id.clone())
            .or_default()
            .push(idx);
    }
    for (run_id, indexes) in run_groups {
        let message_ids = indexes
            .iter()
            .map(|index| messages[*index].message_id)
            .collect::<Vec<_>>();
        let latest_links = load_latest_message_task_links(pool, &run_id, &message_ids).await?;
        for index in &indexes {
            if let Some(link) = latest_links.get(&messages[*index].message_id) {
                messages[*index].linked_task_id = Some(link.task_id.clone());
                messages[*index].linked_task_relation = Some(link.relation.clone());
            }
            messages[*index].thread_topic_key = derive_actor_message_topic_metadata(
                messages[*index].message_id,
                &messages[*index].payload,
                messages[*index].linked_task_id.as_deref(),
            )
            .map(|metadata| metadata.topic_key);
        }
        let topic_keys = indexes
            .iter()
            .filter_map(|index| messages[*index].thread_topic_key.clone())
            .collect::<Vec<_>>();
        let claims = load_active_thread_claims(pool, &run_id, &topic_keys).await?;
        for index in indexes {
            let Some(topic_key) = messages[index].thread_topic_key.clone() else {
                continue;
            };
            let Some(claim) = claims.get(&topic_key) else {
                continue;
            };
            messages[index].thread_claim_status = Some(claim.claim_status.clone());
            messages[index].thread_owner_actor_id = Some(claim.owner_actor_id.clone());
            messages[index].thread_lease_expires_at = claim.lease_expires_at;
        }
    }
    Ok(())
}
