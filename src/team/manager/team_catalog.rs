use std::collections::HashSet;

use chrono::Utc;
use uuid::Uuid;

use super::{
    TeamContextLookupError, TeamDefinitionConfig, TeamDefinitionRecord, TeamManager,
    parse_team_definition_row,
};

impl TeamManager {
    #[cfg(test)]
    pub async fn create_team(
        &self,
        config: TeamDefinitionConfig,
    ) -> anyhow::Result<TeamDefinitionRecord> {
        self.create_team_with_owner(config, None).await
    }

    pub async fn create_team_with_owner(
        &self,
        config: TeamDefinitionConfig,
        owner_user_id: Option<&str>,
    ) -> anyhow::Result<TeamDefinitionRecord> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let spec_json = serde_json::to_string(&config.spec)?;
        let group_id = owner_user_id
            .map(str::trim)
            .filter(|value| !value.is_empty());
        sqlx::query(
            r#"
            INSERT INTO team_definitions (
                id,
                name,
                description,
                spec_json,
                owner_user_id,
                group_id,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&id)
        .bind(&config.name)
        .bind(&config.description)
        .bind(spec_json)
        .bind(owner_user_id)
        .bind(group_id)
        .bind(now)
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(TeamDefinitionRecord {
            id,
            name: config.name,
            description: config.description,
            spec: config.spec,
            owner_user_id: owner_user_id.map(str::to_string),
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn list_teams(&self) -> anyhow::Result<Vec<TeamDefinitionRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, description, spec_json, owner_user_id, created_at, updated_at
            FROM team_definitions
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        let mut teams = Vec::with_capacity(rows.len());
        for row in rows {
            teams.push(parse_team_definition_row(&row)?);
        }
        Ok(teams)
    }

    pub async fn list_teams_referencing_member(
        &self,
        member_id: &str,
    ) -> anyhow::Result<Vec<TeamDefinitionRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT td.id, td.name, td.description, td.spec_json, td.owner_user_id, td.created_at, td.updated_at
            FROM team_definitions AS td,
                 json_each(td.spec_json, '$.members') AS member
            WHERE json_extract(member.value, '$.member_id') = ?1
            ORDER BY td.created_at DESC
            "#,
        )
        .bind(member_id)
        .fetch_all(&self.db)
        .await?;

        let mut teams = Vec::with_capacity(rows.len());
        for row in rows {
            teams.push(parse_team_definition_row(&row)?);
        }
        Ok(teams)
    }

    pub async fn get_team(&self, team_id: &str) -> anyhow::Result<TeamDefinitionRecord> {
        let row = sqlx::query(
            r#"
            SELECT id, name, description, spec_json, owner_user_id, created_at, updated_at
            FROM team_definitions
            WHERE id = ?1
            "#,
        )
        .bind(team_id)
        .fetch_one(&self.db)
        .await?;
        parse_team_definition_row(&row)
    }

    pub async fn delete_team(
        &self,
        team_id: &str,
        member_ids: &HashSet<String>,
    ) -> anyhow::Result<TeamDefinitionRecord> {
        let mut tx = self.db.begin().await?;
        let team_row = sqlx::query(
            r#"
            SELECT id, name, description, spec_json, owner_user_id, created_at, updated_at
            FROM team_definitions
            WHERE id = ?1
            "#,
        )
        .bind(team_id)
        .fetch_one(&mut *tx)
        .await?;
        let team = parse_team_definition_row(&team_row)?;

        for member_id in member_ids {
            sqlx::query("DELETE FROM acp_permission_requests WHERE agent_id = ?1")
                .bind(member_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM agent_sessions WHERE agent_id = ?1")
                .bind(member_id)
                .execute(&mut *tx)
                .await?;
        }

        sqlx::query(
            r#"
            DELETE FROM team_actor_messages
            WHERE run_id IN (
                SELECT id FROM team_runs WHERE team_id = ?1
            )
            "#,
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM team_conversation_messages
            WHERE conversation_id IN (
                SELECT id FROM team_conversations WHERE team_id = ?1
            )
            "#,
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM team_conversations WHERE team_id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM team_tasks WHERE team_id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            r#"
            DELETE FROM team_run_events
            WHERE run_id IN (
                SELECT id FROM team_runs WHERE team_id = ?1
            )
            "#,
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM team_steps
            WHERE run_id IN (
                SELECT id FROM team_runs WHERE team_id = ?1
            )
            "#,
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM team_member_continuity_state WHERE team_id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM team_context_artifacts WHERE team_id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM team_context_flush_checkpoint WHERE team_id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM team_runs WHERE team_id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM team_definitions WHERE id = ?1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        for member_id in member_ids {
            self.event_dbs.remove_agent_db(member_id).await?;
        }
        Ok(team)
    }

    pub(crate) async fn resolve_team_scope(
        &self,
        team_id: Option<&str>,
        run_id: Option<&str>,
    ) -> anyhow::Result<String> {
        let normalized_team_id = team_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let normalized_run_id = run_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if let Some(run_id) = normalized_run_id.as_deref() {
            let run = self.get_run(run_id).await?;
            if let Some(explicit_team_id) = normalized_team_id.as_deref()
                && explicit_team_id != run.team_id
            {
                return Err(TeamContextLookupError::RunTeamMismatch {
                    run_id: run_id.to_string(),
                    actual_team_id: run.team_id,
                    requested_team_id: explicit_team_id.to_string(),
                }
                .into());
            }
            return Ok(run.team_id);
        }
        normalized_team_id.ok_or(TeamContextLookupError::MissingSelector.into())
    }
}
