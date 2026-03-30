use crate::acp::AcpActorSkillContext;
use crate::agent::{AgentConfig, AgentEvent, AgentRecord, AgentTimeTriggerRecord};
use crate::team::{
    TeamContextRecord, TeamTaskDetailRecord, TeamTaskListQuery, TeamTaskRecord, TeamTaskStatus,
};

use super::super::proto::agenthub::internal::v1::{
    AppendTeamTaskNoteRequest as GrpcAppendTeamTaskNoteRequest,
    CancelTimeTriggerRequest as GrpcCancelTimeTriggerRequest,
    CreateTeamTaskRequest as GrpcCreateTeamTaskRequest,
    CreateTimeTriggerRequest as GrpcCreateTimeTriggerRequest,
    DeleteManagedAgentRequest as GrpcDeleteManagedAgentRequest,
    DescribeTeamContextRequest as GrpcDescribeTeamContextRequest,
    EnsureAgentRecordRequest as GrpcEnsureAgentRecordRequest,
    GetAgentRecordRequest as GrpcGetAgentRecordRequest,
    GetTeamTaskRequest as GrpcGetTeamTaskRequest,
    ListAgentEventsRequest as GrpcListAgentEventsRequest,
    ListTeamTasksRequest as GrpcListTeamTasksRequest,
    ListTimeTriggersRequest as GrpcListTimeTriggersRequest,
    ResolveActorRunScopeRequest as GrpcResolveActorRunScopeRequest,
    RespondPermissionReviewRequest as GrpcRespondPermissionReviewRequest,
    RespondPermissionReviewResponse as GrpcRespondPermissionReviewResponse,
    SendAgentInputRequest as GrpcSendAgentInputRequest,
    StartManagedAgentRequest as GrpcStartManagedAgentRequest,
    StopManagedAgentRequest as GrpcStopManagedAgentRequest,
    UpdateTeamTaskRequest as GrpcUpdateTeamTaskRequest,
};
use super::{
    InternalActorRunScopeResolution, InternalGrpcMailboxClient, InternalPermissionReviewResponse,
    InternalTeamTaskPatch, map_grpc_status_anyhow, parse_agent_events, parse_json_response,
    timeout_internal_grpc_call,
};

impl InternalGrpcMailboxClient {
    pub async fn ensure_agent_record(
        &self,
        agent_id: &str,
        config: &AgentConfig,
        source: &str,
    ) -> anyhow::Result<AgentRecord> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.ensure_agent_record(
            self.control_request(GrpcEnsureAgentRecordRequest {
                agent_id: agent_id.trim().to_string(),
                config_json: serde_json::to_string(config)?,
                source: source.trim().to_string(),
            })?,
        ))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        Ok(serde_json::from_str(&response.agent_json)?)
    }

    #[allow(dead_code)]
    pub async fn get_agent_record(&self, agent_id: &str) -> anyhow::Result<AgentRecord> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.get_agent_record(self.control_request(
            GrpcGetAgentRecordRequest {
                agent_id: agent_id.trim().to_string(),
            },
        )?))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        Ok(serde_json::from_str(&response.agent_json)?)
    }

    pub async fn start_managed_agent(
        &self,
        agent_id: &str,
        actor_context: Option<&AcpActorSkillContext>,
    ) -> anyhow::Result<String> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(
            client.start_managed_agent(
                self.control_request(GrpcStartManagedAgentRequest {
                    agent_id: agent_id.trim().to_string(),
                    actor_context_json: actor_context
                        .map(serde_json::to_string)
                        .transpose()?
                        .unwrap_or_default(),
                })?,
            ),
        )
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        Ok(response.session_id)
    }

    pub async fn stop_managed_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        let mut client = self.client();
        timeout_internal_grpc_call(client.stop_managed_agent(self.control_request(
            GrpcStopManagedAgentRequest {
                agent_id: agent_id.trim().to_string(),
            },
        )?))
        .await
        .map_err(map_grpc_status_anyhow)?;
        Ok(())
    }

    pub async fn delete_managed_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        let mut client = self.client();
        timeout_internal_grpc_call(client.delete_managed_agent(self.control_request(
            GrpcDeleteManagedAgentRequest {
                agent_id: agent_id.trim().to_string(),
            },
        )?))
        .await
        .map_err(map_grpc_status_anyhow)?;
        Ok(())
    }

    pub async fn send_agent_input(
        &self,
        agent_id: &str,
        input: &str,
        message_id: Option<&str>,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut client = self.client();
        timeout_internal_grpc_call(client.send_agent_input(self.control_request(
            GrpcSendAgentInputRequest {
                agent_id: agent_id.trim().to_string(),
                input: input.to_string(),
                message_id: message_id.unwrap_or_default().to_string(),
                session_id: session_id.unwrap_or_default().to_string(),
            },
        )?))
        .await
        .map_err(map_grpc_status_anyhow)?;
        Ok(())
    }

    pub async fn respond_permission_review(
        &self,
        team_id: &str,
        actor_id: &str,
        permission_id: &str,
        option_id: Option<&str>,
        outcome: Option<&str>,
    ) -> anyhow::Result<InternalPermissionReviewResponse> {
        let mut client = self.client();
        let response: GrpcRespondPermissionReviewResponse =
            timeout_internal_grpc_call(client.respond_permission_review(self.control_request(
                GrpcRespondPermissionReviewRequest {
                    team_id: team_id.trim().to_string(),
                    actor_id: actor_id.trim().to_string(),
                    permission_id: permission_id.trim().to_string(),
                    option_id: option_id.unwrap_or_default().trim().to_string(),
                    outcome: outcome.unwrap_or_default().trim().to_string(),
                },
            )?))
            .await
            .map_err(map_grpc_status_anyhow)?
            .into_inner();
        Ok(InternalPermissionReviewResponse {
            status: response.status,
            permission_id: response.permission_id,
            request_status: response.request_status,
            reviewed_by_actor_id: response.reviewed_by_actor_id,
        })
    }

    pub async fn describe_team_context(
        &self,
        team_id: Option<&str>,
        run_id: Option<&str>,
        actor_id: &str,
    ) -> anyhow::Result<TeamContextRecord> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.describe_team_context(
            self.control_request(GrpcDescribeTeamContextRequest {
                team_id: team_id.unwrap_or_default().trim().to_string(),
                run_id: run_id.unwrap_or_default().trim().to_string(),
                actor_id: actor_id.trim().to_string(),
            })?,
        ))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.context_json, "context_json")
    }

    pub async fn resolve_actor_run_scope(
        &self,
        actor_id: &str,
        team_id: Option<&str>,
    ) -> anyhow::Result<InternalActorRunScopeResolution> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.resolve_actor_run_scope(
            self.control_request(GrpcResolveActorRunScopeRequest {
                actor_id: actor_id.trim().to_string(),
                team_id: team_id.unwrap_or_default().trim().to_string(),
            })?,
        ))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        Ok(InternalActorRunScopeResolution {
            run_id: response.run_id,
            team_id: (!response.team_id.trim().is_empty()).then_some(response.team_id),
            source: response.source,
        })
    }

    pub async fn list_team_tasks(
        &self,
        actor_id: &str,
        query: &TeamTaskListQuery,
    ) -> anyhow::Result<Vec<TeamTaskRecord>> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(
            client.list_team_tasks(
                self.control_request(GrpcListTeamTasksRequest {
                    team_id: query
                        .team_id
                        .as_deref()
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    actor_id: actor_id.trim().to_string(),
                    limit: query.limit,
                    status: query
                        .status
                        .as_ref()
                        .map(TeamTaskStatus::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    include_shared_thread: query.include_shared_thread,
                    run_id: query
                        .run_id
                        .as_deref()
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    task_id: query
                        .task_id
                        .as_deref()
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    assigned_member_id: query
                        .assigned_member_id
                        .as_deref()
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    topic: query
                        .topic
                        .as_deref()
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                })?,
            ),
        )
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.tasks_json, "tasks_json")
    }

    pub async fn create_team_task(
        &self,
        team_id: &str,
        actor_id: &str,
        title: &str,
        status: &str,
        topic: Option<&str>,
        context: &serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.create_team_task(self.control_request(
            GrpcCreateTeamTaskRequest {
                team_id: team_id.trim().to_string(),
                actor_id: actor_id.trim().to_string(),
                title: title.to_string(),
                status: status.trim().to_string(),
                topic: topic.unwrap_or_default().trim().to_string(),
                context_json: serde_json::to_string(context)?,
            },
        )?))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.output_json, "output_json")
    }

    pub async fn update_team_task(
        &self,
        team_id: &str,
        actor_id: &str,
        task_id: &str,
        patch: InternalTeamTaskPatch<'_>,
    ) -> anyhow::Result<TeamTaskRecord> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(
            client.update_team_task(
                self.control_request(GrpcUpdateTeamTaskRequest {
                    team_id: team_id.trim().to_string(),
                    actor_id: actor_id.trim().to_string(),
                    task_id: task_id.trim().to_string(),
                    status: patch
                        .status
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string),
                    assigned_member_id: patch
                        .assigned_member_id
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string),
                    clear_assigned_member_id: patch.clear_assigned_member_id,
                    context_json: patch.context_json.map(serde_json::to_string).transpose()?,
                    context_merge_json: patch
                        .context_merge_json
                        .map(serde_json::to_string)
                        .transpose()?,
                })?,
            ),
        )
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.task_json, "task_json")
    }

    pub async fn get_team_task(
        &self,
        actor_id: &str,
        team_id: Option<&str>,
        run_id: Option<&str>,
        task_id: &str,
        message_limit: i64,
    ) -> anyhow::Result<TeamTaskDetailRecord> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.get_team_task(self.control_request(
            GrpcGetTeamTaskRequest {
                team_id: team_id.unwrap_or_default().trim().to_string(),
                run_id: run_id.unwrap_or_default().trim().to_string(),
                actor_id: actor_id.trim().to_string(),
                task_id: task_id.trim().to_string(),
                message_limit,
            },
        )?))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.detail_json, "detail_json")
    }

    pub async fn append_team_task_note(
        &self,
        actor_id: &str,
        team_id: Option<&str>,
        run_id: Option<&str>,
        task_id: &str,
        kind: &str,
        text: &str,
    ) -> anyhow::Result<crate::team::TeamConversationMessageRecord> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.append_team_task_note(
            self.control_request(GrpcAppendTeamTaskNoteRequest {
                team_id: team_id.unwrap_or_default().trim().to_string(),
                run_id: run_id.unwrap_or_default().trim().to_string(),
                actor_id: actor_id.trim().to_string(),
                task_id: task_id.trim().to_string(),
                kind: kind.trim().to_string(),
                text: text.to_string(),
            })?,
        ))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.message_json, "message_json")
    }

    pub async fn create_time_trigger(
        &self,
        actor_id: &str,
        message_text: &str,
        fire_at: i64,
    ) -> anyhow::Result<AgentTimeTriggerRecord> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.create_time_trigger(
            self.control_request(GrpcCreateTimeTriggerRequest {
                actor_id: actor_id.trim().to_string(),
                message_text: message_text.to_string(),
                fire_at,
            })?,
        ))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.trigger_json, "trigger_json")
    }

    pub async fn list_time_triggers(
        &self,
        actor_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<AgentTimeTriggerRecord>> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.list_time_triggers(
            self.control_request(GrpcListTimeTriggersRequest {
                actor_id: actor_id.trim().to_string(),
                limit,
            })?,
        ))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.triggers_json, "triggers_json")
    }

    pub async fn cancel_time_trigger(
        &self,
        actor_id: &str,
        trigger_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.cancel_time_trigger(
            self.control_request(GrpcCancelTimeTriggerRequest {
                actor_id: actor_id.trim().to_string(),
                trigger_id: trigger_id.trim().to_string(),
            })?,
        ))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_json_response(&response.output_json, "output_json")
    }

    pub async fn list_agent_events(
        &self,
        agent_id: &str,
        limit: i64,
        session_id: Option<&str>,
        before_event_id: Option<i64>,
    ) -> anyhow::Result<Vec<AgentEvent>> {
        let mut client = self.client();
        let response = timeout_internal_grpc_call(client.list_agent_events(self.control_request(
            GrpcListAgentEventsRequest {
                agent_id: agent_id.trim().to_string(),
                limit,
                before_event_id: before_event_id.unwrap_or_default(),
                session_id: session_id.unwrap_or_default().to_string(),
            },
        )?))
        .await
        .map_err(map_grpc_status_anyhow)?
        .into_inner();
        parse_agent_events(response)
    }
}
