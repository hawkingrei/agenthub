use chrono::Utc;
use sqlx::{Error as SqlxError, Row};
use uuid::Uuid;

use super::shared_thread::{fetch_canonical_shared_thread_target, fetch_team_channel_target};
use super::{TEAM_CHANNEL_BOOTSTRAP_KIND, TEAM_SHARED_THREAD_TITLE, TeamManager};
use crate::team::{TeamChannelRecord, TeamThreadOpenRecord, TeamThreadReplyRecord};

const TEAM_CHANNEL_BOOTSTRAP_SOURCE: &str = "coordinator_created";
const TEAM_CHANNEL_BOOTSTRAP_UNIQUE_INDEX: &str = "idx_team_channel_bootstrap_unique";
const TEAM_CHANNEL_BOOTSTRAP_UNIQUE_CHANNEL_EXPR: &str =
    "lower(trim(COALESCE(json_extract(context_json, '$.channel_id'), '')))";

fn normalize_team_channel_id(channel_id: &str) -> anyhow::Result<String> {
    let normalized = channel_id.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        anyhow::bail!("channel_id is required");
    }
    Ok(normalized)
}

fn is_team_channel_bootstrap_unique_violation(err: &SqlxError) -> bool {
    match err {
        SqlxError::Database(db_err) => {
            let message = db_err.message();
            db_err.code().as_deref() == Some(super::SQLITE_CONSTRAINT_UNIQUE_CODE)
                && (message.contains(TEAM_CHANNEL_BOOTSTRAP_UNIQUE_INDEX)
                    || (message.contains("team_tasks.team_id")
                        && message.contains(TEAM_CHANNEL_BOOTSTRAP_UNIQUE_CHANNEL_EXPR)))
        }
        _ => false,
    }
}

impl TeamManager {
    pub async fn list_channels(&self, team_id: &str) -> anyhow::Result<Vec<TeamChannelRecord>> {
        let normalized_team_id = team_id.trim();
        if normalized_team_id.is_empty() {
            anyhow::bail!("team_id is required");
        }

        self.get_team(normalized_team_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT
                t.id AS task_id,
                c.id AS conversation_id,
                lower(trim(COALESCE(json_extract(t.context_json, '$.channel_id'), ''))) AS channel_id,
                json_extract(t.context_json, '$.description') AS description,
                t.created_by_actor_id,
                t.created_at,
                t.updated_at
            FROM team_tasks t
            INNER JOIN team_conversations c ON c.task_id = t.id
            WHERE t.team_id = ?1
              AND c.team_id = ?1
              AND c.mode = 'group_chat'
              AND lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) = ?2
              AND lower(trim(COALESCE(json_extract(t.context_json, '$.channel_id'), ''))) <> ''
            ORDER BY c.created_at ASC, c.rowid ASC, t.created_at ASC, t.rowid ASC
            "#,
        )
        .bind(normalized_team_id)
        .bind(TEAM_CHANNEL_BOOTSTRAP_KIND)
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TeamChannelRecord {
                team_id: normalized_team_id.to_string(),
                task_id: row.get("task_id"),
                conversation_id: row.get("conversation_id"),
                channel_id: row.get("channel_id"),
                description: row
                    .try_get::<Option<String>, _>("description")
                    .ok()
                    .flatten(),
                created_by_actor_id: row.get("created_by_actor_id"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn open_thread(
        &self,
        team_id: &str,
        channel_id: &str,
        root_message_id: i64,
    ) -> anyhow::Result<TeamThreadOpenRecord> {
        let normalized_team_id = team_id.trim();
        if normalized_team_id.is_empty() {
            anyhow::bail!("team_id is required");
        }
        let normalized_channel_id = normalize_team_channel_id(channel_id)?;
        if root_message_id <= 0 {
            anyhow::bail!("root_message_id must be positive");
        }

        let (task_id, conversation_id, resolved_channel_id) =
            if normalized_channel_id.eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE) {
                let target = fetch_canonical_shared_thread_target(&self.db, normalized_team_id)
                    .await?
                    .ok_or_else(|| {
                        anyhow::anyhow!("shared thread is missing for team {}", normalized_team_id)
                    })?;
                (
                    target.task_id,
                    target.conversation_id,
                    TEAM_SHARED_THREAD_TITLE.to_string(),
                )
            } else {
                let row =
                    fetch_team_channel_target(&self.db, normalized_team_id, &normalized_channel_id)
                        .await?
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "channel '{}' not found for team {}",
                                normalized_channel_id,
                                normalized_team_id
                            )
                        })?;
                (row.task_id, row.conversation_id, row.channel_id)
            };

        let root_exists = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT 1
            FROM team_conversation_messages
            WHERE id = ?1 AND conversation_id = ?2 AND task_id = ?3
            LIMIT 1
            "#,
        )
        .bind(root_message_id)
        .bind(&conversation_id)
        .bind(&task_id)
        .fetch_optional(&self.db)
        .await?;
        if root_exists.is_none() {
            anyhow::bail!(
                "root_message_id {} was not found in channel '{}'",
                root_message_id,
                resolved_channel_id
            );
        }

        Ok(TeamThreadOpenRecord {
            team_id: normalized_team_id.to_string(),
            channel_id: resolved_channel_id,
            task_id,
            conversation_id,
            root_message_id,
            thread_id: root_message_id.to_string(),
        })
    }

    pub async fn reply_thread(
        &self,
        team_id: &str,
        channel_id: &str,
        root_message_id: i64,
        from_actor_id: &str,
        text: &str,
        mention_actor_ids: &[String],
    ) -> anyhow::Result<TeamThreadReplyRecord> {
        let normalized_actor_id = from_actor_id.trim();
        if normalized_actor_id.is_empty() {
            anyhow::bail!("from_actor_id is required");
        }
        let normalized_text = text.trim();
        if normalized_text.is_empty() {
            anyhow::bail!("text is required");
        }

        let normalized_mentions = mention_actor_ids
            .iter()
            .map(|actor_id| actor_id.trim())
            .filter(|actor_id| !actor_id.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let thread = self
            .open_thread(team_id, channel_id, root_message_id)
            .await?;
        let message = self
            .append_task_conversation_message(
                &thread.task_id,
                normalized_actor_id,
                None,
                "team_thread_reply",
                serde_json::json!({
                    "type": "chat_message",
                    "text": normalized_text,
                    "mention_actor_ids": normalized_mentions,
                    "thread_root_message_id": root_message_id,
                }),
            )
            .await?;

        Ok(TeamThreadReplyRecord { thread, message })
    }

    pub async fn create_channel(
        &self,
        team_id: &str,
        channel_id: &str,
        description: Option<&str>,
        created_by_actor_id: &str,
    ) -> anyhow::Result<TeamChannelRecord> {
        let normalized_team_id = team_id.trim();
        if normalized_team_id.is_empty() {
            anyhow::bail!("team_id is required");
        }
        let normalized_created_by_actor_id = created_by_actor_id.trim();
        if normalized_created_by_actor_id.is_empty() {
            anyhow::bail!("created_by_actor_id is required");
        }
        let normalized_channel_id = normalize_team_channel_id(channel_id)?;
        if normalized_channel_id.eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE) {
            anyhow::bail!("channel_id 'all' is reserved");
        }
        let normalized_description = description
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        self.get_team(normalized_team_id).await?;
        let now = Utc::now().timestamp();
        let task_id = Uuid::new_v4().to_string();
        let conversation_id = Uuid::new_v4().to_string();
        let context_json = serde_json::json!({
            "bootstrap_kind": TEAM_CHANNEL_BOOTSTRAP_KIND,
            "bootstrap_source": TEAM_CHANNEL_BOOTSTRAP_SOURCE,
            "channel_id": normalized_channel_id,
            "description": normalized_description,
        })
        .to_string();

        let mut tx = self.db.begin().await?;
        let existing =
            fetch_team_channel_target(&mut *tx, normalized_team_id, &normalized_channel_id).await?;
        if existing.is_some() {
            anyhow::bail!(
                "channel '{}' already exists for team {}",
                normalized_channel_id,
                normalized_team_id
            );
        }

        sqlx::query(
            r#"
            INSERT INTO team_tasks (
                id, team_id, group_id, title, status, priority, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at
            )
            VALUES (?1, ?2, (SELECT group_id FROM team_definitions WHERE id = ?2), ?3, 'open', 'medium', ?4, NULL, ?5, ?6, ?7)
            "#,
        )
        .bind(&task_id)
        .bind(normalized_team_id)
        .bind(&normalized_channel_id)
        .bind(normalized_created_by_actor_id)
        .bind(context_json)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|err| {
            if is_team_channel_bootstrap_unique_violation(&err) {
                anyhow::anyhow!(
                    "channel '{}' already exists for team {}",
                    normalized_channel_id,
                    normalized_team_id
                )
            } else {
                err.into()
            }
        })?;

        sqlx::query(
            r#"
            INSERT INTO team_conversations (
                id, team_id, task_id, mode, topic, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, 'group_chat', ?4, ?5, ?6)
            "#,
        )
        .bind(&conversation_id)
        .bind(normalized_team_id)
        .bind(&task_id)
        .bind(&normalized_channel_id)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(TeamChannelRecord {
            team_id: normalized_team_id.to_string(),
            channel_id: normalized_channel_id,
            task_id,
            conversation_id,
            description: normalized_description,
            created_by_actor_id: normalized_created_by_actor_id.to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn delete_channel(
        &self,
        team_id: &str,
        channel_id: &str,
    ) -> anyhow::Result<TeamChannelRecord> {
        let normalized_team_id = team_id.trim();
        if normalized_team_id.is_empty() {
            anyhow::bail!("team_id is required");
        }
        let normalized_channel_id = normalize_team_channel_id(channel_id)?;
        if normalized_channel_id.eq_ignore_ascii_case(TEAM_SHARED_THREAD_TITLE) {
            anyhow::bail!("channel_id 'all' cannot be deleted");
        }

        let mut tx = self.db.begin().await?;
        let row = fetch_team_channel_target(&mut *tx, normalized_team_id, &normalized_channel_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "channel '{}' not found for team {}",
                    normalized_channel_id,
                    normalized_team_id
                )
            })?;

        let task_id = row.task_id;
        let conversation_id = row.conversation_id;
        let channel_id = row.channel_id;
        let description = row.description;
        let created_at = row.created_at;
        let updated_at = row.updated_at;
        let created_by_actor_id = row.created_by_actor_id;

        sqlx::query(
            "DELETE FROM team_channel_message_replicas WHERE conversation_id = ?1 OR task_id = ?2",
        )
        .bind(&conversation_id)
        .bind(&task_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM team_conversation_messages WHERE conversation_id = ?1")
            .bind(&conversation_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM team_conversations WHERE id = ?1")
            .bind(&conversation_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM team_tasks WHERE id = ?1")
            .bind(&task_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(TeamChannelRecord {
            team_id: normalized_team_id.to_string(),
            channel_id,
            task_id,
            conversation_id,
            description,
            created_by_actor_id,
            created_at,
            updated_at,
        })
    }
}
