use serde_json::Value;
use sqlx::Row;

use agenthub_team_actor::{ACTOR_MAIN_PEER_ID, infer_actor_identity_kind};

use crate::team::{
    TeamActorMessageRecord, TeamActorMessageStatus, TeamActorMessageTransport,
    TeamConversationMessageRecord, TeamConversationRecord, TeamDefinitionRecord,
    TeamMemberContinuityStateRecord, TeamRunEventRecord, TeamRunRecord, TeamRunStatus,
    TeamStepRecord, TeamStepStatus, TeamTaskRecord, TeamTaskStatus,
};

pub(super) fn parse_team_definition_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<TeamDefinitionRecord> {
    let spec_json: String = row.get("spec_json");
    let spec: Value = serde_json::from_str(&spec_json)?;
    Ok(TeamDefinitionRecord {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        spec,
        owner_user_id: row.try_get("owner_user_id")?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn parse_team_run_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<TeamRunRecord> {
    let input_json: String = row.get("input_json");
    let input: Value = serde_json::from_str(&input_json)?;
    let status_raw: String = row.get("status");
    Ok(TeamRunRecord {
        id: row.get("id"),
        team_id: row.get("team_id"),
        context_id: row.get("context_id"),
        status: team_run_status_from_str(&status_raw),
        input,
        summary: None,
        created_at: row.get("created_at"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
    })
}

pub(super) fn parse_team_task_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<TeamTaskRecord> {
    let context_json: String = row.get("context_json");
    let context: Value = serde_json::from_str(&context_json)?;
    let status_raw: String = row.get("status");
    Ok(TeamTaskRecord {
        id: row.get("id"),
        team_id: row.get("team_id"),
        title: row.get("title"),
        status: team_task_status_from_str(&status_raw),
        created_by_actor_id: row.get("created_by_actor_id"),
        assigned_member_id: row.try_get("assigned_member_id")?,
        context,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn parse_team_conversation_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<TeamConversationRecord> {
    Ok(TeamConversationRecord {
        id: row.get("id"),
        team_id: row.get("team_id"),
        task_id: row.get("task_id"),
        mode: row.get("mode"),
        topic: row.try_get("topic")?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn parse_team_conversation_message_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<TeamConversationMessageRecord> {
    let payload_json: String = row.get("payload_json");
    let payload: Value = serde_json::from_str(&payload_json)?;
    Ok(TeamConversationMessageRecord {
        message_id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        task_id: row.get("task_id"),
        from_actor_id: row.get("from_actor_id"),
        to_actor_id: row.try_get("to_actor_id")?,
        route: row.get("route"),
        payload,
        created_at: row.get("created_at"),
    })
}

pub(super) fn parse_run_event_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<TeamRunEventRecord> {
    let payload_json: String = row.get("payload_json");
    let payload: Value = serde_json::from_str(&payload_json)?;
    Ok(TeamRunEventRecord {
        event_id: row.get("id"),
        run_id: row.get("run_id"),
        step_id: row.get("step_id"),
        event_type: row.get("event_type"),
        ts: row.get("ts"),
        payload,
    })
}

pub(super) fn parse_team_actor_message_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<TeamActorMessageRecord> {
    let route = row
        .try_get::<Option<String>, _>("route_json")?
        .map(|raw| serde_json::from_str::<Value>(&raw))
        .transpose()?;
    let payload_json: String = row.get("payload_json");
    let payload: Value = serde_json::from_str(&payload_json)?;
    let transport_raw: String = row.get("transport");
    let status_raw: String = row.get("status");
    let from_actor_id: String = row.get("from_actor_id");
    let from_peer_id: String = row
        .try_get("from_peer_id")
        .unwrap_or_else(|_| ACTOR_MAIN_PEER_ID.to_string());
    let to_actor_id: String = row.get("to_actor_id");
    let to_peer_id: String = row
        .try_get("to_peer_id")
        .unwrap_or_else(|_| ACTOR_MAIN_PEER_ID.to_string());
    Ok(TeamActorMessageRecord {
        message_id: row.get("id"),
        run_id: row.get("run_id"),
        from_actor_id: from_actor_id.clone(),
        from_peer_id,
        from_actor_kind: infer_actor_identity_kind(from_actor_id.as_str()),
        to_actor_id: to_actor_id.clone(),
        to_peer_id,
        to_actor_kind: infer_actor_identity_kind(to_actor_id.as_str()),
        channel: row.get("channel"),
        transport: team_actor_message_transport_from_str(&transport_raw),
        route,
        payload,
        status: team_actor_message_status_from_str(&status_raw),
        created_at: row.get("created_at"),
        delivered_at: row.try_get("delivered_at")?,
    })
}

#[allow(dead_code)]
pub(super) fn parse_team_member_continuity_state_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<TeamMemberContinuityStateRecord> {
    let history_window_json: String = row.get("history_window_json");
    let history_window: Value = serde_json::from_str(&history_window_json)?;
    Ok(TeamMemberContinuityStateRecord {
        team_id: row.get("team_id"),
        member_id: row.get("member_id"),
        source_run_id: row.get("source_run_id"),
        source_session_id: row.try_get("source_session_id")?,
        summary_text: row.get("summary_text"),
        history_window,
        updated_at: row.get("updated_at"),
    })
}

#[allow(dead_code)]
pub(super) fn parse_team_step_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<TeamStepRecord> {
    let status_raw: String = row.get("status");
    let depends_on_json: String = row.get("depends_on_json");
    let depends_on: Vec<String> = serde_json::from_str(&depends_on_json)?;
    let input = row
        .try_get::<Option<String>, _>("input_json")?
        .map(|raw| serde_json::from_str::<Value>(&raw))
        .transpose()?;
    let output = row
        .try_get::<Option<String>, _>("output_json")?
        .map(|raw| serde_json::from_str::<Value>(&raw))
        .transpose()?;
    Ok(TeamStepRecord {
        id: row.get("id"),
        run_id: row.get("run_id"),
        step_key: row.get("step_key"),
        member_id: row.get("member_id"),
        runtime_handle_id: row.try_get("remote_task_id")?,
        status: team_step_status_from_str(&status_raw),
        attempt: row.get("attempt"),
        depends_on,
        input,
        output,
        error_text: row.try_get("error_text")?,
        started_at: row.try_get("started_at")?,
        ended_at: row.try_get("ended_at")?,
    })
}

pub(super) fn team_run_status_to_str(status: &TeamRunStatus) -> &'static str {
    match status {
        TeamRunStatus::Submitted => "submitted",
        TeamRunStatus::Working => "working",
        TeamRunStatus::InputRequired => "input_required",
        TeamRunStatus::Completed => "completed",
        TeamRunStatus::Failed => "failed",
        TeamRunStatus::Canceled => "canceled",
    }
}

pub(super) fn team_task_status_to_str(status: &TeamTaskStatus) -> &'static str {
    match status {
        TeamTaskStatus::Open => "open",
        TeamTaskStatus::InProgress => "in_progress",
        TeamTaskStatus::InReview => "in_review",
        TeamTaskStatus::Completed => "completed",
        TeamTaskStatus::Canceled => "canceled",
    }
}

pub(super) fn team_task_status_from_str(raw: &str) -> TeamTaskStatus {
    match raw {
        "in_progress" => TeamTaskStatus::InProgress,
        "in_review" => TeamTaskStatus::InReview,
        "completed" => TeamTaskStatus::Completed,
        "canceled" => TeamTaskStatus::Canceled,
        _ => TeamTaskStatus::Open,
    }
}

pub(super) fn team_run_status_from_str(status: &str) -> TeamRunStatus {
    match status {
        "submitted" => TeamRunStatus::Submitted,
        "working" => TeamRunStatus::Working,
        "input_required" => TeamRunStatus::InputRequired,
        "completed" => TeamRunStatus::Completed,
        "failed" => TeamRunStatus::Failed,
        "canceled" => TeamRunStatus::Canceled,
        other => {
            tracing::warn!(
                "unknown team run status '{}', defaulting to submitted",
                other
            );
            TeamRunStatus::Submitted
        }
    }
}

pub(super) fn team_actor_message_transport_to_str(
    transport: &TeamActorMessageTransport,
) -> &'static str {
    match transport {
        TeamActorMessageTransport::Local => "local",
        TeamActorMessageTransport::Remote => "remote",
    }
}

pub(super) fn team_actor_message_transport_from_str(raw: &str) -> TeamActorMessageTransport {
    match raw {
        "remote" => TeamActorMessageTransport::Remote,
        _ => TeamActorMessageTransport::Local,
    }
}

pub(super) fn team_actor_message_status_to_str(status: &TeamActorMessageStatus) -> &'static str {
    match status {
        TeamActorMessageStatus::Pending => "pending",
        TeamActorMessageStatus::Delivered => "delivered",
        TeamActorMessageStatus::DeadLetter => "dead_letter",
    }
}

pub(super) fn team_actor_message_status_from_str(raw: &str) -> TeamActorMessageStatus {
    match raw {
        "delivered" => TeamActorMessageStatus::Delivered,
        "dead_letter" => TeamActorMessageStatus::DeadLetter,
        _ => TeamActorMessageStatus::Pending,
    }
}

#[allow(dead_code)]
pub(super) fn team_step_status_to_str(status: &TeamStepStatus) -> &'static str {
    match status {
        TeamStepStatus::Submitted => "submitted",
        TeamStepStatus::Working => "working",
        TeamStepStatus::InputRequired => "input_required",
        TeamStepStatus::Completed => "completed",
        TeamStepStatus::Failed => "failed",
        TeamStepStatus::Canceled => "canceled",
    }
}

#[allow(dead_code)]
pub(super) fn team_step_status_from_str(status: &str) -> TeamStepStatus {
    match status {
        "working" => TeamStepStatus::Working,
        "input_required" => TeamStepStatus::InputRequired,
        "completed" => TeamStepStatus::Completed,
        "failed" => TeamStepStatus::Failed,
        "canceled" => TeamStepStatus::Canceled,
        _ => TeamStepStatus::Submitted,
    }
}
