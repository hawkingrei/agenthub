use chrono::Utc;
use serde_json::Value;
use sqlx::Sqlite;

use super::{
    TeamDefinitionRecord, TeamManager, TeamMemberContinuityStateRecord, TeamRunEventRecord,
};

impl TeamManager {
    pub(super) async fn upsert_member_continuity_state_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        continuity_state: &TeamMemberContinuityStateRecord,
    ) -> anyhow::Result<()> {
        let history_window_json = serde_json::to_string(&continuity_state.history_window)?;
        sqlx::query(
            r#"
            INSERT INTO team_member_continuity_state (
                team_id,
                member_id,
                source_run_id,
                source_session_id,
                summary_text,
                history_window_json,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(team_id, member_id)
            DO UPDATE SET
                source_run_id = excluded.source_run_id,
                source_session_id = excluded.source_session_id,
                summary_text = excluded.summary_text,
                history_window_json = excluded.history_window_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&continuity_state.team_id)
        .bind(&continuity_state.member_id)
        .bind(&continuity_state.source_run_id)
        .bind(continuity_state.source_session_id.as_deref())
        .bind(&continuity_state.summary_text)
        .bind(history_window_json)
        .bind(continuity_state.updated_at)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub(super) async fn append_run_event_tx(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        run_id: &str,
        step_id: Option<&str>,
        event_type: &str,
        ts: i64,
        payload: &Value,
    ) -> anyhow::Result<TeamRunEventRecord> {
        let result = sqlx::query(
            r#"
            INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(run_id)
        .bind(step_id)
        .bind(event_type)
        .bind(ts)
        .bind(payload.to_string())
        .execute(&mut **tx)
        .await?;
        Ok(TeamRunEventRecord {
            event_id: result.last_insert_rowid(),
            run_id: run_id.to_string(),
            step_id: step_id.map(str::to_string),
            event_type: event_type.to_string(),
            ts,
            payload: payload.clone(),
        })
    }

    pub async fn update_team_spec_if_unchanged(
        &self,
        team_id: &str,
        expected_updated_at: i64,
        spec: Value,
    ) -> anyhow::Result<Option<TeamDefinitionRecord>> {
        let now = Utc::now().timestamp();
        let spec_json = serde_json::to_string(&spec)?;
        let update = sqlx::query(
            r#"
            UPDATE team_definitions
            SET spec_json = ?1, updated_at = ?2
            WHERE id = ?3 AND updated_at = ?4
            "#,
        )
        .bind(spec_json)
        .bind(now)
        .bind(team_id)
        .bind(expected_updated_at)
        .execute(&self.db)
        .await?;
        if update.rows_affected() == 0 {
            let exists: Option<i64> = sqlx::query_scalar(
                r#"
                SELECT 1
                FROM team_definitions
                WHERE id = ?1
                "#,
            )
            .bind(team_id)
            .fetch_optional(&self.db)
            .await?;
            if exists.is_none() {
                return Err(sqlx::Error::RowNotFound.into());
            }
            return Ok(None);
        }
        self.get_team(team_id).await.map(Some)
    }
}
