use crate::team::{
    TeamActorMessageStatus, TeamActorMessageTransport, TeamRunStatus, TeamStepStatus,
    TeamTaskPriority, TeamTaskStatus,
};

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
        TeamTaskStatus::Waiting => "waiting",
        TeamTaskStatus::InReview => "in_review",
        TeamTaskStatus::Completed => "completed",
        TeamTaskStatus::Canceled => "canceled",
    }
}

pub(super) fn team_task_status_from_str(raw: &str) -> TeamTaskStatus {
    match raw {
        "in_progress" => TeamTaskStatus::InProgress,
        "waiting" => TeamTaskStatus::Waiting,
        "in_review" => TeamTaskStatus::InReview,
        "completed" => TeamTaskStatus::Completed,
        "canceled" => TeamTaskStatus::Canceled,
        _ => TeamTaskStatus::Open,
    }
}

pub(super) fn team_task_priority_to_str(priority: &TeamTaskPriority) -> &'static str {
    priority.as_str()
}

pub(super) fn team_task_priority_from_str(raw: &str) -> TeamTaskPriority {
    raw.parse::<TeamTaskPriority>().unwrap_or_default()
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
