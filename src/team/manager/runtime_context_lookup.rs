use serde_json::Value;

use super::runtime_view_builders::{
    build_team_runtime_summary, team_run_member_from_runtime_member,
};
use super::{
    TeamContextLookupError, TeamContextRecord, TeamContextRunOverlayRecord, TeamManager,
    parse_team_member_continuity_state_row,
};

impl TeamManager {
    pub async fn describe_team_context(
        &self,
        team_id: Option<&str>,
        run_id: Option<&str>,
    ) -> anyhow::Result<TeamContextRecord> {
        let normalized_team_id = team_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let normalized_run_id = run_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        if let Some(run_id) = normalized_run_id.as_deref() {
            let roster = self.describe_run_members(run_id).await?;
            if let Some(explicit_team_id) = normalized_team_id.as_deref()
                && explicit_team_id != roster.team_id
            {
                return Err(TeamContextLookupError::RunTeamMismatch {
                    run_id: run_id.to_string(),
                    actual_team_id: roster.team_id.clone(),
                    requested_team_id: explicit_team_id.to_string(),
                }
                .into());
            }
            let runtime = self.describe_team_runtime(&roster.team_id).await?;
            return Ok(TeamContextRecord {
                team_id: roster.team_id,
                team_name: roster.team_name,
                runtime: build_team_runtime_summary(&runtime),
                members: roster.members,
                run: Some(TeamContextRunOverlayRecord {
                    run_id: roster.run_id,
                }),
            });
        }

        let team_id = normalized_team_id.ok_or(TeamContextLookupError::MissingSelector)?;
        let runtime = self.describe_team_runtime(&team_id).await?;
        let runtime_summary = build_team_runtime_summary(&runtime);
        let members = runtime
            .members
            .into_iter()
            .map(team_run_member_from_runtime_member)
            .collect::<Vec<_>>();
        Ok(TeamContextRecord {
            team_id: runtime.team_id,
            team_name: runtime.team_name,
            runtime: runtime_summary,
            members,
            run: None,
        })
    }

    #[allow(dead_code)]
    pub async fn get_member_continuity_state(
        &self,
        team_id: &str,
        member_id: &str,
    ) -> anyhow::Result<Option<crate::team::TeamMemberContinuityStateRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                team_id,
                member_id,
                source_run_id,
                source_session_id,
                summary_text,
                history_window_json,
                updated_at
            FROM team_member_continuity_state
            WHERE team_id = ?1 AND member_id = ?2
            "#,
        )
        .bind(team_id)
        .bind(member_id)
        .fetch_optional(&self.db)
        .await?;
        row.as_ref()
            .map(parse_team_member_continuity_state_row)
            .transpose()
    }

    pub async fn team_has_member(&self, team_id: &str, member_id: &str) -> anyhow::Result<bool> {
        let team = self.get_team(team_id).await?;
        let members = team
            .spec
            .get("members")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(members.iter().any(|member| {
            member
                .get("member_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .is_some_and(|value| value == member_id)
        }))
    }
}
