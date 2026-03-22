use crate::acp::AcpActorSkillContext;

use super::AgentRecord;

#[derive(Debug, Clone)]
pub(super) enum AgentStartPlan {
    ReuseRunningSession {
        session_id: String,
    },
    StartLocal {
        agent: AgentRecord,
        actor_context: Option<AcpActorSkillContext>,
    },
    StartRemote {
        agent: AgentRecord,
        target_node_id: String,
        actor_context: Option<AcpActorSkillContext>,
    },
}

pub(super) fn build_agent_start_plan(
    agent: AgentRecord,
    actor_context: Option<AcpActorSkillContext>,
    running_session_id: Option<&str>,
) -> anyhow::Result<AgentStartPlan> {
    if let Some(target_node_id) = agent.target_node_id.clone() {
        return Ok(AgentStartPlan::StartRemote {
            agent,
            target_node_id,
            actor_context,
        });
    }
    if let Some(session_id) = running_session_id {
        if actor_context.is_some() {
            anyhow::bail!(
                "agent already running with session '{}'; cannot start with new actor context",
                session_id
            );
        }
        return Ok(AgentStartPlan::ReuseRunningSession {
            session_id: session_id.to_string(),
        });
    }
    Ok(AgentStartPlan::StartLocal {
        agent,
        actor_context,
    })
}
