use chrono::Utc;
use sqlx::{Executor, Row, Sqlite};
use uuid::Uuid;

use super::{
    TEAM_CHANNEL_BOOTSTRAP_KIND, TEAM_SHARED_THREAD_BOOTSTRAP_KIND, TEAM_SHARED_THREAD_TITLE,
    TeamManager, is_row_not_found,
};
use crate::team::{TeamConversationRecord, TeamRunRecord, TeamTaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedThreadTargetRecord {
    pub task_id: String,
    pub conversation_id: String,
}

#[derive(Debug, Clone)]
pub(super) struct TeamChannelTargetRecord {
    pub(super) task_id: String,
    pub(super) conversation_id: String,
    pub(super) channel_id: String,
    pub(super) description: Option<String>,
    pub(super) created_by_actor_id: String,
    pub(super) created_at: i64,
    pub(super) updated_at: i64,
}

pub(crate) async fn fetch_canonical_shared_thread_target<'e, E>(
    executor: E,
    team_id: &str,
) -> Result<Option<SharedThreadTargetRecord>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query(
        r#"
        SELECT
            t.id AS task_id,
            c.id AS conversation_id,
            (
                SELECT MAX(m.id)
                FROM team_conversation_messages m
                WHERE m.conversation_id = c.id
            ) AS latest_message_id
        FROM team_tasks t
        INNER JOIN team_conversations c ON c.task_id = t.id
        WHERE t.team_id = ?1
          AND (
            lower(trim(t.title)) = ?2
            OR lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) = ?3
          )
        ORDER BY
            CASE WHEN latest_message_id IS NULL THEN 1 ELSE 0 END ASC,
            latest_message_id DESC,
            c.created_at ASC,
            t.created_at ASC,
            t.id ASC
        LIMIT 1
        "#,
    )
    .bind(team_id)
    .bind(TEAM_SHARED_THREAD_TITLE)
    .bind(TEAM_SHARED_THREAD_BOOTSTRAP_KIND)
    .fetch_optional(executor)
    .await?;

    Ok(row.map(|row| SharedThreadTargetRecord {
        task_id: row.get("task_id"),
        conversation_id: row.get("conversation_id"),
    }))
}

pub(super) async fn fetch_team_channel_target<'e, E>(
    executor: E,
    team_id: &str,
    channel_id: &str,
) -> Result<Option<TeamChannelTargetRecord>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query(
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
          AND lower(trim(COALESCE(json_extract(t.context_json, '$.channel_id'), ''))) = ?3
        ORDER BY c.created_at ASC, t.created_at ASC, t.id ASC
        LIMIT 1
        "#,
    )
    .bind(team_id)
    .bind(TEAM_CHANNEL_BOOTSTRAP_KIND)
    .bind(channel_id.trim().to_ascii_lowercase())
    .fetch_optional(executor)
    .await?;

    Ok(row.map(|row| TeamChannelTargetRecord {
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
    }))
}

impl TeamManager {
    pub async fn get_shared_thread_detail_for_team(
        &self,
        team_id: &str,
    ) -> anyhow::Result<
        Option<(
            TeamTaskRecord,
            TeamConversationRecord,
            Option<TeamRunRecord>,
        )>,
    > {
        let Some(target) = fetch_canonical_shared_thread_target(&self.db, team_id).await? else {
            return Ok(None);
        };
        match self
            .load_task_detail_for_team(team_id, &target.task_id)
            .await
        {
            Ok(detail) => Ok(Some(detail)),
            Err(err) if is_row_not_found(&err) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub async fn ensure_shared_thread_detail_for_team(
        &self,
        team_id: &str,
        created_by_actor_id: &str,
    ) -> anyhow::Result<(
        TeamTaskRecord,
        TeamConversationRecord,
        Option<TeamRunRecord>,
    )> {
        let (task_id, _) = self
            .ensure_shared_thread_target_for_team(team_id, created_by_actor_id)
            .await?;
        self.load_task_detail_for_team(team_id, &task_id).await
    }

    pub async fn ensure_shared_thread_target_for_team(
        &self,
        team_id: &str,
        created_by_actor_id: &str,
    ) -> anyhow::Result<(String, String)> {
        let mut tx = self.db.begin().await?;
        if let Some(existing) = fetch_canonical_shared_thread_target(&mut *tx, team_id).await? {
            tx.commit().await?;
            return Ok((existing.task_id, existing.conversation_id));
        }

        let now = Utc::now().timestamp();
        let task_id = Uuid::new_v4().to_string();
        let conversation_id = Uuid::new_v4().to_string();
        let context_json = serde_json::json!({
            "bootstrap_kind": TEAM_SHARED_THREAD_BOOTSTRAP_KIND,
            "bootstrap_source": "server_canonical_reply",
        })
        .to_string();

        sqlx::query(
            r#"
            INSERT INTO team_tasks (
                id,
                team_id,
                group_id,
                title,
                status,
                priority,
                created_by_actor_id,
                assigned_member_id,
                context_json,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, (SELECT group_id FROM team_definitions WHERE id = ?2), ?3, 'open', 'medium', ?4, NULL, ?5, ?6, ?7)
            "#,
        )
        .bind(&task_id)
        .bind(team_id)
        .bind(TEAM_SHARED_THREAD_TITLE)
        .bind(created_by_actor_id)
        .bind(context_json)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO team_conversations (
                id,
                team_id,
                task_id,
                mode,
                topic,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, 'group_chat', ?4, ?5, ?6)
            "#,
        )
        .bind(&conversation_id)
        .bind(team_id)
        .bind(&task_id)
        .bind(TEAM_SHARED_THREAD_TITLE)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((task_id, conversation_id))
    }

    pub(crate) async fn shared_thread_target_for_run(
        &self,
        run_id: &str,
    ) -> anyhow::Result<Option<(String, String, String)>> {
        let team_id =
            sqlx::query_scalar::<_, String>("SELECT team_id FROM team_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_optional(&self.db)
                .await?;
        let Some(team_id) = team_id else {
            return Ok(None);
        };
        let target = fetch_canonical_shared_thread_target(&self.db, &team_id).await?;
        Ok(target.map(|target| (team_id, target.task_id, target.conversation_id)))
    }
}
