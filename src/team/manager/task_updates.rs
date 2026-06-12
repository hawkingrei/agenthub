use sqlx::{Executor, QueryBuilder, Sqlite};

use super::{
    PreparedTeamTaskUpdate, TeamManager, TeamTaskAssignmentUpdate, TeamTaskContextPatch,
    TeamTaskPriority, TeamTaskStatus, TeamTaskUpdateWithNoteInput, parse_team_task_note_row,
    resolve_task_context_patch, team_task_priority_to_str, team_task_status_to_str,
    validate_task_execution_plan,
};

impl TeamManager {
    pub async fn list_task_notes(
        &self,
        task_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<super::TeamTaskNoteRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                conversation_id,
                task_id,
                from_actor_id,
                kind,
                text,
                created_at
            FROM team_conversation_messages
            WHERE task_id = ?1
              AND route = 'task_note'
            ORDER BY id DESC
            LIMIT ?2
            "#,
        )
        .bind(task_id)
        .bind(limit.max(1))
        .fetch_all(&self.db)
        .await?;
        let mut notes = Vec::with_capacity(rows.len());
        for row in rows {
            notes.push(parse_team_task_note_row(&row)?);
        }
        notes.reverse();
        Ok(notes)
    }

    pub async fn update_task_status(
        &self,
        task_id: &str,
        status: TeamTaskStatus,
    ) -> anyhow::Result<super::TeamTaskRecord> {
        self.update_task_with_context_and_priority(
            task_id,
            Some(status),
            TeamTaskAssignmentUpdate::Unchanged,
            None,
            None,
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn update_task(
        &self,
        task_id: &str,
        status: Option<TeamTaskStatus>,
        assignment: TeamTaskAssignmentUpdate,
    ) -> anyhow::Result<super::TeamTaskRecord> {
        self.update_task_with_context_and_priority(task_id, status, assignment, None, None)
            .await
    }

    #[allow(dead_code)]
    pub async fn update_task_with_context(
        &self,
        task_id: &str,
        status: Option<TeamTaskStatus>,
        assignment: TeamTaskAssignmentUpdate,
        context_patch: Option<TeamTaskContextPatch>,
    ) -> anyhow::Result<super::TeamTaskRecord> {
        self.update_task_with_context_and_priority(task_id, status, assignment, None, context_patch)
            .await
    }

    pub async fn update_task_with_context_and_priority(
        &self,
        task_id: &str,
        status: Option<TeamTaskStatus>,
        assignment: TeamTaskAssignmentUpdate,
        priority: Option<TeamTaskPriority>,
        context_patch: Option<TeamTaskContextPatch>,
    ) -> anyhow::Result<super::TeamTaskRecord> {
        let prepared = self
            .prepare_task_update(task_id, status, assignment, priority, context_patch)
            .await?;
        if !prepared.has_changes() {
            return Ok(prepared.current);
        }
        self.execute_prepared_task_update(&self.db, task_id, &prepared)
            .await?;
        self.get_task(task_id).await
    }

    pub async fn update_task_with_note(
        &self,
        input: TeamTaskUpdateWithNoteInput<'_>,
    ) -> anyhow::Result<super::TeamTaskRecord> {
        let prepared = self
            .prepare_task_update(
                input.task_id,
                input.status,
                input.assignment,
                input.priority,
                input.context_patch,
            )
            .await?;
        if !prepared.has_changes() && input.note.is_none() {
            return Ok(prepared.current);
        }

        let mut tx = self.db.begin().await?;
        let mut appended_note = None;
        if let Some(note) = input.note.as_ref() {
            appended_note = Some(
                self.insert_task_conversation_message_in_tx(&mut tx, input.task_id, note)
                    .await?,
            );
        }
        if prepared.has_changes() {
            self.execute_prepared_task_update(&mut *tx, input.task_id, &prepared)
                .await?;
        }
        tx.commit().await?;

        if let Some((conversation, message, created)) = appended_note.as_ref()
            && *created
        {
            self.spawn_archive_task_conversation_message(conversation, message);
            self.emit_task_conversation_message_created(conversation, message);
        }

        if prepared.has_changes() {
            self.get_task(input.task_id).await
        } else {
            Ok(prepared.current)
        }
    }

    async fn prepare_task_update(
        &self,
        task_id: &str,
        status: Option<TeamTaskStatus>,
        assignment: TeamTaskAssignmentUpdate,
        priority: Option<TeamTaskPriority>,
        context_patch: Option<TeamTaskContextPatch>,
    ) -> anyhow::Result<PreparedTeamTaskUpdate> {
        let current = self.get_task(task_id).await?;
        let team = self.get_team(&current.team_id).await?;
        let status_patch = status.filter(|candidate| *candidate != current.status);
        let priority_patch = priority.filter(|candidate| *candidate != current.priority);
        let assignment_patch = match assignment {
            TeamTaskAssignmentUpdate::Unchanged => None,
            TeamTaskAssignmentUpdate::Unassigned => {
                if current.assigned_member_id.is_none() {
                    None
                } else {
                    Some(None)
                }
            }
            TeamTaskAssignmentUpdate::Assigned(member_id) => {
                if current.assigned_member_id.as_deref() == Some(member_id.as_str()) {
                    None
                } else {
                    Some(Some(member_id))
                }
            }
        };
        let context_patch = context_patch
            .and_then(|patch| resolve_task_context_patch(&current.context, patch))
            .map(|next_context| {
                validate_task_execution_plan(&team.spec, &next_context)?;
                Ok::<_, anyhow::Error>(next_context)
            })
            .transpose()?;
        Ok(PreparedTeamTaskUpdate {
            current,
            status_patch,
            priority_patch,
            assignment_patch,
            context_patch,
        })
    }

    async fn execute_prepared_task_update<'e, E>(
        &self,
        executor: E,
        task_id: &str,
        prepared: &PreparedTeamTaskUpdate,
    ) -> anyhow::Result<()>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let now = chrono::Utc::now().timestamp();
        let mut builder = QueryBuilder::<Sqlite>::new("UPDATE team_tasks SET ");
        let mut first = true;
        if let Some(next_status) = prepared.status_patch.as_ref() {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push("status = ");
            builder.push_bind(team_task_status_to_str(next_status));
        }
        if let Some(next_priority) = prepared.priority_patch.as_ref() {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push("priority = ");
            builder.push_bind(team_task_priority_to_str(next_priority));
        }
        if let Some(next_assignment) = prepared.assignment_patch.as_ref() {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push("assigned_member_id = ");
            builder.push_bind(next_assignment.as_deref());
        }
        if let Some(next_context) = prepared.context_patch.as_ref() {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push("context_json = ");
            builder.push_bind(next_context.to_string());
        }
        if !first {
            builder.push(", ");
        }
        builder.push("updated_at = ");
        builder.push_bind(now);
        builder.push(" WHERE id = ");
        builder.push_bind(task_id);
        builder.build().execute(executor).await?;
        Ok(())
    }
}
