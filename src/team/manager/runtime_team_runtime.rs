use super::runtime_view_builders::build_team_member_card;
use super::runtime_view_loaders::{load_agent_runtime_rows, load_running_session_rows_by_agent};
use super::runtime_views::parse_team_member_specs;
use super::{TeamManager, TeamRuntimeMemberRecord, TeamRuntimeRecord, TeamRuntimeStatus};

impl TeamManager {
    pub async fn describe_team_runtime(&self, team_id: &str) -> anyhow::Result<TeamRuntimeRecord> {
        let team = self.get_team(team_id).await?;
        let members = parse_team_member_specs(&team.spec)?;
        let agent_runtime_by_id = load_agent_runtime_rows(&self.db, &members).await?;
        let running_session_by_agent =
            load_running_session_rows_by_agent(&self.db, &members).await?;

        let mut online = 0_usize;
        let mut out = Vec::with_capacity(members.len());
        for member in members {
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
            if session_id.is_some() {
                online += 1;
            }
            let card = build_team_member_card(
                &member,
                agent_runtime_by_id.get(member.member_id.as_str()),
                &display_name,
            );
            out.push(TeamRuntimeMemberRecord {
                member_id: member.member_id,
                display_name,
                role: member.role,
                description: member.description,
                pending_inbox_count: 0,
                agent_status,
                session_id,
                session_status,
                card,
            });
        }

        let status = if out.is_empty() || online == 0 {
            TeamRuntimeStatus::Stopped
        } else if online == out.len() {
            TeamRuntimeStatus::Running
        } else {
            TeamRuntimeStatus::Degraded
        };

        Ok(TeamRuntimeRecord {
            team_id: team.id,
            team_name: team.name,
            status,
            members: out,
        })
    }
}
