use std::collections::BTreeSet;

use agenthub_team_actor::{ACTOR_MAIN_PEER_ID, ACTOR_NODE_PEER_ID};
use serde_json::Value;
use sqlx::{QueryBuilder, Row, Sqlite};

use super::mailbox::{ResolvedChannelMailboxTarget, ResolvedMailboxRecipientDelivery};
use super::mailbox_payloads::collect_channel_mention_actor_ids;
use super::{TEAM_SHARED_THREAD_TITLE, TeamManager, parse_team_member_specs};
use crate::agent::normalize_target_node_id;
use crate::team::{TeamActorMessageTransport, TeamConversationMessageRecord};

impl TeamManager {
    pub(super) async fn resolve_channel_mailbox_target(
        &self,
        run_id: &str,
        channel_id: &str,
        from_actor_id: &str,
    ) -> anyhow::Result<ResolvedChannelMailboxTarget> {
        let trimmed_channel_id = channel_id.trim();
        if trimmed_channel_id.is_empty() {
            anyhow::bail!("channel_id must be a non-empty string");
        }

        let team_id =
            sqlx::query_scalar::<_, String>("SELECT team_id FROM team_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| anyhow::anyhow!("run not found"))?;

        let (task_id, conversation_id, mode) =
            if trimmed_channel_id.eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE) {
                let (task_id, conversation_id) = self
                    .ensure_shared_thread_target_for_team(&team_id, from_actor_id)
                    .await?;
                (task_id, conversation_id, "group_chat".to_string())
            } else {
                let row = sqlx::query(
                    r#"
                SELECT task_id, id AS conversation_id, mode
                FROM team_conversations
                WHERE team_id = ?1 AND id = ?2
                LIMIT 1
                "#,
                )
                .bind(&team_id)
                .bind(trimmed_channel_id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "channel '{}' not found for current team",
                        trimmed_channel_id
                    )
                })?;
                (
                    row.get::<String, _>("task_id"),
                    row.get::<String, _>("conversation_id"),
                    row.get::<String, _>("mode"),
                )
            };

        if mode.trim() != "group_chat" {
            anyhow::bail!(
                "channel '{}' is not a group_chat channel",
                trimmed_channel_id
            );
        }

        let team = self.get_team(&team_id).await?;
        let member_ids = parse_team_member_specs(&team.spec)?
            .into_iter()
            .map(|member| member.member_id)
            .filter(|member_id| member_id != from_actor_id)
            .collect::<Vec<_>>();

        Ok(ResolvedChannelMailboxTarget {
            team_id,
            task_id,
            conversation_id,
            recipient_actor_ids: member_ids,
        })
    }

    pub(super) async fn extract_channel_mention_actor_ids(
        &self,
        run_id: &str,
        payload: &Value,
    ) -> anyhow::Result<Vec<String>> {
        let team_id =
            sqlx::query_scalar::<_, String>("SELECT team_id FROM team_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| anyhow::anyhow!("run not found"))?;
        let team = self.get_team(&team_id).await?;
        let member_ids = parse_team_member_specs(&team.spec)?
            .into_iter()
            .map(|member| member.member_id)
            .collect::<BTreeSet<_>>();
        Ok(collect_channel_mention_actor_ids(payload, &member_ids))
    }

    async fn has_agents_target_node_id_column(&self) -> anyhow::Result<bool> {
        if let Some(cached) = *self
            .agents_target_node_id_column
            .lock()
            .expect("lock agents target_node_id column cache")
        {
            return Ok(cached);
        }
        let rows = sqlx::query("PRAGMA table_info(agents)")
            .fetch_all(&self.db)
            .await?;
        let has_column = rows.into_iter().any(|row| {
            row.get::<String, _>("name")
                .trim()
                .eq_ignore_ascii_case("target_node_id")
        });
        *self
            .agents_target_node_id_column
            .lock()
            .expect("lock agents target_node_id column cache") = Some(has_column);
        Ok(has_column)
    }

    pub(super) async fn resolve_channel_recipient_deliveries(
        &self,
        recipient_actor_ids: &[String],
    ) -> anyhow::Result<Vec<ResolvedMailboxRecipientDelivery>> {
        if recipient_actor_ids.is_empty() {
            return Ok(Vec::new());
        }
        if !self.has_agents_target_node_id_column().await? {
            return Ok(recipient_actor_ids
                .iter()
                .map(|actor_id| ResolvedMailboxRecipientDelivery {
                    actor_id: actor_id.clone(),
                    to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                    transport: TeamActorMessageTransport::Local,
                    route: None,
                })
                .collect());
        }

        let mut builder =
            QueryBuilder::<Sqlite>::new("SELECT id, target_node_id FROM agents WHERE id IN (");
        let mut separated = builder.separated(", ");
        for actor_id in recipient_actor_ids {
            separated.push_bind(actor_id.as_str());
        }
        separated.push_unseparated(")");
        let rows = builder.build().fetch_all(&self.db).await?;
        let mut target_node_by_actor = std::collections::HashMap::with_capacity(rows.len());
        for row in rows {
            let actor_id: String = row.get("id");
            let target_node_id = normalize_target_node_id(
                row.try_get::<Option<String>, _>("target_node_id")
                    .ok()
                    .flatten()
                    .as_deref(),
            );
            target_node_by_actor.insert(actor_id, target_node_id);
        }

        let mut out = Vec::with_capacity(recipient_actor_ids.len());
        for actor_id in recipient_actor_ids {
            if let Some(target_node_id) = target_node_by_actor
                .get(actor_id.as_str())
                .and_then(|value| value.as_deref())
            {
                out.push(ResolvedMailboxRecipientDelivery {
                    actor_id: actor_id.clone(),
                    to_peer_id: ACTOR_NODE_PEER_ID.to_string(),
                    transport: TeamActorMessageTransport::Remote,
                    route: Some(
                        self.remote_relay_adapter
                            .build_registered_grpc_route_for_target_node(target_node_id)
                            .await?,
                    ),
                });
            } else {
                out.push(ResolvedMailboxRecipientDelivery {
                    actor_id: actor_id.clone(),
                    to_peer_id: ACTOR_MAIN_PEER_ID.to_string(),
                    transport: TeamActorMessageTransport::Local,
                    route: None,
                });
            }
        }
        Ok(out)
    }

    pub(super) async fn find_channel_message_by_correlation_id(
        &self,
        conversation_id: &str,
        from_actor_id: &str,
        correlation_id: &str,
    ) -> anyhow::Result<Option<TeamConversationMessageRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                conversation_id,
                task_id,
                from_actor_id,
                to_actor_id,
                route,
                payload_json,
                created_at
            FROM team_conversation_messages
            WHERE conversation_id = ?1
              AND from_actor_id = ?2
              AND route = 'group_chat'
              AND trim(COALESCE(json_extract(payload_json, '$.correlation_id'), '')) = ?3
            ORDER BY id DESC
            LIMIT 1
            "#,
        )
        .bind(conversation_id)
        .bind(from_actor_id)
        .bind(correlation_id)
        .fetch_optional(&self.db)
        .await?;
        row.map(|row| {
            super::codec_rows::parse_team_conversation_message_row(&row)
                .map_err(|err| anyhow::anyhow!(err.to_string()))
        })
        .transpose()
    }
}
