use serde_json::Value;
use sqlx::QueryBuilder;
use uuid::Uuid;

use super::{
    TEAM_CHANNEL_BOOTSTRAP_KIND, TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX, TeamManager,
    TeamTaskCreateInput, TeamTaskListQuery, TeamTaskPriority, TeamTaskRecord, TeamTaskStatus,
    collect_team_member_ids, parse_team_task_row, redact_sensitive_json, team_task_priority_to_str,
    team_task_status_to_str, validate_task_execution_plan,
};
use crate::team::{TeamConversationRecord, TeamTaskDetailRecord};

impl TeamManager {
    // Compatibility helper for legacy/bootstrap task seeding in tests and narrow internal setup
    // paths. Canonical Kanban task authoring should use `create_task_with_metadata` so explicit
    // priority and owner requirements remain visible at the call site.
    #[allow(dead_code)]
    pub async fn create_task(
        &self,
        team_id: &str,
        title: &str,
        created_by_actor_id: &str,
        context: Value,
        conversation_mode: &str,
        topic: Option<&str>,
    ) -> anyhow::Result<(TeamTaskRecord, TeamConversationRecord)> {
        self.create_task_with_metadata(TeamTaskCreateInput {
            team_id,
            title,
            created_by_actor_id,
            priority: TeamTaskPriority::default(),
            assigned_member_id: None,
            context,
            conversation_mode,
            topic,
        })
        .await
    }

    pub async fn create_task_with_metadata(
        &self,
        input: TeamTaskCreateInput<'_>,
    ) -> anyhow::Result<(TeamTaskRecord, TeamConversationRecord)> {
        let team = self.get_team(input.team_id).await?;
        validate_task_execution_plan(&team.spec, &input.context)?;
        let normalized_assigned_member_id = input
            .assigned_member_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if let Some(member_id) = normalized_assigned_member_id.as_deref() {
            let valid_member_ids = collect_team_member_ids(&team.spec);
            if !valid_member_ids
                .iter()
                .any(|candidate| candidate == member_id)
            {
                anyhow::bail!("assigned_member_id must reference spec.members[].member_id");
            }
        }
        let now = chrono::Utc::now().timestamp();
        let task_id = Uuid::new_v4().to_string();
        let conversation_id = Uuid::new_v4().to_string();
        let status = TeamTaskStatus::Open;
        let context_json = redact_sensitive_json(&input.context).to_string();
        let topic = input.topic.map(str::trim).filter(|value| !value.is_empty());

        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO team_tasks (
                id, team_id, group_id, title, status, priority, created_by_actor_id, assigned_member_id, context_json, created_at, updated_at
            )
            VALUES (?1, ?2, (SELECT group_id FROM team_definitions WHERE id = ?2), ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(&task_id)
        .bind(input.team_id)
        .bind(input.title)
        .bind(team_task_status_to_str(&status))
        .bind(team_task_priority_to_str(&input.priority))
        .bind(input.created_by_actor_id)
        .bind(normalized_assigned_member_id)
        .bind(context_json)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO team_conversations (
                id, team_id, task_id, mode, topic, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&conversation_id)
        .bind(input.team_id)
        .bind(&task_id)
        .bind(input.conversation_mode)
        .bind(topic)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        let task = self.get_task(&task_id).await?;
        let conversation = self.get_task_conversation(&task_id).await?;
        Ok((task, conversation))
    }

    pub async fn list_tasks_with_query(
        &self,
        query: TeamTaskListQuery,
    ) -> anyhow::Result<Vec<TeamTaskRecord>> {
        let team_id = self
            .resolve_team_scope(query.team_id.as_deref(), query.run_id.as_deref())
            .await?;
        let mut builder = QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT
                t.id,
                t.team_id,
                t.title,
                t.status,
                t.priority,
                t.created_by_actor_id,
                t.assigned_member_id,
                t.context_json,
                t.created_at,
                t.updated_at
            FROM team_tasks AS t
            LEFT JOIN team_conversations AS c ON c.task_id = t.id
            WHERE t.team_id = "#,
        );
        builder.push_bind(&team_id);
        if let Some(status) = query.status {
            builder.push(" AND t.status = ");
            builder.push_bind(team_task_status_to_str(&status));
        }
        if let Some(priority) = query.priority {
            builder.push(" AND t.priority = ");
            builder.push_bind(team_task_priority_to_str(&priority));
        }
        if let Some(task_id) = query.task_id.as_deref() {
            builder.push(" AND t.id = ");
            builder.push_bind(task_id);
        }
        if let Some(assigned_member_id) = query.assigned_member_id.as_deref() {
            builder.push(" AND t.assigned_member_id = ");
            builder.push_bind(assigned_member_id);
        }
        if let Some(topic) = query.topic.as_deref() {
            builder.push(" AND trim(COALESCE(c.topic, '')) = ");
            builder.push_bind(topic);
        }
        if !query.include_shared_thread {
            builder.push(
                " AND lower(trim(t.title)) != 'all' \
                 AND lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) != ",
            );
            builder.push_bind("shared_thread");
        }
        builder.push(
            " AND lower(trim(COALESCE(json_extract(t.context_json, '$.bootstrap_kind'), ''))) != ",
        );
        builder.push_bind(TEAM_CHANNEL_BOOTSTRAP_KIND);
        builder.push(" ORDER BY t.updated_at DESC, t.id DESC LIMIT ");
        builder.push_bind(query.limit.max(1));
        let rows = builder.build().fetch_all(&self.db).await?;
        let mut tasks = Vec::with_capacity(rows.len());
        for row in rows {
            tasks.push(parse_team_task_row(&row)?);
        }
        Ok(tasks)
    }

    pub async fn get_task(&self, task_id: &str) -> anyhow::Result<TeamTaskRecord> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                team_id,
                title,
                status,
                priority,
                created_by_actor_id,
                assigned_member_id,
                context_json,
                created_at,
                updated_at
            FROM team_tasks
            WHERE id = ?1
            "#,
        )
        .bind(task_id)
        .fetch_one(&self.db)
        .await?;
        parse_team_task_row(&row)
    }

    pub(super) async fn get_task_for_team(
        &self,
        team_id: &str,
        task_id: &str,
    ) -> anyhow::Result<TeamTaskRecord> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                team_id,
                title,
                status,
                priority,
                created_by_actor_id,
                assigned_member_id,
                context_json,
                created_at,
                updated_at
            FROM team_tasks
            WHERE id = ?1
              AND team_id = ?2
            "#,
        )
        .bind(task_id)
        .bind(team_id)
        .fetch_optional(&self.db)
        .await?;
        let row = row
            .ok_or_else(|| anyhow::anyhow!("linked task does not belong to the requested team"))?;
        parse_team_task_row(&row)
    }

    pub async fn get_task_detail(
        &self,
        task_id: &str,
        message_limit: i64,
    ) -> anyhow::Result<TeamTaskDetailRecord> {
        let task = self.get_task(task_id).await?;
        let conversation = self.get_task_conversation(task_id).await?;
        let latest_run = self.get_latest_run_for_task(&task.team_id, task_id).await?;
        let notes = self
            .list_task_notes(
                task_id,
                message_limit.clamp(1, TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX),
            )
            .await?;
        let recent_messages = self
            .list_task_conversation_messages(
                task_id,
                message_limit.clamp(1, TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX),
                None,
            )
            .await?;
        Ok(TeamTaskDetailRecord {
            task,
            conversation,
            latest_run,
            notes,
            recent_messages,
        })
    }
}
