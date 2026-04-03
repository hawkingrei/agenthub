use std::sync::Arc;
use std::time::Duration;

use agenthub_acp::{
    AcpPermissionReviewDispatcher, AcpPermissionReviewRequest, AcpPermissionService,
};
use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorMailboxService, ActorMessageTransport, ActorSendRequest,
};
use serde_json::{Value, json};

use crate::agent::AgentManager;

use super::TeamManager;
use super::mailbox_hint::{
    ActorMailboxPriorityClass, DEFAULT_TEAM_MAILBOX_IDLE_AFTER_SECS, TeamMailboxHintAgentNudger,
    actor_mailbox_is_idle, actor_mailbox_priority_label,
};

const TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE: &str = "permission_review_request";
const TEAM_HUMAN_PERMISSION_CARD_PAYLOAD_TYPE: &str = "permission_review_card";
const DEFAULT_TEAM_PERMISSION_REVIEW_HUMAN_FALLBACK_DELAY: Duration = Duration::from_secs(40);

#[derive(Debug, Clone, Copy)]
pub struct TeamPermissionReviewDispatcherSettings {
    pub human_fallback_delay: Duration,
}

impl Default for TeamPermissionReviewDispatcherSettings {
    fn default() -> Self {
        Self {
            human_fallback_delay: DEFAULT_TEAM_PERMISSION_REVIEW_HUMAN_FALLBACK_DELAY,
        }
    }
}

#[derive(Clone)]
pub struct TeamPermissionReviewDispatcher {
    teams: Arc<TeamManager>,
    agent_nudger: Arc<dyn TeamMailboxHintAgentNudger>,
    permissions: Arc<AcpPermissionService>,
    settings: TeamPermissionReviewDispatcherSettings,
}

impl TeamPermissionReviewDispatcher {
    pub fn new(
        teams: Arc<TeamManager>,
        agents: Arc<AgentManager>,
        permissions: Arc<AcpPermissionService>,
        settings: TeamPermissionReviewDispatcherSettings,
    ) -> Self {
        Self::with_agent_nudger(teams, agents, permissions, settings)
    }

    pub fn with_agent_nudger(
        teams: Arc<TeamManager>,
        agent_nudger: Arc<dyn TeamMailboxHintAgentNudger>,
        permissions: Arc<AcpPermissionService>,
        settings: TeamPermissionReviewDispatcherSettings,
    ) -> Self {
        Self {
            teams,
            agent_nudger,
            permissions,
            settings,
        }
    }

    async fn dispatch_to_review_target(
        &self,
        request: &AcpPermissionReviewRequest,
    ) -> anyhow::Result<()> {
        let Some(team_id) = request.routing.team_id.as_deref() else {
            return Ok(());
        };
        let requester_actor_id = request
            .routing
            .requester_actor_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("permission review routing requires requester actor"))?;
        let requester_role = request
            .routing
            .requester_role
            .as_deref()
            .map(str::trim)
            .unwrap_or("");
        let team = self.teams.get_team(team_id).await?;
        let (review_target_actor_id, dispatch_status) = self
            .resolve_review_target(&team.spec, requester_actor_id, requester_role)
            .await?;

        let (task_id, conversation_id) = self
            .teams
            .ensure_shared_thread_target_for_team(team_id, requester_actor_id)
            .await?;
        let run_id = if let Some(current_run_id) = request
            .current_run_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            current_run_id.to_string()
        } else {
            self.teams
                .ensure_shared_thread_mailbox_run(team_id, &task_id, &conversation_id)
                .await?
                .id
        };
        let payload = build_permission_review_payload(request, review_target_actor_id.as_str());
        let response = self
            .teams
            .actor_mailbox_service()
            .actor_send(ActorSendRequest {
                run_id: run_id.clone(),
                from_actor_id: requester_actor_id.to_string(),
                from_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
                to_actor_id: Some(review_target_actor_id.clone()),
                channel_id: None,
                to_peer_id: Some(ACTOR_MAIN_PEER_ID.to_string()),
                channel: Some("default".to_string()),
                transport: Some(ActorMessageTransport::Local),
                route: None,
                payload,
                idempotency_key: Some(format!(
                    "permission-review:{}:{}",
                    request.request_id, review_target_actor_id
                )),
            })
            .await
            .map_err(|err| {
                anyhow::anyhow!(
                    "dispatch permission review to reviewer failed: {}",
                    err.message
                )
            })?;
        self.permissions
            .record_review_dispatch(
                &request.request_id,
                Some(review_target_actor_id.as_str()),
                dispatch_status,
                Some(run_id.as_str()),
                Some(response.message_id),
            )
            .await?;
        self.nudge_actor(
            review_target_actor_id.as_str(),
            &run_id,
            TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE,
        )
        .await;
        self.spawn_human_review_fallback(request.clone());
        Ok(())
    }

    fn spawn_human_review_fallback(&self, request: AcpPermissionReviewRequest) {
        let dispatcher = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(dispatcher.settings.human_fallback_delay).await;
            let _ = dispatcher
                .notify_human_review_if_pending(&request, "review_timeout")
                .await;
        });
    }

    async fn nudge_actor(&self, actor_id: &str, run_id: &str, payload_type: &str) {
        let priority_label =
            actor_mailbox_priority_label(ActorMailboxPriorityClass::PermissionReview);
        let hint = format!(
            "New {priority_label} mailbox message type '{payload_type}' is pending in run '{run_id}'. Use agenthub actor inbox --run-id \"{run_id}\" to inspect pending messages and batch-handle this type before ack."
        );
        if let Err(err) = self
            .agent_nudger
            .nudge_mailbox_prompt(actor_id, None, &hint)
            .await
        {
            tracing::debug!(
                actor_id = %actor_id,
                run_id = %run_id,
                payload_type = %payload_type,
                "skip permission review mailbox hint because agent input is unavailable: {}",
                err
            );
        }
    }

    async fn resolve_review_target(
        &self,
        spec: &Value,
        requester_actor_id: &str,
        requester_role: &str,
    ) -> anyhow::Result<(String, &'static str)> {
        let candidates =
            collect_team_permission_review_candidates(spec, requester_actor_id, requester_role)?;
        let now = chrono::Utc::now().timestamp();
        for candidate in &candidates {
            if self.actor_is_idle(candidate.actor_id.as_str(), now).await? {
                return Ok((candidate.actor_id.clone(), candidate.idle_dispatch_status));
            }
        }
        let fallback = candidates
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("team has no non-requester reviewer configured"))?;
        Ok((fallback.actor_id, fallback.dispatch_status))
    }

    async fn actor_is_idle(&self, actor_id: &str, now: i64) -> anyhow::Result<bool> {
        let Some(runtime) = self.agent_nudger.running_actor_runtime(actor_id).await else {
            return Ok(false);
        };
        let Some(idle_anchor_ts) = self
            .agent_nudger
            .mailbox_idle_anchor_ts(actor_id, runtime.session_id.as_str())
            .await?
        else {
            return Ok(false);
        };
        Ok(actor_mailbox_is_idle(
            now,
            DEFAULT_TEAM_MAILBOX_IDLE_AFTER_SECS,
            idle_anchor_ts,
        ))
    }

    async fn notify_human_review_if_pending(
        &self,
        request: &AcpPermissionReviewRequest,
        reason: &str,
    ) -> anyhow::Result<()> {
        let Some(team_id) = request.routing.team_id.as_deref() else {
            return Ok(());
        };
        let Some(record) = self.permissions.get(&request.request_id).await? else {
            return Ok(());
        };
        if record.status != "pending" || record.human_review_notified_at.is_some() {
            return Ok(());
        }
        let requester_actor_id = request
            .routing
            .requester_actor_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("leader");
        let (task_id, _) = self
            .teams
            .ensure_shared_thread_target_for_team(team_id, requester_actor_id)
            .await?;
        let payload = json!({
            "type": TEAM_HUMAN_PERMISSION_CARD_PAYLOAD_TYPE,
            "permission_id": request.request_id,
            "agent_id": request.agent_id,
            "agent_session_id": request.agent_session_id,
            "acp_session_id": request.acp_session_id,
            "tool_call_id": request.tool_call_id,
            "tool_call": request.tool_call,
            "tool_name": request.tool_call.as_ref().and_then(extract_tool_name),
            "requester_actor_id": request.routing.requester_actor_id,
            "requester_role": request.routing.requester_role,
            "options": request.options,
            "summary": build_permission_review_summary(request),
            "reason": reason,
            "reason_text": human_review_reason_text(reason),
            "status": "pending",
        });
        let _ = self
            .teams
            .append_task_conversation_message(
                &task_id,
                requester_actor_id,
                None,
                "group_chat",
                payload,
            )
            .await?;
        if !self
            .permissions
            .mark_human_review_notified(&request.request_id)
            .await?
        {
            tracing::debug!(
                permission_id = %request.request_id,
                "human review notification was already marked while appending fallback message"
            );
        }
        self.permissions
            .record_review_dispatch(&request.request_id, None, reason, None, None)
            .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl AcpPermissionReviewDispatcher for TeamPermissionReviewDispatcher {
    async fn dispatch_review(&self, request: AcpPermissionReviewRequest) -> anyhow::Result<()> {
        if let Err(err) = self.dispatch_to_review_target(&request).await {
            tracing::warn!(
                permission_id = %request.request_id,
                error = %err,
                "team permission review dispatch failed; falling back to human review"
            );
            let _ = self
                .permissions
                .record_review_dispatch(
                    &request.request_id,
                    None,
                    "review_dispatch_failed",
                    None,
                    None,
                )
                .await;
            let _ = self
                .notify_human_review_if_pending(&request, "review_dispatch_failed")
                .await;
        }
        Ok(())
    }
}

fn build_permission_review_payload(
    request: &AcpPermissionReviewRequest,
    review_target_actor_id: &str,
) -> Value {
    json!({
        "type": TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE,
        "permission_id": request.request_id,
        "requester_actor_id": request.routing.requester_actor_id,
        "requester_role": request.routing.requester_role,
        "review_target_actor_id": review_target_actor_id,
        "tool_call_id": request.tool_call_id,
        "tool_call": request.tool_call,
        "options": request.options,
        "summary": build_permission_review_summary(request),
        "instruction": "Review the request through the ACP permission approval flow. The reviewer is assigned automatically by the Team runtime.",
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PermissionReviewCandidate {
    actor_id: String,
    dispatch_status: &'static str,
    idle_dispatch_status: &'static str,
}

fn permission_review_candidate(
    actor_id: impl Into<String>,
    dispatch_status: &'static str,
    idle_dispatch_status: &'static str,
) -> PermissionReviewCandidate {
    PermissionReviewCandidate {
        actor_id: actor_id.into(),
        dispatch_status,
        idle_dispatch_status,
    }
}

fn worker_permission_review_candidates(
    spec: &Value,
    leader_member_id: &str,
    requester_actor_id: &str,
) -> Vec<PermissionReviewCandidate> {
    collect_subordinate_reviewers(spec, leader_member_id, requester_actor_id)
        .into_iter()
        .map(|actor_id| {
            permission_review_candidate(actor_id, "worker_dispatched", "worker_idle_dispatched")
        })
        .collect()
}

fn collect_team_permission_review_candidates(
    spec: &Value,
    requester_actor_id: &str,
    requester_role: &str,
) -> anyhow::Result<Vec<PermissionReviewCandidate>> {
    let requester_role = requester_role.trim();
    let leader_member_id = team_leader_member_id(spec)
        .ok_or_else(|| anyhow::anyhow!("team has no leader configured"))?;
    let requester_is_leader =
        requester_actor_id == leader_member_id || requester_role.eq_ignore_ascii_case("leader");
    let mut candidates =
        worker_permission_review_candidates(spec, leader_member_id, requester_actor_id);

    if requester_is_leader {
        if candidates.is_empty() {
            return Err(anyhow::anyhow!(
                "team has no subordinate reviewer configured"
            ));
        }
        return Ok(candidates);
    }

    if leader_member_id != requester_actor_id {
        candidates.push(permission_review_candidate(
            leader_member_id,
            "leader_dispatched",
            "leader_idle_dispatched",
        ));
    }
    if candidates.is_empty() {
        return Err(anyhow::anyhow!(
            "team has no non-requester reviewer configured"
        ));
    }
    Ok(candidates)
}

pub(crate) fn resolve_team_permission_review_target(
    spec: &Value,
    requester_actor_id: &str,
    requester_role: &str,
) -> anyhow::Result<(String, &'static str)> {
    let candidate =
        collect_team_permission_review_candidates(spec, requester_actor_id, requester_role)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("team has no non-requester reviewer configured"))?;
    Ok((candidate.actor_id, candidate.dispatch_status))
}

fn team_leader_member_id(spec: &Value) -> Option<&str> {
    let spec_obj = spec.as_object()?;
    let members = spec_obj.get("members")?.as_array()?;
    let member_ids = members
        .iter()
        .filter_map(member_id_from_spec)
        .collect::<Vec<_>>();
    spec_obj
        .get("leader_member_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && member_ids.contains(value))
        .or_else(|| {
            members
                .iter()
                .filter(|member| {
                    member
                        .get("role")
                        .and_then(Value::as_str)
                        .is_some_and(|role| role.trim().eq_ignore_ascii_case("leader"))
                })
                .find_map(member_id_from_spec)
        })
        .or_else(|| {
            spec_obj
                .get("entrypoint")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty() && member_ids.contains(value))
        })
        .or_else(|| member_ids.first().copied())
}

fn collect_subordinate_reviewers(
    spec: &Value,
    leader_member_id: &str,
    requester_actor_id: &str,
) -> Vec<String> {
    let Some(members) = spec.get("members").and_then(Value::as_array) else {
        return Vec::new();
    };
    let workers = members
        .iter()
        .filter_map(|member| {
            let member_id = member_id_from_spec(member)?;
            let role = member
                .get("role")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_ascii_lowercase);
            Some((member_id, role))
        })
        .filter_map(|(member_id, role)| {
            if member_id == requester_actor_id || member_id == leader_member_id {
                return None;
            }
            if role.as_deref() == Some("worker") {
                return Some(member_id.to_string());
            }
            None
        })
        .collect::<Vec<_>>();
    if !workers.is_empty() {
        return workers;
    }
    members
        .iter()
        .filter_map(|member| {
            let member_id = member_id_from_spec(member)?;
            if member_id == requester_actor_id || member_id == leader_member_id {
                return None;
            }
            Some(member_id.to_string())
        })
        .collect()
}

fn member_id_from_spec(member: &Value) -> Option<&str> {
    member
        .as_object()?
        .get("member_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn build_permission_review_summary(request: &AcpPermissionReviewRequest) -> String {
    let requester = request
        .routing
        .requester_actor_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("worker");
    let tool_name = request
        .tool_call
        .as_ref()
        .and_then(extract_tool_name)
        .unwrap_or("an ACP tool call");
    format!("{requester} requests permission to execute {tool_name}.")
}

fn human_review_reason_text(reason: &str) -> &str {
    match reason {
        "review_timeout" => "Agent review timed out",
        "review_dispatch_failed" | "leader_dispatch_failed" => "Agent review dispatch failed",
        other => other,
    }
}

fn extract_tool_name(value: &Value) -> Option<&str> {
    let obj = value.as_object()?;
    obj.get("title")
        .and_then(Value::as_str)
        .or_else(|| obj.get("name").and_then(Value::as_str))
        .or_else(|| {
            obj.get("tool")
                .and_then(Value::as_object)
                .and_then(|tool| tool.get("name"))
                .and_then(Value::as_str)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    use crate::api::team_tests::build_test_state;
    use crate::team::TeamDefinitionConfig;
    use crate::team::mailbox_hint::RunningActorRuntime;
    use agenthub_acp::AcpPermissionRoutingMetadata;
    use agenthub_acp::acp_permission_review_timeout;
    use agenthub_team_actor::{ActorInboxRequest, ActorMailboxService};
    use serde_json::json;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    #[derive(Default)]
    struct TestMailboxHintAgentNudger {
        runtimes: HashMap<String, RunningActorRuntime>,
        idle_anchor_by_actor: HashMap<String, Option<i64>>,
        prompts: Mutex<Vec<(String, Option<String>, String)>>,
    }

    #[async_trait::async_trait]
    impl TeamMailboxHintAgentNudger for TestMailboxHintAgentNudger {
        async fn running_actor_runtime(&self, actor_id: &str) -> Option<RunningActorRuntime> {
            self.runtimes.get(actor_id).cloned()
        }

        async fn mailbox_idle_anchor_ts(
            &self,
            actor_id: &str,
            _session_id: &str,
        ) -> anyhow::Result<Option<i64>> {
            Ok(self.idle_anchor_by_actor.get(actor_id).cloned().flatten())
        }

        async fn nudge_mailbox_prompt(
            &self,
            actor_id: &str,
            expected_session_id: Option<&str>,
            prompt: &str,
        ) -> anyhow::Result<()> {
            self.prompts.lock().await.push((
                actor_id.to_string(),
                expected_session_id.map(str::to_string),
                prompt.to_string(),
            ));
            Ok(())
        }
    }

    #[test]
    fn permission_review_dispatcher_default_human_fallback_is_forty_seconds() {
        assert_eq!(
            TeamPermissionReviewDispatcherSettings::default().human_fallback_delay,
            DEFAULT_TEAM_PERMISSION_REVIEW_HUMAN_FALLBACK_DELAY
        );
    }

    #[test]
    fn permission_review_human_fallback_stays_below_acp_timeout() {
        assert!(
            TeamPermissionReviewDispatcherSettings::default().human_fallback_delay
                < acp_permission_review_timeout(),
            "human fallback delay must remain below ACP permission timeout"
        );
    }

    #[test]
    fn builds_permission_review_summary_from_tool_name() {
        let request = AcpPermissionReviewRequest {
            request_id: "perm-1".to_string(),
            agent_id: "worker-agent".to_string(),
            agent_session_id: "session-1".to_string(),
            acp_session_id: "acp-1".to_string(),
            tool_call_id: Some("tool-1".to_string()),
            options: Vec::new(),
            tool_call: Some(json!({"tool":{"name":"mcp__fs__read"}})),
            current_run_id: None,
            routing: agenthub_acp::AcpPermissionRoutingMetadata {
                team_id: Some("team-1".to_string()),
                requester_actor_id: Some("worker-1".to_string()),
                requester_role: Some("worker".to_string()),
            },
        };
        assert_eq!(
            build_permission_review_summary(&request),
            "worker-1 requests permission to execute mcp__fs__read."
        );
    }

    #[test]
    fn worker_request_skips_leader_even_if_leader_member_role_is_misconfigured_as_worker() {
        let spec = json!({
            "entrypoint":"leader",
            "leader_member_id":"leader",
            "members":[
                {"member_id":"leader","role":"worker"},
                {"member_id":"reviewer","role":"worker"},
                {"member_id":"worker","role":"worker"}
            ]
        });

        let (reviewer, dispatch_status) =
            resolve_team_permission_review_target(&spec, "worker", "worker")
                .expect("resolve reviewer");

        assert_eq!(reviewer, "reviewer");
        assert_eq!(dispatch_status, "worker_dispatched");
    }

    #[test]
    fn requester_role_is_trimmed_before_review_target_resolution() {
        let spec = json!({
            "entrypoint":"planner",
            "leader_member_id":"planner",
            "members":[
                {"member_id":"planner","role":"leader"},
                {"member_id":"reviewer","role":"worker"}
            ]
        });

        let (reviewer, dispatch_status) =
            resolve_team_permission_review_target(&spec, "planner", " leader ")
                .expect("resolve reviewer");

        assert_eq!(reviewer, "reviewer");
        assert_eq!(dispatch_status, "worker_dispatched");
    }

    #[test]
    fn collect_permission_review_candidates_keeps_leader_as_fallback_after_workers() {
        let spec = json!({
            "entrypoint":"leader",
            "leader_member_id":"leader",
            "members":[
                {"member_id":"leader","role":"leader"},
                {"member_id":"busy","role":"worker"},
                {"member_id":"idle","role":"worker"},
                {"member_id":"worker","role":"worker"}
            ]
        });

        let candidates = collect_team_permission_review_candidates(&spec, "worker", "worker")
            .expect("collect reviewer candidates");

        assert_eq!(
            candidates,
            vec![
                PermissionReviewCandidate {
                    actor_id: "busy".to_string(),
                    dispatch_status: "worker_dispatched",
                    idle_dispatch_status: "worker_idle_dispatched",
                },
                PermissionReviewCandidate {
                    actor_id: "idle".to_string(),
                    dispatch_status: "worker_dispatched",
                    idle_dispatch_status: "worker_idle_dispatched",
                },
                PermissionReviewCandidate {
                    actor_id: "leader".to_string(),
                    dispatch_status: "leader_dispatched",
                    idle_dispatch_status: "leader_idle_dispatched",
                },
            ]
        );
    }

    #[tokio::test]
    async fn dispatches_worker_permission_to_peer_worker_before_human_review() {
        let state = build_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("permission-review-{}", Uuid::new_v4()),
                description: Some("team permission review dispatch".to_string()),
                spec: json!({
                    "entrypoint":"leader",
                    "leader_member_id":"leader",
                    "members":[
                        {"member_id":"leader","role":"leader"},
                        {"member_id":"reviewer","role":"worker"},
                        {"member_id":"worker","role":"worker"}
                    ]
                }),
            })
            .await
            .expect("create team");
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind("worker-agent")
        .bind("worker-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker agent");
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind("worker-session")
        .bind("worker-agent")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker session");
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)
            "#,
        )
        .bind("perm-review-timeout")
        .bind("worker-agent")
        .bind("worker-session")
        .bind("acp-session-1")
        .bind(&team.id)
        .bind("worker")
        .bind("worker")
        .bind("tool-call-1")
        .bind(
            json!([
                {
                    "option_id": "allow",
                    "name": "Allow once",
                    "kind": "allow_once"
                }
            ])
            .to_string(),
        )
        .bind(json!({"tool":{"name":"mcp__fs__read"}}).to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert permission request");

        let dispatcher = TeamPermissionReviewDispatcher::new(
            state.teams.clone(),
            state.agents.clone(),
            Arc::new(AcpPermissionService::new(state.db.clone())),
            TeamPermissionReviewDispatcherSettings {
                human_fallback_delay: Duration::from_millis(10),
            },
        );
        let request = AcpPermissionReviewRequest {
            request_id: "perm-review-timeout".to_string(),
            agent_id: "worker-agent".to_string(),
            agent_session_id: "worker-session".to_string(),
            acp_session_id: "acp-session-1".to_string(),
            tool_call_id: Some("tool-call-1".to_string()),
            options: vec![agenthub_acp::AcpPermissionOption {
                option_id: "allow".to_string(),
                name: "Allow once".to_string(),
                kind: agent_client_protocol::PermissionOptionKind::AllowOnce,
            }],
            tool_call: Some(json!({"tool":{"name":"mcp__fs__read"}})),
            current_run_id: None,
            routing: AcpPermissionRoutingMetadata {
                team_id: Some(team.id.clone()),
                requester_actor_id: Some("worker".to_string()),
                requester_role: Some("worker".to_string()),
            },
        };

        dispatcher
            .dispatch_review(request.clone())
            .await
            .expect("dispatch permission review");

        let record = state
            .acp_permissions
            .get("perm-review-timeout")
            .await
            .expect("load permission record")
            .expect("permission record");
        assert_eq!(record.review_target_actor_id.as_deref(), Some("reviewer"));
        assert_eq!(
            record.review_dispatch_status.as_deref(),
            Some("worker_dispatched")
        );
        let run_id = record
            .review_delivery_run_id
            .as_deref()
            .expect("review run id")
            .to_string();

        let inbox = state
            .teams
            .actor_mailbox_service()
            .actor_inbox(ActorInboxRequest {
                run_id: run_id.clone(),
                actor_id: "reviewer".to_string(),
                cursor: None,
                limit: Some(20),
                states: None,
            })
            .await
            .expect("load reviewer inbox");
        assert_eq!(inbox.messages.len(), 1);
        assert_eq!(
            inbox.messages[0].payload["type"],
            Value::from(TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE)
        );

        let (task_id, _) = state
            .teams
            .ensure_shared_thread_target_for_team(&team.id, "worker")
            .await
            .expect("ensure shared thread");
        dispatcher
            .notify_human_review_if_pending(&request, "review_timeout")
            .await
            .expect("fallback to human review");

        let conversation_messages = state
            .teams
            .list_task_conversation_messages(&task_id, 50, None)
            .await
            .expect("list shared-thread messages");
        let fallback = conversation_messages.iter().find(|message| {
            message.payload.get("type").and_then(Value::as_str)
                == Some(TEAM_HUMAN_PERMISSION_CARD_PAYLOAD_TYPE)
                && message.payload.get("permission_id").and_then(Value::as_str)
                    == Some("perm-review-timeout")
        });
        let record_after_timeout = state
            .acp_permissions
            .get("perm-review-timeout")
            .await
            .expect("reload permission record")
            .expect("permission record after timeout");
        let fallback = fallback.unwrap_or_else(|| {
            panic!(
                "human-review fallback message missing; record={record_after_timeout:?} messages={conversation_messages:?}"
            )
        });
        assert_eq!(fallback.from_actor_id, "worker");
        assert_eq!(
            fallback.payload["reason_text"],
            json!("Agent review timed out")
        );
        assert_eq!(fallback.payload["status"], json!("pending"));

        assert_eq!(
            record_after_timeout.review_dispatch_status.as_deref(),
            Some("review_timeout")
        );
        assert!(record_after_timeout.human_review_notified_at.is_some());
    }

    #[tokio::test]
    async fn dispatches_worker_permission_to_idle_peer_worker_before_busy_peer() {
        let state = build_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("permission-review-idle-first-{}", Uuid::new_v4()),
                description: Some("idle-first permission review dispatch".to_string()),
                spec: json!({
                    "entrypoint":"leader",
                    "leader_member_id":"leader",
                    "members":[
                        {"member_id":"leader","role":"leader"},
                        {"member_id":"busy","role":"worker"},
                        {"member_id":"idle","role":"worker"},
                        {"member_id":"worker","role":"worker"}
                    ]
                }),
            })
            .await
            .expect("create team");
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind("worker-agent")
        .bind("worker-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker agent");
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind("worker-session")
        .bind("worker-agent")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert worker session");
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)
            "#,
        )
        .bind("perm-review-idle-first")
        .bind("worker-agent")
        .bind("worker-session")
        .bind("acp-session-idle-first")
        .bind(&team.id)
        .bind("worker")
        .bind("worker")
        .bind("tool-call-idle-first")
        .bind(
            json!([
                {
                    "option_id": "allow",
                    "name": "Allow once",
                    "kind": "allow_once"
                }
            ])
            .to_string(),
        )
        .bind(json!({"tool":{"name":"mcp__fs__read"}}).to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert permission request");

        let nudger = Arc::new(TestMailboxHintAgentNudger {
            runtimes: HashMap::from([
                (
                    "busy".to_string(),
                    RunningActorRuntime {
                        session_id: "session-busy".to_string(),
                        current_run_id: Some("run-busy".to_string()),
                    },
                ),
                (
                    "idle".to_string(),
                    RunningActorRuntime {
                        session_id: "session-idle".to_string(),
                        current_run_id: Some("run-idle".to_string()),
                    },
                ),
            ]),
            idle_anchor_by_actor: HashMap::from([
                ("busy".to_string(), Some(now - 10)),
                ("idle".to_string(), Some(now - 600)),
            ]),
            prompts: Mutex::new(Vec::new()),
        });
        let dispatcher = TeamPermissionReviewDispatcher::with_agent_nudger(
            state.teams.clone(),
            nudger.clone(),
            Arc::new(AcpPermissionService::new(state.db.clone())),
            TeamPermissionReviewDispatcherSettings {
                human_fallback_delay: Duration::from_millis(10),
            },
        );
        let request = AcpPermissionReviewRequest {
            request_id: "perm-review-idle-first".to_string(),
            agent_id: "worker-agent".to_string(),
            agent_session_id: "worker-session".to_string(),
            acp_session_id: "acp-session-idle-first".to_string(),
            tool_call_id: Some("tool-call-idle-first".to_string()),
            options: vec![agenthub_acp::AcpPermissionOption {
                option_id: "allow".to_string(),
                name: "Allow once".to_string(),
                kind: agent_client_protocol::PermissionOptionKind::AllowOnce,
            }],
            tool_call: Some(json!({"tool":{"name":"mcp__fs__read"}})),
            current_run_id: None,
            routing: AcpPermissionRoutingMetadata {
                team_id: Some(team.id.clone()),
                requester_actor_id: Some("worker".to_string()),
                requester_role: Some("worker".to_string()),
            },
        };

        dispatcher
            .dispatch_review(request)
            .await
            .expect("dispatch permission review");

        let record = state
            .acp_permissions
            .get("perm-review-idle-first")
            .await
            .expect("load permission record")
            .expect("permission record");
        assert_eq!(record.review_target_actor_id.as_deref(), Some("idle"));
        assert_eq!(
            record.review_dispatch_status.as_deref(),
            Some("worker_idle_dispatched")
        );
        let run_id = record
            .review_delivery_run_id
            .as_deref()
            .expect("review run id")
            .to_string();

        let idle_inbox = state
            .teams
            .actor_mailbox_service()
            .actor_inbox(ActorInboxRequest {
                run_id: run_id.clone(),
                actor_id: "idle".to_string(),
                cursor: None,
                limit: Some(20),
                states: None,
            })
            .await
            .expect("load idle reviewer inbox");
        assert_eq!(idle_inbox.messages.len(), 1);

        let busy_inbox = state
            .teams
            .actor_mailbox_service()
            .actor_inbox(ActorInboxRequest {
                run_id,
                actor_id: "busy".to_string(),
                cursor: None,
                limit: Some(20),
                states: None,
            })
            .await
            .expect("load busy reviewer inbox");
        assert!(busy_inbox.messages.is_empty());

        let prompts = nudger.prompts.lock().await;
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].0, "idle");
        assert!(prompts[0].2.contains(TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE));
    }

    #[tokio::test]
    async fn dispatches_leader_permission_to_subordinate_worker() {
        let state = build_test_state().await;
        let team = state
            .teams
            .create_team(TeamDefinitionConfig {
                name: format!("permission-review-leader-{}", Uuid::new_v4()),
                description: Some("leader permission review dispatch".to_string()),
                spec: json!({
                    "entrypoint":"leader",
                    "leader_member_id":"leader",
                    "members":[
                        {"member_id":"leader","role":"leader"},
                        {"member_id":"reviewer","role":"worker"},
                        {"member_id":"worker","role":"worker"}
                    ]
                }),
            })
            .await
            .expect("create team");
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind("leader-agent")
        .bind("leader-agent")
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert leader agent");
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
        )
        .bind("leader-session")
        .bind("leader-agent")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert leader session");
        sqlx::query(
            r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)
            "#,
        )
        .bind("perm-review-leader")
        .bind("leader-agent")
        .bind("leader-session")
        .bind("acp-session-leader")
        .bind(&team.id)
        .bind("leader")
        .bind("leader")
        .bind("tool-call-leader")
        .bind(
            json!([
                {
                    "option_id": "allow",
                    "name": "Allow once",
                    "kind": "allow_once"
                }
            ])
            .to_string(),
        )
        .bind(json!({"tool":{"name":"mcp__fs__write"}}).to_string())
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert leader permission request");

        let dispatcher = TeamPermissionReviewDispatcher::new(
            state.teams.clone(),
            state.agents.clone(),
            Arc::new(AcpPermissionService::new(state.db.clone())),
            TeamPermissionReviewDispatcherSettings {
                human_fallback_delay: Duration::from_millis(10),
            },
        );
        let request = AcpPermissionReviewRequest {
            request_id: "perm-review-leader".to_string(),
            agent_id: "leader-agent".to_string(),
            agent_session_id: "leader-session".to_string(),
            acp_session_id: "acp-session-leader".to_string(),
            tool_call_id: Some("tool-call-leader".to_string()),
            options: vec![agenthub_acp::AcpPermissionOption {
                option_id: "allow".to_string(),
                name: "Allow once".to_string(),
                kind: agent_client_protocol::PermissionOptionKind::AllowOnce,
            }],
            tool_call: Some(json!({"tool":{"name":"mcp__fs__write"}})),
            current_run_id: None,
            routing: AcpPermissionRoutingMetadata {
                team_id: Some(team.id.clone()),
                requester_actor_id: Some("leader".to_string()),
                requester_role: Some("leader".to_string()),
            },
        };

        dispatcher
            .dispatch_review(request)
            .await
            .expect("dispatch leader permission review");

        let record = state
            .acp_permissions
            .get("perm-review-leader")
            .await
            .expect("load permission record")
            .expect("permission record");
        assert_eq!(record.review_target_actor_id.as_deref(), Some("reviewer"));
        assert_eq!(
            record.review_dispatch_status.as_deref(),
            Some("worker_dispatched")
        );
        let run_id = record
            .review_delivery_run_id
            .as_deref()
            .expect("review run id")
            .to_string();

        let inbox = state
            .teams
            .actor_mailbox_service()
            .actor_inbox(ActorInboxRequest {
                run_id,
                actor_id: "reviewer".to_string(),
                cursor: None,
                limit: Some(20),
                states: None,
            })
            .await
            .expect("load reviewer inbox");
        assert_eq!(inbox.messages.len(), 1);
        assert_eq!(
            inbox.messages[0].payload["review_target_actor_id"],
            Value::from("reviewer")
        );
    }
}
