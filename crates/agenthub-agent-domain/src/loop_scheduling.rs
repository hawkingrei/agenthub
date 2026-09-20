//! Durable scheduling intent; execution remains governed by the activation policy.

use agenthub_team_domain::TeamTaskStatus;
use serde::{Deserialize, Serialize};

use crate::loop_runtime::{LoopSourceReferences, LoopTriggerReceipt, validate_loop_id};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LoopSchedule {
    Due {
        due_at: i64,
    },
    Recurring {
        first_at: i64,
        interval_seconds: u32,
    },
    TaskStatus {
        task_id: String,
        statuses: Vec<TeamTaskStatus>,
        repeat: bool,
    },
    ThreadReply {
        root_message_id: i64,
        after_message_id: i64,
        repeat: bool,
    },
    AppEvent {
        app_id: String,
        event_class: String,
        after_cursor: i64,
        repeat: bool,
    },
}

impl LoopSchedule {
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            Self::Due { due_at } => anyhow::ensure!(*due_at >= 0, "invalid due time"),
            Self::Recurring {
                first_at,
                interval_seconds,
            } => {
                anyhow::ensure!(*first_at >= 0, "invalid first due time");
                anyhow::ensure!(
                    (1..=86400).contains(interval_seconds),
                    "invalid bounded recurrence interval"
                );
                first_at
                    .checked_add(i64::from(*interval_seconds))
                    .ok_or_else(|| anyhow::anyhow!("recurrence deadline overflow"))?;
            }
            Self::TaskStatus {
                task_id, statuses, ..
            } => {
                validate_loop_id(task_id)?;
                anyhow::ensure!(
                    !statuses.is_empty() && statuses.len() <= 6,
                    "task condition requires bounded statuses"
                );
                for (index, status) in statuses.iter().enumerate() {
                    anyhow::ensure!(
                        !statuses[..index].contains(status),
                        "duplicate task condition status"
                    );
                }
            }
            Self::ThreadReply {
                root_message_id,
                after_message_id,
                ..
            } => {
                anyhow::ensure!(
                    *root_message_id > 0 && after_message_id >= root_message_id,
                    "invalid thread observation cursor"
                );
            }
            Self::AppEvent {
                app_id,
                event_class,
                after_cursor,
                ..
            } => {
                validate_loop_id(app_id)?;
                anyhow::ensure!(
                    crate::app_tools::valid_name(event_class),
                    "invalid app event class"
                );
                anyhow::ensure!(*after_cursor >= 0, "invalid app event observation cursor");
            }
        }
        Ok(())
    }

    pub fn repeats(&self) -> bool {
        match self {
            Self::Due { .. } => false,
            Self::Recurring { .. } => true,
            Self::TaskStatus { repeat, .. }
            | Self::ThreadReply { repeat, .. }
            | Self::AppEvent { repeat, .. } => *repeat,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopRegistrationState {
    Active,
    Completed,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopRegistrationInput {
    pub actor_id: String,
    pub team_id: String,
    pub source_key: String,
    pub schedule: LoopSchedule,
    pub work_task_id: Option<String>,
    pub references: LoopSourceReferences,
}

/// Caller intent only; authenticated entrypoints supply target scope and provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopScheduleRequest {
    pub source_key: String,
    pub schedule: LoopSchedule,
    pub work_task_id: Option<String>,
}

impl LoopScheduleRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_loop_id(&self.source_key)?;
        if let Some(id) = &self.work_task_id {
            validate_loop_id(id)?;
        }
        self.schedule.validate()
    }
}

impl LoopRegistrationInput {
    pub fn validate(&self) -> anyhow::Result<()> {
        for id in [&self.actor_id, &self.team_id, &self.source_key] {
            validate_loop_id(id)?;
        }
        if let Some(id) = &self.work_task_id {
            validate_loop_id(id)?;
        }
        self.references.validate()?;
        self.schedule.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopRegistration {
    pub id: String,
    pub input: LoopRegistrationInput,
    pub state: LoopRegistrationState,
    pub next_due_at: Option<i64>,
    pub observed_cursor: i64,
    pub pending_cursor: Option<i64>,
    pub pending_due_at: Option<i64>,
    pub next_check_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopRegistrationReceipt {
    pub registration: LoopRegistration,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopRegistrationFiring {
    pub registration_id: String,
    pub first_cursor: i64,
    pub through_cursor: i64,
    pub receipt: LoopTriggerReceipt,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopRegistrationPage {
    pub registrations: Vec<LoopRegistration>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopRegistrationDetail {
    pub registration: LoopRegistration,
    pub firings: Vec<LoopRegistrationFiring>,
    pub next_firing_cursor: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_event_conditions_accept_only_bounded_notification_references() {
        let request = serde_json::json!({"source_key":"watch","schedule":{"kind":"app_event","app_id":"app-a","event_class":"changed","after_cursor":0,"repeat":true}});
        let input: LoopScheduleRequest = serde_json::from_value(request.clone()).unwrap();
        input.validate().unwrap();
        assert!(input.schedule.repeats());
        for (field, value) in [
            ("app_id", serde_json::json!("")),
            ("event_class", serde_json::json!("class with spaces")),
            ("after_cursor", serde_json::json!(-1)),
        ] {
            let mut invalid = request.clone();
            invalid["schedule"][field] = value;
            assert!(
                serde_json::from_value::<LoopScheduleRequest>(invalid)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        for field in ["command", "payload", "references", "team_id", "actor_id"] {
            let mut invalid = request.clone();
            invalid["schedule"][field] = serde_json::json!("forged");
            assert!(serde_json::from_value::<LoopScheduleRequest>(invalid).is_err());
        }
    }

    #[test]
    fn loop_schedule_rejects_unbounded_or_ambiguous_conditions() {
        for schedule in [
            LoopSchedule::Due { due_at: -1 },
            LoopSchedule::Recurring {
                first_at: 1,
                interval_seconds: 0,
            },
            LoopSchedule::Recurring {
                first_at: i64::MAX,
                interval_seconds: 1,
            },
            LoopSchedule::TaskStatus {
                task_id: "task".into(),
                statuses: vec![],
                repeat: false,
            },
            LoopSchedule::TaskStatus {
                task_id: "task".into(),
                statuses: vec![TeamTaskStatus::Completed, TeamTaskStatus::Completed],
                repeat: true,
            },
            LoopSchedule::ThreadReply {
                root_message_id: 2,
                after_message_id: 1,
                repeat: false,
            },
        ] {
            assert!(schedule.validate().is_err(), "{schedule:?}");
        }
        LoopSchedule::TaskStatus {
            task_id: "task".into(),
            statuses: vec![TeamTaskStatus::Completed, TeamTaskStatus::Canceled],
            repeat: false,
        }
        .validate()
        .unwrap();
    }
}
