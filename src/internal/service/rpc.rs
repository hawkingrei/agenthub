use super::*;

#[tonic::async_trait]
impl TeamInternalControl for TeamInternalControlService {
    async fn register_loop_schedule(
        &self,
        request: Request<RegisterLoopScheduleRequest>,
    ) -> Result<Response<RegisterLoopScheduleResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "register_loop_schedule", async move {
            service.register_loop_schedule_request(request).await
        })
        .await
    }

    async fn list_loop_schedules(
        &self,
        request: Request<ListLoopSchedulesRequest>,
    ) -> Result<Response<ListLoopSchedulesResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "list_loop_schedules", async move {
            service.list_loop_schedules_request(request).await
        })
        .await
    }

    async fn get_loop_schedule(
        &self,
        request: Request<GetLoopScheduleRequest>,
    ) -> Result<Response<GetLoopScheduleResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "get_loop_schedule", async move {
            service.get_loop_schedule_request(request).await
        })
        .await
    }

    async fn revoke_loop_schedule(
        &self,
        request: Request<RevokeLoopScheduleRequest>,
    ) -> Result<Response<RevokeLoopScheduleResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "revoke_loop_schedule", async move {
            service.revoke_loop_schedule_request(request).await
        })
        .await
    }

    type ExchangeMcpProxyStream = super::mcp_proxy::McpResponseStream;
    type ListenMcpProxyStream = super::mcp_proxy::McpResponseStream;

    async fn listen_mcp_proxy(
        &self,
        request: Request<crate::internal::proto::agenthub::internal::v1::ListenMcpProxyRequest>,
    ) -> Result<Response<Self::ListenMcpProxyStream>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "listen_mcp_proxy", async move {
            service.listen_mcp_proxy_request(request).await
        })
        .await
    }

    async fn open_mcp_proxy(
        &self,
        request: Request<crate::internal::proto::agenthub::internal::v1::OpenMcpProxyRequest>,
    ) -> Result<
        Response<crate::internal::proto::agenthub::internal::v1::OpenMcpProxyResponse>,
        Status,
    > {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "open_mcp_proxy", async move {
            service.open_mcp_proxy_request(request).await
        })
        .await
    }

    async fn exchange_mcp_proxy(
        &self,
        request: Request<crate::internal::proto::agenthub::internal::v1::ExchangeMcpProxyRequest>,
    ) -> Result<Response<Self::ExchangeMcpProxyStream>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "exchange_mcp_proxy", async move {
            service.exchange_mcp_proxy_request(request).await
        })
        .await
    }

    async fn close_mcp_proxy(
        &self,
        request: Request<crate::internal::proto::agenthub::internal::v1::CloseMcpProxyRequest>,
    ) -> Result<
        Response<crate::internal::proto::agenthub::internal::v1::CloseMcpProxyResponse>,
        Status,
    > {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "close_mcp_proxy", async move {
            service.close_mcp_proxy_request(request).await
        })
        .await
    }

    async fn finish_loop_activation(
        &self,
        request: Request<FinishLoopActivationRequest>,
    ) -> Result<Response<FinishLoopActivationResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "finish_loop_activation", async move {
            service.finish_loop_activation_request(request).await
        })
        .await
    }

    async fn send_actor_message(
        &self,
        request: Request<SendActorMessageRequest>,
    ) -> Result<Response<SendActorMessageResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "send_actor_message", async move {
            service.send_actor_message_request(request).await
        })
        .await
    }

    async fn list_actor_inbox(
        &self,
        request: Request<ListActorInboxRequest>,
    ) -> Result<Response<ListActorInboxResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "list_actor_inbox", async move {
            service.list_actor_inbox_request(request).await
        })
        .await
    }

    async fn ack_actor_message(
        &self,
        request: Request<AckActorMessageRequest>,
    ) -> Result<Response<AckActorMessageResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "ack_actor_message", async move {
            service.ack_actor_message_request(request).await
        })
        .await
    }

    async fn triage_actor_message(
        &self,
        request: Request<TriageActorMessageRequest>,
    ) -> Result<Response<TriageActorMessageResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "triage_actor_message", async move {
            service.triage_actor_message_request(request).await
        })
        .await
    }

    async fn link_actor_message_task(
        &self,
        request: Request<LinkActorMessageTaskRequest>,
    ) -> Result<Response<LinkActorMessageTaskResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "link_actor_message_task", async move {
            service.link_actor_message_task_request(request).await
        })
        .await
    }

    async fn describe_team_context(
        &self,
        request: Request<DescribeTeamContextRequest>,
    ) -> Result<Response<DescribeTeamContextResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "describe_team_context", async move {
            service.describe_team_context_request(request).await
        })
        .await
    }

    async fn resolve_actor_run_scope(
        &self,
        request: Request<ResolveActorRunScopeRequest>,
    ) -> Result<Response<ResolveActorRunScopeResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "resolve_actor_run_scope", async move {
            service.resolve_actor_run_scope_request(request).await
        })
        .await
    }

    async fn list_team_tasks(
        &self,
        request: Request<ListTeamTasksRequest>,
    ) -> Result<Response<ListTeamTasksResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "list_team_tasks", async move {
            service.list_team_tasks_request(request).await
        })
        .await
    }

    async fn create_team_task(
        &self,
        request: Request<CreateTeamTaskRequest>,
    ) -> Result<Response<CreateTeamTaskResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "create_team_task", async move {
            service.create_team_task_request(request).await
        })
        .await
    }

    async fn update_team_task(
        &self,
        request: Request<UpdateTeamTaskRequest>,
    ) -> Result<Response<UpdateTeamTaskResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "update_team_task", async move {
            service.update_team_task_request(request).await
        })
        .await
    }

    async fn get_team_task(
        &self,
        request: Request<GetTeamTaskRequest>,
    ) -> Result<Response<GetTeamTaskResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "get_team_task", async move {
            service.get_team_task_request(request).await
        })
        .await
    }

    async fn create_team_channel(
        &self,
        request: Request<CreateTeamChannelRequest>,
    ) -> Result<Response<CreateTeamChannelResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "create_team_channel", async move {
            service.create_team_channel_request(request).await
        })
        .await
    }

    async fn delete_team_channel(
        &self,
        request: Request<DeleteTeamChannelRequest>,
    ) -> Result<Response<DeleteTeamChannelResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "delete_team_channel", async move {
            service.delete_team_channel_request(request).await
        })
        .await
    }

    async fn open_team_thread(
        &self,
        request: Request<OpenTeamThreadRequest>,
    ) -> Result<Response<OpenTeamThreadResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "open_team_thread", async move {
            service.open_team_thread_request(request).await
        })
        .await
    }

    async fn reply_team_thread(
        &self,
        request: Request<ReplyTeamThreadRequest>,
    ) -> Result<Response<ReplyTeamThreadResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "reply_team_thread", async move {
            service.reply_team_thread_request(request).await
        })
        .await
    }

    async fn append_team_task_note(
        &self,
        request: Request<AppendTeamTaskNoteRequest>,
    ) -> Result<Response<AppendTeamTaskNoteResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "append_team_task_note", async move {
            service.append_team_task_note_request(request).await
        })
        .await
    }

    async fn create_time_trigger(
        &self,
        request: Request<CreateTimeTriggerRequest>,
    ) -> Result<Response<CreateTimeTriggerResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "create_time_trigger", async move {
            service.create_time_trigger_request(request).await
        })
        .await
    }

    async fn list_time_triggers(
        &self,
        request: Request<ListTimeTriggersRequest>,
    ) -> Result<Response<ListTimeTriggersResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "list_time_triggers", async move {
            service.list_time_triggers_request(request).await
        })
        .await
    }

    async fn cancel_time_trigger(
        &self,
        request: Request<CancelTimeTriggerRequest>,
    ) -> Result<Response<CancelTimeTriggerResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "cancel_time_trigger", async move {
            service.cancel_time_trigger_request(request).await
        })
        .await
    }

    async fn respond_permission_review(
        &self,
        request: Request<RespondPermissionReviewRequest>,
    ) -> Result<Response<RespondPermissionReviewResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "respond_permission_review", async move {
            service.respond_permission_review_request(request).await
        })
        .await
    }

    async fn transition_step(
        &self,
        request: Request<TransitionStepRequest>,
    ) -> Result<Response<TransitionStepResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "transition_step", async move {
            service.transition_step_request(request).await
        })
        .await
    }

    async fn issue_node_credential(
        &self,
        request: Request<IssueNodeCredentialRequest>,
    ) -> Result<Response<IssueNodeCredentialResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "issue_node_credential", async move {
            service.issue_node_credential_request(request).await
        })
        .await
    }

    async fn ensure_agent_record(
        &self,
        request: Request<EnsureAgentRecordRequest>,
    ) -> Result<Response<EnsureAgentRecordResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "ensure_agent_record", async move {
            service.ensure_agent_record_request(request).await
        })
        .await
    }

    async fn get_agent_record(
        &self,
        request: Request<GetAgentRecordRequest>,
    ) -> Result<Response<GetAgentRecordResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "get_agent_record", async move {
            service.get_agent_record_request(request).await
        })
        .await
    }

    async fn start_managed_agent(
        &self,
        request: Request<StartManagedAgentRequest>,
    ) -> Result<Response<StartManagedAgentResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "start_managed_agent", async move {
            service.start_managed_agent_request(request).await
        })
        .await
    }

    async fn stop_managed_agent(
        &self,
        request: Request<StopManagedAgentRequest>,
    ) -> Result<Response<StopManagedAgentResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "stop_managed_agent", async move {
            service.stop_managed_agent_request(request).await
        })
        .await
    }

    async fn delete_managed_agent(
        &self,
        request: Request<DeleteManagedAgentRequest>,
    ) -> Result<Response<DeleteManagedAgentResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "delete_managed_agent", async move {
            service.delete_managed_agent_request(request).await
        })
        .await
    }

    async fn send_agent_input(
        &self,
        request: Request<SendAgentInputRequest>,
    ) -> Result<Response<SendAgentInputResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "send_agent_input", async move {
            service.send_agent_input_request(request).await
        })
        .await
    }

    async fn send_agent_reminder(
        &self,
        request: Request<crate::internal::proto::agenthub::internal::v1::SendAgentReminderRequest>,
    ) -> Result<Response<SendAgentInputResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "send_agent_reminder", async move {
            service.send_agent_reminder_request(request).await
        })
        .await
    }

    async fn list_agent_events(
        &self,
        request: Request<ListAgentEventsRequest>,
    ) -> Result<Response<ListAgentEventsResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "list_agent_events", async move {
            service.list_agent_events_request(request).await
        })
        .await
    }

    async fn get_loop_work_source(
        &self,
        request: Request<GetLoopWorkSourceRequest>,
    ) -> Result<Response<GetLoopWorkSourceResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "get_loop_work_source", async move {
            service.get_loop_work_source_request(request).await
        })
        .await
    }

    async fn get_loop_work(
        &self,
        request: Request<GetLoopWorkRequest>,
    ) -> Result<Response<GetLoopWorkResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "get_loop_work", async move {
            service.get_loop_work_request(request).await
        })
        .await
    }

    async fn activate_loop_member(
        &self,
        request: Request<ActivateLoopMemberRequest>,
    ) -> Result<Response<ActivateLoopMemberResponse>, Status> {
        let metadata = request.metadata().clone();
        let service = self.clone();
        self.complete_control_request(&metadata, "activate_loop_member", async move {
            service.activate_loop_member_request(request).await
        })
        .await
    }
}
