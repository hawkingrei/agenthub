use std::collections::HashMap;

use super::runtime_view_builders::build_team_member_card;
use super::runtime_view_loaders::{
    load_agent_runtime_rows, load_running_session_rows_by_agent, load_session_status_rows,
};
use super::runtime_views::parse_team_member_specs;
use super::{
    TeamManager, TeamRunMemberRecord, TeamRunMemberStepRecord, TeamRunMembersRecord, TeamStepRecord,
};

impl TeamManager {
    pub async fn describe_run_members(&self, run_id: &str) -> anyhow::Result<TeamRunMembersRecord> {
        let run = self.get_run(run_id).await?;
        let team = self.get_team(&run.team_id).await?;
        let members = parse_team_member_specs(&team.spec)?;
        let steps = self.list_steps(run_id).await?;
        let pending_inbox_counts = self.list_actor_pending_counts_by_actor(run_id).await?;

        let mut steps_by_member = HashMap::<String, Vec<TeamStepRecord>>::new();
        let mut session_ids = Vec::new();
        for step in steps {
            if let Some(session_id) = step
                .runtime_handle_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                session_ids.push(session_id.to_string());
            }
            steps_by_member
                .entry(step.member_id.clone())
                .or_default()
                .push(step);
        }

        let agent_runtime_by_id = load_agent_runtime_rows(&self.db, &members).await?;
        let running_session_by_agent =
            load_running_session_rows_by_agent(&self.db, &members).await?;
        let session_status_by_id = load_session_status_rows(&self.db, &session_ids).await?;

        let mut out = Vec::with_capacity(members.len());
        for member in members {
            let pending_inbox_count = pending_inbox_counts
                .get(member.member_id.as_str())
                .copied()
                .unwrap_or(0);
            let display_name = agent_runtime_by_id
                .get(member.member_id.as_str())
                .map(|agent| agent.name.clone())
                .unwrap_or_else(|| member.member_id.clone());
            let agent_status = agent_runtime_by_id
                .get(member.member_id.as_str())
                .and_then(|agent| agent.status.clone());
            let running_session = running_session_by_agent.get(member.member_id.as_str());
            let session_id = running_session.map(|session| session.session_id.clone());
            let session_status = running_session.map(|session| session.session_status.clone());
            let steps = steps_by_member
                .remove(member.member_id.as_str())
                .unwrap_or_default()
                .into_iter()
                .map(|step| TeamRunMemberStepRecord {
                    step_id: step.id,
                    step_key: step.step_key,
                    status: step.status,
                    attempt: step.attempt,
                    session_id: step.runtime_handle_id.clone(),
                    session_status: step
                        .runtime_handle_id
                        .as_deref()
                        .and_then(|session_id| session_status_by_id.get(session_id))
                        .cloned(),
                })
                .collect::<Vec<_>>();
            let card = build_team_member_card(
                &member,
                agent_runtime_by_id.get(member.member_id.as_str()),
                &display_name,
            );
            out.push(TeamRunMemberRecord {
                member_id: member.member_id,
                display_name,
                role: member.role,
                description: member.description,
                pending_inbox_count,
                agent_status,
                session_id,
                session_status,
                card,
                steps,
            });
        }

        Ok(TeamRunMembersRecord {
            team_id: team.id,
            team_name: team.name,
            run_id: run.id,
            members: out,
        })
    }
}
