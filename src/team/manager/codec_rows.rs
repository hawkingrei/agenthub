use serde_json::Value;
use sqlx::Row;

use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, infer_actor_identity_kind, parse_actor_message_handling_disposition,
    parse_actor_message_kind,
};

use crate::team::{
    TeamActorMessageRecord, TeamConversationMessageRecord, TeamConversationRecord,
    TeamDefinitionRecord, TeamMemberContinuityStateRecord, TeamRunEventRecord, TeamRunRecord,
    TeamTaskNoteRecord, TeamTaskRecord,
};

use super::TeamStepRecord;
use super::codec_status::{
    team_actor_message_status_from_str, team_actor_message_transport_from_str,
    team_run_status_from_str, team_step_status_from_str, team_task_priority_from_str,
    team_task_status_from_str,
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
        priority: team_task_priority_from_str(&row.get::<String, _>("priority")),
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

/// Parse a `team_conversation_messages` row, returning the record alongside whether its body was moved
/// into the tiered body store. For a moved row the `payload` is a [`Value::Null`] placeholder that the
/// caller must rehydrate (see `conversation_body`); moved-ness is decided purely by the stored sentinel
/// string, so a real payload can never be mistaken for a moved body.
pub(super) fn parse_team_conversation_message_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<(TeamConversationMessageRecord, bool)> {
    let payload_json: String = row.get("payload_json");
    let body_moved = super::conversation_body::conversation_payload_was_moved(&payload_json);
    let payload: Value = if body_moved {
        Value::Null
    } else {
        serde_json::from_str(&payload_json)?
    };
    Ok((
        TeamConversationMessageRecord {
            message_id: row.get("id"),
            conversation_id: row.get("conversation_id"),
            task_id: row.get("task_id"),
            group_id: row.try_get("group_id").ok().flatten(),
            from_actor_id: row.get("from_actor_id"),
            to_actor_id: row.try_get("to_actor_id")?,
            route: row.get("route"),
            payload,
            created_at: row.get("created_at"),
        },
        body_moved,
    ))
}

pub(super) fn parse_team_task_note_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<TeamTaskNoteRecord> {
    Ok(TeamTaskNoteRecord {
        message_id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        task_id: row.get("task_id"),
        from_actor_id: row.get("from_actor_id"),
        kind: row
            .get::<String, _>("kind")
            .parse()
            .map_err(anyhow::Error::msg)?,
        text: row.get("text"),
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
    let message_kind_raw: String = row.try_get("message_kind").unwrap_or_default();
    let handling_disposition_raw: String = row.try_get("handling_disposition").unwrap_or_default();
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
        idempotency_key: row.try_get("idempotency_key").unwrap_or(None),
        message_kind: parse_actor_message_kind(&message_kind_raw),
        status: team_actor_message_status_from_str(&status_raw),
        handling_disposition: parse_actor_message_handling_disposition(&handling_disposition_raw),
        handled_by_actor_id: row.try_get("handled_by_actor_id").unwrap_or(None),
        thread_topic_key: None,
        thread_claim_status: None,
        thread_owner_actor_id: None,
        thread_lease_expires_at: None,
        linked_task_id: None,
        linked_task_relation: None,
        handled_at: row.try_get("handled_at").unwrap_or(None),
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
