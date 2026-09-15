//! Durable loop identity and policy, independent of provider process lifetime.

use serde::{Deserialize, Serialize};

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $(#[serde(rename = $value)] $variant),+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value),+ }
            }
        }

        impl std::str::FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($value => Ok(Self::$variant)),+,
                    _ => anyhow::bail!(concat!("invalid ", stringify!($name))),
                }
            }
        }
    };
}

string_enum!(LoopPolicyState {
    Disabled => "disabled", Enabled => "enabled", Suspended => "suspended"
});
string_enum!(LoopSessionPolicy { Fresh => "fresh", Resume => "resume" });
string_enum!(LoopActivationState {
    Pending => "pending", Starting => "starting", Running => "running",
    Finalizing => "finalizing", Finished => "finished", Interrupted => "interrupted",
    Canceled => "canceled"
});
string_enum!(LoopOutcomeKind {
    Progress => "progress", Handoff => "handoff", Waiting => "waiting",
    NoActionableWork => "no_actionable_work", CompletionProposed => "completion_proposed"
});
string_enum!(LoopTriggerKind {
    Operator => "operator", Message => "message", Assignment => "assignment",
    Continuation => "continuation", Dependency => "dependency", Scheduled => "scheduled",
    MemberRequest => "member_request", AppEvent => "app_event"
});
string_enum!(LoopEventKind {
    TriggerAccepted => "trigger_accepted", Admitted => "admitted", Deferred => "deferred",
    LaunchResolved => "launch_resolved", Running => "running", OutcomeRecorded => "outcome_recorded",
    CleanupVerified => "cleanup_verified", Interrupted => "interrupted", Canceled => "canceled",
    ToolCompleted => "tool_completed"
});
string_enum!(LoopDeferralReason {
    Disabled => "disabled", Suspended => "suspended", NotDue => "not_due",
    Reserved => "reserved", StartupLimit => "startup_limit", NoProgressLimit => "no_progress_limit",
    LeaseExpiredUnfenced => "lease_expired_unfenced",
    ActorRateLimit => "actor_rate_limit", TeamRateLimit => "team_rate_limit",
    TaskOwnedElsewhere => "task_owned_elsewhere", MembershipChanged => "membership_changed"
});

string_enum!(LoopWaitReason {
    Input => "input", Dependency => "dependency", Permission => "permission",
    Knowledge => "knowledge", ExternalEvent => "external_event", DueTime => "due_time"
});

/// A selected canonical note is evidence of recorded work, not task acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopOutcome {
    pub kind: LoopOutcomeKind,
    pub wait_reason: Option<LoopWaitReason>,
    pub task_note_id: Option<i64>,
    pub continuation: Option<LoopContinuation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopContinuation {
    pub due_at: i64,
    pub task_id: Option<String>,
}

/// Effective launch references. Arguments, environment values, prompt bodies, and credentials
/// are deliberately excluded from this inspectable record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopLaunchSnapshot {
    pub version: u32,
    pub provider_id: String,
    pub configuration_digest: String,
    pub entry_prompt_version: String,
    pub session_policy: LoopSessionPolicy,
    pub workspace: String,
    pub model: Option<String>,
    pub thinking_level: Option<String>,
}

impl LoopLaunchSnapshot {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.version == 1, "unsupported loop launch version");
        validate_loop_id(&self.provider_id)?;
        validate_loop_id(&self.entry_prompt_version)?;
        anyhow::ensure!(
            self.configuration_digest.len() == 64
                && self
                    .configuration_digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit()),
            "invalid configuration digest"
        );
        anyhow::ensure!(
            !self.workspace.is_empty()
                && self.workspace.len() <= 4096
                && !self.workspace.chars().any(char::is_control),
            "invalid launch workspace"
        );
        for value in [self.model.as_deref(), self.thinking_level.as_deref()]
            .into_iter()
            .flatten()
        {
            anyhow::ensure!(
                !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control),
                "invalid launch profile reference"
            );
        }
        Ok(())
    }
}

impl LoopOutcome {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (self.kind == LoopOutcomeKind::Waiting) == self.wait_reason.is_some(),
            "waiting requires an explicit wait reason"
        );
        if let Some(id) = self.task_note_id {
            anyhow::ensure!(id > 0, "invalid task note reference");
        }
        if let Some(continuation) = &self.continuation {
            anyhow::ensure!(continuation.due_at >= 0, "invalid continuation deadline");
            if let Some(task_id) = &continuation.task_id {
                validate_loop_id(task_id)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopFinishReceipt {
    pub activation_id: String,
    pub generation: i64,
    pub continuation: Option<LoopTriggerReceipt>,
}

/// Only a trusted runtime may report this after stopping its supervised process tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopCleanupDisposition {
    Exited,
    StartupFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopReservation {
    pub actor_id: String,
    pub team_id: String,
    pub activation_id: Option<String>,
    pub generation: i64,
    pub owner_id: String,
    pub lease_expires_at: i64,
    pub lease_seconds: u32,
    pub renewal_seconds: u32,
    pub session_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopAdmission {
    Admitted(LoopReservation),
    Deferred(LoopDeferralReason),
    NotPending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopLimits {
    pub pending_per_actor: u32,
    pub pending_per_team: u32,
    pub sources_per_activation: u32,
    pub lease_seconds: u32,
    pub renewal_seconds: u32,
    pub startup_attempts: u32,
    pub retry_initial_seconds: u32,
    pub retry_max_seconds: u32,
    pub consecutive_no_progress: u32,
    pub window_seconds: u32,
    pub activations_per_actor: u32,
    pub activations_per_team: u32,
    pub standing_per_actor: u32,
    pub standing_per_team: u32,
}

impl Default for LoopLimits {
    fn default() -> Self {
        Self {
            pending_per_actor: 32,
            pending_per_team: 256,
            sources_per_activation: 64,
            lease_seconds: 60,
            renewal_seconds: 15,
            startup_attempts: 5,
            retry_initial_seconds: 1,
            retry_max_seconds: 60,
            consecutive_no_progress: 3,
            window_seconds: 900,
            activations_per_actor: 12,
            activations_per_team: 120,
            standing_per_actor: 16,
            standing_per_team: 128,
        }
    }
}

impl LoopLimits {
    pub fn startup_retry_seconds(&self, completed_attempts: u32) -> u32 {
        self.retry_initial_seconds
            .saturating_mul(1_u32 << completed_attempts.saturating_sub(1).min(31))
            .min(self.retry_max_seconds)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        for value in [
            self.pending_per_actor,
            self.pending_per_team,
            self.sources_per_activation,
            self.lease_seconds,
            self.renewal_seconds,
            self.startup_attempts,
            self.retry_initial_seconds,
            self.retry_max_seconds,
            self.consecutive_no_progress,
            self.window_seconds,
            self.activations_per_actor,
            self.activations_per_team,
            self.standing_per_actor,
            self.standing_per_team,
        ] {
            anyhow::ensure!(
                (1..=86_400).contains(&value),
                "loop limit must be between 1 and 86400"
            );
        }
        anyhow::ensure!(
            self.renewal_seconds < self.lease_seconds,
            "renewal must precede lease expiry"
        );
        anyhow::ensure!(
            self.retry_initial_seconds <= self.retry_max_seconds,
            "invalid retry range"
        );
        anyhow::ensure!(
            self.pending_per_actor <= self.pending_per_team,
            "invalid pending limits"
        );
        anyhow::ensure!(
            self.activations_per_actor <= self.activations_per_team,
            "invalid activation limits"
        );
        anyhow::ensure!(
            self.standing_per_actor <= self.standing_per_team,
            "invalid standing limits"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopPolicy {
    pub actor_id: String,
    pub team_id: String,
    pub state: LoopPolicyState,
    pub session_policy: LoopSessionPolicy,
    pub revision: i64,
    pub mailbox_run_id: Option<String>,
    pub limits: LoopLimits,
    pub generation: i64,
    pub no_progress_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// References only: no message text, prompt, tool payload, or credentials.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopSourceReferences {
    pub task_id: Option<String>,
    pub mailbox_message_id: Option<i64>,
    pub conversation_message_id: Option<i64>,
    pub thread_id: Option<i64>,
    pub scheduling_actor_id: Option<String>,
    pub scheduling_activation_id: Option<String>,
    pub app_id: Option<String>,
}

impl LoopSourceReferences {
    pub fn validate(&self) -> anyhow::Result<()> {
        for value in [
            self.task_id.as_deref(),
            self.scheduling_actor_id.as_deref(),
            self.scheduling_activation_id.as_deref(),
            self.app_id.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            validate_loop_id(value)?;
        }
        for value in [
            self.mailbox_message_id,
            self.conversation_message_id,
            self.thread_id,
        ]
        .into_iter()
        .flatten()
        {
            anyhow::ensure!(value > 0, "message references must be positive");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopTriggerInput {
    pub actor_id: String,
    pub team_id: String,
    pub kind: LoopTriggerKind,
    pub source_key: String,
    pub due_at: Option<i64>,
    #[serde(default)]
    pub references: LoopSourceReferences,
}

impl LoopTriggerInput {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_loop_id(&self.actor_id)?;
        validate_loop_id(&self.team_id)?;
        validate_loop_id(&self.source_key)?;
        anyhow::ensure!(
            self.due_at.is_none_or(|value| value >= 0),
            "invalid trigger time"
        );
        self.references.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopTriggerReceipt {
    pub trigger_id: String,
    pub activation_id: String,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopTriggerRecord {
    pub id: String,
    pub activation_id: String,
    pub input: LoopTriggerInput,
    pub created_at: i64,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopActivation {
    pub id: String,
    pub actor_id: String,
    pub team_id: String,
    pub state: LoopActivationState,
    pub due_at: i64,
    pub next_admission_at: i64,
    pub policy_revision: i64,
    pub generation: i64,
    pub attempt_count: i64,
    pub mailbox_run_id: Option<String>,
    pub session_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub finished_at: Option<i64>,
    pub outcome: Option<LoopOutcome>,
    pub launch: Option<LoopLaunchSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopEvent {
    pub id: i64,
    pub activation_id: String,
    pub kind: LoopEventKind,
    pub generation: i64,
    pub trigger_id: Option<String>,
    pub reason: Option<LoopDeferralReason>,
    pub created_at: i64,
}

pub fn validate_loop_id(value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !value.is_empty()
            && value.len() <= 256
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:/@".contains(&byte)),
        "loop reference must be a bounded identifier"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_limits_reject_unbounded_or_inconsistent_values() {
        let defaults = LoopLimits::default();
        assert!(defaults.validate().is_ok());
        let mut limits = defaults.clone();
        limits.pending_per_actor = 0;
        assert!(limits.validate().is_err());
        limits = defaults.clone();
        limits.renewal_seconds = limits.lease_seconds;
        assert!(limits.validate().is_err());
        limits = defaults;
        limits.activations_per_actor = limits.activations_per_team + 1;
        assert!(limits.validate().is_err());
    }

    #[test]
    fn loop_references_accept_identifiers_but_not_payload_text() {
        assert!(validate_loop_id("task:0195-a_b/next").is_ok());
        for value in ["", "secret payload", "task\nnext", "{\"prompt\":\"text\"}"] {
            assert!(validate_loop_id(value).is_err());
        }
    }

    #[test]
    fn loop_startup_retry_caps_delay_without_overflow() {
        let limits = LoopLimits::default();
        assert_eq!(limits.startup_retry_seconds(1), 1);
        assert_eq!(limits.startup_retry_seconds(2), 2);
        assert_eq!(limits.startup_retry_seconds(6), 32);
        assert_eq!(limits.startup_retry_seconds(7), 60);
        assert_eq!(limits.startup_retry_seconds(u32::MAX), 60);
    }
}
