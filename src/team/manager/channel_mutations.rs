use chrono::Utc;
use sqlx::Error as SqlxError;
use uuid::Uuid;

use super::shared_thread_targets::fetch_team_channel_target;
use super::{TEAM_CHANNEL_BOOTSTRAP_KIND, TEAM_SHARED_THREAD_TITLE, TeamManager};
use crate::team::TeamChannelRecord;

const TEAM_CHANNEL_BOOTSTRAP_SOURCE: &str = "coordinator_created";
const TEAM_CHANNEL_BOOTSTRAP_UNIQUE_INDEX: &str = "idx_team_channel_bootstrap_unique";
const TEAM_CHANNEL_BOOTSTRAP_UNIQUE_CHANNEL_EXPR: &str =
    "lower(trim(COALESCE(json_extract(context_json, '$.channel_id'), '')))";

pub(super) fn normalize_team_channel_id(channel_id: &str) -> anyhow::Result<String> {
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
