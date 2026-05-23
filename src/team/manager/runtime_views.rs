use std::collections::HashMap;

use serde_json::Value;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use super::{
    TeamContextLookupError, TeamContextRecord, TeamContextRunOverlayRecord, TeamManager,
    TeamMemberCardRecord, TeamRunMemberRecord, TeamRunMemberStepRecord, TeamRunMembersRecord,
    TeamRuntimeMemberRecord, TeamRuntimeRecord, TeamRuntimeStatus, TeamRuntimeSummaryRecord,
    TeamStepRecord, parse_team_member_continuity_state_row,
};

#[derive(Debug, Clone)]
pub(super) struct TeamMemberSpecView {
    pub(super) member_id: String,
    pub(super) role: String,
    pub(super) description: Option<String>,
}

#[derive(Debug, Clone)]
struct AgentRuntimeRow {
    name: String,
    status: Option<String>,
    code_mode: bool,
    worktree_mode: Option<String>,
}

#[derive(Debug, Clone)]
struct AgentRunningSessionRow {
    session_id: String,
    session_status: String,
}

pub(super) fn parse_team_member_specs(spec: &Value) -> anyhow::Result<Vec<TeamMemberSpecView>> {
    let members = spec
        .get("members")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("spec.members must be an array"))?;
    let mut out = Vec::with_capacity(members.len());
    for member in members {
        let member_obj = member
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("spec.members entries must be objects"))?;
        let member_id = member_obj
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("spec.members[].member_id is required"))?;
        let role = member_obj
            .get("role")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("worker");
        let description = member_obj
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        out.push(TeamMemberSpecView {
            member_id: member_id.to_string(),
            role: role.to_string(),
            description,
        });
    }
    Ok(out)
}

async fn load_agent_runtime_rows(
    db: &SqlitePool,
    members: &[TeamMemberSpecView],
) -> anyhow::Result<HashMap<String, AgentRuntimeRow>> {
    if members.is_empty() {
        return Ok(HashMap::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT id, name, status, code_mode, worktree_mode FROM agents WHERE id IN (",
    );
    let mut separated = builder.separated(", ");
    for member in members {
        separated.push_bind(member.member_id.as_str());
    }
    separated.push_unseparated(")");
    let rows = builder.build().fetch_all(db).await?;

    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        let code_mode_raw: i64 = row.get("code_mode");
        out.insert(
            id,
            AgentRuntimeRow {
                name: row.get("name"),
                status: row.get::<Option<String>, _>("status"),
                code_mode: code_mode_raw != 0,
                worktree_mode: row.get::<Option<String>, _>("worktree_mode"),
            },
        );
    }
    Ok(out)
}

async fn load_session_status_rows(
    db: &SqlitePool,
    session_ids: &[String],
) -> anyhow::Result<HashMap<String, String>> {
    if session_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut builder =
        QueryBuilder::<Sqlite>::new("SELECT id, status FROM agent_sessions WHERE id IN (");
    let mut separated = builder.separated(", ");
    for session_id in session_ids {
        separated.push_bind(session_id.as_str());
    }
    separated.push_unseparated(")");
    let rows = builder.build().fetch_all(db).await?;
    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        let status: String = row.get("status");
        out.insert(id, status);
    }
    Ok(out)
}

async fn load_running_session_rows_by_agent(
    db: &SqlitePool,
    members: &[TeamMemberSpecView],
) -> anyhow::Result<HashMap<String, AgentRunningSessionRow>> {
    if members.is_empty() {
        return Ok(HashMap::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT agent_id, id, status
        FROM agent_sessions
        WHERE ended_at IS NULL
          AND agent_id IN (
        "#,
    );
    let mut separated = builder.separated(", ");
    for member in members {
        separated.push_bind(member.member_id.as_str());
    }
    separated.push_unseparated(
        r#")
        ORDER BY started_at DESC, id DESC
        "#,
    );
    let rows = builder.build().fetch_all(db).await?;

    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let member_id: String = row.get("agent_id");
        let session_id = row.get::<String, _>("id").trim().to_string();
        if session_id.is_empty() {
            continue;
        }
        if out.contains_key(member_id.as_str()) {
            continue;
        }
        out.insert(
            member_id,
            AgentRunningSessionRow {
                session_id,
                session_status: row.get("status"),
            },
        );
    }
    Ok(out)
}

fn build_team_member_card(
    member: &TeamMemberSpecView,
    agent: Option<&AgentRuntimeRow>,
    display_name: &str,
) -> TeamMemberCardRecord {
    let mut capability_tags = vec![
        "team_mailbox_v1".to_string(),
        "team_step_execution_v1".to_string(),
    ];
    if let Some(agent) = agent {
        if agent.code_mode {
            capability_tags.push("code_mode".to_string());
        }
        if let Some(worktree_mode) = agent
            .worktree_mode
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            && matches!(worktree_mode, "create_worktree" | "reuse_worktree")
        {
            capability_tags.push("git_worktree".to_string());
        }
    }
    let description = member.description.clone().unwrap_or_else(|| {
        format!(
            "AgentHub team member {} ({}) supports {}",
            display_name,
            member.role,
            capability_tags.join(", ")
        )
    });
    TeamMemberCardRecord {
        card_id: format!("agenthub://team-members/{}", member.member_id),
        schema_version: "agenthub.a2a.discovery_card.v1".to_string(),
        description,
        role: member.role.clone(),
        skills: crate::team::effective_team_member_skills(&member.role),
        capability_tags,
    }
}

fn build_team_runtime_summary(runtime: &TeamRuntimeRecord) -> TeamRuntimeSummaryRecord {
    TeamRuntimeSummaryRecord {
        status: runtime.status,
        online_count: runtime
            .members
            .iter()
            .filter(|member| member.session_id.is_some())
            .count(),
        member_count: runtime.members.len(),
    }
}

fn team_run_member_from_runtime_member(member: TeamRuntimeMemberRecord) -> TeamRunMemberRecord {
    TeamRunMemberRecord {
        member_id: member.member_id,
        display_name: member.display_name,
        role: member.role,
        description: member.description,
        pending_inbox_count: member.pending_inbox_count,
        agent_status: member.agent_status,
        session_id: member.session_id,
        session_status: member.session_status,
        card: member.card,
        steps: Vec::new(),
    }
}

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
