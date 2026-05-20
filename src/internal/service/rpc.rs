use super::*;

fn actor_message_to_proto(message: agenthub_team_actor::ActorMessageRecord) -> ActorMessage {
    let route_json = message
        .route
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_default();
    let payload_json = serde_json::to_string(&message.payload).unwrap_or_default();
    ActorMessage {
        message_id: message.message_id,
        run_id: message.run_id,
        from_actor_id: message.from_actor_id,
        to_actor_id: message.to_actor_id,
        channel: message.channel,
        transport: message.transport.as_str().to_string(),
        route_json,
        payload_json,
        status: message.status.as_str().to_string(),
        created_at: message.created_at,
        delivered_at: message.delivered_at.unwrap_or_default(),
        idempotency_key: String::new(),
        from_peer_id: message.from_peer_id,
        to_peer_id: message.to_peer_id,
        message_kind: message.message_kind.as_str().to_string(),
        handling_disposition: message.handling_disposition.as_str().to_string(),
        handled_by_actor_id: message.handled_by_actor_id.unwrap_or_default(),
        handled_at: message.handled_at.unwrap_or_default(),
        thread_topic_key: message.thread_topic_key.unwrap_or_default(),
        thread_claim_status: message
            .thread_claim_status
            .map(|status| status.as_str().to_string())
            .unwrap_or_default(),
        thread_owner_actor_id: message.thread_owner_actor_id.unwrap_or_default(),
        thread_lease_expires_at: message.thread_lease_expires_at.unwrap_or_default(),
        linked_task_id: message.linked_task_id.unwrap_or_default(),
        linked_task_relation: message
            .linked_task_relation
            .map(|relation| relation.as_str().to_string())
            .unwrap_or_default(),
    }
}

#[tonic::async_trait]
impl TeamInternalControl for TeamInternalControlService {
    async fn send_actor_message(
        &self,
        request: Request<SendActorMessageRequest>,
    ) -> Result<Response<SendActorMessageResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::MessageSend)?;
        let payload = request.into_inner();

        let run_id = required_field(&payload.run_id, "run_id")?;
        self.authz.ensure_run_scope(&principal, run_id)?;

        let from_actor_id = required_field(&payload.from_actor_id, "from_actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, from_actor_id, "from_actor_id")?;

        let to_actor_id = optional_trimmed(&payload.to_actor_id);
        let channel_id = optional_trimmed(&payload.channel_id);
        let channel = optional_trimmed(&payload.channel).unwrap_or("default");
        let transport = parse_actor_transport(optional_trimmed(&payload.transport))
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let route = optional_json_object(optional_trimmed(&payload.route_json), "route_json")?;
        let payload_json = parse_json_required(&payload.payload_json, "payload_json")?;
        let channel_replica = resolve_channel_replica_request(&payload_json);
        if is_channel_broadcast_payload(&payload_json) && channel_replica.is_none() {
            return Err(Status::invalid_argument(
                "payload_json must represent a valid channel_broadcast replica payload",
            ));
        }
        let idempotency_key = optional_trimmed(&payload.idempotency_key);
        let from_peer_id = optional_trimmed(&payload.from_peer_id);
        let to_peer_id = optional_trimmed(&payload.to_peer_id);
        let message_kind = optional_trimmed(&payload.message_kind)
            .map(agenthub_team_actor::parse_actor_message_kind);

        if let Some(replica) = channel_replica.as_ref() {
            self.validate_channel_replica_request(run_id, from_actor_id, replica)
                .await?;
        }

        let message = self
            .deps
            .teams
            .actor_mailbox_service()
            .actor_send(ActorSendRequest {
                run_id: run_id.to_string(),
                from_actor_id: from_actor_id.to_string(),
                from_peer_id: from_peer_id.map(str::to_string),
                to_actor_id: to_actor_id.map(str::to_string),
                channel_id: channel_id.map(str::to_string),
                to_peer_id: to_peer_id.map(str::to_string),
                channel: Some(channel.to_string()),
                transport: Some(transport),
                route,
                payload: payload_json.clone(),
                idempotency_key: idempotency_key.map(str::to_string),
                message_kind,
            })
            .await
            .map_err(map_actor_service_status)?;

        if let (Some(replica), Some(source_node_id)) =
            (channel_replica, principal.source_node_id.as_deref())
        {
            self.deps
                .teams
                .append_channel_replica_message(
                    replica.authority_message_id,
                    &replica.correlation_id,
                    run_id,
                    &replica.team_id,
                    &replica.conversation_id,
                    &replica.task_id,
                    &replica.channel_id,
                    from_actor_id,
                    source_node_id,
                    &payload_json,
                )
                .await
                .map_err(map_manager_error)?;
        }

        match tokio::time::timeout(
            MAILBOX_HINT_NOTIFY_TIMEOUT,
            maybe_notify_actor_new_mailbox_message_type(&self.deps, run_id, &message),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                tracing::warn!(
                    run_id = %run_id,
                    to_actor_id = %message.message.to_actor_id,
                    message_id = message.message_id,
                    "team mailbox type hint notify failed: {}",
                    err
                );
            }
            Err(_) => {
                tracing::warn!(
                    run_id = %run_id,
                    to_actor_id = %message.message.to_actor_id,
                    message_id = message.message_id,
                    timeout_ms = MAILBOX_HINT_NOTIFY_TIMEOUT.as_millis(),
                    "team mailbox type hint notify timed out"
                );
            }
        }

        Ok(Response::new(SendActorMessageResponse {
            message_id: message.message_id,
            status: message.state.as_str().to_string(),
            idempotency_key: idempotency_key.unwrap_or("").to_string(),
            message_json: serde_json::to_string(&message.message).unwrap_or_default(),
        }))
    }

    async fn list_actor_inbox(
        &self,
        request: Request<ListActorInboxRequest>,
    ) -> Result<Response<ListActorInboxResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::InboxList)?;
        let payload = request.into_inner();

        let run_id = required_field(&payload.run_id, "run_id")?;
        self.authz.ensure_run_scope(&principal, run_id)?;

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;

        let limit = payload.limit.clamp(1, 1000);
        let after_id = if payload.after_message_id > 0 {
            Some(payload.after_message_id)
        } else {
            None
        };

        let states = if payload.include_delivered {
            Some(vec![
                ActorMessageStatus::Pending,
                ActorMessageStatus::Delivered,
            ])
        } else {
            None
        };
        let response = self
            .deps
            .teams
            .actor_mailbox_service()
            .actor_inbox(ActorInboxRequest {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                cursor: after_id,
                limit: Some(limit),
                states,
            })
            .await
            .map_err(map_actor_service_status)?;
        let messages = response
            .messages
            .into_iter()
            .map(actor_message_to_proto)
            .collect::<Vec<_>>();

        Ok(Response::new(ListActorInboxResponse {
            messages,
            pending_count: response.pending_count,
        }))
    }

    async fn ack_actor_message(
        &self,
        request: Request<AckActorMessageRequest>,
    ) -> Result<Response<AckActorMessageResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::MessageAck)?;
        let payload = request.into_inner();

        let run_id = required_field(&payload.run_id, "run_id")?;
        self.authz.ensure_run_scope(&principal, run_id)?;

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;

        if payload.message_id <= 0 {
            return Err(Status::invalid_argument("message_id must be positive"));
        }
        let message = self
            .deps
            .teams
            .actor_mailbox_service()
            .actor_ack(ActorAckRequest {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                message_id: payload.message_id,
                ack_token: None,
                result: None,
            })
            .await
            .map_err(map_actor_service_status)?;
        Ok(Response::new(AckActorMessageResponse {
            message: Some(actor_message_to_proto(message.message)),
            status_changed: message.status_changed,
        }))
    }

    async fn triage_actor_message(
        &self,
        request: Request<TriageActorMessageRequest>,
    ) -> Result<Response<TriageActorMessageResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::MessageAck)?;
        let payload = request.into_inner();

        let run_id = required_field(&payload.run_id, "run_id")?;
        self.authz.ensure_run_scope(&principal, run_id)?;

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;

        if payload.message_id <= 0 {
            return Err(Status::invalid_argument("message_id must be positive"));
        }
        let disposition = agenthub_team_actor::parse_actor_message_handling_disposition(
            required_field(&payload.disposition, "disposition")?,
        );
        let triaged = self
            .deps
            .teams
            .actor_mailbox_service()
            .actor_triage(ActorTriageRequest {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                message_id: payload.message_id,
                disposition,
            })
            .await
            .map_err(map_actor_service_status)?;
        Ok(Response::new(TriageActorMessageResponse {
            message: Some(actor_message_to_proto(triaged.message)),
            handling_changed: triaged.handling_changed,
            triaged_at: triaged.triaged_at,
        }))
    }

    async fn link_actor_message_task(
        &self,
        request: Request<LinkActorMessageTaskRequest>,
    ) -> Result<Response<LinkActorMessageTaskResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamTaskWrite)?;
        let payload = request.into_inner();

        let run_id = required_field(&payload.run_id, "run_id")?;
        self.authz.ensure_run_scope(&principal, run_id)?;

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;

        if payload.message_id <= 0 {
            return Err(Status::invalid_argument("message_id must be positive"));
        }
        let task_id = required_field(&payload.task_id, "task_id")?;
        let relation = agenthub_team_actor::parse_actor_message_task_relation(required_field(
            &payload.relation,
            "relation",
        )?)
        .ok_or_else(|| {
            Status::invalid_argument(
                "relation must be one of spawned_task, related_task, evidence_for_task",
            )
        })?;

        let linked = self
            .deps
            .teams
            .actor_mailbox_service()
            .actor_task_link(ActorTaskLinkRequest {
                run_id: run_id.to_string(),
                actor_id: actor_id.to_string(),
                message_id: payload.message_id,
                task_id: task_id.to_string(),
                relation,
            })
            .await
            .map_err(map_actor_service_status)?;

        Ok(Response::new(LinkActorMessageTaskResponse {
            message: Some(actor_message_to_proto(linked.message)),
            task_id: linked.task_id,
            relation: linked.relation.as_str().to_string(),
            linked_at: linked.linked_at,
            created: linked.created,
        }))
    }

    async fn describe_team_context(
        &self,
        request: Request<DescribeTeamContextRequest>,
    ) -> Result<Response<DescribeTeamContextResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamRead)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;

        let team_id = optional_trimmed(&payload.team_id);
        let mut run_id = optional_trimmed(&payload.run_id);
        if let Some(principal_run_id) = principal.run_id.as_deref() {
            if let Some(request_run_id) = run_id {
                self.authz.ensure_run_scope(&principal, request_run_id)?;
            } else {
                run_id = Some(principal_run_id);
            }
        } else if let Some(request_run_id) = run_id {
            self.authz.ensure_run_scope(&principal, request_run_id)?;
        }

        let context = load_team_context_for_actor(
            &self.deps.agents,
            &self.deps.teams,
            team_id,
            run_id,
            actor_id,
        )
        .await?;
        Ok(Response::new(DescribeTeamContextResponse {
            context_json: serde_json::to_string(&context).map_err(map_serde_status)?,
        }))
    }

    async fn resolve_actor_run_scope(
        &self,
        request: Request<ResolveActorRunScopeRequest>,
    ) -> Result<Response<ResolveActorRunScopeResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamRead)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;

        let resolved = resolve_actor_run_scope(
            &self.deps.agents,
            &self.deps.teams,
            actor_id,
            optional_trimmed(&payload.team_id),
        )
        .await?;
        Ok(Response::new(ResolveActorRunScopeResponse {
            run_id: resolved.run_id,
            team_id: resolved.team_id.unwrap_or_default(),
            source: resolved.source.to_string(),
        }))
    }

    async fn list_team_tasks(
        &self,
        request: Request<ListTeamTasksRequest>,
    ) -> Result<Response<ListTeamTasksResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamRead)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let context = load_team_context_for_actor(
            &self.deps.agents,
            &self.deps.teams,
            optional_trimmed(&payload.team_id),
            optional_trimmed(&payload.run_id),
            actor_id,
        )
        .await?;
        let query = TeamTaskListQuery {
            team_id: Some(context.team_id),
            run_id: None,
            limit: payload.limit.clamp(1, MAX_INTERNAL_TEAM_TASK_LIST_LIMIT),
            status: optional_trimmed(&payload.status)
                .map(parse_team_task_status)
                .transpose()?,
            priority: optional_trimmed(&payload.priority)
                .map(parse_team_task_priority)
                .transpose()?,
            task_id: optional_trimmed(&payload.task_id).map(str::to_string),
            assigned_member_id: optional_trimmed(&payload.assigned_member_id).map(str::to_string),
            topic: optional_trimmed(&payload.topic).map(str::to_string),
            include_shared_thread: payload.include_shared_thread,
        };
        let tasks = self
            .deps
            .teams
            .list_tasks_with_query(query)
            .await
            .map_err(map_manager_error)?;

        Ok(Response::new(ListTeamTasksResponse {
            tasks_json: serde_json::to_string(&tasks).map_err(map_serde_status)?,
        }))
    }

    async fn create_team_task(
        &self,
        request: Request<CreateTeamTaskRequest>,
    ) -> Result<Response<CreateTeamTaskResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamTaskWrite)?;
        let payload = request.into_inner();

        let team_id = required_field(&payload.team_id, "team_id")?;
        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let _team = ensure_coordinator_team_access(&self.deps.teams, team_id, actor_id).await?;

        let title = required_field(&payload.title, "title")?;
        let status = parse_team_task_status(required_field(&payload.status, "status")?)?;
        let priority = parse_team_task_priority(required_field(&payload.priority, "priority")?)?;
        let assigned_member_id = required_field(&payload.assigned_member_id, "assigned_member_id")?;
        let topic = optional_trimmed(&payload.topic);
        let context = parse_json_required(&payload.context_json, "context_json")?;

        let (task, conversation) = self
            .deps
            .teams
            .create_task_with_metadata(TeamTaskCreateInput {
                team_id,
                title,
                created_by_actor_id: actor_id,
                priority,
                assigned_member_id: Some(assigned_member_id),
                context,
                conversation_mode: "group_chat",
                topic,
            })
            .await
            .map_err(map_manager_error)?;
        let task = if status == TeamTaskStatus::Open {
            task
        } else {
            self.deps
                .teams
                .update_task_status(&task.id, status)
                .await
                .map_err(map_manager_error)?
        };
        Ok(Response::new(CreateTeamTaskResponse {
            output_json: serde_json::to_string(&serde_json::json!({
                "task": task,
                "conversation": conversation,
            }))
            .map_err(map_serde_status)?,
        }))
    }

    async fn update_team_task(
        &self,
        request: Request<UpdateTeamTaskRequest>,
    ) -> Result<Response<UpdateTeamTaskResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamTaskWrite)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let context = load_team_context_for_actor(
            &self.deps.agents,
            &self.deps.teams,
            optional_trimmed(&payload.team_id),
            None,
            actor_id,
        )
        .await?;
        let team =
            ensure_coordinator_team_access(&self.deps.teams, &context.team_id, actor_id).await?;

        let task_id = required_field(&payload.task_id, "task_id")?;
        let existing = self
            .deps
            .teams
            .get_task(task_id)
            .await
            .map_err(map_manager_error)?;
        if existing.team_id != context.team_id {
            return Err(Status::permission_denied(
                "task does not belong to this team",
            ));
        }
        let status = payload
            .status
            .as_deref()
            .map(parse_team_task_status)
            .transpose()?;
        let priority = payload
            .priority
            .as_deref()
            .map(parse_team_task_priority)
            .transpose()?;
        let assignment = normalize_task_assignment_update(
            &team.spec,
            payload.assigned_member_id.as_deref(),
            payload.clear_assigned_member_id,
        )?;
        let context_json = payload
            .context_json
            .as_deref()
            .map(|raw| parse_json_required(raw, "context_json"))
            .transpose()?;
        let context_merge_json = payload
            .context_merge_json
            .as_deref()
            .map(|raw| parse_json_required(raw, "context_merge_json"))
            .transpose()?;
        if context_json.is_some() && context_merge_json.is_some() {
            return Err(Status::invalid_argument(
                "context_json and context_merge_json cannot be used together",
            ));
        }
        let context_patch = match (context_json, context_merge_json) {
            (Some(context_json), None) => Some(TeamTaskContextPatch::Replace(context_json)),
            (None, Some(context_merge_json)) => {
                if !context_merge_json.is_object() {
                    return Err(Status::invalid_argument(
                        "context_merge_json must be a JSON object",
                    ));
                }
                Some(TeamTaskContextPatch::Merge(context_merge_json))
            }
            (None, None) => None,
            (Some(_), Some(_)) => unreachable!("validated exclusive context patch"),
        };
        let note_kind = payload
            .note_kind
            .as_deref()
            .map(parse_team_task_note_kind)
            .transpose()?;
        let note_text = payload
            .note_text
            .as_deref()
            .and_then(optional_trimmed)
            .map(str::to_string);
        if note_kind.is_some() ^ note_text.is_some() {
            return Err(Status::invalid_argument(
                "note_kind and note_text must be provided together",
            ));
        }
        let status_transition_requested = status
            .as_ref()
            .is_some_and(|candidate| *candidate != existing.status);
        if status_transition_requested && note_kind.is_none() {
            return Err(Status::invalid_argument(
                "task status changes require note_kind and note_text",
            ));
        }
        if status.is_none()
            && priority.is_none()
            && matches!(assignment, TeamTaskAssignmentUpdate::Unchanged)
            && context_patch.is_none()
            && note_kind.is_none()
        {
            return Err(Status::invalid_argument(
                "task update requires status, priority, assigned_member_id, clear_assigned_member_id, context_json, context_merge_json, or a note",
            ));
        }
        let note = note_kind
            .zip(note_text)
            .map(|(kind, text)| TeamTaskNoteCreateInput {
                from_actor_id: actor_id,
                to_actor_id: None,
                route: "task_note",
                payload: serde_json::json!({
                    "type": "task_note",
                    "kind": kind.as_str(),
                    "text": text,
                }),
                idempotency_key: None,
            });
        let task = self
            .deps
            .teams
            .update_task_with_note(TeamTaskUpdateWithNoteInput {
                task_id,
                status,
                assignment,
                priority,
                context_patch,
                note,
            })
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(UpdateTeamTaskResponse {
            task_json: serde_json::to_string(&task).map_err(map_serde_status)?,
        }))
    }

    async fn get_team_task(
        &self,
        request: Request<GetTeamTaskRequest>,
    ) -> Result<Response<GetTeamTaskResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamRead)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let context = load_team_context_for_actor(
            &self.deps.agents,
            &self.deps.teams,
            optional_trimmed(&payload.team_id),
            optional_trimmed(&payload.run_id),
            actor_id,
        )
        .await?;
        let task_id = required_field(&payload.task_id, "task_id")?;
        let detail = self
            .deps
            .teams
            .get_task_detail(
                task_id,
                payload
                    .message_limit
                    .clamp(1, TEAM_TASK_DETAIL_MESSAGE_LIMIT_MAX),
            )
            .await
            .map_err(map_manager_error)?;
        if detail.task.team_id != context.team_id {
            return Err(Status::permission_denied(
                "task does not belong to this team",
            ));
        }
        Ok(Response::new(GetTeamTaskResponse {
            detail_json: serde_json::to_string(&detail).map_err(map_serde_status)?,
        }))
    }

    async fn create_team_channel(
        &self,
        request: Request<CreateTeamChannelRequest>,
    ) -> Result<Response<CreateTeamChannelResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamTaskWrite)?;
        let payload = request.into_inner();

        let team_id = required_field(&payload.team_id, "team_id")?;
        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let _team = ensure_coordinator_team_access(&self.deps.teams, team_id, actor_id).await?;
        let channel_id = required_field(&payload.channel_id, "channel_id")?;
        let channel = self
            .deps
            .teams
            .create_channel(
                team_id,
                channel_id,
                optional_trimmed(&payload.description),
                actor_id,
            )
            .await
            .map_err(map_manager_error)?;

        Ok(Response::new(CreateTeamChannelResponse {
            channel_json: serde_json::to_string(&channel).map_err(map_serde_status)?,
        }))
    }

    async fn delete_team_channel(
        &self,
        request: Request<DeleteTeamChannelRequest>,
    ) -> Result<Response<DeleteTeamChannelResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamTaskWrite)?;
        let payload = request.into_inner();

        let team_id = required_field(&payload.team_id, "team_id")?;
        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let _team = ensure_coordinator_team_access(&self.deps.teams, team_id, actor_id).await?;
        let channel_id = required_field(&payload.channel_id, "channel_id")?;
        let channel = self
            .deps
            .teams
            .delete_channel(team_id, channel_id)
            .await
            .map_err(map_manager_error)?;

        Ok(Response::new(DeleteTeamChannelResponse {
            channel_json: serde_json::to_string(&channel).map_err(map_serde_status)?,
        }))
    }

    async fn open_team_thread(
        &self,
        request: Request<OpenTeamThreadRequest>,
    ) -> Result<Response<OpenTeamThreadResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamRead)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let team_context = load_team_context_for_actor(
            &self.deps.agents,
            &self.deps.teams,
            optional_trimmed(&payload.team_id),
            optional_trimmed(&payload.run_id),
            actor_id,
        )
        .await?;
        let channel_id = optional_trimmed(&payload.channel_id).unwrap_or("all");
        if payload.root_message_id <= 0 {
            return Err(Status::invalid_argument("root_message_id must be positive"));
        }
        let thread = self
            .deps
            .teams
            .open_thread(&team_context.team_id, channel_id, payload.root_message_id)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(OpenTeamThreadResponse {
            thread_json: serde_json::to_string(&thread).map_err(map_serde_status)?,
        }))
    }

    async fn reply_team_thread(
        &self,
        request: Request<ReplyTeamThreadRequest>,
    ) -> Result<Response<ReplyTeamThreadResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamTaskWrite)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let team_context = load_team_context_for_actor(
            &self.deps.agents,
            &self.deps.teams,
            optional_trimmed(&payload.team_id),
            optional_trimmed(&payload.run_id),
            actor_id,
        )
        .await?;
        let channel_id = optional_trimmed(&payload.channel_id).unwrap_or("all");
        if payload.root_message_id <= 0 {
            return Err(Status::invalid_argument("root_message_id must be positive"));
        }
        let text = required_field(&payload.text, "text")?;
        let reply = self
            .deps
            .teams
            .reply_thread(
                &team_context.team_id,
                channel_id,
                payload.root_message_id,
                actor_id,
                text,
                &[],
            )
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(ReplyTeamThreadResponse {
            message_json: serde_json::to_string(&reply).map_err(map_serde_status)?,
        }))
    }

    async fn append_team_task_note(
        &self,
        request: Request<AppendTeamTaskNoteRequest>,
    ) -> Result<Response<AppendTeamTaskNoteResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TeamTaskWrite)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let context = load_team_context_for_actor(
            &self.deps.agents,
            &self.deps.teams,
            optional_trimmed(&payload.team_id),
            optional_trimmed(&payload.run_id),
            actor_id,
        )
        .await?;
        let task_id = required_field(&payload.task_id, "task_id")?;
        let task = self
            .deps
            .teams
            .get_task(task_id)
            .await
            .map_err(map_manager_error)?;
        if task.team_id != context.team_id {
            return Err(Status::permission_denied(
                "task does not belong to this team",
            ));
        }
        let kind = parse_team_task_note_kind(required_field(&payload.kind, "kind")?)?;
        let text = required_field(&payload.text, "text")?;
        let message = self
            .deps
            .teams
            .append_task_conversation_message(
                task_id,
                actor_id,
                None,
                "task_note",
                serde_json::json!({
                    "type": "task_note",
                    "kind": kind.as_str(),
                    "text": text,
                }),
            )
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(AppendTeamTaskNoteResponse {
            message_json: serde_json::to_string(&message).map_err(map_serde_status)?,
        }))
    }

    async fn create_time_trigger(
        &self,
        request: Request<CreateTimeTriggerRequest>,
    ) -> Result<Response<CreateTimeTriggerResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TimeTriggerManage)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        if payload.fire_at <= chrono::Utc::now().timestamp() {
            return Err(Status::invalid_argument("fire_at must be in the future"));
        }
        self.deps
            .agents
            .get_agent(actor_id)
            .await
            .map_err(map_agent_lookup_error)?;
        let manager = AgentTimeTriggerManager::new(self.deps.db.clone());
        let trigger = manager
            .create_time_trigger(AgentTimeTriggerCreateInput {
                agent_id: actor_id.to_string(),
                created_by_actor_id: actor_id.to_string(),
                message_text: required_field(&payload.message_text, "message_text")?.to_string(),
                fire_at: payload.fire_at,
            })
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(CreateTimeTriggerResponse {
            trigger_json: serde_json::to_string(&trigger).map_err(map_serde_status)?,
        }))
    }

    async fn list_time_triggers(
        &self,
        request: Request<ListTimeTriggersRequest>,
    ) -> Result<Response<ListTimeTriggersResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TimeTriggerManage)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let manager = AgentTimeTriggerManager::new(self.deps.db.clone());
        let triggers = manager
            .list_triggers_for_agent(actor_id, payload.limit)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(ListTimeTriggersResponse {
            triggers_json: serde_json::to_string(&triggers).map_err(map_serde_status)?,
        }))
    }

    async fn cancel_time_trigger(
        &self,
        request: Request<CancelTimeTriggerRequest>,
    ) -> Result<Response<CancelTimeTriggerResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::TimeTriggerManage)?;
        let payload = request.into_inner();

        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;
        let trigger_id = required_field(&payload.trigger_id, "trigger_id")?;
        let manager = AgentTimeTriggerManager::new(self.deps.db.clone());
        let canceled = manager
            .cancel_trigger(actor_id, trigger_id)
            .await
            .map_err(map_manager_error)?;
        if !canceled {
            return Err(Status::not_found("time trigger not found"));
        }
        Ok(Response::new(CancelTimeTriggerResponse {
            output_json: serde_json::to_string(&serde_json::json!({
                "status": "ok",
                "trigger_id": trigger_id,
            }))
            .map_err(map_serde_status)?,
        }))
    }

    async fn respond_permission_review(
        &self,
        request: Request<RespondPermissionReviewRequest>,
    ) -> Result<Response<RespondPermissionReviewResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::PermissionReview)?;
        let payload = request.into_inner();

        let team_id = required_field(&payload.team_id, "team_id")?;
        let actor_id = required_field(&payload.actor_id, "actor_id")?;
        self.authz
            .ensure_worker_actor(&principal, actor_id, "actor_id")?;

        let permission_id = required_field(&payload.permission_id, "permission_id")?;
        let Some(record) = self
            .deps
            .acp_permissions
            .get(permission_id)
            .await
            .map_err(map_manager_error)?
        else {
            return Err(Status::not_found("permission request not found"));
        };
        if record.team_id.as_deref() != Some(team_id) {
            return Err(Status::permission_denied(
                "permission request does not belong to this team",
            ));
        }
        if !self
            .deps
            .teams
            .team_has_member(team_id, actor_id)
            .await
            .map_err(map_manager_error)?
        {
            return Err(Status::permission_denied(
                "current actor is not a member of this team",
            ));
        }
        if record.status != "pending" {
            return Ok(Response::new(RespondPermissionReviewResponse {
                status: "already_resolved".to_string(),
                permission_id: permission_id.to_string(),
                request_status: record.status,
                reviewed_by_actor_id: record.reviewed_by_actor_id.unwrap_or_default(),
            }));
        }
        if record.requester_actor_id.as_deref() == Some(actor_id) {
            return Err(Status::permission_denied(
                "requester cannot review its own permission request",
            ));
        }
        let team = self
            .deps
            .teams
            .get_team(team_id)
            .await
            .map_err(map_manager_error)?;
        let active_reviewer =
            if let Some(review_target_actor_id) = record.review_target_actor_id.as_deref() {
                Some(review_target_actor_id.to_string())
            } else {
                let requester_actor_id = record.requester_actor_id.as_deref().ok_or_else(|| {
                    Status::failed_precondition("permission request is missing requester actor")
                })?;
                Some(
                    resolve_team_permission_review_target(
                        &team.spec,
                        requester_actor_id,
                        record.requester_role.as_deref().unwrap_or_default(),
                    )
                    .map(|(reviewer, _)| reviewer)
                    .map_err(|err| {
                        Status::failed_precondition(format!(
                            "failed to resolve active reviewer for permission request: {err}"
                        ))
                    })?,
                )
            };
        if active_reviewer.as_deref() != Some(actor_id) {
            return Err(Status::permission_denied(
                "current actor is not the active reviewer for this permission request",
            ));
        }

        let option_id = optional_trimmed(payload.option_id.as_str()).map(str::to_string);
        let requested_outcome = optional_trimmed(payload.outcome.as_str());
        if option_id.is_some() && requested_outcome.is_some() {
            return Err(Status::invalid_argument(
                "option_id and outcome cannot be set together",
            ));
        }
        let outcome = if let Some(selected_option_id) = option_id.as_ref() {
            agent_client_protocol::schema::RequestPermissionOutcome::Selected(
                agent_client_protocol::schema::SelectedPermissionOutcome::new(
                    selected_option_id.clone(),
                ),
            )
        } else {
            match requested_outcome {
                Some("cancelled") | None => {
                    agent_client_protocol::schema::RequestPermissionOutcome::Cancelled
                }
                Some(other) => {
                    return Err(Status::invalid_argument(format!(
                        "unsupported outcome '{other}', expected 'cancelled'"
                    )));
                }
            }
        };

        let respond_result = self
            .deps
            .acp_permissions
            .respond(
                permission_id,
                outcome,
                option_id,
                Some(actor_id.to_string()),
            )
            .await
            .map_err(map_manager_error)?;
        let permission = self
            .deps
            .acp_permissions
            .get(permission_id)
            .await
            .map_err(map_manager_error)?;
        let (status, request_status, reviewed_by_actor_id) = match respond_result {
            AcpPermissionRespondResult::Applied => (
                "ok".to_string(),
                permission
                    .as_ref()
                    .map(|current| current.status.clone())
                    .unwrap_or_else(|| "resolved".to_string()),
                actor_id.to_string(),
            ),
            AcpPermissionRespondResult::AlreadyResolved => {
                let (request_status, reviewed_by_actor_id) = if let Some(current) = permission {
                    (
                        current.status,
                        current.reviewed_by_actor_id.unwrap_or_default(),
                    )
                } else {
                    ("resolved".to_string(), String::new())
                };
                (
                    "already_resolved".to_string(),
                    request_status,
                    reviewed_by_actor_id,
                )
            }
        };
        Ok(Response::new(RespondPermissionReviewResponse {
            status,
            permission_id: permission_id.to_string(),
            request_status,
            reviewed_by_actor_id,
        }))
    }

    async fn transition_step(
        &self,
        request: Request<TransitionStepRequest>,
    ) -> Result<Response<TransitionStepResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::StepTransition)?;
        let payload = request.into_inner();

        let run_id = required_field(&payload.run_id, "run_id")?;
        self.authz.ensure_run_scope(&principal, run_id)?;

        let step_id = required_field(&payload.step_id, "step_id")?;
        let action = required_field(&payload.action, "action")?;
        let current = self
            .deps
            .teams
            .get_step(step_id)
            .await
            .map_err(map_manager_error)?;
        if current.run_id != run_id {
            return Err(Status::permission_denied(
                "step does not belong to requested run scope",
            ));
        }
        if principal.role == InternalRole::Worker {
            self.authz
                .ensure_worker_actor(&principal, &current.member_id, "step member")?;
        }

        let step = match action {
            "start" => {
                self.deps
                    .teams
                    .start_step(step_id, optional_trimmed(&payload.remote_task_id))
                    .await
            }
            "complete" => {
                self.deps
                    .teams
                    .complete_step(
                        step_id,
                        optional_trimmed(&payload.output_json)
                            .map(serde_json::from_str::<Value>)
                            .transpose()
                            .map_err(|err| Status::invalid_argument(err.to_string()))?,
                    )
                    .await
            }
            "fail" => {
                let err_text = required_field(&payload.error_text, "error_text")?;
                self.deps.teams.fail_step(step_id, err_text).await
            }
            "input_required" => {
                self.deps
                    .teams
                    .set_step_input_required(
                        step_id,
                        optional_trimmed(&payload.reason),
                        optional_trimmed(&payload.input_json)
                            .map(serde_json::from_str::<Value>)
                            .transpose()
                            .map_err(|err| Status::invalid_argument(err.to_string()))?,
                    )
                    .await
            }
            "resume" => {
                self.deps
                    .teams
                    .resume_step(
                        step_id,
                        optional_trimmed(&payload.input_json)
                            .map(serde_json::from_str::<Value>)
                            .transpose()
                            .map_err(|err| Status::invalid_argument(err.to_string()))?,
                    )
                    .await
            }
            "continue" => {
                self.deps
                    .teams
                    .continue_step(
                        step_id,
                        optional_trimmed(&payload.output_json)
                            .map(serde_json::from_str::<Value>)
                            .transpose()
                            .map_err(|err| Status::invalid_argument(err.to_string()))?,
                    )
                    .await
            }
            _ => return Err(Status::invalid_argument("unsupported action")),
        }
        .map_err(map_manager_error)?;
        if matches!(action, "start" | "resume" | "continue")
            && let Err(err) = crate::team::maybe_nudge_reconcile_step_prompt(
                &self.deps.teams,
                &self.deps.agents,
                &step,
            )
            .await
        {
            tracing::warn!(
                run_id = %run_id,
                step_id = %step.id,
                member_id = %step.member_id,
                "internal transition failed to auto-nudge reconcile step prompt: {}",
                err
            );
        }
        Ok(Response::new(step_to_transition_response(step)))
    }

    async fn issue_node_credential(
        &self,
        request: Request<IssueNodeCredentialRequest>,
    ) -> Result<Response<IssueNodeCredentialResponse>, Status> {
        self.ensure_bootstrap_token(request.metadata())?;
        let payload = request.into_inner();

        let node_id = required_field(&payload.node_id, "node_id")?;
        let role_raw = required_field(&payload.role, "role")?;
        let role = InternalRole::parse(role_raw).ok_or_else(|| {
            Status::invalid_argument("unsupported role, expected coordinator/worker")
        })?;
        if role == InternalRole::Orchestrator {
            return Err(Status::invalid_argument(
                "role 'orchestrator' is reserved and cannot be issued via bootstrap",
            ));
        }

        let actor_id = optional_trimmed(&payload.actor_id);
        let run_id = optional_trimmed(&payload.run_id);
        if role == InternalRole::Worker {
            if actor_id.is_none() {
                return Err(Status::invalid_argument(
                    "worker bootstrap requires actor_id",
                ));
            }
            if run_id.is_none() {
                return Err(Status::invalid_argument("worker bootstrap requires run_id"));
            }
        }

        let requested_permissions = payload
            .permissions
            .into_iter()
            .filter_map(|value| normalize_permission(&value))
            .collect::<Vec<_>>();
        let mut permissions = if requested_permissions.is_empty() {
            default_permissions_for_role(role)
        } else {
            requested_permissions
        };
        permissions.sort();
        permissions.dedup();
        validate_role_permissions(role, &permissions)?;

        let ttl_seconds = if payload.ttl_seconds > 0 {
            payload.ttl_seconds.clamp(60, MAX_TOKEN_TTL_SECONDS)
        } else {
            DEFAULT_TOKEN_TTL_SECONDS
        };
        let issued = self
            .authz
            .issue_node_access_token(NodeCredentialRequest {
                source_node_id: node_id.to_string(),
                role: role.as_str().to_string(),
                actor_id: actor_id.map(str::to_string),
                run_id: run_id.map(str::to_string),
                permissions,
                scope: Vec::new(),
                audience: Vec::new(),
                ttl_seconds,
            })
            .map_err(|err| Status::internal(err.to_string()))?;

        let (cert_pem, key_pem, ca_cert_pem) =
            if self.security_mode == InternalGrpcSecurityMode::Disabled {
                (String::new(), String::new(), String::new())
            } else {
                let identity = load_bootstrap_client_identity(&self.cert_dir)
                    .map_err(|err| Status::internal(err.to_string()))?;
                (
                    String::from_utf8_lossy(&identity.cert_pem).to_string(),
                    String::from_utf8_lossy(&identity.key_pem).to_string(),
                    String::from_utf8_lossy(&identity.ca_cert_pem).to_string(),
                )
            };

        self.deps
            .agents
            .touch_agent_node_last_seen(node_id)
            .await
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(IssueNodeCredentialResponse {
            node_id: node_id.to_string(),
            role: role.as_str().to_string(),
            access_token: issued.access_token,
            expires_at: issued.expires_at,
            cert_pem,
            key_pem,
            ca_cert_pem,
            security_mode: security_mode_to_str(self.security_mode).to_string(),
            cluster_id: issued.cluster_id,
            scope: issued.scope,
            audience: issued.audience,
            kid: issued.kid,
            issued_at: issued.issued_at,
            source_node_id: issued.source_node_id,
        }))
    }

    async fn ensure_agent_record(
        &self,
        request: Request<EnsureAgentRecordRequest>,
    ) -> Result<Response<EnsureAgentRecordResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let config = parse_json_as::<AgentConfig>(&payload.config_json, "config_json")?;
        let source = optional_trimmed(&payload.source).unwrap_or("manual");
        let agent = self
            .deps
            .agents
            .ensure_remote_managed_agent(agent_id, config, source)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(EnsureAgentRecordResponse {
            agent_json: serde_json::to_string(&agent)
                .map_err(|err| Status::internal(err.to_string()))?,
        }))
    }

    async fn get_agent_record(
        &self,
        request: Request<GetAgentRecordRequest>,
    ) -> Result<Response<GetAgentRecordResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let agent = self
            .deps
            .agents
            .get_agent(agent_id)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(GetAgentRecordResponse {
            agent_json: serde_json::to_string(&agent)
                .map_err(|err| Status::internal(err.to_string()))?,
        }))
    }

    async fn start_managed_agent(
        &self,
        request: Request<StartManagedAgentRequest>,
    ) -> Result<Response<StartManagedAgentResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let actor_context = optional_trimmed(&payload.actor_context_json)
            .map(|raw| parse_json_str_as::<AcpActorSkillContext>(raw, "actor_context_json"))
            .transpose()?;
        let session_id = self
            .deps
            .agents
            .start_agent_with_actor_context(agent_id, actor_context)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(StartManagedAgentResponse { session_id }))
    }

    async fn stop_managed_agent(
        &self,
        request: Request<StopManagedAgentRequest>,
    ) -> Result<Response<StopManagedAgentResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        self.deps
            .agents
            .stop_agent(agent_id)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(StopManagedAgentResponse {}))
    }

    async fn delete_managed_agent(
        &self,
        request: Request<DeleteManagedAgentRequest>,
    ) -> Result<Response<DeleteManagedAgentResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let _ = self.deps.agents.stop_agent(agent_id).await;
        self.deps
            .agents
            .delete_agent(agent_id)
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(DeleteManagedAgentResponse {}))
    }

    async fn send_agent_input(
        &self,
        request: Request<SendAgentInputRequest>,
    ) -> Result<Response<SendAgentInputResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let input = required_field(&payload.input, "input")?;
        self.deps
            .agents
            .send_input(
                agent_id,
                input,
                optional_trimmed(&payload.message_id),
                optional_trimmed(&payload.session_id),
            )
            .await
            .map_err(map_manager_error)?;
        Ok(Response::new(SendAgentInputResponse {}))
    }

    async fn list_agent_events(
        &self,
        request: Request<ListAgentEventsRequest>,
    ) -> Result<Response<ListAgentEventsResponse>, Status> {
        let principal = self.authz.authenticate(request.metadata())?;
        self.authz
            .ensure_permission(&principal, InternalAction::AgentManage)?;
        let payload = request.into_inner();

        let agent_id = required_field(&payload.agent_id, "agent_id")?;
        let limit = payload.limit.clamp(1, 1000);
        let before_event_id = (payload.before_event_id > 0).then_some(payload.before_event_id);
        let session_id = optional_trimmed(&payload.session_id);
        let events = if let Some(session_id) = session_id {
            self.deps
                .agents
                .list_events_for_session(agent_id, session_id, limit, before_event_id)
                .await
        } else {
            self.deps
                .agents
                .list_events(agent_id, limit, before_event_id)
                .await
        }
        .map_err(map_manager_error)?;
        Ok(Response::new(ListAgentEventsResponse {
            events: events.into_iter().map(agent_event_record).collect(),
        }))
    }
}
