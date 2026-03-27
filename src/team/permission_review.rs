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

const TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE: &str = "permission_review_request";
const TEAM_HUMAN_PERMISSION_CARD_PAYLOAD_TYPE: &str = "permission_review_card";
const DEFAULT_TEAM_PERMISSION_REVIEW_HUMAN_FALLBACK_DELAY: Duration = Duration::from_secs(600);

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
    agents: Arc<AgentManager>,
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
        Self {
            teams,
            agents,
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
        let (review_target_actor_id, dispatch_status) =
            resolve_team_permission_review_target(&team.spec, requester_actor_id, requester_role)?;

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
        let hint = format!(
            "New mailbox message type '{payload_type}' is pending in run '{run_id}'. Use actor_inbox to inspect pending messages and batch-handle this type before ack."
        );
        if let Err(err) = self.agents.send_input(actor_id, &hint, None, None).await {
            tracing::debug!(
                actor_id = %actor_id,
                run_id = %run_id,
                payload_type = %payload_type,
                "skip permission review mailbox hint because agent input is unavailable: {}",
                err
            );
        }
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

pub(crate) fn resolve_team_permission_review_target(
    spec: &Value,
    requester_actor_id: &str,
    requester_role: &str,
) -> anyhow::Result<(String, &'static str)> {
    let requester_role = requester_role.trim();
    let leader_member_id = team_leader_member_id(spec)
        .ok_or_else(|| anyhow::anyhow!("team has no leader configured"))?;
    let requester_is_leader =
        requester_actor_id == leader_member_id || requester_role.eq_ignore_ascii_case("leader");
    if requester_is_leader {
        let reviewer = select_subordinate_reviewer(spec, leader_member_id, requester_actor_id)
            .ok_or_else(|| anyhow::anyhow!("team has no subordinate reviewer configured"))?;
        return Ok((reviewer.to_string(), "worker_dispatched"));
    }
    if let Some(reviewer) = select_subordinate_reviewer(spec, leader_member_id, requester_actor_id)
    {
        return Ok((reviewer.to_string(), "worker_dispatched"));
    }
    if leader_member_id != requester_actor_id {
        return Ok((leader_member_id.to_string(), "leader_dispatched"));
    }
    Err(anyhow::anyhow!(
        "team has no non-requester reviewer configured"
    ))
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

fn select_subordinate_reviewer<'a>(
    spec: &'a Value,
    leader_member_id: &str,
    requester_actor_id: &str,
) -> Option<&'a str> {
    let members = spec.get("members")?.as_array()?;
    members
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
        .find_map(|(member_id, role)| {
            if member_id == requester_actor_id || member_id == leader_member_id {
                return None;
            }
            if role.as_deref() == Some("worker") {
                return Some(member_id);
            }
            None
        })
        .or_else(|| {
            members.iter().find_map(|member| {
                let member_id = member_id_from_spec(member)?;
                if member_id == requester_actor_id || member_id == leader_member_id {
                    return None;
                }
                Some(member_id)
            })
        })
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
    use std::sync::Arc;

    use crate::api::team_tests::build_test_state;
    use crate::team::TeamDefinitionConfig;
    use agenthub_acp::AcpPermissionRoutingMetadata;
    use agenthub_team_actor::{ActorInboxRequest, ActorMailboxService};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn permission_review_dispatcher_default_human_fallback_is_ten_minutes() {
        assert_eq!(
            TeamPermissionReviewDispatcherSettings::default().human_fallback_delay,
            DEFAULT_TEAM_PERMISSION_REVIEW_HUMAN_FALLBACK_DELAY
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
