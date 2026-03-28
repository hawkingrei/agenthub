use super::*;

pub(super) fn step_to_transition_response(step: TeamStepRecord) -> TransitionStepResponse {
    TransitionStepResponse {
        step_id: step.id,
        run_id: step.run_id,
        step_key: step.step_key,
        member_id: step.member_id,
        remote_task_id: step.runtime_handle_id.unwrap_or_default(),
        status: step_status_to_str(&step.status).to_string(),
        error_text: step.error_text.unwrap_or_default(),
    }
}

pub(super) fn step_status_to_str(status: &TeamStepStatus) -> &'static str {
    match status {
        TeamStepStatus::Submitted => "submitted",
        TeamStepStatus::Working => "working",
        TeamStepStatus::InputRequired => "input_required",
        TeamStepStatus::Completed => "completed",
        TeamStepStatus::Failed => "failed",
        TeamStepStatus::Canceled => "canceled",
    }
}

pub(super) fn required_field<'a>(raw: &'a str, field: &str) -> Result<&'a str, Status> {
    optional_trimmed(raw).ok_or_else(|| Status::invalid_argument(format!("{field} is required")))
}

pub(super) fn optional_trimmed(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub(super) fn parse_json_required(raw: &str, field: &str) -> Result<Value, Status> {
    let raw = required_field(raw, field)?;
    serde_json::from_str(raw)
        .map_err(|err| Status::invalid_argument(format!("{field} must be valid JSON: {err}")))
}

pub(super) fn resolve_channel_replica_request(payload: &Value) -> Option<ChannelReplicaRequest> {
    let payload_obj = payload.as_object()?;
    let delivery_scope = payload_obj.get("delivery_scope")?.as_str()?.trim();
    if delivery_scope != "channel_broadcast" {
        return None;
    }
    let authority_message_id = payload_obj.get("authority_message_id")?.as_i64()?;
    if authority_message_id <= 0 {
        return None;
    }
    let team_id = payload_obj.get("team_id")?.as_str()?.trim();
    let conversation_id = payload_obj.get("channel_conversation_id")?.as_str()?.trim();
    let task_id = payload_obj.get("task_id")?.as_str()?.trim();
    let channel_id = payload_obj.get("channel_id")?.as_str()?.trim();
    if team_id.is_empty()
        || conversation_id.is_empty()
        || task_id.is_empty()
        || channel_id.is_empty()
    {
        return None;
    }
    Some(ChannelReplicaRequest {
        authority_message_id,
        team_id: team_id.to_string(),
        conversation_id: conversation_id.to_string(),
        task_id: task_id.to_string(),
        channel_id: channel_id.to_string(),
    })
}

pub(super) async fn maybe_notify_actor_new_mailbox_message_type(
    state: &AppState,
    run_id: &str,
    send_result: &agenthub_team_actor::ActorSendResponse,
) -> anyhow::Result<()> {
    let Some(plan) = plan_actor_mailbox_immediate_hint(&state.teams, run_id, send_result).await?
    else {
        return Ok(());
    };
    let prompt = build_actor_mailbox_immediate_hint_prompt(run_id, plan.reason);
    let reason_label = match plan.reason {
        crate::team::ActorMailboxImmediateHintReason::DirectAgentMessage => "direct_agent_message",
        crate::team::ActorMailboxImmediateHintReason::LeaderChannelMention => {
            "leader_channel_mention"
        }
    };
    let mut sent_targets = Vec::new();
    let mut failed_targets = Vec::new();
    for target_actor_id in &plan.target_actor_ids {
        match state
            .agents
            .send_input(target_actor_id, &prompt, None, None)
            .await
        {
            Ok(()) => sent_targets.push(target_actor_id.clone()),
            Err(err) => {
                tracing::debug!(
                    run_id = %run_id,
                    actor_id = %target_actor_id,
                    reason = ?plan.reason,
                    "skip mailbox hint push because agent input is unavailable: {}",
                    err
                );
                failed_targets.push(target_actor_id.clone());
            }
        }
    }
    if let Err(err) = state
        .teams
        .append_run_event(
            run_id,
            "actor_mailbox_type_hint",
            serde_json::json!({
                "status": if failed_targets.is_empty() { "sent" } else if sent_targets.is_empty() { "send_failed" } else { "partial" },
                "message_id": send_result.message_id,
                "reason": reason_label,
                "target_actor_ids": plan.target_actor_ids,
                "sent_actor_ids": sent_targets,
                "failed_actor_ids": failed_targets,
            }),
        )
        .await
    {
        tracing::warn!(
            run_id = %run_id,
            "failed to append actor_mailbox_type_hint event: {}",
            err
        );
    }
    Ok(())
}

pub(super) async fn load_team_context_for_actor(
    agents: &crate::agent::AgentManager,
    manager: &TeamManager,
    team_id: Option<&str>,
    run_id: Option<&str>,
    actor_id: &str,
) -> Result<crate::team::TeamContextRecord, Status> {
    reconcile_team_runtime_presence(agents, manager, team_id, run_id).await?;
    let context = manager
        .describe_team_context(team_id, run_id)
        .await
        .map_err(map_team_context_error)?;
    ensure_team_member_access(manager, &context.team_id, actor_id).await?;
    Ok(context)
}

async fn reconcile_team_runtime_presence(
    agents: &crate::agent::AgentManager,
    manager: &TeamManager,
    team_id: Option<&str>,
    run_id: Option<&str>,
) -> Result<(), Status> {
    let reconcile_team_id = if let Some(run_id) = run_id {
        Some(
            manager
                .get_run(run_id)
                .await
                .map_err(map_team_context_error)?
                .team_id,
        )
    } else {
        team_id.map(str::to_string)
    };
    let Some(team_id) = reconcile_team_id else {
        return Ok(());
    };
    let team = manager
        .get_team(&team_id)
        .await
        .map_err(map_team_context_error)?;
    let Some(members) = team.spec.get("members").and_then(Value::as_array) else {
        return Ok(());
    };
    for member in members {
        let Some(member_id) = member
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        agents
            .reconcile_runtime_absence(member_id)
            .await
            .map_err(map_manager_error)?;
    }
    Ok(())
}

pub(super) async fn ensure_team_member_access(
    manager: &TeamManager,
    team_id: &str,
    actor_id: &str,
) -> Result<(), Status> {
    if manager
        .team_has_member(team_id, actor_id)
        .await
        .map_err(map_manager_error)?
    {
        return Ok(());
    }
    Err(Status::permission_denied(
        "current actor is not a member of this team",
    ))
}

pub(super) async fn ensure_leader_team_access(
    manager: &TeamManager,
    team_id: &str,
    actor_id: &str,
) -> Result<crate::team::TeamDefinitionRecord, Status> {
    ensure_team_member_access(manager, team_id, actor_id).await?;
    let team = manager.get_team(team_id).await.map_err(map_manager_error)?;
    let leader_member_id = resolve_team_leader_member_id(&team.spec)?;
    if actor_id == leader_member_id {
        return Ok(team);
    }
    Err(Status::permission_denied(
        "only leader may create or update Team tasks",
    ))
}

pub(super) fn parse_team_task_status(raw: &str) -> Result<TeamTaskStatus, Status> {
    let trimmed = raw.trim();
    trimmed.parse::<TeamTaskStatus>().map_err(|_| {
        Status::invalid_argument(format!(
            "invalid task status '{trimmed}', expected one of: open, in_progress, in_review, completed, canceled"
        ))
    })
}

pub(super) fn parse_team_task_note_kind(raw: &str) -> Result<&'static str, Status> {
    match raw.trim() {
        "comment" => Ok("comment"),
        "decision" => Ok("decision"),
        "result" => Ok("result"),
        other => Err(Status::invalid_argument(format!(
            "invalid task note kind '{other}', expected one of: comment, decision, result"
        ))),
    }
}

pub(super) fn normalize_task_assignment_update(
    team_spec: &Value,
    assigned_member_id: Option<&str>,
    clear_assigned_member_id: bool,
) -> Result<TeamTaskAssignmentUpdate, Status> {
    if assigned_member_id.is_some() && clear_assigned_member_id {
        return Err(Status::invalid_argument(
            "assigned_member_id and clear_assigned_member_id cannot be combined",
        ));
    }
    if clear_assigned_member_id {
        return Ok(TeamTaskAssignmentUpdate::Unassigned);
    }
    let Some(assigned_member_id) = assigned_member_id else {
        return Ok(TeamTaskAssignmentUpdate::Unchanged);
    };
    let member_id = assigned_member_id.trim();
    if member_id.is_empty() {
        return Ok(TeamTaskAssignmentUpdate::Unassigned);
    }
    let team_member_ids = collect_team_member_ids(team_spec)?;
    if !team_member_ids
        .iter()
        .any(|candidate| candidate == member_id)
    {
        return Err(Status::invalid_argument(
            "assigned_member_id must reference spec.members[].member_id",
        ));
    }
    Ok(TeamTaskAssignmentUpdate::Assigned(member_id.to_string()))
}

pub(super) fn map_team_context_error(err: anyhow::Error) -> Status {
    if let Some(cause) = err.downcast_ref::<TeamContextLookupError>() {
        return match cause {
            TeamContextLookupError::MissingSelector
            | TeamContextLookupError::RunTeamMismatch { .. } => {
                Status::invalid_argument(cause.to_string())
            }
        };
    }
    map_manager_error(err)
}

pub(super) fn map_agent_lookup_error(err: anyhow::Error) -> Status {
    if err
        .downcast_ref::<sqlx::Error>()
        .is_some_and(|cause| matches!(cause, sqlx::Error::RowNotFound))
    {
        return Status::not_found("agent not found");
    }
    map_manager_error(err)
}

pub(super) fn parse_json_as<T>(raw: &str, field: &str) -> Result<T, Status>
where
    T: serde::de::DeserializeOwned,
{
    let raw = required_field(raw, field)?;
    parse_json_str_as(raw, field)
}

pub(super) fn parse_json_str_as<T>(raw: &str, field: &str) -> Result<T, Status>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(raw)
        .map_err(|err| Status::invalid_argument(format!("{field} must be valid JSON: {err}")))
}

pub(super) fn optional_json_object(
    raw: Option<&str>,
    field: &str,
) -> Result<Option<Value>, Status> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let parsed: Value = serde_json::from_str(raw)
        .map_err(|err| Status::invalid_argument(format!("{field} must be valid JSON: {err}")))?;
    if !parsed.is_object() {
        return Err(Status::invalid_argument(format!(
            "{field} must be a JSON object"
        )));
    }
    Ok(Some(parsed))
}

pub(super) fn map_serde_status(err: serde_json::Error) -> Status {
    Status::internal(err.to_string())
}

pub(super) fn map_manager_error(err: anyhow::Error) -> Status {
    if err
        .downcast_ref::<sqlx::Error>()
        .is_some_and(|cause| matches!(cause, sqlx::Error::RowNotFound))
    {
        return Status::not_found("target record not found");
    }
    Status::internal(err.to_string())
}

pub(super) fn agent_event_record(event: crate::agent::AgentEvent) -> AgentEventRecord {
    AgentEventRecord {
        event_id: event.event_id,
        agent_id: event.agent_id,
        session_id: event.session_id,
        seq: event.seq,
        ts: event.ts,
        stream: agent_output_stream_to_str(&event.stream).to_string(),
        message: event.message,
    }
}

pub(super) fn agent_output_stream_to_str(stream: &crate::agent::OutputStream) -> &'static str {
    match stream {
        crate::agent::OutputStream::Stdout => "stdout",
        crate::agent::OutputStream::Stderr => "stderr",
        crate::agent::OutputStream::System => "system",
        crate::agent::OutputStream::Acp => "acp",
    }
}

pub(super) fn map_actor_service_status(err: ActorServiceError) -> Status {
    match err.code {
        ActorServiceErrorCode::BadRequest | ActorServiceErrorCode::UnprocessableEntity => {
            Status::invalid_argument(err.message)
        }
        ActorServiceErrorCode::Unauthorized => Status::unauthenticated(err.message),
        ActorServiceErrorCode::Forbidden => Status::permission_denied(err.message),
        ActorServiceErrorCode::NotFound => Status::not_found(err.message),
        ActorServiceErrorCode::Conflict => Status::already_exists(err.message),
        ActorServiceErrorCode::Gone => Status::failed_precondition(err.message),
        ActorServiceErrorCode::TooManyRequests => Status::resource_exhausted(err.message),
        ActorServiceErrorCode::Internal => Status::internal(err.message),
    }
}

pub(super) fn resolve_team_leader_member_id(spec: &Value) -> Result<String, Status> {
    if let Some(leader_member_id) = spec
        .get("leader_member_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(leader_member_id.to_string());
    }

    if let Some(members) = spec.get("members").and_then(Value::as_array) {
        for member in members {
            let role = member
                .get("role")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if let Some("leader") = role
                && let Some(member_id) = member
                    .get("member_id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            {
                return Ok(member_id.to_string());
            }
        }
    }

    if let Some(entrypoint) = spec
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(entrypoint.to_string());
    }

    Err(Status::failed_precondition(
        "team spec does not define a leader (leader_member_id, members[].role == 'leader', or entrypoint)",
    ))
}

pub(super) fn collect_team_member_ids(spec: &Value) -> Result<Vec<String>, Status> {
    let members = spec
        .get("members")
        .and_then(Value::as_array)
        .ok_or_else(|| Status::failed_precondition("spec.members must be an array"))?;
    let mut out = Vec::with_capacity(members.len());
    for member in members {
        let member_id = member
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Status::failed_precondition("spec.members[].member_id is required"))?;
        out.push(member_id.to_string());
    }
    Ok(out)
}

pub(super) fn normalize_permission(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub(super) fn default_permissions_for_role(role: InternalRole) -> Vec<String> {
    match role {
        InternalRole::Leader => vec![
            InternalAction::MessageSend.as_str().to_string(),
            InternalAction::InboxList.as_str().to_string(),
            InternalAction::MessageAck.as_str().to_string(),
            InternalAction::TeamRead.as_str().to_string(),
            InternalAction::TeamTaskWrite.as_str().to_string(),
            InternalAction::TimeTriggerManage.as_str().to_string(),
            InternalAction::PermissionReview.as_str().to_string(),
            InternalAction::StepTransition.as_str().to_string(),
            InternalAction::NodeIssue.as_str().to_string(),
        ],
        InternalRole::Worker => vec![
            InternalAction::MessageSend.as_str().to_string(),
            InternalAction::InboxList.as_str().to_string(),
            InternalAction::MessageAck.as_str().to_string(),
            InternalAction::TeamRead.as_str().to_string(),
            InternalAction::TeamTaskWrite.as_str().to_string(),
            InternalAction::TimeTriggerManage.as_str().to_string(),
            InternalAction::PermissionReview.as_str().to_string(),
        ],
        InternalRole::Orchestrator => vec![
            InternalAction::MessageSend.as_str().to_string(),
            InternalAction::InboxList.as_str().to_string(),
            InternalAction::MessageAck.as_str().to_string(),
            InternalAction::TeamRead.as_str().to_string(),
            InternalAction::TeamTaskWrite.as_str().to_string(),
            InternalAction::TimeTriggerManage.as_str().to_string(),
            InternalAction::PermissionReview.as_str().to_string(),
            InternalAction::StepTransition.as_str().to_string(),
            InternalAction::NodeIssue.as_str().to_string(),
        ],
    }
}

pub(super) fn validate_role_permissions(
    role: InternalRole,
    permissions: &[String],
) -> Result<(), Status> {
    if permissions.is_empty() {
        return Err(Status::invalid_argument("permissions cannot be empty"));
    }
    if role == InternalRole::Worker {
        for permission in permissions {
            if permission == "*"
                || permission == InternalAction::StepTransition.as_str()
                || permission == InternalAction::NodeIssue.as_str()
            {
                return Err(Status::invalid_argument(
                    "worker permissions cannot include wildcard/step transition/node issue actions",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn security_mode_to_str(mode: InternalGrpcSecurityMode) -> &'static str {
    match mode {
        InternalGrpcSecurityMode::Disabled => "disabled",
        InternalGrpcSecurityMode::Tls => "tls",
        InternalGrpcSecurityMode::Mtls => "mtls",
    }
}
